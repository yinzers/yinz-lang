//! v0.3-M9 C6: a non-moved shape copy owns a copy of every heap field; a moved one owns nothing
//! new. The release helper frees exactly what a copy allocates, so the copy must allocate it.
//!
//! High tier (data integrity: two shapes sharing one array free it twice once releases run, and
//! a write through one shows in the other). Counted allocations, not strings: an array is two
//! counted allocations (header and buffer).

mod support;

use support::{assert_asan_clean_and_prints, build, run_counted};

fn assert_prints_and_allocates(fixture: &str, expected_stdout: &str, expected_allocations: u64) {
    let built = build(fixture, false);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "stderr:\n{}", output.stderr);
    assert_eq!(output.stdout, expected_stdout, "{fixture}: wrong output");
    assert_eq!(
        counters.alloc, expected_allocations,
        "{fixture}: expected {expected_allocations} counted allocations (counters: {counters:?})"
    );
}

macro_rules! deep_case {
    ($case:ident, $fixture:literal, $expected:literal, $allocations:literal, $bug:literal) => {
        #[doc = $bug]
        mod $case {
            use crate::{assert_asan_clean_and_prints, assert_prints_and_allocates};

            #[test]
            fn counted_allocations() {
                assert_prints_and_allocates($fixture, $expected, $allocations);
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

deep_case!(
    copy_owns_its_array_field,
    "v0_3_m9_c6_shape_copy_array_field.ynz",
    "3\n3\n99\n1\n",
    4,
    "`shape.copy()` memcpys the struct, so the copy and the original share one array field: a write through one shows in the other, and two releases free it twice (High)"
);

deep_case!(
    moved_shape_copies_nothing,
    "v0_3_m9_c6_shape_moved_array_field.ynz",
    "1\n",
    4,
    "a shape stored at its last use is copied deep anyway, paying one array per move where Rust moves for free (High: Golden Rule 8 cost)"
);
