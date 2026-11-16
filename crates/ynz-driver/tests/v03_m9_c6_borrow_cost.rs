//! v0.3-M9 C6: what a non-owned read costs (`IMP-ownership.md` "Non-owned reads").
//!
//! A read of a place someone else owns points at the place when nothing writes the place while
//! the reader is live, and owns a copy otherwise. Copying where Rust borrows costs more than Rust
//! (Golden Rule 8), so each pair below pins the count both ways: no write, no copy; a write, one
//! copy. `alloc` and `free` are the runtime's counted allocations (`array` header and buffer are
//! two each); the copy of a loop item or an argument is released after its iteration or call, the
//! one of a `let` is released at its scope exit (C7 slice 1), so the frees pinned below include the
//! program's own locals and only the allocation count pins the copy cost. A `let` copy of a place
//! is a copy owner (C7 slice 2b), released at its scope exit, so every case here frees what it
//! allocates (a state-machine function releases nothing until C8).
//!
//! Approval condition of the non-owned-read design: EVERY site typeck judges has a no-write case
//! pinned at zero extra copies: plain and state-machine loop, map-entry loop, union loop variable,
//! call argument, UFCS receiver, a call that suspends, `let`, reassignment, `let` of a
//! `maybe<array>` and of a `maybe<map>`, and `return` (one copy at the `return`, none at the
//! caller). A write case pins the one copy where one is owed.
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
    6,
    "a loop variable copied per iteration although the body never writes the array (cost, Rule 8)"
);
cost_case!(
    a_loop_that_overwrites_an_item_copies_one_item_per_iteration,
    "v0_3_m9_c6_cost_loop_write.ynz",
    "1\n1\n",
    14,
    14,
    "a loop variable left pointing at an item the body frees, or a per-iteration copy that is never released (High)"
);
cost_case!(
    share_arguments_with_no_lend_of_their_collection_copy_nothing,
    "v0_3_m9_c6_cost_arg_no_alias.ynz",
    "3\n",
    8,
    8,
    "a `share` argument copied although no argument of the call writes its collection (cost, Rule 8)"
);
cost_case!(
    a_share_argument_read_from_a_lent_collection_is_copied_once_and_released,
    "v0_3_m9_c6_cost_arg_lend_alias.ynz",
    "3\n",
    10,
    10,
    "a `share` argument that reads an item another argument's `lend` overwrites, read after it was freed (High)"
);
cost_case!(
    a_let_of_a_field_nobody_overwrites_copies_nothing,
    "v0_3_m9_c6_cost_let_no_write.ynz",
    "3\n",
    2,
    2,
    "a `let` of a field copied although the field is never written (cost, Rule 8)"
);
cost_case!(
    a_let_of_a_field_that_is_overwritten_copies_it_once,
    "v0_3_m9_c6_cost_let_write.ynz",
    "3\n",
    6,
    6,
    "a `let` of a field left pointing at the array the field assignment frees (High)"
);
cost_case!(
    a_read_that_dies_with_its_roots_scope_copies_nothing,
    "v0_3_m9_c6_cost_outlive_same_scope.ynz",
    "3\n3\n",
    4,
    4,
    "a `let` of a field copied because its root lives in a loop body, although the reader dies with it (cost, Rule 8)"
);
cost_case!(
    a_read_that_outlives_its_roots_scope_is_copied_once,
    "v0_3_m9_c6_cost_outlive_root.ynz",
    "3\n",
    6,
    6,
    "a reassignment left pointing at an array the root's scope exit frees, read after it (High)"
);
cost_case!(
    a_returned_field_is_copied_once_and_the_caller_binds_it_without_a_second_copy,
    "v0_3_m9_c6_cost_return_once.ynz",
    "3\n",
    4,
    4,
    "a call result copied again where the callee already returned an owned copy (cost, Rule 8)"
);
cost_case!(
    a_state_machine_loop_whose_body_writes_nothing_copies_nothing,
    "v0_3_m9_c6_cost_sm_loop_no_write.ynz",
    "1\n1\n",
    7,
    7,
    "a state-machine loop variable copied because a `wait` is in the body, although nothing writes the array (cost, Rule 8)"
);
cost_case!(
    a_state_machine_loop_that_overwrites_an_item_copies_and_releases_one_item_per_iteration,
    "v0_3_m9_c6_cost_sm_loop_write.ynz",
    "1\n1\n",
    15,
    15,
    "a state-machine loop variable left pointing at an item the body frees, or a copy never released (High)"
);

