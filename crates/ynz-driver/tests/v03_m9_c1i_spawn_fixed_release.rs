//! v0.3-M9 C1i (card #472): a `background` call that passes a `fixed<T>` of heap-backed items
//! with no ownership keyword releases every item exactly once, whether the callee never suspends
//! (fire-and-forget, CPU pool) or suspends (handled, scheduler state machine).

mod support;

use support::assert_fixture_balanced_and_asan_clean as assert_fixture_balanced;

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
