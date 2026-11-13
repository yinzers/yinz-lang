//! SPIKE S3 (scratch, never merged): the persisted scope-exit release record.
//!
//! Typeck is the ONLY producer: it already knows, at every scope exit and `return`, which
//! bindings were transferred (`Scope::consume`) and which are alias-class founders. It writes
//! one TOTAL list per exit point (every binding leaving scope appears, with its fate); codegen
//! reads the list and emits releases for `Fate::Release` entries only. Codegen never asks
//! "was this moved?" itself (authoritative-derivation.md).

use std::collections::HashMap;

use crate::types::Type;

/// Key of a binding: the span of its NAME at the binding site (`let` name / parameter name).
/// Unique per binding event, stable across shadowing, the same value typeck stores in
/// `ScopeEntry::defined_at`.
pub type BindingKey = (usize, usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseKind {
    /// `array<int|float|bool>`: header + flat buffer, `ynz_array_drop`.
    ArrayPrimitive,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fate {
    /// Sole owner, never transferred before this exit: codegen frees it here.
    Release,
    /// Transferred before this exit at the SAME nesting depth as the declaration: definitely moved.
    OmittedMoved,
    /// Transferred inside a nested (conditional / loop) scope before this exit: moved on some
    /// paths only. Prototype leaks (sound); the full design needs a drop flag (null-the-slot).
    OmittedMaybeMoved,
    /// Returned (or reachable from the returned value) at this exit.
    OmittedReturned,
    /// Not the sole owner (alias-class member that is not the founder, loop cell, `Reaches`, `Unknown`,
    /// borrowed parameter) — someone else frees the value.
    OmittedNotOwner,
    /// Heap type this prototype has no release for (shape, map, string, ...).
    OmittedUnsupportedType,
    /// Reassigned from a non-fresh value somewhere in the function: slot content not provably owned.
    OmittedReassigned,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseEntry {
    pub name: String,
    pub binding: BindingKey,
    pub ty: Type,
    pub kind: Option<ReleaseKind>,
    pub fate: Fate,
}

/// Exit-point record. `block_end` is keyed by `Block.span` (fallthrough end of a block, one
/// entry list per block — also the loop back-edge for per-iteration locals); `return_edge` by
/// the `Stmt::Return` span.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReleasePlan {
    pub block_end: HashMap<(usize, usize), Vec<ReleaseEntry>>,
    pub return_edge: HashMap<(usize, usize), Vec<ReleaseEntry>>,
}

/// The release kind of a type, if the prototype supports it.
pub fn release_kind_of(ty: &Type) -> Option<ReleaseKind> {
    match ty {
        Type::BuiltinArray { elem } if matches!(**elem, Type::Int | Type::Float | Type::Bool) => {
            Some(ReleaseKind::ArrayPrimitive)
        }
        _ => None,
    }
}

impl ReleasePlan {
    /// SPIKE S3 (crossing coupling): names of every binding that has `Fate::Release` in ANY exit list
    /// whose key span lies inside `[start, end)`. These are "used" at scope exit, so they must be
    /// in the crossing set when a suspension separates their declaration from that exit.
    /// Over-approximation: no suspension-position test. O(L) over the lists.
    pub fn release_owner_names_within(&self, start: usize, end: usize) -> Vec<String> {
        let mut names: Vec<String> = self
            .block_end
            .iter()
            .chain(self.return_edge.iter())
            .filter(|(key, _)| key.0 >= start && key.1 <= end)
            .flat_map(|(_, list)| list.iter())
            .filter(|e| e.fate == Fate::Release)
            .map(|e| e.name.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }
}
