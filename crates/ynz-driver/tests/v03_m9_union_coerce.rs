//! v0.3-M9 C1e (#421): every plain value that reaches a UNION-typed slot is wrapped in the tagged
//! `{tag, data}` envelope at ONE coercion point. Tier: High (memory safety / data integrity: the
//! pre-fix lowering wrapped per call and return site only, so a union-typed shape field, a
//! non-shape variant and a `.or` fallback were stored or passed as the bare value — wrong
//! variant, an out-of-bounds read past the 1-byte `Tiny`, or a SIGSEGV on the raw int).
//!
//! RED-first on the base (f790a45): `field`, `let`, `non_shape` and `or` fail there; the
//! parameter and return slots of a SHAPE variant pass there (#410) and stay as the
//! `v03_m9_union_wrap` fixtures through the new point.
//!
//! Count: 41 tests = 8 fixtures x 4 runs + 9 build refusals, past the ~8 a High-tier unit gets
//! without a written reason. The fixtures are one per slot kind (field, let, non-shape variant,
//! `.or`, generic parameter, narrowed binding into another union, nested arm on a shadowing
//! binding, and the `return`-through-a-variable recovery) because the card requires a
//! red fixture per kind and each kind failed on the base through a different path. The 4 runs
//! per fixture are not copies: plain vs ASan, because the pre-fix miscompiles often print
//! plausible output and only the sanitizer sees the out-of-bounds envelope read; -O0 vs -O2,
//! because the optimizer can mask a misread, so a defect that shows at one level only is named.
//!
//! One named test per case: each fixture runs at two optimization levels, plainly (the VALUE)
//! and under AddressSanitizer (the MEMORY ACCESS), kept apart so a failure says which is wrong.
//! Non-shape variants are matched with `else =>`; an `is int =>` arm does not lower (#418).

mod support;

use std::path::Path;
use std::process::Command;

use support::{
    assert_asan_clean_and_prints as assert_asan_clean, assert_fixture_prints as assert_prints,
};

const WHAT: &str = "a plain value reaching a union slot must carry its own variant tag";

macro_rules! coercion_tests {
    ($fixture:expr, $stdout:expr, $optimized:ident, $no_optimize:ident, $asan_optimized:ident, $asan_no_optimize:ident) => {
        #[test]
        fn $optimized() {
            // WHY: the value read back through the slot must be the variant that was stored.
            assert_prints($fixture, false, $stdout, WHAT);
        }

        #[test]
        fn $no_optimize() {
            // WHY: same defect at -O0, where no optimizer can mask the misread.
            assert_prints($fixture, true, $stdout, WHAT);
        }

        #[test]
        fn $asan_optimized() {
            // WHY: a bare value read as a 16-byte envelope is out of bounds; plausible output
            // can hide it, so the sanitizer verdict stands alone.
            assert_asan_clean($fixture, false, $stdout);
        }

        #[test]
        fn $asan_no_optimize() {
            // WHY: same out-of-bounds envelope read at -O0.
            assert_asan_clean($fixture, true, $stdout);
        }
    };
}

// Bug: a plain shape stored in a union-typed shape FIELD (`holder: { inner: tiny }`) was stored as
// the bare shape pointer — wrong variant, ASan stack-buffer-overflow past `Tiny`. Tier: High.
coercion_tests!(
    "v0_3_m9_union_coerce_field.ynz",
    "tiny\nbig hello\nbig hello\nbig world\nend\n",
    v03_m9_union_coerce_field_optimized,
    v03_m9_union_coerce_field_no_optimize,
    v03_m9_union_coerce_field_asan_optimized,
    v03_m9_union_coerce_field_asan_no_optimize
);

// Bug: `let n: Mixed = 5` kept the raw int as the envelope pointer — SIGSEGV on the first tag
// read; shape lets (call result, loop body) must stay right. Tier: High.
coercion_tests!(
    "v0_3_m9_union_coerce_let.ynz",
    "circle\nother\ncircle\ncircle\nother\ncircle\nother\nend\n",
    v03_m9_union_coerce_let_optimized,
    v03_m9_union_coerce_let_no_optimize,
    v03_m9_union_coerce_let_asan_optimized,
    v03_m9_union_coerce_let_asan_no_optimize
);

