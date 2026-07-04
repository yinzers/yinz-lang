//! Golden tests for the AST walker: format a .ynz fixture, compare against .formatted.
//!
//! Scope: comment-free fixtures covering one AST node category each. Every test also
//! asserts idempotency: `format(format(x)) == format(x)` byte-identical.

use std::path::Path;

fn golden(fixture_name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/walker");
    let input_path = dir.join(format!("{fixture_name}.ynz"));
    let expected_path = dir.join(format!("{fixture_name}.formatted"));

    let source = std::fs::read_to_string(&input_path).expect("fixture .ynz not found");
    let expected = std::fs::read_to_string(&expected_path).expect("fixture .formatted not found");

    let got = ynz_fmt::format(&source).expect("format failed");
    assert_eq!(
        got, expected,
        "\nFormatted output for {fixture_name} did not match golden file.\
        \nDiff (got vs expected):\n{got}"
    );

    // Idempotency: formatting the output again must produce the same result
    let got2 = ynz_fmt::format(&got).expect("format idempotency second pass failed");
    assert_eq!(got, got2, "formatter is not idempotent on {fixture_name}");
}

// ── Edge cases ────────────────────────────────────────────────────────────────

#[test]
fn empty_source_yields_empty_output() {
    let got = ynz_fmt::format("").expect("empty source should not error");
    assert!(
        got.is_empty() || got == "\n",
        "empty source produced unexpected output: {got:?}"
    );
}

#[test]
fn trailing_newline_added() {
    let source = "function f() -> nothing {}";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(
        got.ends_with('\n'),
        "output must end with a trailing newline"
    );
}

#[test]
fn nested_generic_keeps_lexer_required_space() {
    // WHY: the lexer reads `>>` as ONE token (no maximal-munch split), so a
    // nested generic in final argument position MUST keep the space between
    // the consecutive `>`s or format's output does not re-parse (RED-proven:
    // the driver-fixture idempotency sweep failed on the first nested-generic
    // annotation in the tree, v0.3-M5 P2 fix round 2).
    let source =
        "function f() -> nothing {\n  let picks: map<string, maybe<int> > = { `a`: 1 }\n}\n";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(
        got.contains("map<string, maybe<int> >"),
        "nested generic must keep the space before the outer `>`; got:\n{got}"
    );
    let got2 = ynz_fmt::format(&got).expect("format output must re-parse");
    assert_eq!(got, got2, "formatter is not idempotent on nested generics");
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

// ── Regression tests ──────────────────────────────────────────────────────────

#[test]
fn errors_capable_suffix_not_doubled() {
    // WHY: FunctionDecl.errors_capable (bool) and Type::ErrorCapable both carry the
    //      `errors` suffix. Prior to this fix, the formatter appended BOTH, producing
    //      `-> string errors errors`. A future refactor that "helpfully" re-enables the
    //      errors_capable flag would break this test — the test catches that regression.
    let source = "function readConfig() -> string errors { return `ok` }\n";
    let got = ynz_fmt::format(source).expect("format failed");
    let count = got.matches(" errors").count();
    assert_eq!(
        count, 1,
        "expected exactly one ` errors` in output, got {count}: {got}"
    );
    assert!(
        !got.contains("errors errors"),
        "double errors suffix: {got}"
    );
}

// ── Specific invariant tests ──────────────────────────────────────────────────

#[test]
fn backtick_string_preserved_byte_exact() {
    // Backtick string content (including interpolations) must be preserved exactly.
    let source =
        "function f(name: string, val: int) -> string { return `hello ${name}, val=${val}!` }\n";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(
        got.contains("`hello ${name}, val=${val}!`"),
        "backtick content was mangled: {got}"
    );
}

#[test]
fn block_opener_on_same_line_as_keyword() {
    // `{` must stay on the same line as `function`/`if`/`while`/`for`, not on its own line.
    let source = "function f() -> nothing { let x = 1 }\n";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(
        got.contains("function f() -> nothing {"),
        "block opener moved: {got}"
    );
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
    assert!(
        got.contains("(a + b) * c"),
        "precedence parens dropped: {got}"
    );
}

#[test]
fn higher_precedence_subexpr_no_parens() {
    // a + b * c — the * binds tighter; no parens needed.
    let source = "function f(a: int, b: int, c: int) -> int { return a + b * c }\n";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(got.contains("a + b * c"), "spurious parens added: {got}");
    assert!(
        !got.contains("(b * c)"),
        "spurious parens around high-precedence right child: {got}"
    );
}

// test-ratchet: temp diagnostic test added to investigate proptest failure;
// removed now that root cause (parser greedily merges adjacent BacktickString tokens)
// is understood and the proptest strategy fixed. The invariant is now tested by
// proptest_idempotency + proptest_smoke which run 256 randomized cases.
