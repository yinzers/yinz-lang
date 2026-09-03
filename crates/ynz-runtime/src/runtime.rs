/// Tokio-backed scheduler runtime for Yinz compiled binaries.
///
/// C-ABI entry points exported from this module:
///   - `ynz_rt_init()` — create the Tokio runtime at program start
///   - `ynz_rt_spawn_blocking(fn_ptr, ctx_ptr, ctx_size)` — blocking-pool background task
///   - `ynz_rt_spawn(resume_fn, frame_ptr, frame_size, rec_slot, arg_drops, arg_drop_count)` — I/O-pool state-machine task (M2/M3b)
///   - `ynz_rt_async_sleep_create(ms)` — allocate a boxed Tokio Sleep future (M2)
///   - `ynz_rt_async_sleep_poll(handle_ptr, waker_ctx)` — poll an in-flight sleep (M2)
///   - `ynz_rt_run_entrypoint(resume_fn, frame_ptr, frame_size)` — program-entry state-machine driver (M2)
///   - `ynz_rt_check_preempt()` — cooperative yield point at loop back-edges + call sites
///   - `ynz_rt_shutdown()` — drain the runtime at program end
///   - `ynz_rt_spawn_blocking_joinable(fn_ptr, ctx_ptr, ctx_size)` — blocking-pool CPU task returning a joinable handle (M3d)
///   - `ynz_rt_join_poll(handle, waker_ctx, result_out)` — poll a CPU join handle (M3d)
///   - `ynz_rt_join_handle_free(handle)` — detach/drop a CPU join handle (M3d)
///
/// These are called by compiler-generated code; users never see Tokio types directly.
///
/// # Panic safety
///
/// Every background task body is wrapped in `std::panic::catch_unwind`. A panicking
/// background task logs via `eprintln!` and is silently discarded; it never propagates
/// to the spawning scope. The heap frame uses a RAII drop guard (`FrameDropGuard`) that
/// runs on both the happy path AND on unwind, preventing frame leaks even when a task panics.
use std::future::Future;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::time::Sleep;

// The runtime is stored in a process-global `Mutex<Option<Runtime>>`.
// `Mutex` gives `ynz_rt_shutdown` the ability to `take()` the runtime for
// `shutdown_timeout` (requires ownership). `OnceLock` initialises the Mutex
// exactly once; `ynz_rt_init` populates the `Option` on first call and can
// repopulate it after `ynz_rt_shutdown` — enabling test-harness reuse.
//
// SAFETY: `Mutex<Option<Runtime>>` is `Send + Sync`.
static RUNTIME: OnceLock<Mutex<Option<tokio::runtime::Runtime>>> = OnceLock::new();

// Count of blocking-pool tasks that have been handed to `handle.spawn_blocking` but have
// not yet finished running. `ynz_rt_shutdown` drains this to zero BEFORE calling Tokio's
// `shutdown_timeout`, closing a confirmed task-loss race: Tokio's own doc comment on
// `spawn_blocking` says such tasks are scheduled as "non-mandatory, meaning they may not
// get executed in case of runtime shutdown" — any task still sitting in the blocking
// pool's internal queue (handed off but not yet popped by a worker thread) at the exact
// instant `shutdown_timeout` flips the pool's shutdown flag is silently CANCELLED without
// ever running its closure, even though `ynz_rt_shutdown`'s own contract promises
// in-flight tasks up to `timeout_ms` to finish. Confirmed live under ThreadSanitizer (M6
// Phase 6b): 100 rapid-fire `ynz_rt_spawn_blocking` calls immediately followed by
// `ynz_rt_shutdown()` lost up to 20% of tasks across repeated runs — no error, no log,
// simply never executed (see `PendingBlockingGuard` below for the increment/decrement
// discipline; see `ynz_rt_shutdown` for the drain-wait).
static PENDING_BLOCKING_TASKS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

// Set to `true` by `ynz_rt_shutdown` as its very FIRST action (before it even locks
// `RUNTIME`), reset to `false` by `ynz_rt_init` (test-harness reuse across an
// init/shutdown/init cycle — mirrors `RUNTIME`'s own re-populate-after-shutdown
// contract). This closes a SECOND race the plain drain-loop-then-shutdown_timeout
// sequence did not: `PENDING_BLOCKING_TASKS` draining to zero is a SNAPSHOT, not a
// lock — nothing stops a task still executing inside the runtime (e.g. a
// fire-and-forget task `ynz_rt_shutdown` itself promises time to finish) from calling
// `ynz_rt_spawn_blocking`/`_joinable` in the exact window AFTER the drain loop
// observes zero and BEFORE `shutdown_timeout` begins tearing the pool down — both
// spawn entry points resolve their Tokio `Handle` via `Handle::try_current()`, which
// succeeds purely from Tokio's own thread-local task context and is completely
// independent of the `RUNTIME` mutex `ynz_rt_shutdown` already emptied. A new task
// admitted in that window re-opens the EXACT silent-task-loss bug
// `PENDING_BLOCKING_TASKS` was built to close. See `PendingBlockingGuard::try_new`
// for the increment-then-check protocol that closes this window, and
// `m6_shutdown_admission_race.rs` for the regression test.
static SHUTDOWN_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// RAII guard: increments `PENDING_BLOCKING_TASKS` when successfully admitted, decrements
/// on drop. Constructed on the SPAWNING thread via `try_new()` (before
/// `handle.spawn_blocking` is even called), then moved into the closure — and dropped at
/// the closure's FIRST line (see `drop(_pending)` in `ynz_rt_spawn_blocking` and
/// `ynz_rt_spawn_blocking_joinable`), not at the closure's end. This means the count
/// covers only the "queued, not yet STARTED" window — the actual vulnerability window for
/// Tokio's non-mandatory `spawn_blocking` cancellation — and deliberately does NOT extend
/// across the task body's own execution time; holding it for the whole closure lifetime
/// was tried and reverted (M6 Phase 6b fix-loop) because it starved `shutdown_timeout`'s
/// own remaining budget on long-running task bodies (see the `drop(_pending)` call sites'
/// doc comments for the confirmed regression). Decrements on every exit path — normal
/// return AND unwind (`std::panic::resume_unwind`, used by
/// `ynz_rt_spawn_blocking_joinable`) — because Rust runs `Drop` for live locals on both
/// paths.
struct PendingBlockingGuard;

impl PendingBlockingGuard {
    /// Attempts to register a new pending blocking task. Returns `None` if
    /// `ynz_rt_shutdown` has already begun — the caller MUST NOT spawn the task in that
    /// case (log + discard, matching the existing "called after ynz_rt_shutdown"
    /// behavior for the other admission failure modes). Call from the SPAWNING thread,
    /// immediately before `handle.spawn_blocking`; on `Some`, move the returned guard
    /// INTO the closure so it decrements when the closure exits.
    ///
    /// # Race-closing protocol — increment-then-check, never check-then-increment
    /// A naive "check `SHUTDOWN_STARTED`, then increment only if false" ordering
    /// reopens the exact TOCTOU this guard exists to close: a task could observe
    /// `SHUTDOWN_STARTED == false`, then `ynz_rt_shutdown`'s drain loop could observe
    /// `PENDING_BLOCKING_TASKS == 0` (the increment hasn't happened yet) and proceed
    /// straight into `shutdown_timeout` — racing the task's later increment + spawn.
    ///
    /// This ALWAYS increments first (advertising intent to spawn) and only THEN checks
    /// the flag. Both atomics use `SeqCst`, which gives every `SeqCst` operation across
    /// every thread one single global total order consistent with each thread's own
    /// program order. Let `I` = this increment, `F_write` = `ynz_rt_shutdown`'s flag
    /// store, `F_read` = this function's flag load, and `C_read` = any iteration of the
    /// drain loop's counter load:
    /// - If `I` precedes `F_write` in the total order: `F_write` precedes every
    ///   `C_read` in program order (the store is the drain loop's first line), so
    ///   `I` precedes every `C_read` too — the drain loop cannot observe zero (and so
    ///   cannot proceed to `shutdown_timeout`) until this task's guard drops, whether
    ///   `F_read` (which follows `I` in program order) lands before or after `F_write`.
    ///   Either way the task is safely admitted or safely told to retreat before it is
    ///   ever handed to Tokio.
    /// - If `F_write` precedes `I`: then `F_write` precedes `I` precedes `F_read` (the
    ///   latter by program order), so `F_read` observes `true` — the increment is
    ///   undone immediately and the caller discards the task, which is therefore NEVER
    ///   handed to Tokio and can never become the silently-cancelled non-mandatory task
    ///   this whole mechanism exists to prevent.
    fn try_new() -> Option<Self> {
        PENDING_BLOCKING_TASKS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if SHUTDOWN_STARTED.load(std::sync::atomic::Ordering::SeqCst) {
            // Undo the advertisement — shutdown has already been signaled, so this task
            // must never reach `handle.spawn_blocking`.
            PENDING_BLOCKING_TASKS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            None
        } else {
            Some(Self)
        }
    }
}

