//! Which heap-valued expressions a store takes by MOVE (v0.3-M9 C1f round 2, plan #17;
//! `IMP-unions.md` "Moving and copying a value that owns boxes").
//!
//! A shape, union or `maybe` that owns heap cells (a recursive union's boxes, `UnionLayout::boxed`)
//! is copied by bytes, and a byte copy shares its cells with its source: two owners of one box free
//! it twice (C6). Rust moves a value when its source is not used again and clones only on an
//! explicit `.clone()`; codegen does the same, and this is the record it reads to tell the cases
//! apart. It decides nothing itself (`.claude/rules/authoritative-derivation.md`): an expression
//! that is not in the record is copied DEEP, so a hole here costs cells, never a double free.
//!
//! There is no liveness walk here, because the language already has one decision for heap values:
//! a store takes the value over and the checker refuses every read of a handed-over binding
//! (`Checker::check_transfer`, "storing a value hands it over"). A string is different: it is
//! copied on store, so it may be read afterwards, and `string_ownership` finds its last use. A
//! heap value is in this record exactly when the checker admitted its transfer as
//!
//! * a FRESH value no name holds (a literal, a call result whose every return is fresh, an
//!   explicit `.copy()`), or
//! * the WHOLE of an owned binding that the store consumed (`let moved = holder`,
//!   `holders.add(holder)`, a field of a literal), or
//! * a `return` of a fresh value or of an owned local (the caller takes the boxes).
//!
//! A read through a place that stays where it is (a field, an element, a parameter, a call that
//! returns its argument) is never in the record: the binding that takes it owns a deep copy.
//!
//! Cost: O(1) time per recorded store, O(m) memory for m recorded stores.

use std::collections::HashSet;

/// The `(span.start, span.end)` of every heap-valued expression a store takes by move.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeapMoves {
    spans: HashSet<(usize, usize)>,
}

impl HeapMoves {
    /// Record the expression at `start..end` as taken by move.
    pub fn insert(&mut self, start: usize, end: usize) {
        self.spans.insert((start, end));
    }

    /// True when the expression at `start..end` is taken by move, so a store may take its cells
    /// without cloning them.
    pub fn contains(&self, start: usize, end: usize) -> bool {
        self.spans.contains(&(start, end))
    }

    /// The `(start, end)` span of every expression in the record.
    pub fn spans(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.spans.iter().copied()
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
