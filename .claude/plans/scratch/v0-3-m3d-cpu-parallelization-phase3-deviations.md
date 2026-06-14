# v0-3-m3d-cpu-parallelization Phase 3 Deviations — captured 2026-06-14 (Sub-slice 4a)

D_count: 2

> NOTE: Phase 3 is built INCREMENTALLY across live-verified slices (Patrick decision, 2026-06-13),
> one commit at the phase boundary. Slice 4 itself is decomposed into sub-slices 4a–4e (coordinator
> plan in the plan-file Phase-3 Findings Log, 2026-06-14). This scratch file reflects the CURRENT
> sub-slice's deviation set. Slices 1/2/3 (committed 0cc0ae0/d8bef4e/8e984fa) deviation records +
> their judge PASS verdicts live durably in the plan-file Findings Log + committed scratch snapshots.
> Cumulative 4.a re-judges the full Phase-3 deviation set from those durable records.

## Scope Deviations (verbatim from executor report)

None — stayed within declared scope. (All touched files are under `crates/ynz-codegen/src/`, `crates/ynz-driver/tests/`, plus `.claude/todos.md` which the envelope explicitly directed for the tracked carry-forward.)

## Approach Deviations (verbatim from executor report)

**Deviation #1** (4a-ii FIRE-vs-DECLINE — the envelope's own conditional branch, documented honestly): the envelope said `the existing v03_m3d_nested_groups inner work group fires (2→4) IFF verify-first proves nested-spawn-from-worker safe`. I did `DECLINED 4a-ii and kept the test at 2 spawns`. Rationale: `verify-first empirically established that work CANNOT fire under 4a regardless of concurrency safety — it takes a parameter and the zero-param host gate (line 6700, an explicit 4e concern that 4a does not lift) declines it; and the nested-spawn-from-worker hazard itself is architecturally unreachable in 4a because the candidate filter structurally bars any promoted host from being a parallel child. Firing it would require lifting the 4e param gate, outside 4a scope. Per the envelope this conditional DECLINE is not a narrowing — it is the explicitly-sanctioned honest outcome, with the residual tracked in todos and the nested fixture header verified still accurate.` Diff hunks: `crates/ynz-driver/tests/integration.rs:5395-5410` (new 4a-i test; the nested test at integration.rs:5191-5237 was deliberately left unchanged).

**Deviation #2** (carry-forward fold-in left as tracked residual, not fixed): the envelope said `fold in the entrypoint-self-recursion-post-pair benign over-allocation if cheaply in-scope`. I did `left it as a tracked carry-forward and durably re-documented it (now generalized to host-calls-host chains)`. Rationale: `verify-first showed exact probe/emit-set reconciliation requires a fixpoint (removing a host can re-admit a fn that declined only because that host was suspending, which can re-decline a third fn — non-monotone in one pass), and the naive full-promoted-union probe risks re-introducing the slice-1 union-poisoning corpse. The waste is benign (OVER-allocation only, output correct, alloc=free verified on a mid→other host-chain probe). The envelope explicitly permits leaving it tracked if not cheap/safe.` Diff hunks: `crates/ynz-codegen/src/emit.rs:6859-6871, crates/ynz-codegen/src/queries.rs:339-345, .claude/todos.md:194-195`.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: approach
- **rationale**: 4a-ii FIRE-vs-DECLINE conditional branch documented honestly (verify-first: `work` is param-gated by 4e zero-param gate + nested-spawn-from-worker architecturally unreachable via candidate-filter structural bar; envelope-sanctioned conditional DECLINE, residual tracked, nested fixture header verified accurate).
- **diff hunks**: crates/ynz-driver/tests/integration.rs:5395-5410
- **judge identity hash**: fa1a160c80ee403a65702cbf495d39c2724b7d7b
- **carry status**: fresh

### Deviation #2
- **type**: approach
- **rationale**: carry-forward (entrypoint-self-recursion over-alloc) left as tracked residual not fixed — exact probe/emit-set reconciliation needs a non-monotone fixpoint; naive full-promoted-union probe risks slice-1 union-poisoning corpse; waste is benign OVER-allocation only (output correct, alloc=free); envelope permits leaving tracked; durably re-documented generalized to host-calls-host chains.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:6859-6871, crates/ynz-codegen/src/queries.rs:339-345, .claude/todos.md:194-195
- **judge identity hash**: 2ea40721ef270751b4b06e83cd6461b7390b4301
- **carry status**: fresh
