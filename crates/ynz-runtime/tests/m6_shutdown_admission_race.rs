//! Regression fixture — v0.3-M6 Phase 6b fix-loop round 1: shutdown-admission TOCTOU race.
//!
//! Before this round's fix, `PendingBlockingGuard::new()` unconditionally admitted every
//! `ynz_rt_spawn_blocking`/`_joinable` call — there was no gate against a task ALREADY
//! executing inside the runtime (which resolves its Tokio `Handle` via
//! `Handle::try_current()`, entirely independent of the `RUNTIME` mutex `ynz_rt_shutdown`
//! empties) spawning a NEW blocking task after shutdown had already begun. Such a task
//! could be silently cancelled by Tokio's "non-mandatory" `spawn_blocking` semantics
//! without its body ever running — see `SHUTDOWN_STARTED`'s doc comment in
//! `crates/ynz-runtime/src/runtime.rs` for the full race description and the SeqCst
//! total-order proof of the fix (`PendingBlockingGuard::try_new`'s increment-then-check
//! protocol).
//!
//! This test reproduces the exact scenario the doc comment names: a task ALREADY running
//! inside the runtime (on a blocking-pool worker thread, so `Handle::try_current()`
//! succeeds) attempts a NESTED `ynz_rt_spawn_blocking` call strictly AFTER
//! `ynz_rt_shutdown()` has begun (hundreds of ms of real separation — no reliance on
//! winning a sub-millisecond scheduling race). The assertion: that nested attempt's body
//! must NEVER run.
//!
//! Scope note: this proves the admission GATE closes for a live-Tokio-context caller
//! (the concrete vulnerable case named in the code's own docs) — it does not attempt to
//! flakily race the increment-vs-check ORDERING subtlety itself (whether the guard checks
//! `SHUTDOWN_STARTED` before or after incrementing `PENDING_BLOCKING_TASKS`); that
//! ordering guarantee is proven mathematically inline in
//! `PendingBlockingGuard::try_new`'s own doc comment and is not something a runtime race
//! can verify more rigorously than the proof already does.
//!
//! RED→GREEN proof (recorded in the executor's return, not re-derived here): temporarily
//! reverting `PendingBlockingGuard::try_new` to unconditionally return `Some(Self)` (no
//! `SHUTDOWN_STARTED` check — mirroring the pre-fix `PendingBlockingGuard::new()`) makes
//! this test's core assertion (`RACER_ADMITTED.load() == false`) FAIL, because the racer
//! task's body runs. Restoring the real fix makes it pass.
//!
//! This is the only test in this file (a separate `cargo test` binary — the global
//! `RUNTIME` `OnceLock` is process-scoped; see `m6_shutdown_mutex_scope.rs` for the same
//! one-test-per-file convention and its rationale).
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ynz_runtime::runtime::{ynz_rt_init, ynz_rt_shutdown, ynz_rt_spawn_blocking};

#[test]
fn admission_attempt_from_inside_the_runtime_after_shutdown_began_is_rejected() {
    static OUTER_STARTED: AtomicBool = AtomicBool::new(false);
    static RACER_ADMITTED: AtomicBool = AtomicBool::new(false);

    extern "C-unwind" fn racer_task(_ctx: *mut u8) {
        RACER_ADMITTED.store(true, Ordering::SeqCst);
    }

    extern "C-unwind" fn outer_task(_ctx: *mut u8) {
        OUTER_STARTED.store(true, Ordering::SeqCst);
        // Real-time separation from ynz_rt_shutdown's SHUTDOWN_STARTED store (set as its
        // literal first statement, well before this closure was even picked up by a
        // worker thread) — hundreds of ms vs. an atomic store being near-instantaneous,
        // so there is no ambiguity about which happens first.
        std::thread::sleep(Duration::from_millis(300));
        // Runs ON a Tokio blocking-pool worker thread: Handle::try_current() succeeds
        // here, so this goes through PendingBlockingGuard::try_new() exactly like a task
        // spawned from inside an async future would — NOT the RUNTIME-mutex fallback
        // path a bare external OS thread takes (that path is exercised by
        // m6_shutdown_mutex_scope.rs and rejects via a different, pre-existing check).
        unsafe { ynz_rt_spawn_blocking(racer_task, std::ptr::null_mut(), 0) };
    }

    ynz_rt_init();
    // Bounds worst-case shutdown wait; outer_task itself finishes in ~300ms regardless.
    std::env::set_var("YNZ_SHUTDOWN_TIMEOUT_MS", "3000");

    // SAFETY: outer_task takes no ctx; passing null is safe per its signature.
    unsafe { ynz_rt_spawn_blocking(outer_task, std::ptr::null_mut(), 0) };

    // Wait for outer_task to actually be picked up before triggering shutdown, so the
    // drain loop genuinely observes a pending task rather than racing an unpicked spawn.
    let wait_start = Instant::now();
    while !OUTER_STARTED.load(Ordering::SeqCst) {
        assert!(
            wait_start.elapsed() < Duration::from_secs(5),
            "outer_task never started — blocking pool did not pick up the spawn"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    let shutdown_start = Instant::now();
    let shutdown_thread = std::thread::spawn(|| ynz_rt_shutdown());
    shutdown_thread.join().expect("shutdown thread panicked");
    let shutdown_elapsed = shutdown_start.elapsed();

    std::env::remove_var("YNZ_SHUTDOWN_TIMEOUT_MS");

    assert!(
        !RACER_ADMITTED.load(Ordering::SeqCst),
        "a blocking-task spawn attempt made from INSIDE a still-running task, strictly \
         after ynz_rt_shutdown had already begun, was ADMITTED — the shutdown-admission \
         TOCTOU race is open. The racer task's body ran even though it was spawned well \
         after SHUTDOWN_STARTED flipped true; PendingBlockingGuard::try_new must reject \
         (not silently admit) any such attempt."
    );
    assert!(
        shutdown_elapsed < Duration::from_secs(2),
        "shutdown took {shutdown_elapsed:?} — should complete shortly after outer_task's \
         ~300ms sleep, not stall waiting on a wrongly-admitted racer task or a hung drain \
         loop"
    );
}
