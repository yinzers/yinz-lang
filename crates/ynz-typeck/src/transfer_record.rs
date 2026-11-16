//! The persisted transfer record — typeck's answer to "who holds this heap value, and may
//! anything free it here?" (v0.3-M9 C5a, plan #17; `IMP-ownership.md` "The persisted transfer
//! record").
//!
//! The scope walk already tracks alias classes while it runs (`Scope::alias_class`,
//! `check_transfer`, `binding_event_origin`), but a class dies with its scope. A scope-exit
//! release pass needs the FINAL partition, so this module keeps it: one class per set of
//! bindings that denote or reach one value, with
//!
//! * every member keyed by the declaration (`DeclKey`) and by its unique codegen name
//!   (`BindingNames`, C4) — never by the name the user wrote;
//! * the **release anchor**: the one member whose scope exits last (the *founder*), or the
//!   reason no member can be named statically;
//! * the **escapes**: the ways a member's value leaves this function's hands (returned,
//!   given, sent) — a class that escapes on any path is not released at scope exit by the
//!   bindings that merely named it.
//!
//! # What joins a class (the alias edges)
//!
//! Decided (Patrick, plan #17 C5a, on the Chief Architect's recommendation): storing a heap value
//! HANDS IT OVER. `list.add(a)`, `table.set(k, a)`, `x.field = a`, `x[i] = a`, a container literal
//! built from named values and `let b = a` are transfers like a give or a send: the place owns the
//! value, `a` is consumed (reading it is a compile error, `ConsumedByStore`), and the record notes
//! an `Escape::Stored` on `a`'s class. A store therefore never joins two classes: one value, one
//! owner. Classes join only where a name keeps reading a piece of another value:
//!
//! | event | edge |
//! |---|---|
//! | `let f = x.field`, `let c = x[i]`, a call result that is a piece of its argument | `f`/`c` joins every root it reaches |
//! | `for (cell in m)` | `cell` joins `m` |
//! | an untraceable result (an imported call) | joins every binding the expression mentions |
//!
//! A read of a place that typeck judged a copy (`ReadVerdict::MustCopy`, `non_owned_reads`) binds
//! a value of its own: the event neither joins a class nor makes its binding a piece. A binding
//! whose every event is fresh, a `give` parameter, a copy or a take is a **copy owner**: an owner
//! like any other, released at its own scope exit. The verdicts exist only after the walk, so
//! every event keeps the span of the value it bound and its joins are applied when the record is
//! built (`TransferRecorder::finish`).
//! # Who frees what
//!
//! A class must free every distinct object in it exactly once, container and stored elements
//! alike. That is the release pass's job (C6/C7); this record only says WHICH binding anchors
//! the class and whether the class may be released at all. Leaks are acceptable meanwhile;
//! an early or a double free never is.

use std::collections::{HashMap, HashSet};

use crate::binding_names::{BindingNames, DeclKey};
use crate::non_owned_reads::{NonOwnedReads, ReadVerdict};

/// `(span.start, span.end)` of the value expression a binding event bound.
pub type ValueSpan = (usize, usize);

/// How a member can take part in releasing its class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberRole {
    /// Bound only to values it is the first holder of (`Origin::Owned`, a `give` parameter, a
    /// copy of a place or a piece taken out of a temporary): a candidate founder.
    Owner,
    /// A parameter declared `share`, `lend` or bare: the caller keeps the value.
    BorrowedParam,
    /// Bound at least once to a piece of something else, a loop cell, or a value the compiler
    /// cannot trace: it holds the value but never owns it.
    Piece,
}

/// One binding in a class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassMember {
    /// The declaration (`(span.start, span.end, declared name)`).
    pub decl: DeclKey,
    /// The binding's unique name inside its function, as codegen's AST carries it.
    pub codegen_name: String,
    pub role: MemberRole,
    /// The frame path of the scope the binding was declared in (`Scope::path`).
    pub path: Vec<u32>,
    /// The escapes whose read named THIS binding (a class's `escapes` are the union over its
    /// members).
    pub escapes: Vec<Escape>,
    /// An owner at least one of whose events is a copy of a place (`ReadVerdict::MustCopy`).
    pub copy_owner: bool,
}

impl ClassMember {
    /// True when a read naming this binding handed its value over (given, sent or stored).
    fn moved_directly(&self) -> bool {
        self.escapes.iter().any(|escape| {
            matches!(
                escape,
                Escape::Given { .. } | Escape::Sent { .. } | Escape::Stored { .. }
            )
        })
    }
}

