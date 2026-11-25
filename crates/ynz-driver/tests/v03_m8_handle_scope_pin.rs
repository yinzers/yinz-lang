//! A task handle leaving its scope STOPS its task. The fixture binds `h = background worker()`
//! inside an `if` block of a suspending `entrypoint`; the block exits while the child is still in
//! its first sleep, and the release at the block end aborts the child at its next suspension. The
//! child's last line (`child: done`) is never printed; the parent's three lines are.

mod support;

const FIXTURE: &str = "v0_3_m8_p7_handle_scope_exit_pin.ynz";

/// The line the child prints last. A task that was stopped never reaches it.
const CHILD_COMPLETION_LINE: &str = "child: done";

/// Lines the parent prints, in the order it prints them (the parent is a single task).
const PARENT_LINES: [&str; 3] = [
    "parent: leaving block",
    "parent: block exited",
    "parent: done",
];

// bug: a task handle leaving its block does not stop its child, which keeps running and prints after the scope exit | tier: High
#[test]
fn handle_leaving_its_block_stops_the_child() {
    support::at_both_optimization_levels(|no_optimize| {
        let run = support::run_plain(FIXTURE, no_optimize);
        assert_eq!(
            run.exit_code, 0,
            "{FIXTURE} (no_optimize={no_optimize}): must exit 0; stderr:\n{}",
            run.stderr
        );
        assert!(
            !run.stdout.lines().any(|line| line == CHILD_COMPLETION_LINE),
            "{FIXTURE} (no_optimize={no_optimize}): the child printed `{CHILD_COMPLETION_LINE}` \
             after its handle left scope, so the task was not stopped; stdout:\n{}",
            run.stdout
        );
        let parent_lines: Vec<&str> = run
            .stdout
            .lines()
            .filter(|line| line.starts_with("parent: "))
            .collect();
        assert_eq!(
            parent_lines, PARENT_LINES,
            "{FIXTURE} (no_optimize={no_optimize}): stopping the child must not disturb the \
             parent; stderr:\n{}",
            run.stderr
        );
    });
}