// Bug: a NON-shape variant (`int`) passed, returned or stored in a field of a union with a shape
// variant was never wrapped (LLVM verifier failure / SIGSEGV). Tier: High.
coercion_tests!(
    "v0_3_m9_union_coerce_non_shape.ynz",
    "other\nother\nother\ncircle\nend\n",
    v03_m9_union_coerce_non_shape_optimized,
    v03_m9_union_coerce_non_shape_no_optimize,
    v03_m9_union_coerce_non_shape_asan_optimized,
    v03_m9_union_coerce_non_shape_asan_no_optimize
);

// Bug: `.or(fallback)` with a plain shape fallback into a `maybe<union>` result was not wrapped
// (printed `none`, ASan overflow). Tier: High.
coercion_tests!(
    "v0_3_m9_union_coerce_or.ynz",
    "big hello\ntiny\nend\n",
    v03_m9_union_coerce_or_optimized,
    v03_m9_union_coerce_or_no_optimize,
    v03_m9_union_coerce_or_asan_optimized,
    v03_m9_union_coerce_or_asan_no_optimize
);

#[test]
fn v03_m9_union_coerce_channel_union_element_is_refused() {
    // Regression guard (no hole found): a channel never carries a union, so no plain value can
    // reach a union-typed channel slot. If channels start accepting union elements, a send must
    // wrap through the one coercion point and this test becomes a run fixture. Tier: High.
    assert_build_refused(
        "v0_3_m9_union_coerce_channel_refused.ynz",
        "`channel<Tiny | Big>` is not supported yet",
    );
}

#[test]
fn v03_m9_union_coerce_number_precision_mismatch_is_refused() {
    // Bug: a `number<10>` into a `number<40>` variant was wrapped as if it were the wide pointer
    // representation, so the callee read the narrow slot as a wide decimal. Tier: High.
    assert_build_refused(
        "v0_3_m9_union_coerce_number_precision_refused.ynz",
        "a union can't convert a `number<10>` into one yet",
    );
}

#[test]
fn v03_m9_union_coerce_suspending_let_wait_is_refused() {
    // Bug: `let m: Mixed = wait count()` bound the raw int as the envelope pointer in the
    // state-machine lowering, which bypasses the coercion point; the `is` arm read the tag
    // through address 0x4. Tier: High.
    assert_build_refused(
        "v0_3_m9_union_coerce_suspending_let_wait_refused.ynz",
        "`wait count()` pauses and resumes before it gives its result",
    );
}

#[test]
fn v03_m9_union_coerce_suspending_let_direct_is_refused() {
    // Bug: the same raw-int envelope through the other state-machine `let` arm, a suspending
    // callee called without `wait`. Tier: High.
    assert_build_refused(
        "v0_3_m9_union_coerce_suspending_let_direct_refused.ynz",
        "`count()` pauses and resumes before it gives its result",
    );
}

// Bug: a plain shape for a type parameter the call binds to a union (`same(thing, big)`) reached
// the callee unwrapped (`none`); the stopgap refusal is replaced by wrapping through the
// substituted parameter type. Tier: High.
coercion_tests!(
    "v0_3_m9_union_generic_param_plain_shape.ynz",
    "big hello\ntiny\nbig hello\nend\n",
    v03_m9_union_coerce_generic_param_optimized,
    v03_m9_union_coerce_generic_param_no_optimize,
    v03_m9_union_coerce_generic_param_asan_optimized,
    v03_m9_union_coerce_generic_param_asan_no_optimize
);

// Bug: a narrowed binding passed into a union slot that lists its variant at another position
// reused the outer envelope and its tag (read as `Small`, printed `small`); a `let` shadowing the
// narrowed name got the outer envelope too (printed `tiny`). Tier: High.
coercion_tests!(
    "v0_3_m9_union_coerce_narrowed_other_union.ynz",
    "big hello\nbig hello\nbig shadow\nend\n",
    v03_m9_union_coerce_narrowed_other_union_optimized,
    v03_m9_union_coerce_narrowed_other_union_no_optimize,
    v03_m9_union_coerce_narrowed_other_union_asan_optimized,
    v03_m9_union_coerce_narrowed_other_union_asan_no_optimize
);

