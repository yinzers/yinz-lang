//! C1h and C1i: a `fixed<T, N>` value keeps its length and items, is clean under AddressSanitizer
//! and, where it owns heap, frees every allocation exactly once, at both optimization levels. One
//! table, one case per position the value lives in: a shape field, an array element, a nested
//! shape or union variant, a function parameter or return (plain and suspending), a `background`
//! argument (forgotten and handled), and a task result.
//!
//! Before: a `fixed<T>` held a pointer into the stack frame that built it, with no length
//! (`count()` read 0, every item read `none`, a read went through a dead frame), and nothing
//! released the items of one handed to a task.

mod support;

use support::{
    assert_asan_clean_and_prints, assert_fixture_balanced_and_asan_clean as assert_balanced,
    assert_fixture_balanced_and_asan_clean_task_then_done as assert_balanced_either_order,
    at_both_optimization_levels,
};

/// Clean under AddressSanitizer and prints `expected`, at both levels (a value that owns no heap).
fn assert_clean_and_prints(fixture: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean_and_prints(fixture, no_optimize, expected);
    });
}

/// One named test per position of the `fixed value integrity and release` run unit; each case is
/// a test the runner names.
macro_rules! fixed_value_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a `fixed<T>` value reads count 0 or `none` through a dead frame, leaks its items, or frees them twice | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

