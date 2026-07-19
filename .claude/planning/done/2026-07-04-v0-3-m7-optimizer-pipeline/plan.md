---
name: "v0-3-m7-optimizer-pipeline"
plan-id: "2026-07-04-v0-3-m7-optimizer-pipeline"
status: "active"
roadmap-id: "2026-05-21-v0-3-concurrency-perf"
session-id: ["plan-author-2026-07-04-m7-optimizer", "plan-amend-2026-07-04-m7-blockers", "plan-amend-2026-07-04-m7-links", "plan-amend-2026-07-04-m7-phase6-yield", "gate4-signatures-2026-07-04", "executor-2026-07-16-patrick-triage-application", "executor-2026-07-16-phase0-spike", "conductor-2026-07-16-fable-model-override", "executor-2026-07-16-phase0-fixloop", "executor-2026-07-16-phase1-rootcause", "executor-2026-07-16-phase1-sweep-redgate", "executor-2026-07-16-phase1-fragoapply", "conductor-2026-07-16-phase2-dispatch", "executor-2026-07-16-phase2-fix-constructor", "executor-2026-07-16-phase2-frago004-reconcile", "executor-2026-07-16-phase2-fixloop-timing", "executor-2026-07-17-phase3-pipeline-flip", "executor-2026-07-17-phase3-tier-measurement", "executor-2026-07-17-frago005-007-apply", "executor-2026-07-17-phase3-r9-abifix", "executor-2026-07-17-phase3-frago009-fixround", "executor-2026-07-17-frago010-cleanup", "executor-2026-07-17-fr23-uaf-gate", "executor-2026-07-17-frago011-fr23-redlocks", "executor-2026-07-17-phase4-stackfix", "executor-2026-07-17-phase4-cleanup-round", "executor-2026-07-17-phase5-determinism-goldens", "executor-2026-07-17-phase5-stability-matrix", "executor-2026-07-17-frago013-fixround", "executor-2026-07-17-phase6-designnote", "executor-2026-07-17-phase6-transform", "executor-2026-07-17-phase6-fixloop-determinism", "executor-2026-07-17-phase6-review-closeout", "executor-2026-07-17-phase7-ab-harness", "executor-2026-07-17-phase7-rust-equiv", "executor-2026-07-17-phase7-benchdedup-fixround", "executor-2026-07-17-phase8-final-reconciliation", "executor-2026-07-17-phase8-fixround", "executor-2026-07-17-phase8-fixround2", "executor-2026-07-17-phase8-fixround3", "executor-2026-07-18-soa-lint-testfix", "executor-2026-07-18-frago016-phase9-insert", "executor-2026-07-18-phase9-fr23-fix", "executor-2026-07-18-phase9-fr23-fixloop", "executor-2026-07-18-phase9-fixloop-deferral-fr23-c2", "executor-2026-07-18-completion-gate-round2-cleanup", "executor-2026-07-18-completion-gate-round3-fr23-recursive", "executor-2026-07-18-completion-gate-round4-fr23-unify", "executor-2026-07-18-completion-gate-round6-fr23-audit-blocked", "conductor-2026-07-18-completion-gate", "executor-2026-07-18-frago023-default-deny-redesign", "executor-2026-07-19-frago024-round8-apply", "executor-2026-07-19-frago025-fr23-cleanup", "executor-2026-07-19-completion-gate-final-polish"]
created_at: "2026-07-04"
updated_at: "2026-07-19"
metadata:
  type: "plan"
---

# PLAN: v0.3-M7 — Optimizer Pipeline

> **Frontmatter status — `active`.** This OPORD was originally held at `paused` — the
> **conductor-set pre-approval state** — pending two external preconditions: **(1) Gate 4**, the
> orchestrator's human read-through/approval checkpoint, and **(2) the M6-merge precondition** (¶1
> Friendly forces; CCIR item 1, requiring v0.3-M6 to merge to `main` before Phase 1 begins). **Both
> preconditions are now satisfied:** Gate 4 signed on 2026-07-04 (see audit.md session
> `gate4-signatures-2026-07-04`), and M6 merged to `main` as v0.3.2 (commits `0ac76d5` / `10df6d7`,
> 2026-07-16). Status correctly flipped to `active` per the status lifecycle
> ([`REF-plan-format.md`](../../../../../.claude/docs/reference/REF-plan-format.md)).

## 1. Situation

**Terrain (landscape).** The Yinz compiler (`crates/ynz-codegen`, `crates/ynz-typeck`,
`crates/ynz-runtime`) emits every code path — arrays, shapes, the concurrency state-machine engine,
channels, Arc ops — through exactly two `TargetMachine` creation sites
(`crates/ynz-codegen/src/state_machine.rs:755` — the shared `default_target_machine()` constructor,
which `emit_artifact`'s default/`None`-triple branch already routes through (`emit.rs:887`) — and the
explicit-`target_triple` override branch at `crates/ynz-codegen/src/emit.rs:888-905`; citations
re-anchored per FRAGO 001, audit.md), both hardcoded
to `OptimizationLevel::None`. This is the ONLY optimization-level configuration point in the entire
codegen crate (grep-confirmed, roadmap capability-discovery 2026-07-04). Zero LLVM pass-pipeline code
exists anywhere in the workspace (`run_passes`/`PassBuilderOptions`/`PassManager` — zero hits,
repo-wide grep). `inkwell` is pinned at `0.9.0` (`llvm18-1-prefer-dynamic` feature,
[`Cargo.toml`](../../../../Cargo.toml):29) — whether it cleanly exposes LLVM 18's new-PM
`run_passes` API is **unverified**; this is 100% net-new code for this milestone.

