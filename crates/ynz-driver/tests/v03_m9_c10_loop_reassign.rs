//! v0.3-M9 C10: a `for` loop whose body reassigns the binding it is walking keeps walking the
//! collection it started with. Plan #17 made an assignment release the value it replaces; the
//! loop kept reading the released list through the pointer it took before the first pass (heap
//! use-after-free: a segfault or abort in the shipped binary, `heap-use-after-free` under ASan).
//! `main` never released on reassignment, so it ran these programs correctly and leaked.
//!
//! Tier: High (memory safety). One case per iterated kind the loop lowers (array, string, map)
//! and per row kind that is not a plain `int` list (shape rows), plus the exits the held
//! collection must be released on (a `return` inside the body, a reassignment that never runs).
//!
//! Per case, at -O2 and -O0, the bug each test catches:
//! * `asan`: the loop reads the released collection (a plain run may print the right values from
//!   freed memory, so the sanitizer verdict is asserted on its own).
//! * `counts`: the collection the loop held is never freed (allocations above frees), or is freed
//!   twice (frees above allocations).

mod support;

use support::{assert_asan_clean_and_prints, build, run_counted};

fn assert_balanced(fixture: &str, no_optimize: bool, expected_stdout: &str) {
    let built = build(fixture, no_optimize);
    let (output, counters) = run_counted(&built);
    assert_eq!(
        output.exit_code, 0,
        "{fixture} (no_optimize={no_optimize}): stderr:\n{}",
        output.stderr
    );
    assert_eq!(output.stdout, expected_stdout, "{fixture}: wrong output");
    assert_eq!(
        (counters.alloc, counters.string_alloc),
        (counters.free, counters.string_free),
        "{fixture} (no_optimize={no_optimize}): a list or string was leaked or freed twice: {counters:?}"
    );
}

macro_rules! reassign_case {
    ($case:ident, $fixture:literal, $stdout:literal) => {
        mod $case {
            use super::*;

            #[test]
            fn asan_o2() {
                assert_asan_clean_and_prints($fixture, false, $stdout);
            }

            #[test]
            fn asan_o0() {
                assert_asan_clean_and_prints($fixture, true, $stdout);
            }

            #[test]
            fn counts_o2() {
                assert_balanced($fixture, false, $stdout);
            }

            #[test]
            fn counts_o0() {
                assert_balanced($fixture, true, $stdout);
            }
        }
    };
}

reassign_case!(
    array,
    "v0_3_m9_c10_reassign_array.ynz",
    "3\n9\n1\n"
);
reassign_case!(
    string,
    "v0_3_m9_c10_reassign_string.ynz",
    "3\nXYZ\n"
);
reassign_case!(map, "v0_3_m9_c10_reassign_map.ynz", "6\n1\n");
reassign_case!(
    shape_rows,
    "v0_3_m9_c10_reassign_shape_rows.ynz",
    "3\nA\nB\nC\n1\n"
);
reassign_case!(
    early_return,
    "v0_3_m9_c10_reassign_early_return.ynz",
    "4\n3\n"
);
reassign_case!(
    never_runs,
    "v0_3_m9_c10_reassign_never_runs.ynz",
    "3\n3\n"
);
// Guard, green on every sha: a `while` re-reads the binding each pass.
reassign_case!(
    while_over_index,
    "v0_3_m9_c10_while_reassign.ynz",
    "3\n16\n3\n"
);
