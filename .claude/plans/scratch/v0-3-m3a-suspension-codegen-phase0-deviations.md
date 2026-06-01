# v0-3-m3a-suspension-codegen Phase 0 Deviations — ROUND 2 (overwrites round 1) — captured 2026-06-01T20:40

D_count: 10 (complete authoritative set across the cumulative Phase-0 diff)

> Coordinator note: the round-2 executor report re-listed only 5 deviations (dropping the round-1 PASSed ones + the FIX-1 tests/check.rs change). This file is the COMPLETE authoritative set so plan-adherence checks against the full cumulative diff. Status annotations: PASS-r1-unchanged = carry forward round-1 judge PASS (file byte-identical); re-judge = file changed in round 2; new = undocumented in round 1, judged first time in round 2.
>
> Judges spawned this round (6): D2, D3, D5, D6, D9, D10.
> Carried forward PASS (4, byte-unchanged since round-1 judge PASS): D1, D4, D7, D8.

## Resolved spawn list (round 2)

### Deviation D1 — runtime_decls.rs doc comment
- **type**: scope · **status**: PASS-r1-unchanged (carry forward; round-1 judge PASS)
- **diff hunks**: crates/ynz-codegen/src/runtime_decls.rs:173
- **round-1 identity hash**: 3092cd2ef9d5479a209d50c7261c507d870a7967

### Deviation D2 — m2_state_machine_integration.rs assertions + WHY comment
- **type**: scope · **status**: re-judge (changed FIX4 comment reword)
- **rationale**: assertions updated to match renamed intrinsics + reworded SubExprSuspendViolation/MutualSuspensionCycle diagnostic text; FIX4 reworded stale "pointing at M3" WHY comment at line 611.
- **diff hunks**: crates/ynz-driver/tests/m2_state_machine_integration.rs:601-629
- **identity hash**: d1b01541da7ec5360486bea5f18de88a8e7479d5

### Deviation D3 — tests/check.rs source-string rename + FIX1 assertion
- **type**: scope · **status**: re-judge (changed FIX1 test assertion)
- **rationale**: tests/check.rs bulk source-string rename plus FIX1 updated stale assertion at :3653 from contains(v0.3-M3) to contains(step-by-step)||contains(auto-parallelize).
- **diff hunks**: crates/ynz-typeck/tests/check.rs
- **identity hash**: 0233709cff5f0dee41ff4757e4e3686b30522f72

### Deviation D4 — runtime.rs doc comments
- **type**: scope · **status**: PASS-r1-unchanged (carry forward)
- **diff hunks**: crates/ynz-runtime/src/runtime.rs:289-301
- **round-1 identity hash**: 5e983b8dbc60233655eb54997453142ea615b0f2

### Deviation D5 — runtime test files (m2_runtime/m2_spike comments + spike.rs assert-msgs + fn rename)
- **type**: scope · **status**: re-judge (was BLOCK r1; changed FIX3 + reframed)
- **rationale**: m2_runtime.rs and m2_spike.rs comment-only renames; spike.rs assert-message-string renames (live code, behavior-preserving) plus FIX3 test fn rename sleep_ms_approximately_correct→sleep_blocking_approximately_correct. (Round-1 "comment-only" framing was wrong — corrected here per D5 judge BLOCK.)
- **diff hunks**: crates/ynz-runtime/tests/m2_runtime.rs, crates/ynz-runtime/tests/m2_spike.rs, crates/ynz-runtime/tests/spike.rs
- **identity hash**: 01cbf50d79558b1a73fac9a26f49714faaa47ac4

### Deviation D6 — completion.rs rename + sleepBlocking assert + fn rename
- **type**: scope · **status**: re-judge (was BLOCK r1; FIX2 added sleepBlocking assert + renamed fn)
- **rationale**: completion.rs rename sleepAsync→sleep assertion plus FIX2 added sleepBlocking completion assertion and renamed test fn to public_may_block_intrinsics_visible_internal_not_visible.
- **diff hunks**: crates/ynz-lsp/tests/completion.rs:748-785
- **identity hash**: d8ece3bea1a6f5e827813862e08ff3607ccbf9ee

### Deviation D7 — hover.rs source string
- **type**: scope · **status**: PASS-r1-unchanged (carry forward)
- **diff hunks**: crates/ynz-lsp/tests/hover.rs:378
- **round-1 identity hash**: fcfbb97cfd9ac82858d568e5189fe613047e3aea

