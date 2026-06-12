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

/// Byte offset of the `sleep_handle` pointer in a codegen-emitted state-machine frame.
///
/// Frame layout (mirrors `crates/ynz-codegen/src/state_machine.rs`):
///   bytes 0–3   : resume_point (i32)
///   bytes 4–7   : padding (zero for normal SM frames; spike discriminator for spike frames)
///   bytes 8–15  : sleep_handle (*mut Pin<Box<Sleep>>, or null)
///   bytes 16+   : local-variable slots (i64 each)
///
/// This constant is the authoritative offset used by `SpawnStateFnFuture::drop` to
/// read the `sleep_handle` slot and free any in-flight `Sleep` box on task cancellation.
/// It MUST stay in sync with `FRAME_OFFSET_SLEEP_HANDLE` in state_machine.rs — both are
/// `8` and derive from the same frame layout decision.
const FRAME_SLEEP_HANDLE_OFFSET: usize = 8;

/// Byte offset of the spike-frame discriminator within a state-machine frame.
///
/// Normal frames: bytes 4-7 are always zero (ynz_alloc_zeroed guarantees it).
/// Spike frames: codegen writes SPIKE_FRAME_MAGIC here at spawn time so that
/// `SpawnStateFnFuture::drop` can distinguish a spike frame (CPU handles at 32/40)
/// from a normal SM frame (no CPU handles, no free needed at those offsets).
const FRAME_SPIKE_DISCRIMINATOR_OFFSET: usize = 4;

/// Magic value written to bytes 4-7 of a spike frame at CPU-group spawn time.
/// ASCII "SPIK". Non-spike frames always have 0 at this offset (ynz_alloc_zeroed).
/// `SpawnStateFnFuture::drop` reads this to decide whether to free handle slots 32/40.
const SPIKE_FRAME_MAGIC: u32 = 0x5350_494B;

/// Byte offset of the first CPU join handle pointer in a spike frame.
/// Slot holds *mut CpuJoinHandle (8 bytes), null when handle was consumed by Ready poll.
const SPIKE_HANDLE_0_OFFSET: usize = 32;

/// Byte offset of the second CPU join handle pointer in a spike frame.
const SPIKE_HANDLE_1_OFFSET: usize = 40;

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

/// Descriptor for one heap-allocated background arg-copy that must be freed when the
/// spawned state-machine task completes (on both normal completion and cancellation).
///
/// Codegen allocates these arg-copies via `ynz_alloc` (Shape) or `ynz_array_clone_primitive`
/// (array<primitive>) so they outlive the spawner's stack frame. The descriptor records
/// where in the frame the pointer lives so the Drop impl can recover and free it.
///
/// Layout (repr C, matches the alloca'd array in codegen):
///   byte_offset: u64  — byte offset from frame_ptr to the i64 slot holding the heap pointer
///   kind: u64         — 0 = HeapShape (free with ynz_free(ptr, size)), 1 = HeapArray (free with ynz_array_drop(ptr))
///   size: u64         — byte count passed to ynz_free for HeapShape (ignored for HeapArray)
#[repr(C)]
pub struct BgArgDropEntry {
    pub byte_offset: u64,
    pub kind: u64,
    pub size: u64,
}

/// Drives a Yinz codegen-emitted state-machine resume function as a fire-and-forget
/// Tokio Future. Return value is intentionally discarded — `ynz_rt_spawn` callers
/// use channels or atomics to observe completion.
///
/// Separate from `SyncStateFnFuture` so `ynz_rt_spawn`'s external C-ABI signature
/// stays `void` (no return channel at the ABI level).
pub(crate) struct SpawnStateFnFuture {
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
    /// Pointer to a `ynz_alloc`'d array of `BgArgDropEntry` describing heap arg-copies
    /// stored in this frame's local slots. Null when no heap arg-copies exist (e.g., the
    /// callee takes only primitives or strings).
    ///
    /// The array is heap-allocated by codegen at spawn time and freed here in Drop after
    /// all arg-copies have been released — exactly once, on every exit path.
    arg_drop_ptr: *const BgArgDropEntry,
    /// Number of entries at `arg_drop_ptr`. 0 when `arg_drop_ptr` is null.
    arg_drop_count: usize,
}

impl SpawnStateFnFuture {
    /// Construct a `SpawnStateFnFuture` for testing. The `new_*` path exists so tests
    /// can build the future, hold it locally, and drop it without having to spawn it
    /// (which would require a live Tokio runtime and would prevent observing drop behaviour
    /// on the frame before the task runs).
    #[cfg(test)]
    pub(crate) fn new(
        resume_fn: unsafe extern "C" fn(*mut u8, *mut u8) -> i32,
        frame_ptr: *mut u8,
        frame_size: i64,
        rec_slot: *mut u8,
        arg_drop_ptr: *const BgArgDropEntry,
        arg_drop_count: i64,
    ) -> Self {
        let _ = rec_slot; // tested via recursion_slot_offset=-1 path
        Self {
            resume_fn,
            frame_ptr,
            frame_size,
            recursion_slot_offset: -1,
            arg_drop_ptr,
            arg_drop_count: arg_drop_count as usize,
        }
    }
}

