//! The persisted scope-exit release plan (v0.3-M9 C5, plan #17; `IMP-ownership.md` "The
//! persisted release plan").
//!
//! Heap locals are released by ownership when their scope exits. WHICH binding releases WHAT, on
//! WHICH exit edge, is decided here, once, by typeck; codegen (C6/C7) reads the record and derives
//! no transfer fact of its own (`.claude/rules/authoritative-derivation.md`).
//!
//! # What is recorded
//!
//! One TOTAL list per exit edge: every binding of a heap-capable type that leaves scope at that
//! edge appears, each with its [`Fate`]. Edges, keyed by source span:
//!
//! | edge | key | bindings listed |
//! |---|---|---|
//! | end of a block (a function body, an `if` body, a loop body per iteration, a match arm) | the `Block` span | the block's own frame, last declared first |
//! | a `return` | the `Stmt::Return` span | every open frame, innermost first |
//! | errors auto-propagation | the span of the identifier read that fires it | every open frame, innermost first |
//!
//! A binding whose type holds no heap (`int`, `boolean`, `options`, ...) is not listed.
//!
//! # Where each fate comes from (one source per fact)
//!
//! * **Owner** (who may release): the C5a transfer record's anchor ([`ReleaseAnchor`]). Only the
//!   class founder releases; every other member is `Omitted(NotOwner)`.
//! * **Moved / returned on this path**: the live scope at the edge (`Scope::consumed_classes`,
//!   `ScopeEntry::consumed_on_every_path`), the same state `check_transfer` reads, plus the
//!   returned value's holders. A class given, sent or stored on every path is
//!   `Omitted(Moved)`; on some paths only, `ReleaseIfLive` (the move site nulls the founder's
//!   slot, [`MoveSite`]).
//! * **Strings**: `TypedModule::string_moves` (C3b). A string is never statically moved: that record
//!   says a read MAY be taken by a store, and codegen decides at the store. A `let` string with
//!   a record-listed last-use read that can precede the edge is `ReleaseIfLive`, else `Release`; the
//!   listed reads are its [`MoveSite`]s. Plan and record are one fact, not two.
//!
//! # Decisions in force
//!
//! * A binding reassigned anywhere in its function is `Omitted(Reassigned)`: the old value is
//!   leaked, never freed early.
//! * A type this plan has no release for (`maybe`/`union`/`dynamic`/`fixed` of heap, errors-capable
//!   values, nested shapes, generic instances) is `Omitted(UnsupportedType)`; a value of a type
//!   parameter is `Omitted(TypeParameter)` (a generic body is checked once, with the parameter
//!   unresolved, and its lists apply to every instance).
//! * The release owners (bindings with a releasing fate at some edge) feed the ONE crossing-set
//!   producer: a release is a read at its edge, see [`ReleaseReads`].

use std::collections::{BTreeSet, HashMap, HashSet};

use ynz_ast::nodes::{Block, Expr, Stmt};
use ynz_diagnostics::SourceSpan;

use crate::binding_names::{BindingNames, DeclKey};
use crate::shapes::ShapeTable;
use crate::string_ownership::StringMoves;
use crate::transfer_record::{ReleaseAnchor, TransferRecord};
use crate::types::Type;

/// `(span.start, span.end)`.
pub type Span = (usize, usize);

/// What the release frees, by runtime mechanism.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseKind {
    /// A heap or static string (`ynz_string_release` accepts any).
    String,
    /// `array<T>`.
    Array,
    /// `map<K, V>`.
    Map,
    /// `channel<T>`.
    Channel,
    /// A background task handle (releasing it stops the task).
    TaskHandle,
    /// A flat shape (no shape-typed field) with heap fields.
    Shape,
}

/// Why a binding is not released at an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OmitReason {
    /// Its class was given, sent or stored on EVERY path to this edge.
    Moved,
    /// The returned value is, or reaches, its class.
    Returned,
    /// Another member of its class is the founder, or no member owns a fresh value.
    NotOwner,
    /// A `share`/`lend`/bare parameter is in its class: the caller owns the value.
    CallerOwns,
    /// A non-owner outlives every owner (`NoStaticFounder`): C7 decides with a run-time flag.
    NoStaticFounder,
    /// The binding is reassigned somewhere in its function.
    Reassigned,
    /// The type holds heap this plan has no release for.
    UnsupportedType,
    /// The type is a type parameter of a generic function.
    TypeParameter,
}