// Bug: a nested `is` arm on a `let` that shadows the narrowed name removed the OUTER binding's
// narrowing when it ended, so the outer `t.label` was rejected as a field read on a union.
// Tier: High.
coercion_tests!(
    "v0_3_m9_union_coerce_nested_shadow_arm.ynz",
    "inner other\nwother\nhello\nbig hello\nhello\nbig hello\nend\n",
    v03_m9_union_coerce_nested_shadow_arm_optimized,
    v03_m9_union_coerce_nested_shadow_arm_no_optimize,
    v03_m9_union_coerce_nested_shadow_arm_asan_optimized,
    v03_m9_union_coerce_nested_shadow_arm_asan_no_optimize
);

/// `fixture_name` must fail `ynz build`, with `refusal` in its diagnostics.
fn assert_build_refused(fixture_name: &str, refusal: &str) {
    assert_source_refused(fixture_name, &read_fixture(fixture_name), refusal);
}

/// `program` must fail `ynz build`, with `refusal` in its diagnostics. For a refusal that only
/// the default (auto-parallel) build gives, so the program cannot sit in the fixture corpus the
/// mode-matrix sweep compiles in every mode.
fn assert_source_refused(label: &str, program: &str, refusal: &str) {
    let diagnostics = refused_build_diagnostics(label, program);
    assert!(
        diagnostics.contains(refusal),
        "{label}: refusal must contain `{refusal}`; got:\n{diagnostics}"
    );
}

/// The diagnostics `ynz build` prints for `program`, which must fail with a teaching diagnostic:
/// a refusal a user program can reach is never codegen's "This is a compiler bug" box or an
/// internal panic (Golden Rule 11).
fn refused_build_diagnostics(label: &str, program: &str) -> String {
    let directory = tempfile::tempdir().expect("create temp directory");
    let source = directory.path().join("program.ynz");
    std::fs::write(&source, program).expect("write program");
    let output = Command::new(env!("CARGO_BIN_EXE_ynz"))
        .arg("build")
        .arg(Path::new(&source))
        .env("CLICOLOR", "0")
        .output()
        .expect("spawn `ynz build`");
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.status.success(), "{label} compiled:\n{diagnostics}");
    assert!(
        !diagnostics.contains("compiler bug"),
        "{label}: a user-reachable refusal must not say `compiler bug`; got:\n{diagnostics}"
    );
    diagnostics
}

/// `program` returns `call` from a function that returns a union and must be refused with the
/// whole teaching text of `SuspendingResultIntoUnionReturn`: what happened, what to write
/// instead (which compiles, see `v0_3_m9_union_coerce_return_via_variable.ynz`) and why.
fn assert_return_refused(label: &str, program: &str, call: &str) {
    let diagnostics = refused_build_diagnostics(label, program);
    for teaching_part in [
        format!("`{call}` pauses and resumes before it gives its result"),
        "can't be returned straight from this function".to_string(),
        format!("`let got = {call}`, then `return got`"),
        "its result arrives without that tag being added".to_string(),
    ] {
        assert!(
            diagnostics.contains(&teaching_part),
            "{label}: diagnostic must contain `{teaching_part}`; got:\n{diagnostics}"
        );
    }
}

#[test]
fn v03_m9_union_coerce_suspending_let_cpu_promoted_is_refused() {
    // Bug: a callee typeck sees as plain but the compiler promotes to a pausing function
    // (two independent `leaf` calls) reached codegen's guard, so the user saw "compiler bug"
    // instead of the teaching refusal. Tier: High.
    assert_source_refused(
        "promoted callee into a union let",
        PROMOTED_CALLEE_PROGRAM,
        "`combine()` pauses and resumes before it gives its result",
    );
}

