// WHY: v0.3-M7 Phase 6 — the starvation-proof fixture set for the codegen back-edge
// poll-yield transform (R8's committed RED-repro mitigation, authored BEFORE the transform
// landed). Two fixtures, two verdicts:
//
//   (a) SM-positive: a wait-free hot loop INSIDE a state-machine function must yield the
//       I/O worker within ~one quantum, so a sibling I/O-pool task gets scheduled time
//       BEFORE the hog completes. RED under the v0.3-M1 no-op preempt stub (the hog owns
//       the single worker for the loop's whole duration); GREEN once the back-edge
//       poll-yield suspension point is real.
//
//   (b) non-SM residual: the IDENTICAL hot loop inside a PLAIN function is NOT preempted
//       by this mechanism — its protection is the existing CPU-admission blocking-pool
//       routing. This is an expected-behavior PASS fixture documenting the residual
//       explicitly (IMP-no-function-coloring.md "Scheduler Preemption Model"), not a gap.
//
// Both tests pin YNZ_WORKER_THREADS=1 so "another task on the same worker" is
// deterministic: with exactly one I/O worker, the victim can only run before the hog
// finishes if the hog genuinely yields (fixture a) or genuinely never occupies the I/O
// worker at all (fixture b).

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn run_fixture_single_worker(path: &Path) -> (String, String, i32) {
    let out = Command::new(ynz_binary())
        .args(["run", path.to_str().unwrap()])
        .env("CLICOLOR", "0")
        // One I/O worker: the starvation shape is deterministic (see module WHY).
        // The env var is inherited by the compiled fixture binary `ynz run` spawns.
        .env("YNZ_WORKER_THREADS", "1")
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn ynz: {e}"));
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

// WHY: fixture (a) — proves the back-edge poll-yield is REAL: with one I/O worker, the
// SM hog's wait-free loop must yield within ~one quantum so the victim task runs first.
// Under the no-op stub this asserts RED (victim starves until the hog's loop completes).
#[test]
fn sm_hot_loop_yields_worker_to_sibling_task() {
    let (stdout, stderr, code) =
        run_fixture_single_worker(&fixture("v0_3_m7_p6_backedge_starvation_sm.ynz"));

    assert_eq!(code, 0, "fixture (a) must exit 0; stderr:\n{stderr}");
    assert!(
        stdout.contains("victim ran"),
        "stdout must contain `victim ran`; got:\n{stdout}"
    );
    assert!(
        stdout.contains("hog done"),
        "stdout must contain `hog done`; got:\n{stdout}"
    );

    let victim_pos = stdout.find("victim ran").unwrap();
    let hog_pos = stdout.find("hog done").unwrap();
    assert!(
        victim_pos < hog_pos,
        "back-edge poll-yield starvation proof: `victim ran` must appear BEFORE \
         `hog done` — the SM hog's wait-free hot loop must yield the single I/O worker \
         within ~one quantum instead of running to completion. Output:\n{stdout}"
    );
}

// WHY: fixture (b) — documents the non-SM residual as expected behavior: a plain
// function's hot loop is NOT preempted by the back-edge mechanism (no frame, no Pending,
// nothing to resume); the victim still runs first because CPU-admission routed the plain
// hog to the blocking pool, off the single I/O worker. Passes before AND after the
// transform — proving the non-SM path is untouched by Phase 6.
#[test]
fn non_sm_hot_loop_relies_on_blocking_pool_routing_not_preemption() {
    let (stdout, stderr, code) =
        run_fixture_single_worker(&fixture("v0_3_m7_p6_backedge_residual_nonsm.ynz"));

    assert_eq!(code, 0, "fixture (b) must exit 0; stderr:\n{stderr}");
    assert!(
        stdout.contains("victim ran"),
        "stdout must contain `victim ran`; got:\n{stdout}"
    );
    assert!(
        stdout.contains("plain hog done"),
        "stdout must contain `plain hog done`; got:\n{stdout}"
    );

    let victim_pos = stdout.find("victim ran").unwrap();
    let hog_pos = stdout.find("plain hog done").unwrap();
    assert!(
        victim_pos < hog_pos,
        "non-SM residual documentation: `victim ran` must appear BEFORE `plain hog done` \
         because CPU-admission routes the plain hog OFF the I/O workers (blocking pool) — \
         NOT because any back-edge yield fired inside it. Output:\n{stdout}"
    );
}