/// What happens to a binding at one exit edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fate {
    /// Sole owner, not moved on any path to this edge: release it.
    Release(ReleaseKind),
    /// Moved on SOME paths: release it if its slot is non-null (every [`MoveSite`] nulls the
    /// founder's slot).
    ReleaseIfLive(ReleaseKind),
    Omitted(OmitReason),
}

impl Fate {
    /// True when codegen emits a release for this fate.
    pub fn releases(&self) -> bool {
        matches!(self, Fate::Release(_) | Fate::ReleaseIfLive(_))
    }
}

/// One binding at one edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseEntry {
    pub decl: DeclKey,
    /// The unique name codegen's AST carries (`BindingNames`).
    pub codegen_name: String,
    pub fate: Fate,
}

/// A read that moves a value out of its founder: codegen nulls the founder's slot right after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveSite {
    pub founder: DeclKey,
    pub founder_codegen_name: String,
}

/// Which kind of exit an edge is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EdgeKind {
    BlockEnd,
    Return,
    Propagation,
}

/// Whether a binding was given away on the path to an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moved {
    No,
    EveryPath,
    SomePaths,
}

// ── What a type holds ───────────────────────────────────────────────────────────────────────

/// What a binding of some type needs at scope exit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeapClass {
    /// Bits only: nothing to release, not listed.
    NotHeap,
    Supported(ReleaseKind),
    Unsupported,
    TypeParameter,
}

/// The release class of a binding's type. O(fields) for a shape, O(1) otherwise.
pub fn heap_class_of(ty: &Type, shapes: &ShapeTable) -> HeapClass {
    match ty {
        Type::Nothing
        | Type::Error
        | Type::Int
        | Type::Float
        | Type::Number { .. }
        | Type::Bool
        | Type::Range { .. }
        | Type::Options { .. } => HeapClass::NotHeap,
        Type::String => HeapClass::Supported(ReleaseKind::String),
        Type::Sensitive { inner } => heap_class_of(inner, shapes),
        Type::BuiltinArray { .. } => HeapClass::Supported(ReleaseKind::Array),
        Type::BuiltinMap { .. } => HeapClass::Supported(ReleaseKind::Map),
        Type::BuiltinChannel { .. } => HeapClass::Supported(ReleaseKind::Channel),
        Type::BackgroundHandle { .. } => HeapClass::Supported(ReleaseKind::TaskHandle),
        Type::TypeParam { .. } => HeapClass::TypeParameter,
        Type::Shape { name } => match shapes.shapes.get(name) {
            Some(definition) => {
                let field_classes: Vec<HeapClass> = definition
                    .fields
                    .iter()
                    .map(|field| heap_class_of(&field.ty, shapes))
                    .collect();
                if field_classes
                    .iter()
                    .all(|class| matches!(class, HeapClass::NotHeap))
                {
                    HeapClass::NotHeap
                } else if definition
                    .fields
                    .iter()
                    .any(|field| matches!(field.ty, Type::Shape { .. }))
                {
                    // A nested shape cannot be frame-embedded across a suspension (Check 2).
                    HeapClass::Unsupported
                } else {
                    HeapClass::Supported(ReleaseKind::Shape)
                }
            }
            // Imported or anonymous: the table does not describe it.
            None => HeapClass::Unsupported,
        },
        Type::Maybe { inner } => match heap_class_of(inner, shapes) {
            HeapClass::NotHeap => HeapClass::NotHeap,
            _ => HeapClass::Unsupported,
        },
        Type::Dynamic { .. }
        | Type::Generic { .. }
        | Type::BuiltinFixed { .. }
        | Type::MapEntry { .. }
        | Type::Union { .. }
        | Type::ErrorsCapable { .. } => HeapClass::Unsupported,
    }
}

// ── The walk-time record ────────────────────────────────────────────────────────────────────

/// One binding at one edge, as the live scope shows it.
#[derive(Clone, Debug)]
pub struct RawEntry {
    pub decl: DeclKey,
    pub heap: HeapClass,
    /// True for a `let`/`const` string: the only strings that own their bytes.
    pub string_owner: bool,
    /// The binding's alias class in the live scope at the edge.
    pub class: u64,
    pub moved: Moved,
    /// The returned value is, or reaches, this binding's class.
    pub returned: bool,
}

#[derive(Clone, Debug)]
struct RawEdge {
    kind: EdgeKind,
    span: Span,
    consumed_classes: BTreeSet<u64>,
    entries: Vec<RawEntry>,
}

#[derive(Clone, Debug)]
struct RawMoveSite {
    span: Span,
    named: DeclKey,
}

