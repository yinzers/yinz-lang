# v0-3-m3d-cpu-parallelization Phase 3 Sub-slice 4c Deviations — captured 2026-06-14

D_count: 2

> NOTE: Sub-slice 4c (mixed CPU+I/O) BASE = ab2e840 (4b's committed HEAD). The executor ran
> verify-first on BOTH declared FIRE targets, built the named fixes, proved each unsafe/unimplementable
> within 4c's poll-fusion scope, and REVERTED all emit.rs/typeck/runtime changes to BASE (probe-confirmed
> byte-identical). Net deliverable: 2 DECLINEs locked (1 new fixture+test for mixed CPU+I/O; the 2 existing
> 4b nested-in-suspending decline fixtures stay) + a todos:198 residual rewrite. Both deviations are
> FIRE→DECLINE envelope narrowings — the deviation-judges must verify they are LEGITIMATE (genuine
> unsafety) and not a cop-out to avoid work.

## Scope Deviations (verbatim from executor report)

None — stayed within declared scope. (emit.rs/typeck/runtime touched during the verify-first attempt and fully reverted; final diff touches only the declared fixture/test/todos files. `.claude/state.md` is a radar auto-refresh, not executor-touched.)

## Approach Deviations (verbatim from executor report)

**Deviation #1** (4c-i FIRE envelope): plan said `FIRE — a mixed CPU+I/O group overlaps both children (CPU → ynz_rt_spawn_blocking_joinable, I/O → resume_fn poll)`, executor did `kept it a DECLINE and locked the 0-spawn byte-identical shape with a new fixture+test`. Rationale: `the two lowering paths share no continuation — fusing them requires routing codegen off typeck's class-aware partition_groups_classified (which ships with zero codegen consumer by design), a fused poll loop re-driving every live spawn handle on each resume, AND dual-kind (CPU handle/result slots + I/O child sub-frame) frame-layout reservation for one group; this is the "frame work you can't safely land here" the VERIFY-FIRST mandate names. Verified: the shape declines to 0 spawns / 4958 byte-identical today, so locking the decline prevents a future fusion slice from silently regressing it (Silent-Envelope corpse)`. Diff hunks: `crates/ynz-driver/tests/fixtures/v0_3_m3d_mixed_cpu_io_group_declines.ynz:1-37, crates/ynz-driver/tests/integration.rs:6048-6068, .claude/todos.md:198`.

**Deviation #2** (4c-ii FIRE envelope): plan said `FIRE — make Step-1c walk nested blocks so a nested group's bind names get sm_entry allocas, then the nested group FIRES alongside an outer wait/suspending call`, executor did `built exactly that fix (spike_group_bind_names_all_depths + lifted the admission decline), verify-first-proved it produces a HANGING binary, reverted it, and kept the DECLINE`. Rationale: `pre-allocating the nested allocas removed the backend abort but did NOT fix the root hazard — the nested spawn-join yields Pending into a continuation the outer SM suspension's resume never re-drives, so the spawned CPU handle is never re-polled and the binary deadlocks (exit 1, no output, on ..._with_suspending_callee forced to 4 spawns). The explicit-wait host additionally can't promote at all: typeck excludes base_suspends functions from CPU candidacy (queries.rs:763). A safe fire needs the SAME poll-path-fusion machinery as 4c-i PLUS typeck promoting + guard-probing already-suspending hosts — out of this slice's poll-fusion scope. Shipping the deadlock would be a HALT-class regression`. Diff hunks: `.claude/todos.md:198`.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: approach
- **rationale**: (verbatim above) 4c-i mixed CPU+I/O FIRE→DECLINE — two poll paths share no continuation; fusion needs partition_groups_classified codegen consumer + fused poll loop re-driving live handles + dual-kind frame reservation. Locked DECLINE (0 spawns / 4958 byte-identical). RISK: a mixed CPU+I/O shape that COULD fire with less machinery than the executor claims (judge: try to find one), OR the locked DECLINE over-declines a safe shape.
- **diff hunks**: crates/ynz-driver/tests/fixtures/v0_3_m3d_mixed_cpu_io_group_declines.ynz, crates/ynz-driver/tests/integration.rs, .claude/todos.md
- **judge focus**: is mixed CPU+I/O GENUINELY unfusable within 4c's poll-fusion scope, or did the executor narrow to avoid the work? Probe whether a one-CPU + one-I/O group can overlap with cheaper machinery than claimed. Confirm the DECLINE fixture asserts 0 spawns + correct sequential value.

### Deviation #2
- **type**: approach
- **rationale**: (verbatim above) 4c-ii nested-group-in-suspending-host FIRE→DECLINE — the named Step-1c fix compiled but DEADLOCKED (nested spawn-join Pending into a continuation the outer suspension's resume never re-drives → handle never re-polled); typeck also excludes base_suspends hosts from CPU candidacy. Reverted; DECLINE kept. RISK: the deadlock is a FIXABLE bug in the executor's attempt (not fundamental) → the FIRE was abandoned too early; OR the executor's revert left the decline unsound.
- **diff hunks**: .claude/todos.md (residual rewrite; the 2 existing 4b decline fixtures v0_3_m3d_nested_group_with_outer_wait / ..._with_suspending_callee + their tests lock the verified-correct declines, unchanged since 4b)
- **judge focus**: is the deadlock REAL and FUNDAMENTAL to 4c's scope, or a fixable defect in the executor's reverted attempt? Reconstruct/reason about the continuation re-drive claim. Verify typeck queries.rs:763 genuinely blocks already-suspending-host promotion. Confirm the kept DECLINE still holds (0 spawns, no abort, byte-identical) on the existing fixtures.
