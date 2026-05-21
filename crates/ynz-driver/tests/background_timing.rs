// WHY: v0.3-M1's core promise — `background fn()` schedules fn on a separate thread
// so main continues immediately. The runtime spike tests (crates/ynz-runtime/tests/spike.rs)
// verify the Tokio bridge contracts at the C-ABI level; these tests verify the same
// contracts through the full compilation + execution pipeline (lexer → parser → typeck →
// codegen → linker → binary run).
//
// Without these tests, a regression in the codegen emit_loop_preempt or lower_expr_background
// paths could silently make background sequential again, breaking the milestone promise.

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn run_fixture(path: &Path) -> (String, String, i32) {
    let out = Command::new(ynz_binary())
        .args(["run", path.to_str().unwrap()])
        .env("CLICOLOR", "0")
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn ynz: {e}"));
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

// WHY: verifies the core v0.3-M1 promise end-to-end: `background fn()` must not
// block the main thread. The timing fixture has background sleep 200ms and main
// print `main done` immediately; total program time includes the shutdown drain.
// Tolerances are 4× typical CI noise (CI machines vary by ~10ms on short sleeps).
#[test]
fn background_timing_main_does_not_wait_for_background() {
    let start = Instant::now();
    let (stdout, stderr, code) = run_fixture(&fixture("v0_3_m1_background_timing.ynz"));
    let total_elapsed = start.elapsed();

    assert_eq!(code, 0, "timing fixture must exit 0; stderr:\n{stderr}");
    assert!(
        stdout.contains("main done"),
        "stdout must contain `main done`; got:\n{stdout}"
    );
    assert!(
        stdout.contains("background done"),
        "stdout must contain `background done`; got:\n{stdout}"
    );

    // main done must appear BEFORE background done in the output
    let main_pos = stdout.find("main done").unwrap();
    let bg_pos = stdout.find("background done").unwrap();
    assert!(
        main_pos < bg_pos,
        "`main done` must appear before `background done` in stdout\n{stdout}"
    );

    // Total program time >= 150ms (background slept 200ms; shutdown drained it).
    // This proves the background task actually ran, not just that the strings appeared.
    assert!(
        total_elapsed.as_millis() >= 150,
        "background task must have slept; total elapsed {:?}",
        total_elapsed
    );
}

// WHY: background isolation — the background task running on a separate thread must not
// affect the main thread's exit code or stdout ordering. The runtime-level panic-discard
// contract (catch_unwind) is tested in crates/ynz-runtime/tests/spike.rs (spawn+panic,
// spawn+panic+ctx-no-leak, nested-spawn-inner-panic). This test covers the driver/pipeline
// level: a fixture with a background task exits 0 and main prints its output correctly.
#[test]
fn background_task_completion_does_not_affect_main_exit_code() {
    let (stdout, stderr, code) = run_fixture(&fixture("v0_3_m1_background_panic.ynz"));
    assert_eq!(
        code, 0,
        "background task fixture must exit 0 even when background does unexpected work; stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("main done"),
        "stdout must contain `main done`; got:\n{stdout}"
    );
}