#[derive(Clone, Debug)]
struct FunctionExits {
    name: String,
    file: String,
    body_span: Span,
    edges: Vec<RawEdge>,
    move_sites: Vec<RawMoveSite>,
    reassigned: HashSet<DeclKey>,
}

/// The names a list is written in: what the user wrote, or the unique codegen name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Names {
    User,
    Codegen,
}

/// The reads the release plan adds to a function: one per owner per edge, by edge key, in ONE
/// name space. The crossing-set producer adds these as reads at the edges (a release after a
/// suspension IS a use after it), so the owners cross exactly when the release does and the
/// suspension detection stays the single one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReleaseReads {
    file: String,
    body_span: Span,
    block_end: HashMap<Span, Vec<String>>,
    return_edge: HashMap<Span, Vec<String>>,
    propagation_edge: HashMap<Span, Vec<String>>,
}

impl ReleaseReads {
    /// True when no edge releases anything.
    pub fn is_empty(&self) -> bool {
        self.block_end.is_empty() && self.return_edge.is_empty() && self.propagation_edge.is_empty()
    }

    /// Every owner name, sorted and deduplicated.
    pub fn owner_names(&self) -> Vec<String> {
        let mut names: BTreeSet<String> = BTreeSet::new();
        for list in self
            .block_end
            .values()
            .chain(self.return_edge.values())
            .chain(self.propagation_edge.values())
        {
            names.extend(list.iter().cloned());
        }
        names.into_iter().collect()
    }

    /// `body` (a function body's statements) with a read of every owner appended at each edge:
    /// at the end of its block, after its `return`, after the statement holding the identifier
    /// that fires a propagation. The reads are `Stmt::Expr(Ident)` with a span no source
    /// expression has (a propagation read is zero-width at the end of the identifier).
    ///
    /// O(n + e) time and memory for n statements and e edges.
    pub fn with_reads(&self, body: &[Stmt]) -> Vec<Stmt> {
        let mut context = ReadsContext {
            reads: self,
            file: &self.file,
        };
        context.statements(body, Some(self.body_span))
    }
}

struct ReadsContext<'a> {
    reads: &'a ReleaseReads,
    file: &'a str,
}

fn span_of(span: &SourceSpan) -> Span {
    (span.start, span.end)
}

fn stmt_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::Expr(expr) => span_of(expr.span()),
        Stmt::Let { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::If { span, .. }
        | Stmt::Match { span, .. }
        | Stmt::While { span, .. }
        | Stmt::For { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::FieldAssign { span, .. }
        | Stmt::IndexAssign { span, .. } => span_of(span),
    }
}

fn nested_blocks(stmt: &Stmt) -> Vec<&Block> {
    match stmt {
        Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => vec![body],
        Stmt::Match { arms, else_arm, .. } => arms
            .iter()
            .map(|arm| &arm.body)
            .chain(else_arm.iter())
            .collect(),
        _ => Vec::new(),
    }
}

fn encloses(outer: Span, inner: Span) -> bool {
    outer.0 <= inner.0 && inner.1 <= outer.1
}

