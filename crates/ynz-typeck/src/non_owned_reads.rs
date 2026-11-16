//! Which reads of a place that someone else owns may point at the place, and which must copy it
//! (v0.3-M9 C6; `IMP-ownership.md` "Non-owned reads"; `IMP-unions.md` "Views").
//!
//! `let rest = head.next` names a place that stays where it is. Copying it into the binding is
//! what `Clone` does: one new box for every box the value owns, so a recursive walk that peels
//! one link per call copies the whole tail at every step, O(n²) cells where a Rust borrow costs
//! none (Golden Rule 8). A read that only READS what it names can point at the place instead,
//! with the language's value semantics unchanged. But the release helper (C6) frees a value when
//! its place is overwritten, so a read that points at a place the program then writes reads
//! freed memory. This module is the one record of which reads point and which copy; codegen
//! reads it and decides nothing (`authoritative-derivation.md`). A read that is not in the
//! record copies, so a hole costs cells, never a dangling read.
//!
//! The record is keyed by the `(span.start, span.end)` of the expression that reads the place.
//! These sites are judged, each over a place read of a heap-holding type (a string, array, map,
//! shape, union, or a `maybe` of those):
//!
//! * a `let` or reassignment value (`let r = b.rows`, `r = b.rows`): the binding is the reader;
//! * a `for` loop variable (keyed by the iterated expression): the reader is the variable, live
//!   for the whole body;
//! * a call argument at a position the callee only reads: the reader is the callee's parameter,
//!   live for the call.
//!
//! A `return` has no entry: a returned value outlives its frame, so a place read there always
//! copies, and a fresh value or an owned local at its last use moves (`heap_moves`,
//! `string_moves`). Codegen copies the rest, and the caller binds a call result as an owned
//! value (`Provenance::Fresh`).
//!
//! A read of `root.field...` is a [`ReadVerdict::View`] when ALL of these hold:
//!
//! * (a) the reader is never written (`view = …`, `view.f = …`), stored (a literal field, an
//!   element, a method or call argument that is not read-only), returned, given or sent;
//! * (b) no root of the place is assigned, written through a field or index, passed to a
//!   position that may write or consume it, or used as a receiver that may write it, anywhere
//!   in the reader's live range (for a call argument: in any other argument of the same call,
//!   including a `lend` of the root; for a loop variable: anywhere in the body, including
//!   through a `lend` callee). A parameter the function only reads
//!   (`EffectiveOwnership::Reads`, a `share` parameter) satisfies (b) for the whole body;
//! * (c) no `wait` or `background` lies in a binding's live range. Codegen adds the calls that
//!   suspend without a `wait`: a binding that crosses a suspension (`sm_crossing_*`, the one
//!   authoritative set) is never a view.
//!
//! Anything else judged is a [`ReadVerdict::MustCopy`].
//!
//! A temporary dies at the end of its statement (`crate::temporaries`), so a `let` or
//! reassignment whose place chain passes through one never views it: a field of a call's result
//! (a string, array or map) is a [`ReadVerdict::Take`], every other such read a
//! [`ReadVerdict::MustCopy`].
//!
//! The live range comes from the one liveness walk of `string_ownership` (the same `after` sets
//! C3b's `string_moves` uses): this module only judges the expressions that walk evaluates.
//!
//! A place qualifies when it is a field-read chain from a parameter or a `let` (`head.next`), a
//! read through another view (`a.g` with `a` a view of `x`), an element read of a built-in
//! array, fixed list or map (`arr[i]`, `arr[i].f`, `arr.get(i)`, `.first()`, `.last()`), or the
//! `.or(default)` of such a read, which is a piece of the read or of the default (both are
//! roots). An element read copies the element's bytes into per-site staging and so shares its
//! boxes: it stays valid while the collection is not written, and every in-place mutator or
//! growth of the collection breaks (b) (`call_position_reads`).
//!
//! Cost: O(c) per evaluated expression for the c candidates declared so far.

use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{Expr, FunctionDecl, PostfixOpKind};

use crate::binding_names::{decl_key, BindingNames, DeclKey};
use crate::effective_ownership::{
    call_position_reads, provenance, value_like, EffectiveOwnership, EffectiveOwnershipReport,
    Provenance, ProvenanceCtx,
};
use crate::string_ownership::children;
use crate::types::Type;

/// What a non-owned read does with the place it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadVerdict {
    /// The reader points at the place: nothing writes the place while the reader is live.
    View,
    /// The reader owns a copy of the place's value, released with the reader's scope.
    MustCopy,
    /// The place is a field of a temporary (`let rows = makeBucket().rows`): the reader takes the
    /// field's pointer and the temporary's release skips it, as Rust moves a field out of a
    /// temporary and drops the rest. No copy.
    Take,
}

