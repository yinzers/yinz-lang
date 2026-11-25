//! Crossing-local VALUE integrity across real back-edge yield/resume cycles. The fixture's
//! state-machine loop runs 2,400,000 iterations, past two full 2^20 poll budgets, so the back
//! edge takes the real store_resume_point -> Pending -> wake -> reload path at least twice. One
//! crossing local per frame value class (int accumulator, int counter, number, string) is live
//! across the back edge and printed after the loop; the closed-form stdout is byte-exact
//! wherever the yields fire, so a wrong byte is a flush/reload miscompile, never scheduling noise.
//!
//! The IR lock guards against a vacuous pass: if admission declines the fixture's function
//! (`back_edge_yield_admission` in ynz-typeck), the loop keeps plain back edges, never yields and
//! still prints correct values. Re-shape the fixture until it is admitted; do not delete the lock.

mod support;

use support::{at_both_optimization_levels, build, run_counted};

/// The closed-form contract: sum 0..2399999 = 2399999 * 2400000 / 2; the number is 0.25 + 0.5
/// mutated between the yields.
const EXPECTED_STDOUT: &str = "crossing-intact\n2879998800000\n2400000\n0.75\nmain done\n";

// bug: a crossing local's value is corrupted by a back-edge yield/resume (frame flush/reload miscompile) | tier: High
#[test]
fn sm_backedge_yield_resume_preserves_crossing_local_values() {
    at_both_optimization_levels(|no_optimize| {
        let built = build("v0_3_m7_p6_backedge_value_integrity.ynz", no_optimize);
        if !no_optimize {
            let ir = std::fs::read_to_string(&built.ir).expect("the emitted .ll is readable");
            assert!(
                ir.contains("sm_backedge_yield"),
                "the IR has no `sm_backedge_yield` block: the fixture's function is no longer \
                 admitted to the back-edge poll-yield transform, so the run below would pass \
                 vacuously"
            );
            assert!(
                ir.lines()
                    .any(|line| line.contains("call")
                        && line.contains("@ynz_rt_check_preempt(ptr %")),
                "the IR has no non-null-waker `ynz_rt_check_preempt` call: the loop back edge \
                 cannot yield, so the run below would pass vacuously"
            );
        }
        let (run, _counters) = run_counted(&built);
        assert_eq!(
            run.exit_code, 0,
            "(no_optimize={no_optimize}) crashed or exited non-zero; stderr:\n{}",
            run.stderr
        );
        assert_eq!(
            run.stdout, EXPECTED_STDOUT,
            "(no_optimize={no_optimize}) a crossing local's value did not survive the yield"
        );
    });
}
