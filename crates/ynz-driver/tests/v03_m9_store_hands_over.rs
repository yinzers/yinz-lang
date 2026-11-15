//! v0.3-M9 C5a (Patrick, deny and teach): storing a heap value HANDS IT OVER, so reading the
//! source afterwards, or storing a value someone else still holds, is a compile error with a
//! WHAT / WHAT-INSTEAD / WHY text. Tier: High (ownership: before, these programs compiled and
//! gave one array two owners, a double free under any scope-exit release pass).
//!
//! Each fixture is a program that compiled before the rule and is refused now (red on the
//! parent commit: it built and ran). They go through the public compile boundary (`ynz build`).
//! The typeck side of every store form is `ynz-typeck/tests/transfer_record.rs`.
//!
//! One test per refusal path: the store read afterwards (container add, `let`), a parameter the
//! caller keeps (`ParamNeedsGive`), a loop view (`TransferNeedsCopy`).

mod support;

use support::compile;

fn assert_refused_with(fixture_name: &str, fragments: &[&str]) {
    let compiled = compile(fixture_name);
    assert!(
        !compiled.success,
        "{fixture_name}: must be refused, but compiled:\n{}",
        compiled.diagnostics
    );
    for fragment in fragments {
        assert!(
            compiled.diagnostics.contains(fragment),
            "{fixture_name}: diagnostic must contain {fragment:?}; got:\n{}",
            compiled.diagnostics
        );
    }
}

#[test]
fn store_then_read_both_is_refused() {
    // WHY: `o.add(a)` then reading `a` and `o`: a release pass freed `a`'s array under the
    // container (was `store_then_read_both`, accepted as "a store joins, it does not consume").
    assert_refused_with(
        "v0_3_m9_c5a_store_then_read_both.ynz",
        &[
            "`a` was stored in `o` and cannot be used here",
            "store a copy instead: `o.add(a.copy())`",
            "put this line above the store",
            "Every value has one owner",
        ],
    );
}

#[test]
fn let_then_read_both_is_refused() {
    // WHY: `let b = a` was a silent alias; now it hands `a` over.
    assert_refused_with(
        "v0_3_m9_c5a_let_then_read_both.ynz",
        &[
            "`first` was stored in `second` and cannot be used here",
            "store a copy instead: `let second = first.copy()`",
        ],
    );
}

#[test]
fn bg_arg_alias_container_add_is_refused() {
    // WHY: `stash` stores its bare parameter `rows` in `bucket`: the caller keeps `rows`, so the
    // container could not own it too. The green-world lock moved to
    // `bg_arg_container_add_gives_row.ynz` (`give rows`), run by the integration suite.
    assert_refused_with(
        "bg_arg_alias_container_add_red.ynz",
        &[
            "`rows` is a parameter of `stash` that has no ownership word",
            "it is handed to `bucket`",
            "function stash(give rows: array<int>, …)",
        ],
    );
}

#[test]
fn a_map_entry_view_stored_in_an_array_is_refused() {
    // WHY: the loop's `entry` is a view of one slot of `stock`; `saved.add(entry)` put a value
    // `stock` still holds into a second container (was the `m5_p3_sweep_mapentry_array_escape`
    // run; a copy of the view is refused too, so the fix is to store `entry.key` / `entry.value`).
    assert_refused_with(
        "m5_p3_sweep_mapentry_array_escape.ynz",
        &[
            "`entry` is one cell of `stock`, which this loop is walking",
            "it is handed to `saved`",
        ],
    );
}
