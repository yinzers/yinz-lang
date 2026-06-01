/// Tokio-backed scheduler runtime for Yinz compiled binaries.
///
/// C-ABI entry points exported from this module:
///   - `ynz_rt_init()` — create the Tokio runtime at program start
///   - `ynz_rt_spawn_blocking(fn_ptr, ctx_ptr, ctx_size)` — blocking-pool background task
///   - `ynz_rt_spawn(resume_fn, frame_ptr, frame_size)` — I/O-pool state-machine task (M2)
///   - `ynz_rt_async_sleep_create(ms)` — allocate a boxed Tokio Sleep future (M2)
///   - `ynz_rt_async_sleep_poll(handle_ptr, waker_ctx)` — poll an in-flight sleep (M2)
///   - `ynz_rt_run_entrypoint(resume_fn, frame_ptr, frame_size)` — program-entry state-machine driver (M2)
///   - `ynz_rt_check_preempt()` — cooperative yield point at loop back-edges + call sites
///   - `ynz_rt_shutdown()` — drain the runtime at program end
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

/// Byte offset of the `sleep_handle` pointer in a codegen-emitted state-machine frame.
///
/// Frame layout (mirrors `crates/ynz-codegen/src/state_machine.rs`):
///   bytes 0–3   : resume_point (i32)
///   bytes 4–7   : padding
///   bytes 8–15  : sleep_handle (*mut Pin<Box<Sleep>>, or null)
///   bytes 16+   : local-variable slots (i64 each)
///
/// This constant is the authoritative offset used by `SpawnStateFnFuture::drop` to
/// read the `sleep_handle` slot and free any in-flight `Sleep` box on task cancellation.
/// It MUST stay in sync with `FRAME_OFFSET_SLEEP_HANDLE` in state_machine.rs — both are
/// `8` and derive from the same frame layout decision.
const FRAME_SLEEP_HANDLE_OFFSET: usize = 8;

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
    // Latch the alloc-counter flag once here so per-alloc cost is a cheap atomic load.
    // The env var read (lock + heap + UTF-8 scan) happens only once at program start.
    crate::init_alloc_counter_flag();

    let mutex = RUNTIME.get_or_init(|| Mutex::new(None));
    let mut lock = mutex.lock().expect("ynz_rt_init: mutex poisoned");
    if lock.is_none() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(num_cpus::get())
            .enable_all()
            .build()
            .expect("ynz_rt_init: failed to create Tokio runtime");
        *lock = Some(rt);
    }
    // If already initialised (e.g., double-init in the same program or after shutdown
    // in a test harness that re-calls init), this is a no-op. The alloc-counter flag
    // call above is idempotent: re-reading the same env var produces the same value.
}

/// RAII guard that frees the heap frame when dropped (normal return AND unwind).
///
/// Used by both `ynz_rt_spawn_blocking` (ctx copy) and `ynz_rt_spawn` (state-machine
/// frame). The guard holds a raw pointer + length; on drop it reconstructs the Box<[u8]>
/// and lets Rust free it, running on both the happy path and any panic unwind.
struct FrameDropGuard {
    ptr: *mut u8,
    len: usize,
}

// SAFETY: FrameDropGuard wraps a heap pointer owned exclusively by one background task.
// It never leaves that task's closure, so Send is safe here.
unsafe impl Send for FrameDropGuard {}

impl Drop for FrameDropGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() && self.len > 0 {
            // SAFETY: ptr was allocated with Box<[u8]>::into_raw. This drop runs exactly
            // once per FrameDropGuard value (captured by value into the closure).
            let slice = unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) };
            let _ = unsafe { Box::from_raw(slice as *mut [u8]) };
        }
    }
}

