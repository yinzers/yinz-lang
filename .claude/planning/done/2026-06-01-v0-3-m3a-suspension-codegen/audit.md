---
name: "v0-3-m3a-suspension-codegen-audit"
plan-id: "2026-06-01-v0-3-m3a-suspension-codegen"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-06-01-v0-3-m3a-suspension-codegen

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

### phase1-deviations.md

# v0-3-m3a-suspension-codegen Phase 1 Deviations — captured 2026-06-01 (post Round-2 fix + cleanup)

D_count: 6 substantive (judged) + 8 mechanical/documented (no fresh judge).

> Context: Phase 1 (frame-backed mutable locals) went Round-1 (7 codegen bugs found, incl. a masked Tier-A shape miscompile) → Round-2 full fix (Patrick-approved) → cleanup (float fixture + rename residuals). The working tree was corrupted mid-Round-1 by two reviewer agents running destructive `git checkout`/`stash`; recovered from /tmp/p1-backup.patch. This scratch file is the authoritative deviation set for the re-gate. All 6 substantive fixes coordinator-verified on a clean rebuild (shape→30, while→30, contdef→3, shadow→99/10, ErrorsCapable→114, float-via-.toFloat()→8).

## Substantive deviations (NEW codegen — adversarial judges spawned in re-gate)

### P1-D1 — Shape heap-promotion (Bug 1 fix; the headline)
- **rationale**: crossing shape literals allocated via ynz_alloc instead of stack build_alloca; heap ptr in frame slot; freed by frame drop-guard so it round-trips across suspension. (Pre-fix: stack alloca → dangling ptr → garbage 4240011.)
- **diff hunks**: crates/ynz-codegen/src/emit.rs
- **identity hash**: c970fec58a7b541b1e14078e46e2d38a99fc47f6

### P1-D2 — Scope-depth shadowing fix (Bug 6; supersedes Round-1 Approach#2 shadowing BLOCK)
- **rationale**: sm_scope_depth field tracks nesting; nested-construct arms save/restore cg.locals snapshot; collect_crossing_writes nested_scope flag prevents an inner `let` from being treated as a write to an outer crossing local.
- **diff hunks**: crates/ynz-codegen/src/emit.rs
- **identity hash**: 5d7aa67ea4de59925d48add8c455d136a3d08ed1

### P1-D3 — ErrorsCapable 2-slot (Bug 5)
- **rationale**: ErrorsCapable {i64,i64} classified like decimal128 into sm_crossing_errors_capable_set; two consecutive frame slots; companion struct alloca in sm_entry; flush/reload both words (NOT a dangling stack pointer).
- **diff hunks**: crates/ynz-codegen/src/emit.rs
- **identity hash**: c9154327b971f41f4e82267556d6dfaed0f8f6c6

### P1-D4 — Write-recursion extension (Bug 2)
- **rationale**: collect_crossing_writes recurses into While/For/Match bodies + handles FieldAssign/IndexAssign so mutations inside those constructs flush to the frame slot (pre-fix: only Stmt::If recursed → silent stale reads).
- **diff hunks**: crates/ynz-codegen/src/emit.rs
- **identity hash**: 7fe716cf194b6a80487b04429f428357b65ae733

### P1-D5 — Contdef fix at ANALYSIS level (Bug 3) — divergence from the prescribed codegen-level fix
- **rationale**: fixed in check.rs (collect_crossings_in_stmts adds between-suspension let-declared names to the declared set in the past_wait else-branch) rather than codegen. Executor's claim: codegen already creates sm_entry-dominating allocas for names in crossing_names; the real bug was the analysis MISSING between-suspension locals. (Pre-fix: LLVM "does not dominate all uses" crash.)
- **diff hunks**: crates/ynz-typeck/src/check.rs
- **identity hash**: 8c2e31fbdca10cfa0fe911657c2984cbce8d3cd2

