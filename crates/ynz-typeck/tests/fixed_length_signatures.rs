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

// bug: typeck gives a function that returns another function's list no length when it is declared before the function it calls (one pass over the bodies cannot know the callee's length yet) | tier: High
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
    assert_eq!(worked_out.get("first"), Some(&list_of(3)), "first");
    assert_eq!(worked_out.get("second"), Some(&list_of(3)), "second");
    assert_eq!(worked_out.get("third"), Some(&list_of(3)), "third");
}

// bug: typeck gives a function that returns its own unsized parameter no length at the call, so the caller reads a list of unknown length and the specialised version is never requested | tier: High
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

/// What the checker must decide for one program.
enum Expect {
    /// No error at all.
    Accepted,
    /// Exactly this many errors of this kind.
    Refused(DiagnosticKind, usize),
    /// Exactly this many errors, of any kind.
    Errors(usize),
    /// Exactly one error, whose text contains the phrase.
    RejectedWith(&'static str),
}

fn assert_expected(source: &str, expected: Expect) {
    let output = check(source);
    let found = errors(&output);
    match expected {
        Expect::Accepted => assert!(found.is_empty(), "{source}\n{found:#?}"),
        Expect::Refused(kind, count) => {
            let of_kind = found
                .iter()
                .filter(|diagnostic| diagnostic.kind == Some(kind.clone()))
                .count();
            assert_eq!(of_kind, count, "{kind:?}\n{source}\n{found:#?}");
        }
        Expect::Errors(count) => assert_eq!(found.len(), count, "{source}\n{found:#?}"),
        Expect::RejectedWith(phrase) => {
            assert_eq!(found.len(), 1, "{source}\n{found:#?}");
            assert!(found[0].what.contains(phrase), "{:#?}", found[0]);
        }
    }
}

macro_rules! fixed_length_cases {
    ($($case:ident: $source:expr => $expected:expr;)+) => {
        $(
            // bug: typeck accepts or rejects a program against the wrong Fixed length (a list of another length reaches codegen, which reads exactly N cells), or refuses a valid one | tier: High
            #[test]
            fn $case() {
                assert_expected(&$source, $expected);
            }
        )+
    };
}

fn waiting_with(parameter: &str, body: &str, extra: &str) -> String {
    format!(
        "{extra}\nfunction show({parameter}) -> Int {{\n  wait sleep(1)\n{body}\n  return 1\n}}\n\
         function entrypoint() -> Nothing {{\n  let done = wait show([1, 2, 3])\n  print(done)\n}}"
    )
}

fn call_with_length(written: &str, passed: &str) -> String {
    format!(
        "function total(share xs: Fixed<Int, {written}>) -> Int {{ return xs.count() }}\n\
         function entrypoint() -> Nothing {{\n  let xs: Fixed<Int> = {passed}\n  print(total(xs))\n}}"
    )
}

fn return_with_length(written: &str, returned: &str) -> String {
    format!(
        "function make() -> Fixed<Int, {written}> {{\n  let xs: Fixed<Int> = {returned}\n  return xs\n}}\n\
         function entrypoint() -> Nothing {{ print(make().count()) }}"
    )
}

fn store_in_sized_field(stored: &str) -> String {
    format!(
        "shape Bag {{\n  items: Fixed<Int, 3>\n}}\n\
         function entrypoint() -> Nothing {{\n  let source: Fixed<Int> = {stored}\n  let bag: Bag = {{ items: source }}\n  print(bag.items.count())\n}}"
    )
}

fn parameter_of_type(written: &str) -> String {
    format!(
        "function total(share xs: {written}) -> Int {{ return 1 }}\n\
         function entrypoint() -> Nothing {{ print(1) }}"
    )
}

fn or_on_received_with_fallback(fallback: &str) -> String {
    format!(
        "function build() -> Fixed<Int> {{\n  wait sleep(1)\n  let numbers: Fixed<Int> = [10, 20, 30, 40]\n  return numbers\n}}\n\
         function entrypoint() -> Nothing {{\n  let first = background build()\n  wait sleep(20)\n  let got = first.receive()\n  let again = first.receive()\n  let fallback: Fixed<Int> = {fallback}\n  let list = again.or(fallback)\n  print(list.count())\n}}"
    )
}

// Mutually returning functions: the length loops or is made up; the refusal names both.
fixed_length_cases! {
    functions_that_only_return_each_other_have_no_length_to_work_out:
        "function ping() -> Fixed<Int> { return pong() }\n\
         function pong() -> Fixed<Int> { return ping() }\n\
         function entrypoint() -> Nothing { print(1) }"
        => Expect::Refused(DiagnosticKind::FixedReturnLengthUnknown, 2);
}

// Where a written length may sit: the whole type of a field, never deeper (nothing stores the list
// by value inside `Maybe`/`Array`), and a field of an inline shape must write it (it read count 0).
fixed_length_cases! {
    a_length_on_the_whole_type_of_a_field_is_allowed:
        "shape Bag { items: Fixed<Int, 3> }\nfunction entrypoint() -> Nothing { print(1) }"
        => Expect::Accepted;
    a_length_inside_a_maybe_field_is_refused:
        "shape Bag { items: Maybe<Fixed<Int, 2>> }\nfunction entrypoint() -> Nothing { print(1) }"
        => Expect::Refused(DiagnosticKind::FixedLengthNotAllowedHere, 1);
    a_length_inside_an_array_field_is_refused:
        "shape Bag { items: Array<Fixed<Int, 2>> }\nfunction entrypoint() -> Nothing { print(1) }"
        => Expect::Refused(DiagnosticKind::FixedLengthNotAllowedHere, 1);
    an_inline_shape_field_without_a_length_in_a_let_is_refused:
        "function entrypoint() -> Nothing {\n\
           let point: { trail: Fixed<Int> } = { trail: [4, 5, 6] }\n\
           print(point.trail.count())\n\
         }"
        => Expect::Refused(DiagnosticKind::FixedFieldLengthMissing, 1);
    an_inline_shape_field_without_a_length_in_a_parameter_is_refused:
        "function show(share point: { trail: Fixed<Int> }) -> Nothing { print(1) }\n\
         function entrypoint() -> Nothing { print(2) }"
        => Expect::Refused(DiagnosticKind::FixedFieldLengthMissing, 1);
    an_inline_shape_field_without_a_length_in_a_return_is_refused:
        "function make() -> { trail: Fixed<Int> } { return { trail: [1] } }\n\
         function entrypoint() -> Nothing { print(2) }"
        => Expect::Refused(DiagnosticKind::FixedFieldLengthMissing, 1);
    an_inline_shape_field_without_a_length_in_a_named_shape_is_refused:
        "shape Holder { inner: { trail: Fixed<Int> } }\n\
         function entrypoint() -> Nothing { print(2) }"
        => Expect::Refused(DiagnosticKind::FixedFieldLengthMissing, 1);
}

// A function built once (it waits) has no length for an unsized `Fixed` parameter: the refusal
// fires exactly where the body asks for the length, in each way it can, and nowhere else.
fixed_length_cases! {
    a_function_built_once_is_refused_an_unsized_list_it_counts:
        waiting_with("share xs: Fixed<Int>", "  let n = xs.count()", "")
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInTaskFunction, 1);
    a_function_built_once_is_refused_an_unsized_list_it_iterates:
        waiting_with("share xs: Fixed<Int>", "  for (x in xs) {\n    print(x)\n  }", "")
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInTaskFunction, 1);
    a_function_built_once_is_refused_an_unsized_list_it_indexes:
        waiting_with("share xs: Fixed<Int>", "  let first = xs[0].or(0)", "")
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInTaskFunction, 1);
    a_function_built_once_is_refused_an_unsized_list_it_gets_from:
        waiting_with("share xs: Fixed<Int>", "  let first = xs.get(0).or(0)", "")
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInTaskFunction, 1);
    a_function_built_once_is_refused_an_unsized_list_it_returns:
        waiting_with("share xs: Fixed<Int>", "  let back = xs", "")
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInTaskFunction, 1);
    a_function_built_once_is_refused_an_unsized_list_it_hands_to_a_callee_that_needs_the_length:
        waiting_with(
            "share xs: Fixed<Int>",
            "  let n = inner(xs)",
            "function inner(share xs: Fixed<Int>) -> Int { return xs.count() }",
        )
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInTaskFunction, 1);
    a_function_built_once_is_refused_an_owned_unsized_list_with_heap_items:
        waiting_with("give xs: Fixed<String>", "", "").replace("show([1, 2, 3])", "show([`a`, `b`])")
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInTaskFunction, 1);
    a_function_built_once_is_refused_an_unsized_list_it_hands_to_a_task:
        waiting_with(
            "share xs: Fixed<Int>",
            "  let task = background inner(xs)",
            "function inner(share xs: Fixed<Int>) -> Int {\n  wait sleep(1)\n  return 1\n}",
        )
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInTaskFunction, 1);
    // The converse: functions that only receive the list compiled and ran correctly before.
    a_function_built_once_that_only_passes_the_list_to_a_task_is_not_refused:
        "function show(value: Fixed<String>) -> Nothing {\n  wait sleep(1)\n  print(777)\n}\n\
         function entrypoint() -> Nothing {\n  let value: Fixed<String> = [`p`, `q`]\n  background show(value)\n  wait sleep(5)\n}"
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInTaskFunction, 0);
    a_function_built_once_that_only_passes_the_list_to_a_waiting_callee_is_not_refused:
        "function inner(share xs: Fixed<Int>) -> Int {\n  wait sleep(1)\n  return 1\n}\n\
         function show(share xs: Fixed<Int>) -> Int {\n  wait sleep(1)\n  return inner(xs)\n}\n\
         function entrypoint() -> Nothing {\n  let done = wait show([1, 2])\n  print(done)\n}"
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInTaskFunction, 0);
    a_function_built_once_that_never_reads_an_owned_list_is_not_refused:
        "function show(give xs: Fixed<Int>) -> Int {\n  wait sleep(1)\n  return 1\n}\n\
         function entrypoint() -> Nothing {\n  let done = wait show([1, 2])\n  print(done)\n}"
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInTaskFunction, 0);
}

