//! A `for` loop whose body reassigns the binding it is walking keeps walking the collection it
//! started with. An assignment releases the value it replaces; the loop used to keep reading the
//! released list through the pointer it took before the first pass (heap use-after-free).
//!
//! One case per iterated kind the loop lowers (array, string, map), per row kind that is not a
//! plain `int` list (shape rows), per place the loop walks (a binding, a shape field), and per
//! exit the held collection must be released on (a `return` inside the body, a reassignment that
//! never runs). Each case runs at -O2 and -O0, under AddressSanitizer (a plain run may print the
//! right values from freed memory) and with the alloc counters (the held collection is never
//! freed, or freed twice).

mod support;

use support::assert_fixture_balanced_and_asan_clean;

/// One named test per walked-place class of the `for loop over a reassigned binding` unit; each
/// case is a test the runner names.
macro_rules! loop_reassign_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal;)+) => { $(
        // bug: a `for` loop reads the collection its body just released (heap use-after-free), or the held collection is leaked or freed twice | tier: High
        #[test]
        fn $case_name() {
            assert_fixture_balanced_and_asan_clean($fixture, $expected_stdout);
        }
    )+ };
}

loop_reassign_high_case! {
    // case: the walked binding is an array
    reassign_array, "v0_3_m9_c10_reassign_array.ynz", "3\n9\n1\n";
    // case: the walked binding is a string
    reassign_string, "v0_3_m9_c10_reassign_string.ynz", "3\nXYZ\n";
    // case: the walked binding is a map
    reassign_map, "v0_3_m9_c10_reassign_map.ynz", "6\n1\n";
    // case: the rows are shapes, not plain ints
    reassign_shape_rows, "v0_3_m9_c10_reassign_shape_rows.ynz", "3\nA\nB\nC\n1\n";
    // case: a `return` inside the body must release the held collection
    reassign_early_return, "v0_3_m9_c10_reassign_early_return.ynz", "4\n3\n";
    // case: the reassignment sits on a path that never runs
    reassign_never_runs, "v0_3_m9_c10_reassign_never_runs.ynz", "3\n3\n";
    // case: the walked place is a shape field array (`holder.rows = ...` inside `for (row in holder.rows)`)
    reassign_field_array, "v0_3_m9_c10_reassign_field_array.ynz", "6\n1\n";
    // case: the walked place is a shape field string
    reassign_field_string, "v0_3_m9_c10_reassign_field_string.ynz", "3\nX\n";
    // case: the walked field is reassigned on some passes and the body returns
    reassign_field_some_passes_and_return, "v0_3_m9_c10_reassign_field_some_passes.ynz", "6\n3\n";
    // case: a `while` re-reads the binding each pass (guard)
    reassign_while_over_index, "v0_3_m9_c10_while_reassign.ynz", "3\n16\n3\n";
}
