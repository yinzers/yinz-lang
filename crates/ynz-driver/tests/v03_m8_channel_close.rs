//! Channel close semantics, the transfer rule, `channel<number>` marshalling and the runtime-side
//! `refuse_closed` release, locked end-to-end by `.ynz` fixtures. Designs: `IMP-concurrency.md`
//! "Channel Close — End-of-Stream Semantics" and `IMP-ownership.md` "Transfer — Who Else Holds
//! This Value". A red here is a hang, a leak, a double free or a use-after-free.

mod support;

use support::{assert_fixture_prints, build, compile, run_counted, run_plain};

/// Exit 0, byte-exact stdout, and `alloc == free` with a non-vacuous counter: every heap value
/// is released by its owner exactly once (a smaller free count is a double free, a larger alloc
/// count a leak).
fn assert_runs_balanced(fixture: &str, expected_stdout: &str) {
    let built = build(fixture, false);
    let (run, counters) = run_counted(&built);
    assert_eq!(
        run.exit_code, 0,
        "{fixture}: must exit 0; stderr:\n{}",
        run.stderr
    );
    assert_eq!(
        run.stdout, expected_stdout,
        "{fixture}: exact expected stdout; stderr:\n{}",
        run.stderr
    );
    assert!(
        counters.alloc > 0,
        "{fixture}: alloc=0, the counter saw nothing, parity is vacuous"
    );
    assert_eq!(
        counters.alloc, counters.free,
        "{fixture}: alloc must equal free (smaller free = a double free, larger alloc = a leak)"
    );
}

/// One named test per input class of the `channel close and transfer run` unit; each case is a
/// test the runner names.
macro_rules! channel_run_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a channel's close or transfer leaves a receiver parked, drops or double delivers a value, frees a payload twice or leaks it | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

