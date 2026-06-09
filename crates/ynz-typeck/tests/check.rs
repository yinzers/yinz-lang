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
    // test-ratchet: Tier 3 lint warnings (e.g., repeated-inline-shape suggestions) are
    // informational — they do not indicate a compile failure. assert_clean verifies that
    // source compiles error-free; warning-severity diagnostics are intentionally allowed.
    let output = run(source);
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "Expected 0 errors, got {}: {:#?}",
        errors.len(),
        errors
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
        Type::Range {
            element: Box::new(Type::Int),
            end_inclusive: false,
        },
        Type::Shape {
            name: "Player".into(),
        },
        Type::Dynamic {
            contract: "Foo".into(),
        },
    ];
    assert_eq!(
        all.len(),
        10,
        "Type variant count changed from 10 — add // test-ratchet: comment"
    );
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
    let output = assert_errors(
        "function entrypoint() -> nothing { let x = 42\nprint(x.toString()) }",
        0,
    );
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
    let output = assert_errors("function entrypoint() -> nothing { let x: int = 1.5 }", 1);
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
    assert_clean(
        "function entrypoint() -> nothing { let x = 0.1\nlet y = 0.2\nlet z = x + y\nprint(z) }",
    );
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
    // in v0.6, not just "type mismatch". If the message is generic, the developer
    // doesn't know that `.rem()` is the right approach.
    let output = assert_errors(
        "function entrypoint() -> nothing { let a = 0.1\nlet b = 0.2\nlet c = a % b }",
        1,
    );
    assert!(
        output.diagnostics[0].what.contains("%")
            || output.diagnostics[0].what_instead.contains("math"),
        "Diagnostic must mention % or the math module, got: {:#?}",
        output.diagnostics[0]
    );
}