// A received `Fixed<String>` bound to a local released after a later `wait` was released from
// cells in a dead resume-function stack frame (stack-use-after-return): refused. A list of plain
// values owns nothing to release, so a later `wait` after its last use stays allowed.
fixed_length_cases! {
    a_received_fixed_of_strings_cannot_cross_a_later_wait:
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
         }"
        => Expect::RejectedWith("cannot yet cross a `wait`");
    a_received_fixed_of_ints_may_be_followed_by_a_wait:
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
         }"
        => Expect::Accepted;
}

// An export must write the length of each `Fixed` it takes or returns (a module checked alone
// cannot size it); a non-exported function whose returns differ in length is refused; a sized
// export and an unsized non-exported function stay accepted.
fixed_length_cases! {
    an_exported_unsized_parameter_is_refused:
        "export function total(share xs: Fixed<Int>) -> Int { return xs.count() }\n\
         function entrypoint() -> Nothing { print(1) }"
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInExport, 1);
    an_exported_unsized_return_is_refused:
        "export function make() -> Fixed<Int> {\n\
           let xs: Fixed<Int> = [1, 2]\n\
           return xs\n\
         }\n\
         function entrypoint() -> Nothing { print(1) }"
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInExport, 1);
    an_exported_unsized_parameter_and_return_are_each_refused:
        "export function same(share xs: Fixed<Int>) -> Fixed<Int> { return xs }\n\
         function entrypoint() -> Nothing { print(1) }"
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInExport, 2);
    returns_of_different_lengths_are_refused:
        "function pick(share flag: Boolean) -> Fixed<Int> {\n\
           let two: Fixed<Int> = [1, 2]\n\
           let one: Fixed<Int> = [1]\n\
           if (flag) {\n\
             return two\n\
           }\n\
           return one\n\
         }\n\
         function entrypoint() -> Nothing { print(pick(true).count()) }"
        => Expect::Refused(DiagnosticKind::FixedReturnLengthsDiffer, 1);
    an_exported_sized_function_is_accepted:
        "export function total(share xs: Fixed<Int, 3>) -> Fixed<Int, 3> { return xs }\n\
         function entrypoint() -> Nothing { print(1) }"
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInExport, 0);
    an_unsized_function_that_is_not_exported_is_accepted:
        "function total(share xs: Fixed<Int>) -> Fixed<Int> { return xs }\n\
         function entrypoint() -> Nothing { print(1) }"
        => Expect::Refused(DiagnosticKind::FixedLengthMissingInExport, 0);
}