### Deviation D8 — primantis-orders gallery rename
- **type**: scope · **status**: PASS-r1-unchanged (carry forward)
- **diff hunks**: examples/primantis-orders/v0_3_m2_errors.ynz
- **round-1 identity hash**: 71676ce7a3552a7a7547739d45166b527aaa49e6

### Deviation D9 — pirates-roster/entrypoint.ynz rename (NEW — undocumented in round 1)
- **type**: scope · **status**: new (judge first time; was the plan-adherence round-1 BLOCK)
- **rationale**: pirates-roster entrypoint.ynz Step-2 tree-wide rename; 4 call sites plus comments; file is Phase-4 scope.
- **diff hunks**: examples/pirates-roster/entrypoint.ynz
- **identity hash**: 3385f9ce3ce28d8f0e4dc2f239d12ac406436ae3

### Deviation D10 — new wait_on_sleep_blocking_still_warns test (NEW — undocumented in round 1)
- **type**: scope · **status**: new (judge first time; was part of the plan-adherence round-1 BLOCK)
- **rationale**: new test wait_on_sleep_blocking_still_warns (~30 lines) in integration.rs backing AC#3 (wait sleepBlocking still warns).
- **diff hunks**: crates/ynz-driver/tests/integration.rs:2004-2031
- **identity hash**: a374685ce5cb3b0276560e37dcd0ca8de3f87b11

### Deviation D11 — golden.rs Step-2 rename (NEW in round 3 — coordinator scratch-keeping miss in rounds 1-2)
- **type**: scope · **status**: new (judge in round 3; flagged by plan-adherence round-2 BLOCK #1). Coordinator note: golden.rs WAS in the executor's round-1 Files-Modified list; I omitted it from the deviation set in error. Documented now.
- **rationale**: crates/ynz-codegen/tests/golden.rs contains inline IR/codegen golden strings referencing sleepAsync/sleepMs; Step-2 tree-wide rename (sleepAsync→sleep, sleepMs→sleepBlocking), ~30 lines, pure mechanical, behavior-preserving. The plan's Phase-0 `Est. lines` explicitly anticipated it ("golden.rs ~15"); it just wasn't in the enumerated `Files (expected scope)` block.
- **diff hunks**: crates/ynz-codegen/tests/golden.rs
- **identity hash**: 44aec5ee17685f3094e4d2ab785a588cb3f00ce8

### Deviation D12 — cspell.json dictionary additions (NEW in round 3)
- **type**: scope · **status**: documented (flagged by plan-adherence round-3 soft-BLOCK; verifier prescribed adding this entry).
- **rationale**: `cspell.json` gained 9 legitimate programming/Pittsburgh-theme terms (`bitcast`, `desugars`, `miscompiled`, `primantis`, `reimplementation`, `stackless`, `subexpr`, `unbuildable`, `unvalidated`) flagged by cSpell on the plan file + rename diff. Companion to Step-2's tree-wide work; behavior-neutral (spell-check dictionary only, zero code/logic change). Done per Patrick's standing `cspell-words-proactive` instruction (add flagged legit terms immediately, don't skip). Not in the enumerated Phase-0 `Files (expected scope)` block, hence documented here.
- **diff hunks**: cspell.json
- **judge**: not spawned — pure dictionary maintenance, no logic surface to adversarially probe (same carve-out class as a comment-only change).

## In-scope content note (NOT a deviation — check.rs is in Phase-0 expected scope)

**LocalCrossesWait WHY citation M3→M3a** (`crates/ynz-typeck/src/check.rs:471`, cascading to `m2_state_machine_integration.rs:801` test): flagged by plan-adherence round-2 BLOCK #2 as unrequested content creep. **Coordinator ruling: documented as a DELIBERATE, behavior-preserving accuracy fix** (option b of the verifier's offered resolution). Rationale: LocalCrossesWait is a LIFTED guard that genuinely ships in v0.3-M3a (P1) — the "v0.3-M3a" citation is ACCURATE (unlike the kept-guards' false "ships in v0.3-M3" promise that P0 removed). The guard + its diagnostic are DELETED entirely in P1. Reverting to "v0.3-M3" would re-introduce a stale/inaccurate milestone citation for zero benefit and create throwaway churn. The m2_state_machine test's "v0.3-M3a" assertion arm was confirmed by the D2 judge to be coupled to the real diagnostic (it ALSO asserts the durable "frame-slot" mechanism string; not gameable). check.rs is in Phase-0 expected scope, so this is in-scope content, not a scope deviation requiring a judge. Named cost: a milestone citation lives in a test assertion until P1 lifts the guard (acceptable — P1 owns the cleanup when it deletes the guard).