/// Schedule a function on the blocking thread pool.
///
/// # Flow
/// 1. Copy `ctx_size` bytes from `ctx_ptr` to a heap-allocated buffer.
/// 2. Transfer ownership of the heap buffer into the closure via a RAII drop guard.
/// 3. Wrap `fn_ptr(ctx_heap)` in `catch_unwind` — the guard's `Drop` runs on both
///    the happy path and on unwind, so ctx is freed in both cases.
/// 4. Call `spawn_blocking` — returns immediately; the caller continues.
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
///
/// # Side effects
/// Spawns a Tokio blocking task; heap-allocates `ctx_size` bytes.
#[no_mangle]
pub unsafe extern "C" fn ynz_rt_spawn_blocking(
    fn_ptr: extern "C" fn(*mut u8),
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

    // Copy the context bytes onto the heap so they outlive this stack frame.
    let (ctx_heap_ptr, ctx_heap_len): (*mut u8, usize) = if ctx_size > 0 && !ctx_ptr.is_null() {
        // SAFETY: caller guarantees ctx_ptr is valid for ctx_size bytes.
        let len = ctx_size as usize;
        let mut buf: Box<[u8]> = vec![0u8; len].into_boxed_slice();
        std::ptr::copy_nonoverlapping(ctx_ptr, buf.as_mut_ptr(), len);
        let raw = buf.as_mut_ptr();
        std::mem::forget(buf); // ownership moves into CtxDropGuard below
        (raw, len)
    } else {
        (std::ptr::null_mut(), 0)
    };

    // Create the RAII guard BEFORE the closure so it's captured (not the raw *mut u8).
    // FrameDropGuard: Send is impl'd; the closure becomes Send too.
    let ctx_guard = FrameDropGuard {
        ptr: ctx_heap_ptr,
        len: ctx_heap_len,
    };

    handle.spawn_blocking(move || {
        // Guard is captured — frees ctx on both return and unwind.
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

/// Cooperative preemption checkpoint — call at loop back-edges and function call sites.
///
/// # v0.3-M1 stub — intentional no-op
///
/// This stub exists so every loop back-edge and user-function call site in
/// compiled Yinz code has the correct call site from day one. The call overhead
/// is a single `ret` in release mode.
///
/// Full preemption semantics (cooperative suspension at every call site, Go-style
/// loop-back-edge yield, Tokio budget integration) land in v0.3-M2 when state
/// machines ship. Until then, Tokio's blocking-pool threads handle their own
/// internal scheduling without assistance from this function.
///
/// # Side effects
/// None. Pure no-op.
#[no_mangle]
pub extern "C" fn ynz_rt_check_preempt() {
    // M1 stub: intentional no-op. See doc comment above.
}

/// Shut down the Tokio runtime, draining in-flight background tasks.
///
/// # Flow
/// 1. Lock the runtime mutex and take ownership via `Option::take`.
/// 2. Call `shutdown_timeout(5s)` — tasks get 5 seconds to finish; any remaining
///    are dropped (Tokio semantics per `tokio::runtime::Runtime::shutdown_timeout`).
/// 3. If ynz_rt_init was never called, or shutdown was already called, this is a no-op.
///
/// # Side effects
/// Joins OS threads. Blocks for up to 5 seconds if background tasks are still running.
///
/// When `YNZ_ALLOC_COUNTER_OUTPUT` env var is set to a file path, writes final alloc/free
/// counts to that file (one "alloc=N\nfree=M\n" pair). Used by the composed-single-alloc
/// proof in integration tests: the test sets the env var, runs the fixture, reads the file.
#[no_mangle]
pub extern "C" fn ynz_rt_shutdown() {
    let Some(guard) = RUNTIME.get() else { return };
    let mut lock = match guard.lock() {
        Ok(l) => l,
        Err(e) => e.into_inner(), // still usable after poisoning
    };
    if let Some(rt) = lock.take() {
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
                              // shutdown_timeout signals worker tasks and waits up to `timeout_ms` for
                              // async-worker task drains. It joins the blocking pool (empty here), but
                              // async workers complete via waker/signal — 50ms is wall-clock margin for
                              // task drain, not a thread join. Drops all futures (running their Drop impls)
                              // on the worker threads before returning.
        rt.shutdown_timeout(Duration::from_millis(timeout_ms));
    }
    // `lock` drops here, releasing the mutex.

    // Dump alloc counts AFTER shutdown so all Drop impls (including SpawnStateFnFuture::Drop
    // which calls ynz_free) have run. Writing before shutdown would give stale counts on
    // cancellation paths where Drop runs during shutdown.
    if let Ok(output_path) = std::env::var("YNZ_ALLOC_COUNTER_OUTPUT") {
        if !output_path.is_empty() {
            let alloc_count = crate::ynz_alloc_count();
            let free_count = crate::ynz_free_count();
            let content = format!("alloc={alloc_count}\nfree={free_count}\n");
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
//   - resume_fn: extern "C" fn(frame_ptr: *mut u8, waker_ctx: *mut u8) -> i32
//     Returns 0 = Ready, 1 = Pending.
//   - waker_ctx: *mut u8 pointing to &mut Context<'_>. The resume_fn casts back
//     via `&mut *(waker_ctx as *mut Context<'_>)`. No fabricated Wakers.
//   - frame_size: i64 byte length of the heap-allocated state-machine frame.
//   - No Tokio types appear in any C-ABI signature; all params are primitive C types.

/// Drives a Yinz codegen-emitted state-machine resume function as a Tokio Future,
/// returning the state machine's final i32 value when it completes.
///
/// The resume_fn ABI: `(frame_ptr, waker_ctx) -> i32` where 0 = Ready and 1 = Pending.
/// When Ready, the wrapper reads the typed return value directly from the frame at
/// offset 16 (the typed return slot — see `FRAME_OFFSET_RETURN_SLOT` in `state_machine.rs`).
/// The i32 returned here is a truncated legacy artifact; the wrapper ignores it and
/// reads the full typed value from the frame instead.
///
/// Used exclusively by `ynz_rt_run_entrypoint` (program-entry driver).
/// `ynz_rt_spawn` uses `SpawnStateFnFuture` (Output=()) — fire-and-forget; value discarded.
struct SyncStateFnFuture {
    resume_fn: unsafe extern "C" fn(*mut u8, *mut u8) -> i32,
    frame_ptr: *mut u8,
    // frame_size carries the byte length for the Phase 2 deallocation path.
    // Not read by poll() — the resume_fn owns the frame layout.
    // Suppressed until Phase 2 wires the dealloc path.
    #[allow(dead_code)]
    frame_size: i64,
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
                    let ret_slot_ptr = self.frame_ptr.add(16) as *const i64;
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

/// Drives a Yinz codegen-emitted state-machine resume function as a fire-and-forget
/// Tokio Future. Return value is intentionally discarded — `ynz_rt_spawn` callers
/// use channels or atomics to observe completion.
///
/// Separate from `SyncStateFnFuture` so `ynz_rt_spawn`'s external C-ABI signature
/// stays `void` (no return channel at the ABI level).
struct SpawnStateFnFuture {
    resume_fn: unsafe extern "C" fn(*mut u8, *mut u8) -> i32,
    frame_ptr: *mut u8,
    /// Byte length of the heap frame, used by `Drop` to free it via `ynz_free`.
    frame_size: i64,
    /// Byte offset of the recursion-slot pointer within the frame (-1 = no recursion slot).
    ///
    /// For recursive SM functions, codegen allocates a heap-boxed child frame for each
    /// recursive call and stores the pointer at this offset. On cancellation, the Drop
    /// impl walks this pointer chain and frees all live heap-boxed child frames before
    /// freeing the root frame. Without this, a mid-wait cancellation of a deep recursion
    /// leaks all the child frames that were live at abort time.
    recursion_slot_offset: i64,
}

// SAFETY: SpawnStateFnFuture is owned exclusively by the spawned task for its lifetime.
unsafe impl Send for SpawnStateFnFuture {}

impl Drop for SpawnStateFnFuture {
    /// Free the heap state-machine frame when the spawned task ends — on normal completion
    /// AND on cancellation (Tokio dropping the task before it finishes).
    ///
    /// Frame ownership moves into this future at spawn time, so this drop runs exactly
    /// once; the frame is never aliased.
    ///
    /// Recursion chain free: if `recursion_slot_offset >= 0`, the frame may have a heap-
    /// boxed child frame at that offset (stored there by inline-poll-heap-boxed codegen).
    /// Walk the chain — each child is the same recursive function so uses the same frame
    /// size and recursion slot offset — and free each frame. The chain terminates when a
    /// frame's recursion slot holds null (codegen zeroes the slot after `ynz_free` on the
    /// normal Ready path, so null means either "never reached this depth" or "child already
    /// freed normally").
    fn drop(&mut self) {
        if self.frame_ptr.is_null() {
            return;
        }
        unsafe {
            // Free the sleep handle in the root frame (if a sleep is live mid-wait).
            // SAFETY: FRAME_SLEEP_HANDLE_OFFSET=8 is within every valid frame header (32 bytes).
            let handle_slot = self.frame_ptr.add(FRAME_SLEEP_HANDLE_OFFSET) as *const *mut u8;
            let handle_ptr = *handle_slot;
            if !handle_ptr.is_null() {
                drop(Box::from_raw(handle_ptr as *mut Pin<Box<Sleep>>));
            }

            // Walk the recursion chain and free any live heap-boxed child frames.
            // Each recursive call stores the child frame pointer at recursion_slot_offset.
            // The chain uses the same frame_size and recursion_slot_offset at every level
            // (all frames are the same self-recursive function's layout).
            //
            // Pattern: root.rec_slot → child → child.rec_slot → grandchild → ...
            // We iterate starting from root's CHILD (root itself freed at the end).
            // Each iteration: read child.rec_slot (grandchild ptr) BEFORE freeing child,
            // then free child, then advance to grandchild.
            //
            // test-only: `YNZ_SKIP_RECURSION_DROP` bypasses the chain walk so the
            // negative-control test can verify a measurable leak without this code.
            // Production runs never set this env var; the unwrap_or(false) default
            // means the walk always runs in production.
            let skip_recursion_drop = std::env::var("YNZ_SKIP_RECURSION_DROP")
                .map(|v| !v.is_empty())
                .unwrap_or(false); // production default: run the chain walk
            if self.recursion_slot_offset >= 0 && !skip_recursion_drop {
                // SAFETY: frame_ptr is valid; recursion_slot_offset is within the frame.
                let rec_slot =
                    self.frame_ptr.add(self.recursion_slot_offset as usize) as *const *mut u8;
                let mut child_ptr = *rec_slot;
                while !child_ptr.is_null() {
                    // Free the child's sleep handle before freeing its frame.
                    let child_handle_slot =
                        child_ptr.add(FRAME_SLEEP_HANDLE_OFFSET) as *const *mut u8;
                    let child_handle = *child_handle_slot;
                    if !child_handle.is_null() {
                        drop(Box::from_raw(child_handle as *mut Pin<Box<Sleep>>));
                    }
                    // Read grandchild pointer BEFORE freeing child (use-after-free guard).
                    let grandchild_slot =
                        child_ptr.add(self.recursion_slot_offset as usize) as *const *mut u8;
                    let next_ptr = *grandchild_slot;
                    // SAFETY: child_ptr was allocated by ynz_alloc_zeroed(frame_size) and is not aliased.
                    crate::ynz_free(child_ptr, self.frame_size as usize);
                    child_ptr = next_ptr;
                }
            }

            // Free the root frame last (after the chain is fully walked).
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
/// 4. Panic inside `resume_fn` is caught by Tokio's task wrapper; the JoinHandle
///    will surface a `JoinError::is_panic()`. The spawning scope continues normally.
///
/// # Failure modes
/// - `ynz_rt_init` was never called: logs a warning and discards the task.
/// - `ynz_rt_shutdown` was already called: logs a warning and discards the task.
/// - `resume_fn` panics: caught by Tokio's task wrapper; JoinHandle carries the panic.
///
/// # Side effects
/// Enqueues a work-stealing task on the Tokio I/O pool. The frame pointed to by
/// `frame_ptr` must remain valid until the spawned future completes (ownership transfers
/// into the future at spawn time — the caller must NOT free or alias frame_ptr after
/// calling this function).
///
/// # Safety
/// - `resume_fn` must be a valid function pointer matching the `(frame, waker_ctx) -> i32` ABI.
/// - `frame_ptr` must be valid for `frame_size` bytes and exclusively owned by this call.
///   After this function returns, the caller must treat `frame_ptr` as moved — any further
///   access is undefined behaviour.
#[no_mangle]
pub unsafe extern "C" fn ynz_rt_spawn(
    resume_fn: unsafe extern "C" fn(*mut u8, *mut u8) -> i32,
    frame_ptr: *mut u8,
    frame_size: i64,
    recursion_slot_offset: i64,
) {
    let future = SpawnStateFnFuture {
        resume_fn,
        frame_ptr,
        frame_size,
        recursion_slot_offset,
    };

    // Prefer spawning via the current Tokio handle (avoids the RUNTIME mutex deadlock
    // when called from inside a block_on future — block_on holds Handle context so
    // try_current() succeeds, and we can spawn without the mutex).
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(future);
        }
        Err(_) => {
            let Some(guard) = RUNTIME.get() else {
                eprintln!("ynz runtime: ynz_rt_spawn called before ynz_rt_init — task discarded");
                return;
            };
            let handle = {
                let lock = match guard.lock() {
                    Ok(l) => l,
                    Err(e) => e.into_inner(),
                };
                match lock.as_ref() {
                    Some(rt) => rt.handle().clone(),
                    None => {
                        eprintln!("ynz runtime: ynz_rt_spawn called after ynz_rt_shutdown — task discarded");
                        return;
                    }
                }
            };
            handle.spawn(future);
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
#[no_mangle]
pub unsafe extern "C" fn ynz_rt_run_entrypoint(
    resume_fn: unsafe extern "C" fn(*mut u8, *mut u8) -> i32,
    frame_ptr: *mut u8,
    frame_size: i64,
) -> i32 {
    let result = std::panic::catch_unwind(|| {
        let future = SyncStateFnFuture {
            resume_fn,
            frame_ptr,
            frame_size,
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