impl Drop for PendingBlockingGuard {
    fn drop(&mut self) {
        PENDING_BLOCKING_TASKS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

// General SM frame-header offsets (`FRAME_OFFSET_SLEEP_HANDLE`, `FRAME_OFFSET_RETURN_SLOT`) and
// the spike-frame layout contract all live in `ynz-abi` (a dependency-free crate) so codegen and
// the runtime read the identical values without codegen depending on the runtime's tokio tree.
//
// Frame layout (mirrors `crates/ynz-codegen/src/state_machine.rs`):
//   bytes 0–3   : resume_point (i32)
//   bytes 4–7   : padding (zero for normal SM frames; spike discriminator for spike frames)
//   bytes 8–15  : sleep_handle (*mut Pin<Box<Sleep>>, or null)
//   bytes 16–31 : return_slot (16 bytes, typed return value on terminal transition)
//   bytes 32+   : local-variable slots (i64 each)
//
// `FRAME_OFFSET_SLEEP_HANDLE` is the authoritative offset used by `SpawnStateFnFuture::drop` to
// read the `sleep_handle` slot and free any in-flight `Sleep` box on task cancellation.
// `FRAME_OFFSET_RETURN_SLOT` is read by `SyncStateFnFuture::poll` to recover the typed return
// value. Both were moved into `ynz-abi` (v0.3-M3g Phase 1) alongside the spike-frame constants
// below — previously each had a same-valued shadow definition in `state_machine.rs`, held in
// sync only by a prose "must stay in sync" comment with no compile-time binding.
//
// `SPIKE_FRAME_TAG` is the high-16-bit frame tag; `SPIKE_FRAME_DISCRIMINATOR_OFFSET` is
// the byte offset of the discriminator word codegen writes and the runtime reads;
// `SPIKE_HANDLE_BASE_OFFSET` is where the contiguous CPU-handle region begins (==
// `FRAME_HEADER_SIZE`, bound by a const-assert in emit.rs); `SPIKE_HANDLE_SLOT_BYTES` is the
// per-slot stride.
//
// The discriminator is a single u32 that BOTH tags the frame as a spike frame AND carries the
// live CPU-handle count, packed as `(SPIKE_FRAME_TAG << 16) | handle_count`:
//   - high 16 bits: `SPIKE_FRAME_TAG` ("SP") — present iff this is a spike frame.
//   - low 16 bits:  the number of contiguous CPU-handle slots starting at
//     `SPIKE_HANDLE_BASE_OFFSET` that `cleanup_spike_cpu_handles` must inspect on drop.
// Packing the count into the discriminator (rather than a fixed two-slot scan) makes the
// cancellation free-path layout-driven for an arbitrary number of CPU-group members: drop reads
// the count and frees exactly that many slots. Normal (non-spike) SM frames leave bytes 4-7 as
// zero (ynz_alloc_zeroed guarantee), so the high-bits tag never false-matches.
use ynz_abi::{
    BG_ARG_KIND_HEAP_ARRAY, BG_ARG_KIND_HEAP_SHAPE, BG_ARG_KIND_RELEASED,
    BG_ARG_KIND_SHARED_CHANNEL, FRAME_OFFSET_RETURN_SLOT, FRAME_OFFSET_SLEEP_HANDLE,
    SPIKE_FRAME_DISCRIMINATOR_OFFSET, SPIKE_FRAME_TAG, SPIKE_HANDLE_BASE_OFFSET,
    SPIKE_HANDLE_SLOT_BYTES,
};

/// Extract the spike tag (high 16 bits) and handle count (low 16 bits) from a
/// discriminator word. Returns `None` when the word is not a spike discriminator.
fn decode_spike_discriminator(disc: u32) -> Option<usize> {
    if (disc >> 16) == SPIKE_FRAME_TAG {
        Some((disc & 0xFFFF) as usize)
    } else {
        None
    }
}

/// Initialise the Tokio multi-thread runtime.
///
/// # Flow
/// 1. Build a multi-thread runtime with `num_cpus::get()` worker threads.
/// 2. Store in a process-global `OnceLock<Mutex<Option<Runtime>>>` — double-init
///    is safe: if the OnceLock is already populated the second call is a no-op.
///
/// # Memory ordering
/// `OnceLock` uses acquire/release for the store/load pair. The inner `Mutex`
/// provides exclusive access for the `take()` in shutdown. Tokio's `Runtime` is
/// `Send + Sync`, so the combination is correct.
///
/// # Side effects
/// Starts OS threads. No I/O until a task is spawned.
#[no_mangle]
pub extern "C" fn ynz_rt_init() {
    // Latch both test-only env flags once here so their hot-path gates are cheap
    // atomic loads: the alloc counter (per-alloc gate) and the recursion-drop skip
    // (per-task-drop gate). Each env var read (lock + heap + UTF-8 scan) happens
    // only once at program start.
    crate::init_alloc_counter_flag();
    crate::init_skip_recursion_drop_flag();

    // Re-arm blocking-task admission for a fresh init/shutdown/init cycle (test-harness
    // reuse — mirrors `RUNTIME`'s own re-populate-after-shutdown contract). Without this
    // reset, every `ynz_rt_spawn_blocking`/`_joinable` call after the FIRST shutdown in a
    // process would be permanently rejected by `PendingBlockingGuard::try_new`.
    SHUTDOWN_STARTED.store(false, std::sync::atomic::Ordering::SeqCst);

    let mutex = RUNTIME.get_or_init(|| Mutex::new(None));
    let mut lock = mutex.lock().expect("ynz_rt_init: mutex poisoned");
    if lock.is_none() {
        // Test-only worker-count override (v0.3-M7 Phase 6): the back-edge poll-yield
        // starvation fixtures pin YNZ_WORKER_THREADS=1 so "another task on the same
        // worker" is deterministic regardless of the host's core count. Same env-latch
        // register as YNZ_ALLOC_COUNTER_OUTPUT / YNZ_SHUTDOWN_TIMEOUT_MS: read once at
        // init, production default (num_cpus) when unset or unparsable.
        let worker_threads = std::env::var("YNZ_WORKER_THREADS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n >= 1)
            .unwrap_or_else(num_cpus::get);
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(worker_threads)
            .enable_all()
            .build()
            .expect("ynz_rt_init: failed to create Tokio runtime");
        *lock = Some(rt);
    }
    // If already initialised (e.g., double-init in the same program or after shutdown
    // in a test harness that re-calls init), this is a no-op. The two flag calls
    // above are idempotent: re-reading the same env vars produces the same values.
}

/// RAII guard that frees the heap frame when dropped (normal return AND unwind).
///
/// Used by both `ynz_rt_spawn_blocking` (ctx copy) and `ynz_rt_spawn` (state-machine
/// frame). The guard holds a raw pointer + length; on drop it reconstructs the Box<[u8]>
/// and lets Rust free it, running on both the happy path and any panic unwind.
struct FrameDropGuard {
    ptr: *mut u8,
    len: usize,
    /// Test-only ctx-free probe. When this guard drops (freeing its heap buffer), the
    /// Arc counter increments. Installed only by `ynz_rt_spawn_blocking_joinable` (from a
    /// per-spawn thread-local) so the ctx-free of ONE joinable spawn is observable without
    /// affecting `ynz_rt_spawn`/`ynz_rt_spawn_blocking`. Scoped to one guard — concurrent
    /// drops from other tests cannot race into the assertion window.
    #[cfg(test)]
    free_probe: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
}

// SAFETY: FrameDropGuard wraps a heap pointer owned exclusively by one background task.
// It never leaves that task's closure, so Send is safe here.
unsafe impl Send for FrameDropGuard {}

impl Drop for FrameDropGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() && self.len > 0 {
            // SAFETY: ptr was allocated with `crate::ynz_alloc` (libc malloc — see the two
            // construction sites above). This drop runs exactly once per FrameDropGuard
            // value (captured by value into the closure).
            unsafe { crate::ynz_free(self.ptr, self.len) };
            #[cfg(test)]
            if let Some(probe) = &self.free_probe {
                probe.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
    }
}

// Test-only channel for injecting a ctx-free probe into the next
// `ynz_rt_spawn_blocking_joinable` call's ctx `FrameDropGuard`. The probe is read on the
// caller's thread at guard-construction time (before the guard moves into the blocking-pool
// closure), so the Arc is baked into the guard and survives the cross-thread move. Mirrors
// the per-handle `CpuJoinHandle` drop probe.
#[cfg(test)]
thread_local! {
    static CTX_FREE_PROBE: std::cell::RefCell<Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>> =
        const { std::cell::RefCell::new(None) };
}

/// Test-only: arm a ctx-free probe for the next `ynz_rt_spawn_blocking_joinable` call on
/// this thread. The probe fires once when that spawn's ctx heap copy is freed.
#[cfg(test)]
pub(crate) fn arm_ctx_free_probe(arc: std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    CTX_FREE_PROBE.with(|p| *p.borrow_mut() = Some(arc));
}

/// Copies `ctx_size` bytes from `ctx_ptr` onto the heap, returning `(ptr, len)` —
/// `(null, 0)` when `ctx_size <= 0` or `ctx_ptr` is null. Shared by `ynz_rt_spawn_blocking`
/// and `ynz_rt_spawn_blocking_joinable` (per authoritative-derivation.md — one source, not
/// two parallel derivations of the same ctx-copy logic).
///
/// Goes through `crate::ynz_alloc` (the ONE authoritative heap-allocation choke point —
/// same path frame/shape heap allocations use), NOT a `Vec<u8>`/`Box<[u8]>`. Two confirmed
/// bugs, live under Miri (M6 Phase 6b), in the prior `Box<[u8]>` form used independently at
/// both call sites:
///   1. `.as_mut_ptr()` then `mem::forget(buf)` retags the allocation Unique at the
///      `forget` call, invalidating the raw pointer captured beforehand (Stacked
///      Borrows) — `Box::into_raw` alone doesn't fully avoid this either once a second
///      call site copy-pastes the pattern; going through libc `malloc` (`ynz_alloc`)
///      sidesteps Box's aliasing model entirely.
///   2. `Vec<u8>`'s allocator only guarantees 1-byte (`u8`) alignment, but the ctx bytes
///      are read back through typed pointers (e.g. `*const i64`) by the caller's
///      `fn_ptr` — an alignment-8 read off an alignment-1 allocation is UB. `malloc`
///      (via `ynz_alloc`) guarantees max_align_t alignment, satisfying any field type
///      the ctx struct can contain.
///
/// # Safety
/// `ctx_ptr` must point to at least `ctx_size` valid bytes for the duration of this call
/// (same contract both callers already document). Ownership of those bytes is NOT
/// transferred; a copy is made onto the heap.
unsafe fn copy_ctx_to_heap(ctx_ptr: *mut u8, ctx_size: i64) -> (*mut u8, usize) {
    if ctx_size > 0 && !ctx_ptr.is_null() {
        let len = ctx_size as usize;
        // SAFETY: caller guarantees ctx_ptr is valid for ctx_size bytes; forwarded from
        // this function's own safety contract.
        let raw = crate::ynz_alloc(len);
        std::ptr::copy_nonoverlapping(ctx_ptr, raw, len);
        (raw, len)
    } else {
        (std::ptr::null_mut(), 0)
    }
}

/// Schedule a function on the blocking thread pool.
///
/// # Flow
/// 1. Attempt admission via `PendingBlockingGuard::try_new` — rejects (no-op discard) if
///    `ynz_rt_shutdown` has already begun, closing the shutdown-admission TOCTOU race.
/// 2. Copy `ctx_size` bytes from `ctx_ptr` to a heap-allocated buffer.
/// 3. Transfer ownership of the heap buffer into the closure via a RAII drop guard.
/// 4. Wrap `fn_ptr(ctx_heap)` in `catch_unwind` — the guard's `Drop` runs on both
///    the happy path and on unwind, so ctx is freed in both cases.
/// 5. Call `spawn_blocking` — returns immediately; the caller continues.
///
/// # Cache-line alignment note (forward-design for M4)
/// `ctx_ptr` is 64 bytes per cache line on x86_64 and ARM64. In v0.3-M1 there is no
/// padding between ctx fields; false sharing between a spawning thread that still
/// holds a live reference to the original struct and the task's copy can cause cache
/// thrashing. v0.3-M4's arena allocator will align ctx heap copies to 64 bytes.
///
/// # Safety
/// - `ctx_ptr` must point to at least `ctx_size` valid bytes for the duration of this call.
///   Ownership of those bytes is NOT transferred; a copy is made onto the heap.
/// - `fn_ptr` must be safe to call with a single `*mut u8` argument on another thread.
///
/// # Failure modes
/// - If the background task panics: caught, logged via `eprintln!`, discarded.
///   Program continues normally.
/// - If `ctx_size` == 0 or `ctx_ptr` is null: `fn_ptr(null)` is called.
/// - If `ynz_rt_shutdown` has already begun (admission closed): logs a warning, the task
///   is discarded WITHOUT being allocated or handed to Tokio.
///
/// # Side effects
/// Time: O(n) where n = ctx_size bytes (the heap ctx copy)  Space: O(n) (heap ctx buffer).
/// Spawns a Tokio blocking task; heap-allocates `ctx_size` bytes.
#[no_mangle]
pub unsafe extern "C" fn ynz_rt_spawn_blocking(
    fn_ptr: extern "C-unwind" fn(*mut u8),
    ctx_ptr: *mut u8,
    ctx_size: i64,
) {
    // Get the runtime handle without holding the RUNTIME mutex during spawn_blocking
    // (avoids deadlock when called from inside a block_on future).
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => {
            let Some(guard) = RUNTIME.get() else {
                eprintln!(
                    "ynz runtime: ynz_rt_spawn_blocking called before ynz_rt_init — task discarded"
                );
                return;
            };
            let lock = match guard.lock() {
                Ok(l) => l,
                Err(e) => e.into_inner(),
            };
            match lock.as_ref() {
                Some(rt) => rt.handle().clone(),
                None => {
                    eprintln!("ynz runtime: ynz_rt_spawn_blocking called after ynz_rt_shutdown — task discarded");
                    return;
                }
            }
        }
    };

    // Admission check BEFORE the ctx heap copy — see `PendingBlockingGuard::try_new` for
    // the increment-then-check protocol that closes the shutdown-admission TOCTOU race
    // (a task admitted after `ynz_rt_shutdown` has begun draining would be silently
    // cancelled by Tokio's non-mandatory `spawn_blocking` semantics; see
    // `SHUTDOWN_STARTED`'s doc comment). Checking here — before allocating the ctx copy —
    // means a rejected task never allocates anything that would need to be freed.
    let Some(_pending) = PendingBlockingGuard::try_new() else {
        eprintln!(
            "ynz runtime: ynz_rt_spawn_blocking called after ynz_rt_shutdown began — task discarded"
        );
        return;
    };

    // Copy the context bytes onto the heap so they outlive this stack frame.
    // SAFETY: caller guarantees ctx_ptr is valid for ctx_size bytes (this function's own
    // safety contract, forwarded to `copy_ctx_to_heap`).
    let (ctx_heap_ptr, ctx_heap_len) = copy_ctx_to_heap(ctx_ptr, ctx_size);

    // Create the RAII guard BEFORE the closure so it's captured (not the raw *mut u8).
    // FrameDropGuard: Send is impl'd; the closure becomes Send too.
    let ctx_guard = FrameDropGuard {
        ptr: ctx_heap_ptr,
        len: ctx_heap_len,
        #[cfg(test)]
        free_probe: None,
    };

    handle.spawn_blocking(move || {
        // Drop `_pending` FIRST, before touching ctx or calling `fn_ptr` — the closure
        // has now been popped off the blocking pool's internal queue and started running,
        // which per `PENDING_BLOCKING_TASKS`'s doc comment makes it immune to Tokio's
        // non-mandatory queued-task cancellation ("once a task has started, the worker
        // loop always runs it to completion regardless of the shutdown flag"). Holding
        // the guard until the WHOLE closure (including `fn_ptr`'s body) finishes over-
        // extends `ynz_rt_shutdown`'s drain-loop wait far past the actual vulnerability
        // window: a long-running task body (e.g. a multi-hundred-ms CPU burn) would keep
        // `PENDING_BLOCKING_TASKS` elevated for its own execution time, and — after the
        // drain/shutdown_timeout budget-sharing fix — that starves `shutdown_timeout`'s
        // OWN remaining budget down to near-zero, cutting off the async state-machine
        // Drop-chain cleanup that budget is separately needed for (confirmed live: this
        // exact starvation reproduced as a frame alloc/free parity regression in
        // `v03_m6_recursive_spike_cancel.rs::recursive_spike_cancellation_frame_alloc_free_parity`
        // once the shared-deadline fix landed). Dropping here narrows the drain loop back
        // to its documented low-single-digit-millisecond queue-pickup wait; the task's own
        // completion is `shutdown_timeout`'s concern via its own remaining-budget wait.
        // This narrower drop point relies on `shutdown_timeout`'s one remaining-budget
        // wait genuinely covering both the outstanding-blocking-task concern and the
        // async-task Drop-chain-cleanup concern without one starving the other inside
        // Tokio's own internals — see the Tokio-source citation on the
        // `rt.shutdown_timeout(remaining)` call in `ynz_rt_shutdown`, below, for the
        // confirmed mechanism.
        drop(_pending);

        // Guards are captured — free ctx on both return and unwind.
        let ctx_ptr_for_call = ctx_guard.ptr;
        let _guard = ctx_guard;

        let result = std::panic::catch_unwind(|| {
            fn_ptr(ctx_ptr_for_call);
        });

        if let Err(e) = result {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "<non-string panic payload>".to_string()
            };
            eprintln!("ynz runtime: background task panicked (ignored): {}", msg);
        }
    });
}

/// Back-edge polls between granted yields: the cooperative budget is a PURE CALL COUNT,
/// not a wall clock. At the ~1-15ns/poll cost of a tight state-machine loop iteration,
/// 2^20 polls lands in the ~2-15ms band — the same order as the scheduler-preemption
/// model's ~10ms quantum target for exactly the tight-hot-loop starvation shape the
/// mechanism exists to break.
///
/// WHY count, not clock (a recorded Phase 6 divergence from the design note's
/// countdown+quantum sketch): a wall-clock budget makes WHETHER a given loop yields
/// depend on run-to-run timing jitter, which makes compiled-program stdout
/// NONDETERMINISTIC across runs — live-reproduced on `examples/pirates-roster`
/// (six runs, six different output orderings) the moment the clock-based cut landed.
/// Byte-exact output goldens and the cross-mode byte-identity gate are load-bearing
/// in this repo (Phase 5 determinism proofs; the plan-invariants demo golden), so the
/// budget must be a deterministic function of program execution alone. The cost of the
/// trade: loops with heavy per-iteration bodies yield less often than a strict 10ms
/// quantum would — recorded honestly in IMP-no-function-coloring.md's preemption
/// section rather than silently drifted.
const PREEMPT_YIELD_INTERVAL: u32 = 1 << 20;

thread_local! {
    /// Per-worker poll countdown (starvation is a per-worker phenomenon; no cross-worker
    /// synchronization on the hot path). 0 is the "expired, unconsumed" latch: a plain
    /// (null-waker) caller that hits expiry cannot yield, so the expiry stays latched
    /// until a state-machine back edge consumes it.
    static PREEMPT_COUNTDOWN: std::cell::Cell<u32> =
        const { std::cell::Cell::new(PREEMPT_YIELD_INTERVAL) };
}

/// Cooperative preemption budget check — called at every loop back-edge (and, behind the
/// Phase 6 call-site toggle, at user-function call sites).
///
/// # v0.3-M7 Phase 6 — the real budget check (decides WHETHER; never yields itself)
///
/// A synchronous `extern "C"` callee cannot yield the enclosing Tokio task — the YIELD is
/// codegen's: at a state-machine loop back edge the compiler emits
/// `br (ynz_rt_check_preempt(waker_ctx)), yield_bb, header` where `yield_bb` stores the
/// resume point and returns Pending. This function only answers "has this worker burned
/// a full yield interval since it last yielded?" — and, when the answer is yes AND a
/// waker context is provided, arranges the task's wake FIRST so the Pending the codegen
/// is about to return is a yield-and-requeue, never a lost, never-woken task.
///
/// # Flow
/// 1. Fast path: decrement the thread-local countdown; while above the floor → `false`
///    (one Cell decrement + compare — no clock, no atomics; the ≤5ns bound the Phase 0
///    spike measured for the call shape holds).
/// 2. Expiry with a NULL `waker_ctx` (a plain, non-state-machine back edge — it discards
///    the result and CANNOT yield): latch the expiry (countdown 0) and return `true`;
///    the first state-machine back edge on this worker consumes the latch.
/// 3. Expiry (or latched expiry) with a non-null `waker_ctx`: reset the countdown,
///    schedule the wake, return `true`.
///
/// # The wake — off-thread by construction
/// The wake must come from OFF this worker thread. A plain self-`wake_by_ref` during the
/// task's own poll re-queues it into Tokio's LIFO slot, so the yielding task runs again
/// IMMEDIATELY after returning Pending and the queued sibling starves — the exact
/// self-wake starvation Tokio's own `task::yield_now` needed an internal defer-queue to
/// fix (tokio#5115), and which reproduced verbatim on this mechanism's first cut
/// (fixture (a): the victim only ran after the hog fully completed). Tokio's defer list
/// is not public API, so the equivalent fairness is bought by waking from a
/// blocking-pool thread: a remote wake lands in the injection queue, which the worker
/// drains only after its local queue — siblings run first.
///
/// # Safety
/// `waker_ctx` must be null OR a valid `*mut u8` pointing to a live `&mut Context<'_>`
/// for the duration of the call (the locked waker_ctx ABI every resume-fn callee uses).
///
/// # Side effects
/// Thread-local budget bookkeeping; on a granted yield, schedules the calling task's
/// wake on the blocking pool.
#[no_mangle]
pub unsafe extern "C" fn ynz_rt_check_preempt(waker_ctx: *mut u8) -> bool {
    let expired = PREEMPT_COUNTDOWN.with(|c| {
        let n = c.get();
        if n > 1 {
            c.set(n - 1);
            false
        } else {
            // n == 1 (expiring now) or n == 0 (latched by a plain caller earlier).
            true
        }
    });
    if !expired {
        return false;
    }
    if waker_ctx.is_null() {
        // Plain-loop caller: cannot yield — latch the expiry for the next SM back edge
        // on this worker. The returned bool is discarded by plain call sites.
        PREEMPT_COUNTDOWN.with(|c| c.set(0));
        return true;
    }
    PREEMPT_COUNTDOWN.with(|c| c.set(PREEMPT_YIELD_INTERVAL));
    // SAFETY: waker_ctx was cast from &mut Context<'_> by the enclosing state-machine
    // poll (StateFnFuture / SpawnStateFnFuture) — the caller guarantees liveness.
    let cx = unsafe { &mut *(waker_ctx as *mut Context<'_>) };
    let waker = cx.waker().clone();
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn_blocking(move || waker.wake());
        }
        // No runtime context (unreachable for a real SM poll, which always runs inside
        // block_on or a worker): plain wake keeps the task schedulable rather than lost.
        Err(_) => waker.wake(),
    }
    true
}

/// Shut down the Tokio runtime, draining in-flight background tasks.
///
/// # Flow
/// 1. Close blocking-task admission (`SHUTDOWN_STARTED`) — see its doc comment for the
///    increment-then-check protocol this enables in `PendingBlockingGuard::try_new`.
/// 2. Lock the runtime mutex and take ownership via `Option::take`.
/// 3. Drain `PENDING_BLOCKING_TASKS` to zero, then call `shutdown_timeout` with
///    whatever budget remains — tasks get up to `timeout_ms` (5s by default) TOTAL to
///    finish across both phases combined, not `timeout_ms` for each phase separately;
///    any remaining are dropped (Tokio semantics per
///    `tokio::runtime::Runtime::shutdown_timeout`).
/// 4. If ynz_rt_init was never called, or shutdown was already called, this is a no-op.
///
/// # Side effects
/// Joins OS threads. Blocks for up to `timeout_ms` (5s by default) TOTAL if background
/// tasks are still running — this is one shared budget across the drain-wait and
/// `shutdown_timeout` phases combined, never `timeout_ms` for each phase separately.
///
/// When `YNZ_ALLOC_COUNTER_OUTPUT` env var is set to a file path, writes final alloc/free
/// counts to that file (one "alloc=N\nfree=M\n" pair). Used by the composed-single-alloc
/// proof in integration tests: the test sets the env var, runs the fixture, reads the file.
#[no_mangle]
pub extern "C" fn ynz_rt_shutdown() {
    // Close admission FIRST, before anything else in this function — see
    // `SHUTDOWN_STARTED`'s doc comment and `PendingBlockingGuard::try_new`'s SeqCst
    // total-order proof for why this exact placement (before the drain loop even reads
    // `PENDING_BLOCKING_TASKS` for the first time) is what closes the shutdown-admission
    // TOCTOU race rather than merely narrowing it.
    SHUTDOWN_STARTED.store(true, std::sync::atomic::Ordering::SeqCst);

    let Some(guard) = RUNTIME.get() else { return };
    // Take ownership of the Runtime and RELEASE the lock before shutdown_timeout —
    // mirrors the ynz_rt_run_entrypoint pattern (:1052-1062). Holding the mutex across
    // the up-to-5s drain would block any concurrent ynz_rt_spawn_blocking / ynz_rt_spawn
    // caller (they also lock RUNTIME on their fallback path) for the full drain window
    // instead of failing fast with "called after ynz_rt_shutdown" the instant the Option
    // goes to None.
    let rt = {
        let mut lock = match guard.lock() {
            Ok(l) => l,
            Err(e) => e.into_inner(), // still usable after poisoning
        };
        lock.take()
    }; // mutex released here — before shutdown_timeout
    if let Some(rt) = rt {
        // `shutdown_timeout` requires owned `Runtime`, which we now have.
        // test-only: `YNZ_SHUTDOWN_TIMEOUT_MS` lets tests shorten the shutdown drain
        // window to trigger cancellation faster. Production default is 5000ms (5s),
        // which is the correct value for any unset env var. End-user processes that
        // don't set the env var are unaffected. Setting it in production is safe but
        // unsupported — shorter windows risk incomplete task cleanup.
        let timeout_ms = std::env::var("YNZ_SHUTDOWN_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5000); // production default: 5s

        // ONE deadline shared by both the drain-wait and shutdown_timeout phases below —
        // NOT a fresh timeout_ms window for each. Computing a single deadline up front
        // and passing the REMAINING budget to shutdown_timeout is what makes worst-case
        // total shutdown time match the documented `timeout_ms` bound (previously this
        // gave the drain loop a full timeout_ms window AND THEN gave shutdown_timeout
        // another full timeout_ms window, so worst-case shutdown was 2x timeout_ms —
        // 10s at the 5s default — contradicting the "up to 5 seconds" docstring above).
        let shutdown_deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);

        // Drain PENDING_BLOCKING_TASKS to zero BEFORE calling shutdown_timeout — see
        // PENDING_BLOCKING_TASKS's doc comment above for the confirmed-under-TSan race
        // this closes (Tokio's non-mandatory spawn_blocking silently drops any task still
        // queued, not yet popped by a worker thread, at the instant shutdown begins). Once
        // a task has started, the worker loop always runs it to completion regardless of
        // the shutdown flag, so draining to zero here guarantees no task is ever dropped
        // unrun. Bounded by `shutdown_deadline`; in the well-behaved case (the
        // overwhelmingly common one) this resolves in low-single-digit milliseconds since
        // it only waits for OS thread pickup, not full task completion under load.
        while PENDING_BLOCKING_TASKS.load(std::sync::atomic::Ordering::SeqCst) > 0
            && std::time::Instant::now() < shutdown_deadline
        {
            std::thread::sleep(Duration::from_millis(1));
        }

        // shutdown_timeout signals worker tasks and waits up to the REMAINING budget
        // (shutdown_deadline minus whatever the drain loop above already consumed — zero
        // if the deadline already passed) for async-worker task drains. It joins the
        // blocking pool (drained above), but async workers complete via waker/signal.
        // Drops all futures (running their Drop impls) on the worker threads before
        // returning.
        //
        // CONFIRMED against Tokio's own vendored source (tokio 1.52.3, pinned by
        // Cargo.lock; read at
        // ~/.cargo/registry/src/index.crates.io-*/tokio-1.52.3/src/runtime/): this one
        // `remaining` budget genuinely covers BOTH concerns the early `drop(_pending)`
        // fix above depends on — waiting for outstanding blocking-pool OS threads AND
        // waiting for async-task Drop-chain cleanup — with no internal starvation of one
        // by the other. `Runtime::shutdown_timeout` (runtime/runtime.rs:449) does exactly
        // two things: `self.handle.inner.shutdown()` (non-blocking — just flips a
        // close/shutdown flag so every worker's poll loop notices and unwinds) followed by
        // `self.blocking_pool.shutdown(Some(duration))` (blocking/pool.rs:244), which is
        // the ONLY call that consumes `duration`. Critically, the multi-thread scheduler's
        // own async-worker OS threads are THEMSELVES spawned through that same blocking
        // pool (`Launch::launch` → `runtime::spawn_blocking(|| run(worker))`,
        // scheduler/multi_thread/worker.rs:501) and tracked in the exact same
        // `worker_threads: HashMap<usize, thread::JoinHandle<()>>` that
        // `BlockingPool::shutdown` takes and joins (blocking/pool.rs:118, :259, :425). Each
        // worker's `run(worker)` body performs the async Drop-chain cleanup (cancelling and
        // dropping owned tasks via `pre_shutdown`/`shutdown_core`, worker.rs:1278-1303)
        // BEFORE it returns and its blocking-pool thread handle is joinable — so
        // `blocking_pool.shutdown(Some(duration))`'s single `shutdown_rx.wait(timeout)`
        // (blocking/shutdown.rs:37) blocks on the SAME shared clock for outstanding
        // `spawn_blocking` tasks (ours) and for async-worker shutdown/cleanup to finish.
        // There is no second, independently-exhaustible sub-budget for either side to
        // starve the other inside this one call. (Version-specific: this is an
        // implementation detail of tokio 1.52.3, not a documented public contract — if
        // `tokio` is ever upgraded, re-check this reasoning against the new source rather
        // than assuming it still holds.)
        let remaining = shutdown_deadline.saturating_duration_since(std::time::Instant::now());
        rt.shutdown_timeout(remaining);
    }

    // Dump alloc counts AFTER shutdown so all Drop impls (including SpawnStateFnFuture::Drop
    // which calls ynz_free) have run. Writing before shutdown would give stale counts on
    // cancellation paths where Drop runs during shutdown.
    if let Ok(output_path) = std::env::var("YNZ_ALLOC_COUNTER_OUTPUT") {
        if !output_path.is_empty() {
            let alloc_count = crate::ynz_alloc_count();
            let free_count = crate::ynz_free_count();
            let handle_alloc_count = crate::ynz_handle_alloc_count();
            let handle_free_count = crate::ynz_handle_free_count();
            // The handle_* lines come AFTER the alloc=/free= lines: existing prefix-parsers
            // take the FIRST line matching starts_with("alloc")/starts_with("free"), and
            // neither "handle_alloc" nor "handle_free" matches those prefixes.
            let content = format!(
                "alloc={alloc_count}\nfree={free_count}\n\
                 handle_alloc={handle_alloc_count}\nhandle_free={handle_free_count}\n"
            );
            // Best-effort write: ignore errors (the alloc counter is test-only).
            let _ = std::fs::write(&output_path, content);
        }
    }
}

/// Sleep the current OS thread for `ms` milliseconds (blocking sleep, NOT `wait`).
///
/// Used by the Yinz intrinsic `sleepBlocking(int)`. This is a synchronous blocking sleep
/// on the calling thread; it does not use `wait` / async semantics (those ship in
/// v0.3-M2). The name `ynz_thread_sleep_ms` avoids any confusion with the `wait` keyword.
#[no_mangle]
pub extern "C" fn ynz_thread_sleep_ms(ms: i64) {
    if ms > 0 {
        std::thread::sleep(Duration::from_millis(ms as u64));
    }
}

// ── v0.3-M2: async state-machine runtime shims ────────────────────────────────
//
// These four shims back the `wait sleep(N)` Yinz intrinsic and the
// state-machine codegen infrastructure. They are declared in `runtime_decls.rs`
// for the LLVM backend but no call sites are emitted until Phase 2 codegen lands.
//
// ABI invariants (locked in Spike Findings):
//   - resume_fn: extern "C-unwind" fn(frame_ptr: *mut u8, waker_ctx: *mut u8) -> i32
//     Returns 0 = Ready, 1 = Pending.
//   - The `C-unwind` ABI (not plain `C`) is load-bearing: a CPU child that panics is
//     re-raised by `ynz_rt_join_poll` (also `C-unwind`) via `resume_unwind`. That unwind
//     must travel up THROUGH this call site to the entrypoint driver's `catch_unwind`.
//     The codegen-emitted resume fns themselves lack the `nounwind` attribute and carry
//     `.eh_frame`, so an unwind already propagates through THEIR bodies. The abort is a
//     property of the *call site*: a Rust-typed plain `extern "C" fn` call site inserts an
//     abort shim on a foreign unwind (RFC 2945). Declaring this call site `C-unwind`
//     removes that shim, so the unwind passes through to the program's panic path —
//     matching the observable behavior of a sequential panicking callee. Both runtime
//     call sites of a resume fn are `C-unwind`, so no abort shim exists in the path.
//   - waker_ctx: *mut u8 pointing to &mut Context<'_>. The resume_fn casts back
//     via `&mut *(waker_ctx as *mut Context<'_>)`. No fabricated Wakers.
//   - frame_size: i64 byte length of the heap-allocated state-machine frame.
//   - No Tokio types appear in any C-ABI signature; all params are primitive C types.

/// Drives a Yinz codegen-emitted state-machine resume function as a Tokio Future,
/// returning the state machine's final i32 value when it completes.
///
/// The resume_fn ABI: `(frame_ptr, waker_ctx) -> i32` where 0 = Ready and 1 = Pending.
/// When Ready, the wrapper reads the typed return value directly from the frame at
/// offset 16 (the typed return slot — see `ynz_abi::FRAME_OFFSET_RETURN_SLOT`).
/// The i32 returned here is a truncated legacy artifact; the wrapper ignores it and
/// reads the full typed value from the frame instead.
///
/// Used exclusively by `ynz_rt_run_entrypoint` (program-entry driver).
/// `ynz_rt_spawn` uses `SpawnStateFnFuture` (Output=()) — fire-and-forget; value discarded.
struct SyncStateFnFuture {
    resume_fn: unsafe extern "C-unwind" fn(*mut u8, *mut u8) -> i32,
    frame_ptr: *mut u8,
    // frame_size carries the byte length for the Phase 2 deallocation path.
    // Not read by poll() — the resume_fn owns the frame layout.
    // Suppressed until Phase 2 wires the dealloc path.
    #[allow(dead_code)]
    frame_size: i64,
    /// This drive's caller-generation stamp (v0.3-M6 P3-1), minted at construction from the
    /// ONE global counter (`channel::next_caller_generation`) — the same scheme as
    /// `SpawnStateFnFuture::task_gen` and `YnzTaskHandle::send_gen`, so every production
    /// caller identity is stamped NONZERO and the `(caller_token, caller_generation)`
    /// keying is uniform across ALL producers (no unprotected generation-0 class). A sync
    /// drive is immortal by construction — `block_on` runs it to completion and nothing
    /// ever cancels it — so no Drop purge exists for this generation; the stamp's job is
    /// that two sequential sync drives at a reused frame address can never share a key.
    task_gen: u64,
}

// SAFETY: SyncStateFnFuture is driven to completion by a single block_on call; ownership
// of frame_ptr is exclusively held by this future for its lifetime. No aliasing across threads.
unsafe impl Send for SyncStateFnFuture {}

impl Future for SyncStateFnFuture {
    type Output = i32;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Cast Context to *mut u8 — the locked waker_ctx ABI: type-erased pointer to
        // &mut Context<'_>. The resume_fn casts back on the other side.
        let waker_ctx = cx as *mut Context<'_> as *mut u8;
        // Publish this drive's generation for the duration of the resume-fn call (the same
        // discipline as SpawnStateFnFuture::poll): any ynz_channel_send_poll the state
        // machine reaches keys its suspended send by (frame token, THIS generation) — the
        // P3-1 ABA salt, uniform across every producer. RAII: restores the previous value
        // even if resume_fn unwinds; nesting-safe for a sync drive re-entered from inside
        // a Tokio worker/blocking-pool thread.
        // A sync drive owns no spawn-arg drop ladder (nothing was heap-cloned for it), so
        // it publishes a ladder-less identity: a send from the entrypoint never releases
        // anything.
        let _drive_guard =
            crate::channel::DriveGuard::enter(DriveIdentity::ladderless(self.task_gen));
        // SAFETY: resume_fn is a valid C-ABI function pointer (caller guarantee).
        // frame_ptr is valid for frame_size bytes (caller guarantee).
        let result = unsafe { (self.resume_fn)(self.frame_ptr, waker_ctx) };
        match result {
            0 => {
                // Ready — read the typed return value from the return slot at offset 16.
                // Phase 7 codegen stores the i64 return value at FRAME_OFFSET_RETURN_SLOT=16
                // instead of the old i32 at frame[0] (which was the i32-truncation defect).
                // SAFETY: frame_ptr is valid for at least 32 bytes (all codegen-emitted frames
                // have a 32-byte header: resume_point(4)+padding(4)+sleep_handle(8)+return_slot(16)).
                // The return slot's first 8 bytes hold the i64 return value (success path).
                // We truncate to i32 here for the legacy SyncStateFnFuture::Output type;
                // the wrapper reads the full typed value directly from the frame for non-main fns.
                let value = unsafe {
                    let ret_slot_ptr =
                        self.frame_ptr.add(FRAME_OFFSET_RETURN_SLOT as usize) as *const i64;
                    *ret_slot_ptr as i32
                };
                Poll::Ready(value)
            }
            1 => Poll::Pending,
            // Unexpected return values indicate a codegen bug; panic is the correct response
            // because continuing with corrupt state would be worse than a loud failure.
            _ => panic!(
                "ynz runtime: state-machine resume_fn returned unexpected value {result} \
                 (expected 0=Ready or 1=Pending). This is a compiler codegen bug."
            ),
        }
    }
}

