# v0-3-m3e-cross-module-frame-serialization Phase 0 Deviations — captured 2026-06-06 (round 2)

D_count: 4

## Scope Deviations (verbatim from executor reports)

- queries.rs `cargo fmt` whitespace-only (round-1 original). Hunks: queries.rs:286-304, :400-402, :510-558. (Judge PASS round 1 — unchanged.)
- resolve_import.rs `cargo fmt` whitespace-only (round-1 original). Hunks: resolve_import.rs:414-421, :518-529, :542-553, :599-603. (Judge PASS round 1 — unchanged.)
- may_block.rs clippy fixes (collapsible-if collapse + justified `#[allow(clippy::too_many_arguments)]` + `#[allow(clippy::only_used_in_recursion)]` with WHY comment) to satisfy AC4 `cargo clippy -D warnings`. NEW (round-1 fix). File outside Phase 0 declared scope.
- emit.rs `map_or(false, |sig| ...)` → `is_some_and(|sig| ...)` clippy idiom fix (in-scope file, outside the target-machine area). NEW (round-1 fix). Hunk: emit.rs:11532.

## Approach Deviations (verbatim from executor report)

- **Deviation #1** (may_block.rs:448-458): `too_many_arguments` + `only_used_in_recursion` resolved with `#[allow]` + justification rather than a struct refactor. Rationale: all 8 params are independent recursive-tree-walk state; bundling into a struct used nowhere else is ceremony (no-duct-tape #2); thread-through params (`enclosing_fn`, `unresolvable`) are why `only_used_in_recursion` fires — the lint is technically correct but the design is correct.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1 — queries.rs fmt (CARRY: judge PASS round 1, byte-identical)
- type: scope · diff hunks: crates/ynz-typeck/src/queries.rs:286-304, :400-402, :510-558 · identity hash: phase0-dev-queries-fmt

### Deviation #2 — resolve_import.rs fmt (CARRY: judge PASS round 1, byte-identical)
- type: scope · diff hunks: crates/ynz-typeck/src/resolve_import.rs:414-421, :518-529, :542-553, :599-603 · identity hash: phase0-dev-resolveimport-fmt

### Deviation #3 — may_block.rs clippy #[allow] (NEW — judge round 2)
- type: scope+approach · diff hunks: crates/ynz-typeck/src/may_block.rs:448-458, :501 · identity hash: phase0-dev-mayblock-clippy

### Deviation #4 — emit.rs map_or→is_some_and (NEW — judge round 2)
- type: scope · diff hunks: crates/ynz-codegen/src/emit.rs:11530-11535 · identity hash: phase0-dev-emit-mapor

NOTE (coordinator): check.rs EC-dispatch fix from round-1 was REVERTED to base (empty diff) and RELOCATED to Phase 2 Step 1a to preserve Phase 0's no-behavior-change mandate. Not a deviation in the current diff.
