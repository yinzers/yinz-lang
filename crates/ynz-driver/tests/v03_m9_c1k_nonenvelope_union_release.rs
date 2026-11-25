//! C1k, C1l, C1m and C1n (cards #492 to #496): a value held in a union or `maybe` cell is read
//! with the right variant and released exactly once. Covers a union with no shape variant and no
//! tag (`int | float`, `string | nothing`, `array<int> | nothing`), which is released as what it
//! is stored as (nothing, or its one heap variant's raw pointer); a promoted union cell; a generic
//! instance that wraps the value it returns into its union's envelope; and the advised forms of a
//! `share` parameter stored into a union local.

mod support;

use support::{
    assert_fixture_balanced_and_asan_clean as assert_balanced,
    assert_fixture_balanced_and_asan_clean_printing_one_of as assert_balanced_one_of,
};

/// One named test per way a value reaches a union cell, for the `union cell value release` run
/// unit; each case is a test the runner names.
macro_rules! union_cell_release_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a value held in a union or `maybe` cell is read as the wrong variant, leaked, or freed twice (or the build fails with an internal error) | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

union_cell_release_high_case! {
    // case: a shape with an `int | float` field: the build failed with an internal error whenever such a shape was released
    a_shape_with_an_int_or_float_field_builds_and_is_released => { assert_balanced("v0_3_m9_c1k_shape_int_float_field.ynz", "A\nB\n"); }
    // case: a shape with a `string | nothing` field
    a_shape_with_a_string_or_nothing_field_builds_and_is_released => { assert_balanced("v0_3_m9_c1k_shape_string_nothing_field.ynz", "A\n"); }
    // case: an `array<int> | nothing` given to a task: 2 of 4 allocations leaked, the spawner's original was never released
    an_array_or_nothing_given_to_a_task_is_released => { assert_balanced_one_of("v0_3_m9_c1k_spawn_array_nothing.ynz", &["ran\ndone\n", "done\nran\n"]); }
    // case: a copy shared the field's pointer, so releasing original and copy freed one string twice
    a_copied_shape_with_a_string_or_nothing_field_releases_its_own_string => { assert_balanced("v0_3_m9_c1k_shape_string_nothing_copied.ynz", "A\nA\nA\n"); }
    // case: the same copy sharing for array and map fields
    a_copied_shape_with_array_and_map_or_nothing_fields_releases_its_own_values => { assert_balanced("v0_3_m9_c1k_shape_array_map_nothing_copied.ynz", "A\nA\n"); }
    // case: a function returning a shared `map | nothing` parameter as it is aliased the argument, and caller and owner both released it
    a_shared_map_or_nothing_parameter_returned_as_it_is_is_copied => { assert_balanced("v0_3_m9_c1k_union_returned_shared_param.ynz", "ok\nend\n"); }
    // case: a generic instance's `T | nothing` was judged with `T` unsubstituted, so releasing the given argument was an internal error
    a_generic_union_given_to_a_function_is_released_as_the_instance_stores_it => { assert_balanced("v0_3_m9_c1k_generic_union_given_released.ynz", "ate\nate\nate\nend\n"); }
    // case: `resolve_type` flattened a generic `T | U` with `T = U` to the plain shape and the build failed for a program that built before
    a_generic_union_with_both_parameters_the_same_shape_is_that_shape => { assert_balanced("v0_3_m9_c1k_generic_union_same_type_args.ynz", "HELLO\nend\n"); }
    // case: a generic union flattened by `resolve_type` kept the slot's tag as written, so a variant after a bound union was read as the wrong shape
    a_generic_union_bound_to_a_union_tags_a_later_variant_in_the_flat_order => { assert_balanced("v0_3_m9_c1k_generic_union_flat_tag_order.ynz", "tiny\nend\n"); }
    // case: a `fixed<string> | nothing` local leaked its two strings
    a_fixed_string_list_held_by_a_union_local_is_released => { assert_balanced("v0_3_m9_c1m_fixed_string_union_local.ynz", "kept\n"); }
    // case: a `.copy()` of a shape held in a `T | nothing` local was released through an envelope it was never wrapped into (crash when the first field is 0)
    a_shape_copy_held_by_a_generic_union_local_is_released => { assert_balanced("v0_3_m9_c1m_generic_shape_copy_into_union_local.ynz", "kept\nABCDEFGHIJ\n"); }
    // case: `wrap<Big>` copied the shape's bytes raw instead of wrapping them in the `Big | nothing` envelope (printed `absent`, 4 of 10 allocations leaked)
    a_generic_union_return_of_a_shape_is_wrapped_and_released => { assert_balanced("v0_3_m9_c1n_generic_union_return_shape.ynz", "hello\nbye\n"); }
    // case: a generic `T | nothing` instantiated with an envelope union failed to build once the return was recorded as a coercion (a program that was correct)
    a_generic_union_return_of_a_union_value_is_returned_as_it_is => { assert_balanced("v0_3_m9_c1n_generic_union_return_union_bound.ynz", "hello\n"); }
    // case: the advised `.copy()` form of a `share` parameter stored into a union local leaks or double frees the array
    a_copy_of_a_share_parameter_stored_into_a_union_local_is_released => { assert_balanced("v0_3_m9_c1l_share_into_union_local_copy.ynz", "kept\n3\n"); }
    // case: the advised `give` form of a `share` parameter stored into a union local leaks or double frees the array
    a_give_parameter_stored_into_a_union_local_is_released => { assert_balanced("v0_3_m9_c1l_share_into_union_local_give.ynz", "kept\n"); }
}
