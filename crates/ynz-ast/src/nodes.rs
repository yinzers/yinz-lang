use ynz_diagnostics::SourceSpan;

/// A top-level module — the root of the AST for a single source file.
#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub items: Vec<Item>,
    pub span: SourceSpan,
}

/// A top-level item in a module.
///
/// Variant count is pinned by `m4_item_variant_count_locked` in the test suite.
/// Current count: 6 (M8 P2 adds ImportDecl, ConstDecl, ReExport).
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Function(FunctionDecl),
    // test-ratchet: M4 adds ShapeDecl for user-defined data types.
    ShapeDecl(ShapeDecl),
    // test-ratchet: M6 adds OptionsDecl for `options` type declarations.
    OptionsDecl(OptionsDecl),
    // test-ratchet: M8 P2 adds ImportDecl for `import { ... } from "..."` and `import ns from "..."`.
    ImportDecl(ImportDecl),
    // test-ratchet: M8 P2 adds ConstDecl for top-level `[export] const NAME = value`.
    ConstDecl(ConstDecl),
    // test-ratchet: M8 P2 adds ReExport for `export { foo } from "services/users"`.
    ReExport(ReExport),
}

/// How an import binds names into scope.
#[derive(Clone, Debug, PartialEq)]
pub enum ImportKind {
    /// `import { name1, name2 as alias } from "path"` — named import.
    Named(Vec<ImportItem>),
    /// `import ns from "path"` or `import ns as alias from "path"` — namespace import.
    Namespace {
        local_name: String,
        local_name_span: SourceSpan,
    },
}

/// A single item in a named import: `name` or `name as alias`.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportItem {
    /// The name as exported from the source module.
    pub exported_name: String,
    pub exported_name_span: SourceSpan,
    /// The local binding name (same as `exported_name` unless `as alias` is used).
    pub local_name: String,
    pub local_name_span: SourceSpan,
}

/// `import { ... } from "path"` or `import ns from "path"`.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportDecl {
    pub kind: ImportKind,
    /// Project-root-relative source path (no `.ynz` suffix).
    pub source: String,
    pub source_span: SourceSpan,
    pub span: SourceSpan,
}

/// Top-level `[export] const NAME: [Type] = value`.
///
/// M8 introduces top-level const declarations so modules can export named constants.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstDecl {
    pub name: String,
    pub name_span: SourceSpan,
    pub ty: Option<Type>,
    pub value: Expr,
    pub is_exported: bool,
    pub span: SourceSpan,
    /// Doc comment attached immediately before this declaration (`/// ...`).
    /// `None` = no doc comment.
    pub doc: Option<String>,
}

/// A single item in a re-export: `name` or `name as alias`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReExportItem {
    pub name: String,
    pub name_span: SourceSpan,
    pub alias: Option<(String, SourceSpan)>,
}

/// `export { foo, bar as baz } from "services/users"`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReExport {
    pub items: Vec<ReExportItem>,
    /// Project-root-relative source path.
    pub source: String,
    pub source_span: SourceSpan,
    pub span: SourceSpan,
}

/// An `options` type declaration: `options Status { active, inactive, banned }`.
///
/// Lowers to an `i8` tag at codegen time. Variants are assigned sequential
/// integer tags (0, 1, 2, …) in declaration order.
/// Typeck rejects: empty variants, single variant, ≥256 variants, duplicate names.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionsDecl {
    pub name: String,
    pub name_span: SourceSpan,
    /// Each variant: (name, name_span, display_string).
    /// `display_string` is the optional `name: \`Display Value\`` form.
    /// When absent, `.toString()` returns the variant name as-is.
    pub variants: Vec<(String, SourceSpan, Option<String>)>,
    pub span: SourceSpan,
    /// `true` when prefixed with `export` — visible to other modules.
    pub is_exported: bool,
    /// M8 P3: doc comment attached to this options declaration.
    pub doc: Option<String>,
}

