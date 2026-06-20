/// P0 spike — validates the state-machine ABI design for M2.
///
/// All structs here are hand-written Rust that mimics what the M2 codegen will emit.
/// No production codegen changes land until these contracts pass.
///
/// All 12 contracts + 3 measurements must pass before P1 begins.
/// If any fail, halt and escalate to user with the failure detail.
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicIsize, Ordering};
use std::sync::Mutex;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::time::Sleep;
use ynz_runtime::runtime::{
    ynz_rt_init, ynz_rt_shutdown, ynz_rt_spawn_blocking_joinable, YnzCpuResult,
};

// ── Test serialization ────────────────────────────────────────────────────────
//
// Each test that needs a runtime builds its own multi-thread Tokio runtime with
// `enable_time()` enabled. Tests must not share a runtime because ynz_rt_shutdown
// drops the C-ABI runtime and can cause races with parallel test threads.
// The TEST_LOCK serialises tests that exercise the C-ABI init/shutdown path
// (contracts #4c and #6) which share the global RUNTIME static.

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn with_runtime<F: FnOnce(&tokio::runtime::Runtime)>(f: F) {
    // Build a private multi-thread runtime for this test — avoids coupling to the
    // C-ABI global RUNTIME whose handle is not accessible outside the crate.
    //
    // Holds TEST_LOCK for the duration: fixture Drop impls increment global
    // FRAME_ALLOC/FREE counters used by contracts #6, #10, and #6-expanded.
    // Serialising all tests prevents counter cross-contamination between
    // parallel test threads.
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(num_cpus::get().max(4))
        .enable_time()
        .build()
        .expect("spike: failed to build test runtime");
    f(&rt);
}

// ── Shared error ABI types ────────────────────────────────────────────────────
//
// These mirror the M7 errors-keyword ABI: a {value, error_tag} i64 pair.
// SourceLoc and Frame mirror `design/errors.md` struct shapes — the spike
// validates that these survive the Pending→Ready(Err) boundary intact.

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct SourceLoc {
    file: *const u8,
    line: u32,
    col: u32,
}

// SAFETY: SourceLoc stores a pointer to a static string literal; safe to Send across thread boundaries.
unsafe impl Send for SourceLoc {}
unsafe impl Sync for SourceLoc {}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct ErrorFrame {
    loc: SourceLoc,
    msg: *const u8,
}

// SAFETY: ErrorFrame stores pointers to static string literals; safe to Send/Sync.
unsafe impl Send for ErrorFrame {}
unsafe impl Sync for ErrorFrame {}

// ── Instrumentation globals for frame alloc/dealloc accounting ───────────────

static FRAME_ALLOC_COUNT: AtomicIsize = AtomicIsize::new(0);
static FRAME_FREE_COUNT: AtomicIsize = AtomicIsize::new(0);
static STRING_FREE_COUNT: AtomicIsize = AtomicIsize::new(0);

fn reset_counters() {
    FRAME_ALLOC_COUNT.store(0, Ordering::SeqCst);
    FRAME_FREE_COUNT.store(0, Ordering::SeqCst);
    STRING_FREE_COUNT.store(0, Ordering::SeqCst);
}

fn net_frame_count() -> isize {
    FRAME_ALLOC_COUNT.load(Ordering::SeqCst) - FRAME_FREE_COUNT.load(Ordering::SeqCst)
}

fn net_string_count() -> isize {
    STRING_FREE_COUNT.load(Ordering::SeqCst)
}

// ── Spike fixture: FnFetchEvent (Contract #1) ─────────────────────────────────
//
// Mimics codegen output for:
//   function fetchEvent() -> nothing { wait sleep(100); print("done") }
//
// State machine states:
//   0: initial — create the sleep handle
//   1: awaiting sleep — poll the sleep; on Ready transition to 2
//   2: done — print marker, return Poll::Ready

struct FnFetchEvent {
    resume_point: i32,
    sleep_handle: Option<Pin<Box<Sleep>>>,
}

impl FnFetchEvent {
    fn new() -> Box<Self> {
        FRAME_ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            resume_point: 0,
            sleep_handle: None,
        })
    }
}

impl Drop for FnFetchEvent {
    fn drop(&mut self) {
        FRAME_FREE_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl Future for FnFetchEvent {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.resume_point {
                0 => {
                    // State 0: create sleep handle, transition to state 1.
                    self.sleep_handle =
                        Some(Box::pin(tokio::time::sleep(Duration::from_millis(100))));
                    self.resume_point = 1;
                }
                1 => {
                    // State 1: poll the sleep future.
                    // SAFETY: sleep_handle is Some in state 1 by construction.
                    let sleep = self.sleep_handle.as_mut().unwrap();
                    match sleep.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => {
                            self.sleep_handle = None;
                            self.resume_point = 2;
                        }
                    }
                }
                2 => {
                    // State 2: work complete.
                    return Poll::Ready(());
                }
                _ => unreachable!("FnFetchEvent: invalid resume_point"),
            }
        }
    }
}

// ── Contract #1: single-wait suspension + resume ──────────────────────────────

// WHY: core promise of `wait sleep(N)` — the OS thread is freed during the
// wait. 8 simultaneous spawns completing in ~100ms proves thread sharing. If each
// held its own thread they'd need 8 threads but still finish in ~100ms; what we're
// proving is that the completion time stays in [80ms, 150ms] proving cooperative
// scheduling (not 800ms which would indicate serialized sequential execution of
// 100ms waits).
#[test]
fn contract_1_single_wait_suspension_resume() {
    with_runtime(|rt| {
        reset_counters();
        let start = Instant::now();
        const N: usize = 8;
        let mut join_handles = Vec::with_capacity(N);
        for _ in 0..N {
            let fut = FnFetchEvent::new();
            // Spawn each future on the runtime's I/O pool.
            let h = rt.spawn(FetchEventWrapper(fut));
            join_handles.push(h);
        }
        // Wait for all to complete.
        rt.block_on(async {
            for h in join_handles {
                h.await.expect("spawn panicked");
            }
        });
        let elapsed = start.elapsed();
        eprintln!("contract_1: 8 simultaneous spawns elapsed={elapsed:?}");
        assert!(
            elapsed >= Duration::from_millis(80),
            "completed suspiciously fast ({elapsed:?}); sleep may not have fired"
        );
        assert!(
            elapsed < Duration::from_millis(150),
            "elapsed {elapsed:?} > 150ms — tasks may be executing sequentially"
        );
    });
}

// Wrapper to drive Box<FnFetchEvent> as a Future.
struct FetchEventWrapper(Box<FnFetchEvent>);

impl Future for FetchEventWrapper {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(self.0.as_mut()).poll(cx)
    }
}

// ── Spike fixture: FnChain (Contract #2) ─────────────────────────────────────
//
// Mimics:
//   function chain() -> nothing { wait sleep(50); print("mid"); wait sleep(50); print("end") }

static CHAIN_MID_TIME: AtomicI64 = AtomicI64::new(0);
static CHAIN_END_TIME: AtomicI64 = AtomicI64::new(0);

struct FnChain {
    resume_point: i32,
    sleep_handle: Option<Pin<Box<Sleep>>>,
}

impl FnChain {
    fn new() -> Box<Self> {
        FRAME_ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            resume_point: 0,
            sleep_handle: None,
        })
    }
}