impl ReadsContext<'_> {
    fn read_of(&self, name: &str, span: Span) -> Stmt {
        Stmt::Expr(Expr::Ident(
            name.to_string(),
            SourceSpan::new(self.file, span.0, span.1),
        ))
    }

    fn statements(&mut self, stmts: &[Stmt], block_span: Option<Span>) -> Vec<Stmt> {
        let mut out: Vec<Stmt> = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            out.push(self.statement(stmt));
            let this_span = stmt_span(stmt);
            if let Stmt::Return { .. } = stmt {
                if let Some(names) = self.reads.return_edge.get(&this_span) {
                    for name in names {
                        out.push(self.read_of(name, this_span));
                    }
                }
            }
            // A propagation fires inside the statement's own expressions, not in a nested block
            // (the nested block's statements get their own pass).
            let nested = nested_blocks(stmt);
            let mut fired: Vec<(&Span, &Vec<String>)> = self
                .reads
                .propagation_edge
                .iter()
                .filter(|(key, _)| {
                    // Keys are zero-width reads at the end of the identifier; the identifier ends there.
                    encloses(this_span, (key.0, key.1))
                        && !nested
                            .iter()
                            .any(|block| encloses(span_of(&block.span), (key.0, key.1)))
                })
                .collect();
            fired.sort();
            for (key, names) in fired {
                for name in names {
                    out.push(self.read_of(name, (key.1, key.1)));
                }
            }
        }
        if let Some(span) = block_span {
            if let Some(names) = self.reads.block_end.get(&span) {
                for name in names {
                    out.push(self.read_of(name, (span.1, span.1)));
                }
            }
        }
        out
    }

    fn block(&mut self, block: &Block) -> Block {
        Block {
            stmts: self.statements(&block.stmts, Some(span_of(&block.span))),
            span: block.span.clone(),
        }
    }

    fn statement(&mut self, stmt: &Stmt) -> Stmt {
        match stmt {
            Stmt::If { cond, body, span } => Stmt::If {
                cond: cond.clone(),
                body: self.block(body),
                span: span.clone(),
            },
            Stmt::While { cond, body, span } => Stmt::While {
                cond: cond.clone(),
                body: self.block(body),
                span: span.clone(),
            },
            Stmt::For {
                var,
                var_span,
                iter,
                body,
                span,
                destructure_pattern,
                map_destructure_pattern,
            } => Stmt::For {
                var: var.clone(),
                var_span: var_span.clone(),
                iter: iter.clone(),
                body: self.block(body),
                span: span.clone(),
                destructure_pattern: destructure_pattern.clone(),
                map_destructure_pattern: map_destructure_pattern.clone(),
            },
            Stmt::Match {
                scrutinee,
                arms,
                else_arm,
                span,
            } => Stmt::Match {
                scrutinee: scrutinee.clone(),
                arms: arms
                    .iter()
                    .map(|arm| ynz_ast::nodes::MatchArm {
                        pattern: arm.pattern.clone(),
                        body: self.block(&arm.body),
                        arrow_span: arm.arrow_span.clone(),
                    })
                    .collect(),
                else_arm: else_arm.as_ref().map(|block| self.block(block)),
                span: span.clone(),
            },
            other => other.clone(),
        }
    }
}

// ── Recording ───────────────────────────────────────────────────────────────────────────────

/// Fills a [`ReleasePlan`] while the checker walks the module, one function at a time. Mirrors
/// `TransferRecorder`: the checker calls it at the points that already hold the facts.
#[derive(Default, Clone)]
pub struct ReleaseRecorder {
    current: Option<FunctionExits>,
    finished: Vec<FunctionExits>,
    /// What each finished function's early reads were, to compare with the final ones.
    early_reads: HashMap<String, ReleaseReads>,
    internal_errors: Vec<String>,
}

impl ReleaseRecorder {
    /// Start walking the function `name` whose body block has span `body_span`; closes the
    /// previous one.
    pub fn begin_function(&mut self, name: &str, body_span: &SourceSpan) {
        self.close_current();
        self.current = Some(FunctionExits {
            name: name.to_string(),
            file: body_span.file.clone(),
            body_span: span_of(body_span),
            edges: Vec::new(),
            move_sites: Vec::new(),
            reassigned: HashSet::new(),
        });
    }

    fn close_current(&mut self) {
        if let Some(previous) = self.current.take() {
            self.finished.push(previous);
        }
    }

    fn outside_function(&mut self, event: &str) {
        self.internal_errors.push(format!(
            "internal: the release plan was told {event} while no function was being walked"
        ));
    }

    /// An exit edge: the bindings leaving scope and the live scope's consumed classes.
    pub fn note_exit(
        &mut self,
        kind: EdgeKind,
        span: &SourceSpan,
        consumed_classes: BTreeSet<u64>,
        entries: Vec<RawEntry>,
    ) {
        let Some(function) = self.current.as_mut() else {
            self.outside_function("an exit edge");
            return;
        };
        let key = span_of(span);
        if function
            .edges
            .iter()
            .any(|edge| edge.kind == kind && edge.span == key)
        {
            // A node walked twice records its edge once, as the other recorders do.
            return;
        }
        function.edges.push(RawEdge {
            kind,
            span: key,
            consumed_classes,
            entries,
        });
    }

    /// A read at `span` handed the class `named` belongs to over (a give, a send, a store).
    pub fn note_move_site(&mut self, span: &SourceSpan, named: DeclKey) {
        let Some(function) = self.current.as_mut() else {
            self.outside_function("a move site");
            return;
        };
        let key = span_of(span);
        if !function.move_sites.iter().any(|site| site.span == key) {
            function.move_sites.push(RawMoveSite { span: key, named });
        }
    }

    /// The binding `decl` is reassigned.
    pub fn note_reassign(&mut self, decl: DeclKey) {
        let Some(function) = self.current.as_mut() else {
            self.outside_function("a reassignment");
            return;
        };
        function.reassigned.insert(decl);
    }

