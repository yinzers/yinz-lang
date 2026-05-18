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

/// The complete token vocabulary.
///
/// Variant count is pinned by the milestone-locked tests in the test suite.
/// Adding a variant requires both an inline `// test-ratchet: <reason>` marker
/// on that test AND updating this comment with the new count.
///
/// Current count: 60 (M6 P1 added `Options` and `Is` keywords)
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


    /// The `let` keyword.
    Let,
    /// The `const` keyword.
    Const,
    /// The `true` boolean literal keyword.
    True,
    /// The `false` boolean literal keyword.
    False,


    /// An integer literal parsed to i64.
    ///
    /// Accepts decimal (`42`, `1_000`), hex (`0x2A`, `0xDEAD_BEEF`),
    /// and binary (`0b1010`, `0b1111_0000`) forms.
    /// Overflow at lex time produces a three-part diagnostic; no token is emitted.
    IntLit(i64),

    /// A decimal number literal, stored as source text with underscores stripped.
    ///
    /// Any literal that contains `.` or `e`/`E` becomes a `NumberLit`
    /// (e.g. `3.14`, `1.0`, `1e5`, `2.5e-3`). The `float` type is applied
    /// by typeck via annotation context — the lexer does not distinguish
    /// float from number at this stage.
    NumberLit(String),


    /// `+`
    Plus,
    /// `-` (binary subtract or unary negate — parser distinguishes by context)
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,


    /// `==`
    EqEq,
    /// `!=`
    NotEq,
    /// `<`
    Lt,
    /// `<=`
    LtEq,
    /// `>`
    Gt,
    /// `>=`
    GtEq,


    /// `&&`
    AmpAmp,
    /// `||`
    PipePipe,
    /// `!`
    Bang,


    /// `&` (bitwise AND — distinguished from `&&` by token kind)
    Amp,
    /// `|` (bitwise OR — distinguished from `||` by token kind)
    Pipe,
    /// `^`
    Caret,
    /// `~`
    Tilde,
    /// `<<`
    LtLt,
    /// `>>`
    GtGt,


    /// `=` (variable binding and type annotation separator)
    Eq,
    /// `:`
    Colon,


    /// `.` (method-call receiver separator, e.g. `x.toString()`)
    Dot,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `,` (argument separator in calls)
    Comma,


    /// The `if` keyword.
    If,
    /// The `else` keyword (used in multi-case `if` arms as `else =>`).
    Else,
    /// The `while` keyword.
    While,
    /// The `for` keyword.
    For,
    /// The `in` keyword (for-loop separator: `for (x in collection)`).
    In,
    /// The `return` keyword.
    Return,
    /// `=>` (fat arrow — multi-case arm separator in `if (value) { 1 => ... }`).
    FatArrow,


    // ── M4: shapes, inheritance, contracts, dynamic dispatch ────────────────

    /// The `shape` keyword — declares a data type.
    Shape,
    /// The `follows` keyword — declares that a shape satisfies a contract.
    Follows,
    /// The `extends` keyword — data-only inheritance (child inherits parent fields).
    Extends,
    /// The `base` keyword — marks a shape that cannot be instantiated directly.
    Base,
    /// The `hidden` keyword — marks a field visible only to the declaring shape's functions.
    Hidden,
    /// The `dynamic` keyword — opt-in runtime polymorphism via fat pointer + vtable.
    Dynamic,
    /// The `Self` keyword (capital S) — the concrete type of the enclosing shape (type position).
    SelfType,
    /// The `self` keyword (lowercase) — the instance value of the enclosing shape (value position).
    SelfValue,

    // ── M5: generics + maybe + none ───────────────────────────────────────────

    /// The `none` keyword — the absent value of `maybe<T>`.
    ///
    /// `none` is only valid where typeck can resolve the type parameter T from
    /// surrounding context (annotation, return type, parameter type, sibling
    /// branch). See `design/maybe.md` for the full none-inference rules.
    None,

    // ── M6: options + unions + narrowing ──────────────────────────────────────

    /// The `options` keyword — declares a finite set of named values.
    ///
    /// Replaces `enum` from other languages. `enum` is a banned keyword with a
    /// redirecting diagnostic pointing here.
    Options,

    /// The `is` keyword — type-narrowing discriminator in union `if`-arm context.
    ///
    /// Used in two positions:
    ///   1. As a multi-case arm prefix: `if (x) { is Circle => ... }`
    ///   2. As a condition in a regular `if`: `if (x is Circle) { ... }`
    ///
    /// `is` looks up the type name in the types-only namespace (same-name value
    /// bindings are not shadowed). See `design/narrowing.md`.
    Is,
}

impl Token {
    pub fn is_eof(&self) -> bool {
        matches!(self, Token::Eof)
    }
}
