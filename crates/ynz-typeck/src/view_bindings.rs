//! Which `let` bindings of a place read are a read-only VIEW of the place (v0.3-M9 C1f round 2,
//! plan #17; `IMP-unions.md` "Views").
//!
//! `let rest = head.next` names a place that stays where it is. Copying it into the binding is
//! what `Clone` does: one new box for every box the value owns, so a recursive walk that peels
//! one link per call copies the whole tail at every step, O(n²) cells where a Rust borrow costs
//! none (Golden Rule 8). A binding that only READS what it names can point at the place instead,
//! with the language's value semantics unchanged. This module is the one record of which
//! bindings qualify; codegen reads it and decides nothing (`authoritative-derivation.md`). A
//! binding that is not in the record keeps its deep copy, so a hole costs cells, never a
//! dangling read.
//!
//! A `let view = root.field...` is in the record when ALL of these hold:
//!
//! * (a) `view` is never written (`view = …`, `view.f = …`), stored (a literal field, an element,
//!   a method or call argument that is not read-only), returned, given or sent;
//! * (b) the root of the place is not assigned, written through a field or index, passed to a
//!   position that may write or consume it, or used as a receiver that may write it, anywhere in
//!   `view`'s live range. A parameter the function only reads (`EffectiveOwnership::Reads`, a
//!   `share` parameter) satisfies (b) for the whole body. A root given away is already refused by
//!   the checker for any later read of `view` (a `Reaches` binding joins its roots' alias
//!   classes: `check_transfer`);
//! * (c) no `wait` or `background` lies in `view`'s live range. Codegen adds the calls that
//!   suspend without a `wait`: a binding that crosses a suspension (`sm_crossing_*`, the one
//!   authoritative set) is never a view.
//!
//! The live range comes from the one liveness walk of `string_ownership` (the same `after` sets
//! C3b's `string_moves` uses): this module only judges the expressions that walk evaluates.
//!
//! Only a place made of field reads from a parameter or a `let` that is not itself a candidate
//! qualifies: an element read (`arr[i]`, `.get(i)`) is a copy into per-site staging, and whether a
//! built-in collection method writes its receiver has no table to read, so an element read
//! keeps its deep copy. A place read with a declared type annotation (a possible union wrap) is
//! not a view either.
//!
//! Cost: O(c) per evaluated expression for the c candidates declared so far.

use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{Expr, FunctionDecl, PostfixOpKind};

use crate::binding_names::{decl_key, BindingNames, DeclKey};
use crate::effective_ownership::{
    call_position_reads, value_like, EffectiveOwnership, EffectiveOwnershipReport,
};
use crate::string_ownership::children;
use crate::types::Type;

/// The `(span.start, span.end)` of the value expression of every `let` that is a read-only view.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewBindings {
    spans: HashSet<(usize, usize)>,
}

impl ViewBindings {
    /// True when the `let` whose value expression is at `start..end` is a read-only view.
    pub fn contains(&self, start: usize, end: usize) -> bool {
        self.spans.contains(&(start, end))
    }

    /// The `(start, end)` span of every value expression in the record.
    pub fn spans(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.spans.iter().copied()
    }

    /// The number of views in the record.
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    /// True when no binding is a view.
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
}

/// Built-in collection methods whose result is a read of one element of the receiver.
const ELEMENT_READ_METHODS: &[&str] = &["get", "first", "last"];

struct Candidate {
    view: DeclKey,
    root: DeclKey,
    /// True when the root is a parameter the function only reads: (b) holds for the whole body.
    root_is_read_only_parameter: bool,
    value_span: (usize, usize),
    /// The view this one is read through (`let b = a.g` with `a` a view of `x`): `b` is a view of
    /// `x`, and only while `a` is.
    via: Option<DeclKey>,
    viable: bool,
}