    /// The current function's walk is over: its release reads in the user's names, computed from
    /// `anchors` (`TransferRecorder::current_anchors`). The suspension guards run on these, before
    /// the module's records are finished.
    pub fn end_function(&mut self, anchors: &HashMap<DeclKey, ReleaseAnchor>) -> ReleaseReads {
        let Some(function) = self.current.clone() else {
            self.outside_function("the end of a function");
            return ReleaseReads::default();
        };
        let lookup = |decl: &DeclKey| anchors.get(decl).cloned();
        match resolve_function(&function, &lookup, None, &|decl: &DeclKey| decl.2.clone()) {
            Ok(resolved) => {
                let reads = resolved.reads;
                self.early_reads.insert(function.name.clone(), reads.clone());
                reads
            }
            Err(message) => {
                self.internal_errors.push(message);
                ReleaseReads::default()
            }
        }
    }

    /// Close the last function and build the plan. Codegen names come from `names`, anchors from
    /// `transfers`, string moves from `strings`. A hole is kept in `ReleasePlan::internal_errors`,
    /// never skipped.
    ///
    /// O(b · e) time for b bindings and e edges per function.
    pub fn finish(
        mut self,
        names: &BindingNames,
        transfers: &TransferRecord,
        strings: &StringMoves,
    ) -> ReleasePlan {
        self.close_current();
        let mut plan = ReleasePlan {
            internal_errors: self.internal_errors,
            ..ReleasePlan::default()
        };
        let mut moves_by_binding: HashMap<DeclKey, Vec<Span>> = HashMap::new();
        for (span, decl) in strings.last_uses() {
            moves_by_binding.entry(decl.clone()).or_default().push(span);
        }
        for function in &self.finished {
            let lookup = |decl: &DeclKey| {
                let codegen_name = names.codegen_name(decl)?;
                transfers
                    .function(&function.name)?
                    .class_of(codegen_name)
                    .map(|class| class.anchor.clone())
            };
            let codegen_name_of = |decl: &DeclKey| {
                names
                    .codegen_name(decl)
                    .map_or_else(|| decl.2.clone(), str::to_string)
            };
            let resolved = resolve_function(
                function,
                &lookup,
                Some(&moves_by_binding),
                &codegen_name_of,
            );
            let user_reads = resolve_function(
                function,
                &lookup,
                Some(&moves_by_binding),
                &|decl: &DeclKey| decl.2.clone(),
            );
            match (resolved, user_reads) {
                (Ok(resolved), Ok(user_reads)) => {
                    // The early pass wrote the owners in the user's names; the final pass must
                    // agree with it, or the guards and the frame layout read different sets.
                    if self.early_reads.get(&function.name) != Some(&user_reads.reads) {
                        plan.internal_errors.push(format!(
                            "internal: the release owners of `{}` changed between the end of \
                             its walk and the end of the module",
                            function.name
                        ));
                    }
                    plan.absorb(function, resolved, user_reads.reads, &lookup, names, strings);
                }
                (Err(message), _) | (_, Err(message)) => plan.internal_errors.push(message),
            }
        }
        plan
    }
}

struct ResolvedFunction {
    /// Per edge, in walk order.
    edges: Vec<(EdgeKind, Span, Vec<(DeclKey, Fate)>)>,
    /// The reads in the user's names (early) or the codegen names (final), per `codegen_name_of`.
    reads: ReleaseReads,
}

/// The fate of every listed binding at every edge of `function`, and the reads they imply.
/// `moves_by_binding` is `None` for the early pass, which does not know the string moves yet; a
/// string then reads as `Release`, which reads exactly as `ReleaseIfLive` does.
fn resolve_function(
    function: &FunctionExits,
    anchor_of: &dyn Fn(&DeclKey) -> Option<ReleaseAnchor>,
    moves_by_binding: Option<&HashMap<DeclKey, Vec<Span>>>,
    name_of: &dyn Fn(&DeclKey) -> String,
) -> Result<ResolvedFunction, String> {
    let mut edges = Vec::with_capacity(function.edges.len());
    let mut reads = ReleaseReads {
        file: function.file.clone(),
        body_span: function.body_span,
        ..ReleaseReads::default()
    };
    for edge in &function.edges {
        let mut fates = Vec::with_capacity(edge.entries.len());
        let mut owners: Vec<String> = Vec::new();
        for entry in &edge.entries {
            let fate = fate_of(function, edge, entry, anchor_of, moves_by_binding)?;
            if fate.releases() {
                owners.push(name_of(&entry.decl));
            }
            fates.push((entry.decl.clone(), fate));
        }
        if !owners.is_empty() {
            let target = match edge.kind {
                EdgeKind::BlockEnd => &mut reads.block_end,
                EdgeKind::Return => &mut reads.return_edge,
                EdgeKind::Propagation => &mut reads.propagation_edge,
            };
            target.insert(edge.span, owners);
        }
        edges.push((edge.kind, edge.span, fates));
    }
    Ok(ResolvedFunction { edges, reads })
}

