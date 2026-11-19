//! Which `for` loops hand their rows over, and which reads of the loop variable do (v0.3-M9 C1q,
//! plan #17; `IMP-ownership.md` "Loop rows").
//!
//! `for (row in matrix) { if (...) { kept.add(row) } }` is Rust's `for row in matrix`: when
//! `matrix`, and every name sharing it, is not read after the loop or inside it, a store of `row`
//! TAKES the row (no copy) and BLANKS its cell in `matrix`, so the release of `matrix` skips the
//! cell and frees the rows that stayed. When `matrix` is read later, `row` is a cell nobody may
//! hand over and the `.copy()` refusal stands (`TransferNeedsCopy`).
//!
//! The checker decides once (`Checker::check_stmt_for`, reading the one "read later" answer,
//! `Checker::class_used_after`) and writes it here; codegen reads this record and decides nothing
//! (`.claude/rules/authoritative-derivation.md`). A loop or a store that is not in the record
//! copies, so a hole costs a copy, never a double free.
//!
//! Cost: O(1) time per recorded loop or store, O(m) memory for m recorded spans.

use std::collections::HashSet;

/// The loops that hand their rows over, keyed by the span of the loop variable's declaration, and
/// the reads of such a variable that hand the row over, keyed by `(span.start, span.end)`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoopRowHandovers {
    loops: HashSet<(usize, usize)>,
    stores: HashSet<(usize, usize)>,
}

impl LoopRowHandovers {
    /// Record the loop whose variable is declared at `start..end` as handing its rows over.
    pub fn insert_loop(&mut self, start: usize, end: usize) {
        self.loops.insert((start, end));
    }

    /// Record the read of a loop variable at `start..end` as the hand-over of its row.
    pub fn insert_store(&mut self, start: usize, end: usize) {
        self.stores.insert((start, end));
    }

    /// True when the loop whose variable is declared at `start..end` hands its rows over.
    pub fn is_handover_loop(&self, start: usize, end: usize) -> bool {
        self.loops.contains(&(start, end))
    }

    /// True when the read at `start..end` hands a loop row over: the store takes the row and
    /// blanks its cell.
    pub fn is_handover_store(&self, start: usize, end: usize) -> bool {
        self.stores.contains(&(start, end))
    }

    /// The number of loops in the record.
    pub fn loop_count(&self) -> usize {
        self.loops.len()
    }
}
