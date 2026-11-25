//! C6: what a non-owned read costs (`IMP-ownership.md` "Non-owned reads").
//!
//! A read of a place someone else owns points at the place when nothing writes the place while
//! the reader is live, and owns a copy otherwise. Copying where Rust borrows costs more than Rust
//! (Golden Rule 6), so the no-write cases pin the allocation count at zero extra copies
//! (`alloc` is the runtime's counted allocations, `array` header and buffer two each); the write
//! cases are the ones where a reader that still points at the replaced or released place would
//! read freed memory or leak a copy. Two tiers over the same fixtures: the balance of every run
//! (each case frees what it allocates, the copy of a loop item or an argument after its iteration
//! or call, the one of a `let` at its scope exit) is High; the copy count of the no-write cases is
//! Medium.
//!
//! Approval condition of the non-owned-read design: EVERY site typeck judges has a no-write case
//! pinned at zero extra copies: plain and state-machine loop, map-entry loop, union loop variable,
//! call argument, UFCS receiver, a call that suspends, `let`, reassignment, `let` of a
//! `maybe<array>` and of a `maybe<map>`, and `return`.

mod support;

use support::{build, run_counted};

/// Run the fixture with the counters on; check exit 0 and the exact stdout.
fn run_fixture(fixture: &str, expected_stdout: &str) -> support::Counters {
    let built = build(fixture, false);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "stderr:\n{}", output.stderr);
    assert_eq!(output.stdout, expected_stdout, "{fixture}: wrong output");
    counters
}

/// One named test per read site of the `borrow read release balance` run unit; each case is a
/// test the runner names.
macro_rules! borrow_read_balance_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal;)+) => { $(
        // bug: a reader left pointing at a replaced or released place reads freed memory, or a copy it owns is never released | tier: High
        #[test]
        fn $case_name() {
            let counters = run_fixture($fixture, $expected_stdout);
            assert!(
                counters.alloc + counters.string_alloc > 0,
                "{}: non-vacuous: the program must allocate: {counters:?}",
                $fixture
            );
            assert_eq!(
                (counters.alloc, counters.string_alloc),
                (counters.free, counters.string_free),
                "{}: every allocation must be freed: {counters:?}",
                $fixture
            );
        }
    )+ };
}

