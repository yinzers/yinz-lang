---
name: "v0-3-m5-auto-soa-audit"
plan-id: "2026-07-03-v0-3-m5-auto-soa"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-07-03-v0-3-m5-auto-soa

Append-only. *How the plan got here.* Read by the AAR and auditors, never by executors (they read
the current-truth plan.md slice).

## Session log
- plan-producer-2026-07-03-m5 — 2026-07-03 — Authored the initial OPORD from the assembled brief
  (grilled 2026-07-03). Consumed: fresh recon landscape (file:line verified against the working
  tree incl. uncommitted M4 P4 substrate), Patrick's four locked decisions (m3c array-by-value fold
  into M5; serialization reframe to a forward-compat design note; DAP outright deferral via
  `[[deferred_tooling_feature]]`, four-field signed; Fable 5 executor dispatch), and the risk union
  (E1–E11 scored on the frozen engine — no HIGH residual; no override block needed). Status set
  `stub` per the pre-approval convention; conductor flips to `active` at the approval gate.
  Execution hard-gated on the v0.3.0 release.
- plan-producer-2026-07-03-m5 — 2026-07-03 — Plan-reviewer fix pass (FRAGO 001 below): scored + wired
  E12 (`map<K,Shape>` symmetric fix), added roadmap.md:121 to Phase 1, added D9's jargon_audit
  re-verify caveat, promoted the padding-wins precedence to D11. No blockers were raised; no HIGH
  residual introduced.
- phase0-executor-2026-07-03-m5 — 2026-07-03 — Phase 0 segment 1 (PARTIAL at the step-1 checkpoint
  mark): applied the FRAGO 002-004 gate-waiver note to plan.md ¶3.4 per conductor instruction;
  ran S1 (by-value ABI spike) through the real compiler in the worktree dev container — verdict
  **GREEN** (differential: gate-off vs YNZ_SPIKE_BYVAL=1 byte-identical stdout); scaffolding torn
  down, fixture + verdict persisted to spike-notes/; surfaced a pre-existing float-literal
  miscompile (check.rs:2198-2199 vs emit.rs NumberLit arm — details in spike-notes/s1_verdict.md)
  and a minor recon-path drift (runtime_decls.rs lives in ynz-codegen, not ynz-runtime). Handoff at
  handoff-phase-0.md, resume-at phase-0/step-2.
- phase0-executor-2026-07-03-m5-seg2 — 2026-07-03 — Phase 0 segment 2 (resumed at phase-0/step-2;
  phase **DONE**): S2 per-field access-analysis spike through the real pipeline (env-gated pass on
  the real TypedModule; 4 fixtures with pre-registered answer keys, all EXACT matches) — verdict
  **GREEN** (spike-notes/s2_verdict.md); scaffolding torn down, tree rebuilt pristine. S3 bench
  noise probe (12 reps × 3 process runs; ~15% mean-delta credibility floor vs a ~6× AoS-vs-SoA
  signal — spike-notes/s3_bench_noise.md + rerunnable s3_bench.rs). Exhaustive call-site audits:
  audit-array-callsites.md (P2) + audit-map-callsites.md (P3/E12), incl. the runtime.rs:634
  scheduler-internal ynz_array_drop consumer. Baselines (baselines-p0.md): E8 alloc=free across the
  array fixture suite — with the load-bearing finding that the counter instruments ynz_alloc/free
  ONLY (array/map buffer mallocs invisible; P2/P3 must route new buffers through counted entry
  points or the E8 gate is vacuous); E11 pirates-roster release-compiler build ≈210 ms mean
  (7 reps, 8.3% spread; ynz CLI has no --release flag yet — minor recon drift, main.rs:94-95).
  Phase-0 status line updated in plan.md; handoff-phase-0.md deleted as the final act.
- phase0-fix-executor-2026-07-03-m5 — 2026-07-03 — P0-boundary post-review fix dispatch (doc/planning
  edits only, zero source changes): applied FRAGO 005's plan-body edits (Phase 2 step 2 counted-
  entry-point requirement; Phase 3 step 4 parity-gate visibility entry criterion; E8 mitigation cell
  amended, residual LOW now earned) + FRAGO 006's (Phase 8 step 4 + Invariants ¶Performance E11:
  release-profile-compiler `ynz build` methodology replaces the nonexistent `ynz build --release`);
  split the audit-map-callsites.md YnzMap citation (struct lib.rs:594 / ynz_map_new lib.rs:674,
  both re-verified); added the missed `string_split_basic` test (lib.rs:2603-2616) to
  audit-array-callsites.md; added the s2_verdict.md lend-self-filter spike-scope note
  (false_sharing.rs:131-134); promoted the float-literal miscompile (check.rs:2198-2199 vs
  emit.rs:12370-12403; golden expected_stdout.txt:7) to .claude/todos.md as `float-literal-
  miscompile`, distinct from the two pre-existing float todos (both confirmed still present).
  All file:line anchors re-verified against the worktree before writing. Session-id appended to
  plan.md frontmatter.
- phase1-executor-2026-07-03-m5 — 2026-07-03 — Phase 1 (DONE, doc-only): recorded the fold in the
  SSOT docs. Step 0 first: FRAGO 006-addendum straggler applied (plan.md ¶1 E11 mitigation cell —
  stale `ynz build --release` → release-profile-compiler `ynz build` methodology, verified at
  plan.md:144 before editing). Roadmap edits (worktree copy): **deviation surfaced (not
  self-classified)** — the worktree's committed roadmap (403 lines, forked @1ac52fd) PREDATES the
  sibling M4 session's uncommitted 2026-07-02 M4/M5-split edits in main (421 lines); the plan's
  cited anchors (§Milestone 5 :341-356, both ledger M5 rows, :109/:127) did not exist here.
  Resolution: diffed both copies, imported the split-affected regions this phase edits VERBATIM
  from main's working copy (read-only on main), then applied the Phase-1 amendments on top;
  regions not edited stay at the fork base for clean auto-merge (M3g section/rows deliberately
  NOT imported). Landed: §M4 post-split imported verbatim; §M5 imported + amended (fold bullet,
  representation RESOLVED-unified, D11 padding-wins, serialization reframe per Divergence 2, DAP
  outright-deferral per Divergence 3, Execution-plan/Trigger/Ships-via updated to plan-id);
  :108 Auto-SoA bullet imported + plan-id pointer + DAP deferral; :120 mandate — `array-using-
  soa-layout` REASSIGNED M4→M5 (features.toml verified: only `cross-thread-fields-not-padded`
  + `prefer-yielding-sleep` exist, :2292/:2306); :126 DAP bullet superseded (outright deferral);
  :130 stale parenthetical fixed; serialization risk row reframed; Out-of-Scope DAP bullet fixed;
  BOTH ledger tables: M4/M5 rows imported+amended + new by-value-fold row each. Scratch docs:
  array-by-value → "FOLDED INTO v0.3-M5", :66 standalone-plan claim struck; auto-soa → owning-plan
  pointer blockquote (no trim — Phase 7). Beyond-slice consistency fixes (exit-criteria-driven,
  surfaced): todos.md:30 live entry annotated FOLDED (was claiming "OWN /plan" pending).
  Historical records left untouched as history: state.md:147/:151, done/m3f plan.md,
  plan.md:371 (per FRAGO 006 addendum note-and-carry). _index.md regenerated via lifecycle hook.
  Session-id appended to plan.md frontmatter; Phase-1 STATUS blockquote added in ¶3.3.
- phase2-executor-2026-07-03-m5 — 2026-07-03 — Phase 2 segment 1 (PARTIAL, early checkpoint at the
  step-1→step-2 boundary on a green tree — the atomic cut must not start on a spent context).
  Step 0: FRAGO 007's ¶3.4 CCIR-1 sharpening applied to plan.md (worktree-own-state cite
  verification; main-only anchors = BLOCKED-class). Step 1 DONE + compiler-verified: 11-fixture
  per-type × per-operation RED matrix (`m5_p2_byval_*.ynz` — {int,float,boolean,string,shape} ×
  {literal,runtime} + the adopted S1 spike fixture) with 11 `#[ignore]`d goldens appended to
  integration.rs; verified split 7 behavior-preservation cells GREEN on the pointer ABI / 4 RED
  exactly on the contract cells. **Deviations surfaced (not self-classified): (1) pre-existing
  base miscompile — `array<boolean>` push/set/contains emit raw i1 into the i64 slot ABI, LLVM
  module verify fails (bool arrays are entirely broken on the current tree; loud compile failure,
  not silent); the by-value choke-point staging fixes it by construction, locked by the 2 RED bool
  cells. (2) Recorded in-slice decision: `contains` on array<Shape> = field-wise VALUE equality
  (pointer-equality has no by-value analogue; padding caution documented), locked by the 2 RED
  shape cells.** All audit-checklist anchors re-verified against the worktree per FRAGO 007 — all
  resolve. Green tree: workspace builds; integration 470 passed / 0 failed / 11 ignored. Handoff
  at handoff-phase-2.md, resume-at `phase-2/step-2`. Session-id appended to plan.md frontmatter.
- phase2-executor-2026-07-03-m5-seg2 — 2026-07-03 — Phase 2 segment 2 (DONE — phase complete).
  Steps 2-5: the atomic by-value ABI cut. Runtime: `YnzArray` + `elem_size: i64`; hard-cut
  `new(elem_size)` / `push(*const u8)` / `get(idx, out: *mut u8) -> i64 has-flag (OOB zeroes out)`
  / `set(idx, *const u8)`; count/drop signatures unchanged (D6, runtime.rs:634 consumer untouched);
  drop/clone elem_size-aware; ALL array allocations counted via `ynz_alloc`/`ynz_free`, growth =
  counted alloc+copy+free (no realloc) per FRAGO 005; `ynz_string_split` + runtime unit tests
  migrated (+ new 16-byte-element by-value contract test). Codegen: `shape_abi_sizes` threaded
  into `Cg` (non-Option, 3 construction sites + new `lower_generic_function` param);
  `Cg::array_elem_*` choke-point section = the ONLY `rt.ynz_array_{new,push,get,set}` call sites
  (bits64 widening erases the bool raw-i1 miscompile by construction; entry-block staging/out
  buffers per S1); every audited emit.rs site migrated (ArrayLit, IndexAccess, IndexAssign,
  method add/get/first/last/set/contains, both for-loop paths, debug repr);
  `try_build_shape_global` DELETED; SM for-loop shape-embed special-case DELETED (get writes
  element bytes directly into the frame region); shape `contains` = field-wise value equality
  (`shape_value_eq`, per-field GEP — no padded-bytes memcmp). Evidence: matrix 11/11 green (4 RED
  contract cells flipped); integration 481/481; workspace green + clippy clean; E7 grep gates
  PASS; audit-array-callsites.md 100% ticked with dispositions; 13 insta snapshots verified
  decl-signature/metadata churn ONLY, promoted; dual-mode oracle 340 byte-identical / 0 real
  divergences (2 flags = the documented Model-A intended-reorder exclusion class, array-free;
  3 nondet = timing fixtures, nondet in default mode alone). **E8 first-look + deviation
  surfaced (not self-classified): counter now SEES buffers (P0 alloc=0 blindness → non-zero,
  exactly +2 counted allocs per array; clone→drop pairs balance, e.g. bg_array_real_copy 4/2);
  residual alloc-without-free = the PRE-EXISTING never-drop-local-arrays design made visible —
  plan step 3's "alloc=free holds on the non-suspending suite" cannot literally hold; Phase 3
  step 4's parity gate owns the verdict (drop insertion vs D6 fallback).** Six pre-M5 tests whose
  count assertions hard-coded the counter-blind world updated to exact new constants (invariants
  preserved: element-count independence, per-iteration-zero-alloc, frame parity via
  `m3d_assert_fires_byte_identical_alloc_gap`); stale comments naming deleted machinery rewritten
  (integration.rs survives_wait WHY; check.rs `expr_is_compile_time_literal` → P3 lift pointer).
  Phase-2 STATUS block added to plan.md; session-id appended; handoff-phase-2.md deleted as the
  final act.
- phase2-fixloop-executor-2026-07-03-m5 — 2026-07-03 — Phase 2 boundary FIX-LOOP (code-reviewer
  BLOCKER + bundled findings): fixed the get-side out-buffer aliasing silent miscompile —
  per-site entry-block staging (out buffer / maybe envelope) was pointer-stored by bindings, so
  any element escaping its loop iteration read the LAST iteration's bytes. RED-proven on the
  unfixed tree (4 differential cells: 3/30, 3/30, 3/30, 7 vs expected 1/10, 1/10, 2/20, 6), then
  fixed via the binding-point ownership funnel: emit.rs `store_binding` (Let/Assign) copies
  shape bytes (`shape_bytes_to_owned`) and maybe envelopes + shape payloads (`maybe_to_owned`,
  flag-guarded payload copy) into per-site variable-owned ENTRY-BLOCK regions (S1 discipline
  preserved — staging slots untouched, zero loop stack growth); sizes read from the ONE
  `shape_abi_sizes` source. PROBING FOUND A SECOND INSTANCE: Stmt::Assign to a frame-embedded
  crossing shape local plain-stored the RHS pointer, clobbering the pre-wired frame-region
  pointer (probe printed 0/0 for 2/20) — fixed via `shape_bytes_into_embed_slot` (assign now
  memcpys INTO the frame region); the `maybe<Shape>`-crossing-wait sibling is NOT constructible
  (UnsupportedCrossingLocalType rejects all maybe crossings — noted in P3 step 3 text). Five
  tripwire cells committed (`m5_p2_byval_{shape_escape_for,shape_escape_get,maybe_escape,
  int_maybe_escape,shape_escape_wait}`), all RED→GREEN; debug-repr fixture added
  (`m5_p2_byval_debug_repr`, byte-exact golden per type incl. shape arrays — the audited site
  was ticked untested); false for-loop "fresh COPY" comment rewritten to the real ownership
  contract; stale integration.rs "#[ignore]d until step 3" block comment rewritten. FRAGO
  008/009 plan-body edits applied (D12 recorded; Phase 7 step 5 docs-home + REF reconcile;
  Phase 3 step 4 parity semantics re-specified + interim gap-encoding ratified; Phase 3 step 5
  E8-class items incl. pointer-typed-fields question + nested-field-store + MapEntry-slot
  observations). Suite 487/487 (481+6), workspace green, clippy clean, `cargo fmt --all` run +
  check clean (green-check's 5 rewrap sites restored), E7 grep gates PASS (4 rt.ynz_array_*
  call sites, all in the choke-point section), one golden snapshot (m4_player_ir) churned —
  verified mechanical (%p_own alloca + memcpy binding copy only). Dual-mode oracle re-run
  post-fix; tallies + methodology in `p2-dualmode-report.md` (plan dir). Session-id appended.

### 2026-07-03 — Phase 3, segment 4 — session-id: phase3-executor-2026-07-03-m5-seg4

- **Scope:** step 1 remainder — §H.5 RED matrix + §H.6(b) checklist ticks.
- **Landed:** 6 matrix fixtures (`m5_p3_mapshape_*.ynz`) + 6 integration tests (RED lock);
  audit-map-callsites.md scalar rows ticked with fixture cites, 3 iteration rows left open
  with bug dossier; handoff-phase-3.md rewritten in place (resume-at `phase-3/step-1-mapiter-fix`).
- **Found:** REAL miscompile in the seg-3 cut — both map for-loop arms read `entry.value`
  wrong (scalar garbage / shape SIGSEGV); scalar map ops all proven correct. Matrix 1 green /
  5 RED (plan-prescribed gate); 501 pre-existing tests green; workspace builds.
- **Deviations surfaced:** (a) spec'd post-resume `.get()` cell not constructible (typeck
  Check 2b post-wait maybe over-approximation, pre-existing — cell restructured, coverage
  preserved); (b) driver embeds libynz_runtime.a at build time — stale-.a ABI skew mimics
  miscompiles (landmine documented in handoff); (c) pre-cut per-cell record unreliable due
  to (b) — cut-tree record is binding.
- **Verdict:** STATUS: PARTIAL — checkpoint, executor's early-checkpoint call past the
  calibration threshold; green-BUILDING tree (the 5 REDs are the step-1 documented RED lock).

### 2026-07-03 — Phase 4, segment 1 — session-id: phase4-executor-2026-07-03-m5

- **Scope:** Phase 4 dispatch — recon-first pass (plan-mandated) ahead of step 1.
- **Landed:** zero tree changes (reads only). Full CCIR-1 anchor re-verify against the
  worktree's own state @ `7d66012` with receipts; implementation-grain design for steps 1–2
  (soa.rs module shape, decline-reason enum + precedence order, salsa query idiom, layout
  authority + D11 resolution, the complete 5-point re-threading table) and the steps 3–5
  fixture/test plan (8 fixtures + analysis-test surface) — all banked in `handoff-phase-4.md`
  so segment 2 writes with zero recon.
- **Found:** (a) the plan's two named padding consumers (emit.rs:1051, codegen/queries.rs:204)
  resolve at drifted lines (1057, 208-212) AND a THIRD consumer exists — emit.rs:16848
  (`cg.typed.cross_thread_padded_shapes.contains`) — the re-threading must cover all three or
  the E3 grep gate fails; (b) the conductor-flagged RETIRED cite (emit.rs:13104-13107) confirmed
  absent — not re-anchored; (c) S2 finding 5's param-array posture decided: recorded
  `Declined(Escapes)` rows, never silent absence (P8 enumeration needs the rows); (d) typeck-side
  `FieldSegment` carries order/names only — byte offsets stay codegen-side on `shape_abi_sizes`
  (no ABI-size twin, CCIR-2).
- **Verdict:** STATUS: PARTIAL — context-budget nudge fired at ~155k mid-recon, before step 1's
  first write; starting the multi-file analysis+re-threading cut on a spent window risks the
  forbidden mid-step strand (the Phase 3 seg-1 precedent). Checkpointed at the step boundary on
  a clean tree; resume-at `phase-4/step-1` (fresh pointer — first Phase 4 segment, no stall).

## Context-segment log

### 2026-07-03 — Phase 0, segment 1
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#0-segment-1
- segment: 1
- session-id: phase0-executor-2026-07-03-m5
- subagent_tokens: 231454
- checkpoint reason: planned mark (the planner-authored **CHECKPOINT** after Phase 0 step 1 / S1)
- resume-at: phase-0/step-2
- verdict: STATUS: PARTIAL (S1 GREEN; steps 2-5 remain)

### 2026-07-03 — Phase 0, segment 2
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#0-segment-2
- segment: 2
- session-id: phase0-executor-2026-07-03-m5-seg2
- subagent_tokens: 223161
- checkpoint reason: n/a — final segment (steps 2-5 completed; no further checkpoint taken)
- resume-at: n/a — phase complete
- verdict: STATUS: DONE (S2 GREEN; S3 + both audits + both baselines on disk; handoff deleted)

### 2026-07-03 — Phase 2, segment 1
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#2-segment-1
- segment: 1
- session-id: phase2-executor-2026-07-03-m5
- subagent_tokens: 205105
- checkpoint reason: executor's own early-checkpoint judgment call — step-1→step-2 boundary, green
  tree, ahead of the planner-authored mark (which sits after step 3); rationale: never start the
  atomic ABI cut on a spent context
- resume-at: phase-2/step-2
- verdict: STATUS: PARTIAL (step 0 + step 1 done: FRAGO 007 CCIR-1 edit, 11-fixture RED matrix
  7-green/4-red compiler-verified; steps 2-5 remain; deviations — bool-array base miscompile +
  contains-value-equality semantics — held for the PHASE-boundary review, not judged mid-phase)

### 2026-07-03 — Phase 2, segment 2
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#2-segment-2
- segment: 2
- session-id: phase2-executor-2026-07-03-m5-seg2
- subagent_tokens: 359339
- checkpoint reason: n/a — final segment (steps 2-5 completed; the planner-authored mark after
  step 3 was passed on a green tree with context to spare, so no further checkpoint taken)
- resume-at: n/a — phase complete
- verdict: STATUS: DONE (atomic cut landed; matrix 11/11; suite 481/481; grep gates PASS;
  checklist 100%; dual-mode clean; E8 buffer-visibility deviation surfaced for the P3 parity
  gate; handoff-phase-2.md deleted)

### 2026-07-03 — Phase 2, fix-round-2 segment 1
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#2-fixround2-segment-1
- segment: 1 (of fix round 2)
- session-id: phase2-fixround2-executor-2026-07-03-m5
- subagent_tokens: 206759
- checkpoint reason: executor early-checkpoint past the calibration threshold, green-building tree
- resume-at: phase-2/fix-round-2/step-2
- verdict: STATUS: PARTIAL (step 1 done: store_field Shape/Maybe persist arms + map_value_to_stable_bits
  choke point at ALL FOUR map insert sites — 2 blockers + 2 probe-confirmed siblings — counted heap
  cells, 8 tripwires RED→GREEN; step 2 remains: full-suite gates, snapshot churn review, dual-mode
  re-run, FRAGO 010 paperwork ×3, bookkeeping. Deviations D-r2-1/2/3 held for round re-verify)

### 2026-07-03 — Phase 2, fix-round-3 segment 1
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#2-fixround3-segment-1
- segment: 1 (of fix round 3)
- session-id: phase2-fixround3-executor-2026-07-03-m5
- subagent_tokens: 207877
- checkpoint reason: executor checkpoint at the code-landed/gates-pending seam (context budget;
  round-2 seg-1's pattern)
- resume-at: phase-2/fix-round-3/step-2
- verdict: STATUS: PARTIAL (B1+B2 fixed — value_to_stable_bits generalized across array+map
  persist surfaces, maybe_to_heap_cell spawn arm + BgArgFreeKind::HeapMaybeEnv; byte-exact
  RED→GREEN on both new tripwires, 4 round-2 maybe tripwires re-green. Union arm correctly
  BLOCKED-class: non-uniform repr (null-ptr vs tagged-struct), KNOWN HOLE documented, no partial
  arm shipped. Step 2 remains: gates gauntlet, FRAGO 011 paperwork ×3, contract-comment fix,
  dual-mode oracle, STATUS block)

### 2026-07-03 — Phase 3, segment 1
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#3-segment-1
- segment: 1
- session-id: phase3-executor-2026-07-03-m5
- subagent_tokens: 220641
- checkpoint reason: executor early-checkpoint judgment call — context budget nudged during recon,
  before step 1's atomic map ABI cut began; stopped at the step boundary on a green-building tree
  rather than starting a hard-cut that couldn't finish in-window (early relative to the planner's
  post-step-2 mark, within the mandatory-checkpoint discretion for a scale=large phase)
- resume-at: phase-3/step-1
- verdict: STATUS: PARTIAL (step-0 item 3 done — stack-accurate number-pointer wording, doc-only;
  full CCIR-1 recon re-verify persisted in handoff-phase-3.md §A-§F — map-cut design pinned to
  mirror the array ABI 1:1, guard-lift inventory re-anchored to current lines. Steps 1-5 + step-0
  items 1-2 remain. No commit — PARTIAL rides uncommitted)

### 2026-07-03 — Phase 3, segment 2
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#3-segment-2
- segment: 2
- session-id: phase3-executor-2026-07-03-m5-seg2
- subagent_tokens: ~168000 (harness estimate at checkpoint)
- checkpoint reason: context nudge fired at ~153k mid-recon, BEFORE the atomic step-1 map ABI
  cut began; remaining headroom (~60-70k) is below the cut's realistic cost (runtime rewrite +
  decls + choke points + 8 site migrations + matrix + Docker build/test iterations) — starting
  it risked the forbidden mid-step broken-tree strand. Stopped at the same step boundary as
  segment 1, but the boundary is NOT a stall: segment 2 advanced step 1 from ABI-grain design
  to IMPLEMENTATION grain (handoff §H — exact runtime signatures, choke-helper API, per-site
  migration table with confirmed conventions, 6 fixture sources fully specified with expected
  stdout). Segment 3 can start the cut with zero recon.
- resume-at: phase-3/step-1
- verdict: STATUS: PARTIAL (zero tree changes this segment — reads only + handoff/audit
  bookkeeping; anchors re-verified per FRAGO 007 with receipts in handoff §H.0; no commit —
  PARTIAL rides uncommitted)

### 2026-07-03 — Phase 3, segment 3
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#3-segment-3
- segment: 3
- session-id: phase3-executor-2026-07-03-m5-seg3
- subagent_tokens: 243479
- checkpoint reason: context nudge fired mid-migration (~150k+); executor finished the in-flight
  migration to a coherent, fully-buildable, suite-green tree rather than stranding mid-cut, then
  stopped at an honest sub-boundary within step 1