/// Descriptor for one heap-allocated background arg-copy that must be freed when the
/// spawned state-machine task completes (on both normal completion and cancellation).
///
/// Codegen allocates these arg-copies via `ynz_alloc` (Shape) or `ynz_array_clone_primitive`
/// (array<primitive>) so they outlive the spawner's stack frame. The descriptor records
/// where in the frame the pointer lives so the Drop impl can recover and free it.
///
/// Layout (repr C, matches the alloca'd array in codegen):
///   byte_offset: u64  — byte offset from frame_ptr to the i64 slot holding the heap pointer
///   kind: u64         — one of `ynz_abi::BG_ARG_KIND_*` (the free protocol for the slot)
///   size: u64         — byte count passed to ynz_free for `BG_ARG_KIND_HEAP_SHAPE` (ignored
///                       by the other kinds)
///
/// `kind` is written by codegen at spawn time and may be REWRITTEN by the runtime to
/// `BG_ARG_KIND_RELEASED` while the task runs — see [`release_ladder_payload`].
#[repr(C)]
pub struct BgArgDropEntry {
    pub byte_offset: u64,
    pub kind: u64,
    pub size: u64,
}

/// The identity of the state-machine drive whose `poll` is running on the current thread —
/// what a channel send needs to know about its caller: the ABA-salt generation, plus the
/// caller's spawn-arg drop ladder (frame + descriptors) so a payload the ladder owns can be
/// released when the channel takes it. Published per poll by [`crate::channel::DriveGuard`].
#[derive(Clone, Copy)]
pub(crate) struct DriveIdentity {
    /// The drive's caller-generation stamp (0 = none — a bare test call outside any drive).
    pub(crate) generation: u64,
    /// The drive's root frame — the base the descriptors' `byte_offset`s are relative to.
    /// Null for a ladder-less drive.
    pub(crate) frame_ptr: *mut u8,
    /// The drive's `BgArgDropEntry` array (null when it owns no heap arg-copies). Exclusively
    /// owned by the drive's future; only touched on the thread running that future's `poll`.
    pub(crate) arg_drop_ptr: *mut BgArgDropEntry,
    /// Entry count at `arg_drop_ptr` (0 when null).
    pub(crate) arg_drop_count: usize,
}