A baseline-verified spike flipped both sites to `OptimizationLevel::Default` and found **6/470**
`ynz-driver` integration failures — ALL on `number` (decimal128, 16-byte) crossing-local or EC-collect
paths, direct-repro SIGSEGV confirmed; every structurally-identical int/bool/float/string/shape/array/map
sibling passed. The **primary durable evidence** of this verdict is
[`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)'s
"Phase-0 spike — O0 → Default optimization" section (the exact failing-fixture list, the direct-repro
SIGSEGV, and the Fable-verified caveat that a GREEN verdict here could be a false negative). The spike's
mechanical 2-line change itself is preserved as a checked-in unified diff at
[`spike-o0-flip.patch`](./spike-o0-flip.patch) (plan-relative, this directory) — reconstructed and
byte-verified against this repo's current tree, replacing the original throwaway worktree, which was
gitignored, uncommitted, lived in a different clone, and had its branch reused (no longer preserves
anything). Phase 1 reads `spike-o0-flip.patch` directly rather than re-deriving the mechanical change
from memory; it reads the audit doc's section for the evidence of what that change broke. Two code
comments (`emit.rs:9961-9963`, `emit.rs:10717-10719`) blame `mem2reg`, but `TargetMachine`'s
`OptimizationLevel` drives **backend** passes (ISel/regalloc/scheduling) at codegen time, not the
mid-end IR pipeline where `mem2reg`/SROA/DCE live — that attribution is an **unverified theory**, not a
settled fact. This plan's Phase 1 exists specifically to stop that theory being carried forward
unverified.

Two other confirmed, independent bugs sit in this same neighborhood and must not be conflated with the
optimizer question itself: (a) a **general hot-loop O0 stack-exhaustion SIGSEGV** at ~4.19M total
loop-visits, reproducible in BOTH AoS and SoA layout modes, root-caused to nothing yet (roadmap
Capability Ledger, "General hot-loop O0 stack-exhaustion ceiling fix," Patrick-signed next-fix-priority
BUG per his 2026-07-04 triage policy) — the `soa_calibration.rs` bench harness caps `TOTAL_VISITS` at
131,072 specifically to dodge it; (b) `ynz run` masks a SIGSEGV as a diagnostic-free exit code `1`
(`crates/ynz-driver/src/run.rs:75`, `status.code().unwrap_or(1)`) — flagged for **M6**, not this plan.

**Weather (external constraints).** CI is Linux-only (`ubuntu-latest`, LLVM 18 via apt,
`LLVM_SYS_181_PREFIX` pinned — [`.github/workflows/ci.yml`](../../../../.github/workflows/ci.yml)).
All cargo commands run via `docker compose run --rm dev ...` (no `-it`, non-interactive) per this
project's dev-container convention. `crates/ynz-codegen/tests/golden.rs` records IR-text and
object-file SHA-256 goldens for `x86_64-unknown-linux-gnu` only — this plan's Phase 5 invalidates and
regenerates **every one** of them; this is expected, in-scope work, not scope creep.

**Friendly forces.** This plan branches from `main` **after** the sibling hotfix plan (v0.3-M6, authored
in parallel, not yet a file on disk under `.claude/planning/`) merges. M6 owns the concurrency-release
audit's correctness findings (P1-1 UFCS suspension invisibility, P3-1/P2-2 `pending_sends` ABA, P3-2
lost-wakeup, P4-3 unasserted `block_on` fallback, P2-4 buffered-element leak, P3-3 shutdown-mutex scope,
and the `ynz run` signal-masking bug) — this plan does **not** re-fix any of them.
**Assumption (unverified):** M6 ships and merges before this plan's Phase 1 begins. This is the exact
sequencing the assembling brief states as Patrick's intent, not an assumption this plan invents — but
since no M6 plan file exists yet at authoring time, the executor MUST confirm M6 is merged to `main`
before starting Phase 1 (see Coordinating Instructions CCIR). If M6 has not merged, halt and report —
do not build atop unmerged, unstable correctness fixes.

**Assumptions:**
- M6 merges before Phase 1 starts — **unverified**, confirm at Phase 1 kickoff.
- `inkwell` 0.9.0 exposes enough of LLVM 18's PassBuilder surface to build a `run_passes`-equivalent
  pipeline — **unverified**, Phase 0 is the gating spike.
- The spike's 6 failures are a **symptom** of a broader class (missing/incorrect LLVM attributes on
  runtime FFI declarations, or genuine frame/stack-slot handling under real optimization) rather than
  an isolated decimal128-only bug — **unverified**, Phase 1's exhaustive sweep is the test.
- The `--no-auto-parallel` / `default_target_machine` precedent (already the single shared constructor
  in `state_machine.rs`) is the correct place to thread the new pipeline config — **verified** by direct
  reading of both call sites (Situation, above); this plan extends it rather than re-deriving a second
  config path, per [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md).

**Cross-cutting factor sweep** (folds into the risk table below and the phase texts; factors with no
applicable risk get their one-line "N/A — why" here, per the mandatory-factors discipline):
- **Security / PII / compliance / SEO / accessibility:** N/A — this milestone changes a compiler
  codegen tier; no new external input, no new UI/IDE surface, no new user data, no web-facing surface.
- **Type-safety:** N/A beyond existing guarantees — no new language types; the optimizer must not
  change *typed* program behavior, only speed (covered under Safety invariants).
- **Perf/BigO (mem+cpu):** central to this milestone — see R5/R6 below and the Performance invariant.
- **Reusability/DRY:** central — R4 below (the dual-TargetMachine-site hazard) and the reuse of
  `soa_calibration.rs`'s harness pattern for Phase 7 rather than a fresh one.
- **Idempotency:** relevant — reproducible-build invariant (Safety) requires repeated `ynz build` runs
  on the same input to produce byte-identical objects.
- **Error-handling:** relevant — Phase 0/1 must fail the *build* cleanly (not panic) if the pipeline
  API genuinely isn't available; covered in Phase 0 exit criteria.
- **Observability/logging:** minor — no new user-facing surface is required this milestone;
  `YNZ_OPT_FORCE` (Phase 3, dev/bench-only, mirrors `YNZ_SOA_FORCE`) is not a shipped observability
  feature. A user-facing "which tier compiled this binary" surface is out of scope; not silently
  dropped — named here as considered and declined for this milestone.
- **Race/TOCTOU + resource-cleanup:** central — R1 below (drop-glue / Arc / channel calls surviving
  DCE/reordering) is exactly this concern.
- **Civil considerations:** N/A — compiler-internal backend work, no user-facing surface change.

**Risk Assessment** (scored via [REF-risk-engine.md](../../../../../.claude/docs/reference/REF-risk-engine.md),
deterministic lookup; no floor class fires — no money/PII/security/no-backout dimension is present,
this is pre-release compiler-internal work, fully git-reversible):

| Risk | Prob | Sev | Initial | Mitigations (bucket) | Residual | Gate |
|------|------|-----|---------|----------------------|----------|------|
| **R1 — optimizer flip miscompiles suspension frames / runtime FFI calls** (proven: 6/470 decimal128/EC-crossing SIGSEGVs (alignment class — 11 confirmed source sites per Phase 2's IR pointer-provenance audit, beyond the 3 Phase-1 anchors; FRAGO 004), AND — per FRAGO 002 — the Phase-1-confirmed false-ownership-attribute class: `emit.rs`'s `declare_function` (`emit.rs:1656-1686`) emits `readonly`/`noalias` from the raw AST ownership modifier instead of consulting typeck's `effective_ownership` analysis, yielding two deterministic optimized-build miscompiles; unproven: whether `ynz_channel_*` (incl. `ynz_channel_share` refcounting, the real cross-task sharing surface — no `ynz_arc_*` symbols are declared/called from `ynz-codegen`, per FRAGO 001)/drop-glue calls carry correct LLVM attributes to survive DCE/reordering) — *Phases 1–2, 5* | A | III | HIGH | Committed RED fixture set (the 6 spike fixtures + every sibling Phase 1's sweep finds — including the two ownership-attribute fixtures `v0_3_m7_p1_bare_param_mutation.ynz` / `v0_3_m7_p1_share_lend_alias.ynz`, FRAGO 002) gates the build; root-cause-before-fix ordering (**B2 adversarial/RED-repro**, prob −1; proof: failing fixtures committed before any fix lands) | **MEDIUM** (B×III) | recorded |
| **R2 — general hot-loop O0 stack-exhaustion SIGSEGV** (ledger row 439, absorbed by this plan) confounds honest benchmarking and is a live bug independent of SoA — *Phase 4* | A | III | HIGH | Root-cause + eliminate the failure mode (alloca/stack-growth fix + a stress regression fixture) (**B1 eliminate**, prob −2; proof: Phase 4's fixture running past the old ~4.19M-visit crash envelope) | **MEDIUM** (C×III) | recorded |
| **R3 — inkwell 0.9.0 may not cleanly expose LLVM 18's PassBuilder/`run_passes` surface** (net-new code, zero existing call sites) — *Phase 0* | B | III | MEDIUM | Hard-gate P0 spike with explicit accept/reject STOP-conditions before any durable phase depends on it (**B2 canary/staged**, prob −1; proof: Phase 0's persisted spike verdict) | **MEDIUM** (C×III) | recorded |
| **R4 — dual `TargetMachine` creation sites drift on pipeline config**, silently mismatching the main path vs. the state-machine path (this roadmap's own recurring authoritative-derivation corpse class — 4 confirmed instances in M4 alone) — *Phase 2* | B | II | HIGH | Thread ONE authoritative constructor (extend `default_target_machine`; delete the second inline construction) — the divergence class cannot exist with one source (**B1 eliminate**, prob −2; proof: grep-verified single construction call site + both consumers threaded from it) | **MEDIUM** (D×II) | recorded |
| **R5 — LLVM passes regress `ynz build` compile-time** beyond the accepted budget on `pirates-roster` — budget REBASED per Patrick-signed FRAGO 008 to the ABSOLUTE frame: measured 320ms → ~720-760ms (~+400ms, ~2.2x) at `default<O2>` on the pirates-roster demo scale, accepted; the pre-measurement <10% percentage figure is superseded (small-denominator artifact, per the FRAGO) — *Phase 3* | C | III | MEDIUM | Measured wall-clock gate before the default tier ships (**B2 canary w/ auto-reject**, prob −1) — the canary FIRED (no tier meets <10%: O2 +137%, Os +126%, O1 +122%, `executor-2026-07-17-phase3-tier-measurement`), was escalated to the human, and the budget was renegotiated on the record (FRAGO 008) — a recorded renegotiation, not gaming; proof: the committed tier-measurement numbers + the Patrick-signed accepted absolute budget | **LOW** (D×III) | pass |
| **R6 — preemption call-site checks reintroduce the 1190% overhead** measured previously at O0 (wrong-tier evidence) if added blindly — *Phase 6* (call-site checks are, like back-edge checks, codegen-emitted poll-yield sites — never runtime-implicit magic; this risk is about the OVERHEAD of emitting them, orthogonal to R8's frame-layout-correctness hazard for the back-edge mechanism itself) | C | III | MEDIUM | Ship call-site checks ONLY if a fresh O2 measurement clears a pre-registered threshold; otherwise the four-field deferral is the only path — the bad outcome cannot ship (**B1 eliminate via measurement-gated decision**, prob −2; proof: Phase 6's committed O2 measurement + explicit accept/reject line) | **LOW** (E×III) | pass |
| **R7 — optimizer/golden non-determinism** (LLVM pass-ordering or other codegen non-determinism could break the byte-identical 2-run golden-regeneration gate) — *Phase 5* | C | III | MEDIUM | The Phase 5 two-independent-run gate itself (**B2 engineered guard**, probability, −1; proof: golden regeneration re-run a second independent time, byte-diffed against the first — Phase 5 step 3) | **LOW** (D×III) | pass |
| **R8 — the back-edge poll-yield codegen transform introduces a NEW frame-layout/crossing-local suspension hazard** (turning a qualifying loop back edge INSIDE a state-machine function into a new poll-yield suspension point — store `resume_point`, flush crossing locals, return `Pending` — is net-new codegen logic in the same silent-miscompile family as R1, and this repo's four-milestone twin-derivation/frame history: M3a/M3d/M3e/M3g, per [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md)) — *Phase 6* | B | II | HIGH | Adversarial/RED-repro fixtures: loop-crossing-local suspension fixtures (the SM-positive case AND the non-SM residual case) authored and committed BEFORE the transform lands, gating the build (**B2 adversarial/RED-repro**, probability, −1; proof: failing fixtures committed pre-implementation, Phase 6 Steps 1 & 3) — re-lookup(C, II) = **HIGH, unchanged** (Critical severity does not clear High until probability reaches D; no second honestly-provable catalog mitigation applies — full work-shown in the RISK OVERRIDE block immediately below) | **HIGH** (C×II) | **BLOCKED — unsigned RISK OVERRIDE below** |
| **R9 — dangling-stack-return ABI miscompile class** (proven, deterministic: `ret ptr` to the callee's OWN alloca on `maybe<T>`/`number` returns — `emit.rs:2213-2223` / `:5292-5320`'s own "copy-and-forget ABI" comment; UB, garbage+hang under O2; confirmed THIRD O0-reliant class, structurally undiscoverable by Phase 1's attribute/alignment sweep because manifestation is inlining-dependent; FRAGO 005) — *Phase 3 (extended)* | A | III | HIGH | Committed RED fixture gating the fix (`v0_3_m7_p3_dangling_stack_return.ynz` + the `optimizer_red_gate` Class-3 test); root-cause-before-fix ordering (**B2 adversarial/RED-repro**, prob −1; proof: failing fixture + gate test committed before the fix lands) | **MEDIUM** (B×III) | recorded |
| **R10 — pre-existing multi-file build nondeterminism** (proven at clean HEAD/O0: `pirates-roster` object files flap between exactly two hashes — git-stash probe proof, orthogonal to the optimizer; breaks the reproducible-build Safety invariant AND R7's Phase-5 byte-identical 2-run gate mechanism; FRAGO 006) — *Phase 5 (Step 0)* | A | III | HIGH | Root-cause + eliminate the nondeterminism source before Phase 5 Step 3's gate runs, plus a determinism regression check (**B1 eliminate**, prob −2; proof: Phase 5 Step 0's committed determinism check green) | **MEDIUM** (C×III) | recorded |
| **R11 — fr23 confirmed-live UAF: non-plain-ident background-spawn receivers** (Future Requirements #9's disposition-(b) gate, executed 2026-07-17: B′ maybe-payload receiver WRONG at BOTH tiers; C2 call-materialized receiver WRONG 6/6 at O0, IR-proven dangling luck-masked at opt; A/C1 field-access shapes still-latent via `field_own_cell` heap cells; evidence `emit.rs:16417-16431` `is_heap_arg`, `check.rs:1709`; FRAGO 011) — anchor: fr23 / **disposition (a) decided 2026-07-18 (FRAGO 016) and EXECUTED 2026-07-18 (Phase 9, `executor-2026-07-18-phase9-fr23-fix`)**. **Loop-aggravation fact (2026-07-17 cleanup round):** Phase 4's back-edge restore made the fr23 shapes deterministically worse inside plain loops (per-iteration stomp; code-reviewer 2026-07-17) — strengthened the case for disposition (a) fix-in-plan | A | III | HIGH | **FIXED (B1 eliminate, Phase 9):** typeck records both confirmed shapes as `Give` via ONE admission helper (`bg_arg_is_materialized_shape_temp`, `check.rs`); codegen's `is_heap_arg` gate consults the ONE authoritative `background_arg_inferred_ownership` record by span for any expression shape (`emit.rs`, `prepare_bg_arg_for_ctx`) — the existing `HeapShape` heap-upgrade/free ladder covers both spawn arms; the planned-RED locks converted to permanent green regression tests (`fr23_uaf_planned_red.rs`, `#[ignore]` removed); proof: both fixtures re-run at BOTH tiers print `haul: 111/222`, corpus sweep clean with both fixtures included (FRAGO 012 exclusions removed); fix-round 2026-07-18 (`executor-2026-07-18-phase9-fr23-fixloop`): C2 admission extended to GENERIC shape-returning callees (`generic_fn_table` fallback — security live-repro closed) and SM-spawn-arm + generic-B′ coverage test-locked (4 new fixtures, `fr23_uaf_planned_red.rs`); further fix-round 2026-07-18 (cumulative completion-gate round 2, FRAGO 018): C2's substitution-seeding loop extended past ident-only args to also resolve a NESTED CALL argument whose callee has a concrete `sig_table` signature (`bg_arg_type_readonly` helper) — closes a live-reproduced gap where `identity<T>`'s `T` was resolvable ONLY from a non-ident argument (`background identity(makeCargo()).haul()`), confirmed wrong at both tiers before the fix, test-locked (`fr23_generic_call_nested_arg_spawn_receiver_reads_live_values`, 1 new fixture); **fourth fix-round 2026-07-18 (scoped fix-loop round, FRAGO 019 — STRUCTURAL fix, not a fifth narrowing):** FRAGO 018's fix was ITSELF still a one-level-only hand-rolled special case (`bg_arg_type_readonly`'s nested-call arm read `sig_table` ONLY, never falling back to `generic_fn_table` the way the outer `Expr::Call` arm does) — confirmed live for a NESTED GENERIC call argument (`background identity(identity(makeCargo())).haul()`, `identity<T>`'s outer `T` resolvable only from a call to another generic function), reproduced as garbage at both tiers by two independent reviewers before this round. Rather than patch a 4th narrow instance, `bg_arg_is_materialized_shape_temp`'s C2 arm and `bg_arg_type_readonly`'s nested-call arm were BOTH collapsed into ONE authoritative, RECURSIVE resolver (`bg_call_return_type_readonly`, `check.rs`) that resolves a call's return type — concrete or generic, at ANY nesting depth — by recursing into `bg_arg_type_readonly` for each argument that is itself a call. This closes the entire nesting-depth class in one definition rather than one level at a time (authoritative-derivation.md); verified at BOTH 2-deep (`identity(identity(makeCargo()))`) AND 3-deep (`identity(identity(identity(makeCargo())))`) nesting, both tiers, multiple repeated runs each, deterministic `haul: 111/222`; pre-fix Paper-Trace re-confirmed both new fixtures RED (garbage/stomp-sentinel values at both tiers) via a scoped revert-rebuild-restore; 2 new permanent regression fixtures/tests added (`fr23_generic_call_nested_generic_arg_spawn_receiver_reads_live_values`, `fr23_generic_call_triple_nested_spawn_receiver_reads_live_values`); **fifth fix-round 2026-07-18 (scoped fix-loop round, FRAGO 020 — STRUCTURAL fix, UNIFICATION not another narrowing):** FRAGO 019's recursive resolver was itself STILL only half the picture — `bg_arg_is_materialized_shape_temp` (the top-level admission predicate) and `bg_arg_type_readonly` (the nested-argument resolver `bg_call_return_type_readonly` seeded its substitution from) remained TWO independently hand-rolled enumerations of "which expression shapes materialize a temp": the top-level predicate recognized `FieldAccess`/`Call` directly (plus `MethodCall`, indirectly, via `background_spawn_call_form`'s own normalization) while the nested resolver recognized ONLY `Ident`/`Call`. Confirmed live INDEPENDENTLY by two reviewers (code-reviewer, security) for TWO shapes nested inside a generic call's argument: a UFCS method-call chain (`background identity(makeCargo().reroute()).haul()`) and a maybe-payload field access (`background identity(first.value).haul()`) — both garbage at both tiers before this round. Rather than add two more match arms to `bg_arg_type_readonly` (a sixth narrowing), BOTH enumerations were collapsed into ONE exhaustively-matched classifier, `bg_expr_resolved_type` (`check.rs`), consulted by every caller that previously hand-rolled its own shape list: `bg_arg_is_materialized_shape_temp` (top-level admission), `bg_call_return_type_readonly` (plain-`Call` nested resolution, unchanged self-filtering alignment), and a new `bg_ufcs_return_type` (UFCS `MethodCall` nested resolution, self-inclusive alignment — the receiver fills the callee's first parameter position). The classifier's `match` has **NO `_ =>` catch-all** — every one of `Expr`'s 22 variants is listed explicitly, so the Rust compiler itself refuses to build the moment a future `Expr` variant is added without a classification decision here: a genuine COMPILE-TIME exhaustiveness guarantee for the shapes this function enumerates, not a runtime parity test standing in for one. Verified: both new repros fixed, 3 repeated runs each, both tiers, deterministic `haul: 111/222`; pre-fix Paper-Trace re-confirmed both RED (nondeterministic garbage, e.g. `haul: 1/6355112`, `haul: 976778432/976778432`) via a scoped before/after diff-swap rebuild (not `git stash` — the branch's `push`-substring graveyard pre-filter fires on `git stash push`, so the comparison used a manual block-swap instead); one additional self-authored adversarial construction beyond the two reported repros (a `MethodCall` whose receiver is itself a `FieldAccess`, AND a `Call` whose arg is a `MethodCall` whose receiver is a nested `Call`, wrapped in a second generic layer) also verified correct at both tiers, 3 runs each; 2 new permanent regression fixtures/tests added (`fr23_generic_call_ufcs_nested_arg_spawn_receiver_reads_live_values`, `fr23_generic_call_fieldaccess_nested_arg_spawn_receiver_reads_live_values`); full `fr23_uaf_planned_red.rs` suite 11/11 green (9 pre-existing + 2 new, strict superset); `cargo test -p ynz-typeck` all green; `cargo clippy --workspace -- -D warnings` clean; `cargo fmt --all -- --check` clean; corpus sweep (`cross_impl_consistency`) re-run this round, see FRAGO 020 audit entry for the verdict; **SIXTH round 2026-07-18 (FRAGO 021 — full 22-arm semantic audit, NO code fix applied):** security live-reproduced `background haul({ weight: 111, tag: 222 })` (StructLit, wrong both tiers), proving FRAGO 020's exhaustive match still let a semantically-WRONG arm (`Expr::StructLit { .. } => None`) ship undetected. Per dispatch, ran a full live-verified audit of all 22 arms rather than patching StructLit alone — found **3 ADDITIONAL confirmed-live wrong/incomplete arms**: `FieldAccess{"value"}`'s guard misses `MapEntry<K,Shape>.value` (a for-loop map-entry field, backed by a per-iteration-rewritten out-buffer); `PostfixOp`(`.copy()`) and `Wait`, both nested inside a generic call's argument, fail to seed the generic substitution (`identity(c.copy())` / `identity(wait makeCargo())` as spawn receivers never resolve to Shape). All 4 findings (StructLit + 3 new) independently live-repro'd, Paper-Traced, persisted as checked-in documented-RED fixtures (`v0_3_m7_fr23_structlit_spawn_receiver.ynz`, `v0_3_m7_fr23_mapentry_value_spawn_receiver.ynz`, `v0_3_m7_fr23_generic_call_copy_nested_arg_spawn_receiver.ynz`, `v0_3_m7_fr23_generic_call_wait_nested_arg_spawn_receiver.ynz`) — not wired into the green suite. One hypothesis (`SelfValue`) was live-tested and DISCONFIRMED (recorded for transparency). Crossed this round's own "3+ additional wrong arms" STOP threshold — no fix applied (not even the unambiguous StructLit one), pending a conductor decision: continue narrowing (round 7+) vs. redesign as default-DENY (heap-upgrade everything that is NOT a provably-safe stable `Ident`/`SelfValue`/primitive, rather than allowlisting each materializing shape). Full 22-arm audit table + all 4 Paper-Traces in the FRAGO 021 audit.md entry; **SEVENTH round 2026-07-18 (FRAGO 022 decision + FRAGO 023 execution) — ARCHITECTURAL CLOSURE, not another narrowing.** Patrick decided the fork FRAGO 021 surfaced (FRAGO 022, `conductor-2026-07-18-completion-gate`): flip the admission check from ALLOWLIST (`bg_arg_is_materialized_shape_temp` asking "is this one of the shapes we've confirmed dangerous?") to DENYLIST-OF-SAFETY (`bg_arg_is_provably_safe` asking "is this PROVABLY a stable, already-owned binding or a statically-non-`Shape` primitive?", heap-upgrading everything else via a trailing WILDCARD, no `_ =>`-exhaustiveness needed for safety anymore since the wildcard IS the safety net). FRAGO 023 executed the redesign: three call sites updated (`check_stmts`'s give/copy loop, `check_background_handle_spawn`'s loop, AND `background_spawn_call_form`'s non-Ident-receiver UFCS gate — the third call site needed the SAME predicate too, closing FRAGO 021 finding 2 without a special-case arm since a `FieldAccess` receiver is never in the safe set); `bg_expr_resolved_type`/`bg_call_return_type_readonly`/`bg_ufcs_return_type`/`bg_apply_generic_return_subst` UNCHANGED and reused (their `None` now reads as "unresolved," consumed fail-closed, which is what closes findings 3/4 with zero new seeding arms). **Verified:** all 4 of FRAGO 021's fixtures pass via the DEFAULT (no per-shape arm added) — `fr23_uaf_planned_red.rs` 15/15 (11 pre-existing + 4 newly wired), strict superset, no regression; 3 self-authored adversarial constructions never tested before (a `StructLit` through the HANDLE form, `MapEntry<K,Shape>.value` nested inside a generic call's substitution-seeding argument, and the still-latent A/C1 `ship.cargo` class used directly as a spawn arg — the FIRST fixture ever to exercise A/C1 at all) all 6/6 correct at both tiers; one regression probe (scoped, restored, `diff`/`md5sum`-confirmed byte-identical after restore) confirmed the MapEntry-nested-generic construction WAS genuinely vulnerable under old-allowlist semantics (O0 deterministic `haul: 111/0` 6/6; optimized nondeterministic leaked garbage 6/6) before the redesign closed it; full `cross_impl_consistency.rs` corpus sweep (~557 fixtures, 2×2 mode matrix) clean; full `ynz-driver` test suite (20 binaries, 523-test `integration.rs` included) clean, zero `FAILED`; performance sanity check (20,000-spawn `background`-heavy workload, field-access-arg vs. ident-baseline) indistinguishable within noise — see FRAGO 024's audit correction: this comparison isolates two ALREADY-heap-upgraded paths, not a genuine no-upgrade baseline, so it is weaker evidence for the "zero LLVM IR" claim than originally worded (the claim itself remains true by direct code read of `prepare_bg_arg_for_ctx`'s no-op fallback arm, independent of this benchmark). Full record: FRAGO 022/023 audit.md entries; **EIGHTH round 2026-07-18/19 (FRAGO 024) — the redesign's OWN security re-check found the architecture had TWO remaining gaps, both closed this round: (1) a STRUCTURAL WIRING gap — the admission-recording machinery (`bg_arg_is_provably_safe` + its recording loop) ran on only 2 of the syntactic positions a `background` spawn can occupy (`check_stmts`'s Stmt::Expr match, `check_let`'s handle-form); `check_assign`/`check_field_assign`/`check_index_assign` routed through the generic `infer_expr` `Expr::Background` arm with NO recording at all — live-reproduced (`hd.slot = background makeCargo().haul()`, a FieldAssign target, wrong at O0). Closed by moving the recording loop into the generic arm itself — the ONE place every spawn form, in every statement position AND every expression-embedding, provably passes through (verified: a 4th adversarial position, a function-call argument, also reaches it, and the recursion through `infer_expr` is a stronger closure than any fixed enumeration). (2) A `SelfValue` FALSE-SAFE classification — `bg_arg_is_provably_safe`'s blanket `SelfValue => true` assumed self's storage always outlives any spawn using it, true for the single-level case FRAGO 021 arm #15 tested but false for a NESTED spawn (a `give self` parameter whose owning function is itself reached via `background`) — live-reproduced 16/16 this round (`weight` corrupted every run, `tag` survived). Closed by removing `SelfValue` from the safe set; it now rides the same default-`Give` wildcard as every other non-enumerated shape.** A THIRD finding (Bug 3, the borrow-reject diagnostic's `Some(Share)`-only gating) was investigated live, found to break 15/15 pre-existing fr23 fixtures if applied as literally instructed (the fix cannot distinguish a hazardous caller-retained alias from an already-safe materialized temp without becoming call-site-aware), and DEFERRED with a four-field record (FRAGO 024 audit entry) rather than forced through — not a memory-safety gap, a missing teaching nudge. Full record: FRAGO 024 audit.md entry | **CLOSED — architecturally, not empirically.** The allowlist's six-round failure mode (a fixed-but-large `Expr` grammar with no proof of enumeration completeness) is structurally inverted: the wildcard arm cannot be "wrong" in the dangerous direction — any expression `bg_arg_is_provably_safe` does not affirmatively prove safe defaults to heap-upgraded, including any FUTURE `Expr` variant or any shape this round's own adversarial testing did not think of, with ZERO code change required. What remains NOT guaranteed (named honestly, not overclaimed): a latent bug INSIDE the safe-set proof itself (the pre-existing, untouched `Ident` liveness path) is a categorically narrower risk surface than "did we allowlist every dangerous shape" — this round found no evidence of one, but it is not the same class of guarantee. A/C1 (`ship.cargo`) is no longer "still-latent, deliberately excluded" — it rides the same default protection as every other shape, verified harmless (redundant, not unsafe) both by reasoning and by a live adversarial fixture. **Round 8 (FRAGO 024) narrowing — stated precisely, per the dispatch's own instruction to be conservative rather than overclaim:** the architectural inversion above is now ADDITIONALLY confirmed to be wired to every syntactic position (not just the two call sites FRAGO 023 verified) and no longer carries a false-safe `SelfValue` classification. Still NOT guaranteed, named honestly: (a) a latent bug inside the pre-existing `Ident` liveness path itself remains unaudited by any round to date; (b) two syntactic positions (`Stmt::Assign`, `IndexAssign` on `nothing`-typed storage) are proven correct only at the TYPECK layer — full codegen/runtime proof is blocked by an orthogonal, pre-existing, out-of-scope ICE class; (c) the borrow-reject diagnostic's default-ownership teaching gap (Bug 3) remains OPEN, deferred with a four-field record — a missing teaching nudge, not a memory-safety hole, since the admission machinery above already protects the underlying argument regardless of this diagnostic. Work-shown record: FRAGO 011, FRAGO 016, Phase 9, FRAGO 018, FRAGO 019, FRAGO 020, FRAGO 021, FRAGO 022, FRAGO 023, FRAGO 024 | **CLOSED (architecturally) for the wiring/self-classification gaps FRAGO 024 found — `STATUS: DONE` returned 2026-07-19 (FRAGO 024, round 8). Bug 3 (borrow-reject teaching gap) remains OPEN as a named, deferred, non-security finding — not blocking, not silently dropped.** |

R8's residual lands HIGH and, per the frozen risk-engine catalog's available patterns, cannot be
honestly mitigated further at plan-authoring time (see the RISK OVERRIDE block immediately below —
drafted with the work shown, signature deliberately left blank; this producer never self-signs a HIGH
residual). Every OTHER residual in this table stays MEDIUM or LOW; no policy floor fires anywhere in
this table (still no money/PII/security/no-backout dimension). If Phase 1's sweep or any other phase
surfaces a FURTHER NEW risk that scores HIGH, it is surfaced immediately per the CCIR below — **never
self-signed**; the orchestrator's override gate is the only place a HIGH residual gets accepted.

**RISK OVERRIDE — accepted residual: HIGH** (R8; work shown per [REF-risk-engine.md](../../../../../.claude/docs/reference/REF-risk-engine.md)'s gate; this is a producer-drafted surface for the orchestrator's human override gate — it is never self-signed):

```
RISK OVERRIDE — accepted residual: HIGH
  Risk:                     R8 — the Phase 6 back-edge poll-yield codegen transform (turning a
                            qualifying state-machine-function loop back edge into a new poll-yield
                            suspension point: store resume_point, flush crossing locals via the
                            existing suspension machinery, return Pending) introduces a new
                            frame-layout/crossing-local hazard in the same silent-miscompile family
                            as R1, and this repo's four-milestone twin-derivation/frame history
                            (M3a/M3d/M3e/M3g).
  Why not mitigable to LOW: Initial lookup(B, II) = HIGH. The one honestly-provable catalog
                            mitigation — Adversarial/RED-repro test (B2, probability, −1; proof:
                            loop-crossing-local suspension fixtures, both the SM-positive case and
                            the non-SM residual, authored and committed BEFORE the transform lands,
                            Phase 6 Steps 1 & 3) — shifts probability B→C. Re-lookup(C, II) = HIGH,
                            UNCHANGED: Critical severity does not clear High until probability
                            reaches D. No second catalog mitigation honestly applies: (a) the
                            severity-axis B1 patterns (made-reversible / idempotency) don't map to a
                            compiler miscompile, and this plan's own severity-anchor selection
                            (pre-release, fully git-reversible) already prices reversibility into
                            Sev II rather than Sev I — re-applying git-revertibility as a SECOND
                            mitigation step would double-count the same fact; (b) a second
                            probability-axis pattern (canary/staged exposure) does not honestly
                            apply either — its precondition ("small slice first, auto-halt on
                            metric") presumes staged PRODUCTION exposure, which does not exist for
                            compiler-internal, pre-release codegen work; stretching it to fit would
                            be exactly the self-serving cell-picking REF-risk-engine.md's "not a
                            vibes table" clause forbids. Reusing the existing authoritative
                            suspension machinery (`store_resume_point` / `flush_var_slot_to_frame` —
                            the same functions the wait-suspension path already uses and tests) is a
                            genuine, valuable design constraint (satisfies authoritative-derivation.md;
                            named in Phase 6 Step 1) — but it is recorded here as a SCOPING decision,
                            not double-counted as a second, independent catalog mitigation step.
  Accepted by:              Patrick (Gate-4 approval, conducted 2026-07-04)
  Date:                     2026-07-04
  Trigger to revisit:       Before Phase 6 Step 2 begins. Re-score if either (a) Phase 1's exhaustive
                            R1 sibling-sweep changes this risk's probability/severity picture, or
                            (b) a genuinely new B1/B2 catalog mitigation is authored into
                            REF-risk-engine.md (a deliberate authoring act, never an inline
                            plan-time invention) before Phase 6 begins.
```

## Design-Doc Alignment

Governing docs read at plan time; every divergence enumerated as "doc says A; plan does B because C,"
per [`.claude/rules/plan-invariants.md`](../../../rules/plan-invariants.md) `## Design-Doc Alignment`.

**Cited governing docs:**
[`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
"Scheduler Preemption Model" section (lines 214–244) ·
[`authoritative-derivation.md`](../../../rules/authoritative-derivation.md) ·
[`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)
(the concurrency-release audit this plan absorbs P4-1 and the O0→optimizer synthesis item from).

**Citation-depth verification (read live, not assumed):**
- `IMP-no-function-coloring.md`'s "Scheduler Preemption Model" section genuinely SPECIFIES the
  mechanism it is cited for, not merely names the topic: it locks compile-time-assisted safe-point
  preemption with checks at BOTH function call sites AND loop back-edges, a ~10ms default time
  quantum, and auto-inferred CPU-bound task routing (lines 216, 236, 238). Depth confirmed — this
  plan can cite it as ground truth for what the doc currently claims is shipped.
- [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md) genuinely specifies the
  exact discipline this plan's R4/Phase 2 needs: "thread the SAME authoritative value/query into all
  of them... never let a second surface re-derive its own equivalent answer." Depth confirmed — not a
  topical citation.

**Divergences:**

1. **`IMP-no-function-coloring.md` says** (locked pre-v0.2) preemption checks fire at BOTH call sites
   AND loop back-edges; **reality is** codegen emits back-edge calls only, and they call a documented
   no-op stub (`runtime.rs:281-299`) — call-site checks were never implemented, and no
   `[[deferred_language_feature]]` registry entry was ever recorded (audit finding P4-1, HIGH). The
   doc has been FALSE since it was written. **The mechanism gap is architectural, not merely
   unimplemented** — this was corrected in this plan's own authoring after a personal plan-audit
   finding (see [`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)
   "M7-plan addendum"): `ynz_rt_check_preempt` is a synchronous `extern "C"` callee, which
   structurally CANNOT yield the enclosing Tokio task by itself. A real fix requires CODEGEN to turn
   qualifying loop back edges INSIDE state-machine functions into poll-yield suspension points (store
   `resume_point`, flush crossing locals via the already-existing suspension machinery
   `store_resume_point`/`flush_var_slot_to_frame`, return `Pending`), with the runtime function reduced
   to a cheap, synchronous budget CHECK the codegen-emitted branch consumes. Non-SM (plain synchronous)
   functions can never cooperatively yield this way — their only protection is the EXISTING
   CPU-admission routing to the blocking pool (see the Runtime Dependencies invariant below for the
   named residual this leaves: CPU-heavy code inside a non-SM function that admission misses).
   **This plan does B, not A, because C:** Phase 6 makes the
   doc TRUE either by (a) shipping the missing call-site checks for real — restoring the doc's
   original claim — or (b) rewriting the section to state the TRUE shipped mechanism (back-edge only)
   plus a proper four-field deferral, per whichever the fresh, pre-registered O2 measurement decides —
   either way the doc is rewritten to state the true THREE-part architecture: (i) SM-function back
   edges = codegen poll-yield (new, this milestone), (ii) non-SM CPU-bound work = blocking-pool routing
   (already shipped, unaffected), (iii) the named non-SM-admission-miss residual. Either branch closes
   P4-1 honestly; neither leaves the doc silently wrong. This disposition is
   Patrick-signed via the roadmap's row-443/M7-scoping note (¶1 Terrain) and the M6/M7 triage split
   below.
2. **`authoritative-derivation.md` says** thread one authoritative constructor, never re-derive a
   second; **this plan's model matches exactly, no divergence** — Phase 2 closes R4 by deleting the
   remaining inline `TargetMachine` construction (the explicit-`target_triple` override branch,
   `emit.rs:888-905`; re-anchored per FRAGO 001) and routing it through
   `state_machine.rs::default_target_machine`, per the doc's own prescription. Confirmed compliant.

**Milestone-boundary assumption flagged:** M6 owns the concurrency-release audit's correctness
findings (P1-1, P3-1/P2-2, P3-2, P4-3, P2-4, P3-3, and the `ynz run` signal-masking bug — ¶1 Friendly
forces enumerates these explicitly). **P4-1 (preemption honesty) is NOT in that M6 list** — it is this
plan's Phase 6, because P4-1's honest resolution structurally REQUIRES a real O2 measurement that only
exists once Phase 3 ships (the audit's own Priority 5 section states this: "'As fast or faster than
Rust' is unfalsifiable until the optimizer pipeline exists... Recommended plan shape: optimizer
milestone FIRST"). This boundary is drawn by the audit document itself, not invented by this plan —
stated here so the M6/M7 split has zero ambiguity for either plan's reviewer.

**Pre-existing, phase-untouched behavior claims re-verified at recon time (not carried forward
blind):** the audit's own Fable-verified finding that "TargetMachine opt level ≠ IR pass pipeline —
flipping the enum alone does NOT run mem2reg/SROA" (audit doc, Priority 5) is the exact claim ¶1
Terrain's "unverified theory" framing rests on for the spike's mem2reg attribution; it is recon-cited
directly against the audit doc's text, not assumed. Likewise the claim that
`state_machine.rs::default_target_machine` is already the single shared constructor for the
non-override branch is **verified** by direct reading of both call sites (¶1 Assumptions), not
inferred.

## 2. Mission

The Yinz compiler team replaces the hardcoded `OptimizationLevel::None` codegen pipeline with a real,
root-caused, safety-verified LLVM optimization pipeline for `ynz build` — after v0.3-M6's correctness
hotfixes merge to `main` — so that every performance claim about Yinz (concurrency, SoA, and beyond)
becomes falsifiable against measured evidence instead of a compiler that structurally never optimizes;
Phase 7's measured position (FRAGO 014): the shipped pipeline delivers real 1.4–3.1x net wins over
`--no-optimize`, AND idiomatic Rust `--release` remains measurably faster than shipped Yinz
(~2.2–2.7x on scalar/shape microworkloads, ~7–10x on array-scan — dominated by the opaque
runtime-call ABI floor and always-on overflow checks), so "Rust-level performance" is a pursued
positioning with a named, measured gap (Future Requirement #7), not an achieved outcome.

## 3. Execution

### 3.1 Intent & End State

**Purpose.** Turn on real optimization safely: root-cause the ONE hazard proven at authoring time (the
spike's decimal128 SIGSEGVs) before any durable phase depends on an unverified theory about it, sweep
exhaustively for undiscovered siblings (execution has since confirmed two more O0-reliant miscompile
classes: the false-ownership-attribute class, R1/FRAGO 002, and the dangling-stack-return ABI class,
R9/FRAGO 005), and close the two adjacent bugs (the O0 stack-exhaustion ceiling, and honest
benchmark integrity) that would otherwise corrupt every measurement this milestone produces. Execution
also surfaced a THIRD, unrelated bug outside this original two-bug scope — the fr23 confirmed-live UAF
(non-plain-ident `background`-spawn receivers riding raw pointers into a dead frame, exposed by this
milestone's own optimizer flip shrinking/reusing stack-slot lifetimes) — closed as an added Phase 9
(FRAGO 016) per the Purpose's own disciplined-initiative guidance below: a real memory-safety bug
discovered mid-milestone, fixed in-plan rather than silently expanding scope elsewhere.

**Key outcomes:**
1. `ynz build` compiles through a real LLVM pass pipeline by default; `ynz build --no-optimize` is the
   documented escape hatch back to the old O0 behavior (mirrors `--no-auto-parallel`'s exact CLI/env
   threading pattern).
2. Every suspension/concurrency fixture — the full `ynz-driver` + `ynz-codegen` test suite (830+ tests
   pre-existing), the 6 spike-failing fixtures, every sibling Phase 1's sweep finds, and the
   Phase-3-discovered dangling-stack-return fixture (`v0_3_m7_p3_dangling_stack_return.ynz`,
   R9/FRAGO 005) — is GREEN under
   the new pipeline, proven by re-run, not asserted from the spike alone.
3. Real back-edge preemption ships as a NEW codegen poll-yield transform for state-machine functions
   (today only the CALL SITES exist at back edges — they target a documented no-op stub, so runtime
   preemption is currently ZERO, per audit finding P4-1; `ynz_rt_check_preempt` is a synchronous
   `extern "C"` callee and structurally cannot itself yield a Tokio task, so the real fix lives in
   codegen, never inside the stub) AND call-site preemption (likewise codegen-emitted poll-yield
   sites, never runtime-implicit magic) is EITHER shipped
   for real (fresh O2 measurement clears the threshold) OR honestly deferred with a registry entry —
   either way, [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)'s
   "Scheduler Preemption Model" section states the TRUE, three-part shipped architecture: (a)
   SM-function back edges = codegen poll-yield (new, this milestone), (b) non-SM CPU-bound work =
   blocking-pool routing (already shipped, unaffected), (c) a named residual — CPU-heavy code inside a
   non-SM function that admission misses — closing the audit's P4-1
   doc-vs-reality gap honestly rather than asserting full coverage.
4. The O0 stack-exhaustion SIGSEGV (roadmap ledger row 439) is root-caused and fixed, unblocking honest
   hot-loop benchmarking (the `soa_calibration.rs` 131,072-visit cap can be reassessed).
5. A committed, reproducible O0-vs-optimized A/B benchmark suite AND an honestly-framed Rust-equivalent
   comparison suite exist and run clean in CI — reporting the TRUE measured numbers
   (both harnesses live in `opt_pipeline_calibration.rs`; gate mode `-- --test` is the CI-green
   surface, matching the soa_calibration precedent): default-over-o0 net wins of 1.72x / 3.01x /
   1.49x on cpu_loop / shape_alloc / soa_physics
   (`crates/ynz-driver/benches/opt-pipeline-raw-2026-07-17.md`), and a measured gap to idiomatic
   Rust `--release` of 2.70x / 2.25x / 7.20x on the same workloads (2.19x / 1.60x / 9.93x against
   overflow-checks-matched Rust; `crates/ynz-driver/benches/rust-equiv-raw-2026-07-17.md`).
   Phase 7's numbers fell SHORT of "as fast as Rust," so per
   [plan-source-of-truth.md](../../../../rules/plan-source-of-truth.md)'s execution-time reframe
   discipline the Mission and this Key Outcome now state the measured position rather than the
   aspiration (FRAGO 014): Rust parity is NOT achieved as of v0.3-M7; the gap's evidence-backed
   attribution (runtime-call ABI floor, always-on overflow checks, mid-end/backend maturity) and
   its remediation path are named in Future Requirement #7 — not buried, not asserted away.
6. Every golden (`crates/ynz-codegen/tests/golden.rs` IR-text + object-SHA-256, and
   `examples/pirates-roster/expected_stdout.txt`) is regenerated and verified **stable across at least 2
   independent regeneration runs** — not a single-run commit (this repo has an existing, named failure
   mode for exactly that: the M4 audit's "stable across 5 runs" claim that was never actually
   CI-enforced; this plan does not repeat it).

**Definition of done.** All 6 outcomes above are met; the full CI matrix
(`cargo fmt --check && cargo clippy --workspace -- -D warnings && cargo test --workspace && cargo build
--workspace --release`) is green; the roadmap's Capability Ledger reflects this plan absorbing row 439
and shipping row 443, with rows 438/440/441/442 explicitly left unabsorbed and reasoned (§Roadmap
Reconciliation, Phase 8).

**Disciplined-initiative guidance.** When a literal step and reality diverge — inkwell 0.9.0 genuinely
can't expose `run_passes` cleanly, or a sweep or later phase finds a THIRD O0-reliant path nobody
anticipated (this materialized: Phase 3's dangling-stack-return class, handled per this exact
discipline as R9/FRAGO 005), or a golden fails to stabilize across repeated runs (this also
materialized: the pre-existing multi-file nondeterminism, R10/FRAGO 006, surfaced and risk-rowed
rather than silently absorbed) — the fallback is the Purpose above:
**root-cause before fix, prove before ship, never paper over a suspension-frame or runtime-FFI-call
correctness question with a hopeful guess.** Any newly-discovered O0-reliance is a NEW risk row (surface
it), never a silent in-place patch. When genuinely uncertain whether a finding is in-scope for this
plan or belongs in Future Requirements, the test is: does it touch the pipeline's correctness or the
two absorbed bugs (R1–R2)? If not, name it and defer it — do not expand scope silently.

### 3.2 Concept

**Ordering note (hard-sequenced, not either-order):** Phase 4 Step 1 re-confirms the O0
stack-exhaustion crash **under Phase 3's already-flipped optimizer default**, since the failure
envelope may shift once real optimization is live — Phase 4 cannot honestly run before Phase 3
ships. Phase 3 → Phase 4 is therefore a **hard sequence**, not an either-order pair; this is also
what Phase 7's benchmarks need (an honest row-439 repro under the LIVE pipeline, not the stale O0
one).

Ten phases, strictly root-cause-before-fix ordered. **Phase 0** hard-gates the one net-new mechanical
assumption (does `inkwell` 0.9.0 expose enough LLVM 18 PassBuilder surface) before anything durable
depends on it. **Phase 1** root-causes the spike's actual failure mechanism and exhaustively sweeps for
siblings — producing a committed RED fixture set, not a fix. **Phase 2** fixes what Phase 1 found and
threads the single authoritative `TargetMachine` constructor (closing R4). **Phase 3** wires the real
pass pipeline through that one constructor, with the `--no-optimize` escape hatch — extended (FRAGO
005) to also land the R9 dangling-stack-return return-ABI fix and green its Class-3 RED gate before
the phase's exit criteria can be claimed. **Phase 4** fixes the
absorbed O0 stack-exhaustion bug (row 439) — root-caused independently of the optimizer question, but
its own Step 1 re-confirms the crash under Phase 3's now-live pipeline, so it is hard-sequenced AFTER
Phase 3, never before or in parallel. **Phase 5** first eliminates the pre-existing multi-file build
nondeterminism (Step 0, R10/FRAGO 006), then regenerates every invalidated golden and re-runs the
full suite — the proof phase for outcomes 2 and 6. **Phase 6** resolves the preemption honesty question
(real call-site checks or a proper deferral). **Phase 7** builds the two benchmark suites (A/B,
Rust-comparison) now that rows R1/R2 are closed. **Phase 8** closes the loop on documentation, registry,
and roadmap reconciliation. **Phase 9** — added after Phase 8's original close, via FRAGO 016 — closes
R11/the fr23 confirmed-live UAF (non-plain-ident `background`-spawn receivers riding raw pointers into
a dead frame): a real memory-safety fix (give/copy machinery extended to materialized-shape-temp
receivers), not a documentation catch-up like Phase 8. Phases 0–2 gate everything after them; **Phase 3
then Phase 4 run in that strict order** once 0–2 are green (see the Ordering note above); Phases 5–9 are
strictly sequential after 3 and 4, with Phase 9 sequenced last as a post-close security fix-round.

### 3.3 Phases

#### Phase 0 — P0 Spike: inkwell / LLVM PassBuilder Feasibility

- **Task + purpose:** Prove, on a throwaway scratch module, that `inkwell` 0.9.0 (LLVM 18) can build
  and run a real optimization pass pipeline before any durable phase assumes it can. This is the
  [plan-spike-discipline](../../../../rules/plan-spike-discipline.md) Facet 1 hard gate for R3.
- **Steps:**
  1. Read `inkwell` 0.9.0's public API surface (via `cargo doc` or the vendored source under the Cargo
     registry cache) for `PassBuilderOptions`, `TargetMachine::run_passes`, or any equivalent new-PM
     entry point exposed at this pinned version.
  2. Write a throwaway scratch Rust binary (outside the crate tree, e.g.
     `scratch/opt-pipeline-spike/`) that builds a minimal LLVM module (one function, one alloca, one
     dead store) and calls the discovered API with an `"default<O2>"`-style pass pipeline string (or
     whatever `inkwell` 0.9.0's actual signature requires).
  3. Confirm the resulting IR shows the dead store eliminated (proof the pipeline actually ran, not a
     no-op success return).
  4. Record the exact API shape (function name, signature, pass-pipeline string format) in this
     phase's scratch notes for Phase 3 to consume directly — do not re-discover it there.
- **STOP-conditions (hard gate):** RED if `inkwell` 0.9.0 exposes no usable pass-pipeline entry point at
  all (would require either an inkwell version bump — its own ADR-worthy decision, since a version bump
  changes the entire crate's LLVM binding surface — or a raw `llvm-sys`/C-API escape hatch). GREEN if a
  working call sequence is found and the dead-store elimination proof passes.
- **Exit criteria:** GREEN verdict recorded with the working API shape; scratch binary + its output
  persisted as a checked-in note (per plan-spike-discipline Facet 2 — the API shape is exactly the kind
  of artifact a later phase needs and must not be thrown away with the rest of the scaffolding); the
  scratch binary itself is NOT committed to the crate tree (throwaway).
- **Reviewer fan-out:** adversarial gate-checker (is the GREEN verdict genuinely proven — did the dead
  store actually disappear from the IR, not just "the call returned Ok").
- **Model tag:** `(coding, high, small)`

#### Phase 1 — Root-Cause the Spike Failures + Exhaustive Sibling Sweep

- **Task + purpose:** Replace the unverified "mem2reg" theory (`emit.rs:9961-9963`, `:10717-10719`)
  with a confirmed root cause via bisection, and exhaustively sweep for every other O0-reliant path —
  producing a committed RED fixture set that gates Phase 2's fix. **Do not fix anything in this phase.**
- **Steps:**
  1. Confirm M6 has merged to `main` (CCIR precondition — halt and report if not).
  2. Reproduce the spike's 2-line diff in a fresh worktree by applying the checked-in
     [`spike-o0-flip.patch`](./spike-o0-flip.patch) directly — do not re-derive it from memory. Cross-
     reference the 6/470 failing-fixture list in
     [`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)'s
     "Phase-0 spike" section as the evidence of what the applied diff broke.
  3. Bisect the actual failing pass: compile the failing fixture
     (`v0_3_m3a_p1_ec_crossing_local_propagated_number`) at `-O0` and `-O2` via the LLVM `opt`/`llc`
     CLI tools directly (bypassing `ynz build`), diffing the generated assembly/IR at each pass-pipeline
     stage to find which specific pass changes behavior. Confirm or refute the mem2reg theory with this
     evidence — do not assert either way without the diff in hand (Paper-Trace: observed vs. expected
     vs. residual vs. hypothesis vs. evidence path, per this session's verification discipline).
     **CHECKPOINT** — root-cause hypothesis confirmed with a Paper-Trace and a minimal repro; sibling
     sweep (next steps) not yet started.
  4. Exhaustively grep every `extern "C"` runtime declaration the codegen crate calls into
     (`ynz_array_*`, `ynz_map_*`, `ynz_channel_*` — including `ynz_channel_share` refcounting
     (`emit.rs:15880-15886`, `runtime_decls.rs:110-111`), the surface cross-task sharing actually
     rides; no `ynz_arc_*` symbols are declared or called from `ynz-codegen` (they live unconsumed
     in `crates/ynz-runtime/src/arc.rs`) — plus drop-glue helpers,
     `ynz_rt_check_preempt`, `ynz_rt_spawn*`; citation corrected per FRAGO 001) and confirm each
     carries LLVM attributes correct for its
     REAL side-effect profile (no false `readnone`/`speculatable`/`nofree` on anything with an
     observable effect; correct `noalias`/`nocapture` where the ownership model guarantees it). This is
     the general form of R1 the narrow decimal128 spike only sampled.
  5. For each additional O0-reliant path found (beyond the 2 known comments), author a RED fixture that
     fails when compiled optimized, mirroring the 6 spike fixtures' shape.
  6. Commit the full RED fixture set (spike's 6 + any new ones) as failing/ignored tests gating Phase 2
     — this is the R1 mitigation's proof artifact.
- **Exit criteria:** a confirmed, evidenced root cause (not a theory); a complete, committed RED fixture
  set; zero fixes attempted in this phase.
- **Reviewer fan-out:** code-reviewer (sweep completeness — did it actually cover every runtime
  declaration, not a sample); adversarial gate-checker (does the root-cause claim survive an
  independent read of the bisection evidence); design-doc-alignment reviewer (does the finding
  contradict anything [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md) or
  [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
  already commits to).
- **Model tag:** `(coding, high, medium)`

#### Phase 2 — Fix the Root Cause + Thread the Single Authoritative `TargetMachine` Constructor

- **Task + purpose:** Fix exactly what Phase 1 root-caused — BOTH confirmed classes: the alignment
  class AND the false-ownership-attribute class (FRAGO 002) — and eliminate R4 by extending
  `state_machine.rs::default_target_machine` into the ONE authoritative constructor both call sites
  use — never a second, independently-configured `TargetMachine` creation.
- **Steps:**
  1. Implement the fix Phase 1's evidence points to, covering both confirmed classes (per FRAGO 002,
     audit.md, including its Patrick-directed design decision of 2026-07-16):
     (a) **alignment class** — attribute corrections on the affected sites and/or explicit
         frame-slot handling that survives the confirmed pass; the affected-site list is **11
         confirmed source sites** (Phase 2's IR pointer-provenance audit — the SM staged-param
         pointer path makes arbitrary-provenance number pointers 8-aligned-capable, so the class
         extends beyond the 3 Phase-1 anchors; citation updated per FRAGO 004);
     (b) **false-ownership-attribute class, fixed from BOTH ends:**
         (i) *codegen* — `declare_function` (`emit.rs:1656-1686`) must consult typeck's
             `effective_ownership` analysis when emitting `readonly`/`noalias`, never the raw AST
             ownership modifier (authoritative-derivation: consume the computed answer);
         (ii) *typeck* — extend the aliasing checker to REJECT, as a compile error, a call passing
              the same value as both `share` and `lend` (or two aliasing `lend`s) — Patrick's
              decision per FRAGO 002 (Golden Rule 5: an aliasing violation of the ownership
              contract is caught at compile time; teaching diagnostic per Golden Rule 11).
     Note: if the typeck aliasing-rejection work proves large enough to genuinely warrant its own
     phase, Phase 2's executor surfaces a scope-split proposal through the plan's own
     over-fat-step / FRAGO mechanism rather than silently absorbing or silently dropping it — the
     split decision belongs to the deviation-judge → FRAGO seam, not pre-decided here.
  2. Re-run the full RED fixture set from Phase 1; confirm the gate is fully green (per the FRAGO 002
     reshape, reconciled by FRAGO 004): the 5 differential fixtures (4 alignment +
     `bare_param_mutation`) PASS optimized, and the alias fixture — which FRAGO 002's decision makes
     uncompilable by design — is a compile-rejection lock
     (`red_opt_share_lend_alias_rejected_at_compile_time`, locking the rejection + its teaching
     phrases; a differential optimized run is structurally impossible for a program that no longer
     compiles).
     **CHECKPOINT** — RED set green; root cause fix committed; pipeline-wiring work not yet started.
  3. Extend `default_target_machine` to accept the pipeline configuration Phase 3 will need (a
     parameter, not a second global), without yet turning optimization on by default (that is Phase 3's
     job — keep this phase's diff scoped to the constructor's SHAPE, not its default value).
  4. Delete the remaining inline `TargetMachine` construction — the explicit-`target_triple`
     override branch of `emit_artifact` (`emit.rs:888-905`, `OptimizationLevel::None` at
     `emit.rs:900`). M6's merge already routes the default/`None`-triple branch through
     `state_machine::default_target_machine()` (`emit.rs:887`), so the override branch is the only
     survivor (citation re-anchored per FRAGO 001, audit.md). Route it through
     `default_target_machine` instead — grep-verify zero remaining independent construction sites.
  5. Re-run the full pre-existing test suite (830+ tests) to confirm the constructor threading
     introduced no behavior change yet (still O0 by default at this point in the sequence).
     **CHECKPOINT** — single authoritative constructor verified (grep + green suite); ready for Phase 3
     to flip the actual default.
- **Exit criteria:** RED set green; exactly one `TargetMachine` construction call site in the crate
  (grep-verified); full suite green; no default behavior change yet.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (the RED-set-green claim); a dedicated
  grep-verification for the single-constructor invariant (can be folded into code-reviewer's pass).
- **Model tag:** `(coding, high, large)` — scale=large (attribute audit + constructor migration);
  checkpoint marks mandatory.

#### Phase 3 — Wire the Real Optimization Pass Pipeline

- **Task + purpose:** Turn the pipeline on by default through the Phase 2 constructor, with a
  `--no-optimize` escape hatch and a `YNZ_OPT_FORCE` dev/bench override, using Phase 0's proven API
  shape.
- **Steps:**
  1. Using Phase 0's recorded API shape, implement the real pass-pipeline call inside
     `default_target_machine` (or a sibling function it calls), defaulting to a real optimizing tier
     (target: LLVM's `default<O2>`-equivalent, or `Os` if compile-time budget demands it — pick ONE,
     record the choice and why). *Tier choice — FINAL per FRAGO 008 (closes this step's "pick ONE"):*
     **`default<O2>`.** Why: the Os/O1 measurement (`executor-2026-07-17-phase3-tier-measurement` in
     audit.md) showed no tier meets the old <10% figure and the tiers sit within ~7% of each other —
     Os buys only ~5% vs O2 for a smaller optimization surface, and O1 MASKS the R9
     dangling-stack-return manifestation via inlining (correct-looking output over unchanged UB),
     making it hazardous as a default. The compile-time cost is accepted under FRAGO 008's
     Patrick-signed absolute budget.
  2. Add a `--no-optimize` CLI flag to `ynz build`, threaded through the salsa barrier via an env var —
     mirror `--no-auto-parallel`'s exact existing pattern in `crates/ynz-driver/src/main.rs` (same
     plumbing shape, new name).
  3. Add a `YNZ_OPT_FORCE` env override for the benchmark harness (Phase 7), mirroring `YNZ_SOA_FORCE`'s
     precedent — dev/test-only, never a shipped user surface.
  4. Measure `ynz build --release` wall-clock on `examples/pirates-roster/` before/after this phase —
     this is R5's mitigation proof. DONE (FRAGO 008): the measurement is complete
     (`executor-2026-07-17-phase3-tier-measurement` in audit.md — O0 median 320ms; `default<O2>`
     ~720-760ms; Os/O1 within ~7% of O2); this step now records the ACCEPTED numbers under the
     Patrick-signed absolute budget (~+400ms on the pirates-roster demo scale, FRAGO 008) rather
     than gating on the superseded <10% percentage figure.
  4b. Implement the return-ABI fix for the dangling-stack-return class (R9/FRAGO 005): eliminate
     ret-of-own-alloca on `maybe<T>`/`number` returns (`emit.rs:2213-2223` / `:5292-5320`) via a
     caller-provided out-slot/sret or a by-value return — the fix executor picks the evidenced shape,
     reusing the existing authoritative frame/ABI machinery per
     [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md), never a parallel
     second path.
  4c. Un-ignore the Class-3 RED test (`optimizer_red_gate`, `v0_3_m7_p3_dangling_stack_return.ynz`)
     and green the full RED gate including it.
     **CHECKPOINT** — return-ABI fix landed; full RED gate (incl. Class 3) green; Step 5's full-suite
     run not yet done.
  5. Run the full pre-existing test suite (830+ tests) plus Phase 1/2's RED set under the NOW-DEFAULT
     optimized pipeline; every one must be green. Reviewer glance required: Rust-runtime decimal
     reads must not assume 16-alignment of frame-interior i128 slots (Phase 2 lowered those claims
     to align 8).
     **CHECKPOINT** — default pipeline live, compile-time numbers recorded under the FRAGO-008
     rebased budget, full suite green.
- **Exit criteria:** `ynz build` optimizes by default at `default<O2>` (FRAGO 008); `--no-optimize`
  proven to reproduce the exact old
  O0 output byte-for-byte; compile-time numbers recorded and within the FRAGO-008 rebased absolute
  budget; full suite green; Class-3 RED test green and
  un-ignored (R9); the ret-of-own-alloca class is verified at BOTH layers it needs (FRAGO 010):
  (a) static IR scan clean for the direct `ret ptr <own-alloca>` shapes the scan can structurally
  see — `audit_ret_alloca.py` (SSA-rename tracing: gep/bitcast, not through-memory loads) re-run
  post-fix over 12 freshly emitted `--no-optimize --emit-ir` .ll files incl. the multi-module
  pirates-roster `bin.ll`: AUDIT CLEAN (receipt: audit.md `executor-2026-07-17-frago010-cleanup`);
  AND (b) the differential O0-vs-optimized RED gate (`optimizer_red_gate`, 10 tests) as the
  AUTHORITATIVE lock for the laundered int-embedded shapes (ptr_to_int payload bits riding maybe
  envelopes / EC ok-words) the static scan structurally cannot see.
- **Reviewer fan-out:** code-reviewer; design-doc-alignment reviewer (the CLI-flag pattern vs.
  `--no-auto-parallel` precedent); adversarial gate-checker (does `--no-optimize` genuinely reproduce
  old behavior, or does it silently differ).
- **Model tag:** `(coding, high, medium)`

#### Phase 4 — Fix the O0 Stack-Exhaustion SIGSEGV (Absorb Ledger Row 439)

- **Task + purpose:** Root-cause and fix the general hot-loop stack-exhaustion SIGSEGV at ~4.19M
  total loop-visits (reproducible identically in AoS and SoA modes) — root-caused independently of
  the optimizer question, but **hard-sequenced after Phase 3** (never in parallel or before): Step 1
  re-confirms the crash under Phase 3's now-live optimized pipeline before root-causing it, since the
  failure envelope may shift once real optimization is on. Gates honest benchmarking (R2). This is the
  plan's Patrick-signed absorption of roadmap ledger row 439.
- **Steps:**
  1. Reproduce the crash via the existing characterization in `soa_calibration.rs`'s header comment and
     the M5 plan's Future Requirements #13 / risk E13 (starting point, not settled fact — confirm it
     still reproduces under this plan's already-flipped optimizer default from Phase 3, since the
     failure envelope may shift).
  2. Root-cause via the per-iteration `alloca` stack-growth theory (loop-body frame lifetime at
     O0/optimized) — confirm with a minimal repro and a stack-size measurement across increasing visit
     counts, not assumption.
  3. Implement the fix (likely: hoisting or reusing the loop-body frame instead of a fresh alloca per
     iteration, or an explicit stack-probe/growth strategy).
  4. Author a stress regression fixture that runs well past the old ~4.19M-visit crash envelope and
     asserts a correct checksum with no crash.
  5. Re-evaluate (do not blindly raise) the `soa_calibration.rs` 131,072-visit cap now that the
     underlying bug is fixed — raise it only with fresh evidence of a new safe ceiling, and note the
     change in that file's own header comment.
- **Exit criteria:** stress fixture passes at ≥10x the old crash envelope; `soa_calibration.rs`'s cap
  reassessed with evidence; full suite still green.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (does the stress fixture actually
  exceed the old envelope, not just claim to).
- **Model tag:** `(coding, high, medium)`

#### Phase 5 — Regenerate Goldens + Full Suite Re-Verification

- **Task + purpose:** Prove Key Outcomes 2 and 6 — every golden regenerated, every fixture green,
  stability proven across repeated runs (not a single-commit claim).
- **Steps:**
  0. (Step 0, inserted per FRAGO 006 — runs BEFORE Step 1.) Root-cause and fix the pre-existing
     multi-file build nondeterminism (R10). Evidence: `pirates-roster` object files flap between
     exactly two hashes, reproduced at clean HEAD/O0 — see the audit.md phase3 entry (git-stash probe
     proof). Suspect: emission-order nondeterminism in multi-file emission — confirm with evidence,
     never assume. Add a determinism regression check.
  1. Regenerate every `crates/ynz-codegen/tests/golden.rs` IR-text and object-SHA-256 snapshot under
     the new default pipeline (the file's own doc comment states it auto-regenerates on first run —
     use that mechanism, review every diff by hand, do not blind-accept). Note (FRAGO 007): the
     Phase-3 interim golden regeneration predates the R9 ABI fix — those goldens are provisional
     (F1-tainted); this phase's post-fix regeneration is the authoritative one.
  2. Regenerate `examples/pirates-roster/expected_stdout.txt` via its own
     `expected_stdout.txt.regenerate.sh` — confirm the stdout content is byte-identical to the pre-M7
     baseline modulo the documented M2 scheduler-race ordering window (integration.rs:2596-2658,
     pre-existing, optimizer-independent — A/B-probed in both modes; wording amended per judge-ratified
     D-P5.1, FRAGO 013). Any divergence outside that window is a correctness bug, not an expected
     regeneration — halt and investigate before accepting.
     **CHECKPOINT** — first-pass regeneration complete; stability proof (next steps) not yet run.
  3. Re-run golden generation a SECOND independent time (fresh process invocation, not a repeated
     assertion inside one run) and diff against the first regeneration — every byte must match. This
     is the stability proof this plan's Safety invariant requires, closing the gap the M4 audit's
     unenforced "stable across 5 runs" claim left open. Methodology note (FRAGO 013): the 2-run
     stability proof is TRANSITIVE — each independent fresh-process run byte-asserts against the
     committed golden set, so run 1 == committed and run 2 == committed together prove
     run 1 == run 2 by transitivity; no direct run-to-run diff artifact is required beyond the two
     independent green runs against the same committed set.
  4. Run the FULL pre-existing test suite (830+ tests), the Phase 1/2 RED set, and the Phase 4 stress
     fixture together — one combined green run. Reviewer glance required: Rust-runtime decimal reads
     must not assume 16-alignment of frame-interior i128 slots (Phase 2 lowered those claims to
     align 8).
  5. Run the existing cross-implementation consistency harness
     (`crates/ynz-driver/tests/cross_impl_consistency.rs`, `--no-auto-parallel` vs. default) under the
     new pipeline — confirm identical stdout/stderr/exit-code, now ALSO across `--no-optimize` vs.
     default-optimized (extend the harness's assertion matrix to include this new axis).
     **CHECKPOINT** — full suite + stability proof + cross-implementation matrix all green; ready for
     documentation/demo sign-off.
- **Exit criteria:** all goldens regenerated and stable across 2 independent runs; `pirates-roster`
  stdout byte-identical to the pre-M7 baseline modulo the documented M2 scheduler-race ordering window
  (integration.rs:2596-2658, pre-existing, optimizer-independent — A/B-probed in both modes; FRAGO
  013); full suite green; cross-implementation matrix covers the new `--no-optimize` axis.
- **Reviewer fan-out:** code-reviewer; test-quality (does the stability proof actually re-invoke a
  fresh process, not just re-assert the same run); adversarial gate-checker (the cross-implementation
  matrix's new axis).
- **Model tag:** `(coding, high, large)` — scale=large (this is the whole-suite proof phase);
  checkpoint marks mandatory.

#### Phase 6 — Preemption: Codegen Back-Edge Poll-Yield + Call-Site Re-Measurement Decision

- **Task + purpose:** Ship REAL back-edge preemption via a CODEGEN transform (not "inside
  `ynz_rt_check_preempt`" — that framing was architecturally wrong and was corrected in this plan's
  own authoring per a personal plan-audit finding: a synchronous `extern "C"` callee cannot yield the
  enclosing Tokio task by itself; see the Design-Doc Alignment divergence 1 above for the full
  correction and [`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)
  "M7-plan addendum" for the source finding). The true mechanism: codegen turns a qualifying loop back
  edge INSIDE a state-machine function into a poll-yield suspension point (store `resume_point`, flush
  crossing locals via the existing suspension machinery, return `Pending`); `ynz_rt_check_preempt`
  becomes a cheap, synchronous budget CHECK the codegen-emitted branch consumes, never the yield
  itself. Non-SM (plain synchronous) functions can NEVER cooperatively yield this way — their
  protection is the EXISTING CPU-admission routing to the blocking pool; this phase names that
  residual explicitly rather than silently. Also re-measure call-site check cost under the NOW-real O2
  pipeline (the 1190% figure was measured at O0, where nothing inlines — wrong-tier evidence); ship
  call-site checks (likewise codegen-emitted poll-yield sites, never runtime magic) if the fresh number
  clears an explicit threshold, else record a proper four-field deferral with a registry entry. Update
  [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)'s
  "Scheduler Preemption Model" section to state the TRUE three-part shipped architecture — closing the
  audit's P4-1 doc-vs-reality gap honestly either way. **R8 (¶1 Risk Assessment) governs this phase's
  correctness hazard** — its HIGH residual carries an unsigned RISK OVERRIDE that must be signed by
  the orchestrator/Patrick before Step 2 (implementation) begins.
- **Steps:**
  1. **DESIGN step (gates everything after it).** Specify the codegen back-edge poll-yield transform
     for SM functions in a written design note, covering: (a) WHICH loops qualify — loop back edges
     INSIDE a state-machine (wait-containing) function only; (b) WHAT the yield emits — a
     `resume_point` store + a crossing-local flush, both via the EXISTING authoritative suspension
     machinery (`store_resume_point`/`flush_var_slot_to_frame`, `state_machine.rs`/`emit.rs` — reuse,
     never a second, parallel frame-flush implementation, per
     [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md)) + a `Pending` return,
     with the resume path reloading and continuing the loop; (c) the BUDGET mechanism — a cheap
     runtime counter/time check inside `ynz_rt_check_preempt` deciding WHETHER to yield at this back
     edge (the check legitimately lives in the runtime function and returns a bool; the YIELD itself is
     codegen's, never the runtime's); (d) explicitly what happens to loops in NON-SM functions —
     NOTHING new: they have no back-edge yield mechanism at all, and their existing protection is
     CPU-admission routing to the blocking pool (already shipped) — name this residual in the design
     note, do not silently omit it.
     **CHECKPOINT** — design note written, covering all four points above; implementation not yet
     started.
  2. Implement the codegen transform (turn a qualifying SM-function loop back edge into a poll-yield
     suspension point per the Step 1 design note) and implement `ynz_rt_check_preempt` as the real,
     cheap, synchronous budget-check-and-decide function (returns a bool; performs no yield itself) —
     wired to the already-emitted back-edge call sites (`emit.rs:12356-12365`).
  3. Author the starvation-proof fixture set (R8's committed RED-fixture mitigation, per the ¶1 Risk
     Assessment): (a) a hot CPU-bound-loop-with-no-function-calls fixture placed INSIDE a
     state-machine (wait-containing) function — the exact starvation shape
     [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)'s
     preemption section exists to prevent — and confirm another task on the same worker gets scheduled
     time under the new real back-edge poll-yield; (b) a companion fixture with the IDENTICAL hot loop
     placed INSIDE a plain (non-SM, non-wait-containing) function, and confirm it is NOT preempted by
     this phase's new mechanism — relying instead on the existing CPU-admission blocking-pool routing —
     documenting the residual explicitly as a passing, expected-behavior fixture, not a silent gap.
     **CHECKPOINT** — real back-edge poll-yield implemented and fixture-proven (both the SM-positive
     case and the documented non-SM residual); call-site re-measurement work not yet started.
  4. **Pre-register the acceptance threshold BEFORE measuring** (per no-duct-tape's proof-before-ship
     discipline: decide the bar, then look) — e.g. "call-site check overhead must be ≤X% on a
     representative call-heavy microbenchmark under the Phase 3 default pipeline."
  5. Add call-site preempt-check emission (mirroring the existing back-edge emission pattern — likewise
     a codegen-emitted poll-yield site, never runtime-implicit magic) behind a
     compile-time toggle; measure the overhead on the pre-registered microbenchmark under the default
     optimized pipeline.
     **CHECKPOINT** — pre-registered threshold set and the fresh O2 measurement taken; the ship/defer
     decision (next step) not yet made.
  6. **Decision (measurement-gated, R6's mitigation):** if the fresh number clears the pre-registered
     threshold, ship call-site checks unconditionally (matches
     [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)'s
     original lock exactly — no divergence to record). If it does NOT clear the threshold, do not ship
     them; author the four-field deferral (WHAT/WHY/COST/TRIGGER) plus a deferral registry entry
     (`[[deferred_tooling_feature]]` — it gates a compiler-internal compile-time toggle, not
     user-typeable syntax; kind settled at the Phase 6 review round, matching the sibling
     `cooperative-preemption-back-edge-yield` entry's classification) named e.g.
     `preempt-callsite-checks` — closing audit finding P4-1 honestly either way.
  7. Update [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)'s
     "Scheduler Preemption Model" section to state the TRUE, three-part shipped architecture: (a)
     SM-function back edges = codegen poll-yield (new, this milestone), (b) non-SM CPU-bound work =
     blocking-pool routing (already shipped, unaffected), (c) the named residual — CPU-heavy code
     inside a non-SM function that admission misses — plus call-site checks either
     shipped-and-described or deferred-with-a-registry-citation.
- **Exit criteria:** the Step 1 design note is written and covers all four points; the codegen
  transform reuses the existing authoritative suspension machinery (grep-verified — no second,
  parallel frame-flush implementation introduced); back-edge poll-yield is real and fixture-proven for
  BOTH the SM-positive case and the documented non-SM residual; the call-site decision is
  measurement-gated with a pre-registered threshold (not measured-then-rationalized); the design doc
  states the true three-part shipped architecture, not aspiration; R8's RISK OVERRIDE is signed before
  Step 2 begins.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (R8: does the RED-fixture set — both
  the SM-positive and non-SM-residual cases — genuinely prove frame-layout correctness before merge,
  not merely asserted; does the transform genuinely reuse the authoritative suspension machinery
  rather than re-deriving a parallel one); design-doc-alignment reviewer (doc now matches reality,
  either direction, and states the true three-part architecture); docs-consistency reviewer (the
  deferral's registry entry, if any, is well-formed).
- **Model tag:** `(coding, high, medium)` — quality-bar raised from `standard` to `high` because this
  is now a frame-layout-affecting codegen transform in R8's silent-miscompile hazard class (matching
  Phase 1/2's quality bar for the same hazard family); 7 steps trips the >5-step checkpoint trigger;
  checkpoint marks required (after Step 1, after Step 3, after Step 5).

#### Phase 7 — Benchmark Suites: O0-vs-Optimized A/B + Rust-Equivalent Comparison

- **Task + purpose:** Build the honest evidence base this whole milestone exists to produce — extending
  `soa_calibration.rs`'s harness pattern (criterion, compiled-.ynz-binary driving, checksum + IR-content
  gates) rather than writing a new one from scratch (reusability).
- **Steps:**
  1. Extend the `soa_calibration.rs` pattern into a new `opt_pipeline_calibration.rs` bench: drives
     compiled `.ynz` workload binaries at `--no-optimize` vs. default (using `YNZ_OPT_FORCE` from
     Phase 3), across a small representative workload set (a CPU-bound loop, a shape-heavy allocation
     workload, the SoA physics-update workload already characterized in M5) — now that Phase 4 has
     fixed the visit-count ceiling, reassess whether the old 131,072 cap can be relaxed for this new
     harness (do not blindly copy the cap without re-checking).
  2. Every workload binary passes the same three gates `soa_calibration.rs` established: checksum
     tripwire, byte-identical stdout across modes (dual-mode oracle), and an IR-content gate confirming
     the optimized binary's `.ll` actually differs from the unoptimized one (proof the pipeline ran, not
     a silent no-op — mirrors the M3d silent-decline tripwire class this repo has been burned by
     before).
     **CHECKPOINT** — A/B harness green, committed, with raw numbers recorded.
  3. Build the Rust-equivalent comparison suite: for each representative workload above, hand-author an
     idiomatic `cargo --release` Rust program doing equivalent work, run both, and report wall-clock
     honestly — document explicitly what is and is not comparable (e.g., Rust's `--release` LTO/codegen
     defaults vs. Yinz's chosen tier; startup/runtime init cost; any workload where the comparison isn't
     apples-to-apples, state why).
  4. Reconcile the Mission's "as fast or faster than Rust" framing against these real numbers per
     [plan-source-of-truth.md](../../../../rules/plan-source-of-truth.md)'s execution-time reframe
     discipline: if the numbers show parity or better, state that plainly; if they show a gap, rewrite
     the Mission/Key-Outcome-5 text to state the TRUE measured position and name the gap as a Future
     Requirement (never leave the headline overclaiming past what Phase 7 actually measured).
     **CHECKPOINT** — Rust-comparison suite green, numbers committed, Mission text reconciled if needed.
- **Exit criteria:** both harnesses committed and green in CI; every claimed number traces to a
  committed benchmark run, not a hand-wave; the Mission/Key-Outcome text matches reality.
- **Reviewer fan-out:** code-reviewer (harness correctness); docs-consistency reviewer (does the
  Mission text now match the Phase 7 numbers, per the reconciliation discipline); adversarial
  gate-checker (the IR-content gate and dual-mode oracle actually prove what they claim).
- **Model tag:** `(coding, standard, large)` — scale=large (two harnesses, cross-language comparison
  authoring); checkpoint marks mandatory.

#### Phase 8 — Documentation, Registry, and Roadmap Reconciliation

**DONE (fix-loop round `executor-2026-07-17-phase8-fixround3`, closing prior rounds
`executor-2026-07-17-phase8-fixround`/`executor-2026-07-17-phase8-fixround2` and a first-pass dispatch
`executor-2026-07-17-phase8-final-reconciliation`):** all 5 steps below complete. The first pass left
5 real gaps (registry entry never touched; CHANGELOG cited the stale M5-era ~3.3x figure, omitted the
Rust-parity-gap disclosure, and falsely claimed loop-free-recursion starvation Fixed; roadmap
§Milestone 7 status text left stale; ledger row 442 missed in both Capability Ledger tables), each
confirmed by four independent review lenses (code-reviewer, rules-compliance, acceptance-verifier,
deviation-judge) and closed in the first fix-loop round. Two subsequent narrow fix rounds each caught
one more stale "optimizer does not run by default" instance a prior round's fix missed a sibling of
(`fixround2`: the `array-using-soa-layout` lint's `why_template`, plus a `Feature Registry Entries`
subsection gap and a CHANGELOG range-vs-scalar inconsistency). `fixround3` closed the whack-a-mole
pattern with a genuinely exhaustive, paraphrase-aware repo-wide grep sweep (not another single-string
fix): fixed the `array-using-soa-layout` lint's now-self-contradicting COMMENT block (registry entry's
`why_template` was fixed in `fixround2` but the comment above it was not) and
`docs/internal/implementation/IMP-collections.md`'s "Honest performance provenance (E14)" section
(stale M5-era present-tense claim, now marked explicitly historical/pre-M7 with the current shipped
reality stated up top); confirmed via broad multi-phrasing grep across the whole repo that no further
live instances remain — every other hit is legitimate historical record (dated bench/audit reports,
archived-plan Mission text, CHANGELOG's own dated M5 section) or unrelated (kernel-mode diagnostics,
dead-code comments, the not-yet-shipped `ynz build --release` CLI flag). See `audit.md`'s Session log
for all four dispatches' full accounts.

- **Task + purpose:** Close the loop — CHANGELOG, feature registry (if Phase 6 deferred call-site
  checks), and the roadmap's milestones list + **BOTH** Capability Ledger tables (add M7, reconcile
  rows 438–443 per §Roadmap Reconciliation below).
- **Steps:**
  1. Add a `### Milestone 7: v0.3-M7 — Optimizer Pipeline` section to
     [`roadmap.md`](../../active/2026-05-21-v0-3-concurrency-perf/roadmap.md) (mirroring the existing
     per-milestone section shape) and append `v0-3-m7-optimizer-pipeline` to the roadmap frontmatter's
     `milestones` list.
  2. **The roadmap carries TWO duplicate Capability Ledger tables** (a pre-existing, pre-M7 condition,
     not something this plan introduces): `## Capability Ledger (SSOT for capability → milestone
     ownership)` at roadmap.md's line ~365, and `## Capability Ledger` at roadmap.md's line ~417
     (merged 2026-07-01 from the pre-migration companion `capability-ledger.md` file — the roadmap's
     own text says so at that heading). Both currently carry byte-identical rows 438–443. **Update BOTH
     tables in lockstep, in this same step** — the sibling v0.3-M6 plan independently commits to
     updating both tables for its own rows, so this is an established, shared convention for this
     roadmap, not new ceremony invented here. In each table: mark ledger row 439 (general hot-loop O0
     stack-exhaustion) **shipped by M7**; mark ledger row 443 (LLVM optimization pass pipeline)
     **shipped by M7**; leave rows 438 (authoritative-derivation write-time guard), 440 (stale-archive
     ABI-version-checked embedding), 441 (codegen ICE: bare int literal into `number`), and 442
     (selective hot-field-only element materialization) **unchanged and explicitly annotated as NOT
     absorbed by M7**, each with the one-line reason from §Roadmap Reconciliation below. A diff that
     updates only one of the two tables is an incomplete Phase 8 — grep both headings to confirm parity
     before calling this step done.
  3. Registry reconciliation covers **BOTH** preemption entries in
     [`registry/features.toml`](../../../../registry/features.toml), not just the new one: (a)
     confirm the Phase-6-added `preempt-callsite-checks` `[[deferred_tooling_feature]]` deferral
     entry is present and accurate (added at Phase 6 execution time; kind recategorized from
     `[[deferred_language_feature]]` at the Phase 6 review round — it gates a compile-time toggle,
     not user-typeable syntax); (b) update or retire the now-STALE
     `cooperative-preemption-back-edge-yield` `[[deferred_tooling_feature]]` entry (`ships_in =
     "v0.3-M7"`) — the back-edge half genuinely SHIPPED this milestone (real poll-yield; its
     "documented no-op stub" substitute/why text is false post-Phase-6), so rewrite it to the
     shipped reality or remove it, leaving the still-deferred call-site half's record solely to
     `preempt-callsite-checks`. A Step 3 that touches only the new entry is an incomplete
     reconciliation.
  4. CHANGELOG entry for the milestone; confirm no stray references to "compiles at O0" survive in any
     doc this milestone's grep sweep touches.
  5. Carry the FRAGO-008-rebased compile-time budget into the roadmap's own budget text (the roadmap
     still states the <10% figure): restate it as the Patrick-signed absolute frame (measured 320ms →
     ~720-760ms, ~+400ms / ~2.2x at pirates-roster demo scale, at `default<O2>`), citing FRAGO 008, so
     plan and roadmap agree.
- **Exit criteria:** roadmap and registry reflect the true post-M7 state; no unreconciled ledger rows.
- **Reviewer fan-out:** docs-consistency reviewer; design-doc-alignment reviewer (final check that
  nothing in this plan's execution silently contradicted a cited design doc without being surfaced).
- **Model tag:** `(general/mechanical, floor, medium)`

#### Phase 9 — Close the fr23 Confirmed-Live UAF (R11/FRAGO 011 Disposition (a))

**Inserted by FRAGO 016** (`conductor-2026-07-18-completion-gate`, 2026-07-18) — Patrick's own
"morning decision" (two days late, made on the record at the completion gate, not silently skipped):
fix fr23 in this plan rather than defer it to a scoped M8-adjacent follow-up. See `audit.md`'s
`### FRAGO 016` for the full trigger/decision/classification record.

- **Task + purpose:** Close the confirmed-live UAF (R11 / Future Requirements #9) for non-plain-ident
  `background`-spawn receivers — specifically the two shapes the 2026-07-17 fr23 gate CONFIRMED-LIVE:
  **B′** (maybe-payload receiver, e.g. `first.value.haul()` where `first: maybe<Cargo>` was
  materialized from an index — wrong at BOTH tiers) and **C2** (call-materialized receiver, e.g.
  `background makeCargo().haul()` — wrong 6/6 at O0, optimized-tier "correct" only by IR-proven
  stack-layout luck over identical dangling). Both shapes heap-upgrade as raw pointers today because
  `is_heap_arg` (`crates/ynz-codegen/src/emit.rs`, ~16787-16801) and its typeck twin
  (`crates/ynz-typeck/src/check.rs`'s spawn-receiver ownership normalization, ~line 1709 area) gate
  the heap-upgrade path on `Expr::Ident` (with inferred ownership) or an explicit `.copy()` postfix
  only — every other receiver expression shape falls through to `BgArgFreeKind::None`, a raw pointer
  into a task that can outlive the spawner's frame. Phase 4's back-edge stacksave/restore turned this
  from a latent risk (real only under artificially-long O0 stack-slot lifetimes) into a deterministic
  per-iteration stomp inside plain loops — the loop-aggravation fact that strengthened the case for
  fixing now rather than deferring. **A/C1 (the field-access and call-form-field-access-arg shapes,
  protected today by `field_own_cell` heap-cell allocation) are explicitly OUT OF SCOPE** — they are
  still-latent, not confirmed-live, and FRAGO 011 never routed them for a fix; do not touch their
  handling.
- **Steps:**
  1. Root-cause the exact gap between the `Expr::Ident` path (correctly heap-upgraded via
     `background_arg_inferred_ownership`) and the B′/C2 receiver shapes (silently falling through to
     `BgArgFreeKind::None`): read `is_heap_arg`'s match arm in `emit.rs` and the typeck-side spawn
     normalization helper in `check.rs` end-to-end for a maybe-payload receiver and a
     call-materialized receiver, and name the precise site(s) where each shape's ownership/ident-span
     information is lost before it ever reaches the heap-upgrade gate.
  2. Design and implement the fix by EXTENDING the existing give/copy/ownership machinery — never a
     second, parallel mechanism, per this repo's
     [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md) — so that B′
     (maybe-payload) and C2 (call-materialized) spawn receivers get correctly heap-upgraded (or
     copied, whichever the ownership model's existing discipline calls for) before the spawned task's
     context is built. Reuse the same `BgArgFreeKind`/heap-upgrade infrastructure `is_heap_arg`'s
     `Expr::Ident`/`.copy()` arms already use for Shape/array/maybe-payload receivers — extend the
     admission logic (`is_heap_arg`'s match, plus whatever typeck-side ownership recording the fix
     needs) to recognize these two additional receiver expression shapes, not build a sibling path.
     A/C1's `field_own_cell` handling must be left untouched.
  3. Remove the `#[ignore]` attributes from the two planned-RED tests in
     `crates/ynz-driver/tests/fr23_uaf_planned_red.rs` once the fix makes them pass for real — this is
     the planned-RED-to-green conversion this repo's `no-duct-tape.md` legitimate-inverse discipline
     describes. Actually run both tests (`cargo test -p ynz-driver --test fr23_uaf_planned_red --
     --ignored`, or un-ignored via the normal suite) and confirm BOTH genuinely pass at both tiers
     (`CORRECT_HAUL_LINE` — do not just delete the `#[ignore]` marker and assume green).
  4. Remove the two fr23 fixture exclusions in `crates/ynz-driver/tests/cross_impl_consistency.rs`
     (the `v0_3_m7_fr23_maybe_payload_spawn_receiver.ynz` / `v0_3_m7_fr23_call_materialized_spawn_receiver.ynz`
     name-exclusions, both marked `test-ratchet: FRAGO 011 planned-RED fixture` with the explicit
     removal trigger "the exclusions come out in the same change that fixes fr23") — FRAGO 012's
     named removal trigger firing. Re-run the corpus sweep this test drives and confirm it is
     genuinely clean with both fixtures included, not merely that the exclusion lines compile out.
  5. Amend this plan's own R11 risk-table row (¶1 Risk Assessment) and Future Requirements #9's text
     to record disposition (a) **EXECUTED** (not merely decided/inserted) — state concretely what
     shipped (the give/copy extension, which receiver shapes it covers), cite this Phase 9, and update
     the risk's residual/gate columns from "accepted HIGH — morning decision pending" to the closed,
     verified state (both confirmed-live shapes fixed and proven; A/C1 unchanged and still correctly
     out of scope).
  6. Amend the roadmap's fr23 Capability Ledger row in **BOTH** duplicate tables in
     [`roadmap.md`](../../active/2026-05-21-v0-3-concurrency-perf/roadmap.md) (the "Non-plain-ident
     shape receivers/args in background-spawn position" row, currently "confirmed-live — 2 shapes
     (FRAGO 011, 2026-07-17); fix pending M7 morning disposition") to "fixed by M7 Phase 9" — citing
     this phase and its verification evidence. This is Phase 9's own boundary-commit responsibility,
     not a separate completion-gate pass.
- **Exit criteria:** both confirmed-live UAF shapes (B′, C2) produce correct output (`haul: 111/222`
  or the fixture's equivalent correct values) at both O0 and `default<O2>` tiers, independently
  verified by re-running the fixtures — not merely asserted from the fix's design; the two planned-RED
  tests in `fr23_uaf_planned_red.rs` pass with `#[ignore]` removed; the two
  `cross_impl_consistency.rs` exclusions are removed and the corpus sweep is genuinely clean with both
  fixtures included; R11, Future Requirements #9, and the roadmap's fr23 row (both tables) all reflect
  the closed risk; A/C1's `field_own_cell` handling is untouched (grep-confirmed, not just asserted)
  and remains correctly out of scope. **Fix-round addendum (2026-07-18,
  `executor-2026-07-18-phase9-fr23-fixloop`):** the C2 admission also covers GENERIC shape-returning
  callees (`generic_fn_table` fallback in `bg_arg_is_materialized_shape_temp`, mirroring the
  borrow-reject check's established `.or_else` pattern), live-verified red-before/green-after at both
  tiers; the SM spawn arm (both shapes) and the generic-B′ analog are locked by four new permanent
  fixtures/tests in `fr23_uaf_planned_red.rs`.
- **Reviewer fan-out:** code-reviewer (the give/copy fix's correctness — this is exactly the class of
  ownership/memory-safety fix that needs real scrutiny, in the same silent-miscompile family this
  plan's R1/R9/authoritative-derivation history already warns about); security (memory-safety-adjacent
  — this is a UAF fix); deviation-judge (confirm A/C1 stayed untouched, confirm the fix reuses the
  authoritative give/copy/ownership machinery rather than forking a parallel one, per
  `authoritative-derivation.md`).
- **Model tag:** `(coding, high, medium)` — real memory-safety engineering (give/copy machinery for
  two specific receiver shapes) deserves the higher quality bar, but the scope is bounded to one
  subsystem's admission gate, not a multi-file architecture change.

### 3.4 Coordinating Instructions

- **Sequencing:** Phases 0–2 gate everything after them (R3, R1, R4 must be closed before the default
  flips). **Phase 3 then Phase 4 run in that strict, hard-sequenced order** once 0–2 are green — Phase
  4 Step 1 re-confirms the O0 stack-exhaustion crash under Phase 3's now-live optimized pipeline, so
  Phase 4 cannot honestly start before Phase 3 ships (this is also what Phase 7's benchmarks need: an
  honest row-439 repro under the LIVE pipeline, not the stale O0 one). Phases 5–9 are strictly
  sequential after 3 and 4.
- **Verify-before-complete gate (every phase):** the phase's own exit criteria must be independently
  re-run by the closing executor, not just narrated as done — this repo's own precedent (the M4 audit's
  unenforced "stable across 5 runs" claim) is exactly the failure this plan's Phase 5 stability proof
  exists to not repeat elsewhere.
- **CCIR — surface immediately, mid-flight, never silently absorb:**
  1. M6 has not merged to `main` when Phase 1 is dispatched — halt, do not proceed on unmerged
     correctness fixes.
  2. Phase 0 returns a RED spike verdict (inkwell 0.9.0 cannot expose a usable pass-pipeline entry
     point) — halt for re-design (an inkwell version bump or a raw-`llvm-sys` escape hatch is itself a
     new decision requiring its own review, not a quiet substitution).
  3. Any newly-discovered O0-reliant path — during Phase 1's sweep beyond the two known comments, OR
     in any later phase (discovery is not confined to Phase 1: the Phase-3 dangling-stack-return
     class, R9/FRAGO 005, fired exactly this CCIR) — each gets its own RED fixture and risk-row
     treatment before a fix is attempted; never fixed silently alongside the known cases.
  4. Any NEW risk scoring HIGH/EX-HIGH during execution — surfaced unsigned, per the risk engine's
     override gate; never self-signed by an executor.
  5. Any golden or fixture that fails to stabilize across the Phase 5 repeated-run proof — this is a
     correctness signal (nondeterministic codegen), not a flaky-test annoyance; halt and investigate.
- **Design-doc alignment during execution:** if any phase's finding contradicts
  [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
  or [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md), state it as
  "design doc `X` says A; this plan does B because `<reason>`" and surface for sign-off — never
  silently follow the plan over the doc.

## Invariants This Milestone Must Preserve

### Safety

- The spike's 6 failing fixtures (decimal128/EC-crossing paths) AND every sibling fixture Phase 1's
  exhaustive sweep discovers pass GREEN under the real optimized pipeline before Phase 3 flips the
  default — testable via the committed RED fixture set (Phase 1 exit criteria; Phase 2 exit criteria).
  The Phase-3-discovered dangling-stack-return fixture (`v0_3_m7_p3_dangling_stack_return.ynz`,
  R9/FRAGO 005) joins this set and passes GREEN (un-ignored) within Phase 3 itself (Steps 4b/4c)
  before Phase 3's exit criteria are claimed — inlining-dependent manifestation made it structurally
  undiscoverable by Phase 1's sweep.
- No new silent-miscompile class survives the milestone: every audited `ynz_array_*`/`ynz_map_*`/
  `ynz_channel_*` (incl. `ynz_channel_share` refcounting, the real cross-task sharing surface — no
  `ynz_arc_*` symbols are declared or called from `ynz-codegen`, per FRAGO 001)/drop-glue/
  `ynz_rt_check_preempt`/`ynz_rt_spawn*` call site carries LLVM
  attributes correct for its real side-effect profile (Phase 1 step 4), and the full 830+-test
  `ynz-driver` suite plus the Phase 1/2 RED set plus the Phase 4 stress fixture are green TOGETHER
  (Phase 5 step 4) — never proven piecemeal.
- `ynz build --no-optimize` reproduces the exact pre-M7 O0 output byte-for-byte (Phase 3 exit
  criteria) — the escape hatch is PROVEN equivalent, not merely assumed close.
- The cross-implementation consistency harness (`--no-auto-parallel` × `--no-optimize` × default) shows
  byte-identical stdout/stderr/exit-code across every mode combination (Phase 5 step 5) — across
  wait/background/EC suspension paths, not just the happy path.
- Reproducible-build invariant: two independent, fresh-process golden-regeneration runs on identical
  input produce byte-identical output (Phase 5 step 3) — closing the M4 audit's unenforced "stable
  across 5 runs" gap (R7 covers the optimizer-introduced side of this risk; R10/FRAGO 006 covers the
  pre-existing multi-file nondeterminism, which Phase 5 Step 0 must root-cause and eliminate BEFORE
  this gate can honestly run).
- Exactly ONE `TargetMachine` construction call site exists in the crate after Phase 2 (grep-verified,
  Phase 2 exit criteria) — the R4 dual-constructor drift class cannot exist with a single source.

### Performance

- **Auto-promotion analysis (mandatory per [`auto-promotion.md`](../../../rules/auto-promotion.md)):**
  this milestone creates **NO new auto-promotion candidate.** The optimizer pipeline is a backend/
  codegen-tier change applied UNIFORMLY to every compiled program — there is no per-construct
  "stricter form fits in some cases" proof the compiler makes case-by-case (contrast `array→fixed` or
  `let→const`, which are per-binding proofs with a losing alternative form). `--no-optimize` is a
  global CLI escape hatch, not a per-site override direction in the auto-promotion sense. Considered
  and declined — stated explicitly so reviewers know it was evaluated, not skipped.
- O0-vs-optimized A/B expectations (Phase 7): the compiled workload set must show a MEASURABLE
  difference between `--no-optimize` and default-optimized binaries, proven by an IR-content gate
  (Phase 7 step 2) confirming the pass pipeline actually ran — not asserted from a silent no-op. The
  actual magnitude reported is whatever Phase 7 measures, honestly, with no pre-committed number.
- Compile-time budget: `ynz build --release` wall-clock on `pirates-roster` stays within the
  Patrick-signed ABSOLUTE budget rebased by FRAGO 008 — measured 320ms → ~720-760ms (~+400ms, ~2.2x)
  at `default<O2>` on the pirates-roster demo scale, accepted; the earlier <10% percentage figure is
  superseded (small-denominator artifact) (Phase 3 step 4, R5's mitigation proof).
- Golden-stability doubles as a determinism guarantee: the pass pipeline must not introduce
  nondeterministic codegen ordering that would break the byte-identical 2-independent-run gate (R7);
  the pre-existing multi-file nondeterminism (R10/FRAGO 006) is eliminated at Phase 5 Step 0 so the
  gate measures the pipeline, not the pre-existing flap.

### Teaching

- **Phase 2 shipped exactly ONE new user-facing compile-error class** (FRAGO 002, reconciled here per
  FRAGO 004): the aliasing-call rejection — a call passing the same value (or overlapping pieces of
  one value) into two parameter positions where at least one can modify it is a compile error.
  Testable assertions: the diagnostic follows WHAT/WHAT-INSTEAD/WHY per Golden Rule 11 (WHAT names
  both positions and their meanings, including whole-vs-part overlap; WHAT-INSTEAD offers a copyable
  `.copy()` fix; WHY is contextual and jargon-free — no "alias"/"borrow"/"noalias" in user-facing
  text), locked by `crates/ynz-typeck/tests/aliasing_call_rejection.rs`'s teaching-shape test and
  `crates/ynz-driver/tests/error_galleries.rs`'s key-phrase assertions. No other new diagnostic class
  has shipped from Phases 0–2; the remaining optimizer-pipeline wiring (Phases 3, 5, 7) is
  backend/tooling work with no new parse/typeck-visible surface. `--no-optimize`'s CLI help text
  follows the existing `--no-auto-parallel` precedent's shape (mirrored, not reinvented — Phase 3
  step 2).
- If Phase 4's fix or a later phase unexpectedly surfaces a FURTHER genuinely new compile-error
  class, it follows the same WHAT/WHAT-INSTEAD/WHY format per Golden Rule 11 and earns a gallery
  entry (see Demo & Error Gallery below) — named as a live possibility per the CCIR above, never
  assumed away.
- Phase 6 rewrites [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)'s
  "Scheduler Preemption Model" section to state the TRUE, three-part shipped architecture — (a)
  SM-function back-edge codegen poll-yield (new), (b) non-SM CPU-bound blocking-pool routing (already
  shipped), (c) the named non-SM-admission-miss residual — regardless of whether Phase 6 ships
  call-site checks or records a deferral for that one sub-piece; this is a
  documentation-teaching fix (not a compiler diagnostic), held to the same never-silently-diverge
  standard as any other diagnostic.

### Runtime Dependencies

- **This milestone adds NO new runtime dependency**, stated explicitly. The optimizer pipeline runs
  entirely at compile time (inside `ynz build`, via `inkwell`'s LLVM PassBuilder surface, Phase 0/3);
  it adds no new library, syscall, allocator, or scheduler dependency to COMPILED Yinz binaries.
- Real back-edge preemption (Phase 6) uses the EXISTING Tokio dependency already present in
  `ynz-runtime` — not a new one. The mechanism is a CODEGEN transform (poll-yield suspension points at
  qualifying SM-function loop back edges, reusing the existing `store_resume_point`/
  `flush_var_slot_to_frame` suspension machinery and Tokio's `Poll::Pending`/waker plumbing already
  wired for `wait`), never new logic added inside `ynz_rt_check_preempt` itself — that function is
  reduced to a cheap, synchronous budget CHECK the codegen-emitted branch consumes; it never performs
  the yield. Non-SM (plain synchronous) functions have no back-edge yield mechanism at all — their
  existing protection is CPU-admission routing to the blocking pool (already shipped); the named
  residual is CPU-heavy code inside a non-SM function that admission misses (see Future Requirements /
  Revisit #8).
- `--no-optimize` (CLI flag) and `YNZ_OPT_FORCE` (dev/bench env var): dev/CLI-only surfaces, no runtime
  dependency of their own.

### Kernel-Mode Behavior

- `ynz build --kernel` routes through the SAME optimizer pipeline as the default build — stated
  explicitly: **no kernel-mode-specific optimizer behavior is introduced.** The LLVM pass pipeline is
  orthogonal to the `--kernel` runtime-mode gate (kernel mode restricts allocator/scheduler USE at
  typeck/emit time; it says nothing about which backend passes run over already-emitted IR).
  `--kernel` + default-optimized and `--kernel` + `--no-optimize` both compile; the existing kernel-mode
  compile-error gates (heap-allocating shapes, etc.) fire identically regardless of optimization tier.
- Real back-edge poll-yield preemption (Phase 6): unaffected by kernel mode — `background`/scheduler
  behavior is already unavailable in `--kernel` builds without a user-supplied primitive
  ([`IMP-no-runtime-mode.md`](../../../../docs/internal/implementation/IMP-no-runtime-mode.md)),
  unchanged by this plan. The non-SM CPU-admission residual named above is likewise orthogonal to
  `--kernel` mode — kernel-mode compilation already restricts `background`/scheduler use entirely, so
  the residual applies only to non-kernel builds.

### Demo & Error Gallery

- **The `examples/pirates-roster/entrypoint.ynz` byte-exact golden staying GREEN under the new pipeline
  IS this milestone's demo obligation** — Phase 5 step 2 regenerates and byte-diffs
  `expected_stdout.txt` against the pre-M7 baseline; any divergence outside the documented M2
  scheduler-race ordering window (integration.rs:2596-2658, pre-existing, optimizer-independent —
  FRAGO 013) is treated as a correctness bug, not an expected regeneration (Phase 5's own exit
  criteria). **No new demo section is required**: this
  milestone changes the compiler's BACKEND, not the language surface — there is no new syntax/feature
  for `pirates-roster` to demonstrate in context.
- **ONE new compile-error class SHIPPED (Phase 2, FRAGO 002 — reconciled here per FRAGO 004): the
  aliasing-call rejection.** Its gallery obligation is met: `examples/primantis-orders/v0_3_m7_errors.ynz`
  exists with 3 intentional triggers (`share`+`lend` same value; `lend`+`lend` same value;
  whole-vs-part overlap), each carrying a `// WHY:` comment naming the diagnostic class, locked by
  `crates/ynz-driver/tests/error_galleries.rs::v0_3_m7_gallery_fires_expected_diagnostics`
  (diagnostic-count + key-phrase assertions — the established gallery convention). The remaining
  phases stay diagnostic-free as originally scoped: Phases 3, 5, 7 are backend/tooling work with no
  new diagnostic surface; Phase 4's fix removes a crash rather than adding an error class; Phase 6
  either ships a runtime behavior change (no new diagnostic) or records a registry deferral (not a
  diagnostic). If a FURTHER genuinely new user-facing error class surfaces, that becomes a live CCIR
  (¶3.4) and extends the same gallery file — the pattern this milestone has now exercised for real.

### Feature Registry Entries

- The preemption mechanism update (Phase 6): if call-site checks ship for real, **NO new registry
  entry** (matches `IMP-no-function-coloring.md`'s original lock exactly — Phase 6 step 6). If
  deferred instead, exactly ONE new deferral entry, per the four-field deferral. **Executed 2026-07-17:
  deferred; the entry shipped as `preempt-callsite-checks`, kind `[[deferred_tooling_feature]]`**
  (recategorized from the initially-authored `[[deferred_language_feature]]` at the Phase 6 review
  round: it gates the compile-time `YNZ_PREEMPT_CALLSITE_CHECKS` toggle — a compiler-internal
  mechanism with no user-typeable syntax — matching the sibling
  `cooperative-preemption-back-edge-yield` entry's classification on the identical topic).
- The now-shipped-for-real back-edge yield's stale `[[deferred_tooling_feature]]` entry
  (`cooperative-preemption-back-edge-yield`, describing the pre-M7 no-op stub): **modified** — retired
  to a comment-only historical note (mirroring the registry's existing `ec-wrapper-collect-on-completion`
  retirement precedent), since Phase 6 shipped the real loop back-edge poll-yield mechanism it deferred.
  **Executed 2026-07-17** (Phase 8 fix-loop round).
- `--no-optimize` (CLI flag) and `YNZ_OPT_FORCE` (dev/bench env var): **no registry entry** — mirrors
  the existing precedent already set by `--no-auto-parallel`/`YNZ_SOA_FORCE`, neither of which carry
  registry entries (CLI flags and internal test-only env vars are not language keywords, jargon,
  intrinsics, or diagnostic templates — the registry schema has no row kind for them). Stated
  explicitly so reviewers know it was considered, not forgotten.
- **Explicitly none** for the rest: no new keywords, banned_declaration_keywords, banned_jargon,
  primitive_intrinsics, type_attached_constants, diagnostic_templates, or muted_hint_domain entries —
  the milestone's remaining work is backend/codegen-tier; the ONE new language-surface item shipped
  (Phase 2's aliasing-call compile rejection, FRAGO 002/004) needs none of these entry kinds — its
  message is per-site dynamic (names/modifiers/paths interpolated), which stays in code per the
  feature-registry carve-out precedent ([`registry/features.toml`](../../../../registry/features.toml):1700-1705,
  the can't-infer-suspension per-site-dynamic note).

## 4. Sustainment

- **Docker (this project's universal convention):** `docker compose run --rm dev cargo build -p
  ynz-codegen`, `docker compose run --rm dev cargo test --workspace`, `docker compose run --rm dev
  cargo clippy --workspace -- -D warnings`, `docker compose run --rm dev cargo bench -p ynz-driver
  --bench soa_calibration` (existing) and the new `--bench opt_pipeline_calibration` (Phase 7). No
  `-it`; every dispatch is non-interactive.
- **LLVM/inkwell:** LLVM 18 via apt (`LLVM_SYS_181_PREFIX`), `inkwell` pinned at `0.9.0`
  (`llvm18-1-prefer-dynamic` feature). Phase 0's spike works within this pin; a version bump is its own
  decision (CCIR item 2), never a quiet substitution mid-phase.
- **Reference artifact (read-only):** the original spike's 2-line O0→Default diff, preserved as a
  checked-in unified diff at [`spike-o0-flip.patch`](./spike-o0-flip.patch) (this plan's own directory)
  per plan-spike-discipline Facet 2 — Phase 1 reads it, does not recreate it from memory. The
  evidence of what that diff broke (the 6/470 failure list, direct-repro SIGSEGV) lives in
  [`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)'s
  "Phase-0 spike" section — the primary durable evidence Phase 1 corroborates against.
- **CI:** [`.github/workflows/ci.yml`](../../../../.github/workflows/ci.yml), Linux-only
  (`ubuntu-latest`). Every phase's exit criteria must keep `cargo fmt --check`, `cargo clippy --workspace
  -- -D warnings`, `cargo test --workspace`, and `cargo build --workspace --release` green.
- **Golden regeneration tooling:** `crates/ynz-codegen/tests/golden.rs` (auto-regenerates on first run,
  per its own doc comment) and
  `examples/pirates-roster/expected_stdout.txt.regenerate.sh`.
- **Sibling plan:** v0.3-M6 (correctness hotfixes, authored in parallel, not yet a plan file on disk) —
  this plan branches from `main` after M6 merges (CCIR item 1).

## 5. Command & Signal

- **Ownership:** each phase is picked up by whichever executor session the execute-plan conductor
  dispatches next; no named individual owner beyond Patrick's overall sign-off/release authority.
- **Succession:** standard plan-format succession — this `plan-id` + the session-id chain + checkbox
  state in this file. Phases 2, 5, and 7 (scale=large, checkpoint-marked) use `handoff-phase-<N>.md`
  per the [Handoff file convention](../../../../../.claude/docs/reference/REF-plan-format.md#handoff-file-convention)
  when a segment checkpoints.
- **Audit trail:** `audit.md`, sibling to this `plan.md` in whichever status-folder currently holds
  it (created at this amendment pass, under `paused/`; the status↔folder invariant moves the whole
  directory — `plan.md` + `audit.md` together — when status flips to `active`, never a path an
  executor hardcodes) — session log + FRAGO log, append-only. The roadmap's own `audit.md` receives
  the Phase 8 ledger-reconciliation entry as a separate append, not a duplicate of this plan's own
  record.

## Future Requirements / Revisit

1. **Selective hot-field-only element materialization** (roadmap ledger row 442 / M5 Future
   Requirements #15, FRAGO 020) — **WHAT:** SoA codegen computes `hot_fields` via `soa_candidate_query`
   but `soa_gather_into`/`array_elem_get_into` never consume it; every field is gathered unconditionally.
   **WHY not absorbed here:** a different concern (SoA-specific gather selectivity, not the backend pass
   pipeline) — folding it in would mix two unrelated fix classes into one review surface and blow up
   this plan's phase count. **COST:** ~1 dedicated session + E3/E6/E9-style re-review per its own
   ledger text (every full-element consumer must be re-audited). **TRIGGER:** before or alongside any
   future optimization-pipeline milestone — since this plan IS that milestone, the trigger is live now;
   recorded here as a deliberate, reasoned non-absorption for Patrick's own call on sequencing, not a
   silent drop.
2. **Build/release-tooling: ABI-version-checked runtime archive embedding** (roadmap ledger row 440) —
   **WHAT:** `ynz-driver/build.rs` embeds the runtime archive with no ABI/version check; a stale
   cross-profile archive can silently miscompile. **WHY not absorbed:** unrelated bug class (build/release
   tooling), already has a cheap operational mitigation in place (rebuild-from-clean, per the M5 FRAGO
   018 precedent cited in this repo's own `no-duct-tape.md`). **COST/TRIGGER:** unchanged from the
   ledger's own text — the next milestone touching `ynz-runtime`'s ABI, or external binary distribution.
3. **Codegen ICE: bare int literal into a `number`-typed slot** (roadmap ledger row 441, ELEVATED
   priority per Patrick's 2026-07-04 triage — a real user-facing crash on common valid code) — **WHAT:**
   `store`/`store_field`'s `Type::Number` arm assumes a decimal128-pointer representation while
   `Expr::IntLit` lowers to a raw `i64`; typeck admits the coercion, codegen panics. **WHY not absorbed:**
   pure pre-existing literal-lowering bug, orthogonal to the optimizer pipeline and to concurrency
   codegen. **COST/TRIGGER:** unchanged from the ledger's own text — next milestone touching
   numeric-literal codegen, or immediately if a real user hits it. Likely belongs to v0.3-M6 or a
   dedicated hotfix given its ELEVATED classification; not this plan's charter.
4. **Authoritative-derivation write-time guard** (roadmap ledger row 438) — **WHAT:** a mechanical,
   write-time hook catching a second independent derivation of an already-authoritative value before it
   lands. **WHY not absorbed:** a `hook-author` artifact (process/tooling), unrelated to this milestone's
   codegen/perf charter. **COST/TRIGGER:** unchanged — Patrick's own prioritization call, or the next
   milestone-planning session touching compiler-pass/derived-constant work.
5. **`background.cpuBound` explicit override syntax** (concurrency-release-audit P4-2, MEDIUM — the
   auto-promotion "force-the-other-pick" direction documented in
   [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
   but never implemented, no registry entry) — **WHAT/WHY:** unrelated to the optimizer flip itself; a
   different capability (CPU-bound task-routing override). **COST:** small (spawn-site annotation +
   registry entry). **TRIGGER:** the next milestone touching `background`/task-routing surface — v0.3-M6
   or a future one; not this plan's charter.
6. **Preemption call-site checks deferred (Phase 6 measurement-gated decision, 2026-07-17)** — the
   fresh default-pipeline measurement FAILED the pre-registered threshold ("≤5% median wall-clock on
   the fib(30) call-heavy microbenchmark, 5 runs per configuration, toggle-on vs toggle-off" —
   registered in the Phase 6 audit entry BEFORE measuring): toggle-ON median 132ms vs toggle-OFF
   median ~26.5ms = **~+398% overhead** (far below the O0-era 1190%, still ~80× over the bar — the
   opaque per-call-site check defeats inlining at exactly the call boundaries the optimizer
   milestone sped up). Per the gate: NOT shipped. **WHAT:** emitting `ynz_rt_check_preempt` at every
   user-function call site (the "call sites" half of IMP-no-function-coloring.md's safe-point
   model; the loop-back-edge half shipped for real in this same phase). **WHY:** measured cost vs
   bounded benefit — back-edge poll-yield covers the hot-loop starvation class; the uncovered
   residual is loop-free CPU-bound recursion inside a state-machine function, undemonstrated in any
   real workload. **COST to fix later:** small mechanically (emission exists behind the
   `YNZ_PREEMPT_CALLSITE_CHECKS` compile-time toggle at the direct user-call choke point in
   `emit.rs`, microbenchmark fixture committed at
   `crates/ynz-driver/tests/fixtures/v0_3_m7_p6_callsite_overhead_fib.ynz`); the real cost is a
   cheap-check design (inlinable fast path / non-inlined-boundary-only checks) plus re-measurement.
   **TRIGGER:** a reproduced starvation incident traced to loop-free CPU-bound recursion in an SM
   function, OR a cheap-check design making a re-measurement plausible, OR PGO/LTO call-boundary
   work re-opening the question. Registry entry `preempt-callsite-checks`
   (`[[deferred_tooling_feature]]`, [`registry/features.toml`](../../../../registry/features.toml))
   added at Phase 6 execution time per this plan's Feature Registry Entries section (kind
   recategorized from `[[deferred_language_feature]]` at the Phase 6 review round — a compile-time
   toggle, not user-typeable syntax).
7. **"As fast or faster than Rust" — measured NOT achieved as of v0.3-M7; closing the gap is its own
   future work (FRAGO 014)** — Phase 7's committed numbers
   (`crates/ynz-driver/benches/rust-equiv-raw-2026-07-17.md`): idiomatic Rust `--release` is
   2.70x / 2.25x / 7.20x faster than shipped Yinz on cpu_loop / shape_alloc / soa_physics
   (2.19x / 1.60x / 9.93x vs overflow-checks-matched Rust). **WHAT:** closing the measured gap; the
   evidence-backed contributors, largest first: (a) the opaque runtime-call ABI floor —
   `ynz_array_get` per element access, which LLVM cannot inline or vectorize across (~0.48 vs
   ~3.46 ns/visit on soa_physics — essentially all of the 7x); (b) always-on overflow-check
   semantics (roughly a fifth to a third of the scalar gap, quantified by the release-checked
   column); (c) missing LTO/PGO/vectorization tuning in the `default<O2>` tier. **WHY deferred:**
   each contributor is a milestone-scale mechanism (array-access intrinsic lowering or
   runtime-call inlining across the FFI boundary; cross-language LTO against `libynz_rt`; PGO
   plumbing) — out of this plan's pipeline-correctness charter. **COST:** milestone-scale per
   contributor; the array-ABI floor alone touches codegen + runtime ABI. **TRIGGER:** the next
   performance-positioning milestone (roadmap successor to ledger row 443), or the first
   user-facing doc/marketing claim that needs parity numbers to be honest.
8. **Named residual: CPU-heavy code inside a non-SM (non-wait-containing) function that CPU-admission
   misses** — **WHAT:** Phase 6's back-edge poll-yield mechanism only exists for loops inside
   state-machine functions; a plain synchronous function's hot loop can never cooperatively yield this
   way, and its only protection is the existing CPU-admission routing to the blocking pool, which is a
   heuristic (admission), not a guarantee. **WHY not fixed now:** no cooperative-yield mechanism exists
   for non-SM functions in this language's design (per
   [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md) —
   plain functions are, by design, synchronous and non-suspending); closing this residual would require
   either a new function-coloring-adjacent mechanism (a real design change, out of this phase's charter)
   or provably tightening admission's classification, neither of which this plan's scope covers.
   **COST:** unknown until scoped — likely a dedicated design session revisiting
   `cpu_admission.rs`'s classification boundary. **TRIGGER:** a real, reproduced starvation incident
   traced to a non-SM CPU-bound function that admission misclassified, or the next milestone
   revisiting `cpu_admission.rs`/scheduler design.
9. **Patrick-directed addition 2026-07-16 (M6 completion triage)** — **fr23: non-plain-ident
   background-spawn receivers ride as raw pointers today** (roadmap Capability Ledger row,
   Idempotency-Key
   `2026-07-04-v0-3-m6-concurrency-hotfix#8-fr23: non-plain-ident-background-spawn-receivers`).
   **WHAT:** `background fleet.flagship.haul()` / `ships[0].haul()` / `background
   haul(fleet.flagship)` — field/index/return-materialized `background`-spawn receivers heap-upgrade
   as raw pointers, gated only on `Expr::Ident`/explicit `.copy()`. **WHY this is homed to M7, not
   left at the prior "latent, not confirmed-live" verdict:** that verdict was gathered under
   `OptimizationLevel::None`, where LLVM keeps stack slots alive artificially long; THIS plan's
   optimizer pipeline shrinks/reuses stack-slot lifetimes — the exact conditions that turn a latent
   raw-pointer ride into a live UAF. This plan's own optimizer flip is the expiry condition on the
   prior verdict. **DISPOSITION — M7 must do ONE of:** (a) fix the give/copy machinery for
   field/index/return-materialized spawn receivers as an early phase, OR (b) gate: re-run the UAF
   repro (the three call shapes above) under the real optimized pipeline (post Phase 2/3 wiring) and
   route a confirmed-live result like the R13/R14 signed-risk overrides. **TRIGGER:** this plan's own
   Phase 2/3 optimizer-pipeline wiring landing — at that point the repro becomes runnable under real
   optimization and (b) is executable. This entry is the tracking record; the plan's next
   amendment/review cycle should fold it into a real phase.
   **GATE EXECUTED 2026-07-17 (disposition (b), `executor-2026-07-17-fr23-uaf-gate`, HEAD `3e3bf6c`;
   routed via FRAGO 011 → risk row R11 above). Per-shape verdicts:** A (field-access receiver) and
   C1 (call-form field-access arg) — **STILL-LATENT**: genuinely protected by `field_own_cell`
   heap-cell allocation (correct output 6/6 at both tiers); B′ (maybe-payload receiver, the nearest
   expressible index variant) — **CONFIRMED-LIVE at BOTH tiers** (optimized `0/0` 6/6; O0 stomp
   sentinels — live before this plan's flip, detection only was deferred); C2 (call-materialized
   receiver `makeCargo().haul()`) — **CONFIRMED-LIVE** (O0 wrong 6/6; optimized output correct by
   stack-layout luck over IR-proven identical dangling). Both confirmed shapes are locked by
   committed `#[ignore]`d planned-RED tests (`crates/ynz-driver/tests/fr23_uaf_planned_red.rs` +
   fixtures `v0_3_m7_fr23_maybe_payload_spawn_receiver.ynz` /
   `v0_3_m7_fr23_call_materialized_spawn_receiver.ynz`). **Decision-context addition (2026-07-17
   cleanup round):** Phase 4's back-edge restore makes the fr23 shapes deterministically worse
   inside plain loops — the un-upgraded payload alloca a spawned task points into is now FREED at
   every plain-loop back-edge, turning the UAF into a deterministic per-iteration stomp
   (per-iteration stomp; code-reviewer 2026-07-17; documented at emit.rs `loop_stack_save`'s KNOWN
   EXCEPTION note, since rewritten to record closure) — strengthened the case for disposition (a)
   fix-in-plan. **MORNING DECISION MADE 2026-07-18 (FRAGO 016,
   `conductor-2026-07-18-completion-gate`):** disposition (a) — fix lands in THIS plan; **Phase 9 —
   Close the fr23 Confirmed-Live UAF** inserted (§3.3) to execute it. **DISPOSITION (a) EXECUTED
   2026-07-18 (Phase 9, `executor-2026-07-18-phase9-fr23-fix`): CLOSED.** What shipped: typeck's
   spawn-site normalization + ownership recording extended to the two confirmed-live receiver/arg
   shapes via ONE new admission helper (`bg_arg_is_materialized_shape_temp`, `check.rs` — B′
   maybe-payload access `m.value` on a `maybe<Shape>` binding; C2 call-materialized
   shape-returning call), recorded as `BgOwnership::Give` in `background_arg_inferred_ownership`
   (a materialized temp has no binding to read after the spawn); codegen's `is_heap_arg` gate
   (`emit.rs`, `prepare_bg_arg_for_ctx`) now consults that ONE authoritative record by span for
   any expression shape (byte-identical for the Ident arm it subsumes; explicit `.copy()`
   unchanged) — the existing `BgArgFreeKind::HeapShape` heap-upgrade/free ladder does the rest,
   both spawn arms (CPU `lower_expr_background` + SM `lower_sm_background_spawn`) covered since
   they share `prepare_bg_arg_for_ctx`. Never a sibling path (authoritative-derivation.md).
   Verified: both planned-RED tests pass with `#[ignore]` removed (now permanent regression
   locks); both fixtures independently re-run at BOTH tiers print `haul: 111/222`; the two
   `cross_impl_consistency.rs` exclusions removed (FRAGO 012's named trigger fired) with the
   corpus sweep clean, both fixtures included. A/C1 (field-access shapes, `field_own` heap-cell
   protected, still-latent) remain untouched and out of scope — the admission helper explicitly
   excludes field-access expressions. **Fix-round addition (2026-07-18,
   `executor-2026-07-18-phase9-fr23-fixloop`):** the C2 arm's callee resolution now ALSO falls back
   to `generic_fn_table` (security review live-reproduced the UAF for a generic shape-returning
   callee, `background identity(c).haul()` with `identity<T>(give T) -> T` — the sig_table-only
   read silently missed it); the instantiated return type is resolved with the same
   `unify_param`/`apply_substitution` machinery `check_generic_fn_call` uses, never a sibling
   scheme. SM-spawn-arm coverage (both shapes through `lower_sm_background_spawn`) and the
   generic-B′ analog (safe by construction — the B′ arm reads the concrete instantiated binding
   type, never the fn tables) are locked by four new fixtures/tests in `fr23_uaf_planned_red.rs`.
   **Fix-round addition (2026-07-18, cumulative completion-gate round 2, FRAGO 018):** the C2
   arm's argument-based substitution-seeding loop was ALSO ident-only, missing a generic callee
   whose type param is resolvable only from a NON-IDENT argument (a nested call, e.g.
   `background identity(makeCargo()).haul()` — `identity<T>`'s `T` is bound only by `makeCargo()`'s
   return type, never by any ident). Live-reproduced at both tiers before the fix (default:
   nondeterministic garbage; O0: `haul: 0/1`). Fixed via a new side-effect-free helper
   `bg_arg_type_readonly` that resolves a plain-ident arg (unchanged) OR a nested call whose
   callee has a concrete `sig_table` signature — bounded to the confirmed-live case, never a
   general-purpose expression-typer (per the same `&self`-only architectural constraint the
   Frame/SourceLoc gap deferral names, roadmap audit.md Idempotency-Key
   `2026-07-04-v0-3-m7-optimizer-pipeline: crates-ynz-typeck-src-check-rs-1783`). Test-locked
   (`fr23_generic_call_nested_arg_spawn_receiver_reads_live_values`,
   `v0_3_m7_fr23_generic_call_nested_arg_spawn_receiver.ynz`).

   **FOURTH fix-round 2026-07-18 (scoped fix-loop round, FRAGO 019) — closes the class instead of
   patching a 5th narrow instance.** Round 3's fix was itself still a hand-rolled, ONE-LEVEL-ONLY
   special case: `bg_arg_type_readonly`'s own nested-call arm read `sig_table` ONLY, never falling
   back to `generic_fn_table` — so a nested argument that is ITSELF a call to a GENERIC function
   (`background identity(identity(makeCargo())).haul()` — the outer `identity`'s argument is a call
   to `identity` again, not a concrete callee) left `T` unresolved and the receiver fell through
   un-admitted, one level deeper than round 3 closed. Two independent reviewers (code-reviewer,
   security) live-reproduced this as nondeterministic garbage at the default tier and
   deterministic wrong values at O0, and both converged on the same diagnosis: this predicate had
   accreted into 2-3 separate, hand-rolled "what does this call resolve to" derivations instead of
   ONE authoritative, recursive one — a fourth narrow patch would only guarantee a fifth. The fix
   collapses `bg_arg_is_materialized_shape_temp`'s C2 arm AND `bg_arg_type_readonly`'s nested-call
   arm into ONE authoritative, RECURSIVE resolver, `bg_call_return_type_readonly` (`check.rs`):
   resolves a call expression's return type — concrete callee (direct `sig_table.ret`) or generic
   callee (the same `sig_table`/`generic_fn_table` two-table split + `unify_param`/
   `apply_substitution` machinery `check_generic_fn_call` uses) — at ANY nesting depth, by
   recursing into `bg_arg_type_readonly` for each argument that is itself a call. Stays
   `&self`/side-effect-free (never calls `infer_expr`/`ast_type_to_type`) per the same documented
   architectural constraint the round-3 fix and the Frame/SourceLoc deferral both name; an
   unresolved `TypeParam` still never matches `Shape`, so this cannot false-admit. **Verified at
   BOTH 2-deep** (`identity(identity(makeCargo()))`) **AND 3-deep**
   (`identity(identity(identity(makeCargo())))`) **nesting**, both tiers, 5 repeated runs each
   post-fix (deterministic `haul: 111/222` every run); pre-fix Paper-Trace independently
   re-confirmed both new fixtures genuinely RED (garbage at default tier, `stomp()`-sentinel-class
   wrong values at O0) via a scoped revert-rebuild-restore of `check.rs` alone. Termination:
   recursion descends into strictly smaller argument subexpressions of a finite, cycle-free AST
   (a call's arguments can never contain the call itself), so it poses no new stack-safety concern
   beyond what any other recursive typeck walk (`infer_expr` included) already tolerates for
   deeply-nested source — considered explicitly, not a silent risk. Two new permanent regression
   fixtures/tests: `fr23_generic_call_nested_generic_arg_spawn_receiver_reads_live_values`
   (`v0_3_m7_fr23_generic_call_nested_generic_arg_spawn_receiver.ynz`, 2-deep) and
   `fr23_generic_call_triple_nested_spawn_receiver_reads_live_values`
   (`v0_3_m7_fr23_generic_call_triple_nested_spawn_receiver.ynz`, 3-deep, the proof the fix is
   genuinely recursive and not depth-bounded). Full `fr23_uaf_planned_red.rs` suite re-run: 9/9
   green. **R11 is CLOSED as of this round** — see the risk-table row above for the consolidated
   four-round history.

   **FIFTH fix-round 2026-07-18 (scoped fix-loop round, FRAGO 020) — unifies the class instead of
   patching a 6th narrow instance.** Round 4's "R11 is CLOSED" verdict above was accurate for the
   nesting-DEPTH question it closed (a `Call` nested inside a generic call, at any depth) but did NOT
   close a DIFFERENT, ORTHOGONAL gap in the SAME predicate: `bg_arg_is_materialized_shape_temp`
   (top-level admission) and `bg_arg_type_readonly` (the nested-argument resolver
   `bg_call_return_type_readonly` seeded its substitution from) were STILL two independently
   hand-rolled enumerations of "which expression shapes materialize a temp" — the top-level predicate
   recognized `FieldAccess`/`Call` (plus `MethodCall`, indirectly, via
   `background_spawn_call_form`'s own normalization) while the nested resolver recognized ONLY
   `Ident`/`Call`. Two independent reviewers (code-reviewer, security) live-reproduced this
   independently for TWO NEW shapes NESTED inside a generic call's argument — a UFCS method-call
   chain (`background identity(makeCargo().reroute()).haul()`) and a maybe-payload field access
   (`background identity(first.value).haul()`) — both garbage at both tiers, both converging on the
   same root cause and the same recommended fix direction. Rather than add two more match arms to
   `bg_arg_type_readonly` (the sixth narrowing in this predicate's history), BOTH enumerations were
   collapsed into ONE exhaustively-matched classifier, `bg_expr_resolved_type` (`check.rs`) — every
   one of `Expr`'s 22 variants is listed explicitly with **no `_ =>` catch-all**, so the Rust compiler
   itself refuses to build the moment a future `Expr` variant is added without a classification
   decision here. `bg_arg_is_materialized_shape_temp`, `bg_call_return_type_readonly` (plain `Call`),
   and a new `bg_ufcs_return_type` (UFCS `MethodCall`, self-inclusive parameter alignment — the
   receiver fills the callee's first parameter position, unlike plain-`Call`'s self-EXCLUDING
   alignment, which stays unchanged) all now consult this ONE classifier — never a fourth hand-rolled
   scheme. **Honesty note on the compile-time claim:** the exhaustive match genuinely guarantees no
   `Expr` variant can be SILENTLY un-classified again (a real compile-time guarantee, not a runtime
   parity test standing in for one) — it does NOT guarantee no future bug can exist in HOW an
   already-classified variant's alignment/substitution is computed. **Verified:** both new repros
   fixed, 3 repeated runs each, both tiers, deterministic `haul: 111/222`; pre-fix Paper-Trace
   re-confirmed both genuinely RED (nondeterministic garbage, e.g. `haul: 1/6355112`,
   `haul: 976778432/976778432`) via a scoped before/after manual block-swap of `check.rs` (NOT
   `git stash` — this branch's `push`-substring graveyard pre-filter fires on `git stash push` as a
   false positive; the comparison used a saved-block swap instead, avoiding the gate entirely rather
   than routing through its remediation for an unrelated command). One additional self-authored
   adversarial construction beyond the two reported repros — a `MethodCall` whose receiver is itself
   a `FieldAccess` (`first.value.reroute()`), AND a `Call` whose argument is a `MethodCall` whose
   receiver is a nested `Call`, wrapped in a second generic layer
   (`identity(identity(makeCargo().reroute()))`) — also verified correct at both tiers, 3 runs each.
   Two new permanent regression fixtures/tests:
   `fr23_generic_call_ufcs_nested_arg_spawn_receiver_reads_live_values`
   (`v0_3_m7_fr23_generic_call_ufcs_nested_arg_spawn_receiver.ynz`) and
   `fr23_generic_call_fieldaccess_nested_arg_spawn_receiver_reads_live_values`
   (`v0_3_m7_fr23_generic_call_fieldaccess_nested_arg_spawn_receiver.ynz`). Full
   `fr23_uaf_planned_red.rs` suite re-run: 11/11 green (strict superset — 9 pre-existing + 2 new);
   `cargo test -p ynz-typeck` all green; `cargo build --workspace` clean; `cargo clippy --workspace
   -- -D warnings` clean; `cargo fmt --all -- --check` clean; `cross_impl_consistency.rs` corpus
   sweep re-run — see the FRAGO 020 audit.md entry for the verdict. ~~**R11 is CLOSED as of this
   round, on the genuinely structural basis described above**~~ — **FALSIFIED by round 6 (FRAGO
   021) below; R11 is REOPENED.** The struck-through claim was accurate for the nesting-DEPTH
   question it addressed but did not survive a full exhaustive-arm semantic audit.

   **SIXTH round 2026-07-18 (FRAGO 021) — semantic audit found 4 confirmed-live wrong/incomplete
   arms in the SAME "exhaustively-matched" classifier; R11 REOPENED, no code fix applied this
   round.** Security live-reproduced `background haul({ weight: 111, tag: 222 })` — an anonymous
   struct literal used directly as a background-spawn argument, wrong at both tiers — proving
   `bg_expr_resolved_type`'s `Expr::StructLit { .. } => None` arm is semantically WRONG (not
   missing): `lower_struct_lit` always materializes it as a stack alloca in the spawner's frame.
   Per this round's explicit dispatch instruction, rather than fix StructLit and move on (which
   would have been the SEVENTH narrowing), this round ran a full 22-arm audit of
   `bg_expr_resolved_type` against real, live-tested Yinz semantics rather than reasoning from
   type signatures alone. Found **3 ADDITIONAL distinct confirmed-live wrong/incomplete arms**
   beyond StructLit: (2) `FieldAccess { field: "value" }`'s guard only recognizes `maybe<Shape>`
   receivers, silently missing `MapEntry<K, Shape>.value` (a for-loop map-iteration entry's Shape
   field, backed by a per-site out-buffer rewritten every iteration — the SAME hazard class the
   MapEntry-as-whole-arg pre-gate already protects against, but not this access pattern); (3)
   `PostfixOp` (`.copy()`) nested inside a generic call's argument fails to seed the generic
   substitution, so `identity(c.copy())` used as a spawn receiver is never recognized as
   Shape-typed; (4) the same substitution-seeding gap for `Wait`-wrapped nested call arguments
   (`identity(wait makeCargo())`) — M8 sequential semantics make `wait expr` type-identical to
   `expr`, but the classifier does not unwrap it. All 4 findings independently live-repro'd (build
   + run, 3-6 runs per tier, Paper-Traced) and persisted as checked-in, documented-RED fixtures:
   `v0_3_m7_fr23_structlit_spawn_receiver.ynz`, `v0_3_m7_fr23_mapentry_value_spawn_receiver.ynz`,
   `v0_3_m7_fr23_generic_call_copy_nested_arg_spawn_receiver.ynz`,
   `v0_3_m7_fr23_generic_call_wait_nested_arg_spawn_receiver.ynz` — NOT wired into
   `fr23_uaf_planned_red.rs`'s green suite (that file's header claims every test in it is fixed).
   One hypothesis (`SelfValue`, arm 15) was live-tested and DISCONFIRMED — recorded in the FRAGO
   021 audit entry for transparency, since the round's whole premise was distrusting
   reasoned-but-unverified classifications. This round crossed its own explicit "3+ additional
   wrong arms" STOP threshold, so NO code fix was applied — not even the unambiguous StructLit
   fix — pending a conductor decision on whether to continue narrowing (a 7th, 8th, 9th round) or
   redesign the classifier as **default-DENY** (heap-upgrade everything that is not a
   provably-safe, stable `Ident`/`SelfValue`/primitive binding, rather than allowlisting each
   materializing shape one at a time). Full 22-arm audit table, all 4 Paper-Traces, and the
   disconfirmed-hypothesis record are in the FRAGO 021 audit.md entry.

   **SEVENTH round 2026-07-18 (FRAGO 022 decision + FRAGO 023 execution) — ARCHITECTURAL CLOSURE.**
   Patrick decided the fork directly (FRAGO 022, `conductor-2026-07-18-completion-gate`), shown the
   full six-round record (10 confirmed-live shapes) and both options' tradeoffs: **default-deny**.
   `bg_arg_is_materialized_shape_temp` (the allowlist — "is this one of the shapes we've confirmed
   dangerous?") is replaced by `bg_arg_is_provably_safe` (the denylist-of-safety — "is this PROVABLY
   a stable, already-owned binding, or an expression whose STATIC TYPE can never be `Shape`?"), with
   every caller heap-upgrading whatever the new predicate does NOT affirmatively prove safe, via a
   trailing WILDCARD `_ => false` arm. FRAGO 023 executed this: all THREE admission call sites
   updated (`check_stmts`'s statement-form give/copy loop; `check_background_handle_spawn`'s
   handle-form loop; AND `background_spawn_call_form`'s non-Ident-receiver UFCS-normalization gate —
   this third site needed the identical predicate too, which is what closes FRAGO 021 finding 2
   (`entry.value.haul()`, a `MapEntry<K,Shape>.value` UFCS receiver) with ZERO special-case arm: a
   `FieldAccess` is never in the safe set, so the wildcard now normalizes it regardless of which
   `.value` producer it came from). `bg_expr_resolved_type`/`bg_call_return_type_readonly`/
   `bg_ufcs_return_type`/`bg_apply_generic_return_subst` are UNCHANGED — reused per
   authoritative-derivation.md, not rewritten — but their `None` result is now consumed FAIL-CLOSED
   everywhere (an unresolved generic substitution seed is read as "not proven safe," never as "proven
   not-Shape"), which is what closes findings 3/4 (nested `.copy()`/`wait` inside a generic call's
   argument) without adding a single arm to the seeding resolver.

   **Verified — all 4 of FRAGO 021's fixtures pass via the DEFAULT, no special-case arm added**:
   `fr23_uaf_planned_red.rs` 15/15 (11 pre-existing + 4 newly wired —
   `fr23_structlit_spawn_receiver_reads_live_values`,
   `fr23_mapentry_value_spawn_receiver_reads_live_values`,
   `fr23_generic_call_copy_nested_arg_spawn_receiver_reads_live_values`,
   `fr23_generic_call_wait_nested_arg_spawn_receiver_reads_live_values`), strict superset, no
   regression on any already-fixed shape. **Three self-authored adversarial constructions**, none
   reusing an existing fixture's exact AST shape, all 6/6 correct at both tiers: (1) a bare
   `StructLit` used as a spawn ARGUMENT through the HANDLE form (`let h = background
   haul({...})`), the first fr23 fixture to exercise `check_background_handle_spawn` for a
   materialized-shape ARG rather than a receiver; (2) `identity(entry.value).haul()` —
   `MapEntry<K,Shape>.value` nested inside a GENERIC call's substitution-seeding argument (finding 2
   was tested only as the direct UFCS receiver, a different code path); (3) `ship.cargo` — the
   still-latent A/C1 class used DIRECTLY as a spawn argument, never exercised by ANY fr23 fixture
   before this round. A fourth candidate (a bare `StructLit` nested inside a purely generic call's
   argument) was constructed and found genuinely UNREACHABLE valid Yinz (the real type checker
   rejects it — a struct literal has no type without a concrete expected-parameter context, which a
   generic parameter cannot provide) — recorded as a verified negative result, not silently dropped.
   One regression probe (scoped `// TEMP-PROBE-DISABLE` revert to old-allowlist semantics, restored
   and `diff`/`md5sum`-confirmed byte-identical afterward) empirically confirmed construction (2) WAS
   genuinely vulnerable pre-redesign (O0 deterministic `haul: 111/0` 6/6; optimized nondeterministic
   leaked garbage 6/6 — the established fr23 UAF signature). Full `cross_impl_consistency.rs` corpus
   sweep (~557 fixtures, 2×2 mode matrix) clean; full `ynz-driver` test suite (20 test binaries
   including the 523-test `integration.rs`) clean, zero `FAILED`; `cargo test -p ynz-typeck -p
   ynz-codegen` clean. Performance sanity check (a 20,000-iteration `background`-spawn-heavy
   workload, field-access-arg variant vs. plain-ident-baseline variant) showed indistinguishable
   wall-clock (2.03-2.08s both) — empirically confirming, not merely asserting, that over-admission
   costs one typeck-time hashmap entry and zero LLVM IR for anything that doesn't resolve to
   `Shape`/`BuiltinArray`/`Maybe` at codegen time.

   **R11 status: CLOSED — architecturally, not empirically.** What is now GUARANTEED: any
   `background`-spawn expression that is not provably a stable `Ident`/`SelfValue` binding or a
   statically-non-`Shape` primitive gets heap-upgraded, so a FUTURE unknown-dangerous shape — one
   this round's own adversarial testing did not think of, or one a future milestone's grammar adds —
   cannot slip through un-upgraded BY CONSTRUCTION, closing the class the six-round allowlist could
   never structurally close. What is NOT guaranteed (named honestly, not overclaimed): a latent bug
   INSIDE the safe-set proof itself — e.g. the pre-existing, untouched `Ident` liveness-based
   give/copy path — is a categorically narrower, different risk surface than "did we allowlist every
   dangerous shape"; this round found no evidence of one, but that is a distinct claim from "the
   class is closed." A/C1 (`ship.cargo`) is no longer "still-latent, deliberately excluded" — it
   rides the same default `Give` protection as every other non-safe expression, verified harmless
   (one redundant, not unsafe, heap copy) both by reasoning and by adversarial construction (3)
   above. Full record: FRAGO 022 (the decision) and FRAGO 023 (the execution) audit.md entries.

   **EIGHTH round 2026-07-18/19 (FRAGO 024, round 8) — narrowing to this entry, added by FRAGO 025
   (2026-07-19) so a reader consulting this entry in isolation is not misled by the stale
   FRAGO-023-era "CLOSED, architecturally" text above.** FRAGO 023's own security re-check found TWO
   remaining gaps in the just-shipped default-deny architecture, both closed this round: (1) a
   **structural wiring gap** — the admission-recording machinery (`bg_arg_is_provably_safe` + its
   recording loop) ran on only 2 of the syntactic positions a `background` spawn can occupy
   (`check_stmts`'s `Stmt::Expr` match, `check_let`'s handle-form); `check_assign`/
   `check_field_assign`/`check_index_assign` routed through the generic `infer_expr`
   `Expr::Background` arm with NO recording at all — live-reproduced (`hd.slot = background
   makeCargo().haul()`, a `FieldAssign` target, wrong at O0). Closed by moving the recording loop
   into the generic arm itself, the one place every spawn form/statement-position/expression-
   embedding provably passes through. (2) A **`SelfValue` false-safe classification** —
   `bg_arg_is_provably_safe`'s blanket `SelfValue => true` assumed self's storage always outlives any
   spawn using it, true for the single-level case FRAGO 021 tested but false for a NESTED spawn (a
   `give self` parameter whose owning function is itself reached via `background`) — live-reproduced
   16/16 this round. Closed by removing `SelfValue` from the safe set; it now rides the same default-
   `Give` wildcard as every other non-enumerated shape. A THIRD finding (Bug 3, the borrow-reject
   diagnostic's `Some(Share)`-only gating) was investigated live, found to break 15/15 pre-existing
   fr23 fixtures if applied as literally instructed, and DEFERRED with a four-field record (FRAGO 024
   audit entry) — not a memory-safety gap, a missing teaching nudge (see FRAGO 025's sharpened
   wording of that deferral, which also names a separate, non-memory-safety semantic-correctness gap
   the deferred check would have caught: an unannotated `background`-spawned Shape receiver that
   mutates its parameter silently mutates only the task's PRIVATE heap-upgraded copy, never the
   caller's original binding, with zero diagnostic today).

   **R11 status after round 8 (FRAGO 024) — CLOSED, architecturally, for the wiring/self-
   classification gaps this round found.** The admission machinery is now confirmed structurally
   wired to EVERY syntactic position a `background` spawn can occupy (not just the two call sites
   FRAGO 023 verified), and `self` no longer carries a false-safe classification. Still NOT
   guaranteed, stated precisely, per FRAGO 024's own conservative framing: (a) a latent bug inside
   the pre-existing `Ident` liveness path itself remains unaudited by any round to date; (b) two
   syntactic positions (`Stmt::Assign`, `IndexAssign` on `nothing`-typed storage) are proven correct
   only at the TYPECK layer — full codegen/runtime proof is blocked by an orthogonal, pre-existing,
   out-of-scope ICE class; (c) the borrow-reject diagnostic's default-ownership teaching gap (Bug 3)
   remains OPEN, deferred with a four-field record — a missing teaching nudge, not a memory-safety
   hole, since the admission machinery above already protects the underlying argument regardless of
   this diagnostic. `STATUS: DONE` returned 2026-07-19 (FRAGO 024, round 8). Full record: FRAGO 024
   audit.md entry; see also the R11 risk-table row (¶1 Risk Assessment) for the equivalent summary in
   that table.
10. **N1: `fixed<T>` function returns are broken at BOTH tiers** (Phase 3 Step-4b sweep finding,
    FRAGO 009 disposition (5)) — **WHAT:** a `-> fixed<T>` return loses its size through the return
    (probe prints `0,0` for `[7,8,9].get(0)/.get(2)` after return), identically at O0 and optimized —
    tier-identical, so NOT an R9/CCIR-3 O0-reliant differential class; likely size-loss through the
    pointer-returning ABI (`fixed<T>` is the one known remaining pointer-returning case the Step-5 IR
    audit's scope-honesty note names). **WHY not absorbed here:** pre-existing at both tiers and
    orthogonal to this plan's optimizer-flip charter (no O0-vs-opt divergence to gate); it needs its
    own risk-row/ABI decision (by-value vs sized-header return), not a ride-along fix inside an R9
    fix round. **COST:** ~0.5-1 session — a sized `fixed<T>` return ABI (mirror `abi_return_type`'s
    by-value discipline or return a sized heap copy) + a differential/both-tier fixture. **TRIGGER:**
    the milestone that owns return-ABI completion (natural sibling of the roadmap's decimal128
    return-ABI row, now largely absorbed by this plan's `abi_return_type`), OR a real user returning
    `fixed<T>` from a function.
11. **N2: number-LITERAL argument to a direct suspending call stages zero** (Phase 3 Step-4b sweep
    finding, FRAGO 009 disposition (5)) — **WHAT:** `priceParam(3.5)` where `priceParam` is a direct
    SUSPENDING callee stages `0.000…` for the literal arg at BOTH tiers; annotated-binding args
    (`let p: number = 7.0; priceParam(p)`) work. PROVEN pre-existing: identical wrong output from the
    pre-fix `target/release/ynz` at O0 — orthogonal to R9 (not O0-reliant, no optimizer dependence).
    **WHY not absorbed here:** a literal-staging bug in the SM param path, not a return-ABI or
    optimizer-flip defect; same family as (but distinct from) the roadmap's int-literal→`number`
    coercion row (this one is a DECIMAL literal, already type-legal, staged wrong). **COST:** ~0.5
    session — route the literal through the same alloca-backed staging the annotated-binding path
    uses + a both-tier fixture. **TRIGGER:** the next milestone touching SM param staging or
    numeric-literal codegen (natural co-fix with roadmap ledger row 441's coercion work), OR a real
    user hitting a zeroed literal arg.
12. **N3: cross-module bare-`number` PARAMS mismatch (i128 declared vs ptr passed)** (Phase 3
    Step-4b sweep finding, FRAGO 009 disposition (5)) — **WHAT:** the importer's Pass 0.25
    declaration gives a bare `number` PARAM type `i128` while local callers pass `ptr` — pre-existing,
    and it fails LOUD at LLVM verify ("Call parameter type does not match function signature"), so no
    silent wrong value ships. The RETURN half of this exact twin-drift is now unified by this plan's
    ONE authoritative `abi_return_type` producer (Step 4b consumed by all three declaration sites);
    the PARAM half is the cheap, symmetric continuation — an `abi_param_type`-shaped single producer
    consumed by the same three sites. **WHY not absorbed here:** fails loud (no correctness exposure),
    pre-existing, and out of the R9 fix-round's blocker scope; it deserves the same
    authoritative-derivation treatment as the return half rather than a spot patch. **COST:** small —
    ~0.5 session riding the `abi_return_type` pattern already in place. **TRIGGER:** the next
    milestone touching cross-module ABI/declaration codegen, OR the first real cross-module
    bare-`number` param call site a user hits (it will fail loud, pointing here).
13. **Pre-existing test-target clippy debt** (FRAGO 013 disposition (5); supersedes the orphaned M6
    note at
    [`2026-07-04-v0-3-m6-concurrency-hotfix/plan.md:2012-2019`](../../done/2026-07-04-v0-3-m6-concurrency-hotfix/plan.md)
    — that note flagged the `--all-targets`-only findings "for the conductor to route to a backlog
    item" and no backlog item was ever created; THIS entry is that item, and the M6 plan is not
    edited) — **WHAT:** test-target lint debt across crates, outside every declared gate (CI runs
    clippy WITHOUT `--tests`/`--all-targets`): ~25 `ynz-typeck --tests` sites (stash-probe-proven
    pre-existing at clean HEAD), plus the `--all-targets` sightings in ynz-numerics/ynz-watch/ynz-fmt
    tests and the M6-noted ynz-parser (`clippy::len_zero` ×8) / ynz-typeck `independence.rs`
    (`unused_variables` ×5) findings. **WHY:** pre-existing, zero behavior impact, and fixing mid-M7
    widens scope for no correctness gain — the declared gate (`cargo clippy --workspace -- -D
    warnings`, no `--tests`) is clean throughout. **COST:** ~half a session, mechanical sweep.
    **TRIGGER:** the first CI change adding `--tests`/`--all-targets` to the clippy gate, or the
    next test-infra milestone.
14. **Name-keyed loop-var frame-slot collision on suspending-body `for` loops** (Phase 6 fix-round-3
    discovery, surfaced by the D5 admission decline and tracked per the Phase 6 review round;
    **ELEVATED priority** — heap-corruption severity class) — **WHAT:** the crossing-local frame
    slot for a `for` loop variable is keyed by NAME and classified once from the FIRST loop's
    element type (`find_for_loop_var_type_in_stmts`), so two suspending-body loops sharing a var
    name across DIFFERENT element types flush/reload the second loop's variable through a slot
    sized and classified for the first. Live-reproduced two ways (2026-07-17): SIGABRT "corrupted
    size vs. prev_size" — genuine heap corruption, not just wrong output — on the pre-existing
    `m5_p5_soa_copy_wait_bg.ynz` shape (three loops sharing `p` across Point/Part + a background
    spawn), and deterministic silent garbage on the minimized committed repro
    `crates/ynz-driver/tests/fixtures/v0_3_m7_d5_suspending_loop_var_slot_collision.ynz` (a Point
    loop then a string loop sharing `p` — the string reloads raw bytes through the
    Point-classified slot). **Current mitigation (a decline, NOT a fix):** Phase 6's D5 admission
    decline (`for_var_elem_type_conflict`, `crates/ynz-typeck/src/check.rs:8283`) only prevents
    Phase 6 from WIDENING the exposure to wait-free loops; the suspending-body exposure PRE-EXISTS
    Phase 6 and remains live. **LOCK:** committed `#[ignore]`d planned-RED tests
    (`crates/ynz-driver/tests/d5_frame_slot_collision_planned_red.rs`, test-ratchet-marked, both
    tiers, per the FR #9/fr23 planned-RED precedent) assert the correct contract; the repro fixture
    is excluded from both corpus sweeps until the fix, with the exclusions marked for removal in
    the fixing change. **WHY not fixed now:** out of Phase 6's charter — the fix is a frame-slot
    keying/classification redesign in the suspension machinery (per-loop or per-name+type slots),
    not a back-edge-yield concern; the decline keeps the newly-widened path safe in the interim.
    **COST:** ~0.5-1 session — key crossing loop-var slots per loop (or per name+type) instead of
    per name, re-run the m5_p5 shape, activate the planned-RED locks, remove the sweep exclusions,
    and retire whatever of D5's conservatism the fix subsumes. **TRIGGER:** the next milestone
    touching the SM frame flush/reload machinery or crossing-set classification, OR a real user
    hitting the garbage/SIGABRT class — the heap-corruption severity is what justifies pulling
    this forward ahead of ordinary backlog (hence ELEVATED).
15. **Fire-and-forget `background` completion lines are inherently load-flaky under byte-exact
    assertion — a recurring class needing ONE structural decision** (Phase 6 review round,
    deviation-judge disposition; tracking entry, NOT a proposal to build anything now) — **WHAT:**
    v0.3 has no join primitive, so a fire-and-forget `background` task's completion line lands at a
    scheduler-dependent stdout position BY DESIGN; any byte-exact assertion over such a line is
    load-flaky. FOUR independent surfaces of this one root cause are now on record: the two Phase 6
    preemption fixtures (`v0_3_m7_p6_backedge_starvation_sm.ynz` /
    `v0_3_m7_p6_backedge_residual_nonsm.ynz`, closed via determinism-sweep exclusion with the
    invariants owned by `v03_m7_backedge_preemption.rs`), the pirates-roster demo golden (closed
    via presence-relaxation of the `background analytics done` line in `integration.rs`), and the
    m4_p3 build-state-race watch item (flagged in the Phase 6 segment-2 audit entry, not yet
    closed). **WHY tracked:** each per-surface fix is legitimate under the established
    conventions, but four occurrences of one root cause warrant a structural design decision
    rather than an open-ended series of per-surface patches. Options at decision time: a real
    join/handle primitive (a language-design call deserving its own milestone and design doc), or
    a ratified corpus-wide testing convention ("fire-and-forget completion lines are
    presence-checked, never position-checked") applied mechanically across the corpus. **COST:**
    a design session to decide; the convention option is ~0.5 session to sweep the corpus; a join
    primitive is milestone-sized. **TRIGGER:** a fifth occurrence of the class, the next milestone
    touching `background`/task-lifecycle surface, or the m4_p3 watch item escalating to a real
    failure.

## Roadmap Reconciliation (executed at Phase 8; recorded here so the executor has zero ambiguity)

Per the roadmap's Capability Ledger — **the roadmap carries two duplicate tables with byte-identical
rows 438–443**: `## Capability Ledger (SSOT for capability → milestone ownership)` (roadmap.md line
~365) and `## Capability Ledger` (roadmap.md line ~417, merged 2026-07-01 from the pre-migration
`capability-ledger.md` companion file). **Phase 8 Step 2 updates BOTH tables in lockstep** — the
sibling v0.3-M6 plan independently commits to the same both-tables convention for its own rows, so
this is this roadmap's established practice, not new ceremony. The disposition below applies
identically to both tables (rows numbered by table order, unscoped rows only, 438–443):

| Row | Capability | This plan's disposition |
|---|---|---|
| 438 | Authoritative-derivation write-time guard | **NOT absorbed** — process/tooling, unrelated (Future Requirements #4) |
| **439** | **General hot-loop O0 stack-exhaustion ceiling fix** | **ABSORBED — Phase 4** |
| 440 | Build/release-tooling ABI-version-checked runtime archive | **NOT absorbed** — separate bug class, already mitigated (Future Requirements #2) |
| 441 | Codegen ICE: bare int literal into `number`-typed slot | **NOT absorbed** — unrelated correctness bug (Future Requirements #3) |
| 442 | Selective hot-field-only element materialization | **NOT absorbed** — different concern, reasoned non-absorption (Future Requirements #1) |
| **443** | **Add an LLVM optimization pass pipeline to `ynz build`** | **ABSORBED — this plan's core scope (Phases 0–3, 5–7)**, per Patrick's own note flagging it "the single most strategically important item on this list" given the Rust-level-performance positioning and flagship-concurrency framing |
