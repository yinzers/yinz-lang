//! v0.3-M9 C1q: `for (row in matrix) { if (...) { kept.add(row) } }` hands the row over, with no
//! `.copy()` and no row copy, when `matrix` (and every name sharing it) is not read after the
//! loop: Rust's `for row in matrix`. When it is read later, the `.copy()` refusal stands.
//! Patrick's cost bar (2026-11-13): "clone where Rust clones, move where Rust moves".
//!
//! Tier: High (ownership and data integrity: a row owned by two lists is a double free, and a row
//! copied where Rust moves is the cost Chief ruled out).
//!
//! One case per row type (array, string, map, shape stored inline, union, the items of a `fixed`
//! list). Every program hands over only SOME rows (an `if`), so the list's release must free the
//! rows that stayed. Per case the bug each test catches:
//!
//! * `counts`: a row copied at the store (allocations above the copy-free baseline), a row left
//!   with two owners (frees below allocations), a row freed twice. The expected counts are the
//!   baseline of the same program whose store never runs (`seen == 99`), where nothing is copied.
//! * `asan`: a blank cell the release walk frees, or a handed-over row freed with its list.
//!
//! Each runs at O2 and at O0.

mod support;

use support::{assert_asan_clean_and_prints, build, compile, run_counted};

/// `(alloc, free, string_alloc, string_free)` of a copy-free run.
type Counts = (u64, u64, u64, u64);

fn assert_counts(fixture: &str, no_optimize: bool, expected_stdout: &str, expected: Counts) {
    let built = build(fixture, no_optimize);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "stderr:\n{}", output.stderr);
    assert_eq!(output.stdout, expected_stdout, "{fixture}: wrong output");
    assert_eq!(
        (
            counters.alloc,
            counters.free,
            counters.string_alloc,
            counters.string_free
        ),
        expected,
        "{fixture} (no_optimize={no_optimize}): a row was copied, shared or freed twice: {counters:?}"
    );
}

macro_rules! handover_case {
    ($case:ident, $fixture:literal, $stdout:literal, $counts:expr) => {
        mod $case {
            use super::*;

            #[test]
            fn counts_o2() {
                assert_counts($fixture, false, $stdout, $counts);
            }

            #[test]
            fn counts_o0() {
                assert_counts($fixture, true, $stdout, $counts);
            }

            #[test]
            fn asan_o2() {
                assert_asan_clean_and_prints($fixture, false, $stdout);
            }

            #[test]
            fn asan_o0() {
                assert_asan_clean_and_prints($fixture, true, $stdout);
            }
        }
    };
}

handover_case!(
    array_rows,
    "v0_3_m9_c1q_handover_array.ynz",
    "2\n6\n",
    (10, 10, 0, 0)
);
handover_case!(
    string_rows,
    "v0_3_m9_c1q_handover_string.ynz",
    "2\nA\nC\n",
    (4, 4, 3, 3)
);
handover_case!(
    map_rows,
    "v0_3_m9_c1q_handover_map.ynz",
    "2\n4\n",
    (19, 19, 5, 5)
);
handover_case!(
    shape_rows,
    "v0_3_m9_c1q_handover_shape.ynz",
    "2\nA\nC\n",
    (10, 10, 3, 3)
);
handover_case!(
    union_rows,
    "v0_3_m9_c1q_handover_union.ynz",
    "2\nN1\ntiny\n",
    (4, 4, 2, 2)
);
handover_case!(
    fixed_items,
    "v0_3_m9_c1q_handover_fixed.ynz",
    "2\n6\n",
    (8, 8, 0, 0)
);
handover_case!(
    soa_candidate_list,
    "v0_3_m9_c1q_handover_soa_list.ynz",
    "10\n",
    (9, 9, 0, 0)
);

#[test]
fn a_row_given_away_while_the_list_is_read_after_the_loop_still_needs_copy() {
    // WHY: the hand-over must not widen: reading `matrix` after the loop leaves it holding the
    // rows, so handing one over is a double owner (High). The existing refusal, all three parts.
    let compiled = compile("v0_3_m9_c1q_read_after_refused.ynz");
    assert!(
        !compiled.success,
        "must be refused, but compiled:\n{}",
        compiled.diagnostics
    );
    for fragment in [
        "`row` is one cell of `matrix`, which this loop is walking",
        "Give away a copy instead: `row.copy()`",
        "Whoever receives a `Array<Int>` will eventually free it",
    ] {
        assert!(
            compiled.diagnostics.contains(fragment),
            "diagnostic must contain {fragment:?}; got:\n{}",
            compiled.diagnostics
        );
    }
}

#[test]
fn a_spawn_in_an_if_copies_a_binding_that_is_read_after_the_if() {
    // WHY (Medium): the liveness walk behind the `background` give inference stopped at the end
    // of the `if` block, inferred `Give`, and the program was refused as "given away" at the read
    // after the `if`. Before the fix: `rows` was already given away and cannot be used here.
    assert_asan_clean_and_prints("v0_3_m9_c1q_spawn_in_if_read_after.ynz", false, "3\n");
    assert_asan_clean_and_prints("v0_3_m9_c1q_spawn_in_if_read_after.ynz", true, "3\n");
}
