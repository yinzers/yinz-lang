//! v0.3-M9 C1j (card #491): a `background` call that passes a field of a fresh call result
//! (`make().items`) releases the whole temporary once, handing the task only a copy of the field.

mod support;

use support::{
    assert_built_balanced_and_asan_clean_printing_one_of, at_both_optimization_levels, build,
};

/// Every fixture ends `wait sleep(50)` then `print(done)`, while the task it spawned prints
/// `task_output`. Nothing waits for a fire-and-forget task, so the language fixes neither order:
/// the balance and ASan are asserted in both orders.
fn assert_fixture_balanced(fixture: &str, task_output_then_done: &str) {
    let task_output = task_output_then_done
        .strip_suffix("done\n")
        .expect("expected output ends with the spawner's `done` line");
    let done_first = format!("done\n{task_output}");
    at_both_optimization_levels(|no_optimize| {
        let built = build(fixture, no_optimize);
        assert_built_balanced_and_asan_clean_printing_one_of(
            fixture,
            &built,
            &[task_output_then_done, done_first.as_str()],
            no_optimize,
        );
    });
}

// Bug: 2 of 4 strings leaked; the original `fixed` field was never released (High: data
// integrity and release).
#[test]
fn a_fixed_field_of_a_fresh_call_result_given_to_a_forgotten_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_fresh_field_fixed.ynz", "2\nA\ndone\n");
}

// Bug: the original `array` field and two of its strings leaked (High: data integrity and release).
#[test]
fn an_array_field_of_a_fresh_call_result_given_to_a_forgotten_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_fresh_field_array.ynz", "2\nA\ndone\n");
}

// Bug: the original shape field's strings leaked (High: data integrity and release).
#[test]
fn a_shape_field_of_a_fresh_call_result_given_to_a_forgotten_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_fresh_field_shape.ynz", "A\nB\ndone\n");
}
