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
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    /// A bare expression used as a statement (e.g. a function call).
    Expr(Expr),
}

/// An expression.
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
}

impl Expr {
    pub fn span(&self) -> &SourceSpan {
        match self {
            Expr::Ident(_, s) | Expr::StringLit(_, s) | Expr::Error(s) => s,
            Expr::Call(c) => &c.span,
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
#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    /// The `nothing` return type.
    Nothing,
    /// A named type (identifier that names a type).
    Named(String, SourceSpan),
    /// A placeholder inserted during error recovery.
    Error,
}
