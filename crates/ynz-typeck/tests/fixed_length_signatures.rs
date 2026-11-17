//! v0.3-M9 C1h (#433): the lengths the type checker works out for `fixed` in function signatures.
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
        "function third() -> fixed<int> { return second() }\n\
         function second() -> fixed<int> { return first() }\n\
         function first() -> fixed<int> {\n\
           let xs: fixed<int> = [1, 2, 3]\n\
           return xs\n\
         }\n\
         function entrypoint() -> nothing { print(third().count()) }",
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
        "function same(give xs: fixed<int>) -> fixed<int> { return xs }\n\
         function entrypoint() -> nothing {\n\
           let xs: fixed<int> = [1, 2]\n\
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
        "function ping() -> fixed<int> { return pong() }\n\
         function pong() -> fixed<int> { return ping() }\n\
         function entrypoint() -> nothing { print(1) }",
    );
    let unknown: Vec<_> = errors(&output)
        .into_iter()
        .filter(|diagnostic| diagnostic.kind == Some(DiagnosticKind::FixedReturnLengthUnknown))
        .collect();
    assert_eq!(unknown.len(), 2, "{:#?}", errors(&output));
}