/// Judges the `let` bindings of one function while `string_ownership`'s walk visits its
/// expressions in order.
pub(crate) struct ViewTracker<'a> {
    names: &'a BindingNames,
    ownership: &'a EffectiveOwnershipReport,
    imported_fn_names: &'a HashSet<String>,
    expr_types: &'a HashMap<(usize, usize), Type>,
    function_name: String,
    parameters: HashSet<DeclKey>,
    read_only_parameters: HashSet<DeclKey>,
    candidates: Vec<Candidate>,
    views: ViewBindings,
}

impl<'a> ViewTracker<'a> {
    pub(crate) fn new(
        names: &'a BindingNames,
        ownership: &'a EffectiveOwnershipReport,
        imported_fn_names: &'a HashSet<String>,
        expr_types: &'a HashMap<(usize, usize), Type>,
    ) -> Self {
        Self {
            names,
            ownership,
            imported_fn_names,
            expr_types,
            function_name: String::new(),
            parameters: HashSet::new(),
            read_only_parameters: HashSet::new(),
            candidates: Vec::new(),
            views: ViewBindings::default(),
        }
    }

    /// Begin a function: its parameters are the roots that need no `let` liveness.
    pub(crate) fn start_function(&mut self, function: &FunctionDecl) {
        self.function_name = function.name.clone();
        self.parameters.clear();
        self.read_only_parameters.clear();
        self.candidates.clear();
        for (index, parameter) in function.params.iter().enumerate() {
            let key = decl_key(&parameter.name_span, &parameter.name);
            if self.ownership.ownership_of(&function.name, index) == EffectiveOwnership::Reads {
                self.read_only_parameters.insert(key.clone());
            }
            self.parameters.insert(key);
        }
    }

    /// End a function: every candidate no evaluation disqualified is a view.
    pub(crate) fn finish_function(&mut self) {
        // A view read through another view is a view only while that one is: the candidates are
        // in declaration order, so the view it goes through is decided first.
        let mut decided: HashMap<DeclKey, bool> = HashMap::new();
        for candidate in self.candidates.drain(..) {
            let through_is_view = candidate.via.as_ref().map_or(true, |through| {
                decided.get(through).copied().unwrap_or(false)
            });
            let is_view = candidate.viable && through_is_view;
            decided.insert(candidate.view.clone(), is_view);
            if is_view {
                self.views.spans.insert(candidate.value_span);
            }
        }
    }

    pub(crate) fn finish(self) -> ViewBindings {
        self.views
    }

    /// A `let name = value` was just evaluated. It becomes a candidate when `value` is a field
    /// read from a parameter or a plain `let`, of a heap type, with no annotation.
    pub(crate) fn declare(
        &mut self,
        name: &str,
        name_span: &ynz_diagnostics::SourceSpan,
        annotated: bool,
        value: &Expr,
        lets: &HashSet<DeclKey>,
    ) {
        if annotated {
            return;
        }
        let Some(place) = self.view_place(value) else {
            return;
        };
        let span = value.span();
        let heap_valued = match self.expr_types.get(&(span.start, span.end)) {
            Some(Type::Shape { .. } | Type::Union { .. }) => true,
            Some(Type::Maybe { inner }) => {
                matches!(inner.as_ref(), Type::Shape { .. } | Type::Union { .. })
            }
            _ => false,
        };
        if !heap_valued {
            return;
        }
        let Expr::Ident(root_name, root_span) = place else {
            return;
        };
        let Some(root) = self.names.declaration_of_use(root_span, root_name).cloned() else {
            return;
        };
        let (root, root_is_read_only_parameter, via) = match self
            .candidates
            .iter()
            .find(|candidate| candidate.view == root)
        {
            // `let b = a.g` with `a` a view of `x`: `b` is a view of `x`, checked against `x`.
            Some(through) if through.viable => (
                through.root.clone(),
                through.root_is_read_only_parameter,
                Some(through.view.clone()),
            ),
            // A view already disqualified owns a copy: it is a plain `let` root.
            Some(_) if lets.contains(&root) => (root.clone(), false, None),
            Some(_) => return,
            None if self.parameters.contains(&root) => {
                let read_only = self.read_only_parameters.contains(&root);
                (root, read_only, None)
            }
            None if lets.contains(&root) => (root, false, None),
            None => return,
        };
        self.candidates.push(Candidate {
            view: decl_key(name_span, name),
            root_is_read_only_parameter,
            root,
            value_span: (span.start, span.end),
            via,
            viable: true,
        });
    }