#[test]
fn bool_less_than_int_is_type_error() {
    // WHY: `1 < 2 < 3` parses as `(1 < 2) < 3` — the outer `<` is `bool < int`,
    // which is a type error. This catches comparison chaining which silently
    // passes in many languages but produces wrong results.
    assert_errors("function entrypoint() -> nothing { let x = 1 < 2 < 3 }", 1);
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
    let output = assert_errors("function entrypoint() -> nothing { const x = 1\nx = 2 }", 1);
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
    let output = assert_errors(r#"function entrypoint() -> nothing { unknownIdent() }"#, 1);
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
    let output = assert_errors(r#"function entrypoint() -> nothing { print(1, 2) }"#, 1);
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
        what_instead.contains("toString")
            || what_instead.contains("toNumber")
            || what_instead.contains("toFloat"),
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
fn empty_file_missing_entrypoint_produces_diagnostic() {
    let output = run("");
    assert!(
        !output.diagnostics.is_empty(),
        "Empty file must produce a 'no entrypoint function' diagnostic"
    );
    assert!(
        output.diagnostics[0].what.contains("entrypoint"),
        "Diagnostic must mention 'entrypoint', got: {}",
        output.diagnostics[0].what
    );
}

#[test]
fn entrypoint_with_wrong_return_type_produces_diagnostic() {
    let output = run(r#"function entrypoint() -> string { print(`hi`) }"#);
    assert!(
        !output.diagnostics.is_empty(),
        "Wrong return type on entrypoint must produce a diagnostic"
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
    assert_eq!(
        diag_count_after, 0,
        "Valid M1 program should have 0 diagnostics"
    );
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
    assert_clean(
        r#"function answer() -> int { return 42 }
function entrypoint() -> nothing { print(answer()) }"#,
    );
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
    assert!(
        out.diagnostics[0].what.contains("foo"),
        "diagnostic must name the duplicate function"
    );
}

#[test]
fn missing_entrypoint_produces_diagnostic() {
    // WHY: guard M1's invariant — a module without entrypoint is a compile error.
    // This must hold even with multi-function M3 modules.
    let out = assert_errors(r#"function helper() -> nothing { }"#, 1);
    assert!(out.diagnostics[0].what.contains("entrypoint"));
}

#[test]
fn entrypoint_with_parameters_produces_diagnostic() {
    // WHY: `entrypoint` must have no parameters. The signature pre-pass catches this.
    assert_errors(r#"function entrypoint(x: int) -> nothing { }"#, 1);
}

#[test]
fn m3_entrypoint_with_non_nothing_return_type_produces_diagnostic() {
    // WHY: `entrypoint() -> int` is wrong. The signature pre-pass catches this.
    let out = assert_errors(r#"function entrypoint() -> int { return 0 }"#, 1);
    assert!(out.diagnostics[0].what.contains("entrypoint"));
}

#[test]
fn parameter_mutation_produces_m4_deferral() {
    // WHY: assigning to a parameter is a compile error — the read-only-param contract
    // must hold. The error must name the parameter and explain the read-only semantics.
    //
    // test-ratchet: M4 shipped — old check for "milestone 4" text was testing a
    // future-tense deferral message that is now stale. Updated to check the current
    // accurate message ("read-only by default").
    let out = assert_errors(
        r#"function foo(x: int) -> int { x = 5 return x }
function entrypoint() -> nothing { print(foo(1)) }"#,
        1,
    );
    assert!(out.diagnostics[0].what.contains("x"));
    assert!(
        out.diagnostics[0].why.contains("read-only"),
        "diagnostic WHY must explain read-only semantics, got: {:?}",
        out.diagnostics[0].why
    );
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
    assert!(out
        .diagnostics
        .iter()
        .any(|d| d.what.contains("int") || d.what.contains("string")));
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
    assert!(out
        .diagnostics
        .iter()
        .any(|d| matches!(d.severity, ynz_diagnostics::Severity::Warning)));
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
    assert_clean(r#"function entrypoint() -> nothing { let r = range(0, 5) }"#);
    // test-ratchet: M7 P3c lifts M3 range-outside-for restriction
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
    let out = assert_errors(r#"function entrypoint() -> nothing { entrpoint() }"#, 1);
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
    let errors: Vec<_> = out
        .diagnostics
        .iter()
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
    assert!(
        errors.len() <= 2,
        "At most lex + parse error expected, got {}",
        errors.len()
    );
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
    assert_clean(r#"function entrypoint() -> nothing { for (i in range(0, 10)) { print(i) } }"#);
}

#[test]
fn module_signatures_query_is_separate_from_check_query() {
    // WHY: validates the two-pass salsa design. module_signatures_query must
    // exist and return the same diagnostics as check_query for signature errors.
    let db = CompilerDb::default();
    let sf = SourceFile::new(
        &db,
        FILE.to_string(),
        "function helper() -> nothing { }".to_string(),
    );
    let sig_out = ynz_typeck::module_signatures_query(&db, sf);
    // No main → 1 error in sig pass
    assert_eq!(
        sig_out.diagnostics.len(),
        1,
        "Missing main must appear in signature output"
    );
    // check_query should also have the error (it includes sig diags)
    let check_out = check_query(&db, sf);
    assert!(
        check_out.diagnostics.len() >= 1,
        "Missing main must appear in check output"
    );
}

// ── M4 P3a: shape type-checking tests ────────────────────────────────────────

#[test]
fn shape_with_struct_literal_type_checks() {
    // WHY: The end-to-end path — `shape Player { ... }` declared, then `let p: Player = { ... }`
    // constructed — must type-check cleanly. This is the simplest shape program that uses
    // all of P3a's new code paths together.
    assert_clean(
        r#"
shape Player {
  name: string
  health: int
}

function entrypoint() -> nothing {
  let p: Player = { name: `Patrick`, health: 100 }
  print(p.health)
}
"#,
    );
}

#[test]
fn struct_lit_missing_field_produces_error() {
    // WHY: A struct literal that doesn't provide all required fields must error.
    // Without this check, shapes could be partially initialized with garbage values.
    let out = assert_errors(
        r#"
shape Player { name: string health: int }
function entrypoint() -> nothing { let p: Player = { name: `x` } }
"#,
        1,
    );
    let e = &out.diagnostics[0];
    assert!(
        e.what.contains("health") || e.what.contains("Missing"),
        "Error must name the missing field, got: {:?}",
        e.what
    );
}

#[test]
fn struct_lit_wrong_field_type_produces_error() {
    // WHY: Passing an int where a string is expected must be caught.
    assert_errors(
        r#"
shape Player { name: string }
function entrypoint() -> nothing { let p: Player = { name: 42 } }
"#,
        1,
    );
}

#[test]
fn struct_lit_unknown_field_produces_error() {
    // WHY: Providing a field that doesn't exist on the shape must error.
    assert_errors(
        r#"
shape Player { name: string }
function entrypoint() -> nothing { let p: Player = { name: `x`, age: 30 } }
"#,
        1,
    );
}

#[test]
fn field_access_resolves_correct_type() {
    // WHY: `p.health` where health is `int` must produce an int — not Error.
    // The int literal binop check below confirms the field resolved to int.
    assert_clean(
        r#"
shape Player { name: string health: int }
function entrypoint() -> nothing {
  let p: Player = { name: `x`, health: 100 }
  let doubled = p.health * 2
  print(doubled)
}
"#,
    );
}

#[test]
fn field_access_unknown_field_produces_error() {
    // WHY: `p.nonexistent` must error at the field-access site, not silently produce Error
    // which would cascade into cascading spurious errors everywhere the value is used.
    assert_errors(
        r#"
shape Player { name: string }
function entrypoint() -> nothing {
  let p: Player = { name: `x` }
  let x = p.age
}
"#,
        1,
    );
}

#[test]
fn field_assignment_type_checks() {
    // WHY: `p.health = 50` where health is `int` must check that 50 is `int`.
    assert_clean(
        r#"
shape Player { name: string health: int }
function entrypoint() -> nothing {
  let p: Player = { name: `x`, health: 100 }
  p.health = 50
}
"#,
    );
}

#[test]
fn field_assignment_type_mismatch_produces_error() {
    // WHY: `p.health = `fifty`` must error — field is int, value is string.
    assert_errors(
        r#"
shape Player { name: string health: int }
function entrypoint() -> nothing {
  let p: Player = { name: `x`, health: 100 }
  p.health = `fifty`
}
"#,
        1,
    );
}

#[test]
fn const_field_assignment_produces_error() {
    // WHY: `const p` bindings are fully immutable — field mutation must be rejected.
    // This is one of the five const deep-immutability paths from the plan invariants.
    let out = assert_errors(
        r#"
shape Player { name: string health: int }
function entrypoint() -> nothing {
  const p: Player = { name: `x`, health: 100 }
  p.health = 50
}
"#,
        1,
    );
    assert!(
        out.diagnostics[0].what.contains("const") || out.diagnostics[0].what.contains("const"),
        "Error must mention `const`, got: {:?}",
        out.diagnostics[0].what
    );
}

#[test]
fn base_shape_instantiation_produces_error() {
    // WHY: `base shape Entity` cannot be constructed via struct literal.
    // Typeck must reject it at the construction site.
    assert_errors(
        r#"
base shape Entity { name: string }
function entrypoint() -> nothing { let e: Entity = { name: `x` } }
"#,
        1,
    );
}

#[test]
fn hidden_field_outside_shape_produces_error() {
    // WHY: `hidden` fields are visible only to functions with `self: ShapeName`.
    // Reading them from an outside function must error.
    // Hidden field has a default (required since Fix A) so only the read violation fires.
    assert_errors(
        r#"
shape Player { name: string hidden cache: int = 0 }
function entrypoint() -> nothing {
  let p: Player = { name: `x` }
  let x = p.cache
}
"#,
        1,
    );
}

#[test]
fn self_parameter_resolves_to_shape_type() {
    // WHY: A function with `share self: Player` must be able to access `self.name`
    // and have `self` resolve to `Type::Shape { name: "Player" }`.
    assert_clean(
        r#"
shape Player { name: string health: int }
function greet(share self: Player) -> string { return self.name }
function entrypoint() -> nothing {
  let p: Player = { name: `Patrick`, health: 100 }
  let g = greet(p)
  print(g)
}
"#,
    );
}

#[test]
fn ufcs_method_call_on_shape_resolves() {
    // WHY: `player.greet()` (UFCS sugar for `greet(player)`) must resolve the function
    // and return its declared return type. This is the primary M4 interaction pattern.
    assert_clean(
        r#"
shape Player { name: string }
function greet(share self: Player) -> string { return self.name }
function entrypoint() -> nothing {
  let p: Player = { name: `Patrick` }
  let msg = p.greet()
  print(msg)
}
"#,
    );
}

#[test]
fn duplicate_shape_name_produces_error() {
    // WHY: Two shapes with the same name in one file must error — otherwise the
    // second silently shadows the first and field lookups become nondeterministic.
    assert_errors(
        r#"
shape Player { name: string }
shape Player { health: int }
function entrypoint() -> nothing { }
"#,
        1,
    );
}

#[test]
fn struct_lit_without_annotation_produces_error() {
    // WHY: Anonymous struct literals need a type annotation to know which shape
    // to validate against. Without one the compiler cannot check field names.
    assert_errors(
        r#"
shape Player { name: string }
function entrypoint() -> nothing { let p = { name: `x` } }
"#,
        1,
    );
}

// ── M4 P3b: inheritance + follows + dynamic tests ────────────────────────────

#[test]
fn extends_inherits_parent_fields() {
    // WHY: A child shape via `extends` must be able to access all parent fields.
    // The field is resolved through the flattened field list in ShapeDef.
    assert_clean(
        r#"
shape Entity { name: string }
shape Player extends Entity { health: int }
function entrypoint() -> nothing {
  let p: Player = { name: `Patrick`, health: 100 }
  print(p.name)
  print(p.health)
}
"#,
    );
}

#[test]
fn extends_unknown_parent_produces_error() {
    // WHY: `shape Player extends Ghost` where Ghost is not defined must error
    // at the extends declaration, not silently produce a broken shape.
    assert_errors(
        r#"
shape Player extends Ghost { health: int }
function entrypoint() -> nothing { }
"#,
        1,
    );
}

#[test]
fn follows_satisfied_type_checks() {
    // WHY: `shape Player follows Damageable` with a matching standalone function
    // must type-check cleanly — the contract is satisfied.
    assert_clean(
        r#"
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
"#,
    );
}

#[test]
fn follows_missing_function_produces_error() {
    // WHY: If a shape follows a contract but the required function is missing,
    // the compiler must catch it and name which function is absent.
    let out = assert_errors(
        r#"
shape Damageable { takeDamage(lend self, amount: int) -> nothing }
shape Player follows Damageable { health: int }
function entrypoint() -> nothing { }
"#,
        1,
    );
    assert!(
        out.diagnostics[0].what.contains("takeDamage")
            || out.diagnostics[0].what.contains("missing"),
        "Error must name the missing function, got: {:?}",
        out.diagnostics[0].what
    );
}

#[test]
fn follows_wrong_return_type_produces_error() {
    // WHY: A function whose return type doesn't match the contract sig's return type
    // must produce a clear mismatch diagnostic.
    assert_errors(
        r#"
shape Greetable { greet(share self) -> string }
shape Player follows Greetable { name: string }
function greet(share self: Player) -> int { return 42 }
function entrypoint() -> nothing { }
"#,
        1,
    );
}

#[test]
fn type_variant_count_includes_dynamic() {
    // WHY: Dynamic was added in P3b. Pins the count to catch accidental variant additions.
    //
    // test-ratchet: P3b adds Dynamic for runtime polymorphism. Count 9 → 10.
    let all: &[Type] = &[
        Type::Nothing,
        Type::String,
        Type::Error,
        Type::Int,
        Type::Float,
        Type::Number { precision: 34 },
        Type::Bool,
        Type::Range {
            element: Box::new(Type::Int),
            end_inclusive: false,
        },
        Type::Shape {
            name: "Player".into(),
        },
        Type::Dynamic {
            contract: "Damageable".into(),
        },
    ];
    assert_eq!(
        all.len(),
        10,
        "Type variant count changed from 10 — add // test-ratchet: comment"
    );
}

// ── M4 P3c: ownership analysis tests ─────────────────────────────────────────

#[test]
fn give_param_consumes_binding() {
    // WHY: When a function takes a `give` parameter, the caller's binding is consumed.
    // Using it afterward must produce a use-after-give error.
    let out = assert_errors(
        r#"
shape Player { name: string }
function consume(give p: Player) -> nothing { }
function entrypoint() -> nothing {
  let p: Player = { name: `Patrick` }
  consume(p)
  let x = p.name
}
"#,
        1,
    );
    assert!(
        out.diagnostics[0].what.contains("given away"),
        "Error must mention the binding was given away, got: {:?}",
        out.diagnostics[0].what
    );
}

#[test]
fn const_binding_cannot_be_given() {
    // WHY: `const` blocks the `give` path — ownership cannot be transferred out
    // of a const binding. Const deep-immutability path #3.
    assert_errors(
        r#"
shape Player { name: string }
function consume(give p: Player) -> nothing { }
function entrypoint() -> nothing {
  const p: Player = { name: `Patrick` }
  consume(p)
}
"#,
        1,
    );
}

#[test]
fn const_binding_cannot_be_lent() {
    // WHY: `const` blocks the `lend` path. Const deep-immutability path #2.
    assert_errors(
        r#"
shape Player { health: int }
function damage(lend self: Player, amount: int) -> nothing {
  self.health = self.health - amount
}
function entrypoint() -> nothing {
  const p: Player = { health: 100 }
  damage(p, 10)
}
"#,
        1,
    );
}

#[test]
fn give_twice_is_use_after_give() {
    // WHY: After a value is given, the second give must see it as consumed.
    assert_errors(
        r#"
shape Player { name: string }
function consume(give p: Player) -> nothing { }
function entrypoint() -> nothing {
  let p: Player = { name: `Patrick` }
  consume(p)
  consume(p)
}
"#,
        1,
    );
}

#[test]
fn share_on_const_is_allowed() {
    // WHY: A `share` (read-only) parameter can receive a const binding.
    assert_clean(
        r#"
shape Player { name: string }
function greet(share p: Player) -> string { return p.name }
function entrypoint() -> nothing {
  const p: Player = { name: `Patrick` }
  let g = greet(p)
  print(g)
}
"#,
    );
}

#[test]
fn unspecified_param_accepts_const() {
    // WHY: No ownership modifier defaults to share semantics. Const bindings are fine.
    assert_clean(
        r#"
shape Player { name: string }
function greet(p: Player) -> string { return p.name }
function entrypoint() -> nothing {
  const p: Player = { name: `Patrick` }
  let g = greet(p)
  print(g)
}
"#,
    );
}

// ── M6: union typeck (P3b) ───────────────────────────────────────────────────

#[test]
fn m6_union_type_annotation_works() {
    // WHY: A union type `let s: Circle | Square = c` must parse and typecheck without errors.
    // Assigning a concrete variant (Circle) to a union type must be valid.
    assert_clean(
        r#"
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
"#,
    );
}

#[test]
fn m6_union_multicase_exhaustive_clean() {
    // WHY: A fully-covered union multi-case must typecheck cleanly.
    assert_clean(
        r#"
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
"#,
    );
}

#[test]
fn m6_union_multicase_nonexhaustive_is_error() {
    // WHY: A missing `is Foo` arm in a union multi-case must produce an error.
    assert_errors(
        r#"
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
"#,
        1,
    );
}

#[test]
fn m6_union_multicase_with_else_is_clean() {
    // WHY: `else =>` covers all remaining variants — must typecheck clean.
    assert_clean(
        r#"
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
"#,
    );
}

#[test]
fn m6_is_expr_on_non_union_is_error() {
    // WHY: `is Foo` on a non-union (e.g., string) must produce a teaching error.
    assert_errors(
        r#"
function entrypoint() -> nothing {
  let s: string = `hello`
  if (s is int) {
    print(`wrong`)
  }
}
"#,
        1,
    );
}

// ── M6: options typeck ────────────────────────────────────────────────────────

#[test]
fn m6_options_value_typechecks() {
    // WHY: `Status.active` must resolve to Type::Options { name: "Status" }, not Error.
    // If this fails, every options-typed variable is mistyped and comparisons fail.
    assert_clean(
        r#"
options Status { active, inactive, banned }
function entrypoint() -> nothing {
  let s: Status = Status.active
  print(s.toString())
}
"#,
    );
}

#[test]
fn m6_options_unknown_variant_is_error() {
    // WHY: Accessing a non-existent variant must produce an error, not silently type as Error.
    // A typo like `Status.activ` should be caught at compile time.
    assert_errors(
        r#"
options Status { active, inactive, banned }
function entrypoint() -> nothing {
  let s: Status = Status.activ
}
"#,
        1,
    );
}

#[test]
fn m6_options_empty_body_is_error() {
    // WHY: An options type with no variants can never hold a value — the compiler must reject it.
    assert_errors(
        r#"
options Empty { }
function entrypoint() -> nothing { }
"#,
        1,
    );
}

#[test]
fn m6_options_single_variant_is_error() {
    // WHY: Single-variant options types carry no information — symmetric with single-variant union rejection.
    assert_errors(
        r#"
options Single { only }
function entrypoint() -> nothing { }
"#,
        1,
    );
}

#[test]
fn m6_options_multicase_exhaustive_clean() {
    // WHY: A fully-covered options multi-case must typecheck with zero errors.
    // If this fails, correct options code is incorrectly rejected.
    assert_clean(
        r#"
options Status { active, inactive }
function entrypoint() -> nothing {
  let s: Status = Status.active
  if (s) {
    active => print(`ok`)
    inactive => print(`off`)
  }
}
"#,
    );
}

#[test]
fn m6_options_multicase_nonexhaustive_is_error() {
    // WHY: A multi-case with a missing arm must produce an error naming the missing variant.
    // Missing variants silently fall through without this check — a latent bug class.
    assert_errors(
        r#"
options Status { active, inactive, banned }
function entrypoint() -> nothing {
  let s: Status = Status.active
  if (s) {
    active => print(`ok`)
    inactive => print(`off`)
  }
}
"#,
        1,
    );
}

#[test]
fn m6_options_multicase_with_else_arm_is_clean() {
    // WHY: An `else =>` arm covers all remaining variants — must typecheck clean even if
    // individual variants are not all listed.
    assert_clean(
        r#"
options Status { active, inactive, banned }
function entrypoint() -> nothing {
  let s: Status = Status.active
  if (s) {
    active => print(`ok`)
    else => print(`other`)
  }
}
"#,
    );
}

#[test]
fn m6_same_options_comparison_clean() {
    // WHY: Comparing two values of the same options type with `==` must succeed.
    assert_clean(
        r#"
options Status { active, inactive }
function entrypoint() -> nothing {
  let a: Status = Status.active
  let b: Status = Status.inactive
  let eq = a == b
  print(eq.toString())
}
"#,
    );
}

#[test]
fn m6_cross_options_comparison_is_error() {
    // WHY: Comparing values of different options types is almost always a bug —
    // the tags have no shared meaning between types.
    assert_errors(
        r#"
options Status { active, inactive }
options Visibility { visible, invisible }
function entrypoint() -> nothing {
  let s: Status = Status.active
  let v: Visibility = Visibility.visible
  let eq = s == v
}
"#,
        1,
    );
}

#[test]
fn m6_bool_to_int_is_error() {
    // WHY: `.toInt()` on bool must be rejected — no silent 0/1 coercion.
    assert_errors(
        r#"
function entrypoint() -> nothing {
  let x = true.toInt()
}
"#,
        1,
    );
}

#[test]
fn m6_int_to_int_is_clean() {
    // WHY: `.toInt()` on int must be the identity and return `int` directly (not `maybe<int>`).
    assert_clean(
        r#"
function entrypoint() -> nothing {
  let x: int = 42
  let y: int = x.toInt()
  print(y.toString())
}
"#,
    );
}

#[test]
fn m6_float_to_int_returns_maybe() {
    // WHY: `.toInt()` on float is fallible — must return `maybe<int>`, not bare `int`.
    // If this returns int, NaN and OOR cases silently produce wrong values at runtime.
    assert_clean(
        r#"
function entrypoint() -> nothing {
  let x: float = 3.14
  let y: maybe<int> = x.toInt()
  print(y.or(0).toString())
}
"#,
    );
}

#[test]
fn m6_string_to_int_returns_maybe() {
    // WHY: `"42".toInt()` is fallible — the string might not be a valid integer.
    // Must return `maybe<int>` so the caller handles the failure case.
    assert_clean(
        r#"
function entrypoint() -> nothing {
  let s: string = `42`
  let x: maybe<int> = s.toInt()
  print(x.or(0).toString())
}
"#,
    );
}

#[test]
fn bug3_multi_level_inheritance_flatten_deterministic() {
    // WHY: catches the HashMap-iteration-order bug (Bug #3) in flatten_inherited_fields.
    // Previously, if HashMap visited C before B, C would miss A's fields because B wasn't
    // flattened yet. The fix processes shapes in parent-first (depth-first) order, so
    // A is always flattened before B, and B before C. If c.x produces "field not found",
    // the deterministic ordering fix is incomplete.
    assert_clean(
        r#"
shape A { x: int }
shape B extends A { y: int }
shape C extends B { z: int }

function entrypoint() -> nothing {
  let c: C = { x: 1, y: 2, z: 3 }
  print(`${c.x}`)
  print(`${c.y}`)
  print(`${c.z}`)
}
"#,
    );
}

#[test]
fn m8_background_rejects_share_param_callee() {
    // WHY: enforces the locked M8 decision (spec/concurrency.md:164-177): `background`
    // must reject callees whose parameters use `share` ownership. A shared borrow may
    // outlive the caller's scope when the task runs in the background — a memory-safety
    // hole. This test catches regressions where the check is removed or bypassed.
    let output = assert_errors(
        r#"
function readData(share data: string) -> nothing {
  print(data)
}

function entrypoint() -> nothing {
  let s: string = `hello`
  background readData(s)
}
"#,
        1,
    );
    let has_share_msg = output.diagnostics.iter().any(|d| {
        d.what
            .to_lowercase()
            .contains("cannot use `background` with a function that borrows its arguments")
    });
    assert!(
        has_share_msg,
        "Expected 'cannot use `background` with a function that borrows its arguments' diagnostic, got: {:#?}",
        output.diagnostics
    );
}

#[test]
fn incremental_rebuild_invalidates_when_imported_signature_changes() {
    // WHY: Bug #2 — coarse PartialEq on Salsa-tracked outputs caused incremental
    //      builds to skip re-typechecking when an imported file's function signature
    //      changed but the function NAME stayed the same. This test catches a
    //      regression by editing fileB's source after fileA was checked once,
    //      then asserting fileA re-checks against the new signature.
    //
    //      Uses real temp files on disk because resolve_module_path canonicalizes
    //      paths via std::fs — the files must actually exist for imports to resolve.
    use salsa::Setter as _;

    let dir = std::env::temp_dir().join(format!(
        "ynz_incremental_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");

    // yinz.toml required for cross-directory import resolution.
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").expect("write yinz.toml");

    let b_path = dir.join("b.ynz");
    let a_path = dir.join("a.ynz");

    // v1: fileB exports `foo(x: int) -> int`. fileA imports and calls `foo(42)`.
    let file_b_v1 = "export function foo(x: int) -> int { return x }";
    let file_a_src =
        "import { foo } from `b`\nfunction entrypoint() -> nothing { let r = foo(42) }";

    std::fs::write(&b_path, file_b_v1).expect("write b.ynz v1");
    std::fs::write(&a_path, file_a_src).expect("write a.ynz");

    let b_path_str = b_path.display().to_string();
    let a_path_str = a_path.display().to_string();

    let mut db = CompilerDb::default();
    let sf_b = SourceFile::new(&db, b_path_str.clone(), file_b_v1.to_string());
    let sf_a = SourceFile::new(&db, a_path_str.clone(), file_a_src.to_string());
    db.register_source(sf_b);
    db.register_source(sf_a);

    let output1 = check_query(&db, sf_a);
    let v1_errors: Vec<_> = output1
        .diagnostics
        .iter()
        .filter(|d| d.severity == ynz_diagnostics::Severity::Error)
        .collect();
    // test-ratchet: restoring is_empty() — round-2 swallow filter removed; reverted gate
    // makes pure cross-file compile clean again (Phase-6 round-3). `entrypoint` here does
    // NOT independently suspend (no sleep), so no can't-infer error fires under the
    // design-correct `current_fn_suspends` gate.
    assert_eq!(
        v1_errors.len(),
        0,
        "v1: foo(int)->int + call foo(42) should compile clean (no errors); got: {:#?}",
        v1_errors
    );

    // Change fileB's foo to take a string instead of int — same name, different signature.
    let file_b_v2 = "export function foo(x: string) -> int { return 42 }";
    std::fs::write(&b_path, file_b_v2).expect("write b.ynz v2");
    sf_b.set_text(&mut db).to(file_b_v2.to_string());

    let output2 = check_query(&db, sf_a);
    let v2_errors: Vec<_> = output2
        .diagnostics
        .iter()
        .filter(|d| d.severity == ynz_diagnostics::Severity::Error)
        .collect();
    // After fix: foo(42) passes int to a function expecting string — must produce >= 1 error.
    // Before fix: Salsa skipped re-checking because coarse PartialEq said "no change"
    //             (same function name, same length — values were ignored).
    assert!(
        !v2_errors.is_empty(),
        "v2: foo now expects string but call passes int — must produce a type error; got 0 errors. \
         Diagnostics: {:#?}",
        output2.diagnostics
    );

    // Cleanup.
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hidden_field_without_default_value_is_compile_error() {
    // WHY: hidden fields can't be set by external construction, so the spec requires
    //      a default value at declaration. Without this check, codegen would emit
    //      either a silent zero or undefined behavior for the hidden field's slot.
    let output = assert_errors(
        r#"
shape Foo {
    name: string
    hidden bar: int
}
function entrypoint() -> nothing {}
"#,
        1,
    );
    let has_msg = output
        .diagnostics
        .iter()
        .any(|d| d.what.contains("`bar`") && d.what.contains("no default value"));
    assert!(
        has_msg,
        "Expected hidden-no-default diagnostic, got: {:#?}",
        output.diagnostics
    );
}

#[test]
fn hidden_maybe_field_without_default_suggests_none() {
    // WHY: when the field type is `maybe T`, the natural default is `none`, and
    //      the diagnostic should suggest exactly that rather than the generic case-A1 wording.
    let output = assert_errors(
        r#"
shape Foo {
    name: string
    hidden cache: maybe<int>
}
function entrypoint() -> nothing {}
"#,
        1,
    );
    let suggests_none = output
        .diagnostics
        .iter()
        .any(|d| d.what_instead.contains("none"));
    assert!(
        suggests_none,
        "Expected `none` suggestion for maybe-typed field, got: {:#?}",
        output.diagnostics
    );
}

#[test]
fn same_file_construction_can_omit_hidden_field() {
    // WHY: hidden fields can be omitted at construction (defaults fill in), and
    //      same-file code CAN set them explicitly too — the file boundary is what
    //      gates the construction-time set, not the hidden flag alone.
    assert_clean(
        r#"
shape Foo {
    name: string
    hidden bar: int = 0
}

function entrypoint() -> nothing {
    const x: Foo = { name: `hello` }
    print(x.name)
}
"#,
    );
}

#[test]
fn external_file_construction_cannot_set_hidden_field() {
    // WHY: Fix B — external code (in a different file from the shape declaration)
    //      must not be able to set hidden fields at construction. This catches a
    //      regression where the field-existence check at check_struct_lit accepted
    //      hidden fields without distinguishing the file boundary.

    let dir = std::env::temp_dir().join(format!(
        "ynz_hidden_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").expect("write yinz.toml");

    let player_path = dir.join("player.ynz");
    let main_path = dir.join("entrypoint.ynz");

    std::fs::write(
        &player_path,
        "export shape Player { name: string hidden secret: int = 0 }",
    )
    .expect("write player.ynz");
    std::fs::write(
        &main_path,
        "import { Player } from `player`\n\
         function entrypoint() -> nothing {\n\
             const p: Player = { name: `Alice`, secret: 42 }\n\
         }",
    )
    .expect("write entrypoint.ynz");

    let mut db = CompilerDb::default();
    let sf_player = SourceFile::new(
        &db,
        player_path.display().to_string(),
        std::fs::read_to_string(&player_path).unwrap(),
    );
    let sf_main = SourceFile::new(
        &db,
        main_path.display().to_string(),
        std::fs::read_to_string(&main_path).unwrap(),
    );
    db.register_source(sf_player);
    db.register_source(sf_main);

    let output = check_query(&db, sf_main);
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| d.severity == ynz_diagnostics::Severity::Error)
        .collect();
    assert!(
        !errors.is_empty(),
        "Expected hidden-set-from-external diagnostic; got 0 errors. Diagnostics: {:#?}",
        output.diagnostics
    );
    let has_hidden_msg = errors
        .iter()
        .any(|d| d.what.contains("`secret`") && d.what.contains("hidden"));
    assert!(
        has_hidden_msg,
        "Expected the hidden-set diagnostic; got: {:#?}",
        errors
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn m8_background_let_binding_rejected() {
    // WHY: M8 locked: `let h = background fn()` must error — background-handle form
    //      (.send/.receive) ships in v0.3. Without this check, the let-binding silently
    //      gives `h` type `nothing` and the user has no signal anything is wrong.
    let output = assert_errors(
        r#"
function readData() -> nothing { print(`hello`) }
function entrypoint() -> nothing {
  let h = background readData()
}
"#,
        1,
    );
    // test-ratchet: diagnostic wording updated to avoid "result" (banned jargon).
    let has_msg = output
        .diagnostics
        .iter()
        .any(|d| d.what.contains("Capturing the output of `background`"));
    assert!(
        has_msg,
        "Expected handle-form rejection diagnostic, got: {:#?}",
        output.diagnostics
    );
}

// ── Inline / anonymous shape types (v0.1-polish) ──────────────────────────────

#[test]
fn anon_shape_basic_annotation_works() {
    // WHY: minimal inline shape — must compile clean and bind fields correctly.
    assert_clean(
        r#"
function entrypoint() -> nothing {
  let p: { a: int, b: string } = { a: 1, b: `hi` }
  print(p.b)
}
"#,
    );
}

#[test]
fn anon_shape_field_order_irrelevant_for_equivalence() {
    // WHY: structural typing — {a:int, b:int} and {b:int, a:int} must be the same type
    //      so that passing one where the other is expected compiles clean.
    assert_clean(
        r#"
function takesAB(p: { a: int, b: int }) -> nothing { print(`${p.a}`) }
function entrypoint() -> nothing {
  let x: { b: int, a: int } = { a: 1, b: 2 }
  takesAB(x)
}
"#,
    );
}

#[test]
fn anon_shape_unknown_field_rejected() {
    // WHY: setting a field not declared in the inline shape must be caught — the type
    //      contract says exactly which fields exist.
    let output = assert_errors(
        r#"
function entrypoint() -> nothing {
  let x: { a: int } = { a: 1, b: 2 }
}
"#,
        1,
    );
    let has_msg = output.diagnostics.iter().any(|d| d.what.contains("`b`"));
    assert!(
        has_msg,
        "Expected unknown-field diagnostic mentioning `b`; got: {:#?}",
        output.diagnostics
    );
}

#[test]
fn anon_shape_missing_field_rejected() {
    // WHY: declared field not provided at construction must error — forces complete
    //      initialization just like named shapes.
    let output = assert_errors(
        r#"
function entrypoint() -> nothing {
  let x: { a: int, b: int } = { a: 1 }
}
"#,
        1,
    );
    let has_msg = output.diagnostics.iter().any(|d| d.what.contains("`b`"));
    assert!(
        has_msg,
        "Expected missing-field diagnostic; got: {:#?}",
        output.diagnostics
    );
}

#[test]
fn anon_shape_named_shape_are_different_types() {
    // WHY: nominal typing for named shapes — `shape Foo { a: int }` and `{ a: int }`
    //      do NOT interconvert; the user must pick one or the other.
    let output = assert_errors(
        r#"
shape Foo { a: int }
function entrypoint() -> nothing {
  let x: Foo = { a: 1 }
  let y: { a: int } = x
}
"#,
        1,
    );
    let has_mismatch = output.diagnostics.iter().any(|d| {
        d.what.contains("cannot produce")
            || d.what.contains("mismatch")
            || d.what.contains("expected")
            || d.what.contains("is declared as")
            || d.what.contains("This value is")
    });
    assert!(
        has_mismatch,
        "Expected type-mismatch diagnostic; got: {:#?}",
        output.diagnostics
    );
}

#[test]
fn anon_shape_nested_works() {
    // WHY: inline shapes can be nested in their own type annotations — the inner
    //      anon shape is hoisted to its own canonical synthetic name.
    assert_clean(
        r#"
function entrypoint() -> nothing {
  let p: { outer: { inner: int } } = { outer: { inner: 42 } }
  print(`${p.outer.inner}`)
}
"#,
    );
}

#[test]
fn anon_shape_in_fixed_works_with_for_destructure() {
    // WHY: Patrick's motivating example — inline shape in fixed<T>, iterated with
    //      destructuring. End-to-end integration with destructuring from commit 19d9d4c.
    assert_clean(
        r#"
function entrypoint() -> nothing {
  const intervals: fixed<{ minutes: int }> = [
    { minutes: 5 },
    { minutes: 15 },
    { minutes: 60 },
  ]
  for ({ minutes } in intervals) {
    print(`${minutes}`)
  }
}
"#,
    );
}

#[test]
fn anon_shape_field_access_works() {
    // WHY: field access on an anon-shape-typed binding must resolve field types.
    assert_clean(
        r#"
function entrypoint() -> nothing {
  let p: { x: int, y: int } = { x: 3, y: 4 }
  let sum: int = p.x + p.y
  print(`${sum}`)
}
"#,
    );
}

#[test]
fn anon_shape_as_function_param_works() {
    // WHY: inline shapes in function parameter type position must compile clean —
    //      the function signature pre-pass must hoist the anon shape.
    assert_clean(
        r#"
function area(rect: { w: int, h: int }) -> int {
  return rect.w * rect.h
}
function entrypoint() -> nothing {
  let r: { w: int, h: int } = { w: 5, h: 3 }
  print(`${area(r)}`)
}
"#,
    );
}

#[test]
fn anon_shape_rejects_hidden_field() {
    // WHY: `hidden` inside an inline shape type is incoherent — hidden fields are
    //      file-private and require a named shape. The parser must emit the specific
    //      diagnostic pointing at the `hidden` keyword.
    // The `hidden` keyword is consumed and reported; the remaining `b: int` is parsed as
    // a regular field, so the struct literal `{ a: 1 }` gets a secondary "missing b" error.
    let output = run(r#"
function entrypoint() -> nothing {
  let p: { a: int, hidden b: int } = { a: 1 }
}
"#);
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(!errors.is_empty(), "Expected at least 1 error; got none");
    let has_hidden_msg = errors
        .iter()
        .any(|d| d.what.contains("Inline shape types cannot have `hidden`"));
    assert!(
        has_hidden_msg,
        "Expected hidden-in-inline-shape diagnostic; got: {:#?}",
        errors
    );
}

// ── Auto-promotion lint: repeated inline shapes ───────────────────────────────

#[test]
fn lint_warns_when_same_inline_shape_used_twice() {
    // WHY: When the same inline shape `{ a: int, b: string }` appears in two places,
    // the Tier 3 lint must emit a Warning at each use site suggesting extraction to
    // a named shape. Without this, users silently accumulate duplicate type definitions
    // that must be updated separately when the shape changes.
    let output = run(r#"
function f1(p: { a: int, b: string }) -> nothing { print(p.b) }
function f2(q: { a: int, b: string }) -> nothing { print(q.b) }
function entrypoint() -> nothing { f1({ a: 1, b: `hi` })  f2({ a: 2, b: `yo` }) }
"#);
    let warnings: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Warning)
                && d.what.contains("used in 2 places")
        })
        .collect();
    assert_eq!(
        warnings.len(),
        2,
        "Expected 2 warnings (one per use site) for repeated inline shape; got {}:\n{:#?}",
        warnings.len(),
        output.diagnostics
    );
    assert!(
        warnings[0].what_instead.contains("SuggestedName"),
        "Warning must suggest a named shape, got: {:?}",
        warnings[0].what_instead
    );
}

#[test]
fn lint_no_warning_for_unique_inline_shapes() {
    // WHY: When each inline shape is used exactly once, no lint should fire.
    // Guards that the lint doesn't cry wolf on legitimate one-off shapes.
    let output = run(r#"
function f1(p: { a: int }) -> nothing { print(p.a) }
function f2(q: { b: string }) -> nothing { print(q.b) }
function entrypoint() -> nothing { f1({ a: 1 })  f2({ b: `hi` }) }
"#);
    let repeated_warnings: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Warning) && d.what.contains("used in")
        })
        .collect();
    assert!(
        repeated_warnings.is_empty(),
        "Unique inline shapes must not trigger the lint; got warnings:\n{:#?}",
        repeated_warnings
    );
}

#[test]
fn lint_fires_for_inline_shape_in_param_and_return_type() {
    // WHY: A shape that appears at both a parameter position AND a return type
    // counts as 2 uses. Guards that the lint counts both annotation positions,
    // not just the first one it encounters.
    let output = run(r#"
function f1(p: { x: int, y: int }) -> { x: int, y: int } { return p }
function entrypoint() -> nothing {}
"#);
    let warnings: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Warning)
                && d.what.contains("used in 2 places")
        })
        .collect();
    assert_eq!(
        warnings.len(),
        2,
        "param + return-type both count as use sites; got {}:\n{:#?}",
        warnings.len(),
        output.diagnostics
    );
}

// ── Cross-file inline-shape structural equivalence ────────────────────────────

#[test]
fn cross_file_inline_shape_structural_equivalence_positive() {
    // WHY: Inline shapes rely on content-based canonical naming. Two files that
    // declare the same `{ a: int, b: string }` must resolve to the same canonical
    // `__anon__*` shape so cross-file calls type-check correctly. If canonical
    // naming is file-local instead of content-global, every cross-file inline-shape
    // call produces a false type-mismatch error. This test is the regression guard.

    let dir = std::env::temp_dir().join(format!(
        "ynz_inline_cross_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").expect("write yinz.toml");

    let a_src = "export function takesAB(p: { a: int, b: string }) -> nothing { print(p.b) }";
    let b_src = "import { takesAB } from `a`\n\
                 function entrypoint() -> nothing { takesAB({ a: 1, b: `hi` }) }";

    let a_path = dir.join("a.ynz");
    let b_path = dir.join("b.ynz");
    std::fs::write(&a_path, a_src).expect("write a.ynz");
    std::fs::write(&b_path, b_src).expect("write b.ynz");

    let mut db = ynz_parser::CompilerDb::default();
    let sf_a = ynz_parser::SourceFile::new(&db, a_path.display().to_string(), a_src.to_string());
    let sf_b = ynz_parser::SourceFile::new(&db, b_path.display().to_string(), b_src.to_string());
    db.register_source(sf_a);
    db.register_source(sf_b);

    let output = ynz_typeck::check_query(&db, sf_b);
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    // test-ratchet: restoring is_empty() — round-2 swallow filter removed; reverted gate
    // makes pure cross-file compile clean again (Phase-6 round-3). `entrypoint` here does
    // NOT independently suspend (no sleep), so no can't-infer error fires under the
    // design-correct `current_fn_suspends` gate. This is the inline-shape structural
    // equivalence guard — the only valid errors would be type-mismatch; none should fire.
    assert!(
        errors.is_empty(),
        "cross-file structural equivalence: pure (non-suspending) cross-file call must compile clean; got: {:#?}",
        errors
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cross_file_inline_shape_field_mismatch_documents_known_gap() {
    // WHY: A call passing `{ a: 1, c: "hi" }` to a function expecting
    // `{ a: int, b: string }` should produce a type error — field `c` is not
    // part of the canonical shape `{ a: int, b: string }`.
    //
    // KNOWN LIMITATION (v0.1): struct literals at untyped call-site arguments are
    // not checked against the expected param type. An untyped struct-literal
    // argument synthesizes a new anon shape `{ a: int, c: string }` and the call
    // checker compares `__anon__a_int__b_string` (param) vs `__anon__a_int__c_string`
    // (arg) — which should be caught, but the expected-type is not threaded into
    // `check_call` argument checking in v0.1. See design/inline-shape-types.md
    // open question #2.
    //
    // This test documents the gap without asserting incorrect behavior. When the
    // limitation is fixed, change `let _ = errors` back to `assert!(!errors.is_empty())`.
    //
    // test-ratchet: documents known limitation rather than asserting unfixed behavior.

    let dir = std::env::temp_dir().join(format!(
        "ynz_inline_neg_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").expect("write yinz.toml");

    let a_src = "export function takesAB(p: { a: int, b: string }) -> nothing { print(p.b) }";
    let b_src = "import { takesAB } from `a`\n\
                 function entrypoint() -> nothing { takesAB({ a: 1, c: `hi` }) }";

    let a_path = dir.join("a.ynz");
    let b_path = dir.join("b.ynz");
    std::fs::write(&a_path, a_src).expect("write a.ynz");
    std::fs::write(&b_path, b_src).expect("write b.ynz");

    let mut db = ynz_parser::CompilerDb::default();
    let sf_a = ynz_parser::SourceFile::new(&db, a_path.display().to_string(), a_src.to_string());
    let sf_b = ynz_parser::SourceFile::new(&db, b_path.display().to_string(), b_src.to_string());
    db.register_source(sf_a);
    db.register_source(sf_b);

    let output = ynz_typeck::check_query(&db, sf_b);
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    // Known gap: currently 0 errors. When the expected-type gap is fixed, flip this
    // to: assert!(!errors.is_empty(), "field mismatch must produce an error");
    let _ = errors;

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
// WHY: `{ field: UnknownType }` in an inline shape must produce a compile error just
// like `shape Foo { field: UnknownType }` in a named shape. Before this fix the
// validation was silently skipped for synthetic/anon shapes, letting `Timeframe` (or
// any other unimported options/shape type) pass through as Type::Error with no
// diagnostic — user had no idea the import was missing.
fn anon_shape_unknown_field_type_errors() {
    let output = assert_errors(
        r#"
function entrypoint() -> nothing {
  const intervals: fixed<{ minutes: int, timeframe: Timeframe }> = []
}
"#,
        1,
    );
    let has_msg = output
        .diagnostics
        .iter()
        .any(|d| d.what.contains("Timeframe"));
    assert!(
        has_msg,
        "Expected diagnostic mentioning `Timeframe` for unknown type in inline shape; got: {:#?}",
        output.diagnostics
    );
}

#[test]
// WHY: unknown types nested inside `maybe<T>` or `array<T>` inside an inline shape
// must also error — the validation must recurse through container types, not just
// check the top-level field type.
fn anon_shape_unknown_type_in_maybe_field_errors() {
    let output = assert_errors(
        r#"
function entrypoint() -> nothing {
  let x: { val: maybe<GhostType> } = { val: none }
}
"#,
        1,
    );
    let has_msg = output
        .diagnostics
        .iter()
        .any(|d| d.what.contains("GhostType"));
    assert!(
        has_msg,
        "Expected diagnostic mentioning `GhostType` in maybe field; got: {:#?}",
        output.diagnostics
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// v0.3-M1: sleepBlocking intrinsic typeck
// ─────────────────────────────────────────────────────────────────────────────

// WHY: sleepBlocking(int) -> nothing must type-check cleanly with an int arg and
// return nothing. If the typeck dispatch arm is missing, users get a confusing
// "not defined" error instead of a successful type-check.
#[test]
fn sleep_ms_with_int_arg_is_clean() {
    assert_clean("function entrypoint() -> nothing {\n  sleepBlocking(50)\n}");
}

// WHY: sleepBlocking with a non-int arg must produce a clear teaching error.
// Guards against the typeck arm silently accepting wrong-typed arguments.
#[test]
fn sleep_ms_with_wrong_type_produces_error() {
    let out = run("function entrypoint() -> nothing {\n  sleepBlocking(`not an int`)\n}");
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        !errors.is_empty(),
        "sleepBlocking with string arg must produce an error"
    );
    assert!(
        errors[0].what.contains("int") || errors[0].what.contains("sleepBlocking"),
        "error must mention int or sleepBlocking; got: {:?}",
        errors[0].what
    );
}

// WHY: sleepBlocking with 0 args must produce an arity error.
#[test]
fn sleep_ms_with_no_args_produces_error() {
    let out = run("function entrypoint() -> nothing {\n  sleepBlocking()\n}");
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        !errors.is_empty(),
        "sleepBlocking with no args must produce an error"
    );
}

// WHY: sleepBlocking with 2 args must produce an arity error.
#[test]
fn sleep_ms_with_two_args_produces_error() {
    let out = run("function entrypoint() -> nothing {\n  sleepBlocking(50, 100)\n}");
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        !errors.is_empty(),
        "sleepBlocking with 2 args must produce an error"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// v0.3-M1: P4 — lend-cross-thread, large-copy warning, kernel-mode rejection
// ─────────────────────────────────────────────────────────────────────────────

use ynz_typeck::check_with_kernel_mode;

fn run_kernel(source: &str) -> CheckOutput {
    // Drive typeck directly with kernel_mode=true, bypassing the salsa check_query.
    let db = ynz_parser::CompilerDb::default();
    let sf = ynz_parser::SourceFile::new(&db, FILE.to_string(), source.to_string());
    let parse = ynz_parser::parse_query(&db, sf);
    let sig_output = ynz_typeck::queries::module_signatures_query(&db, sf);

    let mut merged_sig_table = sig_output.sig_table.clone();
    for (name, sig) in &sig_output.imported_fns {
        merged_sig_table
            .fns
            .entry(name.clone())
            .or_insert_with(|| sig.clone());
    }

    let (typed, mono_table, check_diags) = check_with_kernel_mode(
        &parse.module,
        &merged_sig_table,
        &sig_output.shape_table,
        &sig_output.generic_fn_table,
        &sig_output.generic_shape_table,
        &ynz_typeck::intrinsics::PrimitiveIntrinsicTable::m6(),
        &sig_output.imported_options,
    );

    let mut all_diags = parse.diagnostics.clone();
    for d in sig_output.diagnostics.iter() {
        all_diags.push(d.clone());
    }
    for d in check_diags.into_iter() {
        all_diags.push(d);
    }

    CheckOutput {
        typed_module: typed,
        mono_table,
        diagnostics: all_diags,
        suspends_set: std::collections::HashSet::new(),
    }
}

// WHY: lend param across thread boundary is a memory-safety error.
// If a function mutates via `lend` and runs on a background thread, the
// original value might be dropped while the mutation is in progress.
#[test]
fn background_with_lend_param_rejected() {
    let src = "function mutate(lend x: int) -> nothing { }\n\
               function entrypoint() -> nothing { let x: int = 5\n background mutate(x) }";
    let out = run(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        !errors.is_empty(),
        "lend param to background must produce an error"
    );
    assert!(
        errors
            .iter()
            .any(|d| d.what.contains("lend") || d.what.contains("mutate")),
        "error must mention lend or background; got: {:?}",
        errors
    );
}

// WHY: large-copy warning fires when a .copy arg is a shape with estimated size > 64B.
// Guards that the warning threshold and message are correct.
#[test]
fn background_with_large_copy_warns() {
    // Shape with 9 fields = 9 * 8 = 72 bytes > 64 byte threshold.
    // NOTE: .copy() with parens (it's an action per dot-postfix rule).
    let src = "shape BigData { a: int, b: int, c: int, d: int, e: int, f: int, g: int, h: int, i: int }\n\
               function process(d: BigData) -> nothing { }\n\
               function entrypoint() -> nothing {\n  let d: BigData = { a: 1, b: 2, c: 3, d: 4, e: 5, f: 6, g: 7, h: 8, i: 9 }\n  background process(d.copy())\n}";
    let out = run(src);
    let warnings: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Warning))
        .collect();
    assert!(
        !warnings.is_empty(),
        "large .copy arg must produce a warning; diags: {:#?}",
        out.diagnostics
    );
    assert!(
        warnings.iter().any(|d| d.what.contains("bytes")),
        "warning must mention bytes; got: {:?}",
        warnings
    );
}

// WHY: small-copy does not warn — guard that the threshold is correct.
#[test]
fn background_with_small_copy_no_warn() {
    // Shape with 4 fields = 32 bytes < 64 byte threshold
    let src = "shape Small { a: int, b: int, c: int, d: int }\n\
               function process(s: Small) -> nothing { }\n\
               function entrypoint() -> nothing {\n  let s: Small = { a: 1, b: 2, c: 3, d: 4 }\n  background process(s.copy())\n}";
    let out = run(src);
    let warnings: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Warning) && d.what.contains("bytes")
        })
        .collect();
    assert!(
        warnings.is_empty(),
        "small .copy arg must NOT produce a bytes warning; got: {:#?}",
        warnings
    );
}

// WHY: kernel mode must reject `wait` with a teaching error. The `Expr::Wait` arm
// emits the kernel diagnostic and returns Type::Error without recursing into the inner
// expression. The `slow()` here is non-suspending (no sleep inside) so the only error
// is from the `wait` keyword itself — one diagnostic, from one arm.
#[test]
fn wait_in_kernel_mode_rejected() {
    let src = "function slow() -> int { return 1 }\n\
               function entrypoint() -> nothing { let x = wait slow() }";
    let out = run_kernel(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        !errors.is_empty(),
        "wait in kernel mode must produce an error"
    );
    assert!(
        errors
            .iter()
            .any(|d| d.what.contains("wait") || d.what.contains("kernel")),
        "error must mention wait or kernel; got: {:?}",
        errors
    );
}

// WHY: `wait suspendingFn()` in kernel mode must produce exactly ONE diagnostic, not two.
// Before the fix, `Expr::Wait` emitted the kernel reject and then recursed into the inner
// `Expr::Call` via `self.infer_expr(inner, hint)`. That recursion hit the call-dispatch
// kernel guard (check.rs:2435) which fired a SECOND diagnostic for the same source site.
// The fix: `Expr::Wait` returns `Type::Error` immediately after emitting its error, so the
// inner call is never visited in isolation. This test asserts count == 1 to lock that
// contract: a regression that re-introduces the recursive call would bump count to 2.
//
// This uses an in-module suspending function (calls sleep) so the sig_table marks it
// suspends=true BEFORE the may-block fixpoint via module_signatures_query. The call-dispatch
// arm checks `callee_suspends` from the merged sig_table — if the flag is set and the arm
// is reached, it fires. The Wait arm returning Type::Error prevents reaching the arm at all.
#[test]
fn wait_suspending_in_kernel_mode_produces_exactly_one_diagnostic() {
    // `slow` is defined with sleep inside — after may-block analysis its sig.suspends=true.
    // Without the fix: wait fires Expr::Wait error, then infers inner Expr::Call(slow),
    // which hits the call-dispatch kernel guard → 2nd error. With the fix: 1 error total.
    // NOTE: `sleep` itself fires a kernel error inside `slow`'s body, so we count errors
    // ONLY at the entrypoint's `wait slow()` call span — filter by containing "wait".
    let src = "function slow() -> nothing { sleep(50) }\n\
               function entrypoint() -> nothing { wait slow() }";
    let out = run_kernel(src);
    // Count errors whose `what` mentions "wait" — these are `wait`-keyword rejections.
    // The `sleep` inside `slow` fires a separate kernel error which is correct and not
    // the double-diagnostic we're guarding against.
    let wait_errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Error)
                && (d.what.contains("wait") || d.what.contains("`wait`"))
        })
        .collect();
    assert!(
        wait_errors.len() == 1,
        "`wait slow()` in kernel mode must produce exactly 1 `wait`-related error, not {}. \
         A count > 1 indicates the Expr::Wait arm recursed into the inner Expr::Call and \
         the call-dispatch kernel guard fired a second time. Got: {:#?}",
        wait_errors.len(),
        wait_errors
    );
}

// WHY: kernel mode must reject `background` with a teaching error.
#[test]
fn background_in_kernel_mode_rejected() {
    let src = "function process() -> nothing { }\n\
               function entrypoint() -> nothing { background process() }";
    let out = run_kernel(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        !errors.is_empty(),
        "background in kernel mode must produce an error"
    );
    assert!(
        errors
            .iter()
            .any(|d| d.what.contains("background") || d.what.contains("kernel")),
        "error must mention background or kernel; got: {:?}",
        errors
    );
}

// WHY: background inside a for loop must compile without LLVM symbol collisions.
// Guards that the per-Cg bg_uid counter prevents duplicate closure names.
#[test]
fn background_inside_for_loop_compiles() {
    let src = "function process(i: int) -> nothing { }\n\
               function entrypoint() -> nothing {\n  for (i in range(0, 3)) {\n    background process(i)\n  }\n}";
    assert_clean(src);
}

// WHY: background with a UFCS method call that has lend self must produce the
// lend-cross-thread error — verifies that desugaring path is covered.
#[test]
fn background_method_call_with_lend_self_rejected() {
    let src = "shape Counter { n: int }\n\
               function increment(lend self: Counter) -> nothing { self.n = self.n + 1 }\n\
               function entrypoint() -> nothing {\n  let c: Counter = { n: 0 }\n  background c.increment()\n}";
    let out = run(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        !errors.is_empty(),
        "background with lend-self UFCS must produce an error"
    );
}

// WHY: background fn(x) where fn takes `give`; use of x after must produce
// use-after-give error. Guards that is_consumed propagation works at background-call sites.
// Note: `.give` has no body-level syntax; the compiler infers give-ownership when the
// function signature declares `give x`. The test passes `x` directly (not `x.give`).
#[test]
fn background_give_then_use_after_rejected() {
    let src = "function process(give x: int) -> nothing { }\n\
               function entrypoint() -> nothing {\n  let x: int = 5\n  background process(x)\n  print(x)\n}";
    let out = run(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        !errors.is_empty(),
        "use-after-give in background must produce an error"
    );
}

// WHY: zero-byte struct (no fields) must NOT trigger the large-copy warning.
// Guards that the size > 64 condition is strictly-greater (not >=) and doesn't
// misfire on zero-sized types or Type::Nothing.
#[test]
fn background_with_zero_byte_struct_no_warn() {
    let src = "shape Empty {}\n\
               function process(e: Empty) -> nothing { }\n\
               function entrypoint() -> nothing {\n  let e: Empty = {}\n  background process(e.copy())\n}";
    let out = run(src);
    let warnings: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Warning) && d.what.contains("bytes")
        })
        .collect();
    assert!(
        warnings.is_empty(),
        "zero-byte struct copy must NOT produce a bytes warning; got: {:#?}",
        warnings
    );
}

// ── v0.3-M2 Option-B deferral errors ────────────────────────────────────────

// WHY: `wait` inside a `while` loop is accepted since M3a Phase 2 — the guard was
// narrowed to `for`/`match` only. This test prevents regression to the old state
// where `while`-body suspension was rejected. If it fails, the guard was incorrectly
// widened to cover `while` again, breaking a supported feature.
#[test]
fn wait_in_while_loop_is_accepted() {
    let src = r#"
function entrypoint() -> nothing {
  let i: int = 0
  while (i < 3) {
    wait sleep(100)
    i = i + 1
  }
}
"#;
    let out = run(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "`wait` inside `while` must produce no typeck error since M3a Phase 2; errors: {:#?}",
        errors
    );
}

// WHY: P3 lifts the WaitInsideLoop guard for `for` — `wait` inside a `for` loop is now
// accepted (the codegen handles frame-backed loop-state). This test was previously checking
// that an error fires; it now guards the opposite: NO error fires.
// test-ratchet: P3 lifted the for-loop guard; updated from expect-error to expect-ok.
#[test]
fn wait_in_for_loop_is_accepted() {
    let src = r#"
function entrypoint() -> nothing {
  for (i in range(0, 3)) {
    wait sleep(100)
  }
}
"#;
    let out = run(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "wait inside for loop must now be accepted (P3 lifted the guard); errors: {:#?}",
        errors
    );
}

// WHY: M3a P1 lifts the LocalCrossesWait guard — a local declared before a `wait` and
// read after is now ACCEPTED and frame-backed by codegen. Typeck must produce no error.
// Guards against regressing to the old "local binding crossing wait is an error" check,
// which would break any program that passes a computed value across a suspension.
#[test]
fn local_binding_crossing_wait_is_accepted() {
    let src = r#"
function entrypoint() -> nothing {
  let x: int = 5
  wait sleep(30)
  print(x.toString())
}
"#;
    assert_clean(src);
}

// WHY: function PARAMETERS read after a `wait` must NOT produce an error — they are
// frame-backed at every resume point and the concurrent-waits demo depends on this.
// If parameters are wrongly flagged as "crossing" a wait, the main demo fixture breaks.
#[test]
fn param_read_after_wait_is_accepted() {
    let src = r#"
function pause(n: int) -> nothing {
  print(n.toString())
  wait sleep(100)
  print(n.toString())
}
function entrypoint() -> nothing {
  background pause(1)
}
"#;
    assert_clean(src);
}

// WHY: `wait` at the top level and inside `if` must be accepted — these are the
// M2-supported cases. If this test fails, the Option-B check is overly broad and
// rejects valid programs.
#[test]
fn wait_at_top_level_and_in_if_is_accepted() {
    let src = r#"
function maybeWait(b: boolean) -> nothing {
  if (b) {
    wait sleep(50)
  }
  wait sleep(10)
}
function entrypoint() -> nothing {
  background maybeWait(true)
  sleepBlocking(200)
}
"#;
    assert_clean(src);
}

// WHY: M3a P1 lifts LocalCrossesWait — a local declared before an if-nested wait and
// read after the if block must now be accepted. The crossing-local analysis identifies
// `x` as frame-backed; codegen preserves it across the conditional suspension. Guards
// against regressing to an over-rejecting check that blocks this common pattern.
#[test]
fn local_before_if_nested_wait_read_after_if_is_accepted() {
    let src = r#"
function entrypoint() -> nothing {
  let x: int = 5
  if (x < 10) {
    wait sleep(30)
  }
  print(x.toString())
}
"#;
    assert_clean(src);
}

// WHY: M3a P1 lifts LocalCrossesWait — a local declared before an if-nested wait and
// read inside the same branch after the wait must now be accepted. The frame-backed
// slot keeps `x` alive across the conditional suspension. Guards against regressing
// to over-rejection that blocks reading locals in the post-wait continuation.
#[test]
fn local_before_if_nested_wait_read_inside_branch_is_accepted() {
    let src = r#"
function entrypoint() -> nothing {
  let x: int = 5
  if (x < 10) {
    wait sleep(30)
    print(x.toString())
  }
}
"#;
    assert_clean(src);
}

// WHY: M3a P1 lifts LocalCrossesWait — a local declared inside an if branch before
// a wait and read after the wait in the same branch must now be ACCEPTED. The crossing
// analysis identifies `y` as frame-backed; the alloca is in sm_entry so it dominates
// all uses. Guards against over-rejection of inline-if-scoped locals in SM functions.
#[test]
fn local_inside_if_branch_before_wait_read_after_wait_is_accepted() {
    let src = r#"
function entrypoint() -> nothing {
  if (true) {
    let y: int = 7
    wait sleep(30)
    print(y.toString())
  }
}
"#;
    assert_clean(src);
}

// WHY: a local declared and read ONLY BEFORE the wait does not cross a suspension
// boundary — no frame slot needed. Must be accepted (not slotted) even after M3a.
#[test]
fn local_declared_and_read_before_wait_is_accepted() {
    let src = r#"
function entrypoint() -> nothing {
  let x: int = 5
  print(x.toString())
  wait sleep(10)
}
"#;
    assert_clean(src);
}

// WHY: a local declared AFTER the last wait must be accepted — it cannot cross any
// suspend point because it doesn't exist before the wait.
#[test]
fn local_declared_after_wait_is_accepted() {
    let src = r#"
function entrypoint() -> nothing {
  wait sleep(10)
  let z: int = 99
  print(z.toString())
}
"#;
    assert_clean(src);
}

// WHY: M3a P1 lifts LocalCrossesWait — `let slot = sleeper(); let other = sleeper();
// return slot + other` must now be ACCEPTED. `slot` is frame-backed (produced by the
// first suspension, survives the second). The old test guarded against silent LLVM
// crashes; that crash path is now closed by the frame-slot machinery. This test guards
// against regressing to over-rejection of the result-binding-crosses-later-suspension
// pattern, which is the primary motivating use case for M3b auto-parallelization.
#[test]
fn result_binding_crosses_later_suspension_is_accepted() {
    let src = r#"
function sleeper() -> int {
  wait sleep(10)
  return 5
}
function compute() -> int {
  let slot = sleeper()
  let other = sleeper()
  return slot + other
}
function entrypoint() -> nothing {
  background compute()
  sleepBlocking(200)
}
"#;
    assert_clean(src);
}

// WHY: no-over-fire guard — `let slot = sleeper(); let other = sleeper(); return other`
// `slot` is never read after the second suspension, so it must NOT be flagged.
// Over-firing here would break any SM that discards an early result before continuing.
#[test]
fn result_binding_not_used_after_later_suspension_is_accepted() {
    let src = r#"
function sleeper() -> int {
  wait sleep(10)
  return 5
}
function compute(n: int) -> int {
  let slot = sleeper()
  let other = sleeper()
  return other
}
function entrypoint() -> nothing {
  background compute(1)
  sleepBlocking(200)
}
"#;
    assert_clean(src);
}

// WHY: single-suspension no-over-fire — `let a = sleeper(); return a` has only ONE
// suspension. The result-binding arm adds `a` to `pending_result_bindings` (not
// `declared`), so `return a` is not scanned against `a` — it is safe. If this test
// regresses (fires an error), the pending-flush logic is broken.
#[test]
fn result_binding_used_immediately_no_second_suspension_is_accepted() {
    let src = r#"
function sleeper() -> int {
  wait sleep(10)
  return 5
}
function compute(n: int) -> int {
  let a = sleeper()
  return a
}
function entrypoint() -> nothing {
  background compute(1)
  sleepBlocking(200)
}
"#;
    assert_clean(src);
}

// ── v0.3-M2 typeck surface: wait diagnostics + sleep/internal intrinsics ──

// WHY: `wait` applied to a non-call expression (a literal, a variable, etc.) must
// produce a hard error. Catches regressions where the check is removed and `wait 42`
// compiles silently with whatever type 42 has, masking the user's mistake.
#[test]
fn wait_on_literal_is_an_error() {
    let src = r#"
function entrypoint() -> nothing {
  wait 42
}
"#;
    let out = run(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(!errors.is_empty(), "wait on literal must produce an error");
    let has_call_msg = errors.iter().any(|d| d.what.contains("function call"));
    assert!(
        has_call_msg,
        "error must mention 'function call'; got: {:#?}",
        errors
    );
}

// WHY: `wait` applied to a wait expression (nested wait) must produce a hard error.
// Guards the `wait_on_non_call_expression` check against the Case 4 adversarial case
// identified during plan review.
#[test]
fn wait_of_wait_rejected() {
    let src = r#"
function entrypoint() -> nothing {
  wait (wait sleep(10))
}
"#;
    let out = run(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(!errors.is_empty(), "wait of wait must produce an error");
}

// WHY: `wait sleep(100)` is the canonical correct usage; must compile clean.
// Guards that the wait_on_non_may_block_warning does NOT fire for may-block callees.
#[test]
fn wait_sleep_async_is_clean() {
    let src = r#"
function entrypoint() -> nothing {
  wait sleep(100)
}
"#;
    assert_clean(src);
}

// WHY: `wait print("hi")` must fire the `wait_on_non_may_block_warning` Tier 3 warning.
// Catches regressions where the may-block predicate is not checked and useless `wait`
// calls compile silently, misleading developers about what `wait` does.
#[test]
fn wait_on_non_may_block_print_warns() {
    let src = r#"
function entrypoint() -> nothing {
  wait print(`hi`)
}
"#;
    let out = run(src);
    let warnings: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Warning))
        .collect();
    assert!(
        !warnings.is_empty(),
        "wait on non-may-block function must produce a warning; got: {:#?}",
        out.diagnostics
    );
    let has_no_effect = warnings.iter().any(|d| d.what.contains("no effect"));
    assert!(
        has_no_effect,
        "warning must say 'no effect'; got: {:#?}",
        warnings
    );
    // Must NOT also produce an error — this is a Tier 3 warning, exit code 0.
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "wait_on_non_may_block must not produce an error (warning only); got: {:#?}",
        errors
    );
}

// WHY: Phase 6 inference model — `sleep(100)` without explicit `wait` is valid.
// The transitive may-block analysis marks the enclosing function as `suspends` and the
// codegen emits the suspension automatically. `unawaited_sleep_async` is retired because
// there is nothing to warn about: the function suspends correctly without writing `wait`.
// Guards regressions where a stale `unawaited_sleep_async` warning is re-introduced.
#[test]
fn unawaited_sleep_async_no_longer_warns() {
    // test-ratchet: unawaited_sleep_async retired by Phase 6 inference model; old warning was bridge-era artifact
    let src = r#"
function entrypoint() -> nothing {
  sleep(100)
}
"#;
    let out = run(src);
    // Under inference, sleep without `wait` is silently handled — no warning.
    let unawaited_warnings: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Warning)
                && d.what.contains("discards it without waiting")
        })
        .collect();
    assert!(
        unawaited_warnings.is_empty(),
        "unawaited_sleep_async warning must NOT fire under Phase 6 inference; got: {:#?}",
        unawaited_warnings
    );
    // No hard errors either.
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "sleep without wait must compile clean; got: {:#?}",
        errors
    );
}

