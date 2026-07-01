---
name: "v0-3-m2-wait-and-state-machines-audit"
plan-id: "2026-05-30-v0-3-m2-wait-and-state-machines"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-05-30-v0-3-m2-wait-and-state-machines

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

### phase6-deviations.md

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

### phase7-deviations.md

# v0-3-m2-wait-and-state-machines Phase 7 Deviations — captured 2026-06-01 (round 3, post-fix)

D_count: 1

## Scope Deviations (verbatim from executor reports across rounds)

Round 2 (codegen): Scope Dev — `crates/ynz-typeck/src/queries.rs` + `tests/check.rs` (add `suspends_set: HashSet<String>` to `CheckOutput` to thread the transitive `suspends` set from typeck → codegen `emit_artifact` — the P6/P7 seam plumbing); `crates/ynz-runtime/tests/m2_runtime.rs` (ynz_rt_spawn gained a 4th arg `recursion_slot_offset`; `SyncValueSm`/`SignalSm` updated to P7 frame layout + `#[repr(C)]` null@8 to prevent Drop chasing garbage); 12 golden IR snapshots (ynz_alloc→ynz_alloc_zeroed + ynz_rt_spawn 3→4 args). Round 3: test file + recursive_cancel fixture only.
Coordinator note: all the above are necessary seam/ABI follow-ons, NOT scope creep. The 5 `integration.rs` env-path snapshots were NOT touched (round-2 poisoning reverted to base by coordinator). The compiled-binary blobs that `ynz run` dropped into fixtures/ were removed + a durable `.gitignore` pattern added.

## Approach Deviations (verbatim from executor report)

- **Deviation #1** (Round 3, Fix 1 root-cause): coordinator's diagnosis said the cancellation-test flakiness was "timing-raced"; executor identified TWO specific mechanisms instead: (a) `shutdown_timeout(0ms)` thread-join race — with 0ms, `Runtime::shutdown_timeout` returns before worker threads are joined, so `ynz_free` calls inside `SpawnStateFnFuture::Drop` (on worker threads) aren't visible to the main-thread counter read in `ynz_rt_shutdown`; fix = 50ms (joins threads, still cancels before countdown's natural 600ms completion); (b) entrypoint-wrapper-frame counting — the positive-control threshold was `>= 3` but the entrypoint wrapper always allocs its own frame so actual alloc at cancel is 4; fix = threshold `>= 4`. Plus a `CANCEL_TEST_LOCK` static mutex serializing the 3 `recursion_cancellation*` tests. Diff hunks: crates/ynz-driver/tests/m2_state_machine_integration.rs:417-424, crates/ynz-driver/tests/m2_state_machine_integration.rs:452-510.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: approach
- **rationale**: cancellation-test determinism — fixed via shutdown_timeout 0→50ms (thread-join-race fix, the real correctness bug), positive-control threshold 3→4 (entrypoint frame), + serialization mutex. Coordinator independently verified 10/10 deterministic.
- **diff hunks**: crates/ynz-driver/tests/m2_state_machine_integration.rs:417-424, crates/ynz-driver/tests/m2_state_machine_integration.rs:452-510

### phase8-deviations.md

# v0-3-m2-wait-and-state-machines Phase 8 Deviations — captured 2026-06-01

D_count: 1

## Scope Deviations (verbatim from executor report)

Scope Devs #1-#5 (all necessary follow-ons of the rename, NOT creep): `m2_runtime.rs` + `m2_spike.rs` (import/call old symbol name → must update to compile); 13 golden IR snapshots (literal `declare i32 @ynz_rt_call_state_machine_sync` in IR text → symbol-name substitution only, no semantic change, avoids `.snap.new`); 2 `.ll` fixtures (hand-written IR referencing the old symbol — NOTE: `.ll` is gitignored so these edits are untracked/moot); `.claude/state.md` (Phase 8 Step 2 explicitly directs recording the guard re-decision there). Coordinator: all necessary for the rename to compile + the guard-decision deliverable. None are scope creep.

## Approach Deviations (verbatim from executor report)

- **Deviation #1** (Step 1: bridge deletion → rename): plan said "delete `ynz_rt_call_state_machine_sync`" (Option A preferred); executor chose Option B (rename → `ynz_rt_run_entrypoint`). Rationale: "LLVM IR can only call C-ABI symbols by name; the codegen-emitted main wrapper is a C function that must call into Rust via a named extern symbol. There is no mechanism to 'delete' a C-ABI symbol and replace it with inlined Rust — the boundary is structural. Option A and Option B converge to the same outcome: the old 'bridge' symbol is gone, replaced by a correctly-named program-entry driver. The plan explicitly authorized Option B with honest rename+rationale when full deletion is genuinely infeasible." Diff hunks: crates/ynz-runtime/src/runtime.rs:657-742, crates/ynz-codegen/src/runtime_decls.rs:194-199, crates/ynz-codegen/src/emit.rs:1747-1760.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: approach
- **rationale**: bridge "delete" (Option A) → "rename to ynz_rt_run_entrypoint" (Option B) because the codegen-emitted C-ABI main wrapper must call a named extern; full deletion is architecturally impossible. Plan explicitly authorized Option B with honest rename + tradeoff. Outcome identical: zero "bridge"/"Shape B" framing; the symbol is the legitimate program-entry driver, never reachable from a resume fn (objdump-proven, no_bridge test passes).
- **diff hunks**: crates/ynz-runtime/src/runtime.rs:657-742, crates/ynz-codegen/src/runtime_decls.rs:194-199, crates/ynz-codegen/src/emit.rs:1747-1760

