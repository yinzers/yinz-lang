//! UFCS suspension parity: `wait x.method()` and bare UFCS suspending calls must suspend exactly
//! like their `Call`-form twins. The predicate sites were MethodCall-blind, so a UFCS suspending
//! call lowered synchronously through the `block_on` wrapper, which the runtime's own contract
//! declares unreachable from a resume fn on a runtime thread.

mod support;

use support::{assert_fixture_prints, compile};

/// One named test per input class of the `UFCS suspending call` run unit; each case is a test
/// the runner names.
macro_rules! ufcs_suspension_run_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal, $what_it_guards:literal;)+) => { $(
        // bug: a UFCS suspending call (or a UFCS background spawn receiver) is lowered synchronously or its receiver rides into the task as a dead spawner-frame pointer, so the program panics, blocks or prints garbage | tier: High
        #[test]
        fn $case_name() {
            assert_fixture_prints($fixture, false, $expected_stdout, $what_it_guards);
        }
    )+ };
}

ufcs_suspension_run_high_case! {
    // case: a fn whose only suspension source is `self.crew()` is not marked suspending by the transitive analysis
    v03_m6_ufcs_transitive_suspension_runs_and_exits_zero, "v0_3_m6_ufcs_transitive.ynz", "11\n", "tally() must return crew(ship)+1 through the suspension chain";
    // case: `let c = wait ship.crew()` is not a real suspension point (`is_direct_suspending_call` matches only Call+Ident)
    v03_m6_ufcs_explicit_wait_on_method_call_suspends, "v0_3_m6_ufcs_explicit_wait.ynz", "10\n", "wait ship.crew() must yield the cargo value 10";
    // case: `background ship.haul()` does not spawn `haul(ship)` as an independent task
    v03_m6_ufcs_background_spawned_method_call_runs, "v0_3_m6_ufcs_background.ynz", "Mon\ndone\n", "the spawned haul prints first, the entrypoint after its longer sleep";
    // case: `let h = background barge.haul()` (handle form) rides the receiver as a raw pointer to the dead spawner frame
    v03_m6_ufcs_background_handle_receiver_survives_spawner_frame, "v0_3_m6_ufcs_background_handle.ynz", "Mon\n7\ndone\n", "the spawned task must read both receiver fields intact after the spawner's frame death";
    // case: a give-transferred multi-field UFCS receiver is heap-upgraded only in its first slot
    v03_m6_ufcs_background_give_receiver_multifield_survives_spawner_frame, "v0_3_m6_ufcs_background_multifield.ynz", "Mon\n42\ndone\n", "the spawned task must read both receiver fields (string + int) intact";
}

/// One named test per input class of the `UFCS suspending call refused` typeck unit; each case
/// is a test the runner names.
macro_rules! ufcs_suspension_refusal_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: typeck accepts a UFCS suspending call in a sub-expression, or a narrowed-union `background` receiver, and the program mis-lowers or reads out of bounds | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

ufcs_suspension_refusal_high_case! {
    // case: `fetchBase() + ship.crew()`: the sub-expression guard is MethodCall-blind, so the UFCS leg silently mis-lowers
    v03_m6_ufcs_mixed_subexpr_rejects_both_legs => {
        let compiled = compile("v0_3_m6_ufcs_mixed_subexpr.ynz");
        assert!(
            !compiled.success,
            "suspending calls in sub-expression position must be a compile error; diagnostics:\n{}",
            compiled.diagnostics
        );
        assert!(
            compiled
                .diagnostics
                .contains("`crew` is a suspending call inside a larger expression"),
            "the UFCS leg (`ship.crew()`) must be rejected with the same teaching error as the Call leg; \
             diagnostics:\n{}",
            compiled.diagnostics
        );
    }
    // case: a union narrowed to a shape variant used as a `background` receiver (statement and handle form) reads out of bounds (CWE-125)
    v03_m6_ufcs_background_narrowed_union_receiver_rejected_both_forms => {
        let compiled = compile("v0_3_m6_ufcs_background_narrowed_union.ynz");
        assert!(
            !compiled.success,
            "a narrowed-union background receiver must be a compile error; diagnostics:\n{}",
            compiled.diagnostics
        );
        // One rejection per spawn site: the statement form (`background fig.haul()`) and the
        // handle form (`let h = background shp.mend()`), no double emission.
        assert_eq!(
            compiled
                .diagnostics
                .matches("cannot yet be used as a `background` receiver")
                .count(),
            2,
            "exactly one rejection per spawn site (2 spawns); diagnostics:\n{}",
            compiled.diagnostics
        );
    }
}
