//! v0.3-M9 C6 round 2: what a non-owned read costs (`IMP-ownership.md` "Non-owned reads").
//!
//! A read of a place someone else owns points at the place when nothing writes the place while
//! the reader is live, and owns a copy otherwise. Copying where Rust borrows costs more than Rust
//! (Golden Rule 8), so each pair below pins the count both ways: no write, no copy; a write, one
//! copy. `alloc` and `free` are the runtime's counted allocations (`array` header and buffer are
//! two each); the copy of a loop item or an argument is released after its iteration or call, the
//! one of a `let` is held until C7's scope exit.
//!
//! Medium tier (cost of an endpoint-level contract); the bug each case catches is its `$bug`.

mod support;

use support::{build, run_counted};

fn assert_cost(fixture: &str, expected_stdout: &str, alloc: u64, free: u64) {
    let built = build(fixture, false);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "stderr:\n{}", output.stderr);
    assert_eq!(output.stdout, expected_stdout, "{fixture}: wrong output");
    assert_eq!(
        (counters.alloc, counters.free),
        (alloc, free),
        "{fixture}: expected {alloc} allocations and {free} frees (counters: {counters:?})"
    );
}

macro_rules! cost_case {
    ($case:ident, $fixture:literal, $expected:literal, $alloc:literal, $free:literal, $bug:literal) => {
        #[doc = $bug]
        #[test]
        fn $case() {
            assert_cost($fixture, $expected, $alloc, $free);
        }
    };
}

cost_case!(
    a_loop_whose_body_writes_nothing_copies_nothing,
    "v0_3_m9_c6_cost_loop_no_write.ynz",
    "1\n1\n",
    6,
    0,
    "a loop variable copied per iteration although the body never writes the array (cost, Rule 8)"
);
cost_case!(
    a_loop_that_overwrites_an_item_copies_one_item_per_iteration,
    "v0_3_m9_c6_cost_loop_write.ynz",
    "1\n1\n",
    14,
    8,
    "a loop variable left pointing at an item the body frees, or a per-iteration copy that is never released (High)"
);
cost_case!(
    share_arguments_with_no_lend_of_their_collection_copy_nothing,
    "v0_3_m9_c6_cost_arg_no_alias.ynz",
    "3\n",
    8,
    0,
    "a `share` argument copied although no argument of the call writes its collection (cost, Rule 8)"
);
cost_case!(
    a_share_argument_read_from_a_lent_collection_is_copied_once_and_released,
    "v0_3_m9_c6_cost_arg_lend_alias.ynz",
    "3\n",
    10,
    4,
    "a `share` argument that reads an item another argument's `lend` overwrites, read after it was freed (High)"
);
cost_case!(
    a_let_of_a_field_nobody_overwrites_copies_nothing,
    "v0_3_m9_c6_cost_let_no_write.ynz",
    "3\n",
    2,
    0,
    "a `let` of a field copied although the field is never written (cost, Rule 8)"
);
cost_case!(
    a_let_of_a_field_that_is_overwritten_copies_it_once,
    "v0_3_m9_c6_cost_let_write.ynz",
    "3\n",
    6,
    2,
    "a `let` of a field left pointing at the array the field assignment frees (High)"
);
cost_case!(
    a_returned_field_is_copied_once_and_the_caller_binds_it_without_a_second_copy,
    "v0_3_m9_c6_cost_return_once.ynz",
    "3\n",
    4,
    0,
    "a call result copied again where the callee already returned an owned copy (cost, Rule 8)"
);
