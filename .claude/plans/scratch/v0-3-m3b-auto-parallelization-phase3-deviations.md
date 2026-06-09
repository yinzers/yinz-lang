# v0-3-m3b-auto-parallelization Phase 3 Deviations — captured 2026-06-07T04:30

D_count: 4

## Scope Deviations (verbatim / coordinator-consolidated)

- **D3 (scope, coordinator-noted — forced)** `crates/ynz-typeck/src/lib.rs` — added `WaitPointHint`, `BackgroundRoutingHint`, `wait_points_hints`, `background_routing_hints` to the crate's `pub use`. Forced consequence of adding the two new passes (the LSP crate consumes them). Not in the literal Phase-3 `Files (expected scope)` list but a mechanical export. Hunks: `crates/ynz-typeck/src/lib.rs`.

## Approach Deviations (verbatim from executor report)

- **D1 (approach, mechanical-zero-width)** plan pseudocode `format!("wait  ")` → code `"wait  ".to_string()`. Rationale: `format!` with no interpolation is a clippy `useless_format` error under `-D warnings`; `.to_string()` is the idiomatic equivalent with identical output. Diff hunks: `crates/ynz-lsp/src/inlay_hint.rs:321`. (Mechanical lint compliance, identical behavior — covered by code-reviewer/rules; no separate judge.)
- **D2 (approach, mechanical-zero-width)** plan `bg_span: SourceSpan` (by value) → code `bg_span: &SourceSpan` (reference). Rationale: `SourceSpan` holds a `String` and is not `Copy`; by-value in a reference-pattern match arm would force a clone per call site. Reference avoids the clone, no semantic difference. Diff hunks: `crates/ynz-typeck/src/inlay_hint_passes.rs:1502-1505`. (Mechanical idiom, identical behavior — no separate judge.)
- **D4 (test-update — IMMUTABLE-TEST ANGLE, judged)** the stale test `test_inlay_hint_wait_points_protocol_only_returns_empty_on_suspending_call` (which asserted `wait_points` returns EMPTY — the OLD protocol-only stopgap) was REPLACED with positive Domain-7/8 assertions. Rationale: Phase 3 LIFTS the protocol-only stopgap, so a test asserting "empty" now locks the wrong (superseded) behavior — same pattern as P1's `cant_infer`→`compiles_clean` renames. Diff hunks: `crates/ynz-lsp/tests/inlay_hint.rs`. (Needs an immutable-test-check judgment: is this a legit behavior-change update or test-weakening?)

## Resolved spawn list (orchestrator's parsed view) — judge D3 (scope) + D4 (test-update); D1/D2 mechanical-zero-width (no judge, covered by reviewers)

### Deviation #3 (D3 — lib.rs pub-use export)
- type: scope
- rationale: forced crate export of the 2 new passes + hint structs so the LSP crate can consume them.
- diff hunks: crates/ynz-typeck/src/lib.rs

### Deviation #4 (D4 — stale protocol-only test replaced with positive assertions)
- type: approach (test-update)
- rationale: P3 lifts the wait_points protocol-only stopgap; the old "returns empty" test locked the superseded behavior. Replaced with positive Domain-7/8 assertions.
- diff hunks: crates/ynz-lsp/tests/inlay_hint.rs

---
## Round-2 (post-gate fix) added deviation — captured 2026-06-07T05:10

D_count (cumulative): 5

- **D5 (approach, R2)** the effective-suspend-set construction (`check.suspends_set` + imported `sig.suspends` names) was built INLINE in both `wait_points_hints` and `background_routing_hints` rather than extracted to a shared helper. Rationale: no shared helper exists; the construction is 4 lines × 2 call sites; extracting would thread (db, source) or (check, sigs) into a helper for no payoff at this call-site count; each function carries a `// WHY:` comment naming the codegen anchor (`crates/ynz-codegen/src/queries.rs:90-94`) so the two cannot silently drift. Coordinator-verified byte-identical to the codegen construction. Diff hunks: `crates/ynz-typeck/src/inlay_hint_passes.rs:1278-1281, crates/ynz-typeck/src/inlay_hint_passes.rs` (the background_routing twin).

### Deviation #5 (D5 — inline effective-set, no shared helper)
- type: approach
- rationale: 4-line construction × 2 call sites; WHY-comment anchored to queries.rs:90-94 to prevent drift; coordinator-verified byte-identical to codegen.
- diff hunks: crates/ynz-typeck/src/inlay_hint_passes.rs

---
## Round-3 (judge-D5 BLOCK fix) — captured 2026-06-07T06:00

D5 RESOLVED: the 4 inline effective-set copies are DELETED; replaced by a single shared `build_effective_suspend_set(base, imported_fns)` helper in `crates/ynz-typeck/src/signatures.rs:83`, exported from `lib.rs:78`, consumed at all 4 sites (queries.rs:92, emit.rs:920, inlay_hint_passes.rs:1280, inlay_hint_passes.rs:1506). Coordinator-verified: 31/31 codegen golden IR snapshots byte-identical → behavior-preserving refactor. code-reviewer concerns #1 (allocators comment lie) + #2 (wait_points description) fixed.

D_count (this round, new): 1

- **D6 (scope, AUTHORIZED — judge-D5 fix)** the helper extraction touched `crates/ynz-codegen/src/queries.rs` + `crates/ynz-codegen/src/emit.rs` + `crates/ynz-typeck/src/signatures.rs` (helper home) — outside Phase 3's original inlay/lsp/registry/tests scope. AUTHORIZED in the Findings Log as the judge-D5 BLOCK resolution (extract shared helper consumed at all 4 sites, de-dups the pre-existing codegen pair). Pure refactor — golden snapshots byte-identical. Diff hunks: `crates/ynz-typeck/src/signatures.rs:83-98, crates/ynz-codegen/src/queries.rs:92, crates/ynz-codegen/src/emit.rs:920`.

### Deviation #6 (D6 — codegen scope expansion for the shared helper)
- type: scope (authorized)
- rationale: judge-D5 fix; extract one SSOT helper consumed at all 4 effective-set sites incl. the 2 codegen ones; behavior-preserving (31/31 golden snapshots byte-identical).
- diff hunks: crates/ynz-typeck/src/signatures.rs, crates/ynz-codegen/src/queries.rs, crates/ynz-codegen/src/emit.rs