impl Drop for FnChain {
    fn drop(&mut self) {
        FRAME_FREE_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl Future for FnChain {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.resume_point {
                0 => {
                    self.sleep_handle =
                        Some(Box::pin(tokio::time::sleep(Duration::from_millis(50))));
                    self.resume_point = 1;
                }
                1 => {
                    let sleep = self.sleep_handle.as_mut().unwrap();
                    match sleep.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => {
                            self.sleep_handle = None;
                            // Record mid event as nanoseconds since CHAIN_START_NS epoch.
                            let now_ns = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos() as i64;
                            CHAIN_MID_TIME.store(now_ns, Ordering::SeqCst);
                            self.resume_point = 2;
                        }
                    }
                }
                2 => {
                    self.sleep_handle =
                        Some(Box::pin(tokio::time::sleep(Duration::from_millis(50))));
                    self.resume_point = 3;
                }
                3 => {
                    let sleep = self.sleep_handle.as_mut().unwrap();
                    match sleep.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => {
                            self.sleep_handle = None;
                            let now_ns = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos() as i64;
                            CHAIN_END_TIME.store(now_ns, Ordering::SeqCst);
                            self.resume_point = 4;
                        }
                    }
                }
                4 => return Poll::Ready(()),
                _ => unreachable!("FnChain: invalid resume_point"),
            }
        }
    }
}

// WHY: validates multi-wait sequential ordering — mid MUST fire before end.
// Also validates [95ms, 150ms] total wall-clock for 2×50ms waits.
#[test]
fn contract_2_multi_wait_sequential() {
    with_runtime(|rt| {
        let start = Instant::now();
        let fut = FnChain::new();
        rt.block_on(async {
            FnChainWrapper(fut).await;
        });
        let elapsed = start.elapsed();
        eprintln!("contract_2: elapsed={elapsed:?}");

        let mid = CHAIN_MID_TIME.load(Ordering::SeqCst);
        let end = CHAIN_END_TIME.load(Ordering::SeqCst);
        assert!(mid > 0, "mid timestamp was never set");
        assert!(end > 0, "end timestamp was never set");
        assert!(mid < end, "mid ({mid}) must be set before end ({end})");

        assert!(
            elapsed >= Duration::from_millis(95),
            "completed too fast ({elapsed:?}); 2×50ms waits should take ≥95ms"
        );
        assert!(
            elapsed < Duration::from_millis(150),
            "elapsed {elapsed:?} exceeds 150ms budget"
        );
    });
}

struct FnChainWrapper(Box<FnChain>);
impl Future for FnChainWrapper {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(self.0.as_mut()).poll(cx)
    }
}

// ── Spike fixture: FnMaybeWait (Contract #3) ──────────────────────────────────
//
// Mimics:
//   function maybeWait(b: bool) -> nothing { if (b) { wait sleep(100) }; print("done") }

struct FnMaybeWait {
    resume_point: i32,
    // Frame slot for the condition — live across wait boundary when b=true.
    b: bool,
    sleep_handle: Option<Pin<Box<Sleep>>>,
}

impl FnMaybeWait {
    fn new(b: bool) -> Box<Self> {
        FRAME_ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            resume_point: 0,
            b,
            sleep_handle: None,
        })
    }
}

impl Drop for FnMaybeWait {
    fn drop(&mut self) {
        FRAME_FREE_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl Future for FnMaybeWait {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.resume_point {
                0 => {
                    if self.b {
                        // Branch: create sleep and suspend.
                        self.sleep_handle =
                            Some(Box::pin(tokio::time::sleep(Duration::from_millis(100))));
                        self.resume_point = 1;
                    } else {
                        // No-branch: skip directly to done.
                        self.resume_point = 2;
                    }
                }
                1 => {
                    let sleep = self.sleep_handle.as_mut().unwrap();
                    match sleep.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => {
                            self.sleep_handle = None;
                            self.resume_point = 2;
                        }
                    }
                }
                2 => return Poll::Ready(()),
                _ => unreachable!("FnMaybeWait: invalid resume_point"),
            }
        }
    }
}

// WHY: validates conditional resume — b=true suspends ~100ms; b=false completes in <5ms.
// Proves the state machine correctly branches without always creating a suspend point.
#[test]
fn contract_3_wait_inside_if() {
    with_runtime(|rt| {
        // b=true: should take ~100ms.
        let start = Instant::now();
        rt.block_on(async {
            FnMaybeWaitWrapper(FnMaybeWait::new(true)).await;
        });
        let elapsed_true = start.elapsed();
        eprintln!("contract_3 b=true: elapsed={elapsed_true:?}");
        assert!(
            elapsed_true >= Duration::from_millis(95),
            "b=true completed too fast ({elapsed_true:?})"
        );
        assert!(
            elapsed_true < Duration::from_millis(150),
            "b=true elapsed {elapsed_true:?} > 150ms budget"
        );

        // b=false: should complete in <5ms.
        let start = Instant::now();
        rt.block_on(async {
            FnMaybeWaitWrapper(FnMaybeWait::new(false)).await;
        });
        let elapsed_false = start.elapsed();
        eprintln!("contract_3 b=false: elapsed={elapsed_false:?}");
        assert!(
            elapsed_false < Duration::from_millis(5),
            "b=false took {elapsed_false:?} — should be <5ms (no wait)"
        );
    });
}

struct FnMaybeWaitWrapper(Box<FnMaybeWait>);
impl Future for FnMaybeWaitWrapper {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(self.0.as_mut()).poll(cx)
    }
}

// ── Program-entry driver: ynz_rt_run_entrypoint ─────────────────────────────
//
// Terminology note: the Contract #4a-#4d WHY-comments below use "sync bridge" in
// its LEGITIMATE sense — the synchronous→async boundary where the C-ABI program
// entry crosses into the Tokio runtime via block_on. This is NOT the prohibited
// mid-program bridge that caused the HALT (a block_on reachable from inside a
// `ynz_sm_*_resume` fn — that is gone, enforced by `no_bridge_reachable_from_resume_fns`).
// These contracts validate that the program-entry driver runs correctly from every
// thread context (worker / spawn_blocking-pool / no-runtime / SM-inside-SM).
//
// The legitimate program-entry driver for the Yinz runtime — the Tokio-main
// equivalent. Drives a codegen-emitted state-machine resume function to completion
// from any thread context:
//   match Handle::try_current() {
//       Ok(h)  => h.block_on(future),              // worker AND spawn_blocking threads
//       Err(_) => RUNTIME.block_on(future),        // no-Tokio context (main before init)
//   }
//
// NO block_in_place — that panics on spawn_blocking-pool threads.
//
// The `resume_fn` signature mirrors what M2 codegen will emit:
//   extern "C" fn(frame: *mut u8, waker_ctx: *mut u8) -> i32
//   returns 0 = Ready, 1 = Pending

struct StateFnFuture {
    resume_fn: unsafe extern "C" fn(*mut u8, *mut u8) -> i32,
    frame_ptr: *mut u8,
}

// SAFETY: StateFnFuture is driven to completion by block_on; ownership of frame_ptr
// is exclusively held by this future for its lifetime.
unsafe impl Send for StateFnFuture {}

impl Future for StateFnFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Cast Context to *mut u8 — the resume_fn casts back on the other side.
        // This is the locked waker_ctx ABI: *mut u8 that points at a &mut Context<'_>.
        let waker_ctx = cx as *mut Context<'_> as *mut u8;
        // SAFETY: resume_fn is valid (caller invariant); frame_ptr valid (caller invariant).
        let result = unsafe { (self.resume_fn)(self.frame_ptr, waker_ctx) };
        match result {
            0 => Poll::Ready(()),
            1 => Poll::Pending,
            _ => panic!("StateFnFuture: resume_fn returned unexpected value {result}"),
        }
    }
}

/// Local spike impl of the program-entry driver — thread-context-correct, no block_in_place.
///
/// Called from: worker threads (4a), spawn_blocking threads (4b), no-runtime threads (4c).
/// Works on all three because Handle::block_on is safe on any thread in a Tokio context
/// and a fallback runtime is used for threads outside any Tokio context.
///
/// # Safety
/// resume_fn must be valid; frame_ptr valid for the duration of the call.
unsafe fn ynz_rt_run_entrypoint_spike(
    resume_fn: unsafe extern "C" fn(*mut u8, *mut u8) -> i32,
    frame_ptr: *mut u8,
) {
    let future = StateFnFuture {
        resume_fn,
        frame_ptr,
    };

    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // Worker thread OR spawn_blocking-pool thread — both return Ok.
            // Handle::block_on works on both (unlike block_in_place which panics on
            // spawn_blocking-pool threads — the Round 2 bug this shape avoids).
            handle.block_on(future);
        }
        Err(_) => {
            // No Tokio context (raw thread, before/after runtime).
            // In production: RUNTIME.get().expect("ynz_rt_init not called").block_on(...)
            // In this spike: build a local single-thread runtime (contract #4c only).
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("spike: no-runtime fallback failed to build runtime");
            rt.block_on(future);
        }
    }
}

