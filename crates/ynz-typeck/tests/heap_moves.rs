// v0.3-M9 C1f round 2 (plan #17): which heap-valued expressions a store takes by move.
//
// WHY (High tier: memory safety / data integrity): a shape that owns a recursive union's boxes is
// copied by bytes, and a byte copy shares its boxes with its source; the release (C6) frees each
// box once. A store that MOVES a value another holder can still reach is a double free; a store
// that deep-copies a value nobody else holds is a clone where Rust moves (Golden Rule 8). Typeck's
// record (`TypedModule::heap_moves`) decides, so these tests assert the record directly: no release
// pass exists yet, so the wrong answer is observable only here and in the allocation counts.
//
// Each case is one NAMED test over a small program. A read is named by the text it is written as
// and which occurrence of that text it is (`("holder", 2)` is the second `holder`).

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::check_query;

/// Check `program`, and assert for each `(text, occurrence, expected)` that the `occurrence`-th
/// (1-based) occurrence of `text` is (or is not) in the move record.
fn assert_moves(program: &str, expectations: &[(&str, usize, bool)]) {
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
    for (text, occurrence, expected) in expectations {
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
            found, *expected,
            "`{text}` (occurrence {occurrence}) moved: expected {expected}, got {found} in:\n{program}"
        );
    }
}

macro_rules! move_cases {
    ($($case:ident: $program:expr, [$($expectation:expr),+ $(,)?], $bug:literal;)+) => {
        $(
            #[doc = $bug]
            #[test]
            fn $case() {
                assert_moves(&format!("{PRELUDE}{}", $program), &[$($expectation),+]);
            }
        )+
    };
}

const PRELUDE: &str = "shape Inner {\n  id: int\n}\nshape Outer {\n  inner: Inner\n  tag: int\n}\n";

move_cases! {
    a_binding_moved_into_a_field_is_a_move:
        "function entrypoint() -> nothing {\n  let inner: Inner = { id: 1 }\n  let outer: Outer = { inner: inner, tag: 2 }\n  print(outer.tag)\n}\n",
        [("inner", 4, true)],
        "a stored binding deep-copied where Rust moves (cost, Rule 8), or a store whose source is still readable moved (double free, High)";

    a_binding_moved_into_a_binding_is_a_move:
        "function entrypoint() -> nothing {\n  let inner: Inner = { id: 1 }\n  let moved = inner\n  print(moved.id)\n}\n",
        [("inner", 3, true)],
        "a `let` of an owned binding that clones it where Rust moves (cost)";

    a_fresh_literal_stored_is_a_move:
        "function entrypoint() -> nothing {\n  let outer: Outer = { inner: { id: 1 }, tag: 2 }\n  print(outer.tag)\n}\n",
        [("{ id: 1 }", 1, true)],
        "a literal nobody names cloned when stored (cost)";

    an_explicit_copy_is_fresh_and_its_receiver_is_not_moved:
        "function entrypoint() -> nothing {\n  let inner: Inner = { id: 1 }\n  let again = inner.copy()\n  print(again.id + inner.id)\n}\n",
        [("inner.copy()", 1, true), ("inner", 3, false)],
        "a `.copy()` receiver treated as consumed (its boxes taken by the copy, a double free when both are released, High)";

    a_field_read_stored_is_never_a_move:
        "function entrypoint() -> nothing {\n  let inner: Inner = { id: 1 }\n  let outer: Outer = { inner: inner, tag: 2 }\n  let alias = outer.inner\n  print(alias.id + outer.tag)\n}\n",
        [("outer.inner", 1, false)],
        "a binding made from a field that takes the field's boxes while the field still owns them (double free, High)";

    a_returned_local_is_a_move_and_a_returned_parameter_is_not:
        "function make() -> Inner {\n  let inner: Inner = { id: 1 }\n  return inner\n}\nfunction pass(share held: Inner) -> Inner {\n  return held\n}\nfunction entrypoint() -> nothing {\n  let first = make()\n  let second = pass(first)\n  print(second.id)\n}\n",
        [("inner", 3, true), ("held", 2, false)],
        "a returned parameter whose boxes the caller also holds treated as moved (double free, High), or a returned local cloned (cost)";

    a_call_result_is_a_move_only_when_the_callee_returns_a_fresh_value:
        "function make() -> Inner {\n  let inner: Inner = { id: 1 }\n  return inner\n}\nfunction pass(share held: Inner) -> Inner {\n  return held\n}\nfunction entrypoint() -> nothing {\n  let first = make()\n  let second = pass(first)\n  print(second.id)\n}\n",
        [("make()", 2, true), ("pass(first)", 1, false)],
        "a call that returns its argument taken by move, so the caller and the result own one set of boxes (double free, High)";

    a_returned_fresh_literal_is_a_move:
        "function make() -> Inner {\n  return { id: 1 }\n}\nfunction entrypoint() -> nothing {\n  print(make().id)\n}\n",
        [("{ id: 1 }", 1, true)],
        "a returned literal cloned on return where Rust moves (cost, Rule 8)";

    a_returned_field_read_is_not_a_move:
        "function pick(share outer: Outer) -> Inner {\n  return outer.inner\n}\nfunction entrypoint() -> nothing {\n  print(`x`)\n}\n",
        [("outer.inner", 1, false)],
        "a returned field whose boxes the shape still owns taken by the caller (double free, High)";

    a_returned_const_is_not_a_move:
        "function make() -> Inner {\n  const inner: Inner = { id: 1 }\n  return inner\n}\nfunction entrypoint() -> nothing {\n  print(make().id)\n}\n",
        [("inner", 3, false)],
        "a returned `const` taken by move although a `const` stays readable to whatever holds it (double free, High)";

    a_returned_give_parameter_is_a_move:
        "function own(give held: Inner) -> Inner {\n  return held\n}\nfunction entrypoint() -> nothing {\n  let first: Inner = { id: 1 }\n  let second = own(first)\n  print(second.id)\n}\n",
        [("held", 2, true)],
        "a returned `give` parameter cloned where the callee owns it and Rust moves (cost, Rule 8)";
}
