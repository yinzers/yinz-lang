//! v0.3-M6 Phase 1d (FRAGO 009): decimal128-across-a-CONCURRENCY-boundary RED→GREEN class.
//!
//! Authored BEFORE the fix (RED-repro-before-fix): each test asserts the CORRECT behavior
//! and fails RED on the pre-fix tree (759bd9b) for the documented reason:
//!
//! - **Defect A (background-spawn `number` arg — silent runtime UAF):**
//!   `prepare_bg_arg_for_ctx` has no `Number` arm, so a decimal128 arg falls through
//!   by-pointer (`BgArgFreeKind::None`) — both spawn arms (`ynz_rt_spawn_blocking` CPU
//!   path and `ynz_rt_spawn` SM path) stage ptr_to_int of a SPAWNER-STACK i128 temp; the
//!   spawner suspends, its stack dies, and the task dereferences the dangling pointer.
//!   Pre-fix signature (probe-confirmed 3/3 per arm): deterministic
//!   `0.000000000000000000000000000000000000000000000` instead of the staged value.
//!
//! - **Defect C (cpu-member `number` arg — loud compile-time ICE on valid code):**
//!   `emit_cpu_member_spawn` build_load(i64)s the member's local slot and the shared
//!   `build_cpu_trampoline` passes that i64 where the compiled callee expects its
//!   pointer-typed number param. Pre-fix signature (probe-confirmed, pure-spike AND
//!   fused): `LLVM module verify failed: Call parameter type does not match function
//!   signature!` surfaced as "This is a compiler bug." — exit != 0, no binary.
//!
//! The suspending-CALL half of the same decimal128 class (Phase 1b, FRAGO 006/R14) is
//! asserted by `v03_m6_shape_arg_frame_backing.rs`; the conduit-send half is
//! VERIFIED-SAFE-BY-GATE (`channel<number>` compile-gated) and recorded as
//! Future-Requirements deferral #12, deliberately not fixed or asserted here.

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
fn v03_m6_number_arg_background_cpu_arm_yields_staged_value() {
    // WHY: `background report(x)` with a NON-suspending callee routes through
    // `ynz_rt_spawn_blocking` — the FRAGO 009 defect-A CPU arm. The task must read the
    // decimal128 the spawner staged, not the corpse of the spawner's stack. Pre-fix:
    // deterministic 0.000... (the UAF tell).
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_number_arg_background_cpu.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "bg-CPU number-arg fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "bg-CPU number-arg fixture must compile and run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "2.5\n",
        "the blocking-pool task must print the staged 2.5 — pre-fix it reads a dangling \
         spawner-stack temp and prints 0.000... (the UAF tell); stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_number_arg_background_cpu_arm_is_deterministic_across_runs() {
    // WHY: the non-vacuous determinism proof (Phase 1d step 4). A fix that merely
    // masked the symptom (a coincidentally-surviving stack slot — exactly what the
    // original spawn-then-return-immediately shape showed pre-fix) could pass a
    // single-run assertion. N=10 identical correct values across fresh processes
    // proves the arg lives in owned heap memory, not surviving-by-luck stack.
    for run in 0..10 {
        let (stdout, stderr, code, timed_out) =
            ynz_run_with_timeout("v0_3_m6_number_arg_background_cpu.ynz", RUN_TIMEOUT);
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
fn v03_m6_number_arg_background_sm_arm_yields_staged_value() {
    // WHY: `background fetchNum(x)` with a SUSPENDING callee routes through
    // `ynz_rt_spawn` — the FRAGO 009 defect-A SM arm. The resumed task's number-param
    // read (`sm_number_param_set` indirection, Phase 1b's child-side convention) must
    // dereference a pointer to bits the task OWNS. Pre-fix: deterministic 0.000...
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_number_arg_background_sm.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "bg-SM number-arg fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "bg-SM number-arg fixture must compile and run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "4.5\n",
        "the spawned state machine must print the staged 4.5 — pre-fix it reads a \
         dangling spawner-stack temp and prints 0.000... (the UAF tell); stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_number_arg_background_sm_arm_is_deterministic_across_runs() {
    // WHY: the non-vacuous determinism proof for the SM arm (Phase 1d step 4) —
    // fresh child process per run; a run-to-run garbage UAF must flip a run red.
    for run in 0..10 {
        let (stdout, stderr, code, timed_out) =
            ynz_run_with_timeout("v0_3_m6_number_arg_background_sm.ynz", RUN_TIMEOUT);
        assert!(!timed_out, "run {run}: must not hang; stderr:\n{stderr}");
        assert_eq!(code, 0, "run {run}: must exit clean; stderr:\n{stderr}");
        assert_eq!(
            stdout, "4.5\n",
            "run {run}: every run must print exactly 4.5 — any variation or 0.000 is \
             the UAF signature; stderr:\n{stderr}"
        );
    }
}

#[test]
fn v03_m6_number_arg_cpu_member_compiles_and_yields_values() {
    // WHY: FRAGO 009 defect C through the pure-CPU spike-group path — a number-arg
    // member is VALID Yinz that auto-parallelization chose to spawn; it must compile
    // and produce the same values sequential execution would. Pre-fix: LLVM verifier
    // ICE ("compiler bug") — the trampoline passes a raw i64 where the callee's
    // pointer-typed number param is declared.
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_number_arg_cpu_member.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "cpu-member number-arg fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "a number-arg CPU member is valid code the compiler itself chose to \
         parallelize — it must compile and run clean, never ICE; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "2.5\n4.5\n",
        "both members' decimal128 args must cross the blocking-pool boundary intact; \
         stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_number_arg_cpu_member_is_deterministic_across_runs() {
    // WHY: the non-vacuous determinism proof for the spike-group half (Phase 1d
    // step 4) — the staged cells must be per-member owned heap, not shared or
    // surviving-by-luck storage.
    for run in 0..10 {
        let (stdout, stderr, code, timed_out) =
            ynz_run_with_timeout("v0_3_m6_number_arg_cpu_member.ynz", RUN_TIMEOUT);
        assert!(!timed_out, "run {run}: must not hang; stderr:\n{stderr}");
        assert_eq!(code, 0, "run {run}: must exit clean; stderr:\n{stderr}");
        assert_eq!(
            stdout, "2.5\n4.5\n",
            "run {run}: every run must print exactly 2.5 then 4.5; stderr:\n{stderr}"
        );
    }
}

#[test]
fn v03_m6_number_arg_fused_cpu_member_compiles_and_yields_values() {
    // WHY: FRAGO 009 defect C through `emit_cpu_member_spawn`'s OTHER caller — the
    // fused (mixed CPU+I/O) group path (`emit_fused_group_spawn_poll`). Same shared
    // trampoline, same ICE pre-fix; the fix must land once in the shared path and
    // green both callers.
    let (stdout, stderr, code, timed_out) =
        ynz_run_with_timeout("v0_3_m6_number_arg_fused_cpu_member.ynz", RUN_TIMEOUT);
    assert!(
        !timed_out,
        "fused cpu-member number-arg fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "a number-arg CPU member of a fused group must compile and run clean, never \
         ICE; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "2.5\n7\n",
        "the fused group must yield 2.5 (CPU member's number arg) then 7 (I/O member); \
         stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_int_literal_to_number_param_cpu_member_is_clean_teaching_error() {
    // WHY: `f(5)` where `f(n: number)` at the cpu-member spawn boundary — the
    // Future-Req #14 int→number call-site coercion gap. The interim guard must be a
    // CLEAN typeck teaching error (WHAT/WHAT-INSTEAD/WHY), never the generic
    // "compiler bug" ICE banner (the pre-fix-loop behavior: the codegen guard's raw
    // Err(String) hit the ICE wrapper) and never a crash/hang from staging a raw
    // i64 as a heap-cell pointer.
    assert_int_literal_number_teaching_error(
        "v0_3_m6_int_literal_number_arg_cpu_member.ynz",
        "plain-call",
    );
}

/// Shared assertions for the int-literal→`number`-param teaching error across
/// call forms: rejected at compile time, the WHAT + WHAT-INSTEAD text present,
/// and never the ICE banner — byte-identical diagnostics between call forms per
/// non-oop.md's dual-style convention.
fn assert_int_literal_number_teaching_error(fixture_name: &str, call_form: &str) {
    let (stdout, stderr, code, timed_out) = ynz_run_with_timeout(fixture_name, RUN_TIMEOUT);
    assert!(
        !timed_out,
        "{call_form} int-literal-to-number fixture must not hang; \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_ne!(
        code, 0,
        "an int literal into a `number` param through the {call_form} form must be \
         REJECTED at compile time (coercion doesn't exist yet — Future-Req #14); \
         stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("passing an int literal to a `number` parameter is not supported yet"),
        "the {call_form} form must render the SAME teaching diagnostic as the plain \
         call form (non-oop.md dual-style convention); stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("5.0"),
        "the teaching diagnostic must offer the decimal-literal WHAT-INSTEAD (`5.0`); \
         stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("compiler bug"),
        "a KNOWN, user-triggerable limitation must never render the ICE banner \
         (the pre-round-2 behavior for the {call_form} form); stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FRAGO"),
        "internal tracking IDs must never leak into user-facing text; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_int_literal_to_number_param_ufcs_is_clean_teaching_error() {
    // WHY: `p.scale(5)` where `scale(share self: Player, factor: number)` — the
    // UFCS dot-call form (Yinz's PRIMARY idiom) previously BYPASSED the typeck
    // gate: the MethodCall arm inferred non-receiver args with no hint and
    // `check_method_call` checked only receiver-vs-first-param, so the program
    // passed typeck and ICE'd at codegen (LLVM verifier: `i64 5` against the
    // callee's pointer-typed decimal128 param, rendered as "compiler bug").
    // Locked RED against the pre-fix tree; the shared gate must now fire here
    // with the SAME text as `scale(p, 5)`.
    assert_int_literal_number_teaching_error(
        "v0_3_m6_int_literal_number_arg_ufcs.ynz",
        "UFCS dot-call",
    );
}

#[test]
fn v03_m6_int_literal_to_number_param_generic_fn_is_clean_teaching_error() {
    // WHY: `scale(5, tag)` where `function scale<T>(factor: number, give item: T)`
    // — a CONCRETE `number` param of a generic fn previously BYPASSED the typeck
    // gate: `check_generic_fn_call` inferred args with no hint and DISCARDED the
    // `unify_param` mismatch (`let _ =`), so the program passed typeck silently
    // and ICE'd at codegen (internal panic: IntValue where PointerValue expected,
    // rendered as the "compiler bug" panic banner). Locked RED against the
    // pre-fix tree; the shared gate must now fire here with the SAME text.
    assert_int_literal_number_teaching_error(
        "v0_3_m6_int_literal_number_arg_generic_fn.ynz",
        "generic-fn",
    );
}

/// Build a fixture with `--emit-ir` (isolated in a tempdir so parallel tests
/// never race on a shared `.ll` path) and return the emitted IR text.
fn emit_fixture_ir(name: &str) -> String {
    let src = fixture(name);
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    let isolated_src = tmp.path().join(src.file_name().expect("fixture filename"));
    std::fs::copy(&src, &isolated_src).expect("failed to copy fixture to tmpdir");

    // --no-optimize pins the CODEGEN-emission anchor tier (v0.3-M7 Phase 3): since the
    // default build optimizes, --emit-ir prints POST-`default<O2>` IR, where the local
    // value-name markers these tests assert (e.g. `_num_ld`) are legally renamed or
    // folded away. The markers are claims about what CODEGEN emits (does the spawn
    // pre-gate interpose a heap cell?), not about the optimized artifact — same
    // anchor-pinning move as optimizer_red_gate.rs's differential harness.
    let build_out = Command::new(env!("CARGO_BIN_EXE_ynz"))
        .args([
            "build",
            "--no-optimize",
            isolated_src.to_str().unwrap(),
            "--emit-ir",
        ])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz build");
    assert!(
        build_out.status.success(),
        "fixture `{name}` must build clean; stderr:\n{}",
        String::from_utf8_lossy(&build_out.stderr)
    );
    std::fs::read_to_string(isolated_src.with_extension("ll"))
        .expect("emitted .ll must be readable")
}

/// The IR value-name markers the Option-2 (D8) number heap-cell machinery emits,
/// each paired with a BOUNDARY fixture whose IR must contain it (the positive
/// control that keeps the negative sweep assertions below non-vacuous — a codegen
/// rename flips the control red instead of silently hollowing the sweep).
///
/// Markers deliberately restricted to names PROVEN to appear in emitted IR:
/// `bg_number`/`bg_number_num_ld` (the `prepare_bg_arg_for_ctx` Number pre-gate's
/// cell + load), `spike_num_{idx}` sites' `_num_ld`, `_num_bits_` (the cpu-member
/// staged ptr_to_int), and `spike_num_arg_ptr` (the trampoline's int_to_ptr
/// reconstruction). The former `number_to_heap_cell` marker was a Rust fn name
/// that never reaches IR, and `spike_num_free` names a void `call` whose name
/// LLVM drops — both structurally inert, so neither is a legal marker here.
const NUMBER_HEAP_CELL_IR_MARKERS: &[(&str, &str)] = &[
    ("bg_number", "v0_3_m6_number_arg_background_cpu.ynz"),
    ("_num_ld", "v0_3_m6_number_arg_background_cpu.ynz"),
    ("_num_bits_", "v0_3_m6_number_arg_cpu_member.ynz"),
    ("spike_num_arg_ptr", "v0_3_m6_number_arg_cpu_member.ynz"),
];

#[test]
fn v03_m6_non_escaping_number_ir_stays_by_pointer() {
    // WHY: persists Phase 1d step-4's IR-level false-positive proof as a committed
    // check (it previously lived only in a session-local `--emit-ir` run with no
    // artifact on disk). A NON-escaping `number` arg to a pure callee must not be
    // heap-cell-copied by the Option-2 (D8) spawn-boundary pre-gate: the sweep
    // fixture's IR must carry ZERO number heap-cell markers, and `bump(x)` must
    // lower to a plain by-pointer call. Note the sweep fixture has no spawn calls
    // at all, so this pins code-path disjointness (Phase 1b/1c classifier + normal
    // call lowering untouched) — in-boundary correctness is asserted by the eight
    // value/determinism repros above, not by this sweep.
    //
    // Each marker's absence claim is DISCRIMINATING, not vacuous: the same marker
    // is asserted PRESENT in a boundary fixture's IR (the positive control), so a
    // future rename of codegen's emitted names breaks this test loudly instead of
    // silently turning the sweep into a list of strings nothing ever emits.
    let sweep_ir = emit_fixture_ir("v0_3_m6_non_escaping_args_false_positive_sweep.ynz");

    let mut control_irs: std::collections::BTreeMap<&str, String> = Default::default();
    for (marker, control_fixture) in NUMBER_HEAP_CELL_IR_MARKERS {
        assert!(
            !sweep_ir.contains(marker),
            "non-escaping number args must not touch the spawn-boundary heap-cell \
             machinery — found marker `{marker}` in the sweep fixture's IR"
        );
        let control_ir = control_irs
            .entry(control_fixture)
            .or_insert_with(|| emit_fixture_ir(control_fixture));
        assert!(
            control_ir.contains(marker),
            "positive control failed: marker `{marker}` no longer appears in the \
             boundary fixture `{control_fixture}`'s IR — codegen's emitted names \
             have drifted from this marker list. Update the marker to the renamed \
             IR value name (do NOT delete the assertion): without a present-in-IR \
             control, the sweep's absence checks above prove nothing."
        );
    }
    assert!(
        sweep_ir.contains("@bump(ptr"),
        "the non-escaping `bump(x)` call must stay a plain by-pointer call \
         (`call ... @bump(ptr ...)`), proving no cell was interposed; IR:\n{sweep_ir}"
    );
}

#[test]
fn v03_m6_number_arg_fused_cpu_member_is_deterministic_across_runs() {
    // WHY: the non-vacuous determinism proof for the fused half (Phase 1d step 4) —
    // fresh child process per run.
    for run in 0..10 {
        let (stdout, stderr, code, timed_out) =
            ynz_run_with_timeout("v0_3_m6_number_arg_fused_cpu_member.ynz", RUN_TIMEOUT);
        assert!(!timed_out, "run {run}: must not hang; stderr:\n{stderr}");
        assert_eq!(code, 0, "run {run}: must exit clean; stderr:\n{stderr}");
        assert_eq!(
            stdout, "2.5\n7\n",
            "run {run}: every run must print exactly 2.5 then 7; stderr:\n{stderr}"
        );
    }
}

// ── Future-Req #14 interim guard — COMPLETION sweep (fix-loop round 3) ────────
//
// Round 2 unified the three call FORMS under the ONE shared typeck gate; round 3
// completes the CLASS: every slot where an `IntLit` (or `-IntLit`) can reach a
// `number`-typed position — generic explicit/sibling-bound params, built-in
// collection method args (elem/key/value), construction slots (shape fields,
// array/fixed/map literals), statement slots (index/bracket/field assigns,
// `return`, multi-case-if arm patterns), and map bracket keys. Each fixture's
// header records its pre-fix RED signature (segfault / LLVM verifier ICE /
// internal panic banner / silent-wrong output), all probe-confirmed against the
// pre-fix tree. The store sites (`let x: number = 5` binding; `hidden f: number = 5`
// field default) are ALSO gated now — see the v0.3-M6 store-site stopgap tests below.
//
// INTENDED BEHAVIOR CHANGE (v0.3-M6 store-site stopgap, human-directed "no duct
// tape"): Phase 1d deliberately left the store sites un-gated — this comment
// previously pinned `let x: number = 5` as "deliberately NOT gated (Future-Req #9)".
// The stopgap flips that: both store sites now REJECT with the SAME teaching error as
// every other slot (store-site behavior intentionally changed ICE→teaching-error).
// This is an intended contract change, NOT a weakened test. The int→number COERCION
// (not the rejection) remains the deferred piece, routed to the
// 2026-07-04-v0-3-hotfix-int-literal-number stub plan, which will REPLACE rejection
// with coercion across ALL facets uniformly.

/// Round-3 shared assertions: the int-literal→`number` teaching error at EVERY
/// slot the class reaches. Asserts the role-agnostic CORE text (the
/// subject/noun clause legitimately varies per slot role); byte-identity across
/// the three call FORMS stays pinned by
/// `assert_int_literal_number_teaching_error` above.
fn assert_int_literal_number_slot_teaching_error(fixture_name: &str, slot: &str, lit: &str) {
    let (stdout, stderr, code, timed_out) = ynz_run_with_timeout(fixture_name, RUN_TIMEOUT);
    assert!(
        !timed_out,
        "{slot} int-literal-to-number fixture must not hang; \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_ne!(
        code, 0,
        "an int literal into a `number` {slot} slot must be REJECTED at compile \
         time (coercion doesn't exist yet — Future-Req #14); stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("is an int literal — passing an int literal to a `number`"),
        "the {slot} slot must render the shared teaching-diagnostic core \
         (one gate, byte-identical core text across slots); stderr:\n{stderr}"
    );
    assert!(
        stderr.contains(&format!("Write it as a decimal literal: `{lit}.0`")),
        "the teaching diagnostic must offer the decimal-literal WHAT-INSTEAD \
         (`{lit}.0`); stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("compiler bug"),
        "a KNOWN, user-triggerable limitation must never render the ICE banner; \
         stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FRAGO"),
        "internal tracking IDs must never leak into user-facing text; stderr:\n{stderr}"
    );
}

// WHY (per test): each fixture's header comment records the slot's pre-fix RED
// signature; the test locks the post-fix contract (clean teaching error).
macro_rules! int_literal_number_slot_test {
    ($test_name:ident, $fixture:literal, $slot:literal, $lit:literal) => {
        #[test]
        fn $test_name() {
            assert_int_literal_number_slot_teaching_error($fixture, $slot, $lit);
        }
    };
}

int_literal_number_slot_test!(
    v03_m6_int_lit_number_generic_explicit_is_teaching_error,
    "v0_3_m6_int_literal_number_arg_generic_explicit.ynz",
    "generic explicit-type-arg param",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_generic_sibling_is_teaching_error,
    "v0_3_m6_int_literal_number_arg_generic_sibling.ynz",
    "generic sibling-bound param",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_array_add_is_teaching_error,
    "v0_3_m6_int_literal_number_arg_array_add.ynz",
    "array-add elem (pre-fix SEGFAULT class)",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_array_set_is_teaching_error,
    "v0_3_m6_int_literal_number_arg_array_set.ynz",
    "array-set elem",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_fixed_set_is_teaching_error,
    "v0_3_m6_int_literal_number_arg_fixed_set.ynz",
    "fixed-set elem",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_map_set_value_is_teaching_error,
    "v0_3_m6_int_literal_number_arg_map_set.ynz",
    "map-set value",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_maybe_or_is_teaching_error,
    "v0_3_m6_int_literal_number_arg_maybe_or.ynz",
    "maybe-or default",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_array_contains_is_teaching_error,
    "v0_3_m6_int_literal_number_arg_array_contains.ynz",
    "array-contains target (pre-fix SILENT-WRONG class)",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_struct_field_is_teaching_error,
    "v0_3_m6_int_literal_number_struct_field.ynz",
    "shape-field construction",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_array_lit_is_teaching_error,
    "v0_3_m6_int_literal_number_array_lit.ynz",
    "array-literal element",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_fixed_lit_is_teaching_error,
    "v0_3_m6_int_literal_number_fixed_lit.ynz",
    "fixed-literal element",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_map_lit_val_is_teaching_error,
    "v0_3_m6_int_literal_number_map_lit_val.ynz",
    "map-literal value",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_index_assign_array_is_teaching_error,
    "v0_3_m6_int_literal_number_index_assign_array.ynz",
    "array index-assign element",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_index_assign_map_is_teaching_error,
    "v0_3_m6_int_literal_number_index_assign_map.ynz",
    "map index-assign value",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_field_assign_is_teaching_error,
    "v0_3_m6_int_literal_number_field_assign.ynz",
    "shape-field assignment",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_return_is_teaching_error,
    "v0_3_m6_int_literal_number_return.ynz",
    "return-from-number-fn",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_match_arm_is_teaching_error,
    "v0_3_m6_int_literal_number_match_arm.ynz",
    "multi-case-if arm pattern",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_map_method_key_is_teaching_error,
    "v0_3_m6_int_literal_number_map_number_key.ynz",
    "map method key (pre-fix SILENT-WRONG class)",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_map_bracket_key_is_teaching_error,
    "v0_3_m6_int_literal_number_map_bracket_key.ynz",
    "map bracket key (pre-fix SILENT-WRONG class)",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_neg_int_lit_number_ufcs_is_teaching_error,
    "v0_3_m6_int_literal_number_arg_ufcs_neg.ynz",
    "UFCS -IntLit param",
    "-5"
);
int_literal_number_slot_test!(
    v03_m6_neg_int_lit_number_generic_is_teaching_error,
    "v0_3_m6_int_literal_number_arg_generic_neg.ynz",
    "generic concrete -IntLit param",
    "-5"
);
int_literal_number_slot_test!(
    v03_m6_neg_int_lit_number_plain_is_teaching_error,
    "v0_3_m6_int_literal_number_arg_plain_neg.ynz",
    "plain-call -IntLit param (unified under the gate)",
    "-5"
);
int_literal_number_slot_test!(
    v03_m6_neg_int_lit_number_array_add_is_teaching_error,
    "v0_3_m6_int_literal_number_array_add_neg.ynz",
    "array-add -IntLit elem (pre-fix SILENT-WRONG class)",
    "-5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_array_remove_is_teaching_error,
    "v0_3_m6_int_literal_number_array_remove.ynz",
    "array-remove elem",
    "5"
);
// Fix-loop round 4: an int-literal `return` from a `-> number errors` function.
// The return type wraps `number` in `ErrorsCapable`, so the shared gate's
// `Type::Number` match was false through the wrapper and this slot fell through
// to the GENERIC mismatch (pre-fix RED: "return produces int, but this function
// must return number errors"). The gate now unwraps `ErrorsCapable` so the
// errors-return routes through the SAME teaching error as a plain `-> number`.
int_literal_number_slot_test!(
    v03_m6_int_lit_number_return_errors_is_teaching_error,
    "v0_3_m6_int_literal_number_return_errors.ynz",
    "return-from-number-errors-fn",
    "5"
);

// ── v0.3-M6 store-site stopgap (human-directed "no duct tape") ────────────────
//
// The two STORE sites Phase 1d left un-gated (Future-Req #9): the local
// `let x: number = 5` binding and the `hidden f: number = 5` declaration-site
// field default. Both ICE'd at codegen pre-fix (raw i64 into the decimal128 pointer
// slot — "Yinz compiler bug" banner). Each fixture's header records that pre-fix RED
// signature; these tests lock the post-fix contract (clean teaching error via the
// SAME shared gate as every slot above). Rejection is now uniform across every
// facet; only the int→number COERCION stays deferred to the stub plan.
int_literal_number_slot_test!(
    v03_m6_int_lit_number_let_store_is_teaching_error,
    "v0_3_m6_int_literal_number_let_store.ynz",
    "let-binding store site (pre-fix ICE class)",
    "5"
);
int_literal_number_slot_test!(
    v03_m6_int_lit_number_hidden_field_default_is_teaching_error,
    "v0_3_m6_int_literal_number_hidden_field_default.ynz",
    "hidden-field-default decl site (pre-fix ICE class)",
    "5"
);

#[test]
fn v03_m6_generic_type_param_hidden_field_compiles_clean() {
    // WHY: over-reach regression guard for the hidden-field-default gate. The
    // gate's decl-site arm (ShapeDecl arm of check_module) runs with an EMPTY
    // type_param_scope, so resolving a type-param-typed hidden field
    // (`hidden buffer: array<T>`) through ast_type_to_type there emitted a
    // spurious `T is not a known type` diagnostic — rejecting VALID generic
    // code (hidden fields require a default, so there was no workaround). The
    // fix guards on the raw AstType and resolves only for a literal `number`
    // annotation. This positive fixture must COMPILE CLEAN, guarding against
    // re-introducing the over-reach.
    let (stdout, stderr, code, timed_out) = ynz_run_with_timeout(
        "v0_3_m6_int_literal_number_generic_hidden_field_ok.ynz",
        RUN_TIMEOUT,
    );
    assert!(
        !timed_out,
        "generic hidden-field fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "a generic shape with a type-param-typed hidden field is VALID code and \
         must compile clean (the gate must not over-reach onto generic params); \
         stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("is not a known type"),
        "the hidden-field gate must never emit `T is not a known type` on a valid \
         generic-param field type; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "generic-hidden-field-ok\n",
        "the fixture must run to completion with its expected output; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m6_int_literal_number_gate_false_positive_sweep_stays_clean() {
    // WHY: the gate must fire ONLY on IntLit/-IntLit into a `number` slot —
    // decimal literals, number idents, and every int-into-int neighbor (call
    // args, collection elems/keys/values, fields, index assigns, returns,
    // multi-case-if arms) must compile and run byte-identically clean.
    let (stdout, stderr, code, timed_out) = ynz_run_with_timeout(
        "v0_3_m6_int_literal_number_false_positive_sweep.ynz",
        RUN_TIMEOUT,
    );
    assert!(
        !timed_out,
        "false-positive sweep must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "the false-positive sweep must compile and run clean; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("int literal"),
        "no gate diagnostic may fire on any int-into-int / decimal-into-number \
         neighbor; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "5.0\ntoll-ok\ntoll-ok\n7\n3\n5\ntrue\n5\n5\n5\nmatch-int-ok\n",
        "sweep output must be byte-exact (silent-wrong guard); stderr:\n{stderr}"
    );
}
