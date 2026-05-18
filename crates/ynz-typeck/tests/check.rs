// WHY: signature mismatch, type-rule violations, and scope errors must produce
// three-part diagnostics. Catches regressions where typeck silently degrades to
// one-line messages that leave the developer without a "what to do instead" or
// "why" field.
//
// test-ratchet: M7 P1 — all double-quoted string literals in Yinz source strings
// replaced with backtick strings. Double-quotes now produce an error diagnostic.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput, Type};

const FILE: &str = "test.ynz";

fn run(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

fn assert_clean(source: &str) {
    let output = run(source);
    assert!(
        output.diagnostics.is_empty(),
        "Expected 0 diagnostics, got {}: {:#?}",
        output.diagnostics.len(),
        output.diagnostics
    );
}

fn assert_errors(source: &str, expected_count: usize) -> CheckOutput {
    let output = run(source);
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert_eq!(
        errors.len(),
        expected_count,
        "Expected {expected_count} errors, got {}:\n{:#?}",
        errors.len(),
        errors
    );
    output
}


#[test]
fn m4_type_variant_count_locked() {
    // WHY: adding new types before their milestones introduces untested paths.
    //
    // test-ratchet: M2 adds 4 variants over M1's 3.
    //   M1: Nothing(1), String(2), Error(3).
    //   M2: Int(4), Float(5), Number(6), Bool(7). Total: 7.
    //
    // test-ratchet: M3 adds 1 variant for the range builtin iterable type.
    //   Range is restricted to for-loop iterable position only.
    //   Full Iterable[T] protocol replaces it in M7. Total: 8.
    //
    // test-ratchet: M4 adds 1 variant for user-defined shape types.
    //   Shape holds the shape name; field layout lives in ShapeTable. Total: 9.
    // (P3b added Dynamic; see type_variant_count_includes_dynamic test below)
    let all: &[Type] = &[
        Type::Nothing,
        Type::String,
        Type::Error,
        Type::Int,
        Type::Float,
        Type::Number { precision: 34 },
        Type::Bool,
        Type::Range { element: Box::new(Type::Int), end_inclusive: false },
        Type::Shape { name: "Player".into() },
        Type::Dynamic { contract: "Foo".into() },
    ];
    assert_eq!(all.len(), 10, "Type variant count changed from 10 — add // test-ratchet: comment");
}


#[test]
fn m1_source_type_checks_clean() {
    // WHY: if this test breaks, the hello-world integration test (Phase 7) breaks too.
    // The string-type assertion below is load-bearing — it proves type inference actually
    // ran and stored results, not just that no diagnostics were emitted.
    let output = run(r#"function entrypoint() -> nothing { print(`hello, yinz`) }"#);
    assert!(
        output.diagnostics.is_empty(),
        "M1 source must type-check with zero diagnostics, got: {:#?}",
        output.diagnostics
    );
    let string_ty = output
        .typed_module
        .expr_types
        .values()
        .find(|t| **t == Type::String);
    assert!(
        string_ty.is_some(),
        "String literal type should be in expr_types — type inference must record results"
    );
}

#[test]
fn m2_smoke_test_type_checks_clean() {
    // WHY: this is the canonical M2 program. Every step of the type-check pass
    // is exercised: let with annotation, literal inference, arithmetic, boolean,
    // and print. If this fails, the M2 integration test (Phase 5) will also fail.
    assert_clean(
        r#"function entrypoint() -> nothing {
  let price = 0.1 + 0.2
  let count: int = 42
  let active = true
  print(price)
  print(count)
  print(active)
}"#,
    );
}

#[test]
fn const_binding_type_checks_clean() {
    // WHY: `const` must be accepted by typeck with the same rules as `let`.
    assert_clean("function entrypoint() -> nothing { const x = 42\nprint(x) }");
}

#[test]
fn conversion_methods_type_check_clean() {
    // WHY: intrinsic method calls must resolve to the correct return type.
    // If `int.toString()` doesn't resolve to `string`, `print` will reject it.
    assert_clean(
        r#"function entrypoint() -> nothing {
  let n: int = 42
  let s = n.toString()
  print(s)
}"#,
    );
}


#[test]
fn int_literal_infers_as_int() {
    // WHY: `let x = 42` must give x the type `int`, not `number`. This is
    // Golden Rule 10 (efficiency first) — the default must be the most
    // performant type. If x becomes `number`, arithmetic is decimal-128 overhead.
    let output = assert_errors("function entrypoint() -> nothing { let x = 42\nprint(x.toString()) }", 0);
    let int_ty = output
        .typed_module
        .expr_types
        .values()
        .find(|t| **t == Type::Int);
    assert!(int_ty.is_some(), "42 should infer as int");
}

#[test]
fn int_literal_retypes_as_number_with_annotation() {
    // WHY: `let x: number = 42` must store x as `number`, not `int`.
    // The annotation context must override the default literal inference.
    assert_clean("function entrypoint() -> nothing { let x: number = 42\nprint(x) }");
}

#[test]
fn int_literal_retypes_as_float_with_annotation() {
    // WHY: `let x: float = 42` must store x as `float`. Same annotation override.
    assert_clean("function entrypoint() -> nothing { let x: float = 42\nprint(x) }");
}

#[test]
fn number_literal_infers_as_number() {
    // WHY: `3.14` has a decimal point so it must infer as `number` (decimal128),
    // not float. `float` would silently corrupt exact decimal values.
    assert_clean("function entrypoint() -> nothing { let x = 3.14\nprint(x) }");
}

#[test]
fn number_literal_retypes_as_float_with_annotation() {
    // WHY: `let x: float = 1.0` must store x as `float`. The user explicitly
    // chose binary floating-point — the annotation governs.
    assert_clean("function entrypoint() -> nothing { let x: float = 1.0\nprint(x) }");
}


#[test]
fn int_annotation_rejects_number_literal() {
    // WHY: `let x: int = 1.5` must error — you can't store a decimal in an int.
    // The type checker must catch this before codegen tries to emit an alloca.
    let output = assert_errors(
        "function entrypoint() -> nothing { let x: int = 1.5 }",
        1,
    );
    assert!(
        output.diagnostics[0].what.contains("number") || output.diagnostics[0].what.contains("int"),
        "Diagnostic must mention the types, got: {}",
        output.diagnostics[0].what
    );
}


#[test]
fn int_arithmetic_type_checks_clean() {
    assert_clean("function entrypoint() -> nothing { let x: int = 2\nlet y: int = 3\nlet z = x + y\nprint(z) }");
}