/// True when a `let` (under `annotation`) or an assignment (`None`) of `value` takes a piece of a
/// temporary: `value` is a chain of field reads off the result of a call of a Yinz function and
/// names a string, array, map or shape (a nested shape is a heap cell the temporary owns), and
/// the annotation, if any, is that same type. THE one answer the
/// `Take` verdict (`NonOwnedReads`) and the checker's binding event (the binding owns what it
/// takes, `check::binding_event_origin`) both read. `type_of` is the type oracle and `is_user_fn`
/// says a name is a function a call can reach.
pub(crate) fn is_taken_piece_of_a_temporary(
    value: &Expr,
    annotation: Option<&ynz_ast::nodes::Type>,
    type_of: &dyn Fn(&Expr) -> Option<Type>,
    is_user_fn: &dyn Fn(&str) -> bool,
) -> bool {
    let value_type = type_of(value);
    let annotation_keeps_the_type = match (annotation, &value_type) {
        (None, _) => true,
        (Some(ynz_ast::nodes::Type::Generic { name, .. }), _) => name == "array" || name == "map",
        // A nested shape is taken only under its own name: any other annotation (a union, an
        // alias) wraps or converts the value, which is a copy.
        (Some(ynz_ast::nodes::Type::Named(name, _)), Some(Type::Shape { name: shape_name })) => {
            name == shape_name
        }
        (Some(ynz_ast::nodes::Type::Named(name, _)), _) => name == "string",
        (Some(_), _) => false,
    };
    if !annotation_keeps_the_type
        || !matches!(
            value_type,
            Some(
                Type::String
                    | Type::BuiltinArray { .. }
                    | Type::BuiltinMap { .. }
                    | Type::Shape { .. }
            )
        )
    {
        return false;
    }
    let mut current = value;
    let mut fields = 0;
    while let Expr::FieldAccess { receiver, .. } = current {
        current = receiver;
        fields += 1;
    }
    fields > 0
        && match current {
            Expr::Call(call) => {
                matches!(&call.callee, Expr::Ident(name, _) if is_user_fn(name))
            }
            Expr::MethodCall {
                receiver, method, ..
            } => {
                // A user function of a built-in's name does not shadow it unless the receiver is
                // a shape or a `dynamic` value (`is_fresh_string`).
                let reaches_function = matches!(
                    type_of(receiver),
                    Some(Type::Shape { .. } | Type::Dynamic { .. })
                );
                is_user_fn(method) && reaches_function
            }
            _ => false,
        }
}

/// The verdict of every judged non-owned read, by the `(span.start, span.end)` of the expression
/// that reads the place (for a loop variable, the iterated expression).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NonOwnedReads {
    verdicts: HashMap<(usize, usize), ReadVerdict>,
}

impl NonOwnedReads {
    /// The verdict for the read at `start..end`; `None` when the read was not judged (a fresh
    /// value, a moved one, a bits value), which codegen treats as a copy unless it moves.
    pub fn verdict(&self, start: usize, end: usize) -> Option<ReadVerdict> {
        self.verdicts.get(&(start, end)).copied()
    }

    /// True when the read at `start..end` points at its place.
    pub fn is_view(&self, start: usize, end: usize) -> bool {
        self.verdict(start, end) == Some(ReadVerdict::View)
    }

    /// True when the read at `start..end` takes a field out of a temporary.
    pub fn is_take(&self, start: usize, end: usize) -> bool {
        self.verdict(start, end) == Some(ReadVerdict::Take)
    }

    /// True when the read at `start..end` was judged and owns a copy.
    pub fn must_copy(&self, start: usize, end: usize) -> bool {
        self.verdict(start, end) == Some(ReadVerdict::MustCopy)
    }

    /// The `(start, end)` span of every read that is a view.
    pub fn views(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.verdicts
            .iter()
            .filter(|(_, verdict)| **verdict == ReadVerdict::View)
            .map(|(span, _)| *span)
    }

    /// The `(start, end)` span of every judged read that owns a copy.
    pub fn copies(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.verdicts
            .iter()
            .filter(|(_, verdict)| **verdict == ReadVerdict::MustCopy)
            .map(|(span, _)| *span)
    }

    /// The number of views in the record.
    pub fn view_count(&self) -> usize {
        self.views().count()
    }

    /// True when no read was judged.
    pub fn is_empty(&self) -> bool {
        self.verdicts.is_empty()
    }
}

/// Built-in collection methods whose result is a read of one element of the receiver.
const ELEMENT_READ_METHODS: &[&str] = &["get", "first", "last"];

/// True when a value of `ty` is held by a binding that a place-read `let` or reassignment may
/// point at the place instead of copying: a shape, union, array or map, or a `maybe` of a shape
/// or union. (A string binding always owns its bytes: `string_ownership`.)
fn binding_holds_place(ty: &Type) -> bool {
    match ty {
        Type::Shape { .. }
        | Type::Union { .. }
        | Type::BuiltinArray { .. }
        | Type::BuiltinMap { .. } => true,
        Type::Maybe { inner } => matches!(
            inner.as_ref(),
            Type::Shape { .. }
                | Type::Union { .. }
                | Type::BuiltinArray { .. }
                | Type::BuiltinMap { .. }
        ),
        _ => false,
    }
}

/// True when a value of `ty` owns heap a reader may share and the store seam copies: a string,
/// array, map, shape or union. The types a loop variable or a call argument is judged for (a
/// `maybe` has its own copy form, used by `let` and reassignment only).
fn reader_holds_heap(ty: &Type) -> bool {
    matches!(
        ty,
        Type::String
            | Type::BuiltinArray { .. }
            | Type::BuiltinMap { .. }
            | Type::Shape { .. }
            | Type::Union { .. }
    ) || matches!(ty, Type::Sensitive { inner } if matches!(inner.as_ref(), Type::String))
}

