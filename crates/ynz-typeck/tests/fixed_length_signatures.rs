//! v0.3-M9 C1h (#433): the lengths the type checker works out for `Fixed` in function signatures.
//! High tier (data integrity across a call): each test names the bug it catches.

use ynz_diagnostics::{DiagnosticKind, Severity};
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, FixedLength, Type};

fn check(source: &str) -> std::sync::Arc<ynz_typeck::CheckOutput> {
    let db = CompilerDb::default();
    let file = SourceFile::new(&db, "fixed.ynz".to_string(), source.to_string());
    check_query(&db, file)
}

fn errors(output: &ynz_typeck::CheckOutput) -> Vec<&ynz_diagnostics::Diagnostic> {
    output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, Severity::Error))
        .collect()
}

fn list_of(count: usize) -> Type {
    Type::BuiltinFixed {
        elem: Box::new(Type::Int),
        size: FixedLength::Known(count),
    }
}

// Bug: a function that returns another function's list got no length when it was declared before
// the function it calls (one pass over the bodies cannot know the callee's length yet).
#[test]
fn a_returned_length_flows_up_a_chain_declared_in_any_order() {
    let output = check(
        "function third() -> Fixed<Int> { return second() }\n\
         function second() -> Fixed<Int> { return first() }\n\
         function first() -> Fixed<Int> {\n\
           let xs: Fixed<Int> = [1, 2, 3]\n\
           return xs\n\
         }\n\
         function entrypoint() -> Nothing { print(third().count()) }",
    );
    assert!(errors(&output).is_empty(), "{:#?}", errors(&output));
    let worked_out = &output.typed_module.inferred_return_types;
    for function in ["first", "second", "third"] {
        assert_eq!(worked_out.get(function), Some(&list_of(3)), "{function}");
    }
}

// Bug: a function returning its own unsized parameter lost the length at the call, so the caller
// read a list of unknown length (and the specialised version was never requested).
#[test]
fn a_parameter_returned_as_it_came_takes_the_length_the_caller_passed() {
    let output = check(
        "function same(give xs: Fixed<Int>) -> Fixed<Int> { return xs }\n\
         function entrypoint() -> Nothing {\n\
           let xs: Fixed<Int> = [1, 2]\n\
           let back = same(xs)\n\
           print(back.count())\n\
         }",
    );
    assert!(errors(&output).is_empty(), "{:#?}", errors(&output));
    let versions: Vec<(String, Vec<Option<usize>>)> = output
        .typed_module
        .fixed_instances
        .instances
        .iter()
        .map(|version| (version.function.clone(), version.lengths.clone()))
        .collect();
    assert_eq!(versions, vec![("same".to_string(), vec![Some(2)])]);
}

// Bug: two functions that only return each other's list made the length loop forever or settle on
// a made-up one; the refusal must name both.
#[test]
fn functions_that_only_return_each_other_have_no_length_to_work_out() {
    let output = check(
        "function ping() -> Fixed<Int> { return pong() }\n\
         function pong() -> Fixed<Int> { return ping() }\n\
         function entrypoint() -> Nothing { print(1) }",
    );
    let unknown: Vec<_> = errors(&output)
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == Some(DiagnosticKind::FixedReturnLengthUnknown))
        .collect();
    assert_eq!(unknown.len(), 2, "{:#?}", errors(&output));
}

// Bug: the field rule let a length through anywhere inside a field's type (`Maybe<Fixed<Int, 2>>`,
// `Array<Fixed<Int, 2>>`), where nothing stores the list by value, or refused the whole field type
// that is allowed.
#[test]
fn a_length_is_allowed_on_the_whole_type_of_a_field_and_nowhere_deeper() {
    let whole = check(
        "shape Bag { items: Fixed<Int, 3> }\n\
         function entrypoint() -> Nothing { print(1) }",
    );
    assert!(errors(&whole).is_empty(), "{:#?}", errors(&whole));
    for nested in ["Maybe<Fixed<Int, 2>>", "Array<Fixed<Int, 2>>"] {
        let output = check(&format!(
            "shape Bag {{ items: {nested} }}\nfunction entrypoint() -> Nothing {{ print(1) }}"
        ));
        let refused: Vec<_> = errors(&output)
            .into_iter()
            .filter(|diagnostic| diagnostic.kind == Some(DiagnosticKind::FixedLengthNotAllowedHere))
            .collect();
        assert_eq!(refused.len(), 1, "{nested}: {:#?}", errors(&output));
    }
}

