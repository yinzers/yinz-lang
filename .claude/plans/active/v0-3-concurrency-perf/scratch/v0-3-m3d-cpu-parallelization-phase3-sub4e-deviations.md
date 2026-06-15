# v0-3-m3d-cpu-parallelization Phase 3 sub-slice 4e Deviations — 2026-06-15

D_count: 5 (3 approach + 2 scope)

> Sub-slice 4e (FINAL): gate lifts + sweep. Outcome = 2 honest verify-first DECLINEs (pre-pair-wait,
> param-host) + 1 FIRE (N>2 single group) + fixture-header sweep + demo. BASE eba9ec1. Corpus 2046/0.
> The two DECLINE judges are INVERTED (try to safely FIRE the declined case; PASS only if it can't be).

## Approach Deviations (verbatim from executor report)

- **Deviation #1 (gate lift 1, FIRE→DECLINE — pre-pair wait)**: envelope said FIRE if a pre-pair wait sequenced before the pair; executor DECLINED. Rationale: "verify-first Paper-Trace — a pre-pair wait host is in typeck's base_suspends, never a CPU-promotion candidate (queries.rs:763, same structural exclusion 4c hit). Lifting it broadly FIRES correctly (2 spawns, 7000 byte-identical, no deadlock) BUT breaks the slice-1 cross-boundary frame-sizing invariant — `imported_suspending_after_pair_declines_consistently_across_boundaries` FAILS = the silent-heap-corruption corpse the entrypoint-only gate prevents. A narrow lift (intrinsic-wait-only hosts) collides with the closure/rollback assumption promoted⊥base_suspends = the dual-kind frame machinery scoped to M3g (todos:198). Clean DECLINE with a 0-spawn lock is safe; unsafe FIRE is not." Diff hunks: `crates/ynz-driver/tests/fixtures/v0_3_m3d_prepair_wait_declines.ynz:1-31, crates/ynz-driver/tests/integration.rs (v03_m3d_prepair_wait_declines_byte_identical)`.
- **Deviation #2 (gate lift 2, FIRE→DECLINE — param-host)**: envelope said FIRE if params round-trip; executor DECLINED. Rationale: "verify-first Paper-Trace — Observed: param-host with the param read AFTER the join (`return seed + a + b`) prints 7000 default; Expected `score(3)+score(4)+seed = 7003` (--no-auto-parallel oracle = 7003); Residual 3 (= the param value). The wrapper writes param slot 0 at byte 32 (`store_local_slot`: FRAME_HEADER_SIZE + idx*8) = exactly the CPU handle-slot-0 byte for a spike host; the spawn overwrites it → post-join param reload reads 0. Params-live-across-the-join are NOT covered by the 4b crossing-slot machinery (locals only); the fix needs wrapper param-store + resume param-load offset past the CPU reserve = param-slot reservation beyond a gate flip. Param used only in spawn args round-trips fine; read-after-join silently corrupts. DECLINE is the honest outcome." Diff hunks: `crates/ynz-driver/tests/fixtures/v0_3_m3d_param_host_declines.ynz:1-38, crates/ynz-driver/tests/integration.rs (v03_m3d_param_host_declines_byte_identical)`.
- **Deviation #3 (gate lift 3 impl — DRY consolidation)**: plan said lift the count gate / [u64;2] arrays / CPU_GROUP_MEMBER_COUNT reservation; executor ADDITIONALLY consolidated 3 duplicated group-extraction blocks (spike_pair_in_block, spike_extract_cpu_group, spike_cpu_group_result_names) into ONE shared `spike_cpu_group_member_indices` source of truth + `spike_cpu_group_member_count`. Rationale: "the N-extension requires dependency/args/run checks across all N members in lockstep at the count gate AND admission gate AND extraction — 3 previously-duplicated copies; one shared member-index fn is the only way the frame reserve (sized from count) and emission (iterating members) can never disagree on N — the drift hazard the slice-1/slice-2 parity BLOCKs repeatedly hit. Net -188 lines (DRY, not bloat)." Diff hunks: `crates/ynz-codegen/src/emit.rs:6974-6991, :6993-7142, :7192-7233, :7326-7345, :7376-7402`.

## Scope Deviations (verbatim)
- **Scope #1**: swept the `s` fixture (`v0_3_m3d_spike_s_imported_suspending_after_pair/{entrypoint,io_lib}.ynz`) beyond the plan-named `r` fixture — same compiler-jargon class ("BARE/EFFECTIVE local suspend set") leaking into user-facing `.ynz` (vocabulary.md); prompt's sweep directive said "grep all fixtures". Diff hunks: those 2 files.
- **Scope #2**: `crates/ynz-typeck/src/queries.rs` touched ONLY during verify-first probing (experimental base_suspends lift), FULLY REVERTED (git diff eba9ec1 -- crates/ynz-typeck/src EMPTY). No net change. (Coordinator probe confirmed 0 diff lines.)
- (Demo: `examples/pirates-roster/entrypoint.ynz` + `expected_stdout.txt` — mandated Demo & Error Gallery per plan-invariants, IN scope.)

## Resolved spawn list

### Judge 1 (INVERTED) — lift-1 pre-pair-wait DECLINE: genuine unsafety or avoidance?
- type: approach
- task: try to SAFELY fire a pre-pair-wait-sequenced CPU group (prove the DECLINE wrong). Confirm the broad lift genuinely trips the slice-1 cross-boundary corpse (`imported_suspending_after_pair_declines_consistently_across_boundaries`) AND a narrow safe lift genuinely collides with promoted⊥base_suspends. PASS iff the DECLINE is genuine (can't safely fire in 4e scope); BLOCK if a safe FIRE exists the executor missed.
- diff hunks: emit.rs (gate 3925 area, unchanged), the prepair_wait_declines fixture
- judge identity: approach-lift1-prepair-wait-decline-genuine

### Judge 2 (INVERTED) — lift-2 param-host DECLINE: genuine corruption or fixable in-slice?
- type: approach
- task: verify the param-slot-collision corruption is REAL (param slot 0 at byte 32 = CPU handle slot 0) and that fixing it needs param-slot reservation beyond a gate flip (NOT cheaply fixable in 4e). Try to safely fire a param-host (e.g. param used only in spawn args, OR a cheap reservation). PASS iff the DECLINE is genuine; BLOCK if a safe FIRE or a cheap in-slice fix exists.
- diff hunks: emit.rs (gate 6906 area), the param_host_declines fixture
- judge identity: approach-lift2-param-host-decline-genuine

### Judge 3 — DRY consolidation behavior-preservation (parity corpse)
- type: approach
- task: the -188-line consolidation of 3 extraction blocks into one shared `spike_cpu_group_member_indices`. Verify it preserves behavior across ALL prior cases (distinct/same-callee, return-class matrix, if/match-arm nested, accumulator/crossing-local, promoted-host, the DECLINEs) AND the new N>2 case — the count gate, admission gate, and emission all agree on N by construction. This is the exact parity-drift surface the slice-1/2 BLOCKs hit. PASS iff no case regresses + the single-source genuinely eliminates drift; BLOCK if any case routes wrong or the consolidation hides a divergence.
- diff hunks: emit.rs:6974-7402 (the consolidated extraction)
- judge identity: approach-lift3-dry-consolidation-parity

## Reviewer gate
- code-reviewer (Opus): N>2 FIRE codegen correctness (dynamic offset arrays, member-count reserve, 3-spawn fire, distinct values, alloc=free) + the DRY consolidation + the removed-constants safety.
- rules-compliance: comments/vocab on new fixtures + the s-fixture sweep + Big-O on new helpers + no changelog/phase-labels.
- plan-adherence: 4e scope (3 lifts attempted, 2 declined verify-first, 1 fired), no re-opening of M3g/loop-body/multi-group, deviations documented + banned-phrase-clean, demo extended.
- acceptance-verifier: LIVE — 2046/0, N=3 FIRE (3 spawns byte-identical alloc=free), both DECLINEs 0-spawn, completeness gate green, m3d suite 44/0. (Coordinator may substitute a live-run if the agent malforms again.)
- design-compliance: the DECLINEs consistent with concurrency.md (sequential default) + no-function-coloring.md; N>2 poll-based; no coloring/bridge.

## FIX ROUND (2026-06-15) — 4e gate verdicts resolved (6 PASS / 2 BLOCK / 2 CONCERN)

BASE eba9ec1. Corpus now 2048/0 (+2 new param-host FIRE tests). Deviation #2 (param-host DECLINE)
was INVERTED by deviation-judge-2 → the wholesale param decline OVER-declined. Resolutions:

- **BLOCK 1 (judge-2 inversion) — narrowed the param-host gate**: replaced the wholesale
  `!f.params.is_empty()` decline with a `spike_param_read_after_join` post-join read check.
  A param used ONLY in spawn args now FIRES (the spawn-arg load at emit.rs:~7855 reads the
  param's stack alloca BEFORE the handle store clobbers byte 32 — a dead store); a param READ
  in a post-join statement still DECLINES. New helper `stmt_tree_ident_reads` (recursive,
  conservative) + `spike_param_read_after_join` in emit.rs; `collect_ident_names` made pub in
  ynz-typeck/independence.rs. Nested-group param-hosts still decline (post-join frontier crosses
  the branch boundary). VERIFIED LIVE: spawn_args_only FIRES 2 spawns 9907 byte-identical;
  n3_spawn_args_only FIRES 3 spawns 14862 byte-identical; param_host_declines (read-after-join)
  DECLINES 0 spawns 9910 byte-identical. Deviation #2 is RESOLVED (no longer a DECLINE for the
  spawn-args-only subset).
- **BLOCK 2 (rules) — added the todos.md deferral entry** `m3d-param-host-read-after-join`
  (4-field: WHAT/WHY/cost/trigger) for the NARROWED residual (read-after-join only). Updated the
  integration.rs param-host-declines test comment so its "tracked in todos.md" claim is now
  accurate + reflects the narrowed scope.
- **CONCERN 1 (independence-check invariant)**: added durable comment at emit.rs
  `earlier_bind_names[..pos]` naming the forward-only-dependency-flow invariant (compacted-list-
  vs-full-index misalignment is a conservative SUPERSET → spurious DECLINE only, never false-ADMIT).
- **CONCERN 2 (dead-symbol fixture comments)**: rewrote the 3 fixture comments
  (spike_i_mixed_locals:4, spike_k_param_host, spike_q_suspending_callee:10) to use plain byte
  descriptions instead of the deleted SPIKE_*_OFFSET symbols. spike_k comment also corrected to
  describe the read-after-join decline reason (not the old wholesale param decline). Grep confirms
  zero remaining references to the 4 deleted symbols across all .ynz fixtures.

New scope touch: `collect_ident_names` made `pub` in ynz-typeck/src/independence.rs (BLOCK 1
mandated reusing the existing walker rather than writing a parallel one).