/// One identifier a place is read through, and whether the function can write it.
#[derive(Clone, PartialEq, Eq)]
struct Root {
    key: DeclKey,
    /// True when the root is a parameter the function only reads: (b) holds for the whole body.
    read_only_parameter: bool,
}

/// The reader a candidate stands for.
#[derive(PartialEq, Eq)]
enum Reader {
    /// A `let` binding: others read through it, and it is live while a later statement mentions
    /// it.
    Binding(DeclKey),
    /// A loop variable: live for the whole body while `body_active`, others read through it.
    LoopVariable(DeclKey),
    /// A call argument: judged once, when its call is evaluated.
    Argument,
}

/// The roots a place is read through and the views it goes through (`ReadTracker::resolve_roots`).
type ResolvedPlace = (Vec<Root>, Vec<DeclKey>);

struct Candidate {
    reader: Reader,
    roots: Vec<Root>,
    value_span: (usize, usize),
    /// The views this one reads through (`let b = a.g` with `a` a view of `x`): `b` is a view of
    /// `x`, and only while each of them is.
    through: Vec<DeclKey>,
    viable: bool,
    /// True while the body of a loop-variable candidate is being walked.
    body_active: bool,
}

impl Candidate {
    fn holder(&self) -> Option<&DeclKey> {
        match &self.reader {
            Reader::Binding(key) | Reader::LoopVariable(key) => Some(key),
            Reader::Argument => None,
        }
    }

    /// True when a `wait` or `background` in the reader's live range ends the view. A binding's
    /// view may point at per-site staging (a shape element read) that the resume function no
    /// longer has after a suspension; a loop variable or an argument of a string, array, map or
    /// shape reads a heap value the frame keeps (a shape loop variable crossing a `wait` is
    /// embedded in the frame by codegen), so only a write to the place ends it.
    fn suspension_breaks(&self) -> bool {
        matches!(self.reader, Reader::Binding(_))
    }

    /// True when others may read through this candidate.
    fn is_readable_through(&self) -> bool {
        matches!(self.reader, Reader::Binding(_) | Reader::LoopVariable(_))
    }

    /// True when the reader is live at a statement: later statements mention it, this one does,
    /// or it is a loop variable whose body is being walked.
    fn live(&self, mentioned: &HashSet<DeclKey>, after: &HashSet<DeclKey>) -> bool {
        match self.holder() {
            Some(holder) => {
                self.body_active || after.contains(holder) || mentioned.contains(holder)
            }
            None => false,
        }
    }
}

/// Judges the non-owned reads of one function while `string_ownership`'s walk visits its
/// expressions in order.
pub(crate) struct ReadTracker<'a> {
    names: &'a BindingNames,
    ownership: &'a EffectiveOwnershipReport,
    imported_fn_names: &'a HashSet<String>,
    expr_types: &'a HashMap<(usize, usize), Type>,
    parameters: HashSet<DeclKey>,
    /// The local functions, for `provenance`.
    local_fn_names: HashSet<String>,
    read_only_parameters: HashSet<DeclKey>,
    candidates: Vec<Candidate>,
    /// The candidates of the loops whose body is being walked, innermost last.
    open_loops: Vec<usize>,
    reads: NonOwnedReads,
}

