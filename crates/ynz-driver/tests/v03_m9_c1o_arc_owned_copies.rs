//! v0.3-M9 C1o (card #503): an Auto-Arc spawn group owns one copy of the shape's heap fields,
//! released by the last reference, so tasks never read the spawner's freed strings.

mod support;

use support::{
    assert_built_balanced_and_asan_clean_printing_one_of, at_both_optimization_levels, build,
};

/// The spawner prints `churn` while the tasks run; `churn` can land before, between or after
/// the two task lines, so every legal order is accepted. `closing` is what the spawner prints
/// last, after every task line: `done\n` when it first receives both results, empty when the
/// runtime's shutdown drain (not a wall-clock sleep) is what waits for the tasks.
fn assert_group_balanced(fixture: &str, churn: &str, task_line: &str, closing: &str) {
    let task_line_twice = format!("{task_line}{task_line}");
    let churn_first = format!("{churn}{task_line_twice}{closing}");
    let churn_between = format!("{task_line}{churn}{task_line}{closing}");
    let churn_last = format!("{task_line_twice}{churn}{closing}");
    at_both_optimization_levels(|no_optimize| {
        let built = build(fixture, no_optimize);
        assert_built_balanced_and_asan_clean_printing_one_of(
            fixture,
            &built,
            &[&churn_first, &churn_between, &churn_last],
            no_optimize,
        );
    });
}

// Bug: tasks of a two-spawn group read `scene.name` after `launch` released it (ASan
// heap-use-after-free at O0 and O2) (High: data integrity and release).
#[test]
fn a_string_field_of_an_arc_group_shape_outlives_the_spawner_and_is_freed_once() {
    assert_group_balanced(
        "v0_3_m9_c1o_arc_string_field.ynz",
        "ABCDEFGH\n",
        "task sees THREE RIVERS\n",
        "done\n",
    );
}

// Bug: the same through the CPU-arm closure free with two string fields: the block's copies of
// BOTH must be released exactly once, whichever task drops last (High: data integrity and release).
#[test]
fn every_string_field_of_a_cpu_arm_arc_group_shape_is_owned_by_the_block() {
    assert_group_balanced(
        "v0_3_m9_c1o_arc_cpu_arm_two_strings.ynz",
        "ABCDEFGH\n",
        "cpu task sees POINT PARK on ALLEGHENY\n",
        "",
    );
}
