//! v0.3-M9 C6: `fixed.set` and `fixed` index assignment release the item they replace.
//!
//! The write stored the new item over the old cell and never released the old one, so a
//! `fixed<string>` or `fixed<Tag>` leaked every overwritten item (an array `set` released it:
//! one producer, two behaviours). Each case is balanced (cells and strings) and AddressSanitizer
//! clean at both optimization levels.

mod support;

use support::assert_fixture_balanced_and_asan_clean;

/// One named test per item kind of the `fixed set` run unit.
macro_rules! fixed_set_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal;)+) => { $(
        // bug: fixed.set or index assignment leaks the item it replaces, or releases one another holder owns | tier: High
        #[test]
        fn $case_name() {
            assert_fixture_balanced_and_asan_clean($fixture, $expected_stdout);
        }
    )+ };
}

fixed_set_high_case! {
    // case: a `fixed<string>` leaks each overwritten string
    fixed_string_set_and_index_assign, "v0_3_m9_c6_fixed_set_string.ynz", "c-3\nd-4\n";
    // case: a `fixed<Tag>` leaks each overwritten shape and its name
    fixed_shape_set, "v0_3_m9_c6_fixed_set_shape.ynz", "c-3\nb-2\n";
    // case: the cell holds a pointer the list owns, so the write frees what it replaces through the same dispatch (`stored_release`)
    fixed_array_set_and_index_assign, "v0_3_m9_c6_fixed_set_array.ynz", "3\n1\n";
    // case: a `fixed<Map>` leaks each overwritten map
    fixed_map_set, "v0_3_m9_c6_fixed_set_map.ynz", "2\n1\n";
    // case: a `fixed<Maybe<..>>` leaks each overwritten payload
    fixed_maybe_set_and_index_assign, "v0_3_m9_c6_fixed_set_maybe.ynz", "2\n";
}