/// A function declaration: `function name<generics>(params) -> return_type [errors] { body }`.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    /// M5: type parameters declared on the function (`function foo<T, U>(...)`).
    /// Empty Vec when the function is not generic. Populated by the parser in P2.
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub body: Block,
    pub span: SourceSpan,
    /// Span of the function name identifier.
    pub name_span: SourceSpan,
    /// M7: true when the return type is qualified with `errors`
    /// (e.g. `-> string errors`). Full error-type typeck arrives in M7 P3a.
    pub errors_capable: bool,
    /// M8: true when prefixed with `export` — visible to other modules.
    pub is_exported: bool,
    /// M8 P3: doc comment attached to this function (`/// ...` immediately before).
    /// `None` = no doc comment. Preserved through to AST for `ynz doc` (v1.1).
    pub doc: Option<String>,
}

/// M5: a type parameter on a generic function or generic shape.
///
/// Example: `<T follows Comparable>` parses to a single `GenericParam`
/// with `name = "T"` and `constraints = [("Comparable", span)]`.
/// Multi-constraint form `<T follows A, B>` produces `constraints` with two entries.
#[derive(Clone, Debug, PartialEq)]
pub struct GenericParam {
    pub name: String,
    pub name_span: SourceSpan,
    /// Zero or more `follows` constraints — each is the named contract shape's identifier.
    pub constraints: Vec<(String, SourceSpan)>,
    pub span: SourceSpan,
}

/// Ownership modifier on a function parameter signature.
///
/// `share self` / `lend self` / `give self` in contract method signatures (required);
/// optional on free function signatures (inferred from body usage when omitted).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnershipModifier {
    Share,
    Lend,
    Give,
}

/// A single function parameter: `[share|lend|give] name: Type`.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: String,
    pub name_span: SourceSpan,
    /// Explicit ownership modifier — `None` for free functions (inferred by typeck from body).
    pub ownership: Option<OwnershipModifier>,
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

/// One binding in a for-loop shape-destructuring pattern.
///
/// Represents `{ field }` or `{ field as alias }` in `for ({ field } in iter)`.
/// The formatter uses this to reconstruct the original syntax from the desugared AST.
#[derive(Debug, Clone, PartialEq)]
pub struct ForDestructureBinding {
    /// The field name on the shape being iterated.
    pub field: String,
    /// A renamed local variable, e.g. `Some("mins")` for `{ minutes as mins }`.
    /// `None` means the local variable has the same name as the field.
    pub alias: Option<String>,
}

/// A single statement.
///
/// Variant count is pinned by the milestone-locked tests in the test suite.
/// Current count: 10 — M4 added FieldAssign(9), M5 added IndexAssign(10).
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
    ///
    /// `destructure_pattern` is `Some(bindings)` when the source used shape
    /// destructuring (`for ({ field, field as alias } in iter)`).  The desugared
    /// `var` + `body` fields are still populated for typeck/codegen; the formatter
    /// uses `destructure_pattern` to reconstruct the original surface syntax.
    For {
        var: String,
        var_span: SourceSpan,
        iter: Expr,
        body: Block,
        span: SourceSpan,
        /// Present when the loop used `for ({ field, ... } in iter)` shape
        /// destructuring.  `None` for plain `for (x in iter)` and map-entry loops.
        destructure_pattern: Option<Vec<ForDestructureBinding>>,
        /// Present when the loop used `for ((key, value) in map)` map-entry
        /// destructuring.  `None` for plain and shape-destructuring loops.
        /// Tuple is `(key_name, val_name)` as written in source.
        map_destructure_pattern: Option<(String, String)>,
    },

    /// An early `return [value]`.
    Return {
        value: Option<Expr>,
        span: SourceSpan,
    },

    // ── M4 ───────────────────────────────────────────────────────────────────

    // test-ratchet: M4 adds FieldAssign for `receiver.field = value`.
    /// Field assignment: `receiver.field = value`.
    ///
    /// `target` is the full `Expr::FieldAccess` LHS (typeck deconstructs it);
    /// `value` is the RHS expression.
    FieldAssign {
        target: Box<Expr>,
        value: Expr,
        span: SourceSpan,
    },

    // ── M5 ───────────────────────────────────────────────────────────────────

    // test-ratchet: M5 adds IndexAssign for `arr[i] = v` / `m[k] = v`.
    /// Index assignment: `receiver[index] = value`.
    ///
    /// Bracket sugar for `.set(index, value)` on built-in collections.
    /// Typeck desugars this to a method call on the receiver type's `.set()`.
    IndexAssign {
        receiver: Box<Expr>,
        index: Box<Expr>,
        value: Expr,
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

/// A single-segment type path used in `is TypeName =>` arms.
///
/// In M6, union variant names are single identifiers. The `TypePath` wrapper
/// carries the name and its source span for diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct TypePath {
    pub name: String,
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
    /// `is TypeName =>` — type-narrowing form (M6).
    ///
    /// Widened from the M3 stub `IsType(String)`. Typeck resolves the
    /// TypePath against the types-only namespace and validates it is a
    /// variant of the scrutinee's union type.
    Is(TypePath),
    /// `VariantName =>` — bare options-variant form (M6).
    ///
    /// Widened from the M3 stub `Variant(String)`. Typeck resolves the
    /// name against the scrutinee's options type's variant list.
    OptionName(String),
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

// ── M7: interpolated strings ─────────────────────────────────────────────────

/// A segment of a backtick-interpolated string.
///
/// An interpolated string like `` `hello ${name}!` `` produces:
/// `[Lit(b"hello "), Expr(name), Lit(b"!")]`
///
/// The literal segments carry their raw bytes; the expression segments carry
/// an arbitrary sub-expression whose value will be converted to string at runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum StringPart {
    /// A raw byte sequence (static text between interpolations).
    Lit(Vec<u8>, SourceSpan),
    /// An interpolated expression `${...}` whose result is stringified.
    Expr(Box<Expr>, SourceSpan),
}