#[test]
fn float_arithmetic_type_checks_clean() {
    assert_clean("function entrypoint() -> nothing { let x: float = 2.0\nlet y: float = 3.0\nlet z = x + y\nprint(z) }");
}

#[test]
fn number_arithmetic_type_checks_clean() {
    assert_clean("function entrypoint() -> nothing { let x = 0.1\nlet y = 0.2\nlet z = x + y\nprint(z) }");
}

#[test]
fn int_plus_number_is_type_error_with_to_number_suggestion() {
    // WHY: `int + number` is the most common numeric mismatch. The diagnostic
    // MUST suggest `.toNumber()` specifically — a generic "types differ" message
    // leaves the user without a fix.
    let output = assert_errors(
        "function entrypoint() -> nothing { let a: int = 1\nlet b: number = 2.0\nlet c = a + b }",
        1,
    );
    assert!(
        output.diagnostics[0].what_instead.contains("toNumber"),
        "Suggestion must mention `toNumber`, got: {}",
        output.diagnostics[0].what_instead
    );
}

#[test]
fn int_plus_float_is_type_error_with_to_float_suggestion() {
    // WHY: `int + float` must suggest `.toFloat()`, not `.toNumber()`.
    // Wrong suggestion sends the developer down the wrong conversion path.
    let output = assert_errors(
        "function entrypoint() -> nothing { let a: int = 1\nlet b: float = 2.0\nlet c = a + b }",
        1,
    );
    assert!(
        output.diagnostics[0].what_instead.contains("toFloat"),
        "Suggestion must mention `toFloat`, got: {}",
        output.diagnostics[0].what_instead
    );
}

#[test]
fn number_plus_float_is_type_error_with_both_options() {
    // WHY: `number + float` has no clear "safe" direction — both conversions
    // lose precision in different ways. The diagnostic must explain BOTH options
    // and the tradeoff. This is a teaching opportunity: the user needs to
    // consciously choose the right direction.
    let output = assert_errors(
        "function entrypoint() -> nothing { let a = 0.1\nlet b: float = 0.2\nlet c = a + b }",
        1,
    );
    let suggestion = &output.diagnostics[0].what_instead;
    assert!(
        suggestion.contains("toFloat") && suggestion.contains("toNumber"),
        "Suggestion must mention both conversion directions, got: {suggestion}"
    );
}

#[test]
fn percent_on_number_produces_specific_error() {
    // WHY: `number % number` emits a special error pointing at the `math` module
    // in v0.7, not just "type mismatch". If the message is generic, the developer
    // doesn't know that `.rem()` is the right approach.
    let output = assert_errors(
        "function entrypoint() -> nothing { let a = 0.1\nlet b = 0.2\nlet c = a % b }",
        1,
    );
    assert!(
        output.diagnostics[0].what.contains("%") || output.diagnostics[0].what_instead.contains("math"),
        "Diagnostic must mention % or the math module, got: {:#?}",
        output.diagnostics[0]
    );
}

#[test]
fn bool_less_than_int_is_type_error() {
    // WHY: `1 < 2 < 3` parses as `(1 < 2) < 3` — the outer `<` is `bool < int`,
    // which is a type error. This catches comparison chaining which silently
    // passes in many languages but produces wrong results.
    assert_errors(
        "function entrypoint() -> nothing { let x = 1 < 2 < 3 }",
        1,
    );
}


#[test]
fn comparison_result_type_is_bool() {
    // WHY: `a < b` must produce `bool`. If it produced `int` or `number`, the
    // boolean operators `&&` and `||` would fail when applied to comparison results.
    assert_clean("function entrypoint() -> nothing { let x: int = 1\nlet y: int = 2\nlet z = x < y\nprint(z) }");
}


#[test]
fn bool_and_type_checks_clean() {
    assert_clean("function entrypoint() -> nothing { let a = true\nlet b = false\nlet c = a && b\nprint(c) }");
}

#[test]
fn int_and_bool_is_type_error() {
    // WHY: `42 && true` looks plausible to a beginner but `&&` only accepts bool.
    assert_errors("function entrypoint() -> nothing { let x = 42 && true }", 1);
}


#[test]
fn unary_neg_on_int_type_checks_clean() {
    assert_clean("function entrypoint() -> nothing { let x: int = 5\nlet y = -x\nprint(y) }");
}

#[test]
fn unary_not_on_bool_type_checks_clean() {
    assert_clean("function entrypoint() -> nothing { let a = true\nlet b = !a\nprint(b) }");
}

#[test]
fn unary_neg_on_bool_is_type_error() {
    // WHY: `-true` makes no mathematical sense. Must error, not silently coerce.
    assert_errors("function entrypoint() -> nothing { let x = -true }", 1);
}


#[test]
fn const_reassignment_is_error() {
    // WHY: `const` expresses the intent that a value does not change. Allowing
    // reassignment would break the contract and confuse readers who see `const`.
    let output = assert_errors(
        "function entrypoint() -> nothing { const x = 1\nx = 2 }",
        1,
    );
    assert!(
        output.diagnostics[0].what.contains("const"),
        "Diagnostic must mention `const`, got: {}",
        output.diagnostics[0].what
    );
    assert!(
        output.diagnostics[0].what_instead.contains("let"),
        "Suggestion must mention `let`, got: {}",
        output.diagnostics[0].what_instead
    );
}


#[test]
fn undefined_identifier_produces_diagnostic() {
    // WHY: referencing an undefined name must error at compile time.
    let output = assert_errors(
        r#"function entrypoint() -> nothing { unknownIdent() }"#,
        1,
    );
    assert!(
        output.diagnostics[0].what.contains("unknownIdent"),
        "Diagnostic must name the unknown identifier, got: {}",
        output.diagnostics[0].what
    );
}

#[test]
fn levenshtein_suggestion_for_similar_name() {
    // WHY: typos like `primt` instead of `print` are common. The suggestion
    // eliminates a frustrating "name not found" with no hint.
    // `conut` is a transposition of `count` — Levenshtein distance 2, within threshold.
    let output = assert_errors(
        "function entrypoint() -> nothing { let count = 42\nlet x = conut }",
        1,
    );
    assert!(
        output.diagnostics[0].what_instead.contains("count"),
        "Suggestion should mention the close name `count`, got: {}",
        output.diagnostics[0].what_instead
    );
}

#[test]
fn undefined_in_assignment_produces_diagnostic() {
    // WHY: assigning to a name that doesn't exist must error, not silently
    // declare a new variable.
    assert_errors("function entrypoint() -> nothing { x = 42 }", 1);
}


