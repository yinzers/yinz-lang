//! v0.3-M9 C1h (card #433): a `fixed<T, N>` in a shape field is stored by value in the shape.
//!
//! Before: a `fixed<T>` field held a pointer into the stack frame that built the shape, with no
//! length (`count()` read 0, every item read `none`, and a read went through a dead frame). Each
//! program here reads the list's real count and items, is clean under AddressSanitizer and, where
//! the list owns heap, frees every allocation, at both optimization levels.

mod support;

use support::at_both_optimization_levels;
use support::{assert_asan_clean_and_prints, assert_fixture_balanced_and_asan_clean};

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

// Bug: each path that moves a shape holding a list must copy the list's cells and the items they
// point at, or two owners release one string (double free) or write one list (High: data
// integrity, release).
#[test]
fn a_fixed_field_is_read_assigned_written_and_copied_with_its_shape() {
    assert_fixture_balanced_and_asan_clean(
        "v0_3_m9_c1h_fixed_field_ops.ynz",
        "3\n15\n3\nX\ny\n3\n7\n9\n100\nq\n7\n-5\np\n",
    );
}

// Bug: `let alias = bag.items` bound the field's own cells, so writing the alias wrote the shape.
#[test]
fn a_written_binding_of_a_fixed_field_owns_a_copy() {
    assert_fixture_balanced_and_asan_clean(
        "v0_3_m9_c1h_fixed_field_read_copy.ynz",
        "1\n99\nB\n11\n16\n",
    );
}

// Bug: an array element's inline bytes carried a list whose strings the element's release skipped.
#[test]
fn an_array_of_shapes_with_a_fixed_field_releases_every_string() {
    assert_fixture_balanced_and_asan_clean(
        "v0_3_m9_c1h_fixed_field_array_of_shapes.ynz",
        "M\nM\nM\n3\nN\n",
    );
}

// Bug: a nested shape or a union variant copied the shape's bytes and shared the list's strings.
#[test]
fn a_fixed_field_survives_nesting_copy_and_a_union() {
    assert_fixture_balanced_and_asan_clean(
        "v0_3_m9_c1h_fixed_field_nested_union.ynz",
        "J\n6\nI\n5\nJ\n",
    );
}

// Bug: a task's argument copy of a shape shared the list's strings with the spawner.
#[test]
fn a_shape_with_a_fixed_field_crosses_into_a_background_task() {
    assert_fixture_balanced_and_asan_clean("v0_3_m9_c1h_fixed_field_background.ynz", "4\n14\nB\n");
}

// Bug: a shape lent across a `wait` lost the list stored inside it.
#[test]
fn a_fixed_field_survives_a_suspension() {
    assert_fixture_balanced_and_asan_clean("v0_3_m9_c1h_fixed_field_suspending.ynz", "50\n50\nB\n");
}

// Bug: a field of decimal items was laid out with 8-byte cells and a hidden field's list default
// was refused.
#[test]
fn fixed_fields_of_every_item_kind_and_a_hidden_default_work() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean_and_prints(
            "v0_3_m9_c1h_fixed_field_item_kinds.ynz",
            no_optimize,
            "2.25\ntrue\n1.5\n3\n9.5\n1.5\n3.75\n",
        );
    });
}

// Bug: the struct-of-arrays layout cut a shape's fields apart and dropped a field's inline cells.
#[test]
fn a_fixed_field_survives_the_struct_of_arrays_layout() {
    assert_fixture_balanced_and_asan_clean("v0_3_m9_c1h_fixed_field_soa.ynz", "4970\n7\n");
}
