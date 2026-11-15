//! Which string-typed expressions a store may take by move (v0.3-M9 C3b, plan #17;
//! `IMP-strings.md` "String Ownership and Release").
//!
//! Every place a string is stored must hold bytes of its own, or the scope-exit release (C6/C7)
//! frees a string that something else still points at. A store therefore takes a string by
//! MOVE when nothing else can reach it, and otherwise by an owned COPY (`owned_copy_plan`'s
//! `StringClone`). This module is the one record of which expressions qualify; codegen reads it
//! and decides nothing itself (`.claude/rules/authoritative-derivation.md`). An expression
//! that is not in the record is copied, so a hole here costs an allocation, never a double free.
//!
//! An expression of type `string` is in the record when it is
//!
//! * a FRESH string, one no other holder has seen: an interpolation with a `${}` part, the
//!   string methods that always build a new string (`toUpperCase`, `toLowerCase`, `trim`,
//!   `substring`, `replace`), `int`/`float`/`number` `.toString()`, `.copy()`, or a call to a
//!   Yinz function (a function returns an owned string: a returned local at its last use moves,
//!   anything else is copied at the `return`). Not fresh: a literal or a pure-text backtick
//!   string (static bytes), `boolean.toString()` and an options name (static), an element,
//!   field, map value, parameter or `.or()` result (someone else holds it);
//! * the LAST USE of a `let`/`const` string binding: the read is the only mention of the binding
//!   in its statement and the binding is not mentioned on any path after it. A parameter, a loop
//!   cell and a destructured alias never are (the caller or the container owns them).
//!
//! The last-use test is deliberately conservative: it never calls a read a last use when a later
//! read is possible. Flow facts are textual and structured (the language has no `break` or
//! `continue`): the continuation of a statement is the rest of its block and then the rest of
//! each enclosing block; a binding declared outside a loop and mentioned in it is live for the
//! whole loop; a `return` has no continuation.
//!
//! Cost: O(n * d) time and O(n * d) memory for a function of n statements and d `let`
//! bindings (one live set per statement).

use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::StringPart;
use ynz_ast::nodes::{Expr, FunctionDecl, Item, MatchPatternKind, Module, PostfixOpKind, Stmt};

use crate::binding_names::{decl_key, BindingNames, DeclKey};
use crate::effective_ownership::EffectiveOwnershipReport;
use crate::non_owned_reads::{NonOwnedReads, ReadTracker};
use crate::types::Type;

/// The record: the `(span.start, span.end)` of every string-typed expression a store takes by
/// move.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StringMoves {
    spans: HashSet<(usize, usize)>,
    /// The expressions of `spans` that are the last use of a `let`/`const` binding, with that
    /// binding. The scope-exit release plan reads this to know which binding a move may take the
    /// string from (`crate::release_plan`); a fresh string has no binding.
    last_uses: HashMap<(usize, usize), DeclKey>,
}

impl StringMoves {
    /// True when the expression at `start..end` is a fresh string or the last use of its
    /// binding, so a store may take it without copying.
    pub fn contains(&self, start: usize, end: usize) -> bool {
        self.spans.contains(&(start, end))
    }

    /// The `(start, end)` span of every expression in the record.
    pub fn spans(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.spans.iter().copied()
    }

    /// The binding whose last use is the expression at `start..end`, when the expression is in the
    /// record as a last use (not as a fresh string).
    pub fn last_use_binding(&self, start: usize, end: usize) -> Option<&DeclKey> {
        self.last_uses.get(&(start, end))
    }

    /// Every last-use expression in the record: its `(start, end)` span and the binding it reads.
    pub fn last_uses(&self) -> impl Iterator<Item = ((usize, usize), &DeclKey)> + '_ {
        self.last_uses
            .iter()
            .map(|(span, declaration)| (*span, declaration))
    }

    /// The number of expressions in the record.
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    /// True when no expression is in the record.
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
}

