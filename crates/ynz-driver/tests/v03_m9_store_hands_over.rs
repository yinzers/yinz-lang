//! v0.3-M9 C5a (Patrick, deny and teach): storing a heap value HANDS IT OVER, so reading the
//! source afterwards, or storing a value someone else still holds, is a compile error (ownership:
//! before, these programs compiled and gave one array two owners, a double free under any
//! scope-exit release pass).
//!
//! Each fixture is a program that compiled before the rule and is refused now. They go through
//! the public compile boundary (`ynz build`). The typeck side of every store form is
//! `ynz-typeck/tests/transfer_record.rs`.

mod support;

use support::assert_refused_with;

/// One named test per refusal path of the `store hands the value over` typeck unit: the store read
/// afterwards (container add, `let`), a parameter the caller keeps, a loop view.
macro_rules! store_hands_over_high_case {
    ($($case_name:ident, $fixture:literal, $phrases:expr;)+) => { $(
        // bug: typeck accepts a store whose source is read afterwards, or a store of a value someone else still holds, giving one value two owners (a double free) | tier: High
        #[test]
        fn $case_name() {
            assert_refused_with($fixture, $phrases);
        }
    )+ };
}

store_hands_over_high_case! {
    // case: `o.add(a)` then reading `a` and `o`: a release pass freed `a`'s array under the container (accepted as "a store joins, it does not consume")
    store_then_read_both_is_refused, "v0_3_m9_c5a_store_then_read_both.ynz", &["`a` was stored in `o` and cannot be used here"];
    // case: `let b = a` was a silent alias; now it hands `a` over
    let_then_read_both_is_refused, "v0_3_m9_c5a_let_then_read_both.ynz", &["`first` was stored in `second` and cannot be used here"];
    // case: `stash` stores its bare parameter `rows` in `bucket`: the caller keeps `rows`, so the container could not own it too (the green-world lock is `bg_arg_container_add_gives_row.ynz`, run by the integration suite)
    bg_arg_alias_container_add_is_refused, "bg_arg_alias_container_add_red.ynz", &["`rows` is a parameter of `stash` that has no ownership word"];
    // case: the loop's `entry` is a view of one slot of `stock`; `saved.add(entry)` put a value `stock` still holds into a second container (a copy of the view is refused too)
    a_map_entry_view_stored_in_an_array_is_refused, "m5_p3_sweep_mapentry_array_escape.ynz", &["`entry` is one cell of `stock`, which this loop is walking"];
}