// A written length is compared with the list passed, returned, stored or used as a fallback:
// one item too short or too long, or an empty list against a length of 0, must not reach codegen.
fixed_length_cases! {
    a_call_with_the_exact_length_is_accepted:
        call_with_length("3", "[1, 2, 3]") => Expect::Accepted;
    a_call_one_item_short_is_refused:
        call_with_length("3", "[1, 2]") => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    a_call_one_item_over_is_refused:
        call_with_length("3", "[1, 2, 3, 4]") => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    a_call_with_an_empty_list_into_3_is_refused:
        call_with_length("3", "[]") => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    a_call_with_an_empty_list_into_0_is_accepted:
        call_with_length("0", "[]") => Expect::Accepted;
    a_call_with_one_item_into_0_is_refused:
        call_with_length("0", "[1]") => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    a_return_with_the_exact_length_is_accepted:
        return_with_length("3", "[1, 2, 3]") => Expect::Accepted;
    a_return_one_item_short_is_refused:
        return_with_length("3", "[1, 2]") => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    a_return_one_item_over_is_refused:
        return_with_length("3", "[1, 2, 3, 4]") => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    a_return_with_an_empty_list_into_0_is_accepted:
        return_with_length("0", "[]") => Expect::Accepted;
    a_return_with_one_item_into_0_is_refused:
        return_with_length("0", "[1]") => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    a_variable_stored_in_a_sized_field_with_the_exact_length_is_accepted:
        store_in_sized_field("[1, 2, 3]") => Expect::Accepted;
    a_variable_one_item_short_stored_in_a_sized_field_is_refused:
        store_in_sized_field("[1, 2]") => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    a_variable_one_item_over_stored_in_a_sized_field_is_refused:
        store_in_sized_field("[1, 2, 3, 4]") => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    a_literal_one_item_short_stored_in_a_sized_field_is_refused:
        "shape Bag {\n  items: Fixed<Int, 3>\n}\n\
         function entrypoint() -> Nothing {\n  let bag: Bag = { items: [1, 2] }\n  print(bag.items.count())\n}"
        => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    an_or_fallback_of_the_exact_length_on_a_received_list_is_accepted:
        or_on_received_with_fallback("[1, 2, 3, 4]") => Expect::Accepted;
    an_or_fallback_of_one_item_on_a_received_list_is_refused:
        or_on_received_with_fallback("[7]") => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    an_or_fallback_one_item_over_on_a_received_list_is_refused:
        or_on_received_with_fallback("[1, 2, 3, 4, 5]") => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    an_or_fallback_of_an_empty_list_on_a_received_list_is_refused:
        or_on_received_with_fallback("[]") => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
    a_list_of_another_length_reassigned_to_a_binding_is_refused:
        "function entrypoint() -> Nothing {\n  let xs: Fixed<Int> = [1, 2, 3]\n  let ys: Fixed<Int> = [1, 2]\n  xs = ys\n  print(xs.count())\n}"
        => Expect::Errors(1);
    a_list_of_another_length_assigned_to_a_sized_field_is_refused:
        "shape Bag {\n  nums: Fixed<Int, 3>\n}\n\
         function entrypoint() -> Nothing {\n  let bag: Bag = { nums: [1, 2, 3] }\n  bag.nums = [1, 2]\n  print(bag.nums.count())\n}"
        => Expect::Refused(DiagnosticKind::FixedLengthMismatch, 1);
}

