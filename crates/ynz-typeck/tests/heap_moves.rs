// v0.3-M9 C1f round 2 (plan #17): which heap-valued expressions a store takes by move.
//
// A shape that owns a recursive union's boxes is copied by bytes, and a byte copy shares its boxes
// with its source; the release (C6) frees each box once. A store that MOVES a value another holder
// can still reach is a double free; a store that deep-copies a value nobody else holds is a clone
// where Rust moves (Golden Rule 6). Typeck's record (`TypedModule::heap_moves`) decides, so these
// tests assert the record directly: no release pass exists yet, so the wrong answer is observable
// only here and in the allocation counts.
//
// One unit (`heap_moves` record: is this expression a move?), two tiers: the not-moved cases guard
// the double free (High); the moved cases guard only an extra clone (Medium).
// Each case is one NAMED test over a small program. A read is named by the text it is written as
// and which occurrence of that text it is (`("holder", 2)` is the second `holder`).

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::check_query;

/// Check `program`, and assert that the `occurrence`-th (1-based) occurrence of `text` is (or is
/// not) in the move record.
fn assert_move(program: &str, text: &str, occurrence: usize, expected: bool) {
    let db = CompilerDb::default();
    let source_file = SourceFile::new(&db, "test.ynz".to_string(), program.to_string());
    let output = (*check_query(&db, source_file)).clone();
    let errors: Vec<String> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, ynz_diagnostics::Severity::Error))
        .map(|diagnostic| format!("{diagnostic:?}"))
        .collect();
    assert!(errors.is_empty(), "program must check clean: {errors:#?}");
    let start = program
        .match_indices(text)
        .nth(occurrence - 1)
        .unwrap_or_else(|| panic!("no occurrence {occurrence} of `{text}` in:\n{program}"))
        .0;
    let found = output
        .typed_module
        .heap_moves
        .spans()
        .any(|(from, to)| from == start && to == start + text.len());
    assert_eq!(
        found, expected,
        "`{text}` (occurrence {occurrence}) moved: expected {expected}, got {found} in:\n{program}"
    );
}

const PRELUDE: &str = "shape Inner {\n  id: Int\n}\nshape Outer {\n  inner: Inner\n  tag: Int\n}\n";

macro_rules! not_moved_cases {
    ($($case:ident: $program:expr, ($text:expr, $occurrence:expr), $bug:literal;)+) => {
        $(
            // bug: typeck's move record marks as moved a value another holder can still reach (double free) | tier: High
            #[doc = $bug]
            #[test]
            fn $case() {
                assert_move(&format!("{PRELUDE}{}", $program), $text, $occurrence, false);
            }
        )+
    };
}

macro_rules! moved_cases {
    ($($case:ident: $program:expr, ($text:expr, $occurrence:expr), $bug:literal;)+) => {
        $(
            // bug: typeck's move record leaves an owned value unmoved, so the store deep-copies it (extra clone, no leak) | tier: Medium
            #[doc = $bug]
            #[test]
            fn $case() {
                assert_move(&format!("{PRELUDE}{}", $program), $text, $occurrence, true);
            }
        )+
    };
}

not_moved_cases! {
    an_explicit_copy_receiver_is_not_moved:
        "function entrypoint() -> Nothing {\n  let inner: Inner = { id: 1 }\n  let again = inner.copy()\n  print(again.id + inner.id)\n}\n",
        ("inner", 3),
        "a `.copy()` receiver treated as consumed (its boxes taken by the copy, a double free when both are released)";

    a_field_read_stored_is_never_a_move:
        "function entrypoint() -> Nothing {\n  let inner: Inner = { id: 1 }\n  let outer: Outer = { inner: inner, tag: 2 }\n  let alias = outer.inner\n  print(alias.id + outer.tag)\n}\n",
        ("outer.inner", 1),
        "a binding made from a field that takes the field's boxes while the field still owns them (double free)";

    a_returned_parameter_is_not_a_move:
        "function make() -> Inner {\n  let inner: Inner = { id: 1 }\n  return inner\n}\nfunction pass(share held: Inner) -> Inner {\n  return held\n}\nfunction entrypoint() -> Nothing {\n  let first = make()\n  let second = pass(first)\n  print(second.id)\n}\n",
        ("held", 2),
        "a returned parameter whose boxes the caller also holds treated as moved (double free)";

    a_returned_field_read_is_not_a_move:
        "function pick(share outer: Outer) -> Inner {\n  return outer.inner\n}\nfunction entrypoint() -> Nothing {\n  print(`x`)\n}\n",
        ("outer.inner", 1),
        "a returned field whose boxes the shape still owns taken by the caller (double free)";

    a_returned_const_is_not_a_move:
        "function make() -> Inner {\n  const inner: Inner = { id: 1 }\n  return inner\n}\nfunction entrypoint() -> Nothing {\n  print(make().id)\n}\n",
        ("inner", 3),
        "a returned `const` taken by move although a `const` stays readable to whatever holds it (double free)";
}

