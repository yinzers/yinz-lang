//! v0.3-M9 C1e (#421): every plain value that reaches a UNION-typed slot is wrapped in the tagged
//! `{tag, data}` envelope at ONE coercion point (memory safety / data integrity: the pre-fix
//! lowering wrapped per call and return site only, so a union-typed shape field, a non-shape
//! variant and a `.or` fallback were stored or passed as the bare value: wrong variant, an
//! out-of-bounds read past the 1-byte `Tiny`, or a SIGSEGV on the raw int).
//!
//! RED-first on the base (f790a45): `field`, `let`, `non_shape` and `or` fail there; the
//! parameter and return slots of a SHAPE variant pass there (#410) and stay as the
//! `v03_m9_union_wrap` fixtures through the new point.
//!
//! The run table has one fixture per slot kind (field, let, non-shape variant, `.or`, generic
//! parameter, narrowed binding into another union, nested arm on a shadowing binding, and the
//! `return`-through-a-variable recovery): each kind failed on the base through a different path.
//! Each case runs the program as shipped (the VALUE) and under AddressSanitizer (the MEMORY
//! ACCESS) at -O0 and -O2: the pre-fix miscompiles often print plausible output and only the
//! sanitizer sees the out-of-bounds envelope read, and the optimizer can mask a misread.
//!
//! The refusal table has one case per refused path, each a distinct compile-time path that must
//! teach instead of panicking: channel element, number precision, a suspending call's result into
//! a union `let` (`wait`, direct, CPU-promoted) and into a union `return` (`wait`, direct,
//! CPU-promoted). Each compiles only, so none costs a run. Non-shape variants are matched with
//! `else =>`; an `is int =>` arm does not lower (#418).

mod support;

use std::path::Path;
use std::process::Command;

use support::{assert_prints_and_asan_clean_at_both_levels, assert_refused_with};

/// The program as shipped and under AddressSanitizer, at both levels: a plain value reaching a
/// union slot carries its own variant tag.
fn assert_union_slot_reads_back(fixture: &str, expected_stdout: &str) {
    assert_prints_and_asan_clean_at_both_levels(
        fixture,
        expected_stdout,
        "a plain value reaching a union slot must carry its own variant tag",
    );
}

/// One named test per union slot kind of the `plain value into a union slot (feature run,
/// integrity)` run unit.
macro_rules! union_coerce_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal;)+) => { $(
        // bug: a plain value reaching a union-typed slot is stored or passed unwrapped, so it reads back as the wrong variant, out of bounds past the envelope or as a SIGSEGV | tier: High
        #[test]
        fn $case_name() {
            assert_union_slot_reads_back($fixture, $expected_stdout);
        }
    )+ };
}

union_coerce_high_case! {
    // case: a plain shape stored in a union-typed shape FIELD (`holder: { inner: tiny }`) was stored as the bare shape pointer: wrong variant, ASan stack-buffer-overflow past `Tiny`
    a_plain_shape_stored_in_a_union_shape_field_is_wrapped, "v0_3_m9_union_coerce_field.ynz", "tiny\nbig hello\nbig hello\nbig world\nend\n";
    // case: `let n: Mixed = 5` kept the raw int as the envelope pointer (SIGSEGV on the first tag read); shape lets (call result, loop body) must stay right
    a_plain_value_bound_to_a_union_let_is_wrapped, "v0_3_m9_union_coerce_let.ynz", "circle\nother\ncircle\ncircle\nother\ncircle\nother\nend\n";
    // case: a NON-shape variant (`int`) passed, returned or stored in a field of a union with a shape variant was never wrapped (LLVM verifier failure / SIGSEGV)
    a_non_shape_variant_passed_returned_or_stored_is_wrapped, "v0_3_m9_union_coerce_non_shape.ynz", "other\nother\nother\ncircle\nend\n";
    // case: `.or(fallback)` with a plain shape fallback into a `maybe<union>` result was not wrapped (printed `none`, ASan overflow)
    an_or_fallback_into_a_maybe_union_is_wrapped, "v0_3_m9_union_coerce_or.ynz", "big hello\ntiny\nend\n";
    // case: a plain shape for a type parameter the call binds to a union (`same(thing, big)`) reached the callee unwrapped (`none`); the stopgap refusal is replaced by wrapping through the substituted parameter type
    a_plain_shape_for_a_type_parameter_bound_to_a_union_is_wrapped, "v0_3_m9_union_generic_param_plain_shape.ynz", "big hello\ntiny\nbig hello\nend\n";
    // case: a narrowed binding passed into a union slot that lists its variant at another position reused the outer envelope and its tag (read as `Small`, printed `small`); a `let` shadowing the narrowed name got the outer envelope too (printed `tiny`)
    a_narrowed_binding_passed_into_another_union_gets_that_unions_tag, "v0_3_m9_union_coerce_narrowed_other_union.ynz", "big hello\nbig hello\nbig shadow\nend\n";
    // case: a nested `is` arm on a `let` that shadows the narrowed name removed the OUTER binding's narrowing when it ended, so the outer `t.label` was rejected as a field read on a union
    a_nested_arm_on_a_shadowing_let_keeps_the_outer_narrowing, "v0_3_m9_union_coerce_nested_shadow_arm.ynz", "inner other\nwother\nhello\nbig hello\nhello\nbig hello\nend\n";
    // case: the recovery the refusals name (`let got = count()` / `let got = wait count()`, then `return got`) must wrap through the one coercion point, so the caller reads an `int` variant
    a_suspending_result_returned_through_a_variable_is_wrapped, "v0_3_m9_union_coerce_return_via_variable.ynz", "int\nint\nend\n";
}

