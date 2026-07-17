// v0.3-M7 Phase 4 — regression lock for the general hot-loop O0 stack-exhaustion
// SIGSEGV (roadmap Capability Ledger row 439, plan risk R2's B1-eliminate proof).
//
// Root cause (Paper-Trace in plan `2026-07-04-v0-3-m7-optimizer-pipeline` audit.md,
// Phase 4 entry, 2026-07-17): statement/expression allocas inside plain loop bodies
// execute once per iteration; at -O0 nothing releases them, so the frame grows
// ~18.75 bytes per loop-visit (measured via ulimit bracketing: 131,072 visits needed
// 2304-2368 KB; 262,144 visits needed 4608-5120 KB — linear) until the 8 MB stack
// guard faults (SIGSEGV between 262,144 and 524,288 visits on the N=8 calibration
// workload). Fix: `loop_stack_save`/`loop_stack_restore` (ynz-codegen/src/emit.rs)
// release each iteration's allocas at every plain-loop back-edge and exit.
//
// This test drives the committed 67,108,864-visit fixture at both tiers and asserts
// the exact closed-form checksum: a crash is the regression, a wrong checksum is
// silent corruption (e.g. a restore clobbering a live loop-carried slot). The two
// legs lock DIFFERENT things:
//   - O0 escape hatch: the genuine stack-growth lock — 16x past the old ~4.19M-visit
//     ledger envelope, where the per-iteration alloca growth actually faulted.
//   - Optimized default: run-level checksum + an IR-level lock that the default
//     pipeline RETAINS the loop (back-edge into `while_body`) — verified 2026-07-17:
//     default<O2> does NOT fold the loop closed-form (the runtime calls
//     `ynz_array_get`/`ynz_rt_check_preempt` are opaque), loop-body allocas survive
//     into the optimized IR, and the stacksave/stackrestore release path is genuinely
//     exercised at this tier too.
// Do NOT shrink the visit count or loosen the checksum to make this pass — a red
// here is the row-439 class returning.

use std::{
    path::PathBuf,
    process::{Command, Output},
    time::Duration,
};

/// 67,108,864 visits complete in ~1-4s per tier; a trip is a hang/crash-class
/// regression, never a slow test — fix the codegen, don't widen this.
const RUN_WATCHDOG: Duration = Duration::from_secs(120);

/// reps * 3 * N(N+1)/2 = 8_388_608 * 3 * 36.
const EXPECTED_CHECKSUM: &str = "905969664";

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/v0_3_m7_p4_hot_loop_stack_stress.ynz")
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
                        "WATCHDOG TRIP: stress fixture did not exit within {RUN_WATCHDOG:?} — \
                         treat as a hang/crash-class regression of the row-439 stack bug"
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Build the fixture at one tier and assert exit 0 + the exact checksum.
///
/// When `lock_ir_backedge` is true, the build also emits post-pipeline IR (`--emit-ir`
/// prints the IR the object was actually lowered from — see emit.rs `emit_artifact`)
/// and asserts a retained conditional back-edge into the `while_body` loop header.
fn assert_stress_green(tier_args: &[&str], tier_label: &str, lock_ir_backedge: bool) {
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    let src = fixture();
    let isolated_src = tmp.path().join(src.file_name().expect("fixture filename"));
    std::fs::copy(&src, &isolated_src).expect("failed to copy fixture into tmpdir");

    let mut args: Vec<&str> = vec!["build"];
    args.extend_from_slice(tier_args);
    if lock_ir_backedge {
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
        "ynz build ({tier_label}) failed for the stress fixture:\n{}",
        String::from_utf8_lossy(&build_out.stderr)
    );

    let binary = isolated_src.with_extension("");

    if lock_ir_backedge {
        // IR-level lock (evidence run 2026-07-17): under the default pipeline the
        // loop is RETAINED — `for_after` ends in a conditional branch back into
        // `while_body` (opaque runtime calls prevent closed-form folding). If this
        // assertion ever fires, the pipeline changed shape (folded the loop or
        // renamed the emitted header block) — re-run the IR evidence and re-lock
        // what is actually true; do not just delete the assertion.
        let ir_path = binary.with_extension("ll");
        let ir = std::fs::read_to_string(&ir_path)
            .unwrap_or_else(|e| panic!("failed to read emitted IR at {}: {e}", ir_path.display()));
        let has_backedge = ir
            .lines()
            .any(|line| line.contains("br i1 ") && line.contains("label %while_body"));
        assert!(
            has_backedge,
            "optimized IR has no conditional back-edge into `while_body` — the default \
             pipeline no longer retains the stress loop (folded closed-form or reshaped); \
             this leg's run-level checksum would silently stop exercising the \
             stacksave/stackrestore release path. Re-verify the IR and re-lock."
        );
    }

    let mut run_cmd = Command::new(&binary);
    run_cmd.env("CLICOLOR", "0");
    let run = run_with_watchdog(run_cmd);
    assert_eq!(
        run.status.code().unwrap_or(-1),
        0,
        "stress fixture ({tier_label}) crashed — the row-439 hot-loop stack-exhaustion \
         class has regressed; stderr:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert_eq!(
        stdout.trim(),
        EXPECTED_CHECKSUM,
        "stress fixture ({tier_label}) checksum mismatch — silent corruption in the \
         loop frame-release path"
    );
}

#[test]
fn hot_loop_67m_visits_survives_o0_with_correct_checksum() {
    // WHY: this is the GENUINE stack-growth lock — the O0 escape hatch is where the
    // per-iteration alloca growth manifested (SIGSEGV between 262,144 and 524,288
    // visits pre-fix); 67.1M visits is 16x past the old ~4.19M-visit ledger envelope.
    // Exit 0 + exact checksum or the bug is back.
    assert_stress_green(&["--no-optimize"], "O0 escape hatch", false);
}

#[test]
fn hot_loop_67m_visits_survives_optimized_default_with_correct_checksum() {
    // WHY: the shipped default must hold the same run-level contract, and the
    // IR-level back-edge lock proves this leg still exercises the retained loop's
    // stacksave/stackrestore release path (evidence 2026-07-17: default<O2> keeps
    // the loop + loop-body allocas; opaque runtime calls block closed-form folding)
    // rather than passing vacuously on a folded constant.
    assert_stress_green(&[], "default optimized", true);
}
