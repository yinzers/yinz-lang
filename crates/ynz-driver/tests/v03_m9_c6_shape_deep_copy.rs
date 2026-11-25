//! v0.3-M9 C6: a non-moved shape copy owns a copy of every heap field; a moved one owns nothing
//! new. The release helper frees exactly what a copy allocates, so the copy must allocate it.
//!
//! Counted allocations, not strings: an array is two counted allocations (header and buffer).

mod support;

use support::{assert_asan_clean_and_prints, at_both_optimization_levels, build, run_counted};

/// Run the fixture with the counters on and compare its output and the counted allocations (and,
/// when `expected_strings` is given, the heap strings).
fn assert_prints_and_allocates(
    fixture: &str,
    expected_stdout: &str,
    expected_allocations: u64,
    expected_strings: Option<u64>,
) {
    let built = build(fixture, false);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "stderr:\n{}", output.stderr);
    assert_eq!(output.stdout, expected_stdout, "{fixture}: wrong output");
    assert_eq!(
        counters.alloc, expected_allocations,
        "{fixture}: expected {expected_allocations} counted allocations (counters: {counters:?})"
    );
    if let Some(strings) = expected_strings {
        assert_eq!(
            counters.string_alloc, strings,
            "{fixture}: expected {strings} heap strings (counters: {counters:?})"
        );
    }
}

/// One named test per copy path of the `shape copy ownership` run unit.
macro_rules! shape_deep_copy_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal, $allocations:expr, $strings:expr;)+) => { $(
        // bug: a shape or fixed copy shares a heap field with its source, so a write shows in both or two releases free it twice | tier: High
        #[test]
        fn $case_name() {
            // Fewer allocations than expected means the copy aliases the source.
            if let Some(allocations) = $allocations {
                assert_prints_and_allocates($fixture, $expected_stdout, allocations, $strings);
            }
            at_both_optimization_levels(|no_optimize| {
                assert_asan_clean_and_prints($fixture, no_optimize, $expected_stdout);
            });
        }
    )+ };
}

shape_deep_copy_high_case! {
    // case: `shape.copy()` memcpys the struct, so the copy and the original share one array field: a write through one shows in the other
    copy_owns_its_array_field, "v0_3_m9_c6_shape_copy_array_field.ynz", "3\n3\n99\n1\n", Some(4), None;
    // case: a moved shape copied deep anyway still frees every array exactly once (the cost half is `moved_shape_copies_nothing`)
    moved_shape_releases_each_array_once, "v0_3_m9_c6_shape_moved_array_field.ynz", "1\n", None::<u64>, None;
    // case: `fixed<Tag>.copy()` copies the block of cells and shares every heap cell and name in it: the copy must allocate 2 cells and 2 names of its own (4 cells, 7 strings: `a-1`, `b-2`, `none`, a name copy in the twin, and a name copy per item `let`)
    fixed_shape_copy_owns_its_items, "v0_3_m9_c6_fixed_shape_copy.ynz", "a-1\nb-2\n", Some(4), Some(7);
}

// bug: a shape stored at its last use is copied deep anyway, paying one array per move where Rust moves for free | tier: Medium
#[test]
fn moved_shape_copies_nothing() {
    assert_prints_and_allocates("v0_3_m9_c6_shape_moved_array_field.ynz", "1\n", 4, None);
}