/// An expression.
///
/// Variant count is pinned by the milestone-locked tests in the test suite.
/// Current count: 19 — M4 added FieldAccess(11), StructLit(12), PostfixOp(13),
/// SelfValue(14); M5 added NoneLit(15), IndexAccess(16), ArrayLit(17), MapLit(18);
/// M6 added Is(19) for `if (x is Foo)` type-narrowing condition.
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

    // ── M4 ───────────────────────────────────────────────────────────────────

    // test-ratchet: M4 adds FieldAccess, StructLit, PostfixOp.
    /// Field access: `receiver.field` (no parens — pure read per dot-postfix rule).
    FieldAccess {
        receiver: Box<Expr>,
        field: String,
        field_span: SourceSpan,
        span: SourceSpan,
    },

    /// Anonymous struct literal: `{ name: "Patrick", health: 100 }`.
    ///
    /// No type-name prefix — the type is resolved from the surrounding annotation
    /// (e.g. `let p: Player = { ... }`) at typeck time.
    StructLit {
        fields: Vec<StructLitField>,
        span: SourceSpan,
    },

    /// Dot-postfix body operation: `value.copy()` or `value.freeze()`.
    ///
    /// Actions (Golden Rule 2 — parens for actions). Only `.copy()` and `.freeze()`
    /// are body-level operations; `.share`/`.lend`/`.give` are inferred at call
    /// sites and have no body syntax.
    PostfixOp {
        receiver: Box<Expr>,
        op: PostfixOpKind,
        span: SourceSpan,
    },

    /// The `self` value keyword (lowercase) — the receiver instance inside a
    /// function whose first parameter is `share self` / `lend self` / `give self`.
    SelfValue { span: SourceSpan },

    // ── M5 ───────────────────────────────────────────────────────────────────

    // test-ratchet: M5 adds NoneLit, IndexAccess.
    /// The `none` literal — the absent value of `maybe<T>` for some T resolved
    /// from context (binding annotation, return type, enclosing call's
    /// parameter type, sibling branch). See `design/maybe.md` for the full
    /// none-inference rules.
    NoneLit { span: SourceSpan },

    /// Bracket-index access: `receiver[index]`.
    ///
    /// Sugar for `.get(index)` on built-in collections (array, fixed, map,
    /// string). Returns `maybe<T>` after typeck-side desugaring.
    IndexAccess {
        receiver: Box<Expr>,
        index: Box<Expr>,
        span: SourceSpan,
    },

    // test-ratchet: M5 P3b adds ArrayLit for `[1, 2, 3]` array/fixed literal syntax.
    /// Array or fixed literal: `[1, 2, 3]`.
    ///
    /// Used for both `array<T>` and `fixed<T>` literals — typeck disambiguates
    /// based on the binding annotation. In `fixed` context, size is locked; in
    /// `array` context, elements are pushed to a heap-allocated array.
    ArrayLit {
        elements: Vec<Expr>,
        span: SourceSpan,
    },

    // test-ratchet: M5 P3c adds MapLit for { "alice": 90, "bob": 85 } map literal syntax.
    /// Map literal: `{ "alice": 90, "bob": 85 }`.
    ///
    /// Keys are expressions (typically string or int literals for static-key maps).
    /// Values are expressions. Typeck validates key/value types against the annotation
    /// and detects duplicate literal keys.
    MapLit {
        entries: Vec<(Expr, Expr)>,
        span: SourceSpan,
    },

    // test-ratchet: M6 adds Is for `expr is TypeName` type-narrowing condition form.
    /// Type-narrowing predicate: `x is Circle`.
    ///
    /// Used in if-condition position: `if (x is Circle) { ... }`.
    /// Inside the then-block, `x` is narrowed to `Circle`. Typeck validates
    /// that the named type is a variant of `x`'s union type.
    /// `is` looks up the type in the types-only namespace — same-name value
    /// bindings do NOT shadow the type lookup.
    Is {
        expr: Box<Expr>,
        ty: TypePath,
        span: SourceSpan,
    },

    // ── M7: interpolated strings ─────────────────────────────────────────────

    // test-ratchet: M7 P1 adds InterpolatedString for backtick string literals with ${...} interpolation.
    /// A backtick-quoted string, potentially with interpolated expressions.
    ///
    /// `` `hello ${name}!` `` → `[Lit("hello "), Expr(name), Lit("!")]`
    ///
    /// A plain string with no interpolations is represented as a single-element
    /// `Vec` containing only a `StringPart::Lit`. Typeck checks that each
    /// interpolated expression's type has a `.toString()` intrinsic.
    /// Full codegen lowering arrives in M7 P2.
    InterpolatedString(Vec<StringPart>, SourceSpan),

    // ── M8: concurrency keywords ─────────────────────────────────────────────

    // test-ratchet: M8 P5 adds Wait and Background for concurrency keyword surface.
    /// `wait expr` — force completion of an I/O or background operation.
    ///
    /// M8 sequential semantics: `wait foo()` compiles identically to `foo()`.
    /// The keyword syntax is locked so v0.3 auto-parallelization is a pure codegen change.
    Wait(Box<Expr>, SourceSpan),

    /// `background expr` — run a function call outside the current function's lifetime.
    ///
    /// M8 sequential semantics: `background foo()` runs `foo()` to completion,
    /// return value is discarded. The ownership rules (no share/lend) are enforced
    /// even in M8 so v0.3 is a codegen-only change.
    Background(Box<Expr>, SourceSpan),
}

