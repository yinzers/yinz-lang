//! Regression fixture — v0.3-M6 Phase 6 step 1 (P3-3).
//!
//! `ynz_rt_shutdown` must not hold the global RUNTIME mutex across its up-to-
//! `shutdown_timeout` drain window. This test proves the fix by racing a concurrent
//! RUNTIME-mutex contender against a shutdown that is actively draining a still-running
//! background task, and asserting the contender is not blocked for anywhere near the
//! drain window's length.
//!
//! This is the only test in this file (a separate `cargo test` binary — the global
//! `RUNTIME` `OnceLock` is process-scoped, so no `TEST_LOCK`-style serialization against
//! other test files is needed; see `tests/spike.rs` for that pattern where it IS needed).
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ynz_runtime::runtime::{ynz_rt_init, ynz_rt_shutdown, ynz_rt_spawn_blocking};

// WHY: `ynz_rt_shutdown` extracts the owned `Runtime` via `lock.take()` but, before this
// fix, never dropped the `MutexGuard` before calling the up-to-5s-blocking
// `rt.shutdown_timeout(...)` — Rust does NOT early-drop a named `MutexGuard` at its last
// use (that's an NLL borrow-checking effect, not a Drop-timing one); the guard lives
// until end of scope. So the RUNTIME mutex was held for the ENTIRE drain window. Any
// concurrent caller on RUNTIME's fallback path (a thread outside Tokio calling
// `ynz_rt_spawn_blocking` / `ynz_rt_spawn`) would block on that same mutex for the full
// drain instead of failing fast with "called after ynz_rt_shutdown — task discarded".
// This test proves the fix mirrors `ynz_rt_run_entrypoint`'s already-correct pattern
// (extract-then-release-then-block): a concurrent contender must return promptly, not
// after the drained task's ~600ms sleep.
#[test]
fn shutdown_does_not_hold_runtime_mutex_across_drain_window() {
    static TASK_DONE: AtomicBool = AtomicBool::new(false);

    extern "C-unwind" fn slow_task(_ctx: *mut u8) {
        std::thread::sleep(Duration::from_millis(600));
        TASK_DONE.store(true, Ordering::SeqCst);
    }

    extern "C-unwind" fn noop_task(_ctx: *mut u8) {}

    ynz_rt_init();

    // SAFETY: slow_task takes no ctx; passing null is safe per its signature.
    unsafe { ynz_rt_spawn_blocking(slow_task, std::ptr::null_mut(), 0) };

    // Shutdown runs on its own OS thread so this test thread can race a concurrent
    // RUNTIME-mutex contender against the drain window. The unset YNZ_SHUTDOWN_TIMEOUT_MS
    // production default (5000ms) comfortably covers the 600ms task, so shutdown_timeout
    // waits for the real task completion rather than truncating it.
    let shutdown_thread = std::thread::spawn(|| ynz_rt_shutdown());

    // Head start so the shutdown thread has definitely taken the Runtime (and, pre-fix,
    // would already be holding the mutex across shutdown_timeout) before the contender
    // races it.
    std::thread::sleep(Duration::from_millis(100));

    let contender_start = Instant::now();
    // SAFETY: noop_task takes no ctx; passing null is safe. This thread is NOT inside a
    // Tokio context, so this call takes ynz_rt_spawn_blocking's RUNTIME-mutex fallback
    // path — exactly the path that contends with ynz_rt_shutdown's mutex.
    unsafe { ynz_rt_spawn_blocking(noop_task, std::ptr::null_mut(), 0) };
    let contender_elapsed = contender_start.elapsed();

    shutdown_thread.join().expect("shutdown thread panicked");

    assert!(
        TASK_DONE.load(Ordering::SeqCst),
        "background task never completed — shutdown must drain it, not race past it"
    );
    // Generous threshold: well under the 500ms of drain time still remaining when the
    // contender fires (600ms task minus the 100ms head start), but well above the
    // near-instant mutex-acquire-then-fast-fail the fix produces.
    assert!(
        contender_elapsed < Duration::from_millis(300),
        "a concurrent RUNTIME-mutex contender blocked for {contender_elapsed:?} while \
         shutdown was still draining a 600ms task — the mutex is being held across \
         shutdown_timeout(), reintroducing the P3-3 over-scope bug"
    );
}