/// Build the string-move record and the read record for every function of `module`, in the ONE
/// liveness walk below (`non_owned_reads` judges the expressions this walk visits).
///
/// `expr_types` is typeck's per-expression type map, `names` resolves each identifier use to its
/// declaration, `callable_fns` holds the names of every Yinz function a call can reach (local and
/// imported), `imported_fns` the imported ones alone, and `ownership` is the effective-ownership fixpoint (which call positions only read).
pub fn record_moves_and_reads(
    module: &Module,
    expr_types: &HashMap<(usize, usize), Type>,
    names: &BindingNames,
    callable_fns: &HashSet<String>,
    imported_fns: &HashSet<String>,
    ownership: &EffectiveOwnershipReport,
) -> (StringMoves, NonOwnedReads) {
    let mut recorder = Recorder {
        expr_types,
        names,
        callable_fns,
        lets: HashSet::new(),
        moves: StringMoves::default(),
        reads: ReadTracker::new(names, ownership, imported_fns, expr_types),
    };
    for item in &module.items {
        if let Item::Function(function) = item {
            recorder.function(function);
        }
    }
    (recorder.moves, recorder.reads.finish())
}

struct Recorder<'a> {
    expr_types: &'a HashMap<(usize, usize), Type>,
    names: &'a BindingNames,
    callable_fns: &'a HashSet<String>,
    /// The `let`/`const` declarations of the function being walked.
    lets: HashSet<DeclKey>,
    moves: StringMoves,
    /// Judges which non-owned reads are views, over the same evaluations.
    reads: ReadTracker<'a>,
}

/// Names of the string methods that always build a new string (never return the receiver).
const FRESH_STRING_METHODS: &[&str] =
    &["toUpperCase", "toLowerCase", "trim", "substring", "replace"];