/// What a release decision needs to know about one binding, from the record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberFacts {
    /// Where the binding's release is anchored (`AliasClass::anchor_for`).
    pub anchor: ReleaseAnchor,
    /// A read naming this binding gave, sent or stored it. Only then can the class's consumed
    /// state in the live scope be about this binding's own value.
    pub moved_directly: bool,
    /// Every binding that may point into this binding's value: itself and the pieces of its
    /// class. A `return` of one of them that does not copy returns this binding's value.
    pub aliases: Vec<DeclKey>,
}

/// How a member's value can leave this function's hands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Escape {
    /// Returned (whole, or as a piece of the returned value), on at least one path.
    Returned,
    /// Given to a `give` position of `callee` (a call, a UFCS call, a `background` spawn), on
    /// at least one path.
    Given { callee: String },
    /// Sent into `channel`, on at least one path.
    Sent { channel: String },
    /// Stored in `place` (`list.add(a)`, `x.field = a`, `let b = a`, a container literal): the
    /// place owns the value from then on, on at least one path. `place` is the place as typeck
    /// names it, e.g. `` `list` `` or ``the literal `[a, c]` ``; a place no binding names
    /// (`makeBag().items = a`) is recorded the same way.
    Stored { place: String },
}

/// Why no member can be named the founder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoFounderReason {
    /// A member that is not an owner outlives every owner (or shares the outermost scope with
    /// none): the owner's scope ends while `holder` still holds the value. Which one of the
    /// values `holder` holds at that moment is only known at run time.
    OutlivedByNonOwner { holder: String },
}

/// Where the class's release is anchored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleaseAnchor {
    /// Release the class once, when this member's scope exits: the member whose scope exits
    /// last. Members in the same outermost scope: the earliest declared.
    Founder { decl: DeclKey, codegen_name: String },
    /// A parameter that is not `give` is in the class: the caller owns every object in it,
    /// so nothing here releases it.
    CallerOwns { parameter: String },
    /// No member owns a fresh value (all pieces, loop cells or untraceable results), so no
    /// binding here has a release duty. The allocation, if any, belongs to code this
    /// function cannot see (an imported function's result).
    NoOwner,
    /// Owners exist but no static founder (see [`NoFounderReason`]).
    NoStaticFounder(NoFounderReason),
}

/// One alias class: bindings that denote or reach one value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AliasClass {
    /// In declaration order.
    pub members: Vec<ClassMember>,
    pub anchor: ReleaseAnchor,
    pub escapes: Vec<Escape>,
}

impl AliasClass {
    /// The anchor `codegen_name` is judged by. A class joins bindings whose values one name may
    /// reach (`found.or(backup)` reaches `rows`' item or `backup`), so it can hold several
    /// `Owner`s, each bound only to values of its own and so each its own allocation. The class
    /// names ONE founder; every other `Owner` founds its own allocation and is released at its
    /// own scope exit like the founder, whether or not the class escapes (an escape names the
    /// binding that was handed over: `ClassMember::escapes`). One exception keeps it safe: a
    /// `Piece` whose scope strictly encloses the owner's may still point into the owner's value
    /// when the owner dies, so that owner is left to the class anchor (never released early).
    pub fn anchor_for(&self, codegen_name: &str) -> ReleaseAnchor {
        match self
            .members
            .iter()
            .position(|member| member.codegen_name == codegen_name)
        {
            Some(index) => self.anchor_of_member(index),
            None => self.anchor.clone(),
        }
    }

    /// `anchor_for` for the member at `index` (declared names are not unique, positions are).
    pub fn anchor_of_member(&self, index: usize) -> ReleaseAnchor {
        let member = &self.members[index];
        if matches!(self.anchor, ReleaseAnchor::Founder { .. }) && member.role == MemberRole::Owner
        {
            let outlived_by_piece = self.members.iter().any(|other| {
                other.role == MemberRole::Piece
                    && other.path.len() < member.path.len()
                    && member.path.starts_with(&other.path)
            });
            let is_class_founder = matches!(&self.anchor,
                ReleaseAnchor::Founder { decl, .. } if *decl == member.decl);
            if is_class_founder || !outlived_by_piece {
                return ReleaseAnchor::Founder {
                    decl: member.decl.clone(),
                    codegen_name: member.codegen_name.clone(),
                };
            }
        }
        self.anchor.clone()
    }