borrow_read_balance_high_case! {
    // case: a loop variable copied per iteration although the body never writes the array
    a_loop_whose_body_writes_nothing_copies_nothing, "v0_3_m9_c6_cost_loop_no_write.ynz", "1\n1\n";
    // case: a loop variable left pointing at an item the body frees, or a per-iteration copy that is never released
    a_loop_that_overwrites_an_item_copies_one_item_per_iteration, "v0_3_m9_c6_cost_loop_write.ynz", "1\n1\n";
    // case: a `share` argument copied although no argument of the call writes its collection
    share_arguments_with_no_lend_of_their_collection_copy_nothing, "v0_3_m9_c6_cost_arg_no_alias.ynz", "3\n";
    // case: a `share` argument that reads an item another argument's `lend` overwrites, read after it was freed
    a_share_argument_read_from_a_lent_collection_is_copied_once_and_released, "v0_3_m9_c6_cost_arg_lend_alias.ynz", "3\n";
    // case: a `let` of a field copied although the field is never written
    a_let_of_a_field_nobody_overwrites_copies_nothing, "v0_3_m9_c6_cost_let_no_write.ynz", "3\n";
    // case: a `let` of a field left pointing at the array the field assignment frees
    a_let_of_a_field_that_is_overwritten_copies_it_once, "v0_3_m9_c6_cost_let_write.ynz", "3\n";
    // case: a `let` of a field copied because its root lives in a loop body, although the reader dies with it
    a_read_that_dies_with_its_roots_scope_copies_nothing, "v0_3_m9_c6_cost_outlive_same_scope.ynz", "3\n3\n";
    // case: a reassignment left pointing at an array the root's scope exit frees, read after it
    a_read_that_outlives_its_roots_scope_is_copied_once, "v0_3_m9_c6_cost_outlive_root.ynz", "3\n";
    // case: a call result copied again where the callee already returned an owned copy
    a_returned_field_is_copied_once_and_the_caller_binds_it_without_a_second_copy, "v0_3_m9_c6_cost_return_once.ynz", "3\n";
    // case: a state-machine loop variable copied because a `wait` is in the body, although nothing writes the array
    a_state_machine_loop_whose_body_writes_nothing_copies_nothing, "v0_3_m9_c6_cost_sm_loop_no_write.ynz", "1\n1\n";
    // case: a state-machine loop variable left pointing at an item the body frees, or a copy never released
    a_state_machine_loop_that_overwrites_an_item_copies_and_releases_one_item_per_iteration, "v0_3_m9_c6_cost_sm_loop_write.ynz", "1\n1\n";
    // case: a map-entry loop variable cloned per iteration (its array value and its string key) although the body never writes the map
    a_map_loop_whose_body_writes_nothing_copies_no_entry, "v0_3_m9_c6_cost_map_loop_no_write.ynz", "2\n1\n";
    // case: a union loop variable copied per iteration (its string variant cloned) although the body never writes the list
    a_union_loop_whose_body_writes_nothing_copies_no_item, "v0_3_m9_c6_cost_union_loop_no_write.ynz", "big-1\n";
    // case: a dotted UFCS receiver (`world.bucket.total(world.owners)`) copied although no argument of the call writes the world
    a_ufcs_receiver_field_nobody_writes_is_not_copied, "v0_3_m9_c6_cost_ufcs_receiver_no_write.ynz", "3\n";
    // case: a dotted UFCS receiver (`world.bucket.total(world.owners)`) read after another argument's `lend` of its root overwrote an element, or a copy never released
    a_ufcs_receiver_field_whose_root_is_lent_is_copied_once_and_released, "v0_3_m9_c6_cost_ufcs_receiver_write.ynz", "3\n";
    // case: a plain-call `share` argument copied although no argument of the call writes its list
    a_shape_list_element_argument_nobody_writes_is_not_copied, "v0_3_m9_c6_cost_call_arg_shape_no_write.ynz", "2\n";
    // case: a plain-call `share` argument that reads an element another argument's `lend` overwrites, read after it was freed
    a_shape_list_element_argument_read_from_a_lent_list_is_copied_once_and_released, "v0_3_m9_c6_cost_call_arg_shape_write.ynz", "2\n";
    // case: a reassignment that views the field leaks the binding's first value, or one that copies leaks the copy
    a_reassignment_from_a_field_nobody_writes_copies_it_once_and_balances, "v0_3_m9_c6_cost_assign_no_write.ynz", "3\n";
    // case: a `let` of a `Maybe<Array>` and of its `.or` copying the payload array although the list is never written
    a_let_of_a_maybe_array_element_nobody_writes_copies_no_payload, "v0_3_m9_c6_cost_maybe_array_let_no_write.ynz", "3\n";
    // case: a `let` of a `Maybe<Map>` and of its `.or` copying the payload map although the list is never written
    a_let_of_a_maybe_map_element_nobody_writes_copies_no_payload, "v0_3_m9_c6_cost_maybe_map_let_no_write.ynz", "1\n";
    // case: an argument staged into a suspending callee's frame copied although nothing writes its place
    an_argument_of_a_suspending_call_nobody_writes_is_staged_without_a_copy, "v0_3_m9_c6_cost_suspending_call_no_write.ynz", "2\n";
}

