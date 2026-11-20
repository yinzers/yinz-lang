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

const PRELUDE: &str = "shape Bag {\n  items: Array<String>\n  label: String\n}\n";

move_cases! {
    the_last_read_of_a_local_is_a_move:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let keep: String = `a-${n}`\n  let other: String = keep\n  print(other)\n}\n",
        [("keep", 2, true), ("other", 2, true)],
        "a last read taken by copy where Rust moves (cost), or a binding whose only read is the last one treated as live (High)";

    a_read_followed_by_another_read_is_not_a_move:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let keep: String = `a-${n}`\n  let other: String = keep\n  print(keep)\n}\n",
        [("keep", 2, false), ("keep", 3, true)],
        "a store moves a string the program still reads, so the store and the later read hold one pointer (High)";

    two_reads_in_one_statement_are_not_moves:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let keep: String = `a-${n}`\n  let bag: Bag = { items: [keep], label: keep }\n  print(bag.label)\n}\n",
        [("keep", 2, false), ("keep", 3, false)],
        "a literal that stores one binding twice moves it into both places (High)";

    a_read_in_a_loop_of_an_outer_binding_is_not_a_move:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let keep: String = `a-${n}`\n  let bag: Bag = { items: [`x`], label: `y` }\n  for (i in range(0, 3)) {\n    bag.items.add(keep)\n  }\n}\n",
        [("keep", 2, false)],
        "the loop's second pass stores the string it already moved on the first (High)";

    a_binding_declared_in_the_loop_is_a_move_each_pass:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let bag: Bag = { items: [`x`], label: `y` }\n  for (i in range(0, 3)) {\n    let each: String = `a-${n}`\n    bag.items.add(each)\n  }\n}\n",
        [("each", 2, true)],
        "a per-pass local treated as live across passes, copying where Rust moves (cost)";

    a_read_in_a_branch_followed_by_a_read_is_not_a_move:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let keep: String = `a-${n}`\n  let bag: Bag = { items: [`x`], label: `y` }\n  if (n == 1) {\n    bag.items.add(keep)\n  }\n  print(keep)\n}\n",
        [("keep", 2, false), ("keep", 3, true)],
        "a branch moves a string the code after the branch still reads (High)";

    a_read_in_a_branch_with_no_later_read_is_a_move:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let keep: String = `a-${n}`\n  let bag: Bag = { items: [`x`], label: `y` }\n  if (n == 1) {\n    bag.items.add(keep)\n  }\n}\n",
        [("keep", 2, true)],
        "a conditional last use copied where Rust moves (cost)";

    a_returned_local_moves_even_when_code_after_the_return_reads_it:
        "function build(n: Int) -> String {\n  let keep: String = `a-${n}`\n  if (n == 1) {\n    return keep\n  }\n  return keep\n}\nfunction entrypoint() -> Nothing {\n  print(build(1))\n}\n",
        [("keep", 2, true), ("keep", 3, true)],
        "a `return` that leaves the function treated as followed by the code after it, so every return copies (cost)";

    a_parameter_is_never_a_move:
        "function pass(text: String) -> String {\n  return text\n}\nfunction entrypoint() -> Nothing {\n  print(pass(`x`))\n}\n",
        [("text", 2, false)],
        "a borrowed parameter returned or stored without a copy, so the caller's string has two owners (High)";

    a_shadowing_binding_is_resolved_by_declaration:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let keep: String = `a-${n}`\n  if (n == 1) {\n    let keep: String = `b-${n}`\n    let inner: String = keep\n    print(inner)\n  }\n  print(keep)\n}\n",
        [("keep", 2, false), ("keep", 3, true), ("keep", 4, true)],
        "the inner `keep`'s last read counted as a read of the outer one (or the reverse), moving a live string (High)";

    fresh_strings_are_moves:
        "function make(n: Int) -> String {\n  return `m-${n}`\n}\nfunction entrypoint() -> Nothing {\n  let n: Int = 1\n  let a: String = `a-${n}`\n  let b: String = n.toString()\n  let c: String = a.toUpperCase()\n  let d: String = make(n)\n  let e: String = a.copy()\n  print(`${a}${b}${c}${d}${e}`)\n}\n",
        [
            ("`a-${n}`", 1, true),
            ("n.toString()", 1, true),
            ("a.toUpperCase()", 1, true),
            ("make(n)", 1, true),
            ("a.copy()", 1, true),
        ],
        "a fresh string copied at its store, or an `int` toString (a heap string since C3a) treated as borrowed (cost)";

    a_builtin_method_is_not_a_function_call_of_the_same_name:
        "function or(first: String, second: String) -> String {\n  return second\n}\nfunction entrypoint() -> Nothing {\n  let names: Array<String> = [`a`]\n  let got: String = names[0].or(`none`)\n  print(got)\n}\n",
        [("names[0].or(`none`)", 1, false)],
        "a function that shares a built-in method's name makes the built-in's borrowed result look fresh, so a store takes an element's string without a copy and a release frees it twice (High)";

    a_read_in_a_while_loop_of_an_outer_binding_is_not_a_move:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let keep: String = `a-${n}`\n  let bag: Bag = { items: [`x`], label: `y` }\n  let pass: Int = 0\n  while (pass < 3) {\n    bag.items.add(keep)\n    pass = pass + 1\n  }\n}\n",
        [("keep", 2, false)],
        "a `while` pass stores the string it already moved on the previous pass (High)";

    a_read_in_an_inner_loop_of_a_binding_of_the_outer_loop_is_not_a_move:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let bag: Bag = { items: [`x`], label: `y` }\n  for (i in range(0, 2)) {\n    let each: String = `a-${n}`\n    for (j in range(0, 2)) {\n      bag.items.add(each)\n    }\n  }\n}\n",
        [("each", 2, false)],
        "the inner loop's second pass stores the string its first pass moved, because the binding belongs to the outer loop (High)";

    a_read_in_an_if_branch_followed_by_a_read_after_it_is_not_a_move_in_a_multi_arm_if:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let keep: String = `a-${n}`\n  let bag: Bag = { items: [`x`], label: `y` }\n  if (n) {\n    1 => {\n      bag.items.add(keep)\n    }\n    else => {\n      print(`other`)\n    }\n  }\n  print(keep)\n}\n",
        [("keep", 2, false), ("keep", 3, true)],
        "a multi-case `if` arm moves a string the code after the `if` still reads (High)";

    a_read_in_each_of_two_arms_is_a_move_in_both:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let keep: String = `a-${n}`\n  let bag: Bag = { items: [`x`], label: `y` }\n  if (n) {\n    1 => {\n      bag.items.add(keep)\n    }\n    else => {\n      bag.items.add(keep)\n    }\n  }\n}\n",
        [("keep", 2, true), ("keep", 3, true)],
        "an arm's last use treated as live because a sibling arm, which never runs after it, reads the same binding (cost)";

    a_binding_moved_into_another_and_then_reassigned_is_a_move:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let first: String = `a-${n}`\n  let second: String = first\n  first = `b-${n}`\n  print(second)\n}\n",
        [("first", 2, true)],
        "`let b = a; a = x`: the read of `a` copied although the assignment overwrites `a`, so no later read of the old value exists (cost)";

    a_binding_moved_into_another_and_read_after_reassignment_is_not_a_move:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let first: String = `a-${n}`\n  let second: String = first\n  first = `b-${n}`\n  print(first)\n  print(second)\n}\n",
        [("first", 2, false), ("first", 4, true)],
        "`let b = a; a = x; print(a)`: the first read moved because the assignment between looked like the end of `a`'s uses, so `a` and `b` share one string (High)";

    a_binding_read_twice_in_one_loop_pass_is_not_a_move_at_the_first_read:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let bag: Bag = { items: [`x`], label: `y` }\n  for (i in range(0, 2)) {\n    let each: String = `a-${n}`\n    bag.items.add(each)\n    bag.label = each\n  }\n}\n",
        [("each", 2, false), ("each", 3, true)],
        "a per-pass binding moved at its first read and read again later in the same pass (High)";

    a_function_called_with_a_shape_receiver_is_fresh:
        "shape Tag {\n  name: String\n}\nfunction label(tag: Tag) -> String {\n  return `L-${tag.name}`\n}\nfunction entrypoint() -> Nothing {\n  let tag: Tag = { name: `t` }\n  let text: String = tag.label()\n  print(text)\n}\n",
        [("tag.label()", 1, true)],
        "a dot-call of a function on a shape receiver copied where Rust moves (cost)";

    borrowed_strings_are_not_moves:
        "function entrypoint() -> Nothing {\n  let n: Int = 1\n  let flag: Boolean = true\n  let names: Array<String> = [`a`]\n  let literal: String = `plain`\n  let element: String = names[0].or(`none`)\n  let shown: String = flag.toString()\n  print(`${literal}${element}${shown}${n}`)\n}\n",
        [
            ("`plain`", 1, false),
            ("names[0].or(`none`)", 1, false),
            ("flag.toString()", 1, false),
            ("`a`", 1, false),
        ],
        "a literal, an element, an `.or()` result or a static `boolean` name stored without a copy: a later release frees static or shared bytes (High)";
}
