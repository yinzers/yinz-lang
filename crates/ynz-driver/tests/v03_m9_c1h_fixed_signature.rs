//! v0.3-M9 C1h (card #433): a `fixed<T>` in a function signature carries its length.
//!
//! Before: a returned `fixed` was a bare pointer into the callee's dead stack frame with no
//! length (`count()` read 0, every index read `none`), and an unsized `fixed` parameter had no
//! length either. Each program here reads the list's real count and items, is clean under
//! AddressSanitizer and (where it owns heap) frees every allocation, at both optimization levels.

mod support;

use support::{
    assert_asan_clean_and_prints, assert_fixture_balanced_and_asan_clean as assert_fixture_balanced,
    at_both_optimization_levels,
};

// Bug: a returned `fixed<int>` reads count 0 and `none` for every item (High: data integrity of a
// value that leaves its function).
#[test]
fn a_returned_fixed_of_ints_keeps_its_count_and_items() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean_and_prints(
            "v0_3_m9_c1h_fixed_return_read.ynz",
            no_optimize,
            "3\n10\n30\n",
        );
    });
}

// Bug: a returned `fixed<Part>` reads count 0 and its items leak (High: data integrity and release).
#[test]
fn a_returned_fixed_of_shapes_keeps_its_count_and_items_and_releases_them() {
    assert_fixture_balanced("v0_3_m9_c1h_fixed_shape_return_read.ynz", "2\nA\nbb\n");
}

// Bug: an unsized `fixed` parameter has no length (`count()` 0), and iterating one stops the
// compiler with an internal error (High: data integrity across a call).
#[test]
fn an_unsized_fixed_parameter_sees_the_length_of_each_argument() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean_and_prints(
            "v0_3_m9_c1h_fixed_param_two_lengths.ynz",
            no_optimize,
            "2\n3\n3\n60\n",
        );
    });
}

// The four programs the leak sweep listed as known leaks: each frees every allocation now
// (bug: the items of an unsized `fixed` in a signature were never released).
#[test]
fn a_fixed_of_strings_returned_by_a_function_is_released() {
    assert_fixture_balanced("v0_3_m9_c8_s4b_unsized_fixed_string_return.ynz", "built\n");
}

#[test]
fn a_fixed_of_strings_given_to_a_function_is_released() {
    assert_fixture_balanced("v0_3_m9_c8_s4b_unsized_fixed_string_param.ynz", "1\n");
}

#[test]
fn a_fixed_of_shapes_returned_by_a_function_is_released() {
    assert_fixture_balanced("v0_3_m9_c8_s4b_unsized_fixed_shape_return.ynz", "built\n");
}

#[test]
fn a_fixed_of_shapes_given_to_a_function_is_released() {
    assert_fixture_balanced("v0_3_m9_c8_s4b_unsized_fixed_shape_param.ynz", "1\n");
}