impl DriveIdentity {
    /// No drive is running (the thread-local's rest state).
    pub(crate) const NONE: DriveIdentity = DriveIdentity::ladderless(0);

    /// A drive with a generation but no spawn-arg drop ladder — the sync entrypoint drive
    /// (nothing was heap-cloned for it) and bare test drives.
    pub(crate) const fn ladderless(generation: u64) -> DriveIdentity {
        DriveIdentity {
            generation,
            frame_ptr: std::ptr::null_mut(),
            arg_drop_ptr: std::ptr::null_mut(),
            arg_drop_count: 0,
        }
    }
}

/// Release `bits` from a spawned task's spawn-arg drop ladder: every descriptor whose frame
/// slot holds exactly `bits` is rewritten to [`BG_ARG_KIND_RELEASED`], so the ladder no
/// longer frees that payload when the task retires. Returns how many descriptors changed.
///
/// This is the ONE link between the drop ladder and the places ownership of a ladder-owned
/// payload can leave the task while it runs — a channel send (`channel_send_poll_guarded`,
/// which both `ch.send` and `h.send` funnel through) and a task-handle return
/// (`HandleStateFnFuture::poll`'s completion extraction). Before it existed the same pointer
/// was owned by the ladder AND by the channel/parent, and the ladder freed it under the
/// receiver: `got.count()` on a freed array printed `-4760032263271174595`.
///
/// Matching is by pointer identity, not by static type or by which parameter name was
/// written: `wire.send(rows)`, `let alias = rows; wire.send(alias)`, and a nested SM callee
/// sending its caller's parameter all hand the SAME pointer bits over, and all must release
/// the same slot. Two live heap allocations never share an address, so a match is never a
/// coincidence for a heap-pointer payload. Callers gate on the payload being a heap pointer
/// (the channel's element drop glue exists; the handle return kind is `*_HEAP_PTR`) so a
/// plain `int` payload is never compared at all — an `int` can hold any bits.
///
/// Only payload kinds ([`BG_ARG_KIND_HEAP_SHAPE`], [`BG_ARG_KIND_HEAP_ARRAY`]) are
/// releasable; a [`BG_ARG_KIND_SHARED_CHANNEL`] slot is a refcount the task itself holds and
/// is never a channel element (typeck rejects `channel<channel<T>>`).
///
/// Time: O(n) where n = the task's descriptor count (bounded by its parameter count,
/// typically 0-3). Space: O(1).
///
/// # Safety
/// `drive` must describe the future whose `poll` is running on the current thread (or be
/// ladder-less): `frame_ptr` valid for every descriptor's `byte_offset + 8`, and
/// `arg_drop_ptr` valid for `arg_drop_count` entries and not aliased by any other thread.
pub(crate) unsafe fn release_ladder_payload(drive: &DriveIdentity, bits: i64) -> usize {
    if drive.arg_drop_ptr.is_null() || drive.arg_drop_count == 0 || drive.frame_ptr.is_null() {
        return 0;
    }
    let descs = std::slice::from_raw_parts_mut(drive.arg_drop_ptr, drive.arg_drop_count);
    let mut released = 0;
    for desc in descs {
        if desc.kind != BG_ARG_KIND_HEAP_SHAPE && desc.kind != BG_ARG_KIND_HEAP_ARRAY {
            continue;
        }
        let slot = drive.frame_ptr.add(desc.byte_offset as usize) as *const i64;
        if *slot == bits {
            desc.kind = BG_ARG_KIND_RELEASED;
            released += 1;
        }
    }
    released
}