// SAFETY: SpawnStateFnFuture is owned exclusively by the spawned task for its lifetime.
unsafe impl Send for SpawnStateFnFuture {}

/// Free live CPU join handles stored in a spike frame, if any.
///
/// Reads the discriminator at frame offset 4. Normal frames always have 0 there
/// (ynz_alloc_zeroed initialises the frame). Spike frames have SPIKE_FRAME_MAGIC written
/// by codegen at spawn time, enabling this function to safely access handle slots at
/// offsets 32 and 40 without misreading a normal frame.
///
/// For each non-null handle slot: drops the Box<CpuJoinHandle>, detaching the blocking-pool
/// task (it runs to completion; results are discarded). Null slots are skipped — they were
/// either never spawned or already consumed by a Ready poll.
///
/// Called from `SpawnStateFnFuture::drop` on cancellation. Extracted as a `pub(crate)` helper
/// so the discriminator + handle-free logic can be tested independently without constructing
/// a full `SpawnStateFnFuture` (which requires live resume-fn scaffolding).
///
/// # Safety
/// `frame_ptr` must be a non-null, valid pointer to at least 48 bytes when the discriminator
/// matches, or at least 8 bytes for the discriminator read (4 bytes at offset 4) to be safe.
/// The spike frame allocated by codegen always satisfies this: header (32 bytes) + handle
/// region (48 bytes) = 80 bytes minimum.
pub(crate) unsafe fn cleanup_spike_cpu_handles(frame_ptr: *mut u8) {
    // Normal frames: bytes 4-7 are always zero. Spike frames: SPIKE_FRAME_MAGIC written at
    // spawn time. Only free handle slots when the discriminator confirms this is a spike frame.
    let disc_slot = frame_ptr.add(FRAME_SPIKE_DISCRIMINATOR_OFFSET) as *const u32;
    if *disc_slot == SPIKE_FRAME_MAGIC {
        // SAFETY: spike_cpu_candidates declines any host with ≥1 params, so the frame is
        // always ≥80 bytes (32-byte header + 48-byte spike region). The discriminator check
        // above is proof that this is a spike frame, making the 48-byte region accessible.
        // The compile-time assertion below documents the layout invariant: handle slots
        // must both fit within the 80-byte minimum spike frame.
        const _: () = assert!(
            SPIKE_HANDLE_1_OFFSET + 8 <= 80,
            "spike region must fit within 80-byte minimum spike frame"
        );
        for handle_offset in [SPIKE_HANDLE_0_OFFSET, SPIKE_HANDLE_1_OFFSET] {
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
            // SAFETY: FRAME_SLEEP_HANDLE_OFFSET=8 is within every valid frame header (32 bytes).
            let handle_slot = self.frame_ptr.add(FRAME_SLEEP_HANDLE_OFFSET) as *const *mut u8;
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
                        0 => {
                            // HeapShape: allocated with ynz_alloc; free with ynz_free(ptr, size).
                            crate::ynz_free(heap_ptr, desc.size as usize);
                        }
                        1 => {
                            // HeapArrayPrimitive: allocated by ynz_array_clone_primitive (malloc);
                            // free with ynz_array_drop which handles both the data buffer and the header.
                            crate::ynz_array_drop(heap_ptr as *mut crate::YnzArray);
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
    resume_fn: unsafe extern "C" fn(*mut u8, *mut u8) -> i32,
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
        arg_drop_ptr,
        arg_drop_count: arg_drop_count as usize,
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

// ── v0.3-M3d: CPU-parallel join shims ─────────────────────────────────────────
//
// These three shims back the joinable CPU-spawn mechanism for pure-CPU statement
// parallelization. ABI invariants (locked in P0 Decision Record):
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

#[cfg(test)]
impl Drop for CpuJoinHandle {
    fn drop(&mut self) {
        if let Some(probe) = &self.probe {
            probe.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// Schedule a pure-CPU function on the blocking thread pool and return a joinable handle.
///
/// # Flow
/// 1. Copy `ctx_size` bytes from `ctx_ptr` to a heap buffer (RAII via `FrameDropGuard`).
///    The child owns its ctx copy; the parent frame may be dropped at any time without
///    dangling the child's args (the UAF-on-cancellation fix from Research Finding 3).
/// 2. Spawn via `Handle::spawn_blocking(closure)`. The closure calls `fn_ptr(ctx_heap_ptr)`
///    and returns the `YnzCpuResult`. `spawn_blocking` is non-async: the CPU work runs on
///    a dedicated blocking-pool OS thread, not on an I/O event-loop thread.
/// 3. Box the `JoinHandle<YnzCpuResult>` — gives it a stable heap address for the frame
///    handle slot. Return the box pointer as `*mut u8`.
///
/// # Failure modes
/// - `ynz_rt_init` was never called: logs a warning, returns null. Caller must treat null
///   as "run inline sequentially" (codegen always runs after ynz_rt_init in generated main;
///   null only occurs in hand-written misuse). Polling a null handle aborts with a message.
/// - `ynz_rt_shutdown` was already called: logs a warning, returns null.
/// - The CPU closure panics: caught by the JoinHandle as `JoinError::is_panic()`.
///   `ynz_rt_join_poll` re-raises via `resume_unwind` so the parent's panic handler takes over —
///   matching the observable behavior of sequential execution on the same panicking callee.
/// - `ctx_size == 0` or `ctx_ptr` is null: `fn_ptr(null)` is called in the child.
///
/// # Side effects
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
    fn_ptr: extern "C" fn(*mut u8) -> YnzCpuResult,
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

    // Copy ctx bytes to the heap. The FrameDropGuard moves into the closure so it
    // runs on both normal return and panic unwind — preventing a ctx leak if the
    // child panics before fn_ptr returns.
    let (ctx_heap_ptr, ctx_heap_len): (*mut u8, usize) = if ctx_size > 0 && !ctx_ptr.is_null() {
        let len = ctx_size as usize;
        let mut buf: Box<[u8]> = vec![0u8; len].into_boxed_slice();
        // SAFETY: ctx_ptr is valid for ctx_size bytes (caller guarantee). The source and
        // destination are non-overlapping (heap allocation vs caller's stack/frame).
        std::ptr::copy_nonoverlapping(ctx_ptr, buf.as_mut_ptr(), len);
        let raw = buf.as_mut_ptr();
        std::mem::forget(buf); // ownership moves into FrameDropGuard below
        (raw, len)
    } else {
        (std::ptr::null_mut(), 0)
    };

    let ctx_guard = FrameDropGuard {
        ptr: ctx_heap_ptr,
        len: ctx_heap_len,
    };

    // Spawn on the blocking pool. The closure is Send because FrameDropGuard: Send.
    // spawn_blocking returns a JoinHandle<YnzCpuResult> immediately (non-async).
    //
    // WHY catch_unwind here: fn_ptr is `extern "C" fn`, and RFC 2945 / Rust 1.71+
    // specifies that a panic crossing an `extern "C"` boundary aborts the process —
    // before Tokio's own catch_unwind (which wraps the closure result) can form a
    // JoinError. Capturing the unwind here converts it into a native Rust panic inside
    // Tokio's harness, so ynz_rt_join_poll's Ready(Err(panic)) → resume_unwind path
    // becomes reachable end-to-end. Mirrors ynz_rt_spawn_blocking at runtime.rs:198-200.
    let join_handle = handle.spawn_blocking(move || {
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
///    Re-raise via `resume_unwind` so the parent's panic handler fires — matching
///    the observable behavior of sequential execution on a panicking callee.
///    (Other JoinError variants — like abort — are treated as panics for the same reason.)
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
/// On Ready: frees the `Box<CpuJoinHandle>` heap allocation (the JoinHandle is dropped,
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
/// (pure Rust, no C boundary) and from the codegen-emitted SM resume functions. The SM
/// resume functions are emitted as `extern "C"` — an unwind originating here that reaches
/// the SM resume boundary will abort the process. Full end-to-end `C-unwind` propagation
/// (resume functions emitted as `extern "C-unwind"`) is a future deliverable for P1.
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
            // Child panicked (or was aborted — treated identically for sequential-parity).
            // Re-raise via resume_unwind so the parent's panic handler takes over,
            // matching the observable behavior of sequential execution on a panicking callee.
            // The Box is intentionally not freed here: resume_unwind unwinds through this
            // call frame, so FrameDropGuard patterns won't fire on the local allocation —
            // but the JoinHandle is already exhausted by the JoinError extraction, so no
            // double-free occurs. The Box leaks on this path (bounded: one per panicking
            // child), UNLESS SpawnStateFnFuture::drop rescues it. For spike frames,
            // SpawnStateFnFuture::drop reads the discriminator at frame offset 4, detects
            // the spike magic, and frees any non-null handle slots at offsets 32/40 — so
            // the leak is bounded to non-spike-frame callers (which is only the P0 spike
            // test path). Production P1+ codegen always emits spike frames and is rescued.
            if join_err.is_panic() {
                std::panic::resume_unwind(join_err.into_panic());
            } else {
                // Abort (non-panic cancellation via JoinHandle::abort). Treat as a panic
                // with a clear message so the parent sees a loud failure rather than a
                // silent wrong value.
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