#[test]
fn print_with_two_args_produces_arity_error() {
    // WHY: `print(1, 2)` parses fine (parser doesn't enforce arity) but typeck
    // must catch it. Without this check, the user gets a confusing codegen error
    // instead of a clear teaching diagnostic.
    let output = assert_errors(
        r#"function entrypoint() -> nothing { print(1, 2) }"#,
        1,
    );
    assert!(
        output.diagnostics[0].what.contains("print") && output.diagnostics[0].what.contains("1"),
        "Diagnostic must mention print and argument count, got: {}",
        output.diagnostics[0].what
    );
}

#[test]
fn print_with_each_primitive_type_is_clean() {
    // WHY: `print` is polymorphic. If any primitive type fails to resolve,
    // the whole M2 type surface is broken for printing.
    assert_clean(r#"function entrypoint() -> nothing { print(42) }"#);
    assert_clean(r#"function entrypoint() -> nothing { print(3.14) }"#);
    assert_clean(r#"function entrypoint() -> nothing { print(true) }"#);
    assert_clean("function entrypoint() -> nothing { let f: float = 1.0\nprint(f) }");
}


#[test]
fn unknown_method_produces_error_with_available_list() {
    // WHY: `1.unknownMethod()` must name the available methods on `int`.
    // A generic "method not found" without alternatives leaves the developer
    // to guess what conversions exist.
    let output = assert_errors(
        "function entrypoint() -> nothing { let x: int = 1\nlet s = x.unknownMethod() }",
        1,
    );
    let what_instead = &output.diagnostics[0].what_instead;
    assert!(
        what_instead.contains("toString") || what_instead.contains("toNumber") || what_instead.contains("toFloat"),
        "Suggestion must list available methods, got: {what_instead}"
    );
}

#[test]
fn to_string_on_int_produces_string() {
    // WHY: the return type of `.toString()` must be `string` so `print` accepts it.
    assert_clean(
        "function entrypoint() -> nothing { let x: int = 42\nlet s = x.toString()\nprint(s) }",
    );
}

#[test]
fn to_float_on_int_produces_float() {
    // WHY: the return type of `.toFloat()` must be `float` so float arithmetic works.
    assert_clean(
        "function entrypoint() -> nothing { let x: int = 5\nlet f: float = x.toFloat()\nprint(f) }",
    );
}


#[test]
fn parse_error_gate_prevents_cascade_noise() {
    // WHY: type-checking a body that has parse errors produces duplicate diagnostics
    // that confuse the developer. The gate ensures typeck is silent for functions
    // whose bodies contain error nodes from parser recovery.
    let output = run("function entrypoint() -> nothing { $ }");
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert_eq!(
        errors.len(),
        1,
        "Parse error gate should suppress typeck cascade — expected 1 error, got {}: {:#?}",
        errors.len(),
        errors
    );
}


#[test]
fn empty_file_missing_main_produces_diagnostic() {
    let output = run("");
    assert!(
        !output.diagnostics.is_empty(),
        "Empty file must produce a 'no main function' diagnostic"
    );
    assert!(
        output.diagnostics[0].what.contains("entrypoint"),
        "Diagnostic must mention 'entrypoint', got: {}",
        output.diagnostics[0].what
    );
}

#[test]
fn main_with_wrong_return_type_produces_diagnostic() {
    let output = run(r#"function entrypoint() -> string { print(`hi`) }"#);
    assert!(
        !output.diagnostics.is_empty(),
        "Wrong return type on main must produce a diagnostic"
    );
}


#[test]
fn check_re_runs_when_source_changes() {
    // WHY: check_query depends on parse_query. When source changes, salsa must
    // re-run the full pipeline. If not, stale type results persist.
    use salsa::Setter as _;

    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), "".to_string());
    let diag_count_before = check_query(&db, sf).diagnostics.len();

    sf.set_text(&mut db)
        .to(r#"function entrypoint() -> nothing { print(`hi`) }"#.to_string());
    let diag_count_after = check_query(&db, sf).diagnostics.len();

    assert!(diag_count_before > 0, "Empty file should have diagnostics");
    assert_eq!(diag_count_after, 0, "Valid M1 program should have 0 diagnostics");
}



fn assert_warnings(source: &str, expected_count: usize) -> CheckOutput {
    let output = run(source);
    let warnings: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Warning))
        .collect();
    assert_eq!(
        warnings.len(),
        expected_count,
        "Expected {expected_count} warnings, got {}:\n{:#?}",
        warnings.len(),
        warnings
    );
    output
}

#[test]
fn m3_fibonacci_type_checks_clean() {
    // WHY: the M3 headline contract. Fibonacci uses recursion, parameters, return,
    // and if. If any of these paths is broken, this test catches it first.
    assert_clean(
        r#"function fib(n: int) -> int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}
function entrypoint() -> nothing {
  let result = fib(10)
  print(result)
}"#,
    );
}

#[test]
fn m3_mutual_recursion_type_checks_clean() {
    // WHY: mutual recursion requires the two-pass design — `ping` calls `pong`
    // and vice versa. Without signature pre-pass, one call sees undefined-function.
    assert_clean(
        r#"function ping(n: int) -> int {
  if (n <= 0) {
    return 0
  }
  return pong(n - 1)
}
function pong(n: int) -> int {
  if (n <= 0) {
    return 0
  }
  return ping(n - 1)
}
function entrypoint() -> nothing {
  print(ping(5))
}"#,
    );
}

#[test]
fn m3_while_loop_type_checks_clean() {
    // WHY: while loop with a bool condition must type-check clean.
    assert_clean(
        r#"function entrypoint() -> nothing {
  let x: int = 5
  while (x > 0) {
    x = x - 1
  }
  print(x)
}"#,
    );
}

#[test]
fn m3_for_range_loop_type_checks_clean() {
    // WHY: `for (i in range(0, 5))` — the canonical loop form — must type-check
    // clean with `i` typed as `int` inside the body.
    assert_clean(
        r#"function entrypoint() -> nothing {
  for (i in range(0, 5)) {
    print(i)
  }
}"#,
    );
}

