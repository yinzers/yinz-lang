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
    pub return_type: Type,
    pub body: Block,
    pub span: SourceSpan,
    /// Span of the function name identifier.
    pub name_span: SourceSpan,
}

/// A block of statements surrounded by `{` ... `}`.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: SourceSpan,
}

/// A single statement.
///
/// Variant count is pinned by `m2_stmt_variant_count_locked` in the test suite.
/// M1 count: 1 (Expr). M2 adds 2 (Let, Assign). Current M2 count: 3.
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
}

/// Binary operator kinds.
///
/// Variant count is pinned by `m2_binopkind_variant_count_locked` in the test suite.
/// M2 count: 18 (5 arithmetic + 6 comparison + 2 boolean + 5 bitwise).
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
/// M1 count: 4 (Ident, StringLit, Call, Error).
/// M2 adds 6 (IntLit, NumberLit, BoolLit, BinOp, UnaryOp, MethodCall).
/// Current M2 count: 10.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    // ─── M1 variants ─────────────────────────────────────────────────────────

    /// A function or identifier name.
    Ident(String, SourceSpan),
    /// A string literal (raw UTF-8 bytes).
    StringLit(Vec<u8>, SourceSpan),
    /// A function call: `callee(arg1, arg2)`.
    Call(Box<CallExpr>),
    /// A placeholder inserted by the parser when it recovers from an error.
    /// The type checker skips functions whose bodies contain Error nodes.
    Error(SourceSpan),

    // ─── M2 variants ─────────────────────────────────────────────────────────

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
/// Variant count is pinned by `m2_type_variant_count_locked` in the test suite.
/// M1 count: 3 (Nothing, Named, Error). M2 adds 4 (Int, Float, Number, Bool).
/// Current M2 count: 7.
#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    // ─── M1 variants ─────────────────────────────────────────────────────────

    /// The `nothing` return type.
    Nothing,
    /// A named type (identifier that names a user-defined type).
    Named(String, SourceSpan),
    /// A placeholder inserted during error recovery.
    Error,

    // ─── M2 variants ─────────────────────────────────────────────────────────

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
}
