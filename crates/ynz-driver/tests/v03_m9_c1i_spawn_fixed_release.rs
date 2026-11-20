//! v0.3-M9 C1i (card #472): a `background` call that passes a `fixed<T>` of heap-backed items
//! with no ownership keyword releases every item exactly once, whether the callee never suspends
//! (fire-and-forget, CPU pool) or suspends (handled, scheduler state machine).

mod support;

use support::{
    assert_built_balanced_and_asan_clean_printing_one_of, at_both_optimization_levels, build,
};

/// Every fixture here ends `wait sleep(50)` then `print(done)`, while the task it spawned prints
/// `task_output`. Nothing waits for a fire-and-forget task, and a handled task is not awaited
/// either, so the language fixes neither order: a loaded machine puts `done` first (3 of 50 runs
/// under 16 busy loops). What the bug check needs is the balance and ASan, asserted in both orders.
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

// Bug: 2 of 4 strings leaked (High: data integrity and release).
#[test]
fn a_fixed_of_strings_given_to_a_forgotten_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1i_spawn_fixed_string_forget.ynz", "2\nA\ndone\n");
}

#[test]
fn a_fixed_of_strings_given_to_a_handled_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1i_spawn_fixed_string_handle.ynz", "2\nA\ndone\n");
}

#[test]
fn a_fixed_of_shapes_given_to_a_forgotten_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1i_spawn_fixed_shape_forget.ynz", "2\nA\ndone\n");
}

#[test]
fn a_fixed_of_shapes_given_to_a_handled_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1i_spawn_fixed_shape_handle.ynz", "2\nA\ndone\n");
}

// Bug: the spawn released a consumed binding's original, but the `background` move site was keyed
// by the whole spawn expression, so the read of the binding never nulled its slot: the scope exit
// after the `if` released it again, a double free (High: data integrity and release).
#[test]
fn a_fixed_given_to_a_task_inside_a_branch_is_released_once() {
    assert_fixture_balanced("v0_3_m9_c1i_spawn_fixed_in_branch.ynz", "2\nA\ndone\n");
}

// The same producer with an `array`: the branch spawn crashed before `fixed` joined it.
#[test]
fn an_array_given_to_a_task_inside_a_branch_is_released_once() {
    assert_fixture_balanced("v0_3_m9_c1i_spawn_array_in_branch.ynz", "2\nA\ndone\n");
}

// Bug: a fresh `fixed` (a call result, not a named binding) leaked its original items: the
// spawn's release of a fresh original covered only a shape (High: data integrity and release).
#[test]
fn a_fresh_fixed_of_strings_given_to_a_forgotten_task_is_released() {
    assert_fixture_balanced(
        "v0_3_m9_c1i_spawn_fixed_fresh_string_forget.ynz",
        "2\nA\ndone\n",
    );
}

#[test]
fn a_fresh_fixed_of_strings_given_to_a_handled_task_is_released() {
    assert_fixture_balanced(
        "v0_3_m9_c1i_spawn_fixed_fresh_string_handle.ynz",
        "2\nA\ndone\n",
    );
}

#[test]
fn a_fresh_fixed_of_shapes_given_to_a_forgotten_task_is_released() {
    assert_fixture_balanced(
        "v0_3_m9_c1i_spawn_fixed_fresh_shape_forget.ynz",
        "2\nA\ndone\n",
    );
}

#[test]
fn a_fresh_fixed_of_shapes_given_to_a_handled_task_is_released() {
    assert_fixture_balanced(
        "v0_3_m9_c1i_spawn_fixed_fresh_shape_handle.ynz",
        "2\nA\ndone\n",
    );
}

// The same producer reached through a list literal instead of a call result.
#[test]
fn a_fixed_literal_given_to_a_forgotten_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1i_spawn_fixed_literal_forget.ynz", "2\nA\ndone\n");
}
