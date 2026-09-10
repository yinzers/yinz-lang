// A local bound AFTER a suspension, and read only where no further suspension separates
// the binding from the read, is NOT a crossing local — these three programs are correct
// Yinz and must compile and run.
//
// The mirror image of `frago002_c1_c2_planned_red.rs`. That file locks the direction
// where the crossing analysis reported TOO LITTLE (a real crossing missed → no frame
// slot → silent wrong output). This file locks the direction where it reports TOO MUCH
// (a non-crossing read reported → an `UnsupportedCrossingLocalType` compile error on a
// valid program). One producer, `collect_crossings_in_stmts`
// (crates/ynz-typeck/src/check.rs), answers both — so both directions need a pin, or the
// next fix to that function trades one failure for the other and the suite applauds.
//
// The false-rejection producer, precisely: `declared` accumulated every local bound since
// the most recent suspension, and nothing asked whether a suspension actually fell
// between a local's binding and the read being scanned. A suspending statement's own
// operands run BEFORE that statement suspends, so a local bound since the last suspension
// and read only there crosses nothing. The fix stages post-suspension bindings in
// `declared_since_suspension` and flushes them into `declared` at the next suspension —
// after the operand scan for a root-form suspending statement, before it for a
// control-flow one (see `SuspensionSite`).
//
// The 626-fixture corpus did not contain this shape, which is exactly why the rejection
// shipped and why these three are committed as fixtures rather than checked ad hoc.
//
// Run (dev container):
//   docker compose exec dev cargo test -p ynz-driver --test post_suspension_local_not_crossing

use std::{
    path::PathBuf,
    process::{Command, Output},
    time::Duration,
};

/// Watchdog ceiling for a compiled fixture run. A trip is a real hang-class miscompile,
/// never a slow test — fix the codegen, don't widen this. (Liveness, not performance:
/// these fixtures run in milliseconds.)
const RUN_WATCHDOG: Duration = Duration::from_secs(60);

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
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

/// The contract, asserted per tier: the program COMPILES (the false-rejection half) and
/// then prints the right answer (the half that proves the widening did not simply trade
/// a compile error for a bad frame layout). Asserted at both tiers because the two
/// halves fail differently — a rejection is identical at both, a layout defect is not.
fn assert_tier_compiles_and_prints(fixture_name: &str, tier_args: &[&str], expected: &str) {
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    let src = fixture(fixture_name);
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
        "{fixture_name} ({tier_args:?}): REJECTED a correct program — a local bound after a \
         suspension and read where no suspension separates the binding from the read is not \
         a crossing local. Producer: `collect_crossings_in_stmts` in \
         crates/ynz-typeck/src/check.rs.\n{}",
        String::from_utf8_lossy(&build_out.stderr)
    );

    let binary = isolated_src.with_extension("");
    let mut run_cmd = Command::new(&binary);
    run_cmd.env("CLICOLOR", "0");
    let run = run_with_watchdog(run_cmd);
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    assert!(
        run.status.success(),
        "{fixture_name} ({tier_args:?}): did not exit 0 (status: {:?}); stderr:\n{}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        stdout, expected,
        "{fixture_name} ({tier_args:?}): compiled but printed the wrong value"
    );
}

fn assert_both_tiers_compile_and_print(fixture_name: &str, expected: &str) {
    assert_tier_compiles_and_prints(fixture_name, &["--no-optimize"], expected);
    assert_tier_compiles_and_prints(fixture_name, &[], expected);
}

/// The reported regression: a `maybe<int>` bound after the leading `wait`, read only in a
/// suspending call's argument list. Rejected as "a `maybe<int>` value cannot yet cross a
/// `wait`" until this dispatch's fix; the identical read compiled the moment the leading
/// `wait sleep(1)` was deleted, which is what identified the false premise.
#[test]
fn maybe_bound_after_wait_read_by_a_suspending_call_is_not_crossing() {
    assert_both_tiers_compile_and_print(
        "v0_3_hardening_c1_post_wait_maybe_read_by_suspending_call.ynz",
        "42\n",
    );
}

/// The same shape on a second unsupported-crossing type (`fixed<T>`), so a green here
/// cannot be one type's guard being loosened — the fix has to be in the shared crossing
/// analysis for both to pass.
#[test]
fn fixed_bound_after_wait_read_by_a_suspending_call_is_not_crossing() {
    assert_both_tiers_compile_and_print(
        "v0_3_hardening_c1_post_wait_fixed_read_by_suspending_call.ynz",
        "3\n",
    );
}

/// The PRE-EXISTING instance: the read is an ordinary non-suspending statement and there
/// is no later suspension at all. This one was rejected on trees well before step 3.1
/// widened anything, so it is the proof that the fix landed at the producer rather than
/// on the statement shape step 3.1 happened to touch.
#[test]
fn maybe_bound_after_wait_with_no_later_suspension_is_not_crossing() {
    assert_both_tiers_compile_and_print(
        "v0_3_hardening_c1_post_wait_maybe_read_no_later_suspension.ynz",
        "42\n",
    );
}
