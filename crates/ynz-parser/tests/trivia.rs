//! `lex_with_trivia` must produce exactly the token stream `lex` produces, plus a
//! parallel list of every `//` and `///` comment. The formatter merges those comments
//! back into the output by span, so a token that differs between the two paths, a
//! missed comment, or a comment span that does not slice back to its own text would
//! move or drop user comments on format.
//!
//! One generated table, one named test per case so nextest runs them in parallel and
//! names the failing case. Each case states the comments it expects as
//! `(kind, text)` pairs in source order.

use ynz_parser::{lex_with_trivia, lexer::lex, CommentKind};

const FILE: &str = "<trivia_test>";

const LINE: CommentKind = CommentKind::LineComment;
const DOC: CommentKind = CommentKind::DocComment;

/// One case per source: both lexing paths agree on every token and span, the captured
/// comments are exactly the expected `(kind, text)` pairs, and each comment's span
/// slices back to its text.
macro_rules! trivia_cases {
    ($($case:ident: $source:expr => [$(($kind:expr, $text:expr)),* $(,)?];)+) => {$(
        // bug: lex_with_trivia changes the token stream, drops or invents a comment, or gives a comment a span that does not slice back to its text | tier: High
        #[test]
        fn $case() {
            let source: &str = $source;
            let (plain_tokens, _plain_diagnostics) = lex(FILE, source);
            let (trivia_tokens, comments) = lex_with_trivia(FILE, source);
            assert_eq!(
                plain_tokens, trivia_tokens,
                "lex_with_trivia must produce the same tokens as lex"
            );
            let expected: Vec<(CommentKind, &str)> = vec![$(($kind, $text)),*];
            let captured: Vec<(CommentKind, &str)> = comments
                .iter()
                .map(|comment| (comment.kind.clone(), comment.text.as_str()))
                .collect();
            assert_eq!(captured, expected);
            for comment in &comments {
                assert_eq!(
                    &source[comment.span.start..comment.span.end],
                    comment.text,
                    "comment span must slice back to the comment text"
                );
            }
        }
    )+};
}

trivia_cases! {
    empty_source: "" => [];
    whitespace_only: "   \n\t  " => [];
    bom_only: "\u{FEFF}" => [];
    plain_source_without_comments:
        "function add(a: Int, b: Int) -> Int { return a + b }" => [];
    line_comment_before_declaration:
        "// comment\nfunction foo() -> Nothing {}" => [(LINE, "// comment")];
    inline_comment_after_statement:
        "function f() -> Nothing { let x = 1 // inline\n }" => [(LINE, "// inline")];
    multiple_line_comments:
        "// a\n// b\n// c\nfunction f() -> Nothing {}" => [(LINE, "// a"), (LINE, "// b"), (LINE, "// c")];
    doc_and_line_comment_mixed:
        "/// doc\n// note\nfunction f() -> Nothing {}" => [(DOC, "/// doc"), (LINE, "// note")];
    multi_line_doc_comment:
        "/// Line one\n/// Line two\nfunction f() -> Nothing {}" => [(DOC, "/// Line one"), (DOC, "/// Line two")];
    doc_comment_at_eof_without_newline: "/// doc line" => [(DOC, "/// doc line")];
    trailing_comment_at_eof_without_newline:
        "function f() -> Nothing {} // trailing" => [(LINE, "// trailing")];
    backtick_string_with_interpolation:
        "function f(name: String) -> Nothing { let s = `Hello ${name}!` }" => [];
    double_slash_inside_backtick_is_not_a_comment:
        "function f() -> Nothing { let s = `http://example.com` }" => [];
    unterminated_backtick_at_eof: "let x = `hello" => [];
}
