# v0-3-m3b-auto-parallelization Phase 2 Deviations — captured 2026-06-07T03:40 (Round-3 re-gate, post heap-deep-copy fix)

D_count: 9

Phase 2 grew well beyond its planned typeck-only scope because the plan rested on a false "codegen already honors give/copy" assumption (same defect as Phase 1). Patrick decided Option C ("fix the heap deep-copy now"). The diff now spans typeck + codegen + a runtime ABI extension. Deviations below; R1-judged ones (D1-D5) carry their prior PASS (their code is unchanged since Round 1) but are re-listed for the cumulative record.

## Scope Deviations (verbatim / coordinator-consolidated)

- **D1 (scope, R1, prior PASS)** `crates/ynz-typeck/src/queries.rs` — 1-line forced init of the new `TypedModule.background_arg_inferred_ownership` field in the salsa cycle-initial construction. Hunks: `crates/ynz-typeck/src/queries.rs:223`.
- **D2 (scope, R1, prior PASS)** `crates/ynz-typeck/tests/inlay_hint_ownership_ufcs.rs` — 2 new pass tests for the give/copy hint (AC4's home). Hunks: `crates/ynz-typeck/tests/inlay_hint_ownership_ufcs.rs`.
- **D3 (scope, R1, prior PASS)** `crates/ynz-driver/tests/integration.rs` — fixture-assertion integration tests (now ~10 `v03_m3b_p2_*` incl. UAF/alias/alloc-balance). Canonical home; mandated by Phase-2 Verification `cargo test --workspace`. Hunks: `crates/ynz-driver/tests/integration.rs`.
- **D4 (scope, AUTHORIZED — Option C codegen + runtime ABI)** `crates/ynz-codegen/src/emit.rs` + `crates/ynz-runtime/src/runtime.rs` + `crates/ynz-runtime/src/lib.rs` + `crates/ynz-codegen/src/runtime_decls.rs` + `crates/ynz-runtime/tests/m2_runtime.rs` + 13 `crates/ynz-codegen/tests/snapshots/golden__*.snap`. The heap-deep-copy fix: `prepare_bg_arg_for_ctx`/`BgArgFreeKind`/`emit_bg_arg_frees` (heap-alloc the copy via `ynz_alloc`, free on task completion), `ynz_array_clone_primitive` runtime fn (real array<primitive> copy), and the **`ynz_rt_spawn` ABI extended 4→6 params** (`arg_drop_ptr`/`arg_drop_count` so `SpawnStateFnFuture::drop` frees the SM-path arg-copies). m2_runtime.rs + the 13 snapshots are MECHANICAL consequences of the 6-param signature. Authorized in the Findings Log (Patrick Option C). Hunks: `crates/ynz-codegen/src/emit.rs`, `crates/ynz-runtime/src/runtime.rs`, `crates/ynz-runtime/src/lib.rs`, `crates/ynz-codegen/src/runtime_decls.rs`.
- **D5 (scope, R2)** `crates/ynz-lsp/src/inlay_hint.rs` — forced 1-line removal of the hint-gate that matched the now-removed `.give` what_instead string. Hunks: `crates/ynz-lsp/src/inlay_hint.rs:285`.
- **D6 (scope, R2)** `crates/ynz-diagnostics/tests/jargon_audit.rs` — extended the diagnostic-string source-scan to catch `inferred`/`inference` (closes the `jargon-audit-dual-test-pattern` gap that let the R1 banned-jargon slip). Hunks: `crates/ynz-diagnostics/tests/jargon_audit.rs`.

## Approach Deviations

- **D7 (approach, R1, prior PASS)** typeck-local `BgOwnership { Give, Copy }` enum (AST `OwnershipModifier` has no Copy variant). Hunks: `crates/ynz-typeck/src/check.rs`.
- **D8 (approach, R3)** heap-upgrade extended to the **Give path** too, not just Copy. Rationale: a Give+Shape+nested-spawner has the IDENTICAL stack-alloca UAF (the copy dies with the spawner's frame regardless of the give/copy label). Restricting to Copy-only would leave a silent UAF, violating Patrick's binding invariant. Hunks: `crates/ynz-codegen/src/emit.rs`.
- **D9 (approach, R3)** SM-path arg-copy free via **mechanism (2) — runtime future-drop** (`BgArgDropEntry` descriptor + `SpawnStateFnFuture::drop` frees arg-copies before the frame), the plan-offered "more robust (frees on cancellation too)" option, over mechanism (1) codegen-at-terminals. Hunks: `crates/ynz-runtime/src/runtime.rs`, `crates/ynz-codegen/src/emit.rs`.

## Resolved spawn list (orchestrator's parsed view) — re-gate judges D4/D6/D8/D9 (substantive new); D1/D2/D3/D5/D7 carry prior-round PASS

### Deviation #4 (D4 — codegen + runtime ABI heap-copy)
- type: scope (authorized)
- rationale: Option C heap-alloc + free-discipline + ynz_rt_spawn 4→6 ABI; m2_runtime + 13 snapshots mechanical.
- diff hunks: crates/ynz-codegen/src/emit.rs, crates/ynz-runtime/src/runtime.rs

### Deviation #6 (D6 — jargon source-scan)
- type: scope
- rationale: extended jargon_audit to catch inferred/inference in diagnostic strings (closes the gap that let the R1 jargon slip).
- diff hunks: crates/ynz-diagnostics/tests/jargon_audit.rs

### Deviation #8 (D8 — Give-path heap-upgrade)
- type: approach
- rationale: Give+Shape+nested-spawner has identical UAF; heap-upgrade must apply to Give too or a silent UAF survives.
- diff hunks: crates/ynz-codegen/src/emit.rs

### Deviation #9 (D9 — SM runtime future-drop free)
- type: approach
- rationale: runtime future-drop mechanism for SM-path arg-copy free (frees on cancellation; mirrors frame-drop).
- diff hunks: crates/ynz-runtime/src/runtime.rs, crates/ynz-codegen/src/emit.rs
