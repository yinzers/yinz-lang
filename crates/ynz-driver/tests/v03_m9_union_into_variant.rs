//! v0.3-M9 C1b (security): a union value must not flow into a slot that expects ONE of its
//! variants. Tier: High (type checker; the pre-fix flow read a 16-byte union envelope as the
//! variant — SIGSEGV with an address the tag/data words control).
//!
//! The fixtures go through the public compile boundary (`ynz build`). On the fix commit's parent
//! the `arg` and `return` fixtures compile (`arg` then segfaults when run); the annotated `let`
//! and the field are accepted by the checker there too (the binding is typed as the union), and
//! the build only stops later on an unrelated `values do not have fields` error.
//!
//! Test count: 5, against the "about 8" High-tier budget — one per slot class the one
//! directional check serves (argument, return, `let` + field together) plus the two legal flows
//! it must keep (widening, narrowed arm).

use std::path::Path;
use std::process::Command;

mod support;

struct Compiled {
    success: bool,
    diagnostics: String,
}

fn compile(fixture_name: &str) -> Compiled {
    let directory = tempfile::tempdir().expect("create temp directory");
    let source = directory.path().join("program.ynz");
    std::fs::copy(support::fixture_path(fixture_name), &source).expect("copy fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_ynz"))
        .arg("build")
        .arg(Path::new(&source))
        .env("CLICOLOR", "0")
        .output()
        .expect("spawn `ynz build`");
    Compiled {
        success: output.status.success(),
        diagnostics: format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

/// The compile must fail and say all three parts of the lesson: WHAT (the value, its union
/// type, the slot), WHAT-INSTEAD (an `is` arm), WHY (the tagged box).
fn assert_rejected_with_teaching(fixture_name: &str, what_fragments: &[&str]) {
    let compiled = compile(fixture_name);
    assert!(
        !compiled.success,
        "{fixture_name}: a union reached a variant slot and compiled:\n{}",
        compiled.diagnostics
    );
    for fragment in what_fragments {
        assert!(
            compiled.diagnostics.contains(fragment),
            "{fixture_name}: diagnostic must contain {fragment:?}; got:\n{}",
            compiled.diagnostics
        );
    }
    assert!(
        compiled.diagnostics.contains("is Big => {"),
        "{fixture_name}: WHAT-INSTEAD must suggest narrowing with an `is Big` arm; got:\n{}",
        compiled.diagnostics
    );
    assert!(
        compiled.diagnostics.contains("is a box with a tag"),
        "{fixture_name}: WHY must explain the tagged box; got:\n{}",
        compiled.diagnostics
    );
}

#[test]
fn v03_m9_union_into_variant_argument_is_rejected() {
    // WHY: `showBig(thing)` with `thing: Thing` for a `share b: Big` parameter compiled and the
    // callee read the union envelope as a `Big` (SIGSEGV, attacker-steerable read address).
    assert_rejected_with_teaching(
        "v0_3_m9_union_into_variant_arg.ynz",
        &[
            "`thing` is a `Tiny | Big`",
            "`showBig` expects a `Big` here",
        ],
    );
}

#[test]
fn v03_m9_union_into_variant_return_is_rejected() {
    // WHY: `return t` (a union) from `-> Big` compiled; the caller read the returned envelope
    // as a `Big`.
    assert_rejected_with_teaching(
        "v0_3_m9_union_into_variant_return.ynz",
        &["`t` is a `Tiny | Big`", "this function must return a `Big`"],
    );
}

#[test]
fn v03_m9_union_into_variant_let_and_field_are_rejected() {
    // WHY: the same symmetric check served the annotated `let` and the shape-literal field, so
    // `let again: Big = thing` and `{ inner: thing }` compiled with the same envelope misread.
    assert_rejected_with_teaching(
        "v0_3_m9_union_into_variant_other_slots.ynz",
        &[
            "`again` is declared as `Big`",
            "`Holder.inner` is declared as a `Big`",
        ],
    );
}

#[test]
fn v03_m9_union_widening_and_narrowed_arm_still_compile_and_run() {
    // WHY: guards the fix against over-rejecting — a plain value into `T | nothing`, a variant
    // into a union `let`, and the narrowed `thing` passed as a `Big` inside `is Big` must all
    // keep working (a directional check that lost widening would reject correct programs).
    support::assert_fixture_prints(
        "v0_3_m9_union_widening_and_narrowed.ynz",
        false,
        "hello\nhello\nok\nend\n",
        "legal union flows must still compile and run",
    );
}

#[test]
fn v03_m9_union_widening_and_narrowed_arm_run_without_optimization() {
    // WHY: same legal flows at -O0, where no optimizer can hide a wrong envelope/payload read.
    support::assert_fixture_prints(
        "v0_3_m9_union_widening_and_narrowed.ynz",
        true,
        "hello\nhello\nok\nend\n",
        "legal union flows must still compile and run",
    );
}