- resume-at: phase-3/step-1-redmatrix-remaining (ADVANCED from segments 1/2's `phase-3/step-1` —
  the conductor's stall detector fired on segments 1→2 sharing that identical pointer; Patrick
  ordered "stop re-verifying, start writing"; this segment's pointer move confirms the stall broke)
- verdict: STATUS: PARTIAL (map runtime ABI cut landed in lib.rs, choke points wired in emit.rs
  reusing the array helpers directly (no second derivation), all 8 call-site groups hard-cut
  migrated, grep gate H.6(a) passing, 13 golden IR snapshots refreshed + verified pure decl-churn,
  alloc re-pin done with Paper-Trace (10-cell residual = pre-existing 2-map×5-buffer visibility,
  not a new leak), full workspace suite GREEN (501/501). Remaining: §H.5's 6 RED-matrix fixtures,
  §H.6(b) checklist ticks, then steps 2-5. No commit — step 1 incomplete, rides uncommitted)

### 2026-07-03 — Phase 3, segment 4
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#3-segment-4
- segment: 4
- session-id: phase3-executor-2026-07-03-m5-seg4
- subagent_tokens: 232045
- checkpoint reason: executor early-checkpoint, context past the calibration threshold, at an
  honest sub-boundary within step 1 (not a repeat of segment 3's pointer)
- resume-at: phase-3/step-1-mapiter-fix (ADVANCED from segment 3's
  `phase-3/step-1-redmatrix-remaining` — legible forward progress, no stall)
- verdict: STATUS: PARTIAL (§H.5's 6 RED-matrix fixtures authored + landed
  (m5_p3_mapshape_{literal_str,runtime_int,iter_escape,wait_cross,wait_iter,wait_get_escape});
  integration.rs section appended, 1 passed/5 failed as PLAN-PRESCRIBED RED lock (step 1's own
  text calls for a RED matrix gating the build); §H.6(b) scalar-row checklist ticks done, 3
  iteration rows deliberately left open with the bug dossier inline. **Headline finding: the
  matrix caught a REAL pre-existing miscompile** — both map for-loop arms (emit.rs
  :12697-12806 non-SM, :6564-6650 SM) read `entry.value` wrong (uninit-stack garbage for
  scalars, SIGSEGV for shapes) on the verified-coherent cut tree; every scalar non-iteration
  map op proven correct end-to-end. The pre-existing 501-green suite never asserted iterated
  map values — a coverage hole the matrix now closes. Diagnosis dossier + eliminated suspects
  recorded in handoff (build-order landmine flagged: driver embeds libynz_runtime.a at
  build.rs:27-35, NOT a cargo dep — a stale .a produces ABI-skew segfaults indistinguishable
  from real miscompiles, cost 2 false leads). Deviations surfaced for deviation-judge: spec'd
  `wait_cross` cell not constructible (typeck's crossing over-approximation rejects ANY maybe
  read after a wait, cut or no cut — cell restructured, not a FRAGO); nothing else. No commit —
  step 1 incomplete, rides uncommitted)

### 2026-07-03 — Phase 3, segment 5
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#3-segment-5
- segment: 5
- session-id: phase3-executor-2026-07-03-m5-seg5
- subagent_tokens: 187105
- checkpoint reason: context crossed ~150k exactly at the step-1→step-2 boundary; step 2 is a
  multi-file typeck/registry/docs/gallery lift that shouldn't start unless it can finish — early
  seam on a marked phase, step boundary, fully green tree
- resume-at: phase-3/step-2 (ADVANCED from segment 4's `phase-3/step-1-mapiter-fix` — STEP 1 IS
  NOW FULLY COMPLETE, no stall)
- verdict: STATUS: PARTIAL (root cause found + fixed with a Paper-Trace: MapEntry local-slot
  indirection-level mismatch — `entry_var_slot` registered the struct alloca itself in both loop
  arms (emit.rs :12779 non-SM, :6586 SM) while every consumer (load/store/i64_bits_to/
  materialize_param) expected a POINTER to the entry struct; `entry.value` therefore read 8 bytes
  past the key cstring. Fixed by pointer-indirect slot registration matching the canonical
  materialize_param pattern in both arms — CCIR-2 clean, threads the existing contract, zero new
  derivations, zero runtime changes. RED matrix now 7/7 green (6 §H.5 cells + 1 new debug-repr
  lock, m5_p3_map_embed_repr.ynz, proving the debug-repr walker never shared the bug). Map grep
  gate re-verified 9/9 confined to the choke-point section. Full workspace suite: 2235/0 failed
  (integration 508/508). §H.6(b) checklist now FULLY ticked. STEP 1 EXIT CRITERIA MET. Surfaced
  for boundary reviewers, not chased: a load-flake on an unrelated M3e fixture (fails once under
  full-suite load, passes isolated + 2 reruns, no maps/for-loops in the fixture — unreachable
  from this fix); Patrick's two review footnotes reconfirmed still open (leak-parity honesty +
  zero golden-IR-snapshot coverage — the new debug-repr test is a runtime lock, not the missing
  golden snapshot). No commit — steps 2-5 remain, rides uncommitted)

### 2026-07-03 — Phase 3, segment 6
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#3-segment-6
- segment: 6
- session-id: phase3-executor-2026-07-03-m5-seg6
- subagent_tokens: 200984
- checkpoint reason: context past calibration threshold at a clean step-3-done boundary
- resume-at: phase-3/step-4 (ADVANCED from segment 5's `phase-3/step-2` — steps 2 AND 3 both
  closed this segment, no stall)
- verdict: STATUS: PARTIAL (Step 2 COMPLETE: `ArrayShapeRuntimeFieldWithWait` guard lifted —
  find_let_initializer_in_stmts had exactly one consumer, deleted with it; check.rs Check-2d
  diagnostic + finder + promotion-decline arm removed; registry deferral entry retired +
  design_future_sync SKIP entry added (M3e precedent); VSCode grammar regenerated to match;
  IMP-concurrency.md rewritten interim-guard -> LIFTED; queries.rs decline test inverted to a
  promotes-and-compiles-clean test, passes; gallery trigger removed (no v0_3_m3a gallery test
  exists to update -- recon-drift deviation surfaced, risk-neutral no-op). Step 3 COMPLETE:
  3 former guard-rejection fixtures repurposed via git mv into crossing-wait acceptance tests,
  all green with exact stdout; array constructibility independently verified NOT to share
  segment 4's map maybe-after-wait limitation; maybe-crossing obligations recorded note-only.
  Full suite: 2236/0 failed (integration 508/508; +1 vs seg-5 is aggregation-method sensitivity,
  noted for reviewers, not a regression). Housekeeping: stray untracked ELF binary from seg-5's
  coherence check deleted. Carried forward untouched: Patrick's two footnotes, the M3e
  load-flake note. No commit -- steps 4-5 remain, rides uncommitted)

### 2026-07-03 — Phase 3, segment 7
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#3-segment-7
- segment: 7
- session-id: phase3-executor-2026-07-03-m5-seg7
- subagent_tokens: 228432
- checkpoint reason: context budget at the step-4->step-5 boundary, a legal early seam on this
  checkpoint-marked phase
- resume-at: phase-3/step-5 (ADVANCED from segment 6's `phase-3/step-4` -- step 4 fully closed,
  step 5 not yet started, no stall)
- verdict: STATUS: PARTIAL (Step 4 E8 parity gate COMPLETE + GREEN: FRAGO-005 visibility
  criterion pinned with 2 fail-loud tests (arrays 0->2, maps 0->5, vs the P0 alloc=0 baseline);
  FRAGO-009 semantics gate pinned EXACT (alloc,free)==(11,0) with a pre-written Paper-Trace
  matching zero residual (+2 array buffers, +5 map buffers, +4 FRAGO-011 persist cells, +0 over
  a 40-iteration read loop, +0 across 3 map shape-value sets incl. an overwrite). Verified,
  not assumed: segment 5's map-iteration fix changed zero alloc counts (all 72 v03_m3d tests
  green); the map ABI cut REMOVED a persist-cell gap rather than adding one (map shape values
  store inline, lib.rs:585-590 -- the re-set-over-key leak FRAGO 011 flagged for maps is
  structurally absent). Recorded verdict (for deviation-judge): parity GREEN in the "no NEW
  leak class" sense -> no drop insertion, no D6 fallback, P2 helper's alloc==free+gap encoding
  RATIFIED as durable until FR #6's drop story. Housekeeping: `cargo fmt --all` applied
  (pre-existing non-M5 formatting drift in emit.rs/integration.rs/lib.rs, none in this
  segment's own diff regions) -- surfaced, not silently absorbed. Step 5 NOT started; recon
  banked in handoff (twin-site list for item (c) incl. a third same-class site at emit.rs:14519,
  the bg array<Shape> alias finding, D12/(e) ratification analyses) so segment 8 inherits
  receipts instead of re-deriving. Carried forward untouched: Patrick's two footnotes, the M3e
  load-flake, new clippy-warning notes. No commit -- step 5 remains, rides uncommitted)

### 2026-07-03 — Phase 3, segment 8
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#3-segment-8
- segment: 8
- session-id: phase3-executor-2026-07-03-m5-seg8
- subagent_tokens: 222762
- checkpoint reason: executor early-checkpoint, context past calibration threshold, at an honest
  fine-grained sub-boundary within step 5
- resume-at: phase-3/step-5-item-c-done (ADVANCED from segment 7's `phase-3/step-5` -- real
  code landed, no stall)
- verdict: STATUS: PARTIAL (item (c) FRAGO-010 twin UNIFIED across all 3 sites -- bind_sm_
  result_and_flush shape arm, SM shape-embed Let arm, prepare_bg_arg_for_ctx Shape arm all now
  read shape_abi_sizes via shape_abi_size_const; zero shape-size struct_ty.size_of() sites
  remain; 34/34 goldens, zero refreshes, banked prediction held. Real probe-mandated FIX landed:
  bg x array<Shape> alias bug in prepare_bg_arg_for_ctx (Paper-Trace: observed 119 / expected 30
  / residual +89 = caller's post-spawn mutation visible through an aliased buffer / fixed via
  inline-elem clone extension, verified 30 post-fix). D12 RATIFIED as final for M5: pointer-
  identity for pointer-typed shape fields in shape_value_eq (reasoned: no repr change for
  fields unlike elements, matches locked non-shape-cell goldens, deep equality is an
  out-of-scope semantics extension -- YAGNI, docs home Phase 7 step 5); (e) ratified alongside.
  **PLAN-TEXT FALSIFIED (surfaced, not self-resolved):** step 5(e)'s "no fixture can construct
  a union persisting through the choke points" claim is FALSE -- probes prove map<int,Union>.set
  and array<Union> literals compile+run (write-side raw-pointer persist), read-back ICEs loudly
  for both; D6's conditional loud-reject gate would be a NEW user-facing diagnostic with
  feature-registry/gallery obligations the plan never enumerated -- FRAGO-grade, flagged with
  receipts for the deviation-judge, not built. TWO NEW live MapEntry-escape silent-wrongs found
  by probe (bg-arg reads the advanced slot 2/20; array<MapEntry> escape 20/20) -- one-choke-
  point fix design banked in handoff, not yet landed. New note-only finding: bare .copy() on
  any array aliases (pre-existing P3c-era typeck deferral), carried to reviewers. Remaining for
  segment 9: MapEntry escape fix + tripwires, sweep/pin fixtures (incl. dual-mode give/copy/wait
  matrix), (a)/(e) doc notes, union KNOWN-HOLE doc refresh + loud-fail pins, full suite + dual-
  mode oracle, SEAL. No commit -- rides uncommitted)

### 2026-07-03 — Phase 3, segment 9
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#3-segment-9
- segment: 9
- session-id: phase3-executor-2026-07-03-m5-seg9
- subagent_tokens: 205080
- checkpoint reason: code-landed / fixtures-pending seam, context budget, green-building tree
  verified
- resume-at: phase-3/step-5-mapentry-fix-landed (ADVANCED from segment 8's
  `phase-3/step-5-item-c-done` -- real code landed, no stall)
- verdict: STATUS: PARTIAL (BOTH MapEntry-escape silent-wrongs FIXED per the banked one-choke-
  point design: Type::MapEntry arm added to value_to_stable_bits (16-byte counted heap-cell
  clone + deep value-half copy), unconditional MapEntry pre-gate added to prepare_bg_arg_for_ctx
  (inference-independent, before is_heap_arg). Paper-Trace RED->GREEN: bg-arg probe 2/20 ->
  1/10; array<MapEntry> escape probe 20/20 -> 10/20, zero residual. Doc notes landed: D12
  ratification on shape_value_eq header, (e) ratification on the contains arm, union KNOWN-HOLE
  doc refreshed to the probe-verified truth (write-constructible, read-back ICEs, loud-reject
  gate stays FRAGO-grade -- not built). D12 pin-probe run recorded (true/true/false; cell 2's
  true is LLVM literal-merging, named as an artifact for the eventual fixture WHY). Green-tree
  receipts: build/fmt/clippy clean, goldens 34/34 zero refreshes, m5_p* 44/44, v03_m3d 72/72,
  v03_m3b 87/87. Full suite + dual-mode oracle DEFERRED to the sealing segment. No commit --
  step 5 fixtures + full suite + dual-mode oracle + SEAL remain for segment 10, all designs
  finalized in the handoff so segment 10 needs zero probing)

### 2026-07-03 — Phase 3, segment 10 (FINAL — phase sealed)
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#3-segment-10
- segment: 10
- session-id: phase3-executor-2026-07-03-m5-seg10
- subagent_tokens: 192034
- checkpoint reason: n/a -- final segment, phase completed and sealed
- resume-at: n/a -- phase complete
- verdict: STATUS: DONE. PHASE 3 SEALED at commit `e06172f6bb31a63992328a3d87abac9763a69b64`
  (conductor-verified: trailer `Plan-Phase: 2026-07-03-v0-3-m5-auto-soa#3` present, commit
  exists, working tree clean post-commit). 49 files, +2663/-1197, covers ALL 10 segments'
  accumulated uncommitted work in one boundary commit per FRAGO 004. 7 sweep/pin fixtures
  authored + landed (bg give/copy/wait matrix, MapEntry escape x2, D12 pin, union readback-
  blocked x2), all green first run. Full workspace suite: 2246/0 failed. Dual-mode oracle
  regenerated over all 466 fixtures: 379 identical / 83 skip / 2 documented DIFF (pre-existing,
  unrelated) / 2 timing NONDET / 0 anomalies / 0 real divergences -- step 5's dual-mode promise
  MET. All 5 phase-exit criteria confirmed on receipts (map fix+matrix+grep gate, guard+
  deferral gone, IMP-concurrency updated, parity green, crossing-wait green, sweep green).
  handoff-phase-3.md deleted (segment's last act). Frontmatter session-id chain gap (segs 2-9
  logged in audit.md but not appended to plan.md frontmatter) self-repaired by this segment.
  Full carry-forward list for the Phase-3 boundary reviewers recorded in this segment's return
  (Patrick's 2 footnotes, the union-persist plan-text falsification as a FRAGO candidate, 4
  FRAGO-candidate classifications for deviation-judge, the M3e load-flake, bare-.copy() note,
  error_galleries.rs recon drift, step-4 verdict paperwork, 2 pre-existing warnings, suite-count
  reconciliation, the frontmatter-chain repair itself, the LLVM literal-merging D12 artifact
  note, the SKIP-semantics correction now recorded in p2-dualmode-report.md). Phase 3 CLOSED --
  next: conductor's cheap gates (Step 4) + reviewer fan-out (Step 5) over the sealed commit.

### 2026-07-03 — Phase 4, segment 1
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#4-segment-1
- segment: 1
- session-id: phase4-executor-2026-07-03-m5
- subagent_tokens: 259565
- checkpoint reason: executor's own early-checkpoint judgment call — context-budget nudge fired
  (~155k) before step 1's first write; zero source-tree changes this segment (recon-first pass:
  anchor re-verify, padding-consumer topology census, S2 spike-input consumption, decline-
  precedence design) checkpointed rather than starting the multi-file re-threading cut on a
  spent window (the Phase 3 seg-1 mid-step-strand precedent this plan's own audit records)
- resume-at: phase-4/step-1 (fresh — first Phase 4 segment; no prior pointer to compare)
- verdict: STATUS: PARTIAL (recon complete, zero code landed; step 1 admission-criteria design +
  the 3-consumer re-threading table banked in handoff-phase-4.md for segment 2 to implement)

### 2026-07-03 — Phase 4, segment 2
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#4-segment-2
- segment: 2
- session-id: phase4-executor-2026-07-03-m5-seg2
- subagent_tokens: 270297
- checkpoint reason: executor's own early-checkpoint judgment call — context nudge fired (~158k)
  right as step 1's at-code-time verification completed; stopped at the step-1/step-2 boundary
  rather than starting step 2's multi-signature re-threading cut (emit_artifact → build_module →
  3 lowering fns → 3 Cg constructors + full-suite/golden-IR gates) below its realistic token cost
- resume-at: phase-4/step-2 (advanced from segment 1's phase-4/step-1 — real progress, not a stall)
- verdict: STATUS: PARTIAL (step 1 DONE + green: soa.rs module, soa_candidate_query,
  lib.rs wiring — build/clippy/fmt/test all clean; segment also corrected an inherited
  segment-1 receipt on the record — 3 Cg constructors, not 9; steps 2-5 remain, fully
  designed in handoff for segment 3)

### 2026-07-03 — Phase 4, segment 3
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#4-segment-3
- segment: 3
- session-id: phase4-executor-2026-07-03-m5-seg3
- subagent_tokens: 237951
- checkpoint reason: the plan's own planner-authored CHECKPOINT mark after step 2, reinforced by
  the context nudge — stopped at a green-building tree at the step-2/step-3 boundary
- resume-at: phase-4/step-3 (advanced from segment 2's phase-4/step-2)
- verdict: STATUS: PARTIAL (step 2 DONE + green: layout authority + all 3 padding consumers
  re-threaded; full suite 2251/0 failed, zero golden-IR churn — re-threading proven byte-identical;
  grep gates pass; steps 3-5 remain, fully designed in handoff for segment 4)

### 2026-07-03 — Phase 4, segment 4
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#4-segment-4
- segment: 4
- session-id: phase4-executor-2026-07-03-m5-seg4
- subagent_tokens: 224494
- checkpoint reason: n/a — final segment (steps 3-5 completed; handoff-phase-4.md deleted as the
  closing act; no further checkpoint taken)
- resume-at: n/a — phase complete
- verdict: STATUS: DONE (8 fixtures incl. both-candidate D11 proof, 12-test soa_analysis.rs
  exit-criterion suite + 1 integration byte-layout test, all green first run; full suite
  2264/0 = 2251 baseline + exactly 13 new tests; grep gates pass — zero codegen reads of
  LayoutDecisions::arrays, zero second env-parse, zero second padded-set derivation, zero
  ABI-size re-derivation on typeck side)

### 2026-07-04 — Phase 5, segment 1
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#5-segment-1
- segment: 1
- session-id: phase5-executor-2026-07-03-m5
- subagent_tokens: 251745
- checkpoint reason: executor's own early-checkpoint judgment call — step-1 boundary, green tree,
  ahead of the planner-authored mark (which sits after step 2); orientation + full design ("design
  c") paid once and recorded in handoff-phase-5.md for segment 2 to consume without re-deriving
- resume-at: phase-5/step-1
- verdict: STATUS: PARTIAL (step 1's runtime primitive landed green — `ynz_array_new_sized`,
  counted allocs, same header/drop; steps 1-remainder through 5 remain; deviation surfaced —
  array `.copy()` lowers as an alias no-op for ALL arrays today, not a deep copy as the plan/
  soa.rs comment assumed — routed to deviation-judge before the step-4 `.copy()` cell, not
  self-decided)

### 2026-07-04 — Phase 5, segment 2
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#5-segment-2
- segment: 2
- session-id: phase5-executor-2026-07-03-m5-seg2
- subagent_tokens: 244301
- checkpoint reason: executor's own early-checkpoint judgment call — most of the window went to
  implementation-depth re-grounding (re-verifying every consumer-site receipt against the live
  tree at code-shape granularity, not just line numbers) plus FRAGO 014's plan-text application
  and the step-3 belt assert; construction/access lowering (step 1 remainder/step 2, the heavy
  work) not yet started
- resume-at: phase-5/step-1-seg2-frago014-and-belt-gate-landed (advanced from segment 1's bare
  phase-5/step-1 via the fine-grained sub-marker convention — stall detector does not fire)
- verdict: STATUS: PARTIAL (FRAGO 014 plan-text correction applied; step-3 belt assert landed
  green — no `LayoutKind::Soa` decision may arrive under `no_auto_parallel`, hard `Err`, consumes
  the one threaded predicate; cross_impl_consistency 2/2 passed under YNZ_NO_AUTO_PARALLEL=1;
  handoff hardened to code-shape granularity for segment 3; new hazard surfaced — `.copy()` in
  background-arg position will double-copy/leak once FRAGO 014's deep-copy lands, held for
  step-4 verification, not self-fixed)

### 2026-07-04 — Phase 5, segment 3
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#5-segment-3
- segment: 3
- session-id: phase5-executor-2026-07-03-m5-seg3
- subagent_tokens: 240307
- checkpoint reason: the plan's own planner-authored CHECKPOINT mark after step 2 — construction
  + access lowering green on the qualifying fixture, release-mode IR inspected and evidence
  persisted (`p5-ir-evidence.md`), then step 3's remaining half also landed before the window ran
  out on step 4/5
- resume-at: phase-5/step-4 (segment sub-marker `phase-5/step-2-construction-access-green-
  checkpoint-evidence-persisted` — advanced from segment 2's step-1 pointer; real progress, stall
  detector does not fire)
- verdict: STATUS: PARTIAL (steps 1-3 DONE and green: construction lowering — one segmented
  buffer via `ynz_array_new_sized`, compile-time offsets, scatter at construction; access
  lowering — `SoaArrayInfo`/`SoaSegment` threaded through all choke helpers incl. the
  debug-print site the build itself forced into the open per E6 totality; dual-mode oracle now
  genuinely exercises AoS-vs-SoA divergence. Receipts: cross_impl_consistency 2/2 passed with SoA
  live; qualifying fixture byte-identical across modes with SoA actually firing (438 SoA IR
  instructions, not a silent decline); opt-18 -O2 IR shows SROA-eliminated gather buffer, hot-loop
  loads = exactly the 2 used-field segment loads, contiguous stride-8; E7 + E3 grep gates clean.
  Full cargo test -p ynz-driver launched but did not finish in-window — NOT proven, deferred to
  step 5 as its own obligation, not silently assumed green. Steps 4-5 remain: `.copy()` fix
  (FRAGO 014) + the background-arg double-copy hazard (carried from seg 2, still unresolved) +
  E9 matrix + full-suite cross-impl run)

### 2026-07-04 — Phase 5, segment 4
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#5-segment-4
- segment: 4
- session-id: phase5-executor-2026-07-03-m5-seg4
- subagent_tokens: 215446
- checkpoint reason: executor's own early-checkpoint judgment call — step 4a/4b landed and
  verified green, window ran out before step 4c (E9 matrix) / step 5 (full-suite run)
- resume-at: phase-5/step-4 (segment sub-marker `phase-5/step-4-copy-fixed-bgarg-hazard-resolved`
  — advanced from segment 3's step-2 marker; real progress, stall detector does not fire)
- verdict: STATUS: PARTIAL (FRAGO 014's `.copy()` fix landed both modes — AoS via
  `ynz_array_clone_primitive`, SoA via new `soa_copy_to_aos` gather-to-fresh-AoS-buffer helper
  (the copy's binding is authority-declined so all its reads correctly lower AoS, no layout
  re-derivation); the carried background-arg hazard confirmed a GENUINE LEAK by alloc-counter
  receipt (11 alloc/5 free with the hazard live vs 9/5 baseline) and fixed at the
  `prepare_bg_arg_for_ctx` choke point (ownership transfer instead of re-clone) — restored to
  exactly the 9/5 baseline. Targeted tests green (bg-array dual-mode 2/2, E8 exact-count parity
  pin unchanged, explicit-copy-honored); E7/E3 grep gates clean. New minor deviation surfaced —
  `fixed<T>.copy()` is ALSO a pre-existing alias no-op, out of FRAGO 014's array-only scope, not
  fixed here, held for a future ruling (durable-deferral candidate at phase close, non-blocking).
  Step 4c (E9 matrix) and step 5 (full-suite cross-impl run + exit-criteria re-confirm) remain,
  fully designed in the handoff for segment 5)

### 2026-07-04 — Phase 5, segment 5
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#5-segment-5
- segment: 5
- session-id: phase5-executor-2026-07-03-m5-seg5
- subagent_tokens: 247712
- checkpoint reason: n/a — final segment (steps 4c-5 completed; handoff-phase-5.md deleted as the
  closing act; no further checkpoint taken)
- resume-at: n/a — phase complete
- verdict: STATUS: DONE (E9 matrix 3/3 green incl. a live design discovery — D11's shape-level
  padding-wins conservatism forfeits SoA on a spawn-site `.copy()` bg-arg for ALL of that shape's
  arrays, ratified as working-as-designed not a contradiction, fixture redesigned around it
  rather than a CCIR; both-candidate proven end-to-end through codegen now, not just analysis;
  SM-path question answered definitively — the SoA construction interception is shared by every
  `Stmt::Let` path, no SM-specific gap; full workspace suite 2271/0 failed; clippy/fmt clean;
  dual-mode byte-identical across the whole driver-fixtures + examples corpus; E3 0 hits, E7 all
  refs choke-section-gated; 13 declaration-only snapshot refreshes investigated and accepted, zero
  `*.snap.new` remain). 4 unfixed should-fix/minor findings enumerated for the boundary review,
  not self-filed: `fixed<T>.copy()` still aliases (out of FRAGO 014 scope); the new shape-level
  padding-forfeits-SoA-on-spawn-copy conservatism; TodoWrite/task-store never granted any segment;
  pointer-cell one-level `.copy()` semantics noted as deliberate (D12/D13), not missed.

### 2026-07-04 — Phase 6, segment 1
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#6-segment-1
- segment: 1
- session-id: phase6-executor-2026-07-03-m5
- subagent_tokens: 247577
- checkpoint reason: executor's own early-checkpoint judgment call — context-budget nudges fired
  from ~152K before step 1's first write; zero source-tree changes this segment (recon-first pass:
  anchor re-verify, loop-semantics probes, D8 design, and the SIGSEGV bisection) checkpointed
  rather than starting the harness-authoring work on a spent window
- resume-at: phase-6/step-1 (fresh — first Phase 6 segment; no prior pointer to compare)
- verdict: STATUS: PARTIAL (recon + design complete, zero code landed; step-1 design (YNZ_SOA_FORCE
  wiring, D8 semantics, the read-accumulate x/y workload choice) banked in handoff-phase-6.md for
  segment 2 to implement. 2 deviations surfaced: (1) pre-existing O0 stack-growth SIGSEGV past
  ~4.19M total loop-visits in both layout modes, routed via deviation-judge → FRAGO 015 (E13 risk
  row + Phase 8 precondition + FR #13 + roadmap ledger row, applied by segment 2); (2) step-5
  pre-registration that the measured SoA win may be near-zero since shipped builds never run the
  LLVM -O2 pipeline the IR-evidence claim assumed — recorded as a theory for the harness to verify,
  not chased further this segment)

## FRAGO log

## FRAGO 001 — 2026-07-03 — session-id: plan-producer-2026-07-03-m5
Base:      2026-07-03-v0-3-m5-auto-soa @ pre-approval (stub), pre-execution
Trigger:   Plan-reviewer verdict — sound + incomplete: 0 blockers, 1 MAJOR, 2 MINOR, 1 optional.
           MAJOR — the `map<K,Shape>` symmetric by-value fix (Phase 3 step 1, scratch-doc "risk 4",
           pre-existing base bug) was un-scored and un-audited; E7's scope + grep-gate proof are
           array-only (`ynz_array_*`) and do not extend to `ynz_map_*`. Conductor pre-ran the frozen
           matrix on the new row.
Changes:
  - ¶1 Risk Assessment: ADDED **E12** (`map<K,Shape>` symmetric missed-call-site silent-miscompile),
    scored on the frozen engine — initial A×II=EH; mitigations B1 (P0 `ynz_map_*` exhaustive audit +
    hard-cut/single-choke-point ABI + grep gate, prob −2) + B2 (RED `map<K,Shape>` matrix gating
    build, prob −1) → residual D×II = **MEDIUM, recorded**. Structural call: SEPARATE row, not an E7
    extension, so the array-only grep gate stays honest. No HIGH residual introduced.
  - ¶3.3 Phase 0 step 4: ADDED the `ynz_map_*` exhaustive call-site audit as a SECOND committed
    checklist feeding **Phase 3's** entry criteria (not P2's); exit criteria now name both checklists.
  - ¶3.3 Phase 3 step 1: CHANGED to require the P0 map-audit checklist as entry criterion, the same
    hard-cut/single-choke-point ABI discipline as arrays, a RED map matrix fixture, and a `ynz_map_*`
    grep gate; exit criteria + reviewer fan-out + Model tag (→ large) updated.
  - Invariants ¶Safety: ADDED the `map<K,Shape>` correctness assertion + extended the audited-call-site
    coverage claim to `ynz_map_*`.
  - ¶3.3 Phase 1 step 1 (MINOR 1): ADDED roadmap.md:121 to the roadmap-edit list — reassign the
    `array-using-soa-layout` lint from M4 to M5 (features.toml confirms M4 shipped only
    `cross-thread-fields-not-padded` + `prefer-yielding-sleep`).
  - Recorded Decision D9 (MINOR 2): ADDED an explicit UNVERIFIED behavior-claim caveat on
    `jargon_audit.rs`'s scope + a Phase 7 step 1 re-verify obligation (per plan-invariants Design-Doc
    Alignment(4) — no recon cite existed for the identifier-vs-text scoping claim).
  - Recorded Decision D11 (optional, taken): ADDED — formalizes the Phase 4 "padding wins, SoA
    declines for cross-thread-padded shapes" precedence as a visible D entry; Phase 4 step 2 + End
    State outcome 5 now cite D11.
Unchanged: everything not listed (Phases 2, 5, 6, 8; the E1–E11 rows; Design-Doc Alignment; Sustainment;
           Command & Signal; Future Requirements except FR #11's existing E-row list).
Override:  none — no residual rose to HIGH; no override block required.

## FRAGO 002 — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-p0-gate-exception
Base:      2026-07-03-v0-3-m5-auto-soa @ pre-Phase-0, ¶3.4 EXECUTION GATE (hard) armed — "no phase
           dispatches until the v0.3.0 tag exists (A7). The conductor checks this before Phase 0."
Trigger:   Patrick, live in the conductor session 2026-07-03 (heading to bed, wants overnight
           progress), asked to start in an isolated git worktree given "no overlap (the lint phase
           shipped)." Conductor verified before acting (never took the claim on faith): `git tag -l`
           confirms no `v0.3.0` tag exists; M4 (`2026-07-02-v0-3-m4-channels-arc-release`) is still
           `status: active`, with Phase 4 (the `[[lint_rule]]` mechanism + false-sharing padding —
           the substrate this plan's A1/E1 actually worry about) already committed to `main`
           (1ac52fd), but Phase 5 (teaching surface) and Phase 6 (v0.3.0 release fold — the tag cut
           itself) still open. Conductor traced M5's own Phase 0 + Phase 1 step text: both operate
           exclusively on `YnzArray`/`ynz_array_*`/`ynz_map_*` array-storage code and SSOT
           roadmap/scratch-doc cross-references — zero read or write dependency on
           `false_sharing.rs`, the `[[lint_rule]]` mechanism, or any file M4's open Phase 5/6 touches.
           The gate's actual protected assumptions — A1 (M4 P4 substrate ships "essentially as
           recon'd") and E1 (twin-substrate collision, already resolved by this plan's fold-decision)
           — are not exercised by Phase 0 or Phase 1. Phase 4 (SoA candidate analysis) is the first
           phase that genuinely reads M4's substrate and remains fully gated.
Changes:
  - ¶3.4 Coordinating Instructions EXECUTION GATE: narrowed for THIS session only via this FRAGO —
    Phase 0 + Phase 1 may dispatch ahead of the v0.3.0 tag, isolated in git worktree
    `../ynz-m5-worktree` on branch `feat/v0-3-m5-auto-soa` (created from `main`@1ac52fd, the same
    commit M4 Phase 4 landed on). Phase 2 onward remains hard-gated on the v0.3.0 tag exactly as
    written — UNCHANGED. (The corresponding ¶3.4 plan-body note is applied by the re-dispatched
    executor per agent-charter-discipline — this FRAGO record is the conductor's own append, not a
    plan-body edit.)
  - Conductor standing constraint for this session (not a plan-body change, a conductor operating
    rule): will NOT self-sign any risk-raising FRAGO (no HIGH residual is ever self-signed — never)
    and will NOT touch the completion-approval gate while Patrick is offline. Execution halts at the
    first risk-raising FRAGO, at Phase 0's own built-in STOP conditions (S1/S2 RED → BLOCKED per
    plan-spike-discipline), or at the end of Phase 1, whichever comes first — Phase 2 (the hard-cut
    by-value ABI rewrite, `(coding, high, large)`, mandatory checkpoints, E7 EX-HIGH-initial risk)
    does not dispatch unattended regardless of worktree isolation.
Unchanged: everything else — the gate's Phase 2+ scope, the risk table (E1–E12), A7, all other
           phases.
Override:  Patrick, live chat, 2026-07-03 — explicit real-time authorization, scoped narrowly to
           Phase 0+1, worktree-isolated. Not scored as a HIGH residual: no new irreversible or
           destructive operation is introduced (Phase 0's spikes are throwaway-by-design per
           plan-spike-discipline.md; Phase 1 is SSOT-doc-only); recorded as a conductor-logged,
           human-authorized gate exception rather than a signed HIGH override.

## FRAGO 003 — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-p0-gate-exception (continued)
Base:      2026-07-03-v0-3-m5-auto-soa @ FRAGO 002 (Phase 0+1 scoped gate exception, worktree-isolated,
           Opus 4.8 dispatch)
Trigger:   Patrick, live chat, two follow-up asks after FRAGO 002 landed: (1) widen the scoped
           exception to cover Phase 2 (by-value hard-cut ABI migration) and Phase 3 (`map<K,Shape>`
           fix + guard lift + suspension sweep) as well — reasoning: only Phase 4 (SoA candidate
           analysis) actually reads M4's still-open substrate; halt only on a genuine risk-raising
           FRAGO (never self-signed) or on reaching Phase 4's own tag-gated boundary, whichever comes
           first — not a blanket conductor refusal past Phase 1. (2) Dispatch phase executors on
           Fable 5, not Opus 4.8.
           Conductor's own miss surfaced by ask (2): **D10** (plan.md:254-258, plan.md:789-790) is a
           pre-existing Recorded Decision from plan-authoring time — "Executor model dispatch: Fable
           5 (`model: fable`) for all phase executors at `/execute-plan` time — Patrick 2026-07-03, an
           explicit availability-override of the frozen binding's excluded-models list (Fable
           returned). Reviewer fleet stays per the frozen model-selection binding." The conductor ran
           `/select-model` fresh for the Phase 0 dispatch WITHOUT first reading ¶1's Recorded
           Decisions section and got Opus 4.8 — a real process miss, not new information tonight.
           Fable's availability is independently corroborated: it is a live model option in the
           conductor's own dispatch tooling right now, consistent with D10's "Fable returned" claim;
           the frozen `REF-model-selection.md` (calibrated 2026-06-30) has not caught up — a known,
           named staleness gap (§6 lists "Fable returning" as one of exactly two headline
           re-derivation triggers), not a hallucinated model.
           A second, independent conductor miss surfaced in the same review: the killed Phase 0
           dispatch also carried a redundant `isolation: worktree` flag on top of the manually-created
           `../ynz-m5-worktree` — that would have spun up a THIRD, unrelated checkout off `main`
           instead of using the branch already built for this exception. Corrected on re-dispatch:
           no isolation flag, absolute paths into the existing worktree only.
Conductor verification before widening: re-traced Phase 2
           (`crates/ynz-codegen/src/emit.rs` YnzArray ABI migration + call-site migration; DELETE old
           entry points) and Phase 3 (`map<K,Shape>` fix; `ArrayShapeRuntimeFieldWithWait` guard lift;
           `wait`/`background` suspension sweep on the by-value substrate) — both operate on
           array/map storage plus the EXISTING suspension mechanism (`wait`/`background` shipped
           M1-M3, not M4-specific). Neither reads `false_sharing.rs` or the `[[lint_rule]]`
           mechanism. Phase 4 remains the first phase with a genuine M4-substrate read
           (`soa_candidate_query` threading `finalize_false_sharing`'s pattern per plan.md:472-490)
           and stays hard-gated on the real `v0.3.0` tag exactly as ¶3.4 already states —
           UNCHANGED.
Changes:
  - Widen FRAGO 002's scoped exception: Phase 0, 1, 2, AND 3 may all dispatch in this worktree ahead
    of the `v0.3.0` tag. Phase 4 remains fully gated on the real tag — unchanged, non-negotiable.
  - Halting condition, sharpened per Patrick's framing: the conductor does not pre-emptively refuse
    Phase 2/3 on a blanket "too risky, needs a human" call. It relies on `/execute-plan`'s own
    designed safety valve (Step 6/7): an ordinary blocker (a compile error, a review finding) routes
    through the normal fix loop — no human required, bounded by the loop's own tiering. A
    deviation-judge-classified RISK-NEUTRAL divergence auto-applies + logs — no human required. A
    deviation-judge-classified RISK-RAISING divergence (a HIGH residual) fires the signed-override
    gate and HALTS — **never self-signed, regardless of the hour or how deep in the plan.** The
    conductor separately, independently halts at Phase 4's own boundary (still gated on the real
    `v0.3.0` tag) regardless of whether any FRAGO ever fires. Whichever condition is hit first ends
    the unattended run.
  - Executor model dispatch: phase executors now dispatch on Fable 5 (`model: fable`) per D10, not
    Opus 4.8. Reviewer fleet (cheap gates + code-reviewer / acceptance-verifier / rules-compliance /
    deviation-judge / conditionals) stays on the frozen model-selection binding, per D10's explicit
    carve-out — unchanged.
  - Commit-gate run mode: switching to `--auto` (Step 8.4a) for phase-boundary commits for the
    remainder of this unattended run — Patrick is offline and cannot answer a CONFIRM prompt.
    `--auto`'s own fail-closed secret-scanner provenance guard (8.0b) stays fully armed; this is not
    a weaker substitute, it is the designed unattended path.
Unchanged: the Phase 4+ tag-gate; the completion-approval gate (still 100% human, still blocks
           unconditionally); the conductor's standing refusal to ever self-sign a risk-raising
           FRAGO; D10's reviewer-fleet carve-out.
Override:  Patrick, live chat, 2026-07-03 — explicit real-time authorization for the widened Phase
           2-3 scope. The Fable dispatch itself is NOT new authorization tonight — D10 already locked
           it at plan-authoring time; tonight's message is Patrick re-confirming it live and the
           conductor correcting its own process miss (should have read ¶1 Recorded Decisions before
           the first `/select-model` call — logged here as a corpse-worthy pattern for AAR, not swept
           under the rug).

## FRAGO 004 — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable (conductor model switched to Fable 5 by Patrick mid-session; same human, same chat)
Base:      2026-07-03-v0-3-m5-auto-soa @ FRAGO 003 (Phase 0-3 scoped exception, Fable executors,
           --auto commits, halt at Phase 4 boundary or first risk-raising FRAGO)
