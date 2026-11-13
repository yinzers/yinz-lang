use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use ynz_ast::nodes::OwnershipModifier;
use ynz_diagnostics::SourceSpan;

use crate::types::Type;

/// Why a binding is no longer usable — the cause selects the diagnostic the consumed-read
/// site renders (`Consumed` for a give, `ConsumedBySend` for a channel send) and fills its
/// slots. ONE field on the entry, never a bool beside an `Option<String>` (v0.3-M8 Phase 4,
/// `IMP-ownership.md` "Binding events, origin and alias classes").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsumedBy {
    /// Given to a `give` position of `callee` (a call, a UFCS dot-call, a `background` spawn);
    /// `given` is the binding that was named at the give — it differs from the consumed name
    /// when the consumption reached this entry through its alias class (the `{via}` slot).
    /// `missed_revive`: see [`ConsumedBy::missed_revive`].
    Given {
        callee: String,
        given: String,
        missed_revive: Option<BranchKind>,
    },
    /// Sent into `channel` (a `channel<T>.send` / `h.send` of an owned-heap payload); `sent`
    /// is the binding that was named at the send — it differs from the consumed name when
    /// the consumption reached this entry through its alias class.
    /// `missed_revive`: see [`ConsumedBy::missed_revive`].
    Sent {
        channel: String,
        sent: String,
        missed_revive: Option<BranchKind>,
    },
}

/// The kind of statement that opened a branch point (see [`Scope::begin_branches`]); names the
/// path a revive did not cover in the maybe-given read diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchKind {
    /// An `if` (it has no `else`: the condition-false path enters no arm).
    If,
    /// A `match`.
    Match,
    /// A `while` or `for` loop (its body may run zero times).
    Loop,
}

impl ConsumedBy {
    /// The branch point at whose join the binding stayed given away on a path that did not
    /// reassign it, though another path did (`None`: given away with no revive involved). It
    /// fills the `{via}` slot of the maybe-given read, so the explanation of the give stays the
    /// one canonical `Consumed` text.
    pub fn missed_revive(&self) -> Option<BranchKind> {
        match self {
            ConsumedBy::Given { missed_revive, .. } | ConsumedBy::Sent { missed_revive, .. } => {
                *missed_revive
            }
        }
    }

    /// This cause marked as left maybe-given by a join of `kind`; an earlier (inner) mark wins.
    fn marked_missed_revive(self, kind: BranchKind) -> ConsumedBy {
        match self {
            ConsumedBy::Given {
                callee,
                given,
                missed_revive,
            } => ConsumedBy::Given {
                callee,
                given,
                missed_revive: missed_revive.or(Some(kind)),
            },
            ConsumedBy::Sent {
                channel,
                sent,
                missed_revive,
            } => ConsumedBy::Sent {
                channel,
                sent,
                missed_revive: missed_revive.or(Some(kind)),
            },
        }
    }
}

/// Where a binding's value came from — a property of the BINDING EVENT (the `let`, the
/// reassignment, the parameter, the loop), recomputed at every such event, never of the name.
/// Decides transferability at every sink (`check_transfer`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Bound to a fresh value nobody else reaches — transferable.
    Owned,
    /// A function parameter with its DECLARED modifier (`None` = bare). Only `Some(Give)` is
    /// transferable; the rest are explicit statements that the caller keeps the value.
    Param(Option<OwnershipModifier>),
    /// A `for` loop variable — one cell of the iterated value; never transferable. Carries
    /// the rendered `{reason}` for `TransferNeedsCopy` ("one cell of `matrix`, which this
    /// loop is walking").
    Cell(String),
    /// Bound to a piece of something someone else still holds (a field, an index, a literal
    /// built from named values, a call that returns a piece of its argument) — never
    /// transferable; the fix is `.copy()`. Carries the rendered `{reason}`, computed at the
    /// binding event from the initializer ("a field of `bucket`", "what `pick` returns, and
    /// `pick` returns a piece of its `b` argument", …).
    Reaches(String),
    /// Bound to a value the compiler cannot trace to one owner — never transferable.
    Unknown,
}