channel_run_high_case! {
    // case: `.copy()` on a map aliases the original (the independence lock was RED on the alias no-op)
    m8_p4_map_copy_is_independent_of_the_original => {
        assert_fixture_prints(
            "v0_3_m8_p4_map_copy_independent.ynz",
            false,
            "original b: 99\ncopy b: 20\noriginal count: 3\ncopy count: 2\n",
            "a copy of a map must not see writes to the original",
        );
    }
    // case: after `close()` the receiver drains the buffered values, then sees `none`
    m8_p4_close_then_receive_drains_then_none => {
        assert_fixture_prints("v0_3_m8_p4_close_drain_then_none.ynz", false, "total: 10\nend\n", "close must drain then end");
    }
    // case: a second `close()` is a no-op
    m8_p4_double_close_is_a_safe_no_op => {
        assert_fixture_prints("v0_3_m8_p4_close_double_idempotent.ynz", false, "1\nclosed twice ok\n", "a double close must be safe");
    }
    // case: a send after close is the typed runtime error, not a delivery
    m8_p4_send_after_close_is_the_typed_runtime_error => {
        assert_fixture_prints("v0_3_m8_p4_send_after_close_refused.ynz", false, "refused\nend\n", "a send after close must be refused");
    }
    // case: delivered == received under every interleaving of a send racing `close()`
    m8_p4_concurrent_send_during_close_is_linearized_at_the_sender_lock => {
        assert_fixture_prints("v0_3_m8_p4_close_concurrent_send_linearized.ynz", false, "parity ok\nend\n", "delivered must equal received");
    }
    // case: `close()` wakes a receiver parked on an empty channel (the watchdog is the gate for "stayed parked")
    m8_p4_close_wakes_a_parked_receiver => {
        assert_fixture_prints("v0_3_m8_p4_close_wakes_parked_receiver.ynz", false, "woken by close\n", "close must wake the parked receiver");
    }
    // case: a send that happened before `close()` still lands
    m8_p4_in_flight_pre_close_send_lands => {
        assert_fixture_prints("v0_3_m8_p4_close_inflight_send_lands.ynz", false, "1\n2\nend\n", "a pre-close send must be delivered");
    }
    // case: dropping a channel without closing it is unchanged
    m8_p4_drop_without_close_is_unchanged => {
        assert_fixture_prints("v0_3_m8_p4_drop_without_close_unchanged.ynz", false, "10\n", "a channel dropped without close must behave as before");
    }
    // case: `h.receive()` of a plain `maybe` return handed the parent a dead-frame address (exit 139)
    m8_p4_handle_receive_of_a_plain_maybe_return_does_not_segfault => {
        assert_fixture_prints("v0_3_m8_p4_handle_maybe_return.ynz", false, "7\ndone\n", "the parent must read the maybe the task returned");
    }
    // case: the errors-capable twin of the `maybe` return stays correct
    m8_p4_handle_receive_of_a_maybe_errors_return_stays_correct => {
        assert_fixture_prints("v0_3_m8_p4_handle_maybe_errors_return.ynz", false, "7\ndone\n", "the errors-capable maybe return must stay correct");
    }
    // case: a `channel<array<int>>` round trip leaves the receiver the sole holder of the task-built array
    m8_p4_chan_array_roundtrip_receiver_is_sole_holder => {
        assert_runs_balanced("v0_3_m8_p4_chan_array_roundtrip.ynz", "3\n10\n30\nend\n");
    }
    // case: a refused array payload is freed exactly once by `refuse_closed`
    m8_p4_chan_array_send_after_close_frees_the_refused_payload_once => {
        assert_runs_balanced("v0_3_m8_p4_chan_array_send_after_close.ynz", "refused\nend\n");
    }
    // case: an array still buffered at exit is freed with the spawner's channel local
    m8_p4_chan_array_buffered_at_exit_is_held_by_the_channel => {
        assert_runs_balanced("v0_3_m8_p4_chan_array_drop_with_buffered.ynz", "buffered\n");
    }
    // case: `channel<map<string, int>>` round trip, task-built and `give` background argument
    m8_p4_chan_map_roundtrip_both_task_built_and_give_bg_arg => {
        assert_runs_balanced("v0_3_m8_p4_chan_map_roundtrip.ynz", "2\n2\n3\n9\nend\n");
    }
    // case: a refused map payload is freed exactly once (`ynz_map_drop`)
    m8_p4_chan_map_send_after_close_frees_the_refused_payload_once => {
        assert_runs_balanced("v0_3_m8_p4_chan_map_send_after_close.ynz", "refused\nend\n");
    }
    // case: a map still buffered at exit is freed with the spawner's channel local
    m8_p4_chan_map_buffered_at_exit_is_held_by_the_channel => {
        assert_runs_balanced("v0_3_m8_p4_chan_map_drop_with_buffered.ynz", "buffered\n");
    }
    // case: a `give` parameter carries ownership through a call into a send
    m8_p4_flow_give_parameter_carries_ownership_through_a_call => {
        assert_fixture_prints("v0_3_m8_p4_flow_give_through_call.ynz", false, "3\nend\n", "ownership must travel through the call");
    }
    // case: a two-hop relay with `give` on both frames
    m8_p4_flow_two_hop_relay_with_give_on_both_frames => {
        assert_fixture_prints("v0_3_m8_p4_flow_two_hop_give.ynz", false, "4\nend\n", "ownership must travel through both hops");
    }
    // case: every admitted payload form sends and each received array is released once
    m8_p4_flow_every_admitted_payload_form_sends => {
        assert_runs_balanced("v0_3_m8_p4_flow_admitted_forms.ynz", "3\n4\n4\n2\n0\n5\nend\n");
    }
    // case: reassigning a consumed binding revives it (a correct program was rejected as "already given away")
    m8_p4_reassigning_a_consumed_binding_revives_it => {
        assert_fixture_prints("v0_3_m8_p4_revive_on_reassign.ynz", false, "3\n2\n4\n5\n", "a reassigned binding must be usable again");
    }
    // case: a `channel<number>` round trip through a receive-side suspension frees both 16-byte cells
    m8_p4_number_channel_round_trips_through_a_receive_side_suspension => {
        assert_runs_balanced("v0_3_m8_p4_number_chan_roundtrip.ynz", "2.5\n2.5\nend\n");
    }
    // case: a refused `number` payload's cell is freed by `refuse_closed` through `ynz_number_cell_free`
    m8_p4_number_channel_send_after_close_frees_the_minted_cell => {
        assert_runs_balanced("v0_3_m8_p4_number_chan_send_after_close.ynz", "refused\nend\n");
    }
    // case: a buffered `number` cell is freed with the spawner's channel local
    m8_p4_number_channel_buffered_cell_is_held_by_the_channel => {
        assert_runs_balanced("v0_3_m8_p4_number_chan_drop_with_buffered.ynz", "buffered\n");
    }
    // case: a parked `number` send is drained after close, both cells freed at the receives
    m8_p4_number_channel_parked_send_is_drained_after_close => {
        assert_runs_balanced("v0_3_m8_p4_number_chan_parked_send_drained_after_close.ynz", "1.5\n2.5\nend\n");
    }
    // case: a builder with a computed scalar local is fresh (was refused as TransferNeedsCopy: the oracle answered `None` for every `let` local)
    m8_p4_fix2_builder_with_a_computed_scalar_local_is_fresh_and_sends => {
        assert_runs_balanced("v0_3_m8_p4_builder_with_computed_scalar_is_fresh.ynz", "1\n3\nend\n");
    }
    // case: `.message` on a failed send reads the runtime error text (was an ICE: no `.message` codegen arm)
    m8_p4_fix2_errors_value_message_reads_the_runtime_error_text => {
        let run = run_plain("v0_3_m8_p4_errors_message_after_failed.ynz", false);
        assert_eq!(run.exit_code, 0, "must exit 0; stderr:\n{}", run.stderr);
        assert!(
            run.stdout.contains("The channel is closed") && run.stdout.ends_with("\nend\n"),
            "`.message` must print the closed-channel error text, then `end`; stdout:\n{}",
            run.stdout
        );
    }
}