    /// What a release decision needs about the member at `index`.
    pub fn facts_of_member(&self, index: usize) -> MemberFacts {
        let member = &self.members[index];
        let mut aliases = vec![member.decl.clone()];
        aliases.extend(
            self.members
                .iter()
                .filter(|other| other.role == MemberRole::Piece && other.decl != member.decl)
                .map(|other| other.decl.clone()),
        );
        MemberFacts {
            anchor: self.anchor_of_member(index),
            moved_directly: member.moved_directly(),
            aliases,
        }
    }

    /// The facts of the member `codegen_name`.
    pub fn facts_for(&self, codegen_name: &str) -> Option<MemberFacts> {
        self.members
            .iter()
            .position(|member| member.codegen_name == codegen_name)
            .map(|index| self.facts_of_member(index))
    }

    /// True when `codegen_name` is a member.
    pub fn has_member(&self, codegen_name: &str) -> bool {
        self.members
            .iter()
            .any(|member| member.codegen_name == codegen_name)
    }
}

/// The classes of one function.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FunctionTransfers {
    pub classes: Vec<AliasClass>,
}

impl FunctionTransfers {
    /// The class `codegen_name` belongs to.
    pub fn class_of(&self, codegen_name: &str) -> Option<&AliasClass> {
        self.classes
            .iter()
            .find(|class| class.has_member(codegen_name))
    }

    /// True when both bindings are in one class.
    pub fn same_class(&self, first: &str, second: &str) -> bool {
        self.class_of(first)
            .is_some_and(|class| class.has_member(second))
    }
}

/// The finished record of a module, carried on `TypedModule::transfers`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransferRecord {
    functions: HashMap<String, FunctionTransfers>,
    /// Events the recorder could not place and anchors it could not build: each is a hole in the
    /// checker's own bookkeeping, never a user mistake. `TypedModule::with_codegen_names` fails
    /// on them, exactly as it does on a hole in `BindingNames`. Empty for every record the
    /// checker builds today.
    internal_errors: Vec<String>,
}

impl TransferRecord {
    /// The classes of the function named `function_name` (functions are unique by name).
    pub fn function(&self, function_name: &str) -> Option<&FunctionTransfers> {
        self.functions.get(function_name)
    }

    /// The holes found while recording (see the field). Empty when the record is whole.
    pub fn internal_errors(&self) -> &[String] {
        &self.internal_errors
    }
}

// ── The recorder ────────────────────────────────────────────────────────────

/// What one binding event says about the binding's part in releasing a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleEvidence {
    /// Bound to a fresh value (`Origin::Owned`) or a `give` parameter.
    Owner,
    /// A parameter that is `share`, `lend` or bare.
    BorrowedParam,
    /// Bound to a piece of something else, a loop cell, or an untraceable value.
    Piece,
}

#[derive(Clone)]
struct BindingState {
    decl: DeclKey,
    /// The frame path (`Scope::path`) of the scope the binding was declared in.
    path: Vec<u32>,
    borrowed_param: bool,
    /// Every event that bound a non-owned value, by the span of the value it read (`None`: no
    /// place read to judge, e.g. a loop cell or an untraceable result). An event whose read
    /// typeck judged a copy binds a value of its own (`resolved_event`); any other
    /// means the binding cannot be trusted to hold only values of its own, so it never anchors
    /// a release.
    piece_events: Vec<Option<ValueSpan>>,
}

/// A join recorded at a binding event; applied when the record is built, unless the event's read
/// turned out to be a copy.
#[derive(Clone)]
struct PendingJoin {
    first: usize,
    second: usize,
    event: Option<ValueSpan>,
}

#[derive(Clone)]
struct FunctionState {
    name: String,
    bindings: Vec<BindingState>,
    index: HashMap<DeclKey, usize>,
    joins: Vec<PendingJoin>,
    escapes: Vec<(usize, Escape)>,
}

/// True when the event that read the value at `event` bound a value of its own: typeck judged the
/// read a copy (`non_owned_reads`). A `Take` is not resolved here: the checker already binds a take
/// as a fresh value when the piece is of a temporary no name reaches, and a take of a call result
/// that is a piece of its arguments (a generic callee) stays a piece joined to them.
fn resolved_event(reads: &NonOwnedReads, event: Option<ValueSpan>) -> bool {
    event.is_some_and(|(start, end)| reads.verdict(start, end) == Some(ReadVerdict::MustCopy))
}

/// Union-find over binding positions.
struct Partition {
    parent: Vec<usize>,
}

impl Partition {
    fn new(count: usize) -> Self {
        Self {
            parent: (0..count).collect(),
        }
    }

