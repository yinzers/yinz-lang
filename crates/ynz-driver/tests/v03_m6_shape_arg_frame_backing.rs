//! v0.3-M6 Phase 1b (FRAGO 004): shape-arg frame-backing use-after-free RED→GREEN class.
//!
//! Authored BEFORE the crossing-classifier fix (RED-repro-before-fix): each test asserts
//! the CORRECT behavior and fails RED on the pre-fix tree for the documented UAF reason —
//! a shape (or `fixed<T>`) aggregate passed BY POINTER to a suspending callee is staged in
//! the PARENT resume fn's stack alloca; the child frame stores a `ptrtoint` of that stack
//! address; the parent returns Pending, its stack dies, and the child resumes on a dangling
//! `self`. Pre-fix signature: silent NONDETERMINISTIC garbage across runs (observed:
//! 13-15-digit pointer-like values, different every run), never a stable wrong value.
//!
//! The sibling UFCS fixture (b) `v0_3_m6_ufcs_explicit_wait.ynz` (carved out of Phase 1 as
//! this phase's locked RED repro) is asserted by `v03_m6_ufcs_suspension.rs`.

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
fn v03_m6_shape_arg_pure_call_yields_deterministic_value() {
    // WHY: `const c = wait crew(ship)` — the pure-`Call` form of the FRAGO 004 UAF. `ship`
    // escapes by pointer into the suspending callee, so it crosses the suspension and must
    // be frame-embedded in the parent's heap frame. Pre-fix: `ship` lives in a parent
    // stack alloca and the resumed child reads freed stack (nondeterministic garbage).
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_shape_arg_pure_call.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "pure-Call shape-arg fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "pure-Call shape-arg fixture must compile and run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "7\n",
        "wait crew(ship) must yield the cargo value 7 — pre-fix this is nondeterministic \
         pointer garbage (the UAF tell); stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_shape_arg_transitive_chain_yields_deterministic_value() {
    // WHY: the AUTO-INSERTED suspension twin — bare `tally(ship)` (no explicit wait at the
    // arg-passing site) through a param-passthrough chain into the suspending leaf. The
    // FRAGO 004 corroboration confirmed the UAF reproduces in this form too; the fix must
    // close BOTH forms via the one crossing classifier.
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_shape_arg_transitive_chain.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "transitive shape-arg fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "transitive shape-arg fixture must compile and run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "8\n",
        "tally(ship) must yield crew(ship)+1 == 8 through the auto-inserted suspension \
         chain — pre-fix this is nondeterministic garbage; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_shape_arg_pure_call_is_deterministic_across_runs() {
    // WHY: the non-vacuous determinism proof (Phase 1b step 5). The pre-fix UAF signature
    // is DIFFERENT garbage on every run (freed-stack reads); a fix that merely masked the
    // symptom (e.g. a coincidentally-stable staging slot) could pass a single-run value
    // assertion. N=10 identical correct values proves the value lives in stable frame
    // memory, not surviving-by-luck stack.
    for run in 0..10 {
        let (stdout, stderr, code, timed_out) =
            ynz_run_with_timeout("v0_3_m6_shape_arg_pure_call.ynz", RUN_TIMEOUT);
        assert!(!timed_out, "run {run}: must not hang; stderr:\n{stderr}");
        assert_eq!(code, 0, "run {run}: must exit clean; stderr:\n{stderr}");
        assert_eq!(
            stdout, "7\n",
            "run {run}: every run must print exactly 7 — any variation is the UAF \
             signature; stderr:\n{stderr}"
        );
    }
}