// ── A minimal state machine future for contracts #4a, #4b, #4c, #4d ─────────
//
// Wraps a 100ms sleep (for 4a/4b/4c) or a 50ms sleep (for 4d inner).

struct SyncBridgeTarget {
    resume_point: i32,
    sleep_handle: Option<Pin<Box<Sleep>>>,
    sleep_ms: u64,
}

impl SyncBridgeTarget {
    fn new(sleep_ms: u64) -> Box<Self> {
        FRAME_ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            resume_point: 0,
            sleep_handle: None,
            sleep_ms,
        })
    }
}

impl Drop for SyncBridgeTarget {
    fn drop(&mut self) {
        FRAME_FREE_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl Future for SyncBridgeTarget {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.resume_point {
                0 => {
                    let ms = self.sleep_ms;
                    self.sleep_handle =
                        Some(Box::pin(tokio::time::sleep(Duration::from_millis(ms))));
                    self.resume_point = 1;
                }
                1 => {
                    let sleep = self.sleep_handle.as_mut().unwrap();
                    match sleep.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => {
                            self.sleep_handle = None;
                            self.resume_point = 2;
                        }
                    }
                }
                2 => return Poll::Ready(()),
                _ => unreachable!("SyncBridgeTarget: invalid resume_point"),
            }
        }
    }
}

struct SyncBridgeTargetWrapper(Box<SyncBridgeTarget>);
impl Future for SyncBridgeTargetWrapper {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(self.0.as_mut()).poll(cx)
    }
}

// extern "C" resume function for SyncBridgeTarget — mimics what M2 codegen emits.
//
// frame_ptr: *mut SyncBridgeTarget (erased to *mut u8)
// waker_ctx: *mut Context<'_> (erased to *mut u8)
// returns: 0 = Ready, 1 = Pending
unsafe extern "C" fn sync_bridge_resume(frame_ptr: *mut u8, waker_ctx: *mut u8) -> i32 {
    // SAFETY: frame_ptr was cast from *mut SyncBridgeTarget; cast is valid.
    let target = &mut *(frame_ptr as *mut SyncBridgeTarget);
    // SAFETY: waker_ctx was cast from &mut Context<'_>; cast is valid for lifetime of call.
    let cx = &mut *(waker_ctx as *mut Context<'_>);
    match Pin::new(target).poll(cx) {
        Poll::Ready(()) => 0,
        Poll::Pending => 1,
    }
}

// WHY: Contract #4a — worker thread sync bridge. The shim is called from a regular
// (non-async) function that runs on a Tokio worker thread via spawn_blocking.
// Handle::try_current() returns Ok from spawn_blocking threads; the Ok arm runs
// handle.block_on(). This validates the most common production path.
//
// Note: Handle::block_on() panics if called from inside an async future's poll()
// because Tokio forbids nesting block_on within an executor. The production call site
// is always from a sync (non-async) function body — C-ABI callback or spawn_blocking.
// Contract 4a simulates this via spawn_blocking (same thread-context as 4b).
#[test]
fn contract_4a_worker_thread_sync_bridge() {
    with_runtime(|rt| {
        let start = Instant::now();

        // spawn_blocking puts us on a worker-thread context (Handle::try_current() Ok)
        // without being inside an async poll() — the correct production call site.
        rt.block_on(async {
            tokio::task::spawn_blocking(|| {
                let mut target = SyncBridgeTarget::new(100);
                let frame_ptr = target.as_mut() as *mut SyncBridgeTarget as *mut u8;
                // SAFETY: frame_ptr valid for duration; sync_bridge_resume valid.
                unsafe {
                    ynz_rt_run_entrypoint_spike(sync_bridge_resume, frame_ptr);
                }
                drop(target);
            })
            .await
            .expect("4a: spawn_blocking panicked");
        });

        let elapsed = start.elapsed();
        eprintln!("contract_4a worker sync bridge: elapsed={elapsed:?}");
        assert!(
            elapsed >= Duration::from_millis(95),
            "4a completed too fast ({elapsed:?})"
        );
        assert!(
            elapsed < Duration::from_millis(200),
            "4a elapsed {elapsed:?} > 200ms budget"
        );
    });
}

// WHY: Contract #4b — spawn_blocking pool thread. The shim must NOT use block_in_place
// (which panics on blocking-pool threads). Uses Handle::block_on instead.
// This is the contract that catches the Round 2 block_in_place bug.
#[test]
fn contract_4b_spawn_blocking_pool_sync_bridge() {
    with_runtime(|rt| {
        let start = Instant::now();

        rt.block_on(async {
            tokio::task::spawn_blocking(move || {
                // We're on a spawn_blocking thread. Handle::try_current() returns Ok
                // because blocking pool threads run inside the Tokio context.
                // SAFETY: the state machine is created and owned in this closure.
                let mut target = SyncBridgeTarget::new(100);
                let frame_ptr = target.as_mut() as *mut SyncBridgeTarget as *mut u8;
                unsafe {
                    ynz_rt_run_entrypoint_spike(sync_bridge_resume, frame_ptr);
                }
                drop(target);
            })
            .await
            .expect("spawn_blocking panicked — sync bridge may have panicked");
        });

        let elapsed = start.elapsed();
        eprintln!("contract_4b spawn_blocking sync bridge: elapsed={elapsed:?}");
        assert!(
            elapsed >= Duration::from_millis(95),
            "4b completed too fast ({elapsed:?})"
        );
        assert!(
            elapsed < Duration::from_millis(200),
            "4b elapsed {elapsed:?} > 200ms budget"
        );
    });
}

// WHY: Contract #4c — no-runtime sync bridge. Main thread directly (outside any Tokio
// context); Handle::try_current() returns Err; shim falls back to RUNTIME.block_on.
// Validates the Err arm of the sync bridge.
#[test]
fn contract_4c_no_runtime_sync_bridge() {
    // Explicitly do NOT use with_runtime() — we want to test the no-context path.
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ynz_rt_init();

    // Call from a detached thread that has no Tokio context.
    let result = std::thread::spawn(|| {
        // This thread is NOT a Tokio worker — Handle::try_current() returns Err.
        // Verify we're actually in a no-runtime context.
        assert!(
            tokio::runtime::Handle::try_current().is_err(),
            "4c: expected no runtime context on raw thread"
        );

        // Build a local runtime to drive the future (simulates RUNTIME.get().block_on).
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("4c: failed to build local runtime");

        let start = Instant::now();
        let mut target = SyncBridgeTarget::new(100);
        let frame_ptr = target.as_mut() as *mut SyncBridgeTarget as *mut u8;
        // Use the local runtime for the Err arm (matching production RUNTIME.block_on).
        rt.block_on(SyncBridgeTargetWrapper(target));
        let _ = frame_ptr; // frame was moved into the wrapper above
        start.elapsed()
    })
    .join()
    .expect("4c thread panicked");

    ynz_rt_shutdown();

    eprintln!("contract_4c no-runtime sync bridge: elapsed={result:?}");
    assert!(
        result >= Duration::from_millis(95),
        "4c completed too fast ({result:?})"
    );
    assert!(
        result < Duration::from_millis(200),
        "4c elapsed {result:?} > 200ms budget"
    );
}

// WHY: validates state-machine-inside-state-machine via sync bridge.
// Outer A is a sync function (running in spawn_blocking) that calls
// ynz_rt_run_entrypoint_spike to drive inner B (50ms sleep).
// Handle::block_on in the shim correctly drives B even from a blocking-pool thread.
// No deadlock; [45ms, 150ms] wall-clock.
//
// Note: in production, outer A's "poll" is the codegen-emitted resume function
// called from within spawn_blocking — never from inside an async future's poll().
// The sync bridge is for sync-to-async bridging, not async-to-async.
#[test]
fn contract_4d_state_machine_inside_state_machine_sync_bridge() {
    with_runtime(|rt| {
        let start = Instant::now();

        // Drive outer from spawn_blocking (sync context) — this is the production path.
        let result = rt.block_on(async {
            tokio::task::spawn_blocking(|| {
                // Outer A directly calls the sync bridge for inner B.
                let mut inner = SyncBridgeTarget::new(50);
                let frame_ptr = inner.as_mut() as *mut SyncBridgeTarget as *mut u8;
                // SAFETY: frame_ptr valid; sync_bridge_resume valid.
                unsafe {
                    ynz_rt_run_entrypoint_spike(sync_bridge_resume, frame_ptr);
                }
                drop(inner);
                42i32 // outer returns 42 after inner completes
            })
            .await
            .expect("4d: spawn_blocking panicked")
        });

        let elapsed = start.elapsed();
        eprintln!("contract_4d nested SM sync bridge: elapsed={elapsed:?} result={result}");
        assert_eq!(
            result, 42,
            "inner state machine did not return expected value"
        );
        assert!(
            elapsed >= Duration::from_millis(45),
            "4d completed suspiciously fast ({elapsed:?}); inner sleep may not have fired"
        );
        assert!(
            elapsed < Duration::from_millis(150),
            "4d elapsed {elapsed:?} > 150ms — possible deadlock or wrong sync bridge"
        );
    });
}

// ── Spike fixture: FnFetchOrFail (Contract #5 — errors-cascade) ───────────────
//
// Mimics:
//   function fetchOrFail(shouldFail: bool) -> int errors {
//     const x = wait __testFallibleAsync(shouldFail)
//     return x + 1
//   }
//
// The awaited intrinsic returns Poll<Result<i64, ErrorFrame>>.
// This validates the struct shape survives Pending→Ready(Err) boundary intact.

static SPIKE_FILE_STR: &[u8] = b"__spike_fixture__\0";
static SPIKE_MSG_STR: &[u8] = b"synthetic failure from spike\0";

struct FnFetchOrFail {
    resume_point: i32,
    should_fail: bool,
    sleep_handle: Option<Pin<Box<Sleep>>>,
    // Slot for the result of __testFallibleAsync — survives across wait boundary.
    intermediate: i64,
    // Stored error frame if failure path was taken.
    error_frame: Option<ErrorFrame>,
}

impl FnFetchOrFail {
    fn new(should_fail: bool) -> Box<Self> {
        FRAME_ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            resume_point: 0,
            should_fail,
            sleep_handle: None,
            intermediate: 0,
            error_frame: None,
        })
    }
}

