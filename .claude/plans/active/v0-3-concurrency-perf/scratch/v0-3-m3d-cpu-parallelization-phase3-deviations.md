# v0-3-m3d-cpu-parallelization Phase 3 Deviations — captured 2026-06-14 (Sub-slice 4b, live-locals + for-body-decline fix)

D_count: 2

> NOTE: Phase 3 built incrementally across sub-slices 4a–4e. This scratch reflects the CURRENT 4b state
> after the live-locals crossing-slot fix (Patrick Option-2) AND the for-body-decline fix round (R1 gate
> found for-body FIRE over-admits nested shapes → declined per the pre-authorized verify-first escape).
> The accumulator straight-line fix (the load-bearing Option-2 win) STANDS and is validated. Prior R1/R2
> deviations are SUPERSEDED. Sub-slice 4a committed deaa30c. BASE for 4b = deaa30c.

## Scope Deviations (verbatim from executor report)

None — stayed within declared scope. (All code changes in `crates/ynz-typeck/src/**` + `crates/ynz-codegen/src/**` + `crates/ynz-driver/tests/**` + `.claude/todos.md`, all in the plan front-matter `files:`. `.claude/state.md` shows a 1-line hook-generated radar delta — not touched by the executor.)

## Approach Deviations (verbatim from executor report)

**Deviation #1** (codegen reload path — `reload_spike_results: bool` param on the canonical `reload_params_from_frame`, now corrected): the live-locals fix added a `reload_spike_results: bool` param to gate the trailing spike-result reload (which fails for nested groups with no Step-1c sm_entry alloca). The R1 gate (judge #1) found it was left `true` at the orphan-block terminator → a pure nested-if group with NO surrounding crossing local aborted codegen. Fix-round corrected emit.rs:3877 to `false`. Rationale: `orphan blocks are dead/unreachable code (they exist only to satisfy LLVM's terminator requirement), so the spike-result reload there is a no-op — false is safe regardless of nested vs top-level placement. The post-join site stays false; the 5 wait/IO sites stay true (judge #1 confirmed those correct). The wait/I-O callers keep existing behavior.` Diff hunks: `crates/ynz-codegen/src/emit.rs:3514-3530` (helper signature), `crates/ynz-codegen/src/emit.rs:3877` (orphan-terminator site corrected to false).

**Deviation #2** (for-body DECLINE — the pre-authorized verify-first escape outcome): the live-locals fix re-enabled for-body FIRE, but the R1 gate (code-reviewer) found it over-admits: a CPU group in a for-body nested under an `if` silently miscompiled, and a group in the inner of two nested for-loops aborted codegen. Per the verify-first escape (for-body needs multi-level synthetic-index work beyond crossing-slot reservation), the fix-round DECLINES ALL for-body. Rationale: `spike_nested_blocks reverted to exclude For/While so a CPU group in ANY for/while body declines to sequential (byte-identical); the nested for-body placements need multi-level synthetic-index reservation deferred to a dedicated future loop-placement-matrix slice; the for-loop cpu_supported threading reverted as now-dead (no-duct-tape); judge #2 proved the SIMPLE top-level for-body case fires correctly, which de-risks the future slice. Two DECLINE regression fixtures (for-under-if, inner-nested-for) lock the corpse-prevention (no abort, no silent-wrong).` Diff hunks: `crates/ynz-codegen/src/emit.rs:7011` (`spike_nested_blocks` excludes For), `crates/ynz-typeck/src/check.rs:6588-6612` (for-loop cpu_supported threading reverted), `crates/ynz-driver/tests/integration.rs:5854-5950` (for-body test FIRE→DECLINE + 2 new DECLINE regressions + 1 new orphan-terminator FIRE fixture).

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: approach
- **rationale**: nested-group-in-host-with-suspension DECLINE (closes judge #1's abort). spike_cpu_candidates (emit.rs:6903) declines a nested CPU group when the host body contains any other wait/suspending op. Verify-first refined the shape: an explicit `wait` makes the host a non-promotion-candidate (declines harmlessly, never reaches the gate); the REAL abort needs a nested group + a PROMOTED suspending CALLEE (invisible at candidate-id → host promoted → Step-1c spike_cpu_group_result_names scans only top-level → nested bind names get no alloca → wait/IO resume reload aborts). Decline-around: nested group fires only in a pure-CPU host; mixed CPU+wait deferred to 4c (proper FIRE needs Step-1c nested-block walk). Top-level group + suspension still FIRES (no regression). The orphan-terminator bool fix (emit.rs:3877 false) from the prior round stays. RISK: a nested+suspension shape that still FIRES into the abort despite the decline.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:6903, crates/ynz-driver/tests/fixtures/v0_3_m3d_nested_group_with_suspending_callee.ynz:1-50, crates/ynz-driver/tests/integration.rs:5994-6051
- **judge identity hash**: fae99f4893e3d6e7f6cd1d33dda864da6e6d7bee
- **carry status**: re-judged at fix round (nested+suspension decline replaces the partial orphan-only fix)

### Deviation #2
- **type**: approach
- **rationale**: for-body DECLINE (verify-first escape) — spike_nested_blocks excludes For/While, any for/while-body CPU group declines to sequential; nested placements need multi-level synthetic-index work deferred to a future slice; for-loop cpu_supported threading reverted as dead; judge #2 validated the simple case works. RISK: an unenumerated for/while-body shape that still FIRES into the corpse (silent-wrong or abort) instead of declining.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:7011, crates/ynz-typeck/src/check.rs:6588-6612, crates/ynz-driver/tests/integration.rs:5854-5950
- **judge identity hash**: b66edde4de80160fa9a8a397b181c591424c52b8
- **carry status**: re-judged at fix round (for-body FIRE → DECLINE)
