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
    Given { callee: String, given: String },
    /// Sent into `channel` (a `channel<T>.send` / `h.send` of an owned-heap payload); `sent`
    /// is the binding that was named at the send — it differs from the consumed name when
    /// the consumption reached this entry through its alias class.
    Sent { channel: String, sent: String },
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
    /// Mint for alias-class ids — each fresh value gets its own class.
    next_class: u64,
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
            next_class: 1,
        }
    }

    pub fn push(&mut self) {
        self.frames.push(HashMap::new());
    }

    pub fn pop(&mut self) {
        if self.frames.len() > 1 {
            self.frames.pop();
        }
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
    pub fn consume(&mut self, name: &str, cause: ConsumedBy) {
        let Some(class) = self.class_of(name) else {
            return;
        };
        for frame in self.frames.iter_mut() {
            for entry in frame.values_mut() {
                if entry.alias_class == class && entry.consumed.is_none() {
                    entry.consumed = Some(cause.clone());
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
