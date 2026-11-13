//! v0.3-M9 C1d: the union exhaustiveness check counts NON-shape variants. Tier: High (a state
//! machine over union variants: a missed variant is a silent wrong behavior, Golden Rule 5).
//!
//! Pre-fix, `check_stmt_match` skipped every variant that is not a shape, so a match on
//! `Circle | int` with only `is Circle` and no `else` compiled (the fixture's first test failed
//! then: the build succeeded).
//!
//! 2 tests, one per contract edge: the rejection and its teaching text, and the accepted forms
//! (an `else`, an arm per shape variant, and the `nothing` exemption).

mod support;

use std::process::Command;

fn build(fixture_name: &str) -> (bool, String) {
    let directory = tempfile::tempdir().expect("create temp directory");
    let source = directory.path().join("program.ynz");
    std::fs::copy(support::fixture_path(fixture_name), &source).expect("copy fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_ynz"))
        .arg("build")
        .arg(&source)
        .env("CLICOLOR", "0")
        .output()
        .expect("spawn `ynz build`");
    (
        output.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

#[test]
fn v03_m9_union_match_missing_a_non_shape_variant_is_rejected() {
    // WHY: `Circle | int` matched with only `is Circle` and no `else` compiled, so an `int`
    // value silently fell through every arm.
    let (success, diagnostics) = build("v0_3_m9_union_nonexhaustive_non_shape.ynz");
    assert!(!success, "non-exhaustive match compiled:\n{diagnostics}");
    assert!(
        diagnostics.contains("Non-exhaustive union multi-case — 1 variant is not handled: int."),
        "WHAT must name the missing non-shape variant; got:\n{diagnostics}"
    );
    assert!(
        diagnostics.contains("Add an `else =>` default arm to handle int."),
        "WHAT-INSTEAD must suggest `else =>`; got:\n{diagnostics}"
    );
    // The card requires the pre-existing text unchanged apart from the added names: the WHAT
    // sentence is asserted above, the WHY sentence here.
    assert!(
        diagnostics.contains(
            "The compiler knows every union variant at compile time. A missing arm means some \
             values silently fall through — likely a bug."
        ),
        "WHY must keep the pre-existing exhaustiveness text; got:\n{diagnostics}"
    );
    assert!(
        !diagnostics.contains("`is int =>`"),
        "WHAT-INSTEAD must not suggest an `is int` arm — it does not compile; got:\n{diagnostics}"
    );
}

#[test]
fn v03_m9_union_match_accepted_forms_still_compile_and_run() {
    // WHY: guards against over-rejecting — `else =>` for a non-shape variant, an arm per
    // shape variant, and `T | nothing` with only the present arm (nothing cannot be named by
    // an arm, and skipping the absent case drops no data) must all keep compiling.
    support::assert_fixture_prints(
        "v0_3_m9_union_exhaustive_forms.ynz",
        false,
        "circle\ncircle\nsquare\ncircle\nend\n",
        "the accepted exhaustive forms must still compile and run",
    );
}