    fn union(&mut self, first: usize, second: usize) {
        let first_root = self.find(first);
        let second_root = self.find(second);
        if first_root == second_root {
            return;
        }
        // The smaller index stays the root so class order follows declaration order.
        let (kept, absorbed) = if first_root < second_root {
            (first_root, second_root)
        } else {
            (second_root, first_root)
        };
        self.parent[absorbed] = kept;
    }

    fn find(&mut self, mut node: usize) -> usize {
        while self.parent[node] != node {
            self.parent[node] = self.parent[self.parent[node]];
            node = self.parent[node];
        }
        node
    }
}

/// Fills a [`TransferRecord`] while the checker walks the module, one function at a time.
/// Mirrors `BindingNameRecorder`: the checker calls it at the same events that update its live
/// scope, so the record can never disagree with what the checker decided.
#[derive(Default, Clone)]
pub struct TransferRecorder {
    current: Option<FunctionState>,
    finished: Vec<FunctionState>,
    /// An event that named no current function or an undeclared binding. Recorded, never
    /// dropped: `finish` hands them on in the record.
    internal_errors: Vec<String>,
}

/// The internal error for an event that arrived while no function was being walked.
fn outside_function_message(event: &str, decl: Option<&DeclKey>) -> String {
    let subject = decl.map_or_else(String::new, |decl| {
        format!(" for `{}` at bytes {}..{}", decl.2, decl.0, decl.1)
    });
    format!(
        "internal: the transfer record was told {event}{subject} while no function was being walked"
    )
}

/// The internal error for an event that names a binding the function never declared.
fn undeclared_message(event: &str, function_name: &str, decl: &DeclKey) -> String {
    format!(
        "internal: the transfer record was told {event} for `{}` at bytes {}..{} in `{function_name}`, \
         which it never saw declared",
        decl.2, decl.0, decl.1
    )
}

impl TransferRecorder {
    /// Start walking the function `function_name`; closes the previous one.
    pub fn begin_function(&mut self, function_name: &str) {
        if let Some(previous) = self.current.take() {
            self.finished.push(previous);
        }
        self.current = Some(FunctionState {
            name: function_name.to_string(),
            bindings: Vec::new(),
            index: HashMap::new(),
            joins: Vec::new(),
            escapes: Vec::new(),
        });
    }

    /// Record a declaration in the scope at `path`; `value` is the span of the value its first
    /// event read, when it read a place. A node walked twice is declared once; its later events
    /// only add evidence.
    pub fn declare(
        &mut self,
        decl: DeclKey,
        path: Vec<u32>,
        event: RoleEvidence,
        value: Option<ValueSpan>,
    ) {
        let Some(function) = self.current.as_mut() else {
            self.internal_errors
                .push(outside_function_message("a declaration", Some(&decl)));
            return;
        };
        if let Some(&existing) = function.index.get(&decl) {
            Self::apply_event(&mut function.bindings[existing], event, value);
            return;
        }
        let position = function.bindings.len();
        let mut binding = BindingState {
            decl: decl.clone(),
            path,
            borrowed_param: false,
            piece_events: Vec::new(),
        };
        Self::apply_event(&mut binding, event, value);
        function.bindings.push(binding);
        function.index.insert(decl, position);
    }

    fn apply_event(binding: &mut BindingState, event: RoleEvidence, value: Option<ValueSpan>) {
        match event {
            RoleEvidence::Owner => {}
            RoleEvidence::BorrowedParam => binding.borrowed_param = true,
            RoleEvidence::Piece => {
                if !binding.piece_events.contains(&value) {
                    binding.piece_events.push(value);
                }
            }
        }
    }

    /// A later binding event (a reassignment) bound `decl` to a value of this role, read from the
    /// value at `value`.
    pub fn note_event(&mut self, decl: &DeclKey, event: RoleEvidence, value: Option<ValueSpan>) {
        let Some(function) = self.current.as_mut() else {
            self.internal_errors
                .push(outside_function_message("a binding event", Some(decl)));
            return;
        };
        let Some(&position) = function.index.get(decl) else {
            self.internal_errors
                .push(undeclared_message("a binding event", &function.name, decl));
            return;
        };
        Self::apply_event(&mut function.bindings[position], event, value);
    }

