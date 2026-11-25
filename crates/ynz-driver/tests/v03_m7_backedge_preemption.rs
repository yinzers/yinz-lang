//! Starvation proof for the codegen back-edge poll-yield. With exactly one I/O worker
//! (`YNZ_WORKER_THREADS=1`) a sibling task can run before a hot loop finishes only if the loop
//! yields the worker (state-machine function) or never occupies it (plain function, routed to
//! the blocking pool by CPU admission).

mod support;

use support::run_plain_with_env;

/// One named test per hot-loop shape of the `back-edge preemption` scheduler unit; each case is
/// a test the runner names.
macro_rules! backedge_preemption_high_case {
    ($($case_name:ident, $fixture:literal, $hog_done_line:literal;)+) => { $(
        // bug: a wait-free hot loop keeps the only I/O worker until it finishes, so a sibling task starves | tier: High
        #[test]
        fn $case_name() {
            let run = run_plain_with_env(
                $fixture,
                false,
                &[
                    ("YNZ_WORKER_THREADS", "1"),
                    // Liveness budget, not a performance bound: shutdown abandons a blocking-pool hog
                    // still running after the 5s default grace, which a loaded machine can exceed for a
                    // 100M-iteration loop.
                    ("YNZ_SHUTDOWN_TIMEOUT_MS", "120000"),
                ],
            );
            assert_eq!(run.exit_code, 0, "{} must exit 0; stderr:\n{}", $fixture, run.stderr);
            let victim_position = run
                .stdout
                .find("victim ran")
                .unwrap_or_else(|| panic!("stdout must contain `victim ran`; got:\n{}", run.stdout));
            let hog_position = run
                .stdout
                .find($hog_done_line)
                .unwrap_or_else(|| panic!("stdout must contain `{}`; got:\n{}", $hog_done_line, run.stdout));
            assert!(
                victim_position < hog_position,
                "`victim ran` must appear before `{}`; output:\n{}",
                $hog_done_line,
                run.stdout
            );
        }
    )+ };
}

backedge_preemption_high_case! {
    // case: a wait-free hot loop inside a state-machine function never yields the worker (back-edge poll-yield is a no-op)
    sm_hot_loop_yields_worker_to_sibling_task, "v0_3_m7_p6_backedge_starvation_sm.ynz", "hog done";
    // case: a plain function's hot loop occupies the I/O worker instead of the blocking pool
    non_sm_hot_loop_relies_on_blocking_pool_routing_not_preemption, "v0_3_m7_p6_backedge_residual_nonsm.ynz", "plain hog done";
}
