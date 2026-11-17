//! v0.3-M9 C1h (card #433): a `fixed<T, N>` in a shape field is stored by value in the shape.
//!
//! Before: a `fixed<T>` field held a pointer into the stack frame that built the shape, with no
//! length (`count()` read 0, every item read `none`, and a read went through a dead frame). Each
//! program here reads the list's real count and items, is clean under AddressSanitizer and, where
//! the list owns heap, frees every allocation, at both optimization levels.

mod support;

use support::{assert_asan_clean_and_prints, assert_fixture_balanced_and_asan_clean};
use support::at_both_optimization_levels;

// Bug: a shape's `fixed<int>` field reads count 0 and `none` once the building function returned
// (High: data integrity of a value stored in a shape).
#[test]
fn a_fixed_field_of_ints_keeps_its_count_and_items() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean_and_prints("v0_3_m9_c1h_fixed_field_int.ynz", no_optimize, "3\n1\n3\n");
    });
}

// Bug: a shape's `fixed<string>` field never released its strings and read freed stack (High: data
// integrity and release).
#[test]
fn a_fixed_field_of_strings_is_read_and_released() {
    assert_fixture_balanced_and_asan_clean("v0_3_m9_c1h_fixed_field_string.ynz", "2\nAMY\nbob\n");
}