/// THE fate rule. One function, read by the early and the final pass alike.
fn fate_of(
    function: &FunctionExits,
    edge: &RawEdge,
    entry: &RawEntry,
    anchor_of: &dyn Fn(&DeclKey) -> Option<ReleaseAnchor>,
    moves_by_binding: Option<&HashMap<DeclKey, Vec<Span>>>,
) -> Result<Fate, String> {
    let kind = match entry.heap {
        HeapClass::NotHeap => {
            return Err(format!(
                "internal: `{}` in `{}` is listed at an exit edge but holds no heap",
                entry.decl.2, function.name
            ))
        }
        HeapClass::Unsupported => return Ok(Fate::Omitted(OmitReason::UnsupportedType)),
        HeapClass::TypeParameter => return Ok(Fate::Omitted(OmitReason::TypeParameter)),
        HeapClass::Supported(kind) => kind,
    };
    if function.reassigned.contains(&entry.decl) {
        return Ok(Fate::Omitted(OmitReason::Reassigned));
    }
    if kind == ReleaseKind::String {
        // A string owns its bytes whatever else it is joined to (a store copies or moves).
        if !entry.string_owner {
            return Ok(Fate::Omitted(OmitReason::NotOwner));
        }
        let edge_end = edge.span.1;
        let may_have_moved = moves_by_binding
            .and_then(|moves| moves.get(&entry.decl))
            .is_some_and(|spans| spans.iter().any(|span| span.0 < edge_end));
        return Ok(if may_have_moved {
            Fate::ReleaseIfLive(kind)
        } else {
            Fate::Release(kind)
        });
    }
    let Some(anchor) = anchor_of(&entry.decl) else {
        return Err(format!(
            "internal: `{}` in `{}` has no release anchor in the transfer record",
            entry.decl.2, function.name
        ));
    };
    match anchor {
        ReleaseAnchor::CallerOwns { .. } => return Ok(Fate::Omitted(OmitReason::CallerOwns)),
        ReleaseAnchor::NoOwner => return Ok(Fate::Omitted(OmitReason::NotOwner)),
        ReleaseAnchor::NoStaticFounder(_) => {
            return Ok(Fate::Omitted(OmitReason::NoStaticFounder))
        }
        ReleaseAnchor::Founder { decl, .. } if decl != entry.decl => {
            return Ok(Fate::Omitted(OmitReason::NotOwner))
        }
        ReleaseAnchor::Founder { .. } => {}
    }
    if entry.returned {
        return Ok(Fate::Omitted(OmitReason::Returned));
    }
    Ok(match entry.moved {
        Moved::No => Fate::Release(kind),
        Moved::SomePaths => Fate::ReleaseIfLive(kind),
        Moved::EveryPath => Fate::Omitted(OmitReason::Moved),
    })
}

// ── The finished plan ───────────────────────────────────────────────────────────────────────

/// What the walk saw at one edge, kept beside the fates it produced so the plan can be checked
/// against the live scope and the transfer record ([`ReleasePlan::parity_violations`]).
#[derive(Clone, Debug)]
pub struct EdgeFacts {
    pub function: String,
    pub kind: EdgeKind,
    pub span: Span,
    /// `Scope::consumed_classes` at the edge.
    pub consumed_classes: BTreeSet<u64>,
    pub entries: Vec<EntryFacts>,
}

#[derive(Clone, Debug)]
pub struct EntryFacts {
    pub decl: DeclKey,
    pub codegen_name: String,
    pub heap: HeapClass,
    pub class: u64,
    pub returned: bool,
    pub fate: Fate,
}

/// The persisted release plan of a module, carried on `TypedModule::release_plan`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReleasePlan {
    block_end: HashMap<Span, Vec<ReleaseEntry>>,
    return_edge: HashMap<Span, Vec<ReleaseEntry>>,
    propagation_edge: HashMap<Span, Vec<ReleaseEntry>>,
    move_sites: HashMap<Span, MoveSite>,
    reads_user: HashMap<String, ReleaseReads>,
    reads_codegen: HashMap<String, ReleaseReads>,
    facts: Vec<EdgeFacts>,
    internal_errors: Vec<String>,
}