### phase9-deviations.md

# v0-3-m2-wait-and-state-machines Phase 9 Deviations — captured 2026-06-01

D_count: 3

## Round-by-round trail (Phase 9 took 3 rounds — coordinator caught a HALT-class hole the executor worked around)

- **ROUND 1 (BLOCK):** executor restored demo/gallery/release-prep BUT its Approach Deviation #2 admitted — and SILENTLY WORKED AROUND in the demo — a residual codegen crash: a local crossing an INFERRED suspension point fails LLVM codegen ("Instruction does not dominate all uses"). That's the Phase-5-omission anti-pattern. Coordinator BLOCKed.
- **ROUND 2 (STILL BLOCK):** executor extended `locals_crossing_wait` to detect inferred-suspension suspend POINTS — fixed the minimal repro (`let slot = 7; ...`) but coordinator re-verified the ORIGINAL crashing input (`let slot = sleeper(); let other = sleeper(); return slot + other`) and found it STILL CRASHED — a suspending-call-RESULT local crossing a later suspension. Root-caused to check.rs:4603-4607 (result-binding never added to `declared`).
- **ROUND 3 (FIXED):** executor added `pending_result_bindings` deferred-flush so a result-binding is tracked as a crossing candidate for the NEXT suspension (not its own producing step). Coordinator verified all boundaries + a 10-shape adversarial sweep: zero crashes, no over-fire. HALT-class hole closed across every shape.

## Scope Deviations (verbatim from executor reports)

Round 1: `crates/ynz-driver/tests/integration.rs` + `examples/pirates-roster/expected_stdout.txt` (demo revert broke the byte-exact golden match — necessary follow-on). Round 3: none beyond the guard fix + its fixture/test.

## Approach Deviations (verbatim)

- **Deviation #1** (Round 1, demo restructure — INITIALLY a worked-around-crash, CORRECTED): the executor's "use slot before second suspension" demo rearrangement masked the codegen crash instead of surfacing it. Coordinator caught it, BLOCKed, and the underlying guard was fixed (rounds 2-3) so the demo no longer needs the workaround — it's naturally step-by-step. NOT an accepted deviation — the fix is the guard, not the rearrangement.
- **Deviation #2** (Round 3, fix mechanism): coordinator spec said "add `name` to `declared` after past_wait=true"; executor used a `pending_result_bindings` deferred-flush instead. Rationale: the coordinator's immediate-add would OVER-FIRE on `let a = sleeper(); return a` (single suspension, result used immediately — no LATER suspension, so safe); the deferred-flush tracks the binding only when a SUBSEQUENT suspension is encountered, which is the correct semantics (a result-binding is safe across its OWN producing step, a crossing candidate only for a LATER one). Coordinator verified boundary 3 (`let a = sleeper(); return a` → works) — the executor's model is strictly more correct than the coordinator's spec. Diff hunks: crates/ynz-typeck/src/check.rs:4570-4658.
- **Deviation #3** (Round 1, AC-timing reinterpretation): executor noted the demo total wall-clock is ~0.68s (M1 sleepMs(200) + sequential added sections) and applied the "<300ms" demo AC to the 8-pirate CONCURRENT block, not the total. Coordinator verified the concurrency property genuinely holds (all 8 STARTs before any DONE; ~150ms concurrent window not 800ms sequential) — so the reinterpretation is sound (the concurrency PROOF is the load-bearing AC, not total wall-clock).

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1 (demo workaround — corrected, not accepted)
- **type**: approach
- **rationale**: demo-rearrangement-to-hide-codegen-crash → CAUGHT + BLOCKED + fixed at the guard (rounds 2-3); not an accepted deviation.
- **diff hunks**: examples/pirates-roster/entrypoint.ynz (restored to natural step-by-step after the guard fix)

### Deviation #2 (deferred-flush guard mechanism)
- **type**: approach
- **rationale**: pending_result_bindings deferred-flush instead of immediate-add-to-declared — strictly more correct (no over-fire on result-used-immediately). Coordinator-verified across all boundaries + 10-shape sweep.
- **diff hunks**: crates/ynz-typeck/src/check.rs:4570-4658

### Deviation #3 (AC-timing scope)
- **type**: approach
- **rationale**: "<300ms" AC applies to the 8-pirate concurrent block, not the total demo wall-clock; concurrency property verified to hold.
- **diff hunks**: .claude/plans/active/v0-3-m2-wait-and-state-machines.md (AC evidence)

