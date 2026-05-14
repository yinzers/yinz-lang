use ynz_diagnostics::SourceSpan;

/// A top-level module — the root of the AST for a single source file.
#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub items: Vec<Item>,
    pub span: SourceSpan,
}

/// A top-level item in a module.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Function(FunctionDecl),
}

/// A function declaration: `function name(params) -> return_type { body }`.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub body: Block,
    pub span: SourceSpan,
    /// Span of the function name identifier.
    pub name_span: SourceSpan,
}

/// A single function parameter: `name: Type`.
///
/// No ownership annotations in M3 — those arrive in M4 (`share`, `lend`, `give`).
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: String,
    pub name_span: SourceSpan,
    pub ty: Type,
    pub ty_span: SourceSpan,
    pub span: SourceSpan,
}

/// A block of statements surrounded by `{` ... `}`.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: SourceSpan,
}

/// A single statement.
///
/// Variant count is pinned by `m3_stmt_variant_count_locked` in the test suite.
/// Current count: 8.
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    /// A bare expression used as a statement (e.g. a function call).
    Expr(Expr),

    /// A `let` or `const` variable binding: `let name [: type] = value`.
    ///
    /// `is_const: true` means the binding was declared with `const`.
    /// Typeck enforces the "no reassign" rule for const bindings.
    Let {
        is_const: bool,
        name: String,
        name_span: SourceSpan,
        ty: Option<Type>,
        value: Expr,
        span: SourceSpan,
    },

    /// A variable reassignment: `name = value`.
    ///
    /// Typeck enforces that `name` is a `let`-bound variable (not `const`).
    Assign {
        target: String,
        target_span: SourceSpan,
        value: Expr,
        span: SourceSpan,
    },

    /// A simple `if (cond) { body }` — no else clause.
    ///
    /// The Yinz spec has no standalone `else { }` block. Alternation is expressed
    /// via early return (pattern 1) or pre-assignment (pattern 2). For value-based
    /// branching, use the `Stmt::Match` multi-case form with `else =>` catch-all.
    If {
        cond: Expr,
        body: Block,
        span: SourceSpan,
    },

    /// A multi-case `if (scrutinee) { arms }` with optional `else =>` catch-all.
    ///
    /// Distinct from `Stmt::If` to prevent the malformed state where both a body
    /// and arms are populated simultaneously.
    Match {
        scrutinee: Expr,
        arms: Vec<MatchArm>,
        else_arm: Option<Block>,
        span: SourceSpan,
    },

    /// A `while (cond) { body }` loop.
    While {
        cond: Expr,
        body: Block,
        span: SourceSpan,
    },

    /// A `for (var in iter) { body }` loop.
    ///
    /// In M3, `iter` must be a `range(...)` call — enforced by typeck (P3).
    /// Parser accepts any expression so M7's `Iterable[T]` protocol requires
    /// only a typeck change, not a parser change.
    For {
        var: String,
        var_span: SourceSpan,
        iter: Expr,
        body: Block,
        span: SourceSpan,
    },

    /// An early `return [value]`.
    Return {
        value: Option<Expr>,
        span: SourceSpan,
    },
}

/// A single multi-case arm: `pattern => { body }`.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub body: Block,
    pub arrow_span: SourceSpan,
}

/// The pattern in a multi-case arm.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchPattern {
    pub kind: MatchPatternKind,
    pub span: SourceSpan,
}

/// The kind of a multi-case arm pattern.
///
/// Variant count is pinned by `m3_match_pattern_kind_variant_count` in the test suite.
/// Current count: 3.
#[derive(Clone, Debug, PartialEq)]
pub enum MatchPatternKind {
    /// A value literal or expression: `1 => ...`, `"hello" => ...`.
    Value(Expr),
    /// `is TypeName =>` — type-narrowing form, deferred to M6.
    // REPLACE-AT M6: widen String to TypePath for narrowing
    IsType(String),
    /// `variant_name =>` — options-variant form, deferred to M6.
    // REPLACE-AT M6: widen String to VariantPath for options exhaustiveness
    Variant(String),
}

