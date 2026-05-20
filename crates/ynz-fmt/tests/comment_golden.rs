//! Golden tests for comment-aware formatting.
//!
//! Each test reads a .ynz fixture, formats it, and compares against the .formatted golden.

use std::path::Path;

fn golden(subpath: &str) {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/comments");
    let input_path = base.join(format!("{subpath}.ynz"));
    let expected_path = base.join(format!("{subpath}.formatted"));

    let source = std::fs::read_to_string(&input_path)
        .unwrap_or_else(|_| panic!("fixture not found: {}", input_path.display()));
    let expected = std::fs::read_to_string(&expected_path)
        .unwrap_or_else(|_| panic!("golden not found: {}", expected_path.display()));

    let got = ynz_fmt::format(&source).unwrap_or_else(|e| panic!("format error on {subpath}: {e}"));
    assert_eq!(got, expected, "comment golden mismatch for {subpath}");

    // Idempotency check
    let got2 = ynz_fmt::format(&got).expect("format idempotency second pass failed");
    assert_eq!(got, got2, "formatter is not idempotent on {subpath}");
}

// ── Basic 7 comment scenarios ─────────────────────────────────────────────────

#[test]
fn leading_comment() {
    golden("leading_comment");
}

#[test]
fn inline_comment() {
    golden("inline_comment");
}

#[test]
fn comment_block() {
    golden("comment_block");
}

#[test]
fn comment_between_decls() {
    golden("comment_between_decls");
}

#[test]
fn comment_inside_block() {
    golden("comment_inside_block");
}

#[test]
fn doc_comment() {
    golden("doc_comment");
}

#[test]
fn mixed_comments() {
    golden("mixed_comments");
}

// ── 16 locked comment-merge-spec edge cases ───────────────────────────────────

#[test]
fn floating_comment() {
    golden("edge/floating_comment");
}

#[test]
fn inline_on_split_expr() {
    golden("edge/inline_on_split_expr");
}

#[test]
fn backtick_with_slashes() {
    golden("edge/backtick_with_slashes");
}

#[test]
fn comment_in_array() {
    golden("edge/comment_in_array");
}

#[test]
fn comment_in_map() {
    golden("edge/comment_in_map");
}

#[test]
fn mixed_doc_and_line() {
    golden("edge/mixed_doc_and_line");
}

#[test]
fn doc_comment_split() {
    golden("edge/doc_comment_split");
}

#[test]
fn comments_only() {
    golden("edge/comments_only");
}

#[test]
fn comment_with_tokens() {
    golden("edge/comment_with_tokens");
}

#[test]
fn crlf_input() {
    golden("edge/crlf_input");
}

#[test]
fn no_trailing_newline() {
    golden("edge/no_trailing_newline");
}

#[test]
fn empty_function_body() {
    golden("edge/empty_function_body");
}

#[test]
fn long_backtick_string() {
    golden("edge/long_backtick_string");
}

#[test]
fn inline_comment_eof() {
    golden("edge/inline_comment_eof");
}

#[test]
fn two_inline_on_split() {
    golden("edge/two_inline_on_split");
}

#[test]
fn long_identifier() {
    golden("edge/long_identifier");
}

// ── Behavioral invariants ─────────────────────────────────────────────────────

#[test]
fn inline_comment_has_two_spaces_before_slashes() {
    let source = "function f() -> nothing { let x = 1 // note\n }\n";
    let got = ynz_fmt::format(source).expect("format failed");
    // The inline comment should have exactly 2 spaces between code and //
    assert!(
        got.contains("  // note"),
        "inline comment should have 2 spaces prefix: {got}"
    );
}

#[test]
fn crlf_input_becomes_lf_output() {
    let source = "function f() -> nothing {\r\n  let x = 1\r\n}\r\n";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(
        !got.contains('\r'),
        "CRLF input must produce LF-only output"
    );
}

#[test]
fn comments_only_file_preserved() {
    let source = "// just a comment\n// and another\n";
    let got = ynz_fmt::format(source).expect("format failed");
    assert!(
        got.contains("// just a comment"),
        "standalone comment must be preserved"
    );
    assert!(
        got.contains("// and another"),
        "standalone comment must be preserved"
    );
}

#[test]
fn doc_comment_not_double_emitted() {
    // WHY: doc comments are captured both in the AST (FunctionDecl.doc) AND in the trivia
    //      vec. If both paths emit the comment, it appears twice. This test guards the
    //      invariant that only the trivia path emits in the comment-aware walker — the AST
    //      field is ignored by emit_function_with_comments.
    let source = "/// My doc comment\nfunction f() -> nothing { print(`ok`) }\n";
    let got = ynz_fmt::format(source).expect("format failed");
    let count = got.matches("/// My doc comment").count();
    assert_eq!(count, 1, "doc comment must appear exactly once, got: {got}");
}