// A length written where the checker cannot use it (too large, a second argument, on a type that
// takes none, inside another type) was silently dropped or stopped the compiler later. The
// largest length and zero are accepted; each refusal names the right kind, once.
fixed_length_cases! {
    a_length_of_zero_is_accepted:
        parameter_of_type("Fixed<Int, 0>") => Expect::Accepted;
    the_largest_length_is_accepted:
        parameter_of_type("Fixed<Int, 4294967295>") => Expect::Accepted;
    a_length_one_over_the_largest_is_not_usable:
        parameter_of_type("Fixed<Int, 4294967296>") => Expect::Refused(DiagnosticKind::FixedLengthNotUsable, 1);
    a_second_length_argument_is_not_usable:
        parameter_of_type("Fixed<Int, 3, 4>") => Expect::Refused(DiagnosticKind::FixedLengthNotUsable, 1);
    a_length_on_an_array_is_not_allowed:
        parameter_of_type("Array<Int, 4>") => Expect::Refused(DiagnosticKind::FixedLengthNotAllowedHere, 1);
    a_length_inside_a_maybe_parameter_is_not_allowed:
        parameter_of_type("Maybe<Fixed<Int, 3>>") => Expect::Refused(DiagnosticKind::FixedLengthNotAllowedHere, 1);
    a_length_inside_a_maybe_return_is_not_allowed:
        "function make() -> Maybe<Fixed<Int, 3>> { return none }\n\
         function entrypoint() -> Nothing { print(1) }"
        => Expect::Refused(DiagnosticKind::FixedLengthNotAllowedHere, 1);
}