// WHY: Phase 6 inference model — `wait_required_on_state_machine_call` is retired.
// Under the transitive analysis, every suspending caller is itself a state machine and
// every suspending call is an inline-poll-yield. `wait` is never "required" because the
// compiler infers suspension; calling a suspending fn without explicit `wait` is correct
// and emits no warning. Guards regressions where the old bridge-era warning resurfaces.
#[test]
fn state_machine_calling_state_machine_without_wait_clean_under_inference() {
    // test-ratchet: wait_required_on_state_machine_call retired by Phase 6; bridge-era artifact; no-wait SM→SM calls are correct under inference
    let src = r#"
function inner() -> nothing {
  wait sleep(10)
}
function outer() -> nothing {
  wait sleep(5)
  inner()
}
function entrypoint() -> nothing {
  background outer()
}
"#;
    let out = run(src);
    // Under inference, SM→SM call without explicit `wait` is correct — no warning.
    let wait_required_warnings: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Warning)
                && d.what.contains("state-machine")
        })
        .collect();
    assert!(
        wait_required_warnings.is_empty(),
        "wait_required_on_state_machine_call must NOT fire under Phase 6 inference; got: {:#?}",
        wait_required_warnings
    );
    // No hard errors either.
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "SM→SM call without wait must compile clean; got: {:#?}",
        errors
    );
}