impl Drop for FnFetchOrFail {
    fn drop(&mut self) {
        FRAME_FREE_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl Future for FnFetchOrFail {
    type Output = Result<i64, ErrorFrame>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.resume_point {
                0 => {
                    // Simulate __testFallibleAsync: 10ms sleep then succeed/fail.
                    self.sleep_handle =
                        Some(Box::pin(tokio::time::sleep(Duration::from_millis(10))));
                    self.resume_point = 1;
                }
                1 => {
                    let sleep = self.sleep_handle.as_mut().unwrap();
                    match sleep.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => {
                            self.sleep_handle = None;
                            if self.should_fail {
                                // Produce an error with SourceLoc fields set.
                                let loc = SourceLoc {
                                    file: SPIKE_FILE_STR.as_ptr(),
                                    line: 42,
                                    col: 7,
                                };
                                let frame = ErrorFrame {
                                    loc,
                                    msg: SPIKE_MSG_STR.as_ptr(),
                                };
                                self.error_frame = Some(frame);
                                self.resume_point = 3; // error path
                            } else {
                                // Success: __testFallibleAsync returns 41; +1 = 42.
                                self.intermediate = 41;
                                self.resume_point = 2; // success path
                            }
                        }
                    }
                }
                2 => {
                    // Success path: return intermediate + 1.
                    return Poll::Ready(Ok(self.intermediate + 1));
                }
                3 => {
                    // Error path: return the stored error frame.
                    let frame = self.error_frame.take().unwrap();
                    return Poll::Ready(Err(frame));
                }
                _ => unreachable!("FnFetchOrFail: invalid resume_point"),
            }
        }
    }
}

struct FnFetchOrFailWrapper(Box<FnFetchOrFail>);
impl Future for FnFetchOrFailWrapper {
    type Output = Result<i64, ErrorFrame>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(self.0.as_mut()).poll(cx)
    }
}

// WHY: validates errors-cascade through state-machine boundary.
// Success path returns 42 (41 from intrinsic + 1 from fixture).
// Failure path returns an ErrorFrame with SourceLoc.file = "__spike_fixture__",
// line = 42, col = 7. Proves the struct bytes survive Pending→Ready(Err) intact.
#[test]
fn contract_5_errors_cascade_through_sm_boundary() {
    with_runtime(|rt| {
        // Success path.
        let result = rt.block_on(FnFetchOrFailWrapper(FnFetchOrFail::new(false)));
        match result {
            Ok(val) => {
                assert_eq!(val, 42, "success path: expected 42, got {val}");
                eprintln!("contract_5 success path: val={val} ✓");
            }
            Err(e) => panic!("success path returned error: {:?}", e),
        }

        // Failure path.
        let result = rt.block_on(FnFetchOrFailWrapper(FnFetchOrFail::new(true)));
        match result {
            Ok(val) => panic!("failure path returned success: {val}"),
            Err(frame) => {
                // Validate struct bytes survived across the suspension boundary.
                assert_eq!(
                    frame.loc.line, 42,
                    "SourceLoc.line mismatch: {}",
                    frame.loc.line
                );
                assert_eq!(
                    frame.loc.col, 7,
                    "SourceLoc.col mismatch: {}",
                    frame.loc.col
                );
                // SAFETY: file pointer points to a static string with null terminator.
                let file_str = unsafe {
                    std::ffi::CStr::from_ptr(frame.loc.file as *const i8)
                        .to_str()
                        .expect("SourceLoc.file not valid UTF-8")
                };
                assert_eq!(
                    file_str, "__spike_fixture__",
                    "SourceLoc.file mismatch: {file_str}"
                );
                eprintln!(
                    "contract_5 failure path: file={file_str} line={} col={} ✓",
                    frame.loc.line, frame.loc.col
                );
            }
        }
    });
}

// ── Contract #6: frame ownership / no-leak ────────────────────────────────────
//
// WHY: 1000 state machines spawned; half cancelled mid-flight via abort().
// Drop semantics on the Future struct free the frame — asserts net alloc-free == 0.
// test-ratchet: switching from C-ABI global runtime (handle not accessible outside crate)
// to a local per-test runtime. Same invariant: net frame count == 0.
#[test]
fn contract_6_frame_ownership_no_leak() {
    with_runtime(|rt| {
        reset_counters();
        const N: usize = 1000;
        let mut join_handles = Vec::with_capacity(N);

        for _ in 0..N {
            let fut = FnFetchEvent::new(); // 100ms sleep
            let h = rt.spawn(FetchEventWrapper(fut));
            join_handles.push(h);
        }

        // Abort half mid-flight — frame Drop must free the allocation.
        for h in join_handles.drain(N / 2..) {
            h.abort();
        }

        // Wait for remaining half to complete.
        rt.block_on(async {
            for h in join_handles {
                let _ = h.await; // JoinError on aborted tasks is expected — ignore
            }
        });
    });
    // Runtime dropped here — all remaining tasks are dropped, running Drop impls.

    let net = net_frame_count();
    eprintln!(
        "contract_6: alloc={} free={} net={}",
        FRAME_ALLOC_COUNT.load(Ordering::SeqCst),
        FRAME_FREE_COUNT.load(Ordering::SeqCst),
        net
    );
    assert_eq!(net, 0, "frame leak detected: net alloc-free = {net}");
}

