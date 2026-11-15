// v0.3-M9 C1f round 2 (plan #17): which `let` bindings of a place read are a read-only view.
//
// WHY (High tier: memory safety / data integrity): a view points at the place instead of copying
// it, so a view that outlives a write to the place reads a changed or freed value, and a view that
// is written changes the place under its owner. A binding that could be a view but is copied is a
// clone where Rust borrows (Golden Rule 8: a recursive walk that peels one link per call copies the
// whole tail at every step). Typeck's record (`TypedModule::view_bindings`) decides, so these tests
// assert the record directly; the alloc counts and the AddressSanitizer runs are in the driver.
//
// Each case is one NAMED test over a small program. The view is named by its value expression
// (`outer.inner`) and which occurrence of that text it is.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::check_query;

/// Check `program`, and assert for each `(text, occurrence, expected)` that the `let` whose value
/// is the `occurrence`-th (1-based) occurrence of `text` is (or is not) a view.
fn assert_views(program: &str, expectations: &[(&str, usize, bool)]) {
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
            .view_bindings
            .spans()
            .any(|(from, to)| from == start && to == start + text.len());
        assert_eq!(
            found, *expected,
            "`{text}` (occurrence {occurrence}) view: expected {expected}, got {found} in:\n{program}"
        );
    }
}

macro_rules! view_cases {
    ($($case:ident: $program:expr, [$($expectation:expr),+ $(,)?], $bug:literal;)+) => {
        $(
            #[doc = $bug]
            #[test]
            fn $case() {
                assert_views(&format!("{PRELUDE}{}", $program), &[$($expectation),+]);
            }
        )+
    };
}

const PRELUDE: &str = "shape Inner {\n  id: int\n}\nshape Outer {\n  inner: Inner\n  tag: int\n}\nfunction bump(lend outer: Outer) -> nothing {\n  outer.tag = 3\n}\n";

view_cases! {
    a_field_read_from_a_share_parameter_is_a_view:
        "function read(share outer: Outer) -> int {\n  let inner = outer.inner\n  return inner.id + outer.tag\n}\nfunction entrypoint() -> nothing {\n  print(`x`)\n}\n",
        [("outer.inner", 1, true)],
        "a binding of a field of a `share` parameter copied where Rust borrows (O(n^2) cells in a recursive walk, Rule 8)";

    a_field_read_from_a_local_that_is_not_written_is_a_view:
        "function entrypoint() -> nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  print(inner.id + outer.tag)\n}\n",
        [("outer.inner", 1, true)],
        "a local root read but never written treated as written, so every binding of it copies (cost)";

    a_root_field_written_while_the_view_is_live_is_not_a_view:
        "function entrypoint() -> nothing {\n  let first: Inner = { id: 1 }\n  let second: Inner = { id: 9 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  outer.inner = second\n  print(inner.id)\n}\n",
        [("outer.inner", 2, false)],
        "a view read after its place was overwritten reads the new value, where the binding must keep the old (value semantics broken, High)";

    a_root_passed_to_a_lend_parameter_while_the_view_is_live_is_not_a_view:
        "function entrypoint() -> nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  bump(outer)\n  print(inner.id)\n}\n",
        [("outer.inner", 1, false)],
        "a view live across a call that may write its root, read after the write (High)";

    a_root_written_after_the_last_read_of_the_view_is_still_a_view:
        "function entrypoint() -> nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  print(inner.id)\n  outer.tag = 5\n}\n",
        [("outer.inner", 1, true)],
        "a write after the view's last read ends nothing, so refusing the view costs a copy (cost)";

    a_written_view_is_not_a_view:
        "function entrypoint() -> nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  inner.id = 5\n  print(inner.id)\n}\n",
        [("outer.inner", 1, false)],
        "a write through a view that changes the place under its owner (High)";

    a_returned_view_is_not_a_view:
        "function pick(share outer: Outer) -> Inner {\n  let inner = outer.inner\n  return inner\n}\nfunction entrypoint() -> nothing {\n  print(`x`)\n}\n",
        [("outer.inner", 1, false)],
        "a view that leaves the function pointing into a place the caller still holds (High)";

    an_annotated_binding_is_not_a_view:
        "function read(share outer: Outer) -> int {\n  let inner: Inner = outer.inner\n  return inner.id\n}\nfunction entrypoint() -> nothing {\n  print(`x`)\n}\n",
        [("outer.inner", 1, false)],
        "an annotated binding can wrap the value into a union slot, which a view cannot be";
}
