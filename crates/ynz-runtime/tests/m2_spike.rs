//! The post-shutdown branch of `ynz_rt_spawn_blocking_joinable`, which only a real
//! `ynz_rt_init` -> `ynz_rt_shutdown` sequence can reach. The hand-written state-machine
//! fixtures that used to live here (the P0 spike contracts) ran no `ynz_runtime` code; the real
//! entry points are covered by `m2_runtime.rs` and the `ynz-driver` compiled-program tests.

use std::sync::Mutex;

use ynz_runtime::runtime::{
    ynz_rt_init, ynz_rt_shutdown, ynz_rt_spawn_blocking_joinable, YnzCpuResult,
};

/// Serialises tests that touch the global C-ABI RUNTIME (init and shutdown share it).
static TEST_LOCK: Mutex<()> = Mutex::new(());

// WHY: ynz_rt_spawn_blocking_joinable's post-shutdown branch (RUNTIME populated, inner
// None after shutdown) must DISCARD by returning null — never panic, abort, or spawn onto
// a dead runtime. spawn_before_init_returns_null (in lib.rs unit tests) only covers the
// PRE-init branch (RUNTIME OnceLock empty); the post-shutdown branch is a distinct early
// return that requires a real ynz_rt_init → ynz_rt_shutdown sequence, which only the
// integration binaries can drive (init/shutdown are not called in the lib unit-test binary).
// A regression here (e.g. dropping the None arm) would attempt a spawn on a torn-down
// runtime and abort the program.
// bug: ynz_rt_spawn_blocking_joinable after ynz_rt_shutdown spawns onto the torn-down runtime or aborts instead of discarding the task and returning null | tier: High
#[test]
fn spawn_after_shutdown_returns_null() {
    // Serialise against other tests that touch the global RUNTIME (init/shutdown share it).
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    extern "C-unwind" fn never_runs(_ctx: *mut u8) -> YnzCpuResult {
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
