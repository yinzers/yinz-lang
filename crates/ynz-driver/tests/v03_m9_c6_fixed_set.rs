//! v0.3-M9 C6 round 2: `fixed.set` and `fixed` index assignment release the item they replace.
//!
//! Bug: the write stored the new item over the old cell and never released the old one, so a
//! `fixed<string>` or `fixed<Tag>` leaked every overwritten item (an array `set` released it:
//! one producer, two behaviours). High tier: data integrity (a leak per write, or a release of an
//! item another holder owns, which AddressSanitizer reports).

mod support;

use support::{assert_asan_clean_and_prints, build, run_counted};

macro_rules! fixed_set_case {
    ($case:ident, $fixture:literal, $expected:literal, $alloc:literal, $free:literal, $string_alloc:literal, $string_free:literal) => {
        mod $case {
            use crate::{assert_asan_clean_and_prints, build, run_counted};

            #[test]
            fn replaced_item_is_released() {
                let built = build($fixture, false);
                let (output, counters) = run_counted(&built);
                assert_eq!(output.exit_code, 0, "stderr:\n{}", output.stderr);
                assert_eq!(output.stdout, $expected);
                assert_eq!(
                    (
                        counters.alloc,
                        counters.free,
                        counters.string_alloc,
                        counters.string_free
                    ),
                    ($alloc, $free, $string_alloc, $string_free),
                    "{counters:?}"
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
    "c-3\nd-4\n",
    0,
    0,
    5,
    2
);
fixed_set_case!(
    fixed_shape_set,
    "v0_3_m9_c6_fixed_set_shape.ynz",
    "c-3\nb-2\n",
    3,
    1,
    4,
    1
);