// Bug: a field of an inline shape written `Fixed<T>` (no length) was accepted and read count 0:
// the field rules covered only named shapes (High: data integrity).
#[test]
fn an_inline_shape_field_without_a_length_is_refused_like_a_named_one() {
    for written in [
        "function entrypoint() -> Nothing {\n\
           let point: { trail: Fixed<Int> } = { trail: [4, 5, 6] }\n\
           print(point.trail.count())\n\
         }",
        "function show(share point: { trail: Fixed<Int> }) -> Nothing { print(1) }\n\
         function entrypoint() -> Nothing { print(2) }",
        "function make() -> { trail: Fixed<Int> } { return { trail: [1] } }\n\
         function entrypoint() -> Nothing { print(2) }",
        "shape Holder { inner: { trail: Fixed<Int> } }\n\
         function entrypoint() -> Nothing { print(2) }",
    ] {
        let output = check(written);
        let refused = errors(&output)
            .into_iter()
            .filter(|diagnostic| diagnostic.kind == Some(DiagnosticKind::FixedFieldLengthMissing))
            .count();
        assert_eq!(refused, 1, "{written}\n{:#?}", errors(&output));
    }
}

fn task_function_refusals(source: &str) -> usize {
    errors(&check(source))
        .into_iter()
        .filter(|diagnostic| {
            diagnostic.kind == Some(DiagnosticKind::FixedLengthMissingInTaskFunction)
        })
        .count()
}

// Bug: a function built once (it waits) that takes an unsized `Fixed` has no length for the
// parameter. The refusal must fire exactly where the body asks for the length, in each way it can
// (High: data integrity; before the refusal each of these stopped the compiler with an internal
// error or released with no count).
#[test]
fn a_function_built_once_is_refused_an_unsized_list_it_asks_the_length_of() {
    let waiting_with = |parameter: &str, body: &str, extra: &str| {
        format!(
            "{extra}\nfunction show({parameter}) -> Int {{\n  wait sleep(1)\n{body}\n  return 1\n}}\n\
             function entrypoint() -> Nothing {{\n  let done = wait show([1, 2, 3])\n  print(done)\n}}"
        )
    };
    let refused_bodies = [
        ("reads count", "  let n = xs.count()"),
        ("iterates", "  for (x in xs) {\n    print(x)\n  }"),
        ("indexes", "  let first = xs[0].or(0)"),
        ("calls get", "  let first = xs.get(0).or(0)"),
        ("returns it", "  let back = xs"),
    ];
    for (branch, body) in refused_bodies {
        let source = waiting_with("share xs: Fixed<Int>", body, "");
        assert_eq!(task_function_refusals(&source), 1, "{branch}\n{source}");
    }
    // handed to a function that needs the length
    let source = waiting_with(
        "share xs: Fixed<Int>",
        "  let n = inner(xs)",
        "function inner(share xs: Fixed<Int>) -> Int { return xs.count() }",
    );
    assert_eq!(
        task_function_refusals(&source),
        1,
        "hands it to a callee that needs it\n{source}"
    );
    // owned, with items that must be released
    let source = waiting_with("give xs: Fixed<String>", "", "");
    let source = source.replace("show([1, 2, 3])", "show([`a`, `b`])");
    assert_eq!(
        task_function_refusals(&source),
        1,
        "gives it away with heap items\n{source}"
    );
    // handed to a task: the task's copy is sized from the length here
    let source = waiting_with(
        "share xs: Fixed<Int>",
        "  let task = background inner(xs)",
        "function inner(share xs: Fixed<Int>) -> Int {\n  wait sleep(1)\n  return 1\n}",
    );
    assert_eq!(
        task_function_refusals(&source),
        1,
        "spawns a task with it\n{source}"
    );
}

// Bug (the converse): the refusal fired on functions that only receive the list (the C6
// spawn-copy guards pass a `Fixed<String>` to a callee that waits and never reads it), which
// compiled and ran correctly before.
#[test]
fn a_function_built_once_that_never_asks_for_the_length_is_not_refused() {
    let sources = [
        "function show(value: Fixed<String>) -> Nothing {\n  wait sleep(1)\n  print(777)\n}\n\
         function entrypoint() -> Nothing {\n  let value: Fixed<String> = [`p`, `q`]\n  background show(value)\n  wait sleep(5)\n}",
        "function inner(share xs: Fixed<Int>) -> Int {\n  wait sleep(1)\n  return 1\n}\n\
         function show(share xs: Fixed<Int>) -> Int {\n  wait sleep(1)\n  return inner(xs)\n}\n\
         function entrypoint() -> Nothing {\n  let done = wait show([1, 2])\n  print(done)\n}",
        "function show(give xs: Fixed<Int>) -> Int {\n  wait sleep(1)\n  return 1\n}\n\
         function entrypoint() -> Nothing {\n  let done = wait show([1, 2])\n  print(done)\n}",
    ];
    for source in sources {
        assert_eq!(task_function_refusals(source), 0, "{source}");
    }
}