// WHY: `wait inner()` where `inner` contains `wait` — the CORRECT pattern — must compile
// clean with no warning. Guards that wait_required_on_state_machine_call does not
// false-positive when `wait` is properly present.
#[test]
fn state_machine_calling_state_machine_with_wait_is_clean() {
    let src = r#"
function inner() -> nothing {
  wait sleep(10)
}
function outer() -> nothing {
  wait sleep(5)
  wait inner()
}
function entrypoint() -> nothing {
  background outer()
}
"#;
    assert_clean(src);
}

// WHY: `background sm_fn()` from inside a state machine must be EXEMPT from the
// wait_required_on_state_machine_call warning. Background scheduling is a legal
// route-to-I/O-pool pattern (Round 2 Required Fix #2).
#[test]
fn state_machine_can_background_state_machine_without_wait() {
    let src = r#"
function inner() -> nothing {
  wait sleep(10)
}
function outer() -> nothing {
  wait sleep(5)
  background inner()
}
function entrypoint() -> nothing {
  background outer()
}
"#;
    assert_clean(src);
}

// WHY: a non-state-machine function calling a state-machine function must NOT fire the
// wait_required warning — the non-coloring promise means non-SM callers get the sync
// bridge automatically. Guards against the warning over-firing and breaking valid programs.
#[test]
fn non_state_machine_calling_state_machine_is_clean() {
    let src = r#"
function inner() -> nothing {
  wait sleep(10)
}
function outer() -> nothing {
  inner()
}
function entrypoint() -> nothing {
  outer()
}
"#;
    assert_clean(src);
}

