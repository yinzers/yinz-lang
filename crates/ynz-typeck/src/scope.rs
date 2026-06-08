use std::collections::HashMap;

use ynz_ast::nodes::OwnershipModifier;
use ynz_diagnostics::SourceSpan;

use crate::types::Type;

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
    /// True after ownership was transferred via a `give` parameter.
    /// Any subsequent use of this binding produces a use-after-give error.
    pub is_consumed: bool,
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

    /// Mark a binding as consumed (ownership transferred). No-op if not found.
    pub fn consume(&mut self, name: &str) {
        for frame in self.frames.iter_mut().rev() {
            if let Some(entry) = frame.get_mut(name) {
                entry.is_consumed = true;
                return;
            }
        }
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
