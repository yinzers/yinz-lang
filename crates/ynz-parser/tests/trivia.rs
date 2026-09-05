//! Tests that `lex_with_trivia` produces the same token stream as `lex` for the same
//! input, with a parallel `Vec<Comment>` capturing every `//` and `///` comment.

use ynz_parser::{lex_with_trivia, lexer::lex, CommentKind};

const FILE: &str = "<trivia_test>";

fn tokens_equal(source: &str) -> bool {
    let (plain_tokens, _diags) = lex(FILE, source);
    let (trivia_tokens, _comments) = lex_with_trivia(FILE, source);
    // Token values + spans must be identical
    plain_tokens == trivia_tokens
}

// ── Identity tests: lex == lex_with_trivia.tokens for 20+ inputs ───────────────

#[test]
fn empty_source() {
    assert!(tokens_equal(""));
}

#[test]
fn just_whitespace() {
    assert!(tokens_equal("   \n\t  "));
}

#[test]
fn single_line_comment_only() {
    assert!(tokens_equal("// hello world"));
}

#[test]
fn doc_comment_only() {
    assert!(tokens_equal("/// doc line"));
}

#[test]
fn line_comment_before_function() {
    assert!(tokens_equal("// comment\nfunction foo() -> nothing {}"));
}

#[test]
fn inline_comment_after_statement() {
    assert!(tokens_equal(
        "function f() -> nothing { let x = 1 // inline\n }"
    ));
}

#[test]
fn multiple_line_comments() {
    assert!(tokens_equal("// a\n// b\n// c\nfunction f() -> nothing {}"));
}

#[test]
fn doc_and_line_comment_mixed() {
    assert!(tokens_equal("/// doc\n// note\nfunction f() -> nothing {}"));
}

#[test]
fn comment_inside_function_body() {
    assert!(tokens_equal(
        "function f() -> nothing {\n  // body comment\n  let x = 1\n}"
    ));
}

#[test]
fn comment_between_two_functions() {
    assert!(tokens_equal(
        "function f() -> nothing {}\n// between\nfunction g() -> nothing {}"
    ));
}

#[test]
fn comment_in_shape_field() {
    assert!(tokens_equal(
        "shape Foo {\n  x: int // x field\n  y: int // y field\n}"
    ));
}

#[test]
fn backtick_string_no_comments() {
    assert!(tokens_equal("function f() -> nothing { let s = `hello` }"));
}

#[test]
fn backtick_string_with_interpolation() {
    assert!(tokens_equal(
        "function f(name: string) -> nothing { let s = `Hello ${name}!` }"
    ));
}

#[test]
fn double_slash_inside_backtick_is_not_a_comment() {
    // `//` inside backtick string content should NOT be captured as a comment
    let source = "function f() -> nothing { let s = `http://example.com` }";
    let (plain, _) = lex(FILE, source);
    let (trivia, comments) = lex_with_trivia(FILE, source);
    assert_eq!(plain, trivia);
    assert!(
        comments.is_empty(),
        "// inside backtick must not be captured as comment"
    );
}

#[test]
fn only_line_comments_no_decls() {
    assert!(tokens_equal("// first\n// second\n// third"));
}

#[test]
fn complex_program() {
    assert!(tokens_equal(
        "// File header\nfunction greet(name: string) -> nothing {\n  // greeting\n  let msg = `Hello, ${name}!` // inline\n  print(msg)\n}"
    ));
}

#[test]
fn comment_at_end_of_file_no_newline() {
    assert!(tokens_equal("function f() -> nothing {} // trailing"));
}

#[test]
fn doc_comment_multi_line() {
    assert!(tokens_equal(
        "/// Line one\n/// Line two\nfunction f() -> nothing {}"
    ));
}

#[test]
fn no_comments_plain_source() {
    assert!(tokens_equal(
        "function add(a: int, b: int) -> int { return a + b }"
    ));
}

#[test]
fn options_declaration() {
    assert!(tokens_equal("options Status { active\n inactive }"));
}

// ── Error-mode identity tests (5 required by plan) ──────────────────────────────

#[test]
fn error_unterminated_backtick_at_eof() {
    // Unterminated backtick string: both paths must produce identical tokens
    let source = "let x = `hello";
    let (plain, plain_diags) = lex(FILE, source);
    let (trivia, _comments) = lex_with_trivia(FILE, source);
    assert_eq!(
        plain, trivia,
        "token streams must match on unterminated backtick"
    );
    assert!(
        !plain_diags.is_empty(),
        "plain lex must emit diagnostic for unterminated backtick"
    );
}

#[test]
fn error_mid_comment_eof() {
    // Comment at EOF with no trailing newline: valid in both paths
    let source = "let x = 1\n// hello";
    assert!(tokens_equal(source));
}

#[test]
fn error_empty_input() {
    // Zero bytes: only an Eof token
    let (plain, _) = lex(FILE, "");
    let (trivia, comments) = lex_with_trivia(FILE, "");
    assert_eq!(plain, trivia);
    assert!(comments.is_empty());
    assert_eq!(
        plain.len(),
        1,
        "empty input should produce exactly one Eof token"
    );
}

#[test]
fn error_bom_only() {
    // BOM-only input: three bytes, no real content
    let source = "\u{FEFF}";
    let (plain, _) = lex(FILE, source);
    let (trivia, comments) = lex_with_trivia(FILE, source);
    assert_eq!(plain, trivia);
    assert!(
        comments.is_empty(),
        "BOM-only input should produce no comments"
    );
}

// ── Comment capture correctness tests ────────────────────────────────────────────

#[test]
fn captures_single_line_comment() {
    let source = "// hello\nfunction f() -> nothing {}";
    let (_tokens, comments) = lex_with_trivia(FILE, source);
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "// hello");
    assert_eq!(comments[0].kind, CommentKind::LineComment);
    assert_eq!(comments[0].span.start, 0);
}

#[test]
fn captures_doc_comment() {
    let source = "/// my doc\nfunction f() -> nothing {}";
    let (_tokens, comments) = lex_with_trivia(FILE, source);
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].kind, CommentKind::DocComment);
    assert!(
        comments[0].text.starts_with("///"),
        "doc comment text must start with ///"
    );
}

#[test]
fn captures_multiple_comments() {
    let source = "// a\n// b\nfunction f() -> nothing {}";
    let (_tokens, comments) = lex_with_trivia(FILE, source);
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].text, "// a");
    assert_eq!(comments[1].text, "// b");
}

#[test]
fn comment_spans_are_correct() {
    let source = "function f() -> nothing { } // inline";
    let (_tokens, comments) = lex_with_trivia(FILE, source);
    assert_eq!(comments.len(), 1);
    let c = &comments[0];
    // The comment starts at the `//` position
    assert_eq!(&source[c.span.start..c.span.end], "// inline");
}

#[test]
fn inline_comment_does_not_change_tokens() {
    // Inline comment after a statement: token stream must match plain lex
    let source = "function f() -> int { return 42 // the answer\n}";
    let (plain, _) = lex(FILE, source);
    let (trivia, comments) = lex_with_trivia(FILE, source);
    assert_eq!(plain, trivia);
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "// the answer");
}