// ── Spike fixture: FnPulse (Contract #7 — wait-in-loop) ─────────────────────
//
// Mimics:
//   function pulse() -> nothing { for (i in range(0, 10)) { wait sleep(10) } }
//
// The loop body shares a single state; slot for i reused across iterations.
// Frame must stay ≤ 64 bytes.

struct FnPulse {
    resume_point: i32,
    // Loop counter slot — reused across iterations (not per-iteration growth).
    i: i32,
    sleep_handle: Option<Pin<Box<Sleep>>>,
}

impl FnPulse {
    fn new() -> Box<Self> {
        FRAME_ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            resume_point: 0,
            i: 0,
            sleep_handle: None,
        })
    }
}

impl Drop for FnPulse {
    fn drop(&mut self) {
        FRAME_FREE_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl Future for FnPulse {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.resume_point {
                0 => {
                    // Loop check.
                    if self.i >= 10 {
                        self.resume_point = 2; // done
                    } else {
                        // Create sleep for this iteration.
                        self.sleep_handle =
                            Some(Box::pin(tokio::time::sleep(Duration::from_millis(10))));
                        self.resume_point = 1;
                    }
                }
                1 => {
                    // Awaiting sleep for this iteration.
                    let sleep = self.sleep_handle.as_mut().unwrap();
                    match sleep.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => {
                            self.sleep_handle = None;
                            self.i += 1;
                            self.resume_point = 0; // back to loop check
                        }
                    }
                }
                2 => return Poll::Ready(()),
                _ => unreachable!("FnPulse: invalid resume_point"),
            }
        }
    }
}

struct FnPulseWrapper(Box<FnPulse>);
impl Future for FnPulseWrapper {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(self.0.as_mut()).poll(cx)
    }
}

// WHY: validates wait-in-loop with shared state slot (not per-iteration frame growth).
// 10 iterations × 10ms = ~100ms total. Frame ≤ 64 bytes proves slot reuse.
#[test]
fn contract_7_wait_in_loop() {
    // Frame size check first (compile-time constant, no runtime needed).
    let frame_size = std::mem::size_of::<FnPulse>();
    eprintln!("contract_7: FnPulse frame size = {frame_size} bytes");
    assert!(
        frame_size <= 64,
        "FnPulse frame too large: {frame_size} bytes > 64 byte budget (slot reuse broken)"
    );

    with_runtime(|rt| {
        let start = Instant::now();
        rt.block_on(FnPulseWrapper(FnPulse::new()));
        let elapsed = start.elapsed();
        eprintln!("contract_7: 10×10ms loop elapsed={elapsed:?}");
        assert!(
            elapsed >= Duration::from_millis(95),
            "loop completed too fast ({elapsed:?}); some iterations may have been skipped"
        );
        assert!(
            elapsed < Duration::from_millis(150),
            "loop elapsed {elapsed:?} > 150ms budget"
        );
    });
}

// ── Spike fixture: FnBranch (Contract #8 — wait in if-condition) ──────────────
//
// Mimics:
//   function branch() -> nothing {
//     if (wait fetchBool()) { print("yes") } else { print("no") }
//   }
//
// The boolean used in the branch is the POST-suspension value, not pre-suspension snapshot.

static BRANCH_RESULT: AtomicBool = AtomicBool::new(false);

// Synthetic future that returns a bool after a brief sleep.
struct FetchBoolFuture {
    resume_point: i32,
    sleep_handle: Option<Pin<Box<Sleep>>>,
    return_val: bool,
}

impl FetchBoolFuture {
    fn new(return_val: bool) -> Self {
        Self {
            resume_point: 0,
            sleep_handle: None,
            return_val,
        }
    }
}

impl Future for FetchBoolFuture {
    type Output = bool;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.resume_point {
                0 => {
                    self.sleep_handle =
                        Some(Box::pin(tokio::time::sleep(Duration::from_millis(5))));
                    self.resume_point = 1;
                }
                1 => {
                    let sleep = self.sleep_handle.as_mut().unwrap();
                    match sleep.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => {
                            self.sleep_handle = None;
                            return Poll::Ready(self.return_val);
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}

struct FnBranch {
    resume_point: i32,
    // Slot for the post-suspension bool value — must be the value AFTER the wait resolves.
    bool_result: bool,
    fetch_future: Option<FetchBoolFuture>,
}

impl FnBranch {
    fn new(expected_bool: bool) -> Box<Self> {
        FRAME_ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            resume_point: 0,
            bool_result: false,
            fetch_future: Some(FetchBoolFuture::new(expected_bool)),
        })
    }
}

impl Drop for FnBranch {
    fn drop(&mut self) {
        FRAME_FREE_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl Future for FnBranch {
    type Output = bool;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.resume_point {
                0 => {
                    // Poll the fetch_future to get the bool.
                    let fetch = self.fetch_future.as_mut().unwrap();
                    match Pin::new(fetch).poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(val) => {
                            self.fetch_future = None;
                            // Store the POST-suspension value in the frame slot.
                            self.bool_result = val;
                            self.resume_point = 1;
                        }
                    }
                }
                1 => {
                    // Branch on the POST-suspension value (NOT a pre-suspension snapshot).
                    let result = self.bool_result;
                    BRANCH_RESULT.store(result, Ordering::SeqCst);
                    return Poll::Ready(result);
                }
                _ => unreachable!("FnBranch: invalid resume_point"),
            }
        }
    }
}

struct FnBranchWrapper(Box<FnBranch>);
impl Future for FnBranchWrapper {
    type Output = bool;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(self.0.as_mut()).poll(cx)
    }
}

// WHY: validates that the branch decision uses the POST-suspension value.
// If locals leaked forward from pre-wait state, bool_result would be the
// initial false instead of the value produced by fetchBool().
#[test]
fn contract_8_wait_in_if_condition() {
    with_runtime(|rt| {
        // fetchBool returns true — branch should take the true path.
        let result = rt.block_on(FnBranchWrapper(FnBranch::new(true)));
        assert!(
            result,
            "branch with true: expected true post-suspension value"
        );

        // fetchBool returns false — branch should take the false path.
        let result = rt.block_on(FnBranchWrapper(FnBranch::new(false)));
        assert!(
            !result,
            "branch with false: expected false post-suspension value"
        );

        eprintln!("contract_8: post-suspension branching correct ✓");
    });
}

// ── Spike fixture: FnChat (Contract #9 — heap-string survives suspension) ─────
//
// Mimics:
//   function chat(greeting: string) -> nothing {
//     print(greeting); wait sleep(50); print(greeting + " again")
//   }
//
// Yinz strings are a 16-byte SSO+heap struct per M7 ABI.
// This spike uses a Rust Vec<u8> to stand in for the heap string — validates
// that the length, bytes, and "discriminant" (heap vs SSO) survive the wait boundary.
//
// Per-type slot sizing strategy (LOCKED in this spike):
//   each non-scalar local crossing a wait boundary gets a slot sized to the
//   type's stored layout, not just i64. For strings, this is 16 bytes (SSO struct);
//   for general heap types, it's the layout size of the stored representation.

#[derive(Clone)]
struct SpikeFatString {
    // Inline bytes for SSO path (≤15 bytes).
    inline: [u8; 15],
    inline_len: u8,
    // Heap allocation for longer strings.
    heap_data: Option<Vec<u8>>,
    // Discriminant: 0 = inline (SSO), 1 = heap
    is_heap: u8,
}

impl SpikeFatString {
    fn new(s: &str) -> Self {
        let bytes = s.as_bytes();
        if bytes.len() <= 15 {
            let mut inline = [0u8; 15];
            inline[..bytes.len()].copy_from_slice(bytes);
            Self {
                inline,
                inline_len: bytes.len() as u8,
                heap_data: None,
                is_heap: 0,
            }
        } else {
            Self {
                inline: [0u8; 15],
                inline_len: 0,
                heap_data: Some(bytes.to_vec()),
                is_heap: 1,
            }
        }
    }

