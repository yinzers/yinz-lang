# v0-3-m2-wait-and-state-machines Phase 6 Deviations — captured 2026-06-01 (round 3)

D_count: 2

## Scope Deviations (verbatim from executor report)

- **Scope Deviation #1** (file: `crates/ynz-driver/tests/integration.rs`): touched outside declared scope. Rationale: `m5_dyn_dispatch_chained_both_calls_succeed` was weakened in round 2 alongside the Finding-#1 gate change — it was changed to expect a can't-infer error from a non-suspending caller. Reverting the gate restores the correct behavior (exit 0) but left the test asserting exit nonzero, causing an immediate test failure. The test required un-weakening simultaneous with the gate revert or the whole test suite would fail. Diff hunks: `crates/ynz-driver/tests/integration.rs:990-1004`.

## Approach Deviations (verbatim from executor report)

- **Deviation #1** (Change 2 — arg-loop dynamic gate): plan said `gate it on self.current_fn_suspends` by adding a separate outer `if` block; executor `incorporated the guard as an additional condition in the inner if expected_contract == actual_contract check`. Rationale: `adding an outer if block with the current_fn_suspends check would create an unclosed brace that breaks the indented structure; combining into the inner condition is equivalent and avoids a dangling-brace lint`. Diff hunks: `crates/ynz-typeck/src/check.rs:2039-2071`.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: scope
- **rationale**: un-weakening m5_dyn_dispatch_chained_both_calls_succeed (round-2 weakened it to expect can't-infer; reverted gate makes non-suspending dynamic callers compile clean → exit 0). Forced by the gate revert.
- **diff hunks**: crates/ynz-driver/tests/integration.rs:990-1004

### Deviation #2
- **type**: approach
- **rationale**: arg-loop dynamic gate combined `current_fn_suspends` into the inner `if expected_contract == actual_contract` condition rather than a separate outer `if` block (brace hygiene; equivalent).
- **diff hunks**: crates/ynz-typeck/src/check.rs:2039-2071
