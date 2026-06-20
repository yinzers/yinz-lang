# v0-3-m3b-auto-parallelization Phase 4 Deviations — captured 2026-06-07T07:30

D_count: 5

Phase 4 = the auto-parallelize headline. Behavior is CORRECT per `design/concurrency.md` Model A (independent side effects intentionally reorder; `wait` orders them — coordinator-verified + resolved with Patrick: design doc wins). The plan's over-broad byte-identical ACs were CORRECTED by the coordinator (scoped to transparent programs). Two coordinator fix-rounds: (R1) aliasing soundness now enforced by the analysis; Model-A fixtures; harness flake fixed; one-hop docs reframed. Coordinator also corrected the `design/concurrency.md` "one-hop divergence" entry (it falsely claimed a share→lend hole that's a compile error in Yinz).

## Scope Deviations

- **D1 (scope, in-plan)** `crates/ynz-codegen/src/independence.rs` (NEW 747-line module) — the independence analysis. The plan's Step 1 explicitly allowed "a new analysis module consuming the producers." In scope. + `crates/ynz-codegen/src/lib.rs` (1-line module registration, forced).
- **D2 (scope, bookkeeping)** `.claude/todos.md` — filed the pre-existing aliased-shape × lend-across-wait write-back bug (found during P4 probing; NOT a P4 regression — identical in both modes).
- **D3 (scope, Phase-6-file — JUDGE)** `examples/pirates-roster/entrypoint.ynz` — added `wait` prefixes to the EXISTING demo-section calls. Rationale: Phase 4's auto-parallel makes the existing independent demo calls (`m3m2_demo()`, etc.) reorder their printed output (intended Model A behavior), which breaks the `examples_basics_runs_end_to_end` golden test. The `wait` prefixes restore source-order output. This is a Phase-6-scope file (demo); the change is correctness-preservation for the EXISTING demo under the now-active auto-parallel, NOT new Phase-6 demo content. JUDGE: is adding `wait` the right call, or should the golden test be updated to accept the reorder / deferred to Phase 6?

## Approach Deviations

- **D4 (approach — THE CORRECTNESS CRUX — JUDGE adversarially)** the aliasing guard: `stmts_are_independent` returns `false` (→ Singletons, sequential) when BOTH statements have ≥1 `lend` arg. Rationale: Yinz `let b = a` ALIASES (shapes are reference-semantics on assignment — coordinator-verified: mutating `b` mutates `a`), so two distinct binding names can alias one heap object; the per-binding write tracking CANNOT prove two `lend`-statements write different objects → conservative-correct sequentializes any lend-pair. This is MORE conservative than `design/concurrency.md` line 61 ("independent writes to different resources auto-parallelize") — that aspiration is unsafe for heap types given aliasing; only no-`lend`-arg (read-only) statements parallelize. Unit tests: `aliased_lend_pair_produces_singletons_not_parallel` + `no_lend_args_different_bindings_can_parallelize`. Diff hunks: `crates/ynz-codegen/src/independence.rs` (the lend-pair guard + module doc).
- **D5 (approach, documented divergence)** same-callee concurrent calls run sequentially (`worker(1); worker(2)` → sequential), because the composed frame allocates one sub-frame slot per unique callee NAME. Documented as a Design Divergence in `design/concurrency.md` with named cost + reversal path. Conservative-correct (always sequential = correct). Diff hunks: `crates/ynz-codegen/src/emit.rs`, `design/concurrency.md`.

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
- **HOLE C (code-reviewer BLOCK — FOUNDATIONAL)** — `share` not enforced read-only. The auto-parallel soundness premise (`design/concurrency.md:651`). Fixed at the ROOT.
- **HOLE D (rules BLOCK)** — 3 banned "previously" comments in integration.rs. Reworded.