/// The kind of dot-postfix body operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostfixOpKind {
    Copy,
    Freeze,
}

/// A single field initializer in an anonymous struct literal.
#[derive(Clone, Debug, PartialEq)]
pub struct StructLitField {
    pub name: String,
    pub name_span: SourceSpan,
    pub value: Expr,
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
            | Expr::MethodCall { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::StructLit { span, .. }
            | Expr::PostfixOp { span, .. }
            | Expr::IndexAccess { span, .. }
            | Expr::ArrayLit { span, .. }
            | Expr::MapLit { span, .. }
            | Expr::Is { span, .. }
            | Expr::InterpolatedString(_, span) => span,
            Expr::SelfValue { span } => span,
            Expr::NoneLit { span } => span,
            Expr::Wait(_, span) | Expr::Background(_, span) => span,
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Expr::Error(_))
    }
}

/// A call expression: `callee(args)` or `callee<types>(args)`.
#[derive(Clone, Debug, PartialEq)]
pub struct CallExpr {
    pub callee: Expr,
    /// M5: explicit type arguments at the call site (`foo<int>(x)`).
    /// `None` when the user relies on inference (the common case — `foo(x)`).
    /// Populated by P2's contextual `<` disambiguation.
    pub type_args: Option<Vec<Type>>,
    pub args: Vec<Expr>,
    pub span: SourceSpan,
}