// WHY: `sleep` must be rejected in --kernel mode because the Tokio runtime does not
// run in kernel mode. Guards the kernel_mode_rejects_sleep acceptance criterion.
#[test]
fn kernel_mode_rejects_sleep_async() {
    let src = r#"
function entrypoint() -> nothing {
  sleep(100)
}
"#;
    let out = run_kernel(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        !errors.is_empty(),
        "sleep in --kernel mode must produce an error"
    );
    let has_kernel = errors.iter().any(|d| d.what.contains("kernel"));
    assert!(
        has_kernel,
        "error must mention 'kernel'; got: {:#?}",
        errors
    );
}

// WHY: M3e Phase 2 lifts the universal cross-module reject so suspending cross-module
// calls compile and run. This guard confirms that --kernel mode still REJECTS bare
// (auto-suspension, no `wait` keyword) cross-module suspending calls. In Yinz the
// bare call form IS the real production form — every suspending call auto-suspends
// without `wait`. A test using only `wait longJob()` would fire the `Expr::Wait` arm,
// not the call-dispatch arm, and would pass even if the call-dispatch arm has no kernel
// guard at all. This test uses the bare form to guard the correct path.
//
// The kernel-mode check fires on the CALLING module (the module that contains a call to
// a suspending imported fn). check_with_kernel_mode is driven directly here because
// the salsa check_query always uses kernel_mode=false.
//
// test-ratchet: strengthened from `wait longJob()` to bare `longJob()` — the `wait`
// form tested the Expr::Wait arm (already guarded), not the call-dispatch arm that
// guards bare auto-suspension. The bare form is the actual production call pattern.
#[test]
fn kernel_mode_rejects_cross_module_suspending_call() {
    use ynz_typeck::check_with_kernel_mode;

    let dir = std::env::temp_dir().join(format!(
        "ynz_kernel_cross_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").expect("write yinz.toml");

    // Leaf module: exports a suspending function (calls sleep).
    let leaf_src = "export function longJob() -> nothing {\n  sleep(100)\n}\n";
    let leaf_path = dir.join("leaf_ops.ynz");
    std::fs::write(&leaf_path, leaf_src).expect("write leaf_ops.ynz");

    // Entrypoint: bare (no `wait`) cross-module suspending call — the auto-suspension form.
    // Every real Yinz suspending call uses this form. `wait` would fire the Expr::Wait arm
    // (already guarded); this tests the call-dispatch arm where the bare suspend is rejected.
    let entry_src =
        "import { longJob } from `leaf_ops`\nfunction entrypoint() -> nothing {\n  longJob()\n}\n";
    let entry_path = dir.join("entrypoint.ynz");
    std::fs::write(&entry_path, entry_src).expect("write entrypoint.ynz");

    let mut db = ynz_parser::CompilerDb::default();
    let sf_leaf =
        ynz_parser::SourceFile::new(&db, leaf_path.display().to_string(), leaf_src.to_string());
    let sf_entry =
        ynz_parser::SourceFile::new(&db, entry_path.display().to_string(), entry_src.to_string());
    db.register_source(sf_leaf);
    db.register_source(sf_entry);

    // Run check_query on leaf so salsa computes its signatures (marks longJob suspends=true).
    let _leaf_out = ynz_typeck::check_query(&db, sf_leaf);

    // Run module_signatures_query on the entrypoint so imported_fns carries longJob.suspends=true.
    let sig_out = ynz_typeck::queries::module_signatures_query(&db, sf_entry);
    let parse = ynz_parser::parse_query(&db, sf_entry);

    // Merge imported_fns into sig_table so check_with_kernel_mode sees longJob as suspending.
    let mut merged_sig = sig_out.sig_table.clone();
    for (name, sig) in &sig_out.imported_fns {
        merged_sig
            .fns
            .entry(name.clone())
            .or_insert_with(|| sig.clone());
    }

    // Call check_with_kernel_mode on the entrypoint module.
    let (_typed, _mono, diags) = check_with_kernel_mode(
        &parse.module,
        &merged_sig,
        &sig_out.shape_table,
        &sig_out.generic_fn_table,
        &sig_out.generic_shape_table,
        &ynz_typeck::intrinsics::PrimitiveIntrinsicTable::m6(),
        &sig_out.imported_options,
    );

    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();

    assert!(
        !errors.is_empty(),
        "cross-module suspending call in --kernel mode must produce an error; got no errors"
    );
    // The error must mention kernel (the existing kernel-mode suspension rejection path).
    assert!(
        errors
            .iter()
            .any(|d| d.what.contains("kernel") || d.why.contains("kernel")),
        "error must mention 'kernel'; got: {:#?}",
        errors
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// WHY: the UFCS dot-call path `player.longJob()` resolves via check_method_call's
// Shape-receiver branch (check.rs:3308). The bare call-dispatch arm (check.rs:2435)
// guards `Expr::Call name =>` but UFCS calls route through check_method_call — a separate
// code path that needs its own kernel guard. Without it, `player.longJob()` in kernel mode
// passes the call-dispatch guard silently (the name-lookup falls through to the UFCS path)
// and the suspending call reaches codegen unchecked.
#[test]
fn kernel_mode_rejects_cross_module_suspending_method_call() {
    use ynz_typeck::check_with_kernel_mode;

    let dir = std::env::temp_dir().join(format!(
        "ynz_kernel_ufcs_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").expect("write yinz.toml");

    // Worker module: a shape + suspending method (standalone function, first param = Worker).
    let worker_src = "export shape Worker { name: string }\nexport function longJob(share self: Worker) -> nothing {\n  sleep(100)\n}\n";
    let worker_path = dir.join("worker_ops.ynz");
    std::fs::write(&worker_path, worker_src).expect("write worker_ops.ynz");

    // Entrypoint: UFCS dot-call form — `w.longJob()` is sugar for `longJob(w)`.
    // In kernel mode this must be rejected: longJob suspends.
    let entry_src = "import { Worker, longJob } from `worker_ops`\nfunction entrypoint() -> nothing {\n  const w: Worker = { name: `bob` }\n  w.longJob()\n}\n";
    let entry_path = dir.join("entrypoint.ynz");
    std::fs::write(&entry_path, entry_src).expect("write entrypoint.ynz");

    let mut db = ynz_parser::CompilerDb::default();
    let sf_worker = ynz_parser::SourceFile::new(
        &db,
        worker_path.display().to_string(),
        worker_src.to_string(),
    );
    let sf_entry =
        ynz_parser::SourceFile::new(&db, entry_path.display().to_string(), entry_src.to_string());
    db.register_source(sf_worker);
    db.register_source(sf_entry);

    let _worker_out = ynz_typeck::check_query(&db, sf_worker);

    let sig_out = ynz_typeck::queries::module_signatures_query(&db, sf_entry);
    let parse = ynz_parser::parse_query(&db, sf_entry);

    // Merge imported_fns into sig_table so check_with_kernel_mode sees longJob as suspending.
    let mut merged_sig = sig_out.sig_table.clone();
    for (name, sig) in &sig_out.imported_fns {
        merged_sig
            .fns
            .entry(name.clone())
            .or_insert_with(|| sig.clone());
    }

    let (_typed, _mono, diags) = check_with_kernel_mode(
        &parse.module,
        &merged_sig,
        &sig_out.shape_table,
        &sig_out.generic_fn_table,
        &sig_out.generic_shape_table,
        &ynz_typeck::intrinsics::PrimitiveIntrinsicTable::m6(),
        &sig_out.imported_options,
    );

    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();

    assert!(
        !errors.is_empty(),
        "UFCS suspending method call in --kernel mode must produce an error; got no errors"
    );
    assert!(
        errors
            .iter()
            .any(|d| d.what.contains("kernel") || d.why.contains("kernel")),
        "error must mention 'kernel'; got: {:#?}",
        errors
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// WHY: Phase 6 inference model — transitive may-block analysis correctly marks `bar` and
// `foo` as `suspends` even without explicit `wait` tokens in their bodies. `bar` reaches
// `sleep` directly; `foo` calls `bar` transitively. `wait foo()` is now valid since
// `foo.suspends==true` — the `wait_on_non_may_block` warning no longer fires.
// This REPLACES the M2 local-predicate checkpoint. Under Phase 6, the whole fixture
// compiles clean — no warnings, no errors. Catches regressions where transitive
// propagation breaks and the analysis reverts to the old local-only predicate.
//
// POSITIVE assertion: if the fixpoint is dead or returns empty, `foo.suspends=false`,
// `wait foo()` fires "never suspends" warning → test FAILS on `stale_warnings`.
// The suspends-set check directly verifies the fixpoint produced the right result —
// a no-op fixpoint leaves the set empty and fails both assertions.
#[test]
fn transitive_no_wait_compiles_clean_under_inference() {
    // test-ratchet: Phase 6 transitive analysis makes bar.suspends=true and foo.suspends=true; wait foo() is valid; old M2-local-predicate checkpoint superseded
    // bar calls sleep without wait — Phase 6 analysis: bar.suspends = true.
    // foo calls bar() — Phase 6 analysis: foo.suspends = true (transitive).
    // wait foo() — foo.suspends is true, so this is valid-but-redundant (no warning).
    let src = r#"
function bar() -> nothing {
  sleep(100)
}
function foo() -> nothing {
  bar()
}
function entrypoint() -> nothing {
  wait foo()
}
"#;
    let out = run(src);
    // POSITIVE assertion 1: both bar and foo are in the transitive suspends set.
    // A dead fixpoint returns an empty set → bar and foo absent → this fails.
    {
        use std::collections::HashSet;
        let db = ynz_parser::CompilerDb::default();
        let sf = ynz_parser::SourceFile::new(&db, FILE.to_string(), src.to_string());
        let module = ynz_parser::parse_query(&db, sf).module.clone();
        let suspends = ynz_typeck::may_block_suspends_set(&module, &HashSet::new());
        assert!(
            suspends.contains("bar"),
            "fixpoint must mark bar as suspends (direct sleep caller); set={suspends:?}"
        );
        assert!(
            suspends.contains("foo"),
            "fixpoint must mark foo as suspends (transitive via bar); set={suspends:?}"
        );
    }
    // POSITIVE assertion 2: no "never suspends" warning for `wait foo()`.
    // If the fixpoint is broken and foo.suspends==false, `wait foo()` fires this warning.
    let stale_warnings: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Warning)
                && (d.what.contains("discards it without waiting")
                    || d.what.contains("never suspends"))
        })
        .collect();
    assert!(
        stale_warnings.is_empty(),
        "no stale bridge-era warnings expected; got: {:#?}",
        stale_warnings
    );
    // No errors either.
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "transitive fixture must compile clean; got: {:#?}",
        errors
    );
}

// WHY: `wait inner(sleepBlocking(10))` — `wait` applies to `inner`, not to `sleepBlocking` which is
// an argument. If inside_wait leaks into argument recursion, `sleepBlocking(10)` (a non-may-block
// call inside the arg list) spuriously gets a `wait_on_non_may_block` warning even though
// the user never wrote `wait sleepBlocking(10)`. The fix: clear inside_wait before recursing into
// call.args, so only the directly-awaited call sees the wait context.
#[test]
fn wait_on_non_may_block_does_not_warn_on_nested_arg_call() {
    // inner is not a state machine (no wait in body). wait inner(...) fires ONE
    // wait_on_non_may_block warning for inner — correct.
    // addOne(5) is an argument to inner — must NOT get a spurious wait_on_non_may_block.
    let src = r#"
function addOne(n: int) -> int {
  return n + 1
}
function inner(n: int) -> nothing {
  sleepBlocking(n)
}
function entrypoint() -> nothing {
  wait inner(addOne(5))
}
"#;
    let out = run(src);
    let warnings: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Warning))
        .collect();
    // Exactly ONE wait_on_non_may_block warning (on inner), none on sleepBlocking.
    let no_effect_warnings: Vec<_> = warnings
        .iter()
        .filter(|d| d.what.contains("no effect"))
        .collect();
    assert_eq!(
        no_effect_warnings.len(),
        1,
        "exactly one wait_on_non_may_block warning expected (on inner); got: {:#?}",
        warnings
    );
    // The single warning must mention 'inner'.
    assert!(
        no_effect_warnings[0].what_instead.contains("inner")
            || no_effect_warnings[0].what.contains("no effect"),
        "warning must be for inner; got: {:#?}",
        no_effect_warnings[0]
    );
    // addOne (the nested arg call) must not appear as the target of a wait_on_non_may_block warning.
    let add_one_warned = warnings
        .iter()
        .any(|d| d.what.contains("no effect") && d.what_instead.contains("addOne"));
    assert!(
        !add_one_warned,
        "addOne(5) inside an argument must not get a spurious wait_on_non_may_block; got: {:#?}",
        warnings
    );
    // No hard errors.
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(errors.is_empty(), "no errors expected; got: {:#?}", errors);
}