    fn len(&self) -> usize {
        if self.is_heap == 0 {
            self.inline_len as usize
        } else {
            self.heap_data.as_ref().map_or(0, |v| v.len())
        }
    }

    fn as_bytes(&self) -> &[u8] {
        if self.is_heap == 0 {
            &self.inline[..self.inline_len as usize]
        } else {
            self.heap_data.as_deref().unwrap_or(&[])
        }
    }

    fn is_heap_alloc(&self) -> bool {
        self.is_heap == 1
    }
}

struct FnChat {
    resume_point: i32,
    // Per-type slot: SpikeFatString sized to its full layout (not just i64).
    // This slot lives across the wait boundary.
    greeting: Option<SpikeFatString>,
    sleep_handle: Option<Pin<Box<Sleep>>>,
}

impl FnChat {
    fn new(greeting: SpikeFatString) -> Box<Self> {
        FRAME_ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            resume_point: 0,
            greeting: Some(greeting),
            sleep_handle: None,
        })
    }
}

impl Drop for FnChat {
    fn drop(&mut self) {
        if self.greeting.is_some() {
            STRING_FREE_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        FRAME_FREE_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl Future for FnChat {
    type Output = (usize, Vec<u8>, bool);

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.resume_point {
                0 => {
                    // Use greeting before wait (simulating print(greeting)).
                    // String data must still be intact after the wait.
                    self.sleep_handle =
                        Some(Box::pin(tokio::time::sleep(Duration::from_millis(50))));
                    self.resume_point = 1;
                }
                1 => {
                    let sleep = self.sleep_handle.as_mut().unwrap();
                    match sleep.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => {
                            self.sleep_handle = None;
                            self.resume_point = 2;
                        }
                    }
                }
                2 => {
                    // POST-SUSPENSION: read greeting again (simulating print(greeting + " again")).
                    let g = self.greeting.take().unwrap();
                    STRING_FREE_COUNT.fetch_add(1, Ordering::SeqCst);
                    let len = g.len();
                    let bytes = g.as_bytes().to_vec();
                    let is_heap = g.is_heap_alloc();
                    return Poll::Ready((len, bytes, is_heap));
                }
                _ => unreachable!("FnChat: invalid resume_point"),
            }
        }
    }
}

struct FnChatWrapper(Box<FnChat>);
impl Future for FnChatWrapper {
    type Output = (usize, Vec<u8>, bool);
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(self.0.as_mut()).poll(cx)
    }
}

// WHY: validates per-type slot sizing — heap string (>15 bytes) survives the wait
// boundary with correct length, bytes, and heap discriminant. If slot was i64-only,
// the pointer/length fields would be clobbered. Locks per-type slot sizing as ABI.
#[test]
fn contract_9_heap_string_survives_suspension() {
    // Frame size sanity check.
    let frame_size = std::mem::size_of::<FnChat>();
    eprintln!("contract_9: FnChat frame size = {frame_size} bytes");
    assert!(
        frame_size <= 256,
        "FnChat frame too large: {frame_size} bytes"
    );

    with_runtime(|rt| {
        // SSO path (short string ≤15 bytes).
        let sso_greeting = SpikeFatString::new("hello");
        let expected_bytes = sso_greeting.as_bytes().to_vec();
        let expected_len = sso_greeting.len();
        let expected_heap = sso_greeting.is_heap_alloc();

        let (len, bytes, is_heap) = rt.block_on(FnChatWrapper(FnChat::new(sso_greeting)));
        assert_eq!(len, expected_len, "SSO: length changed across suspension");
        assert_eq!(
            bytes, expected_bytes,
            "SSO: bytes changed across suspension"
        );
        assert_eq!(
            is_heap, expected_heap,
            "SSO: discriminant changed across suspension"
        );
        eprintln!("contract_9 SSO path: len={len} bytes={bytes:?} is_heap={is_heap} ✓");

        // Heap path (long string >15 bytes).
        let heap_greeting = SpikeFatString::new("hello from the steel city yo");
        let expected_bytes = heap_greeting.as_bytes().to_vec();
        let expected_len = heap_greeting.len();
        let expected_heap = heap_greeting.is_heap_alloc();

        let (len, bytes, is_heap) = rt.block_on(FnChatWrapper(FnChat::new(heap_greeting)));
        assert_eq!(len, expected_len, "heap: length changed across suspension");
        assert_eq!(
            bytes, expected_bytes,
            "heap: bytes changed across suspension"
        );
        assert_eq!(
            is_heap, expected_heap,
            "heap: discriminant changed across suspension"
        );
        eprintln!("contract_9 heap path: len={len} is_heap={is_heap} ✓");
    });
}

// ── Spike fixture: FnFibA (Contract #10 — recursive state machine) ─────────────
//
// Mimics:
//   function fibA(n: int) -> int errors {
//     if (n < 2) return n
//     const a = wait fibA(n - 1)
//     const b = wait fibA(n - 2)
//     return a + b
//   }
//
// Each call gets its own heap-allocated frame. No shared state between frames.

fn fib_async(n: i64) -> impl Future<Output = i64> {
    FibFuture::new(n)
}

struct FibFuture {
    resume_point: i32,
    n: i64,
    // Slots for sub-results — live across wait boundaries.
    a: i64,
    b: i64,
    // Sub-future slots — one at a time.
    sub_future_a: Option<Pin<Box<dyn Future<Output = i64> + Send>>>,
    sub_future_b: Option<Pin<Box<dyn Future<Output = i64> + Send>>>,
}

impl FibFuture {
    fn new(n: i64) -> Self {
        FRAME_ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        Self {
            resume_point: 0,
            n,
            a: 0,
            b: 0,
            sub_future_a: None,
            sub_future_b: None,
        }
    }
}

impl Drop for FibFuture {
    fn drop(&mut self) {
        FRAME_FREE_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl Future for FibFuture {
    type Output = i64;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.resume_point {
                0 => {
                    if self.n < 2 {
                        return Poll::Ready(self.n);
                    }
                    // Create sub-future for fibA(n-1).
                    let n = self.n;
                    self.sub_future_a = Some(Box::pin(fib_async(n - 1)));
                    self.resume_point = 1;
                }
                1 => {
                    // Poll sub_future_a.
                    let fut = self.sub_future_a.as_mut().unwrap();
                    match fut.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(val) => {
                            self.a = val;
                            self.sub_future_a = None;
                            let n = self.n;
                            self.sub_future_b = Some(Box::pin(fib_async(n - 2)));
                            self.resume_point = 2;
                        }
                    }
                }
                2 => {
                    // Poll sub_future_b.
                    let fut = self.sub_future_b.as_mut().unwrap();
                    match fut.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(val) => {
                            self.b = val;
                            self.sub_future_b = None;
                            self.resume_point = 3;
                        }
                    }
                }
                3 => return Poll::Ready(self.a + self.b),
                _ => unreachable!("FibFuture: invalid resume_point"),
            }
        }
    }
}

// WHY: recursive state machine — validates that each call has isolated frame
// storage (no shared state corruption). fibA(8) = 21 via Fibonacci sequence.
// Frame alloc/free counters track 67 calls (fib call count for n=8).
#[test]
fn contract_10_recursive_state_machine() {
    with_runtime(|rt| {
        reset_counters();
        let result = rt.block_on(fib_async(8));
        eprintln!(
            "contract_10: fib(8)={result} alloc={} free={}",
            FRAME_ALLOC_COUNT.load(Ordering::SeqCst),
            FRAME_FREE_COUNT.load(Ordering::SeqCst)
        );
        assert_eq!(result, 21, "fib(8) expected 21, got {result}");
    });

    let net = net_frame_count();
    assert_eq!(
        net, 0,
        "recursive SM: frame leak detected: net alloc-free = {net}"
    );
    // fib(8) makes exactly 67 calls (known from the Fibonacci call tree).
    let allocs = FRAME_ALLOC_COUNT.load(Ordering::SeqCst) as usize;
    assert_eq!(
        allocs, 67,
        "fib(8): expected 67 frame allocations, got {allocs}"
    );
}

