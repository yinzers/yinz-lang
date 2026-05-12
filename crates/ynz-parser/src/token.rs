use ynz_diagnostics::SourceSpan;

/// A token with its source position attached.
#[derive(Clone, Debug, PartialEq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: SourceSpan,
}

impl<T> Spanned<T> {
    pub fn new(value: T, file: &str, start: usize, end: usize) -> Self {
        Self {
            value,
            span: SourceSpan::new(file, start, end),
        }
    }
}

/// The complete token vocabulary for M1.
///
/// Variant count is pinned by `m1_token_variant_count_locked` in the test suite.
/// Adding a variant requires both an inline `// test-ratchet: <reason>` marker
/// on that test AND updating this comment with the new count.
///
/// Current M1 count: 10 (Function, Nothing, Identifier, StringLit, LParen,
/// RParen, LBrace, RBrace, Arrow, Eof)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    /// The `function` keyword.
    Function,
    /// The `nothing` return type keyword.
    Nothing,
    /// An identifier: a letter or underscore followed by letters, digits, or underscores.
    Identifier(String),
    /// A string literal: the raw UTF-8 bytes between the quotes, unescaped.
    StringLit(Vec<u8>),
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `->`
    Arrow,
    /// End of file.
    Eof,
}

impl Token {
    pub fn is_eof(&self) -> bool {
        matches!(self, Token::Eof)
    }
}