/// An entry in the variable scope — everything the type checker tracks per binding.
pub struct ScopeEntry {
    pub ty: Type,
    /// True when declared with `const`; the type checker rejects reassignment.
    pub is_const: bool,
    /// True for function parameters — reassignment emits a diagnostic.
    pub is_param: bool,
    /// The declared ownership modifier when this binding is a parameter.
    ///
    /// - `None` for non-parameter bindings (`let`/`const` locals, loop variables).
    /// - `None` for a parameter with no explicit modifier (`bare` — the compiler figures
    ///   out the effective modifier from how the body uses it, per `design/ownership.md`).
    /// - `Some(Share)` for an explicitly read-only parameter — the body may NOT change it.
    /// - `Some(Lend)` for an explicitly mutable parameter — the body may change it.
    /// - `Some(Give)` for an explicitly owned parameter — the body owns and may change it.
    ///
    /// Used to enforce the `share`-is-read-only rule: an explicit `share` parameter whose
    /// body mutates a field is a contradiction the compiler rejects.
    pub param_ownership: Option<OwnershipModifier>,
    /// True for `for`-loop variables — immutable inside the loop body.
    pub is_loop_var: bool,
    /// `Some(cause)` after ownership was transferred (given to a `give` position, or sent
    /// into an owned-heap channel). Any subsequent use of this binding — or of any member
    /// of its alias class — produces the use-after-give / use-after-send error.
    pub consumed: Option<ConsumedBy>,
    /// Where the currently-bound value came from (see [`Origin`]).
    pub origin: Origin,
    /// The alias class this entry belongs to: every live entry that denotes or reaches one
    /// value shares an id, so consuming any member consumes all of them. Keyed by ENTRY —
    /// a shadowed outer entry keeps its membership while hidden.
    pub alias_class: u64,
    /// Where the binding was declared (for "previously defined here" spans).
    pub defined_at: SourceSpan,
}

/// Block-scoped variable environment.
///
/// A new frame is pushed at the start of each `{ }` block and popped at the end.
/// Shadowing is allowed: re-declaring a name in an inner scope hides the outer one.
/// `lookup` searches from innermost to outermost, returning the first match.
pub struct Scope {
    frames: Vec<HashMap<String, ScopeEntry>>,
    /// One id per open frame, parallel to `frames`: the path of ids from the function frame
    /// down to the innermost frame names a scope, and a frame is inside another exactly when
    /// its path extends the other's (the transfer record's scope-exit order).
    frame_ids: Vec<u32>,
    next_frame_id: u32,
    /// Mint for alias-class ids — each fresh value gets its own class.
    next_class: u64,
    /// The open branch points (an `if`, a match, a loop), innermost last: where a give or a
    /// reassignment is made on ONE path and the join must not believe it covers the others.
    branches: Vec<Branches>,
}

/// An entry named by where it lives: frame index, name, declaration span.
type EntryKey = (usize, String, usize, usize);

/// The key of the entry `name` declared at `defined_at` in frame `frame_index`.
fn entry_key(frame_index: usize, name: &str, defined_at: &SourceSpan) -> EntryKey {
    (
        frame_index,
        name.to_string(),
        defined_at.start,
        defined_at.end,
    )
}

/// The part of an entry a path can change and a join must reconcile: whether it is given away,
/// where its value came from, and which alias class it is in. A reassignment changes all three
/// (`Scope::rebind`); a give changes the first (`Scope::consume`).
#[derive(Clone, Debug)]
struct PathState {
    consumed: Option<ConsumedBy>,
    origin: Origin,
    alias_class: u64,
}

impl PathState {
    fn of(entry: &ScopeEntry) -> PathState {
        PathState {
            consumed: entry.consumed.clone(),
            origin: entry.origin.clone(),
            alias_class: entry.alias_class,
        }
    }
}

/// True when a give of a value of this origin is allowed (`check_transfer`'s rule).
fn origin_transferable(origin: &Origin) -> bool {
    matches!(
        origin,
        Origin::Owned | Origin::Param(Some(OwnershipModifier::Give))
    )
}

/// What the join after a branch point needs (see [`Scope::begin_branches`]). Ordered maps, so
/// the join visits entries and paths in one fixed order (the cause a read names, the class kept
/// by a merge).
struct Branches {
    /// The statement that opened it.
    kind: BranchKind,
    /// Frame index of the arm being checked: only entries BELOW it outlive the arm. 0 between
    /// arms, so nothing is tracked there.
    arm_frame: usize,
    /// Every entry outliving the arms that some arm changed, with its state when the branch
    /// point opened. Every arm starts from that state: `end_arm` puts it back.
    opened: BTreeMap<EntryKey, PathState>,
    /// The entries the current arm changed.
    arm_touched: BTreeSet<EntryKey>,
    /// Per arm that reaches the join (an arm that always returns does not), in order: the end
    /// state of every entry that arm changed. An entry an arm did not change ends it as opened.
    arm_ends: Vec<BTreeMap<EntryKey, PathState>>,
    /// Entries some arm reassigned: a maybe-given read of one names the path that did not.
    reassigned: BTreeSet<EntryKey>,
}