/// Binary operator kinds.
///
/// Variant count is pinned by `m2_binopkind_variant_count_locked` in the test suite.
/// Current count: 18.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BinOpKind {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    // Comparison
    Lt,
    LtEq,
    Gt,
    GtEq,
    EqEq,
    NotEq,
    // Boolean
    And,
    Or,
    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// Unary operator kinds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnaryOpKind {
    /// Arithmetic negation: `-x`
    Neg,
    /// Boolean NOT: `!x`
    Not,
    /// Bitwise NOT: `~x`
    BitNot,
}

/// An expression.
///
/// Variant count is pinned by `m2_expr_variant_count_locked` in the test suite.
/// Current count: 10.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {

    /// A function or identifier name.
    Ident(String, SourceSpan),
    /// A string literal (raw UTF-8 bytes).
    StringLit(Vec<u8>, SourceSpan),
    /// A function call: `callee(arg1, arg2)`.
    Call(Box<CallExpr>),
    /// A placeholder inserted by the parser when it recovers from an error.
    /// The type checker skips functions whose bodies contain Error nodes.
    Error(SourceSpan),


    /// An integer literal: `42`, `0xFF`, `0b1010`.
    IntLit(i64, SourceSpan),
    /// A decimal number literal (underscores stripped): `3.14`, `1e5`, `2.5e-3`.
    NumberLit(String, SourceSpan),
    /// A boolean literal: `true` or `false`.
    BoolLit(bool, SourceSpan),
    /// A binary operator expression: `lhs op rhs`.
    BinOp {
        op: BinOpKind,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: SourceSpan,
    },
    /// A unary prefix expression: `-x`, `!x`, `~x`.
    UnaryOp {
        op: UnaryOpKind,
        operand: Box<Expr>,
        span: SourceSpan,
    },
    /// A method call: `receiver.method(args)`.
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        method_span: SourceSpan,
        args: Vec<Expr>,
        span: SourceSpan,
    },
}

impl Expr {
    pub fn span(&self) -> &SourceSpan {
        match self {
            Expr::Ident(_, s)
            | Expr::StringLit(_, s)
            | Expr::Error(s)
            | Expr::IntLit(_, s)
            | Expr::NumberLit(_, s)
            | Expr::BoolLit(_, s) => s,
            Expr::Call(c) => &c.span,
            Expr::BinOp { span, .. }
            | Expr::UnaryOp { span, .. }
            | Expr::MethodCall { span, .. } => span,
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Expr::Error(_))
    }
}

/// A call expression: `callee(args)`.
#[derive(Clone, Debug, PartialEq)]
pub struct CallExpr {
    pub callee: Expr,
    pub args: Vec<Expr>,
    pub span: SourceSpan,
}

/// A type annotation.
///
/// Variant count is pinned by `m3_type_variant_count_locked` in the test suite.
/// Current count: 8.
#[derive(Clone, Debug, PartialEq)]
pub enum Type {

    /// The `nothing` return type.
    Nothing,
    /// A named type (identifier that names a user-defined type).
    Named(String, SourceSpan),
    /// A placeholder inserted during error recovery.
    Error,


    /// The `int` primitive type (signed 64-bit integer).
    Int,
    /// The `float` primitive type (IEEE 754 binary64).
    Float,
    /// The `number` or `number[N]` primitive type (IEEE 754 decimal128 with N digits of precision).
    ///
    /// `precision: 34` is the default. Values > 34 are deferred to M8 (bignum).
    /// The parser emits a deferral diagnostic for any N != 34.
    Number { precision: u32 },
    /// The `bool` primitive type.
    Bool,

    /// The internal type of a `range(...)` expression.
    ///
    /// Never written by users in M3 — only produced by typeck when it checks
    /// the iterable position of a `for` loop. Using `Range` in any other position
    /// (let binding, parameter, return type) is a typeck error pointing to M7.
    ///
    /// REPLACE-AT M7: remove this variant and replace with `Iterable[T]` protocol dispatch.
    Range {
        /// Always `Int` in M3 — `range(...)` only produces integer ranges.
        element: Box<Type>,
        /// Always `false` in M3 — `range(end)` and `range(start, end)` are end-exclusive.
        end_inclusive: bool,
    },
}