// WHY: Phase 6 inference — `wait wrapper(sleep(100))` where `wrapper` is not suspending.
// The remaining behavior: `wait wrapper(...)` still fires `wait_on_non_may_block` for
// `wrapper` (wrapper doesn't suspend). Guards that the `inside_wait` flag is correctly
// cleared before arg recursion so arg-position calls don't inherit the `wait` context.
#[test]
fn wait_on_non_suspending_callee_fires_for_wrapper_not_for_sleep_async_arg() {
    // test-ratchet: Fix 1 (Round 4) added sub-expression suspending call guard; sleep(100) as an argument IS a sub-expression position → now a hard error. The old Phase 6 assertion ("no hard errors") is superseded by the HALT-class fix that correctly rejects this pattern.
    // wrapper is NOT suspending (only calls sleepBlocking). wait wrapper() fires the
    // wait_on_non_may_block warning. sleep(100) in the arg is a sub-expression
    // position suspending call — Fix 1 correctly rejects it with a teaching error.
    // The entrypoint is suspending (calls sleep, even in arg position) so the
    // sub-expression guard fires.
    let src = r#"
function wrapper(n: nothing) -> nothing {
  sleepBlocking(1)
}
function entrypoint() -> nothing {
  wait wrapper(sleep(100))
}
"#;
    let out = run(src);
    // Retired unawaited_sleep_async warning must NOT fire (Phase 6 invariant retained).
    let unawaited_warns: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Warning)
                && d.what.contains("discards it without waiting")
        })
        .collect();
    assert!(
        unawaited_warns.is_empty(),
        "unawaited_sleep_async must NOT fire under Phase 6 inference; got: {:#?}",
        unawaited_warns
    );
    // Fix 1: sleep(100) in arg position IS a sub-expression suspending call.
    // The new guard correctly rejects it with a teaching error pointing at M3.
    let subexpr_errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Error)
                && d.what
                    .contains("suspending call inside a larger expression")
        })
        .collect();
    assert!(
        !subexpr_errors.is_empty(),
        "sub-expression suspending call guard must fire for sleep in arg position; \
         got: {:#?}",
        out.diagnostics
    );
    // test-ratchet: P0 reworded SubExprSuspendViolation WHY to state the Golden-Rule-7 rationale;
    // "v0.3-M3" was removed. Assert the new canonical WHY text is present instead.
    assert!(
        subexpr_errors
            .iter()
            .any(|d| d.why.contains("step-by-step") || d.why.contains("auto-parallelize")),
        "teaching error must state the step-by-step / auto-parallelize rationale; got: {:#?}",
        subexpr_errors
    );
}