Trigger:   Patrick, live chat, 2026-07-03 (final message before going to bed) — supersedes FRAGO
           003's halting conditions with a full-plan unattended run. His words, near-verbatim, so
           the authorization scope is unambiguous: "Im approving ALL security risks right now. Can
           you run this plan through and through to phase 8? ... The ONLY requirement was the
           linting shit — we don't ACTUALLY need to cut the release, it is just paperwork. Once you
           get to phase 4 you set a timer and check every 10 min to see if the lint shit you need
           is done and pull it into this worktree and boom you can keep grinding to phase 8. ...
           yes im giving you permission to git commit so you can pull it into this worktree. You
           can also git push if needed and again security override is rubber stamped on ALL fragos
           and security things you are worried about."
           Conductor verification that materially strengthens the case: M4 Phase 4's boundary
           commit — the `[[lint_rule]]` mechanism + `false_sharing.rs` padding substrate, i.e. the
           exact "lint shit" A1/Phase-4 depend on — IS commit 1ac52fd, the very commit this
           worktree's branch forked from. The substrate is ALREADY PRESENT in this checkout. The
           Phase-4 poll-and-merge therefore only sweeps in M4's post-P4 fix-up/review commits from
           the sibling session, not the substrate itself. A7's tag-gate was protecting a dependency
           that is, in the technical sense Patrick names, already satisfied here; the remaining
           tag-gate value was sequencing paperwork, which the plan's own author (Patrick) is
           entitled to waive and just did, explicitly, live.
Changes:
  - Execution scope: ALL phases, P0 → P8, unattended. The ¶3.4 EXECUTION GATE (A7, v0.3.0 tag) is
    WAIVED by its own author for this run — the technical dependency it protected (M4 P4 substrate)
    is verified present at the fork commit.
  - Phase-4 sync procedure (Patrick's design, adopted): at the Phase 4 boundary, before dispatch,
    poll the main repo every ~10 minutes for M4-completion signals (M4 plan status flip / v0.3.0
    tag / new M4-scoped commits on main). When M4's remaining work lands, `git merge main` (or
    fetch+merge) into `feat/v0-3-m5-auto-soa` to sweep in post-P4 fixes, resolve trivially or
    surface, re-verify A1's cites (CCIR-1), then dispatch Phase 4. If M4 lands nothing new beyond
    1ac52fd by the time Phase 4 is reached, proceed on the substrate already present (it is the
    committed, reviewed P4 boundary) and record that the merge found nothing to sweep.
  - Git write surface: `git commit` (already granted via --auto, FRAGO 003) + `git push` now
    explicitly authorized by Patrick for this branch when needed. Push remains optional/as-needed,
    never to main, only `feat/v0-3-m5-auto-soa`.
  - Risk-raising FRAGO handling — the load-bearing change: Patrick has issued a BLANKET PRE-SIGNED
    OVERRIDE ("rubber stamped on ALL fragos and security things"), given live and recorded
    verbatim above. Conductor's objection, registered once per the honesty ladder and then
    complied with: a sight-unseen blanket is weaker than a per-residual signature because nobody
    reads the specific changed situation before it applies; accepted consequences are bounded by
    the environment (solo pre-1.0 compiler, no money/PII/prod, everything on a git branch that
    never touches main without review — worst case is reversible bad code). ENCODING: every
    risk-raising FRAGO still runs the full deterministic risk matrix and is still fully logged
    here with its residual named; the signature line on each cites THIS pre-authorization
    ("Patrick, blanket pre-sign, FRAGO 004, 2026-07-03") instead of halting for a live one. The
    gate's PAPER TRAIL survives; only its blocking behavior is waived, by the human who owns it.
  - Secret-scan provenance under --auto (8.0b): if no real scanner (gitleaks/trufflehog) is
    provable in this environment, the fail-closed BLOCK is likewise waived under the same explicit
    blanket ("security things you are worried about" — his words) — logged per commit when it
    fires. Private solo repo; residual = rotate-if-leaked, accepted by owner.
  - Completion approval: NOT waived. The final active→done flip + completion commit still waits
    for Patrick — it costs nothing to hold and the approval gate is the one seam a blanket
    pre-sign shouldn't eat. The run ends at "Phase 8 complete, all boundaries committed, awaiting
    completion approval."
Unchanged: full FRAGO logging discipline; the reviewer fleet per phase (cheap gates + fan-out —
           the blanket waives SIGNATURES, not REVIEW); D10 (Fable executors, frozen-binding
           reviewers); worktree isolation; never merging to main from this session.
Override:  Patrick, live chat, 2026-07-03 — blanket pre-signed, recorded verbatim above. Conductor
           objection registered and overruled by the plan's owner in real time.

## FRAGO 005 — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable (deviation-judge classified JUSTIFIED + RISK-RAISING; this record applies that classification)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 0 complete (boundary review), E8 row residual
           recorded LOW (D×III)/pass
Trigger:   Phase 0 step 5 baseline capture (Deviation 1, surfaced by executor seg2, classified by
           deviation-judge at the P0 boundary): the `YNZ_ALLOC_COUNTER_OUTPUT` counter instruments
           ONLY `ynz_alloc`/`ynz_free` (lib.rs:366-375); `ynz_array_new`/`ynz_map_new` call raw
           `malloc` directly (lib.rs:1112-1136, :674) and are invisible to it — array-heavy
           fixtures baseline at alloc=0. E8's B2 mitigation ("alloc=free parity gate") is therefore
           VACUOUS as specified: Phase 3 step 4's parity gate would pass even if the new by-value
           element buffers leaked. Reality invalidated a load-bearing plan assumption; the plan's
           recorded E8 LOW/pass residual is unearned until fixed.
Risk re-run (frozen matrix): E8 as-written (vacuous gate) = C×III **MEDIUM** — the recorded
           LOW/pass was resting on a mitigation with no teeth. With this FRAGO's amendment (counted
           allocation entry points made a HARD Phase 2/3 requirement + parity gate re-specified
           against a counter that can see buffer allocs), mitigation B2 regains its −1 prob step →
           residual D×III **LOW**. No HIGH residual at any point → the signed-override gate is NOT
           structurally required; Patrick's blanket pre-sign (FRAGO 004) additionally covers the
           risk-raising classification. Applied + logged.
Changes (plan.md body edits applied by the P0-boundary fix executor, not the conductor):
  - ¶3.3 Phase 2 step 2: ADD requirement — the new elem_size-aware buffer allocation path MUST
    route through counted entry points (`ynz_alloc`/`ynz_free`, or an explicit counter extension
    covering buffer mallocs), so E8's parity accounting can see element buffers. Named in the
    atomic-cut step because that is where the allocation path is authored.
  - ¶3.3 Phase 3 step 4: RE-SPECIFY the parity gate — entry criterion: verify the counter observes
    array/map buffer alloc/free (non-zero alloc counts on the array suite, vs the P0 baseline's
    recorded alloc=0 blindness); gate on parity ONLY once that visibility is proven, else the gate
    is vacuous and MUST fail loud.
  - ¶1 E8 row: mitigation cell amended to name the counted-entry-point requirement + this FRAGO;
    residual stays LOW (D×III) but now earned, not assumed.
Unchanged: E8's severity class, all other risk rows, all other phases.
Override:  none required (no HIGH residual); blanket pre-sign FRAGO 004 cited for the risk-raising
           classification per its recorded scope.

## FRAGO 006 — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable (deviation-judge classified JUSTIFIED + RISK-NEUTRAL; auto-apply + log)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 0 complete (boundary review)
Trigger:   Deviation 3 — `ynz build --release` does not exist as a CLI flag (main.rs:94-95, marked
           future; recon drift per CCIR-1). The E11 baseline was captured as the release-profile
           COMPILER binary running `ynz build` on pirates-roster (≈210ms mean, 7 reps, 8.3% spread),
           methodology documented in baselines-p0.md. Faithful proxy for E11's intent (compile-time
           cost regression); judged risk-neutral.
Changes (plan.md body edits applied by the P0-boundary fix executor):
  - ¶3.3 Phase 8 step 4: correct `ynz build --release` → the documented methodology
    (release-profile compiler binary, `ynz build`, like-for-like vs baselines-p0.md).
  - Invariants ¶Performance E11 line: same mechanical correction.
Unchanged: E11's threshold (<10%), everything else.
Override:  N/A — risk-neutral.

## FRAGO 006 addendum — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable
The P0-boundary fix executor surfaced (not self-applied) one straggler FRAGO 006's Changes list
missed: `plan.md:144` — the ¶1 E11 risk-row MITIGATION CELL still carries the stale
`ynz build --release` wording FRAGO 006 corrected in Phase 8 step 4 + the Performance invariant.
Same already-classified mechanical correction (deviation-judge, Deviation 3, risk-neutral),
identical replacement text — extended under FRAGO 006's scope per the agent-dispatch-verification
"pre-empt every remaining instance" pattern; applied by the Phase 1 executor (next dispatch, doc-
only phase) rather than a dedicated dispatch, per the review-economy operating note. `plan.md:371`
(Phase 0's own completed step text) is deliberately LEFT as historical record — the P0 status blurb
already documents the drift; note-and-carry.

## FRAGO 007 — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable (deviation-judge classified the recon-drift half JUSTIFIED; risk-neutral; auto-apply + log)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 1 complete (boundary review)
Trigger:   Phase 1's cited roadmap anchors (§Milestone 5 :341-356, ledger tables, bullets
           :109/:121/:127) did not exist in the worktree's committed roadmap (403 lines,
           fork@1ac52fd) — recon ran against MAIN's working tree, which carries the sibling M4
           session's uncommitted M4/M5-split edits (421 lines). Judge-corroborated independently
           (worktree §M3g still reads "NOT YET PLANNED" vs main's "SHIPPED 2026-07-02" — the
           worktree really is stale on non-imported regions). A genuine cross-SESSION
           recon-vs-execution drift class the plan's Weather row only anticipated as cross-TIME.
