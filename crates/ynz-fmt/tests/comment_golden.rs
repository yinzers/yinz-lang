//! Golden tests for comment-aware formatting.
//!
//! The semantic round-trip sweep ignores comments (`ast_eq_modulo_trivia`), so these
//! goldens are the only guard that the formatter keeps every user comment: not deleted,
//! not duplicated, not moved to another declaration, not mistaken for string content.
//!
//! One generated table, one named test per fixture: read the `.ynz` fixture, format it,
//! compare against its `.formatted` golden, and check the output is idempotent. Fixtures
//! whose only claim is layout live in the sweeps' corpus without a golden.

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

macro_rules! comment_golden_cases {
    ($($case:ident: $fixture:expr;)+) => {$(
        // bug: the formatter deletes, duplicates, moves or re-lexes a user comment, silently losing or corrupting the user's text | tier: High
        #[test]
        fn $case() {
            golden($fixture);
        }
    )+};
}

comment_golden_cases! {
    comment_between_decls: "comment_between_decls";
    comment_inside_block: "comment_inside_block";
    inline_comment: "inline_comment";
    doc_comment: "doc_comment";
    // WHY: a function body with only // comments and no statements must not be
    // collapsed to `{ }` — that silently deletes the user's planning notes.
    comments_only_body_preserved: "comments_only_body";
    // WHY: F9 (2026-07-11 non-concurrency audit) — a `//` comment block following a union-alias
    // declaration (`shape U = A | B`) was silently DELETED by the formatter. Root cause: the
    // parser's union-alias arm in `parse_shape_decl` computed `ShapeDecl.span.end` from the START
    // of the next unconsumed token instead of the END of the last consumed one, over-extending the
    // declaration's span to swallow a trailing comment before the formatter's comment-merge pass
    // ever saw it as available. Locks the fix and round-trip idempotency together.
    union_alias_trailing_comment: "union_alias_trailing_comment";
    floating_comment: "edge/floating_comment";
    inline_on_split_expr: "edge/inline_on_split_expr";
    backtick_with_slashes: "edge/backtick_with_slashes";
    mixed_doc_and_line: "edge/mixed_doc_and_line";
    doc_comment_split: "edge/doc_comment_split";
    comments_only: "edge/comments_only";
    comment_with_tokens: "edge/comment_with_tokens";
    crlf_input: "edge/crlf_input";
    inline_comment_eof: "edge/inline_comment_eof";
}
