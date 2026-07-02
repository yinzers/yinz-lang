---
name: "v0-3-m3b-auto-parallelization-audit"
plan-id: "2026-06-05-v0-3-m3b-auto-parallelization"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-06-05-v0-3-m3b-auto-parallelization

Migrated 2026-07-01 from the pre-migration `.claude/plans/` ledger format. This sidecar is a
best-effort mechanical migration: the `## Session log` below is reconstructed from the old
scratch/*-deviations.md files (concatenated verbatim, NOT reformatted into individual FRAGO
delta-records — that reformatting was out of scope for a drive-by migration). Historical
session-ids were not tracked pre-migration, so the frontmatter `session-id` list starts empty.

## Session log

(pre-migration history — see plan.md body's Findings Logs / "Committed:" lines for the
authoritative narrative; this section is the raw scratch/ deviation record)

## FRAGO log

(none recorded in the old format — deviations were logged inline in the plan body's
"Findings Log" per-phase, not as discrete FRAGO records; see plan.md)

## Migrated scratch/ deviation notes

### phase0-deviations.md

# v0-3-m3b-auto-parallelization Phase 0 Deviations — captured 2026-06-05

D_count: 3

## Scope Deviations (verbatim from executor report)

- **Scope Deviation #1** (file: `crates/ynz-registry/tests/lsp_adapter.rs`): touched outside declared scope. Rationale: `hover_wait_includes_what_what_instead_why` and `hover_background_includes_what_what_instead_why` asserted on old M2 hover text ("Suspends the calling function" / "separate thread"); updating the registry keyword hover fields to M3b semantics is the phase's explicit goal, so these tests were the correct counterpart — failing to update them would have left AC-green registry text with AC-red test assertions. Diff hunks: `crates/ynz-registry/tests/lsp_adapter.rs:122-172`.

- **Scope Deviation #2** (file: `crates/ynz-lsp/tests/hover.rs`): touched outside declared scope. Rationale: Same cause as Deviation #1 — `hover_wait_keyword_returns_m2_suspension_text` and `hover_background_keyword_returns_routing_distinction_text` in the LSP integration test suite asserted on old text that the registry update intentionally replaced; the `background` test used `mc.value.contains("I/O pool")` which does not match "I/O thread pool" (not a substring match). Leaving these tests red would block all subsequent phases. Diff hunks: `crates/ynz-lsp/tests/hover.rs:1-250` (approximate range covering the two updated test functions).

- **Scope Deviation #3** (file: `cspell.json`): touched outside declared scope. Rationale: "callees" appears in the new `background_routing` `description` field at `registry/features.toml:2077`; project memory `cspell-words-proactive` requires immediately adding flagged legitimate programming terms — deferring would leave a cspell failure in the repo. Diff hunks: `cspell.json:18-19`.

## Approach Deviations (verbatim from executor report)

None — implementation matched plan's named approaches.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: scope
- **rationale**: test-sync — `lsp_adapter.rs` hover assertions updated to M3b ordering-barrier / routing hover text (the explicit goal of P0 steps 4-5). Without it, AC-green registry text would have AC-red tests.
- **diff hunks**: crates/ynz-registry/tests/lsp_adapter.rs:122-172
- **judge identity hash**: 45fff33026fdbb1d416e0ec7578fb6b3abe39424

### Deviation #2
- **type**: scope
- **rationale**: test-sync — `crates/ynz-lsp/tests/hover.rs` LSP integration assertions updated; old text ("I/O pool" non-substring of "I/O thread pool") replaced. Leaving red blocks later phases.
- **diff hunks**: crates/ynz-lsp/tests/hover.rs:1-250
- **judge identity hash**: 069f9b01c300ce45dcdae13bc99712bd2d998ecd

### Deviation #3
- **type**: scope
- **rationale**: cspell word "callees" (appears in new `background_routing` description). Project memory `cspell-words-proactive` mandates immediate add.
- **diff hunks**: cspell.json:18-19
- **judge identity hash**: 45990061f93f34069c0aa157eff501b8e5e68a32

### phase1-deviations.md

# v0-3-m3b-auto-parallelization Phase 1 Deviations — CLOSE-OUT ROUND (round 3), captured 2026-06-05

Scope: Patrick split full cross-module suspension codegen → milestone M3e. P1 now ships WORKING cases + LOUD-REJECT guards for the rest (the M3a→M3c pattern). Close-out round: fixed the buggy `composed_frame_simple` predicate (round-1 used `imported_fn_names` not `imported_suspending_names` → guard didn't fire → SIGILL), stripped debug prints, added loud-reject fixtures + the M3e deferral doc + registry entry.

D_count: 4 (judged — all mechanical/forced) + 1 SAFETY-FLOOR probe (the guard predicate, in-scope step 8 — judged for escape-completeness, not as a deviation).

## SAFETY-FLOOR PROBE (not a deviation — the core of step 8)
- **The conservative loud-reject guard (`composed_frame_simple` on FunctionSig).** Must reject EVERY cross-module suspending combo outside the proven-working set. Adversarial target (code-reviewer + dedicated judge): find a combo that ESCAPES the guard (marked simple=true) and still silently CRASHES — 4-module chain, shape RETURN cross-module, number/decimal128/float value-return cross-module, loop-var crossing, mixed transitive+shape, transitive depth ≥2. Over-rejection = OK (clean error); escape-then-crash = BLOCK (violates Patrick's no-silent-failure floor).

## Documented deviations (mechanical — forced)
### J-C hover.rs — `composed_frame_simple: true` added to 3 FunctionSig literals (struct exhaustiveness, forced by in-scope signatures.rs field).
### J-D completion.rs — same, 4 literals.
### J-E tmgrammar — `cargo run -p ynz-tmgrammar` regenerated `ynz.tmLanguage.json` (forced: registry `[[deferred_language_feature]]` add would otherwise fail the grammar-drift snapshot test).
### J-F docs/internal/scratchpad/SCRATCH-future-designs-index.md — one table row added for the new M3e deferral doc (SSOT index completeness).

(Per coordinator judgment + deep context budget: the 4 mechanical deviations are verified by plan-adherence + rules-compliance in-band rather than 4 separate judge spawns — same inert class J-C/J-D already PASSed in round 2. The high-value adversarial spawn is the guard-escape judge.)

### phase2-deviations.md

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

### phase3-deviations.md

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

### phase4-deviations.md

# v0-3-m3b-auto-parallelization Phase 4 Deviations — captured 2026-06-07T07:30

D_count: 5

Phase 4 = the auto-parallelize headline. Behavior is CORRECT per [`docs/internal/implementation/IMP-concurrency.md`](../../../../docs/internal/implementation/IMP-concurrency.md) Model A (independent side effects intentionally reorder; `wait` orders them — coordinator-verified + resolved with Patrick: design doc wins). The plan's over-broad byte-identical ACs were CORRECTED by the coordinator (scoped to transparent programs). Two coordinator fix-rounds: (R1) aliasing soundness now enforced by the analysis; Model-A fixtures; harness flake fixed; one-hop docs reframed. Coordinator also corrected the [`docs/internal/implementation/IMP-concurrency.md`](../../../../docs/internal/implementation/IMP-concurrency.md) "one-hop divergence" entry (it falsely claimed a share→lend hole that's a compile error in Yinz).

## Scope Deviations

- **D1 (scope, in-plan)** `crates/ynz-codegen/src/independence.rs` (NEW 747-line module) — the independence analysis. The plan's Step 1 explicitly allowed "a new analysis module consuming the producers." In scope. + `crates/ynz-codegen/src/lib.rs` (1-line module registration, forced).
- **D2 (scope, bookkeeping)** [`.claude/todos.md`](../../../todos.md) — filed the pre-existing aliased-shape × lend-across-wait write-back bug (found during P4 probing; NOT a P4 regression — identical in both modes).
- **D3 (scope, Phase-6-file — JUDGE)** `examples/pirates-roster/entrypoint.ynz` — added `wait` prefixes to the EXISTING demo-section calls. Rationale: Phase 4's auto-parallel makes the existing independent demo calls (`m3m2_demo()`, etc.) reorder their printed output (intended Model A behavior), which breaks the `examples_basics_runs_end_to_end` golden test. The `wait` prefixes restore source-order output. This is a Phase-6-scope file (demo); the change is correctness-preservation for the EXISTING demo under the now-active auto-parallel, NOT new Phase-6 demo content. JUDGE: is adding `wait` the right call, or should the golden test be updated to accept the reorder / deferred to Phase 6?

## Approach Deviations

- **D4 (approach — THE CORRECTNESS CRUX — JUDGE adversarially)** the aliasing guard: `stmts_are_independent` returns `false` (→ Singletons, sequential) when BOTH statements have ≥1 `lend` arg. Rationale: Yinz `let b = a` ALIASES (shapes are reference-semantics on assignment — coordinator-verified: mutating `b` mutates `a`), so two distinct binding names can alias one heap object; the per-binding write tracking CANNOT prove two `lend`-statements write different objects → conservative-correct sequentializes any lend-pair. This is MORE conservative than [`docs/internal/implementation/IMP-concurrency.md`](../../../../docs/internal/implementation/IMP-concurrency.md) line 61 ("independent writes to different resources auto-parallelize") — that aspiration is unsafe for heap types given aliasing; only no-`lend`-arg (read-only) statements parallelize. Unit tests: `aliased_lend_pair_produces_singletons_not_parallel` + `no_lend_args_different_bindings_can_parallelize`. Diff hunks: `crates/ynz-codegen/src/independence.rs` (the lend-pair guard + module doc).
- **D5 (approach, documented divergence)** same-callee concurrent calls run sequentially (`worker(1); worker(2)` → sequential), because the composed frame allocates one sub-frame slot per unique callee NAME. Documented as a Design Divergence in [`docs/internal/implementation/IMP-concurrency.md`](../../../../docs/internal/implementation/IMP-concurrency.md) with named cost + reversal path. Conservative-correct (always sequential = correct). Diff hunks: `crates/ynz-codegen/src/emit.rs`, [`docs/internal/implementation/IMP-concurrency.md`](../../../../docs/internal/implementation/IMP-concurrency.md).

## Resolved spawn list — judges D3 (pirates-roster scope) + D4 (aliasing guard soundness — adversarial)

### Deviation #3 (D3 — pirates-roster wait-prefixes, Phase-6 file)
- type: scope
- rationale: Phase 4 auto-parallel reorders the existing demo's printed output (intended Model A); wait-prefixes restore source-order to keep the golden test green; correctness-preservation not new demo content.
- diff hunks: examples/pirates-roster/entrypoint.ynz

### Deviation #4 (D4 — aliasing guard: any-lend-pair → sequential)
- type: approach
- rationale: Yinz `let b = a` aliases; can't prove two lend-statements write different objects → sequentialize lend-pairs (conservative-correct). Only no-lend statements parallelize.
- diff hunks: crates/ynz-codegen/src/independence.rs

---
## Round-2 (gate-BLOCK fix) — captured 2026-06-07T09:00

D_count (this round, new): 0 (the 2 fixes were authorized BLOCK resolutions, not new deviations)

BLOCK 1 fixed: parallel-group let-binding now routes through the UNIFIED `bind_sm_return_value` (emit.rs ~5936) — number/float/string parallel returns compile + byte-identical (was: corpse-(a) i64-only crash). Coordinator-verified: `priceA()/priceB()` both `-> number` → default==nap `a:12.75/b:99.50`.
BLOCK 2 fixed: D4's aliasing guard `independence.rs:224` `&&`→`||` — any pair member with a `lend` arg → Singletons. Coordinator-verified (ordering, avoiding the shape-crossing-wait garbage confound): aliased share-read[60ms]+lend-write[10ms] → `read-done/write-done/done` in BOTH modes (sequentialized; read completes before write). Unit tests `aliased_lend_pair_produces_singletons` + `aliased_share_read_vs_lend_write_produces_singletons` both pass.
Concern 1 (env-var salsa side-channel) → 4-field deferral in todos (LSP/watch don't pass --no-auto-parallel; safe today). Concern 2 (stmt_reads cleanup) done. Concern 3 (comment) auto-fixed.

NOTE on D4 (now `||`): the guard sequentializes ANY pair where either statement has a `lend` arg — more conservative than the design's "independent writes parallelize" (forfeited because Yinz `let b=a` aliasing makes distinct heap binding names un-provably-distinct). Only no-`lend` (pure-read/no-arg) statements parallelize = the M3b I/O-overlap case.

---
## Round-3 (gate-R2 BLOCK fixes — FOUNDATIONAL) — captured 2026-06-07T~18:00

D_count (this round, new): 1 (authorized scope expansion; the other 3 are BLOCK resolutions, not new deviations)

Gate Round 2 (5 reviewers + judge D4, BASE `93b703a`) found FOUR holes. Coordinator reproduced all four live, escalated the foundational one (C) to Patrick, who said "the correct non-lazy long-term answer following our rules + design doc." Fixes dispatched to one opus fix-executor:

- **HOLE A (judge D4 BLOCK)** — `give` + bare-mutating params missed by the write-effect summary. Fixed within the Part-2 analysis rewrite (below).
- **HOLE B (code-reviewer BLOCK)** — `-> T errors` parallel return SEGFAULT; missing `Type::ErrorsCapable` arm in `load_sm_return_value_typed` (emit.rs ~4970). Corpse-(a) forked LOAD reader. Fixed by adding the EC arm so `bind_sm_return_value`'s StructValue arm fires.
- **HOLE C (code-reviewer BLOCK — FOUNDATIONAL)** — `share` not enforced read-only. The auto-parallel soundness premise (`docs/internal/implementation/IMP-concurrency.md:651`). Fixed at the ROOT.
- **HOLE D (rules BLOCK)** — 3 banned "previously" comments in integration.rs. Reworded.

- **D7 (scope, AUTHORIZED — Patrick green-lit "the correct long-term answer") — `share` read-only enforcement (typeck) added as Phase-4 soundness foundation.** This is M4-ownership-completion work (the "verify explicit modifier matches body" rule per `docs/internal/implementation/IMP-ownership.md:41`, specified in M4 plan:759 but never implemented) that M3b's auto-parallel newly REQUIRES. Building auto-parallel on the unenforced premise = the v0.3-M2-HALT mistake (feature on a missing foundation). The fix is 2-part and makes the analysis sound BY CONSTRUCTION:
  1. **typeck (`crates/ynz-typeck/src/check.rs`)**: explicit `share` param (incl. `share self`) + body mutation (field-assign / element-write / pass-to-`lend`-or-`give`) → three-part compile error. Bare param + mutation stays legal (= inferred lend). `ScopeEntry` gains `param_ownership`.
  2. **analysis (`crates/ynz-codegen/src/independence.rs`)**: write-effect = (heap type, via reused `is_trivially_copyable`) AND (declared ownership ≠ `Some(Share)`). Share-heap reads now parallelize (better than the `||`); give/bare/lend-heap sequentialize. Renamed `lend_*`→`write_*`.
  - JUDGE (deviation-judge, adversarial): is the conservative cost (bare-READ heap params don't parallelize because effective ownership isn't threaded into the sig table) sound + correctly named, and does the typeck enforcement correctly leave bare-mutation legal while rejecting explicit-share-mutation? Construct the smallest in-scope input where the fix overshoots (e.g. a legitimate bare-read param that should parallelize but doesn't, or a bare-mutation falsely rejected).
  - diff hunks: `crates/ynz-typeck/src/check.rs`, `crates/ynz-typeck/src/inlay_hint_passes.rs` (or types.rs — `is_trivially_copyable` pub), `crates/ynz-typeck/src/lib.rs`, `crates/ynz-codegen/src/independence.rs`, `crates/ynz-codegen/src/emit.rs`, `crates/ynz-driver/tests/integration.rs`, fixtures, `examples/primantis-orders/v0_3_m3b_errors.ynz`.

NOTE: after Part 1, `docs/internal/implementation/IMP-concurrency.md:651`'s "share → lend escalation is a compile error" becomes TRUE in the implementation (it was aspirational/false before). No design-doc change needed — the code is being made to match the doc.

---
## Round-4 (effective-ownership fixpoint — Patrick chose COMPLETE fix) — captured 2026-06-08

D_count (this round, new): 1 (the fixpoint is the authorized completion of Phase-4 Step-1's transitive write-effect, NOT a new deviation — but the cross-crate scope is noted)

Round-3's executor closed all DIRECT share-mutation cases but coordinator re-verify found a RESIDUAL: `fa(share x){ helper(x) }` (helper bare+mutating) compiled+printed 999 — the executor's escalation only checks the callee's DECLARED modifier, missing bare-inferred-lend. Patrick chose "Build the effective-ownership fixpoint now" (the COMPLETE design-correct fix, the plan's original Step-1, NOT a conservative-floor divergence).

- **D8 (scope, AUTHORIZED — Patrick-directed) — transitive effective-ownership fixpoint (`crates/ynz-typeck/src/effective_ownership.rs`, NEW 1176 lines).** Mirrors `may_block.rs` Kleene fixpoint; 3-valued `{Reads, Writes, Unknown}` lattice (unanalyzable → `Unknown` → conservative, NEVER silently Reads). Two decoupled consumers: typeck (`queries.rs` — emits the transitive `share`-violation diagnostic on declared-`share`+effective-`Writes`; `Unknown`→no error) + independence (`independence.rs` — write-position = `(effective ∈ {Writes,Unknown}) AND mutable-heap`). Cross-module imported fn → `Unknown` → sequentialize (sound; full propagation tracked follow-on with named cost). This fully enforces `concurrency.md:651` transitively + unlocks mutable-heap share-read parallelism. Diff hunks: `crates/ynz-typeck/src/effective_ownership.rs` (new), `crates/ynz-typeck/src/queries.rs`, `crates/ynz-typeck/src/check.rs`, `crates/ynz-typeck/src/scope.rs`, `crates/ynz-typeck/src/types.rs`, `crates/ynz-typeck/src/lib.rs`, `crates/ynz-codegen/src/independence.rs`, `crates/ynz-codegen/src/emit.rs`.
  - JUDGE (adversarial): does the fixpoint's `Unknown`-is-conservative invariant hold structurally (every unanalyzable construct → `Unknown` → treated-as-write by independence, never silently parallelized)? Construct the smallest in-scope input where the analysis might wrongly classify a write as `Reads` (e.g. a param flowing into a higher-order call, a method on a `dynamic` value, a generic param bounded by a contract with a `lend` method, a cross-module callee). Verify each lands `Unknown` or `Writes`, never `Reads`.

- **EXECUTOR DEATH + COORDINATOR RECOVERY (process note for the gate):** the opus fix-executor DIED on a 401 auth error ~18min in (115 tool-uses) after writing the analysis + wiring + unit tests + production code (PRODUCTION BUILD GREEN), but BEFORE: (a) fixing 2 test-side call sites it had broken (`tests/check.rs` `CheckOutput` initializer; `independence.rs:1042` gate-discrim test arity); (b) 3 clippy `redundant_closure` warnings in the new module; (c) the 3 INTEGRATION regression-lock fixtures + tests; (d) the TRANSITIVE gallery trigger. **The coordinator completed all of (a)-(d)** — these are coordinator edits to: `crates/ynz-typeck/src/effective_ownership.rs` (added `EffectiveOwnershipReport::empty()` + 3 closure-fix one-liners), `crates/ynz-typeck/tests/check.rs` (1 field), `crates/ynz-codegen/src/independence.rs` (1 test arg), `crates/ynz-driver/tests/integration.rs` (+5 tests), 3 new fixtures, `examples/primantis-orders/v0_3_m3b_errors.ynz` (transitive trigger). All coordinator-live-verified (V1-V6 + transparent sweep 22/22 + 5 new tests pass + clippy/jargon clean). The coordinator did NOT modify the executor's analysis logic — only completed the unfinished test/fixture/lint scaffolding the death interrupted.

### phase5-deviations.md

# v0-3-m3b-auto-parallelization Phase 5 Deviations — captured 2026-06-08

D_count: 1

Phase 5 was RE-SCOPED with Patrick's explicit approval. The original "implement `background`-spawned EC-wrapper copy-before-free / collect-on-completion" rested on a FALSE premise and was superseded.

## The deviation (D1 — SCOPE, Patrick-approved)

- **type**: scope (a documented plan-vs-reality correction, not a silent narrowing — surfaced LOUDLY to Patrick per no-duct-tape "legitimate inverse"; Patrick approved fixing the real bug).
- **What the plan said**: Phase 5 = lift the `ec-wrapper-collect-on-completion` deferral by implementing copy-before-free when a `background`-spawned `-> T errors` task's result is collected at the join.
- **What reality is**: that deferral is VACUOUS in M3b — its trigger ("collect a `background` task's result via its handle") cannot fire. `background` output capture does NOT exist in M3b: `let h = background ecFn()` → `Error: Capturing the output of background is not yet supported`; the handle-collection form (`.send`/`.receive`) ships as a separate later feature (`background-handle-form`, check.rs:1307). The inline-poll EC path (the deferral's own `substitute`) is "correct and complete" (coordinator-verified: parallel `-> number errors` collection is byte-exact). So "lifting" the deferral would require first building the handle-collection feature (out of M3b) OR writing copy-before-free for a collection path with no caller (speculative untestable code — verification.md violation).
- **What investigating it FOUND**: a REAL P4 codegen bug — a parallel-group return binding that crosses a SUBSEQUENT `wait` barrier → LLVM "Instruction does not dominate all uses" → compile crash. Auto-parallel-ONLY (`--no-auto-parallel` works), NOT EC-specific (fails for `-> int`), triggers with even one binding crossing. The P4 wait-barrier matrix tested barriers between/before groups, never "parallel group bound → later wait → use binding."
- **Patrick's decision (2026-06-08)**: "Long term right answer based on rules and design decisions. Especially no-duct-tape rule." → FIX THE BUG NOW (not loud-reject — that's duct tape: fails a valid program). Re-scope Phase 5 to: (a) fix the dominance bug (frame-back the bindings); (b) keep the `background` deferral deferred + correct `ships_in`; (c) lock the working inline-poll EC cases.
- **The fix**: parallel-group `let`-binding (emit.rs:5985-5996) routes through `bind_sm_result_and_flush` (the canonical sequential binder, emit.rs:5122) instead of bare `bind_sm_return_value`+`flush_crossing_local_if_needed`. That binder already branches crossing (store-into-existing-entry-block-alloca + flush) vs non-crossing (fresh alloca). One substitution; non-crossing path byte-identical; corpse-(a) honored (same machinery, no forked dispatch).
- **diff hunks**: `crates/ynz-codegen/src/emit.rs` (the binding substitution), `crates/ynz-driver/tests/integration.rs` (+10 tests), 5 new `v0_3_m3b_p5_*.ynz` fixtures, [`registry/features.toml`](../../../../registry/features.toml) (ships_in M3b→M4 + rewritten substitute/why/triggers), [`docs/internal/implementation/IMP-concurrency.md`](../../../../docs/internal/implementation/IMP-concurrency.md) (ECWrapperResultCollection gated on background-handle-form).

## JUDGE (deviation-judge — adversarial)

The fix claims a parallel-group binding now survives any subsequent suspension via the entry-block + frame-backed slot. Construct the smallest in-scope input where the fix is WIDER than its stated problem or MISSES a crossing case: e.g. a parallel binding crossing a `wait` INSIDE an `if`/`while`/`for` body; a 3+-member parallel group where only SOME bindings cross; a parallel binding that is BOTH crossing AND shadowed; a parallel binding of a shape/string (heap) type crossing a wait (does the heap value survive, or hit the pre-existing shape-crossing-wait corruption — and is that corruption identical in both modes = not a P5 regression?); a parallel binding used in the SAME group's later member. Verify each is byte-identical default↔--no-auto-parallel OR fails identically in both modes (pre-existing, not a P5 regression). PASS if the fix is sound + minimal; BLOCK if it overshoots or misses a crossing pattern.

