//! v0.3-M9 C1b (security): a union value must not flow into a slot that expects ONE of its
//! variants (type checker; the pre-fix flow read a 16-byte union envelope as the variant:
//! SIGSEGV with an address the tag/data words control).
//!
//! The fixtures go through the public compile boundary (`ynz build`). On the fix commit's parent
//! the `arg` and `return` fixtures compile (`arg` then segfaults when run); the annotated `let`
//! and the field are accepted by the checker there too (the binding is typed as the union), and
//! the build only stops later on an unrelated `values do not have fields` error.
//!
//! One case per slot class and call form the one directional check serves (plain, dot-call and
//! generic argument; return; `let` + field together) plus the legal flows it must keep, run at
//! both optimization levels. A refusal keeps the fragments naming the value and the slot; the
//! advice (an `is Big` arm) and the WHY (a tagged box) are diagnostic wording. A plain shape for a
//! union-bound type parameter is covered by `v03_m9_union_coerce`.

mod support;

use support::assert_refused_with;

/// One named test per slot class of the `union flowing into a variant slot (typeck accept/reject)`
/// unit.
macro_rules! union_into_variant_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: typeck accepts a union value into a slot that expects one of its variants (or refuses a legal flow), so the callee reads the 16-byte envelope as the variant (SIGSEGV, attacker-steerable read address) | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

union_into_variant_high_case! {
    // case: `showBig(thing)` with `thing: Thing` for a `share b: Big` parameter compiled and the callee read the union envelope as a `Big`
    an_argument_into_a_variant_parameter_is_rejected => {
        assert_refused_with(
            "v0_3_m9_union_into_variant_arg.ynz",
            &["`thing` is a `Tiny | Big`", "`showBig` expects a `Big` here"],
        );
    }
    // case: the dot-call form `big.useBig(thing)` is the same argument slot, but its non-receiver arguments had no type check at all, so the union compiled and the callee printed a stack address
    a_dot_call_argument_into_a_variant_parameter_is_rejected => {
        assert_refused_with(
            "v0_3_m9_union_into_variant_ufcs_arg.ynz",
            &["`thing` is a `Tiny | Big`", "`useBig` expects a `Big` here"],
        );
    }
    // case: the generic call form discarded the mismatch: a concrete `Big` parameter, `T` bound to `Big` by a sibling argument, and an explicit `<int>` all compiled and segfaulted (one fixture holds the three call shapes; the diagnostic must name each callee)
    a_generic_call_argument_into_a_variant_parameter_is_rejected => {
        assert_refused_with(
            "v0_3_m9_union_into_variant_generic_arg.ynz",
            &[
                "`pick` expects a `Big` here",
                "`same` expects a `Big` here",
                "`useBig` expects a `Big` here",
            ],
        );
    }
    // case: `return t` (a union) from `-> Big` compiled; the caller read the returned envelope as a `Big`
    a_union_returned_from_a_variant_function_is_rejected => {
        assert_refused_with(
            "v0_3_m9_union_into_variant_return.ynz",
            &["`t` is a `Tiny | Big`", "this function must return a `Big`"],
        );
    }
    // case: the same symmetric check served the annotated `let` and the shape-literal field, so `let again: Big = thing` and `{ inner: thing }` compiled with the same envelope misread
    a_union_bound_to_a_variant_let_or_field_is_rejected => {
        assert_refused_with(
            "v0_3_m9_union_into_variant_other_slots.ynz",
            &["`again` is declared as `Big`", "`Holder.inner` is declared as a `Big`"],
        );
    }
    // case: guards the fix against over-rejecting: a plain value into `T | nothing`, a variant into a union `let`, and the narrowed `thing` passed as a `Big` inside `is Big` must all keep working, at -O0 too where no optimizer can hide a wrong envelope or payload read
    widening_and_a_narrowed_arm_still_compile_and_run => {
        support::at_both_optimization_levels(|no_optimize| {
            support::assert_fixture_prints(
                "v0_3_m9_union_widening_and_narrowed.ynz",
                no_optimize,
                "hello\nhello\nhello\nhello\nhello\ntiny\nok\nok\nend\n",
                "legal union flows must still compile and run",
            );
        });
    }
}
