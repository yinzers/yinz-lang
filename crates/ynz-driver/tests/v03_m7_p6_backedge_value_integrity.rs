// v0.3-M7 Phase 6 — crossing-local VALUE integrity across real back-edge yield/resume
// cycles (fixture (c), `v0_3_m7_p6_backedge_value_integrity.ynz`).
//
// The starvation harness (v03_m7_backedge_preemption.rs) proves the yield FIRES and
// proves scheduling ORDER under YNZ_WORKER_THREADS=1. Neither of its fixtures proves a
// crossing-local's VALUE survives a real yield/resume through the Phase-6-widened
// SM-routing path — exactly the R8 hazard class this repo has shipped four silent
// miscompiles in (M3a per-type flush-dispatch drift, M3d envelope narrowing, M3e dead
// resolver, M3g re-derived suspend sets). This test closes that gap as a plain
// deterministic correctness proof, NOT a scheduling proof:
//
//   - the fixture's SM loop runs 2,400,000 iterations — past TWO full 2^20 poll budgets
//     (the real shipped PREEMPT_YIELD_INTERVAL, a pure call count, never a clock), so the
//     back edge takes the real store_resume_point -> Pending -> wake -> reload path at
//     least twice, deterministically;
//   - one crossing local per frame value class (int accumulator, int counter, number,
//     string) is live across the back edge, mutated across the yield window, and printed
//     after the loop — the closed-form stdout is byte-exact regardless of WHERE the
//     yields fire, so a wrong byte is a flush/reload miscompile, never scheduling noise.
//
// The IR lock (default tier) guards against this test passing VACUOUSLY: if a future
// admission change silently DECLINES the fixture's function (back_edge_yield_admission,
// crates/ynz-typeck/src/check.rs), the loop would keep legacy plain back edges, never
// yield, and still print correct values — green without exercising the resume path. The
// lock asserts the emitted IR genuinely carries the yield machinery: the
// `sm_backedge_yield` block and a NON-null-waker `ynz_rt_check_preempt` call (plain,
// non-yielding sites pass `ptr null`). If the lock ever fires, the fixture stopped being
// admitted — re-shape the fixture until it is admitted again; do not delete the lock.

use std::{
    path::PathBuf,
    process::{Command, Output},
    time::Duration,
};

/// 2.4M iterations complete in well under a second per tier; a trip is a lost-wake /
/// never-resumed hang (the yield returned Pending and the task was never woken), never
/// a slow test — fix the runtime/codegen, don't widen this.
const RUN_WATCHDOG: Duration = Duration::from_secs(60);

/// The closed-form contract (see the fixture header for the derivation):
/// sum 0..2399999 = 2399999 * 2400000 / 2; frac = 0.25 + 0.5 mutated between the yields.
const EXPECTED_STDOUT: &str = "crossing-intact\n2879998800000\n2400000\n0.75\nmain done\n";

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/v0_3_m7_p6_backedge_value_integrity.ynz")
}

fn run_with_watchdog(mut cmd: Command) -> Output {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("watchdog: failed to spawn child process");
    let start = std::time::Instant::now();
    loop {
        match child
            .try_wait()
            .expect("watchdog: failed to poll child status")
        {
            Some(_status) => {
                return child
                    .wait_with_output()
                    .expect("watchdog: failed to collect child output after exit");
            }
            None => {
                if start.elapsed() >= RUN_WATCHDOG {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "WATCHDOG TRIP: value-integrity fixture did not exit within \
                         {RUN_WATCHDOG:?} — a yield that returned Pending was never \
                         re-woken (lost-wake class); fix the runtime, never widen this"
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Build the fixture at one tier and assert exit 0 + the byte-exact stdout contract.
/// When `lock_ir_yield_path` is true, also `--emit-ir` and assert the admitted yield
/// machinery is structurally present (the vacuous-pass guard described in the header).
fn assert_value_integrity(tier_args: &[&str], tier_label: &str, lock_ir_yield_path: bool) {
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    let src = fixture();
    let isolated_src = tmp.path().join(src.file_name().expect("fixture filename"));
    std::fs::copy(&src, &isolated_src).expect("failed to copy fixture into tmpdir");

    let mut args: Vec<&str> = vec!["build"];
    args.extend_from_slice(tier_args);
    if lock_ir_yield_path {
        args.push("--emit-ir");
    }
    let src_str = isolated_src.to_str().unwrap();
    args.push(src_str);
    let build_out = Command::new(ynz_binary())
        .args(&args)
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz build");
    assert!(
        build_out.status.success(),
        "ynz build ({tier_label}) failed for the value-integrity fixture:\n{}",
        String::from_utf8_lossy(&build_out.stderr)
    );

    let binary = isolated_src.with_extension("");

    if lock_ir_yield_path {
        let ir_path = binary.with_extension("ll");
        let ir = std::fs::read_to_string(&ir_path)
            .unwrap_or_else(|e| panic!("failed to read emitted IR at {}: {e}", ir_path.display()));
        assert!(
            ir.contains("sm_backedge_yield"),
            "emitted IR has no `sm_backedge_yield` block — the fixture's function is no \
             longer ADMITTED to the back-edge poll-yield transform, so this test's \
             run-level contract would pass vacuously (legacy plain back edges, no \
             yield/resume exercised). Re-shape the fixture until it is admitted; do not \
             delete this lock."
        );
        let has_sm_preempt_call = ir
            .lines()
            .any(|line| line.contains("call") && line.contains("@ynz_rt_check_preempt(ptr %"));
        assert!(
            has_sm_preempt_call,
            "emitted IR has no NON-null-waker ynz_rt_check_preempt call — only plain \
             (`ptr null`, discard-result) sites remain, so the loop back edge cannot \
             actually yield and the run-level contract is vacuous. Re-verify admission \
             and the SM back-edge emission (emit_sm_loop_back_edge)."
        );
    }

    let mut run_cmd = Command::new(&binary);
    run_cmd.env("CLICOLOR", "0");
    let run = run_with_watchdog(run_cmd);
    assert_eq!(
        run.status.code().unwrap_or(-1),
        0,
        "value-integrity fixture ({tier_label}) crashed or exited non-zero — a \
         frame-corruption-class regression on the widened SM path; stderr:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert_eq!(
        stdout, EXPECTED_STDOUT,
        "value-integrity fixture ({tier_label}): a crossing-local's value did NOT \
         survive the back-edge yield/resume — silent flush/reload miscompile (R8 class, \
         M3a/M3d/M3e/M3g family)"
    );
}

// WHY: the shipped default must carry the full contract AND the IR lock proving the
// yield/resume path is genuinely exercised (admitted, non-null waker) rather than the
// values being correct over a silently-declined plain loop.
#[test]
fn sm_backedge_yield_resume_preserves_crossing_local_values_default() {
    assert_value_integrity(&[], "default optimized", true);
}

// WHY: the --no-optimize escape hatch shares the identical frame flush/reload machinery
// and must hold the same byte-exact contract — a divergence between the tiers here is
// exactly the O0-reliant class this milestone exists to close.
#[test]
fn sm_backedge_yield_resume_preserves_crossing_local_values_o0() {
    assert_value_integrity(&["--no-optimize"], "O0 escape hatch", false);
}
