/// Production runtime tests for the v0.3-M2 async state-machine shims.
///
/// These tests promote the spike contracts into the permanent test suite.
/// They validate the four new C-ABI shims directly:
///   - `ynz_rt_spawn`
///   - `ynz_rt_async_sleep_create` + `ynz_rt_async_sleep_poll`
///   - `ynz_rt_call_state_machine_sync`
///
/// Each test drives a hand-written state-machine struct that mimics what Phase 2
/// codegen will emit. The shims must be inert (no call sites yet) but must compile
/// and link cleanly, and when driven via these tests must behave per the locked ABI.
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::time::Sleep;
use ynz_runtime::runtime::{
    ynz_rt_async_sleep_create, ynz_rt_async_sleep_poll, ynz_rt_call_state_machine_sync,
    ynz_rt_init, ynz_rt_shutdown, ynz_rt_spawn,
};

// ── Test serialization ────────────────────────────────────────────────────────
//
// Tests that use the C-ABI global RUNTIME static (init/shutdown path) serialize
// via TEST_LOCK to prevent counter cross-contamination between parallel test threads.
// Tests that use only a private runtime skip the lock.

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn with_private_runtime<F: FnOnce(&tokio::runtime::Runtime)>(f: F) {
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(num_cpus::get().max(4))
        .enable_time()
        .build()
        .expect("m2_runtime: failed to build test runtime");
    f(&rt);
}

// ── Frame allocation counters ─────────────────────────────────────────────────

static FRAME_ALLOC: AtomicIsize = AtomicIsize::new(0);
static FRAME_FREE: AtomicIsize = AtomicIsize::new(0);

fn reset_counters() {
    FRAME_ALLOC.store(0, Ordering::SeqCst);
    FRAME_FREE.store(0, Ordering::SeqCst);
}

fn net_frames() -> isize {
    FRAME_ALLOC.load(Ordering::SeqCst) - FRAME_FREE.load(Ordering::SeqCst)
}

// ── Minimal state-machine fixture ────────────────────────────────────────────
//
// Mimics what Phase 2 codegen will emit for:
//   function fetchEvent() -> nothing { wait sleepAsync(N); }
//
// States:
//   0: create sleep handle via ynz_rt_async_sleep_create
//   1: poll handle via ynz_rt_async_sleep_poll; on Ready → 2
//   2: done

struct TestStateMachine {
    resume_point: i32,
    sleep_handle: *mut u8, // opaque handle from ynz_rt_async_sleep_create (null = none)
    sleep_ms: u64,
}

impl TestStateMachine {
    fn new(sleep_ms: u64) -> Box<Self> {
        FRAME_ALLOC.fetch_add(1, Ordering::SeqCst);
        Box::new(Self { resume_point: 0, sleep_handle: std::ptr::null_mut(), sleep_ms })
    }
}

// SAFETY: TestStateMachine is driven to completion by block_on or spawn; ownership of
// sleep_handle (raw pointer to a heap-pinned Sleep) is exclusively held by one task at a time.
unsafe impl Send for TestStateMachine {}

impl Drop for TestStateMachine {
    fn drop(&mut self) {
        FRAME_FREE.fetch_add(1, Ordering::SeqCst);
        // If cancelled mid-wait, the sleep handle is still live and must be freed
        // to avoid a heap leak. Reconstruct and drop the Pin<Box<Sleep>>.
        if !self.sleep_handle.is_null() {
            // SAFETY: sleep_handle was returned by ynz_rt_async_sleep_create
            // (Box::into_raw of Box<Pin<Box<Sleep>>>). Reconstructing the Box here
            // is the correct inverse; it runs the Sleep drop impl.
            drop(unsafe {
                Box::from_raw(self.sleep_handle as *mut Pin<Box<Sleep>>)
            });
            self.sleep_handle = std::ptr::null_mut();
        }
    }
}

impl Future for TestStateMachine {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.resume_point {
                0 => {
                    // State 0: allocate the sleep handle.
                    let ms = self.sleep_ms as i64;
                    self.sleep_handle = ynz_rt_async_sleep_create(ms);
                    self.resume_point = 1;
                }
                1 => {
                    // State 1: poll the sleep handle using the production shim.
                    let waker_ctx = cx as *mut Context<'_> as *mut u8;
                    // SAFETY: sleep_handle is non-null (set in state 0).
                    // waker_ctx is valid for this call's duration.
                    let result =
                        unsafe { ynz_rt_async_sleep_poll(self.sleep_handle, waker_ctx) };
                    match result {
                        1 => return Poll::Pending,
                        0 => {
                            // Ready — shim freed the handle; null our slot.
                            self.sleep_handle = std::ptr::null_mut();
                            self.resume_point = 2;
                        }
                        v => panic!("ynz_rt_async_sleep_poll returned unexpected {v}"),
                    }
                }
                2 => return Poll::Ready(()),
                _ => unreachable!("TestStateMachine: invalid resume_point"),
            }
        }
    }
}