fixed_value_high_case! {
    // case: a shape's `fixed<int>` field read count 0 and `none` once the building function returned
    a_fixed_field_of_ints_keeps_its_count_and_items => { assert_clean_and_prints("v0_3_m9_c1h_fixed_field_int.ynz", "3\n1\n3\n"); }
    // case: a shape's `fixed<string>` field never released its strings and read freed stack
    a_fixed_field_of_strings_is_read_and_released => { assert_balanced("v0_3_m9_c1h_fixed_field_string.ynz", "2\nAMY\nbob\n"); }
    // case: each path that moves a shape holding a list must copy its cells and items, or two owners release one string
    a_fixed_field_is_read_assigned_written_and_copied_with_its_shape => { assert_balanced("v0_3_m9_c1h_fixed_field_ops.ynz", "3\n15\n3\nX\ny\n3\n7\n9\n100\nq\n7\n-5\np\n"); }
    // case: `let alias = bag.items` bound the field's own cells, so writing the alias wrote the shape
    a_written_binding_of_a_fixed_field_owns_a_copy => { assert_balanced("v0_3_m9_c1h_fixed_field_read_copy.ynz", "1\n99\nB\n11\n16\n"); }
    // case: an array element's inline bytes carried a list whose strings the element's release skipped
    an_array_of_shapes_with_a_fixed_field_releases_every_string => { assert_balanced("v0_3_m9_c1h_fixed_field_array_of_shapes.ynz", "M\nM\nM\n3\nN\n"); }
    // case: a nested shape or a union variant copied the shape's bytes and shared the list's strings
    a_fixed_field_survives_nesting_copy_and_a_union => { assert_balanced("v0_3_m9_c1h_fixed_field_nested_union.ynz", "J\n6\nI\n5\nJ\n"); }
    // case: a task's argument copy of a shape shared the list's strings with the spawner
    a_shape_with_a_fixed_field_crosses_into_a_background_task => { assert_balanced("v0_3_m9_c1h_fixed_field_background.ynz", "4\n14\nB\n"); }
    // case: a shape lent across a `wait` lost the list stored inside it
    a_fixed_field_survives_a_suspension => { assert_balanced("v0_3_m9_c1h_fixed_field_suspending.ynz", "50\n50\nB\n"); }
    // case: a field of decimal items was laid out with 8-byte cells and a hidden field's list default was refused
    fixed_fields_of_every_item_kind_and_a_hidden_default_work => { assert_clean_and_prints("v0_3_m9_c1h_fixed_field_item_kinds.ynz", "2.25\ntrue\n1.5\n3\n9.5\n1.5\n3.75\n"); }
    // case: the struct-of-arrays layout cut a shape's fields apart and dropped a field's inline cells
    a_fixed_field_survives_the_struct_of_arrays_layout => { assert_balanced("v0_3_m9_c1h_fixed_field_soa.ynz", "4970\n7\n"); }
    // case: two inline shapes that differ only in a list's length got the same synthetic name, so the second one's type was lost
    inline_shapes_with_different_list_lengths_are_different_types => { assert_balanced("v0_3_m9_c1h_fixed_field_inline_shape.ynz", "2\n3\n9\nP\n"); }
    // case: a returned `fixed<int>` was a bare pointer into the callee's dead frame (count 0, `none` for every item)
    a_returned_fixed_of_ints_keeps_its_count_and_items => { assert_clean_and_prints("v0_3_m9_c1h_fixed_return_read.ynz", "3\n10\n30\n"); }
    // case: a returned `fixed<Part>` read count 0 and its items leaked
    a_returned_fixed_of_shapes_keeps_its_count_and_items_and_releases_them => { assert_balanced("v0_3_m9_c1h_fixed_shape_return_read.ynz", "2\nA\nbb\n"); }
    // case: an unsized `fixed` parameter had no length (`count()` 0) and iterating one stopped the compiler with an internal error
    an_unsized_fixed_parameter_sees_the_length_of_each_argument => { assert_clean_and_prints("v0_3_m9_c1h_fixed_param_two_lengths.ynz", "2\n3\n3\n60\n"); }
    // case: the items of an unsized `fixed<string>` returned from a function were never released
    a_fixed_of_strings_returned_by_a_function_is_released => { assert_balanced("v0_3_m9_c8_s4b_unsized_fixed_string_return.ynz", "built\n"); }
    // case: the items of an unsized `fixed<string>` given to a function were never released
    a_fixed_of_strings_given_to_a_function_is_released => { assert_balanced("v0_3_m9_c8_s4b_unsized_fixed_string_param.ynz", "1\n"); }
    // case: the items of an unsized `fixed<Shape>` returned from a function were never released
    a_fixed_of_shapes_returned_by_a_function_is_released => { assert_balanced("v0_3_m9_c8_s4b_unsized_fixed_shape_return.ynz", "built\n"); }
    // case: the items of an unsized `fixed<Shape>` given to a function were never released
    a_fixed_of_shapes_given_to_a_function_is_released => { assert_balanced("v0_3_m9_c8_s4b_unsized_fixed_shape_param.ynz", "1\n"); }
    // case: a suspending function's returned `fixed<int>` was a pointer into the resume function's dead frame
    a_fixed_a_suspending_function_returns_keeps_its_count_and_items => { assert_clean_and_prints("v0_3_m9_c1h_fixed_return_suspending.ynz", "3\n10\n30\n"); }
    // case: a suspending function's returned `fixed<Part>` of heap-owning shapes had the wrong count and leaked its items
    a_fixed_of_shapes_a_suspending_function_returns_is_read_and_released => { assert_balanced("v0_3_m9_c1h_fixed_shape_return_suspending.ynz", "2\nA\nbb\n"); }
    // case: a suspending function that wrote the length of its returned `fixed` got no return region in its frame (internal error)
    a_fixed_a_suspending_function_returns_with_a_written_length_is_read => { assert_clean_and_prints("v0_3_m9_c1h_fixed_return_sized_suspending.ynz", "2\n6\n"); }
    // case: a `background` task returning a `fixed` handed its parent the address of the list in the task's freed frame
    a_fixed_a_background_task_returns_is_received_with_its_items => { assert_clean_and_prints("v0_3_m9_c1h_fixed_handle_ints.ynz", "3\n30\n2\n6\n"); }
    // case: the strings of a received `fixed<string>` task result were never released
    a_fixed_of_strings_a_task_returns_is_received_and_released => { assert_balanced("v0_3_m9_c1h_fixed_handle_strings.ynz", "2\nB\n"); }
    // case: the strings of a `fixed<string>` task result nobody received were never released
    an_unreceived_fixed_of_strings_a_task_returns_is_released => { assert_balanced("v0_3_m9_c1h_fixed_handle_unreceived.ynz", "done\n"); }
    // case: a `fixed<string>` a suspending function returned, bound to a local released after a later `wait`, was stored as "moved" and released as a null list
    a_fixed_a_suspending_function_returns_is_released_after_a_later_wait => { assert_balanced("v0_3_m9_c1h_fixed_return_suspending_released_after_wait.ynz", "A\n"); }
    // case: a written length (`fixed<int, 4>`, a parameter of `fixed<int, 0>`, an exported function) was only compiled, never run; empty is the boundary
    a_written_length_is_read_returned_and_released => { assert_balanced("v0_3_m9_c1h_fixed_written_lengths.ynz", "10\n2\nbo\n3\n0\n"); }
    // case: a function returning `fixed<int, 0>` loaded and stored a zero-sized `[0 x i64]` through a one-cell slot (ASan underflow)
    a_fixed_of_zero_items_a_function_returns_is_asan_clean => { assert_clean_and_prints("v0_3_m9_c1h_fixed_return_zero.ynz", "0\n"); }
    // case: the empty list a suspending function or a task returns travels through the suspension frame and the task handle as zero cells too
    a_fixed_of_zero_items_a_suspending_function_or_task_returns_is_asan_clean => { assert_clean_and_prints("v0_3_m9_c1h_fixed_return_zero_suspending.ynz", "0\n0\n"); }
    // case: `received.or([`x`, `y`])` bound a pointer at the received list while the binding and the result each released it
    a_received_fixed_of_strings_read_with_an_inline_fallback_is_released_once => { assert_balanced("v0_3_m9_c1h_fixed_handle_strings_inline_or.ynz", "2\nB\n"); }
    // case: an inline `.or` fallback list of ints, or of shapes owning a string, releases the picked list once and the fallback's temporary once
    a_received_fixed_of_ints_and_of_shapes_read_with_an_inline_fallback_is_released_once => { assert_balanced("v0_3_m9_c1h_fixed_handle_item_kinds_inline_or.ynz", "3\n30\n2\nbb\n"); }
    // case: a `fixed<string>` given to a forgotten (fire-and-forget) task leaked 2 of 4 strings
    a_fixed_of_strings_given_to_a_forgotten_task_is_released => { assert_balanced_either_order("v0_3_m9_c1i_spawn_fixed_string_forget.ynz", "2\nA\ndone\n"); }
    // case: a `fixed<string>` given to a handled (state-machine) task
    a_fixed_of_strings_given_to_a_handled_task_is_released => { assert_balanced_either_order("v0_3_m9_c1i_spawn_fixed_string_handle.ynz", "2\nA\ndone\n"); }
    // case: a `fixed<Shape>` given to a forgotten task
    a_fixed_of_shapes_given_to_a_forgotten_task_is_released => { assert_balanced_either_order("v0_3_m9_c1i_spawn_fixed_shape_forget.ynz", "2\nA\ndone\n"); }
    // case: a `fixed<Shape>` given to a handled task
    a_fixed_of_shapes_given_to_a_handled_task_is_released => { assert_balanced_either_order("v0_3_m9_c1i_spawn_fixed_shape_handle.ynz", "2\nA\ndone\n"); }
    // case: the background move site was keyed by the whole spawn expression, so a spawn inside a branch never nulled the binding's slot and the scope exit freed it again
    a_fixed_given_to_a_task_inside_a_branch_is_released_once => { assert_balanced_either_order("v0_3_m9_c1i_spawn_fixed_in_branch.ynz", "2\nA\ndone\n"); }
    // case: the same branch spawn with an `array` crashed before `fixed` joined it
    an_array_given_to_a_task_inside_a_branch_is_released_once => { assert_balanced_either_order("v0_3_m9_c1i_spawn_array_in_branch.ynz", "2\nA\ndone\n"); }
    // case: a fresh `fixed<string>` (a call result, not a named binding) given to a forgotten task leaked its original items
    a_fresh_fixed_of_strings_given_to_a_forgotten_task_is_released => { assert_balanced_either_order("v0_3_m9_c1i_spawn_fixed_fresh_string_forget.ynz", "2\nA\ndone\n"); }
    // case: a fresh `fixed<string>` given to a handled task
    a_fresh_fixed_of_strings_given_to_a_handled_task_is_released => { assert_balanced_either_order("v0_3_m9_c1i_spawn_fixed_fresh_string_handle.ynz", "2\nA\ndone\n"); }
    // case: a fresh `fixed<Shape>` given to a forgotten task (the release of a fresh original covered only a shape)
    a_fresh_fixed_of_shapes_given_to_a_forgotten_task_is_released => { assert_balanced_either_order("v0_3_m9_c1i_spawn_fixed_fresh_shape_forget.ynz", "2\nA\ndone\n"); }
    // case: a fresh `fixed<Shape>` given to a handled task
    a_fresh_fixed_of_shapes_given_to_a_handled_task_is_released => { assert_balanced_either_order("v0_3_m9_c1i_spawn_fixed_fresh_shape_handle.ynz", "2\nA\ndone\n"); }
    // case: the same producer reached through a list literal instead of a call result
    a_fixed_literal_given_to_a_forgotten_task_is_released => { assert_balanced_either_order("v0_3_m9_c1i_spawn_fixed_literal_forget.ynz", "2\nA\ndone\n"); }
    // case: the recovery the refusal teaches: the items in an `array` are handed to a task and released
    an_array_of_arrays_handed_to_a_task_is_read_and_released => { assert_balanced("v0_3_m9_c1i_spawn_array_of_arrays_given.ynz", "2\ndone\n"); }
}
