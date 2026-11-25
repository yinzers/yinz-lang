//! C1o (card #503): an Auto-Arc spawn group owns one copy of the shape's heap fields, released by
//! the last reference, so tasks never read the spawner's freed strings.

mod support;

use support::assert_fixture_balanced_and_asan_clean_printing_one_of;

/// The spawner prints `churn` while the tasks run; `churn` can land before, between or after the
/// two task lines, so every legal order is accepted. `closing` is what the spawner prints last,
/// after every task line: `done\n` when it first receives both results, empty when the runtime's
/// shutdown drain (not a wall-clock sleep) is what waits for the tasks.
fn assert_group_balanced(fixture: &str, churn: &str, task_line: &str, closing: &str) {
    let task_line_twice = format!("{task_line}{task_line}");
    let churn_first = format!("{churn}{task_line_twice}{closing}");
    let churn_between = format!("{task_line}{churn}{task_line}{closing}");
    let churn_last = format!("{task_line_twice}{churn}{closing}");
    assert_fixture_balanced_and_asan_clean_printing_one_of(
        fixture,
        &[&churn_first, &churn_between, &churn_last],
    );
}

/// One named test per task arm of the `Auto-Arc group heap field ownership` run unit; each case
/// is a test the runner names.
macro_rules! arc_owned_copies_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a task of an Auto-Arc group reads a heap field the spawner already freed, or the block's copy is leaked or freed twice | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

arc_owned_copies_high_case! {
    // case: tasks of a two-spawn group read `scene.name` after `launch` released it (ASan heap-use-after-free)
    a_string_field_of_an_arc_group_shape_outlives_the_spawner_and_is_freed_once => {
        assert_group_balanced("v0_3_m9_c1o_arc_string_field.ynz", "ABCDEFGH\n", "task sees THREE RIVERS\n", "done\n");
    }
    // case: the CPU-arm closure free with two string fields: the block's copies of both are released once, whichever task drops last
    every_string_field_of_a_cpu_arm_arc_group_shape_is_owned_by_the_block => {
        assert_group_balanced("v0_3_m9_c1o_arc_cpu_arm_two_strings.ynz", "ABCDEFGH\n", "cpu task sees POINT PARK on ALLEGHENY\n", "");
    }
}
