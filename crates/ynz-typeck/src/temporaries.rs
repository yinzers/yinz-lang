//! The temporaries of a statement: fresh heap values no binding holds, released where their
//! statement ends (v0.3-M9 C7 slice 2; `IMP-ownership.md` "Scope-exit release on the plain path").
//!
//! `makeRows().count()`, `i.toString()` inside an interpolation, `makeBucket().rows.count()` and
//! `for (row in makeRows())` each build a heap value that nothing names. The release plan lists
//! bindings, so it cannot reach them; this record names them instead, by the span of the
//! expression that builds each, and the release point of each. Codegen reads it and decides
//! nothing (`.claude/rules/authoritative-derivation.md`).
//!
//! # What is a temporary
//!
//! An expression is a temporary when
//!
//! * it builds a value nobody else holds: a call of a Yinz function (every function returns an
//!   owned value, `copied_at_return`, so this holds for an imported one too), an array, map or
//!   shape literal, a fresh string (`string_ownership::is_fresh_string_expr`) or an explicit
//!   `.copy()`;
//! * the value owns heap the plain emitter releases: a string, array, map, shape, `fixed` list,
//!   `maybe` or union, or a value of a type parameter (judged per instance by codegen); and
//! * nothing takes it: a `let`, an assignment, a `return`, a store, a `give` argument, a
//!   `background` argument or an element of a literal owns the value from then on, so it is not
//!   a temporary. Every other position only reads it (an operand, an interpolated part, a
//!   receiver, an argument at a position the callee does not own, a condition, the value of an
//!   expression statement), and the value dies at the end of its statement.
//!
//! The walk is a whitelist: a built-in method not named in [`BORROWING_METHODS`], a call of an
//! unknown function and every position not listed above take what they are given, so an
//! unclassified use leaks a value and never frees one twice.
//!
//! A piece of a temporary (`makeBucket().rows`) is not itself a temporary: the call is, and its
//! whole object graph is released at the statement end. A piece a `let` or assignment keeps is
//! copied or taken before that (`non_owned_reads`: `Take` and `MustCopy`). A place read taken
//! whole into a type the copy at `let` and `return` does not cover (a `maybe`, a union) suppresses
//! every temporary inside it: such a copy would share the temporary's cells.
//!
//! # Where each is released
//!
//! | statement | key | released |
//! |---|---|---|
//! | expression statement, `let`, assignment, field or index assignment, `return` | the statement | after it (a `return`: after its value is built, before the exit edge) |
//! | `if`, `match` | the statement | after the whole statement: a narrowed binding may read the scrutinee in an arm |
//! | `while` | the condition | after the condition is evaluated, every pass |
//! | `for` | the statement | after the loop |
//!
//! A statement that suspends keeps its temporaries in frame slots (`Cg::open_wait_temporaries`).

use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{Block, Expr, Item, MatchPatternKind, OwnershipModifier, PostfixOpKind, Stmt};

use crate::release_plan::{heap_class_of, HeapClass, ReleaseKind, TypeTables};
use crate::signatures::FunctionSig;
use crate::string_ownership::is_fresh_string_expr;
use crate::types::{is_owned_text, Type, DECIMAL128_MAX_PRECISION};

/// `(span.start, span.end)`.
pub type Span = (usize, usize);

/// Built-in methods that only read what they are given: their receiver and arguments are
/// borrowed (a store position, `collection_store_positions`, takes its argument).
const BORROWING_METHODS: &[&str] = &[
    "count",
    "byteCount",
    "graphemeCount",
    "contains",
    "startsWith",
    "endsWith",
    "byteAt",
    "indexOf",
    "toUpperCase",
    "toLowerCase",
    "trim",
    "substring",
    "replace",
    "toString",
    "exists",
    "get",
    "first",
    "last",
    "or",
    "failed",
    "split",
    "add",
    "set",
];