// Bug: a received `Fixed<String>` task result, bound to a local released after a later `wait`, was
// released from cells in a dead resume-function stack frame (stack-use-after-return); it must be
// refused with a diagnostic instead (High: data integrity).
#[test]
fn a_received_fixed_of_strings_cannot_cross_a_later_wait() {
    let output = check(
        "function names() -> Fixed<String> {\n\
           wait sleep(1)\n\
           let rows: Fixed<String> = [`a`, `b`]\n\
           return rows\n\
         }\n\
         function entrypoint() -> Nothing {\n\
           let task = background names()\n\
           wait sleep(20)\n\
           let received = task.receive()\n\
           wait sleep(1)\n\
         }",
    );
    let refusals = errors(&output);
    assert_eq!(refusals.len(), 1, "{refusals:#?}");
    assert!(
        refusals[0].what.contains("cannot yet cross a `wait`"),
        "{:#?}",
        refusals[0]
    );
}

// A received list of plain values owns nothing to release, so a later `wait` after its last use
// stays allowed.
#[test]
fn a_received_fixed_of_ints_may_be_followed_by_a_wait() {
    let output = check(
        "function numbers() -> Fixed<Int> {\n\
           wait sleep(1)\n\
           let list: Fixed<Int> = [1, 2]\n\
           return list\n\
         }\n\
         function entrypoint() -> Nothing {\n\
           let task = background numbers()\n\
           wait sleep(20)\n\
           let received = task.receive()\n\
           wait sleep(1)\n\
         }",
    );
    assert!(errors(&output).is_empty(), "{:#?}", errors(&output));
}

fn refusals_of(source: &str, kind: DiagnosticKind) -> usize {
    errors(&check(source))
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == Some(kind.clone()))
        .count()
}

// Bug: the export refusal was pinned for a returned list only, so an exported `Fixed` PARAMETER
// without its length (which a module checked alone cannot size) could compile unnoticed. The rows
// are the refusals the Chief Architect's ruling lists, each a complete program with the number of
// refusals of its kind: (a) an exported unsized parameter, (b) an exported unsized return, both at
// once, (c) a non-exported function whose returns differ in length; the last two rows are the
// programs that stay accepted (a sized export, an unsized function that is not exported).
#[test]
fn an_exported_function_must_write_the_length_of_each_fixed_it_takes_or_returns() {
    let rows: [(&str, DiagnosticKind, usize); 6] = [
        (
            "export function total(share xs: Fixed<Int>) -> Int { return xs.count() }\n\
             function entrypoint() -> Nothing { print(1) }",
            DiagnosticKind::FixedLengthMissingInExport,
            1,
        ),
        (
            "export function make() -> Fixed<Int> {\n\
               let xs: Fixed<Int> = [1, 2]\n\
               return xs\n\
             }\n\
             function entrypoint() -> Nothing { print(1) }",
            DiagnosticKind::FixedLengthMissingInExport,
            1,
        ),
        (
            "export function same(share xs: Fixed<Int>) -> Fixed<Int> { return xs }\n\
             function entrypoint() -> Nothing { print(1) }",
            DiagnosticKind::FixedLengthMissingInExport,
            2,
        ),
        (
            "function pick(share flag: Boolean) -> Fixed<Int> {\n\
               let two: Fixed<Int> = [1, 2]\n\
               let one: Fixed<Int> = [1]\n\
               if (flag) {\n\
                 return two\n\
               }\n\
               return one\n\
             }\n\
             function entrypoint() -> Nothing { print(pick(true).count()) }",
            DiagnosticKind::FixedReturnLengthsDiffer,
            1,
        ),
        (
            "export function total(share xs: Fixed<Int, 3>) -> Fixed<Int, 3> { return xs }\n\
             function entrypoint() -> Nothing { print(1) }",
            DiagnosticKind::FixedLengthMissingInExport,
            0,
        ),
        (
            "function total(share xs: Fixed<Int>) -> Fixed<Int> { return xs }\n\
             function entrypoint() -> Nothing { print(1) }",
            DiagnosticKind::FixedLengthMissingInExport,
            0,
        ),
    ];
    for (source, kind, expected) in rows {
        assert_eq!(
            refusals_of(source, kind.clone()),
            expected,
            "{kind:?}: {source}"
        );
    }
}

