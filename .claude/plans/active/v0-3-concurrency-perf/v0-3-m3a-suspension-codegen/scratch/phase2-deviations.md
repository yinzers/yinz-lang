# v0-3-m3a-suspension-codegen Phase 2 Deviations — captured 2026-06-03 (ROUND 3, coordinator-authoritative)

D_count: 14 (5 scope + 9 approach)

Round-1 gate: D1/D3/D4/D5 PASS (carry-forward, unchanged identity). D2/D6 BLOCK + code-reviewer back-edge ICE BLOCK → round-2 fixes below.
Round-2 re-judge set: S2 (pirates demo, now wired), S5 (golden, new), A3 (back-edge crossing scan = CR-1 fix, new), A4 (has_top_level_let_before_suspension arm = D6 fix, new). Carry-forward PASS (unchanged hunks + already PASSed): S1, S3, S4, A1.

## Scope Deviations

### Deviation S1 (round-1 D1) — CARRY-FORWARD PASS
- **type**: scope
- **rationale**: the existing test wait_in_while_loop_is_an_error asserts the old guard fired for while; after narrowing the guard, this test would fail with the implementation — updating it to assert the new accepted behavior is required for suite to remain green
- **diff hunks**: crates/ynz-typeck/tests/check.rs:2823-2856
- **judge identity hash**: 691374f3748dff1b0488ac09c187d7a8534f3403
- **status**: PASS (round 1, judge a08013b078672e37c) — hunks unchanged round 2

### Deviation S2 (round-1 D2, REVISED round 2) — RE-JUDGE
- **type**: scope
- **rationale**: plan-invariants.md Demo & Error Gallery subsection requires extending pirates-roster with each new feature; plan explicitly states this obligation — round-2 wires the demo into entrypoint() so it actually runs
- **diff hunks**: examples/pirates-roster/entrypoint.ynz:234-242, examples/pirates-roster/entrypoint.ynz:667-702
- **judge identity hash**: ce43c7a18c1d608f11d6af7796933091655cfb09
- **status**: round-1 BLOCK (declared-but-never-called); round-2 fix wires call into entrypoint() — re-judge

### Deviation S3 (round-1 D3) — CARRY-FORWARD PASS
- **type**: scope
- **rationale**: plan-invariants.md Demo & Error Gallery subsection requires adding the new P2 error class (for-loop wait still rejected) to the milestone error gallery
- **diff hunks**: examples/primantis-orders/v0_3_m3a_errors.ynz:106-120
- **judge identity hash**: 63979920dc7f80417389675f3ac80bc2147142ec
- **status**: PASS (round 1, judge a69fbc1e09a20a4e7) — hunks unchanged round 2

### Deviation S4 (round-1 D4) — CARRY-FORWARD PASS
- **type**: scope
- **rationale**: the alloc-count assertion for fixture (d) belongs in this file which owns the run_with_alloc_counter harness; adding it to integration.rs would require duplicating the harness
- **diff hunks**: crates/ynz-driver/tests/m2_state_machine_integration.rs:885-906
- **judge identity hash**: 20098b6d8c947b211895d1ced4d69d341c733c53
- **status**: PASS (round 1, judge a21ad24030b99c311) — hunks unchanged round 2

### Deviation S5 (round 2, NEW) — RE-JUDGE
- **type**: scope
- **rationale**: the golden stdout file must be updated with the Phase-2 demo output added in BLOCK-3 fix; without this update the examples_basics_runs_end_to_end integration test fails
- **diff hunks**: examples/pirates-roster/expected_stdout.txt:1
- **judge identity hash**: 659c6f0f68ead4bcaa78ce0fbdc1dc5013d3f93c
- **status**: new round-2 — judge

## Approach Deviations

### Deviation A1 (round-1 D5) — CARRY-FORWARD PASS
- **type**: approach
- **rationale**: the old function had no external callers after the callsite was moved to the new function — keeping it would produce a dead_code warning that clippy -D warnings rejects; deletion is the cleaner option
- **diff hunks**: crates/ynz-typeck/src/check.rs:5042-5075
- **judge identity hash**: abea4c97b169a4ca75f525925ddc3cc1fbe6f596
- **status**: PASS (round 1, judge a72e30aa3da8d44d0) — hunks unchanged round 2