/// As [`assert_cost`], and the heap strings allocated and released: a copied string-keyed entry or
/// union variant clones its strings, which the counted allocations do not show.
fn assert_cost_with_strings(
    fixture: &str,
    expected_stdout: &str,
    cells: (u64, u64),
    strings: (u64, u64),
) {
    let built = build(fixture, false);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "stderr:\n{}", output.stderr);
    assert_eq!(output.stdout, expected_stdout, "{fixture}: wrong output");
    assert_eq!(
        (counters.alloc, counters.free),
        cells,
        "{fixture}: expected {cells:?} counted (allocations, frees) (counters: {counters:?})"
    );
    assert_eq!(
        (counters.string_alloc, counters.string_free),
        strings,
        "{fixture}: expected {strings:?} heap (allocated, released) strings (counters: {counters:?})"
    );
}

macro_rules! string_cost_case {
    ($case:ident, $fixture:literal, $expected:literal, $cells:expr, $strings:expr, $bug:literal) => {
        #[doc = $bug]
        #[test]
        fn $case() {
            assert_cost_with_strings($fixture, $expected, $cells, $strings);
        }
    };
}

// The no-write case of every other site typeck judges, each at zero extra copies.
string_cost_case!(
    a_map_loop_whose_body_writes_nothing_copies_no_entry,
    "v0_3_m9_c6_cost_map_loop_no_write.ynz",
    "2\n1\n",
    (9, 9),
    (2, 2),
    "a map-entry loop variable cloned per iteration (its array value and its string key) although the body never writes the map (cost, Rule 8)"
);
string_cost_case!(
    a_union_loop_whose_body_writes_nothing_copies_no_item,
    "v0_3_m9_c6_cost_union_loop_no_write.ynz",
    "big-1\n",
    (2, 2),
    (2, 2),
    "a union loop variable copied per iteration (its string variant cloned) although the body never writes the list (cost, Rule 8)"
);
cost_case!(
    a_ufcs_receiver_field_nobody_writes_is_not_copied,
    "v0_3_m9_c6_cost_ufcs_receiver_no_write.ynz",
    "3\n",
    7,
    7,
    "a dotted UFCS receiver (`world.bucket.total(world.owners)`) copied although no argument of the call writes the world (cost, Rule 8)"
);
cost_case!(
    a_ufcs_receiver_field_whose_root_is_lent_is_copied_once_and_released,
    "v0_3_m9_c6_cost_ufcs_receiver_write.ynz",
    "3\n",
    11,
    11,
    "a dotted UFCS receiver (`world.bucket.total(world.owners)`) read after another argument's `lend` of its root overwrote an element, or a copy never released (High)"
);
cost_case!(
    a_shape_list_element_argument_nobody_writes_is_not_copied,
    "v0_3_m9_c6_cost_call_arg_shape_no_write.ynz",
    "2\n",
    6,
    6,
    "a plain-call `share` argument copied although no argument of the call writes its list (cost, Rule 8)"
);
cost_case!(
    a_shape_list_element_argument_read_from_a_lent_list_is_copied_once_and_released,
    "v0_3_m9_c6_cost_call_arg_shape_write.ynz",
    "2\n",
    10,
    10,
    "a plain-call `share` argument that reads an element another argument's `lend` overwrites, read after it was freed (High)"
);
cost_case!(
    a_reassignment_from_a_field_nobody_writes_copies_it_once_and_balances,
    "v0_3_m9_c6_cost_assign_no_write.ynz",
    "3\n",
    6,
    6,
    "a reassignment that views the field leaks the binding's first value, or one that copies leaks the copy (High)"
);
cost_case!(
    a_let_of_a_maybe_array_element_nobody_writes_copies_no_payload,
    "v0_3_m9_c6_cost_maybe_array_let_no_write.ynz",
    "3\n",
    6,
    6,
    "a `let` of a `maybe<array>` and of its `.or` copying the payload array although the list is never written (cost, Rule 8)"
);
cost_case!(
    a_let_of_a_maybe_map_element_nobody_writes_copies_no_payload,
    "v0_3_m9_c6_cost_maybe_map_let_no_write.ynz",
    "1\n",
    12,
    12,
    "a `let` of a `maybe<map>` and of its `.or` copying the payload map although the list is never written (cost, Rule 8)"
);
cost_case!(
    an_argument_of_a_suspending_call_nobody_writes_is_staged_without_a_copy,
    "v0_3_m9_c6_cost_suspending_call_no_write.ynz",
    "2\n",
    7,
    7,
    "an argument staged into a suspending callee's frame copied although nothing writes its place (cost, Rule 8)"
);