impl Recorder<'_> {
    fn function(&mut self, function: &FunctionDecl) {
        self.lets.clear();
        collect_lets(&function.body.stmts, &mut self.lets);
        self.reads.start_function(function);
        self.block(&function.body.stmts, &HashSet::new());
        self.reads.finish_function();
    }

    fn type_of(&self, expr: &Expr) -> Option<&Type> {
        let span = expr.span();
        self.expr_types.get(&(span.start, span.end))
    }

    /// The `let` declaration an identifier use resolves to, if it is one.
    fn let_decl_of(&self, expr: &Expr) -> Option<DeclKey> {
        let Expr::Ident(name, span) = expr else {
            return None;
        };
        let declaration = self.names.declaration_of_use(span, name)?;
        self.lets.contains(declaration).then(|| declaration.clone())
    }

    /// Walk a block whose continuation (what runs after it) mentions `after_block`.
    fn block(&mut self, stmts: &[Stmt], after_block: &HashSet<DeclKey>) {
        let mut live_after: Vec<HashSet<DeclKey>> = vec![HashSet::new(); stmts.len()];
        let mut running = after_block.clone();
        for (index, stmt) in stmts.iter().enumerate().rev() {
            live_after[index] = running.clone();
            self.mentions_stmt(stmt, &mut running);
        }
        for (stmt, after) in stmts.iter().zip(&live_after) {
            self.stmt(stmt, after);
        }
    }

    fn stmt(&mut self, stmt: &Stmt, after: &HashSet<DeclKey>) {
        self.view_writes(stmt, after);
        self.stmt_evaluations(stmt, after);
        // A `let` or reassignment of a place read is judged once its own value was evaluated.
        match stmt {
            Stmt::Let {
                name,
                name_span,
                ty,
                value,
                ..
            } => self
                .reads
                .declare_let(name, name_span, ty.as_ref(), value, &self.lets),
            Stmt::Assign {
                target,
                target_span,
                value,
                ..
            } => self
                .reads
                .declare_assign(target, target_span, value, &self.lets),
            _ => {}
        }
    }

    /// The writes and returns a statement makes, as far as non-owned reads are concerned: `name = …`,
    /// `name.f = …`, `name[i] = …` and `return …`.
    fn view_writes(&mut self, stmt: &Stmt, after: &HashSet<DeclKey>) {
        let mut mentioned = HashSet::new();
        match stmt {
            Stmt::Assign {
                target,
                target_span,
                value,
                ..
            } => {
                self.mentions_expr(value, &mut mentioned);
                self.reads
                    .evaluate_write_to(target, target_span, &mentioned, after);
            }
            Stmt::FieldAssign { target, value, .. } => {
                self.mentions_expr(target, &mut mentioned);
                self.mentions_expr(value, &mut mentioned);
                self.reads.evaluate_write(target, &mentioned, after);
            }
            Stmt::IndexAssign {
                receiver,
                index,
                value,
                ..
            } => {
                self.mentions_expr(receiver, &mut mentioned);
                self.mentions_expr(index, &mut mentioned);
                self.mentions_expr(value, &mut mentioned);
                self.reads.evaluate_write(receiver, &mentioned, after);
            }
            Stmt::Return {
                value: Some(value), ..
            } => self.reads.evaluate_return(value),
            _ => {}
        }
    }

    fn stmt_evaluations(&mut self, stmt: &Stmt, after: &HashSet<DeclKey>) {
        match stmt {
            Stmt::Expr(expr) | Stmt::Let { value: expr, .. } | Stmt::Assign { value: expr, .. } => {
                self.simple(&[expr], after);
            }
            // Nothing runs after a `return`, on any path.
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.simple(&[value], &HashSet::new());
                }
            }
            Stmt::FieldAssign { target, value, .. } => self.simple(&[target, value], after),
            Stmt::IndexAssign {
                receiver,
                index,
                value,
                ..
            } => self.simple(&[receiver, index, value], after),
            Stmt::If { cond, body, .. } => {
                let mut with_body = after.clone();
                self.mentions_block(&body.stmts, &mut with_body);
                self.simple(&[cond], &with_body);
                self.block(&body.stmts, after);
            }
            Stmt::Match {
                scrutinee,
                arms,
                else_arm,
                ..
            } => {
                let mut with_arms = after.clone();
                for arm in arms {
                    self.mentions_block(&arm.body.stmts, &mut with_arms);
                }
                if let Some(else_block) = else_arm {
                    self.mentions_block(&else_block.stmts, &mut with_arms);
                }
                self.simple(&[scrutinee], &with_arms);
                for arm in arms {
                    if let MatchPatternKind::Value(pattern) = &arm.pattern.kind {
                        self.simple(&[pattern], &with_arms);
                    }
                    self.block(&arm.body.stmts, after);
                }
                if let Some(else_block) = else_arm {
                    self.block(&else_block.stmts, after);
                }
            }
            Stmt::While { cond, body, .. } => {
                let around = self.loop_live(Some(cond), &body.stmts, after);
                self.simple(&[cond], &around);
                self.block(&body.stmts, &around);
            }
            Stmt::For {
                var,
                var_span,
                iter,
                body,
                ..
            } => {
                let mut with_body = after.clone();
                self.mentions_block(&body.stmts, &mut with_body);
                self.simple(&[iter], &with_body);
                let around = self.loop_live(None, &body.stmts, after);
                // The loop variable reads an element of the iterated place for the whole body.
                self.reads.begin_loop(var, var_span, iter, &self.lets);
                self.block(&body.stmts, &around);
                self.reads.end_loop(var_span, var);
            }
        }
    }

    /// What is live around a loop body: the continuation plus every `let` the loop mentions
    /// that was declared outside it (the next pass reads it again).
    fn loop_live(
        &self,
        cond: Option<&Expr>,
        body: &[Stmt],
        after: &HashSet<DeclKey>,
    ) -> HashSet<DeclKey> {
        let mut mentioned = HashSet::new();
        if let Some(cond) = cond {
            self.mentions_expr(cond, &mut mentioned);
        }
        self.mentions_block(body, &mut mentioned);
        let mut declared_inside = HashSet::new();
        collect_lets(body, &mut declared_inside);
        let mut around = after.clone();
        around.extend(
            mentioned
                .into_iter()
                .filter(|declaration| !declared_inside.contains(declaration)),
        );
        around
    }

    /// Record the movable expressions of a statement's own expressions. `after` is what runs
    /// after the expressions are evaluated.
    fn simple(&mut self, exprs: &[&Expr], after: &HashSet<DeclKey>) {
        let mut mentioned = HashSet::new();
        for expr in exprs {
            self.mentions_expr(expr, &mut mentioned);
        }
        self.reads.evaluate(exprs, &mentioned, after);
        for expr in exprs {
            self.reads.judge_calls(expr, &self.lets);
        }
        let mut counts: HashMap<DeclKey, usize> = HashMap::new();
        for expr in exprs {
            self.count_expr(expr, &mut counts);
        }
        for expr in exprs {
            self.walk(expr, &counts, after);
        }
    }

    fn walk(&mut self, expr: &Expr, counts: &HashMap<DeclKey, usize>, after: &HashSet<DeclKey>) {
        if matches!(self.type_of(expr), Some(Type::String)) {
            let last_use = self.let_decl_of(expr).filter(|declaration| {
                counts.get(declaration) == Some(&1) && !after.contains(declaration)
            });
            if self.is_fresh_string(expr) || last_use.is_some() {
                let span = expr.span();
                self.moves.spans.insert((span.start, span.end));
                if let Some(declaration) = last_use {
                    self.moves
                        .last_uses
                        .insert((span.start, span.end), declaration);
                }
            }
        }
        for child in children(expr) {
            self.walk(child, counts, after);
        }
    }

    /// True when `expr` (typed `string`) builds a string no other holder has seen.
    fn is_fresh_string(&self, expr: &Expr) -> bool {
        match expr {
            Expr::InterpolatedString(parts, _) => parts
                .iter()
                .any(|part| matches!(part, StringPart::Expr(..))),
            Expr::MethodCall {
                receiver, method, ..
            } => {
                // A dot-call reaches a Yinz function only on a shape (or `dynamic` contract)
                // receiver, and only after every built-in method of that name is ruled out
                // (`Checker::check_method_call`). On any other receiver the name is a built-in
                // such as `.or()`, whose result is borrowed even when a function shares its name.
                let reaches_function = matches!(
                    self.type_of(receiver),
                    Some(Type::Shape { .. } | Type::Dynamic { .. })
                );
                if reaches_function && self.callable_fns.contains(method) {
                    return true;
                }
                match self.type_of(receiver) {
                    Some(Type::String) => FRESH_STRING_METHODS.contains(&method.as_str()),
                    Some(Type::Int | Type::Float | Type::Number { .. }) => method == "toString",
                    _ => false,
                }
            }
            Expr::Call(call) => {
                matches!(&call.callee, Expr::Ident(name, _) if self.callable_fns.contains(name))
            }
            Expr::PostfixOp {
                op: PostfixOpKind::Copy,
                ..
            } => true,
            Expr::Wait(inner, _) => self.is_fresh_string(inner),
            _ => false,
        }
    }

    // ── mentions: the `let` bindings an expression reads ────────────────────────────────

    fn mentions_block(&self, stmts: &[Stmt], out: &mut HashSet<DeclKey>) {
        for stmt in stmts {
            self.mentions_stmt(stmt, out);
        }
    }

    fn mentions_stmt(&self, stmt: &Stmt, out: &mut HashSet<DeclKey>) {
        match stmt {
            Stmt::Expr(expr) | Stmt::Let { value: expr, .. } | Stmt::Assign { value: expr, .. } => {
                self.mentions_expr(expr, out);
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.mentions_expr(value, out);
                }
            }
            Stmt::FieldAssign { target, value, .. } => {
                self.mentions_expr(target, out);
                self.mentions_expr(value, out);
            }
            Stmt::IndexAssign {
                receiver,
                index,
                value,
                ..
            } => {
                self.mentions_expr(receiver, out);
                self.mentions_expr(index, out);
                self.mentions_expr(value, out);
            }
            Stmt::If { cond, body, .. } => {
                self.mentions_expr(cond, out);
                self.mentions_block(&body.stmts, out);
            }
            Stmt::Match {
                scrutinee,
                arms,
                else_arm,
                ..
            } => {
                self.mentions_expr(scrutinee, out);
                for arm in arms {
                    if let MatchPatternKind::Value(pattern) = &arm.pattern.kind {
                        self.mentions_expr(pattern, out);
                    }
                    self.mentions_block(&arm.body.stmts, out);
                }
                if let Some(else_block) = else_arm {
                    self.mentions_block(&else_block.stmts, out);
                }
            }
            Stmt::While { cond, body, .. } => {
                self.mentions_expr(cond, out);
                self.mentions_block(&body.stmts, out);
            }
            Stmt::For { iter, body, .. } => {
                self.mentions_expr(iter, out);
                self.mentions_block(&body.stmts, out);
            }
        }
    }

    fn mentions_expr(&self, expr: &Expr, out: &mut HashSet<DeclKey>) {
        if let Some(declaration) = self.let_decl_of(expr) {
            out.insert(declaration);
        }
        for child in children(expr) {
            self.mentions_expr(child, out);
        }
    }

    fn count_expr(&self, expr: &Expr, counts: &mut HashMap<DeclKey, usize>) {
        if let Some(declaration) = self.let_decl_of(expr) {
            *counts.entry(declaration).or_insert(0) += 1;
        }
        for child in children(expr) {
            self.count_expr(child, counts);
        }
    }
}