// Bug: a written length was compared with the list passed or returned only on the cases the
// first tests tried, so a list one item too short or too long (or an empty one against a length
// of 0) reached codegen, which reads exactly N cells (High: data integrity; the off-by-one and
// empty cases are the boundary). Each row is a complete program and the number of
// `FixedLengthMismatch` refusals it gets.
#[test]
fn a_written_length_is_checked_against_each_list_passed_or_returned_at_its_boundaries() {
    let call = |written: &str, passed: &str| {
        format!(
            "function total(share xs: Fixed<Int, {written}>) -> Int {{ return xs.count() }}\n\
             function entrypoint() -> Nothing {{\n  let xs: Fixed<Int> = {passed}\n  print(total(xs))\n}}"
        )
    };
    let returning = |written: &str, returned: &str| {
        format!(
            "function make() -> Fixed<Int, {written}> {{\n  let xs: Fixed<Int> = {returned}\n  return xs\n}}\n\
             function entrypoint() -> Nothing {{ print(make().count()) }}"
        )
    };
    let rows: Vec<(String, String, usize)> = vec![
        ("call, exact".into(), call("3", "[1, 2, 3]"), 0),
        ("call, one short".into(), call("3", "[1, 2]"), 1),
        ("call, one over".into(), call("3", "[1, 2, 3, 4]"), 1),
        ("call, empty into 3".into(), call("3", "[]"), 1),
        ("call, empty into 0".into(), call("0", "[]"), 0),
        ("call, one into 0".into(), call("0", "[1]"), 1),
        ("return, exact".into(), returning("3", "[1, 2, 3]"), 0),
        ("return, one short".into(), returning("3", "[1, 2]"), 1),
        ("return, one over".into(), returning("3", "[1, 2, 3, 4]"), 1),
        ("return, empty into 0".into(), returning("0", "[]"), 0),
        ("return, one into 0".into(), returning("0", "[1]"), 1),
    ];
    for (label, source, expected) in rows {
        let output = check(&source);
        let mismatches = refusals_of(&source, DiagnosticKind::FixedLengthMismatch);
        assert_eq!(
            mismatches,
            expected,
            "{label}\n{source}\n{:#?}",
            errors(&output)
        );
        if expected == 0 {
            assert!(
                errors(&output).is_empty(),
                "{label}: {:#?}",
                errors(&output)
            );
        }
    }
}

// Bug: the shape-field rules said the compiler checks "every list stored in a field" against the
// field's written length, but a list of another length stored in `Fixed<Int, 3>` field was
// accepted: two items were padded with a zero and a fourth item was dropped, silently (High: data
// integrity of a value stored in a shape).
#[test]
fn a_list_of_another_length_is_refused_where_it_is_stored_in_a_sized_field() {
    let storing = |stored: &str| {
        format!(
            "shape Bag {{\n  items: Fixed<Int, 3>\n}}\n\
             function entrypoint() -> Nothing {{\n  let source: Fixed<Int> = {stored}\n  let bag: Bag = {{ items: source }}\n  print(bag.items.count())\n}}"
        )
    };
    let literal = "shape Bag {\n  items: Fixed<Int, 3>\n}\n\
                   function entrypoint() -> Nothing {\n  let bag: Bag = { items: [1, 2] }\n  print(bag.items.count())\n}";
    let rows = [
        ("variable, exact", storing("[1, 2, 3]"), 0),
        ("variable, one short", storing("[1, 2]"), 1),
        ("variable, one over", storing("[1, 2, 3, 4]"), 1),
        ("literal, one short", literal.to_string(), 1),
    ];
    for (label, source, expected) in rows {
        let refused = errors(&check(&source))
            .into_iter()
            .filter(|diagnostic| diagnostic.kind == Some(DiagnosticKind::FixedLengthMismatch))
            .count();
        assert_eq!(refused, expected, "{label}\n{source}");
    }
}

