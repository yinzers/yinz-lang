//! C1q: `for (row in matrix) { if (...) { kept.add(row) } }` hands the row over, with no `.copy()`
//! and no row copy, when `matrix` (and every name sharing it) is not read after the loop: Rust's
//! `for row in matrix`. When it is read later, the `.copy()` refusal stands. Patrick's cost bar
//! (2026-11-13): "clone where Rust clones, move where Rust moves".
//!
//! One case per row type (array, string, map, shape stored inline, union, the items of a `fixed`
//! list). Every program hands over only SOME rows (an `if`), so the list's release must free the
//! rows that stayed. Two tiers over the same fixtures: a row left with two owners, freed twice or
//! leaked (balance + AddressSanitizer) is High; a row copied at the store (allocations above the
//! copy-free baseline of the same program whose store never runs) is Medium.

mod support;

use support::{
    assert_asan_clean_and_prints, assert_fixture_balanced_and_asan_clean as assert_balanced,
    assert_refused_with, at_both_optimization_levels, build, run_counted,
};

/// One named test per row type of the `loop row hand-over` run unit; each case is a test the
/// runner names.
macro_rules! handover_integrity_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a loop row handed over to another list is owned twice (double free), freed with its old list, or leaked, or a valid program reading the list after an `if` spawn is refused | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

handover_integrity_high_case! {
    // case: array rows handed over by `kept.add(row)`
    array_rows => { assert_balanced("v0_3_m9_c1q_handover_array.ynz", "2\n6\n"); }
    // case: string rows
    string_rows => { assert_balanced("v0_3_m9_c1q_handover_string.ynz", "2\nA\nC\n"); }
    // case: map rows
    map_rows => { assert_balanced("v0_3_m9_c1q_handover_map.ynz", "2\n4\n"); }
    // case: shape rows stored inline
    shape_rows => { assert_balanced("v0_3_m9_c1q_handover_shape.ynz", "2\nA\nC\n"); }
    // case: union rows
    union_rows => { assert_balanced("v0_3_m9_c1q_handover_union.ynz", "2\nN1\ntiny\n"); }
    // case: the items of a `fixed` list
    fixed_items => { assert_balanced("v0_3_m9_c1q_handover_fixed.ynz", "2\n6\n"); }
    // case: a list that qualifies for the struct-of-arrays layout
    soa_candidate_list => { assert_balanced("v0_3_m9_c1q_handover_soa_list.ynz", "10\n"); }
    // case: the liveness walk behind the `background` give inference stopped at the end of the `if`, so a binding read after the `if` was refused as "given away"
    a_spawn_in_an_if_copies_a_binding_that_is_read_after_the_if => {
        at_both_optimization_levels(|no_optimize| {
            assert_asan_clean_and_prints("v0_3_m9_c1q_spawn_in_if_read_after.ynz", no_optimize, "3\n");
        });
    }
}

// bug: typeck accepts a row handed over while the list is read after the loop, so two lists own one row | tier: High
#[test]
fn a_row_given_away_while_the_list_is_read_after_the_loop_still_needs_copy() {
    assert_refused_with(
        "v0_3_m9_c1q_read_after_refused.ynz",
        &["`row` is one cell of `matrix`, which this loop is walking"],
    );
}

/// One named test per row type of the `loop row hand-over cost` unit; each case is a test the
/// runner names. `$counts` is `(alloc, string_alloc)` of the copy-free run.
macro_rules! handover_cost_medium_case {
    ($($case_name:ident, $fixture:literal, $stdout:literal, $counts:expr;)+) => { $(
        // bug: a loop row is copied at the store where Rust moves it, so the program allocates more than the copy-free baseline | tier: Medium
        #[test]
        fn $case_name() {
            at_both_optimization_levels(|no_optimize| {
                let built = build($fixture, no_optimize);
                let (output, counters) = run_counted(&built);
                assert_eq!(output.exit_code, 0, "stderr:\n{}", output.stderr);
                assert_eq!(output.stdout, $stdout, "{}: wrong output", $fixture);
                assert_eq!(
                    (counters.alloc, counters.string_alloc),
                    $counts,
                    "{} (no_optimize={no_optimize}): a row was copied at the store: {counters:?}",
                    $fixture
                );
            });
        }
    )+ };
}

handover_cost_medium_case! {
    // case: array rows
    array_rows_are_not_copied, "v0_3_m9_c1q_handover_array.ynz", "2\n6\n", (10, 0);
    // case: string rows
    string_rows_are_not_copied, "v0_3_m9_c1q_handover_string.ynz", "2\nA\nC\n", (4, 3);
    // case: map rows
    map_rows_are_not_copied, "v0_3_m9_c1q_handover_map.ynz", "2\n4\n", (19, 5);
    // case: shape rows stored inline
    shape_rows_are_not_copied, "v0_3_m9_c1q_handover_shape.ynz", "2\nA\nC\n", (10, 3);
    // case: union rows
    union_rows_are_not_copied, "v0_3_m9_c1q_handover_union.ynz", "2\nN1\ntiny\n", (4, 2);
    // case: the items of a `fixed` list
    fixed_items_are_not_copied, "v0_3_m9_c1q_handover_fixed.ynz", "2\n6\n", (8, 0);
    // case: a struct-of-arrays candidate list
    soa_candidate_list_rows_are_not_copied, "v0_3_m9_c1q_handover_soa_list.ynz", "10\n", (9, 0);
}