### Deviation A3 (round 2, NEW — CR-1 back-edge fix) — RE-JUDGE
- **type**: approach
- **rationale**: round-1 extended collect_crossings_in_stmts to treat suspending While as a suspension point but did NOT scan the While condition/body for references to outer-declared locals (the back-edge case); adding collect_ident_refs_in_stmt(stmt, declared, out) before the recursive call is the minimal correct fix using existing infrastructure so back-edge reads are detected
- **diff hunks**: crates/ynz-typeck/src/check.rs:6143-6183
- **judge identity hash**: 507d0fc1a59963c3e264201abef0f4ee7538200f
- **status**: fixes round-1 code-reviewer back-edge ICE BLOCK — judge (supersedes round-1 A2/D6 extension)

### Deviation A4 (round 2, NEW — D6 missing-arm fix) — RE-JUDGE
- **type**: approach
- **rationale**: has_top_level_let_before_suspension is one of three parallel suspension-point helpers that must agree; round-1 missed it, causing spurious redeclaration-after-wait errors on two sequential suspending while loops with distinct counters; added a Stmt::While arm parallel to the existing Stmt::If arm
- **diff hunks**: crates/ynz-typeck/src/check.rs:5369-5383
- **judge identity hash**: dc2fe2636394453a48586937f31da94b35ab4606
- **status**: fixes round-1 D6 BLOCK — judge

### Deviation A5 (round 3, NEW — A3-judge silent-miscompile fix) — RE-JUDGE
- **type**: approach
- **rationale**: outer_is_genuine_crossing_local scanned only post-wait statements for outer-binding reads (Cases 1-2). When the first top-level suspension is a `while`, back-edge reads of the outer local live INSIDE the while node (condition + body), not after it — so the post-wait scan returned false and the shadow check was skipped, compiling a dangerous inner shadow silently. Case 3 uses the existing stmt_refs_target_non_shadowed helper (already handles Stmt::While with correct cond+body and shadow-aware semantics) to detect back-edge outer-binding reads when the suspension IS the while node itself. This makes all four suspension-point helpers agree on the suspending-while-is-a-suspension-point invariant.
- **diff hunks**: crates/ynz-typeck/src/check.rs:5592-5617, crates/ynz-driver/tests/integration.rs:3022-3047, crates/ynz-driver/tests/fixtures/v0_3_m3a_p2_while_shadow_no_postloop_read_rejected.ynz:1-18
- **judge identity hash**: 9c4ee19cb6060ad09be51f76a7533a939ca1a993
- **status**: fixes round-2 A3 BLOCK (Tier-A silent miscompile, coordinator-confirmed repro). NOTE: executor deviated from judge-named `collect_ident_refs_in_stmt` → used `stmt_refs_target_non_shadowed` (zero-alloc bool, claimed identical coverage + correct shadow semantics) — helper-swap MUST be adversarially judged (the bug WAS a shadow-semantics gap). Round-3 re-judge: A5 only (covers A3+A5 combined back-edge/shadow correctness — A3 hunks unchanged, its R2 BLOCK is resolved by A5). Carry-forward PASS R2: S2/S5/A4; R1: S1/S3/S4/A1.

### Deviation A6 (round 4, NEW — param-twin ICE fix + systematic audit) — RE-JUDGE
- **type**: approach
- **rationale**: param_is_genuine_crossing_after_wait scanned only post-wait statements for outer-binding reads, missing the while-condition back-edge read of a parameter — the parameter twin of the round-3 A5 local-gate bug; a param shadowed in a suspending while read only via the condition back-edge suppressed the param-shadow diagnostic and ICEd codegen. Added a while-back-edge case (named Case 2 in this gate since params have no top-level-redeclaration case) mirroring outer_is_genuine_crossing_local Case 3, calling stmt_refs_target_non_shadowed on the while node. Systematic audit of all 6 post-suspension-reference gates confirms class closed (3 immune, 2 patched R3+R4, 1 correct-by-design).
- **diff hunks**: crates/ynz-typeck/src/check.rs:5692-5715, crates/ynz-driver/tests/fixtures/v0_3_m3a_p2_while_param_shadow_rejected.ynz:1-18
- **judge identity hash**: c161c29ebb4e9dfcbbc4c6a217b5582c6343b142
- **status**: fixes round-3 code-reviewer param-shadow-in-while ICE BLOCK (coordinator-confirmed repro). Round-4 re-judge: A6 only (carry-forward PASS: all rounds 1-3 deviations incl. A5 — hunks unchanged). Judge must stress the systematic-audit "immune" verdicts + hunt for a 6th variant.

