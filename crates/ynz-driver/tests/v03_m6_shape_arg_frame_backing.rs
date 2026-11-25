//! Aggregate arguments (shape, `number`, `maybe`, union, anonymous struct literal, `fixed<T>`)
//! passed BY POINTER to a SUSPENDING callee: the argument is staged in the PARENT resume fn's
//! stack alloca, the child frame stores a `ptrtoint` of that address, the parent returns
//! Pending, its stack dies, and the child resumes on a dangling pointer. The crossing
//! classifier must frame-back (or promote, or refuse) every such argument. Before the fix the
//! value was silent, run-to-run varying garbage or a stable wrong value (`0.000...`, the wrong
//! union variant), never a crash.
//!
//! The sibling UFCS repro `v0_3_m6_ufcs_explicit_wait.ynz` is asserted by
//! `v03_m6_ufcs_suspension.rs`.

mod support;

use support::{assert_fixture_prints, at_both_optimization_levels, compile};

/// One named test per input class of the `aggregate arg to a suspending callee` unit; each
/// case is a test the runner names.
macro_rules! suspending_call_arg_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: an argument passed by pointer to a suspending callee reads freed parent stack after the wait (garbage or the wrong value), or a valid non-escaping argument is wrongly rejected | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

suspending_call_arg_high_case! {
    // case: `wait crew(ship)` with a shape argument (the pure-Call form) reads freed stack
    v03_m6_shape_arg_pure_call_yields_deterministic_value => {
        assert_fixture_prints("v0_3_m6_shape_arg_pure_call.ynz", false, "7\n", "wait crew(ship) must yield the cargo value 7");
    }
    // case: bare `tally(ship)` (auto-inserted suspension) through a pass-through chain reads freed stack
    v03_m6_shape_arg_transitive_chain_yields_deterministic_value => {
        assert_fixture_prints("v0_3_m6_shape_arg_transitive_chain.ynz", false, "8\n", "tally(ship) must yield crew(ship)+1 through the auto-inserted suspension chain");
    }
    // case: a `number` (decimal128) argument is copied to a resume-fn stack temp and reads back 0.000...
    v03_m6_number_arg_pure_call_yields_deterministic_value => {
        assert_fixture_prints("v0_3_m6_number_arg_pure_call.ynz", false, "2.5\n", "wait grow(x) must yield 2.5, not a dangling stack temp's 0.000...");
    }
    // case: `number` arguments of an auto-parallelized I/O group (third arg-staging loop) read freed stack
    v03_m6_number_arg_parallel_group_yields_deterministic_values => {
        assert_fixture_prints("v0_3_m6_number_arg_parallel_group.ynz", false, "2.5\n4.5\n", "the parallel group must yield 2.5 then 4.5");
    }
    // case: a `number` argument of the I/O member in a fused CPU+I/O group reads freed stack
    v03_m6_number_arg_fused_group_yields_deterministic_values => {
        assert_fixture_prints("v0_3_m6_number_arg_fused_group.ynz", false, "1226\n4.5\n", "the fused group must yield 1226 (CPU) then 4.5 (I/O number arg)");
    }
    // case: a `maybe<int>` envelope in a parent stack alloca reads back pointer garbage instead of 42
    v03_m6_maybe_arg_pure_call_yields_deterministic_value => {
        assert_fixture_prints("v0_3_m6_maybe_arg_pure_call.ynz", false, "42\n", "wait pick(m) must yield 42");
    }
    // case: a union argument's tag reads from freed stack and selects the wrong variant
    v03_m6_union_arg_pure_call_yields_deterministic_value => {
        assert_fixture_prints("v0_3_m6_union_arg_pure_call.ynz", false, "square\n", "wait describe(fig) must yield the variant `square`");
    }
    // case: an anonymous struct literal argument has no `let` name for the classifier and reads freed stack
    v03_m6_anon_struct_arg_pure_call_yields_deterministic_value => {
        assert_fixture_prints("v0_3_m6_anon_struct_arg_pure_call.ynz", false, "7\n", "wait crew({...}) must yield the cargo value 7");
    }
    // case: a `number` literal or expression argument (`tally(2.25)`, `tally(seed + 2.75)`) reads dead stack after the callee's wait, at -O0 and -O2
    v03_m9_c1h_number_literal_and_expression_args_reach_a_suspending_callee => {
        at_both_optimization_levels(|no_optimize| {
            assert_fixture_prints(
                "v0_3_m9_c1h_number_literal_arg_suspending_call.ynz",
                no_optimize,
                "2.50\n5.50\n",
                "a number literal or expression argument must survive the callee's wait",
            );
        });
    }
    // case: non-escaping number/fixed/maybe/union/anon arguments must stay accepted and unchanged (a classifier false positive rejects valid code)
    v03_m6_non_escaping_args_of_every_widened_type_are_not_wrongly_affected => {
        assert_fixture_prints(
            "v0_3_m6_non_escaping_args_false_positive_sweep.ynz",
            false,
            "3.5\n9\n40\nsquare\n6\n",
            "bump(x) == 3.5, headroom(nums) == 9, firstOr(m) == 40, tagOf(f) == `square`, crewOf({...}) == 6",
        );
    }
    // case: a `fixed<T>` local escaping into a suspending callee compiled and shipped a dangling stack pointer instead of being refused
    v03_m6_fixed_arg_to_suspending_callee_is_rejected => {
        let compiled = compile("v0_3_m6_fixed_arg_suspending_call_rejected.ynz");
        assert!(
            !compiled.success,
            "a Fixed<Int> passed to a suspending callee must be a compile error; diagnostics:\n{}",
            compiled.diagnostics
        );
        assert!(
            compiled.diagnostics.contains("cannot yet cross a `wait`"),
            "the UnsupportedCrossingLocalType teaching error must fire; diagnostics:\n{}",
            compiled.diagnostics
        );
    }
}
