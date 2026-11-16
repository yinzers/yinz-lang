//! v0.3-M9 C6: `fixed.set` and `fixed` index assignment release the item they replace.
//!
//! Bug: the write stored the new item over the old cell and never released the old one, so a
//! `fixed<string>` or `fixed<Tag>` leaked every overwritten item (an array `set` released it:
//! one producer, two behaviours). High tier: data integrity (a leak per write, or a release of an
//! item another holder owns, which AddressSanitizer reports).

mod support;

use support::{assert_asan_clean_and_prints, build, run_counted};

/// Run the fixture with the counters on and compare `(cell alloc, cell free, string alloc,
/// string free)`: the cells and strings the write replaced must have been freed.
fn assert_replaced_item_released(
    fixture: &str,
    no_optimize: bool,
    expected_stdout: &str,
    expected_counts: (u64, u64, u64, u64),
) {
    let built = build(fixture, no_optimize);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "stderr:\n{}", output.stderr);
    assert_eq!(output.stdout, expected_stdout);
    assert_eq!(
        (
            counters.alloc,
            counters.free,
            counters.string_alloc,
            counters.string_free
        ),
        expected_counts,
        "{fixture} (no_optimize={no_optimize}): {counters:?}"
    );
}

macro_rules! fixed_set_case {
    ($case:ident, $fixture:literal, $expected:literal, $alloc:literal, $free:literal, $string_alloc:literal, $string_free:literal) => {
        mod $case {
            use crate::{assert_asan_clean_and_prints, assert_replaced_item_released};

            #[test]
            fn replaced_item_is_released() {
                assert_replaced_item_released(
                    $fixture,
                    false,
                    $expected,
                    ($alloc, $free, $string_alloc, $string_free),
                );
            }

            #[test]
            fn replaced_item_is_released_no_optimize() {
                assert_replaced_item_released(
                    $fixture,
                    true,
                    $expected,
                    ($alloc, $free, $string_alloc, $string_free),
                );
            }

            #[test]
            fn asan_optimized() {
                assert_asan_clean_and_prints($fixture, false, $expected);
            }

            #[test]
            fn asan_no_optimize() {
                assert_asan_clean_and_prints($fixture, true, $expected);
            }
        }
    };
}

fixed_set_case!(
    fixed_string_set_and_index_assign,
    "v0_3_m9_c6_fixed_set_string.ynz",
    // C7 slice 2, 2 -> 3: `fallback` is read last by `.or(fallback)`, which only borrows it, so its
    // scope exit releases it.
    "c-3\nd-4\n",
    0,
    0,
    5,
    3
);
fixed_set_case!(
    fixed_shape_set,
    "v0_3_m9_c6_fixed_set_shape.ynz",
    "c-3\nb-2\n",
    3,
    1,
    // C7 slice 2b, 2 -> 4 strings: `firstItem` and `secondItem` read an item of a list the fixture
    // writes, so each owns a copy (a copy owner) and releases its name at its scope exit.
    6,
    4
);
// Items that are not strings or shapes: the cell holds a pointer the list owns, so the write
// frees what it replaces through the same dispatch (`stored_release`). The frees are the
// replaced item plus the `fallback` the fixture reads with `.or(fallback)`, which is released at
// scope exit now that an alias class with no escape releases each of its owners; the items the
// list still holds (a `fixed` list is slice 3's kind) stay unreleased.
fixed_set_case!(
    fixed_array_set_and_index_assign,
    "v0_3_m9_c6_fixed_set_array.ynz",
    "3\n1\n",
    10,
    6,
    0,
    0
);
fixed_set_case!(
    fixed_map_set,
    "v0_3_m9_c6_fixed_set_map.ynz",
    "2\n1\n",
    20,
    10,
    4,
    1
);
fixed_set_case!(
    fixed_maybe_set_and_index_assign,
    "v0_3_m9_c6_fixed_set_maybe.ynz",
    "2\n",
    6,
    4,
    8,
    4
);