### Deviation A7 (round 5 — SUPERSEDED by A8)
- **type**: approach
- **rationale**: round-5 removed `param_is_genuine_crossing_after_wait` from Check 3b Shape (a) and replaced it with a blanket reject of ANY nested param shadow in SM functions, to close the LLVM ICE class. This was the correct conservative fallback — but it was over-conservative: non-crossing param shadows (inner binding not read across any wait) compile correctly with the R6 entry-block-alloca + restore-all codegen fix. A7 is superseded by A8.
- **diff hunks**: crates/ynz-typeck/src/check.rs (blanket-reject guard; lines shifted by R6 — see A8 for current state)
- **status**: SUPERSEDED by A8 (R6 right-long-term fix, Patrick-directed)

### Deviation A8 (round 6 — R6 lexical-scope codegen + precise typeck) — NEW
- **type**: approach
- **rationale**: R6 implements the Patrick-directed right-long-term fix for variable shadowing in SM functions, replacing R5's over-conservative blanket reject with two co-located fixes:
  (1) CODEGEN (emit.rs): `alloca_in_entry` helper builds non-crossing `let` allocas in the function entry block (satisfying LLVM SSA dominance for all successor blocks), and all 6 scope-exit restore loops changed from `sm_crossing_names`-only to `locals_snapshot`-all (correct lexical scoping: outer bindings restored on scope exit, no clobber from inner shadows). Yinz allows variable shadowing (design/linting.md `shadowed-variables` lint); this makes it correct at the codegen level.
  (2) TYPECK (check.rs): `param_has_nested_let_shadow` blanket reject (A7) replaced by `param_shadow_has_genuine_crossing` which runs `collect_crossings_in_stmts` on nested block bodies with an empty param filter — only rejects when the inner binding itself is read after a `wait` inside its own scope (a genuine frame-slot collision). Non-crossing shadows now compile and produce correct output.
  (3) DESIGN DOC (design/concurrency.md): amended `ShadowsCrossingLocal` section to accurately reflect the param Shape-A rule — crossing shadows rejected, non-crossing shadows compile.
  Fixtures `v0_3_m3a_p2_param_shadow_noncrossing_rejected.ynz` and `v0_3_m3a_p1_param_shadow_crossing.ynz` flipped from compile-error to compile-and-run (correct behavior verified). Integration tests updated with `// test-ratchet:` annotations explaining the behavior change.
- **diff hunks**: `crates/ynz-codegen/src/emit.rs:1450-1483` (alloca_in_entry helper), `crates/ynz-codegen/src/emit.rs:5163-5175` (non-crossing let uses alloca_in_entry), `crates/ynz-codegen/src/emit.rs:5204-5258` (lower_stmt scope arms restore-all), `crates/ynz-codegen/src/emit.rs:3462-3496,3628-3640` (lower_sm_block scope arms restore-all), `crates/ynz-typeck/src/check.rs:5636-5689` (param_shadow_has_genuine_crossing new helper), `crates/ynz-typeck/src/check.rs:759-826` (Check 3b Shape a predicate + diagnostic text), `design/concurrency.md:293-301` (Shape A clarification), `crates/ynz-driver/tests/fixtures/v0_3_m3a_p1_param_shadow_crossing.ynz:1-8` (WHY comment update), `crates/ynz-driver/tests/fixtures/v0_3_m3a_p2_param_shadow_noncrossing_rejected.ynz:1-30` (fixture rewritten for non-crossing semantics), `crates/ynz-driver/tests/integration.rs:2714-2744` (P1 test flipped), `crates/ynz-driver/tests/integration.rs:3135-3162` (P2 noncrossing test flipped)
- **judge identity hash**: 65516f5e08e76f066fff2bdb5123912989f7a5c1
- **status**: new round-6 — judge required. Coordinator pre-verified: non-coroutine shadow→5/9; deeply-nested non-crossing→100/7; crossing-via-back-edge→clean reject (accurate frame-slot diagnostic, no ICE/no infinite-loop); full suite 110 ok / exit 0 / 0 failed.

D_count: 13 (5 scope + 8 approach). Round-6 re-judge set: A8 only (carry-forward PASS: all rounds 1-5 deviations — confirmed at round 4/5 gate; A7 superseded by A8).