/// The `let`/`const` declarations in `stmts`, at any depth.
fn collect_lets(stmts: &[Stmt], out: &mut HashSet<DeclKey>) {
    for stmt in stmts {
        match stmt {
            Stmt::Let {
                name, name_span, ..
            } => {
                out.insert(decl_key(name_span, name));
            }
            Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_lets(&body.stmts, out);
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    collect_lets(&arm.body.stmts, out);
                }
                if let Some(else_block) = else_arm {
                    collect_lets(&else_block.stmts, out);
                }
            }
            Stmt::Expr(_)
            | Stmt::Assign { .. }
            | Stmt::Return { .. }
            | Stmt::FieldAssign { .. }
            | Stmt::IndexAssign { .. } => {}
        }
    }
}

/// The direct sub-expressions of `expr`.
pub(crate) fn children(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::Ident(..)
        | Expr::StringLit(..)
        | Expr::Error(_)
        | Expr::IntLit(..)
        | Expr::NumberLit(..)
        | Expr::BoolLit(..)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. } => Vec::new(),
        Expr::Call(call) => {
            let mut all = vec![&call.callee];
            all.extend(call.args.iter());
            all
        }
        Expr::BinOp { lhs, rhs, .. } => vec![lhs.as_ref(), rhs.as_ref()],
        Expr::UnaryOp { operand, .. } => vec![operand.as_ref()],
        Expr::MethodCall { receiver, args, .. } => {
            let mut all = vec![receiver.as_ref()];
            all.extend(args.iter());
            all
        }
        Expr::FieldAccess { receiver, .. } => vec![receiver.as_ref()],
        Expr::StructLit { fields, .. } => fields.iter().map(|field| &field.value).collect(),
        Expr::PostfixOp { receiver, .. } => vec![receiver.as_ref()],
        Expr::IndexAccess {
            receiver, index, ..
        } => vec![receiver.as_ref(), index.as_ref()],
        Expr::ArrayLit { elements, .. } => elements.iter().collect(),
        Expr::MapLit { entries, .. } => entries
            .iter()
            .flat_map(|(key, value)| [key, value])
            .collect(),
        Expr::Is { expr, .. } => vec![expr.as_ref()],
        Expr::InterpolatedString(parts, _) => parts
            .iter()
            .filter_map(|part| match part {
                StringPart::Expr(inner, _) => Some(inner.as_ref()),
                StringPart::Lit(..) => None,
            })
            .collect(),
        Expr::Wait(inner, _) | Expr::Background(inner, _) => vec![inner.as_ref()],
    }
}