// WHY: `background foo(sm_bar())` where both are suspending. The `wait_required_on_state_machine_call`
// warning (retired by Phase 6) must NOT fire — SM calls without explicit `wait` are correct under
// inference. HOWEVER, `sm_bar()` appears in an ARGUMENT position to the background spawn: arguments
// evaluate in the CALLING context before the spawn, so `sm_bar()` suspending there is the same
// sub-expression violation class as `add(inner(), 4)`. Round-4 fix closes this hole — the pattern
// now produces the subexpr teaching error (not a wait_required warning). This test verifies: (a) no
// stale wait_required-style warning, and (b) the subexpr error fires (the hole is closed).
#[test]
fn background_arg_state_machine_call_is_clean_under_inference() {
    // test-ratchet: wait_required_on_state_machine_call retired by Phase 6; sm_bar() as arg-of-background now correctly fires the subexpr suspension error (args evaluate in caller context before spawn)
    // sm_bar is suspending (contains wait). foo is also suspending.
    // background foo(sm_bar()) — sm_bar() in arg position evaluates in calling context.
    // entrypoint calls sleep directly → entrypoint.suspends = true.
    let src = r#"
function sm_bar() -> nothing {
  wait sleep(10)
}
function foo(ignored: nothing) -> nothing {
  wait sleep(10)
}
function entrypoint() -> nothing {
  wait sleep(1)
  background foo(sm_bar())
}
"#;
    let out = run(src);
    // No wait_required-style warning for sm_bar() — that diagnostic is retired.
    let state_machine_warns: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Warning)
                && d.what.contains("state-machine")
        })
        .collect();
    assert!(
        state_machine_warns.is_empty(),
        "wait_required_on_state_machine_call must NOT fire under Phase 6; got: {:#?}",
        state_machine_warns
    );
    // The sub-expression guard MUST fire: sm_bar() in arg position is a violation.
    let subexpr_errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Error)
                && d.what
                    .contains("suspending call inside a larger expression")
        })
        .collect();
    assert!(
        !subexpr_errors.is_empty(),
        "sm_bar() in background arg position must produce 'suspending call inside a larger expression' error; got: {:#?}",
        out.diagnostics
    );
}

// WHY: `wait __testFallibleAsync(true)` must typecheck cleanly through the production
// check_query path. Before Fix B, the production table lacked with_m2_internals() so
// lookup_free_fn_including_internal returned None and the else branch emitted a self-
// contradictory arity error: "`__testFallibleAsync` takes 1 argument, got 1."
// This confirms that the internal intrinsic is accessible in production typeck.
#[test]
fn wait_test_fallible_async_true_is_clean() {
    let src = r#"
function entrypoint() -> nothing {
  wait __testFallibleAsync(true)
}
"#;
    assert_clean(src);
}

// WHY: `wait __testFallibleAsync(false)` must also typecheck cleanly (the false path
// exercises the error-return branch at runtime; the typeck concern is just arity + type).
#[test]
fn wait_test_fallible_async_false_is_clean() {
    let src = r#"
function entrypoint() -> nothing {
  wait __testFallibleAsync(false)
}
"#;
    assert_clean(src);
}

// WHY: `wait __testFallibleAsync()` with ZERO args must produce a REAL arity error
// ("`__testFallibleAsync` takes 1 argument, got 0."), not the self-contradictory
// "takes 1, got 1" that fired before Fix B. The else-branch in the dispatch arm means
// "wrong arity" and must only fire for genuinely wrong arg counts.
#[test]
fn wait_test_fallible_async_zero_args_gives_real_arity_error() {
    let src = r#"
function entrypoint() -> nothing {
  wait __testFallibleAsync()
}
"#;
    let out = run(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        !errors.is_empty(),
        "zero-arg __testFallibleAsync must produce an arity error; got: {:#?}",
        out.diagnostics
    );
    // The error must say "got 0" — not "got 1" (the pre-fix self-contradiction).
    let has_got_zero = errors.iter().any(|d| d.what.contains("got 0"));
    assert!(
        has_got_zero,
        "arity error must say 'got 0', not the self-contradictory 'got 1'; got: {:#?}",
        errors
    );
}