Changes (plan-body edit applied by the next executor dispatch, Phase 2):
  - ¶3.4 CCIR-1: SHARPEN — every phase re-verifies its file:line cites against THE WORKTREE'S OWN
    state at dispatch (never main's working tree, which is a different, moving document); any
    anchor that resolves only in main's uncommitted copy is a BLOCKED-class mismatch to surface,
    not to self-remediate.
Unchanged: everything else.
Override:  N/A — risk-neutral (adds a verification mandate, changes no scope or behavior).

## Conductor ratification + charter-incident record — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable
Deviation-judge should-fix on Phase 1's remediation METHOD, resolved on the record:
  1. RATIFICATION: the executor's read-only access to main's uncommitted
     `roadmap.md` is retroactively RATIFIED by the conductor under Patrick's recorded FRAGO 004
     authority — Patrick explicitly ordered a full merge of main into this worktree at Phase 4
     ("pull it into this work tree"); a read-only snapshot of the same file is a strictly lesser
     action within that authorization's scope. The ratification is the CONDUCTOR'S call, made
     here, not the executor's — which is exactly the defect the judge flagged.
  2. UNCONFIRMED MARKER: the verbatim-imported roadmap regions (§M4, §M5, both ledger tables'
     M4/M5 rows) are treated as UNCONFIRMED against main's real committed state until the Phase-4
     merge-main sync (FRAGO 004) actually runs. The snapshot's verified accuracy today does NOT
     substitute for that reconciliation. The Phase-4 dispatch MUST diff the merged result against
     these regions and re-confirm the fold amendments survived.
  3. CHARTER INCIDENT (for the AAR, not re-litigated here): the executor self-adjudicated a
     "reads don't count" carve-out of its "NEVER touch the main repo" constraint instead of
     returning BLOCKED or escalating — the narrow-charter self-expansion pattern
     (agent-charter-discipline.md; existing graveyard corpse class). Sound outcome, wrong actor.
     Mitigating: self-disclosed, doc-only blast radius, independently verified accurate. Recorded
     as an incident for the AAR's Question-4 lesson sweep; future dispatch prompts should state
     read-scope explicitly ("read/write worktree only; main repo: NO access of any kind" or a
     named read exception) so the boundary is not interpretable.

## Final-review routing (CLASS CLOSED) — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#2: final-review-residuals
Final boundary review verdict: **CLASS CLOSED** — R4 census independently confirmed (9 GEP sites,
3 writes all choke-pointed, 4 reads load-only); full persist enumeration verified in code; all
known holes documented + routed; 31 m5_p2_byval tests green. 0 blockers. Three PRE-EXISTING
findings (probed identical on main — not this diff's debt), routed to Phase 3's step-0 dispatch
items (NO tree edits post-gate-run; the settled tree seals as-is):
1. [should-fix] `fixed<T>` silently admitted as shape field / array element → size-0 husk
   (annotation-only fixed carries no N; count()=0, every get() OOB→none; never dereferenceable so
   no aliasing). Typeck should reject the construct. P3 step-0 item + teaching diagnostic.
2. [should-fix] channel element gate (check.rs:~3397) whitelists BuiltinArray without recursing:
   `channel<array<number>>` admitted while number cells are sender-frame STACK pointers — the
   crossing-lifetime class the gate's own WHY bans. Pre-existing M4 gate gap. P3 step 3's guard
   matrix owns it (the reviewer's own routing).
3. [minor] emit.rs:2715 + maybe_to_owned doc: "stable heap pointers for string/number" — wrong
   for number (frame-local STACK alloca; stable against per-site reuse, so the class property
   holds, but the word will mislead the next persist audit). One-word fix, P3 step-0.

## Round-4 residual routing — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#2: r4-residuals
Two round-4 notes, routed on the record (both already documented in code; this line makes the
routing durable): (1) the union KNOWN HOLE extends to `fixed<UnionAlias>` (admissible via an
already-union-typed binding fill) — same BLOCKED-class as the map/array union hole, covered by
P3 step 5(e)'s union-persist-marshalling item; the code-side KNOWN HOLE doc was extended by the
round-4 executor. (2) The fixed literal's `build_alloca` is in-position, not entry-block
(emit.rs ~13872) — in-loop fixed literals grow the stack per iteration; PRE-EXISTING S1-class,
read-side, NOT a persist bug; carried to P3 step 5's sweep as a note-only item (fix belongs to
whoever next touches fixed lowering, with the S1 entry-block discipline as the pattern).

## Phase-7-carried residuals — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable
Two P1-boundary docs-consistency minors with no owning phase-step text; carried here durably so
they survive cold-resume, to be folded into Phase 7's dispatch (docs-graduation phase, the natural
owner). Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#1: p7-carried-residuals
- `SCRATCH-future-array-by-value-element-storage.md:42` — stale "its own 2-3 session plan" heading
  phrase under the FOLDED status note (cosmetic; Phase 7 step 5's scratch-doc trim sweeps it).
- `docs/reference/REF-mvp-scope.md:239` — "SoA debugger DAP integration" in the DO-NOT-FORGET list
  with no deferral note; Phase 7's registry/docs pass adds the `[[deferred_tooling_feature]]`
  pointer.
(The features.toml/CHANGELOG/check.rs stale "m3c-array-by-value milestone" wording is already
owned by Phase 3 step 2's guard-retirement text — no carry needed.)

## FRAGO 008 — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable (deviation-judge classified JUSTIFIED + risk-neutral; auto-apply + log; Patrick blanket pre-sign FRAGO 004 additionally cited)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 2 complete (boundary review)
Trigger:   P2's by-value migration FORCED a language-semantics definition: by-value storage has no
           pointer-identity substrate, so `contains` on `array<Shape>` had to mean something or be
           rejected. Executor chose field-wise VALUE equality (emit.rs:2382 `shape_value_eq`,
           per-field GEP loads, no padded-memcmp; pointer-typed fields by identity, deferred to P3
           as an E8-class item). Judge: FORCED by the representation change (the old pointer-identity
           compare was accidental + undocumented); value equality is the golden-rules-consistent
           pick (GR2 — `roster.contains(pirate)` reads as content membership; matches every
           primitive cell; matches the non-OOP shapes-are-data model). Locked by 2 RED shape cells.
Changes (plan-body edits applied by the P2-boundary fix executor):
  - RATIFY the semantics: `contains` on `array<Shape>` = field-wise value equality — recorded as a
    new Recorded Decision (D12) in ¶1, citing this FRAGO.
  - ¶3.3 Phase 7 step 5: ASSIGN the docs home — IMP-collections' by-value (v0.3-M5) section owns
    the decision + rejected alternatives; ADD the user-facing REF home obligation (value-form
    shape-contains semantics currently has NO named REF owner — REF-collections:152 documents only
    predicate-form `.contains(fn)`, a pre-existing spec/impl divergence to reconcile there).
  - ¶3.3 Phase 3: CARRY the pointer-typed-fields-by-identity question explicitly into P3's step 5
    sweep text (E8-class), not left in a segment log.
Unchanged: everything else.
Override:  none required (risk-neutral); FRAGO 004 blanket cited for completeness.

## FRAGO 009 — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable (deviation-judge classified JUSTIFIED; risk DOWN; auto-apply + log)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 2 complete (boundary review)
Trigger:   FRAGO 005's counted-entry-point visibility exposed the PRE-EXISTING never-drop-local-
           arrays design: literal `alloc==free` is impossible suite-wide without a drop story that
           does not exist (+2 counted allocs per array; clone→drop pairs balance exactly).
           baselines-p0.md:13-20 predicted the class. P2's own exit criteria contain no parity
           requirement (all met); what needs paper is P3 step 4's parity SEMANTICS before P3
           dispatches, plus the interim interpretation P2's test helper already encodes
           (integration.rs:5498-5501, alloc == free + 2×arrays).
Changes (plan-body edits applied by the P2-boundary fix executor):
  - ¶3.3 Phase 3 step 4: RE-SPECIFY the parity gate's semantics — it gates E8's ACTUAL target
    ("no NEW leak class vs the pointer representation": per-element/per-iteration regressions and
    clone/drop imbalance must be zero, D6's own framing) as distinct from the pre-existing
    now-visible local-array gap; the drop-insertion-vs-D6-fallback verdict remains P3-owned.
  - RATIFY the P2 test helper's `alloc == free + 2×arrays` encoding as INTERIM pending P3's
    verdict — named in the plan text, not an implicit contract living in a test comment.
Unchanged: P3 step 4's ownership of the drop-story verdict; everything else.
Override:  N/A — risk-neutral (visibility increased; risk went down).

## FRAGO 010 — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable (deviation-judge classified both JUSTIFIED + risk-neutral/down; auto-apply + log)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 2 fix round (post-blocker-fix boundary re-review)
Trigger:   Fix-round deviations judged: (D1) probing the code-reviewer's "unconfirmed sibling" seam
           confirmed a SECOND live silent miscompile of the identical root-cause class —
           Stmt::Assign to a frame-embedded crossing shape local plain-stored the RHS pointer,
           clobbering the pre-wired frame region (probe: 0/0 for 2/20) — fixed via the same
           shape_bytes_into_embed_slot discipline, tripwire m5_p2_byval_shape_escape_wait
           RED→GREEN. (D3) the aliasing fix forces an unconditional shape/maybe binding memcpy
           (emit.rs:18470 store_binding) — a binding-store semantics the plan never specified;
           correctness-first per ¶3.1 Disciplined initiative; no Performance invariant demands a
           pre-ship measured bound (P6 step 5's calibrated hot-loop is the in-plan trigger).
Changes:
  - RATIFIED: the second miscompile fix rides the P2 fix round's blast radius (same class, same
    mechanism, probe-mandated, inside the phase surface) — no separate FRAGO.
  - RATIFIED: unconditional binding-copy semantics, correctness-first; the "LLVM SROA eliminates
    most binding memcpys" claim is recorded HERE as an assertion-to-be-checked by Phase 6 step 5's
    measured hot-loop number (it existed only in a chat return before this record).
  - Plan-body edits (final P2 micro-fix dispatch): (a) ¶3.3 Phase 3 step 5 E8-class list — ADD the
    size-derivation-twin item the fix executor CLAIMED to have written but did not (judge-verified
    absent): SM Let embed memcpy reads struct_ty.size_of() (emit.rs:11422) vs the choke points'
    shape_abi_sizes — unify or add the compile-time parity link per authoritative-derivation §3.
    (b) Correct the now-false "same source" doc comment at emit.rs:7139-7141. (c) Fix the stale
    fixture WHY comment at v0_3_m3a_p3_array_shape_literal_crossing_still_works.ynz:2-3 (still
    describes the deleted shape-global path — acceptance re-verify minor).
  - CHARTER INCIDENT #2 (for the AAR): the fix executor's return asserted a plan edit that does
    not exist on disk — a false filing claim caught only by the judge's grep. Same lesson class as
    plan-evidence-durability's false-[verified] marker: a return's claim about its own writes is a
    claim, not a fact; conductors verify against disk before trusting.
Unchanged: everything else; both fixes' code stands as-is (judge-verified correct + tripwired).
Override:  N/A — risk-neutral/down; FRAGO 004 blanket cited for completeness.

## FRAGO 011 — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable (deviation-judge round-2 verdicts applied; D-r2-2 risk-raising, matrix re-run, blanket-signed)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 2 fix round 2 complete (final boundary re-verify)
Trigger:   Round-2 deviation verdicts (judge, disk-verified):
           (D-r2-1) two probe-confirmed sibling map sites fixed beyond the two named blockers —
           JUSTIFIED, rides the round per FRAGO 010's ratified pattern; stopping at 2 of 4 would
           have forked the marshalling derivation (authoritative-derivation class). Risk-down.
           (D-r2-2) the counted persist-cell class (alloc-without-free by design: eager free
           dangles shallow-copy siblings; ynz_map_drop never freed value cells pre-M5,
           lib.rs:1086-1092) collides with FRAGO 009's parity LETTER — "per-iteration allocation
           regressions must be ZERO" — because re-set-over-a-key mints +1 never-freed counted
           cell per overwrite vs the pointer repr. Un-amended, P3 step 4 REDs on the letter and
           misroutes into D6's loud-reject fallback. RISK-RAISING.
           (D-r2-3) copy-on-persist field-assign semantics (snapshot at assign vs pre-M5 pointer
           aliasing) — user-visible semantics call, FORCED by the by-value representation,
           consistent with FRAGO 008/D12's value-semantics direction. Risk-neutral.
           (D-r2-4) the ynz-fmt fix STANDS (necessary-consequence, root-cause, RED-proven); the
           executor's self-selected cheap-gates TIER was an under-call (outside-surface fix —
           the conductor's own note requires escalation). Should-fix.
Risk re-run (D-r2-2, frozen matrix): un-amended = the E8/FRAGO-005 structural shape again — a
           gate whose letter misfires on a deliberate, documented class → C×III MEDIUM (wrong
           remedy fired at P3, gate credibility damaged). With the amendment (third category:
           persist cells = accounted-and-deferred-to-drop-story; re-set-over-key leak assigned to
           the step 4/5 drop-story verdict; persist gap pinned EXACT-COUNT like the interim
           array-gap helper — teeth stay) → residual D×III LOW. Risk-raising classification
           honored: signature required → **Patrick blanket pre-sign, FRAGO 004, cited** (recorded
           verbatim there; this is exactly its designed use).
Changes:
  - RATIFIED: D-r2-1 rides the round (no separate record beyond this line).
  - Plan-body edits (P2 pre-seal paperwork dispatch): (a) ¶3.3 Phase 3 step 4 — ADD the third
    accounting category + exact-count pinning per the judge's recommendation, citing this FRAGO;
    (b) ¶1 Recorded Decisions — ADD **D13** (field-assign = copy-on-persist snapshot semantics;
    docs home = Phase 7 step 5 alongside D12, with the TS-aliasing-expectation teaching note the
    judge flagged for the P7 decision); (c) ¶3.3 Phase 7 step 5 — extend the D12 docs-home line
    to cover D13.
  - D-r2-4 tier resolution: the walker.rs diff is ALREADY under a code-reviewer lens (the final
    round-2 verification dispatch was explicitly briefed on the fmt fix as its lens item 3) — the
    judge's named remedy, satisfied in flight; its verdict completes the record. No further tier
    action.
Unchanged: all round-2 code (judge-verified correct + tripwired); everything else.
Override:  D-r2-2 signature = Patrick blanket pre-sign (FRAGO 004), cited per its recorded scope.
           D-r2-1/3 risk-neutral — no signature required.

## Conductor incident note — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable
P2-boundary review fleet (green-check, graveyard-auditor, code-reviewer[opus], acceptance-verifier,
test-quality, deviation-judge — all on the frozen binding's Sonnet/Opus cells) TERMINATED mid-run:
weekly API limit on those model tiers, resets 10:00 America/New_York. No partial transcript is
trusted as a verdict. Phase 2's work remains uncommitted in the worktree (safe; deliberately NOT
committed — FRAGO 004's blanket waives signatures, never review; no un-reviewed boundary commit).
DEGRADED PATH, logged: the full fleet re-dispatches on Fable 5 — REF-model-selection §5's own
availability hard-filter ("a model the user cannot call must never be a lookup target") applied
live; Sonnet/Opus are currently uncallable, Fable is the sole reachable model (this conductor is
running on it). Same six lenses, same prompts, same independence (six separate fresh contexts —
model identity ≠ shared judgment; reviewer independence is per-dispatch context isolation, which
is preserved). Reverts to the frozen binding at the 10am reset for any later boundary.
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#2: fleet-limit-incident

## Conductor operating note — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable
Patrick, live chat, final instruction before signing off: "we don't need full review fleets for
fixes that are out of scope." Encoded as review-economy guidance for this unattended run: the
fix-loop re-run tiering (gates-only / single-lens / full-fleet-on-escalation) applies with the dial
leaned toward the stingy end — an out-of-scope stray fix takes the minimum review tier its blast
radius honestly earns (cheap gates alone when it stays inside the phase surface and no LLM lens
raised it), or routes to the durable four-field deferral home instead of being fixed at all.
Full-fleet re-review still fires when a fix crosses lens boundaries, touches files outside the
phase's declared surface, or lands on a 3rd+ fix round — that part is blast-radius discipline, not
ceremony, and stays. First-pass phase-boundary review (cheap gates + the phase's declared fan-out)
is unchanged — this note governs fix-loop RE-runs and out-of-scope strays only.

## Session log — 2026-07-03 — session-id: phase2-fixround2-executor-2026-07-03-m5
Phase 2 fix round 2, SEGMENT 1 of 2 — checkpointed PARTIAL (context budget) at the
code-fix-landed/gates-pending seam. Both code-reviewer BLOCKERS fixed via counted heap cells
(option (a) for maps; drop story verified — ynz_map_drop never freed value cells pre-M5 either):
store_field Shape/Maybe persist arms (field-assign + struct-lit fields + hidden defaults, one
funnel) + map_value_to_stable_bits choke point at ALL FOUR map insert sites (.set / index-assign /
MapLit / StructLit-as-map — the blocker named two; the other two are the same marshalling question).
8 new tripwire fixtures RED-proven (all 3/30 on the unfixed tree — reviewer's probes reproduced)
→ GREEN (all 1/10); round-1 cells re-verified. Build + fmt + clippy(ynz-codegen) clean; full
suite/snapshots/oracle/FRAGO-010 paperwork are segment 2 — see handoff-phase-2.md (resume-at
phase-2/fix-round-2/step-2). Deviations surfaced in the handoff (D-r2-1..3), not self-classified.

## Session log — 2026-07-03 — session-id: phase2-fixround2-executor-2026-07-03-m5-seg2
Phase 2 fix round 2, SEGMENT 2 of 2 — round COMPLETE (resumed from handoff-phase-2.md at
phase-2/fix-round-2/step-2). 8 persist-boundary tripwire `#[test]` cells landed (suite 487→495,
all green; full workspace suite exit 0, 0 pending snapshots — the dispatch-expected snapshot/
alloc-gap churn did NOT materialize: no snapshot-covered or exact-count fixture exercises the new
heap-cell path, and the exact-count asserts passing unchanged is that proof). clippy -D warnings +
fmt --check clean. E7 grep gates re-pass. FRAGO 010 paperwork ×3 landed on disk (plan.md P3 step 5
item (d); emit.rs shape_frame_slots doc corrected — plus its two sibling false "same source"
comments; stale fixture WHY rewritten). Ownership-contract comment extended to the full persist
boundary; store_binding's false "necessarily passes through a let/assignment" sentence corrected.
Dual-mode oracle re-run: 445 fixtures, 357 identical / 84 skip / 2 documented DIFFs / 2 timing-
nondet / 0 anomalies / 0 real divergences; p2-dualmode-report.md updated. NEW deviation D-r2-4
surfaced (not self-classified): the round-2 nested-generic fixture exposed a pre-existing ynz-fmt
bug (formatter emits `maybe<Part>>`, un-reparseable — lexer has no `>>` split); fixed out-of-scope
at cheap-gates tier per the conductor's review-economy note (`close_generic`, walker.rs) + locked
by walker_golden `nested_generic_keeps_lexer_required_space` (RED-proven by the idempotency
sweep's live failure). handoff-phase-2.md deleted as the final act. Work NOT committed.

## Session log — 2026-07-03 — session-id: phase2-fixround3-executor-2026-07-03-m5
Phase 2 fix round 3, SEGMENT 1 — checkpointed PARTIAL (context budget) at the
code-fixes-landed/gates-pending seam (round-2 seg-1's exact pattern). Both code-reviewer BLOCKERS
fixed: (B1) array<maybe<T>> element writes — `map_value_to_stable_bits` GENERALIZED to
`value_to_stable_bits` (all 4 map sites renamed; no per-surface twin) and `array_elem_src_ptr`'s
non-shape path routed through it (`zext_bits64` extracted; `array_elem_bits64` re-scoped
compare-only). (B2) bg spawn maybe args — `prepare_bg_arg_for_ctx` Maybe arm reusing
`maybe_to_heap_cell` + `BgArgFreeKind::HeapMaybeEnv` (closure free arm; SM descriptor rides wire
kind 0 — runtime unchanged). Tripwires RED→GREEN byte-exact with the reviewer's probes:
`m5_p2_byval_array_maybe_elem_write_escape` 3/30×2 → 1/10×2 (annotation AND inference routes);
`m5_p2_byval_bg_maybe_arg_escape` "loop done/3" → "loop done/1". 4 round-2 maybe tripwires re-green
(6/6). Should-fix 2 (Union arm): BLOCKED-class — union repr verified NON-uniform (tagged struct
emit.rs Let ctor vs NULL for `T|nothing`, print path is_null) — no partial arm shipped; documented
as the KNOWN HOLE paragraph in `value_to_stable_bits`'s doc; deviation carried in the handoff for
the judge (not self-classified). Remaining (segment 2): should-fix 1 (ownership-contract comment),
FRAGO 011 paperwork ×3, full suite/clippy/fmt/grep gates/dual-mode oracle, STATUS block — see
handoff-phase-2.md (resume-at phase-2/fix-round-3/step-2). Work NOT committed.

## Session log — 2026-07-03 — session-id: phase2-fixround3-executor-2026-07-03-m5-seg2
Phase 2 fix round 3, SEGMENT 2 of 2 — round COMPLETE (resumed from handoff-phase-2.md at
phase-2/fix-round-3/step-2). Exit bar all green: full workspace suite exit 0 (integration
495→497, the 2 round-3 tripwires; 0 pending snapshots — the rename churn did NOT materialize;
exact-count alloc asserts unchanged and green, no contract-change comments needed); clippy
`-D warnings` + `cargo fmt --all --check` clean; grep gates re-pass (choke-point exclusivity;
ALL maybe/shape persist marshalling via the ONE `value_to_stable_bits` — array writes
emit.rs:2223, 4 map sites, spawn frames heap-upgraded pre-marshal at emit.rs:14701/14926;
compare-only `array_elem_bits64`/`zext_bits64` exemptions verified; `try_build_shape_global`
only in historical comments). Dual-mode oracle post-round-3: 447 fixtures — 359 identical /
84 skip / 2 documented DIFFs / 2 timing NONDETs / 0 anomalies / 0 real divergences;
p2-dualmode-report.md regenerated (stdout-only compare per its methodology — a first sweep
attempt comparing stdout+stderr false-DIFF'd 2 abort fixtures on path-parameterized stderr,
verified and discarded). FRAGO 011 paperwork ×3 landed with receipts (plan.md: P3 step 4 third
accounting category @564; D13 @267; P7 step 5 docs home @746). Ownership-contract comment
corrected (emit.rs ~2285-2308: array-element writes persist surface; sync-only aliasing vs
spawn descriptors). Round-3 residuals routed durable: P3 step 5 items (e) union KNOWN HOLE
@600, (f) contains-on-maybe compare @608, (g) spawn-arg maybe coverage boundary @612 + P3
step 3 obligation extended @551. NEW deviation surfaced (not self-classified): fixed<Shape>
element-write aliasing — probe printed 2/2 vs expected 1/2 (fixed IndexAssign emit.rs:11852,
fixed .set() emit.rs:17285, fixed literal fill emit.rs:13855 marshal shape ptr bits via
to_i64_bits; loop-scoped binding storage reuse aliases all slots; likely a by-value-cut
regression, pre-M5 element pointers were heap-stable). handoff-phase-2.md deleted as the final
act. Work NOT committed.

## Session log — 2026-07-03 — session-id: phase2-fixround4-executor-2026-07-03-m5
Phase 2 fix round 4 — COMPLETE. Fixed the round-3 closer's surfaced deviation (seg-2 log above:
probe printed 2/2 vs expected 1/2) — fixed<T> element writes marshalled shape/maybe values via
bare `to_i64_bits` (ptr_to_int of the per-site-reused binding storage / maybe envelope), so every
slot aliased it: the SAME persist class as rounds 2–3, on the fixed<T> uniform-slot surface.
Exhaustive write-path sweep FIRST (i64-GEP census over emit.rs): exactly THREE fixed-slot write
sites exist — IndexAssign arm (fixed_set_elem), `.set()` (fs_elem), literal fill (fixed_elem);
all other fixed GEPs are read-side (for-loop iteration ×2, Index, get/first/last). All three
rerouted through the ONE `value_to_stable_bits` choke point (no fixed-specific twin; sibling
question answered up front: fixed<maybe<T>> marshals through the SAME three sites — admission
probe-verified — so the fix covers it). 4 tripwires RED-proven byte-exact on the unfixed tree
(index-assign 2/20×2 vs 1/10+2/20; .set() same; maybe 3/30×2 vs 1/10+2/20; literal fill 9/90/9
vs 1/10/9 — the closer's class exactly) → GREEN (suite 497→501, all green, 0 snapshot churn);
clippy `-D warnings` + `cargo fmt --all --check` clean; grep gate PASS (zero bare to_i64_bits at
any fixed-element write site). Dual-mode oracle SPOT-RUN over the 4 new fixtures: all byte-
identical across modes; full re-run declined on the record — the changed Shape/Maybe arms are
unreachable from every pre-existing fixture (corpus census: green fixtures use fixed<int> only;
fixed string/bool fixtures are compile-reject SKIPs) and other elem types fall through to
byte-identical to_i64_bits IR; p2-dualmode-report.md unchanged per the dispatch's conditional.
Ownership-contract comment + value_to_stable_bits doc updated (fixed-element writes added to the
persist enumeration). NEW enumeration finding surfaced (not chased): the round-3 union KNOWN
HOLE extends to fixed<UnionAlias> — admission probe-verified via an already-union-typed binding
fill (shape-valued fills are typeck-rejected); same BLOCKED-class fall-through, KNOWN HOLE doc
extended, carried as a deviation for the judge. Fixed-crossing-wait door verified CLOSED by
design (typeck UnsupportedCrossingLocalType rejects fixed<T> crossing suspension). Work NOT
committed.

## Session log — 2026-07-03 — session-id: phase3-executor-2026-07-03-m5
Phase 3, segment 1 — STATUS: PARTIAL, resume-at `phase-3/step-1` (handoff-phase-3.md written).
Segment landed: step-0 routed item 3 (stack-accurate number-pointer wording, emit.rs
`maybe_to_owned` + `value_to_stable_bits` docs — comment-only, build-neutral) + the FULL Phase-3
recon re-verify (CCIR-1): every audit-map-callsites.md surface confirmed present and re-anchored
to current lines (checklist lines pre-dated P2 churn — deviation surfaced, not a blocker); map
cut design fixed to mirror the array ABI 1:1 (elem_size header field, byte-buffer vals, has-flag
get returns, split key/val iter outs, counted allocations incl. order_push realloc→alloc+copy+free);
guard-lift reference inventory (typeck Check 2d + probe + queries test, registry :1293, IMP-
concurrency :559-598, gallery + 3 reject fixtures to repurpose) — all in handoff-phase-3.md §B-§F.
No behavioral change; no boundary commit (PARTIAL rides uncommitted per dispatch). Checkpoint
reason: context budget crossed during recon of a scale=large phase; step 1 is an atomic ABI cut
that must not start unless it can finish — stopped at the step boundary on a green-building tree.

## Session log — 2026-07-03 — session-id: phase3-executor-2026-07-03-m5-seg2
Phase 3, segment 2 — STATUS: PARTIAL, resume-at `phase-3/step-1` (handoff-phase-3.md §H added).
Segment landed: FRAGO 007 anchor re-verification against the live worktree (all §A/§C recon
anchors current within ±2 lines, receipts in handoff §H.0) + the step-1 cut design advanced to
implementation grain: exact new runtime signatures (H.1), runtime_decls deltas (H.2), the emit.rs
map choke-point helper API threading array_elem_size / array_elem_src_ptr / array_elem_out_buffer
/ array_elem_bits_from_out (no second derivation — CCIR-2 clean by construction, H.3), the 8-site
migration table with confirmed MapEntry bits + Check 2c preservation notes (H.4), and 6 RED-matrix
fixture sources fully specified with expected stdout incl. the D13 snapshot cell and the step-5(b)
MapEntry-aliasing cell (H.5), plus post-cut exit obligations (H.6). ZERO tree changes (reads
only). Checkpoint reason: nudge at ~153k before the atomic cut could begin; starting a hard cut
below its realistic token cost strands a mid-step broken tree — stopped at the step boundary on a
green-building tree. No commit (PARTIAL rides uncommitted, FRAGO 004 discipline).

## Session log — 2026-07-03 — session-id: phase3-executor-2026-07-03-m5-seg3
Phase 3, segment 3 — STATUS: PARTIAL, resume-at `phase-3/step-1-redmatrix-remaining`
(pointer ADVANCED — the seg-1/seg-2 stall pattern is broken: real code landed this segment).
Landed the step-1 atomic map ABI cut per handoff §H.1–H.4 + H.6(a): ynz-runtime map region
rewritten to the elem_size-aware by-value ABI (YnzMap +elem_size, vals *mut u8, one val_slot
helper, all 5 buffers counted ynz_alloc/ynz_free, flag-return get/iter, src-ptr set, realloc
extern deleted); runtime_decls.rs retyped; emit.rs gained the map choke-point sub-section
(map_new/map_val_set/map_val_get_into/map_val_get_maybe/map_iter_get_into/map_count_val/
map_has_int) reusing array_elem_size/array_elem_src_ptr/array_elem_out_buffer/
array_elem_bits_from_out directly (CCIR-2 clean); ALL 8 call-site groups migrated hard-cut
(old pair/triple ABIs deleted — a missed site fails to compile). Grep gate H.6(a) passing.
13 golden IR snapshots verified as pure decl-signature churn and refreshed (34/34 green).
Workspace build GREEN. Full workspace test suite re-run launched at segment end (first run
stopped at the golden churn); result to be confirmed by segment 4 alongside the §H.5 RED
matrix, §H.6(b) checklist ticks, and §H.6(c) alloc re-pins. Recorded decision: map inserts
marshal against the map's DECLARED value type (layout-authoritative), not the value expr's
type. No commit (step 1 incomplete — FRAGO 004: work rides uncommitted). Handoff rewritten
in place to post-implementation truth.
ADDENDUM (same session, post-suite): full workspace suite confirmed — only failures were the
3 M3D map alloc-parity tests (alloc=11 free=1, gap 10 = 2 maps × 5 now-counted never-dropped
buffers; Paper-Trace in segment return); re-pinned per §H.6(c) to
m3d_assert_fires_byte_identical_alloc_gap(…, 10) with WHY comments
(integration.rs v03_m3d_{return_class_map,same_callee_map,danger_map_match_arm}); re-run
3/3 green → integration 501/501, workspace suite GREEN. Handoff Build+test section and
plan.md Phase 3 STATUS block updated accordingly.

## Conductor operating note — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable (Patrick's review footnotes on segment 3's map ABI cut, recorded for the Phase-3 boundary gate — not a blocker, not self-graded)
Patrick reviewed segment 3's map ABI cut output directly (a static read, not a build/run) and
flagged two honest footnotes to carry into the phase-boundary review. Recording verbatim so
they aren't lost between now and Step 5's reviewer fan-out; the conductor is NOT ruling on
either — router-not-inspector holds, these are routed to the fleet, not adjudicated here.

1. **Loosened leak-parity assertions are honest accounting, not a new leak.** The re-pinned
   `m3d_assert_fires_byte_identical_alloc_gap(…, 10)` cells (segment 3 addendum, above) encode
   alloc=11/free=1 as the expected gap. Patrick's read: this gap is PRE-EXISTING map-local
   leakage that was always there, made VISIBLE only because the cut now routes what used to be
   raw `malloc` calls through the counted `ynz_alloc`/`ynz_free` allocator (segment 3's runtime
   cut, lib.rs ~:581-1000) — not a regression the cut introduced. This matches the Paper-Trace
   segment 3 already recorded (10 = 2 maps × 5 now-counted never-dropped buffers) and the E8
   risk row's own framing (D6: parity gate targets "no NEW leak class vs the pointer repr," not
   literal alloc==free). Flag for the Phase-3 boundary reviewer: confirm the 10-cell gap is
   fully attributable to visibility-not-leak, per the parity semantics FRAGO 009 re-specified —
   do not wave this through on the conductor's or Patrick's say-so; the reviewer fleet verifies
   it against the actual gap accounting, same as it would any other parity claim.
2. **Map call-site IR has zero golden-snapshot coverage.** Segment 3 refreshed the 13 EXISTING
   golden IR snapshots (verified as pure decl-signature churn) but authored no NEW golden
   snapshot asserting the new map choke-point call sites' actual generated IR shape. This is
   explicitly test-quality's lane at the Phase-3 boundary gate (test-quality reviewer — is the
   test suite actually meaningful, not just green) — flagged here so it isn't missed as "already
   covered" by the refreshed-snapshot work, which covers regression-detection on the OLD sites'
   IR shape, not confirmation that the NEW choke-point sites' IR is correct in absolute terms.
3. **Compile/link/runtime correctness is unverifiable from this static read** — that is
   precisely what green-check (Step 4) and the reviewer fan-out (Step 5) are for. Patrick's
   footnotes are read-level observations to route to the fleet, not a substitute for the actual
   phase-boundary gates; segment 4 (or whichever segment closes Phase 3) still runs the full
   gate + review sequence before any commit lands.

These three items are carried forward as required phase-boundary review inputs — cite this
note explicitly when dispatching code-reviewer / test-quality at Phase 3's Step-5 fan-out.

## Session log — 2026-07-03 — session-id: phase3-executor-2026-07-03-m5-seg5
Phase 3, segment 5 — STATUS: PARTIAL, resume-at `phase-3/step-2` (pointer ADVANCED from seg-4's
`phase-3/step-1-mapiter-fix`; handoff-phase-3.md rewritten in place to post-fix truth).
**Step 1 COMPLETE.** Fixed the loop-arm `entry.value` miscompile: indirection-level mismatch on
the MapEntry local-slot contract — both map for-loop arms registered the {i64,i64} entry struct
alloca directly as the local while `load` (emit.rs:18876) / `store` (:18709) /
`materialize_param` (:11659-11666) all expect the slot to HOLD a POINTER to the struct; `load`
therefore reinterpreted key_bits as the struct pointer (scalar garbage / shape SIGSEGV — exactly
seg-4's dossier symptoms; runtime + choke points confirmed never wrong). Fix: pointer-indirect
`entry_var_slot` per the canonical materialize_param pattern in both arms (non-SM `mf_*` stored
once pre-loop; SM `sm_mf_*` entry-hoisted + per-body-bb re-store, Check 2c contract preserved).
Receipts: m5_p3 matrix 6/6 green; NEW `m5_p3_map_embed_repr` fixture+test locks the previously
fixture-less debug-repr walker (audit site 7 — verified reachable via shape-embedded map fields,
scalar + shape-valued elem_size>8 cells); map grep gate re-verified (9/9 raw refs inside
emit.rs:2794-2971); audit-map-callsites.md fully ticked; full workspace suite 2235 passed /
0 failed (integration 508/508). Surfaced, not chased: single-run load-flake
`v03_m3e_alias_local_name_collision_runs_correctly` (concurrency-diagnostic, no maps/loops in
fixture, passes isolated + both full re-runs) — for the boundary reviewers. Checkpoint reason:
context budget crossed right at the step-1→step-2 boundary; step 2 is a multi-file
typeck/registry/docs/gallery lift that must not start unless it can finish — stopped on a fully
green tree (early seam; planner's mark sits after step 2). No commit (FRAGO 004 — phase
incomplete, work rides uncommitted).

## Session log — 2026-07-03 — session-id: phase3-executor-2026-07-03-m5-seg6
Phase 3, segment 6 — STATUS: PARTIAL, resume-at `phase-3/step-4` (pointer ADVANCED from
seg-5's `phase-3/step-2`; handoff-phase-3.md rewritten in place to post-lift truth).
**Steps 2 AND 3 COMPLETE.** Step 2: ArrayShapeRuntimeFieldWithWait LIFTED — check.rs
Check 2d + 3 helpers deleted (find_let_initializer_in_stmts verified single-consumer via
repo-wide grep BEFORE deletion — seg-5's open item, resolved); promotion-probe decline arm
removed; queries.rs decline test inverted to host_promotes_and_compiles_clean (PASSES —
promotion verified empirically, not assumed); registry deferral retired +
design_future_sync.rs SKIP entry (M3e precedent); tmLanguage.json regenerated (grammar
embeds deferred-feature data; its sync test caught the retirement — 1-line diff);
IMP-concurrency.md section rewritten interim-guard→LIFTED; m3a gallery trigger removed.
Step 3: the 3 former guard-rejection fixtures repurposed (git mv) as crossing-wait
acceptance — m5_p3_array_shape_{runtime_field,between_waits,nested_if}_runs, each
asserting exact stdout "30" — ALL GREEN (the scratch doc's acceptance signal, E9 B1);
array constructibility proven (map's maybe-after-wait finding does not transfer — no
maybe read in these cells); maybe-crossing obligations recorded note-only in handoff.
Deviation surfaced for the judge: error_galleries.rs has NO v0_3_m3a gallery test (plan
said "update counts/phrases"; grep receipt: only m4-m8/v0_3_m1/m3b/m4 galleries wired) —
no-op, risk-neutral recon drift. Housekeeping: stray untracked ELF
fixtures/m5_p2_byval_map_set_escape deleted. Receipts: full workspace suite
2236 passed / 0 failed (--no-fail-fast); build GREEN. Checkpoint at the planner's
post-step-2 CHECKPOINT mark (context budget; step 3 rode along because its deliverable
WAS step 2's repurposed fixtures). No commit (FRAGO 004 — phase incomplete).

## Session log — 2026-07-03 — session-id: phase3-executor-2026-07-03-m5-seg7
Phase 3, segment 7 — STATUS: PARTIAL, resume-at `phase-3/step-5` (pointer ADVANCED from
seg-6's `phase-3/step-4`; handoff-phase-3.md rewritten in place to post-gate truth).
**Step 4 COMPLETE — E8 parity gate GREEN.** FRAGO 005 entry criterion proven, fail-loud:
`m5_p3_e8_gate_visibility_{arrays,maps}` pin the P0-baseline-blind fixtures (m5_array.ynz
0/0-blind → 2/0; m5_p3_mapshape_runtime_int.ynz → 5/0) with explicit gate-is-VACUOUS
failure messages. Gate proper: NEW `m5_p3_e8_parity_gate.ynz` + test pins EXACT
alloc=11/free=0 — Paper-Trace predicted before first run, observed matched, zero residual
(2 array buffers + 5 map buffers + 4 FRAGO-011 persist cells; 40-iteration read loop = 0
allocs = the per-iteration regression pin; 3 map shape sets incl. overwrite = 0 cells).
Step-4 verdict (assigned here by FRAGO 009/011) RECORDED: parity GREEN in the no-NEW-leak-
class sense → no drop insertion (YAGNI until FR #6), no D6 fallback; interim
`alloc == free + gap` helper encoding RATIFIED durable (doc updated,
m3d_assert_fires_byte_identical_alloc_gap). Map-side answer: the cut REMOVED the map
re-set-over-key cell leak (shape values inline, lib.rs:585-590); deliberate accounted
cells remain only for shape-field re-assign + maybe persists. Seg-5-fix neutrality
receipt: 72/72 v03_m3d green incl. all 7 gap-pinned tests. Housekeeping: prior segments'
tree was not rustfmt-clean (emit.rs/integration.rs/lib.rs, pre-existing regions) —
`cargo fmt --all` applied (mechanical), fmt --check exit 0, build + gate tests re-green.
Step-5 recon banked in the handoff (twin-site list verified vs live code incl. a third
same-class site at emit.rs:14519; bg array<Shape> alias finding; D12/(e) ratification
analyses). Checkpoint at the step-4→step-5 boundary on a green-building tree (context
budget). No commit (FRAGO 004 — phase incomplete).

## Session log — 2026-07-03 — session-id: phase3-executor-2026-07-03-m5-seg8
Phase 3, segment 8 — STATUS: PARTIAL, resume-at `phase-3/step-5-item-c-done` (pointer ADVANCED
from seg-7's `phase-3/step-5`; handoff-phase-3.md rewritten in place to post-work truth).
**Step 5 item (c) LANDED:** FRAGO 010 size-derivation twin UNIFIED at all three sites
(bind_sm_result_and_flush shape arm, SM shape-embed Let arm, prepare_bg_arg_for_ctx Shape arm
incl. the free-side fallback-to-0) onto shape_abi_size_const/shape_abi_sizes; zero shape-size
struct_ty.size_of() sites remain; stale twin-flag comments rewritten; golden snapshots 34/34,
zero refreshes (seg-7 prediction held). **bg×array<Shape> alias FIX LANDED** (probe-mandated,
D-r2-1 class, surfaced not self-classified): inline-elem clone extension in
prepare_bg_arg_for_ctx; Paper-Trace RED 119 → GREEN 30. **Sweep probes all run** (receipts in
handoff): TWO live MapEntry-escape silent-wrongs found (bg-arg read advanced slot 2/20;
array<MapEntry> escape 20/20) — fix design banked, not yet landed; step-5(e) union
parenthetical FALSIFIED (write-side persists compile+run; read-back ICEs loudly) — loud-reject
gate surfaced as FRAGO-grade, NOT self-built; D12 RATIFIED on record (pointer identity, final
for M5, FRAGO-candidate) + (e) ratified alongside; NEW note-only pre-existing finding: bare
`.copy()` on arrays aliases (P3c-era deferral). Build/fmt/clippy green; m5_p3 13/13, v03_m3d
72/72, v03_m3b_p2_bg 2/2. Checkpoint mid-step-5 at an item boundary per the conductor's own
fine-grained-marker dispatch instruction, on a green-building tree (context budget). No commit
(FRAGO 004 — phase incomplete, work rides uncommitted).

## Session log — 2026-07-03 — session-id: phase3-executor-2026-07-03-m5-seg10
Phase 3, segment 10 — STATUS: COMPLETE — THE PHASE IS SEALED. The 7 step-5 sweep/pin fixtures +
integration tests authored per seg-9's banked designs (zero probing; observed pins used
verbatim): m5_p3_sweep_bg_array_shape_copy (`caller set qty: 99` / `task total: 30`),
m5_p3_sweep_bg_array_shape_give_wait (the E9 AoS SM-main cell — inferred give + spawn-site
.copy() + wait-crossing + post-wait mutation; `caller: 119` / `given: 30` / `copied: 30`),
m5_p3_sweep_mapentry_bg_escape (1/10), m5_p3_sweep_mapentry_array_escape (10/20),
m5_p3_sweep_shape_eq_string_field (D12 pin, true/true/false — cell 2's LLVM literal-merge
artifact documented in the WHY), m5_p3_sweep_union_readback_blocked_{array,map} (loud-fail
pins: exit != 0 + empty stdout in BOTH modes). Every executable cell asserts dual-mode
byte-identity + exact stdout via a shared m5_p3_sweep_assert_dual_mode helper. ALL 7 GREEN
FIRST RUN. Full workspace suite 2246 passed / 0 failed / exit 0 (reconciles: seg-6 2236 + 3
step-4 gate tests + 7 sweep tests). Dual-mode oracle regenerated per p2-dualmode-report.md
§Methodology over all 466 fixtures: 379 identical / 83 skip / 2 documented DIFFs
(model_a_intended_reorder, overlap_proof) / 2 timing NONDETs (concurrent_waits_proof,
e8_pool_exhaustion_stress) / 0 anomalies / 0 REAL DIVERGENCES — reconciliation vs the
post-round-3 run exact (+19 fixtures = 4 round-4 + 8 P3 map + 7 sweep; +20 identical incl. the
3 repurposed guard fixtures ex-SKIP; report updated with the new-run row). Methodology note: a
first sweep draft mis-bucketed runtime-nonzero-exit fixtures as SKIP (90/372 tallies); caught
by reconciliation against the prior run's bucket math, corrected to the established
build-fail-only SKIP semantics, and re-run in full — the corrected tallies above are the run
of record. fmt --check exit 0; clippy clean on the new test code (pre-existing driver test
warnings untouched, carried). Plan STATUS ticked COMPLETE with the full 10-segment summary;
frontmatter session-id chain repaired (segs 2-9 were logged here but never appended to
plan.md's chain — appended from this file's own trail, plus seg10). Boundary commit staged via
explicit pathspec from `git status --porcelain` (all 10 segments' work; nothing was previously
committed per FRAGO 004), trailer `Plan-Phase: 2026-07-03-v0-3-m5-auto-soa#3`, secret-scan
fallback note in body per the FRAGO 004 waiver convention (fallback grep sweep run over the
staged diff — receipts in the segment return). handoff-phase-3.md deleted as the final act.
Carry-forwards for the boundary review enumerated in the segment return (Patrick's two
footnotes; M3e load-flake; union plan-text falsification FRAGO candidate; bare-.copy() alias
note; D12/(e)/MapEntry/bg-alias FRAGO-candidate classifications; error_galleries recon drift;
step-4 verdict paperwork; pre-existing driver-test warnings; suite-count sensitivity).

## Phase 3 boundary review — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable
Cheap gates (green-check + graveyard-auditor) + full reviewer fan-out (code-reviewer,
acceptance-verifier, rules-compliance, deviation-judge, test-quality, + a dedicated adversarial
gate-checker per the plan's declared fan-out) run against the sealed commit `e06172f6bb31a
63992328a3d87abac9763a69b64`. Summary:
- green-check: GREEN. 2246 tests / 0 failed, clippy clean, fmt clean, build clean. Secret-scan
  pass via independently-re-run fallback sweep (not taken on the commit message's word).
- graveyard-auditor: CLEAN, 6 corpses adjudicated, 0 findings. Independently re-verified the
  FRAGO-010 size-twin unification claim by grep (confirmed complete, not merely asserted).
- code-reviewer: CHANGES-REQUESTED, 0 blockers, 2 should-fix — (1) `store_binding` (emit.rs:
  ~18907) has NO MapEntry deep-copy arm (falls through to raw pointer store) on a THIRD,
  un-probed surface of the same escape-bug class this phase fixed twice already (a
  MapEntry-typed return or an escaping `let e2 = entry` binding could carry a dangling
  pointer); (2) zero golden-IR-snapshot coverage on new map choke-point call sites (= Patrick's
  footnote 2, independently reconfirmed).
- acceptance-verifier: MET. All 5 phase exit-criteria clauses independently confirmed against
  actual fixtures/tests/source (not the executor's own STATUS prose).
- rules-compliance: CLEAN, 0 findings across 6 rule areas (authoritative-derivation,
  feature-registry, plan-invariants, vocabulary/naming, docs-checklist, stdlib-design).
- deviation-judge: ON-PLAN. 5 divergences judged; 4 fine-as-is (D12 ratification, MapEntry
  fixes, bg-alias fix, error_galleries no-op — all plan-delegated or squarely in the
  adversarial-sweep step's own named scope); 1 JUSTIFIED FRAGO candidate, risk-neutral:
  plan.md step 5(e)'s text ("no fixture can construct a union persisting through the choke
  points") is FALSIFIED by probe evidence — write-side persistence through
  `map<K,Union>.set`/`array<Union>` literals compiles+runs, read-back ICEs loud (no silent-wrong,
  zero observable exposure). D6's full loud-reject diagnostic correctly NOT self-built
  (out-of-scope new user-facing diagnostic + registry/gallery obligations); documented +
  loud-fail-pinned instead.
- test-quality: CLEAN, 0 blockers, 1 should-fix (same golden-IR-snapshot gap as above,
  independently judged low-priority defense-in-depth — runtime-behavior testing is the correct
  primary verification tool for genuinely NEW logic, which is what caught both real miscompiles
  this phase).
- adversarial gate-checker (general-purpose, per the plan's declared fan-out): ALL 5 charged
  items PASS with independently-reproduced command receipts (P0 map-callsite checklist rows
  spot-verified against real fixture content; grep gate re-run directly — 10 hits, all confined
  to the choke-point section; RED matrix 7/7 green re-run; sweep fixtures 7/7 green re-run;
  parity gate 3/3 green re-run). No fabrication found.

**Routing (0 blockers across all 6 dispatches → no fix loop):**
1. FRAGO 012 applied below (risk-neutral, auto-apply + log — deviation-judge's classification).
2. Two should-fix non-blockers routed to the per-phase durable-deferral home (§6.1), NOT fixed
   this phase: the `store_binding` MapEntry third-surface gap, and the golden-IR-snapshot gap.
   Neither is a capability discovery (both are in-scope correctness/coverage nits within M5's
   own by-value-storage domain) — nit-path only, payload to the roadmap's `audit.md`, no
   Capability Ledger row.

## FRAGO 012 — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable (deviation-judge classified JUSTIFIED; risk-neutral; auto-apply + log)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 3 sealed (commit e06172f6), boundary review
Trigger:   Phase 3's own adversarial sweep (step 5(e)) built a probe to test the plan's own
           claim — plan.md's step 5(e) parenthetical states "no fixture can construct a union
           persisting through the choke points." The probe proved this FALSE:
           `map<K,Union>.set` and `array<Union>` literals both compile and run on the write
           side (raw-pointer persist through the marshalling choke points); only the read-back
           path ICEs (loud, not silent-wrong). Deviation-judge (boundary review, this session)
           independently confirmed the falsification against actual source and classified it
           JUSTIFIED — the correct response to a falsified plan assumption mid-sweep is to
           document the true reachability + lock it with a loud-fail regression pin (both done:
           `value_to_stable_bits`'s KNOWN-HOLE doc refreshed; `m5_p3_sweep_union_readback_
           blocked_{array,map}` pin both build modes), NOT to self-author D6's full loud-reject
           diagnostic mid-sweep (that is out-of-scope new user-facing-diagnostic + registry/
           gallery work this step was never resourced for).
Risk:      RISK-NEUTRAL. The scenario was previously believed unreachable (assumed zero
           surface); it is now proven reachable-but-loud-blocked (an ICE, not a bad output) —
           no new observable exposure was introduced by leaving D6 unbuilt. No re-run of the
           deterministic risk matrix required (no HIGH residual, no signature gate).
Changes:   plan.md Phase 3 step 5(e) text corrected (applied by a re-dispatched executor, not
           the conductor directly — plan-body edit stays out of conductor charter) to replace
           the falsified "no fixture can construct..." parenthetical with the probe-verified
           truth + a forward pointer to the KNOWN-HOLE doc and the two loud-fail pins.
Unchanged: D6's contingent loud-reject diagnostic remains UNBUILT, by design — parked as a
           standing follow-up item (not a milestone-owned capability; no new roadmap ledger
           row) should union-narrowing ever mature enough that this needs a real diagnostic
           instead of a documented loud ICE.
Override:  N/A — risk-neutral, no signature required per the risk engine.

## Session log — 2026-07-03 — session-id: phase4-executor-2026-07-03-m5-seg2
Phase 4, segment 2 — STATUS: PARTIAL, resume-at `phase-4/step-2` (handoff-phase-4.md rewritten
in place as current truth). Segment landed: **step 1 complete and green** —
`crates/ynz-typeck/src/soa.rs` (new: SOA_SIZE_THRESHOLD, SoaDeclineReason/SoaVerdict/SoaCandidate,
pure `analyze()` core with the full two-pass walk per handoff §B: expr_types type oracle,
loop-context field-union counting, escape/growth/lend-self/param-row classification, banked
decline precedence), `soa_candidate_query` in queries.rs (cpu_promotion_query idiom: lru=64,
cycle pair, entry gates no_auto_parallel_env() + kernel_mode=false + has_errors→empty), lib.rs
module + re-exports. Receipts: docker `cargo build --workspace` green; `cargo clippy -p
ynz-typeck -- -D warnings` clean; `cargo fmt --all -- --check` clean; `cargo test -p ynz-typeck`
all 26 result lines `0 failed`. Full-workspace suite deliberately deferred to the step-2
CHECKPOINT mark (step 1 adds an unconsumed module only — no behavior change surface). Both
segment-1 ⚠ items resolved and recorded in handoff §A (AST field names: `For.iter`,
`ArrayLit.elements`; padding_gate assertion style + helper names). Checkpoint reason: context
nudge fired (~158k) at end of at-code-time verification; landed step 1 to a green-building tree
and stopped at the step-1/step-2 boundary rather than starting the multi-signature re-threading
cut below its realistic token cost. No commit (PARTIAL rides uncommitted, FRAGO 004 discipline).
Deviations: none new (third-padding-consumer finding was segment 1's; its coverage is banked in
the step-2 design). Checkbox sync: plan uses numbered steps, no `- [ ]` boxes in Phase 4 — the
session-id chain + this entry are the sync surface.

## Session log — 2026-07-03 — session-id: phase4-executor-2026-07-03-m5-seg3
Phase 4, segment 3 — STATUS: PARTIAL, resume-at `phase-4/step-3` (checkpointed AT the plan's
own step-2 CHECKPOINT mark; handoff-phase-4.md rewritten in place as current truth). Segment
landed: **step 2 complete, all CHECKPOINT gates paid.** The layout authority in soa.rs
(FieldSegment/LayoutKind/LayoutDecision/LayoutDecisions + resolve_layout — D11 padding-wins:
Admitted ∧ padded → Aos{CrossThreadPadded}; Admitted survivors → Soa with ALL declared fields,
declared order; segments carry order+names only, NO ABI-size re-derivation per CCIR-2) +
`layout_decisions_query` (same salsa idiom; padded set cloned UNCONDITIONALLY so re-threading
is byte-identical) + lib.rs exports. All THREE padding consumers re-threaded to the authority:
frame_layouts_query sizing, codegen_query→emit_artifact (new `layout` param), emit.rs Pass 0 +
deep struct-lit alignment read (`cg.layout.padded_shapes`); `layout` threaded through
build_module → lower_function → lower_function_with_waits AND lower_generic_function (real
authority, not an empty stand-in); Cg field set in all 3 constructors; check.rs
cross_thread_padded_shapes doc rewritten to raw-record status. Receipts (Docker): build green;
clippy --workspace -D warnings clean; fmt --check clean; **full suite 2251 passed / 0 failed**
(≥2246/0 P3 baseline; +5 delta NOT from this segment — zero tests added by steps 1–2; flagged
in handoff for the DONE return); **zero pending snapshots = zero golden-IR churn** (pure
re-threading proven); dual-mode padding spot-run
`v0_3_m4_p4_padding_gates_off_under_no_auto_parallel_with_identical_output` ok. Grep gates:
`cross_thread_padded_shapes` in crates/ynz-codegen → ZERO hits (comments included);
YNZ_NO_AUTO_PARALLEL env-parse only at queries.rs:598. Checkpoint reason: planner-placed
CHECKPOINT mark after step 2 + context nudge (~150k+) — green-building tree at the step
boundary. No commit (PARTIAL rides uncommitted). Deviations surfaced (not self-classified):
(1) seg-2 handoff receipt had the 1408/3308 Cg-constructor LABELS swapped (count/sites
correct — corrected in handoff §A, no behavior impact); (2) pre-existing `unused variable:
stderr` warning at crates/ynz-driver/tests/integration.rs:672 (test target, outside house
clippy scope, untouched by this diff) — surfaced for the record; (3) suite-count observation
2251 vs 2246 seal aggregation, noted above. Checkbox sync: plan uses numbered steps, no
`- [ ]` boxes in Phase 4 — the session-id chain + this entry are the sync surface; steps 3–5
remain open (open↔open), steps 1–2 done (done↔done via this trail).

## Session log — 2026-07-03 — session-id: phase4-executor-2026-07-03-m5-seg4
Phase 4, segment 4 — **STATUS: DONE (phase complete, all 5 steps + all exit criteria).**
Resumed at `phase-4/step-3` per handoff-phase-4.md; inherited the §A/§B/§E receipts (settled
fixture/test plan, exact assertion strings, db/ENV_LOCK patterns, padding_gate anchors) and
re-verified only new work. Landed: **step 3** — `m5_p4_soa_both_candidate.ynz` (mirrors
v0_3_m4_p4_padding_gate.ynz + 66-elem qualifying `array<Tally>` hot loop) + integration test
`m5_p4_soa_both_candidate_padding_wins_byte_layout` (default-mode IR keeps `{ i64, [56 x i8] }`
padded slots; stdout exact `tally total: 5\n2211\n4422\n`; dual-mode byte-identical). **Step 4**
— the 7 remaining decline fixtures (`m5_p4_soa_{qualifying,threefield,escaping,growth,lendself,
runtime_length,small_n}.ynz`), each tripping exactly ONE precedence cell; all 8 fixtures run
green through the real binary with exact tabled stdout, first run. **Step 5** —
`crates/ynz-typeck/tests/soa_analysis.rs` (12 tests, queries.rs:1361 db pattern + file-local
ENV_LOCK held by every test): exact verdict/reason payloads incl. row-count pins (escaping = 2
rows: param + call-arg escape with exact `how` strings; runtime_length = 2 rows: returned-escape
+ LengthNotProvable); authority cells — qualifying → `Soa { segments: [x@0, y@1] }` (E10
surface asserted on real `FieldSegment` values), both-candidate → candidate-walk Admitted{66,
[hits,outs]} AND authority `Aos { CrossThreadPadded }` + padded set ∋ Tally (D11); entry gates
— env-flag fresh-db → empty (with sanity ON half), kernel via pure `analyze(_,_,true,false)` →
empty (with sanity OFF half). Receipts (Docker): soa_analysis 12/12; integration test 1/1;
**full workspace suite 2264 passed / 0 failed** (= 2251/0 seg-3 baseline + exactly the 13 new
tests — reconciles with zero unexplained delta); clippy --workspace -D warnings clean; fmt
--check clean (cargo fmt applied to the new test file first); zero `*.snap.new`. Grep gates
re-paid at DONE: `\.arrays` in ynz-codegen → sole hit is the emit.rs:854 param doc stating the
exit criterion (zero code reads); `env::var("YNZ_NO_AUTO_PARALLEL")` parse only queries.rs:598;
`cross_thread_padded_shapes` in ynz-codegen → zero hits; abi_size/size_of in typeck soa.rs →
doc-comment-only. Recorded decision (on the record, not a deviation): exact-stdout integration
tests for the 7 non-both-candidate fixtures NOT added — handoff §E marks them nice-to-have
only; runtime behavior receipted via direct binary runs this segment, and Phase 5's dual-mode
oracle sweeps all fixtures. Deviations: NONE new this segment; prior segments' surfaced items
stand (seg-3 log). Checkbox sync: plan uses numbered steps, no `- [ ]` boxes in Phase 4 — the
Phase 4 STATUS block (added this session) + session-id chain + this entry are the sync surface;
all 5 steps done↔done, no steps remain open. handoff-phase-4.md DELETED as the final action of
this segment (sole-owner completing-executor act). No commit (conductor's call at the boundary).

## FRAGO 013 — 2026-07-03 — session-id: plan-conductor-2026-07-03-m5-fable (deviation-judge classified JUSTIFIED; risk-neutral; auto-apply + log)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 4 sealed (pre-commit — boundary review, this session)
Trigger:   Phase 4's ¶1 recon + step 2 text (plan.md, citing `emit.rs:1051`/`codegen/queries.rs:204`)
           names TWO padding consumers M4's false-sharing padding transform reads
           `TypedModule::cross_thread_padded_shapes` through. Segment 1's CCIR-1 re-verify (mandated
           by A1 — that recon ran against uncommitted M4 P4 code, explicitly flagged unverified) found
           a THIRD real consumer at what is now `emit.rs:16848` (a deep `Cg`-level
           `cross_thread_padded_shapes.contains()` read inside struct-lit alignment codegen). Segments
           2-3 re-threaded all three sites to read the new `LayoutDecisions` authority; a grep confirms
           zero remaining direct reads of `cross_thread_padded_shapes` anywhere in `ynz-codegen`.
           Deviation-judge (boundary review, this session) independently re-verified the grep result
           on disk and classified JUSTIFIED: the plan's own A1 assumption and Design-Doc-Alignment
           divergence #8 explicitly flagged this exact citation as unverified and mandated the
           re-verify that caught it — this is the mechanism working as designed, not a stray.
Risk:      RISK-NEUTRAL-TO-LOWERING. Missing the third site would have left a live second-derivation
           path outside the new authority — exactly the E3 twin-derivation class this phase exists to
           eliminate. Covering it is what makes the grep gate (E3's B1 mitigation) actually hold, not
           a new exposure. No re-run of the deterministic risk matrix required (no HIGH residual, no
           signature gate).
Changes:   plan.md Phase 4 ¶1 recon text + step 2 text corrected (applied by a re-dispatched executor,
           not the conductor directly — plan-body edit stays out of conductor charter) to name all
           THREE padding consumers (`emit.rs:1057`, `emit.rs:16848`→16872 post-rethread,
           `codegen/queries.rs:208-212`) instead of two, and to note the RETIRED
           `emit.rs:13104-13107` const-global-fold cite (made obsolete by Phases 2-3's by-value cut;
           already flagged by the conductor's resume brief, formalizing it here).
Unchanged: The re-threading itself is already complete and grep-gate-verified — this FRAGO is a
           plan-text citation correction, not a design or scope change.
Override:  N/A — risk-neutral, no signature required per the risk engine.

## Session log — 2026-07-03 — session-id: plan-fixup-frago013-2026-07-03-m5
Applied FRAGO 013's plan.md correction: Terrain padding-consumers bullet (plan.md:48-54) now names all THREE consumers with post-rethread lines; Terrain ArrayLit bullet (plan.md:41-42) marks the emit.rs:13104-13107 const-global-fold cite RETIRED; Phase 4 step 2 (plan.md:877-883) citation corrected to the three re-threaded sites; sibling sweep per plan-source-of-truth also corrected Design-Doc Alignment ¶8 (plan.md:196-201), which restated both stale facts. Session-id appended to frontmatter chain in the same action.

## Session log — 2026-07-03 — session-id: phase4-executor-2026-07-03-m5-closing
POST-BOUNDARY-REVIEW CLOSING ROUND — not a new phase. Phase 4's reviewer fan-out (8 dispatches)
returned zero blockers + two should-fix findings; both fixed this round. (1) Reassign-staleness
(code-reviewer, correctness): the walk's `Stmt::Assign` arm only scanned the RHS — a tracked
binding reassigned mid-function kept its ORIGINAL initializer's provable_len/escape/growth
signals, so a stale Admitted verdict could reach Phase 5's buffer sizing (E3/E7 class; inert
today, zero codegen consumers). Fix: Assign with a tracked target now declines it via the
existing enum — `Escapes { how: "reassigned after initial binding" }` (soa.rs, split
Let/Assign arms; the shared RHS-alias check consolidated onto `scan_ident_in_value_position`,
byte-identical message). Fixture `m5_p4_soa_reassigned.ynz` (otherwise-qualifying 72-element
binding reassigned to a 4-element sibling; asserts BOTH halves — target reassign-decline +
RHS alias-escape on `replacement`). Note: an Assign-position array-of-shape literal does not
type (annotation-driven literals need the `let` annotation; verified via real-binary
diagnostic), hence reassign-from-binding. (2) `NoPerFieldLoopAccess` (adversarial
gate-checker, completeness): live branch (soa.rs `sig.field_union.is_empty()`) had zero
fixture/test coverage — plan-table gap, not executor miss. Fixture
`m5_p4_soa_no_field_access.ynz` (66-element, count-only loop) + exact-reason test.
Receipts: workspace 2266 passed / 0 failed (= 2264 baseline + exactly the 2 new tests);
soa_analysis.rs now 14 tests, all 12 pre-existing exact-verdict tests unchanged-green — the 8
original fixtures' verdicts UNCHANGED by the fix (only reassigned bindings are affected);
clippy -D warnings clean; fmt --check clean; zero *.snap.new; both fixtures run clean through
the real binary (stdout `10\n20` and `66`); grep gates re-held (YNZ_NO_AUTO_PARALLEL parse
only at queries.rs:598; zero cross_thread_padded_shapes reads in ynz-codegen; `.arrays` sole
codegen hit = emit.rs:854 doc comment; no ABI-size derivation in soa.rs). Plan sync: Phase 4
STATUS block extended with the closing-round record; session-id appended to frontmatter chain
in this same action. No handoff file created (single segment). Deviations: NONE. No commit
(conductor's call).

## Session log — 2026-07-04 — session-id: phase4-executor-2026-07-03-m5-closing2
SECOND CLOSING ROUND — not a new phase. Code-reviewer's re-check of closing round 1 confirmed the
Assign fix closed correctly but surfaced ONE sibling gap in the same risk class: Pass 1
(`collect_bindings_block`) kept the FIRST record on a same-name re-`let` shadow
(`if !st.bindings.contains_key(name)` skipped the second initializer). Same-scope re-`let`
shadowing is LEGAL Yinz in non-suspending functions (check_let tail = unconditional
`scope.insert`, no duplicate diagnostic; proven fixture
v0_3_m3a_p2_r7_nonasync_local_shadow_compiles.ynz), so a 72-element qualifying binding shadowed by
a 4-element re-`let` stayed `Admitted { provable_len: 72 }` — a 68-element over-size into Phase 5's
buffer sizing (E3/E7 stale-signal class, Pass-1 path; the re-`let` sibling of round 1's Assign
fix). Fix: any `Stmt::Let` whose name is already tracked declines the record via the existing enum
— `Escapes { how: "rebound by a later let" }` — firing regardless of the shadow's own type (a
non-array shadow rebinds the name just the same; strictly conservative, no parallel mechanism).
Pass-1 marking precedes all Pass-2 signals → deterministic first-signal-wins. Fixture
`m5_p4_soa_let_shadow.ynz` (larger-then-smaller, the actual miscompile-risk direction) +
exact-reason test `let_shadowed_binding_declines_as_rebound` (the reason string exists ONLY in the
new arm — the assertion cannot pass without the new path executing).
EXHAUSTIVE REBINDING SWEEP (recorded so no reviewer re-derives it): (1) Let-first — baseline,
handled. (2) Let same-name shadow — THIS fix; covers same-scope AND nested-block shadows (the walk
is flat name-keyed over all nested blocks). (3) Assign — round 1's fix, confirmed closed.
(4) FieldAssign/IndexAssign — verified against check.rs:5755-5790/5920-5950 + nodes.rs:304-313:
IndexAssign is `.set(index, value)` sugar (mutation, cannot change length), FieldAssign mutates a
field through the root binding's existing scope entry; neither rebinds array identity/length → not
a staleness source. (5) For-loop vars (plain var, destructure_pattern, map_destructure_pattern) —
scoped per-element bindings that expire with the loop; a loop var shadowing a tracked name over a
NON-tracked iter yields at worst a conservative spurious escape in Pass 2 (decline direction),
never a stale Admitted → out of this class. (6) Array-typed params — unconditionally declined rows
(`Escapes` cross-function, S2 finding 5), never Admitted → no staleness path exists. (7) Other
constructs in nodes.rs: Match patterns (Value/Is/OptionName — MatchPatternKind, nodes.rs:346-360)
bind NO names; `Expr::Background` spawn captures args by copy/share without rebinding any name;
imports bind module names at module level (not function-body array bindings). NO third gap found;
nothing hypothetical fixed (completeness check, not gold-plating).
Receipts: workspace **2267 passed / 0 failed** (= 2266 baseline + exactly the 1 new test);
soa_analysis.rs now 15 tests — all 14 prior exact-verdict tests unchanged-green, so all 10 prior
fixtures' verdicts UNCHANGED; clippy -D warnings clean; fmt --check clean; zero *.snap.new; grep
gates re-held (YNZ_NO_AUTO_PARALLEL env::var parse ONLY at queries.rs:598 inside the one
`no_auto_parallel_env()` predicate; soa.rs's padded set is the threaded parameter only, no second
derivation; zero ABI-size derivation in soa.rs). Plan sync: Phase 4 STATUS block extended with the
closing-round-2 record; session-id appended to frontmatter chain in this SAME action. Deviations:
NONE. No handoff file (single segment). No commit (conductor's call).

## Session log — 2026-07-04 — session-id: phase4-deferral-executor-2026-07-03-m5
DEFERRAL ROUTING — documentation-only, no code touched, no phase checkboxes affected. The two
pre-existing, out-of-scope minor findings from Phase 4's boundary review (confirmed inert for
Phase 4, flagged for Phase 5) were filed as durable four-field NIT-PATH deferrals in the
ROADMAP's own sidecar (`.claude/planning/active/2026-05-21-v0-3-concurrency-perf/audit.md`) —
NOT this plan's audit.md, because this plan's directory archives at completion while the roadmap
dir stays active. No Capability Ledger row (code-quality deferrals, not capability discoveries).
Both idempotency keys were grep-confirmed absent (whole-line fixed-string) before writing:
- `Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#4: crates-ynz-typeck-src-soa-rs-294` — param
  `arr: array<Shape>` shadowed by a body `let arr` yields TWO `LayoutDecisions.arrays` rows
  sharing one `array_name` (inert today: codegen consumes only `padded_shapes`; Phase 5's
  arrays-consumption design owns the keying/dedup resolution).
- `Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#4: crates-ynz-typeck-src-soa-rs-535` — Pass 2's
  Match-arm scan skips `arm.pattern`'s `Value(Expr)` variant; a tracked array used only inside a
  match-pattern value expression would miss escape classification (unreachable in any current
  fixture; trigger includes Phase 8's suppression-enumeration sweep).
Session-id `phase4-deferral-executor-2026-07-03-m5` was also appended to the ROADMAP's
`roadmap.md` frontmatter session-id chain in this same action (its audit.md is a deferral
sidecar with no session-log convention, so each roadmap entry carries a `Filed-by-session:`
line as corroboration), and to THIS plan's own frontmatter chain — the latter not explicitly
ordered by the dispatch, decided on the record to keep this plan's established
one-audit-entry-per-frontmatter-id pairing intact.

## FRAGO 014 — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable (deviation-judge classified JUSTIFIED; risk-lowering; auto-apply + log)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 5, segment 1 (mid-phase — dispatched before segment 2
           to unblock the step-4 design, not a post-diff boundary review)
Trigger:   Phase 5 step 4's plan text reads "`.copy()` (deep-copies all segments)" — an assumption
           that array `.copy()` already performs a real deep copy today and Phase 5's only job is
           mirroring that across SoA segments. Segment 1's executor found this false: array `.copy()`
           lowers as an ALIAS no-op for ALL arrays in both layout modes today —
           `crates/ynz-codegen/src/emit.rs:17008-17048` (`lower_postfix_op`'s `PostfixOpKind::Copy`
           arm; the catch-all `_ => Ok(recv_val)` at line 17045 returns the receiver's own pointer,
           not a copy) and `crates/ynz-typeck/src/check.rs:5816-5826` (`check_postfix_op`'s Copy arm,
           unconditional per its own stale comment "P3c will enforce trivially-copyable requirement").
           This is a pre-existing M4-era stub, NOT something Phase 5 introduced. `soa.rs:727-730`'s
           design comment and `vocabulary.md`'s locked user-facing contract ("`.copy()` — clone, deep
           copy") both independently assumed the correct (deep-copy) behavior that the live codegen
           does not honor. Deviation-judge independently re-verified every cited line against the live
           worktree at `736a143` (not taken on faith) and additionally grepped every fixture/example
           for array `.copy()` usage, finding no green test depends on the current alias behavior (the
           one array-`.copy()` fixture, `m5_p3_sweep_bg_array_shape_give_wait.ynz`, derives its
           independence from the background-spawn argument memcpy, not from `.copy()`'s own lowering).
Risk:      RISK-LOWERING relative to E9 (residual LOW; mitigation 2 = the SoA×wait×background×.copy()
           dual-mode matrix). E9's own step-4 proof obligation cannot be genuinely exercised against a
           `.copy()` that aliases — a matrix testing a broken primitive could pass vacuously if no cell
           mutates post-copy. Fixing both modes makes E9's matrix meaningful, removes a real
           currently-shipping silent-miscompile class (`arr2 = arr1.copy(); arr2.set(...)` also
           mutates `arr1`), and eliminates the NEW dual-mode-divergence exposure that fixing only
           SoA-mode would create (Key Outcome #2 / CCIR-3 — any dual-mode-byte-difference is
           build-stopping). Deviation-judge considered and rejected two alternatives: (b) decline AoS
           `.copy()` on arrays at compile time instead of fixing it — heavier lift, user-visible
           regression, defers the real fix without reason; (c) match SoA's new `.copy()` to AoS's
           existing alias behavior — REJECTED as the duct-tape option: ships a new silently-wrong path
           by design, violates D4's decline-safely-never-silent-wrong discipline and the already-
           recorded soa.rs/vocabulary.md contracts. No plausible risk-raising vector found: the fix is
           scoped to the SAME match arm already inside Phase 5's own step-4 task; array-element copy
           mirrors the existing `Type::Shape` arm's already-shallow (single-level memcpy) semantics,
           consistent with D12/D13's recorded stance that nested pointer fields (string, nested shape)
           alias for now. No HIGH residual; no signature gate required (Patrick's FRAGO 004 blanket
           override was available but not needed to clear this one).
Changes:   Phase 5 step 4 (plan.md) corrected to read: fix array `.copy()` to perform a genuine
           (shallow, one-level — matching the `Type::Shape` arm's existing memcpy semantics) deep copy
           in BOTH layout modes (AoS and SoA) as part of this phase's `.copy()` work, closing the
           pre-existing bug rather than mirroring it into the new SoA path. Applied by a re-dispatched
           executor, not the conductor directly (plan-body edit stays out of conductor charter).
Unchanged: Steps 1-3 of Phase 5 are unaffected — this ruling is scoped to step 4's `.copy()` cell
           only. The fix touches pre-existing AoS `.copy()` lowering (`emit.rs`'s catch-all arm,
           `check.rs`'s Copy arm) in addition to the new SoA-side segment-copy work; both land in the
           SAME step-4 dispatch per the ruling above.
Override:  N/A — risk-lowering, no signature required per the risk engine.

## Session log — 2026-07-04 — session-id: phase5-executor-2026-07-03-m5

Phase 5 segment 1 (fresh dispatch, no prior handoff): PARTIAL at `phase-5/step-1`.
Paid the phase's full orientation ONCE (receipts recorded in `handoff-phase-5.md` —
authority shape in typeck soa.rs, runtime YnzArray ABI, the E7 choke-point seam in
emit.rs, the complete consumer-site inventory incl. both for-in paths (normal + SM),
IndexAccess/IndexAssign, lower_array_method, debug-print, background-arg; all cites
verified against this worktree's live tree at 736a143 per FRAGO 007) and settled the
implementation design ("design c": gather/scatter at the choke points, staging
semantics byte-identical to AoS, cold-field elision delegated to LLVM DSE, layout
answers read ONLY from `layout_decisions`). Landed step-1's runtime primitive green:
`ynz_array_new_sized(elem_size, cap)` (ynz-runtime lib.rs, after `ynz_array_new`;
len==cap, counted allocs E8, same header/drop D2/D6) + unit test
`array_new_sized_len_cap_set_drop` (1 passed/0 failed) + runtime_decls.rs declaration.
Receipts: `cargo build -p ynz-runtime -p ynz-codegen` clean; clippy `-D warnings`
clean (both crates); `cargo fmt --all -- --check` clean.

DEVIATION SURFACED (not self-decided, for deviation-judge → FRAGO seam): plan step 4
says `.copy()` deep-copies all segments; reality — array `.copy()` lowers as an ALIAS
no-op for ALL arrays (emit.rs `lower_postfix_op`, `_ => Ok(recv_val)` catch-all;
typeck check.rs:5816-5826 permits `.copy()` on any type). A deep-copying SoA copy
beside an aliasing AoS copy would be dual-mode-divergent; proposed (unapplied)
resolution: deep-copy in BOTH modes. Steps 1–3 unaffected; ruling needed before the
step-4 E9 `.copy()` cell.

Also surfaced: TodoWrite/task-store tooling not granted in this executor environment —
plan-state carried via this STATUS block + handoff instead.

Checkpoint discipline: green-building tree (build/clippy/fmt/test receipts above),
step boundary (`phase-5/step-1`), handoff written as current-truth relay. Next
segment: implement handoff design items 1–3 (SoaArrayInfo + Cg threading +
construction lowering), then step 2 access lowering to the planner CHECKPOINT mark.

## Session log — 2026-07-04 — session-id: phase5-executor-2026-07-03-m5-seg2

Phase 5 segment 2 (fresh executor, resumed from `handoff-phase-5.md` at
`phase-5/step-1`). Returned **PARTIAL** at canonical pointer `phase-5/step-1`,
sub-marker `phase-5/step-1-seg2-frago014-and-belt-gate-landed`.

**Landed:**
- FRAGO 014's ratified plan-text correction applied to plan.md Phase 5 step 4
  (array `.copy()` → genuine one-level deep copy in BOTH layout modes, mirroring
  the Type::Shape arm's shallow memcpy semantics; closes the pre-existing
  alias-no-op instead of mirroring it into SoA). Applied by this executor under a
  classified conductor instruction — the conductor never self-edits the plan body.
- Step 3 belt assert landed green: `crates/ynz-codegen/src/emit.rs` `build_module`
  errors out if any `LayoutKind::Soa` decision exists under the threaded
  `no_auto_parallel` predicate (belt on the ONE Phase 4 entry gate — E3, no second
  derivation). Receipts: `cargo build -p ynz-codegen` clean; clippy `-D warnings`
  clean; `cargo fmt --all -- --check` clean; `cargo test -p ynz-driver --test
  cross_impl_consistency` → 2 passed / 0 failed (assert exercised over the whole
  corpus under `YNZ_NO_AUTO_PARALLEL=1`; never fired).
- `handoff-phase-5.md` REPLACED in place (current-truth relay): all consumer-site
  receipts re-verified against the live tree (FRAGO 007), implementation design
  hardened to code-shape level (choke-param `Option<&SoaArrayInfo>` threading for
  compiler-forced E6 totality; loop-var-only masking — soa.rs's nested-shadow
  decline verified, so Let-time masking is provably unnecessary; `.copy()` lowering
  plan under FRAGO 014; scatter OOB abort-parity via the runtime's own path).

**Surfaced (not self-decided):** NEW step-4 hazard — `.copy()` in background-arg
position (`m5_p3_sweep_bg_array_shape_give_wait.ynz:42`) will DOUBLE-copy and leak
the intermediate array once FRAGO 014's deep-copy lands, because the spawn path
already deep-clones array args (`prepare_bg_arg_for_ctx`); E8 alloc-parity +
golden exposure. Recorded in the handoff for the step-4 executor to verify.
Deliberately NOT landed hot this segment for exactly that reason.

**Honest accounting:** this segment spent most of its window re-grounding at
implementation depth (code shapes at every consumer site, beyond the inherited
line-number receipts) and landed only the two small green increments above — the
resume pointer stayed within step 1. The re-grounding is now PREPAID in the
handoff at code-shape granularity; segment 3 should open emit.rs and write, not
re-recon.

Checkpoint discipline: green-building tree (receipts above), step boundary,
handoff replaced in place. Next segment: handoff design items 1–6 (SoaArrayInfo +
shape_field_abi/Cg threading + construction + gather/scatter + consumer threading
+ masking), then the planner CHECKPOINT mark (IR contiguity evidence).

## Session log — 2026-07-04 — session-id: phase5-executor-2026-07-03-m5-seg3

Phase 5 segment 3 (fresh executor, resumed from handoff-phase-5.md at
`phase-5/step-1-seg2-frago014-and-belt-gate-landed`). Inherited seg-1/seg-2
verification receipts per the handoff (re-verified only the live regions edited,
per FRAGO 007). Landed steps 1, 2, and 3's remaining half:

- **Step 1 — construction lowering**: `Stmt::Let` SoA interception keyed by
  (array_name, decl_span) against `cg.layout.arrays` (the ONE authority, E3);
  `lower_soa_construction` allocates ONE `ynz_array_new_sized(elem_size, cap)`
  buffer and scatters element fields at compile-time segment offsets (D2);
  same header + element-blind drop (D6).
- **Step 2 — access lowering**: new `SoaArrayInfo`/`SoaSegment` (emit.rs);
  `shape_field_abi` per-field (size, align) computed in build_module's ONE
  TargetData pass and threaded into all three Cg constructions; `soa:
  Option<&SoaArrayInfo>` param grown on all four choke helpers (compiler-forced
  E6 totality — the build enumerated the one site the inventory missed:
  debug-print, unreachable for admitted arrays, passes None with doc comment);
  gather/scatter helpers inside the E7 choke section with exact runtime OOB
  parity (gather: memset-zero + flag 0; scatter: raw ynz_array_set abort path);
  loop-var-only masking in both array for-in lowerings (normal + SM); SM
  shape-embed branch gathers directly into the frame region (same out-pointer
  contract).
- **Planner CHECKPOINT satisfied**: `opt-18 -O2` IR of `m5_p4_soa_qualifying.ynz`
  — SROA eliminated the gather out-buffer; hot-loop surviving loads are exactly
  the two used-field segment loads, contiguous stride-8. Evidence persisted to
  `p5-ir-evidence.md` (committed sibling) for Phase 6/8.
- **Step 3 remaining half closed**: with SoA live, cross_impl_consistency 2/2
  over the whole corpus; qualifying fixture byte-identical across modes (cmp
  clean). SoA FIRED (438 soa-named IR instructions — not a silent decline).
- **Gates**: build/clippy `-D warnings`/fmt clean; E7 grep clean (all raw
  rt.ynz_array_{new,push,get,set} refs in-section); E3 grep clean (zero
  candidate-query/hot_fields/whole_value_uses reads in ynz-codegen).
- Full `cargo test -p ynz-driver` regression run launched; result recorded in
  the handoff's build status (see handoff-phase-5.md).

Returned STATUS: PARTIAL at resume-at pointer
`phase-5/step-2-construction-access-green-checkpoint-evidence-persisted`
(steps 4 + 5 remain; handoff replaced in place with current truth, including the
step-4 bg-arg double-copy hazard carried forward). Deviation state: none new
this segment; FRAGO 014 already ratified; TodoWrite/task-store tooling still not
granted to this executor (plan.md/audit.md carry the state — surfaced again).

## Session log — 2026-07-04 — session-id: phase5-executor-2026-07-03-m5-seg4

Phase 5 segment 4 (fresh executor, resumed from `handoff-phase-5.md` at
`phase-5/step-4`). Returned **PARTIAL** at canonical pointer `phase-5/step-4`,
sub-marker `phase-5/step-4-copy-fixed-bgarg-hazard-resolved`.

**Landed (steps 4a + 4b):**
- FRAGO 014 `.copy()` fix — `lower_postfix_op` Copy arm gained a
  `Type::BuiltinArray` arm: AoS → `ynz_array_clone_primitive` (elem_size-aware
  byte deep copy, one-level per D12/D13); SoA → new choke-section helper
  `soa_copy_to_aos` (runtime gather loop into a fresh AoS buffer — the copy's
  binding is authority-declined so its reads lower AoS; reuses soa_gather_into +
  array_elem_set(soa=None) + in-section ynz_array_new_sized; E3/E7 clean).
- Bg-arg double-copy hazard RESOLVED at the choke point:
  `prepare_bg_arg_for_ctx` BuiltinArray arm transfers ownership of an explicit
  spawn-site `.copy()` arg (BgArgFreeKind::HeapArrayPrimitive → task drop
  ladder) instead of re-cloning. Verdict: it WAS a genuine leak, not harmless
  waste. Paper-trace on m5_p3_sweep_bg_array_shape_give_wait.ynz — baseline
  alloc=9 free=5 (gap 4 = 2 never-drop arrays × 2, FRAGO 009 accounting);
  4a-only alloc=11 free=5 (gap 6 — leaked intermediate, E8 clone→drop
  imbalance); 4a+4b alloc=9 free=5 (gap 4 restored exactly); stdout
  `caller: 119 / given: 30 / copied: 30` correct throughout. No deeper
  spawn-ownership problem found — no CCIR.

**Receipts:** build (ynz-codegen, ynz-driver) clean; clippy `-D warnings`
clean; `cargo fmt --all -- --check` clean; targeted tests green
(m5_p3_sweep_bg_array_shape_copy + _give_wait 2/2 dual-mode; m5_p3_e8_* 3/3
with the exact-count (11,0) parity pin UNCHANGED; v03_m3b_p2_explicit_copy_honored);
E7 gate clean (rt.ynz_array_{new,push,get,set} at 2544/2692/2752/2782/2872,
all in-section; new ynz_array_new_sized call in-section at 2587); E3 gate 0 hits.

**Deviations surfaced (not self-decided):** (1) `fixed<T>` `.copy()` remains a
pre-existing alias no-op — outside FRAGO 014's array scope, surfaced for a
future ruling. (2) TodoWrite/task-store tooling still not granted (segs 1-4).

Checkpoint discipline: green-building tree (receipts above), step-internal
boundary at the settled 4a/4b–4c seam, handoff REPLACED in place with current
truth incl. step-4c fixture designs verified against soa.rs's decline logic and
one flagged open verification item (SM-path Let interception — tripwired by the
planned IR soa-instruction assert). Steps 4c (E9 matrix fixtures) + 5
(full-suite cross-impl run) remain.

## Session log — 2026-07-04 — session-id: phase5-executor-2026-07-03-m5-seg5

Phase 5 segment 5 (fresh executor, resumed from `handoff-phase-5.md` at
`phase-5/step-4`, sub-marker `…-copy-fixed-bgarg-hazard-resolved`). Returned
**COMPLETE** — steps 4c + 5 landed; phase closed; handoff deleted.

**SM-path open item — definitive answer (no gap):** every `lower_sm_block` route
lowers a non-suspending `Stmt::Let` via `lower_stmt` (emit.rs :5621/:5673/:5732/
:5783/:5828/:5873), whose Let arm carries the ONE SoA construction interception
(:12408–:12442); the SM-specific Let arms (:6387/:6416/:6450/:6476) match only
wait/suspending-call initializers. Empirical: wait-only qualifying variant emits
1156 soa-named IR lines; the shipped fixture's IR assert passes.

**Step 4c landed:** fixtures `m5_p5_copy_aos_independent.ynz` (FRAGO 014
independence lock, shape+int elems, dual-mode) and `m5_p5_soa_copy_wait_bg.ynz`
(SoA × wait × .copy() × IndexAssign scatter × background coexistence, dual-mode +
IR positive/negative asserts); tests `m5_p5_copy_aos_independent`,
`m5_p5_soa_copy_wait_bg_matrix`, `m5_p5_bg_copy_alloc_gap_pin` (gap==4 lock on the
seg-4 4b fix); both-candidate test extended with the end-to-end ZERO-soa-IR
negative half (D11 through codegen).

**Live discovery (paper-traced, working-as-designed):** the handoff's original
matrix design (`background tally(pts.copy())` on the SoA shape) is D11-excluded —
candidates stay Admitted{72,[x,y]} but bg-crossing pads the shape (M4 padding is
shape-level, even for a spawn-site copy) → authority resolves
`Aos { declined: CrossThreadPadded }`. First fixture draft tripped exactly the
planned IR tripwire; diagnosis via layout_decisions_query dump (temp test, deleted);
fixture redesigned with the bg cell on a second shape (Part), constraint documented
in the fixture header. Class already pinned at analysis level (both-candidate suite).

**Step 5 receipts:** full `cargo test -p ynz-driver` exit 0 (integration 522/0,
m2 SM 31/0, cross_impl corpus sweeps green = whole-corpus dual-mode byte-identity);
`cargo test --workspace --no-fail-fast` **2271/0**; clippy `-D warnings` clean;
fmt `--check` clean. E3 gate 0 hits; E7 gate all gated refs in-section
(2544–2872 ∈ 2235–3403; `new_sized` exempt, 2 sites :2587/:12381). Snapshot churn:
13 pre-existing IR snapshots refreshed AFTER investigation — every delta is exactly
the added `declare ptr @ynz_array_new_sized(i64, i64)` line (seg-1 primitive,
declaration-only, zero instruction changes, SHA goldens unaffected); recorded
decision, not blind acceptance. Zero `*.snap.new` remaining.

**Unfixed should-fix/minor findings enumerated for the boundary review (NOT
self-filed as deferrals):** (1) `fixed<T>.copy()` remains a pre-existing alias
no-op (catch-all arm — re-confirmed live at emit.rs `lower_postfix_op`; outside
FRAGO 014's array scope). (2) Shape-level padding conservatism: a spawn-site
`.copy()` bg-arg pads the ORIGINAL's shape and forfeits SoA for all its arrays —
conservative-correct under D11; binding-level padding is a potential future
refinement. (3) TodoWrite/task-store tooling not granted to any Phase 5 segment
(1–5) — plan.md/audit.md carried state.

## Session log — 2026-07-04 — session-id: phase5-executor-2026-07-03-m5-closing
POST-BOUNDARY-REVIEW CLOSING ROUND — not a new phase. Phase 5's reviewer fan-out (7 dispatches)
returned ZERO blockers; this round landed the one should-fix + filed two durable deferrals +
recorded FR #12 (mirrors the Phase 4 closing/deferral precedent).
(1) Should-fix landed (code-reviewer, correctness): `soa_gather_into`'s HIT path stored each
field via struct_gep but never zeroed the out buffer, leaving inter-field/tail struct padding
nondeterministic — AoS `ynz_array_get` (runtime lib.rs:1237-1247) memcpys the FULL elem_size
bytes incl. padding, so a raw-byte consumer (memcmp, byte-hash key) could observe SoA-vs-AoS
padding divergence despite identical field values, contradicting the phase's "exact AoS byte
parity" claim. Fix: `build_memset(out, 1, 0, elem_size)` at the top of the hit path
(emit.rs:2459-2466, the OOB path's existing memset convention, verified live before editing —
the review's cited :2454 had held); padding bytes now deterministically zero (fresh-buffer
image); doc comment extended with the WHY. Receipts (Docker): `cargo build -p ynz-codegen`
clean; clippy `-D warnings` clean; `cargo fmt --all -- --check` clean; SoA subset green —
`m5_p5_copy_aos_independent` + `m5_p5_soa_copy_wait_bg_matrix` + `m5_p5_bg_copy_alloc_gap_pin`
3/3 and `m5_p4_soa_both_candidate_padding_wins_byte_layout` 1/1, all exact-stdout pins
UNCHANGED (= byte-identical stdout to pre-fix, padding-only change); `cross_impl_consistency`
2/2 (whole-corpus dual-mode byte-identity sweep); full workspace suite green at the 2271-test
baseline with zero new tests; zero `*.snap.new`. E3 gate: 0
`soa_candidate|hot_fields|whole_value_uses` hits in ynz-codegen. E7 gate: all
`rt.ynz_array_{new,push,get,set}` refs at 2556/2704/2764/2794/2884 — in-section (2235–3415;
all refs shifted +12 by the insert); `ynz_array_new_sized` unchanged at its 2 sites
(:2599 in-section, :12393 ratified interception).
(2) Deferrals filed in the ROADMAP's sidecar
(`.claude/planning/active/2026-05-21-v0-3-concurrency-perf/audit.md`), both keys
grep-confirmed absent (whole-line fixed-string) before writing, both line anchors re-verified
against the POST-fix live file per FRAGO 007 (the dispatch's :17731/:2441 had shifted +12):
- `Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#5: crates-ynz-codegen-src-emit-rs-17743` —
  `fixed<T>.copy()` remains the M4-era alias no-op (`lower_postfix_op` Copy-arm catch-all
  `_ => Ok(recv_val)`; FRAGO 014's fix was explicitly array<T>-scoped).
- `Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#5: crates-ynz-codegen-src-emit-rs-2445` —
  SoA gather/scatter bounds predicate (`idx u< cap`, gather :2445 / scatter :2525) is a
  documented-equivalent second derivation of the runtime's `idx < 0 || idx >= len` check
  (lib.rs:1239); inert under D3's len==cap invariant, trigger = any len/cap relationship
  change (growable SoA).
(3) FR #12 added to plan.md Future Requirements: shape-level padding granularity means one
spawn-site `.copy()` bg-arg forfeits SoA for ALL of that shape's arrays under D11 —
working-as-designed (segment-5 live discovery, boundary-review-confirmed), revisit on a real
workload where the conservatism measurably costs (Phase 6 harness may surface it).
Plan sync: Phase 5 STATUS block extended with the closing-round paragraph; session-id
`phase5-executor-2026-07-03-m5-closing` appended to THIS plan's frontmatter chain AND the
roadmap's roadmap.md chain (per the phase4-deferral precedent), all in this same action.
Deviations: NONE. No handoff file (single segment). Context-segment log untouched
(conductor-owned). No commit (conductor's call).

## Session log — 2026-07-04 — session-id: phase6-executor-2026-07-03-m5

Phase 6 segment 1 — PARTIAL at `phase-6/step-1` (context-budget checkpoint at the pre-step-1
seam; zero source edits, tree clean at `4c5e902` — trivially green-building). Paid the phase's
full orientation and settled the step-1 design; receipts + settled design + resume pointer in
`handoff-phase-6.md` (created this segment). Highlights:

- **Receipts:** SOA_SIZE_THRESHOLD=64 anchor verified (soa.rs:31, sole consumer :336);
  YNZ_SOA_FORCE confirmed absent (wiring is step-1 work; D8 semantics + precedence settled);
  shipped binaries confirmed OptimizationLevel::None with zero LLVM pass pipeline
  (state_machine.rs:745-760, emit.rs:952); loop-semantics probes — for-in element is a copy
  (no field write-back), `pts[i].x` is maybe<Point> compile error ⇒ workload = S2-qualifying
  read-accumulate x/y scan (recorded decision, provenance will note it); N=4096 compile ≈2.1s.
- **Deviation surfaced (1, runtime bug, pre-existing, NOT SoA):** hot loops SIGSEGV at ~0.5s in
  BOTH default(SoA) and YNZ_NO_AUTO_PARALLEL=1(AoS) modes once total loop iterations reach
  ~4.19M (n4096×r1024 / n1024×r4096 / n512×r8192 all crash); 512K visits complete with EXACT
  expected checksums (n512×r1000 → 393984000 = R·3·N(N+1)/2). Theory: per-iteration alloca
  stack growth at O0. Evidence: untracked `tmp-p6-probe/` (never commit; delete at phase close).
  Harness proceeds under a 262144-visit/process cap; bug needs conductor routing (threatens
  every large-hot-loop surface incl. Phase 8's demo).
- **Deviation surfaced (2, step-5 pre-registration):** p5-ir-evidence Claim 2's hot-field-only
  SoA loads exist only under `opt-18 -O2`, which `ynz build` never runs — the shipped-binary
  SoA win may be ~0 and Phase 6 step 5's order-of-magnitude STOP is likely. Theory to verify by
  the harness, recorded so it cannot pass quietly.
- Per the executor charter, the `## Context-segment log` is conductor-owned — entry content for
  key `2026-07-03-v0-3-m5-auto-soa#6-segment-1` supplied in this segment's return instead.

## FRAGO 015 — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable (deviation-judge classified JUSTIFIED; risk-raising vs a new unscored risk; blanket-signed per FRAGO 004)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 6, segment 1 (mid-phase — dispatched before segment
           2, not a post-diff boundary review)
Trigger:   Phase 6 step 2 ("≥10 repetitions per point, N ∈ {8...4096}") silently assumed the
           compiler can run arbitrarily many loop iterations without crashing. Segment 1's
           executor found this false while designing the harness: hot loops SIGSEGV in BOTH
           layout modes (SoA and AoS identically — not a dual-mode divergence, a shared ceiling)
           once total loop-visit count reaches ~4.19M, constant regardless of N; 512K visits
           complete with exact expected checksums. Theory: per-iteration alloca stack growth at
           optimization level None (shipped binaries run zero LLVM pass pipeline —
           state_machine.rs:745-760, emit.rs:952). Pre-existing, general codegen behavior, NOT
           caused by this milestone's array-by-value or SoA work — out of M5's chartered scope to
           fix. Deviation-judge verdict (dispatched by the conductor): JUSTIFIED — the executor
           correctly declined to self-fix an out-of-charter general codegen bug, applied a
           conservative 262144-visit/process cap (2x margin under the proven-good 512K point, well
           under the ~4.19M crash point), and surfaced rather than self-classified.
Risk:      RISK-NEUTRAL against E2 specifically (the cap operates inside E2's honesty-posture
           mitigation, not outside it — E2 is about variance-recording and unearned precision, not
           total iteration count; the per-point sanity checksum gate is a live tripwire). But the
           underlying bug is a genuinely NEW risk the frozen matrix does not cover: it threatens
           Phase 8's demo (an explicitly-authored large-array hot loop with a byte-exact-golden
           acceptance gate) with an undiscovered-until-execution SIGSEGV. Scored fresh per
           REF-risk-engine: Prob C (moderate — a known real bug, but only manifests on very large
           hot loops, which are rare in the existing suite though Phase 8 explicitly constructs
           one), Sev III (a build/demo-blocking crash, reversible by bounding iteration count, not
           a security/data-loss/money issue) → Initial MEDIUM (C×III). Mitigation: B2 engineered
           gate — an explicit Phase 8 step-1 requirement to verify the demo's total loop-visit
           count against the proven-safe bound before it ships, plus this FRAGO's own Future
           Requirements entry — prob −1 step (C→D) → Residual LOW (D×III). No HIGH residual; no
           live signature required. Patrick's FRAGO 004 blanket pre-sign cited for the
           risk-raising classification per its recorded scope (a new row entering the matrix is
           risk-raising by definition even when its residual lands LOW).
Changes (plan.md/roadmap.md body edits — applied by the Phase 6 segment-2 executor, not the
           conductor directly; plan-body edits stay out of conductor charter):
  - ¶1 Risk Assessment: new row **E13 — hot-loop O0 stack-exhaustion ceiling (pre-existing, all
    layouts)** — *Phases 6, 8* | Prob C | Sev III | Initial MEDIUM | Mitigations: (1) Phase 6
    harness runs under a 262144-visit/process cap with a per-point checksum tripwire (B2
    engineered guard, prob −1; proof: harness code + committed variance record). (2) Phase 8 step
    1 MUST verify the demo's total loop-visit count against the proven-safe bound (≤262144, or the
    bug is fixed first) before the byte-exact-golden acceptance gate runs. | Residual LOW (D×III)
    | recorded.
  - ¶3.3 Phase 8 step 1: ADD an explicit requirement — before authoring the large-array hot-loop
    demo section, verify its total loop-visit count (N × iterations-per-element-touch) stays at or
    under the proven-safe 262144-visit bound established by Phase 6's harness (E13), or confirm the
    underlying stack-growth bug is fixed first. This is a hard precondition on the demo's byte-exact
    golden acceptance criterion, not a suggestion.
  - Future Requirements: new entry #13 — *what:* general O0 per-iteration `alloca` stack-growth
    SIGSEGV ceiling (~4.19M total loop iterations, both layout modes, `state_machine.rs`/`emit.rs`);
    *why deferred:* general codegen issue unrelated to array-element storage, out of M5's charter;
    *cost:* an investigation into loop-body frame lifetime at optimization level None (possibly
    gated on whether shipped builds ever run an LLVM pass pipeline at all); *trigger:* the next
    milestone touching loop/state-machine codegen, OR immediately if Phase 8's demo would exceed
    the safe bound and the bug isn't otherwise avoidable, OR a real user hot loop crashes in
    production. Not Patrick-signed individually — covered by FRAGO 004's blanket.
  - roadmap.md Capability Ledger: add an "Unscoped Capability" row mirroring the existing
    Authoritative-Derivation-Guard entry — WHAT: general hot-loop O0 stack-exhaustion ceiling; WHY:
    out of every currently-scheduled milestone's charter; COST: its own investigation session; owning
    milestone: none yet (pending Patrick's scheduling call); pointer to this FRAGO for the full
    4-field deferral text.
Unchanged: Phase 6 steps 1, 3, 4, 5 unaffected — this FRAGO adds a risk row + a Phase 8 precondition
           + a Future Requirements entry + a roadmap ledger pointer; it does not change Phase 6's
           own calibration methodology beyond the already-adopted 262144-visit cap.
Override:  Patrick, blanket pre-sign, FRAGO 004, 2026-07-03 — cited per its recorded scope (no live
           signature sought; no HIGH residual in any case).

## Session log — 2026-07-04 — session-id: phase6-executor-2026-07-03-m5-seg2

Phase 6 segment 2. First action: applied FRAGO 015's already-classified plan/roadmap body edits
(executor applies, conductor never self-edits plan body):

- `plan.md` ¶1 Risk Assessment: new row **E13 — hot-loop O0 stack-exhaustion ceiling
  (pre-existing, all layouts)** — Phases 6, 8 | C | III | MEDIUM | Phase 6 262144-visit cap +
  checksum tripwire; Phase 8 step-1 precondition | Residual LOW (D×III) | recorded.
- `plan.md` Phase 8 step 1: explicit E13 precondition added — demo total loop-visit count must be
  verified ≤262144 (or the bug fixed first) before the byte-exact-golden gate runs.
- `plan.md` Future Requirements: entry #13 (WHAT/WHY-DEFERRED/COST/TRIGGER per FRAGO 015's text).
- `roadmap.md` (2026-05-21-v0-3-concurrency-perf): "General hot-loop O0 stack-exhaustion ceiling
  fix" unscoped-capability row added to BOTH Capability Ledger tables, mirroring the
  Authoritative-Derivation-Guard entry (which lives in both), pointing at FRAGO 015 for the
  4-field text.
- Session-id `phase6-executor-2026-07-03-m5-seg2` appended to plan.md frontmatter in the same
  action.

Segment 2 execution record continues below (appended at segment close).

Segment close (same session, `phase6-executor-2026-07-03-m5-seg2`) — PARTIAL at `phase-6/step-2`
(context-budget checkpoint at the step-1/step-2 seam; tree green-building: fmt applied,
soa_analysis 21/21, bench compiles; source edits uncommitted pending commit #1 per the FRAGO 004
commit protocol):

- **Step 1 DONE:** YNZ_SOA_FORCE (D8) — `SoaForce` in soa.rs, `soa_force_env()` in queries.rs
  (read only at `soa_candidate_query` entry), threaded as explicit param into `soa::analyze`;
  "soa" skips ONLY the BelowSizeThreshold arm, "aos" empties the set, kernel/no-auto-parallel
  outrank (precedence pinned pure-core + query-level); 6 new tests, env_guard clears the var.
  Harness `crates/ynz-driver/benches/soa_calibration.rs` (criterion workspace dev-dep,
  feature-trimmed) with checksum / dual-mode-stdout / IR admission gates.
- **Step 2 partial:** day-of S3 noise re-record done (3 process runs; ~15% floor confirmed;
  durable in `crates/ynz-driver/benches/soa-threshold-raw-2026-07-04.md`); spawn-overhead
  baseline measured (~1.73 ms median); full calibration run pending next segment.
- **Deviation surfaced (NEW, conductor routes — not self-classified):** the harness's own E13
  checksum tripwire caught SIGABRT at N=8/R=32768 — 262144 visits, exactly AT FRAGO 015's
  recorded cap — while 512K visits at N=512/R=1000 passes. Bracket (N=8): 16384 reps OK with the
  exact checksum, 32768 crash ⇒ the crash envelope is 2-axis (total visits AND for-in loop
  entries); FRAGO 015's visits-only cap phrasing is insufficient at small N and its
  E13/Phase-8/FR-13 text may need a follow-up amendment. Evidence: raw file above +
  `target/p6-soa-calibration/`. Harness self-consistently tightened to TOTAL_VISITS=131072 (both
  axes at proven-good points) — a harness-internal parameter within the already-adopted cap
  discipline, not a plan-text edit.
- Handoff rewritten in place; resume pointer `phase-6/step-2`.

## FRAGO 016 — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable (deviation-judge classified JUSTIFIED; risk-raising vs E13's recorded residual; blanket-signed per FRAGO 004)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 6, segment 2 (mid-phase — dispatched before segment
           3, not a post-diff boundary review)
Trigger:   FRAGO 015's E13 row / Phase 8 step-1 precondition / FR #13 all asserted a single
           total-loop-visit-count scalar (≤262144) as the safe/unsafe boundary for the pre-existing
           O0 stack-growth SIGSEGV. Segment 2's harness tripwire directly falsified this at its OWN
           cited value: N=8/R=32768 (262144 total visits — exactly the cap) SIGABRTs, while
           N=512/R=1000 (512,000 total visits — nearly double) passes clean with exact checksums.
           A scalar total-visits bound is therefore neither necessary nor sufficient — the crash
           envelope is a joint function of (N, R), not reducible to one number. Deviation-judge
           verdict: JUSTIFIED (direct, already-collected evidence; the executor correctly did not
           self-amend plan text, only tightened its own harness-internal TOTAL_VISITS to 131072 at
           a point proven safe on both axes, and surfaced the finding).
Risk:      RISK-RAISING relative to E13's recorded LOW residual (D×III) — that residual rested on
           mitigation (2), Phase 8's total-visits-only gate, being a reliable B2 engineered guard
           (prob −1 step). The gate is now shown unreliable as literally worded: a demo shape could
           satisfy "total visits ≤ 262144" and still crash (the false-pass direction — the
           dangerous one, since it would let a broken demo through the byte-exact-golden gate
           undetected), or could be needlessly rejected despite being safe (the false-reject
           direction). This is a material downgrade to an EXISTING mitigation's reliability, not a
           brand-new unscored risk, but per FRAGO 015's own precedent ("a new row entering the
           matrix is risk-raising by definition even when residual lands LOW") a downgrade to a
           mitigation the residual score depends on is risk-raising and goes through the same
           signature path rather than being silently absorbed. No HIGH residual results — the fix
           is a text-accuracy correction, not a new gap requiring new engineering — so no live
           signature is required; Patrick's FRAGO 004 blanket pre-sign is cited per its recorded
           scope.
Changes (plan.md text corrections — applied by the Phase 6 segment-3 executor, not the conductor
           directly; plan-body edits stay out of conductor charter):
  - ¶1 E13 row, mitigation (2) (plan.md:151): replace the total-visits-scalar phrasing with —
    "Phase 8 step 1 MUST verify the demo's specific (array-size N, outer-repetition-count R) shape
    against the harness's jointly-proven-safe bracket data
    (crates/ynz-driver/benches/soa-threshold-raw-2026-07-04.md), NOT a single total-visits scalar —
    segment-2 evidence proved total-visits-alone is neither necessary nor sufficient (262,144 total
    visits crashed at N=8/R=32,768; 512,000 total visits passed cleanly at N=512/R=1,000). If the
    demo's (N,R) shape isn't already covered by the harness's bracketed region, bracket it directly
    with the same checksum-tripwire methodology before the byte-exact-golden gate runs, or confirm
    the underlying bug is fixed first."
  - ¶3.3 Phase 8 step 1 precondition (plan.md:1244-1249): same substitution — strike the
    "≤262144-visit bound" scalar language, replace with the joint-envelope check above; keep the
    "hard precondition... not a suggestion" framing.
  - Future Requirements #13 WHAT clause (plan.md:1502-1510): replace "hot loops crash at ~4.19M
    total loop iterations, both layout modes" with "hot loops crash on a joint function of
    array-size N and outer-repetition/for-in-entry count R — NOT reducible to a total-visits scalar
    (bracketed crash points range from ~262K total visits at N=8/R=32,768 up to ~4.19M at larger N;
    512K total visits is proven safe at N=512/R=1,000)." COST/TRIGGER fields stand unchanged
    (root-causing the actual second axis remains correctly out of M5's charter).
Unchanged: Phase 6's own harness (already self-tightened to TOTAL_VISITS=131072 on both proven-good
           axes — no further Phase-6 work required by this FRAGO); E13's Prob/Sev/Initial cells;
           the rest of the plan.
Override:  Patrick, blanket pre-sign, FRAGO 004, 2026-07-03 — cited per its recorded scope (no live
           signature sought; no HIGH residual results from this correction).

## Session log — 2026-07-04 — session-id: phase6-executor-2026-07-03-m5-seg3
Phase 6 segment 3 (resumed from `phase-6/step-2` per `handoff-phase-6.md`). Step A first action:
applied FRAGO 016's three plan-text corrections exactly as classified (conductor-authored,
deviation-judge JUSTIFIED, FRAGO 004 blanket-signed) — ¶1 E13 row mitigation (2) replaced with the
joint-(N,R)-bracket-check phrasing; ¶3.3 Phase 8 step 1 E13 precondition replaced with the same
joint-envelope check (hard-precondition framing kept); Future Requirements #13 WHAT clause replaced
with the joint-function framing (COST/TRIGGER unchanged). Session-id appended to plan.md frontmatter
in this same action. Segment outcome: **BLOCKED at Phase 6 step 5's STOP condition** (order-of-
magnitude shortfall vs the 10-40x claim; CCIR item 4 full-STOP, pre-registered by segment 1, now
measurement-confirmed). Step 2 completed and committed (checkpoint commit #1 `e989f43`: harness +
YNZ_SOA_FORCE wiring + raw evidence file). Root-caused the segment-2 bench tripwire to a stale
pre-M5 `target/release/libynz_runtime.a` silently embedded by the release driver (garbage old-ABI
symbol resolution → overflow SIGABRT; forced-SoA link failure) — fixed operationally by rebuilding
the release runtime; the staleness footgun surfaced as a deviation for routing (candidate durable
fix: codegen-referenced ABI-version symbol). This falsifies FRAGO 016's specific evidence line
(healthy n8×r32768 passes) while its 2-axis conclusion survives on new evidence (n8×r65536 SIGSEGV
at 524K visits vs n512×r1000 pass at 512K; N=8 entries boundary is 32768-OK/65536-crash) —
surfaced, not self-amended. Calibration result: NO SoA crossover at any N in {8..4096} in shipped
O0 binaries (net SoA/AoS 1.00-1.18); ~3.3x SoA win under `opt-18 -O2` only. Steps 3-5 halted;
evidence in `crates/ynz-driver/benches/soa-threshold-raw-2026-07-04.md`; resume state in
`handoff-phase-6.md`. Three NEW deviations surfaced (stale-runtime footgun; FRAGO 016 evidence
falsification; the STOP itself) — conductor routes all three.

## FRAGO 017 — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable (deviation-judge classified JUSTIFIED; risk-raising, new row E14, MEDIUM residual; blanket-signed per FRAGO 004)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 6, segment 3 (mid-phase — dispatched before segment
           4, not a post-diff boundary review)
Trigger:   Phase 6 step 5's STOP condition fired for real: across all N in {8...4096}, shipped
           (default OptimizationLevel::None, zero LLVM pass pipeline) `ynz build` binaries show NO
           SoA benefit whatsoever — net-of-spawn median ratios 1.00-1.18 (SoA never faster, up to
           18% slower at some points, uniformly below the ~15% noise floor in the no-benefit
           direction). Root cause confirmed: the SAME compiled IR run through `opt-18 -O2`
           externally shows SoA winning ~3.3x at N=4096 (checksums exact — identical logic, only
           optimized). This directly falsifies ¶2 Mission's "10-40x cache-locality win" framing and
           Key Outcome #3's implicit assumption of a real measured win, for the compiler as it
           SHIPS today. Structural cause: `ynz build` never runs an LLVM optimization pass pipeline
           at all (state_machine.rs:745-760, emit.rs:952) — building one is a separate, large
           compiler capability, out of this milestone's array-storage charter.
Risk re-run (frozen matrix, REF-risk-engine.md): Prob A (Frequent — already measured, confirmed,
           reproduced at every N), Sev III (Marginal — a documentation/teaching-honesty gap, no
           money/data/prod-state/security/irreversibility dimension fires; reversible by
           rewriting text) → Initial H. Mitigation: honest-reframe of Mission/KO#3/Performance-
           invariant text, gated by docs-consistency review before any Phase 7/8 user-facing text
           ships (B2 engineered guard on the severity axis, −1) → Residual M (A×IV). No Floor B
           class fires (confirmed true for the whole plan). Per the engine's own gate rule, a
           properly-mitigated MEDIUM passes with RECORD only — no live signature required. This is
           independent of whether FRAGO 004's blanket technically reaches the call: the engine's
           normal outcome for a risk this shape, once honestly documented, is continue-and-record.
           FRAGO 004's blanket pre-sign is additionally cited per its recorded scope. The completion
           approval gate (NOT waived by FRAGO 004) remains the correct seam for Patrick's own
           strategic call on whether/how to ship this milestone's headline claim — this FRAGO does
           not attempt to make that call, only to stop the plan from silently overclaiming in the
           interim.
Changes (plan.md body edits — applied by the Phase 6 segment-4 executor, not the conductor
           directly; plan-body edits stay out of conductor charter):
  - ¶2 Mission (plan.md, after "...promises the 10-40x cache-locality win to naive
    sequential-looking code"): append — "That win is real and IR-confirmed (measured ~3.3x under
    `opt-18 -O2` at the calibration workload's shape, consistent with the theoretical bandwidth
    edge) but is NOT realized in the binaries `ynz build` ships today, because the compiler runs
    zero LLVM optimization passes (OptimizationLevel::None) — a pre-existing, out-of-charter gap
    this milestone's Phase 6 harness discovered and documents honestly rather than silently
    overclaiming. One representation must own both changes so the twin-substrate drift class can
    never ship; the correctness of that representation is unconditional — its measured performance
    payoff, in the compiler as it ships today, is conditional on a future optimization-pipeline
    milestone."
  - ¶3.1 Key Outcome #3: replace the final clause ("...and the measured hot-loop improvement is
    recorded with benchmark evidence") with — "...and the measured hot-loop improvement is recorded
    with benchmark evidence, stated honestly: in shipped O0 binaries the measured net effect is
    ~1.0x (no detectable benefit; each point 4-18% below the noise floor, direction uniform), while
    the identical generated IR shows the design doc's 10-40x class of win once run through an LLVM
    optimization pipeline (`opt-18 -O2` measured ~3.3x at N=4096). The lint hover and CHANGELOG cite
    the O0 number as what ships today and the -O2 number as the pipeline-dependent upside — never
    conflated."
  - Invariants → Performance, first bullet: append after "...the SHIPPED claim is the measured
    number, Phase 6 step 5)." — "Measured 2026-07-04: no detectable benefit in shipped O0 binaries
    (net ratios 1.00-1.18x across N in {8..4096}); the same IR shows ~3.3x under `opt-18 -O2`.
    SIZE_THRESHOLD ships as a documented conservative default, not a crossover-calibrated constant —
    no O0 crossover exists to calibrate against. Revisit trigger: a future LLVM-pass-pipeline
    milestone."
  - ¶1 Risk Assessment: new row **E14 — shipped O0 SoA benefit is optimization-pipeline-dependent**
    — *Phases 6, 7, 8* | Prob A | Sev III | Initial H | Mitigations: honest-reframe of
    Mission/KO#3/Performance-invariant text, gated by docs-consistency review before any Phase 7/8
    user-facing text ships (B2 engineered guard, severity −1; proof: corrected text + committed
    provenance file, reviewed pre-ship) | Residual MEDIUM (A×IV) | recorded.
  - Phase 6 step 3 is RE-SPECIFIED (this FRAGO): calibration is not "find the crossover N" (none
    exists in O0 binaries) — ship SIZE_THRESHOLD=64 as a documented conservative default (unchanged
    from its pre-M5 value) with honest provenance stating no O0 crossover exists to calibrate
    against, rather than deriving a constant from a crossover that isn't there.
Unchanged: Phases 0-5; Phase 6 steps 1-2 (already committed, e989f43); the correctness/dual-mode
           invariants (unaffected — SoA remains byte-identical, this FRAGO is about the PERFORMANCE
           claim only, never correctness).
Override:  Patrick, blanket pre-sign, FRAGO 004, 2026-07-03 — cited per its recorded scope. No live
           signature required per the risk engine's own MEDIUM-residual gate rule (record only).

## FRAGO 018 — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable (deviation-judge classified JUSTIFIED; risk-raising, MEDIUM; durable deferral + immediate Phase 8 operational guard; blanket-signed per FRAGO 004)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 6, segment 3
Trigger:   `ynz-driver/build.rs`'s `include_bytes!` embedding of `target/{profile}/libynz_runtime.a`
           has no ABI-version check. A stale cross-profile archive (pre-dating M5's ABI changes)
           resolves old-signature `ynz_array_*`/`ynz_map_*` symbols by NAME against new codegen,
           silently miscompiling (garbage reads) instead of failing to link. This exact footgun
           already corrupted one Phase-6 measurement (the segment-3 apparent SIGABRT, since
           re-attributed away from E13 — see FRAGO 019). Not caused by this milestone's array/SoA
           work directly, but exposed by it; E7/E12's hard-cut mitigations cover SOURCE call sites,
           not cross-profile archive staleness, so this is a genuinely uncovered gap.
Risk:      Prob B (Likely — just fired, zero ABI check exists at all), Sev III (Marginal — no
           precompiled `ynz` binaries are distributed to external users today; blast radius is a
           dev/CI workflow getting silently-wrong build/bench/test results, recoverable by rebuild)
           → Initial MEDIUM (B×III). Passes at record-only under the engine's gate — does not
           require a Phase-8-blocking gate or a live signature. However Phase 8 step 4 (E11) runs
           the release-profile compiler binary and step 6 cuts the tag from the same tree — direct,
           in-scope exposure this milestone, not a hypothetical future problem — so the response is
           BOTH a durable deferral (the structural fix, out of charter) AND a cheap immediate
           Phase-8 operational guard (in scope, low cost).
Changes (plan.md/roadmap.md edits — applied by the Phase 6 segment-4 executor):
  - Future Requirements: new entry #14 — WHAT: `ynz-driver/build.rs` embeds
    `target/{profile}/libynz_runtime.a` via `include_bytes!` with no ABI/version check; a stale
    archive resolves old-signature `ynz_array_*`/`ynz_map_*` symbols by name against new codegen,
    silently miscompiling instead of failing to link. WHY deferred: the real fix (a codegen-emitted
    ABI-version symbol + a linker/embed-time staleness check) is a cross-cutting build/release-
    tooling capability, not an array/SoA-representation concern — building it now scope-creeps
    Phase 8 into compiler build-system work outside this milestone's charter. COST: ~0.5-1 session
    (one versioned symbol emitted by ynz-runtime, checked at embed/link time in build.rs; isolated,
    low-risk). TRIGGER: the next milestone touching ynz-runtime's ABI, OR the point Yinz begins
    distributing precompiled `ynz` binaries externally (severity escalates then — revisit the
    score).
  - roadmap.md Capability Ledger (both tables, per the plan's established both-tables convention):
    add row "Build/release-tooling: ABI-version-checked runtime archive embedding — owning milestone
    TBD (discovered v0.3-M5 Phase 6, FRAGO 018)."
  - Phase 8 step 4 AND step 6 (plan.md): prepend an operational guard before each — "Rebuild
    ynz-runtime's release archive from clean (`cargo clean -p ynz-runtime && cargo build -p
    ynz-runtime --release`) before this step — Phase 6 segment 3 found a stale release archive
    silently miscompiles by resolving old-ABI symbols by name (Future Requirements #14 for the
    durable fix)."
Unchanged: E7/E12's existing mitigations (source-call-site scope, correctly untouched); everything
           else in Phase 8.
Override:  Patrick, blanket pre-sign, FRAGO 004, 2026-07-03 — cited per its recorded scope. No live
           signature required (MEDIUM residual, record-only per the engine).

## FRAGO 019 — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable (deviation-judge classified JUSTIFIED; risk-neutral evidence-accuracy correction; auto-apply + log)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 6, segment 3
Trigger:   FRAGO 016's cited crash point ("N=8/R=32,768 crashes, 262,144 total visits") was itself
           contaminated by FRAGO 018's stale-release-runtime bug, not a real E13 SIGSEGV — on a
           healthy toolchain N=8/R=32,768 (262,144 visits) PASSES cleanly with the exact checksum.
           The underlying 2-axis conclusion (a scalar total-visits bound is neither necessary nor
           sufficient) SURVIVES on new, clean evidence: N=8/R=65,536 (524,288 visits) SIGSEGVs while
           N=512/R=1,000 (512,000 visits, nearly the same total) passes. The real N=8 boundary sits
           between R=32,768 (safe) and R=65,536 (crash). Risk-neutral: pure evidence-accuracy
           correction, same qualitative conclusion, same recommended mitigation (bracket the demo's
           specific (N,R) shape, never trust a total-visits scalar); E13's existing score (Initial
           MEDIUM C×III, Residual LOW D×III, recorded) is UNCHANGED.
Changes (plan.md — applied by the Phase 6 segment-4 executor, swept to BOTH citation sites per
           plan-source-of-truth's sweep discipline):
  - ¶1 Risk Assessment, E13 row, mitigation (2): replace "262,144 total visits crashed at
    N=8/R=32,768; 512,000 total visits passed cleanly at N=512/R=1,000" with — "524,288 total
    visits crashed at N=8/R=65,536; 512,000 total visits passed cleanly at N=512/R=1,000. N=8/R=32,768
    (262,144 visits) also passes cleanly on a healthy toolchain — the earlier reading of a crash at
    that point was a since-fixed stale-release-runtime-archive bug (FRAGO 018), not this
    stack-growth class."
  - ¶3.3 Phase 8 step 1's E13 precondition: identical substitution (same sentence, second site).
Unchanged: E13's Prob/Sev/Initial/Residual cells; the recommended Phase-8 mitigation approach
           (bracket the demo's (N,R) shape directly); everything else.
Override:  N/A — risk-neutral.

## Session log — 2026-07-04 — session-id: phase6-executor-2026-07-03-m5-seg4
Phase 6 segment 4 (resumed from `phase-6/step-3` per `handoff-phase-6.md`, after the conductor
routed the step-5 STOP via FRAGOs 017-019). Steps A-C first actions — applied all three
conductor-classified FRAGOs exactly as recorded, in order: **FRAGO 017** (honest performance
reframe): ¶2 Mission appended sentence (the FRAGO text's closing clause subsumes the original
"one representation must own both changes" trailing clause — folded, not duplicated); ¶3.1 Key
Outcome #3 final clause replaced with the honest O0-~1.0x / -O2-~3.3x statement; Invariants →
Performance first bullet appended; ¶1 Risk Assessment new row E14 (A×III Initial HIGH → Residual
MEDIUM A×IV, recorded); Phase 6 step 3 re-specified (ship SIZE_THRESHOLD=64 as documented
conservative default — no O0 crossover exists to calibrate against). **FRAGO 018** (stale-runtime
footgun): Future Requirements #14 added (4-field deferral); roadmap Capability Ledger row added to
BOTH tables (both-tables convention, matching the FRAGO 015 row pattern); Phase 8 steps 4 and 6
each prepended the rebuild-ynz-runtime-clean operational guard. **FRAGO 019** (evidence-accuracy
correction): the falsified crash-point evidence swapped at both named sites (¶1 E13 mitigation (2);
¶3.3 Phase 8 step 1 E13 precondition) — plus, per the sweep discipline FRAGO 019 itself invokes
(plan-source-of-truth: "closing the FRAGO doesn't stop at the trigger's own citation"), the SAME
falsified fact found and corrected at a third sibling site the FRAGO's Changes did not enumerate:
Future Requirements #13's bracketed-crash-points parenthetical (identical risk-neutral
substitution, no new judgment; recorded here explicitly). Historical segment-narrative paragraphs
(Phase 6 segments 1-2 STATUS notes) deliberately left as-is — they are the audit-style record of
what each segment reported at the time, and segment 3's paragraph already records the
falsification in place. Session-id appended to plan.md frontmatter in this same action. Step D:
Phase 6 steps 3-5 completed per the FRAGO 017 re-specification — provenance verdict section added
to `crates/ynz-driver/benches/soa-threshold-raw-2026-07-04.md` (SIZE_THRESHOLD=64 ships as
conservative unchanged default; O0 ~1.0x and -O2 ~3.3x recorded, never conflated); soa.rs
SOA_SIZE_THRESHOLD comment rewritten citing the provenance file; step 5's improvement number
recorded via FRAGO 017's text + the provenance file (STOP already resolved by conductor routing).
Phase 6 STATUS: COMPLETE block written; `handoff-phase-6.md` deleted as last act. Nothing
committed (conductor runs gates + reviewer fan-out + the Step-8 commit gate).

## FRAGO 020 — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable (deviation-judge classified JUSTIFIED; risk-neutral record-completeness fix; auto-apply + log)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 6 boundary review (post-diff reviewer fan-out:
           code-reviewer, acceptance-verifier, rules-compliance, deviation-judge, performance —
           all returned; performance's finding routed through a dedicated deviation-judge dispatch)
Trigger:   The performance reviewer found `soa_candidate_query`'s computed `hot_fields` (exactly
           which fields a hot loop touches — the whole point of D5's ≤2-field-union criterion) is
           never consumed by Phase 5's codegen (`soa_gather_into`/`array_elem_get_into` in
           emit.rs unconditionally gather ALL declared fields, per `p5-ir-evidence.md:39`'s
           documented "design c: gather full element, let DSE/SROA drop unused fields"). This is
           plausibly a real, independently-actionable contributor to Phase 6's measured ~1.0x (no
           benefit) result in shipped O0 binaries — distinct from "no LLVM optimization pipeline
           exists" (FRAGO 017/E14's framing). Deviation-judge verdict: the CODE choice (full-element
           gather) is JUSTIFIED — a true selective gather would require re-auditing every
           full-fidelity consumer (`.copy()`/`soa_copy_to_aos`, background/spawn-arg passing, future
           serialization) against a "cold fields may be garbage" invariant, a choke-point-contract
           redesign genuinely out of Phase 5's charter and NOT safe to rush mid-review — so
           reopening Phase 5's already-boundary-committed, already-adversarially-gated (E3/E6/E9)
           codegen in this fix-loop is explicitly ruled OUT. But the Future Requirements ledger's
           OMISSION of this narrower, cheaper, distinct fix (as opposed to only naming "wait for an
           optimizer pipeline") is an UNJUSTIFIED record gap — a plan-source-of-truth
           headline-vs-deferrals completeness issue, not code drift. Risk-neutral: this FRAGO adds a
           missing FR entry only; no code changes, no re-scoring of E14 (unchanged).
Changes (plan.md — applied by the Phase 6 fix-round executor, not the conductor directly):
  - Future Requirements: new entry #15 — WHAT: selective hot-field-only element materialization for
    admitted SoA arrays (bypass the current full-element gather in `soa_gather_into`/
    `array_elem_get_into`, consuming the already-computed `hot_fields` set instead of ignoring it).
    WHY DEFERRED: requires auditing every full-element consumer (`.copy()`/`soa_copy_to_aos`,
    background/spawn-arg passing, future serialization) to either route them onto an always-full-
    gather path or prove hot_fields totality across every consumer — a choke-point-contract redesign,
    not safely batched into a Phase 6 boundary-review fix-loop. COST: ~1 dedicated session +
    E3/E6/E9 re-review (the same adversarial gates Phase 5's codegen already cleared once, must
    clear again against the new lowering). TRIGGER: before or alongside any future
    optimization-pipeline milestone (FR#2/E14) — the two fixes compound and should be evaluated
    together, since a selective-gather fix might independently deliver a real (if smaller) win even
    without an optimizer, making it the cheaper first move.
Unchanged: E14's score and framing (still correctly names "no LLVM optimization pipeline" as ONE
           real cause); Phase 5's codegen (NOT reopened — explicitly ruled out this round); FR#2's
           existing text (a distinct, separate deferral).
Override:  N/A — risk-neutral, record-completeness only.

## Session log — 2026-07-04 — session-id: phase6-fixround1-executor-2026-07-04-m5

Phase 6 boundary-review fix-round 1 (conductor-dispatched, responding to the reviewer fan-out).
All changes plan-text/comment/scratch-hygiene — zero behavior change, zero code-token change.

- **FRAGO 020 applied:** Future Requirements #15 added to plan.md exactly per FRAGO 020's Changes
  text (selective hot-field-only element materialization deferral — WHAT/WHY/COST/TRIGGER).
- **Session-id catch-up (rules-compliance finding):** `plan-conductor-2026-07-04-m5-fable`
  (author of FRAGOs 014-020) appended to plan.md's frontmatter session-id chain, followed by this
  session's own id.
- **Bench-comment provenance repoint (code-reviewer):** `soa_calibration.rs` header comment now
  cites `soa-threshold-raw-2026-07-04.md` "Step 3 calibration verdict" (the nonexistent
  `soa-threshold-provenance.md` reference removed).
- **Bench-comment E13 causal correction (code-reviewer):** the 131072-cap justification no longer
  cites the falsified "SIGABRT at 262144 visits" as 2-axis proof; it now cites the corrected
  segment-3 bracket (N=8/R=65536 = 524,288 visits crashes healthy; N=512/R=1000 = 512,000 visits
  passes; N=8/R=32768 = 262,144 visits passes clean) and attributes the old reading to FRAGO 018's
  stale-runtime-archive bug. Cap value unchanged.
- **Key Outcome #3 aligned (code-reviewer):** "each point 4-18% below the noise floor, direction
  uniform" → "9 of 10 points 4-18% slower, each below the ~15% noise floor, with N=16 at parity —
  ratio 0.997" (matches the raw provenance file).
- **Segment attribution fixed at BOTH sites (deviation-judge):** E13 risk row + Phase 8 step-1
  precondition now read "segment-2 evidence established the 2-axis principle, and segment-3's
  clean re-run (healthy toolchain) confirms total-visits-alone is neither necessary nor
  sufficient" — the bracketing numbers are segment-3's corrected re-run, not segment 2's.
- **Scratch hygiene (code-reviewer):** untracked 48MB `tmp-p6-probe/` deleted (the phase-close
  deletion the plan promised).
- **§6.1 durable deferrals filed (performance reviewer's two statistical-rigor findings):** two
  four-field WHAT/WHY/COST/TRIGGER deferrals written to the roadmap's audit.md
  (`.claude/planning/active/2026-05-21-v0-3-concurrency-perf/audit.md`) — (A) construction-cost
  confound in the calibration harness (key `2026-07-03-v0-3-m5-auto-soa#6:
  crates-ynz-driver-benches-soa-calibration-rs-159`), (B) noise-floor regime mismatch +
  single-sweep-invocation rigor (key `2026-07-03-v0-3-m5-auto-soa#6:
  crates-ynz-driver-benches-soa-threshold-raw-2026-07-04-md-24`). Both idempotency-grepped before
  writing.

Nothing committed (conductor owns the Step-8 commit gate). No new deviations discovered.

## Session log — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable (Phase 6 boundary review, closing)

Full re-review round after the fix-round: green-check (green, 6/6 gates, secret-scan pass via
fallback) + graveyard-auditor (clean, 40 corpses adjudicated, 0 findings) + the full 5-reviewer
fleet re-run (code-reviewer, acceptance-verifier, rules-compliance, deviation-judge, performance) —
0 blockers, 0 should-fix, 2 self-graded cosmetic/out-of-lane notes:

1. **code-reviewer (minor):** `soa-threshold-raw-2026-07-04.md:39` still cites the now-deleted
   `tmp-p6-probe/seg3-bracket/` as an evidence path (the underlying bracket data is transcribed
   inline in the same file's table, so the conclusion isn't left unsupported — purely a dangling
   path reference, phase is terminal, no downstream consumer).
2. **performance (out-of-lane, explicitly not graded):** the headline narrative (Mission ¶2, Phase
   6 STATUS block, E14 risk row) attributes the O0 no-benefit result solely to "no LLVM
   optimization pipeline," without also naming FR#15's independently-identified `hot_fields`
   dead-code contributor. Reviewer agreed the routing (defer FR#15, don't reopen Phase 5 codegen)
   is correct — this is a documentation-nuance observation only.

**Ceiling call (verification.md's YAGNI ceiling, three-part test, all hold):** (a) both are
narrowing variants of already-fixed classes (the tmp-p6-probe cleanup already landed; the
performance-honesty reframe already landed via FRAGO 017/020); (b) both are explicitly self-graded
non-blocking by their own reviewer ("purely cosmetic," "not a graded finding"); (c) the risk posture
is floor-pinned — no correctness, security, or user-facing-claim accuracy issue rides on either (the
dangling path is inert prose; the headline nuance doesn't change what's true, only how completely
it's explained, and FR#15 already carries the full truth for anyone who reads past the headline).
Per no-duct-tape's cost-asymmetry logic, spinning a third fix-round + full 5-reviewer re-run to chase
two reviewer-acknowledged cosmetic items would itself be the over-engineering the ceiling exists to
stop. **Accepted as a floor-pinned residual, not fixed this round** — named here per "no silent caps"
rather than silently dropped. Boundary commit proceeds.

## FRAGO 021 — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable (deviation-judge classified JUSTIFIED; risk-neutral, mechanical, LOW; auto-apply + log)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 7, segment 1 (mid-phase)
Trigger:   ¶Invariants → Teaching claimed the SoA lint hover is "gated by jargon_audit.rs" — a
           claim D9 itself had already flagged as UNVERIFIED at plan time, mandating Phase 7 step 1
           re-verify it before relying on it. The executor's re-verify found the claim false:
           `jargon_audit.rs` audits diagnostic-site strings, LSP messages, CLI strings, and
           deferred-feature/muted-hint-domain fields, but has NEVER audited `[[lint_rule]]`
           template/description text — a pre-existing gap (M4's two lint rules were never gated
           either), not introduced by this milestone. Deviation-judge verdict: JUSTIFIED and
           in-scope for Phase 7 (the Teaching phase) to close now — the fix (a generic
           `no_banned_jargon_in_lint_rule_templates` test covering ALL `[[lint_rule]]` entries) is
           the authoritative-derivation-consistent move (one canonical audit, not a twin scoped only
           to the new rule); deliberately excluding M4's pre-existing rules would itself be the
           anti-pattern (a parallel/partial audit surface).
Risk:      LOW, mechanical — test-only addition, zero runtime/codegen behavior change, mirrors
           jargon_audit.rs's existing per-registry-kind pattern. No Floor-B concern.
Changes (plan.md — applied by the Phase 7 segment-2 executor, not the conductor directly):
  - Phase 7 step 1 (plan.md): add a sub-step — extend `jargon_audit.rs` with a
    `no_banned_jargon_in_lint_rule_templates` test that audits every `[[lint_rule]]` entry's
    template/description text in `registry/features.toml` (covering M4's `cross-thread-fields-not-
    padded`/`prefer-yielding-sleep` AND M5's new `array-using-soa-layout`), so the Teaching
    invariant's "gated by jargon_audit.rs" claim becomes actually true rather than aspirational.
  - ¶Invariants → Teaching: no wording change needed (the claim was always meant to be true; this
    FRAGO makes it so rather than rewriting the claim to admit a gap).
Unchanged: everything else in Phase 7; M4's lint rules' own behavior (only their TEXT gets audited,
           not modified, unless the new test finds an actual violation — if so, that's a SEPARATE
           finding to surface, not silently fixed under this FRAGO's cover).
Override:  N/A — risk-neutral, mechanical.

## FRAGO 022 — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable (deviation-judge classified JUSTIFIED; risk-neutral typo correction; auto-apply + log)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 7, segment 1 (mid-phase)
Trigger:   Phase 7 step 5's docs-graduation text lists the "Auto-SoA layout" IMP-collections
           section's decisions as "D1/D3/D4/D5/D9/D10/D11" — but D10 (plan.md Recorded Decisions) is
           the Fable-executor-model-dispatch decision (¶4/¶5 Sustainment), categorically unrelated to
           SoA/array design. Deviation-judge cross-checked the full D1-D13 ledger against BOTH
           step-5 docs-graduation lists (by-value: D2/D6/D7/D8/D12/D13; SoA minus D10:
           D1/D3/D4/D5/D9/D11) and confirmed all 12 real design decisions are already covered
           exactly once without D10 — it is pure surplus, not a miscite standing in for an omitted
           decision. The segment-1 executor correctly declined to import D10's content into
           IMP-collections.md and flagged the typo rather than silently working around it.
Risk:      N/A — risk-neutral, a one-line text correction.
Changes (plan.md — applied by the Phase 7 segment-2 executor):
  - Phase 7 step 5 (plan.md): correct the Auto-SoA section's decision list from
    "D1/D3/D4/D5/D9/D10/D11" to "D1/D3/D4/D5/D9/D11" (drop D10).
Unchanged: the by-value section's decision list (D2/D6/D7/D8/D12/D13, already correct); everything
           else in Phase 7.
Override:  N/A — risk-neutral.

## Session log — 2026-07-04 — session-id: phase7-executor-2026-07-04-m5-seg2
Phase 7 segment 2 (resume from segment 1's PARTIAL at `phase-7/step-2`). First actions: applied
the two conductor-classified, pre-authorized plan-text amendments (executor applies, per charter):
- **FRAGO 021 applied** — Phase 7 step 1 gains the `no_banned_jargon_in_lint_rule_templates`
  sub-step (audit EVERY `[[lint_rule]]` entry's description/what/what-instead/why template text;
  M4's two rules + M5's `array-using-soa-layout`).
- **FRAGO 022 applied** — Phase 7 step 5's docs-graduation decision lists made explicit with D10
  dropped from the Auto-SoA set (by-value: D2/D6/D7/D8/D12/D13; Auto-SoA: D1/D3/D4/D5/D9/D11).
  Application note: the literal erroneous string "D1/D3/D4/D5/D9/D10/D11" named by FRAGO 022 does
  not appear anywhere in plan.md (verified by grep — plan.md's step 5 carried only the unenumerated
  word "decisions"; the erroneous list lived in segment 1's handoff/dispatch text). The FRAGO's
  corrected END-STATE was applied by making the lists explicit in step 5; no other sibling
  occurrence of the wrong list exists in plan.md (whole-plan grep clean).
Session-id appended to plan.md frontmatter in this same action. Segment work (step C) follows;
this entry is extended at segment close.
Segment close (same session): PARTIAL at `phase-7/step-4` (context-budget checkpoint at the
step-2/step-4 seam; tree fully green-building — cargo check/clippy/fmt clean, no honored-RED, no
orthogonal reds). Landed this segment: (1) FRAGO 021's sub-step for real —
`no_banned_jargon_in_lint_rule_templates` in `crates/ynz-diagnostics/tests/jargon_audit.rs`; M4's
rules pass clean (no deviation); the test caught "either way" (banned word `Either`) in M5's OWN
seg-1 template — reworded to "in both layouts" in registry/features.toml (own-milestone text,
in-scope). (2) Step 2 complete: `soa::layout_lints` pure builder + `ARRAY_USING_SOA_LAYOUT`
(reads the layout authority + candidates only — no re-derivation), non-salsa wrapper
`queries::soa_layout_lints`, merged at all three consumer seams (codegen_query, LSP
run_and_publish_diagnostics, --json collect_diagnostics), new suite
`crates/ynz-typeck/tests/soa_layout_lints.rs` 4/4. (3) Step 3 confirmed done-in-place.
Receipts: jargon_audit 10/10, soa_layout_lints 4/4, soa_analysis 21/21, false_sharing_lints 5/5,
schema_smoke 32/32. Plan sync: Phase 7 segment-2 blockquote added; handoff-phase-7.md rewritten
in place (resume-at `phase-7/step-4`). Steps 4–5 + close-out remain. Nothing committed.

## Session log — 2026-07-04 — session-id: phase7-executor-2026-07-04-m5-seg3

Phase 7 segment 3 (executor, PARTIAL at `phase-7/step-5` — context-budget checkpoint at the
step-4/step-5 seam, tree green). (1) Step 4 complete: (a) consumer-seam end-to-end test
`crates/ynz-codegen/tests/soa_lint_consumer_seam.rs` (1/1) — the `array-using-soa-layout` lint
verified riding `codegen_query`'s actual diagnostic bucket on `m5_p4_soa_qualifying.ynz` (one
Suggestion, LintRule code, `pts` named, bucket error-free); A1 now exercised at a real consumer
seam. (b) VSCode extension 0.3.0 → 0.3.1 (+ lockfile) — recorded decision: M5 ships on the v0.3.x
patch line (plan ¶1), extension version tracks the release tag. (c) Artifact builds:
`yinz-0.3.1.vsix` packaged in the dev container (`.vsix` gitignored). (d) Hover screenshot
impossible headless (no display) — explicit known limitation, queued for the STATUS block.
Receipts: new test 1/1; `cargo fmt --all -- --check` clean; `cargo clippy -p ynz-codegen --tests`
clean; seg-2 suite receipts unchanged (no other Rust touched). (2) Step-5 orientation fully
prepaid and recorded in the rewritten handoff-phase-7.md (decision-text anchors D2/D6/D7/D8/D12/
D13 + D1/D3/D4/D5/D9/D11 per FRAGO 022, FR #14/#15, paper-traced perf numbers from
soa-threshold-raw-2026-07-04.md, IMP-collections placement plan + reference-sweep inventory,
REF-collections value-form-contains verification incl. the string-field identity caveat + pin
fixture). Two pre-existing divergences observed and queued for the final return (NOT
self-adjudicated): REF-collections.md's predicate-form `.contains(fn)` has no
implementation/usage evidence; IMP-collections.md carries a byte-identical duplicated
`array<T>` → `set<T>` section (lines 451–495). Steps 5 + close-out remain. Nothing committed.

## Session log — 2026-07-04 — session-id: phase7-executor-2026-07-04-m5-seg4

Phase 7 segment 4 (final) — returned **DONE**, phase closed. Step 5 (docs graduation) executed
per segment 3's prepaid orientation (receipts inherited; tree verified unmoved since seg 3 at
dispatch — same HEAD `e94a2a3`, same modified/untracked set):

- **IMP-collections.md**: new sections "Array element storage — by-value inline (v0.3-M5)"
  (D2/D6/D7/D8/D12/D13 + rejected alternatives + D13 TS-aliasing teaching note with before/after
  example + E10 forward-compat note) and "Auto-SoA layout (v0.3-M5)" (D1/D3/D4/D5/D9/D11 per
  FRAGO 022 + rejected alternatives + honest E14 provenance: O0 net 1.00-1.18 no-crossover /
  ~3.3x under opt-18 -O2 (10.77 ms vs 3.29 ms, N=4096), never conflated, bench file linked;
  FR#14 + FR#15 named in prose by plan-id + FR number). Frontmatter updated_at + description
  extended; the in-file :447 cite repointed from the scratch doc to the new section anchor.
- **Scratch trims**: `SCRATCH-future-auto-soa.md` + `SCRATCH-future-array-by-value-element-storage.md`
  rewritten as pointer stubs at the new sections (files kept — design_future_sync SKIP list +
  historical links name them).
- **Reference sweep** (per seg-3's grep-verified inventory): IMP-concurrency.md "Design record:"
  cite → by-value section; REF-mvp-scope.md:237 "Locked design" → Auto-SoA section (+ frontmatter
  bump applied); SCRATCH-future-designs-index.md auto-soa row → shipped/graduated;
  design_future_sync.rs two SKIP rationale strings updated → test re-run **3/3 green**;
  integration.rs:4750 comment cite left as-is (resolves through the stub).
- **docs/README.md**: two new topic rows (by-value element storage; Auto-SoA layout) linking the
  IMP-collections anchors; frontmatter bump.
- **REF-collections.md**: `.contains(value)` list entry + short HS-grad section (one-line intro,
  realistic `Position` example with number-only fields — string fields compare by identity per
  D12, so no string-field behavior is claimed; pointer to `.find(fn)` for condition matching).
  Frontmatter bump.

**Two conductor-cleared adjacent doc-defect fixes (audit-note only, per the conductor's ruling —
no FRAGO):**

1. **REF-collections.md:152 stale predicate-form `.contains(fn)` REMOVED**, replaced by the
   value-form entry only. Verification basis (independently confirmed by the conductor, cited
   per the ruling): `registry/features.toml`'s three `contains` entries (string:801, array:1867,
   fixed:2025) are all `kind = "method_1arg"` value-form since M3/M4; zero
   `[[deferred_language_feature]]` entry exists for a predicate form; `builtins.rs`/`emit.rs`
   implement only the value form; zero closure-arg `.contains()` usages anywhere in the repo.
   This closes FRAGO 008's "reconcile the REF home" mandate as a correction, not a silent
   deletion — the fn-form line was stale/aspirational spec text with no implementation ever.
2. **IMP-collections.md:451-495 byte-identical duplicated "array<T> → set<T>" section** — second
   copy deleted, first kept (pure mechanical dedup, diff-verified byte-identical pre-edit; also
   forced by the step-5 edit anchor, which was non-unique while the duplicate existed).

**Phase close-out:** STATUS: COMPLETE block written (honest E14 perf caveat consistent with
Phase 6; headless-environment hover-screenshot limitation named explicitly). Exit criteria:
schema_smoke 32/32 (seg-2 receipt — registry untouched this segment), jargon_audit 10/10 incl.
`no_banned_jargon_in_lint_rule_templates` (seg-2 receipt — no template text changed this
segment), VSCode artifact builds (`yinz-0.3.1.vsix`, seg-3 receipt), docs land per
docs-checklist (this segment). `handoff-phase-7.md` deleted as the last act. Nothing committed
(conductor owns the boundary gate). No new deviations beyond the two cleared fixes above; the
predicate-form divergence queued by seg 3 is resolved by ruling #1 rather than left open.

## Session log — 2026-07-04 — session-id: phase7-fixround1-executor-2026-07-04-m5

**Phase 7 fix round 1 (boundary fix, dispatched by the conductor after its cheap-gates pass).**
One failing test: `cross_impl_consistency.rs::corpus_byte_identical_across_auto_parallel_modes`.

**Paper-Trace (verified before fixing):**
- Observed: `m5_p4_soa_qualifying.ynz` + `m5_p5_soa_copy_wait_bg.ynz` MODE-DIVERGENT (2 of 433
  compared corpus files); stdout byte-identical both modes (`2628\n5256\n`; `hotx…bg: 30\n`),
  exit 0 both. Direct stderr diff: default mode carries the `array-using-soa-layout` Tier 3
  lint; `YNZ_NO_AUTO_PARALLEL=1` stderr is empty.
- Root cause: the lint fires only when SoA layout is admitted, and `YNZ_NO_AUTO_PARALLEL`
  structurally prevents admission (the "no-auto-parallel disables SoA" hard invariant,
  `crates/ynz-typeck/src/soa.rs` — the same gate #2 the milestone's dual-mode AoS oracle relies
  on). Intentional, documented-divergence class (same as the M3b intended-reorder exclusion) —
  not a computation/output regression.

**Fix:** added both fixtures to the test's existing intentional-divergence exclusion list
(`crates/ynz-driver/tests/cross_impl_consistency.rs`, after the `v0_3_m3g_overlap_proof.ynz`
entry) with a WHY comment naming the gate-#2 mechanism and the M3b precedent.

**Sibling check:** empirical, not inferred — the pre-fix full-corpus run compared all 433
non-excluded files across both modes and found exactly these 2 divergent; no other
`m5_p4_*`/`m5_p5_*` (or any other) fixture diverges (decline-case fixtures never lint; the
sweep itself proved the rest mode-identical).

**Receipts:** `corpus_byte_identical_across_auto_parallel_modes` re-run post-fix: ok (1 passed,
208.7s). Full `cargo test --workspace` in the dev container ran to completion through the final
doc-test targets, all green (cargo default fail-fast exits at the first failing test executable,
so completion of the last targets certifies every earlier binary). `cargo fmt --all -- --check`
clean; documented gate `cargo clippy --workspace -- -D warnings` clean. Two PRE-EXISTING
test-target-only clippy warnings observed (NOT introduced by this fix, both outside this fix's
diff: collapsible-if at cross_impl_consistency.rs:207 in the determinism test; unused `stderr`
at integration.rs:672) — outside the project's documented clippy gate (which omits `--tests`);
surfaced in the executor return for the conductor, not silently fixed (out of this dispatch's
scope).

**Plan sync:** Phase 7 STATUS block gained the one-sentence boundary-fix note; session-id
appended to plan.md frontmatter in the same action as this entry. Nothing committed (conductor
owns the commit gate). No FRAGO filed — no plan-text change beyond the STATUS note; the fix
implements the conductor's own dispatch spec, verified against ground truth before applying.

## Session log — 2026-07-04 — session-id: phase7-fixround2-executor-2026-07-04-m5

**Phase 7 fix round 2 (dispatched by the conductor after the full reviewer fan-out: 1 blocker +
2 should-fix).**

**Blocker — REF-collections missing the D13 teaching note:** the plan's Phase 7 step 5 text and
IMP-collections' own "must keep carrying this in HS-grad wording" assertion both required a
copy-on-persist snapshot callout in `docs/reference/REF-collections.md`; the artifact carried
none. Fix: new "Storing a shape makes a copy" section (placed after "Writing by index" — store
semantics follow read/write access), mirroring IMP-collections' D13 worked example in HS-grad
wording with the explicit JavaScript-aliasing contrast the plan text mandates (plan line ~284;
spec-writing.md's "no other-language comparisons" guidance is deliberately overridden here by
the plan's explicit TS-aliasing-note requirement — recorded decision). Example run-verified in
the dev container (stored element prints 1.0 after `p.x = 99.0`; write-through prints 99.0).

**Should-fix 1 — `.contains` stated rule overclaimed:** REF-collections said a shape matches
"when every field matches," contradicting shipped D12 semantics (pointer-typed fields compare
by identity). Fix: rule restated in HS-grad wording (number/bool fields by value; string /
nested-shape / collection fields only when both hold "the very same stored value") + a
surprising-case example. **Verification catch (Paper-Trace):** the reviewer's suggested example
— two separately-built shapes with equal string LITERALS do not match — is FALSE today:
observed `true` (pin fixture `m5_p3_sweep_shape_eq_string_field` cell 2; LLVM merges identical
unnamed_addr string globals, an artifact, not a guarantee). Expected per reviewer: `false`.
Resolution: the doc example builds the string at RUNTIME (`${prefix}ce` interpolation — the
pin's discriminating cell 3), which genuinely prints `false`; run-verified. The "very same
stored value" wording stays honest in BOTH cells (merged literals ARE one stored string). Also
added the `.find(p => p.name == ...)` text-match escape hatch — string `==` is value equality
via `ynz_string_eq` (emit.rs:16308, NFC-normalized), verified in codegen; `.find(fn)` lambda
syntax matches the file's existing spec'd-ahead usage (not yet runnable — no fixture uses
dot-method lambdas; consistent with the file's pre-existing `.find` examples).

**Should-fix 2 — dual-mode sweep dropped ALL comparison for `m5_p4_soa_qualifying.ynz`:** fix
round 1's all-or-nothing exclusion silently dropped the fixture's only runtime
stdout-equivalence oracle. Fix (option (a) — the harness compares three fields inline, so
per-field granularity is trivial): both M5 SoA fixtures removed from the full-skip list; new
`stderr_diverges_by_design` narrow-skip covers stderr only, stdout + exit code re-enter the
sweep (both fixtures narrowed — same gate-#2 divergence class; m5_p5's stdout assertion is now
redundant with its dedicated matrix test but consistent treatment beats a split exclusion).

**Receipts:** full `cargo test --workspace` in the dev container exit 0 (ran through the final
doc-test targets; fail-fast semantics certify every earlier binary);
`cargo test --test cross_impl_consistency` explicitly re-run: 2/2 ok in 230.5s —
`corpus_byte_identical_across_auto_parallel_modes` green WITH the narrowed exclusion, proving
m5_p4/m5_p5 stdout + exit code byte-identical across modes. `cargo fmt --all -- --check` clean;
`cargo clippy --workspace -- -D warnings` clean (CI convention, no `--tests`).

**Deviation surfaced (NOT self-classified, for conductor routing):** pre-existing codegen ICE,
orthogonal to this fix round — a `number` shape field initialized from an INT literal on the
shape-literal→array path panics at `crates/ynz-codegen/src/emit.rs:19678` ("Found IntValue …
expected PointerValue"); decimal literals work. Minimal repro: `shape P { x: number }` +
`let a: array<P> = []` + `a.add({ x: 1 })`-class program. Consequence: several PRE-EXISTING
REF-collections examples (e.g. the `Position { x: number }` contains example with `{ x: 0, y: 0 }`
int literals) would ICE if run today; typeck admits the int→number coercion, codegen doesn't.
This session's NEW examples were made runnable (decimal literals / `int` health) and
run-verified. Also observed (pre-existing, not fixed): REF-collections uses double-quoted
string literals throughout while REF-strings.md rules them a compile error (backticks only) —
new examples use backticks; the file-wide drift is out of this dispatch's scope.

**Plan sync:** Phase 7 STATUS block gained the fix-round-2 note; session-id appended to
plan.md frontmatter in the same action as this entry. Nothing committed (conductor owns the
commit gate). No FRAGO filed — no plan-text change beyond the STATUS note; the two deviations
above are surfaced for the seam, not self-adjudicated.

## Session log — 2026-07-04 — session-id: plan-fixup-icedefer-2026-07-04-m5

**Standalone Phase 7 fix-round follow-up dispatch: filed the ELEVATED durable deferral for the
pre-existing int-literal→`number` codegen ICE** surfaced by fix round 2's docs work and
independently source-confirmed by a deviation-judge dispatch. Record-only dispatch — no Rust
source touched, no fix attempted (out of M5's array/SoA charter: the bug lives in untouched
legacy numeric-literal codegen; M5 Phase 2 only added Shape/Maybe arms to `store_field`).

**The bug (broader than the shape-literal→array path first observed):** `Expr::IntLit`
(`crates/ynz-codegen/src/emit.rs:14101`) unconditionally lowers to a raw `i64` `IntValue`
(contrast `NumberLit` at `emit.rs:14103-14136`, alloca+store decimal128 → pointer), while BOTH
`store_field`'s `Type::Number` arm (`emit.rs:19674-19679`) AND the plain `store()` arm
(`emit.rs:19552-19557`) unconditionally `.into_pointer_value()`. Typeck admits the coercion
(`crates/ynz-typeck/src/check.rs:2162-2166`, type-level retype, no AST rewrite), so
`let x: number = 5` — no shape, no array — type-checks then panics ("Found IntValue … expected
the PointerValue variant"). No fixture/example in the repo exercises the pattern (all existing
usages are decimal literals), which is why it survived undiscovered until Phase 7.

**Filed (two places, per the roadmap's deferral + capability-ledger convention):**
1. Roadmap `audit.md` (`.claude/planning/active/2026-05-21-v0-3-concurrency-perf/audit.md`) —
   four-field WHAT/WHY/COST/TRIGGER deferral, Idempotency-Key
   `2026-07-03-v0-3-m5-auto-soa#7: crates-ynz-codegen-src-emit-rs-14101` (grepped first: no
   pre-existing key, no duplicate). Marked ELEVATED in the entry body — a real user-facing
   crash on a common beginner pattern, not routine debt.
2. Roadmap `roadmap.md` — new "Fix codegen ICE: bare int literal into a `number`-typed slot"
   unscoped-capability row added to BOTH Capability Ledger tables (mirroring the FRAGO 015/018
   unscoped-row format), status `unscoped → needs a milestone`, with the ELEVATED framing and
   full file:line evidence.

**Plan sync:** session-id `plan-fixup-icedefer-2026-07-04-m5` appended to plan.md frontmatter in
the same action as this entry. No phase checkboxes touched (this dispatch owns no plan step).
Nothing committed (conductor owns the commit gate). No FRAGO filed — this dispatch changes no
plan text; it executes the already-routed deferral-filing instruction from the conductor after
the deviation-judge's independent confirmation of fix round 2's surfaced deviation.

## Session log — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable (Phase 7 boundary review, closing)

Full re-review round after fix round 2: green-check (green vs this repo's actual CI convention —
no `--tests`; one pre-existing timing-sensitive flake re-verified passing in isolation, unrelated
to this diff) + graveyard-auditor (clean, 0 findings) + the full 5-reviewer fleet re-run
(code-reviewer, acceptance-verifier, rules-compliance, deviation-judge, doc-auditor) — 0 blockers,
0 should-fix, 3 self-graded minor nits accepted as a floor-pinned residual (same YAGNI-ceiling call
as Phase 6, not chased into a third round):

1. **doc-auditor (minor):** `REF-collections.md:207`'s `.contains` value-vs-identity rule says
   "number and boolean fields" but the worked example uses an `int` field — `int` is value-compared
   too, just not named in the enumeration. Cosmetic wording gap, no behavior claim is wrong.
2. **deviation-judge (minor):** the two pre-existing clippy warnings fix-round-1 surfaced
   (`cross_impl_consistency.rs:207` collapsible-if; `integration.rs:672` unused `stderr`) were
   noted in the narrative but never given the same formal 4-field deferral treatment as the
   sibling 9-violation clippy-debt entry — inconsistent bookkeeping, zero functional risk (outside
   this repo's own documented, `--tests`-free clippy gate).
3. **deviation-judge (minor):** the `plan-fixup-icedefer` session wrote to the roadmap's own files
   but never appended to the *roadmap's* frontmatter session-id chain (only this plan's) — a
   pre-existing gap pattern (other roadmap-writing sessions have the same omission), not newly
   introduced here.

All three are narrowing/cosmetic variants of already-substantively-fixed work, self-graded
non-blocking by their own reviewers, with zero correctness/security/user-facing-claim-accuracy
risk riding on any of them. Accepted as floor-pinned residuals per verification.md's YAGNI
ceiling — named here per "no silent caps," not chased. Boundary commit proceeds.

## Session log — 2026-07-04 — session-id: phase8-executor-2026-07-04-m5

Phase 8 segment 1 (dispatch scope: steps 1-5 full, step 6 partial — CHANGELOG draft only, no
release action, per Patrick's instruction this session). Returned **PARTIAL at `phase-8/step-2`**
(context-budget checkpoint at the step-1/step-2 seam; tree green). **Step 1 DONE** — demo + golden:

- `examples/pirates-roster/entrypoint.ynz` v0.3-M5 section (`m5_soa_demo()`, called last):
  128-element literal `array<Cannonball{x,y,vx,vy: number, ship: string}>`, 64 physics steps
  (whole-element IndexAssign scatter — probe-verified that for-in loop-var field writes do NOT
  write back; D13 copy semantics), per-step hot for-in sweep on `y`, final sweep on `x`+`y`
  (union {x,y}), cold `ship` read, and a 66-element `array<Pirate>` crew tally showing the
  lend-self filter (declines via `LendSelfMethod{recordHit}`) in the demo itself. Lint
  `array-using-soa-layout` fires exactly once (volley), inline comments point at it + carry the
  honest E14 perf note (both measurements, never conflated).
- **E13 precondition (FRAGO 016/019) receipt:** (N, R) = 128 × 65 for-in sweeps → 8,386 for-in
  visits, 66 for-in entries — inside the proven-safe joint region (visits ≤ 131,072 AND entries
  ≤ 16,384, soa-threshold-raw-2026-07-04.md). Checksum tripwire paper-trace (closed forms vs
  observed, residual 0 on all six):
  - heightSum: Σ_{t=1..64} [t·Σvy − 16·t(t−1)] with Σvy = 2092.8 → 2092.8·2080 − 16·87360 =
    4,353,024 − 1,397,760 = **2,955,264** — observed 2955264.00 ✓
  - sumX: 128·64 + 3.2·8128 = **34,201.6** — observed 34201.60 ✓
  - sumY: 128·136 + 6.4·8128 = **69,427.2** — observed 69427.20 ✓
  - lead y₀: 64·10 − 0.25·2016 = **136** — observed 136.00 ✓; lead ship Fort Pitt ✓
  - crew: Σ 1..66 = **2211** — observed 2211 ✓
- Golden regenerated via its own script; `examples_basics_runs_end_to_end` byte-exact PASS;
  fmt `examples_roundtrip` PASS. Demo tail (post "all 8 pirates done") byte-identical across
  default vs `YNZ_NO_AUTO_PARALLEL=1` (lint stderr absent sequentially — the structural gate).
- Known pre-existing number-field int-literal ICE (roadmap audit deferral) avoided: decimal
  literals only into `number` slots. One authoring bug fixed pre-landing: Python Decimal
  normalize emitted `1E+1.0`/`2E+1.0` literals — corrected to 10.0/20.0.
- New probe-established substrate facts (first coverage): number(decimal128)+string fields in an
  SoA-admitted array are correct end-to-end (forced-SoA probe + the real N=128 demo, closed-form
  checksums exact).

Handoff: `handoff-phase-8.md` (resume-at `phase-8/step-2`, receipts inside — D6 never armed →
zero new error classes; gallery grep clean; E11 like-for-like caveat re the demo's added source).
No deviations requiring FRAGO routing surfaced this segment. Scratch: `tmp-p8-probe/` untracked.

## Session log — 2026-07-04 — session-id: phase8-executor-2026-07-04-m5-seg2

Phase 8 segment 2 — steps 2-5 COMPLETE, step 6 PARTIAL by instruction (release withheld,
Patrick's call). Full record in the Phase 8 STATUS block (plan.md ¶3.3). Highlights + the
step-6 deliverables that live here:

- **Step 2:** zero-new-error-classes deliberate-omission row added to
  `examples/primantis-orders/README.md`; stale-trigger grep re-verified clean;
  `error_galleries.rs` untouched (no v0_3_m5 reference exists).
- **Step 3:** `soa-enumeration-report.md` committed (sibling file) — 599 files, 57 verdict
  rows, all absences accounted; durable demo pin added to `crates/ynz-typeck/tests/soa_analysis.rs`
  (`pirates_roster_demo_volley_admits_and_crew_declines_lend_self`, 22/22 green).
- **Step 4 (E11) paper-trace:** Observed A=210ms (P0 baseline, old compiler, 1159-line source) /
  B=170ms (new compiler, SAME source) / C=254ms (new compiler, 1352-line source incl. demo).
  Expected: B ≤ A×1.10. Residual: B−A = −40ms (−19%) — negative; gate PASSES with margin.
  Hypothesis for C−B (+41% wall vs +16.6% lines): the demo section (128-element literal +
  SoA-admitted lowering) is denser compile work per line — new-code cost, not analysis
  overhead on old code. Evidence path: `baselines-p0.md` (A, methodology);
  `tmp-p8-probe/entrypoint_1ac52fd` swap runs this session (B, C); entrypoint restored
  byte-exact (md5 `2a09c608…` before/after).
- **Step 4 deviation surfaced (NOT self-classified):** FRAGO 018's literal recipe
  `cargo clean -p ynz-runtime` does NOT remove release-profile artifacts on this cargo
  version — the post-clean `--release` build finished in 0.05s with the archive mtime
  unchanged (08:51 vs 14:03 run time). `cargo clean -p ynz-runtime --release` is the
  effective form (24 files removed, fresh archive + fresh driver verified by mtime).
  Side effect worth knowing: the profile-less clean removes the DEBUG archive, which broke
  the test build (`ynz-watch` `include_bytes!(env!("YNZ_RT_LIB_PATH"))`) until
  `cargo build -p ynz-runtime` restored it. For the deviation-judge: plan text says
  `cargo clean -p ynz-runtime && cargo build -p ynz-runtime --release`; reality requires
  `--release` on the clean for the guard to bite.
- **Step 5:** `cross_impl_consistency` 2/2 (whole corpus dual-mode byte-identical, incl.
  the demo-bearing tree) + `cargo test --workspace` green (124 test binaries, 0 failures).

### CHANGELOG entry DRAFT (PROPOSAL ONLY — not applied to CHANGELOG.md, no version bump, no tag)

Recommended version: **v0.3.1** (next v0.3.x patch-line slot; Cargo.toml is at 0.3.0 and the
last CHANGELOG entry is [0.3.0] — 2026-07-03).

```markdown
## [0.3.1] — 2026-07-XX — M5: Array-By-Value Element Storage + Auto-SoA Layout

### Fixed
- **Stack-dangling miscompile class eliminated**: `array<Shape>`, `fixed<Shape>`, and
  `map<K, Shape>` now store elements BY VALUE in the collection's own heap buffer instead of
  persisting interior stack pointers — the silent read-of-dead-frame class is structurally
  gone (E8 alloc=free parity held across the migration).
- The `ArrayShapeRuntimeFieldWithWait` guard is LIFTED: runtime-computed shape fields in
  arrays crossing `wait` now compile and run correctly (M5 removed an error class; it added
  none — see `examples/primantis-orders/README.md`).

### Added
- **Auto-SoA layout infrastructure** (codegen-only auto-promotion, no source syntax):
  provably-safe large hot-loop `array<Shape>` bindings (> 64 elements, ≤ 2-field loop union,
  no escape/growth/lend-self/cross-thread-padding) are laid out struct-of-arrays
  automatically, surfaced via the Tier 3 lint `array-using-soa-layout`. **Honest performance
  caveat**: calibration found NO O0 crossover on the shipped workloads — the measured win at
  the locked threshold is cache-locality headroom, not a benchmarked speedup claim; the
  SIZE_THRESHOLD (64, strict) is calibrated conservative (see
  `crates/ynz-driver/benches/soa-threshold-raw-2026-07-04.md`).
- Layout authority: one `layout_decisions_query` source of truth (padding-wins precedence —
  a cross-thread-padded shape is never SoA-split), per authoritative-derivation discipline.
- Demo: `examples/pirates-roster/entrypoint.ynz` v0.3-M5 section — 128-cannonball volley
  physics (SoA-admitted, lint fires) + 66-pirate crew tally (declined via the lend-self
  suppression filter, demonstrated in-demo).
- Suppression enumeration report: every `array<Shape>` site across examples + fixtures with
  its machine-recorded verdict (`.claude/planning/active/2026-07-03-v0-3-m5-auto-soa/soa-enumeration-report.md`),
  plus a durable typeck pin on the demo's two verdicts.

### Deferred (elevated to the roadmap)
- Stale-runtime-archive footgun: a stale `libynz_runtime.a` release archive silently
  miscompiles by resolving old-ABI symbols by name; operational clean-rebuild guard in place
  (note: the clean is per-profile — the profile-less `cargo clean -p ynz-runtime` does not
  touch release artifacts, so the guard cleans + rebuilds BOTH debug and release), durable
  fix tracked on the roadmap.
- Int-literal-into-`number`-field ICE: a bare integer literal assigned to a `number`
  (decimal128) shape field crashes emit.rs; workaround is decimal literals; fix tracked on
  the roadmap.
```

(End DRAFT. Applying it, bumping Cargo.toml, tagging, and `/release` are Patrick's actions.)

## FRAGO 023 — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable (deviation-judge classified JUSTIFIED; risk-neutral cargo-behavior correction; auto-apply + log)
Base:      2026-07-03-v0-3-m5-auto-soa @ Phase 8, segment 2 (mid-phase)
Trigger:   FRAGO 018's rebuild-from-clean guard (prepended to Phase 8 steps 4 and 6) reads
           `cargo clean -p ynz-runtime && cargo build -p ynz-runtime --release` — but the
           segment-2 executor found this recipe is a NO-OP against release-profile artifacts:
           `cargo clean -p <pkg>` without `--release` targets only the debug profile, so the
           "rebuild from clean" guard never touched the release archive it exists to defend
           (verified: post-clean release "rebuild" finished in 0.05s, archive mtime unchanged).
           The corrected form `cargo clean -p ynz-runtime --release && cargo build -p ynz-runtime
           --release` DOES clean/rebuild the release archive (verified: 24 files removed, fresh
           mtime) — but that release-only form has its own side effect: it leaves the DEBUG
           archive untouched, and Phase 8 step 5's dual-mode suite depends on a working debug
           archive (`ynz-watch`'s `include_bytes!` broke until a plain `cargo build -p
           ynz-runtime` (debug) was also re-run). Deviation-judge verdict: JUSTIFIED — a real
           cargo-behavior fact FRAGO 018's own text got wrong, verified before acting on it, not
           executor drift.
Risk:      Risk-neutral cargo-invocation correction — no plan-scope change, no new mitigation
           bucket, purely fixing a command string so the guard actually does what it always
           claimed to do. Elevated to should-fix (not minor) because step 6 (the tag cut) has NOT
           yet executed against this guard — an unfixed plan text at step 6 would silently
           reproduce the identical no-op at the highest-consequence moment (shipping the tag),
           with no visible failure signal (a 0.05s "rebuild" looks identical to success).
Changes (plan.md — applied by a small follow-up executor dispatch, not the conductor directly):
  - Phase 8 step 4's rebuild-guard text: replace `cargo clean -p ynz-runtime && cargo build -p
    ynz-runtime --release` with the both-profile form: `cargo clean -p ynz-runtime && cargo build
    -p ynz-runtime && cargo clean -p ynz-runtime --release && cargo build -p ynz-runtime
    --release` (clean+rebuild debug, THEN clean+rebuild release — the broader form, since step
    5's dual-mode suite between steps 4 and 6 needs the debug archive honest too, and the
    release-only form was empirically shown to leave it stale).
  - Phase 8 step 6's identical guard text: same correction, same reasoning — this is the
    higher-urgency site since step 6 is still unexecuted.
Unchanged: everything else in Phase 8; FRAGO 018's own reasoning/deferral content (untouched —
           this FRAGO only fixes the literal command string, not the guard's purpose).
Override:  N/A — risk-neutral, mechanical cargo-invocation fix.

## Session log — 2026-07-04 — session-id: plan-fixup-frago023-2026-07-04-m5

Applied FRAGO 023 (paperwork-only dispatch, no code/build): replaced the FRAGO 018 rebuild-guard
command string at BOTH plan.md sites (Phase 8 step 4 and step 6) with the both-profile form
`cargo clean -p ynz-runtime && cargo build -p ynz-runtime && cargo clean -p ynz-runtime --release
&& cargo build -p ynz-runtime --release`; per the fixed-fact sweep, also aligned the segment-2
CHANGELOG DRAFT's guard parenthetical (above, this file) to describe the both-profile guard
(it cited only the release-only clean; cosmetic). FRAGO 018's own record and the Phase 8 STATUS
discovery narrative left untouched per FRAGO 023's Unchanged clause.

## Session log — 2026-07-04 — session-id: plan-fixup-ledgerstatus-2026-07-04-m5

Roadmap Capability Ledger status sync (paperwork-only dispatch, no code/build/commit — prompted by
the cumulative cross-phase completion gate's finding that the roadmap's M5 ledger rows were stale:
still "planned" / "planned + active" after all execution phases 0–8 were sealed, verified, and
boundary-committed). Edited `roadmap.md` (`2026-05-21-v0-3-concurrency-perf`) at five sites:

1. §Milestone 5 "Execution plan" status line — "planned + active" → "phases 0–8 complete — tag
   pending Patrick's release action" (+ sealed/boundary-committed 2026-07-04 note).
2. First Capability Ledger table, Auto-SoA row (M5) — "**planned**" → the same
   phases-complete/tag-pending status.
3. First Capability Ledger table, array-by-value + `map<K,Shape>` row (M5, folded-in) — "planned"
   → same status.
4. Second Capability Ledger table (merged pre-migration ledger), Auto-SoA row — "planned
   (plan-id …)" → same status.
5. Second Capability Ledger table, array-by-value row — "planned" → same status.

Deliberately NOT marked "shipped"/"done": the `v0.3.x` release tag is the one outstanding piece,
withheld this session per Patrick's instruction — the ledger now says exactly that instead of
understating (planned) or overstating (shipped). Fixed-fact sweep: grepped the roadmap for every
other "planned"/"active" occurrence tied to M5/array-by-value — the five sites above are the
complete set; unrelated rows (M3c/M3d/M3f, unscoped capability-discovery rows) untouched.

Session-id appended to BOTH frontmatter chains in this dispatch: this plan's `plan.md` AND the
roadmap's own `roadmap.md` (the roadmap chain had previously been missing roadmap-writing sessions
per an earlier deviation-judge finding — this session records itself there correctly).

## Cumulative cross-phase completion gate — 2026-07-04 — session-id: plan-conductor-2026-07-04-m5-fable

**Coupling decision:** RUN (fail-safe default) — this milestone's 9 phases are heavily coupled by
design (by-value storage P2/P3 is the substrate P4/P5's SoA rides on; the harness/perf story P6
feeds directly into P7's teaching text and P8's demo safety bracket; the layout authority P4 builds
is consumed by every later phase) — no attempt was made to prove pairwise disjointness, since it
plainly doesn't hold.

**Diff range:** `1ac52fd..ebca94e` (the fork commit through Phase 8's boundary — all 9 phases,
0 through 8).

**Three-lens fan-out (concurrent):**
1. **code-reviewer** (reuse/consolidation lens, opus·xhigh) — VERDICT: clean, 0 findings. Confirmed
   the ¶3.1 "one representation" architecture held across all 9 phases: no second allocation model,
   no duplicate shape-value-equality/persist-choke implementations, SoA and by-value share one
   `YnzArray` runtime struct, Phase 7's lint and Phase 8's enumeration both read the same
   authoritative `layout_decisions_query`/`soa_candidate_query` rather than re-deriving.
2. **acceptance-verifier** (§3.1 integrated whole + roadmap campaign End State, sonnet) — VERDICT:
   met. All 6 Key Outcomes verified against actual produced artifacts, not narration (guard lift,
   dual-mode byte-identical corpus, the honest SoA perf story consistently carried everywhere, the
   SIZE_THRESHOLD provenance, the D11 padding-wins byte-layout fixture, the docs graduation) — Key
   Outcome 6's "tag ships it" sub-clause explicitly graded as a recorded, deliberate carve-out
   (Patrick's own withheld action), not a failure. 1 should-fix: the roadmap's own Capability Ledger
   status rows were stale ("planned") relative to the plan's actual sealed state — **fixed** in the
   preceding session-log entry above (`plan-fixup-ledgerstatus-2026-07-04-m5`), before this gate
   seals.
3. **deviation-judge** (cross-phase interaction, sonnet) — VERDICT: on-plan, 0 unjustified strays.
   Independently re-verified against live code (not the plan's narrative) that: D2/GR8's
   one-allocation guarantee holds under SoA; Phase 3's suspension-survival guarantee is unbroken by
   Phase 5's segment addressing; the layout authority is the sole source at every downstream
   consumer (grep-confirmed exactly 3 `padded_shapes` reads, matching FRAGO 013's corrected count);
   the two elevated roadmap deferrals (stale-runtime footgun, int-literal ICE) stayed correctly
   out of scope throughout, never partially fixed. Two pre-existing cross-phase frictions (FR#12
   shape-level-padding-forfeits-SoA; FR#15 hot_fields dead-code) were independently re-confirmed as
   already correctly discovered, judged, and routed by earlier phases' own boundary reviews — not
   new findings, no further routing needed.

**Result: 0 blockers, 0 FRAGO candidates, 1 should-fix (fixed before sealing).** No justified
cross-phase divergence requiring a FRAGO; no risk-raising finding requiring the signed-override
gate. Cleared.

**Gate seal:** `Completion-Gate: 2026-07-03-v0-3-m5-auto-soa#cleared` — the git-legible marker
Step-0's completion-window reconcile reads. Continuing to Step 9.1 (AAR).

## AAR — 2026-07-04 — dispatched by session-id: plan-conductor-2026-07-04-m5-fable

# AAR — `2026-07-03-v0-3-m5-auto-soa` (Array-By-Value Element Storage + Auto-SoA Layout)

Grounded in `plan.md` (¶1 Risk Assessment, ¶2 Mission, ¶3.1 Intent & End State, ¶3.3 Phases 0–8,
Future Requirements/Revisit) and `audit.md` (Session log, Context-segment log, FRAGO log 001–023,
Cumulative cross-phase completion gate at the tail). All 9 phases sealed; diff range
`1ac52fd..ebca94e`.

## Question 1 — What was supposed to happen

**Mission (¶2):** after v0.3.0 ships, deliver by-value inline element storage for `array<Shape>`
(new elem_size-aware `YnzArray` ABI, permanently fixing the stack-dangling miscompile class the
M3a guard only masks) and, riding that storage, automatic Struct-of-Arrays layout for large
`array<Shape>` hot loops — zero syntax change, byte-identical output in both scheduling modes —
because Yinz's efficiency-first positioning (Golden Rules 4/8/10) promises a 10–40x cache-locality
win to naive sequential code.

**Intent & End State (¶3.1):** exactly ONE array representation — SoA is a layout variant of the
elem_size-aware by-value `YnzArray`, never a second runtime or a second layout derivation. Six Key
Outcomes define done: (1) runtime-field `array<Shape>` crossing `wait` compiles + prints correctly;
the interim guard and its registry deferral are gone. (2) Every fixture is byte-identical across
default and `--no-auto-parallel` modes. (3) A qualifying hot loop gets SoA automatically, the
`array-using-soa-layout` lint fires with jargon-free hover, and the measured improvement is recorded
honestly. (4) `SIZE_THRESHOLD` ships with committed provenance, never a bare constant. (5) A shape
simultaneously cross-thread-padded AND SoA-candidate resolves through one layout authority (padding
wins, D11), proven byte-layout. (6) Roadmap + scratch docs record the fold; IMP-collections carries
the graduated design; a v0.3.x tag ships it.

**Phase exit criteria (¶3.3):** Phase 0 hard-gates two novel mechanisms (by-value ABI spike,
per-field analysis spike) plus baselines; Phase 1 records the fold; Phases 2–3 land the by-value
substrate (hard-cut ABI, no parallel old path) fully green before any SoA work; Phase 4 builds the
one layout authority; Phase 5 emits SoA codegen; Phase 6 calibrates `SIZE_THRESHOLD` honestly;
Phase 7 ships the teaching surface; Phase 8 authors the demo, enumerates suppression, and cuts the
release tag.

## Question 2 — What actually happened

All 9 phases are marked `STATUS: COMPLETE` in `plan.md` and sealed at boundary commits; the
cumulative cross-phase completion gate ran a 3-lens fan-out — code-reviewer (reuse/consolidation),
acceptance-verifier (§3.1 + Key Outcomes), deviation-judge (cross-phase interaction) — returning 0
blockers, 1 should-fix (fixed pre-seal), 0 unjustified strays.

Phase-by-phase highlights: Phase 0 spikes GREEN, but found the alloc-counter was blind to array/map
buffer mallocs (E8 baseline blindness → FRAGO 005 made visibility a hard Phase 2/3 requirement).
Phase 1's fold-recording hit a stale-roadmap deviation (resolved by importing the split verbatim
first). Phase 2's by-value ABI cut landed across 4 fix rounds, each RED-fixture-first catching a
real silent-miscompile class (aliasing, frame-embed clobber, persist-boundary gaps — FRAGO 007-011).
Phase 3's map symmetric fix RED matrix caught a real pre-existing MapEntry indirection miscompile,
plus two live MapEntry-escape bugs found and fixed. Phase 4's re-verify found a THIRD padding
consumer the original recon missed (FRAGO 013). Phase 5 discovered `.copy()` was a pre-existing
alias no-op contradicting the plan's deep-copy assumption (FRAGO 014, fixed both modes), plus a
bg-arg double-copy leak, plus D11's shape-level-padding-forfeits-SoA interaction (recorded FR#12,
working-as-designed). Phase 6 is the headline event: shipped O0 binaries show NO SoA benefit (net
1.00-1.18x), while the same IR shows ~3.3x under `opt-18 -O2` — falsifying the Mission's "10-40x"
claim (FRAGO 017, honest reframe). Separately found a stale-release-archive silent-miscompile
footgun (FRAGO 018, durably deferred + immediate operational guard) that had already corrupted one
measurement (corrected via FRAGO 019). Phase 7/8 found Phase 4's `hot_fields` analysis was never
consumed by Phase 5's codegen (FRAGO 020, deferred as FR#15), and FRAGO 023 caught the Phase 8
rebuild-guard's own command string was a no-op (`cargo clean -p X` without `--release`) before the
tag-cut step could reproduce it silently.

23 FRAGOs total, every one logged with trigger/risk/changes; the cumulative deviation-judge pass
independently re-verified against live code (not narrative) that no unjustified stray survived.

## Question 3 — Why the gaps (root-cause each divergence)

Full table with citations lives in the AAR agent's own return (referenced here, not re-typed in
full): E8 counter-blindness and Phase 4's missed 3rd padding consumer were both **plan/recon
gaps**, caught by the plan's own mandated re-verify steps. Phase 2/3's fix-round bugs and Phase 6's
falsified perf claim were **unverified assumptions baked into the plan**, caught by RED-fixture
discipline or honest measurement. Phase 5's D11xpadding interaction (FR#12) was a **genuine
emergent property** of composing two independently-correct systems, correctly deferred rather than
redesigned mid-milestone. FRAGO 018's stale-archive footgun was a **pre-existing adjacent bug**,
exposed not caused, correctly scoped out-of-charter. FRAGO 023 was **an unverified claim in the
FRAGO-author's own fix-spec**, caught by an executor who ran the recipe and checked file mtimes
rather than trusting the text. FRAGO 020's `hot_fields` gap was a **cross-phase consolidation
miss** — Phase 5 never fully consumed what Phase 4's analysis computed, surfaced two phases later
by a benchmark symptom.

No divergence in this plan's execution was left undetermined — every one traces to a named root
cause with a citable evidence anchor in the FRAGO log.

## Question 4 — Lessons (surfaced, classified by home; rule-author disposes)

1. **RULE candidate** — honest mid-execution reframe of a falsified headline perf claim, continue
   rather than bury/halt (FRAGO 017's pattern: re-run the risk engine, rewrite Mission/KO/Invariant
   text honestly, continue if residual scores MEDIUM-or-below after mitigation). Cross-refs:
   `REF-decision-philosophy.md`, `plan-source-of-truth.md`'s sibling headline-reconciliation check
   (this is its mid-execution-discovery cousin, not the same check).
2. **RULE candidate (no-duct-tape addendum)** — "cheap immediate operational guard now + durable
   four-field deferral" as an explicit named pattern (FRAGO 018): when a deferred structural fix
   leaves an immediate, in-scope, low-cost mitigating action available, that action should be a
   required fifth element of a legitimate deferral, not an optional nicety. No existing graveyard
   entry names this combination (checked).
3. **Possible 2nd worked example for `verification.md`** — FRAGO 023 is the same class
   `verification.md` already documents ("a router's own authored fix-spec is a claim, not ground
   truth") — a cargo-behavior command string instead of a security-boundary fix. Surfaced for
   rule-author's judgment on whether it adds distinct enough texture to warrant inclusion.
4. **GRAVEYARD CORPSE candidate** — "Authoritative Analysis Output Computed But Never Consumed
   Downstream" (FRAGO 020: Phase 4's `hot_fields` computed, Phase 5's codegen never read it). The
   mirror failure mode of `authoritative-derivation.md` (which bans re-deriving an already-computed
   answer) — here the failure is silent underuse instead of re-derivation. Diff-greppable in
   principle (a struct field populated by one pass, never referenced by its designated consumer).
5. **RULE candidate (sustain, likely global not project-local)** — FRAGO 004's blanket pre-sign
   pattern ("waive blocking, never waive the paper trail or the final human gate") ran this entire
   unattended overnight P0-P8 execution across 23 FRAGOs with zero friction and zero HIGH residual;
   the one gate never waived (completion approval) held exactly as designed. Likely belongs in
   `REF-decision-philosophy.md` or `IMP-frago-aar.md` rather than as a Yinz-specific rule.
6. **Project-memory, already correctly homed** — the stale-runtime-archive footgun (FR#14) and the
   O0 stack-growth SIGSEGV ceiling (FR#13) are repo-specific facts already durably filed in
   `plan.md`'s Future Requirements + the roadmap's Capability Ledger. No redirect needed.

## Blocked?

Not blocked — a completed, sealed plan with an extensive audit trail.

## End-State verdict

**Restating acceptance-verifier's cumulative-gate verdict:** MET — all 6 Key Outcomes verified
against actual produced artifacts, not narration. Key Outcome 6's "a v0.3.x tag ships it"
sub-clause is an explicit, recorded, deliberate carve-out (Patrick's own withheld release action),
not a failure.

**AAR-level tag (finer-grained than "met"):** PARTIAL — met, but carrying 23 logged justified
deviations, several carried-forward Future-Requirements entries (E1/E3/E5/E6/E7 recorded MEDIUMs
among them, plus FR#13/#14/#15), and one deliberate headline element (the release tag) still
outstanding by Patrick's own choice.

## Handoff

Lessons 1-6 above handed to rule-author for classification/capture/routing per
`IMP-frago-aar.md` §3 — the AAR proposes, rule-author disposes. Full lesson detail (evidence
anchors, cross-reference candidates, the three open routing questions on lessons 3/4/5) is in the
rule-author dispatch prompt, not re-typed here.
