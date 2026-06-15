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
- **P1-D15 (NEW) — nested/owned-heap-field shape guard**: a crossing shape with a shape-typed field (or other owned-heap field whose inner storage can't survive a shallow embed) gets a CLEAN COMPILE ERROR at typeck (WHAT/WHAT-INSTEAD/WHY) + documented deferral — NOT a silent miscompile. Recursive aggregate frame-embedding is a v0.3+/M3b follow-up. Boundary = ownership.md r4 transitively-trivial. Files: check.rs (guard). MUST document in design/concurrency.md + check registry [[deferred_language_feature]].
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
- **FIX 6 — deferral docs**: design/concurrency.md "M3a Scope Boundaries" (ShadowsCrossingLocal permanent + NestedShapeCrossing deferred, 4-field form); registry [[deferred_language_feature]] nested-shape-crossing-wait; examples/primantis-orders/v0_3_m3a_errors.ynz gallery; jargon audit struct→shape fix (9/0 green).

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

Regenerated (not hand-edited) from `registry/features.toml` via `cargo run -p ynz-tmgrammar` to include the `nested-shape-crossing-wait` deferred feature added in FIX 6 Round 5. The file is generated output; regenerating it after any registry change is the required workflow per `crates/ynz-tmgrammar/`. Diff hunk: `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json:1-<EOF>`.

### FIX D2 — Scope deviation: crates/ynz-driver/tests/fixtures/v0_3_m2_alloc_proof_3level.ynz

Cosmetic comment correction (tangle residual from P0 sleepAsync→sleep rename — comment in the fixture still referenced the old name). No behavioral change; file is a test fixture. Diff hunk: `crates/ynz-driver/tests/fixtures/v0_3_m2_alloc_proof_3level.ynz:1-20`.

## Known low-frequency flake (tracked, NOT a codegen bug — follow-up)
- `v03_m3a_p1_shape_crossing_local` flaked ONCE under full parallel `cargo test --workspace` (passed on retry). Coordinator stress-tested: 20/20 standalone shape runs print 30 (deterministic, not racy); 8/8 isolated test runs pass; 3 full integration-suite runs 131/131 green. Conclusion: low-freq parallel-load harness contention (subprocess/runtime startup, possibly the `background_timing` 80ms assertion under load), NOT a shape-codegen correctness issue. Track + investigate in a follow-up; do not block P1.

## ROUND 9 ADDENDUM (round-8 found 2 loud bugs: shadow false-positive 3rd-variant + bool-return ICE; both root-fixed)

Patrick directive: "whatever the long-term right answer per design docs + rules." Two ROOT-CAUSE fixes (NOT another heuristic), coordinator-verified on compiled binaries + full suite green:

- **FIX 1 — shadow guard scope-aware predicate** (`crates/ynz-typeck/src/check.rs`): replaced the structural `has_top_level_let_before_suspension` heuristic (which false-positived 3× across rounds) with `outer_is_genuine_crossing_local` + helpers `stmt_refs_target_non_shadowed` / `expr_refs_ident`. The `ShadowsCrossingLocal` error now fires ONLY when the outer same-named binding has a read AFTER a top-level suspension ATTRIBUTABLE to the outer binding (not masked by an inner shadow's reads). This precisely matches the design/concurrency.md "M3a Scope Boundaries" ShadowsCrossingLocal constraint ("outer binding which crosses a wait"). Executor deviation (fixed the guard predicate rather than `collect_crossings_in_stmts` itself): SOUND — the analysis must keep reporting inner crossings for codegen slot allocation; only the guard needs the outer-vs-inner distinction. Coordinator-verified: dangerous shadows (a)+(b) STILL ERROR (no false-NEGATIVE/silent-miscompile), false-positive (outer read-only-before-wait + inner crossing) now COMPILES (hello/42), disjoint siblings compile.
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
