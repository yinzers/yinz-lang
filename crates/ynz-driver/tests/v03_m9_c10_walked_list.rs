//! A `for` loop whose body frees the list it is walking is refused at compile time: an
//! assignment releases the value it replaces, and the loop reads its list straight from that
//! value. Refused: a write to a prefix of the walked place, to an element it walks through, to
//! the root binding, and a call that writes the walked place (directly, through a parameter with
//! no modifier, in the method form, through mutual recursion, from another module). Kept legal
//! and run balanced and ASan-clean at both optimization levels: a callee that only writes an
//! element, only reads, or writes another field, and a write to a field of the loop variable. The
//! exact binding and field path the body assigns stay legal and are held by codegen
//! (`v03_m9_c10_loop_reassign`).

mod support;

use support::{assert_fixture_balanced_and_asan_clean, compile};

/// One named test per refused write of the `for loop over a list its body frees` typeck unit;
/// each case is a test the runner names. `$phrase` is the clause naming the exact write.
macro_rules! walked_list_refusal_high_case {
    ($($case_name:ident, $fixture:literal, $phrase:literal;)+) => { $(
        // bug: typeck accepts a `for` loop whose body frees the list the loop is walking, so the next pass reads freed memory | tier: High
        #[test]
        fn $case_name() {
            let compiled = compile($fixture);
            assert!(
                !compiled.success,
                "{} must be refused, but compiled:\n{}",
                $fixture,
                compiled.diagnostics
            );
            assert!(
                compiled.diagnostics.contains($phrase),
                "{}: the diagnostic must name the write {:?}; got:\n{}",
                $fixture,
                $phrase,
                compiled.diagnostics
            );
        }
    )+ };
}

walked_list_refusal_high_case! {
    // case: `outer.inner = ...` releases the `Inner` whose `rows` the loop is walking
    a_write_to_a_prefix_of_the_walked_place_is_refused, "v0_3_m9_c10_walked_prefix_refused.ynz", "The `for` loop over `outer.inner.rows` assigns `outer.inner`. That frees the list `row` is still being read from.";
    // case: `cube[0] = ...` releases the list at `cube[0]`, the one the loop walks
    a_write_to_an_element_the_loop_walks_through_is_refused, "v0_3_m9_c10_walked_element_refused.ynz", "The `for` loop over `cube[0].or(empty)` assigns `cube[0]`. That frees the list `row` is still being read from.";
    // case: `holder = ...` releases the whole `Holder`, and the list with it
    a_write_to_the_root_binding_is_refused, "v0_3_m9_c10_walked_root_refused.ynz", "The `for` loop over `holder.rows` assigns `holder`. That frees the list `row` is still being read from.";
    // case: `reset(holder)` assigns `holder.rows` inside `reset`, out of the loop body's sight
    a_call_that_writes_the_walked_field_through_its_parameter_is_refused, "v0_3_m9_c10_walked_callee_refused.ynz", "calls `reset(holder)`, which assigns `holder.rows`";
    // case: the compiler infers `lend` for a bare parameter the callee assigns; the refusal must not depend on the modifier being written
    a_parameter_with_no_modifier_that_the_callee_assigns_counts_as_lend, "v0_3_m9_c10_walked_callee_bare_param_refused.ynz", "calls `reset(holder)`, which assigns `holder.rows`";
    // case: `holder.reset()` is `reset(holder)`
    a_method_form_call_is_read_as_the_call_it_is_sugar_for, "v0_3_m9_c10_walked_callee_method_form_refused.ynz", "calls `holder.reset()`, which assigns `holder.rows`";
    // case: `replaceInner(outer)` assigns `outer.inner`, a prefix of the walked `outer.inner.rows`
    passing_a_prefix_of_the_walked_place_to_a_callee_that_assigns_below_it_is_refused, "v0_3_m9_c10_walked_callee_prefix_refused.ynz", "calls `replaceInner(outer)`, which assigns `outer.inner`";
    // case: `ping` writes `holder.rows` only through `pong`, which calls `ping` back (callee summary is a fixpoint)
    a_write_reached_through_mutually_recursive_callees_is_refused, "v0_3_m9_c10_walked_callee_mutual_recursion_refused.ynz", "calls `ping(holder, 3)`, which assigns `holder.rows`";
    // case: the importer sees only the exported signature, which must carry what the body writes
    a_write_by_a_callee_from_another_module_is_refused, "v0_3_m9_c10_walked_imported_callee_refused", "calls `reset(holder)`, which assigns `holder.rows`";
}

/// One named test per legal loop of the `for loop over a list its body does not free` typeck
/// unit; each case is a test the runner names.
macro_rules! walked_list_legal_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal;)+) => { $(
        // bug: typeck refuses a loop that never frees the walked list, or the legal loop leaks or reads freed memory | tier: High
        #[test]
        fn $case_name() {
            assert_fixture_balanced_and_asan_clean($fixture, $expected_stdout);
        }
    )+ };
}

walked_list_legal_high_case! {
    // case: a callee that assigns an element frees an item, not the list
    a_callee_that_assigns_only_an_element_stays_legal, "v0_3_m9_c10_walked_callee_element_only.ynz", "6\n";
    // case: a callee that only reads
    a_callee_that_only_reads_stays_legal, "v0_3_m9_c10_walked_callee_reads_only.ynz", "15\n";
    // case: a callee that assigns another field
    a_callee_that_assigns_another_field_stays_legal, "v0_3_m9_c10_walked_callee_other_field.ynz", "6\nrenamed\n";
    // case: `leaf.rows = ...` frees the cell's list; the walked `tree.rows` is untouched
    a_write_to_a_field_of_the_loop_variable_stays_legal, "v0_3_m9_c10_walked_loop_variable_field.ynz", "10\n";
    // case: a write through a `let view = outer.inner` copy changes the copy, not the walked list
    a_callee_that_writes_through_a_view_binding_stays_legal, "v0_3_m9_c10_walked_callee_view_write.ynz", "6\n3\n";
}