/// Drives a Yinz codegen-emitted state-machine resume function as a fire-and-forget
/// Tokio Future. Return value is intentionally discarded — `ynz_rt_spawn` callers
/// use channels or atomics to observe completion.
///
/// Separate from `SyncStateFnFuture` so `ynz_rt_spawn`'s external C-ABI signature
/// stays `void` (no return channel at the ABI level).
pub(crate) struct SpawnStateFnFuture {
    pub(crate) resume_fn: unsafe extern "C-unwind" fn(*mut u8, *mut u8) -> i32,
    pub(crate) frame_ptr: *mut u8,
    /// Byte length of the heap frame, used by `Drop` to free it via `ynz_free`.
    pub(crate) frame_size: i64,
    /// Byte offset of the recursion-slot pointer within the frame (-1 = no recursion slot).
    ///
    /// For recursive SM functions, codegen allocates a heap-boxed child frame for each
    /// recursive call and stores the pointer at this offset. On cancellation, the Drop
    /// impl walks this pointer chain and frees all live heap-boxed child frames before
    /// freeing the root frame. Without this, a mid-wait cancellation of a deep recursion
    /// leaks all the child frames that were live at abort time.
    pub(crate) recursion_slot_offset: i64,
    /// Pointer to a `ynz_alloc`'d array of `BgArgDropEntry` describing heap arg-copies
    /// stored in this frame's local slots. Null when no heap arg-copies exist (e.g., the
    /// callee takes only primitives or strings).
    ///
    /// The array is heap-allocated by codegen at spawn time and freed here in Drop after
    /// all arg-copies have been released — exactly once, on every exit path. Mutable because
    /// [`release_ladder_payload`] rewrites an entry's `kind` when its payload's ownership
    /// leaves the task mid-run (channel send / handle return).
    pub(crate) arg_drop_ptr: *mut BgArgDropEntry,
    /// Number of entries at `arg_drop_ptr`. 0 when `arg_drop_ptr` is null.
    pub(crate) arg_drop_count: usize,
    /// This task's caller-generation stamp (v0.3-M6 P3-1), minted once at construction from
    /// the ONE global counter (`channel::next_caller_generation`). Published to the channel
    /// module via a thread-local while `poll` runs (so every frame token this task mints
    /// carries it), and used by `Drop`'s kind-2 arm to purge the task's suspended sends.
    pub(crate) task_gen: u64,
}

impl SpawnStateFnFuture {
    /// Construct a `SpawnStateFnFuture` for testing. The `new_*` path exists so tests
    /// can build the future, hold it locally, and drop it without having to spawn it
    /// (which would require a live Tokio runtime and would prevent observing drop behaviour
    /// on the frame before the task runs).
    #[cfg(test)]
    pub(crate) fn new(
        resume_fn: unsafe extern "C-unwind" fn(*mut u8, *mut u8) -> i32,
        frame_ptr: *mut u8,
        frame_size: i64,
        recursion_slot_offset: i64,
        arg_drop_ptr: *const BgArgDropEntry,
        arg_drop_count: i64,
    ) -> Self {
        Self {
            resume_fn,
            frame_ptr,
            frame_size,
            recursion_slot_offset,
            arg_drop_ptr: arg_drop_ptr as *mut BgArgDropEntry,
            arg_drop_count: arg_drop_count as usize,
            task_gen: crate::channel::next_caller_generation(),
        }
    }
}

impl SpawnStateFnFuture {
    /// This task's identity as a channel caller: generation + drop ladder. Published for the
    /// duration of each `poll` (see `DriveGuard`) and consulted directly by the handle
    /// future's completion extraction.
    pub(crate) fn drive_identity(&self) -> DriveIdentity {
        DriveIdentity {
            generation: self.task_gen,
            frame_ptr: self.frame_ptr,
            arg_drop_ptr: self.arg_drop_ptr,
            arg_drop_count: self.arg_drop_count,
        }
    }
}

// SAFETY: SpawnStateFnFuture is owned exclusively by the spawned task for its lifetime.
unsafe impl Send for SpawnStateFnFuture {}

/// Free live CPU join handles stored in a spike frame, if any.
///
/// Reads the discriminator word at frame offset 4. Normal frames always have 0 there
/// (ynz_alloc_zeroed initialises the frame). Spike frames have a packed discriminator
/// written by codegen at spawn time: the high-16-bit `SPIKE_FRAME_TAG` confirms the frame
/// is a spike frame, and the low 16 bits give the live CPU-handle count. The count drives
/// the scan, so this is correct for an arbitrary number of CPU-group members — the handle
/// region is a contiguous run of `count` slots starting at `SPIKE_HANDLE_BASE_OFFSET`.
///
/// For each non-null handle slot: drops the Box<CpuJoinHandle>, detaching the blocking-pool
/// task (it runs to completion; results are discarded). Null slots are skipped — they were
/// either never spawned or already consumed by a Ready poll.
///
/// Called from `SpawnStateFnFuture::drop` on cancellation, at BOTH frame grains: on the
/// root frame (drop step 1.5) and on each recursion-chain child frame inside the chain
/// walk (drop step 3) — the ONE authoritative cleanup path for spike CPU handles; never
/// duplicate this logic at a call site. Extracted as a `pub(crate)` helper
/// so the discriminator + handle-free logic can be tested independently without constructing
/// a full `SpawnStateFnFuture` (which requires live resume-fn scaffolding).
///
/// Time: O(n)  Space: O(1) where n = the encoded handle count (one drop per non-null slot).
///
/// # Safety
/// `frame_ptr` must be a non-null, valid pointer to at least 8 bytes for the discriminator
/// read (4 bytes at offset 4), and — when the discriminator tags it as a spike frame with
/// count `n` — to at least `SPIKE_HANDLE_BASE_OFFSET + n * SPIKE_HANDLE_SLOT_BYTES` bytes.
/// The spike frame codegen allocates always satisfies this: it sizes the handle region from
/// the same per-member layout that produced the count written here.
pub(crate) unsafe fn cleanup_spike_cpu_handles(frame_ptr: *mut u8) {
    // Normal frames: bytes 4-7 are zero, so `decode_spike_discriminator` returns None and no
    // handle slots are touched. Spike frames carry the tag + the live handle count.
    let disc_slot = frame_ptr.add(SPIKE_FRAME_DISCRIMINATOR_OFFSET) as *const u32;
    let Some(handle_count) = decode_spike_discriminator(*disc_slot) else {
        return;
    };
    for i in 0..handle_count {
        let handle_offset = SPIKE_HANDLE_BASE_OFFSET + i * SPIKE_HANDLE_SLOT_BYTES;
        let slot = frame_ptr.add(handle_offset) as *mut *mut u8;
        let ptr = *slot;
        if !ptr.is_null() {
            drop(Box::from_raw(ptr as *mut CpuJoinHandle));
            // Null the slot after free to prevent double-free if cleanup is called
            // again (or if the frame is inspected after this function returns).
            *slot = std::ptr::null_mut();
        }
    }
}

impl Drop for SpawnStateFnFuture {
    /// Free all resources owned by this spawned task — on normal completion AND
    /// on cancellation (Tokio dropping the task before it finishes).
    ///
    /// # Free order
    ///
    /// 1. Sleep handle in the root frame (frees the in-flight Tokio Sleep box if cancelled mid-wait).
    /// 2. Heap arg-copies: read each entry in `arg_drop_descs`, recover the pointer from the
    ///    frame slot, and free it — BEFORE freeing the frame (avoids use-after-free on the slots).
    /// 3. Recursion-chain child frames (for self-recursive SM functions — walks the chain).
    ///    Each child's sleep handle AND spike CPU handles (via the same
    ///    `cleanup_spike_cpu_handles` choke point step 1.5 uses on the root) are freed
    ///    before the child frame itself.
    /// 4. The arg-drop descriptor array itself (freed via `ynz_free`, same allocator as codegen).
    /// 5. The root frame (freed last, after all frame-resident pointers have been read).
    ///
    /// Every resource is freed exactly once regardless of which exit path runs.
    fn drop(&mut self) {
        if self.frame_ptr.is_null() {
            return;
        }
        unsafe {
            // 1. Free the sleep handle in the root frame (if a sleep is live mid-wait).
            // SAFETY: FRAME_OFFSET_SLEEP_HANDLE=8 is within every valid frame header (32 bytes).
            let handle_slot =
                self.frame_ptr.add(FRAME_OFFSET_SLEEP_HANDLE as usize) as *const *mut u8;
            let handle_ptr = *handle_slot;
            if !handle_ptr.is_null() {
                drop(Box::from_raw(handle_ptr as *mut Pin<Box<Sleep>>));
            }

            // 1.5. Free CPU join handles if this is a spike frame (discriminator at bytes 4-7).
            // SAFETY: frame_ptr is valid for at least the spike frame body (caller guarantee).
            cleanup_spike_cpu_handles(self.frame_ptr);

            // 2. Free heap arg-copies stored as i64 bit-patterns in this frame's local slots.
            //    Each BgArgDropEntry names one frame slot (by byte offset) and the free protocol.
            //    Must run BEFORE free_frame so we can still read the slot values.
            if !self.arg_drop_ptr.is_null() && self.arg_drop_count > 0 {
                let descs = std::slice::from_raw_parts(self.arg_drop_ptr, self.arg_drop_count);
                for desc in descs {
                    // Read the i64 bit-pattern from the frame slot, cast to pointer.
                    let slot = self.frame_ptr.add(desc.byte_offset as usize) as *const i64;
                    let bits = *slot;
                    if bits == 0 {
                        // Defensive: a descriptor is only emitted for a slot that WAS
                        // heap-copied (both `give` and `copy` of a heap arg copy — see D8),
                        // so a 0 here means `ynz_alloc` returned null and an upstream abort
                        // already fired. Skip rather than free(null).
                        continue;
                    }
                    let heap_ptr = bits as *mut u8;
                    match desc.kind {
                        BG_ARG_KIND_HEAP_SHAPE => {
                            // HeapShape: allocated with ynz_alloc; free with ynz_free(ptr, size).
                            crate::ynz_free(heap_ptr, desc.size as usize);
                        }
                        BG_ARG_KIND_HEAP_ARRAY => {
                            // HeapArrayPrimitive: allocated by ynz_array_clone_primitive (counted
                            // ynz_alloc); free with ynz_array_drop which handles both the data
                            // buffer and the header.
                            crate::ynz_array_drop(heap_ptr as *mut crate::YnzArray);
                        }
                        BG_ARG_KIND_RELEASED => {
                            // Ownership left this task while it ran (`release_ladder_payload`):
                            // the payload was sent into a channel or returned through the task
                            // handle, and whoever holds it now frees it. The frame slot still
                            // holds the pointer (the task's own later reads keep working); it is
                            // simply no longer this ladder's to free.
                        }
                        BG_ARG_KIND_SHARED_CHANNEL => {
                            // v0.3-M4 SharedChannel: the task's refcounted reference to a
                            // channel it was handed (`ynz_channel_share` at the spawn site).
                            // v0.3-M6 P2-2: purge THIS task's suspended sends BEFORE releasing
                            // the reference — a cancelled sender's orphaned pending_sends
                            // entry is both a leak and the P3-1 ABA precondition. Idempotent:
                            // an already-resolved/absent entry is a safe no-op. Then release
                            // exactly one reference, keeping alloc=free balanced on every
                            // task exit path, including cancellation.
                            crate::channel::purge_pending_sends(heap_ptr, self.task_gen);
                            crate::channel::ynz_channel_free(heap_ptr);
                        }
                        _ => {
                            // Unknown kind — defensive no-op; avoids a bad free on future kind values.
                        }
                    }
                }
            }

            // 3. Walk the recursion chain and free any live heap-boxed child frames.
            //
            // test-only: `YNZ_SKIP_RECURSION_DROP` bypasses the chain walk so the
            // negative-control test can verify a measurable leak without this code.
            // Production runs never set this env var; the latched `false` default
            // means the walk always runs in production. The flag is a once-latched
            // atomic (see `init_skip_recursion_drop_flag`), not a per-drop env read:
            // this drop runs on every task completion/cancellation — a hot path.
            if self.recursion_slot_offset >= 0 && !crate::skip_recursion_drop() {
                // SAFETY: frame_ptr is valid; recursion_slot_offset is within the frame.
                let rec_slot =
                    self.frame_ptr.add(self.recursion_slot_offset as usize) as *const *mut u8;
                let mut child_ptr = *rec_slot;
                while !child_ptr.is_null() {
                    // Free the child's sleep handle before freeing its frame.
                    let child_handle_slot =
                        child_ptr.add(FRAME_OFFSET_SLEEP_HANDLE as usize) as *const *mut u8;
                    let child_handle = *child_handle_slot;
                    if !child_handle.is_null() {
                        drop(Box::from_raw(child_handle as *mut Pin<Box<Sleep>>));
                    }
                    // Free the child's spike CPU handles through the SAME choke point the
                    // root frame uses (step 1.5) — a chain child cancelled while parked at
                    // its CPU join owns live boxed CpuJoinHandles in its frame handle slots.
                    // SAFETY: child_ptr was allocated by ynz_alloc_zeroed(frame_size) with
                    // the same layout as the root (self-recursion, same function); a
                    // non-spike child's zeroed discriminator decodes None and is untouched.
                    cleanup_spike_cpu_handles(child_ptr);
                    // Read grandchild pointer BEFORE freeing child (use-after-free guard).
                    let grandchild_slot =
                        child_ptr.add(self.recursion_slot_offset as usize) as *const *mut u8;
                    let next_ptr = *grandchild_slot;
                    // SAFETY: child_ptr was allocated by ynz_alloc_zeroed(frame_size) and is not aliased.
                    crate::ynz_free(child_ptr, self.frame_size as usize);
                    child_ptr = next_ptr;
                }
            }

            // 4. Free the arg-drop descriptor array (ynz_alloc'd by codegen at spawn time).
            //    24 bytes per entry (3 × u64); freed after all arg-copies are already released.
            if !self.arg_drop_ptr.is_null() && self.arg_drop_count > 0 {
                let desc_bytes = self.arg_drop_count * std::mem::size_of::<BgArgDropEntry>();
                crate::ynz_free(self.arg_drop_ptr as *mut u8, desc_bytes);
            }

            // 5. Free the root frame last (after all frame-resident pointers have been read).
            // SAFETY: frame_ptr was returned by `ynz_alloc` for `frame_size` bytes and moved
            // into this future exclusively; freed exactly once here.
            crate::ynz_free(self.frame_ptr, self.frame_size as usize);
        }
    }
}