/// The temporaries of a module, by expression span.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Temporaries {
    /// Expression span -> the span of the release point it dies at.
    temporaries: HashMap<Span, Span>,
    /// Release point -> the temporaries released there, in walk order.
    released_at: HashMap<Span, Vec<Span>>,
    /// The temporaries that are an explicit `.copy()` of a `maybe`: each owns a heap envelope cell
    /// of its own (`OwnedCopy::MaybeCellClone`), which its release frees after the payload.
    minted_maybe_copies: HashSet<Span>,
    /// Spans of explicit `.copy()` temporaries whose type is a type parameter `T` (a generic body is
    /// checked once): an instance whose `T` is a `maybe` mints an envelope cell for it.
    type_parameter_copies: HashSet<Span>,
    /// The arguments of a call of a Yinz function that build a shape or `maybe` value no
    /// binding holds and that are not temporaries: one at a `give` position (the callee owns it), or a
    /// shape that owns no heap. The bytes of such a value sit where the expression built them, which a callee that
    /// suspends cannot read after its own `wait`; codegen keeps them in a frame region while it
    /// stages the argument (`Cg::stage_value_in_frame`).
    staged_arguments: HashSet<Span>,
    /// The `background` arguments that are a piece of a fresh value (`make().rows`) of a type the
    /// task cannot be given a copy of (`OwnedCopy::Refused`): the spawn hands the piece over
    /// uncopied, so neither the task nor the statement end can release it, and typeck refuses the
    /// spawn (`Temporaries::refused_spawn_pieces`).
    refused_spawn_pieces: Vec<ynz_diagnostics::SourceSpan>,
}

impl Temporaries {
    /// The release point of the temporary built by the expression at `start..end`.
    pub fn release_point(&self, start: usize, end: usize) -> Option<Span> {
        self.temporaries.get(&(start, end)).copied()
    }

    /// True when the temporary built by the expression at `start..end` is an explicit `.copy()` of a
    /// `maybe`: it sits in a heap envelope cell of its own that its release frees
    /// (`Cg::open_temporaries`).
    pub fn is_minted_maybe_copy(&self, start: usize, end: usize) -> bool {
        self.minted_maybe_copies.contains(&(start, end))
    }

    /// True when the temporary at `start..end` is an explicit `.copy()` typed as a type parameter `T`.
    pub fn is_type_parameter_copy(&self, start: usize, end: usize) -> bool {
        self.type_parameter_copies.contains(&(start, end))
    }

    /// True when the expression at `start..end` is an argument of a call of a Yinz function that
    /// builds a shape or `maybe` value and is no temporary (a `give` position, or a
    /// shape that owns no heap).
    pub fn is_staged_argument(&self, start: usize, end: usize) -> bool {
        self.staged_arguments.contains(&(start, end))
    }

    /// The `background` arguments that are a piece of a fresh value of a type with no owned copy
    /// (`make().rows` typed `fixed<array<string>>`), in walk order.
    pub fn refused_spawn_pieces(&self) -> &[ynz_diagnostics::SourceSpan] {
        &self.refused_spawn_pieces
    }

    /// True when the expression at `start..end` builds a temporary.
    pub fn is_temporary(&self, start: usize, end: usize) -> bool {
        self.temporaries.contains_key(&(start, end))
    }

    /// The temporaries (expression spans) released at the release point `start..end`.
    pub fn released_at(&self, start: usize, end: usize) -> &[Span] {
        self.released_at
            .get(&(start, end))
            .map_or(&[], Vec::as_slice)
    }

    /// The number of temporaries in the record.
    pub fn len(&self) -> usize {
        self.temporaries.len()
    }

    /// True when the record holds none.
    pub fn is_empty(&self) -> bool {
        self.temporaries.is_empty()
    }
}

/// What a position does with the value an expression builds there.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Use {
    /// Only read: the value dies at the end of its statement.
    Borrowed,
    /// Owned by someone from then on (a binding, a store, a callee, a task, the caller).
    Taken,
}

/// What the walk reads of the checked module.
pub struct TemporaryFacts<'a> {
    /// The type of every expression.
    pub expr_types: &'a HashMap<Span, Type>,
    /// Every value that flows into a union slot: the envelope takes it, so it is not a temporary.
    pub union_coercions: &'a HashMap<Span, crate::check::UnionCoercion>,
    /// Every Yinz function a call can reach, local and imported.
    pub callable_fns: &'a HashSet<String>,
    /// The signature of every function the checker knows (the declared ownership of each
    /// parameter).
    pub signatures: &'a HashMap<String, FunctionSig>,
    /// The generic functions' signatures: a generic callee is absent from `signatures`, and an
    /// unknown callee is assumed to take its arguments, which left a temporary passed to a
    /// generic function unreleased.
    pub generic_signatures: &'a HashMap<String, crate::generics::GenericFnSig>,
    /// The shape tables `heap_class_of` reads.
    pub tables: TypeTables<'a>,
}

