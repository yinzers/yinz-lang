//! v0.3-M9 C10 (plan #17): a `for` loop whose body frees the list it is walking is refused at
//! compile time. An assignment releases the value it replaces; the loop reads its list straight
//! from that value, so the next pass read freed memory (a segfault or abort at -O0 and -O2).
//!
//! Tier: High (memory safety, a refusal that must name the exact write). Refused here, with the
//! text asserted: a write to a prefix of the walked place, an element it walks through, the root
//! binding, and a call that passes the root or a prefix to a function that writes the walked
//! place (directly, through a parameter with no modifier, in the method form, through mutual
//! recursion, from another module). Kept legal, run balanced and ASan-clean at both optimization
//! levels: a callee that only writes an element, only reads, or writes another field, and a
//! write to a field of the loop variable. The exact binding and exact field path the body
//! assigns stay legal and are held by codegen (`v03_m9_c10_loop_reassign`).

mod support;

use support::{assert_fixture_balanced_and_asan_clean, compile};

fn assert_refused(fixture: &str, fragments: &[&str]) {
    let compiled = compile(fixture);
    assert!(
        !compiled.success,
        "{fixture} must be refused, but compiled:\n{}",
        compiled.diagnostics
    );
    for fragment in fragments {
        assert!(
            compiled.diagnostics.contains(fragment),
            "{fixture}: the diagnostic must contain {fragment:?}; got:\n{}",
            compiled.diagnostics
        );
    }
}

#[test]
fn a_write_to_a_prefix_of_the_walked_place_is_refused() {
    // WHY: `outer.inner = ...` releases the `Inner` whose `rows` the loop is walking.
    assert_refused(
        "v0_3_m9_c10_walked_prefix_refused.ynz",
        &[
            "The `for` loop over `outer.inner.rows` assigns `outer.inner`. That frees the list `row` is still being read from.",
            "Copy the list into a local before the loop and walk the copy: `let rowsCopy = outer.inner.rows.copy()`, then `for (row in rowsCopy)`.",
            "once `outer.inner` is assigned, the next pass of `row` would read a list that was already released",
        ],
    );
}

#[test]
fn a_write_to_an_element_the_loop_walks_through_is_refused() {
    // WHY: `cube[0] = ...` releases the list at `cube[0]`, which is the one the loop walks.
    assert_refused(
        "v0_3_m9_c10_walked_element_refused.ynz",
        &[
            "The `for` loop over `cube[0].or(empty)` assigns `cube[0]`. That frees the list `row` is still being read from.",
            "`let cubeCopy = cube[0].or(empty).copy()`, then `for (row in cubeCopy)`",
        ],
    );
}

#[test]
fn a_write_to_the_root_binding_is_refused() {
    // WHY: `holder = ...` releases the whole `Holder`, and the list with it.
    assert_refused(
        "v0_3_m9_c10_walked_root_refused.ynz",
        &[
            "The `for` loop over `holder.rows` assigns `holder`. That frees the list `row` is still being read from.",
            "`let rowsCopy = holder.rows.copy()`, then `for (row in rowsCopy)`",
        ],
    );
}

#[test]
fn a_call_that_writes_the_walked_field_through_its_parameter_is_refused() {
    // WHY: `reset(holder)` assigns `holder.rows` inside `reset`, out of the loop body's sight.
    assert_refused(
        "v0_3_m9_c10_walked_callee_refused.ynz",
        &[
            "The `for` loop over `holder.rows` calls `reset(holder)`, which assigns `holder.rows`. That frees the list `row` is still being read from.",
            "The body can then assign `holder.rows` freely.",
        ],
    );
}

#[test]
fn a_parameter_with_no_modifier_that_the_callee_assigns_counts_as_lend() {
    // WHY: the compiler infers `lend` for a bare parameter the body assigns; the refusal must
    // not depend on the modifier being written.
    assert_refused(
        "v0_3_m9_c10_walked_callee_bare_param_refused.ynz",
        &["calls `reset(holder)`, which assigns `holder.rows`"],
    );
}

#[test]
fn a_method_form_call_is_read_as_the_call_it_is_sugar_for() {
    // WHY: `holder.reset()` is `reset(holder)`.
    assert_refused(
        "v0_3_m9_c10_walked_callee_method_form_refused.ynz",
        &["calls `holder.reset()`, which assigns `holder.rows`"],
    );
}

#[test]
fn passing_a_prefix_of_the_walked_place_to_a_callee_that_assigns_below_it_is_refused() {
    // WHY: `replaceInner(outer)` assigns `outer.inner`, a prefix of `outer.inner.rows`.
    assert_refused(
        "v0_3_m9_c10_walked_callee_prefix_refused.ynz",
        &["calls `replaceInner(outer)`, which assigns `outer.inner`"],
    );
}

#[test]
fn a_write_reached_through_mutually_recursive_callees_is_refused() {
    // WHY: `ping` has no assignment of its own; it writes `holder.rows` through `pong`, which
    // calls `ping` back. The callee summary is a fixpoint over the call graph.
    assert_refused(
        "v0_3_m9_c10_walked_callee_mutual_recursion_refused.ynz",
        &["calls `ping(holder, 3)`, which assigns `holder.rows`"],
    );
}

#[test]
fn a_write_by_a_callee_from_another_module_is_refused() {
    // WHY: the importer sees only the exported signature; it must carry what the body writes.
    assert_refused(
        "v0_3_m9_c10_walked_imported_callee_refused",
        &["calls `reset(holder)`, which assigns `holder.rows`"],
    );
}

macro_rules! legal_case {
    ($case:ident, $fixture:literal, $stdout:literal) => {
        #[test]
        fn $case() {
            assert_fixture_balanced_and_asan_clean($fixture, $stdout);
        }
    };
}

// A callee that assigns an element of the walked list frees an item, not the list (C6 reads).
legal_case!(
    a_callee_that_assigns_only_an_element_stays_legal,
    "v0_3_m9_c10_walked_callee_element_only.ynz",
    "6\n"
);
legal_case!(
    a_callee_that_only_reads_stays_legal,
    "v0_3_m9_c10_walked_callee_reads_only.ynz",
    "15\n"
);
legal_case!(
    a_callee_that_assigns_another_field_stays_legal,
    "v0_3_m9_c10_walked_callee_other_field.ynz",
    "6\nrenamed\n"
);
// `leaf.rows = ...` frees the cell's list; `tree.rows`, the walked one, is untouched.
legal_case!(
    a_write_to_a_field_of_the_loop_variable_stays_legal,
    "v0_3_m9_c10_walked_loop_variable_field.ynz",
    "10\n"
);