impl Future for SpawnStateFnFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker_ctx = cx as *mut Context<'_> as *mut u8;
        // Publish this task's identity for the duration of the resume-fn call: any
        // ynz_channel_send_poll the state machine reaches keys its suspended send by
        // (frame token, THIS generation) — the P3-1 ABA salt — and releases a sent payload
        // from THIS task's drop ladder (`release_ladder_payload`), with the extern-C send ABI
        // unchanged. RAII: restores the previous value even if resume_fn unwinds; re-set
        // from the future's own fields at every poll, so work-stealing is safe.
        let _drive_guard = crate::channel::DriveGuard::enter(self.drive_identity());
        // SAFETY: resume_fn is a valid C-ABI function pointer (caller guarantee).
        // frame_ptr is valid for frame_size bytes (caller guarantee).
        let result = unsafe { (self.resume_fn)(self.frame_ptr, waker_ctx) };
        match result {
            // Ready — fire-and-forget; return value in frame slot 0 is intentionally ignored.
            0 => Poll::Ready(()),
            1 => Poll::Pending,
            _ => panic!(
                "ynz runtime: state-machine resume_fn returned unexpected value {result} \
                 (expected 0=Ready or 1=Pending). This is a compiler codegen bug."
            ),
        }
    }
}

/// Schedule a state-machine function on the Tokio I/O worker pool (non-blocking spawn).
///
/// # Flow
/// 1. Wrap the `(resume_fn, frame_ptr)` pair in a `SpawnStateFnFuture` — a `Future` impl
///    that calls `resume_fn(frame_ptr, waker_ctx)` on each poll and discards the return
///    value (fire-and-forget; callers signal completion via channels or atomics).
/// 2. Spawn the future on the global Tokio runtime via `rt.spawn`. Returns immediately;
///    the future runs cooperatively on the work-stealing I/O thread pool.
/// 3. The frame pointed to by `frame_ptr` is freed by `SpawnStateFnFuture`'s `Drop` when the
///    task ends — on normal completion AND on cancellation. Ownership of the frame moves into
///    the future at spawn time, so the drop frees it exactly once via `ynz_free`.
/// 4. Heap arg-copies for heap-typed background arguments (Shape, array<primitive>) are freed
///    by `SpawnStateFnFuture::drop` using `arg_drop_ptr`/`arg_drop_count` — before the frame
///    is freed, after the task's callee has read them. Pass null/0 when no heap arg-copies exist.
/// 5. Panic inside `resume_fn` is caught by Tokio's task wrapper; the JoinHandle
///    will surface a `JoinError::is_panic()`. The spawning scope continues normally.
///
/// # Failure modes
/// - `ynz_rt_init` was never called: logs a warning and discards the task.
/// - `ynz_rt_shutdown` was already called: logs a warning and discards the task.
/// - `resume_fn` panics: caught by Tokio's task wrapper; JoinHandle carries the panic.
///
/// # Side effects
/// Time: O(1)  Space: O(1) — the frame is MOVED into the future (no byte copy here);
/// `frame_size` bytes are reused, not duplicated.
/// Enqueues a work-stealing task on the Tokio I/O pool. The frame pointed to by
/// `frame_ptr` must remain valid until the spawned future completes (ownership transfers
/// into the future at spawn time — the caller must NOT free or alias frame_ptr after
/// calling this function). The `arg_drop_ptr` array (if non-null) is also owned by this
/// call and freed by `SpawnStateFnFuture::drop`.
///
/// # Safety
/// - `resume_fn` must be a valid function pointer matching the `(frame, waker_ctx) -> i32` ABI.
/// - `frame_ptr` must be valid for `frame_size` bytes and exclusively owned by this call.
///   After this function returns, the caller must treat `frame_ptr` as moved.
/// - `arg_drop_ptr` must be null (when `arg_drop_count == 0`) or valid for
///   `arg_drop_count * sizeof(BgArgDropEntry)` bytes and exclusively owned by this call.
///   May be null when `arg_drop_count == 0` (no heap arg-copies).
#[no_mangle]
pub unsafe extern "C" fn ynz_rt_spawn(
    resume_fn: unsafe extern "C-unwind" fn(*mut u8, *mut u8) -> i32,
    frame_ptr: *mut u8,
    frame_size: i64,
    recursion_slot_offset: i64,
    arg_drop_ptr: *const BgArgDropEntry,
    arg_drop_count: i64,
) {
    let future = SpawnStateFnFuture {
        resume_fn,
        frame_ptr,
        frame_size,
        recursion_slot_offset,
        // Exclusively owned by this call (safety contract above): the future may rewrite
        // entry kinds as payload ownership leaves the task.
        arg_drop_ptr: arg_drop_ptr as *mut BgArgDropEntry,
        arg_drop_count: arg_drop_count as usize,
        task_gen: crate::channel::next_caller_generation(),
    };
    let _ = spawn_on_runtime(future, "ynz_rt_spawn");
}

/// Spawn `future` on the global Tokio runtime, returning its `JoinHandle` — the SINGLE
/// runtime-handle resolution shared by `ynz_rt_spawn` and `ynz_rt_spawn_handle`.
///
/// Prefers the current Tokio handle (avoids the RUNTIME mutex deadlock when called from inside
/// a block_on future — block_on holds Handle context so `try_current()` succeeds, and we can
/// spawn without the mutex). Falls back to the global RUNTIME; returns `None` (task discarded,
/// warning logged) when the runtime was never initialized or already shut down.
pub(crate) fn spawn_on_runtime<F>(future: F, caller: &str) -> Option<tokio::task::JoinHandle<()>>
where
    F: Future<Output = ()> + Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => Some(handle.spawn(future)),
        Err(_) => {
            let Some(guard) = RUNTIME.get() else {
                eprintln!("ynz runtime: {caller} called before ynz_rt_init — task discarded");
                return None;
            };
            let handle = {
                let lock = match guard.lock() {
                    Ok(l) => l,
                    Err(e) => e.into_inner(),
                };
                match lock.as_ref() {
                    Some(rt) => rt.handle().clone(),
                    None => {
                        eprintln!(
                            "ynz runtime: {caller} called after ynz_rt_shutdown — task discarded"
                        );
                        return None;
                    }
                }
            };
            Some(handle.spawn(future))
        }
    }
}

/// Allocate and return a heap-pinned `tokio::time::Sleep` future for `sleep(ms)`.
///
/// # Flow
/// 1. Create `tokio::time::sleep(Duration::from_millis(ms))` — a `Sleep` future.
/// 2. Box and pin the future so it can be safely polled via `as_mut()`.
/// 3. Convert to a raw pointer via `Box::into_raw`; return as `*mut u8` (opaque to caller).
///
/// The caller (codegen-emitted state-machine resume function) stores the returned pointer
/// in the state-machine frame's `awaited_handle` slot and passes it to
/// `ynz_rt_async_sleep_poll` on each subsequent poll until Ready.
///
/// # Failure modes
/// - `ms <= 0`: creates a sleep with duration 0 (fires immediately on first poll).
///
/// # Side effects
/// Heap-allocates a `Pin<Box<Sleep>>`. The caller is responsible for freeing the
/// allocation by calling `ynz_rt_async_sleep_poll` until it returns 0 (Ready), at which
/// point the box is dropped internally. If the frame is dropped mid-wait (cancellation),
/// the frame's Drop impl must call the appropriate free shim to avoid leaking the Sleep box.
///
/// # Safety
/// The returned pointer is valid until `ynz_rt_async_sleep_poll` returns 0 (Ready) or
/// until explicitly freed. After Ready, the pointer is dangling — do not use it.
#[no_mangle]
pub extern "C" fn ynz_rt_async_sleep_create(ms: i64) -> *mut u8 {
    let duration = if ms > 0 {
        Duration::from_millis(ms as u64)
    } else {
        Duration::ZERO
    };
    let sleep: Pin<Box<Sleep>> = Box::pin(tokio::time::sleep(duration));
    // Convert to raw pointer; ownership transfers to the caller (stored in the frame).
    // SAFETY: Box::into_raw returns a valid aligned pointer; Pin guarantees the allocation
    // is stable in memory. The caller must not move the pointee.
    Box::into_raw(Box::new(sleep)) as *mut u8
}

/// Poll an in-flight `Sleep` future created by `ynz_rt_async_sleep_create`.
///
/// # Flow
/// 1. Cast `handle_ptr` back to `*mut Pin<Box<Sleep>>` and borrow it mutably.
/// 2. Cast `waker_ctx` back to `&mut Context<'_>` — the real Tokio waker forwarded
///    from the enclosing state-machine's poll. No fabricated Wakers.
/// 3. Call `sleep.as_mut().poll(cx)`.
/// 4. On `Pending`: waker is registered with Tokio's timer reactor (by `Sleep::poll`
///    internally). Return 1 so the state-machine frame saves `resume_point` and returns
///    `Pending` to its own caller.
/// 5. On `Ready`: drop the `Pin<Box<Sleep>>` allocation. Return 0 so the state-machine
///    advances to its next resume point.
///
/// # Failure modes
/// - Panic inside `sleep.poll()`: caught by `std::panic::catch_unwind`; returns `1`
///   (Pending) so the frame is not corrupted. The task's overall panic propagation is
///   handled by the Tokio task wrapper around the state machine's Future.
///
/// # Side effects
/// Time: O(1)  Space: O(1) — one `Sleep::poll` call, no loop; frees one heap box on Ready.
/// - On Ready: frees the `Pin<Box<Sleep>>` heap allocation.
/// - On Pending: `Sleep::poll` registers the real Tokio timer waker — the task is woken
///   automatically when the timer fires, with no tight-loop polling.
///
/// # Safety
/// - `handle_ptr` must be a non-null pointer previously returned by
///   `ynz_rt_async_sleep_create` and not yet consumed (i.e., `ynz_rt_async_sleep_poll`
///   has not yet returned 0 for this handle).
/// - `waker_ctx` must be a valid `*mut u8` pointing to a live `&mut Context<'_>` for
///   the duration of this call. This is the same context passed into the enclosing
///   state-machine's `Future::poll` invocation.
#[no_mangle]
pub unsafe extern "C" fn ynz_rt_async_sleep_poll(handle_ptr: *mut u8, waker_ctx: *mut u8) -> i32 {
    let result = std::panic::catch_unwind(|| {
        // SAFETY: handle_ptr was returned by ynz_rt_async_sleep_create (Box::into_raw of
        // Box<Pin<Box<Sleep>>>). It is valid, aligned, and exclusively owned by this call.
        let sleep_box = &mut *(handle_ptr as *mut Pin<Box<Sleep>>);
        // SAFETY: waker_ctx was cast from &mut Context<'_> by StateFnFuture::poll or by
        // the codegen-emitted resume function. Valid for the duration of this call.
        let cx = &mut *(waker_ctx as *mut Context<'_>);
        match sleep_box.as_mut().poll(cx) {
            Poll::Pending => 1i32,
            Poll::Ready(()) => {
                // Drop the Sleep box now that it has fired. Reconstruct ownership from
                // the raw pointer so Rust's drop machinery runs the deallocation.
                // SAFETY: handle_ptr is valid, non-null, uniquely owned; reconstructing
                // Box<Pin<Box<Sleep>>> is the inverse of Box::into_raw above.
                drop(Box::from_raw(handle_ptr as *mut Pin<Box<Sleep>>));
                0i32
            }
        }
    });
    match result {
        Ok(v) => v,
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "<non-string panic payload>".to_string()
            };
            eprintln!("ynz runtime: ynz_rt_async_sleep_poll panicked (returning Pending): {msg}");
            1 // Return Pending on panic so the frame is not corrupted.
        }
    }
}

