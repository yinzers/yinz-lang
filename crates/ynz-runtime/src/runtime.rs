/// Tokio-backed scheduler runtime for Yinz compiled binaries.
///
/// Four C-ABI entry points are exported:
///   - `ynz_rt_init()` — create the Tokio runtime at program start
///   - `ynz_rt_spawn_blocking(fn_ptr, ctx_ptr, ctx_size)` — schedule a background task
///   - `ynz_rt_check_preempt()` — cooperative yield point at loop back-edges + call sites
///   - `ynz_rt_shutdown()` — drain the runtime at program end
///
/// These are called by compiler-generated code; users never see Tokio types directly.
///
/// # Panic safety
///
/// Every background task body is wrapped in `std::panic::catch_unwind`. A panicking
/// background task logs via `eprintln!` and is silently discarded; it never propagates
/// to the spawning scope. The heap context uses a RAII drop guard that runs on both
/// the happy path AND on unwind, preventing ctx leaks even when the task panics.
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

// The runtime is stored in a process-global `Mutex<Option<Runtime>>`.
// `Mutex` gives `ynz_rt_shutdown` the ability to `take()` the runtime for
// `shutdown_timeout` (requires ownership). `OnceLock` initialises the Mutex
// exactly once; `ynz_rt_init` populates the `Option` on first call and can
// repopulate it after `ynz_rt_shutdown` — enabling test-harness reuse.
//
// SAFETY: `Mutex<Option<Runtime>>` is `Send + Sync`.
static RUNTIME: OnceLock<Mutex<Option<tokio::runtime::Runtime>>> = OnceLock::new();

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
    // in a test harness that re-calls init), this is a no-op.
}

/// RAII guard that frees the heap context when dropped (normal return AND unwind).
struct CtxDropGuard {
    ptr: *mut u8,
    len: usize,
}

// SAFETY: CtxDropGuard wraps a heap pointer that is passed to one specific
// background task. It never leaves that task's closure, so Send is safe here.
unsafe impl Send for CtxDropGuard {}

impl Drop for CtxDropGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() && self.len > 0 {
            // SAFETY: ptr was allocated with Box<[u8]>::into_raw in ynz_rt_spawn_blocking.
            // This drop runs exactly once per CtxDropGuard value.
            let slice = unsafe {
                std::slice::from_raw_parts_mut(self.ptr, self.len)
            };
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
    let Some(guard) = RUNTIME.get() else {
        eprintln!("ynz runtime: ynz_rt_spawn_blocking called before ynz_rt_init — task discarded");
        return;
    };
    let lock = match guard.lock() {
        Ok(l) => l,
        Err(e) => e.into_inner(),
    };
    let Some(rt) = lock.as_ref() else {
        // Runtime was shut down — best-effort: discard the task silently.
        eprintln!("ynz runtime: ynz_rt_spawn_blocking called after ynz_rt_shutdown — task discarded");
        return;
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
    // CtxDropGuard: Send is impl'd below; the closure becomes Send too.
    let ctx_guard = CtxDropGuard { ptr: ctx_heap_ptr, len: ctx_heap_len };

    rt.spawn_blocking(move || {
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
#[no_mangle]
pub extern "C" fn ynz_rt_shutdown() {
    let Some(guard) = RUNTIME.get() else { return };
    let mut lock = match guard.lock() {
        Ok(l) => l,
        Err(e) => e.into_inner(), // still usable after poisoning
    };
    if let Some(rt) = lock.take() {
        // `shutdown_timeout` requires owned `Runtime`, which we now have.
        rt.shutdown_timeout(Duration::from_secs(5));
    }
    // `lock` drops here, releasing the mutex.
}

/// Sleep the current OS thread for `ms` milliseconds (blocking sleep, NOT `wait`).
///
/// Used by the Yinz intrinsic `sleepMs(int)`. This is a synchronous blocking sleep
/// on the calling thread; it does not use `wait` / async semantics (those ship in
/// v0.3-M2). The name `ynz_thread_sleep_ms` avoids any confusion with the `wait` keyword.
#[no_mangle]
pub extern "C" fn ynz_thread_sleep_ms(ms: i64) {
    if ms > 0 {
        std::thread::sleep(Duration::from_millis(ms as u64));
    }
}