/// A type annotation.
///
/// Variant count is pinned by the milestone-locked tests in the test suite.
/// Current count: 15 — M4 added Dynamic(9), SelfType(10); M5 added
/// TypeParam(11), Generic(12), Maybe(13); M6 added Union(14);
/// v0.1-polish added AnonShape(15).
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

    /// The type of a `range(...)` expression — an integer range value.
    ///
    /// First-class from M7 onward: can be stored, passed, and returned.
    /// Iterating over a `Range` in a `for` loop yields integers.
    Range {
        element: Box<Type>,
        /// Always `false` — `range(end)` and `range(start, end)` are end-exclusive.
        end_inclusive: bool,
    },

    // ── M4 ───────────────────────────────────────────────────────────────────

    // test-ratchet: M4 adds Dynamic and SelfType.
    /// `dynamic Foo` — runtime polymorphism via fat pointer + vtable lookup.
    ///
    /// Opt-in only: static dispatch is the default when the concrete type is known.
    Dynamic { contract: String, span: SourceSpan },

    /// `Self` — the concrete type of the enclosing shape (in type position).
    ///
    /// Only valid inside a shape body or a function whose first parameter is `self`.
    SelfType { span: SourceSpan },

    // ── M5 ───────────────────────────────────────────────────────────────────

    // test-ratchet: M5 adds TypeParam, Generic, Maybe.
    /// A type parameter reference inside a generic function or generic shape body.
    ///
    /// Example: in `function identity<T>(give value: T) -> T`, the two `T` uses
    /// produce `Type::TypeParam { name: "T", ... }`. Resolved against the
    /// enclosing function/shape's `generics` list at typeck time.
    TypeParam { name: String, span: SourceSpan },

    /// A generic type instantiation: `array<int>`, `Pair<A, B>`, `maybe<Player>`,
    /// `Box<map<string, int>>`, etc.
    ///
    /// `name` is the type constructor (built-in like `array` / `fixed` / `map`,
    /// or user-defined generic shape like `Pair`). `args` are the concrete
    /// type arguments. Typeck resolves `name` against the type-name table
    /// (built-ins + user-defined generic shapes).
    Generic {
        name: String,
        name_span: SourceSpan,
        args: Vec<Type>,
        span: SourceSpan,
    },

    /// `maybe<T>` — the built-in optional type.
    ///
    /// Stored as a distinct variant (rather than as `Generic { name: "maybe", ... }`)
    /// because `maybe<T>` has special typeck rules: flow-sensitive `.value`
    /// enforcement, the `none` literal, and a fixed lowering decision table.
    /// See `design/maybe.md` for the full spec.
    Maybe { inner: Box<Type>, span: SourceSpan },

    // ── M6 ───────────────────────────────────────────────────────────────────

    // test-ratchet: M6 adds Union for `A | B | C` union type declarations.
    /// A union type: `Circle | Square | Triangle` in type position.
    ///
    /// Typeck validates: ≥2 variants, no duplicate names, no `T | none` form
    /// (rewritten to `maybe<T>`), no single-variant degenerate form.
    /// LLVM lowering is chosen per variant set at codegen time.
    /// See `design/unions.md` for the full decision table.
    Union {
        variants: Vec<Type>,
        span: SourceSpan,
    },

    // ── M7: errors keyword ──────────────────────────────────────────────────

    // test-ratchet: M7 P1 adds ErrorCapable for `-> T errors` fallible return types.
    /// A fallible return type: `-> string errors`.
    ///
    /// The `errors` qualifier signals that the function can fail. Full
    /// error-type typeck (error propagation, the `?` operator, error shapes)
    /// arrives in M7 P3a. In M7 P1, this node is parsed and stored on
    /// `FunctionDecl.errors_capable`; typeck defers to the inner type.
    ErrorCapable { inner: Box<Type>, span: SourceSpan },

    // test-ratchet: M8 P4 adds Sensitive for `sensitive T` type modifier.
    /// A sensitive value that auto-redacts in `print()` and interpolation.
    ///
    /// Only wraps `string` in v0.1 (typeck enforces this). `.reveal()` strips
    /// the modifier and returns the underlying value.
    Sensitive(Box<Type>),

    // ── v0.1-polish: anonymous inline shapes ────────────────────────────────

    // test-ratchet: v0.1-polish adds AnonShape for `field: { name: T }` inline type syntax.
    /// An anonymous inline shape type: `spread: { bid: number\n ask: number }`.
    ///
    /// `{` in type position is unambiguous — no other Yinz type annotation uses it.
    /// Typeck synthesizes a name (`__anon_ParentShape_fieldName`) and registers it as
    /// a real shape so all downstream passes (codegen, print, field access) treat it
    /// identically to a named shape.
    AnonShape {
        fields: Vec<FieldDecl>,
        span: SourceSpan,
    },
}

