//! v0.3-M9 C1h (card #433): a `fixed<T>` in a function signature carries its length.
//!
//! Before: a returned `fixed` was a bare pointer into the callee's dead stack frame with no
//! length (`count()` read 0, every index read `none`), and an unsized `fixed` parameter had no
//! length either. Each program here reads the list's real count and items, is clean under
//! AddressSanitizer and (where it owns heap) frees every allocation, at both optimization levels.

mod support;

use support::{
    assert_asan_clean_and_prints,
    assert_fixture_balanced_and_asan_clean as assert_fixture_balanced, at_both_optimization_levels,
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

// Bug: a SUSPENDING function's returned `fixed<int>` read count 0 and `none` for every item (the
// state-machine path returned a pointer into the resume function's dead stack frame; High: data
// integrity of a value that leaves its function).
#[test]
fn a_fixed_a_suspending_function_returns_keeps_its_count_and_items() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean_and_prints(
            "v0_3_m9_c1h_fixed_return_suspending.ynz",
            no_optimize,
            "3\n10\n30\n",
        );
    });
}

// Bug: the same for a `fixed<Part>` of heap-owning shapes: wrong count, and the items leaked.
#[test]
fn a_fixed_of_shapes_a_suspending_function_returns_is_read_and_released() {
    assert_fixture_balanced(
        "v0_3_m9_c1h_fixed_shape_return_suspending.ynz",
        "2\nA\nbb\n",
    );
}

// Bug: a suspending function that WROTE the length of its returned `fixed` got no return region in
// its frame (only an inferred length did), so the compiler stopped with an internal error (High:
// data integrity of a value that leaves its function).
#[test]
fn a_fixed_a_suspending_function_returns_with_a_written_length_is_read() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean_and_prints(
            "v0_3_m9_c1h_fixed_return_sized_suspending.ynz",
            no_optimize,
            "2\n6\n",
        );
    });
}

// Bug: a `background` task that returns a `fixed` list handed its parent the address of the list in
// the task's freed frame: `receive()` read count 0 (High: data integrity of a value that leaves its
// task).
#[test]
fn a_fixed_a_background_task_returns_is_received_with_its_items() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean_and_prints(
            "v0_3_m9_c1h_fixed_handle_ints.ynz",
            no_optimize,
            "3\n30\n2\n6\n",
        );
    });
}

// Bug: the strings of a received `fixed<string>` result were never released.
#[test]
fn a_fixed_of_strings_a_task_returns_is_received_and_released() {
    assert_fixture_balanced("v0_3_m9_c1h_fixed_handle_strings.ynz", "2\nB\n");
}

// Bug: the strings of a `fixed<string>` result nobody received were never released.
#[test]
fn an_unreceived_fixed_of_strings_a_task_returns_is_released() {
    assert_fixture_balanced("v0_3_m9_c1h_fixed_handle_unreceived.ynz", "done\n");
}

// Bug: a `fixed<string>` a suspending function returned, bound to a local that is released after a
// later `wait`, was stored as "moved" and released as a null list (High: data integrity of a value
// that leaves its function).
#[test]
fn a_fixed_a_suspending_function_returns_is_released_after_a_later_wait() {
    assert_fixture_balanced(
        "v0_3_m9_c1h_fixed_return_suspending_released_after_wait.ynz",
        "A\n",
    );
}

// Bug: a WRITTEN length (`fixed<int, 4>`, `fixed<string, 2>`, a parameter of `fixed<int, 0>`, an
// exported function) was only compiled in diagnostic tests, never run: a returned list of strings
// that leaked, or an empty list read as one cell, would not have been caught (High: data
// integrity of a value that crosses a call; empty is the boundary).
#[test]
fn a_written_length_is_read_returned_and_released() {
    assert_fixture_balanced("v0_3_m9_c1h_fixed_written_lengths.ynz", "10\n2\nbo\n3\n0\n");
}

// Bug: a function returning `fixed<int, 0>` loaded and stored a zero-sized `[0 x i64]` through a
// pointer to a one-cell stack slot; AddressSanitizer reports it as an underflow of that slot
// (High: memory safety at the empty boundary; the ASan-clean exit check covers the whole feature).
#[test]
fn a_fixed_of_zero_items_a_function_returns_is_asan_clean() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean_and_prints("v0_3_m9_c1h_fixed_return_zero.ynz", no_optimize, "0\n");
    });
}

// Bug (same cause as above): the empty list a suspending function or a `background` task returns
// travels through the suspension frame and the task handle; each of those copies is zero cells too
// (High: memory safety at the empty boundary).
#[test]
fn a_fixed_of_zero_items_a_suspending_function_or_task_returns_is_asan_clean() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean_and_prints(
            "v0_3_m9_c1h_fixed_return_zero_suspending.ynz",
            no_optimize,
            "0\n0\n",
        );
    });
}
