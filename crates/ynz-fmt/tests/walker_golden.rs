//! Golden tests for the AST walker: format a .ynz fixture, compare against .formatted.
//!
//! Phase 2 scope: no comments — the input fixtures have none. All 15 fixtures cover
//! a different AST node category per the Phase 2 plan.

use std::path::Path;

fn golden(fixture_name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/walker");
    let input_path = dir.join(format!("{fixture_name}.ynz"));
    let expected_path = dir.join(format!("{fixture_name}.formatted"));

    let source =
        std::fs::read_to_string(&input_path).expect("fixture .ynz not found");
    let expected =
        std::fs::read_to_string(&expected_path).expect("fixture .formatted not found");

    let got = ynz_fmt::format(&source).expect("format failed");
    assert_eq!(
        got, expected,
        "\nFormatted output for {fixture_name} did not match golden file.\
        \nDiff (got vs expected):\n{got}"
    );

    // Idempotency: formatting the output again must produce the same result
    let got2 = ynz_fmt::format(&got).expect("format idempotency second pass failed");
    assert_eq!(
        got, got2,
        "formatter is not idempotent on {fixture_name}"
    );
}

// ── Edge cases ────────────────────────────────────────────────────────────────

#[test]
fn empty_source_yields_empty_output() {
    let got = ynz_fmt::format("").expect("empty source should not error");
    assert!(got.is_empty() || got == "\n", "empty source produced unexpected output: {got:?}");
}

#[test]
fn trailing_newline_added() {
    let source = "function f() -> nothing {}";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(got.ends_with('\n'), "output must end with a trailing newline");
}

// ── Per-node-category golden tests ───────────────────────────────────────────

#[test]
fn function_decl() {
    golden("function_decl");
}

#[test]
fn shape_decl() {
    golden("shape_decl");
}

#[test]
fn if_else() {
    golden("if_else");
}

#[test]
fn while_loop() {
    golden("while_loop");
}

#[test]
fn for_loop() {
    golden("for_loop");
}

#[test]
fn match_chain() {
    golden("match_chain");
}

#[test]
fn array_literal() {
    golden("array_literal");
}

#[test]
fn shape_literal() {
    golden("shape_literal");
}

#[test]
fn arithmetic_expr() {
    golden("arithmetic_expr");
}

#[test]
fn string_interpolation() {
    golden("string_interpolation");
}

#[test]
fn import_stmt() {
    golden("import_stmt");
}

#[test]
fn generic_function() {
    golden("generic_function");
}

#[test]
fn ufcs_call() {
    golden("ufcs_call");
}

#[test]
fn let_const() {
    golden("let_const");
}

#[test]
fn operator_precedence() {
    golden("operator_precedence");
}

// ── Specific invariant tests ──────────────────────────────────────────────────

#[test]
fn backtick_string_preserved_byte_exact() {
    // Backtick string content (including interpolations) must be preserved exactly.
    let source = "function f(name: string, val: int) -> string { return `hello ${name}, val=${val}!` }\n";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(got.contains("`hello ${name}, val=${val}!`"), "backtick content was mangled: {got}");
}

#[test]
fn block_opener_on_same_line_as_keyword() {
    // `{` must stay on the same line as `function`/`if`/`while`/`for`, not on its own line.
    let source = "function f() -> nothing { let x = 1 }\n";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(got.contains("function f() -> nothing {"), "block opener moved: {got}");
}

#[test]
fn output_ends_in_exactly_one_trailing_newline() {
    let source = "function f() -> int { return 42 }\n";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(got.ends_with('\n'), "must end with newline");
    assert!(!got.ends_with("\n\n"), "must not end with double newline");
}

#[test]
fn operator_precedence_parens_preserved() {
    // The AST stores the precedence correctly; the formatter must not lose parens.
    let source = "function f(a: int, b: int, c: int) -> int { return (a + b) * c }\n";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(got.contains("(a + b) * c"), "precedence parens dropped: {got}");
}

#[test]
fn higher_precedence_subexpr_no_parens() {
    // a + b * c — the * binds tighter; no parens needed.
    let source = "function f(a: int, b: int, c: int) -> int { return a + b * c }\n";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(got.contains("a + b * c"), "spurious parens added: {got}");
    assert!(!got.contains("(b * c)"), "spurious parens around high-precedence right child: {got}");
}