// Wrapper to drive Box<TestStateMachine> as a Future through rt.spawn / rt.block_on.
struct TestSmWrapper(Box<TestStateMachine>);

// SAFETY: TestSmWrapper wraps TestStateMachine whose Send safety is declared above.
unsafe impl Send for TestSmWrapper {}

impl Future for TestSmWrapper {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(self.0.as_mut()).poll(cx)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

// WHY: ynz_rt_async_sleep_create must return a non-null pointer to a valid Sleep future.
// A null return would cause the state machine to dereference null on the first poll.
// Requires a Tokio runtime context because tokio::time::sleep panics outside one.
#[test]
fn sleep_create_returns_non_null() {
    with_private_runtime(|rt| {
        // Must run inside a Tokio context: tokio::time::sleep panics without a reactor.
        let handle = rt.block_on(async { ynz_rt_async_sleep_create(10) });
        assert!(
            !handle.is_null(),
            "ynz_rt_async_sleep_create returned null for ms=10"
        );
        // Reconstruct and drop to avoid leak — this test does not poll.
        // SAFETY: handle was just returned by ynz_rt_async_sleep_create.
        drop(unsafe { Box::from_raw(handle as *mut Pin<Box<Sleep>>) });
    });
}

// WHY: ynz_rt_async_sleep_create(0) must produce a handle that fires immediately on
// the first poll (duration = 0). Drives the zero-duration edge case path.
#[test]
fn sleep_create_zero_ms_fires_immediately() {
    with_private_runtime(|rt| {
        let fired = rt.block_on(async {
            let sm = TestStateMachine {
                resume_point: 0,
                sleep_handle: std::ptr::null_mut(),
                sleep_ms: 0,
            };
            FRAME_ALLOC.fetch_add(1, Ordering::SeqCst);
            let boxed = Box::new(sm);
            TestSmWrapper(boxed).await;
            true
        });
        assert!(fired, "zero-ms sleep should complete");
    });
}

// WHY: ynz_rt_async_sleep_poll drives the full suspend+resume cycle for a 100ms sleep.
// This is the core `wait sleepAsync(N)` primitive. Validates that the waker is registered
// (no tight-loop), the sleep fires after ~100ms, and the handle is freed on Ready (poll
// returns 0). Without this, sleepAsync suspends forever or busy-polls.
#[test]
fn sleep_poll_suspend_and_resume() {
    with_private_runtime(|rt| {
        reset_counters();
        let start = Instant::now();
        rt.block_on(TestSmWrapper(TestStateMachine::new(100)));
        let elapsed = start.elapsed();
        eprintln!("sleep_poll_suspend_and_resume: elapsed={elapsed:?}");
        assert!(
            elapsed >= Duration::from_millis(80),
            "sleep completed too fast ({elapsed:?}); waker may not have fired"
        );
        assert!(
            elapsed < Duration::from_millis(250),
            "sleep took too long ({elapsed:?}); scheduler may be broken"
        );
        assert_eq!(net_frames(), 0, "frame alloc/free mismatch after single sleep");
    });
}

// WHY: 8 simultaneous sleepAsync(100) tasks must complete in ~100ms wall-clock (not 800ms).
// Proves the OS thread is freed during wait (cooperative scheduling). If each task held a
// thread, 8 tasks would still finish in ~100ms on a 4-core machine — what we're proving
// is that the thread pool shares threads across tasks rather than serialising them.
// On a machine with < 8 cores, sequential execution would take >800ms; concurrent
// execution finishes in ~100ms regardless of core count because threads are released.
#[test]
fn sleep_eight_concurrent_share_threads() {
    with_private_runtime(|rt| {
        reset_counters();
        let start = Instant::now();
        const N: usize = 8;
        let mut handles = Vec::with_capacity(N);
        for _ in 0..N {
            let h = rt.spawn(TestSmWrapper(TestStateMachine::new(100)));
            handles.push(h);
        }
        rt.block_on(async {
            for h in handles {
                h.await.expect("spawned sleep task panicked");
            }
        });
        let elapsed = start.elapsed();
        eprintln!("sleep_eight_concurrent_share_threads: elapsed={elapsed:?}");
        assert!(
            elapsed >= Duration::from_millis(80),
            "completed suspiciously fast ({elapsed:?}); sleeps may not have fired"
        );
        assert!(
            elapsed < Duration::from_millis(250),
            "elapsed {elapsed:?} > 250ms — tasks may be running sequentially"
        );
        assert_eq!(net_frames(), 0, "frame leak: {}", net_frames());
    });
}

// ── Sync-bridge value fixture ─────────────────────────────────────────────────
//
// Mimics what M2 codegen emits for a state-machine function that returns 42.
// On terminal transition (Ready), writes 42 into frame slot 0 (the first i32 of the
// frame) so ynz_rt_call_state_machine_sync can read and return the value.
// This matches the ABI locked in the Spike Findings (contract #4d: result=42).

// repr(C) guarantees return_slot is at byte offset 0 (the first field).
// SyncStateFnFuture reads frame slot 0 as *const i32 after block_on returns Ready;
// without repr(C), Rust's default layout may reorder fields and the read returns garbage.
#[repr(C)]
struct SyncValueSm {
    // Frame slot 0: the return value written before signalling Ready.
    // Starts as resume_point discriminant; overwritten with the return value on
    // terminal transition so the sync bridge shim can read it from frame slot 0.
    return_slot: i32,
    sleep_handle: *mut u8,
}

// SAFETY: exclusively owned by the sync bridge call; not shared across threads.
unsafe impl Send for SyncValueSm {}

impl SyncValueSm {
    fn new() -> Box<Self> {
        FRAME_ALLOC.fetch_add(1, Ordering::SeqCst);
        Box::new(Self { return_slot: 0, sleep_handle: std::ptr::null_mut() })
    }
}

impl Drop for SyncValueSm {
    fn drop(&mut self) {
        FRAME_FREE.fetch_add(1, Ordering::SeqCst);
        if !self.sleep_handle.is_null() {
            // SAFETY: sleep_handle was returned by ynz_rt_async_sleep_create.
            drop(unsafe { Box::from_raw(self.sleep_handle as *mut Pin<Box<Sleep>>) });
            self.sleep_handle = std::ptr::null_mut();
        }
    }
}

// extern "C" resume function for SyncValueSm.
// On Ready: writes 42 into frame slot 0 (return_slot), then returns 0.
unsafe extern "C" fn sync_value_sm_resume(frame_ptr: *mut u8, waker_ctx: *mut u8) -> i32 {
    // SAFETY: frame_ptr was cast from *mut SyncValueSm; cast is valid.
    let sm = &mut *(frame_ptr as *mut SyncValueSm);
    // SAFETY: waker_ctx was cast from &mut Context<'_>; valid for call duration.
    let cx = &mut *(waker_ctx as *mut Context<'_>);

    loop {
        match sm.return_slot {
            0 => {
                // State 0: allocate a 50ms sleep.
                sm.sleep_handle = ynz_rt_async_sleep_create(50);
                sm.return_slot = 1;
            }
            1 => {
                // State 1: poll the sleep.
                let waker_ptr = cx as *mut Context<'_> as *mut u8;
                let poll_result = ynz_rt_async_sleep_poll(sm.sleep_handle, waker_ptr);
                match poll_result {
                    1 => return 1, // Pending
                    0 => {
                        // Sleep fired — shim freed the handle.
                        sm.sleep_handle = std::ptr::null_mut();
                        sm.return_slot = 2;
                    }
                    v => panic!("sync_value_sm_resume: unexpected poll result {v}"),
                }
            }
            2 => {
                // Terminal: write the return value into frame slot 0 before signalling Ready.
                // The sync bridge shim reads frame slot 0 as *const i32 after block_on completes.
                sm.return_slot = 42;
                return 0; // Ready
            }
            _ => unreachable!("SyncValueSm: invalid state"),
        }
    }
}

// WHY: ynz_rt_call_state_machine_sync must work from a spawn_blocking thread (the most
// common production call site: a non-wait function calling a state-machine function).
// Handle::try_current() returns Ok from spawn_blocking threads; the Ok arm must run
// handle.block_on() — NOT block_in_place, which panics on blocking-pool threads
// (confirmed by Spike Contract #4b). Any regression here silently re-introduces the
// Round 2 bug. Also validates that the return value (42) propagates correctly through
// the sync bridge, mirroring Spike Contract #4d.
#[test]
fn call_state_machine_sync_from_spawn_blocking() {
    with_private_runtime(|rt| {
        let start = Instant::now();
        let result = rt.block_on(async {
            tokio::task::spawn_blocking(|| {
                let mut sm = SyncValueSm::new();
                let frame_ptr = sm.as_mut() as *mut SyncValueSm as *mut u8;
                // SAFETY: frame_ptr valid; sync_value_sm_resume valid; frame lives for call duration.
                let ret = unsafe {
                    ynz_rt_call_state_machine_sync(sync_value_sm_resume, frame_ptr, 0)
                };
                drop(sm);
                ret
            })
            .await
            .expect("spawn_blocking panicked")
        });
        let elapsed = start.elapsed();
        eprintln!("call_state_machine_sync_from_spawn_blocking: elapsed={elapsed:?} result={result}");
        assert_eq!(result, 42, "sync bridge must return state machine's final value (expected 42)");
        assert!(
            elapsed >= Duration::from_millis(40),
            "sync bridge returned too fast ({elapsed:?})"
        );
        assert!(
            elapsed < Duration::from_millis(250),
            "sync bridge timed out ({elapsed:?})"
        );
    });
}

// WHY: ynz_rt_call_state_machine_sync must work from a thread with no Tokio context
// (e.g., the main thread before ynz_rt_init, or a detached std::thread). This tests
// the Err(_) arm which falls back to RUNTIME.get().block_on(). The global RUNTIME is
// initialised via ynz_rt_init. Without this path, any non-async-context caller would
// panic at Handle::try_current(). The return value (42) must propagate correctly
// through the Err-arm path as well.
#[test]
fn call_state_machine_sync_no_tokio_context() {
    // Serialize with TEST_LOCK because this test touches the global C-ABI RUNTIME.
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ynz_rt_init();

    let start = Instant::now();
    // Use a detached std::thread — no Tokio context (Handle::try_current() returns Err).
    let result = std::thread::spawn(|| {
        let mut sm = SyncValueSm::new();
        let frame_ptr = sm.as_mut() as *mut SyncValueSm as *mut u8;
        // SAFETY: frame_ptr valid; sync_value_sm_resume valid; frame lives for call duration.
        let ret = unsafe {
            ynz_rt_call_state_machine_sync(sync_value_sm_resume, frame_ptr, 0)
        };
        drop(sm);
        ret
    })
    .join()
    .expect("detached thread panicked");

    let elapsed = start.elapsed();
    eprintln!("call_state_machine_sync_no_tokio_context: elapsed={elapsed:?} result={result}");
    assert_eq!(result, 42, "sync bridge Err-arm must return state machine's final value (expected 42)");
    assert!(
        elapsed >= Duration::from_millis(40),
        "sync bridge returned too fast ({elapsed:?}) — may have skipped the sleep"
    );
    assert!(
        elapsed < Duration::from_millis(250),
        "sync bridge timed out ({elapsed:?})"
    );

    ynz_rt_shutdown();
}

// ── SignalSm: state machine that signals a channel on completion ──────────────
//
// Used by rt_spawn_drives_state_machine_on_io_pool to observe completion of a
// fire-and-forget ynz_rt_spawn call. The resume_fn ABI is signal_sm_resume below.

struct SignalSm {
    inner: Box<TestStateMachine>,
    tx: std::sync::mpsc::Sender<()>,
    sent: bool,
}

// SAFETY: inherits TestStateMachine's Send safety (exclusively owned pointer).
unsafe impl Send for SignalSm {}

impl Future for SignalSm {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(self.inner.as_mut()).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => {
                self.sent = true;
                let _ = self.tx.send(());
                Poll::Ready(())
            }
        }
    }
}