// ── M4 shape declarations ────────────────────────────────────────────────────

/// Ownership modifier on the receiver `self` parameter in a contract signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReceiverKind {
    /// `share self` — read-only access; caller keeps ownership.
    Share,
    /// `lend self` — mutable access; caller keeps ownership.
    Lend,
    /// `give self` — ownership transfer into the function; caller gives up the value.
    Give,
}

/// A bare method signature inside a `shape` body (contract shapes only — no body).
///
/// Example: `greet(share self) -> string`
///
/// The `function` keyword is NOT used. Typeck verifies that a matching standalone
/// function exists in scope when the shape is used as a contract target.
#[derive(Clone, Debug, PartialEq)]
pub struct ContractSig {
    pub name: String,
    pub name_span: SourceSpan,
    /// The receiver kind for the implicit `self` parameter, if present.
    pub receiver: Option<ReceiverKind>,
    /// Additional parameters after `self`.
    pub params: Vec<Param>,
    pub return_type: Type,
    pub span: SourceSpan,
}

/// A field declaration inside a `shape` body.
///
/// Examples:
///   `name: string` — visible field, no default
///   `hidden cache: int = 0` — hidden field with a constant default
#[derive(Clone, Debug, PartialEq)]
pub struct FieldDecl {
    pub name: String,
    pub name_span: SourceSpan,
    pub ty: Type,
    pub ty_span: SourceSpan,
    /// `true` when declared with the `hidden` keyword.
    pub is_hidden: bool,
    /// Default expression for hidden fields (constants and empty literals only).
    pub default: Option<Expr>,
    pub span: SourceSpan,
    /// M8 P3: doc comment attached to this field.
    pub doc: Option<String>,
}

/// A shape (data type) declaration.
///
/// Examples:
///   `shape Player { name: string, health: int }`
///   `base shape Entity { name: string }`
///   `shape Warrior extends Entity follows Damageable { weapon: string }`
///   `shape Pair<A, B> { first: A, second: B }`  (M5 — generic shape)
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeDecl {
    pub name: String,
    pub name_span: SourceSpan,
    /// `true` when declared with the `base` keyword — cannot be instantiated directly.
    pub is_base: bool,
    /// M5: type parameters declared on the shape (`shape Pair<A, B> { ... }`).
    /// Empty Vec when the shape is not generic. Populated by the parser in P2.
    pub generics: Vec<GenericParam>,
    /// Optional parent shape for data-only inheritance.
    pub extends: Option<(String, SourceSpan)>,
    /// Zero or more contract shapes this shape must satisfy.
    pub follows: Vec<(String, SourceSpan)>,
    /// Data fields.
    pub fields: Vec<FieldDecl>,
    /// Bare method signatures (for contract shapes).
    pub contract_sigs: Vec<ContractSig>,
    /// M6: when set, this is a union type alias declaration: `shape Shape = Circle | Square`.
    /// The type is stored as a `Type::Union` (or any other type alias).
    /// When `Some`, `fields`, `extends`, `follows`, and `contract_sigs` are empty.
    pub alias_ty: Option<Type>,
    pub span: SourceSpan,
    /// M8: true when prefixed with `export` — visible to other modules.
    pub is_exported: bool,
    /// M8 P3: doc comment attached to this shape declaration.
    pub doc: Option<String>,
}