    /// The two bindings denote or reach one value from now on. `event` is the span of the value
    /// whose read made them so, when a copy of it would not (`resolved_event`).
    pub fn join(&mut self, first: &DeclKey, second: &DeclKey, event: Option<ValueSpan>) {
        let Some(function) = self.current.as_mut() else {
            self.internal_errors
                .push(outside_function_message("a join", Some(first)));
            return;
        };
        let (Some(&first_position), Some(&second_position)) =
            (function.index.get(first), function.index.get(second))
        else {
            let undeclared = if function.index.contains_key(first) {
                second
            } else {
                first
            };
            self.internal_errors
                .push(undeclared_message("a join", &function.name, undeclared));
            return;
        };
        let pending = PendingJoin {
            first: first_position,
            second: second_position,
            event,
        };
        if !function.joins.iter().any(|seen| {
            seen.first == pending.first
                && seen.second == pending.second
                && seen.event == pending.event
        }) {
            function.joins.push(pending);
        }
    }

    /// The value `decl` holds leaves this function's hands.
    pub fn escape(&mut self, decl: &DeclKey, escape: Escape) {
        let Some(function) = self.current.as_mut() else {
            self.internal_errors
                .push(outside_function_message("an escape", Some(decl)));
            return;
        };
        let Some(&position) = function.index.get(decl) else {
            self.internal_errors
                .push(undeclared_message("an escape", &function.name, decl));
            return;
        };
        function.escapes.push((position, escape));
    }

    /// What a release decision needs about every binding the current function has declared so
    /// far, keyed by declaration: the class partition as it stands, before any read is judged (a
    /// copy of a place is not yet known to be one). Used when a function's walk ends, before
    /// `BindingNames` has named its bindings, so a pass that needs the facts inside the walk (the
    /// release plan's owner set, which the suspension guards read) asks the same partition
    /// `finish` will build, minus the copies. A hole in the partition is recorded in
    /// `internal_errors` and yields an empty map. O(b · d) time and O(b) memory for b bindings at
    /// frame depth d.
    pub fn current_facts(&mut self) -> HashMap<DeclKey, MemberFacts> {
        let Some(function) = self.current.clone() else {
            self.internal_errors
                .push(outside_function_message("a request for anchors", None));
            return HashMap::new();
        };
        let declared_name = |decl: &DeclKey| Some(decl.2.clone());
        match build_function(function, &declared_name, &NonOwnedReads::default()) {
            Ok(transfers) => {
                transfers
                    .classes
                    .iter()
                    .flat_map(|class| {
                        class.members.iter().enumerate().map(|(index, member)| {
                            (member.decl.clone(), class.facts_of_member(index))
                        })
                    })
                    .collect()
            }
            Err(message) => {
                self.internal_errors.push(message);
                HashMap::new()
            }
        }
    }

    /// Close the last function and build the record. Codegen names come from `names`, which
    /// must have named every declaration this recorder saw (the checker declares both at the
    /// same event); `reads` says which place reads are copies. An event the recorder could not
    /// place, or a declaration `names` lacks, ends up in `TransferRecord::internal_errors`; it
    /// is never skipped.
    ///
    /// O(b · d) time and O(b) memory for b bindings at frame depth d.
    pub fn finish(mut self, names: &BindingNames, reads: &NonOwnedReads) -> TransferRecord {
        if let Some(last) = self.current.take() {
            self.finished.push(last);
        }
        let mut record = TransferRecord {
            internal_errors: self.internal_errors,
            ..TransferRecord::default()
        };
        for function in self.finished {
            let name = function.name.clone();
            let codegen_name_of = |decl: &DeclKey| names.codegen_name(decl).map(str::to_string);
            match build_function(function, &codegen_name_of, reads) {
                Ok(transfers) => {
                    record.functions.insert(name, transfers);
                }
                Err(message) => record.internal_errors.push(message),
            }
        }
        record
    }
}

