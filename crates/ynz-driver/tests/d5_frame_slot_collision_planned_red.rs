// v0.3-M7 planned-RED gate — the name-keyed loop-var frame-slot collision UNDERLYING
// Phase 6's D5 admission decline (plan `2026-07-04-v0-3-m7-optimizer-pipeline` Future
// Requirements entry "name-keyed loop-var frame-slot collision", ELEVATED priority).
//
// What D5 did and did NOT do: `for_var_elem_type_conflict` (crates/ynz-typeck/src/
// check.rs) DECLINES back-edge-yield admission when two `for` loops share a loop-var
// name across differently-typed elements — that stops Phase 6 from WIDENING the
// exposure, but the hazard itself PRE-EXISTS Phase 6 for loops whose bodies SUSPEND
// (they were already routed through the SM arms): the crossing frame slot is keyed by
// NAME and classified once from the FIRST loop's element type, so the second loop's
// differently-typed variable flushes/reloads through a mis-classified, mis-sized slot.
//
// Two live manifestations (both 2026-07-17):
//   - this gate's minimized fixture (Point loop then string loop, both suspending):
//     the string element reloads as raw garbage bytes — silent corruption, exit 0;
//   - m5_p5_soa_copy_wait_bg.ynz's original three-loop + background shape (pre-D5
//     widened routing): heap corruption, SIGABRT "corrupted size vs. prev_size".
//
// These tests are the no-duct-tape planned-RED inverse: they assert the CORRECT
// contract (each loop's variable reloads ITS OWN element values at both tiers), so
// they FAIL today and go green only when the real fix — per-loop (or per-name+type)
// frame-slot keying/classification — lands. Do NOT weaken an assertion or assert the
// currently-observed garbage to force green: a red here is silent frame corruption in
// the suspension machinery (the R8 / M3a-family class).
//
// test-ratchet: planned-RED lock authored per the Phase 6 review round (2026-07-17) —
// `#[ignore]`d pending the Future Requirements fix, mirroring the FRAGO 011 / fr23
// precedent (fr23_uaf_planned_red.rs). The fixing milestone removes the `#[ignore]`
// marks; deleting or weakening these tests instead is the test-weakening corpse.
//
// Run explicitly (dev container): docker compose run --rm dev \
//   cargo test -p ynz-driver --test d5_frame_slot_collision_planned_red -- --ignored

use std::{
    path::PathBuf,
    process::{Command, Output},
    time::Duration,
};

/// Watchdog ceiling for a compiled fixture run. A trip is a real deadlock/miscompile,
/// never a slow test — fix the codegen, don't widen this.
const RUN_WATCHDOG: Duration = Duration::from_secs(60);

/// The correct contract: each loop's variable carries ITS OWN loop's element values
/// through every suspension — never another loop's slot classification.
const EXPECTED_STDOUT: &str = "111\nanchor\nharbor\ndone\n";

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/v0_3_m7_d5_suspending_loop_var_slot_collision.ynz")
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
                        "WATCHDOG TRIP: fixture did not exit within {RUN_WATCHDOG:?} — treat \
                         as a hang-class miscompile; fix the codegen, never widen this timeout"
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Build the fixture at ONE tier and assert exit 0 + the byte-exact correct contract.
fn assert_tier_reloads_own_loop_values(tier_args: &[&str], tier_label: &str) {
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    let src = fixture();
    let isolated_src = tmp.path().join(src.file_name().expect("fixture filename"));
    std::fs::copy(&src, &isolated_src).expect("failed to copy fixture into tmpdir");

    let mut args: Vec<&str> = vec!["build"];
    args.extend_from_slice(tier_args);
    let src_str = isolated_src.to_str().unwrap();
    args.push(src_str);
    let build_out = Command::new(ynz_binary())
        .args(&args)
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz build");
    assert!(
        build_out.status.success(),
        "ynz build ({tier_label}) failed for the slot-collision fixture:\n{}",
        String::from_utf8_lossy(&build_out.stderr)
    );

    let binary = isolated_src.with_extension("");
    let mut run_cmd = Command::new(&binary);
    run_cmd.env("CLICOLOR", "0");
    let run = run_with_watchdog(run_cmd);
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();

    assert_eq!(
        run.status.code().unwrap_or(-1),
        0,
        "slot-collision fixture ({tier_label}): non-zero exit — the name-keyed \
         frame-slot collision escalated to a crash (m5_p5's SIGABRT class); stderr:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        stdout, EXPECTED_STDOUT,
        "slot-collision fixture ({tier_label}): a loop variable reloaded through \
         ANOTHER loop's name-keyed frame slot — the pre-existing collision D5 declines \
         around but does not fix"
    );
}

#[test]
#[ignore = "planned-RED: name-keyed loop-var frame-slot collision on suspending-body loops — pre-existing hazard D5 declines around; fix is per-loop slot keying (plan Future Requirements, ELEVATED)"]
fn d5_red_suspending_loops_sharing_var_name_reload_their_own_values_default() {
    // WHY: locks the correct contract on the shipped default tier — the second loop's
    // string variable must reload `anchor`/`harbor`, not raw bytes through the first
    // loop's Point-classified slot.
    assert_tier_reloads_own_loop_values(&[], "default optimized");
}

#[test]
#[ignore = "planned-RED: name-keyed loop-var frame-slot collision on suspending-body loops — pre-existing hazard D5 declines around; fix is per-loop slot keying (plan Future Requirements, ELEVATED)"]
fn d5_red_suspending_loops_sharing_var_name_reload_their_own_values_o0() {
    // WHY: the --no-optimize escape hatch shares the identical name-keyed slot
    // machinery and must hold the same contract independently.
    assert_tier_reloads_own_loop_values(&["--no-optimize"], "O0 escape hatch");
}
