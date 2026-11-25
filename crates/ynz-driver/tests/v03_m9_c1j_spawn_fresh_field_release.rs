//! C1i, C1j and C1l (cards #472, #491, #493): what a `background` call may be handed from a value
//! it does not own whole. A piece with an owned copy (a field of a fresh call result, a copyable
//! field of a named value, a union field, a channel) is released once by the spawner while the task
//! reads its own copy; one binding passed in two positions is handed over once; a piece with no
//! owned copy (a `fixed` of arrays or maps, an array of channels, a union with no envelope) is
//! refused at compile time, with typeck's teaching text and never a backend error.

mod support;

use support::{
    assert_fixture_balanced_and_asan_clean_task_then_done as assert_balanced_either_order,
    assert_refused_with,
};

/// One named test per piece of the `background argument piece release` run unit; each case is a
/// test the runner names.
macro_rules! spawn_piece_release_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a piece handed to a `background` task leaks the original (or the temporary it came from), is freed twice, or the task reads memory the spawner freed | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

spawn_piece_release_high_case! {
    // case: a `fixed` field of a fresh call result (`make().items`): 2 of 4 strings leaked, the original field was never released
    a_fixed_field_of_a_fresh_call_result_given_to_a_forgotten_task_is_released => { assert_balanced_either_order("v0_3_m9_c1j_spawn_fresh_field_fixed.ynz", "2\nA\ndone\n"); }
    // case: an `array` field of a fresh call result: the original array and two of its strings leaked
    an_array_field_of_a_fresh_call_result_given_to_a_forgotten_task_is_released => { assert_balanced_either_order("v0_3_m9_c1j_spawn_fresh_field_array.ynz", "2\nA\ndone\n"); }
    // case: a shape field of a fresh call result: the original shape field's strings leaked
    a_shape_field_of_a_fresh_call_result_given_to_a_forgotten_task_is_released => { assert_balanced_either_order("v0_3_m9_c1j_spawn_fresh_field_shape.ynz", "A\nB\ndone\n"); }
    // case: a field two levels deep: the whole temporary (outer shape, inner shape, array, strings) is released once
    a_field_two_levels_deep_of_a_fresh_call_result_given_to_a_task_is_released => { assert_balanced_either_order("v0_3_m9_c1j_spawn_fresh_field_nested.ynz", "2\nA\ndone\n"); }
    // case: `make()[0]`, a `maybe<string>` read out of the fresh array
    an_element_of_a_fresh_call_result_given_to_a_task_is_released => { assert_balanced_either_order("v0_3_m9_c1j_spawn_fresh_index.ynz", "A\ndone\n"); }
    // case: a channel piece is shared with the task by a counted reference; the piece refusal wrongly read the owned-copy table and rejected a correct program
    a_channel_field_of_a_fresh_call_result_is_shared_with_the_task_and_the_rest_released => { assert_balanced_either_order("v0_3_m9_c1j_spawn_fresh_channel_field.ynz", "sent\ndone\n"); }
    // case: control: a copyable field of a named value stays accepted and is released
    a_copyable_field_of_a_named_value_given_to_a_task_stays_accepted => { assert_balanced_either_order("v0_3_m9_c1j_spawn_named_copyable_field.ynz", "2\nA\ndone\n"); }
    // case: a union field of a named value gets a slot of its own in the task (on `main` it ran the wrong variant)
    a_union_field_of_a_named_value_given_to_a_task_stays_accepted => { assert_balanced_either_order("v0_3_m9_c1j_spawn_named_union_field.ynz", "BOXED\ndone\n"); }
    // case: a union variant that owns heap: the slot copies the payload and releases it once
    a_union_field_with_a_heap_payload_of_a_named_value_is_copied_for_the_task => { assert_balanced_either_order("v0_3_m9_c1j_spawn_union_heap_field.ynz", "2\nA\nL\ndone\n"); }
    // case: the same payload read out of a fresh call result: the temporary is released once, the task reads its own copy
    a_union_field_with_a_heap_payload_of_a_fresh_call_result_is_copied_for_the_task => { assert_balanced_either_order("v0_3_m9_c1j_spawn_union_heap_fresh.ynz", "2\nA\nL\ndone\n"); }
    // case: a named union with heap read by the spawner after the spawn and by the task after the spawner returned
    a_named_union_with_a_heap_payload_read_after_the_spawn_is_copied_for_the_task => { assert_balanced_either_order("v0_3_m9_c1j_spawn_union_heap_reused.ynz", "L\n2\nA\nL\ndone\n"); }
    // case: a union with no envelope (`array<int> | nothing`) is given as on `main`; `spawn_rehoming` promised a slot codegen never made (internal error)
    a_named_union_without_an_envelope_is_given_to_a_task_as_before => { assert_balanced_either_order("v0_3_m9_c1j_spawn_plain_union_given.ynz", "task\ntask\ndone\n"); }
    // case: one array binding in two positions of one spawn: the second read cloned the null the first read left in its slot (abort)
    one_array_binding_passed_twice_to_a_task_is_copied_twice_and_released_once => { assert_balanced_either_order("v0_3_m9_c1l_spawn_same_binding_twice.ynz", "3\n3\ndone\n"); }
}