// Bug: a length written where the checker cannot use it (too large, a second argument, on a type
// that takes none, inside another type) was silently dropped or stopped the compiler later
// (High: validator of untrusted input). The largest length and zero are accepted; each refusal
// names the right kind, once.
#[test]
fn an_unusable_written_length_is_refused_by_kind_and_the_limits_are_accepted() {
    let parameter = |written: &str| {
        format!(
            "function total(share xs: {written}) -> Int {{ return 1 }}\n\
             function entrypoint() -> Nothing {{ print(1) }}"
        )
    };
    let rows: Vec<(String, Option<DiagnosticKind>)> = vec![
        (parameter("Fixed<Int, 0>"), None),
        (parameter("Fixed<Int, 4294967295>"), None),
        (
            parameter("Fixed<Int, 4294967296>"),
            Some(DiagnosticKind::FixedLengthNotUsable),
        ),
        (
            parameter("Fixed<Int, 3, 4>"),
            Some(DiagnosticKind::FixedLengthNotUsable),
        ),
        (
            parameter("Array<Int, 4>"),
            Some(DiagnosticKind::FixedLengthNotAllowedHere),
        ),
        (
            parameter("Maybe<Fixed<Int, 3>>"),
            Some(DiagnosticKind::FixedLengthNotAllowedHere),
        ),
        (
            "function make() -> Maybe<Fixed<Int, 3>> { return none }\n\
             function entrypoint() -> Nothing { print(1) }"
                .to_string(),
            Some(DiagnosticKind::FixedLengthNotAllowedHere),
        ),
    ];
    for (source, expected) in rows {
        let output = check(&source);
        match expected {
            None => assert!(
                errors(&output).is_empty(),
                "{source}\n{:#?}",
                errors(&output)
            ),
            Some(kind) => assert_eq!(
                refusals_of(&source, kind.clone()),
                1,
                "{source}\n{kind:?}\n{:#?}",
                errors(&output)
            ),
        }
    }
}

// Bug: the fallback of `.or` on a received `Fixed` list was not compared with the list's length:
// `.or(oneItem)` gave a list of 1 item typed as the callee's 4, and `list[3]` read past the
// fallback's stack block (High: memory safety of a value that leaves a task).
#[test]
fn a_fallback_of_another_length_is_refused_for_or_on_a_received_fixed() {
    let received = |fallback: &str| {
        format!(
            "function build() -> Fixed<Int> {{\n  wait sleep(1)\n  let numbers: Fixed<Int> = [10, 20, 30, 40]\n  return numbers\n}}\n\
             function entrypoint() -> Nothing {{\n  let first = background build()\n  wait sleep(20)\n  let got = first.receive()\n  let again = first.receive()\n  let fallback: Fixed<Int> = {fallback}\n  let list = again.or(fallback)\n  print(list.count())\n}}"
        )
    };
    let rows = [
        ("exact", received("[1, 2, 3, 4]"), 0),
        ("one item", received("[7]"), 1),
        ("one over", received("[1, 2, 3, 4, 5]"), 1),
        ("empty", received("[]"), 1),
    ];
    for (label, source, expected) in rows {
        let output = check(&source);
        assert_eq!(
            refusals_of(&source, DiagnosticKind::FixedLengthMismatch),
            expected,
            "{label}\n{source}\n{:#?}",
            errors(&output)
        );
        if expected == 0 {
            assert!(
                errors(&output).is_empty(),
                "{label}: {:#?}",
                errors(&output)
            );
        }
    }
}

// Bug: a list of another length reassigned to a binding, or assigned to a sized field, was refused
// with a message that named `Fixed<Int>` on both sides, so it read as no mismatch (Golden Rule 3).
// Each side shows its length.
#[test]
fn a_length_mismatch_on_reassignment_or_field_assignment_names_both_lengths() {
    let reassign = "function entrypoint() -> Nothing {\n  let xs: Fixed<Int> = [1, 2, 3]\n  let ys: Fixed<Int> = [1, 2]\n  xs = ys\n  print(xs.count())\n}";
    let output = check(reassign);
    let reassign_texts: Vec<String> = errors(&output)
        .iter()
        .map(|diagnostic| format!("{} {}", diagnostic.what, diagnostic.why))
        .collect();
    assert_eq!(reassign_texts.len(), 1, "{:#?}", errors(&output));
    assert!(
        reassign_texts[0].contains("Fixed<Int, 2>") && reassign_texts[0].contains("Fixed<Int, 3>"),
        "{}",
        reassign_texts[0]
    );

    let field = "shape Bag {\n  nums: Fixed<Int, 3>\n}\n\
                 function entrypoint() -> Nothing {\n  let bag: Bag = { nums: [1, 2, 3] }\n  bag.nums = [1, 2]\n  print(bag.nums.count())\n}";
    let output = check(field);
    let field_errors = errors(&output);
    assert_eq!(field_errors.len(), 1, "{field_errors:#?}");
    assert_eq!(
        field_errors[0].kind,
        Some(DiagnosticKind::FixedLengthMismatch)
    );
    assert!(
        field_errors[0].what.contains("2 items") && field_errors[0].what.contains("3 items"),
        "{}",
        field_errors[0].what
    );
}
