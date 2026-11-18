//! v0.3-M9 C1k (card #492): a union with no shape variant and no tag (`int | float`,
//! `string | nothing`, `array<int> | nothing`) is released through the one release helper as what
//! it is stored as: nothing, or its one heap variant's raw pointer.

mod support;

use support::{
    assert_built_balanced_and_asan_clean_printing_one_of, at_both_optimization_levels, build,
};

fn assert_fixture_balanced(fixture: &str, accepted: &[&str]) {
    at_both_optimization_levels(|no_optimize| {
        let built = build(fixture, no_optimize);
        assert_built_balanced_and_asan_clean_printing_one_of(
            fixture,
            &built,
            accepted,
            no_optimize,
        );
    });
}

// Bug: the build failed with an internal error whenever such a shape was released (High: data
// integrity and release).
#[test]
fn a_shape_with_an_int_or_float_field_builds_and_is_released() {
    assert_fixture_balanced("v0_3_m9_c1k_shape_int_float_field.ynz", &["A\nB\n"]);
}

#[test]
fn a_shape_with_a_string_or_nothing_field_builds_and_is_released() {
    assert_fixture_balanced("v0_3_m9_c1k_shape_string_nothing_field.ynz", &["A\n"]);
}

// Bug: 2 of 4 allocations leaked (also on `main`): the spawner's original was never released.
#[test]
fn an_array_or_nothing_given_to_a_task_is_released() {
    assert_fixture_balanced(
        "v0_3_m9_c1k_spawn_array_nothing.ynz",
        &["ran\ndone\n", "done\nran\n"],
    );
}

// Bug the release alone would add: a copy shared the field's pointer, so releasing both the
// original and the copy freed one string, array or map twice (High: data integrity and release).
#[test]
fn a_copied_shape_with_a_string_or_nothing_field_releases_its_own_string() {
    assert_fixture_balanced(
        "v0_3_m9_c1k_shape_string_nothing_copied.ynz",
        &["A\nA\nA\n"],
    );
}

#[test]
fn a_copied_shape_with_array_and_map_or_nothing_fields_releases_its_own_values() {
    assert_fixture_balanced(
        "v0_3_m9_c1k_shape_array_map_nothing_copied.ynz",
        &["A\nA\n"],
    );
}

// Bug: the call result of a function returning a shared `map | nothing` parameter aliased the
// argument, and the caller released it while its owner did too (High: data integrity and release).
#[test]
fn a_shared_map_or_nothing_parameter_returned_as_it_is_is_copied() {
    assert_fixture_balanced(
        "v0_3_m9_c1k_union_returned_shared_param.ynz",
        &["ok\nend\n"],
    );
}

// Bug: a generic instance's `T | nothing` was judged with `T` unsubstituted, so releasing the
// given argument was an internal error at build time (High: data integrity and release).
#[test]
fn a_generic_union_given_to_a_function_is_released_as_the_instance_stores_it() {
    assert_fixture_balanced(
        "v0_3_m9_c1k_generic_union_given_released.ynz",
        &["ate\nate\nate\nend\n"],
    );
}

// Bug: `resolve_type` flattened a generic `T | U` with `T = U` to the plain shape, and the build
// failed with an internal error for a program that built before (High: data integrity).
#[test]
fn a_generic_union_with_both_parameters_the_same_shape_is_that_shape() {
    assert_fixture_balanced(
        "v0_3_m9_c1k_generic_union_same_type_args.ynz",
        &["HELLO\nend\n"],
    );
}

// Bug: a generic union flattened by `resolve_type` kept the tag of the slot as written, so a
// variant after a bound union was read as the wrong shape (High: data integrity).
#[test]
fn a_generic_union_bound_to_a_union_tags_a_later_variant_in_the_flat_order() {
    assert_fixture_balanced(
        "v0_3_m9_c1k_generic_union_flat_tag_order.ynz",
        &["tiny\nend\n"],
    );
}