### P1-D6 — Float bitcast (Bug 4) — executor's "float untestable" claim was FALSE
- **rationale**: sm_crossing_float_set; flush loads f64 then bitcasts f64→i64; reload bitcasts i64→f64 (not a raw i64 load from an f64 alloca). Executor initially shipped no fixture claiming float untestable; coordinator proved float crossing IS testable via .toFloat() (→7/→8); cleanup pass added the fixture.
- **diff hunks**: crates/ynz-codegen/src/emit.rs
- **identity hash**: 310768d0d679279b47e25ec201c9e6c16ee0b79b

## Mechanical / documented deviations (no fresh judge — P0-class or already-validated)

- **P1-D7** — 5 fields on Cg struct (sm_crossing_names/slot indices/scalar/decimal128/float/errors_capable sets). Round-1 Approach#4 judge PASSED (verified fresh `cg` per function, no cross-function state leak). emit.rs.
- **P1-D8** — single-source `crossing_local_names` export from ynz-typeck (Round-1 Approach#2). One source of truth; the shadowing concern Approach#2 raised is now fixed by P1-D2. check.rs + lib.rs + emit.rs.
- **P1-D9** — golden.rs: sleepAsync/sleepMs rename in inline IR golden strings (tangle re-fix; P0-class, D11-equivalent). crates/ynz-codegen/tests/golden.rs.
- **P1-D10** — v0_3_m2_*/v0_3_m1_* fixtures: sleepAsync/sleepMs rename in live Yinz code (tangle re-fix; P0-class). crates/ynz-driver/tests/fixtures/.
- **P1-D11** — tests/check.rs: 5 guard tests rewritten error→accept (legit ratchet for lifted guard) + sleep rename + stale `.contains("sleepMs")`→`.contains("sleepBlocking")` assertion. crates/ynz-typeck/tests/check.rs.
- **P1-D12** — integration.rs: new P1 fixture tests + rename-residual assert-message cleanup. crates/ynz-driver/tests/integration.rs.
- **P1-D13** — m2_state_machine_integration.rs: alloc-count + non-crossing + result_binding tests updated for lifted guard. crates/ynz-driver/tests/m2_state_machine_integration.rs.
- **P1-D14** — lib.rs: re-export crossing_local_names + LocalCrossesWait for codegen consumption (required by P1-D8 single-source). crates/ynz-typeck/src/lib.rs.

## ROUND 3 ADDENDUM (design-correct redesign per Patrick "ownership model" directive)

Round-3 re-gate found 4 BLOCKs (shape leak+re-promote+nested-shallow-copy, EC StructValue-arm crash, wait-bearing-if shadow, doc-comment regressions). Round-3 fix (coordinator-verified: shape→30/array→3/EC→107/shadow→99-10/field-mutate→119 all alloc=1/free=1; cancellation 4/4; workspace green):

- **P1-D1 (SUPERSEDED → frame-embed)**: shape heap-promotion REPLACED by inline frame storage (N=ceil(size/8) consecutive frame slots, like decimal128's 2-slot). The crossing shape's storage IS the frame region; ptr alloca is a stable indirection into it. Eliminates the separate `ynz_alloc` → fixes the leak (freed with frame) AND re-promote-on-fieldassign (in-place mutation). This is the ownership-model-correct design (value owned by task, lives in frame, freed at scope end). Files: emit.rs (frame-embed), check.rs (slot accounting via shape_table).
- **P1-D15 (NEW) — nested/owned-heap-field shape guard**: a crossing shape with a shape-typed field (or other owned-heap field whose inner storage can't survive a shallow embed) gets a CLEAN COMPILE ERROR at typeck (WHAT/WHAT-INSTEAD/WHY) + documented deferral — NOT a silent miscompile. Recursive aggregate frame-embedding is a v0.3+/M3b follow-up. Boundary = ownership.md r4 transitively-trivial. Files: check.rs (guard). MUST document in docs/internal/implementation/IMP-concurrency.md + check registry [[deferred_language_feature]].
- **P1-D3 (EC) extended**: added the `StructValue` arm to `bind_sm_result_and_flush` (EC-callee result that is itself a crossing local crossing a 2nd wait) — stores both i64 words to the EC 2-slot scheme. Fixes the crash.
- **P1-D2 (shadow) extended**: scope-depth/snapshot added to `lower_sm_stmt_with_wait`'s If arm (the SM codegen path, where the guard was missing). Fixes wait-bearing-if clobber.
- **doc-comment regressions fixed**: state_machine.rs:61/:553, runtime_decls.rs:173 restored to sleep/sleepBlocking + comprehensive sweep + `is_sleep_async_call`→`is_sleep_call` internal rename.
- **FIX 5 (recursion parity)**: `find_let_typeck_type_in_stmts` now recurses into While/For/Match (defensive; the case isn't reachable in P1 since waits-in-loops are still guarded, but prevents a latent decimal128/EC truncation in P2/P3).

## ROUND 5 ADDENDUM (round-4 gate found 3 frame-embed edge bugs + shadow exotic + code-quality + missing docs; all fixed)

Round-4 re-gate verdicts: acceptance PASS (all 8 ACs), design PASS (frame-embed = ownership-correct), P1-D1 shape PASS (string/array/large/multi/decimal128-field all sound, nested-guard correct), P1-D3 EC PASS. BLOCKs: code-reviewer (3 edge bugs), P1-D2 shadow (nested-wait shadow), rules (code-quality), plan-adherence (deferral docs missing). Round-5 fix (executor died mid-FIX4 on auth-expiry; resumed; coordinator INDEPENDENTLY VERIFIED including compiled-binary runs + shape-size battery 1/3/6-field + mixed-alignment):

- **FIX 1 — single-bool-field shape SIGSEGV**: root cause was the HAND-ROLLED `shape_byte_size_from_def` (no-duct-tape #7 parallel impl) diverging from LLVM layout on the degenerate single-i1 case. Removed it; size now single-sourced from LLVM `TargetData::get_abi_size()`. (Deeper find during resume: `IntValue::get_zero_extended_constant()` returns None on LLVM's sizeof ConstantExpr → an earlier attempt gave EVERY shape 1 slot → heap corruption; the TargetData fix is correct. Verified: 1/3/6-field + mixed-align shapes all round-trip, alloc=1/free=1, compiled binary clean ×5.)
- **FIX 2 — if-arm SOLE-defined crossing local LLVM-ICE**: distinguished "crossing local's own nested definition" (reuse sm_entry alloca even at depth>0) from "shadow" (an enclosing same-named binding exists). → `42`.
- **FIX 3 — crossing-name SHADOW → CLEAN COMPILE ERROR** (permanent constraint, Golden Rule 2: one name = one value across a `wait`). `find_shadow_in_stmts` + diagnostic. Replaced the round-2/3 shadow-support machinery. → clean error.
- **FIX 4 — shape RESULT of suspending call crossing 2nd wait → munmap double-free/heap-corruption**: `bind_sm_result_and_flush` now memcpy's the shape's bytes into r's frame-embed slot region (mirrors EC StructValue arm + definition-site memcpy), no pointer-aliasing the freed sub-frame. → `10`, alloc=1/free=1, compiled-binary clean ×5.
- **FIX 5 — code-quality**: dup reload_params_from_frame doc-comment removed; `heap_promote`→`shape_embed` rename; clippy fixes (collapsible_match, too_many_arguments — removed unused shape_table param from compute_frame_size/build_frame_layouts).
- **FIX 6 — deferral docs**: docs/internal/implementation/IMP-concurrency.md "M3a Scope Boundaries" (ShadowsCrossingLocal permanent + NestedShapeCrossing deferred, 4-field form); registry [[deferred_language_feature]] nested-shape-crossing-wait; examples/primantis-orders/v0_3_m3a_errors.ynz gallery; jargon audit struct→shape fix (9/0 green).

**Follow-ups (NOT P1 — tracked):** (a) pre-existing M2 `EC<string>` return bug (lower_stmt_return misreads string ptr as EC-struct ptr); (b) float-literal decimal128-format bug; (c) minor over-slot: frame may allocate a slot for a post-final-wait local (perf/cosmetic, output correct, alloc=1/free=1); (d) lower_stmt_return shape staging is dead-but-correct code for a future non-crossing-shape-return case.

## ROUND 7 ADDENDUM (round-6 gate found bool-SIGSEGV, nested-after-wait ICE, shadow false-positive, mechanical gaps)

Round-6 re-gate BLOCKs: bool-SIGSEGV (alloca type mismatch), nested-after-wait ICE (collect_crossings_in_stmts past_wait branch doesn't recurse), shadow false-positive (inner-only crossing local triggers outer shadow check), tmgrammar deviation, alloc-proof-3level comment, error gallery gap (NestedShapeCrossing missing trigger). Round-7 fix:

- **FIX A — bool SIGSEGV (systematic classifier fix)**: split `sm_crossing_scalar_set` (int) from new `sm_crossing_bool_set` (bool). Bool keeps its natural i1 alloca; flush now loads i1 and zexts to i64 for the frame slot; reload truncates i64 frame slot back to i1 before storing to alloca. Int path unchanged (i64 alloca, raw load/store). ONE classifier (`sm_crossing_bool_set`) drives all three: alloca type, flush, and reload — no width-mismatch possible by construction. emit.rs.

- **FIX B — sole-nested crossing local AFTER prior top-level wait → LLVM ICE**: in `collect_crossings_in_stmts` (check.rs), the `past_wait == true` else branch now recurses into any If/While/For/Match body that contains a suspension. Previously the branch only called `collect_ident_refs_in_stmt` (scanning refs to already-declared outer locals) but never recursed to detect crossing locals DECLARED INSIDE the branch — so no sm_entry alloca was created for them, and their alloca landed in a non-dominating state block. check.rs.

- **FIX C — shadow false-positive**: guard `find_shadow_in_stmts` call in Check 3 with `has_top_level_let` (new helper). Shadow detection only runs for crossing locals whose `let` declaration is at the function body's TOP LEVEL. An inner-only crossing local (declared solely inside a nested block) cannot be shadowed by a later outer `let` of the same name — the outer `let` is not itself a crossing local, so no alloca ambiguity exists. Genuine shadows (outer crossing `let x` + inner `let x`) still produce the clean error. check.rs.

- **FIX D1 — tmgrammar scope deviation**: added deviation entry for `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` (regenerated to include nested-shape-crossing-wait deferred feature per FIX-6 registry entry). See below.

- **FIX D2 — alloc-proof-3level comment**: added deviation entry for `crates/ynz-driver/tests/fixtures/v0_3_m2_alloc_proof_3level.ynz` (cosmetic comment correction, tangle residual from P0 rename). See below.

- **FIX D3 — error gallery gap**: added `NestedShapeCrossing` intentional trigger to `examples/primantis-orders/v0_3_m3a_errors.ynz` (shape with nested-shape field crossing a wait → clean diagnostic). Previously the gallery header mentioned NestedShapeCrossing but had no trigger function for it.

### FIX D1 — Scope deviation: tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json

Regenerated (not hand-edited) from [`registry/features.toml`](../../../../registry/features.toml) via `cargo run -p ynz-tmgrammar` to include the `nested-shape-crossing-wait` deferred feature added in FIX 6 Round 5. The file is generated output; regenerating it after any registry change is the required workflow per `crates/ynz-tmgrammar/`. Diff hunk: `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json:1-<EOF>`.

### FIX D2 — Scope deviation: crates/ynz-driver/tests/fixtures/v0_3_m2_alloc_proof_3level.ynz

Cosmetic comment correction (tangle residual from P0 sleepAsync→sleep rename — comment in the fixture still referenced the old name). No behavioral change; file is a test fixture. Diff hunk: `crates/ynz-driver/tests/fixtures/v0_3_m2_alloc_proof_3level.ynz:1-20`.

## Known low-frequency flake (tracked, NOT a codegen bug — follow-up)
- `v03_m3a_p1_shape_crossing_local` flaked ONCE under full parallel `cargo test --workspace` (passed on retry). Coordinator stress-tested: 20/20 standalone shape runs print 30 (deterministic, not racy); 8/8 isolated test runs pass; 3 full integration-suite runs 131/131 green. Conclusion: low-freq parallel-load harness contention (subprocess/runtime startup, possibly the `background_timing` 80ms assertion under load), NOT a shape-codegen correctness issue. Track + investigate in a follow-up; do not block P1.

## ROUND 9 ADDENDUM (round-8 found 2 loud bugs: shadow false-positive 3rd-variant + bool-return ICE; both root-fixed)

Patrick directive: "whatever the long-term right answer per design docs + rules." Two ROOT-CAUSE fixes (NOT another heuristic), coordinator-verified on compiled binaries + full suite green:

- **FIX 1 — shadow guard scope-aware predicate** (`crates/ynz-typeck/src/check.rs`): replaced the structural `has_top_level_let_before_suspension` heuristic (which false-positived 3× across rounds) with `outer_is_genuine_crossing_local` + helpers `stmt_refs_target_non_shadowed` / `expr_refs_ident`. The `ShadowsCrossingLocal` error now fires ONLY when the outer same-named binding has a read AFTER a top-level suspension ATTRIBUTABLE to the outer binding (not masked by an inner shadow's reads). This precisely matches the docs/internal/implementation/IMP-concurrency.md "M3a Scope Boundaries" ShadowsCrossingLocal constraint ("outer binding which crosses a wait"). Executor deviation (fixed the guard predicate rather than `collect_crossings_in_stmts` itself): SOUND — the analysis must keep reporting inner crossings for codegen slot allocation; only the guard needs the outer-vs-inner distinction. Coordinator-verified: dangerous shadows (a)+(b) STILL ERROR (no false-NEGATIVE/silent-miscompile), false-positive (outer read-only-before-wait + inner crossing) now COMPILES (hello/42), disjoint siblings compile.
- **FIX 2 — bool wrapper-return trunc** (`crates/ynz-codegen/src/emit.rs:~2278` + zext-on-write at the return-slot store): a bool-returning suspending fn previously ICE'd ("ret i64 ... i1"). Split the `Type::Int | Type::Bool` wrapper-return arm to `trunc i64→i1` for Bool. Completes FIX-A's systematic bool-width coverage across all 4 sites (alloca/flush/reload/wrapper-return). The zext-on-write at the store is the upstream invariant making the trunc-on-read safe (false→0x0→i1=0). Coordinator + bool-return-judge verified `false`→false (no garbage-high-bit→true), `true`→true, in-caller-`if` correct.
- New fixtures: `v0_3_m3a_p1_scope_aware_shadow_false_positive.ynz` (compiles, hello/42), `v0_3_m3a_p1_disjoint_sibling_scope_shadow.ynz` (compiles, 42/99), `v0_3_m3a_p1_bool_returning_suspending_fn.ynz` (true/false, exit 0) — all integration-tested.

## ROUND 10 housekeeping (plan-adherence flag)
- Deleted 9 unreferenced `adv_*.ynz` dev-scratch fixtures (adversarial testing residue written into the fixtures dir; 0 test refs; proper `v0_3_m3a_p1_*` counterparts exist + are integration-tested).

## GIT-STATE NOTE (for the record)
- Rounds 4/6 reviewer/judge agents ran destructive `git checkout`/`stash` → stranded the working tree on `main` (off the feature branch). Recovered: backed up P1 to /tmp/p1-backup.patch + /tmp/p1-delta.patch + `git stash create` snapshot b3adbfc; reset crates/examples to 827f8bb; re-applied; restored phase-0 scratch (committed in 827f8bb). All gate prompts from round 4 onward carry a HARD read-only-git-only guardrail. /learn graveyard entry pending: reviewers/judges must use read-only git only.

## ROUND 10 code-reviewer follow-ups (non-blocking; carry forward)
- **P2/P3 ENTRY OBLIGATION (load-bearing)**: the shadow-guard suspension scan (`outer_is_genuine_crossing_local` + `has_top_level_let_before_suspension`, check.rs ~5066/~5219) currently treats ONLY `If`-body waits as top-level suspension points. Correct in P1 because `wait_in_loop_or_match_body` (check.rs:434) still rejects waits in While/For/Match bodies. **When P2/P3 lift WaitInsideLoop, this scan MUST be extended to treat loop/match bodies as suspension points** — else a real blind spot (outer crossing a loop-body wait + same-name inner shadow → guard says "not genuine" → compiles → single-alloca conflation = silent miscompile). Add to P2 entry checklist.
- **Hardening nicety**: add a `// Invariant:` comment at the crossing-local alloca site (emit.rs ~1969) documenting that name-only single-alloca-per-crossing-name is safe ONLY because the shadow guard rejects the genuine-outer + same-name-shadow overlap. Prevents a future contributor relaxing the guard without re-examining the alloca model. (Code-quality, defer to a cleanup or fold into P2.)

---

## ROUNDS 11-27 ADDENDUM (deviation record; full round-by-round in the plan's ORCHESTRATION STATE resume anchor)

**Shadow guard (R11-15):** R11 replaced the round-9 body-suppression heuristic with a lexically-correct walker (read resolves to nearest enclosing binding); R12 extended it to PARAMETERS (param-shadow was a confirmed LLVM ICE). R14 tried to make the top-level/param scans permissive to kill the ADV10 "false positive" → SILENT MISCOMPILE (ADV10 7/7 not 7/99). R15 REVERTED to safe-conservative (the "fix" provably re-opens ADV10 because Shape-B is gated by `outer_is_genuine` at check.rs:564) + documented `shadow-crossing-local-support` deferral. **Deviation:** the guard is deliberately CONSERVATIVE (rejects same-name reuse around a wait), NOT precise — accepted as a documented deferral, not a bug.

**Return-path scope-creep (R16-27) — the root of the long tail:** R16 found a `-> float`/`-> number` suspending-return crash (missing wrapper-return match arm). R17 fixed it but the executor went WIDER than scoped (Deviation: added EC/shape return-by-value handling never designed). Consequences, each a distinct real defect:
- R18: EC-number-return silent miscompile (success read as error). R19 "fixed" it with a leaking `ynz_alloc` (alloc=2/free=1). R20 REVERTED the leak → loud-reject `Shape`/`Shape errors`/`number errors` via `WideValueSuspendingReturn` guard + deferral.
- R22: anon-shape diagnostic leaked `__anon__` internal name (→ `type_name()`); union/maybe/dynamic CROSSING LOCALS hit a raw LLVM ICE (M3a lifted LocalCrossesWait too broadly) → clean-reject via `UnsupportedCrossingLocalType` guard + deferral.
- R23: `number errors` return LIFTED out of deferral via a 16-byte frame STAGING slot (ownership-correct, alloc=1/free=1 — the ownership-correct version of R19's rejected leak). Per Patrick: decimal128 fallible-async return is the precision type's flagship use case, must work.
- R25: EC value (int/string/number errors) RETURN-propagated across a 2nd suspension read back as a stack address (Tier-A silent miscompile) → fixed by making the EC crossing local FRAME-RESIDENT (value bits in 2 slots, not a stack-alloca pointer). 31-probe sweep confirmed the representation fix complete.
- R27: removed a dead `Type::ErrorsCapable` heap-copy in the standalone EC wrapper (guard never matched due to type-flattening; would have LEAKED if it fired). The standalone-wrapper EC result-collection is deferred to M3b (`ec-wrapper-collect-on-completion`); fire-and-forget `background` discard is safe + leak-free today.

**New deferred-feature registry entries (R15-27):** `shadow-crossing-local-support`, `nested-shape-crossing-wait`, `wide-value-suspending-return` (Shape/Shape-errors only after R23 narrowed it), `unsupported-crossing-local-type`, `ec-wrapper-collect-on-completion`.

**Commit-time coordinator actions done:** corrected AC#2 + AC#6 stale evidence strings; filled Phase Review Gates; staged 15 untracked fixtures; removed stray `stderr` artifact. **Pending:** /learn graveyard entry (reviewers/judges must use read-only git only — 2 agents corrupted the branch in early rounds).

### phase2-deviations.md

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
  (1) CODEGEN (emit.rs): `alloca_in_entry` helper builds non-crossing `let` allocas in the function entry block (satisfying LLVM SSA dominance for all successor blocks), and all 6 scope-exit restore loops changed from `sm_crossing_names`-only to `locals_snapshot`-all (correct lexical scoping: outer bindings restored on scope exit, no clobber from inner shadows). Yinz allows variable shadowing (docs/internal/implementation/IMP-linting.md `shadowed-variables` lint); this makes it correct at the codegen level.
  (2) TYPECK (check.rs): `param_has_nested_let_shadow` blanket reject (A7) replaced by `param_shadow_has_genuine_crossing` which runs `collect_crossings_in_stmts` on nested block bodies with an empty param filter — only rejects when the inner binding itself is read after a `wait` inside its own scope (a genuine frame-slot collision). Non-crossing shadows now compile and produce correct output.
  (3) DESIGN DOC (docs/internal/implementation/IMP-concurrency.md): amended `ShadowsCrossingLocal` section to accurately reflect the param Shape-A rule — crossing shadows rejected, non-crossing shadows compile.
  Fixtures `v0_3_m3a_p2_param_shadow_noncrossing_rejected.ynz` and `v0_3_m3a_p1_param_shadow_crossing.ynz` flipped from compile-error to compile-and-run (correct behavior verified). Integration tests updated with `// test-ratchet:` annotations explaining the behavior change.
- **diff hunks**: `crates/ynz-codegen/src/emit.rs:1450-1483` (alloca_in_entry helper), `crates/ynz-codegen/src/emit.rs:5163-5175` (non-crossing let uses alloca_in_entry), `crates/ynz-codegen/src/emit.rs:5204-5258` (lower_stmt scope arms restore-all), `crates/ynz-codegen/src/emit.rs:3462-3496,3628-3640` (lower_sm_block scope arms restore-all), `crates/ynz-typeck/src/check.rs:5636-5689` (param_shadow_has_genuine_crossing new helper), `crates/ynz-typeck/src/check.rs:759-826` (Check 3b Shape a predicate + diagnostic text), `docs/internal/implementation/IMP-concurrency.md:293-301` (Shape A clarification), `crates/ynz-driver/tests/fixtures/v0_3_m3a_p1_param_shadow_crossing.ynz:1-8` (WHY comment update), `crates/ynz-driver/tests/fixtures/v0_3_m3a_p2_param_shadow_noncrossing_rejected.ynz:1-30` (fixture rewritten for non-crossing semantics), `crates/ynz-driver/tests/integration.rs:2714-2744` (P1 test flipped), `crates/ynz-driver/tests/integration.rs:3135-3162` (P2 noncrossing test flipped)
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
  `docs/internal/implementation/IMP-concurrency.md:293-300` (param Shape-A + non-async paragraph),
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

### phase3-deviations.md

# v0-3-m3a-suspension-codegen Phase 3 Deviations — ROUND 6 (captured 2026-06-04)

D_count: 2 approach (+ established scope touches: registry/features.toml, docs/internal/implementation/IMP-concurrency.md, ynz.tmLanguage.json, integration.rs)

> Round 6 = Patrick-directed INTERIM loud-reject guard (`ArrayShapeRuntimeFieldWithWait`) for the round-5 8th head (array<Shape> with runtime field values crossing a wait → silent stack-garbage). Long-term fix = by-value element storage, tracked as milestone m3c-array-by-value (docs/internal/scratchpad/SCRATCH-future-array-by-value-element-storage.md + todos.md). North star honored: silent → loud.

## Round-6 (coordinator-verified live)
- Runtime-field array<Shape> crossing EXPLICIT wait → exit 1, `ArrayShapeRuntimeFieldWithWait` clean diagnostic (WHAT + concrete rewrite WHAT-INSTEAD + full root-cause WHY + design ref). Deterministic (twice).
- Runtime-field array<Shape> crossing INFERRED suspension (bare `pause()` call, no explicit wait) → ALSO loud-rejects (crossing analysis recognizes the call as a suspension point — the guard is NOT explicit-wait-limited; the executor's feared edge did not materialize).
- Literal-field array<Shape> crossing wait → STILL WORKS (30) — round-5 win preserved, no over-reject.
- Runtime-field array<Shape> NOT crossing a wait → WORKS (30) — no over-reject.
- registry [[deferred_language_feature]] array-shape-runtime-field-with-wait + docs/internal/implementation/IMP-concurrency.md note + 2 fixtures + tmLanguage regen. jargon fixed (struct/implementation banned words). 216 integ + 31 SM tests 0 failures, clippy/fmt/jargon clean.

## Approach Deviations (verbatim from round-6 executor report)

- **Deviation #1** (guard predicate — `is_let_declared_before_wait_in_stmts` pre-check): task said "detect array<Shape> with runtime field values that CROSSES a wait." The crossing analysis (`locals_crossing_wait`) is intentionally conservative and can mark a `let` declared AFTER the first wait as a crossing candidate for a later suspension → would over-reject arrays constructed after the wait. Added `is_let_declared_before_wait_in_stmts` walking top-level stmts so the guard fires only if the array `let` appears before a suspension. Rationale: tighter scoping avoids false positives; the crossing analysis alone is too broad for this guard. Diff hunks: `crates/ynz-typeck/src/check.rs:6577-6612`. **[COORDINATOR-VERIFIED: no over-reject (array-after-wait works) AND still catches inferred-suspension crossings.]**

- **Deviation #2** (literal predicate — IntLit|BoolLit only): task said "not a compile-time IntLit/BoolLit/FloatLit/etc." Yinz AST has no `FloatLit` (float literals are `NumberLit`). `expr_is_compile_time_literal` uses `IntLit | BoolLit` only — the exact set `try_build_shape_global` can fold to a stable global. `NumberLit`/`StringLit`/`NoneLit` fields also cause `try_build_shape_global` to return None (stack alloca → unsafe), so the guard correctly fires for them. Rationale: mirrors the codegen fold-ability exactly. Diff hunks: `crates/ynz-typeck/src/check.rs:6614-6626`.

## Resolved spawn list

### Deviation #1 (approach) — guard pre-check scoping [JUDGE D1 — over/under-reject boundary]
- type: approach | hunks: crates/ynz-typeck/src/check.rs:6577-6612 | hash: 3b8a5c30e6baac4b4e0e2a470c2aab8d0b5333ad

### Deviation #2 (approach) — literal predicate [JUDGE D2 — does it fire on NumberLit/StringLit fields?]
- type: approach | hunks: crates/ynz-typeck/src/check.rs:6614-6626 | hash: 0745fc54be56d9ec054c7c56f9f6527c57cb19dd

### phase4-deviations.md

# v0-3-m3a-suspension-codegen Phase 4 Deviations — captured 2026-06-04

D_count: 1 (scope)

## Scope Deviations (verbatim from executor report)

- **Scope Deviation #1** (file: `examples/pirates-roster/expected_stdout.txt`): touched outside declared scope. Rationale: the integration test `examples_basics_runs_end_to_end` does byte-exact golden-file comparison against this file; without updating it to include the new P4 demo output the test fails — it is a direct dependency of the pirates-roster demo extension that IS in scope. Diff hunks: `examples/pirates-roster/expected_stdout.txt:1-4` (appended 4 lines for the `scout total: 438` demo output).

## Approach Deviations (verbatim from executor report)

None — implementation matched plan's named approaches.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1 (scope) — expected_stdout.txt golden update
- **type**: scope
- **rationale**: byte-exact golden-file dependency of the in-scope pirates-roster demo extension; updating it is mandatory for the test to pass.
- **diff hunks**: examples/pirates-roster/expected_stdout.txt:1-4
- (trivial mechanical golden update; plan-adherence covers it — no dedicated judge needed)