/// Run the program's entry state machine to completion — the `#[tokio::main]`-equivalent
/// single program-entry driver.
///
/// This is called ONLY by the codegen-emitted `main` wrapper (and non-entry wrapper
/// functions called from non-state-machine contexts). Every call is at a genuine
/// top-level program-entry point; this function is NOT reachable from inside any
/// state-machine resume function (`ynz_sm_*_resume`) — those inline-poll-yield into
/// embedded child sub-frames without going through this driver.
///
/// Returns the state machine's final i32 value. The codegen-emitted wrapper ignores
/// this return value and reads the typed return directly from the frame at offset 16
/// (the typed return slot). The i32 here is a legacy holdover for the `main` exit-code
/// path only; non-main wrappers discard it entirely.
///
/// # Flow
/// 1. Wrap `(resume_fn, frame_ptr)` in a `SyncStateFnFuture` (Output = i32).
/// 2. Detect Tokio context via `Handle::try_current()`:
///    - `Ok(handle)` — inside Tokio (worker thread OR spawn_blocking-pool thread).
///      Call `handle.block_on(future)`. Works on both thread types; unlike
///      `block_in_place`, which panics on spawn_blocking-pool threads (confirmed by
///      Spike Contract #4b).
///    - `Err(_)` — outside Tokio (main thread before runtime boots). Use the global
///      `RUNTIME` (initialised by `ynz_rt_init`). Codegen guarantees `ynz_rt_init`
///      runs before any state-machine call so `RUNTIME.get().expect(...)` is
///      a defence-in-depth assertion, not an expected failure path.
/// 3. Wrap the entire `match` in `catch_unwind` so a panicking state machine does not
///    abort the calling thread; the panic propagates as a Rust panic to the caller.
///
/// # Failure modes
/// - `resume_fn` panics: `catch_unwind` catches and re-panics; the caller's panic
///   handler (or Tokio's task wrapper) takes over.
/// - `RUNTIME` not initialised (codegen bug): panics with a clear message.
/// - `RUNTIME` mutex poisoned: recovers via `into_inner` (same as other shims).
///
/// # Side effects
/// Blocks the calling thread until the state machine returns `Ready` (0).
///
/// # Safety
/// - `resume_fn` must be a valid function pointer matching the `(frame, waker_ctx) -> i32` ABI.
/// - `frame_ptr` must be valid for `frame_size` bytes for the duration of this call.
///   The frame is NOT freed by this function; the caller retains ownership and must free it.
///
/// # ABI: `extern "C-unwind"`
/// On the `Err` branch this function re-raises the caught panic via `resume_unwind`. The
/// `C-unwind` ABI lets that unwind propagate out to the codegen-emitted `main` wrapper
/// instead of aborting at the C boundary (RFC 2945) — required so a CPU child's panic
/// reaches the program's panic path with the same observable behavior as sequential code.
#[no_mangle]
pub unsafe extern "C-unwind" fn ynz_rt_run_entrypoint(
    resume_fn: unsafe extern "C-unwind" fn(*mut u8, *mut u8) -> i32,
    frame_ptr: *mut u8,
    frame_size: i64,
) -> i32 {
    let result = std::panic::catch_unwind(|| {
        let future = SyncStateFnFuture {
            resume_fn,
            frame_ptr,
            frame_size,
            task_gen: crate::channel::next_caller_generation(),
        };
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // Inside Tokio — worker thread OR spawn_blocking-pool thread.
                // Handle::block_on works on both (unlike block_in_place which panics on
                // spawn_blocking-pool threads — confirmed by Spike Contract #4b).
                handle.block_on(future)
            }
            Err(_) => {
                // Outside Tokio — main thread before/after runtime, or detached thread.
                // Acquire the runtime handle (not block_on directly via locked mutex) to
                // avoid a deadlock: ynz_rt_spawn_blocking and ynz_rt_spawn also acquire
                // the same RUNTIME mutex. Holding the lock across block_on would deadlock
                // any background call made inside the future.
                let rt_guard = RUNTIME.get().expect(
                    "ynz runtime: ynz_rt_run_entrypoint called before ynz_rt_init. \
                         This is a compiler codegen bug — ynz_rt_init must be the first \
                         instruction in main for any program using wait or background.",
                );
                // Get the Tokio Handle (cheap clone), then RELEASE the lock before block_on.
                let handle = {
                    let lock = match rt_guard.lock() {
                        Ok(l) => l,
                        Err(e) => e.into_inner(),
                    };
                    lock.as_ref()
                        .expect("ynz runtime: ynz_rt_run_entrypoint called after ynz_rt_shutdown.")
                        .handle()
                        .clone()
                }; // mutex released here — before block_on
                handle.block_on(future)
            }
        }
    });
    match result {
        Ok(value) => value,
        Err(e) => std::panic::resume_unwind(e),
    }
}

// The CPU-parallel join shims back the joinable CPU-spawn mechanism for pure-CPU statement
// parallelization. ABI invariants (locked in the P0 Decision Record):
//
//   YnzCpuResult = [i64; 2] (16-byte POD, covers every supported return class:
//     int/bool: [i64_val, 0], float: [f64_bits, 0], string/array/map: [ptr_bits, 0],
//     number/decimal128: [lo, hi], T-errors: [err_tag, ok_bits]).
//
//   ynz_rt_spawn_blocking_joinable: copies ctx, spawns on the blocking pool,
//     returns heap-boxed JoinHandle<YnzCpuResult> as *mut u8.
//   ynz_rt_join_poll: polls the handle with the real forwarded waker (NO fabricated
//     wakers — identical discipline to ynz_rt_async_sleep_poll).
//   ynz_rt_join_handle_free: detaches (drops) a handle that was never polled to Ready.
//
// The join is POLL-BASED — returning Pending from ynz_rt_join_poll suspends the
// enclosing state-machine and hands the thread back to the scheduler. This is the
// key invariant: no synchronous join (block_on, thread::park, spin-wait) anywhere in
// these call paths. Violation would reintroduce the M2-HALT block_on corpse.

/// The 16-byte return type for CPU-spawned children.
///
/// Every Yinz return class that may be returned from a CPU child maps to two i64 fields:
///   int / bool             → [value_bits, 0]
///   float                  → [f64_as_i64_bits, 0]   (bit-cast, not truncation)
///   string / array / map   → [heap_ptr_as_i64, 0]
///   number (decimal128)    → [lo_word, hi_word]
///   T errors (EC pair)     → [err_tag, ok_bits]
///
/// Shape and Shape-errors returns are NOT in this contract: the promotion pass declines
/// candidates whose callee returns a Shape type (WideValueSuspendingReturn decline rule).
///
/// Alignment: the frame result slot must be 16-byte aligned to hold a decimal128 or EC pair
/// without SIGBUS on architectures with strict alignment. Codegen ensures this via its
/// alloca-with-alignment path for result slots.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct YnzCpuResult(pub [i64; 2]);

/// A heap-boxed JoinHandle for a CPU-spawned child task.
///
/// Opaque to the codegen: stored as `*mut u8` in the parent's frame handle slot.
/// Ownership protocol:
///   - Created by `ynz_rt_spawn_blocking_joinable`; pointer returned to caller.
///   - Consumed (Ready): `ynz_rt_join_poll` drops the Box on Ready. Pointer is dangling after.
///   - Dropped (drop path): `ynz_rt_join_handle_free` drops the Box (detach — task runs to completion).
///   - The two paths are mutually exclusive by construction: codegen nulls the handle slot
///     after each Ready poll and the drop shim only fires on non-null slots.
///
/// `pub(crate)` so unit tests in lib.rs can construct a panicking handle directly
/// without going through the `extern "C" fn` boundary (which aborts on panic, RFC 2945).
/// The inner field is private — all construction goes through `CpuJoinHandle::new` so
/// codegen cannot directly call `.abort()` or `.poll()` on the handle (those paths are
/// mutually exclusive and must stay that way: Ready poll drops the box; drop-shim detaches).
pub(crate) struct CpuJoinHandle {
    inner: tokio::task::JoinHandle<YnzCpuResult>,
    /// Test-only per-handle drop probe. When this specific handle is dropped, the Arc
    /// counter increments. Injected via `set_drop_probe` before boxing — scoped to ONE
    /// handle so concurrent drops from other tests can never race into the assertion window.
    #[cfg(test)]
    probe: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
}

impl CpuJoinHandle {
    /// Wrap a `JoinHandle<YnzCpuResult>` into the opaque handle type.
    ///
    /// The only construction path — keeps callers from holding a direct `JoinHandle`
    /// reference, which would let them call `.abort()` independently of the slot-null
    /// protocol that prevents double-frees.
    pub(crate) fn new(h: tokio::task::JoinHandle<YnzCpuResult>) -> Self {
        crate::handle_counter_record_alloc();
        CpuJoinHandle {
            inner: h,
            #[cfg(test)]
            probe: None,
        }
    }

    /// Test-only: inject a drop probe into this handle.
    ///
    /// When the handle is dropped (via Box::from_raw + drop), the Arc counter increments.
    /// Assertions compare before/after count for THIS specific handle — unaffected by
    /// concurrent drops from other tests because the Arc is not shared globally.
    #[cfg(test)]
    pub(crate) fn set_drop_probe(&mut self, arc: std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        self.probe = Some(arc);
    }
}

impl Drop for CpuJoinHandle {
    /// Every handle-free path — Ready poll (`ynz_rt_join_poll`), detach free
    /// (`ynz_rt_join_handle_free`), and cancellation cleanup (`cleanup_spike_cpu_handles`)
    /// — destroys the handle VALUE via `Box::from_raw` + drop, so this `Drop` is the one
    /// choke point that observes them all for the env-gated handle parity counter.
    fn drop(&mut self) {
        crate::handle_counter_record_free();
        #[cfg(test)]
        if let Some(probe) = &self.probe {
            probe.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// Schedule a pure-CPU function on the blocking thread pool and return a joinable handle.
///
/// # Flow
/// 1. Attempt admission via `PendingBlockingGuard::try_new` — rejects (returns null,
///    allocates nothing) if `ynz_rt_shutdown` has already begun, closing the
///    shutdown-admission TOCTOU race.
/// 2. Copy `ctx_size` bytes from `ctx_ptr` to a heap buffer (RAII via `FrameDropGuard`).
///    The child owns its ctx copy; the parent frame may be dropped at any time without
///    dangling the child's args (the UAF-on-cancellation fix from Research Finding 3).
/// 3. Spawn via `Handle::spawn_blocking(closure)`. The closure calls `fn_ptr(ctx_heap_ptr)`
///    and returns the `YnzCpuResult`. `spawn_blocking` is non-async: the CPU work runs on
///    a dedicated blocking-pool OS thread, not on an I/O event-loop thread.
/// 4. Box the `JoinHandle<YnzCpuResult>` — gives it a stable heap address for the frame
///    handle slot. Return the box pointer as `*mut u8`.
///
/// # Failure modes
/// - `ynz_rt_init` was never called: logs a warning, returns null. Caller must treat null
///   as "run inline sequentially" (codegen always runs after ynz_rt_init in generated main;
///   null only occurs in hand-written misuse). Polling a null handle aborts with a message.
/// - `ynz_rt_shutdown` was already called (or has already begun draining): logs a
///   warning, returns null. Nothing is allocated on this path.
/// - The CPU closure panics: caught by the JoinHandle as `JoinError::is_panic()`.
///   `ynz_rt_join_poll` re-raises via `resume_unwind` so the parent's panic handler takes over —
///   matching the observable behavior of sequential execution on the same panicking callee.
/// - `ctx_size == 0` or `ctx_ptr` is null: `fn_ptr(null)` is called in the child.
///
/// # Side effects
/// Time: O(n) where n = ctx_size bytes (the heap ctx copy)  Space: O(n) (ctx buffer +
/// one fixed-size `Box<CpuJoinHandle>`).
/// Heap-allocates `ctx_size` bytes (ctx copy) and one `Box<CpuJoinHandle>` per call.
/// Both are freed by `ynz_rt_join_poll` on Ready (the normal path) or by
/// `ynz_rt_join_handle_free` on the drop path. No double-free is possible because
/// the two paths are mutually exclusive.
///
/// # Safety
/// - `ctx_ptr` must point to at least `ctx_size` valid bytes for the duration of this call.
///   The bytes are copied; ownership of the original buffer stays with the caller.
/// - `fn_ptr` must be safe to call with a single `*mut u8` argument on a blocking-pool thread.
#[no_mangle]
pub unsafe extern "C" fn ynz_rt_spawn_blocking_joinable(
    fn_ptr: extern "C-unwind" fn(*mut u8) -> YnzCpuResult,
    ctx_ptr: *mut u8,
    ctx_size: i64,
) -> *mut u8 {
    // Resolve the runtime handle using the same ladder as ynz_rt_spawn_blocking:
    // try_current() first (avoids the RUNTIME mutex when already inside Tokio),
    // then fall through to the global RUNTIME for the main-thread path.
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => {
            let Some(guard) = RUNTIME.get() else {
                eprintln!(
                    "ynz runtime: ynz_rt_spawn_blocking_joinable called before ynz_rt_init — handle is null"
                );
                return std::ptr::null_mut();
            };
            let lock = match guard.lock() {
                Ok(l) => l,
                Err(e) => e.into_inner(),
            };
            match lock.as_ref() {
                Some(rt) => rt.handle().clone(),
                None => {
                    eprintln!("ynz runtime: ynz_rt_spawn_blocking_joinable called after ynz_rt_shutdown — handle is null");
                    return std::ptr::null_mut();
                }
            }
        }
    };

    // Admission check BEFORE the ctx heap copy — see `PendingBlockingGuard::try_new` for
    // the increment-then-check protocol that closes the shutdown-admission TOCTOU race
    // (same rationale as `ynz_rt_spawn_blocking`, above). Checking here means a rejected
    // task never allocates the ctx copy or the `Box<CpuJoinHandle>`.
    let Some(_pending) = PendingBlockingGuard::try_new() else {
        eprintln!(
            "ynz runtime: ynz_rt_spawn_blocking_joinable called after ynz_rt_shutdown began — handle is null"
        );
        return std::ptr::null_mut();
    };

    // Copy ctx bytes to the heap. The FrameDropGuard moves into the closure so it
    // runs on both normal return and panic unwind — preventing a ctx leak if the
    // child panics before fn_ptr returns.
    // SAFETY: ctx_ptr is valid for ctx_size bytes (this function's own safety contract,
    // forwarded to `copy_ctx_to_heap`).
    let (ctx_heap_ptr, ctx_heap_len) = copy_ctx_to_heap(ctx_ptr, ctx_size);

    let ctx_guard = FrameDropGuard {
        ptr: ctx_heap_ptr,
        len: ctx_heap_len,
        // Drains the per-spawn ctx-free probe armed by tests on this thread (read here, on the
        // caller's thread, so the Arc moves into the closure with the guard). None in production.
        #[cfg(test)]
        free_probe: CTX_FREE_PROBE.with(|p| p.borrow_mut().take()),
    };

    // Spawn on the blocking pool. The closure is Send because FrameDropGuard: Send.
    // spawn_blocking returns a JoinHandle<YnzCpuResult> immediately (non-async).
    //
    // WHY the inner catch_unwind around fn_ptr: even with `fn_ptr` correctly typed
    // `extern "C-unwind"` (M6 Phase 6b fix — was `extern "C"`, which is UB to invoke a
    // Rust-ABI/unwind-capable callee through: Miri caught this live as "calling a function
    // with calling convention Rust using calling convention C" once a real Miri run reached
    // a test that panics inside `fn_ptr`), Tokio's own task-harness catch_unwind boundary
    // sits OUTSIDE `spawn_blocking`'s closure, not around this specific call — capturing the
    // unwind here, then re-raising it as a native Rust panic, lets Tokio's harness catch it
    // and surface it as `JoinError::is_panic()` at the JoinHandle. That makes
    // ynz_rt_join_poll's Ready(Err(panic)) → resume_unwind branch reachable at the JoinHandle
    // boundary. (End-to-end propagation all the way back to the user's panic handler
    // additionally requires the SM resume functions that CALL ynz_rt_join_poll to be emitted
    // as `extern "C-unwind"`; that codegen ABI flip lands in a later phase.) Mirrors the
    // catch_unwind in ynz_rt_spawn_blocking.
    // See PendingBlockingGuard's doc comment (ynz_rt_spawn_blocking, above) — same
    // confirmed-under-TSan task-loss race applies here; the guard must survive the
    // `resume_unwind` panic path below, which it does since Drop still runs on unwind.
    let join_handle = handle.spawn_blocking(move || {
        // Drop `_pending` FIRST — see the identical rationale in `ynz_rt_spawn_blocking`,
        // above. This is exactly the mechanism the recursion-chain spike-cancellation
        // fixture exercises: a chain child's two `burn()` calls are joinable spawn_blocking
        // tasks running ~600ms each, and holding `_pending` for their whole body starved
        // `ynz_rt_shutdown`'s `shutdown_timeout` phase of its own remaining budget.
        // See the Tokio-source citation on the `rt.shutdown_timeout(remaining)` call in
        // `ynz_rt_shutdown`, above, for the confirmed mechanism this early drop relies on.
        drop(_pending);

        let ctx_for_call = ctx_guard.ptr;
        let _guard = ctx_guard; // freed on normal return and on unwind

        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fn_ptr(ctx_for_call)));
        match result {
            Ok(v) => v,
            Err(payload) => std::panic::resume_unwind(payload),
        }
        // YnzCpuResult is Copy/trivially-movable — no special drop needed.
    });