impl PartialEq for EdgeFacts {
    fn eq(&self, other: &Self) -> bool {
        self.function == other.function && self.kind == other.kind && self.span == other.span
    }
}

impl ReleasePlan {
    /// The list for the end of the block at `span`.
    pub fn block_end(&self, span: &SourceSpan) -> Option<&[ReleaseEntry]> {
        self.block_end.get(&span_of(span)).map(Vec::as_slice)
    }

    /// The list for the `return` at `span`.
    pub fn return_edge(&self, span: &SourceSpan) -> Option<&[ReleaseEntry]> {
        self.return_edge.get(&span_of(span)).map(Vec::as_slice)
    }

    /// The list for the auto-propagation fired by the identifier read at `span`.
    pub fn propagation_edge(&self, span: &SourceSpan) -> Option<&[ReleaseEntry]> {
        self.propagation_edge
            .get(&span_of(span))
            .map(Vec::as_slice)
    }

    /// The founder a read at `span` takes its value out of, when the read is a move site.
    pub fn move_site(&self, span: &SourceSpan) -> Option<&MoveSite> {
        self.move_sites.get(&span_of(span))
    }

    /// The codegen names of the founders whose slot a read inside `stmt` nulls (every move site
    /// within the statement's span, nested blocks included), sorted and deduplicated. A move is
    /// a write to the founder's slot, so a state-machine walk flushes it like an assignment
    /// (`collect_crossing_writes`). O(move sites).
    pub fn founders_moved_in(&self, stmt: &Stmt) -> Vec<String> {
        let span = stmt_span(stmt);
        let mut names: BTreeSet<String> = BTreeSet::new();
        for (site_span, site) in &self.move_sites {
            if encloses(span, *site_span) {
                names.insert(site.founder_codegen_name.clone());
            }
        }
        names.into_iter().collect()
    }