/// Key phrase of the same-call alias refusal: two arguments of one call share one allocation.
const SAME_CALL_ALIAS_PHRASE: &str = "`other` was already given away and cannot be used here";

/// One named test per refused program of the `channel transfer rule refusals` typeck unit; each
/// case is a test the runner names.
macro_rules! channel_transfer_refusal_high_case {
    ($($case_name:ident, $fixture:literal, $phrase:expr;)+) => { $(
        // bug: typeck accepts a program that aliases one allocation across two owners (same-call alias pair) or reads `.message` before `.failed()` | tier: High
        #[test]
        fn $case_name() {
            let compiled = compile($fixture);
            assert!(!compiled.success, "{} must be refused at compile time", $fixture);
            assert!(
                compiled.diagnostics.contains($phrase),
                "{}: diagnostics must contain {:?}; got:\n{}",
                $fixture,
                $phrase,
                compiled.diagnostics
            );
        }
    )+ };
}

channel_transfer_refusal_high_case! {
    // case: `.message` on a value not yet checked with `.failed()` SIGABRT'd (typeck typed it `string` unconditionally)
    m8_p4_fix3_message_before_failed_check_is_refused, "v0_3_m8_p4_fix3_message_before_failed_check.ynz", "hasn't been checked with `.failed()` yet";
    // case: one binding at two `give` positions of one call compiled and printed `3 3`
    m8_p4_fix2_same_call_alias_pair_at_two_give_positions_is_refused, "v0_3_m8_p4_same_call_alias_give_give.ynz", SAME_CALL_ALIAS_PHRASE;
    // case: one binding at a `give` and a `share` position of one call compiled and printed `3 3`
    m8_p4_fix2_same_call_alias_pair_at_give_and_share_positions_is_refused, "v0_3_m8_p4_same_call_alias_give_share.ynz", SAME_CALL_ALIAS_PHRASE;
    // case: an alias pair through a `background` spawn compiled and the ladder got two descriptors for one allocation
    m8_p4_fix2_same_call_alias_pair_through_a_background_spawn_is_refused, "v0_3_m8_p4_same_call_alias_background.ynz", SAME_CALL_ALIAS_PHRASE;
}