/// Record the temporaries of every function of `module`.
///
/// O(size of the module) time and memory.
pub fn record_temporaries(
    module: &ynz_ast::nodes::Module,
    facts: &TemporaryFacts<'_>,
) -> Temporaries {
    let mut walk = Walk {
        facts,
        out: Temporaries::default(),
    };
    for item in &module.items {
        if let Item::Function(function) = item {
            walk.block(&function.body);
        }
    }
    walk.out
}

struct Walk<'a> {
    facts: &'a TemporaryFacts<'a>,
    out: Temporaries,
}

fn span_of(expr: &Expr) -> Span {
    let span = expr.span();
    (span.start, span.end)
}

impl Walk<'_> {
    fn type_of(&self, expr: &Expr) -> Option<&Type> {
        self.facts.expr_types.get(&span_of(expr))
    }

    fn block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        let key = crate::release_plan::stmt_span(stmt);
        match stmt {
            Stmt::Expr(expr) => self.value(expr, Use::Borrowed, key, false),
            Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
                self.value(value, Use::Taken, key, false)
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.value(value, Use::Taken, key, false);
                }
            }
            Stmt::FieldAssign { target, value, .. } => {
                self.value(target, Use::Borrowed, key, false);
                self.value(value, Use::Taken, key, false);
            }
            Stmt::IndexAssign {
                receiver,
                index,
                value,
                ..
            } => {
                self.value(receiver, Use::Borrowed, key, false);
                // `table[key] = value` stores the key in the map, which owns it from then on
                // (`collection_store_positions` says the same of `table.set(key, value)`); an
                // array or `fixed` index is an `int`.
                let index_usage = if matches!(self.type_of(receiver), Some(Type::BuiltinMap { .. }))
                {
                    Use::Taken
                } else {
                    Use::Borrowed
                };
                self.value(index, index_usage, key, false);
                self.value(value, Use::Taken, key, false);
            }
            Stmt::If { cond, body, .. } => {
                self.value(cond, Use::Borrowed, key, false);
                self.block(body);
            }
            Stmt::Match {
                scrutinee,
                arms,
                else_arm,
                ..
            } => {
                self.value(scrutinee, Use::Borrowed, key, false);
                for arm in arms {
                    // A value pattern is compared, never released: left alone (a leak at worst).
                    let _ = matches!(arm.pattern.kind, MatchPatternKind::Value(_));
                    self.block(&arm.body);
                }
                if let Some(else_block) = else_arm {
                    self.block(else_block);
                }
            }
            Stmt::While { cond, body, .. } => {
                // The condition runs every pass: its temporaries die after it, not after the loop.
                self.value(cond, Use::Borrowed, span_of(cond), false);
                self.block(body);
            }
            Stmt::For { iter, body, .. } => {
                self.value(iter, Use::Borrowed, key, false);
                self.block(body);
            }
        }
    }

    /// Walk `expr`, used as `usage`, whose temporaries are released at `key`. `suppress` is true
    /// inside a place read taken whole into a type the copy does not cover.
    fn value(&mut self, expr: &Expr, usage: Use, key: Span, suppress: bool) {
        let suppress_inside = suppress
            || (usage == Use::Taken
                && self.is_piece_read(expr)
                && !self.copy_covers(self.type_of(expr)));
        self.children(expr, key, suppress_inside);
        if usage == Use::Borrowed && !suppress && self.is_temporary(expr) {
            let span = span_of(expr);
            if self.out.temporaries.insert(span, key).is_none() {
                self.out.released_at.entry(key).or_default().push(span);
                if self.is_maybe_copy(expr) {
                    self.out.minted_maybe_copies.insert(span);
                } else if matches!(
                    expr,
                    Expr::PostfixOp {
                        op: PostfixOpKind::Copy,
                        ..
                    }
                ) && matches!(self.type_of(expr), Some(Type::TypeParam { .. }))
                {
                    self.out.type_parameter_copies.insert(span);
                }
            }
        }
    }

    /// True when a `let` or `return` of a place read of this type copies it (or takes it) before
    /// the temporary behind the place dies.
    fn copy_covers(&self, ty: Option<&Type>) -> bool {
        match ty {
            Some(
                Type::String
                | Type::BuiltinArray { .. }
                | Type::BuiltinMap { .. }
                | Type::Shape { .. },
            ) => true,
            Some(ty) if is_owned_text(ty) => true,
            // A value that owns no heap (an `int`, a `boolean`) cannot share a temporary's cells.
            Some(ty) => matches!(heap_class_of(ty, &self.facts.tables), HeapClass::NoHeap),
            None => false,
        }
    }

    /// True when `expr` reads a place (a field, an element, a built-in method's result) instead of
    /// building a value.
    fn is_piece_read(&self, expr: &Expr) -> bool {
        match expr {
            Expr::FieldAccess { .. } | Expr::IndexAccess { .. } => true,
            Expr::MethodCall { .. } => !self.builds_a_value(expr),
            Expr::Wait(inner, _) => self.is_piece_read(inner),
            _ => false,
        }
    }

    /// True when `expr` is a call of a Yinz function or another form that returns a value of its
    /// own (never a piece of an argument).
    fn is_user_call(&self, expr: &Expr) -> bool {
        crate::string_ownership::is_user_call(expr, self.facts.expr_types, self.facts.callable_fns)
    }

    /// True when `expr` is an explicit `.copy()` of a `maybe` whose copy mints an envelope cell.
    fn is_maybe_copy(&self, expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::PostfixOp {
                op: PostfixOpKind::Copy,
                ..
            }
        ) && self.type_of(expr).is_some_and(|ty| {
            matches!(ty, Type::Maybe { .. })
                && matches!(
                    crate::owned_copy::owned_copy_plan(ty),
                    crate::owned_copy::OwnedCopy::MaybeCellClone
                )
        })
    }

    /// True when `expr` is `string.get(i)` or `string.graphemeAt(i)`: a `maybe<string>` holding a new
    /// string (`builtins::builtin_method_returns_fresh_maybe_string`).
    fn is_fresh_maybe_string(&self, expr: &Expr) -> bool {
        let Expr::MethodCall {
            receiver, method, ..
        } = expr
        else {
            return false;
        };
        self.type_of(receiver).is_some_and(|ty| {
            crate::builtins::builtin_method_returns_fresh_maybe_string(ty, method)
        })
    }

    /// True when `expr` is a built-in method on a string whose result is a fresh heap value
    /// (`builtins::builtin_method_returns_fresh`: `s.split(sep)` builds an array of new strings).
    /// A channel or task handle `receive` is in the same table but is not a string method.
    fn is_fresh_builtin_result(&self, expr: &Expr) -> bool {
        let Expr::MethodCall {
            receiver, method, ..
        } = expr
        else {
            return false;
        };
        let on_a_string = match self.type_of(receiver) {
            Some(Type::String) => true,
            Some(Type::Sensitive { inner }) => matches!(inner.as_ref(), Type::String),
            _ => false,
        };
        on_a_string && crate::builtins::builtin_method_returns_fresh(method)
    }

    /// True when `expr` builds a value no other holder has seen.
    fn builds_a_value(&self, expr: &Expr) -> bool {
        match expr {
            Expr::ArrayLit { .. } | Expr::MapLit { .. } | Expr::StructLit { .. } => true,
            Expr::PostfixOp {
                op: PostfixOpKind::Copy,
                ..
            } => true,
            _ if self.is_user_call(expr) => true,
            _ if self.is_fresh_builtin_result(expr) => true,
            _ if self.is_fresh_maybe_string(expr) => true,
            _ => {
                self.type_of(expr).is_some_and(is_owned_text)
                    && is_fresh_string_expr(expr, self.facts.expr_types, self.facts.callable_fns)
            }
        }
    }

    /// True when `expr` builds a value of a kind the plain emitter releases (a string, array,
    /// map or flat shape) that nobody else holds.
    fn is_temporary(&self, expr: &Expr) -> bool {
        if !self.builds_a_value(expr) {
            return false;
        }
        // A value that enters a union slot is wrapped by the envelope, which takes it.
        if self.facts.union_coercions.contains_key(&span_of(expr)) {
            return false;
        }
        let Some(ty) = self.type_of(expr) else {
            return false;
        };
        // A function hands back a channel, or a `maybe` of one, as a counted reference of its own
        // (`copied_at_return`), which nobody gives back unless the statement does.
        let owns_a_channel_reference = matches!(ty, Type::BuiltinChannel { .. })
            || matches!(ty, Type::Maybe { inner } if matches!(inner.as_ref(), Type::BuiltinChannel { .. }));
        // A function returns a `maybe` of a string, shape or number with the payload its `return`
        // allocated (a string clone, a heap cell); a `let` takes it (`maybe_to_owned_taking`), so a
        // call whose `maybe` nothing binds (`find(rows).or(fallback)`, `.exists()`) frees it here.
        let owns_a_returned_payload = (self.is_user_call(expr) || self.is_fresh_maybe_string(expr))
            && matches!(ty, Type::Maybe { inner } if !matches!(inner.as_ref(), Type::BuiltinChannel { .. }))
            && crate::owned_copy::copied_at_return(ty);
        // A value of a type parameter `T` (`value.copy()` in a generic body): the body is checked once,
        // so it is recorded as one that may own heap, and codegen releases it only in an instance
        // whose `T` is a string, array, map or shape (`Cg::open_temporaries`).
        let is_type_parameter = matches!(ty, Type::TypeParam { .. });
        owns_a_channel_reference
            || owns_a_returned_payload
            || self.is_maybe_copy(expr)
            || is_type_parameter
            || matches!(
                heap_class_of(ty, &self.facts.tables),
                HeapClass::Heap(
                    ReleaseKind::String
                        | ReleaseKind::Array
                        | ReleaseKind::Map
                        | ReleaseKind::Shape
                        | ReleaseKind::NestedShape
                        | ReleaseKind::Fixed
                        | ReleaseKind::Union
                        | ReleaseKind::ErrorsCapable
                )
            )
    }

    /// The declared ownership of position `index` of the call of `name`: `None` when the
    /// function is unknown (its parameter is then assumed to take the value).
    fn param_ownership(&self, name: &str, index: usize) -> Option<Option<OwnershipModifier>> {
        if let Some(signature) = self.facts.signatures.get(name) {
            return signature.param_ownerships.get(index).cloned();
        }
        self.facts
            .generic_signatures
            .get(name)
            .and_then(|signature| signature.param_ownerships.get(index).cloned())
    }

    /// The use a call of `name` makes of its argument at `index`.
    fn argument_use(&self, name: &str, index: usize) -> Use {
        match self.param_ownership(name, index) {
            Some(Some(OwnershipModifier::Give)) | None => Use::Taken,
            Some(_) => Use::Borrowed,
        }
    }

    /// An argument of a Yinz function that builds a shape or `maybe` value and that no
    /// temporary covers (it is at a `give` position, or its shape owns no heap), or a decimal128
    /// `number` that is not a named local (a literal or an expression; a named local is frame-backed
    /// as a crossing local): recorded so codegen keeps its bytes in the frame while a suspending
    /// callee runs (`Temporaries::is_staged_argument`).
    /// A value that enters a union slot is staged as a union, not as a shape. A `maybe` of an envelope
    /// union has no frame home yet and is not staged.
    ///
    /// O(1) time.
    fn staged_argument(&mut self, arg: &Expr) {
        let span = span_of(arg);
        if self.out.temporaries.contains_key(&span)
            || self.facts.union_coercions.contains_key(&span)
            || !(self.builds_a_value(arg) || self.is_unnamed_decimal(arg))
            || !self
                .type_of(arg)
                .is_some_and(|ty| self.has_staging_region(ty))
        {
            return;
        }
        self.out.staged_arguments.insert(span);
    }

    /// True when `expr` is a decimal128 `number` that is not a named local: its value sits in a stack
    /// temporary the expression made, which a named local does not (the crossing classifier frames it).
    ///
    /// O(1) time.
    fn is_unnamed_decimal(&self, expr: &Expr) -> bool {
        !matches!(expr, Expr::Ident(..))
            && self
                .type_of(expr)
                .is_some_and(|ty| matches!(ty, Type::Number { precision } if *precision <= DECIMAL128_MAX_PRECISION))
    }

    /// True for the kinds a staged argument keeps in a frame region: a shape, a decimal128 `number` and a `maybe` that is not
    /// of an envelope union (a `fixed` call result has no known size: its signature type is unsized).
    ///
    /// O(1) time.
    fn has_staging_region(&self, ty: &Type) -> bool {
        match ty {
            Type::Shape { .. } => true,
            Type::Maybe { inner } => !crate::check::is_envelope_union(inner),
            Type::Number { precision } => *precision <= DECIMAL128_MAX_PRECISION,
            _ => false,
        }
    }

    /// An argument of a Yinz function that typeck coerces into a union parameter (`kind(`text`)`) is
    /// copied into an envelope the call mints; when the callee only reads it (anything but `give`) the
    /// envelope is a temporary of the statement: it owns the text it wrapped (a copy, or the fresh
    /// string moved in) and nothing else releases it. Recorded under the argument's span, and codegen
    /// reads the union slot from `union_coercions` for the temporary's type. Only owned text enters
    /// an envelope by copy: a shape variant is viewed, an `int` owns nothing.
    ///
    /// O(1) time.
    fn envelope_argument(&mut self, arg: &Expr, usage: Use, key: Span, suppress: bool) {
        let span = span_of(arg);
        if usage != Use::Borrowed
            || suppress
            || !self.facts.union_coercions.contains_key(&span)
            || !self.type_of(arg).is_some_and(is_owned_text)
        {
            return;
        }
        if self.out.temporaries.insert(span, key).is_none() {
            self.out.released_at.entry(key).or_default().push(span);
        }
    }

    /// A `background` argument that is a field or element read off a value (`make().items`,
    /// `make().inner.items`, `make()[0]`): when that value is a temporary, the task is handed a
    /// copy of the piece and nothing else holds the rest, so the value dies at the statement end
    /// like any other temporary (`value`). The exception is a piece of a type with no owned copy
    /// (`OwnedCopy::Refused`): codegen hands that piece to the task as it is, so releasing the
    /// value would free what the task still reads. It is recorded instead
    /// (`Temporaries::refused_spawn_pieces`) for typeck to refuse. A whole value or a piece of a
    /// binding is not touched (the spawn releases the first, the binding's scope exit the second).
    ///
    /// O(d) time for a piece d fields deep; O(1) space.
    fn spawn_argument_piece(&mut self, arg: &Expr, key: Span) {
        let mut base = arg;
        while let Expr::FieldAccess { receiver, .. } | Expr::IndexAccess { receiver, .. } = base {
            base = receiver;
        }
        if std::ptr::eq(base, arg) {
            return;
        }
        match self.type_of(arg).map(crate::owned_copy::owned_copy_plan) {
            Some(crate::owned_copy::OwnedCopy::Refused(_)) => {
                if self.is_temporary(base) {
                    self.out.refused_spawn_pieces.push(arg.span().clone());
                }
            }
            Some(_) => self.value(base, Use::Borrowed, key, false),
            None => {}
        }
    }

    fn children(&mut self, expr: &Expr, key: Span, suppress: bool) {
        match expr {
            Expr::Ident(..)
            | Expr::StringLit(..)
            | Expr::Error(_)
            | Expr::IntLit(..)
            | Expr::NumberLit(..)
            | Expr::BoolLit(..)
            | Expr::SelfValue { .. }
            | Expr::NoneLit { .. } => {}
            Expr::Call(call) => {
                let Expr::Ident(name, _) = &call.callee else {
                    for arg in &call.args {
                        self.value(arg, Use::Taken, key, suppress);
                    }
                    return;
                };
                for (index, arg) in call.args.iter().enumerate() {
                    let usage = if self.facts.callable_fns.contains(name) {
                        self.argument_use(name, index)
                    } else if name == "sensitive" {
                        // Identity at the ABI level: the result is the argument.
                        Use::Taken
                    } else {
                        // `print`, `range`, `channel` and the other free built-ins read their
                        // arguments.
                        Use::Borrowed
                    };
                    self.value(arg, usage, key, suppress);
                    if self.facts.callable_fns.contains(name) {
                        self.envelope_argument(arg, usage, key, suppress);
                        self.staged_argument(arg);
                    }
                }
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                if self.is_user_call(expr) {
                    self.value(receiver, self.argument_use(method, 0), key, suppress);
                    self.staged_argument(receiver);
                    for (index, arg) in args.iter().enumerate() {
                        self.value(arg, self.argument_use(method, index + 1), key, suppress);
                        self.envelope_argument(
                            arg,
                            self.argument_use(method, index + 1),
                            key,
                            suppress,
                        );
                        self.staged_argument(arg);
                    }
                    return;
                }
                if !BORROWING_METHODS.contains(&method.as_str()) {
                    self.value(receiver, Use::Taken, key, suppress);
                    for arg in args {
                        self.value(arg, Use::Taken, key, suppress);
                    }
                    return;
                }
                self.value(receiver, Use::Borrowed, key, suppress);
                let stores: &[usize] = self.type_of(receiver).map_or(&[], |ty| {
                    crate::builtins::collection_store_positions(ty, method)
                });
                for (index, arg) in args.iter().enumerate() {
                    let usage = if stores.contains(&index) {
                        Use::Taken
                    } else {
                        Use::Borrowed
                    };
                    self.value(arg, usage, key, suppress);
                }
            }
            Expr::BinOp { lhs, rhs, .. } => {
                self.value(lhs, Use::Borrowed, key, suppress);
                self.value(rhs, Use::Borrowed, key, suppress);
            }
            Expr::UnaryOp { operand, .. } => self.value(operand, Use::Borrowed, key, suppress),
            Expr::FieldAccess { receiver, .. } => {
                self.value(receiver, Use::Borrowed, key, suppress)
            }
            Expr::IndexAccess {
                receiver, index, ..
            } => {
                self.value(receiver, Use::Borrowed, key, suppress);
                self.value(index, Use::Borrowed, key, suppress);
            }
            Expr::PostfixOp { receiver, op, .. } => {
                let usage = match op {
                    PostfixOpKind::Copy => Use::Borrowed,
                    PostfixOpKind::Freeze => Use::Taken,
                };
                self.value(receiver, usage, key, suppress);
            }
            Expr::StructLit { fields, .. } => {
                for field in fields {
                    self.value(&field.value, Use::Taken, key, suppress);
                }
            }
            Expr::ArrayLit { elements, .. } => {
                for element in elements {
                    self.value(element, Use::Taken, key, suppress);
                }
            }
            Expr::MapLit { entries, .. } => {
                for (entry_key, entry_value) in entries {
                    self.value(entry_key, Use::Taken, key, suppress);
                    self.value(entry_value, Use::Taken, key, suppress);
                }
            }
            Expr::Is { expr: inner, .. } => self.value(inner, Use::Borrowed, key, suppress),
            Expr::InterpolatedString(parts, _) => {
                for part in parts {
                    if let ynz_ast::nodes::StringPart::Expr(inner, _) = part {
                        self.value(inner, Use::Borrowed, key, suppress);
                    }
                }
            }
            // The value a `wait` produces is taken by its position like any call result. What the
            // awaited call is handed is judged as for a plain call: a temporary of the statement
            // that survives the suspension in a frame slot (C8 slice 2, `open_wait_temporaries`).
            Expr::Wait(inner, _) => self.value(inner, Use::Taken, key, suppress),
            // A `background` call hands its arguments to the task: a whole value it is given is
            // not a temporary of this statement (the spawn releases it, `prepare_bg_arg_for_ctx`).
            // A PIECE of a fresh value (`make().items`) is different: the task gets a copy of the
            // piece and nothing hands the rest of `make()`'s result on, so that result is a
            // temporary of the statement like any other (`spawn_argument_piece`).
            Expr::Background(inner, _) => {
                self.value(inner, Use::Taken, key, true);
                if let Expr::Call(call) = inner.as_ref() {
                    for arg in &call.args {
                        self.spawn_argument_piece(arg, key);
                    }
                }
            }
        }
    }
}
