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
