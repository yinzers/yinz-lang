use std::collections::{HashMap, HashSet};

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
    /// The open branch points (an `if`, a match, a loop), innermost last: where a revive is made
    /// on ONE path and the join must not believe it covers the others.
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

/// What the join after a branch point needs (see [`Scope::begin_branches`]).
struct Branches {
    /// The statement that opened it.
    kind: BranchKind,
    /// Entries consumed when the branch point opened, with their cause.
    consumed_before: HashMap<EntryKey, ConsumedBy>,
    /// Frame index of the arm being checked: only entries BELOW it outlive the arm.
    arm_frame: usize,
    /// Entries revived in the current arm, with the state each had just before its first revive.
    arm_revived: Vec<EntryKey>,
    arm_prior: HashMap<EntryKey, ConsumedBy>,
    /// Entries the current arm itself gave away (so they were live when the arm began).
    arm_consumed: HashSet<EntryKey>,
    /// Per entry revived in some arm: how many arms that reach the join recorded it, and the
    /// cause it was left consumed with by the first arm that left it consumed.
    recorded: HashMap<EntryKey, (usize, Option<ConsumedBy>)>,
    /// Arms that reach the join (an arm that always returns does not).
    arms_reaching_join: usize,
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

    /// Mutable form of [`Scope::lookup`] — for the binding-event rule (a reassignment
    /// changes the entry's origin, class and consumed state in place).
    pub fn lookup_mut(&mut self, name: &str) -> Option<&mut ScopeEntry> {
        for frame in self.frames.iter_mut().rev() {
            if let Some(e) = frame.get_mut(name) {
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
        for (frame_index, frame) in self.frames.iter_mut().enumerate() {
            for (entry_name, entry) in frame.iter_mut() {
                if entry.alias_class == class && entry.consumed.is_none() {
                    entry.consumed = Some(cause.clone());
                    reached.push(ConsumedEntry {
                        frame_index,
                        name: entry_name.clone(),
                        defined_at: entry.defined_at.clone(),
                    });
                }
            }
        }
        for entry in &reached {
            let key = entry_key(entry.frame_index, &entry.name, &entry.defined_at);
            for branch in self.branches.iter_mut() {
                branch.arm_consumed.insert(key.clone());
            }
        }
        reached
    }

    /// Revive `name`: a reassignment gives it a NEW value, so it is no longer given away on the
    /// path that reaches here. Inside an open branch point the revive covers only that arm; the
    /// join (`end_branches`) decides what is true after it.
    /// O(frames + revived entries in the open arm) time.
    pub fn revive(&mut self, name: &str) {
        let Some(frame_index) = self.frame_of(name) else {
            return;
        };
        let Some(entry) = self.frames[frame_index].get_mut(name) else {
            return;
        };
        let Some(prior) = entry.consumed.take() else {
            return;
        };
        let key = entry_key(frame_index, name, &entry.defined_at);
        if let Some(branch) = self.branches.last_mut() {
            if frame_index < branch.arm_frame && !branch.arm_revived.contains(&key) {
                branch.arm_prior.insert(key.clone(), prior);
                branch.arm_revived.push(key);
            }
        }
    }

    fn entry_mut(&mut self, key: &EntryKey) -> Option<&mut ScopeEntry> {
        let entry = self.frames.get_mut(key.0)?.get_mut(&key.1)?;
        (entry.defined_at.start == key.2 && entry.defined_at.end == key.3).then_some(entry)
    }

    /// Open a branch point of `kind`: an `if` body, the arms of a match, a loop body. Records
    /// which entries are given away now. O(entries).
    pub fn begin_branches(&mut self, kind: BranchKind) {
        let mut consumed_before = HashMap::new();
        for (frame_index, frame) in self.frames.iter().enumerate() {
            for (name, entry) in frame {
                if let Some(cause) = &entry.consumed {
                    consumed_before.insert(
                        entry_key(frame_index, name, &entry.defined_at),
                        cause.clone(),
                    );
                }
            }
        }
        self.branches.push(Branches {
            kind,
            consumed_before,
            arm_frame: 0,
            arm_revived: Vec::new(),
            arm_prior: HashMap::new(),
            arm_consumed: HashSet::new(),
            recorded: HashMap::new(),
            arms_reaching_join: 0,
        });
    }

    /// An arm starts (its frame is already pushed).
    pub fn begin_arm(&mut self) {
        let innermost = self.innermost_frame();
        if let Some(branch) = self.branches.last_mut() {
            branch.arm_frame = innermost;
            branch.arm_revived.clear();
            branch.arm_prior.clear();
            branch.arm_consumed.clear();
        }
    }

    /// An arm ends (before its frame pops). `returns`: it always returns, so it never reaches the
    /// join. What the arm revived is recorded for the join and then put back to the state before
    /// the branch point, so the NEXT arm does not inherit this arm's revive.
    /// O(revived entries in the arm) time.
    pub fn end_arm(&mut self, returns: bool) {
        let Some(mut branch) = self.branches.pop() else {
            return;
        };
        let revived = std::mem::take(&mut branch.arm_revived);
        for key in revived {
            let end_state = self
                .entry_mut(&key)
                .and_then(|entry| entry.consumed.clone());
            if !returns {
                let slot = branch.recorded.entry(key.clone()).or_insert((0, None));
                slot.0 += 1;
                if slot.1.is_none() {
                    slot.1 = end_state;
                }
            }
            // Put it back to what the NEXT arm should see: live if this arm gave it away itself
            // (it was live when the arm began), else what it was when the arm revived it.
            let restored = if branch.arm_consumed.contains(&key) {
                None
            } else {
                branch.arm_prior.get(&key).cloned()
            };
            if let Some(entry) = self.entry_mut(&key) {
                entry.consumed = restored;
            }
        }
        if !returns {
            branch.arms_reaching_join += 1;
        }
        self.branches.push(branch);
    }

    /// Close the branch point. An entry revived in an arm is live after it only if EVERY path
    /// that reaches the join revived it: each arm that reaches the join, and the path that
    /// enters no arm when `skip_path` (an `if` without an else, a loop that may run zero times, a
    /// match with no else that is not exhaustive). Otherwise it stays given away (maybe given),
    /// and reading it is the use-after-give error; its cause is marked with this branch point's
    /// kind, so the read names the path that did not reassign it. A revive that holds at the join
    /// is a revive for the enclosing branch point too. O(revived entries).
    pub fn end_branches(&mut self, skip_path: bool) {
        let Some(branch) = self.branches.pop() else {
            return;
        };
        let consumed_before = branch.consumed_before;
        let kind = branch.kind;
        for (key, (recorded_arms, left_consumed)) in branch.recorded {
            let covered = !skip_path && recorded_arms == branch.arms_reaching_join;
            let now = self.entry_mut(&key).map(|entry| entry.consumed.clone());
            let Some(now) = now else {
                continue;
            };
            let exit = left_consumed
                .or(if covered { None } else { now })
                .map(|cause| cause.marked_missed_revive(kind));
            if let Some(entry) = self.entry_mut(&key) {
                entry.consumed = exit.clone();
            }
            if exit.is_none() {
                if let Some(parent) = self.branches.last_mut() {
                    if key.0 < parent.arm_frame && !parent.arm_revived.contains(&key) {
                        if let Some(before) = consumed_before.get(&key) {
                            parent.arm_prior.insert(key.clone(), before.clone());
                        }
                        parent.arm_revived.push(key);
                    }
                }
            }
        }
    }

    /// Join two alias classes into one: every live entry of `absorbed` moves to `kept`. Used
    /// where one value comes to hold another (a literal built from several named values, a
    /// store into a container), so consuming or returning any member covers all of them.
    /// O(entries) time: scans every live entry of every frame.
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