## No-progress reference (BLOCK hashes for deviations now being re-judged)
- S2 (pirates demo): round-1 BLOCK; round-2 hash ce43c7a1… differs (hunks changed) — content moved, expect resolution
- A3 / A4: net-new round-2 deviations addressing the round-1 code-reviewer + D6 BLOCKs
- A5: net-new round-3 deviation addressing the round-2 A3 BLOCK (silent miscompile)
- A6: net-new round-4 deviation addressing the round-3 code-reviewer param-twin ICE BLOCK + systematic class-closure audit
- A7 (round 5, SUPERSEDED): blanket param-shadow reject — over-conservative, replaced by A8
- A8 (round 6, SUPERSEDED by A9): lexical-scope codegen + precise typeck — over-reached; caused silent miscompile
- A9 (round 7, current): reverted R6 Part B to blunt param_has_nested_let_shadow (Option A, Patrick-decided)

### Deviation A9 (round 7 — Option A revert: blunt param guard + design-doc revert) — NEW
- **type**: approach
- **rationale**: R6's precise `param_shadow_has_genuine_crossing` predicate was Patrick-decided UNSAFE after the code-reviewer reproduced a silent miscompile (`f(7)` with a param shadow inside an if body containing a `wait sleep` → compiled, printed garbage instead of `7`). Option A (the safe answer): revert the param gate to `param_has_nested_let_shadow` — any nested `let pname` in a suspending function is a clean compile error, regardless of crossing position. The R6 Part A codegen fix (alloca_in_entry + restore-all) is KEPT — it's correct for NON-async functions and harmless for async non-shadow cases (the typeck reject gates out async shadows before codegen). Design/concurrency.md §ShadowsCrossingLocal param section reverted: removed the R6 "non-crossing compiles" bullet, restored the blunt Shape-A description, added explicit "Non-async functions" paragraph. Two fixtures flipped back to reject (exit 1): `v0_3_m3a_p2_param_shadow_noncrossing_rejected.ynz` (non-crossing shadow in SM function) and `v0_3_m3a_p1_param_shadow_crossing.ynz` (shadow after top-level wait). Four new regression fixtures added to lock the Option-A boundary: `r7_silent_miscompile_rejected` (the code-reviewer repro), `r7_local_shadow_crossing_rejected` (judge A8's local case — consistent with blunt param guard), `r7_nonasync_param_shadow_compiles` (non-async → compile), `r7_nonasync_local_shadow_compiles` (non-async → compile). The roadmap M3c deferral (`v0-3-m3c-shadow-parity`) and `registry/features.toml:1135` cross-ref are unchanged. The ShadowsCrossingLocal pattern is now documented with the correct non-async exception.
- **diff hunks**:
  `crates/ynz-typeck/src/check.rs:754-797` (Check 3b comment + param_has_nested_let_shadow call + diagnostic text),
  `crates/ynz-typeck/src/check.rs:5640-5678` (param_has_nested_let_shadow new helper, replaces param_shadow_has_genuine_crossing),
  `design/concurrency.md:293-300` (param Shape-A + non-async paragraph),
  `crates/ynz-driver/tests/fixtures/v0_3_m3a_p2_param_shadow_noncrossing_rejected.ynz:1-26` (reverted to reject semantics),
  `crates/ynz-driver/tests/fixtures/v0_3_m3a_p1_param_shadow_crossing.ynz:1-26` (reverted to reject semantics),
  `crates/ynz-driver/tests/integration.rs:2724-2744` (P1 param test → exit 1),
  `crates/ynz-driver/tests/integration.rs:3151-3180` (P2 noncrossing → exit 1),
  `crates/ynz-driver/tests/integration.rs:3181-3290` (4 new R7 regression tests),
  `crates/ynz-driver/tests/fixtures/v0_3_m3a_p2_r7_silent_miscompile_rejected.ynz:1-22`,
  `crates/ynz-driver/tests/fixtures/v0_3_m3a_p2_r7_local_shadow_crossing_rejected.ynz:1-22`,
  `crates/ynz-driver/tests/fixtures/v0_3_m3a_p2_r7_nonasync_param_shadow_compiles.ynz:1-18`,
  `crates/ynz-driver/tests/fixtures/v0_3_m3a_p2_r7_nonasync_local_shadow_compiles.ynz:1-18`
- **status**: new round-7 — judge required (A9). Verified: silent-miscompile case 3×=exit 1 (no garbage); probe-6=exit 1; A8 local case=exit 1; non-async param/local shadows=exit 0; 5 Phase-2 ACs green; decimal128 back-edge=0.3 exact; two-sequential-while=3; cargo test --workspace 0 failures; clippy clean; fmt clean; jargon 9/0.