/// `combine` has no `wait`, so typeck sees an ordinary function; the compiler promotes it to a
/// pausing function (the two `leaf` calls run at the same time) and `entrypoint` holds a
/// `wait`, so `let m: Mixed = combine()` is bound by the state-machine lowering. Inline, not a
/// fixture: under `--no-auto-parallel` nothing is promoted and it compiles, which the
/// every-mode corpus sweep would report as a divergence.
const PROMOTED_CALLEE_PROGRAM: &str = r#"
shape Circle {
  name: String
}
shape Mixed = Circle | Int

function show(share m: Mixed) -> String {
  if (m) {
    is Circle => {
      return `circle ${m.name}`
    }
    else => {
      return `int`
    }
  }
}

function leaf(n: Int) -> Int {
  let t = 0
  let i = 0
  while (i < n) {
    t = t + i
    i = i + 1
  }
  return t
}

function combine() -> Int {
  let a = leaf(10)
  let b = leaf(11)
  return a + b
}

function entrypoint() -> Nothing {
  wait sleep(1)
  let m: Mixed = combine()
  print(show(m))
}
"#;

/// `program` must fail `ynz build`, with `refusal` in its diagnostics and never codegen's "This is
/// a compiler bug" box (Golden Rule 3). For a refusal that only the default (auto-parallel) build
/// gives, so the program cannot sit in the fixture corpus the mode-matrix sweep compiles in every
/// mode.
fn assert_source_refused(label: &str, program: &str, refusal: &str) {
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
    assert!(
        diagnostics.contains(refusal),
        "{label}: refusal must contain `{refusal}`; got:\n{diagnostics}"
    );
}

/// One named test per refused path of the `plain value into a union slot refusal` typeck unit.
macro_rules! union_coerce_refusal_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: typeck accepts a value that cannot be wrapped into a union slot (a channel element, a narrower number, a suspending call's result into a union let or return), so codegen panics or binds the raw result as the envelope | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

union_coerce_refusal_high_case! {
    // case: regression guard (no hole found): a channel never carries a union, so no plain value can reach a union-typed channel slot. If channels start accepting union elements, a send must wrap through the one coercion point and this becomes a run fixture
    a_union_channel_element_is_refused => {
        assert_refused_with(
            "v0_3_m9_union_coerce_channel_refused.ynz",
            &["`Channel<Tiny | Big>` is not supported yet"],
        );
    }
    // case: a `number<10>` into a `number<40>` variant was wrapped as if it were the wide pointer representation, so the callee read the narrow slot as a wide decimal
    a_number_of_another_precision_into_a_union_variant_is_refused => {
        assert_refused_with(
            "v0_3_m9_union_coerce_number_precision_refused.ynz",
            &["a union can't convert a `Number<10>` into one yet"],
        );
    }
    // case: `let m: Mixed = wait count()` bound the raw int as the envelope pointer in the state-machine lowering, which bypasses the coercion point; the `is` arm read the tag through address 0x4
    a_waited_suspending_result_bound_to_a_union_let_is_refused => {
        assert_refused_with(
            "v0_3_m9_union_coerce_suspending_let_wait_refused.ynz",
            &["`wait count()` pauses and resumes before it gives its value"],
        );
    }
    // case: the same raw-int envelope through the other state-machine `let` arm, a suspending callee called without `wait`
    a_direct_suspending_result_bound_to_a_union_let_is_refused => {
        assert_refused_with(
            "v0_3_m9_union_coerce_suspending_let_direct_refused.ynz",
            &["`count()` pauses and resumes before it gives its value"],
        );
    }
    // case: a callee typeck sees as plain but the compiler promotes to a pausing function (two independent `leaf` calls) reached codegen's guard, so the user saw "compiler bug" instead of the teaching refusal
    a_cpu_promoted_result_bound_to_a_union_let_is_refused => {
        assert_source_refused(
            "promoted callee into a union let",
            PROMOTED_CALLEE_PROGRAM,
            "`combine()` pauses and resumes before it gives its value",
        );
    }
    // case: `return wait count()` from a union-returning pausing function panicked in the state-machine lowering ("no handler", the compiler-bug box)
    a_waited_suspending_result_returned_into_a_union_is_refused => {
        assert_refused_with(
            "v0_3_m9_union_coerce_suspending_return_wait_refused.ynz",
            &["can't be returned straight from this function"],
        );
    }
    // case: `return count()` stored the raw result in the return slot, then panicked in `to_i64_bits` (`into_pointer_value` on an int); a `string`, `float` or `boolean` callee gave the same panic and takes the same refusal
    a_direct_suspending_result_returned_into_a_union_is_refused => {
        assert_refused_with(
            "v0_3_m9_union_coerce_suspending_return_direct_refused.ynz",
            &["can't be returned straight from this function"],
        );
    }
    // case: a callee typeck sees as plain but the compiler promotes to a pausing function reached the `return` arm's panic instead of the teaching refusal
    a_cpu_promoted_result_returned_into_a_union_is_refused => {
        let program = PROMOTED_CALLEE_PROGRAM.replace(
            "function entrypoint() -> Nothing {\n  wait sleep(1)\n  let m: Mixed = combine()\n  print(show(m))\n}",
            "function pick() -> Mixed {\n  wait sleep(1)\n  return combine()\n}\n\n\
             function entrypoint() -> Nothing {\n  let m = pick()\n  print(show(m))\n}",
        );
        assert!(
            program.contains("return combine()"),
            "the promoted-callee program must carry the `return` under test"
        );
        assert_source_refused(
            "promoted callee returned into a union",
            &program,
            "can't be returned straight from this function",
        );
    }
}
