//! v0.3-M6 P1-1: UFCS suspension-invisibility RED→GREEN fixture class.
//!
//! Authored BEFORE the 4-site fix (verify-before-you-fix): each test asserts the CORRECT
//! behavior — `wait x.method()` / bare UFCS suspending calls suspend exactly like their
//! `Call`-form twins — and fails RED on the pre-fix tree for the documented P1-1 reason
//! (the MethodCall-blind predicate sites lower the call synchronously through the
//! block_on wrapper, which the runtime's own doc contract declares unreachable from a
//! resume fn on a runtime thread).
//!
//! The runner kills the child after a hard timeout: the pre-fix failure mode is pinned by
//! Phase 1 step 8 (block vs panic), so a BLOCK mode must not wedge the suite.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Run `ynz run <fixture>` with a kill-after-timeout guard.
/// Returns (stdout, stderr, exit_code, timed_out). exit_code is -1 when killed/signalled.
fn ynz_run_with_timeout(name: &str, timeout: Duration) -> (String, String, i32, bool) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ynz"))
        .args(["run", fixture(name).to_str().unwrap()])
        .env("CLICOLOR", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn ynz binary");

    let start = Instant::now();
    let mut timed_out = false;
    loop {
        match child.try_wait().expect("try_wait failed") {
            Some(_) => break,
            None if start.elapsed() > timeout => {
                child.kill().ok();
                child.wait().ok();
                timed_out = true;
                break;
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut s) = child.stdout.take() {
        s.read_to_string(&mut stdout).ok();
    }
    if let Some(mut s) = child.stderr.take() {
        s.read_to_string(&mut stderr).ok();
    }
    let code = if timed_out {
        -1
    } else {
        child
            .try_wait()
            .ok()
            .flatten()
            .and_then(|s| s.code())
            .unwrap_or(-1)
    };
    (stdout, stderr, code, timed_out)
}

const RUN_TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn v03_m6_ufcs_transitive_suspension_runs_and_exits_zero() {
    // WHY: a fn whose ONLY suspension source is a UFCS call (`self.crew()`) must be marked
    // suspending by the transitive may-block analysis (call-graph edge for MethodCall) and
    // inline-polled by its SM caller — the R1-parity twin of the Call-form transitive
    // fixture (v0_3_m2_transitive_suspends.ynz). Pre-fix: no edge → synchronous block_on
    // wrapper on a runtime thread (panic or block), never a clean 11/exit-0.
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_ufcs_transitive.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "transitive UFCS fixture must not hang (pre-fix synchronous-block mode?); \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "transitive UFCS suspension must compile and run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "11\n",
        "tally() must return crew(ship)+1 == 11 through the suspension chain; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_ufcs_explicit_wait_on_method_call_suspends() {
    // WHY: `let c = wait ship.crew()` — an explicit `wait` DIRECTLY on a MethodCall node —
    // must be a real suspension point (inline-poll of the callee SM), byte-parity with
    // `let c = wait crew(ship)`. Pre-fix: `is_direct_suspending_call` matches only
    // Expr::Call+Ident, so the statement falls to the no-op wait arm and lowers as a
    // synchronous call on the runtime.
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_ufcs_explicit_wait.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "explicit-wait UFCS fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "explicit `wait ship.crew()` must compile and run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "10\n",
        "wait ship.crew() must yield the cargo value 10; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_ufcs_mixed_subexpr_rejects_both_legs() {
    // WHY: suspending calls in sub-expression position are a PERMANENT typeck reject
    // (Golden Rule 7 / check.rs Check 3). In `fetchBase() + ship.crew()` BOTH legs suspend;
    // UFCS parity demands the teaching error fire for the UFCS leg too. Pre-fix: the
    // subexpr-position walker is MethodCall-blind — only `fetchBase` is reported, and the
    // UFCS leg would silently mis-lower synchronously.
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_ufcs_mixed_subexpr.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "mixed-subexpr fixture must fail at COMPILE time, never run; stdout:\n{stdout}"
    );
    assert_ne!(
        code, 0,
        "suspending calls in sub-expression position must be a compile error; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("`fetchBase` is a suspending call inside a larger expression"),
        "the Call leg must be rejected (pre-existing behavior); stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("`crew` is a suspending call inside a larger expression"),
        "the UFCS leg (`ship.crew()`) must be rejected with the SAME teaching error — \
         UFCS parity for the subexpr-position guard; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_ufcs_background_spawned_method_call_runs() {
    // WHY: `background ship.haul()` must spawn `haul(ship)` (UFCS sugar at the spawn site)
    // as an independent task — parity with `background haul(ship)`. The task's own sleep
    // suspends INSIDE the spawned task; the entrypoint's longer sleep sequences the output
    // deterministically ("Mon" at ~+20ms, "done" at ~+120ms). Pre-fix: the spawn lowering
    // destructures Expr::Call-shaped inners only.
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_ufcs_background.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "background UFCS fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "background-spawned UFCS suspending call must compile and run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "Mon\ndone\n",
        "spawned haul prints first (+20ms), entrypoint prints after its 120ms sleep; \
         stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_ufcs_background_handle_receiver_survives_spawner_frame() {
    // WHY: `let h = background barge.haul()` (HANDLE form) is the same UFCS spawn as the
    // statement form one binding over — the give-transferred shape receiver must be
    // heap-upgraded through the ONE typeck spawn normalization, so the task reads both
    // receiver fields intact after the spawner's resume-fn frame dies at `wait sleep(120)`.
    // Pre-fix (FRAGO 025, security-reproduced): `check_background_handle_spawn` registered
    // ownership / resolved the callee only for `Expr::Call` inners — the receiver rode into
    // the task as a raw pointer to the dead spawner frame (empty name / heap corruption),
    // and the program compiled clean with zero diagnostics.
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_ufcs_background_handle.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "handle-form background UFCS fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "handle-form UFCS spawn must compile and run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "Mon\n7\ndone\n",
        "the spawned task must read BOTH receiver fields intact after the spawner's \
         frame death (handle-form heap upgrade); stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_ufcs_background_narrowed_union_receiver_rejected_both_forms() {
    // WHY: a union binding narrowed to a shape variant (`is Circle =>` arm) must be a
    // FAIL-CLOSED teaching compile error as a `background` receiver in BOTH spawn forms
    // (FRAGO 026). The binding still holds the 16-byte {tag,data} union storage; codegen's
    // Shape heap-upgrade would load sizeof(shape) >= 64 bytes from it — a confirmed
    // out-of-bounds read (CWE-125). Pre-fix: the fixture compiled clean, ran, and printed
    // garbage (`0` for radius 5.0). The durable payload-extraction fix is deferred
    // (Future Requirements #21); until it lands, rejection is the contract.
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_ufcs_background_narrowed_union.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "narrowed-union fixture must fail at COMPILE time, never run; stdout:\n{stdout}"
    );
    assert_ne!(
        code, 0,
        "a narrowed-union background receiver must be a compile error (pre-fix it \
         compiled and ran the OOB read); stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "compile rejection must produce no program output; stdout:\n{stdout}"
    );
    // Statement form (`background fig.haul()`, narrowed to Circle).
    assert!(
        stderr.contains(
            "a union value narrowed to `Circle` cannot yet be used as a `background` receiver"
        ),
        "the STATEMENT-form spawn must be rejected with the teaching error; stderr:\n{stderr}"
    );
    // Handle form (`let h = background shp.mend()`, narrowed to Square).
    assert!(
        stderr.contains(
            "a union value narrowed to `Square` cannot yet be used as a `background` receiver"
        ),
        "the HANDLE-form spawn must be rejected with the SAME teaching error; stderr:\n{stderr}"
    );
    // Exactly one diagnostic per spawn site — no double emission through the shared
    // normalization helper.
    assert_eq!(
        stderr
            .matches("cannot yet be used as a `background` receiver")
            .count(),
        2,
        "exactly one rejection per spawn site (2 spawns); stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_ufcs_background_give_receiver_multifield_survives_spawner_frame() {
    // WHY: the Phase 3c UAF fix (FRAGO 024) heap-upgrades the WHOLE give-transferred UFCS
    // receiver, not just its first slot. The task reads BOTH fields (string + int) after
    // its own suspension — strictly after the spawner's resume-fn frame has died at
    // `wait sleep(120)`. Pre-fix both reads hit freed stack (empty/garbage output).
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_ufcs_background_multifield.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "multifield background UFCS fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "give-receiver UFCS spawn must compile and run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "Mon\n42\ndone\n",
        "the spawned task must read BOTH receiver fields intact after the spawner's \
         frame death (whole-struct heap upgrade); stderr:\n{stderr}"
    );
}
