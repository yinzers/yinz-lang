use ynz_diagnostics::SourceSpan;

/// A single `//` or `///` comment captured by [`lex_with_trivia`].
///
/// [`lex_with_trivia`]: super::lex_with_trivia
#[derive(Clone, Debug, PartialEq)]
pub struct Comment {
    pub kind: CommentKind,
    /// Full comment text including the `//` or `///` prefix, NOT including the trailing `\n`.
    pub text: String,
    pub span: SourceSpan,
}

/// Whether the comment is a regular line comment or a doc-comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommentKind {
    /// `//` — a regular line comment.
    LineComment,
    /// `///` — a doc-comment. Note: doc-comments also appear in the token stream as
    /// `Token::DocComment`; the trivia capture captures them here too for span/text access.
    DocComment,
}