// WHY: P1 lifted the cross-module stopgap. A suspending function calling a non-suspending
// cross-module callee must compile clean — the compiler now propagates `suspends` flags
// across module boundaries via check_query, so it knows remoteOp doesn't suspend and
// allows the call without error. Guards regressions where the stopgap is re-introduced
// and cross-module calls from suspending functions are blocked again.
#[test]
fn cross_module_call_from_suspending_fn_compiles_clean() {
    let dir = std::env::temp_dir().join(format!(
        "ynz_cross_mod_clean_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");

    // utils.ynz — exported pure-CPU function (non-suspending)
    let utils_src = "export function remoteOp() -> nothing { print(`remote op`) }";
    // entrypoint.ynz — suspending function (reaches sleep) calling the non-suspending import
    let entry_src = "import { remoteOp } from `utils`\n\
                     function entrypoint() -> nothing { sleep(10)\n  remoteOp() }";

    let utils_path = dir.join("utils.ynz");
    let entry_path = dir.join("entrypoint.ynz");
    std::fs::write(&utils_path, utils_src).expect("write utils.ynz");
    std::fs::write(&entry_path, entry_src).expect("write entrypoint.ynz");

    let mut db = ynz_parser::CompilerDb::default();
    let sf_utils =
        ynz_parser::SourceFile::new(&db, utils_path.display().to_string(), utils_src.to_string());
    let sf_entry =
        ynz_parser::SourceFile::new(&db, entry_path.display().to_string(), entry_src.to_string());
    db.register_source(sf_utils);
    db.register_source(sf_entry);

    let output = ynz_typeck::check_query(&db, sf_entry);
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "cross-module call from suspending fn must compile clean after P1; got errors: {:#?}",
        errors
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// WHY: P1 lifted the cross-module stopgap. A suspending function that independently
// reaches sleep AND calls a non-suspending cross-module callee must compile clean.
// The compiler knows `maybeBlocking` doesn't suspend (via check_query propagation),
// so the call is safe inside a state machine. Guards regressions where the stopgap
// is re-introduced for the "has local sleep + cross-module call" specific case.
#[test]
fn cross_module_call_from_fn_with_local_sleep_compiles_clean() {
    let dir = std::env::temp_dir().join(format!(
        "ynz_local_sleep_cross_mod_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");

    // utils.ynz — non-suspending export
    let utils_src = "export function maybeBlocking() -> nothing { print(`runs`) }";
    // entrypoint.ynz — suspending (reaches sleep) AND calls the non-suspending import
    let entry_src = "import { maybeBlocking } from `utils`\n\
                     function entrypoint() -> nothing { sleep(1)\n  maybeBlocking() }";

    let utils_path = dir.join("utils.ynz");
    let entry_path = dir.join("entrypoint.ynz");
    std::fs::write(&utils_path, utils_src).expect("write utils.ynz");
    std::fs::write(&entry_path, entry_src).expect("write entrypoint.ynz");

    let mut db = ynz_parser::CompilerDb::default();
    let sf_utils =
        ynz_parser::SourceFile::new(&db, utils_path.display().to_string(), utils_src.to_string());
    let sf_entry =
        ynz_parser::SourceFile::new(&db, entry_path.display().to_string(), entry_src.to_string());
    db.register_source(sf_utils);
    db.register_source(sf_entry);

    let output = ynz_typeck::check_query(&db, sf_entry);
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "suspending fn + non-suspending cross-module call must compile clean after P1; got: {:#?}",
        errors
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// WHY: a function that independently suspends (local sleep) AND makes a
// dynamic-dispatch call through a vtable gets the can't-infer compile error. This is
// the design-correct gate per design/future/concurrency.md:75 — the caller already IS
// a state machine, and calling an unanalyzable vtable from inside a state machine
// requires the fence. A non-suspending caller with only dynamic dispatch compiles clean
// (M2 under-approximation — dynamic suspension propagation deferred to a future version).
// Guards regressions where the check fires on non-suspending dynamic callers (R2 regression).
#[test]
fn cant_infer_suspension_dynamic_dispatch_with_local_sleep_fires_error() {
    let src = r#"
shape Worker {
    doWork(share self) -> nothing
}

shape FastWorker follows Worker {
    speed: int
}

function doWork(share self: FastWorker) -> nothing {
    print(`fast`)
}

function runWorker(w: dynamic Worker) -> nothing {
    sleep(1)
    w.doWork()
}

function entrypoint() -> nothing {
    let fw: FastWorker = { speed: 10 }
    runWorker(fw)
}
"#;
    let out = run(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        !errors.is_empty(),
        "suspending fn + dynamic-dispatch call must produce a can't-infer error; got: {:#?}",
        out.diagnostics
    );
    let has_cant_infer = errors
        .iter()
        .any(|d| d.what.contains("Can't determine") && d.what.contains("dynamic-dispatch"));
    assert!(
        has_cant_infer,
        "error must mention Can't-determine and dynamic-dispatch; got: {:#?}",
        errors
    );
}

// WHY: the free-fn form `dispatch(w)` where `w: dynamic Worker` and `dispatch` expects
// `dynamic Worker` must fire the can't-infer error when the caller independently suspends.
// Guards the code path at check.rs:2048-2068 (arg-loop Dynamic==Dynamic gate) — distinct
// from the dot-call vtable path at check.rs:2543. The scenario: relay suspends (sleep)
// and passes an already-dynamic value `w` to `dispatch(w: dynamic Worker)`. The gate fires
// because expected_ty=Dynamic Worker, actual_ty=Dynamic Worker, and current_fn_suspends=true.
// Without this test, the free-fn gate could be removed while the dot-call test still passes.
#[test]
fn cant_infer_suspension_dynamic_dispatch_free_fn_form_fires_error() {
    let src = r#"
shape Worker {
    doWork(share self) -> nothing
}

shape FastWorker follows Worker {
    speed: int
}

function doWork(share self: FastWorker) -> nothing {
    print(`fast`)
}

function dispatch(w: dynamic Worker) -> nothing {
    w.doWork()
}

function relay(w: dynamic Worker) -> nothing {
    sleep(1)
    dispatch(w)
}

function entrypoint() -> nothing {
    let fw: FastWorker = { speed: 10 }
    relay(fw)
}
"#;
    let out = run(src);
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert!(
        !errors.is_empty(),
        "suspending fn + free-fn dynamic-dispatch call must produce a can't-infer error; got: {:#?}",
        out.diagnostics
    );
    let has_cant_infer = errors
        .iter()
        .any(|d| d.what.contains("Can't determine") && d.what.contains("dynamic"));
    assert!(
        has_cant_infer,
        "error must mention Can't-determine and dynamic; got: {:#?}",
        errors
    );
}

// WHY: a non-suspending function that only makes a dynamic-dispatch call must NOT fire
// the can't-infer error. Guards the R2 regression: design/future/concurrency.md:75
// specifies that dynamic dispatch from a non-suspending caller is treated as a
// non-suspending leaf in the M2 under-approximation (cross-module/dynamic suspension
// propagation is deferred to M3+M8). A future gate edit that re-broke R2 (over-firing
// on non-suspending callers) would make THIS test fail. The assertion is non-vacuous:
// it filters diagnostics to those mentioning "Can't determine" and asserts that filtered
// list is empty — so re-enabling the gate unconditionally causes a loud failure here.
#[test]
fn dynamic_dispatch_non_suspending_caller_compiles_clean() {
    let src = r#"
shape Worker {
    doWork(share self) -> nothing
}

shape FastWorker follows Worker {
    speed: int
}

function doWork(share self: FastWorker) -> nothing {
    print(`fast`)
}

function relay(w: dynamic Worker) -> nothing {
    w.doWork()
}

function entrypoint() -> nothing {
    let fw: FastWorker = { speed: 10 }
    relay(fw)
}
"#;
    let out = run(src);
    let cant_infer_errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Error)
                && d.what.contains("Can't determine")
        })
        .collect();
    assert!(
        cant_infer_errors.is_empty(),
        "non-suspending dynamic caller must NOT fire can't-infer error; \
         re-enabling the gate unconditionally would break the M2 under-approximation \
         (design/future/concurrency.md:75). got: {:#?}",
        cant_infer_errors
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// v0.3-M3b P1: cross-module `suspends` propagation — salsa incremental tests
//
// These three tests probe the THREE directions where salsa staleness can hide:
// non-suspending→suspending propagation, suspending→non-suspending clearance, and diamond fan-out.
// All use real temp files because resolve_module_path canonicalizes via std::fs.
// ─────────────────────────────────────────────────────────────────────────────

/// Build a named temp dir with yinz.toml for import resolution.
fn make_salsa_temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ynz_m3b_salsa_{tag}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").expect("write yinz.toml");
    dir
}

#[test]
fn incremental_non_suspending_to_suspending_flips_caller_to_state_machine() {
    // WHY: When module A's exported function changes from non-suspending to suspending
    // (adds a sleep call), module B's caller must be re-checked and promoted to a state
    // machine. Guards salsa staleness where B's check_query returns a cached result
    // with suspends_set missing the caller even after A's source changes.
    // Tests the load_export_table → check_query dependency chain in resolve_import.rs.
    use salsa::Setter as _;

    let dir = make_salsa_temp_dir("non_suspending_to_suspending");
    let a_path = dir.join("a.ynz");
    let b_path = dir.join("b.ynz");

    // v1: A exports a non-suspending function.
    let a_v1 = "export function op() -> nothing { print(`op`) }";
    let b_src = "import { op } from `a`\nfunction caller() -> nothing { op() }";

    std::fs::write(&a_path, a_v1).expect("write a.ynz v1");
    std::fs::write(&b_path, b_src).expect("write b.ynz");

    let mut db = CompilerDb::default();
    let sf_a = SourceFile::new(&db, a_path.display().to_string(), a_v1.to_string());
    let sf_b = SourceFile::new(&db, b_path.display().to_string(), b_src.to_string());
    db.register_source(sf_a);
    db.register_source(sf_b);

    let out1 = check_query(&db, sf_b);
    assert!(
        !out1.suspends_set.contains("caller"),
        "v1: caller must NOT be in suspends_set (op is non-suspending); got: {:?}",
        out1.suspends_set
    );

    // v2: A's op now calls sleep — it becomes suspending.
    let a_v2 = "export function op() -> nothing { sleep(10)\nprint(`op`) }";
    std::fs::write(&a_path, a_v2).expect("write a.ynz v2");
    sf_a.set_text(&mut db).to(a_v2.to_string());

    let out2 = check_query(&db, sf_b);
    assert!(
        out2.suspends_set.contains("caller"),
        "v2: caller must be in suspends_set (op is now suspending via sleep); \
         salsa staleness would leave suspends_set unchanged. got: {:?}",
        out2.suspends_set
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn incremental_suspending_to_non_suspending_clears_caller_from_state_machine() {
    // WHY: When module A's exported function changes from suspending to non-suspending
    // (removes the sleep call), module B's caller must be re-checked and the state
    // machine classification must be cleared. Guards the inverse staleness: stale
    // over-marking that leaves a function as a state machine after its only suspending
    // callee is removed. A false SM is a correctness bug (wrong codegen path).
    use salsa::Setter as _;

    let dir = make_salsa_temp_dir("suspending_to_non_suspending");
    let a_path = dir.join("a.ynz");
    let b_path = dir.join("b.ynz");

    // v1: A exports a suspending function.
    let a_v1 = "export function op() -> nothing { sleep(10)\nprint(`op`) }";
    let b_src = "import { op } from `a`\nfunction caller() -> nothing { op() }";

    std::fs::write(&a_path, a_v1).expect("write a.ynz v1");
    std::fs::write(&b_path, b_src).expect("write b.ynz");

    let mut db = CompilerDb::default();
    let sf_a = SourceFile::new(&db, a_path.display().to_string(), a_v1.to_string());
    let sf_b = SourceFile::new(&db, b_path.display().to_string(), b_src.to_string());
    db.register_source(sf_a);
    db.register_source(sf_b);

    let out1 = check_query(&db, sf_b);
    assert!(
        out1.suspends_set.contains("caller"),
        "v1: caller must be in suspends_set (op suspends); got: {:?}",
        out1.suspends_set
    );

    // v2: A's op removes sleep — it becomes non-suspending.
    let a_v2 = "export function op() -> nothing { print(`op`) }";
    std::fs::write(&a_path, a_v2).expect("write a.ynz v2");
    sf_a.set_text(&mut db).to(a_v2.to_string());

    let out2 = check_query(&db, sf_b);
    assert!(
        !out2.suspends_set.contains("caller"),
        "v2: caller must NOT be in suspends_set (op no longer suspends); \
         stale over-marking would leave caller as a state machine. got: {:?}",
        out2.suspends_set
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn incremental_diamond_a_imported_by_b_and_c_both_rechecked() {
    // WHY: When A is the common dependency in a diamond (A→B, A→C), toggling A's
    // suspending status must invalidate BOTH B's and C's check_query results, not
    // just the first one salsa happens to re-run. Guards a staleness class where the
    // second importer sees the cached (pre-toggle) result because salsa only
    // propagated the dependency to one of the two importers.
    use salsa::Setter as _;

    let dir = make_salsa_temp_dir("diamond");
    let a_path = dir.join("a.ynz");
    let b_path = dir.join("b.ynz");
    let c_path = dir.join("c.ynz");

    let a_v1 = "export function op() -> nothing { print(`op`) }";
    let b_src = "import { op } from `a`\nfunction b_caller() -> nothing { op() }";
    let c_src = "import { op } from `a`\nfunction c_caller() -> nothing { op() }";

    std::fs::write(&a_path, a_v1).expect("write a.ynz v1");
    std::fs::write(&b_path, b_src).expect("write b.ynz");
    std::fs::write(&c_path, c_src).expect("write c.ynz");

    let mut db = CompilerDb::default();
    let sf_a = SourceFile::new(&db, a_path.display().to_string(), a_v1.to_string());
    let sf_b = SourceFile::new(&db, b_path.display().to_string(), b_src.to_string());
    let sf_c = SourceFile::new(&db, c_path.display().to_string(), c_src.to_string());
    db.register_source(sf_a);
    db.register_source(sf_b);
    db.register_source(sf_c);

    let out_b1 = check_query(&db, sf_b);
    let out_c1 = check_query(&db, sf_c);
    assert!(
        !out_b1.suspends_set.contains("b_caller"),
        "v1/b: b_caller must NOT be in suspends_set; got: {:?}",
        out_b1.suspends_set
    );
    assert!(
        !out_c1.suspends_set.contains("c_caller"),
        "v1/c: c_caller must NOT be in suspends_set; got: {:?}",
        out_c1.suspends_set
    );

    // A's op now calls sleep — both B and C must be re-checked.
    let a_v2 = "export function op() -> nothing { sleep(10)\nprint(`op`) }";
    std::fs::write(&a_path, a_v2).expect("write a.ynz v2");
    sf_a.set_text(&mut db).to(a_v2.to_string());

    let out_b2 = check_query(&db, sf_b);
    let out_c2 = check_query(&db, sf_c);
    assert!(
        out_b2.suspends_set.contains("b_caller"),
        "v2/b: b_caller must be in suspends_set after A becomes suspending; got: {:?}",
        out_b2.suspends_set
    );
    assert!(
        out_c2.suspends_set.contains("c_caller"),
        "v2/c: c_caller must be in suspends_set after A becomes suspending; \
         both diamond legs must be re-checked, not just the first. got: {:?}",
        out_c2.suspends_set
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// WHY: The EC-method-dispatch fix (check.rs:1826-1867) restores `ErrorsCapable<T>` for
// `.or`/`.failed`/`.message`/`.suggestions`/`.trace`/`.source` when resolve_ident strips
// the wrapper during auto-propagation inside an errors-capable function. Four adversarial
// cases guard the narrow restoration logic:
//   (a) `.failed` and `.message` on a let-bound EC value resolve correctly — not just `.or`.
//   (b) an inner `let x: int` shadows an outer `let x: T errors` — `.or(0)` on the inner
//       `x` must NOT restore EC (the inner binding is int, not ErrorsCapable).
//   (c) an EC-method name on a non-EC binding in an EC function must not trigger restoration.
//   (d) EC method names inside a non-errors-capable function dispatch as normal methods
//       (no restoration applies because the function context is not EC).
// All four assert error-count and, where relevant, the absence of spurious EC error.

#[test]
fn ec_method_dispatch_failed_and_message_resolve_in_ec_fn() {
    // WHY: guards that `.failed()` and `.message` (not just `.or`) work on a let-bound
    // EC value inside an errors-capable function. The restoration check in check.rs:1841
    // covers the full EC_METHODS set — missing methods would produce "has no method `failed`"
    // even though the value is `T errors`. Catches narrow restoration that forgets siblings.
    assert_clean(
        r#"
function compute() -> int errors {
  return 42
}

function entrypoint() -> nothing {
  let x = compute()
  const failed = x.failed()
  const v = x.or(0)
  print(failed.toString())
  print(v.toString())
}
"#,
    );
}

#[test]
fn ec_method_no_over_restoration_when_inner_shadows_outer_ec() {
    // WHY: the shadowing boundary check — an inner `let x: int` must block EC restoration
    // on `x.or(0)` even when an outer `let x: T errors` is in scope. The fix looks up the
    // INNERMOST scope binding; if the lookup incorrectly walks to the outer binding, it would
    // restore ErrorsCapable and produce a wrong dispatch (or silently accept int.or(0) as EC).
    // This test asserts the OUTER-x code is still EC-clean (baseline), and the INNER-x code
    // produces a type error because int has no `.or` method (the restoration must NOT fire).
    // One error expected: int does not have method `or`.
    let out = assert_errors(
        r#"
function compute() -> int errors {
  return 42
}

function entrypoint() -> nothing {
  let x = compute()
  const outer_v = x.or(0)
  print(outer_v.toString())
  let x: int = 7
  const inner_v = x.or(0)
  print(inner_v.toString())
}
"#,
        1,
    );
    // The error must be about `or` on a non-EC type, not a false "unknown binding" error.
    assert!(
        out.diagnostics
            .iter()
            .any(|d| (d.what.contains("or") || d.what.contains("`int`"))
                && d.severity == ynz_diagnostics::Severity::Error),
        "error must be about `or` on int (no EC restoration for shadowed inner int binding); \
         got: {:#?}",
        out.diagnostics
    );
}

#[test]
fn ec_method_named_call_on_non_ec_binding_no_restoration() {
    // WHY: an EC-method-named call (`.or(0)`) on a non-EC binding in an errors-capable
    // function must not trigger restoration. The restoration check (check.rs:1845) guards
    // with `matches!(entry.ty, Type::ErrorsCapable { .. })`. If this guard is absent or
    // wrong, a plain `int` variable would be promoted to ErrorsCapable for dispatch,
    // hiding the type error and miscompiling the call. One error expected: int.or not found.
    let out = assert_errors(
        r#"
function compute() -> int errors {
  return 42
}

function entrypoint() -> nothing {
  let n: int = 5
  let _ = n.or(0)
  let x = compute()
  const v = x.or(0)
  print(v.toString())
}
"#,
        1,
    );
    assert!(
        out.diagnostics.iter().any(|d| {
            d.severity == ynz_diagnostics::Severity::Error
                && (d.what.contains("or") || d.what.contains("`int`"))
        }),
        "error must be about `or` on int; restoration must not fire for non-EC binding; \
         got: {:#?}",
        out.diagnostics
    );
}

#[test]
fn ec_method_named_call_in_non_ec_fn_no_restoration() {
    // WHY: EC-method restoration is gated on the caller being inside an errors-capable
    // function (check.rs:1841: the fix runs under Expr::MethodCall, but the auto-propagation
    // that causes stripping only fires when current_fn_errors_capable=true). In a non-EC
    // function, resolve_ident does NOT strip ErrorsCapable — so the restoration path is
    // never entered. This test verifies the baseline: in a non-EC function, calling `.or`
    // on a non-EC type still produces exactly one type error (the method is not found on
    // the receiver type), and no spurious "kernel" or "EC" errors appear.
    // Confirms no false EC restoration fires in a non-EC function context.
    let out = assert_errors(
        r#"
function entrypoint() -> nothing {
  let n: int = 5
  let _ = n.or(0)
}
"#,
        1,
    );
    assert!(
        out.diagnostics.iter().any(|d| {
            d.severity == ynz_diagnostics::Severity::Error
                && (d.what.contains("or") || d.what.contains("`int`"))
        }),
        "error must be a type error about `or` on int in a non-EC fn; got: {:#?}",
        out.diagnostics
    );
}

#[test]
fn ec_method_message_resolves_in_ec_fn() {
    // WHY: guards that `.message` resolves on an EC value inside an errors-capable function.
    // EC_METHODS in check.rs includes "message" — removing it would produce "has no method
    // `message`" on the stripped inner type. The `ec_method_dispatch_failed_and_message_resolve_in_ec_fn`
    // test covers `.failed` and `.message` together; this test asserts `.message` alone
    // compiles clean so a narrow regression (removing "message" from EC_METHODS) is caught
    // even if `.failed` still passes.
    assert_clean(
        r#"
function compute() -> int errors {
  return 42
}

function entrypoint() -> nothing {
  let x = compute()
  const msg = x.message
  print(msg)
}
"#,
    );
}

#[test]
fn ec_method_suggestions_resolves_in_ec_fn() {
    // WHY: guards that `.suggestions` resolves on an EC value inside an errors-capable function.
    // Removing "suggestions" from EC_METHODS drops the restoration and produces a type error
    // on the stripped inner-type dispatch. Isolated test ensures the sibling is independently
    // covered — a single combined test cannot pinpoint which EC_METHODS entry was dropped.
    // `.suggestions` returns array<string> — printable directly (BuiltinArray is printable).
    assert_clean(
        r#"
function compute() -> int errors {
  return 42
}

function entrypoint() -> nothing {
  let x = compute()
  const sug = x.suggestions
  print(sug)
}
"#,
    );
}

#[test]
fn ec_method_trace_resolves_in_ec_fn() {
    // WHY: guards that `.trace` resolves on an EC value inside an errors-capable function.
    // Same rationale as the `.suggestions` test above — each EC_METHODS sibling must have
    // its own isolated test so a narrow removal is caught by exactly one failure.
    // `.trace` returns array<Frame> — BuiltinArray is printable; Frame is a compiler shape.
    assert_clean(
        r#"
function compute() -> int errors {
  return 42
}

function entrypoint() -> nothing {
  let x = compute()
  const t = x.trace
  print(t)
}
"#,
    );
}

#[test]
fn ec_method_source_resolves_in_ec_fn() {
    // WHY: guards that `.source` resolves on an EC value inside an errors-capable function.
    // Same rationale as the `.trace` test above. Completes the sibling coverage for all six
    // members of EC_METHODS: or, failed, message, suggestions, trace, source.
    assert_clean(
        r#"
function compute() -> int errors {
  return 42
}

function entrypoint() -> nothing {
  let x = compute()
  const src = x.source
  print(src)
}
"#,
    );
}

#[test]
fn ec_method_dispatch_after_failed_guard_narrowing() {
    // WHY: locks the errors_success_narrowed channel. When `x.failed()` returns true,
    // the `if` guard narrows `x` to its inner success type (errors_success_narrowed.insert).
    // After the guard, `x.or(0)` is called on the already-narrowed (non-EC) binding.
    // Current behavior: accepted because the narrowed type is the inner type and `.or` is
    // looked up on int — which should produce a type error (int has no `.or`), unless the
    // restoration check incorrectly re-promotes. This asserts exactly clean compilation for
    // the non-failing-branch usage pattern: the or() call outside the failed guard on the
    // original binding is valid, and the inner-guard int binding has no such EC method.
    assert_clean(
        r#"
function compute() -> int errors {
  return 42
}

function entrypoint() -> nothing {
  let x = compute()
  if (x.failed()) {
    print(`failed`)
  }
  const v = x.or(0)
  print(v.toString())
}
"#,
    );
}

#[test]
fn ec_method_dispatch_on_const_bound_ec_value_in_ec_fn() {
    // WHY: all existing EC-method tests use `let`-bound EC values. `const`-bound EC values
    // go through the same restore path (scope lookup by name, type check for ErrorsCapable)
    // but the binding is `const` — checks that the restoration does not depend on mutability.
    // If the restoration incorrectly gates on `let` (mutable binding), `const x = compute()`
    // would fail to restore and `.or(0)` would produce "int has no method `or`".
    assert_clean(
        r#"
function compute() -> int errors {
  return 42
}

function entrypoint() -> nothing {
  const x = compute()
  const v = x.or(0)
  print(v.toString())
}
"#,
    );
}