/// `combine` has no `wait`, so typeck sees an ordinary function; the compiler promotes it to a
/// pausing function (the two `leaf` calls run at the same time) and `entrypoint` holds a
/// `wait`, so `let m: Mixed = combine()` is bound by the state-machine lowering. Inline, not a
/// fixture: under `--no-auto-parallel` nothing is promoted and it compiles, which the
/// every-mode corpus sweep would report as a divergence.
const PROMOTED_CALLEE_PROGRAM: &str = r#"
shape Circle {
  name: string
}
shape Mixed = Circle | int

function show(share m: Mixed) -> string {
  if (m) {
    is Circle => {
      return `circle ${m.name}`
    }
    else => {
      return `int`
    }
  }
}

function leaf(n: int) -> int {
  let t = 0
  let i = 0
  while (i < n) {
    t = t + i
    i = i + 1
  }
  return t
}

function combine() -> int {
  let a = leaf(10)
  let b = leaf(11)
  return a + b
}

function entrypoint() -> nothing {
  wait sleep(1)
  let m: Mixed = combine()
  print(show(m))
}
"#;

#[test]
fn v03_m9_union_coerce_suspending_return_wait_is_refused() {
    // Bug: `return wait count()` from a union-returning pausing function panicked in the
    // state-machine lowering ("no handler", the compiler-bug box). Tier: High.
    assert_return_refused(
        "return wait count()",
        &read_fixture("v0_3_m9_union_coerce_suspending_return_wait_refused.ynz"),
        "wait count()",
    );
}

#[test]
fn v03_m9_union_coerce_suspending_return_direct_is_refused() {
    // Bug: `return count()` stored the raw result in the return slot, then panicked in
    // `to_i64_bits` (`into_pointer_value` on an int). Tier: High.
    assert_return_refused(
        "return count()",
        &read_fixture("v0_3_m9_union_coerce_suspending_return_direct_refused.ynz"),
        "count()",
    );
}

#[test]
fn v03_m9_union_coerce_suspending_return_other_variants_are_refused() {
    // Bug: the same panic for every non-shape variant the callee can give back. Tier: High.
    for (variant, literal) in [("string", "`text`"), ("float", "1.5"), ("boolean", "true")] {
        let program = format!(
            "shape Circle {{\n  name: string\n}}\n\
             shape Mixed = Circle | {variant}\n\n\
             function later() -> {variant} {{\n  wait sleep(1)\n  return {literal}\n}}\n\n\
             function pick() -> Mixed {{\n  wait sleep(1)\n  return later()\n}}\n\n\
             function entrypoint() -> nothing {{\n  let m = pick()\n  print(`done`)\n}}\n"
        );
        assert_return_refused(&format!("return later() as {variant}"), &program, "later()");
    }
}

#[test]
fn v03_m9_union_coerce_suspending_return_cpu_promoted_is_refused() {
    // Bug: a callee typeck sees as plain but the compiler promotes to a pausing function
    // reached the `return` arm's panic instead of the teaching refusal. Tier: High.
    let program = PROMOTED_CALLEE_PROGRAM
        .replace(
            "function entrypoint() -> nothing {\n  wait sleep(1)\n  let m: Mixed = combine()\n  print(show(m))\n}",
            "function pick() -> Mixed {\n  wait sleep(1)\n  return combine()\n}\n\n\
             function entrypoint() -> nothing {\n  let m = pick()\n  print(show(m))\n}",
        );
    assert!(
        program.contains("return combine()"),
        "the promoted-callee program must carry the `return` under test"
    );
    assert_return_refused(
        "promoted callee returned into a union",
        &program,
        "combine()",
    );
}

// Bug: the recovery the refusals name (`let got = count()` / `let got = wait count()`, then
// `return got`) must wrap through the one coercion point, so the caller reads an `int` variant.
// Tier: High.
coercion_tests!(
    "v0_3_m9_union_coerce_return_via_variable.ynz",
    "int\nint\nend\n",
    v03_m9_union_coerce_return_via_variable_optimized,
    v03_m9_union_coerce_return_via_variable_no_optimize,
    v03_m9_union_coerce_return_via_variable_asan_optimized,
    v03_m9_union_coerce_return_via_variable_asan_no_optimize
);

fn read_fixture(fixture_name: &str) -> String {
    std::fs::read_to_string(support::fixture_path(fixture_name)).expect("read fixture")
}