/// One function's classes; an error when a recorded binding has no codegen name or a class has no
/// anchor candidate (a hole in the record, see `TransferRecord::internal_errors`).
fn build_function(
    function: FunctionState,
    codegen_name_of: &dyn Fn(&DeclKey) -> Option<String>,
    reads: &NonOwnedReads,
) -> Result<FunctionTransfers, String> {
    let count = function.bindings.len();
    let mut partition = Partition::new(count);
    for join in &function.joins {
        if !resolved_event(reads, join.event) {
            partition.union(join.first, join.second);
        }
    }
    let mut roots: Vec<usize> = Vec::with_capacity(count);
    for position in 0..count {
        roots.push(partition.find(position));
    }
    let mut class_order: Vec<usize> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
    for &root in &roots {
        if seen.insert(root) {
            class_order.push(root);
        }
    }
    let mut classes = Vec::with_capacity(class_order.len());
    for root in class_order {
        let positions: Vec<usize> = (0..count)
            .filter(|&position| roots[position] == root)
            .collect();
        let members = positions
            .iter()
            .map(|&position| {
                let binding = &function.bindings[position];
                let is_piece = binding
                    .piece_events
                    .iter()
                    .any(|event| !resolved_event(reads, *event));
                let role = if binding.borrowed_param {
                    MemberRole::BorrowedParam
                } else if is_piece {
                    MemberRole::Piece
                } else {
                    MemberRole::Owner
                };
                let copy_owner = role == MemberRole::Owner && !binding.piece_events.is_empty();
                let codegen_name = codegen_name_of(&binding.decl)
                    .ok_or_else(|| undeclared_in_names_message(&function.name, &binding.decl))?;
                let mut own_escapes: Vec<Escape> = Vec::new();
                for (escaped, escape) in &function.escapes {
                    if *escaped == position && !own_escapes.contains(escape) {
                        own_escapes.push(escape.clone());
                    }
                }
                Ok(ClassMember {
                    decl: binding.decl.clone(),
                    codegen_name,
                    role,
                    path: binding.path.clone(),
                    escapes: own_escapes,
                    copy_owner,
                })
            })
            .collect::<Result<Vec<ClassMember>, String>>()?;
        let anchor = anchor_of(&function.name, &members, &positions, &function.bindings)?;
        let mut escapes: Vec<Escape> = Vec::new();
        for (position, escape) in &function.escapes {
            if roots[*position] == root && !escapes.contains(escape) {
                escapes.push(escape.clone());
            }
        }
        classes.push(AliasClass {
            members,
            anchor,
            escapes,
        });
    }
    Ok(FunctionTransfers { classes })
}

/// The internal error for a binding the recorder holds that `BindingNames` never named.
fn undeclared_in_names_message(function_name: &str, decl: &DeclKey) -> String {
    format!(
        "internal: the binding `{}` at bytes {}..{} in `{function_name}` was recorded for \
         releasing but has no codegen name from type checking",
        decl.2, decl.0, decl.1
    )
}

/// The founder rule: the owner whose scope exits last. `positions[i]` is the binding behind
/// `members[i]`. An error when no member's scope encloses the others (a hole in the record).
fn anchor_of(
    function_name: &str,
    members: &[ClassMember],
    positions: &[usize],
    bindings: &[BindingState],
) -> Result<ReleaseAnchor, String> {
    if let Some(parameter) = members
        .iter()
        .find(|member| member.role == MemberRole::BorrowedParam)
    {
        return Ok(ReleaseAnchor::CallerOwns {
            parameter: parameter.codegen_name.clone(),
        });
    }
    if !members
        .iter()
        .any(|member| member.role == MemberRole::Owner)
    {
        return Ok(ReleaseAnchor::NoOwner);
    }
    // The scope that exits last is the one every member's scope is inside: the longest path
    // all members share. A member declared exactly there is "outermost".
    let mut shared: &[u32] = &bindings[positions[0]].path;
    for &position in positions {
        let path = &bindings[position].path;
        let common = shared
            .iter()
            .zip(path.iter())
            .take_while(|(first, second)| first == second)
            .count();
        shared = &shared[..common];
    }
    let outermost: Vec<usize> = (0..members.len())
        .filter(|&member_index| bindings[positions[member_index]].path.len() == shared.len())
        .collect();
    // Empty only for a hole in the record. A join needs both bindings visible at once, so two
    // joined bindings are always in an enclosing relation (one scope inside the other). In a class
    // connected by such links, take a member of minimal depth: any link leaving its subtree would
    // reach a shallower scope, contradicting minimality. So the minimal-depth member's scope
    // encloses every member, and the longest shared path is exactly its path.
    if outermost.is_empty() {
        return Err(format!(
            "internal: no member of the class of `{}` in `{function_name}` has a scope that \
             encloses all the others",
            members[0].codegen_name
        ));
    }
    Ok(
        match outermost
            .iter()
            .find(|&&member_index| members[member_index].role == MemberRole::Owner)
        {
            Some(&member_index) => ReleaseAnchor::Founder {
                decl: members[member_index].decl.clone(),
                codegen_name: members[member_index].codegen_name.clone(),
            },
            None => ReleaseAnchor::NoStaticFounder(NoFounderReason::OutlivedByNonOwner {
                holder: members[outermost[0]].codegen_name.clone(),
            }),
        },
    )
}
