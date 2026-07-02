---
name: "v0-3-m3f-codegen-correctness-fixes-audit"
plan-id: "2026-06-09-v0-3-m3f-codegen-correctness-fixes"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-06-09-v0-3-m3f-codegen-correctness-fixes

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

### phase1-deviations.md

# v0-3-m3f-codegen-correctness-fixes Phase 1 Deviations — captured 2026-06-09

D_count: 0

## Scope Deviations (verbatim from executor report)

None — stayed within declared scope.

## Approach Deviations (verbatim from executor report)

None — implementation matched plan's named approaches.

## Resolved spawn list (orchestrator's parsed view)

No deviations — no judges spawned this phase.

### phase2-deviations.md

# v0-3-m3f-codegen-correctness-fixes Phase 2 Deviations — captured 2026-06-09 (round 2: after fix)

D_count: 3 documented (1 scope + 2 approach); judges spawned: 1 (only Approach #1 has an adversarial behavioral surface).

## Scope Deviations (verbatim from executor report)

**Scope Deviation #1** (`crates/ynz-driver/tests/integration.rs`): touched outside declared scope. Rationale: `cargo fmt --all` auto-formatted the Phase-1 assertion blocks (pure whitespace normalization, no semantic change). Diff hunks: `integration.rs:6510-6575`.

> COORDINATOR: NO judge — a `cargo fmt` whitespace change has no behavioral/adversarial surface (plan-adherence round-1 confirmed values unchanged). Documented now (was falsely "None" in the first executor run).

## Approach Deviations (verbatim from executor report)

**Deviation #1** (Step 3 — "leave the crossing path untouched"): executor reworked the crossing EC<Number> path to 3-slot storage (`{f0, i128_lo, i128_hi}` instead of `{f0, staging_ptr}`) AND kept it after the fix round. Rationale: p1 is a crossing local (live across p2's suspension); baseline stored the staging POINTER as f1, which same-callee reuse clobbers; 3-slot stores lo/hi directly → value-stable. Empirically: reverting the rework keeps `v03_m3b_p5_parallel_number_crosses_wait` GREEN (rework NOT needed for that test — concedes the original necessity claim), but executor claims it IS needed for AC1 (p1 crossing). FIX A (f0==0 guard) fixed the error-path regression separately. Diff hunks: `emit.rs:1474-1480, 2112-2130, 2197-2200, 2286-2421, 3214-3280, 3648-3710, 5443-5607, 5862-5880`.

**Deviation #2** (Step 2 — stack-alloca): executor used a per-binding i128 stack alloca placed at resume-fn entry. Rationale: "matches the plan's stated preference — stated for explicitness."

> COORDINATOR: NO judge — this is plan COMPLIANCE (the plan said "stack-alloca where lifetime permits"; executor did exactly that). No adversarial surface; nothing to adjudicate.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1 (judge spawned)
- **type**: approach
- **rationale**: (above) crossing-path 3-slot rework kept; executor claims necessity for AC1 (p1 crossing), concedes it's not needed for v03_m3b_p5.
- **diff hunks**: emit.rs:1474-1480, 2112-2130, 2197-2200, 2286-2421, 3214-3280, 3648-3710, 5443-5607, 5862-5880
- **judge identity hash**: 4c031892e9193ec1433adc928da02993d2dc2824

> COORDINATOR GROUND-TRUTH for the judge: code-reviewer's round-1 #4 called this rework OVERSHOOT (staging slot is frame-stable in the composed heap frame; `ec_crossing_local_propagated_number` passed at baseline; recommended revert). The executor now rebuts: p1 is a SAME-CALLEE crossing local and the baseline path stored a staging POINTER that p2's reuse clobbers — so single-binding-crossing being correct at baseline doesn't prove same-callee-crossing is. The judge must settle OVERSHOOT vs NECESSARY: does reverting the 3-slot rework (keeping only non-crossing copy-on-bind) make AC1's p1 read 31.75 (aliased → rework necessary) or 24.50 (frame-stable → rework overshoot)? That single fact decides it.

### phase3-deviations.md

# v0-3-m3f-codegen-correctness-fixes Phase 3 Deviations — captured 2026-06-09

D_count: 0 (judge-worthy). The executor documented 2 "scope deviations" but BOTH are coordinator/hook artifacts, NOT executor changes — confirmed by `git diff 9e7ee78 --stat -- crates/` showing the executor touched ONLY `crates/ynz-codegen/src/emit.rs` (+18/-1).

## Scope Deviations (verbatim from executor report)

- **Scope Deviation #1** (`.claude/plans/active/...md`): the change is the COORDINATOR's Phase-2 `Committed: 9e7ee78` tick + SessionStart radar rebuild — NOT the executor. Confirmed: executor's only crates/ change is emit.rs.
- **Scope Deviation #2** ([`.claude/state.md`](../../../state.md)): SessionStart radar rebuild (hook-driven). NOT the executor.

> COORDINATOR: neither is an executor scope deviation. The executor correctly identified them as coordinator/hook activity (honest flagging). No judge — there is no executor-authored deviation to adjudicate this phase.

## Approach Deviations (verbatim from executor report)

None — implementation matched plan's named approaches. The fix is in `bind_sm_result_and_flush` (frame-slot materialization), NOT `independence.rs` (grouping suppression), exactly as the plan mandated.

## Resolved spawn list (orchestrator's parsed view)

No judge-worthy deviations — no judges spawned this phase.

### phase4-deviations.md

# v0-3-m3f-codegen-correctness-fixes Phase 4 Deviations — captured 2026-06-09

D_count: 0 (judge-worthy). One approach deviation documented (cargo-fmt collateral) — no adversarial behavioral surface.

## Scope Deviations (verbatim from executor report)

None — stayed within declared scope.

## Approach Deviations (verbatim from executor report)

**Deviation #1** (Step 7 — `cargo fmt`): the initial `cargo fmt --all --check` FAILED because the new `assert_eq!` calls in the 10 test functions used the single-line form; ran `cargo fmt --all` to auto-correct (rustfmt expands multi-arg `assert_eq!` with long string literals to multi-line), then re-verified `--check` passes. Test logic + WHY comments unchanged. Diff hunks: `integration.rs:6587-6845`.

> COORDINATOR: NO judge — pure formatting normalization (rustfmt), no behavioral surface; test logic unchanged. Same class as the Phase-2 fmt collateral.

## Resolved spawn list (orchestrator's parsed view)

No judge-worthy deviations — no judges spawned this phase. (Cumulative judge for Phase-2 Deviation #1 is spawned in the Step 4.a cumulative sweep.)