#[test]
fn m3_for_range_one_arg_type_checks_clean() {
    // WHY: `range(end)` (one-argument form) must also type-check clean.
    assert_clean(r#"function entrypoint() -> nothing { for (i in range(5)) { print(i) } }"#);
}

#[test]
fn m3_multi_case_int_type_checks_clean() {
    // WHY: multi-case `if` with int arms must type-check without errors.
    // If the scrutinee and pattern types match and the arms type-check, no errors.
    assert_clean(
        r#"function entrypoint() -> nothing {
  let x: int = 2
  if (x) {
    1 => print(`one`)
    2 => print(`two`)
    else => print(`other`)
  }
}"#,
    );
}

#[test]
fn m3_multi_case_string_type_checks_clean() {
    // WHY: multi-case `if` on a string scrutinee with string patterns. String
    // comparison in multi-case ships in M3 (byte-equality via `ynz_string_eq`
    // in codegen). Typeck just checks pattern types match the scrutinee.
    assert_clean(
        r#"function entrypoint() -> nothing {
  let s = `hello`
  if (s) {
    `hello` => print(`hi`)
    else => print(`bye`)
  }
}"#,
    );
}

#[test]
fn m3_return_with_correct_type_is_clean() {
    // WHY: `return 42` in a `-> int` function — the simplest happy-path return.
    assert_clean(r#"function answer() -> int { return 42 }
function entrypoint() -> nothing { print(answer()) }"#);
}

#[test]
fn m3_return_nothing_in_nothing_fn_is_clean() {
    // WHY: bare `return` in a `-> nothing` function — valid early exit.
    assert_clean(r#"function entrypoint() -> nothing { return }"#);
}

#[test]
fn m3_nested_calls_type_check_clean() {
    // WHY: `add(add(1, 2), add(3, 4))` — nested call expressions. Each call's
    // return type must flow correctly as the arg type of the outer call.
    assert_clean(
        r#"function add(a: int, b: int) -> int { return a + b }
function entrypoint() -> nothing { print(add(add(1, 2), add(3, 4))) }"#,
    );
}

#[test]
fn m3_multicase_fall_through_ok_for_nothing_fn() {
    // WHY: a non-exhaustive multi-case (no else_arm) is NOT a missing-return error
    // in a `-> nothing` function — fall-through is fine because the function
    // doesn't need to produce a value.
    assert_clean(
        r#"function entrypoint() -> nothing {
  let x: int = 3
  if (x) {
    1 => print(`one`)
    2 => print(`two`)
  }
  print(`done`)
}"#,
    );
}

#[test]
fn duplicate_function_name_produces_diagnostic() {
    // WHY: two functions named `foo` must produce exactly 1 error naming both spans.
    // Silent acceptance would mean the second definition silently overwrites the first.
    let out = assert_errors(
        r#"function foo() -> nothing { }
function foo() -> nothing { }
function entrypoint() -> nothing { }"#,
        1,
    );
    assert!(out.diagnostics[0].what.contains("foo"), "diagnostic must name the duplicate function");
}

#[test]
fn missing_main_produces_diagnostic() {
    // WHY: guard M1's invariant — a module without entrypoint is a compile error.
    // This must hold even with multi-function M3 modules.
    let out = assert_errors(
        r#"function helper() -> nothing { }"#,
        1,
    );
    assert!(out.diagnostics[0].what.contains("entrypoint"));
}

#[test]
fn main_with_parameters_produces_diagnostic() {
    // WHY: `entrypoint` must have no parameters. The signature pre-pass catches this.
    assert_errors(r#"function entrypoint(x: int) -> nothing { }"#, 1);
}

#[test]
fn m3_main_with_non_nothing_return_type_produces_diagnostic() {
    // WHY: `entrypoint() -> int` is wrong. The signature pre-pass catches this.
    let out = assert_errors(r#"function entrypoint() -> int { return 0 }"#, 1);
    assert!(out.diagnostics[0].what.contains("entrypoint"));
}

#[test]
fn parameter_mutation_produces_m4_deferral() {
    // WHY: assigning to a parameter is a compile error in M3 (ownership annotations
    // land in M4). The error must name the parameter and mention M4 so the user
    // knows what to expect and when.
    let out = assert_errors(
        r#"function foo(x: int) -> int { x = 5 return x }
function entrypoint() -> nothing { print(foo(1)) }"#,
        1,
    );
    assert!(out.diagnostics[0].what.contains("x"));
    assert!(out.diagnostics[0].why.contains("milestone 4"));
}

#[test]
fn loop_var_mutation_produces_diagnostic() {
    // WHY: assigning to the for-loop variable inside the loop body is a compile error.
    // The loop variable is the iteration counter — mutating it would make loop
    // behavior unpredictable (skip iterations, run forever, etc.).
    let out = assert_errors(
        r#"function entrypoint() -> nothing { for (i in range(0, 5)) { i = 10 } }"#,
        1,
    );
    assert!(out.diagnostics[0].what.contains("i"));
}

#[test]
fn wrong_return_type_produces_diagnostic() {
    // WHY: `return `hi`` in a `-> int` function must produce a type-mismatch
    // diagnostic pointing at the wrong expression, not the whole function.
    let out = assert_errors(
        r#"function foo() -> int { return `hi` }
function entrypoint() -> nothing { print(foo()) }"#,
        1,
    );
    assert!(out.diagnostics.iter().any(|d| d.what.contains("int") || d.what.contains("string")));
}

#[test]
fn missing_return_produces_diagnostic() {
    // WHY: a `-> int` function with no `return` on all paths must error.
    // Without this check, the function silently exits with an undefined value.
    // test-ratchet: M7 P1 — migrated to backtick syntax
    let out = assert_errors(
        "function foo() -> int { print(`no return`) }\nfunction entrypoint() -> nothing { print(foo()) }",
        1,
    );
    assert!(out.diagnostics.iter().any(|d| d.what.contains("foo")));
}

#[test]
fn return_without_value_in_non_nothing_fn_produces_diagnostic() {
    // WHY: bare `return` in a `-> int` function is wrong — the function
    // promised a value but returns nothing.
    assert_errors(
        r#"function foo() -> int { return }
function entrypoint() -> nothing { print(foo()) }"#,
        1,
    );
}

#[test]
fn return_with_value_in_nothing_fn_produces_diagnostic() {
    // WHY: `return 1` inside a `-> nothing` function is a contradiction —
    // the function said it produces no value, then tries to return one.
    assert_errors(r#"function entrypoint() -> nothing { return 1 }"#, 1);
}

#[test]
fn dead_code_after_return_produces_warning() {
    // WHY: `return 1; print(2)` — `print(2)` is unreachable. A warning (not error)
    // so the function still compiles, but the user is informed. Silently ignoring
    // dead code hides bugs (e.g. a return that was meant to be conditional).
    let out = assert_warnings(
        r#"function foo() -> int { return 1 print(2) }
function entrypoint() -> nothing { print(foo()) }"#,
        1,
    );
    assert!(out.diagnostics.iter().any(|d| matches!(d.severity, ynz_diagnostics::Severity::Warning)));
}

#[test]
fn multicase_non_exhaustive_in_non_nothing_fn_produces_diagnostic() {
    // WHY: `function foo(x: int) -> int { if (x) { 1 => return 1 } }` —
    // no else_arm means there's a fall-through path where no value is returned.
    assert_errors(
        r#"function foo(x: int) -> int {
  if (x) {
    1 => return 1
    2 => return 2
  }
}
function entrypoint() -> nothing { print(foo(1)) }"#,
        1,
    );
}

#[test]
fn multicase_with_else_all_arms_return_is_clean() {
    // WHY: exhaustive multi-case with else_arm — all paths return, no error.
    assert_clean(
        r#"function foo(x: int) -> int {
  if (x) {
    1 => return 1
    else => return 0
  }
}
function entrypoint() -> nothing { print(foo(1)) }"#,
    );
}

#[test]
fn if_condition_must_be_bool() {
    // WHY: `if (42) { ... }` — integer condition must produce a diagnostic.
    // JavaScript-style truthy coercion is explicitly rejected in Yinz.
    assert_errors(
        r#"function entrypoint() -> nothing { if (42) { print(`hi`) } }"#,
        1,
    );
}

#[test]
fn while_condition_must_be_bool() {
    // WHY: `while (1) { ... }` — same as if: no truthy coercion.
    assert_errors(
        r#"function entrypoint() -> nothing { let x: int = 1 while (x) { x = x - 1 } }"#,
        1,
    );
}

#[test]
fn range_outside_for_produces_m7_deferral() {
    // WHY: `let r = range(0, 5)` — range values are first-class from M7 onward.
    // Storing a range in a variable is now valid; the M3 restriction is lifted.
    // test-ratchet: M7 P3c removes the range-outside-for restriction; the test
    // now verifies that storing a range produces NO error (non-regression).
    assert_clean(r#"function entrypoint() -> nothing { let r = range(0, 5) }"#); // test-ratchet: M7 P3c lifts M3 range-outside-for restriction
}

#[test]
fn range_wrong_arity_produces_diagnostic() {
    // WHY: `range(1, 2, 3)` — only 1 or 2 args accepted. Three args must error.
    assert_errors(
        r#"function entrypoint() -> nothing { for (i in range(0, 5, 1)) { print(i) } }"#,
        1,
    );
}

#[test]
fn range_wrong_arg_type_produces_diagnostic() {
    // WHY: `range(`hi`)` — range requires `int` args. A string arg must error.
    assert_errors(
        r#"function entrypoint() -> nothing { for (i in range(`hi`)) { print(i) } }"#,
        1,
    );
}

#[test]
fn undefined_function_produces_diagnostic_with_levenshtein() {
    // WHY: `unknownFn()` must produce an error. With a close enough name (`main`
    // vs `mann`), the "did you mean" suggestion must fire.
    let out = assert_errors(
        r#"function entrypoint() -> nothing { entrpoint() }"#,
        1,
    );
    assert!(
        out.diagnostics[0].what_instead.contains("entrypoint"),
        "Levenshtein must suggest `entrypoint` for `entrpoint`, got: {:?}",
        out.diagnostics[0].what_instead
    );
}

#[test]
fn function_arg_type_mismatch_produces_diagnostic() {
    // WHY: `function foo(x: int) -> nothing { }; foo(`hi`)` — string passed
    // where int expected. Must produce exactly 1 type-mismatch diagnostic.
    let out = assert_errors(
        r#"function foo(x: int) -> nothing { }
function entrypoint() -> nothing { foo(`hi`) }"#,
        1,
    );
    assert!(out.diagnostics[0].what.contains("int") || out.diagnostics[0].what.contains("string"));
}

#[test]
fn function_arg_arity_mismatch_produces_diagnostic() {
    // WHY: `foo(1, 2)` when `foo` takes 1 arg must error with arity count.
    let out = assert_errors(
        r#"function foo(x: int) -> nothing { }
function entrypoint() -> nothing { foo(1, 2) }"#,
        1,
    );
    assert!(out.diagnostics[0].what.contains("foo"));
}

#[test]
fn parse_error_gate_still_works_in_m3() {
    // WHY: M1's gate: if a function body has a parse error, skip body typechecking.
    // This must survive the M3 refactor so parse errors don't cascade into
    // confusing "undefined identifier" errors on error-recovery nodes.
    // `let x = $` produces: (1) lex error on `$`, (2) parse error on the
    // missing expression. Both are lex/parse errors — typeck must not add more.
    let out = run(r#"function entrypoint() -> nothing { let x = $ }"#);
    let errors: Vec<_> = out.diagnostics.iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    // All errors must be parse/lex errors, not typeck errors.
    // Typeck cascade would produce "undefined `x`" or type-mismatch errors.
    for e in &errors {
        assert!(
            !e.what.contains("is not defined") && !e.what.contains("type"),
            "typeck must not cascade after parse error, but got: {:?}",
            e.what
        );
    }
    assert!(errors.len() <= 2, "At most lex + parse error expected, got {}", errors.len());
}

#[test]
fn while_true_with_no_break_is_missing_return_error() {
    // WHY: `function foo() -> int { while (true) { } }` — the typeck does NOT
    // constant-fold `true`, so while loops always look like "may-not-execute."
    // A non-nothing function that only has a while loop must get a missing-return
    // error. The diagnostic must guide the user to add a `return` inside the body.
    assert_errors(
        r#"function foo() -> int { while (true) { } }
function entrypoint() -> nothing { print(foo()) }"#,
        1,
    );
}

#[test]
fn empty_function_body_non_nothing_is_missing_return() {
    // WHY: `function foo() -> int { }` — zero statements, no return. Must error.
    // Edge case for `analyze_return_paths` with an empty block.
    assert_errors(
        r#"function foo() -> int { }
function entrypoint() -> nothing { print(foo()) }"#,
        1,
    );
}

#[test]
fn for_loop_var_is_typed_as_int() {
    // WHY: inside `for (i in range(0, 5))`, `i` must be `int` so it can be
    // passed to `print` without a `.toString()` call. If `i` is Error or
    // unknown, print(i) would produce a false type error.
    assert_clean(
        r#"function entrypoint() -> nothing { for (i in range(0, 10)) { print(i) } }"#,
    );
}

#[test]
fn module_signatures_query_is_separate_from_check_query() {
    // WHY: validates the two-pass salsa design. module_signatures_query must
    // exist and return the same diagnostics as check_query for signature errors.
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), "function helper() -> nothing { }".to_string());
    let sig_out = ynz_typeck::module_signatures_query(&db, sf);
    // No main → 1 error in sig pass
    assert_eq!(sig_out.diagnostics.len(), 1, "Missing main must appear in signature output");
    // check_query should also have the error (it includes sig diags)
    let check_out = check_query(&db, sf);
    assert!(check_out.diagnostics.len() >= 1, "Missing main must appear in check output");
}

// ── M4 P3a: shape type-checking tests ────────────────────────────────────────

#[test]
fn shape_with_struct_literal_type_checks() {
    // WHY: The end-to-end path — `shape Player { ... }` declared, then `let p: Player = { ... }`
    // constructed — must type-check cleanly. This is the simplest shape program that uses
    // all of P3a's new code paths together.
    assert_clean(r#"
shape Player {
  name: string
  health: int
}

function entrypoint() -> nothing {
  let p: Player = { name: `Patrick`, health: 100 }
  print(p.health)
}
"#);
}

#[test]
fn struct_lit_missing_field_produces_error() {
    // WHY: A struct literal that doesn't provide all required fields must error.
    // Without this check, shapes could be partially initialized with garbage values.
    let out = assert_errors(r#"
shape Player { name: string health: int }
function entrypoint() -> nothing { let p: Player = { name: `x` } }
"#, 1);
    let e = &out.diagnostics[0];
    assert!(e.what.contains("health") || e.what.contains("Missing"),
        "Error must name the missing field, got: {:?}", e.what);
}

#[test]
fn struct_lit_wrong_field_type_produces_error() {
    // WHY: Passing an int where a string is expected must be caught.
    assert_errors(r#"
shape Player { name: string }
function entrypoint() -> nothing { let p: Player = { name: 42 } }
"#, 1);
}

#[test]
fn struct_lit_unknown_field_produces_error() {
    // WHY: Providing a field that doesn't exist on the shape must error.
    assert_errors(r#"
shape Player { name: string }
function entrypoint() -> nothing { let p: Player = { name: `x`, age: 30 } }
"#, 1);
}

#[test]
fn field_access_resolves_correct_type() {
    // WHY: `p.health` where health is `int` must produce an int — not Error.
    // The int literal binop check below confirms the field resolved to int.
    assert_clean(r#"
shape Player { name: string health: int }
function entrypoint() -> nothing {
  let p: Player = { name: `x`, health: 100 }
  let doubled = p.health * 2
  print(doubled)
}
"#);
}

#[test]
fn field_access_unknown_field_produces_error() {
    // WHY: `p.nonexistent` must error at the field-access site, not silently produce Error
    // which would cascade into cascading spurious errors everywhere the value is used.
    assert_errors(r#"
shape Player { name: string }
function entrypoint() -> nothing {
  let p: Player = { name: `x` }
  let x = p.age
}
"#, 1);
}

#[test]
fn field_assignment_type_checks() {
    // WHY: `p.health = 50` where health is `int` must check that 50 is `int`.
    assert_clean(r#"
shape Player { name: string health: int }
function entrypoint() -> nothing {
  let p: Player = { name: `x`, health: 100 }
  p.health = 50
}
"#);
}

#[test]
fn field_assignment_type_mismatch_produces_error() {
    // WHY: `p.health = `fifty`` must error — field is int, value is string.
    assert_errors(r#"
shape Player { name: string health: int }
function entrypoint() -> nothing {
  let p: Player = { name: `x`, health: 100 }
  p.health = `fifty`
}
"#, 1);
}

#[test]
fn const_field_assignment_produces_error() {
    // WHY: `const p` bindings are fully immutable — field mutation must be rejected.
    // This is one of the five const deep-immutability paths from the plan invariants.
    let out = assert_errors(r#"
shape Player { name: string health: int }
function entrypoint() -> nothing {
  const p: Player = { name: `x`, health: 100 }
  p.health = 50
}
"#, 1);
    assert!(out.diagnostics[0].what.contains("const") || out.diagnostics[0].what.contains("const"),
        "Error must mention `const`, got: {:?}", out.diagnostics[0].what);
}

#[test]
fn base_shape_instantiation_produces_error() {
    // WHY: `base shape Entity` cannot be constructed via struct literal.
    // Typeck must reject it at the construction site.
    assert_errors(r#"
base shape Entity { name: string }
function entrypoint() -> nothing { let e: Entity = { name: `x` } }
"#, 1);
}

#[test]
fn hidden_field_outside_shape_produces_error() {
    // WHY: `hidden` fields are visible only to functions with `self: ShapeName`.
    // Reading them from an outside function must error.
    assert_errors(r#"
shape Player { name: string hidden cache: int }
function entrypoint() -> nothing {
  let p: Player = { name: `x` }
  let x = p.cache
}
"#, 1);
}

#[test]
fn self_parameter_resolves_to_shape_type() {
    // WHY: A function with `share self: Player` must be able to access `self.name`
    // and have `self` resolve to `Type::Shape { name: "Player" }`.
    assert_clean(r#"
shape Player { name: string health: int }
function greet(share self: Player) -> string { return self.name }
function entrypoint() -> nothing {
  let p: Player = { name: `Patrick`, health: 100 }
  let g = greet(p)
  print(g)
}
"#);
}

#[test]
fn ufcs_method_call_on_shape_resolves() {
    // WHY: `player.greet()` (UFCS sugar for `greet(player)`) must resolve the function
    // and return its declared return type. This is the primary M4 interaction pattern.
    assert_clean(r#"
shape Player { name: string }
function greet(share self: Player) -> string { return self.name }
function entrypoint() -> nothing {
  let p: Player = { name: `Patrick` }
  let msg = p.greet()
  print(msg)
}
"#);
}

#[test]
fn duplicate_shape_name_produces_error() {
    // WHY: Two shapes with the same name in one file must error — otherwise the
    // second silently shadows the first and field lookups become nondeterministic.
    assert_errors(r#"
shape Player { name: string }
shape Player { health: int }
function entrypoint() -> nothing { }
"#, 1);
}

#[test]
fn struct_lit_without_annotation_produces_error() {
    // WHY: Anonymous struct literals need a type annotation to know which shape
    // to validate against. Without one the compiler cannot check field names.
    assert_errors(r#"
shape Player { name: string }
function entrypoint() -> nothing { let p = { name: `x` } }
"#, 1);
}

// ── M4 P3b: inheritance + follows + dynamic tests ────────────────────────────

#[test]
fn extends_inherits_parent_fields() {
    // WHY: A child shape via `extends` must be able to access all parent fields.
    // The field is resolved through the flattened field list in ShapeDef.
    assert_clean(r#"
shape Entity { name: string }
shape Player extends Entity { health: int }
function entrypoint() -> nothing {
  let p: Player = { name: `Patrick`, health: 100 }
  print(p.name)
  print(p.health)
}
"#);
}

#[test]
fn extends_unknown_parent_produces_error() {
    // WHY: `shape Player extends Ghost` where Ghost is not defined must error
    // at the extends declaration, not silently produce a broken shape.
    assert_errors(r#"
shape Player extends Ghost { health: int }
function entrypoint() -> nothing { }
"#, 1);
}

#[test]
fn follows_satisfied_type_checks() {
    // WHY: `shape Player follows Damageable` with a matching standalone function
    // must type-check cleanly — the contract is satisfied.
    assert_clean(r#"
shape Damageable {
  takeDamage(lend self, amount: int) -> nothing
}
shape Player follows Damageable { health: int }
function takeDamage(lend self: Player, amount: int) -> nothing {
  self.health = self.health - amount
}
function entrypoint() -> nothing {
  let p: Player = { health: 100 }
  takeDamage(p, 10)
}
"#);
}

#[test]
fn follows_missing_function_produces_error() {
    // WHY: If a shape follows a contract but the required function is missing,
    // the compiler must catch it and name which function is absent.
    let out = assert_errors(r#"
shape Damageable { takeDamage(lend self, amount: int) -> nothing }
shape Player follows Damageable { health: int }
function entrypoint() -> nothing { }
"#, 1);
    assert!(out.diagnostics[0].what.contains("takeDamage") || out.diagnostics[0].what.contains("missing"),
        "Error must name the missing function, got: {:?}", out.diagnostics[0].what);
}

#[test]
fn follows_wrong_return_type_produces_error() {
    // WHY: A function whose return type doesn't match the contract sig's return type
    // must produce a clear mismatch diagnostic.
    assert_errors(r#"
shape Greetable { greet(share self) -> string }
shape Player follows Greetable { name: string }
function greet(share self: Player) -> int { return 42 }
function entrypoint() -> nothing { }
"#, 1);
}

#[test]
fn type_variant_count_includes_dynamic() {
    // WHY: Dynamic was added in P3b. Pins the count to catch accidental variant additions.
    //
    // test-ratchet: P3b adds Dynamic for runtime polymorphism. Count 9 → 10.
    let all: &[Type] = &[
        Type::Nothing, Type::String, Type::Error,
        Type::Int, Type::Float, Type::Number { precision: 34 }, Type::Bool,
        Type::Range { element: Box::new(Type::Int), end_inclusive: false },
        Type::Shape { name: "Player".into() },
        Type::Dynamic { contract: "Damageable".into() },
    ];
    assert_eq!(all.len(), 10, "Type variant count changed from 10 — add // test-ratchet: comment");
}

// ── M4 P3c: ownership analysis tests ─────────────────────────────────────────

#[test]
fn give_param_consumes_binding() {
    // WHY: When a function takes a `give` parameter, the caller's binding is consumed.
    // Using it afterward must produce a use-after-give error.
    let out = assert_errors(r#"
shape Player { name: string }
function consume(give p: Player) -> nothing { }
function entrypoint() -> nothing {
  let p: Player = { name: `Patrick` }
  consume(p)
  let x = p.name
}
"#, 1);
    assert!(out.diagnostics[0].what.contains("given away"),
        "Error must mention the binding was given away, got: {:?}", out.diagnostics[0].what);
}

#[test]
fn const_binding_cannot_be_given() {
    // WHY: `const` blocks the `give` path — ownership cannot be transferred out
    // of a const binding. Const deep-immutability path #3.
    assert_errors(r#"
shape Player { name: string }
function consume(give p: Player) -> nothing { }
function entrypoint() -> nothing {
  const p: Player = { name: `Patrick` }
  consume(p)
}
"#, 1);
}

#[test]
fn const_binding_cannot_be_lent() {
    // WHY: `const` blocks the `lend` path. Const deep-immutability path #2.
    assert_errors(r#"
shape Player { health: int }
function damage(lend self: Player, amount: int) -> nothing {
  self.health = self.health - amount
}
function entrypoint() -> nothing {
  const p: Player = { health: 100 }
  damage(p, 10)
}
"#, 1);
}

#[test]
fn give_twice_is_use_after_give() {
    // WHY: After a value is given, the second give must see it as consumed.
    assert_errors(r#"
shape Player { name: string }
function consume(give p: Player) -> nothing { }
function entrypoint() -> nothing {
  let p: Player = { name: `Patrick` }
  consume(p)
  consume(p)
}
"#, 1);
}

#[test]
fn share_on_const_is_allowed() {
    // WHY: A `share` (read-only) parameter can receive a const binding.
    assert_clean(r#"
shape Player { name: string }
function greet(share p: Player) -> string { return p.name }
function entrypoint() -> nothing {
  const p: Player = { name: `Patrick` }
  let g = greet(p)
  print(g)
}
"#);
}

#[test]
fn unspecified_param_accepts_const() {
    // WHY: No ownership modifier defaults to share semantics. Const bindings are fine.
    assert_clean(r#"
shape Player { name: string }
function greet(p: Player) -> string { return p.name }
function entrypoint() -> nothing {
  const p: Player = { name: `Patrick` }
  let g = greet(p)
  print(g)
}
"#);
}

// ── M6: union typeck (P3b) ───────────────────────────────────────────────────

#[test]
fn m6_union_type_annotation_works() {
    // WHY: A union type `let s: Circle | Square = c` must parse and typecheck without errors.
    // Assigning a concrete variant (Circle) to a union type must be valid.
    assert_clean(r#"
shape Circle { radius: number }
shape Square { side: number }
shape Shape = Circle | Square
function describe(s: Shape) -> nothing {
  if (s) {
    is Circle => print(`circle`)
    is Square => print(`square`)
  }
}
function entrypoint() -> nothing {
  let c: Circle = { radius: 5.0 }
  let s: Shape = c
  describe(s)
}
"#);
}

#[test]
fn m6_union_multicase_exhaustive_clean() {
    // WHY: A fully-covered union multi-case must typecheck cleanly.
    assert_clean(r#"
shape Circle { radius: number }
shape Square { side: number }
shape Shape = Circle | Square
function classify(s: Shape) -> nothing {
  if (s) {
    is Circle => print(`circle`)
    is Square => print(`square`)
  }
}
function entrypoint() -> nothing { }
"#);
}

#[test]
fn m6_union_multicase_nonexhaustive_is_error() {
    // WHY: A missing `is Foo` arm in a union multi-case must produce an error.
    assert_errors(r#"
shape Circle { radius: number }
shape Square { side: number }
shape Triangle { width: number
  height: number }
shape Shape = Circle | Square | Triangle
function classify(s: Shape) -> nothing {
  if (s) {
    is Circle => print(`circle`)
    is Square => print(`square`)
  }
}
function entrypoint() -> nothing { }
"#, 1);
}

#[test]
fn m6_union_multicase_with_else_is_clean() {
    // WHY: `else =>` covers all remaining variants — must typecheck clean.
    assert_clean(r#"
shape Circle { radius: number }
shape Square { side: number }
shape Triangle { width: number
  height: number }
shape Shape = Circle | Square | Triangle
function classify(s: Shape) -> nothing {
  if (s) {
    is Circle => print(`circle`)
    else => print(`other`)
  }
}
function entrypoint() -> nothing { }
"#);
}

#[test]
fn m6_is_expr_on_non_union_is_error() {
    // WHY: `is Foo` on a non-union (e.g., string) must produce a teaching error.
    assert_errors(r#"
function entrypoint() -> nothing {
  let s: string = `hello`
  if (s is int) {
    print(`wrong`)
  }
}
"#, 1);
}

// ── M6: options typeck ────────────────────────────────────────────────────────

#[test]
fn m6_options_value_typechecks() {
    // WHY: `Status.active` must resolve to Type::Options { name: "Status" }, not Error.
    // If this fails, every options-typed variable is mistyped and comparisons fail.
    assert_clean(r#"
options Status { active, inactive, banned }
function entrypoint() -> nothing {
  let s: Status = Status.active
  print(s.toString())
}
"#);
}

#[test]
fn m6_options_unknown_variant_is_error() {
    // WHY: Accessing a non-existent variant must produce an error, not silently type as Error.
    // A typo like `Status.activ` should be caught at compile time.
    assert_errors(r#"
options Status { active, inactive, banned }
function entrypoint() -> nothing {
  let s: Status = Status.activ
}
"#, 1);
}

#[test]
fn m6_options_empty_body_is_error() {
    // WHY: An options type with no variants can never hold a value — the compiler must reject it.
    assert_errors(r#"
options Empty { }
function entrypoint() -> nothing { }
"#, 1);
}

#[test]
fn m6_options_single_variant_is_error() {
    // WHY: Single-variant options types carry no information — symmetric with single-variant union rejection.
    assert_errors(r#"
options Single { only }
function entrypoint() -> nothing { }
"#, 1);
}

#[test]
fn m6_options_multicase_exhaustive_clean() {
    // WHY: A fully-covered options multi-case must typecheck with zero errors.
    // If this fails, correct options code is incorrectly rejected.
    assert_clean(r#"
options Status { active, inactive }
function entrypoint() -> nothing {
  let s: Status = Status.active
  if (s) {
    active => print(`ok`)
    inactive => print(`off`)
  }
}
"#);
}

#[test]
fn m6_options_multicase_nonexhaustive_is_error() {
    // WHY: A multi-case with a missing arm must produce an error naming the missing variant.
    // Missing variants silently fall through without this check — a latent bug class.
    assert_errors(r#"
options Status { active, inactive, banned }
function entrypoint() -> nothing {
  let s: Status = Status.active
  if (s) {
    active => print(`ok`)
    inactive => print(`off`)
  }
}
"#, 1);
}

#[test]
fn m6_options_multicase_with_else_arm_is_clean() {
    // WHY: An `else =>` arm covers all remaining variants — must typecheck clean even if
    // individual variants are not all listed.
    assert_clean(r#"
options Status { active, inactive, banned }
function entrypoint() -> nothing {
  let s: Status = Status.active
  if (s) {
    active => print(`ok`)
    else => print(`other`)
  }
}
"#);
}

#[test]
fn m6_same_options_comparison_clean() {
    // WHY: Comparing two values of the same options type with `==` must succeed.
    assert_clean(r#"
options Status { active, inactive }
function entrypoint() -> nothing {
  let a: Status = Status.active
  let b: Status = Status.inactive
  let eq = a == b
  print(eq.toString())
}
"#);
}

#[test]
fn m6_cross_options_comparison_is_error() {
    // WHY: Comparing values of different options types is almost always a bug —
    // the tags have no shared meaning between types.
    assert_errors(r#"
options Status { active, inactive }
options Visibility { visible, invisible }
function entrypoint() -> nothing {
  let s: Status = Status.active
  let v: Visibility = Visibility.visible
  let eq = s == v
}
"#, 1);
}

#[test]
fn m6_bool_to_int_is_error() {
    // WHY: `.toInt()` on bool must be rejected — no silent 0/1 coercion.
    assert_errors(r#"
function entrypoint() -> nothing {
  let x = true.toInt()
}
"#, 1);
}

#[test]
fn m6_int_to_int_is_clean() {
    // WHY: `.toInt()` on int must be the identity and return `int` directly (not `maybe<int>`).
    assert_clean(r#"
function entrypoint() -> nothing {
  let x: int = 42
  let y: int = x.toInt()
  print(y.toString())
}
"#);
}

#[test]
fn m6_float_to_int_returns_maybe() {
    // WHY: `.toInt()` on float is fallible — must return `maybe<int>`, not bare `int`.
    // If this returns int, NaN and OOR cases silently produce wrong values at runtime.
    assert_clean(r#"
function entrypoint() -> nothing {
  let x: float = 3.14
  let y: maybe<int> = x.toInt()
  print(y.or(0).toString())
}
"#);
}

#[test]
fn m6_string_to_int_returns_maybe() {
    // WHY: `"42".toInt()` is fallible — the string might not be a valid integer.
    // Must return `maybe<int>` so the caller handles the failure case.
    assert_clean(r#"
function entrypoint() -> nothing {
  let s: string = `42`
  let x: maybe<int> = s.toInt()
  print(x.or(0).toString())
}
"#);
}