#[test]
fn v03_m6_number_arg_pure_call_yields_deterministic_value() {
    // WHY: `const y = wait grow(x)` with `x: number` — the FRAGO 006 / signed-R14 growth
    // of the same UAF class. The parent's arg-staging copies the decimal128 bits into a
    // fresh resume-fn STACK temp (load()'s Number arm) and stages ptr_to_int(temp) into
    // the child frame; the parent returns Pending, the temp dies, and the child reads
    // freed memory. Pre-fix signature (probe-confirmed): deterministic 0.000... instead
    // of 2.5. The fix must stage a pointer to FRAME-RESIDENT bits via the one classifier.
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_number_arg_pure_call.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "number-arg fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "number-arg fixture must compile and run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "2.5\n",
        "wait grow(x) must yield 2.5 — pre-fix this reads a dangling stack temp and \
         prints 0.000... (the UAF tell); stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_number_arg_pure_call_is_deterministic_across_runs() {
    // WHY: the non-vacuous determinism proof (Phase 1b step 5) for the number half.
    // A fix that merely masked the symptom (a coincidentally-surviving staging slot)
    // could pass a single-run assertion. N=10 identical correct values proves the
    // staged bits live in stable frame memory, not surviving-by-luck stack.
    for run in 0..10 {
        let (stdout, stderr, code, timed_out) =
            ynz_run_with_timeout("v0_3_m6_number_arg_pure_call.ynz", RUN_TIMEOUT);
        assert!(!timed_out, "run {run}: must not hang; stderr:\n{stderr}");
        assert_eq!(code, 0, "run {run}: must exit clean; stderr:\n{stderr}");
        assert_eq!(
            stdout, "2.5\n",
            "run {run}: every run must print exactly 2.5 — any variation or 0.000 is \
             the UAF signature; stderr:\n{stderr}"
        );
    }
}

#[test]
fn v03_m6_shape_arg_transitive_chain_is_deterministic_across_runs() {
    // WHY: the non-vacuous determinism proof for the transitive-chain form (fix-loop
    // hardening — this repro was single-run while its pure-Call siblings had N=10).
    // The pre-fix UAF signature is DIFFERENT garbage per run; N=10 identical correct
    // values proves the value lives in stable frame memory across the whole chain.
    for run in 0..10 {
        let (stdout, stderr, code, timed_out) =
            ynz_run_with_timeout("v0_3_m6_shape_arg_transitive_chain.ynz", RUN_TIMEOUT);
        assert!(!timed_out, "run {run}: must not hang; stderr:\n{stderr}");
        assert_eq!(code, 0, "run {run}: must exit clean; stderr:\n{stderr}");
        assert_eq!(
            stdout, "8\n",
            "run {run}: every run must print exactly 8 — any variation is the UAF \
             signature; stderr:\n{stderr}"
        );
    }
}

#[test]
fn v03_m6_number_arg_parallel_group_yields_deterministic_values() {
    // WHY: the FRAGO 006 number-arg UAF through the THIRD arg-staging loop
    // (`emit_io_member_init`, the auto-parallelized independent I/O-group path) — missed
    // by the initial two-loop conversion. `grow(x)`/`shrink(y)` form one parallel group;
    // each callee reads its decimal128 arg after its own suspend point. Pre-fix
    // (reproduced 3/3 on the unfixed tree): both print
    // 0.000000000000000000000000000000000000000000000 instead of 2.5/4.5.
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_number_arg_parallel_group.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "parallel-group number-arg fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "parallel-group number-arg fixture must compile and run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "2.5\n4.5\n",
        "the parallel group must yield 2.5 then 4.5 — pre-fix both read dangling stack \
         temps and print 0.000... (the UAF tell); stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_number_arg_parallel_group_is_deterministic_across_runs() {
    // WHY: the non-vacuous determinism proof for the parallel-group number half — a
    // run-to-run garbage UAF must flip a run red (fresh child process per run).
    for run in 0..10 {
        let (stdout, stderr, code, timed_out) =
            ynz_run_with_timeout("v0_3_m6_number_arg_parallel_group.ynz", RUN_TIMEOUT);
        assert!(!timed_out, "run {run}: must not hang; stderr:\n{stderr}");
        assert_eq!(code, 0, "run {run}: must exit clean; stderr:\n{stderr}");
        assert_eq!(
            stdout, "2.5\n4.5\n",
            "run {run}: every run must print exactly 2.5 then 4.5 — any variation or \
             0.000 is the UAF signature; stderr:\n{stderr}"
        );
    }
}