- **D7 (scope, AUTHORIZED — Patrick green-lit "the correct long-term answer") — `share` read-only enforcement (typeck) added as Phase-4 soundness foundation.** This is M4-ownership-completion work (the "verify explicit modifier matches body" rule per `design/ownership.md:41`, specified in M4 plan:759 but never implemented) that M3b's auto-parallel newly REQUIRES. Building auto-parallel on the unenforced premise = the v0.3-M2-HALT mistake (feature on a missing foundation). The fix is 2-part and makes the analysis sound BY CONSTRUCTION:
  1. **typeck (`crates/ynz-typeck/src/check.rs`)**: explicit `share` param (incl. `share self`) + body mutation (field-assign / element-write / pass-to-`lend`-or-`give`) → three-part compile error. Bare param + mutation stays legal (= inferred lend). `ScopeEntry` gains `param_ownership`.
  2. **analysis (`crates/ynz-codegen/src/independence.rs`)**: write-effect = (heap type, via reused `is_trivially_copyable`) AND (declared ownership ≠ `Some(Share)`). Share-heap reads now parallelize (better than the `||`); give/bare/lend-heap sequentialize. Renamed `lend_*`→`write_*`.
  - JUDGE (deviation-judge, adversarial): is the conservative cost (bare-READ heap params don't parallelize because effective ownership isn't threaded into the sig table) sound + correctly named, and does the typeck enforcement correctly leave bare-mutation legal while rejecting explicit-share-mutation? Construct the smallest in-scope input where the fix overshoots (e.g. a legitimate bare-read param that should parallelize but doesn't, or a bare-mutation falsely rejected).
  - diff hunks: `crates/ynz-typeck/src/check.rs`, `crates/ynz-typeck/src/inlay_hint_passes.rs` (or types.rs — `is_trivially_copyable` pub), `crates/ynz-typeck/src/lib.rs`, `crates/ynz-codegen/src/independence.rs`, `crates/ynz-codegen/src/emit.rs`, `crates/ynz-driver/tests/integration.rs`, fixtures, `examples/primantis-orders/v0_3_m3b_errors.ynz`.

NOTE: after Part 1, `design/concurrency.md:651`'s "share → lend escalation is a compile error" becomes TRUE in the implementation (it was aspirational/false before). No design-doc change needed — the code is being made to match the doc.

---
## Round-4 (effective-ownership fixpoint — Patrick chose COMPLETE fix) — captured 2026-06-08

D_count (this round, new): 1 (the fixpoint is the authorized completion of Phase-4 Step-1's transitive write-effect, NOT a new deviation — but the cross-crate scope is noted)

Round-3's executor closed all DIRECT share-mutation cases but coordinator re-verify found a RESIDUAL: `fa(share x){ helper(x) }` (helper bare+mutating) compiled+printed 999 — the executor's escalation only checks the callee's DECLARED modifier, missing bare-inferred-lend. Patrick chose "Build the effective-ownership fixpoint now" (the COMPLETE design-correct fix, the plan's original Step-1, NOT a conservative-floor divergence).

- **D8 (scope, AUTHORIZED — Patrick-directed) — transitive effective-ownership fixpoint (`crates/ynz-typeck/src/effective_ownership.rs`, NEW 1176 lines).** Mirrors `may_block.rs` Kleene fixpoint; 3-valued `{Reads, Writes, Unknown}` lattice (unanalyzable → `Unknown` → conservative, NEVER silently Reads). Two decoupled consumers: typeck (`queries.rs` — emits the transitive `share`-violation diagnostic on declared-`share`+effective-`Writes`; `Unknown`→no error) + independence (`independence.rs` — write-position = `(effective ∈ {Writes,Unknown}) AND mutable-heap`). Cross-module imported fn → `Unknown` → sequentialize (sound; full propagation tracked follow-on with named cost). This fully enforces `concurrency.md:651` transitively + unlocks mutable-heap share-read parallelism. Diff hunks: `crates/ynz-typeck/src/effective_ownership.rs` (new), `crates/ynz-typeck/src/queries.rs`, `crates/ynz-typeck/src/check.rs`, `crates/ynz-typeck/src/scope.rs`, `crates/ynz-typeck/src/types.rs`, `crates/ynz-typeck/src/lib.rs`, `crates/ynz-codegen/src/independence.rs`, `crates/ynz-codegen/src/emit.rs`.
  - JUDGE (adversarial): does the fixpoint's `Unknown`-is-conservative invariant hold structurally (every unanalyzable construct → `Unknown` → treated-as-write by independence, never silently parallelized)? Construct the smallest in-scope input where the analysis might wrongly classify a write as `Reads` (e.g. a param flowing into a higher-order call, a method on a `dynamic` value, a generic param bounded by a contract with a `lend` method, a cross-module callee). Verify each lands `Unknown` or `Writes`, never `Reads`.

- **EXECUTOR DEATH + COORDINATOR RECOVERY (process note for the gate):** the opus fix-executor DIED on a 401 auth error ~18min in (115 tool-uses) after writing the analysis + wiring + unit tests + production code (PRODUCTION BUILD GREEN), but BEFORE: (a) fixing 2 test-side call sites it had broken (`tests/check.rs` `CheckOutput` initializer; `independence.rs:1042` gate-discrim test arity); (b) 3 clippy `redundant_closure` warnings in the new module; (c) the 3 INTEGRATION regression-lock fixtures + tests; (d) the TRANSITIVE gallery trigger. **The coordinator completed all of (a)-(d)** — these are coordinator edits to: `crates/ynz-typeck/src/effective_ownership.rs` (added `EffectiveOwnershipReport::empty()` + 3 closure-fix one-liners), `crates/ynz-typeck/tests/check.rs` (1 field), `crates/ynz-codegen/src/independence.rs` (1 test arg), `crates/ynz-driver/tests/integration.rs` (+5 tests), 3 new fixtures, `examples/primantis-orders/v0_3_m3b_errors.ynz` (transitive trigger). All coordinator-live-verified (V1-V6 + transparent sweep 22/22 + 5 new tests pass + clippy/jargon clean). The coordinator did NOT modify the executor's analysis logic — only completed the unfinished test/fixture/lint scaffolding the death interrupted.