    /// The identifier a view of `value` points through, when `value` is a read of one place: a
    /// field chain (`head.next`), an element (`arr[i]`, `arr[i].f`) or an element method of a
    /// built-in collection (`arr.get(i)`, `.first()`, `.last()`). An element read is a copy of
    /// the element's bytes into per-site staging, so it shares the element's boxes and stays
    /// valid while the collection is not written: growth, `set`, `remove` and `clear` are
    /// writes (`call_position_reads`), and any write breaks (b).
    fn view_place<'e>(&self, value: &'e Expr) -> Option<&'e Expr> {
        match value {
            Expr::FieldAccess { .. } | Expr::IndexAccess { .. } => {
                place_root(value).filter(|root| matches!(root, Expr::Ident(..)))
            }
            Expr::MethodCall {
                receiver, method, ..
            } if ELEMENT_READ_METHODS.contains(&method.as_str())
                && !self.ownership.is_local_fn(method)
                && !self.imported_fn_names.contains(method.as_str()) =>
            {
                let receiver_span = receiver.span();
                let is_collection = matches!(
                    self.expr_types
                        .get(&(receiver_span.start, receiver_span.end)),
                    Some(
                        Type::BuiltinArray { .. }
                            | Type::BuiltinFixed { .. }
                            | Type::BuiltinMap { .. }
                    )
                );
                if is_collection {
                    place_root(receiver)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// The expressions `exprs` are evaluated while `after` (the `let`s read later) and the
    /// `mentioned` lets of these expressions say which views are live. Disqualify every live view
    /// that is written, escapes, or whose root may change here, and every view live across a
    /// `wait` or `background`.
    pub(crate) fn evaluate(
        &mut self,
        exprs: &[&Expr],
        mentioned: &HashSet<DeclKey>,
        after: &HashSet<DeclKey>,
    ) {
        for index in 0..self.candidates.len() {
            let candidate = &self.candidates[index];
            if !candidate.viable
                || !(after.contains(&candidate.view) || mentioned.contains(&candidate.view))
            {
                continue;
            }
            let (view, root, check_root) = (
                candidate.view.clone(),
                candidate.root.clone(),
                !candidate.root_is_read_only_parameter,
            );
            let disqualified = exprs.iter().any(|expr| {
                suspends(expr)
                    || self.changes(expr, &view)
                    || (check_root && self.changes(expr, &root))
            });
            if disqualified {
                self.candidates[index].viable = false;
            }
        }
    }

    /// A statement writes the binding `target_use` names (`name = …`, `name.f = …`,
    /// `name[i] = …`) while `after` / `mentioned` say which views are live.
    pub(crate) fn evaluate_write(
        &mut self,
        target: &Expr,
        mentioned: &HashSet<DeclKey>,
        after: &HashSet<DeclKey>,
    ) {
        let Some(Expr::Ident(name, span)) = place_root(target) else {
            return;
        };
        self.evaluate_write_to(name, span, mentioned, after);
    }

    pub(crate) fn evaluate_write_to(
        &mut self,
        name: &str,
        span: &ynz_diagnostics::SourceSpan,
        mentioned: &HashSet<DeclKey>,
        after: &HashSet<DeclKey>,
    ) {
        let written = self.names.declaration_of_use(span, name).cloned();
        for candidate in &mut self.candidates {
            if !candidate.viable
                || !(after.contains(&candidate.view) || mentioned.contains(&candidate.view))
            {
                continue;
            }
            let hits_root =
                !candidate.root_is_read_only_parameter && written.as_ref() == Some(&candidate.root);
            // An unresolved target could be anything: the view is not proven.
            if written.is_none() || written.as_ref() == Some(&candidate.view) || hits_root {
                candidate.viable = false;
            }
        }
    }

    /// A `return value` while views are live: a returned view (or a place read through one)
    /// escapes the function.
    pub(crate) fn evaluate_return(&mut self, value: &Expr) {
        if !self.holds_boxes(value) {
            return;
        }
        let Some(Expr::Ident(name, span)) = place_root(value) else {
            return;
        };
        let Some(returned) = self.names.declaration_of_use(span, name).cloned() else {
            return;
        };
        for candidate in &mut self.candidates {
            if candidate.view == returned {
                candidate.viable = false;
            }
        }
    }

    /// False when `expr` is typed as bits (an `int`, a `string`, a `maybe` of those): copying it
    /// is the value, so it shares nothing. An expression with no recorded type holds boxes.
    fn holds_boxes(&self, expr: &Expr) -> bool {
        let span = expr.span();
        !self
            .expr_types
            .get(&(span.start, span.end))
            .is_some_and(value_like)
    }

    /// True when `expr` may write, store, give, send or return what `target` names, or pass it
    /// to a position that may.
    fn changes(&self, expr: &Expr, target: &DeclKey) -> bool {
        // A place that is bits (an `int` field read through the view) is copied, never shared.
        let is_target = |place: &Expr| -> bool {
            self.holds_boxes(place)
                && matches!(
                    place_root(place),
                    Some(Expr::Ident(name, span))
                        if self.names.declaration_of_use(span, name) == Some(target)
                )
        };
        match expr {
            Expr::Call(call) => {
                let callee_name = match &call.callee {
                    Expr::Ident(name, _) => Some(name.as_str()),
                    _ => None,
                };
                call.args.iter().enumerate().any(|(position, arg)| {
                    if is_target(arg) {
                        // A position proven read-only reads the place; anything else may not.
                        return callee_name.map_or(true, |name| {
                            !call_position_reads(
                                self.ownership,
                                self.imported_fn_names,
                                name,
                                position,
                                false,
                            )
                        });
                    }
                    self.changes(arg, target)
                })
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                let reads = |position: usize| {
                    call_position_reads(
                        self.ownership,
                        self.imported_fn_names,
                        method,
                        position,
                        true,
                    )
                };
                if is_target(receiver) && !reads(0) {
                    return true;
                }
                if !is_target(receiver) && self.changes(receiver, target) {
                    return true;
                }
                args.iter().enumerate().any(|(position, arg)| {
                    if is_target(arg) {
                        return !reads(position + 1);
                    }
                    self.changes(arg, target)
                })
            }
            Expr::StructLit { fields, .. } => fields
                .iter()
                .any(|field| is_target(&field.value) || self.changes(&field.value, target)),
            Expr::ArrayLit { elements, .. } => elements
                .iter()
                .any(|element| is_target(element) || self.changes(element, target)),
            Expr::MapLit { entries, .. } => entries.iter().any(|(key, value)| {
                is_target(key)
                    || is_target(value)
                    || self.changes(key, target)
                    || self.changes(value, target)
            }),
            // A copy reads its receiver; everything else only reads what it mentions.
            Expr::PostfixOp {
                op: PostfixOpKind::Copy,
                receiver,
                ..
            } => !is_target(receiver) && self.changes(receiver, target),
            other => children(other)
                .into_iter()
                .any(|child| self.changes(child, target)),
        }
    }
}

/// The identifier at the base of a place made of field and index reads (`a`, `a.f`, `a.f[i].g`).
fn place_root(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Ident(..) => Some(expr),
        Expr::FieldAccess { receiver, .. } | Expr::IndexAccess { receiver, .. } => {
            place_root(receiver)
        }
        _ => None,
    }
}

/// True when `expr` holds a `wait` or `background`: a suspension point.
fn suspends(expr: &Expr) -> bool {
    matches!(expr, Expr::Wait(..) | Expr::Background(..))
        || children(expr).into_iter().any(suspends)
}