#[test]
fn v03_m6_number_arg_fused_group_yields_deterministic_values() {
    // WHY: the same third-staging-loop UAF through `emit_io_member_init`'s OTHER caller —
    // `emit_fused_group_spawn_poll` (mixed CPU+I/O group). Pre-fix (reproduced 3/3 on
    // the temp-reverted staging loop): 1226 (CPU member, unaffected) then 0.000...
    // instead of 4.5 (the I/O member's decimal128 arg read from freed stack).
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_number_arg_fused_group.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "fused-group number-arg fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "fused-group number-arg fixture must compile and run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "1226\n4.5\n",
        "the fused group must yield 1226 (CPU) then 4.5 (I/O number arg) — pre-fix the \
         number reads a dangling stack temp and prints 0.000...; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_number_arg_fused_group_is_deterministic_across_runs() {
    // WHY: the non-vacuous determinism proof for the fused-group number half (fresh
    // child process per run; a run-to-run garbage UAF must flip a run red).
    for run in 0..10 {
        let (stdout, stderr, code, timed_out) =
            ynz_run_with_timeout("v0_3_m6_number_arg_fused_group.ynz", RUN_TIMEOUT);
        assert!(!timed_out, "run {run}: must not hang; stderr:\n{stderr}");
        assert_eq!(code, 0, "run {run}: must exit clean; stderr:\n{stderr}");
        assert_eq!(
            stdout, "1226\n4.5\n",
            "run {run}: every run must print exactly 1226 then 4.5 — any variation or \
             0.000 is the UAF signature; stderr:\n{stderr}"
        );
    }
}

#[test]
fn v03_m6_non_escaping_number_and_fixed_args_are_not_wrongly_affected() {
    // WHY: the committed FALSE-POSITIVE sweep (previously only run on gitignored probe
    // files). A NON-escaping `number` arg to a pure callee must not be wrongly
    // frame-backed (still computes 3.5) and a NON-escaping `fixed<int>` arg to a pure
    // callee must not be wrongly Check-2b-rejected (compiles, runs, prints 9). The
    // fixture's layout notes document two TRUE-crossing shapes (declare-after-wait
    // reads; adjacent CPU-spike pairs) this sweep deliberately avoids.
    let (stdout, stderr, code, timed_out) = ynz_run_with_timeout(
        "v0_3_m6_non_escaping_args_false_positive_sweep.ynz",
        RUN_TIMEOUT,
    );
    assert!(
        !timed_out,
        "false-positive sweep fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "non-escaping number/fixed args must compile and run clean (a Check 2b rejection \
         here is a classifier false positive); stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "3.5\n9\n",
        "non-escaping args must be untouched: bump(x) == 3.5, headroom(nums) == 9; \
         stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_fixed_arg_to_suspending_callee_is_rejected() {
    // WHY: a `fixed<T>` local escaping by pointer into a suspending callee is the same
    // dangling-stack class, but fixed arrays cannot yet be frame-backed (stack `[N x i64]`
    // allocas; recursive aggregate frame-embedding is a later milestone). The established
    // design for a fixed<T> crossing a suspension is the UnsupportedCrossingLocalType
    // teaching error (check.rs Check 2b) — the escape-through-callee form must route into
    // that SAME guard via the one classifier. Pre-fix: this compiles and runs (exit 0),
    // shipping the silent-UAF hazard.
    let (stdout, stderr, code, timed_out) = ynz_run_with_timeout(
        "v0_3_m6_fixed_arg_suspending_call_rejected.ynz",
        RUN_TIMEOUT,
    );
    assert!(
        !timed_out,
        "fixed-arg fixture must fail at COMPILE time, never run; stdout:\n{stdout}"
    );
    assert_ne!(
        code, 0,
        "a fixed<int> passed to a suspending callee must be a compile error — pre-fix it \
         compiles and runs with a dangling stack pointer; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("cannot yet cross a `wait`"),
        "the UnsupportedCrossingLocalType teaching error must fire; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("array<T>"),
        "the teaching error must suggest the heap-backed `array<T>` alternative; \
         stderr:\n{stderr}"
    );
}
