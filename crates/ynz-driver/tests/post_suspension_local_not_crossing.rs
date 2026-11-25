// A local bound AFTER a suspension, and read only where no further suspension separates the
// binding from the read, is NOT a crossing local: these programs are correct Yinz and must
// compile and run.
//
// The mirror image of `frago002_c1_c2_planned_red.rs`. That file locks the direction where the
// crossing analysis reported TOO LITTLE (a real crossing missed: no frame slot, silent wrong
// output). This file locks the direction where it reported TOO MUCH (a non-crossing read
// reported: an `UnsupportedCrossingLocalType` compile error on a valid program). One producer,
// `collect_crossings_in_stmts` (crates/ynz-typeck/src/check.rs), answers both, so both directions
// need a pin, or the next fix to that function trades one failure for the other.
//
// The false-rejection producer: `declared` accumulated every local bound since the most recent
// suspension, and nothing asked whether a suspension actually fell between a local's binding and
// the read being scanned. A suspending statement's own operands run BEFORE that statement
// suspends, so a local bound since the last suspension and read only there crosses nothing. The
// fix stages post-suspension bindings in `declared_since_suspension` and flushes them into
// `declared` at the next suspension.
//
// Each case runs at both optimisation levels because the two halves fail differently: a
// rejection is identical at both, a layout defect is not.

mod support;

use support::{assert_fixture_prints, at_both_optimization_levels};

/// One named test per input class of the `post-suspension local is not crossing` unit; each case
/// is a test the runner names.
macro_rules! post_suspension_local_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal;)+) => { $(
        // bug: typeck rejects a valid program by reporting a local bound after a suspension as a crossing local, or accepts it and lays its frame out wrong | tier: High
        #[test]
        fn $case_name() {
            at_both_optimization_levels(|no_optimize| {
                assert_fixture_prints(
                    $fixture,
                    no_optimize,
                    $expected_stdout,
                    "a correct program must compile and print the right value",
                );
            });
        }
    )+ };
}

post_suspension_local_high_case! {
    // case: a `maybe<int>` bound after the leading `wait`, read only in a suspending call's argument list (the reported regression)
    maybe_bound_after_wait_read_by_a_suspending_call_is_not_crossing, "v0_3_hardening_c1_post_wait_maybe_read_by_suspending_call.ynz", "42\n";
    // case: the same shape on a second unsupported-crossing type (`fixed<T>`), so one type's guard being loosened cannot make it green
    fixed_bound_after_wait_read_by_a_suspending_call_is_not_crossing, "v0_3_hardening_c1_post_wait_fixed_read_by_suspending_call.ynz", "3\n";
    // case: the pre-existing instance: an ordinary non-suspending read with no later suspension at all (proves the fix is at the producer)
    maybe_bound_after_wait_with_no_later_suspension_is_not_crossing, "v0_3_hardening_c1_post_wait_maybe_read_no_later_suspension.ynz", "42\n";
}
