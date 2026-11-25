//! v0.3-M9 C1d: the union exhaustiveness check counts NON-shape variants (a state machine over
//! union variants: a missed variant is a silent wrong behavior, Golden Rule 2).
//!
//! Pre-fix, `check_stmt_match` skipped every variant that is not a shape, so a match on
//! `Circle | int` with only `is Circle` and no `else` compiled.
//!
//! One case per contract edge: the rejection, and the accepted forms (an `else`, an arm per shape
//! variant, and the `nothing` exemption).

mod support;

/// One named test per contract edge of the `union match exhaustiveness (typeck accept/reject)` unit.
macro_rules! union_exhaustiveness_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a match over a union with a non-shape variant compiles with that variant unhandled (or an exhaustive form is refused), so a value silently falls through every arm | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

union_exhaustiveness_high_case! {
    // case: `Circle | int` matched with only `is Circle` and no `else` compiled, so an `int` value silently fell through every arm
    a_match_missing_a_non_shape_variant_is_rejected => {
        support::assert_refused_with(
            "v0_3_m9_union_nonexhaustive_non_shape.ynz",
            &["Non-exhaustive union multi-case — 1 variant is not handled: Int."],
        );
    }
    // case: guards against over-rejecting: `else =>` for a non-shape variant, an arm per shape variant, and `T | nothing` with only the present arm (nothing cannot be named by an arm, and skipping the absent case drops no data) must all keep compiling
    the_accepted_exhaustive_forms_still_compile_and_run => {
        support::assert_fixture_prints(
            "v0_3_m9_union_exhaustive_forms.ynz",
            false,
            "circle\ncircle\nsquare\ncircle\nend\n",
            "the accepted exhaustive forms must still compile and run",
        );
    }
}