impl Branches {
    /// An absorbed class is now `kept` (`Scope::merge_classes`): the recorded states follow.
    fn rename_class(&mut self, kept: u64, absorbed: u64) {
        let states = self
            .opened
            .values_mut()
            .chain(self.arm_ends.iter_mut().flat_map(|end| end.values_mut()));
        for state in states {
            if state.alias_class == absorbed {
                state.alias_class = kept;
            }
        }
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

impl Scope {
    pub fn new() -> Self {
        Self {
            frames: vec![HashMap::new()],
            frame_ids: vec![0],
            next_frame_id: 1,
            next_class: 1,
            branches: Vec::new(),
        }
    }

    pub fn push(&mut self) {
        self.frames.push(HashMap::new());
        self.frame_ids.push(self.next_frame_id);
        self.next_frame_id += 1;
    }

    pub fn pop(&mut self) {
        if self.frames.len() > 1 {
            self.frames.pop();
            self.frame_ids.pop();
        }
    }

    /// The path of frame ids from the outermost frame to the innermost one. O(depth).
    pub fn path(&self) -> Vec<u32> {
        self.frame_ids.clone()
    }

    /// Index of the innermost frame (frames below it enclose the current scope).
    pub fn innermost_frame(&self) -> usize {
        self.frames.len() - 1
    }

    /// Index of the frame holding the entry `lookup(name)` finds.
    pub fn frame_of(&self, name: &str) -> Option<usize> {
        self.frames
            .iter()
            .rposition(|frame| frame.contains_key(name))
    }

    /// The entry named `name` in the frame at `frame_index`, if that frame still holds it.
    pub fn entry_in_frame(&self, frame_index: usize, name: &str) -> Option<&ScopeEntry> {
        self.frames
            .get(frame_index)
            .and_then(|frame| frame.get(name))
    }

    /// A brand-new alias class (a value with exactly one holder so far).
    pub fn new_class(&mut self) -> u64 {
        let id = self.next_class;
        self.next_class += 1;
        id
    }

    pub fn insert(&mut self, name: String, entry: ScopeEntry) {
        if let Some(frame) = self.frames.last_mut() {
            frame.insert(name, entry);
        }
    }

    /// Walk from innermost to outermost frame and return the first match.
    pub fn lookup(&self, name: &str) -> Option<&ScopeEntry> {
        for frame in self.frames.iter().rev() {
            if let Some(e) = frame.get(name) {
                return Some(e);
            }
        }
        None
    }

    /// The alias class of the entry `lookup(name)` finds, if any.
    pub fn class_of(&self, name: &str) -> Option<u64> {
        self.lookup(name).map(|e| e.alias_class)
    }

    /// Every VISIBLE name (the entry `lookup` would find) whose entry belongs to `class`.
    /// Innermost frames win, so a shadowed outer entry of the same name is not listed twice.
    pub fn visible_members_of(&self, class: u64) -> Vec<String> {
        let mut seen: Vec<&str> = Vec::new();
        let mut members: Vec<String> = Vec::new();
        for frame in self.frames.iter().rev() {
            for (name, entry) in frame {
                // The innermost occurrence of a name is the entry `lookup` finds; an outer
                // entry hidden behind it is not visible and is not listed.
                if seen.contains(&name.as_str()) {
                    continue;
                }
                seen.push(name.as_str());
                if entry.alias_class == class {
                    members.push(name.clone());
                }
            }
        }
        members.sort();
        members
    }

    /// Consume a binding's WHOLE alias class (ownership transferred): every entry in any
    /// frame that shares the class is marked with `cause`. No-op if the name is not found.
    ///
    /// Returns the entries newly marked (frame index, name, declaration), in no particular
    /// order: the loop-carried-give check and the transfer record both need to know exactly
    /// which entries this transfer reached.
    ///
    /// O(entries + reached . open branch points) time.
    pub fn consume(&mut self, name: &str, cause: ConsumedBy) -> Vec<ConsumedEntry> {
        let Some(class) = self.class_of(name) else {
            return Vec::new();
        };
        let mut reached = Vec::new();
        for (frame_index, frame) in self.frames.iter().enumerate() {
            for (entry_name, entry) in frame {
                if entry.alias_class == class && entry.consumed.is_none() {
                    reached.push(ConsumedEntry {
                        frame_index,
                        name: entry_name.clone(),
                        defined_at: entry.defined_at.clone(),
                    });
                }
            }
        }
        for reached_entry in &reached {
            let key = entry_key(
                reached_entry.frame_index,
                &reached_entry.name,
                &reached_entry.defined_at,
            );
            self.touch(&key);
            if let Some(entry) = self.entry_mut(&key) {
                entry.consumed = Some(cause.clone());
            }
        }
        reached
    }

    /// Rebind `name` to a NEW value (a reassignment): it is no longer given away, and it takes
    /// the new value's `origin` and `alias_class` (it leaves its old class, whose other members
    /// still hold the old value). Inside an open branch point this holds only on that arm's path;
    /// the join (`end_branches`) decides what is true after it. No-op if the name is not found.
    /// O(frames + open branch points) time.
    pub fn rebind(&mut self, name: &str, origin: Origin, alias_class: u64) {
        let Some(frame_index) = self.frame_of(name) else {
            return;
        };
        let Some(defined_at) = self.frames[frame_index]
            .get(name)
            .map(|entry| entry.defined_at.clone())
        else {
            return;
        };
        let key = entry_key(frame_index, name, &defined_at);
        self.touch(&key);
        if let Some(branch) = self.branches.last_mut() {
            if key.0 < branch.arm_frame {
                branch.reassigned.insert(key.clone());
            }
        }
        if let Some(entry) = self.entry_mut(&key) {
            entry.consumed = None;
            entry.origin = origin;
            entry.alias_class = alias_class;
        }
    }

    /// Called BEFORE the entry `key` changes its path state: every open branch point whose arm
    /// it outlives remembers the entry's state when the branch point opened (the first change it
    /// sees is made from that state, since every arm starts from it) and that the current arm
    /// changed it. O(open branch points).
    fn touch(&mut self, key: &EntryKey) {
        let Some(before) = self
            .frames
            .get(key.0)
            .and_then(|frame| frame.get(&key.1))
            .map(PathState::of)
        else {
            return;
        };
        for branch in self.branches.iter_mut() {
            if key.0 < branch.arm_frame {
                branch
                    .opened
                    .entry(key.clone())
                    .or_insert_with(|| before.clone());
                branch.arm_touched.insert(key.clone());
            }
        }
    }

    fn entry_mut(&mut self, key: &EntryKey) -> Option<&mut ScopeEntry> {
        let entry = self.frames.get_mut(key.0)?.get_mut(&key.1)?;
        (entry.defined_at.start == key.2 && entry.defined_at.end == key.3).then_some(entry)
    }

    fn set_path_state(&mut self, key: &EntryKey, state: PathState) {
        if let Some(entry) = self.entry_mut(key) {
            entry.consumed = state.consumed;
            entry.origin = state.origin;
            entry.alias_class = state.alias_class;
        }
    }

    /// Open a branch point of `kind`: an `if` body, the arms of a match, a loop body. O(1).
    pub fn begin_branches(&mut self, kind: BranchKind) {
        self.branches.push(Branches {
            kind,
            arm_frame: 0,
            opened: BTreeMap::new(),
            arm_touched: BTreeSet::new(),
            arm_ends: Vec::new(),
            reassigned: BTreeSet::new(),
        });
    }

    /// An arm starts (its frame is already pushed).
    pub fn begin_arm(&mut self) {
        let innermost = self.innermost_frame();
        if let Some(branch) = self.branches.last_mut() {
            branch.arm_frame = innermost;
            branch.arm_touched.clear();
        }
    }

    /// An arm ends (before its frame pops). `returns`: it always returns, so it never reaches the
    /// join. The end state of every entry the arm changed is kept for the join, and the entry is
    /// put back to its state when the branch point opened, so the NEXT arm does not inherit this
    /// arm's gives or reassignments. O(entries the arm changed . log) time.
    pub fn end_arm(&mut self, returns: bool) {
        let Some(mut branch) = self.branches.pop() else {
            return;
        };
        let touched = std::mem::take(&mut branch.arm_touched);
        let mut ends = BTreeMap::new();
        for key in touched {
            let end_state = self.entry_mut(&key).map(|entry| PathState::of(entry));
            let (Some(end_state), Some(opened)) = (end_state, branch.opened.get(&key).cloned())
            else {
                continue;
            };
            ends.insert(key.clone(), end_state);
            self.set_path_state(&key, opened);
        }
        if !returns {
            branch.arm_ends.push(ends);
        }
        branch.arm_frame = 0;
        self.branches.push(branch);
    }

    /// Close the branch point. Every entry an arm changed is joined over every path that reaches
    /// here: each arm that reaches the join, and the path that enters no arm when `skip_path` (an
    /// `if`, a loop that may run zero times, a match with no else that is not exhaustive).
    /// - Given away on ANY path: given away (maybe given), so reading it is the use-after-give
    ///   error. When an arm reassigned it, its cause is marked with this branch point's kind, so
    ///   the read names the path that did not reassign it.
    /// - Origin: the one every path shares, else the first that cannot be given away (a give
    ///   must be allowed on every path).
    /// - Alias class: every path's class, merged into one (it may hold any of those values).
    ///
    /// The enclosing branch points already saw each change when it was made (`touch`).
    /// O(changed entries . paths + merges . entries) time.
    pub fn end_branches(&mut self, skip_path: bool) {
        let Some(mut branch) = self.branches.pop() else {
            return;
        };
        let keys: Vec<EntryKey> = branch.opened.keys().cloned().collect();
        for key in keys {
            let Some(opened) = branch.opened.get(&key).cloned() else {
                continue;
            };
            let mut paths: Vec<PathState> = branch
                .arm_ends
                .iter()
                .map(|ends| ends.get(&key).cloned().unwrap_or_else(|| opened.clone()))
                .collect();
            if skip_path {
                paths.push(opened.clone());
            }
            let Some(first) = paths.first().cloned() else {
                // No path reaches the join (every arm returns): nothing after it runs.
                continue;
            };
            let consumed = match paths.iter().find_map(|path| path.consumed.clone()) {
                Some(cause) if branch.reassigned.contains(&key) => {
                    Some(cause.marked_missed_revive(branch.kind))
                }
                other => other,
            };
            let origin = if paths.iter().all(|path| path.origin == first.origin) {
                first.origin.clone()
            } else {
                paths
                    .iter()
                    .map(|path| &path.origin)
                    .find(|origin| !origin_transferable(origin))
                    .unwrap_or(&first.origin)
                    .clone()
            };
            let kept = first.alias_class;
            let absorbed: BTreeSet<u64> = paths
                .iter()
                .map(|path| path.alias_class)
                .filter(|class| *class != kept)
                .collect();
            for class in absorbed {
                self.merge_classes(kept, class);
                branch.rename_class(kept, class);
            }
            self.set_path_state(
                &key,
                PathState {
                    consumed,
                    origin,
                    alias_class: kept,
                },
            );
        }
    }

    /// Join two alias classes into one: every entry of `absorbed` moves to `kept`. Used where
    /// one value comes to hold another (a literal built from several named values, a store into
    /// a container), so consuming or returning any member covers all of them, and at a branch
    /// join. The states the open branch points recorded follow, so a later restore or join
    /// cannot split the class again. O(entries + recorded states) time.
    pub fn merge_classes(&mut self, kept: u64, absorbed: u64) {
        if kept == absorbed {
            return;
        }
        for frame in self.frames.iter_mut() {
            for entry in frame.values_mut() {
                if entry.alias_class == absorbed {
                    entry.alias_class = kept;
                }
            }
        }
        for branch in self.branches.iter_mut() {
            branch.rename_class(kept, absorbed);
        }
    }

    /// Every alias class with at least one consumed entry in any frame — the snapshot a call
    /// form takes BEFORE it runs its transfer decisions, so `check_transfer` can tell a class
    /// consumed by an earlier statement (its read was reported when the argument was
    /// inferred) from one consumed by an earlier position of the SAME call (never reported
    /// anywhere else — v0.3-M8 Phase 4 fix round 2, the `eat2(rows, other)` alias pair).
    pub fn consumed_classes(&self) -> HashSet<u64> {
        let mut out = HashSet::new();
        for frame in &self.frames {
            for entry in frame.values() {
                if entry.consumed.is_some() {
                    out.insert(entry.alias_class);
                }
            }
        }
        out
    }

    /// All names currently in scope (all frames), for Levenshtein suggestions.
    pub fn all_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for frame in &self.frames {
            names.extend(frame.keys().map(String::as_str));
        }
        names
    }
}

/// An entry a transfer reached: where it lives now and which declaration it is.
#[derive(Clone, Debug)]
pub struct ConsumedEntry {
    pub frame_index: usize,
    pub name: String,
    pub defined_at: SourceSpan,
}