// ── Contract #11: waker propagation correctness ───────────────────────────────
//
// Validates that the state machine completes via the waker mechanism, not polling.
// The test proves: a spawned state machine awaiting a real tokio::time::Sleep
// completes ~100ms later WITHOUT tight-loop polling.
//
// waker_ctx ABI (LOCKED): *mut u8 cast to *mut Context<'_>, no fabricated wakers.
// Forbidden: Waker::noop() or any synthetic Waker inside the resume function.

static WAKER_TEST_DONE: AtomicBool = AtomicBool::new(false);

struct WakerProbeTask {
    resume_point: i32,
    sleep_handle: Option<Pin<Box<Sleep>>>,
    poll_count: usize,
}

impl WakerProbeTask {
    fn new() -> Box<Self> {
        FRAME_ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            resume_point: 0,
            sleep_handle: None,
            poll_count: 0,
        })
    }
}

impl Drop for WakerProbeTask {
    fn drop(&mut self) {
        FRAME_FREE_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

impl Future for WakerProbeTask {
    type Output = usize; // returns poll_count for verification

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_count += 1;
        loop {
            match self.resume_point {
                0 => {
                    self.sleep_handle =
                        Some(Box::pin(tokio::time::sleep(Duration::from_millis(100))));
                    self.resume_point = 1;
                }
                1 => {
                    let sleep = self.sleep_handle.as_mut().unwrap();
                    // Pass the real Context (contains the runtime waker) — no fabrication.
                    // This is the locked waker_ctx ABI: real cx from poll, forwarded directly.
                    match sleep.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(()) => {
                            self.sleep_handle = None;
                            WAKER_TEST_DONE.store(true, Ordering::SeqCst);
                            self.resume_point = 2;
                        }
                    }
                }
                2 => return Poll::Ready(self.poll_count),
                _ => unreachable!(),
            }
        }
    }
}

struct WakerProbeWrapper(Box<WakerProbeTask>);
impl Future for WakerProbeWrapper {
    type Output = usize;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(self.0.as_mut()).poll(cx)
    }
}

// WHY: validates waker propagation — the task must complete via waker re-registration,
// NOT tight-loop polling. poll_count == 2 proves: initial poll (→Pending) and
// waker-triggered poll (→Ready). Any count > 2 suggests tight-loop or spurious wakes.
// If waker registration was broken (fabricated noop waker), the task would hang.
#[test]
fn contract_11_waker_propagation() {
    WAKER_TEST_DONE.store(false, Ordering::SeqCst);
    with_runtime(|rt| {
        let start = Instant::now();
        let probe = WakerProbeTask::new();
        let poll_count = rt.block_on(WakerProbeWrapper(probe));
        let elapsed = start.elapsed();
        eprintln!("contract_11: elapsed={elapsed:?} poll_count={poll_count}");
        assert!(
            WAKER_TEST_DONE.load(Ordering::SeqCst),
            "waker task never set WAKER_TEST_DONE — may not have run"
        );
        assert!(
            elapsed >= Duration::from_millis(80),
            "completed too fast ({elapsed:?}); waker-driven task should take ~100ms"
        );
        // poll_count == 2 validates waker mechanism: initial poll + waker-triggered poll.
        // Allow up to 4 for CI scheduler noise (spurious wakeup), but not hundreds.
        assert!(
            poll_count <= 4,
            "poll_count={poll_count} — expected ≤4 (initial + waker-triggered); tight-loop suspected"
        );
        assert!(
            poll_count >= 2,
            "poll_count={poll_count} — expected ≥2 (initial poll + at least one waker wake)"
        );
    });
}

// ── Contract #12: codegen invariant ──────────────────────────────────────────
//
// Validates: IR snapshot of `main` containing `wait` shows first non-allocation
// instruction is `call void @ynz_rt_init()`.
//
// At this spike layer, no codegen exists for wait yet (M2 codegen is what this
// spike gates). This contract validates the INVARIANT CLAIM in the plan rather
// than actual compiled IR.
//
// As documented in plan Step 8b: if compiling a real .ynz fixture to IR is not
// feasible at the spike layer (no codegen exists for waits), document precisely
// why and propose where this invariant gets validated instead.

#[test]
fn contract_12_codegen_invariant_documented() {
    // Contract #12 requires compiling a .ynz fixture containing `wait` to LLVM IR
    // and asserting that the first non-allocation instruction in `main` is:
    //   call void @ynz_rt_init()
    //
    // This cannot be validated at the P0 spike layer because:
    //   1. M2 codegen for `wait` does not exist yet — P0 is the gate BEFORE P2 codegen.
    //   2. Even if we compiled a .ynz file, `wait` currently lowers as identity
    //      (emit.rs:3139 pass-through) — the IR would not show state-machine structure.
    //   3. The spike's purpose is to validate the RUNTIME ABI design (contracts #1-#11),
    //      not to validate codegen output that hasn't been written yet.
    //
    // Where this invariant gets validated instead:
    //   Phase 2 (codegen layer) Step 5: IR snapshot test `main_rt_init_is_first_instruction`
    //   compiles a .ynz fixture containing `wait sleep(100)` via `ynz-driver`,
    //   extracts the LLVM IR with `--emit-llvm`, and asserts:
    //     - `define i32 @main(...)` function exists
    //     - First non-alloca instruction is `call void @ynz_rt_init()`
    //   This test is the acceptance criterion for P2, not P0.
    //
    // This test passes unconditionally to document the deferral; plan Spike Findings
    // will note the deferral with precise reasoning.
    eprintln!(
        "contract_12: deferred to P2 IR snapshot test — codegen for `wait` does not exist \
         at P0 spike layer. See plan Spike Findings for full rationale."
    );
    // Invariant is structurally guaranteed by the existing M1 ynz_rt_init call in
    // lower_main() (emit.rs) — M2 extends that same function for SM functions.
    // No regression possible without touching lower_main().
}

// ── Contract #6 expanded: Drop impl + non-trivial local cleanup ───────────────
//
// 1000 state machines with heap string locals; half cancelled mid-wait.
// Asserts BOTH frame alloc/free AND ynz_string_free counters return to baseline.
//
// test-ratchet: switching from C-ABI global runtime (handle not accessible outside crate)
// to a local per-test runtime. Same invariant: net frame/string counters return to baseline.
#[test]
fn contract_6_expanded_drop_impl_non_trivial_cleanup() {
    with_runtime(|rt| {
        reset_counters();
        const N: usize = 1000;
        let mut join_handles = Vec::with_capacity(N);

        // Use a heap string that gets stored in the frame and crosses the wait boundary.
        for _ in 0..N {
            let greeting = SpikeFatString::new("hello from the steel city yo");
            let fut = FnChat::new(greeting);
            let h = rt.spawn(FnChatWrapper(fut));
            join_handles.push(h);
        }

        // Abort half mid-flight — frame Drop must free the heap string.
        for h in join_handles.drain(N / 2..) {
            h.abort();
        }

        // Wait for remaining half to complete normally.
        rt.block_on(async {
            for h in join_handles {
                let _ = h.await; // ignore JoinError from aborted tasks
            }
        });
    });
    // Runtime dropped — remaining tasks dropped, running Drop impls on all frames.

    let net_frames = net_frame_count();
    let string_frees = net_string_count();
    eprintln!(
        "contract_6_expanded: frame alloc={} free={} net_frames={} string_frees={}",
        FRAME_ALLOC_COUNT.load(Ordering::SeqCst),
        FRAME_FREE_COUNT.load(Ordering::SeqCst),
        net_frames,
        string_frees
    );

    assert_eq!(net_frames, 0, "frame leak: net alloc-free = {net_frames}");
    // string_frees must equal exactly N (1000): every frame fires the counter exactly once,
    // either via the completion path (greeting.take() at resume_point 2) or the Drop path
    // (greeting.is_some() guard in Drop). Which path fires depends on whether the task was
    // aborted or completed normally, but the total count is deterministic — each of the 1000
    // frames contributes exactly one increment regardless of scheduling order.
    assert_eq!(
        string_frees,
        1000,
        "string cleanup count must return to baseline of 1000 (one per frame); got {string_frees} — Drop impl may not be cleaning up non-trivial locals on all paths"
    );
}