/// One named test per no-write read site of the `borrow read copy cost` run unit; each case is a
/// test the runner names. `$alloc` is the counted allocations at zero extra copies; `$strings` is
/// the heap strings allocated, for the sites whose copy clones strings the counted allocations do
/// not show.
macro_rules! borrow_read_cost_medium_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal, $alloc:literal, $strings:expr;)+) => { $(
        // bug: a read of a place nothing writes is copied where Rust borrows, so the program allocates more than the zero-copy baseline | tier: Medium
        #[test]
        fn $case_name() {
            let counters = run_fixture($fixture, $expected_stdout);
            assert_eq!(
                counters.alloc, $alloc,
                "{}: expected the zero-copy allocation count: {counters:?}",
                $fixture
            );
            let expected_strings: Option<u64> = $strings;
            if let Some(expected_strings) = expected_strings {
                assert_eq!(
                    counters.string_alloc, expected_strings,
                    "{}: expected the zero-copy heap string count: {counters:?}",
                    $fixture
                );
            }
        }
    )+ };
}

borrow_read_cost_medium_case! {
    // case: a loop variable copied per iteration although the body never writes the array
    loop_without_writes_copies_nothing, "v0_3_m9_c6_cost_loop_no_write.ynz", "1\n1\n", 6, None;
    // case: a `share` argument copied although no argument of the call writes its collection
    share_argument_without_lend_copies_nothing, "v0_3_m9_c6_cost_arg_no_alias.ynz", "3\n", 8, None;
    // case: a `let` of a field copied although the field is never written
    let_of_an_unwritten_field_copies_nothing, "v0_3_m9_c6_cost_let_no_write.ynz", "3\n", 2, None;
    // case: a `let` of a field copied because its root lives in a loop body, although the reader dies with it
    read_dying_with_its_roots_scope_copies_nothing, "v0_3_m9_c6_cost_outlive_same_scope.ynz", "3\n3\n", 4, None;
    // case: a call result copied again where the callee already returned an owned copy
    returned_field_is_not_copied_again_by_the_caller, "v0_3_m9_c6_cost_return_once.ynz", "3\n", 4, None;
    // case: a state-machine loop variable copied because a `wait` is in the body, although nothing writes the array
    state_machine_loop_without_writes_copies_nothing, "v0_3_m9_c6_cost_sm_loop_no_write.ynz", "1\n1\n", 7, None;
    // case: a map-entry loop variable cloned per iteration (its array value and its string key) although the body never writes the map
    map_loop_without_writes_copies_no_entry, "v0_3_m9_c6_cost_map_loop_no_write.ynz", "2\n1\n", 9, Some(2);
    // case: a union loop variable copied per iteration (its string variant cloned) although the body never writes the list
    union_loop_without_writes_copies_no_item, "v0_3_m9_c6_cost_union_loop_no_write.ynz", "big-1\n", 2, Some(2);
    // case: a dotted UFCS receiver (`world.bucket.total(world.owners)`) copied although no argument of the call writes the world
    ufcs_receiver_field_without_writes_copies_nothing, "v0_3_m9_c6_cost_ufcs_receiver_no_write.ynz", "3\n", 7, None;
    // case: a plain-call `share` argument copied although no argument of the call writes its list
    shape_list_element_argument_without_writes_copies_nothing, "v0_3_m9_c6_cost_call_arg_shape_no_write.ynz", "2\n", 6, None;
    // case: a `let` of a `Maybe<Array>` and of its `.or` copying the payload array although the list is never written
    let_of_a_maybe_array_element_without_writes_copies_no_payload, "v0_3_m9_c6_cost_maybe_array_let_no_write.ynz", "3\n", 6, None;
    // case: a `let` of a `Maybe<Map>` and of its `.or` copying the payload map although the list is never written
    let_of_a_maybe_map_element_without_writes_copies_no_payload, "v0_3_m9_c6_cost_maybe_map_let_no_write.ynz", "1\n", 12, None;
    // case: an argument staged into a suspending callee's frame copied although nothing writes its place
    suspending_call_argument_without_writes_is_staged_without_a_copy, "v0_3_m9_c6_cost_suspending_call_no_write.ynz", "2\n", 7, None;
}