// extern "C" resume function for SignalSm — passed to ynz_rt_spawn.
unsafe extern "C" fn signal_sm_resume(frame_ptr: *mut u8, waker_ctx: *mut u8) -> i32 {
    // SAFETY: frame_ptr was cast from *mut SignalSm; cast is valid.
    let sm = &mut *(frame_ptr as *mut SignalSm);
    // SAFETY: waker_ctx was cast from &mut Context<'_>; valid for call duration.
    let cx = &mut *(waker_ctx as *mut Context<'_>);
    match Pin::new(sm).poll(cx) {
        Poll::Ready(()) => 0,
        Poll::Pending => 1,
    }
}

// WHY: ynz_rt_spawn must place a state-machine task on the Tokio I/O pool and drive it
// to completion asynchronously via the C-ABI `ynz_rt_spawn` shim (not rt.spawn directly).
// The task must complete in ~100ms. A regression in ynz_rt_spawn's routing to the global
// RUNTIME would go undetected if this test never calls ynz_rt_spawn — that was the bug
// this round fixes. SignalSm (defined at module level above) wraps TestStateMachine and
// signals a channel on Ready; signal_sm_resume is the extern "C" resume fn passed to
// ynz_rt_spawn. The channel recv_timeout(2s) is the join-equivalent for fire-and-forget.
// test-ratchet: old test used local_rt.spawn (never called ynz_rt_spawn); replaced with
// ynz_rt_spawn on global RUNTIME — stronger, not weaker. The recv_timeout assertion is kept.
#[test]
fn rt_spawn_drives_state_machine_on_io_pool() {
    // Serialize with TEST_LOCK because ynz_rt_spawn uses the global C-ABI RUNTIME.
    let _lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    ynz_rt_init();

    let start = Instant::now();
    // ynz_rt_spawn is fire-and-forget at the C-ABI level; signal completion via channel.
    let (tx, rx) = std::sync::mpsc::channel::<()>();

    // Heap-allocate the SignalSm frame. ynz_rt_spawn takes ownership of frame_ptr.
    // The frame leaks after the task completes (Phase 2 wires dealloc via resume_fn).
    let mut sm = Box::new(SignalSm {
        inner: TestStateMachine::new(100),
        tx,
        sent: false,
    });
    let frame_ptr = sm.as_mut() as *mut SignalSm as *mut u8;
    let frame_size = std::mem::size_of::<SignalSm>() as i64;

    // Transfer ownership into ynz_rt_spawn — do NOT drop sm after this.
    std::mem::forget(sm);

    // SAFETY: frame_ptr valid for frame_size bytes; signal_sm_resume valid; ownership transferred.
    unsafe {
        ynz_rt_spawn(signal_sm_resume, frame_ptr, frame_size);
    }

    rx.recv_timeout(Duration::from_secs(2)).expect("state machine did not complete within 2s");

    let elapsed = start.elapsed();
    eprintln!("rt_spawn_drives_state_machine_on_io_pool: elapsed={elapsed:?}");
    assert!(elapsed >= Duration::from_millis(80), "completed too fast ({elapsed:?})");
    assert!(elapsed < Duration::from_millis(500), "timed out ({elapsed:?})");

    ynz_rt_shutdown();
}

// WHY: Frame allocation must return to baseline after a full spawn+complete cycle.
// A non-zero net count indicates a heap leak in the state-machine frame management.
// This catches the "frame freed but Sleep handle leaked" class of bug (Spike Contract #6
// expanded per Round 3 Required Fix #6).
#[test]
fn frame_no_leak_after_complete() {
    with_private_runtime(|rt| {
        reset_counters();
        const N: usize = 50;
        let mut handles = Vec::with_capacity(N);
        for _ in 0..N {
            handles.push(rt.spawn(TestSmWrapper(TestStateMachine::new(20))));
        }
        rt.block_on(async {
            for h in handles {
                h.await.expect("task panicked");
            }
        });
        assert_eq!(
            net_frames(),
            0,
            "frame leak: alloc={} free={} net={}",
            FRAME_ALLOC.load(Ordering::SeqCst),
            FRAME_FREE.load(Ordering::SeqCst),
            net_frames()
        );
    });
}

// WHY: Panic inside a state-machine's resume function must not abort the Tokio runtime
// or corrupt the thread pool. The panic must be caught by Tokio's task wrapper and
// surface as JoinError::is_panic(). This mirrors Spike quality gate "panic during poll
// is caught by Tokio's task wrapper" and mirrors M1's spawn+panic test.
#[test]
fn panic_during_state_machine_poll_is_caught() {
    with_private_runtime(|rt| {
        struct PanicSm;
        impl Future for PanicSm {
            type Output = ();
            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                panic!("intentional test panic in state-machine poll");
            }
        }
        let h = rt.spawn(PanicSm);
        let result = rt.block_on(h);
        assert!(
            result.is_err() && result.unwrap_err().is_panic(),
            "panic inside poll should surface as JoinError::is_panic()"
        );
    });
}