    /// Every move site, as (read span, founder).
    pub fn move_sites(&self) -> impl Iterator<Item = (Span, &MoveSite)> + '_ {
        self.move_sites.iter().map(|(span, site)| (*span, site))
    }

    /// The release reads of `function` in `names`. Empty for a function with no recorded edge
    /// (an imported or hidden-default context).
    pub fn reads(&self, function: &str, names: Names) -> ReleaseReads {
        let table = match names {
            Names::User => &self.reads_user,
            Names::Codegen => &self.reads_codegen,
        };
        table.get(function).cloned().unwrap_or_default()
    }

    /// Holes in the walk's bookkeeping; `TypedModule::with_codegen_names` fails on them.
    pub fn internal_errors(&self) -> &[String] {
        &self.internal_errors
    }

    /// What the walk saw at each edge, in walk order.
    pub fn facts(&self) -> &[EdgeFacts] {
        &self.facts
    }

    fn absorb(
        &mut self,
        function: &FunctionExits,
        resolved: ResolvedFunction,
        user_reads: ReleaseReads,
        anchor_of: &dyn Fn(&DeclKey) -> Option<ReleaseAnchor>,
        names: &BindingNames,
        strings: &StringMoves,
    ) {
        let codegen_name_of = |decl: &DeclKey| {
            names
                .codegen_name(decl)
                .map_or_else(|| decl.2.clone(), str::to_string)
        };
        let mut releasing_if_live: HashSet<DeclKey> = HashSet::new();
        for ((kind, span, fates), raw_edge) in resolved.edges.iter().zip(&function.edges) {
            let list: Vec<ReleaseEntry> = fates
                .iter()
                .map(|(decl, fate)| {
                    if matches!(fate, Fate::ReleaseIfLive(_)) {
                        releasing_if_live.insert(decl.clone());
                    }
                    ReleaseEntry {
                        decl: decl.clone(),
                        codegen_name: codegen_name_of(decl),
                        fate: *fate,
                    }
                })
                .collect();
            let table = match kind {
                EdgeKind::BlockEnd => &mut self.block_end,
                EdgeKind::Return => &mut self.return_edge,
                EdgeKind::Propagation => &mut self.propagation_edge,
            };
            table.insert(*span, list);
            self.facts.push(EdgeFacts {
                function: function.name.clone(),
                kind: *kind,
                span: *span,
                consumed_classes: raw_edge.consumed_classes.clone(),
                entries: raw_edge
                    .entries
                    .iter()
                    .zip(fates)
                    .map(|(entry, (_, fate))| EntryFacts {
                        decl: entry.decl.clone(),
                        codegen_name: codegen_name_of(&entry.decl),
                        heap: entry.heap,
                        class: entry.class,
                        returned: entry.returned,
                        fate: *fate,
                    })
                    .collect(),
            });
        }
        self.reads_user.insert(function.name.clone(), user_reads);
        self.reads_codegen
            .insert(function.name.clone(), resolved.reads);
        // Move sites: where a read takes a value out of a founder that may release later.
        for site in &function.move_sites {
            if let Some(ReleaseAnchor::Founder { decl, .. }) = anchor_of(&site.named) {
                if releasing_if_live.contains(&decl) {
                    let founder_codegen_name = codegen_name_of(&decl);
                    self.move_sites.insert(
                        site.span,
                        MoveSite {
                            founder: decl,
                            founder_codegen_name,
                        },
                    );
                }
            }
        }
        for (span, decl) in strings.last_uses() {
            if releasing_if_live.contains(decl) && function_declares(function, decl) {
                self.move_sites.insert(
                    span,
                    MoveSite {
                        founder: decl.clone(),
                        founder_codegen_name: codegen_name_of(decl),
                    },
                );
            }
        }
        for decl in &releasing_if_live {
            let has_site = self
                .move_sites
                .values()
                .any(|site| &site.founder == decl);
            if !has_site {
                self.internal_errors.push(format!(
                    "internal: `{}` in `{}` is released only if live but no read moves it",
                    decl.2, function.name
                ));
            }
        }
    }

    /// Where the plan disagrees with the live scope's consumed classes or the transfer record.
    /// Per edge and listed founder of a supported non-string kind that is not reassigned:
    ///
    /// * its class is in `consumed_classes` and not returned => `Omitted(Moved)` or
    ///   `ReleaseIfLive`, and the record holds a given/sent/stored escape for the class;
    /// * not consumed and not returned => `Release`;
    /// * returned => `Omitted(Returned)` and the record holds a returned escape.
    ///
    /// Empty when they agree. O(entries · class size).
    pub fn parity_violations(&self, transfers: &TransferRecord) -> Vec<String> {
        use crate::transfer_record::Escape;
        let mut violations = Vec::new();
        for edge in &self.facts {
            let Some(function_transfers) = transfers.function(&edge.function) else {
                violations.push(format!("no transfer record for `{}`", edge.function));
                continue;
            };
            for entry in &edge.entries {
                let HeapClass::Supported(kind) = entry.heap else {
                    continue;
                };
                if kind == ReleaseKind::String {
                    continue;
                }
                let Some(class) = function_transfers.class_of(&entry.codegen_name) else {
                    violations.push(format!("`{}` has no class in the record", entry.codegen_name));
                    continue;
                };
                let is_founder = matches!(&class.anchor,
                    ReleaseAnchor::Founder { decl, .. } if *decl == entry.decl);
                if !is_founder || matches!(entry.fate, Fate::Omitted(OmitReason::Reassigned)) {
                    continue;
                }
                let consumed = edge.consumed_classes.contains(&entry.class);
                let moved_escape = class.escapes.iter().any(|escape| {
                    matches!(
                        escape,
                        Escape::Given { .. } | Escape::Sent { .. } | Escape::Stored { .. }
                    )
                });
                let returned_escape = class.escapes.contains(&Escape::Returned);
                let ok = if entry.returned {
                    entry.fate == Fate::Omitted(OmitReason::Returned) && returned_escape
                } else if consumed {
                    matches!(
                        entry.fate,
                        Fate::Omitted(OmitReason::Moved) | Fate::ReleaseIfLive(_)
                    ) && moved_escape
                } else {
                    entry.fate == Fate::Release(kind)
                };
                if !ok {
                    violations.push(format!(
                        "{:?} edge at {:?} in `{}`: `{}` has fate {:?} but consumed={consumed}, \
                         returned={}, record escapes {:?}",
                        edge.kind,
                        edge.span,
                        edge.function,
                        entry.codegen_name,
                        entry.fate,
                        entry.returned,
                        class.escapes
                    ));
                }
            }
        }
        violations
    }
}

fn function_declares(function: &FunctionExits, decl: &DeclKey) -> bool {
    function
        .edges
        .iter()
        .flat_map(|edge| edge.entries.iter())
        .any(|entry| &entry.decl == decl)
}