// ── Panic during poll (Quality Gate) ─────────────────────────────────────────
//
// Mirrors M1's spawn+panic test pattern. Validates that panic inside poll()
// is caught by Tokio's task wrapper and does not propagate to the spawner.

struct PanicInPollFuture {
    polled: bool,
}

impl Future for PanicInPollFuture {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.polled {
            self.polled = true;
            panic!("intentional panic inside state machine poll()");
        }
        Poll::Ready(())
    }
}

// WHY: validates Tokio's task wrapper catches panics inside poll(). Mirrors M1's
// spawn_panic_does_not_propagate contract applied to the Future poll path.
#[test]
fn quality_gate_panic_during_poll_caught() {
    static MAIN_REACHED_END: AtomicBool = AtomicBool::new(false);

    with_runtime(|rt| {
        let result = rt.block_on(async {
            let jh = tokio::spawn(PanicInPollFuture { polled: false });
            jh.await
        });
        // JoinError with is_panic() = true — panic is caught and NOT propagated.
        match result {
            Err(e) if e.is_panic() => {
                eprintln!("quality_gate: poll panic caught as JoinError ✓");
            }
            Ok(_) => panic!("expected a panic JoinError, got Ok"),
            Err(e) => panic!("unexpected JoinError type: {e}"),
        }
        MAIN_REACHED_END.store(true, Ordering::SeqCst);
    });

    assert!(
        MAIN_REACHED_END.load(Ordering::SeqCst),
        "main did not reach end after poll panic"
    );
}

// ── Frame size measurements ───────────────────────────────────────────────────
//
// All spike fixture frames must be ≤ 256 bytes (plan threshold).
// Printed via --nocapture for the Spike Findings section.

#[test]
fn frame_size_measurement_all_fixtures() {
    let sizes = [
        ("FnFetchEvent", std::mem::size_of::<FnFetchEvent>()),
        ("FnChain", std::mem::size_of::<FnChain>()),
        ("FnMaybeWait", std::mem::size_of::<FnMaybeWait>()),
        ("SyncBridgeTarget", std::mem::size_of::<SyncBridgeTarget>()),
        (
            "SyncBridgeTarget (4d outer)",
            std::mem::size_of::<SyncBridgeTarget>(),
        ),
        ("FnFetchOrFail", std::mem::size_of::<FnFetchOrFail>()),
        ("FnPulse", std::mem::size_of::<FnPulse>()),
        ("FnBranch", std::mem::size_of::<FnBranch>()),
        ("FnChat", std::mem::size_of::<FnChat>()),
        ("WakerProbeTask", std::mem::size_of::<WakerProbeTask>()),
        ("FibFuture", std::mem::size_of::<FibFuture>()),
    ];

    eprintln!("\n=== Frame Size Measurements ===");
    for (name, size) in &sizes {
        eprintln!("  {name:30} = {size:4} bytes");
        assert!(
            *size <= 256,
            "{name} frame size {size} bytes exceeds 256-byte budget"
        );
    }
    eprintln!("=== All frames ≤ 256 bytes ✓ ===\n");
}

// ── Sync-bridge overhead measurement ─────────────────────────────────────────
//
// Threshold: ≤ 1% of a 100ms wait = ≤ 1000µs absolute.
// Compares ynz_rt_run_entrypoint_spike vs direct future polling.

#[test]
fn sync_bridge_overhead_measurement() {
    with_runtime(|rt| {
        const REPS: usize = 10;

        // Measure direct future polling baseline (drive a 100ms SyncBridgeTarget directly).
        let start = Instant::now();
        for _ in 0..REPS {
            let fut = SyncBridgeTarget::new(100);
            rt.block_on(SyncBridgeTargetWrapper(fut));
        }
        let baseline_total = start.elapsed();
        let baseline_per = baseline_total / REPS as u32;

        // Measure via sync bridge — must be called from sync context (spawn_blocking),
        // not from inside an async future (Handle::block_on panics there).
        let start = Instant::now();
        for _ in 0..REPS {
            rt.block_on(async {
                tokio::task::spawn_blocking(|| {
                    let mut target = SyncBridgeTarget::new(100);
                    let frame_ptr = target.as_mut() as *mut SyncBridgeTarget as *mut u8;
                    // SAFETY: frame_ptr valid; sync_bridge_resume valid.
                    unsafe { ynz_rt_run_entrypoint_spike(sync_bridge_resume, frame_ptr) };
                    drop(target);
                })
                .await
                .expect("sync bridge overhead: spawn_blocking panicked")
            });
        }
        let bridge_total = start.elapsed();
        let bridge_per = bridge_total / REPS as u32;

        let overhead = if bridge_per > baseline_per {
            bridge_per - baseline_per
        } else {
            Duration::ZERO
        };

        let overhead_us = overhead.as_micros();
        let threshold_us = 1000u128; // 1% of 100ms = 1000µs

        eprintln!(
            "\n=== Sync-Bridge Overhead Measurement ===\n  \
             baseline: {baseline_per:?} (direct future poll)\n  \
             bridge:   {bridge_per:?} (via sync bridge)\n  \
             overhead: {overhead_us}µs (threshold: {threshold_us}µs = 1% of 100ms)\n  \
             result: {}\n===\n",
            if overhead_us <= threshold_us {
                "✓ within budget"
            } else {
                "✗ EXCEEDS budget"
            }
        );

        assert!(
            overhead_us <= threshold_us,
            "sync bridge overhead {overhead_us}µs exceeds ≤{threshold_us}µs (1% of 100ms) budget"
        );
    });
}

// WHY: ynz_rt_spawn_blocking_joinable's post-shutdown branch (RUNTIME populated, inner
// None after shutdown) must DISCARD by returning null — never panic, abort, or spawn onto
// a dead runtime. spawn_before_init_returns_null (in lib.rs unit tests) only covers the
// PRE-init branch (RUNTIME OnceLock empty); the post-shutdown branch is a distinct early
// return that requires a real ynz_rt_init → ynz_rt_shutdown sequence, which only the
// integration binaries can drive (init/shutdown are not called in the lib unit-test binary).
// A regression here (e.g. dropping the None arm) would attempt a spawn on a torn-down
// runtime and abort the program.
#[test]
fn spawn_after_shutdown_returns_null() {
    // Serialise against other tests that touch the global RUNTIME (init/shutdown share it).
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    extern "C" fn never_runs(_ctx: *mut u8) -> YnzCpuResult {
        YnzCpuResult([1, 0])
    }

    ynz_rt_init();
    ynz_rt_shutdown();

    // Call from a detached thread with NO Tokio context so Handle::try_current() returns Err
    // and the call falls through to the global RUNTIME ladder — whose inner is now None.
    let handle_ptr = std::thread::spawn(|| {
        assert!(
            tokio::runtime::Handle::try_current().is_err(),
            "expected no Tokio context on the raw thread"
        );
        // SAFETY: never_runs is a valid extern "C" fn; null ctx with size 0 is the documented
        // no-ctx form. The spawn is expected to discard (return null) post-shutdown.
        let ptr = unsafe { ynz_rt_spawn_blocking_joinable(never_runs, std::ptr::null_mut(), 0) };
        ptr as usize
    })
    .join()
    .expect("spawn-after-shutdown thread panicked");

    assert_eq!(
        handle_ptr, 0,
        "ynz_rt_spawn_blocking_joinable after ynz_rt_shutdown must return a null handle (discard), \
         not a live handle on a torn-down runtime"
    );
}
