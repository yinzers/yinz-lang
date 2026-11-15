// v0.3-M9 C3b (plan #17): which string expressions a store may take by move.
//
// WHY (High tier: memory safety / data integrity): the scope-exit release (C6/C7) frees a heap
// string once. A store that takes a string by move while another holder can still read it is a
// double free or a use-after-free; a store that copies a fresh string or a last use is a copy
// where Rust moves. Typeck's record (`TypedModule::string_moves`) decides, so these tests assert
// the record directly: no release pass exists yet, so the wrong answer is observable only here.
//
// Each case is one NAMED test over a small program. A read is named by the text it is written
// as and which occurrence of that text it is (`("keep", 2)` is the second `keep`). The cases are
// the boundaries of the last-use rule (a later read, two reads in one statement, a loop, a
// branch, a `return`, shadowing, a parameter) and of the fresh forms (a built string is fresh; a
// literal, an element read and an `.or()` result are not).

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
        // A backtick string's span starts one byte after its opening backtick and runs on past the
        // closing one, so a read is found by where it starts, not by its exact end.
        let found = output
            .typed_module
            .string_moves
            .spans()
            .any(|(from, to)| from >= start && from <= start + 1 && to >= start + text.len());
        assert_eq!(
            found, *expected,
            "`{text}` (occurrence {occurrence}) movable: expected {expected}, got {found} in:\n{program}"
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

const PRELUDE: &str = "shape Bag {\n  items: array<string>\n  label: string\n}\n";

move_cases! {
    the_last_read_of_a_local_is_a_move:
        "function entrypoint() -> nothing {\n  let n: int = 1\n  let keep: string = `a-${n}`\n  let other: string = keep\n  print(other)\n}\n",
        [("keep", 2, true), ("other", 2, true)],
        "a last read taken by copy where Rust moves (cost), or a binding whose only read is the last one treated as live (High)";

    a_read_followed_by_another_read_is_not_a_move:
        "function entrypoint() -> nothing {\n  let n: int = 1\n  let keep: string = `a-${n}`\n  let other: string = keep\n  print(keep)\n}\n",
        [("keep", 2, false), ("keep", 3, true)],
        "a store moves a string the program still reads, so the store and the later read hold one pointer (High)";

    two_reads_in_one_statement_are_not_moves:
        "function entrypoint() -> nothing {\n  let n: int = 1\n  let keep: string = `a-${n}`\n  let bag: Bag = { items: [keep], label: keep }\n  print(bag.label)\n}\n",
        [("keep", 2, false), ("keep", 3, false)],
        "a literal that stores one binding twice moves it into both places (High)";

    a_read_in_a_loop_of_an_outer_binding_is_not_a_move:
        "function entrypoint() -> nothing {\n  let n: int = 1\n  let keep: string = `a-${n}`\n  let bag: Bag = { items: [`x`], label: `y` }\n  for (i in range(0, 3)) {\n    bag.items.add(keep)\n  }\n}\n",
        [("keep", 2, false)],
        "the loop's second pass stores the string it already moved on the first (High)";

    a_binding_declared_in_the_loop_is_a_move_each_pass:
        "function entrypoint() -> nothing {\n  let n: int = 1\n  let bag: Bag = { items: [`x`], label: `y` }\n  for (i in range(0, 3)) {\n    let each: string = `a-${n}`\n    bag.items.add(each)\n  }\n}\n",
        [("each", 2, true)],
        "a per-pass local treated as live across passes, copying where Rust moves (cost)";

    a_read_in_a_branch_followed_by_a_read_is_not_a_move:
        "function entrypoint() -> nothing {\n  let n: int = 1\n  let keep: string = `a-${n}`\n  let bag: Bag = { items: [`x`], label: `y` }\n  if (n == 1) {\n    bag.items.add(keep)\n  }\n  print(keep)\n}\n",
        [("keep", 2, false), ("keep", 3, true)],
        "a branch moves a string the code after the branch still reads (High)";

    a_read_in_a_branch_with_no_later_read_is_a_move:
        "function entrypoint() -> nothing {\n  let n: int = 1\n  let keep: string = `a-${n}`\n  let bag: Bag = { items: [`x`], label: `y` }\n  if (n == 1) {\n    bag.items.add(keep)\n  }\n}\n",
        [("keep", 2, true)],
        "a conditional last use copied where Rust moves (cost)";

    a_returned_local_moves_even_when_code_after_the_return_reads_it:
        "function build(n: int) -> string {\n  let keep: string = `a-${n}`\n  if (n == 1) {\n    return keep\n  }\n  return keep\n}\nfunction entrypoint() -> nothing {\n  print(build(1))\n}\n",
        [("keep", 2, true), ("keep", 3, true)],
        "a `return` that leaves the function treated as followed by the code after it, so every return copies (cost)";

    a_parameter_is_never_a_move:
        "function pass(text: string) -> string {\n  return text\n}\nfunction entrypoint() -> nothing {\n  print(pass(`x`))\n}\n",
        [("text", 2, false)],
        "a borrowed parameter returned or stored without a copy, so the caller's string has two owners (High)";

    a_shadowing_binding_is_resolved_by_declaration:
        "function entrypoint() -> nothing {\n  let n: int = 1\n  let keep: string = `a-${n}`\n  if (n == 1) {\n    let keep: string = `b-${n}`\n    let inner: string = keep\n    print(inner)\n  }\n  print(keep)\n}\n",
        [("keep", 2, false), ("keep", 3, true), ("keep", 4, true)],
        "the inner `keep`'s last read counted as a read of the outer one (or the reverse), moving a live string (High)";

    fresh_strings_are_moves:
        "function make(n: int) -> string {\n  return `m-${n}`\n}\nfunction entrypoint() -> nothing {\n  let n: int = 1\n  let a: string = `a-${n}`\n  let b: string = n.toString()\n  let c: string = a.toUpperCase()\n  let d: string = make(n)\n  let e: string = a.copy()\n  print(`${a}${b}${c}${d}${e}`)\n}\n",
        [
            ("`a-${n}`", 1, true),
            ("n.toString()", 1, true),
            ("a.toUpperCase()", 1, true),
            ("make(n)", 1, true),
            ("a.copy()", 1, true),
        ],
        "a fresh string copied at its store, or an `int` toString (a heap string since C3a) treated as borrowed (cost)";

    a_builtin_method_is_not_a_function_call_of_the_same_name:
        "function or(first: string, second: string) -> string {\n  return second\n}\nfunction entrypoint() -> nothing {\n  let names: array<string> = [`a`]\n  let got: string = names[0].or(`none`)\n  print(got)\n}\n",
        [("names[0].or(`none`)", 1, false)],
        "a function that shares a built-in method's name makes the built-in's borrowed result look fresh, so a store takes an element's string without a copy and a release frees it twice (High)";

    a_function_called_with_a_shape_receiver_is_fresh:
        "shape Tag {\n  name: string\n}\nfunction label(tag: Tag) -> string {\n  return `L-${tag.name}`\n}\nfunction entrypoint() -> nothing {\n  let tag: Tag = { name: `t` }\n  let text: string = tag.label()\n  print(text)\n}\n",
        [("tag.label()", 1, true)],
        "a dot-call of a function on a shape receiver copied where Rust moves (cost)";

    borrowed_strings_are_not_moves:
        "function entrypoint() -> nothing {\n  let n: int = 1\n  let flag: boolean = true\n  let names: array<string> = [`a`]\n  let literal: string = `plain`\n  let element: string = names[0].or(`none`)\n  let shown: string = flag.toString()\n  print(`${literal}${element}${shown}${n}`)\n}\n",
        [
            ("`plain`", 1, false),
            ("names[0].or(`none`)", 1, false),
            ("flag.toString()", 1, false),
            ("`a`", 1, false),
        ],
        "a literal, an element, an `.or()` result or a static `boolean` name stored without a copy: a later release frees static or shared bytes (High)";
}