impl<'a> ReadTracker<'a> {
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
            local_fn_names: ownership.local_fn_names(),
            parameters: HashSet::new(),
            read_only_parameters: HashSet::new(),
            candidates: Vec::new(),
            open_loops: Vec::new(),
            reads: NonOwnedReads::default(),
        }
    }

    /// Begin a function: its parameters are the roots that need no `let` liveness.
    pub(crate) fn start_function(&mut self, function: &FunctionDecl) {
        self.parameters.clear();
        self.read_only_parameters.clear();
        self.candidates.clear();
        self.open_loops.clear();
        for (index, parameter) in function.params.iter().enumerate() {
            let key = decl_key(&parameter.name_span, &parameter.name);
            if self.ownership.ownership_of(&function.name, index) == EffectiveOwnership::Reads {
                self.read_only_parameters.insert(key.clone());
            }
            self.parameters.insert(key);
        }
    }

    /// End a function: every candidate no evaluation disqualified is a view, the rest copy.
    pub(crate) fn finish_function(&mut self) {
        // A read through another view is a view only while that one is: the candidates are in
        // declaration order, so the view it goes through is decided first.
        let mut decided: HashMap<DeclKey, bool> = HashMap::new();
        for candidate in self.candidates.drain(..) {
            let through_are_views = candidate
                .through
                .iter()
                .all(|through| decided.get(through).copied().unwrap_or(false));
            let is_view = candidate.viable && through_are_views;
            if candidate.is_readable_through() {
                if let Some(holder) = candidate.holder() {
                    decided.insert(holder.clone(), is_view);
                }
            }
            let verdict = if is_view {
                ReadVerdict::View
            } else {
                ReadVerdict::MustCopy
            };
            self.reads.verdicts.insert(candidate.value_span, verdict);
        }
    }

    pub(crate) fn finish(self) -> NonOwnedReads {
        self.reads
    }

    fn type_of(&self, expr: &Expr) -> Option<&Type> {
        let span = expr.span();
        self.expr_types.get(&(span.start, span.end))
    }

    /// Record that the read at `span` owns a copy, with no analysis to run.
    fn must_copy(&mut self, span: (usize, usize)) {
        self.reads.verdicts.insert(span, ReadVerdict::MustCopy);
    }

    // ── places ──────────────────────────────────────────────────────────────────────────

    /// The identifiers a read of `value` points through: `Some([])` for a read of a temporary
    /// (nobody else holds it), `None` for a form the analysis cannot trace. `value` must be a
    /// place read (`is_place_read`) for the answer to mean "reads a place".
    ///
    /// A field or element chain reads through its base; `.get`/`.first`/`.last` of a built-in
    /// collection through the collection; `.or(default)` of such a read through the read and the
    /// default (`let r = rows[0].or(fallback)` is the element or `fallback`).
    fn place_roots<'e>(&self, value: &'e Expr) -> Option<Vec<&'e Expr>> {
        match value {
            Expr::Ident(..) => Some(vec![value]),
            Expr::FieldAccess { receiver, .. } | Expr::IndexAccess { receiver, .. } => {
                self.place_roots(receiver)
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } if self.is_builtin_method(receiver, method) => {
                if ELEMENT_READ_METHODS.contains(&method.as_str()) {
                    let receiver_type = self.type_of(receiver);
                    if !matches!(
                        receiver_type,
                        Some(
                            Type::BuiltinArray { .. }
                                | Type::BuiltinFixed { .. }
                                | Type::BuiltinMap { .. }
                        )
                    ) {
                        return None;
                    }
                    self.place_roots(receiver)
                } else if method == "or" && args.len() == 1 {
                    let mut roots = self.place_roots(receiver)?;
                    roots.extend(self.default_roots(&args[0])?);
                    Some(roots)
                } else {
                    None
                }
            }
            // A piece of what a call returns: the result is an owned copy nobody else holds.
            Expr::Call(_) | Expr::MethodCall { .. } => Some(Vec::new()),
            Expr::Wait(inner, _) => self.place_roots(inner),
            _ => None,
        }
    }

    /// The identifiers the default of an `.or(default)` reaches: its own place, or none for a
    /// value built in place (a literal, a call result).
    fn default_roots<'e>(&self, default: &'e Expr) -> Option<Vec<&'e Expr>> {
        match default {
            Expr::Ident(..) | Expr::FieldAccess { .. } | Expr::IndexAccess { .. } => {
                self.place_roots(default)
            }
            Expr::MethodCall { .. } if self.is_place_read(default) => self.place_roots(default),
            Expr::ArrayLit { .. }
            | Expr::MapLit { .. }
            | Expr::StructLit { .. }
            | Expr::StringLit(..)
            | Expr::InterpolatedString(..)
            | Expr::IntLit(..)
            | Expr::Call(_)
            | Expr::PostfixOp { .. } => Some(Vec::new()),
            _ => None,
        }
    }

    /// True when the place chain of `value` passes through a temporary: a call result, or the
    /// default of an `.or` that is built in place. A view of such a place would outlive the
    /// temporary, which is released at the end of its statement.
    fn through_temporary(&self, value: &Expr) -> bool {
        match value {
            Expr::FieldAccess { receiver, .. } | Expr::IndexAccess { receiver, .. } => {
                self.through_temporary(receiver)
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } if self.is_builtin_method(receiver, method) => {
                if ELEMENT_READ_METHODS.contains(&method.as_str()) {
                    self.through_temporary(receiver)
                } else if method == "or" && args.len() == 1 {
                    self.through_temporary(receiver) || self.default_is_temporary(&args[0])
                } else {
                    false
                }
            }
            Expr::Call(_) | Expr::MethodCall { .. } => true,
            Expr::Wait(inner, _) => self.through_temporary(inner),
            _ => false,
        }
    }

    /// True when the default of an `.or` is a value built in place or a place read through one.
    fn default_is_temporary(&self, default: &Expr) -> bool {
        match default {
            Expr::Ident(..) => false,
            Expr::FieldAccess { .. } | Expr::IndexAccess { .. } => self.through_temporary(default),
            Expr::MethodCall { .. } if self.is_place_read(default) => {
                self.through_temporary(default)
            }
            _ => true,
        }
    }

    /// True when `value` bound under `annotation` takes a piece of a temporary
    /// (`is_taken_piece_of_a_temporary`, the one answer the checker's binding event reads too) that
    /// no name reaches. A call result that may be a piece of its arguments (a generic callee that
    /// returns one of them) is not a temporary to take from: the binding would own what a name
    /// still holds, so the read copies, and the checker binds it as a piece, not an owner.
    fn takes_a_piece_of_a_temporary(
        &self,
        value: &Expr,
        annotation: Option<&ynz_ast::nodes::Type>,
    ) -> bool {
        let type_of = |expr: &Expr| self.type_of(expr).cloned();
        let ctx = ProvenanceCtx::from_report(
            self.ownership,
            &self.local_fn_names,
            self.imported_fn_names,
            &type_of,
        );
        matches!(provenance(value, &ctx), Provenance::Reaches(roots) if roots.is_empty())
            && is_taken_piece_of_a_temporary(
            value,
            annotation,
            &|expr| self.type_of(expr).cloned(),
            &|name| self.ownership.is_local_fn(name) || self.imported_fn_names.contains(name),
        )
    }

    /// True when `method` names a built-in on `receiver`: a user function of that name does not
    /// shadow it unless the receiver is a shape or a `dynamic` value (`is_fresh_string`).
    fn is_builtin_method(&self, receiver: &Expr, method: &str) -> bool {
        let reaches_function = matches!(
            self.type_of(receiver),
            Some(Type::Shape { .. } | Type::Dynamic { .. })
        );
        !(reaches_function
            && (self.ownership.is_local_fn(method) || self.imported_fn_names.contains(method)))
    }

    /// True when `value` is a call whose heap result typeck cannot trace to a holder (`Provenance::Unknown`:
    /// an imported function that returns a `maybe` or a union, a function value, a `dynamic`
    /// dispatch): it may be a piece of an argument.
    fn is_untraceable_result(&self, value: &Expr) -> bool {
        // A string, array, map, shape, channel or `maybe` of a channel a function returns is an owned
        // copy (`copied_at_return`; a channel's copy is a counted reference, `ynz_channel_share`):
        // only another `maybe` or a union can still be a piece of an argument. A `maybe` or union
        // that holds a channel and a task handle are left alone (a union of a channel is item 7's
        // untagged form; no function returns a handle).
        if !matches!(value, Expr::Call(_) | Expr::MethodCall { .. })
            || !self.type_of(value).is_some_and(|ty| {
                matches!(ty, Type::Maybe { .. } | Type::Union { .. })
                    && (!value_like(ty)
                        || matches!(ty, Type::Maybe { inner } if matches!(inner.as_ref(), Type::String)))
                    && !holds_a_conduit(ty)
            })
        {
            return false;
        }
        let type_of = |expr: &Expr| self.type_of(expr).cloned();
        let ctx = ProvenanceCtx::from_report(
            self.ownership,
            &self.local_fn_names,
            self.imported_fn_names,
            &type_of,
        );
        match provenance(value, &ctx) {
            Provenance::Unknown => true,
            // A user function whose result may be one of its arguments (`pick(rows, other)`):
            // the same hazard, traced to its roots. A built-in element read (`rows.get(0)`) is a
            // place read, judged as a view or a copy by the place rules.
            Provenance::Reaches(roots) => !roots.is_empty() && self.is_user_function_call(value),
            Provenance::Fresh | Provenance::Whole(_) => false,
        }
    }

    /// True when `value` calls a Yinz function (local or imported), not a built-in method.
    fn is_user_function_call(&self, value: &Expr) -> bool {
        match value {
            Expr::Call(call) => matches!(&call.callee, Expr::Ident(name, _)
                if self.ownership.is_local_fn(name) || self.imported_fn_names.contains(name)),
            Expr::MethodCall { receiver, method, .. } => {
                (self.ownership.is_local_fn(method) || self.imported_fn_names.contains(method))
                    && !self.is_builtin_method(receiver, method)
            }
            _ => false,
        }
    }

    /// True when `value` reads a place that stays where it is: a field or element read, an
    /// element method of a built-in collection, or the `.or` of such a read.
    fn is_place_read(&self, value: &Expr) -> bool {
        match value {
            Expr::FieldAccess { .. } | Expr::IndexAccess { .. } => true,
            Expr::MethodCall {
                receiver, method, ..
            } if self.is_builtin_method(receiver, method) => {
                (ELEMENT_READ_METHODS.contains(&method.as_str())
                    && matches!(
                        self.type_of(receiver),
                        Some(
                            Type::BuiltinArray { .. }
                                | Type::BuiltinFixed { .. }
                                | Type::BuiltinMap { .. }
                        )
                    ))
                    || (method == "or"
                        && (self.is_place_read(receiver)
                            || matches!(receiver.as_ref(), Expr::Ident(..))))
            }
            Expr::Wait(inner, _) => self.is_place_read(inner),
            _ => false,
        }
    }

    /// Resolve the identifiers of a place to the roots they stand for and the views they are
    /// read through. `None` when one is not a parameter, a `let` or a live view: nothing the
    /// analysis can follow, so the read copies.
    fn resolve_roots(&self, idents: &[&Expr], lets: &HashSet<DeclKey>) -> Option<ResolvedPlace> {
        let mut roots: Vec<Root> = Vec::new();
        let mut through: Vec<DeclKey> = Vec::new();
        let add_root = |roots: &mut Vec<Root>, root: Root| {
            if !roots.contains(&root) {
                roots.push(root);
            }
        };
        for ident in idents {
            let Expr::Ident(name, span) = ident else {
                return None;
            };
            let key = self.names.declaration_of_use(span, name).cloned()?;
            let reader = self.candidates.iter().find(|candidate| {
                candidate.is_readable_through() && candidate.holder() == Some(&key)
            });
            match reader {
                // `let b = a.g` with `a` a view of `x`: `b` is a view of `x`, checked against `x`.
                Some(candidate) if candidate.viable => {
                    for root in &candidate.roots {
                        add_root(&mut roots, root.clone());
                    }
                    if !through.contains(&key) {
                        through.push(key);
                    }
                }
                // A view already disqualified owns a copy: it is a plain `let` root.
                Some(_) if lets.contains(&key) => add_root(
                    &mut roots,
                    Root {
                        key,
                        read_only_parameter: false,
                    },
                ),
                Some(_) => return None,
                None if self.parameters.contains(&key) => {
                    let read_only_parameter = self.read_only_parameters.contains(&key);
                    add_root(
                        &mut roots,
                        Root {
                            key,
                            read_only_parameter,
                        },
                    );
                }
                None if lets.contains(&key) => add_root(
                    &mut roots,
                    Root {
                        key,
                        read_only_parameter: false,
                    },
                ),
                None => return None,
            }
        }
        Some((roots, through))
    }

    // ── declaring readers ───────────────────────────────────────────────────────────────

    /// A `let name = value` was just evaluated. It becomes a candidate when `value` is a place
    /// read of a heap type whose annotation (if any) cannot wrap it into a union slot.
    pub(crate) fn declare_let(
        &mut self,
        name: &str,
        name_span: &ynz_diagnostics::SourceSpan,
        annotation: Option<&ynz_ast::nodes::Type>,
        value: &Expr,
        lets: &HashSet<DeclKey>,
    ) {
        let reader = Reader::Binding(decl_key(name_span, name));
        self.declare_binding(Some(reader), annotation, value, lets);
    }

    /// A reassignment `name = value` was just evaluated.
    pub(crate) fn declare_assign(
        &mut self,
        target: &str,
        target_span: &ynz_diagnostics::SourceSpan,
        value: &Expr,
        lets: &HashSet<DeclKey>,
    ) {
        let Some(key) = self.names.declaration_of_use(target_span, target).cloned() else {
            self.judge_unfollowable(value);
            return;
        };
        if !lets.contains(&key) {
            self.judge_unfollowable(value);
            return;
        }
        self.declare_binding(None, None, value, lets);
    }

    /// A judged place read that cannot be followed owns a copy.
    fn judge_unfollowable(&mut self, value: &Expr) {
        if self.is_place_read(value) && self.type_of(value).is_some_and(binding_holds_place) {
            let span = value.span();
            self.must_copy((span.start, span.end));
        }
    }

    /// One `let` (`Some(reader)`) or reassignment (`None`) of a binding. A reassignment never views:
    /// see the comment where it is judged.
    fn declare_binding(
        &mut self,
        reader: Option<Reader>,
        annotation: Option<&ynz_ast::nodes::Type>,
        value: &Expr,
        lets: &HashSet<DeclKey>,
    ) {
        if self.is_untraceable_result(value) {
            // An imported function that returns a `maybe` or a union may hand back a piece of an
            // argument: the binding cannot point at it (the argument's owner may die first) and
            // no name is known to hold it, so it owns a copy.
            let span = value.span();
            self.must_copy((span.start, span.end));
            return;
        }
        if !self.is_place_read(value) {
            return;
        }
        let span = value.span();
        let value_span = (span.start, span.end);
        let Some(value_type) = self.type_of(value) else {
            return;
        };
        // The binding outlives the temporary a piece is read from: it takes the piece (a string,
        // array or map field), or copies it.
        if self.takes_a_piece_of_a_temporary(value, annotation) {
            self.reads.verdicts.insert(value_span, ReadVerdict::Take);
            return;
        }
        if !binding_holds_place(value_type) {
            return;
        }
        if self.through_temporary(value) {
            self.must_copy(value_span);
            return;
        }
        // An annotation can wrap the value into a union or `maybe` slot, which a view cannot be;
        // `array<T>` and `map<K, V>` annotations name the type the value already has.
        let annotation_keeps_the_value = match (annotation, value_type) {
            (None, _) => true,
            (
                Some(ynz_ast::nodes::Type::Generic { name, .. }),
                Type::BuiltinArray { .. } | Type::BuiltinMap { .. },
            ) => name == "array" || name == "map",
            _ => false,
        };
        if !annotation_keeps_the_value {
            self.must_copy(value_span);
            return;
        }
        // A reassigned binding copies at every event. Its `let` is a view only while nothing writes the
        // binding, and the reassignment is such a write, so the `let` owns a copy (or a fresh value);
        // a reassignment that viewed would make the binding hold a place it does not own beside a value
        // it does, and the release of the first value would be lost (an annotated `maybe<Tag>` that
        // is reassigned from `tags.get(i)` leaked its first payload). It also outlives the iteration it
        // is assigned in, which a view read through a loop variable of per-site staging does not
        // (`m5_p3_mapshape_iter_escape`).
        let Some(reader) = reader else {
            self.must_copy(value_span);
            return;
        };
        let Some((roots, through)) = self
            .place_roots(value)
            .and_then(|idents| self.resolve_roots(&idents, lets))
        else {
            self.must_copy(value_span);
            return;
        };
        self.candidates.push(Candidate {
            reader,
            roots,
            value_span,
            through,
            viable: true,
            body_active: false,
        });
    }

    /// A `for (var in iter)` is about to walk its body. The loop variable reads an element of
    /// what `iter` names; it is a view while nothing in the body writes the iterated place.
    pub(crate) fn begin_loop(
        &mut self,
        var: &str,
        var_span: &ynz_diagnostics::SourceSpan,
        iter: &Expr,
        lets: &HashSet<DeclKey>,
    ) {
        let Some(iter_type) = self.type_of(iter) else {
            return;
        };
        let element_holds_heap = match iter_type {
            Type::BuiltinArray { elem } | Type::BuiltinFixed { elem, .. } => {
                reader_holds_heap(elem)
            }
            Type::BuiltinMap { key, val } => reader_holds_heap(key) || reader_holds_heap(val),
            _ => false,
        };
        let span = iter.span();
        let value_span = (span.start, span.end);
        if !element_holds_heap {
            return;
        }
        let followed = match iter {
            Expr::Ident(..) => Some(vec![iter]),
            other if self.is_place_read(other) => self.place_roots(other),
            // A built value (a call result, a literal) is held by no name.
            Expr::Call(_) | Expr::ArrayLit { .. } | Expr::MapLit { .. } => Some(Vec::new()),
            _ => None,
        };
        let Some((roots, through)) = followed.and_then(|idents| self.resolve_roots(&idents, lets))
        else {
            self.must_copy(value_span);
            return;
        };
        self.open_loops.push(self.candidates.len());
        self.candidates.push(Candidate {
            reader: Reader::LoopVariable(decl_key(var_span, var)),
            roots,
            value_span,
            through,
            viable: true,
            body_active: true,
        });
    }

    /// A block was walked: the `let`s in `dying` (declared directly in it) die here, and `inside`
    /// holds every `let` declared anywhere in it. A reader that is a `let` declared outside the
    /// block and points into one that dies would point at a released value once the scope exit
    /// frees it, so it owns a copy instead: a read must not outlive its root (Golden Rule 8: this
    /// is the one case where a read with no write still copies, pinned by
    /// `v03_m9_c6_borrow_cost`). O(c) for the c candidates.
    pub(crate) fn end_block(
        &mut self,
        dying: &HashSet<DeclKey>,
        inside: &HashSet<DeclKey>,
        lets: &HashSet<DeclKey>,
    ) {
        for candidate in &mut self.candidates {
            if !candidate.viable {
                continue;
            }
            let outlives = match &candidate.reader {
                Reader::Binding(holder) => {
                    lets.contains(holder) && !inside.contains(holder)
                }
                Reader::LoopVariable(_) | Reader::Argument => false,
            };
            if outlives && candidate.roots.iter().any(|root| dying.contains(&root.key)) {
                candidate.viable = false;
            }
        }
    }

    /// The body of the innermost loop `begin_loop` opened (when it judged one) was walked.
    pub(crate) fn end_loop(&mut self, var_span: &ynz_diagnostics::SourceSpan, var: &str) {
        let key = decl_key(var_span, var);
        if let Some(&index) = self.open_loops.last() {
            if self.candidates[index].holder() == Some(&key) {
                self.candidates[index].body_active = false;
                self.open_loops.pop();
            }
        }
    }

    /// Judge the call arguments inside `expr`, a statement's own expression.
    pub(crate) fn judge_calls(&mut self, expr: &Expr, lets: &HashSet<DeclKey>) {
        match expr {
            // A `background` task takes a copy of every argument: nothing to judge at the spawn.
            Expr::Background(inner, _) => {
                for child in children(inner) {
                    self.judge_calls(child, lets);
                }
            }
            _ => {
                self.judge_call_node(expr, lets);
                for child in children(expr) {
                    self.judge_calls(child, lets);
                }
            }
        }
    }

    /// One call of a Yinz function: each argument at a position the callee only reads, that
    /// reads a place, is a view unless another argument of the call may write that place.
    fn judge_call_node(&mut self, expr: &Expr, lets: &HashSet<DeclKey>) {
        let (name, arguments, is_method): (&str, Vec<&Expr>, bool) = match expr {
            Expr::Call(call) => match &call.callee {
                Expr::Ident(name, _)
                    if self.ownership.is_local_fn(name)
                        || self.imported_fn_names.contains(name.as_str()) =>
                {
                    (name.as_str(), call.args.iter().collect(), false)
                }
                _ => return,
            },
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } if (self.ownership.is_local_fn(method)
                || self.imported_fn_names.contains(method.as_str()))
                && !self.is_builtin_method(receiver, method) =>
            {
                let mut all: Vec<&Expr> = vec![receiver.as_ref()];
                all.extend(args.iter());
                (method.as_str(), all, true)
            }
            _ => return,
        };
        let reads = |position: usize| {
            call_position_reads(
                self.ownership,
                self.imported_fn_names,
                name,
                position,
                is_method,
            )
        };
        let mut judged: Vec<((usize, usize), Option<ResolvedPlace>)> = Vec::new();
        for (position, argument) in arguments.iter().enumerate() {
            if !reads(position)
                || !self.is_place_read(argument)
                || !self.type_of(argument).is_some_and(reader_holds_heap)
            {
                continue;
            }
            let span = argument.span();
            let followed = self
                .place_roots(argument)
                .and_then(|idents| self.resolve_roots(&idents, lets));
            judged.push(((span.start, span.end), followed));
            let Some(last) = judged.last_mut() else {
                continue;
            };
            let Some((roots, _)) = &last.1 else {
                continue;
            };
            let conflict = roots.iter().any(|root| {
                !root.read_only_parameter
                    && arguments.iter().enumerate().any(|(other, other_argument)| {
                        other != position
                            && self.argument_changes(other_argument, other, &root.key, &reads)
                    })
            });
            if conflict {
                last.1 = None;
            }
        }
        for (value_span, followed) in judged {
            match followed {
                Some((roots, through)) => self.candidates.push(Candidate {
                    reader: Reader::Argument,
                    roots,
                    value_span,
                    through,
                    viable: true,
                    body_active: false,
                }),
                None => self.must_copy(value_span),
            }
        }
    }

    /// True when evaluating `argument`, at call position `position`, may write what `root`
    /// names: the root passed to a position the callee may write, or any write inside it.
    fn argument_changes(
        &self,
        argument: &Expr,
        position: usize,
        root: &DeclKey,
        reads: &dyn Fn(usize) -> bool,
    ) -> bool {
        let is_root_place = self.holds_boxes(argument)
            && matches!(
                place_root(argument),
                Some(Expr::Ident(name, span))
                    if self.names.declaration_of_use(span, name) == Some(root)
            );
        if is_root_place {
            return !reads(position);
        }
        self.changes(argument, root)
    }

    // ── evaluations ─────────────────────────────────────────────────────────────────────

    /// The expressions `exprs` are evaluated while `after` (the `let`s read later) and the
    /// `mentioned` lets of these expressions say which readers are live. Disqualify every live
    /// reader that is written, escapes, or whose root may change here, and every one live across
    /// a `wait` or `background`.
    pub(crate) fn evaluate(
        &mut self,
        exprs: &[&Expr],
        mentioned: &HashSet<DeclKey>,
        after: &HashSet<DeclKey>,
    ) {
        for index in 0..self.candidates.len() {
            let candidate = &self.candidates[index];
            if !candidate.viable || !candidate.live(mentioned, after) {
                continue;
            }
            let Some(holder) = candidate.holder().cloned() else {
                continue;
            };
            let suspension_breaks = candidate.suspension_breaks();
            let roots: Vec<DeclKey> = candidate
                .roots
                .iter()
                .filter(|root| !root.read_only_parameter)
                .map(|root| root.key.clone())
                .collect();
            let disqualified = exprs.iter().any(|expr| {
                (suspension_breaks && suspends(expr))
                    || self.changes(expr, &holder)
                    || roots.iter().any(|root| self.changes(expr, root))
            });
            if disqualified {
                self.candidates[index].viable = false;
            }
        }
    }

    /// A statement writes through `target` (`name.f = …`, `name[i] = …`) while `after` /
    /// `mentioned` say which readers are live. A target whose base is not an identifier
    /// (`make().f = …`, `arr.get(i).f = …`) names no binding the analysis can compare with a
    /// reader or its roots, so every live reader is disqualified, as for an unresolved name in
    /// `evaluate_write_to`: an unproven write must keep the copy.
    pub(crate) fn evaluate_write(
        &mut self,
        target: &Expr,
        mentioned: &HashSet<DeclKey>,
        after: &HashSet<DeclKey>,
    ) {
        let Some(Expr::Ident(name, span)) = place_root(target) else {
            self.disqualify_live_readers(mentioned, after);
            return;
        };
        self.evaluate_write_to(name, span, mentioned, after);
    }

    /// Disqualify every candidate that is live (`after`) or mentioned by the statement.
    fn disqualify_live_readers(&mut self, mentioned: &HashSet<DeclKey>, after: &HashSet<DeclKey>) {
        for candidate in &mut self.candidates {
            if candidate.live(mentioned, after) {
                candidate.viable = false;
            }
        }
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
            if !candidate.viable || !candidate.live(mentioned, after) {
                continue;
            }
            let hits_root = candidate
                .roots
                .iter()
                .any(|root| !root.read_only_parameter && written.as_ref() == Some(&root.key));
            // An unresolved target could be anything: the reader is not proven.
            if written.is_none() || written.as_ref() == candidate.holder() || hits_root {
                candidate.viable = false;
            }
        }
    }

    /// A `return value` while readers are live: a returned reader (or a place read through one)
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
            if candidate.holder() == Some(&returned) {
                candidate.viable = false;
            }
        }
    }

    /// False when `expr` is typed as bits (an `int`, a `string`, a `maybe` of those): copying it
    /// is the value, so it shares nothing. An expression with no recorded type holds boxes.
    fn holds_boxes(&self, expr: &Expr) -> bool {
        !self.type_of(expr).is_some_and(value_like)
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

/// True when `ty` is or holds a channel or a task handle.
fn holds_a_conduit(ty: &Type) -> bool {
    match ty {
        Type::BuiltinChannel { .. } | Type::BackgroundHandle { .. } => true,
        Type::Maybe { inner } => holds_a_conduit(inner),
        Type::Union { variants } => variants.iter().any(holds_a_conduit),
        _ => false,
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