moved_cases! {
    a_binding_moved_into_a_field_is_a_move:
        "function entrypoint() -> Nothing {\n  let inner: Inner = { id: 1 }\n  let outer: Outer = { inner: inner, tag: 2 }\n  print(outer.tag)\n}\n",
        ("inner", 4),
        "a stored binding deep-copied where Rust moves (cost, Rule 6)";

    a_binding_moved_into_a_binding_is_a_move:
        "function entrypoint() -> Nothing {\n  let inner: Inner = { id: 1 }\n  let moved = inner\n  print(moved.id)\n}\n",
        ("inner", 3),
        "a `let` of an owned binding that clones it where Rust moves (cost)";

    a_fresh_literal_stored_is_a_move:
        "function entrypoint() -> Nothing {\n  let outer: Outer = { inner: { id: 1 }, tag: 2 }\n  print(outer.tag)\n}\n",
        ("{ id: 1 }", 1),
        "a literal nobody names cloned when stored (cost)";

    an_explicit_copy_is_fresh_and_is_a_move:
        "function entrypoint() -> Nothing {\n  let inner: Inner = { id: 1 }\n  let again = inner.copy()\n  print(again.id + inner.id)\n}\n",
        ("inner.copy()", 1),
        "a fresh `.copy()` result cloned a second time when stored (cost)";

    a_returned_local_is_a_move:
        "function make() -> Inner {\n  let inner: Inner = { id: 1 }\n  return inner\n}\nfunction entrypoint() -> Nothing {\n  print(make().id)\n}\n",
        ("inner", 3),
        "a returned local cloned where Rust moves (cost)";

    a_call_result_of_make_is_a_move_because_return_copies:
        "function make() -> Inner {\n  let inner: Inner = { id: 1 }\n  return inner\n}\nfunction pass(share held: Inner) -> Inner {\n  return held\n}\nfunction entrypoint() -> Nothing {\n  let first = make()\n  let second = pass(first)\n  print(second.id)\n}\n",
        ("make()", 2),
        "a call result cloned a second time where the callee already returned an owned copy (cost, Rule 6)";

    a_call_result_of_pass_is_a_move_because_return_copies:
        "function make() -> Inner {\n  let inner: Inner = { id: 1 }\n  return inner\n}\nfunction pass(share held: Inner) -> Inner {\n  return held\n}\nfunction entrypoint() -> Nothing {\n  let first = make()\n  let second = pass(first)\n  print(second.id)\n}\n",
        ("pass(first)", 1),
        "a `return` of a parameter copies, so the call result is fresh and a second clone is wasted (cost, Rule 6)";

    a_returned_fresh_literal_is_a_move:
        "function make() -> Inner {\n  return { id: 1 }\n}\nfunction entrypoint() -> Nothing {\n  print(make().id)\n}\n",
        ("{ id: 1 }", 1),
        "a returned literal cloned on return where Rust moves (cost, Rule 6)";

    a_returned_give_parameter_is_a_move:
        "function own(give held: Inner) -> Inner {\n  return held\n}\nfunction entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let second = own(first)\n  print(second.id)\n}\n",
        ("held", 2),
        "a returned `give` parameter cloned where the callee owns it and Rust moves (cost, Rule 6)";
}