    // Box the JoinHandle to give it a stable heap address. The codegen stores the
    // returned pointer in the parent frame's handle slot.
    let boxed = Box::new(CpuJoinHandle::new(join_handle));
    Box::into_raw(boxed) as *mut u8
}

/// Poll an in-flight CPU join handle created by `ynz_rt_spawn_blocking_joinable`.
///
/// # Flow
/// 1. Cast `handle_ptr` back to `*mut CpuJoinHandle` and pin it mutably.
/// 2. Cast `waker_ctx` back to `&mut Context<'_>` — the real Tokio waker forwarded
///    from the enclosing state-machine poll. No fabricated Wakers — same discipline
///    as `ynz_rt_async_sleep_poll`.
/// 3. Call `Pin::new(&mut join_handle).poll(cx)`.
/// 4. On `Pending`: the Tokio task runtime has registered the real waker; it will be
///    woken when the blocking task finishes. Return 1 (Pending) so the state-machine
///    saves resume_point and yields Pending up to its own caller.
/// 5. On `Ready(Ok(result))`: write 16 bytes to `result_out` (the frame result slot),
///    drop the `Box<CpuJoinHandle>`, return 0 (Ready).
/// 6. On `Ready(Err(join_err))` where `join_err.is_panic()`: the child panicked.
///    Drop the `Box<CpuJoinHandle>` (the JoinError already carries the panic payload
///    independently of the box), then re-raise via `resume_unwind` so the parent's
///    panic handler fires — matching the observable behavior of sequential execution
///    on a panicking callee. (Other JoinError variants — like abort — are treated as
///    panics for the same reason, and free the box the same way before panicking.)
///
/// # Result layout in `result_out`
/// Writes exactly 16 bytes: `[lo: i64, hi: i64]` in little-endian host byte order.
/// The frame result slot must be at least 16-byte aligned (codegen ensures this).
///
/// # Failure modes
/// - `handle_ptr` is null: aborts with a clear message (indicates codegen bug — the
///   parent frame should not be polling a null handle slot).
/// - Child panicked: `resume_unwind` re-raises. Program terminates with the same
///   diagnostic as sequential execution of the panicking callee.
///
/// # Side effects
/// Time: O(1)  Space: O(1) — one poll, writes ≤16 bytes, no loop; frees one box on Ready
/// (success OR panic/abort — see item 6 above; confirmed via Miri, M6 Phase 6b, that the
/// panic/abort arms must free the box too, not just the success arm).
/// On Ready(Ok): frees the `Box<CpuJoinHandle>` heap allocation (the JoinHandle is dropped,
/// detaching the blocking thread if it hasn't already finished). Writes 16 bytes to `result_out`.
///
/// # Safety
/// - `handle_ptr` must be a non-null pointer previously returned by
///   `ynz_rt_spawn_blocking_joinable` and not yet consumed (i.e., poll has not yet returned 0
///   for this handle).
/// - `waker_ctx` must be a valid `*mut u8` pointing to a live `&mut Context<'_>` for
///   the duration of this call (same contract as `ynz_rt_async_sleep_poll`).
/// - `result_out` must be valid and writable for at least 16 bytes, and at least 8-byte
///   aligned (i64 writes). Codegen ensures 16-byte alignment for decimal128 correctness.
/// # ABI note: `extern "C-unwind"`
///
/// This function uses `extern "C-unwind"` instead of `extern "C"` because on the
/// `Ready(Err(panic))` path it calls `std::panic::resume_unwind`, which initiates a Rust
/// unwind. An `extern "C"` function that unwinds aborts the process (Rust RFC 2945); an
/// `extern "C-unwind"` function allows the unwind to propagate to the caller.
///
/// Current deployment: `ynz_rt_join_poll` is called from `SpawnStateFnFuture::poll`
/// (pure Rust, no C boundary) and — once codegen wires it — from the emitted SM resume
/// functions. Those resume functions are emitted as `extern "C"` today, so an unwind
/// originating here that reaches the SM resume boundary aborts the process at that boundary.
/// Full end-to-end `C-unwind` propagation (resume functions emitted as `extern "C-unwind"`
/// so the unwind reaches the user's panic path) is the codegen ABI flip that lands with the
/// SM-promotion lowering phase — it is NOT part of this runtime-ABI phase. The runtime side
/// (this function's `extern "C-unwind"` ABI + the `catch_unwind` re-raise in
/// `ynz_rt_spawn_blocking_joinable`) is complete and pinned by the unit tests below.
/// deferral-tracked: .claude/plans/active/v0-3-m3d-cpu-parallelization.md Phase 3 (SM-promotion lowering — end-to-end C-unwind resume-fn ABI)
#[no_mangle]
pub unsafe extern "C-unwind" fn ynz_rt_join_poll(
    handle_ptr: *mut u8,
    waker_ctx: *mut u8,
    result_out: *mut u8,
) -> i32 {
    if handle_ptr.is_null() {
        // Null handle means codegen emitted a poll on a slot that was already Ready or
        // was never set — either is a codegen bug.
        panic!(
            "ynz runtime: ynz_rt_join_poll called with null handle (codegen bug — \
             poll a join handle slot that was already consumed or never initialised)"
        );
    }

    // SAFETY: handle_ptr was returned by ynz_rt_spawn_blocking_joinable (Box::into_raw
    // of Box<CpuJoinHandle>). Valid, aligned, exclusively owned by this call.
    let join_box = &mut *(handle_ptr as *mut CpuJoinHandle);
    // SAFETY: waker_ctx was cast from &mut Context<'_> by the enclosing SM's poll.
    // Valid for the duration of this call.
    let cx = &mut *(waker_ctx as *mut Context<'_>);

    // Pin the JoinHandle reference for polling. JoinHandle<T> implements Future<Output=Result<T,JoinError>>.
    // The pin is trivial here — JoinHandle is Unpin.
    match std::pin::Pin::new(&mut join_box.inner).poll(cx) {
        Poll::Pending => 1i32,
        Poll::Ready(Ok(result)) => {
            // Write 16 bytes to the frame result slot before dropping the handle.
            // SAFETY: result_out is valid for 16 bytes (caller guarantee from frame layout).
            let out = result_out as *mut [i64; 2];
            *out = result.0;
            // Drop the Box (frees the JoinHandle) — safe because we've already read the result.
            drop(Box::from_raw(handle_ptr as *mut CpuJoinHandle));
            0i32
        }
        Poll::Ready(Err(join_err)) => {
            // Child panicked (real user-code panic) or was aborted (Tokio-internal
            // cancellation — see the `is_panic == false` arm below for how that actually
            // arises). The two arms free the Box differently; see each arm's own comment
            // for why they are NOT symmetric despite both ending in a loud failure.
            let is_panic = join_err.is_panic();
            if is_panic {
                // Box ownership on this path: the JoinHandle inside the Box is already
                // exhausted by the JoinError extraction (the result was consumed), so there
                // is no double-free risk in freeing it now. Free the Box BEFORE unwinding —
                // confirmed via Miri (M6 Phase 6b) that leaving it for "the program is
                // terminating anyway" was a real, unbounded leak: resume_unwind's payload
                // can be (and, in this crate's own tests, is) caught by an ancestor
                // catch_unwind rather than crashing the process, so any long-running
                // process that catches a CPU-child panic would leak one CpuJoinHandle
                // (plus the tokio task allocation it references) per occurrence, not just
                // "at most one, ever." Extracting the panic payload first and freeing the
                // Box before calling resume_unwind closes that leak without changing the
                // observable re-raise behavior. This arm fires on a genuine user-code panic,
                // which can happen at ANY point during normal (non-shutdown) execution, so
                // the "program is terminating anyway" excuse was genuinely false here.
                let payload = join_err.into_panic();
                drop(Box::from_raw(handle_ptr as *mut CpuJoinHandle));
                std::panic::resume_unwind(payload);
            } else {
                // Abort (non-panic cancellation). `CpuJoinHandle`'s inner JoinHandle is
                // never explicitly `.abort()`'d anywhere in this crate (verified: no
                // `.abort()` call site exists on any `CpuJoinHandle`) — the ONLY way this
                // arm is reachable is Tokio's OWN internal blocking-pool cancellation of a
                // still-queued task during `ynz_rt_shutdown`'s `rt.shutdown_timeout()` (see
                // `PENDING_BLOCKING_TASKS`'s doc comment above). That means, unlike the
                // is_panic arm, this arm is reachable ONLY while the runtime is actively
                // tearing itself down — "the program is terminating anyway" is actually TRUE
                // here, so the Box is deliberately NOT freed (same leak-but-safe choice this
                // function made before M6 Phase 6b, restored here after a real regression).
                //
                // Freeing the Box here — dropping the inner JoinHandle — was tried during
                // M6 Phase 6b (by analogy with the is_panic arm above, without its own
                // confirmed-leak repro) and caused a live, reproducible crash: freeing the
                // JoinHandle for a task Tokio's own shutdown machinery just cancelled races
                // with that same shutdown machinery's concurrent teardown of the task's
                // shared internal state, corrupting Tokio's own task bookkeeping (observed
                // as a misaligned-pointer abort inside `tokio::runtime::task::core::Core`'s
                // atomic accessor, on the `ynz-driver` integration test
                // `v03_m3g_background_fused_group_detach_no_leak_and_rate_unchanged`,
                // reproducible within ~1-20 runs; 100+ runs clean once the free was removed
                // again). Leaving this leaked (bounded to at most once per detached
                // background CPU child that loses the shutdown race, never accumulating
                // since the process is already exiting) is the correct, verified tradeoff.
                panic!("ynz runtime: CPU child task was aborted before it could produce a result");
            }
        }
    }
}

/// Detach a CPU join handle, freeing it without collecting its result.
///
/// Called by the parent frame's drop shim when the parent is cancelled mid-join:
/// a frame handle slot that is non-null at drop time has an in-flight child that was
/// never polled to Ready. Dropping the JoinHandle detaches it — the blocking-pool task
/// runs to completion, results are discarded. No UAF: the child owns its ctx copy.
///
/// This mirrors the sleep-handle free discipline: `SpawnStateFnFuture::drop` frees the
/// sleep_handle slot on cancellation; this function frees the join handle slot on
/// cancellation.
///
/// # Flow
/// Drop the `Box<CpuJoinHandle>` unconditionally; detaches the blocking-pool child.
///
/// # Failure modes
/// Null `handle_ptr` → no-op early return.
///
/// # Side effects
/// Time: O(1)  Space: O(1) — drops one `Box<CpuJoinHandle>`, no loop.
/// Frees the heap Box; the detached child runs to completion and its result is discarded.
///
/// # Idempotence
/// Never called after a Ready poll — the parent frame's drop shim only fires on slots
/// whose pointer is non-null, and codegen nulls the handle slot when `ynz_rt_join_poll`
/// returns 0 (Ready). Double-free is impossible by construction.
///
/// # Safety
/// - `handle_ptr` must be a non-null pointer previously returned by
///   `ynz_rt_spawn_blocking_joinable` and not yet consumed (poll returned 0 or this
///   function was already called for this handle).
#[no_mangle]
pub unsafe extern "C" fn ynz_rt_join_handle_free(handle_ptr: *mut u8) {
    if handle_ptr.is_null() {
        return;
    }
    // Reconstruct and drop the Box — detaches the blocking task.
    // SAFETY: handle_ptr is valid and exclusively owned (caller guarantee).
    drop(Box::from_raw(handle_ptr as *mut CpuJoinHandle));
}