/// One named test per refused piece of the `background argument piece refusal` typeck unit; each
/// case is a test the runner names. `$phrases` names the piece's type and the refusal.
macro_rules! spawn_piece_refusal_high_case {
    ($($case_name:ident, $fixture:literal, $phrases:expr;)+) => { $(
        // bug: typeck accepts a piece with no owned copy as a `background` argument, so the task reads memory the spawner (or its dead frame) already freed | tier: High
        #[test]
        fn $case_name() {
            assert_refused_with($fixture, $phrases);
        }
    )+ };
}

spawn_piece_refusal_high_case! {
    // case: a `fixed` of arrays (handed over whole) is kept with the function that made it
    a_handed_over_fixed_of_arrays_is_refused_for_a_task, "v0_3_m9_c1i_spawn_fixed_array_refused.ynz", &["`xs` is a `Fixed<Array<String>>`, and a `Fixed<Array<String>>` cannot be handed to a background task"];
    // case: a `fixed` of maps (handed over whole)
    a_handed_over_fixed_of_maps_is_refused_for_a_task, "v0_3_m9_c1i_spawn_fixed_map_refused.ynz", &["`xs` is a `Fixed<Map<String, Int>>`, and a `Fixed<Map<String, Int>>` cannot be handed to a background task"];
    // case: a field with no owned copy of a fresh call result: releasing the temporary at the statement end would free what the task reads
    a_field_with_no_owned_copy_of_a_fresh_call_result_is_refused_for_a_task, "v0_3_m9_c1j_spawn_fresh_field_refused.ynz", &["A `Fixed<Array<String>>` read out of another value", "cannot be handed to a background task"];
    // case: an array of channels has no independent copy and stays refused for a fresh value
    an_array_of_channels_field_of_a_fresh_call_result_stays_refused_for_a_task, "v0_3_m9_c1j_spawn_fresh_channels_array_refused.ynz", &["`Array<Channel<Int>>`", "cannot be handed to a background task"];
    // case: `background f(bag.rows)` with an uncopyable `rows` handed the task the array `bag` still owns, and `launch` released `bag` while the task read it
    an_array_of_channels_field_of_a_named_value_is_refused_for_a_task, "v0_3_m9_c1j_spawn_named_channels_array_refused.ynz", &["`Array<Channel<Int>>`", "cannot be handed to a background task"];
    // case: the same through a `fixed<array<string>, 2>` field: the task read the dead frame (ASan stack-use-after-return)
    a_fixed_of_arrays_field_of_a_named_value_is_refused_for_a_task, "v0_3_m9_c1j_spawn_named_fixed_refused.ynz", &["`Fixed<Array<String>>`", "cannot be handed to a background task"];
    // case: a field of a union with no envelope has no slot to copy into, so the task would share the array
    a_field_of_a_union_without_an_envelope_of_a_fresh_call_result_is_refused_for_a_task, "v0_3_m9_c1j_spawn_plain_union_field_refused.ynz", &["`Array<Int> | Nothing`", "cannot be handed to a background task"];
}
