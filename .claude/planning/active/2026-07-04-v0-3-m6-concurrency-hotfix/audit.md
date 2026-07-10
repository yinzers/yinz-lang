---
name: "v0-3-m6-concurrency-hotfix-audit"
plan-id: "2026-07-04-v0-3-m6-concurrency-hotfix"
created_at: "2026-07-09"
updated_at: "2026-07-10"
metadata:
  type: "audit"
---

# Audit sidecar — v0.3-M6 Concurrency Hotfix

Append-only execution history for the M6 hotfix plan. `plan.md` is current truth;
this file is the session + FRAGO + context-segment log.

## Session log

- `executor-2026-07-10-m6-phase1c-seg7` — 2026-07-10 — Phase 1c segment 7 — **PHASE 1c
  DONE** (steps 3d + 4 + 5 + close-out). **Step 3d:** heap-cell LOOP parity verdict
  RESOLVED (not assumed): **leak-by-shipped-design confirmed** — per-iteration crossing
  maybe/union bindings orphan their promotion cell(s); probe fixture
  `v0_3_m6_heap_cell_loop_parity.ynz` (5-iter maybe loop + 3-iter union loop, values
  200 + square×3 proving genuine per-iteration crossing) shows alloc=12/free=1, gap
  EXACTLY 11 = 5×1 maybe envelope + 3×2 union envelope+payload (Paper-Trace predicted
  pre-run, matched, stable 4/4 runs); pinned as
  `v03_m6_p1c_heap_cell_loop_parity_pins_documented_per_iteration_leak`
  (integration.rs, next to the M5 E8 parity gate). Disposition SURFACED to the seam
  with a candidate four-field deferral (drop-story class, Future Requirements #6) —
  not self-decided. Per-case IR proof: maybe (`m_env_cell` staged via loaded cell ptr
  + frame-slot flush), union (`fig_env_cell` + tag-switched `fig_pay1_cell` deep-copy,
  null-preserving phi), anon (`sm_arg_anon_cell` memcpy'd from the dying literal,
  cell address staged) — no dying stack address reaches any child frame. **Step 5:**
  false-positive sweep extended in place (ONE fixture, no parallel sweep):
  non-escaping maybe/union/anon-arg cases added
  (`v0_3_m6_non_escaping_args_false_positive_sweep.ynz` prints 3.5/9/40/square/6;
  test renamed `v03_m6_non_escaping_args_of_every_widened_type_are_not_wrongly_affected`
  with all prior assertions retained); IR-verified ZERO heap-cell promotion sites in
  the sweep fixture. **Step 4:** full workspace `cargo test --workspace
  --no-fail-fast` GREEN (exit 0; totals recorded below), M3e determinism held under
  load (FRAGO 008 fix), N=10 in-test determinism per new repro; `cargo fmt --all
  --check` clean; house `cargo clippy --workspace -- -D warnings` clean; the two
  pre-existing ynz-driver test warnings GENUINELY fixed (not `#[allow]`'d):
  integration.rs:672's dead `stderr` now feeds a strengthened diagnostic assertion
  (`return` + `nothing` load-bearing terms), cross_impl_consistency.rs:207's nested
  if collapsed to `&&` — `clippy -p ynz-driver --tests` = 0 warnings. Tracked
  WHY-comment clarification confirmed already folded (seg 2). Close-out: Phase 1c
  completion note written to plan.md; `handoff-phase-1c.md` DELETED as the final act.
  Workspace totals: 2307 passed / 0 failed (2306 + the new parity pin).

- `executor-2026-07-10-m6-phase1c-seg6` — 2026-07-10 — Phase 1c segment 6 (PARTIAL
  checkpoint at the plan's 3c `**CHECKPOINT**` mark). **Step 3c DONE — codegen union,
  all union-specific machinery**: (1) `mark_aggregate_arg` widened with `Type::Union`
  (`check.rs`) TOGETHER WITH the annotation-aware override in emit.rs's classification
  loop — the Let's Union annotation resolved via the SAME `ast_type_to_typeck_type`
  the union-ctor arm uses and the SAME `find_let_annotation_type_in_stmts` finder the
  typeck guards use (made `pub` + exported for exactly this — one finder, one
  resolver, no twin), so a union-annotated crossing local routes to the pointer-alloca
  strategy, never `shape_embed_set` (Decision 12); (2) Check 2 union skip
  (arg-escape-only ∧ annotation resolves Union) AND Check 2b union skip AND the M3d
  decline-probe mirror of BOTH — Decision 18, both touch points, probe's nested-shape
  half included; (3) new `union_to_heap_cell` (null→null preserving `is`-none; tag
  tag-switched payload deep-copy into counted heap cells, sizes from the one
  `shape_abi_sizes` source; non-shape variant bits copy raw via switch default);
  (4) union-ctor Let arm crossing branch promotes and stores the cell into the
  PRE-CREATED sm_entry crossing alloca (never a fresh `outer_slot` — the Decision-14
  clobber gotcha); (5) `store_binding` Union arm (crossing → `union_to_heap_cell`;
  non-crossing → plain store, byte-identical); (6) `value_to_stable_bits` KNOWN-HOLE
  doc refreshed textually — NO Union arm added (Decision 15), persist pins unchanged.
  Exit verified: **ALL 6 locked repros GREEN** (maybe/union/anon, value + N=10 each;
  union `square` deterministic); fixed<T> + m3a rejections green; **full workspace
  2306 passed / 0 failed** (incl. integration 522/522 under load); fmt + clippy
  (`-p ynz-typeck -p ynz-codegen -- -D warnings`) clean. Steps 3d + 4-5 remain
  (the two pre-existing ynz-driver test warnings are step 4's job). No new deviation.
  Resume `phase-1c/step-3d` via handoff.
- `executor-2026-07-10-m6-phase1c-seg5` — 2026-07-10 — Phase 1c segment 5 (PARTIAL
  checkpoint at the plan's 3b `**CHECKPOINT**` mark). **Step 3b DONE — codegen maybe +
  anon, both fixes in `crates/ynz-codegen/src/emit.rs` only**: (1) `store_binding`
  Maybe arm promotes a CROSSING maybe binding to a counted heap cell via the existing
  `maybe_to_heap_cell` funnel (crossing membership read from `sm_crossing_names`, the
  one threaded typeck set — no new per-type set, no second classification path;
  non-crossing maybes keep `maybe_to_owned` byte-identical); (2)
  `stage_suspending_call_arg_bits` routes `Expr::StructLit` args through
  `value_to_stable_bits` (the ONE stable-bits marshalling point; covers all three
  staging loops by construction; scope-minimal — StructLit only). Exit verified: maybe
  repro pair (value + N=10) GREEN, anon pair GREEN → 4/6 locked repros GREEN; union
  pair still RED with the byte-identical pre-fix tell (`circle` vs `square` — the
  documented 3c handoff, NOT a regression/ICE); fixed<T> + m3a rejections green; full
  workspace 2304 passed / 2 failed (only the union pair); integration 522/522 under
  full-workspace load; `cargo fmt --all --check` + `clippy -p ynz-codegen -D warnings`
  clean. No new deviation. Resume `phase-1c/step-3c` via handoff.

- `executor-2026-07-10-m6-phase1c-seg3` — 2026-07-10 — Phase 1c segment 3 (PARTIAL
  continuation at `phase-1c/step-3`). Landed NO code edits: the window was consumed by
  step 3's own mandated pre-work — recipe verification against live code plus the
  handoff's open constructibility probes (union re-bind from a variant: typeck-rejected;
  `maybe<int> = 42`: typeck-rejected; union alias + union-to-union assign: constructible,
  so `store_binding` needs a crossing Union arm; union-with-nothing `= none`:
  typeck-rejected). Falsified one segment-2 receipt at new-work granularity: a
  union-ANNOTATED let classifies by RHS type (`Shape{variant}`) in
  `crossing_local_type_from_body` (emit.rs:8418), so post-widening it would wrongly enter
  `shape_embed_set` — the fix needs an annotation-aware classification override, and
  Check 2 (nested-shape) needs the same arg-escape-only skip as Check 2b. All findings
  relayed via `handoff-phase-1c.md` (replaced in place, Decisions 11–16). Returned
  **STATUS: BLOCKED** carrying an `**OVER-FAT-STEP PROPOSAL**` for step 3 (4 sub-steps:
  typeck routing / maybe+anon codegen / union codegen / parity-verdict+IR) — no
  completed-step boundary behind this segment, so no PARTIAL checkpoint was legal; tree
  left green-building (documented RED = the 6 locked repros, unchanged).

- `conductor-2026-07-09-m6-exec` — 2026-07-09 — Execution conductor session opened
  (`/execute-plan`). Step-0 cold-resume: plan was active-but-untouched (no prior
  `audit.md`, no ticked phases) → start at Phase 0. Execution gate **confirmed
  satisfied**: M5 auto-SoA merged to `main` (`97e32f6`) AND M5 tag cut (`v0.3.0-m5`
  present). Feature branch `feat/v0-3-m6-concurrency-hotfix` cut from post-M5 `main`
  (R8 collision window eliminated by construction). Clean-tree preflight surfaced
  three pre-existing dirty files as **known-not-mine** (outside M6 scope, never to be
  staged): `CLAUDE.md`, `crates/ynz-watch/src/error.rs`, `crates/ynz-watch/src/rebuild.rs`.
  Curiosity flagged (non-blocking): tags `v0.3.0-m6` / `v0.3.0-m7` already exist
  pre-execution — presumed pre-cut by the audit/planning PRs (#76/#78/#80), not prior
  M6 work; Phase 0 citation re-verify will confirm the tree matches the plan.
  Executor model override in force for this run: **fable for ALL code execution**;
  reviewers / gates / recon unchanged.

- `executor-2026-07-09-m6-phase0` — 2026-07-09 — Phase 0 (dormancy verification +
  execution-gate confirmation, GATE) executed. All five verdicts, each proven by direct
  read of the live tree:
  1. **Execution gate: SATISFIED.** `97e32f6` (M5 auto-SoA merge, PR #79) confirmed
     ancestor of `main` (`git merge-base --is-ancestor`); tag `v0.3.0-m5` → `f5495c9`
     ("release: v0.3.0-m5"). Pre-existing tags noted: `v0.3.0-m6` → `f0d4946`
     ("release: v0.3.0-m6") and `v0.3.0-m7` → `e7c00a7` ("release: v0.3.0-m7") — both
     are release-style commits cut BEFORE any M6/M7 execution began (this plan's Phase 0
     is the first M6 work). They are pre-cut planning/audit-era tags, not real M6/M7
     code work. Recorded, non-blocking per the gate's own terms (gate requires M5 only);
     flagged for the conductor as a release-hygiene curiosity (a future `/release` for
     real M6 will collide with the existing `v0.3.0-m6` tag name).
  2. **Citation re-verify (CCIR-1): all substantive citations MATCH; 4 trivial drifts.**
     MATCHES at cited lines: `check.rs:4357-4394` (UFCS arm; `sig_table.fns.get` :4358,
     first-param match :4360-4361, kernel guard :4384), `may_block.rs:1296` (MethodCall
     arm, no call-graph edge; arm body extends to :1324), `cpu_admission.rs:823-828`,
     `emit.rs:653-658`, `emit.rs:8433-8441`, `emit.rs:8276`/`emit.rs:8364` (twin
     walkers), `emit.rs:11651-11654` (raw `ptr_to_int` conduit token, no generation
     salt), `emit.rs:12356-12365` + `runtime.rs:297-299` (back-edge calls into no-op
     stub), `emit.rs:15120-15141` (unasserted non-SM `wait` fallback comment + arm),
     `emit.rs:11160-11163` (recursive-path no-fallback doc), `emit.rs:11833-11841`
     (closed1 "structurally unreachable" abort), `channel.rs:109-123`/`:120`
     (`pending_sends`), `channel.rs:270` (insert keyed by raw token), `channel.rs:311-339`
     (recv_poll: poll :323 / Ready-wake :331 / Pending-record :336 — two separate
     critical sections), `handle.rs:297-303` (panic→Pending), `handle.rs:326`
     (`handle_ptr as u64` token into the SAME `pending_sends` via `msg_chan`),
     `handle.rs:337-351` (`ynz_handle_free` calls `ynz_channel_free` :346, never purges
     pending_sends), `runtime.rs:316-354` (shutdown holds RUNTIME mutex across the
     :338 `shutdown_timeout` drain; "lock drops here" comment at :340 is after the
     drain), `runtime.rs:591-694` (drop ladder; kind-2 at :636-642 only
     `ynz_channel_free`; chain walk :659-680 frees child sleep handles only),
     `runtime.rs:921-925`/`:966-1008`/`:995-1006` (driver doc + lock-release-before-
     block_on pattern), `IMP-no-function-coloring.md:214-216`/`:247`,
     `IMP-concurrency.md:840` (Design Divergences), `check.rs:2930-2937` (bare-call
     kernel guard), auto-arc registry entries (`features.toml:1230`/`:1341`).
     DRIFTED (trivial, substance intact): `run.rs:75` → **:76** (`status.code()
     .unwrap_or(1)`); `Cg.type_params` → field is named **`Cg.type_subst`**
     (`emit.rs:1763-1765`); roadmap duplicate `## Capability Ledger` sections at
     **417 and 471** (plan says 365/417 — Phase 8 must use the new lines);
     `runtime.rs:296-299` no-op body is :297-299.
  3. **P1-2 verdict: DORMANT.** The twin walkers diverge only under a non-empty
     `Cg.type_subst` (`find_let_type_in_stmts` returns `cg.expr_type(value)`
     [`emit.rs:8368`] which applies `resolve_type`'s `Type::TypeParam` substitution
     [`emit.rs:1962-1968`]; `find_let_typeck_type_in_stmts` returns raw
     `typed.expr_types` [`emit.rs:8283-8285`]). SM-resume context with non-empty
     `type_subst` is structurally unreachable: (a) every SM classification site is
     gated `f.generics.is_empty() && suspend_set.contains(&f.name)` — frame-layout
     Step 1 `emit.rs:252`, Step 4 `emit.rs:308`, resume-fn declaration `emit.rs:1228`;
     non-generic lowering gate `emit.rs:1277`; (b) `lower_generic_function` constructs
     its Cg with `suspend_set: empty_suspend_set()`, `wait_cache: empty_wait_cache()`,
     `frame_layouts: empty_frame_layouts()`, `sm_frame_ptr: None` (`emit.rs:1490-1496`);
     (c) generic fns are excluded from the main sig table (`signatures.rs:132-135`) and
     `GenericFnSig` carries no `suspends` flag — generics cannot suspend today
     (`check.rs:4005-4020` documents the v0.4 deferral). Both frame-layout call sites
     (`emit.rs:4455` via `crossing_local_type_from_body`, cross-checked at :4458; slot
     computation at :4192/:8246 uses the typeck walker only) run exclusively inside the
     generics-gated SM path. Future-Requirements deferral #2 stands (four fields
     confirmed present).
  4. **P2-5 verdict: LIVE** — the assumed mutual-exclusion gate does NOT exist for a
     NESTED (branch-arm) CPU group. Evidence: (a) the cycle exclusion in
     `compute_cpu_promotions` (`queries.rs:900-917`) uses
     `find_mutual_suspension_cycles`, whose Kosaraju SCC pass emits only components
     with `component.len() >= 2` (`may_block.rs:1619-1621` region) — a SELF-recursive
     fn (self-loop, SCC size 1) is never excluded, and the comment at
     `queries.rs:901-903` says outright "self-recursion is allowed by the spawn ABI";
     (b) `admitted_cpu_group`'s nested arm declines only on `!f.params.is_empty()`
     (`cpu_admission.rs:157-159`) then delegates to `nested_group_member_path`
     (`cpu_admission.rs:632-663`), whose pre/post-group suspension gates
     (`cpu_admission.rs:508-534`) run ONLY within the nested block — a top-level
     self-recursive suspending call is never examined; (c) the v0.3-M3g Phase 3
     admission flip explicitly REMOVED the co-resident-suspension decline
     (`cpu_admission.rs:101-152`) — a host that also suspends elsewhere is admitted;
     (d) `FrameLayout` carries `recursion_slot` and `cpu_group_slots` as independent
     co-existing fields (`emit.rs:372-423`). Concrete LIVE shape: a zero-param
     self-recursive suspending fn (e.g. `wait sleep(...)` + statement-position `f()`
     self-call, both top-level) hosting a pure-CPU group inside an `if` arm is
     ADMITTED as a spike host AND gets a recursion slot; on cancellation,
     `SpawnStateFnFuture::Drop` runs `cleanup_spike_cpu_handles` on the ROOT frame
     only (`runtime.rs:607`), while the recursion-chain walk (`runtime.rs:659-680`)
     frees each heap-boxed child's sleep handle + frame but never its spike CPU
     handles — a chain child suspended at its CPU join leaks its boxed
     `CpuJoinHandle`s. (The TOP-LEVEL-group case is incidentally shielded: the
     pre/post-group deep-suspension gates at `cpu_admission.rs:508-534` see a
     top-level self-call and decline.) Verdict surfaced to the conductor for the
     deviation-judge → FRAGO seam per CCIR-4 — NOT fixed, NOT self-folded into a
     phase. Note: read-only phase — verdict is proven by code reading; a RED repro
     fixture belongs to whatever fix phase the FRAGO adds.
  5. **Dynamic-dispatch × suspension verdict: GAP at the predicate layer, currently
     UNREACHABLE (moot by construction TODAY).** (a) Typeck PERMITS a suspending fn to
     satisfy a `follows` contract: `check_follows_contracts` (`check.rs:5052-5136`)
     checks only first-param type (:5099-5100) and return type (:5102-5104) — it never
     reads `fn_sig.suspends`; (b) all four suspension predicates are MethodCall-blind
     (`may_block.rs:1296-1324` no edge; `cpu_admission.rs:823-828` recurse-only;
     `emit.rs:653-658` recurse-only; `emit.rs:8433-8441` `Call`+`Ident` only) — the
     vtable form shares P1-1's shape exactly; BUT (c) no `dynamic Contract` method
     call can reach a binary today: the single Dynamic-receiver dispatch arm in
     `lower_expr` hard-errors — "codegen: dynamic dispatch call sites not yet lowered
     in M4 P4" (`emit.rs:14622-14625`). The failure mode is a LOUD compile-time
     codegen error for every dynamic call site (suspending or not), never a silent
     mis-suspension. Surfaced to the conductor: whether this is "COVERED-enough"
     (record + Future-Requirements trigger on dynamic-dispatch lowering shipping) or a
     FRAGO adding predicate coverage to Phase 1's threading is the deviation-judge →
     FRAGO seam's call, not this executor's.
  Assumptions table updated (A1 verified; A3 verified-dormant; A4 FALSIFIED — LIVE).
  Phase 0 verdicts block added under the Phase 0 section in `plan.md`. No code
  touched; tree state unchanged (pre-existing dirty files `CLAUDE.md`,
  `crates/ynz-watch/src/{error,rebuild}.rs` remain known-not-mine, untouched).

- `executor-2026-07-09-m6-phase1b` — 2026-07-09 — Phase 1b segment 1 (fresh dispatch,
  checkpointed at the step 2→3 boundary on the context budget; `STATUS: PARTIAL`, resume-at
  `phase-1b/step-3`, handoff `handoff-phase-1b.md`). **Steps 1–2 DONE.** CCIR-1: all FRAGO-004
  citations verified live (`check.rs:8122/8242`; the shape_embed classification is at
  `crates/ynz-codegen/src/emit.rs:4504-4529` — crate-name drift only, no substance drift).
  RED repros locked (RED-before-fix): fixture (b) re-confirmed RED (10 runs → 10 distinct
  garbage values vs expected `10`); NEW `v0_3_m6_shape_arg_pure_call.ynz` (garbage vs `7`),
  `v0_3_m6_shape_arg_transitive_chain.ynz` (auto-inserted suspension + param passthrough;
  garbage vs `8`), `v0_3_m6_fixed_arg_suspending_call_rejected.ynz` (pre-fix wrongly compiles
  + runs, exit 0), all locked by `crates/ynz-driver/tests/v03_m6_shape_arg_frame_backing.rs`
  (4 tests incl. the N=10 determinism gate; 0 passed / 4 failed pre-fix, each for the
  documented UAF reason). Fix design settled + recorded in the handoff: extend the ONE
  classifier (`crossing_local_names_with_cpu_spike`, check.rs:7553) with an
  arg-escape-to-suspending-callee collector mirroring `collect_conduit_locals` — zero codegen
  changes expected. **Deviation surfaced (for deviation-judge):** plan text says fixed<T> is
  "frame-backed via shape_embed machinery"; no such machinery exists for fixed — the
  ESTABLISHED design (Check 2b UnsupportedCrossingLocalType) blocks non-embeddable aggregates
  crossing suspension with a teaching error, and the classifier extension routes fixed-arg
  escape into that same guard (UAF closed as a deterministic compile error, consistent with
  the read-after-wait twin). Residuals to probe post-fix (recorded in handoff, not
  self-expanded): anonymous aggregate args; loop-var-as-arg; number/maybe/union args (same
  by-pointer staging per `value_to_i64_bits`). Also observed: pre-existing backend ICE
  "cannot iterate fixed array with unknown size" on fixed<T> PARAM iteration (orthogonal).
  Tree green-building: workspace compiles, fmt clean; only reds are the locked planned-RED
  repros. Nothing staged/committed (conductor seals).

- `executor-2026-07-09-m6-phase1b-seg2` — 2026-07-09 — Phase 1b segment 2 (FRAGO 005
  application + step 3 + residual probes; `STATUS: PARTIAL`, EARLY checkpoint at the
  step-3/step-4 boundary on context budget — advanced past `phase-1b/step-3`, no stall).
  **First action: applied ratified FRAGO 005 to plan.md** (step 3/exit criteria: shape →
  `shape_embed` frame-backing, `fixed<T>` → existing Check 2b teaching error; step 4 gained the
  fixed<T> false-positive sweep; new step 6 residual-probes text; Future Requirements #11 —
  fixed<T> PARAM-iteration ICE, four-field) plus the plan-source-of-truth sibling sweep (Key
  Outcome 1b, Safety invariant, step 2 fixture wording — all corrected; remaining "fixed<T>"
  mentions verified accurate-history/signed-override text). Receipt delta check: HEAD `f921efe`
  (Phase 1 seal), no code moved since seg 1 — all receipts inherited. **Step 3 DONE:** extended
  the ONE classifier `crossing_local_names_with_cpu_spike` with
  `collect_aggregate_args_to_suspending_calls` (+ stmt/expr walkers + `mark_aggregate_arg`;
  check.rs, after `collect_conduit_locals`) — suspending `Call` via `is_suspending_call`, UFCS
  via the ONE `expr_is_ufcs_suspending_call` arm (receiver = arg 0), `Wait` unwrapped,
  `Background` not recursed, candidates = LET-bound Shape/BuiltinFixed idents (params + loop
  vars excluded). ZERO codegen changes — `crossing_local_names` delegates into the extended
  variant, so Check 2/2b and all codegen consumers thread the same set. **RED→GREEN:** locking
  suite `v03_m6_shape_arg_frame_backing` 4/4 GREEN (was 0/4), incl. the committed N=10
  determinism gate and the fixed<T>-rejected test; Phase-1 `v03_m6_ufcs_suspension` 4/4 GREEN —
  fixtures (a,b,c,d) ALL GREEN. **IR verified:** pure_call fixture post-fix shows
  `%ship_frame_region = getelementptr i8, ptr %0, i64 32` (heap-frame embed) + child staging
  `ptrtoint` of the frame-region pointer; the dying-stack-alloca staging is GONE. **Residual
  probes (FRAGO 005 step 6):** anonymous struct-lit arg **LIVE** (prints 4240380 vs 7);
  indexed arg NOT REACHABLE (maybe<Ship> typeck error); loop-var shape arg NOT LIVE (correct
  3,4 — heap array storage survives); `number` arg **LIVE** (prints 0.000... vs 2.5);
  maybe/union not probed decisively (parser friction) but share number's `value_to_i64_bits`
  staging arm — presumed same class. **Both LIVE verdicts SURFACED for the deviation-judge →
  FRAGO seam, NOT self-fixed** (R13 scoped shape+fixed only). Green-building: clippy
  `-D warnings` GREEN, fmt applied (only check.rs), no planned reds remain. Probe scratch in
  `target/probe_*.ynz`. Remaining: step 4 (full workspace suite + M3a/M4/M5 invariant re-read +
  fixed<T> false-positive sweep), step 5 formal record, close-out + handoff deletion. Resume at
  `phase-1b/step-4` via `handoff-phase-1b.md`. Nothing staged/committed (conductor seals).

- `executor-2026-07-09-m6-phase1b-seg3` — 2026-07-09 — Phase 1b segment 3 (FRAGO 006 application
  + fix-design recon; `STATUS: PARTIAL`, checkpoint at the FRAGO-application boundary on context
  budget — same boundary shape as seg 2's FRAGO 005 first-action). **First action: applied the
  SIGNED FRAGO 006 / R14 delta to plan.md in full** — Phase 1b scope grown (task/steps 2-6/exit
  criteria now cover `number`/`maybe`/`union` arg frame-backing via the `value_to_i64_bits`
  staging arm, `emit.rs:12323` — WORK across suspension, NOT rejected); **Phase 1c inserted**
  (anonymous-aggregate arg frame-backing, full spec: CCIR-1 / RED anon repro / anchoring fix /
  full-regression + N≥10 determinism / false-positive sweep, reviewer fan-out, Model tag
  `(coding, high, medium)`); Key Outcomes 1b grown + 1c added; Concept 12→13 phases; Coordinating
  Instructions sequencing P1→P1b→P1c→P2 + Phase 6b now after 1,1b,1c,3,3b,4,5 (both its
  Coordinating-Instructions line AND its own task text); R14 row + SIGNED override block recorded
  in Risk Assessment alongside R13 ("Two signed HIGH residuals" prose + Floor check updated);
  Safety invariants: Phase 1b invariant grown, Phase 1c invariant added. Whole-plan sibling sweep
  done (remaining "shape-arg"/R13-scoped mentions verified accurate-history). Mechanical
  amendment of a signed FRAGO — no Paper-Trace (not a bug response). **Receipt delta check:**
  HEAD still `f921efe`, seg-2 check.rs fix + 4 untracked test files intact, known-not-mine files
  untouched — all seg-1/2 receipts inherited, zero code moved. **Fix-design recon (new receipts,
  recorded in the handoff for the next segment):** (a) crossing-`number` machinery is a
  BITS-COPY, not a storage embed — i128 sm_entry alloca + 2 frame slots, flush/reload copies bits
  (`emit.rs:4455/4515-4516/4552/6022-6045`) — so classifier membership ALONE does not fix the
  number-arg escape (the staged `ptr_to_int` still targets the dying i128 stack alloca);
  consumer-side plumbing is confirmed REQUIRED, exactly as FRAGO 006's delta text says (staged
  pointer must target frame-resident bytes — shape_embed-analog region or staging-site copy);
  (b) **Check 2b currently HARD-REJECTS `maybe`/`union` crossing locals** ("frame-slot
  save/restore … not yet implemented", `check.rs:872-984`, blocked-categories match at
  `:919-933`) — widening the classifier's candidate arm alone would convert the maybe/union arg
  UAF into a Check 2b compile error, CONTRADICTING R14's signed "frame-back, NOT rejected"
  disposition; the fix must implement arg-escape frame-backing for maybe/union AND route those
  names past Check 2b's rejection while keeping ONE crossing classifier (design constraint
  recorded, not resolved); (c) maybe/union repro CONSTRUCTIBILITY is unresolved (seg-2 parser
  friction on `maybe<int>` params) — next segment probes constructibility first; if genuinely
  unconstructible from source, that verdict gets surfaced, not silently skipped. **Resume-at
  re-based to `phase-1b/step-2` (the number/maybe/union RED repros)** — NOT a stall: the signed
  FRAGO 006 re-opened steps 2–3's number/maybe/union halves in the amended plan (the plan's own
  step ledger changed this segment); the shape+fixed halves of steps 1–3 remain DONE. Advancement
  this segment = the FRAGO 006 amendment itself (plan.md materially rewritten per the signed
  ruling) + the fix-design receipts. Tree green-building: zero code changes this segment; seg-2
  gate state stands (locking suite 4/4, Phase 1 fixtures 4/4, clippy `-D warnings` green, fmt
  applied). Nothing staged/committed (conductor seals).

- `executor-2026-07-09-m6-phase1b-seg4` — 2026-07-09 — Phase 1b segment 4 (Part A maybe/union
  nature verdicts + step-2 number half; `STATUS: PARTIAL`, checkpoint at the step-2/step-3
  boundary on context budget). **Part A (verify-before-fix, probe-confirmed on the pre-fix
  tree):** (1) `maybe` arg-escape = **LIVE UAF, NOT moot** — constructible with fixture-proven
  forms (bare `m: maybe<int>` param, `` `42`.toInt() `` RHS, `.or(0)` read; seg-2's friction was
  positional, not structural), COMPILES (Check 2b never fires — the classifier misses arg-escape
  so the blocked-type match never sees the name), prints nondeterministic 13-15-digit pointer
  garbage vs correct 42 (4 distinct values across 4 runs). (2) `union` arg-escape = **LIVE UAF,
  NOT moot** — a Square arg deterministically prints `circle` 5/5 runs; non-suspending control
  prints `square` (suspension-caused). (3) **Feasibility: NEEDS-NEW-MACHINERY (fixed<T>/anon
  category), NOT number-like** — no Maybe/Union arm exists anywhere in the crossing strategy
  table (`emit.rs:4492-4556`, flush `:6022-6173`); Maybe needs envelope+payload ownership
  machinery; Union repr is non-uniform (tagged struct vs NULL for `T|nothing`, the documented
  `value_to_stable_bits` KNOWN-HOLE loud-fail-pinned by `m5_p3_sweep_union_readback_*`); plus the
  Check-2b routing constraint. **SURFACED TO THE DEVIATION-JUDGE → FRAGO SEAM, not self-decided:
  R14's signed "frame-back maybe/union in Phase 1b" disposition appears infeasible/
  disproportionate in-phase — conductor must re-route (candidates: fold into Phase 1c's
  new-anchoring work, or re-decide reject-vs-frame-back with Patrick).** (4) `number` re-confirmed
  LIVE (deterministic 0.000 vs 2.5). **Step 2 number half DONE:** RED repro locked —
  `crates/ynz-driver/tests/fixtures/v0_3_m6_number_arg_pure_call.ynz` + 2 tests (value + N=10
  determinism) in `v03_m6_shape_arg_frame_backing.rs`; confirmed RED for exactly the documented
  UAF reason (`0.000...` vs `2.5`). maybe/union repro-LOCKING deferred to the seam's disposition
  (repro shape differs: assert-value vs assert-compile-error). **Step-3 number fix design
  reconned and recorded in the handoff** (root-cause chain verified: staging loop
  `emit.rs:11103-11110` → `load()`'s Number COPY-to-fresh-stack-alloca `emit.rs:19851-19865` →
  `ptr_to_int` into child frame; design: widen `mark_aggregate_arg` to Number + stage a GEP into
  the parent frame's 2-slot decimal128 region keyed off `sm_crossing_decimal128_set` — ONE
  classifier drives both; heap-boxed twin `emit.rs:11263+` must be covered too). **Receipt delta
  check:** HEAD still `f921efe`; seg-2 check.rs fix intact; zero compiler-code changes this
  segment (fixture + tests + probes + planning docs only); known-not-mine files untouched.
  Green-building: workspace compiles; TWO PLANNED REDs (the locked number repro tests,
  RED-repro-before-fix per the plan's own step 2 — seg-1 shape-repro precedent), all other suites
  green per standing seg-2 receipts. Probe scratch: `target/probe_{maybe_arg,union_arg,
  union_arg_nosuspend,number_arg,number_control}.ynz` (gitignored). Resume at `phase-1b/step-3`
  (number half) via `handoff-phase-1b.md`; maybe/union halves BLOCKED on the seam. Nothing
  staged/committed (conductor seals).

- `conductor-2026-07-09-m6-exec` — 2026-07-09 — **PAUSE (user-requested; ~1h, low session tokens).**
  Phase 1b segment 5 (`executor-2026-07-09-m6-phase1b-seg5`) was **killed by the user mid-edit** while
  wiring the `number` frame-backing (`load()` GEP indirection into the parent decimal128 frame region).
  **Authoritative pause state (supersedes any ambiguity in the killed segment's own entry):**
  - **Committed / safe:** Phase 0 (`96bff95`) + Phase 1 9-site UFCS fix (`f921efe`). HEAD = `f921efe`.
  - **Uncommitted in the working tree, COMPILES clean** (`cargo check -p ynz-codegen -p ynz-typeck` green):
    Phase 1b's shape UAF fix (frame-backed, DONE) + fixed<T> (Check-2b compile-error, DONE) in
    `check.rs`/`emit.rs`; the 4 shape/fixed RED-repro fixtures + `v03_m6_shape_arg_frame_backing.rs`;
    the `number` RED repro (`v0_3_m6_number_arg_pure_call.ynz`); and the **PARTIAL `number` fix**
    (mark_aggregate_arg widened to Number + partial `load()` indirection — functionally INCOMPLETE, so
    the number RED repro is still RED; tree nonetheless compiles).
  - **Plan state:** FRAGO 007 **fully applied** — 13 phases; Phase 1c = anonymous-aggregate + maybe +
    union (new-machinery); R13 (shape) + R14 (wider class) signed overrides recorded; maybe/union
    re-homed to Phase 1c; Phase 1b narrowed to shape/fixed/number.
  - **Known-not-mine (never stage):** `CLAUDE.md`, `crates/ynz-watch/src/{error,rebuild}.rs`.
  - **RESUME INSTRUCTION (cold-resumable):** re-dispatch a fresh Phase-1b executor (fable) from
    `handoff-phase-1b.md`'s pointer `phase-1b/step-3` — **complete the `number` `load()` GEP indirection**
    (or `git checkout -- crates/ynz-typeck/src/check.rs crates/ynz-codegen/src/emit.rs` to drop BOTH the
    shape+fixed AND partial-number fixes back to `f921efe` and reapply cleanly — NOT recommended, loses
    the good shape+fixed work; prefer completing the number wiring). Then step 4 (full regression +
    false-positive sweep) + step 5 (N≥10 determinism) → **seal Phase 1b** (CONFIRM commit gate — needs
    Patrick's ok) on shape/fixed/number. Then Phase 1c (the three new-machinery cases). Executor model
    override still in force: **fable for all code**; reviewers/gates unchanged. Run mode: **CONFIRM**
    (no `--auto`) — no unattended commits.

- `executor-2026-07-09-m6-phase1b-seg7` — 2026-07-10 — **Phase 1b segment 7: number fix verified
  complete on disk; steps 3–5 + close-out done; Phase 1b SEALED on shape/fixed/number
  (`STATUS: COMPLETE`).** **Ground-truth correction to the dispatch premise (verify-before-fix):**
  the PAUSE note's "PARTIAL number fix — number RED repro still RED" was STALE relative to disk —
  the working tree already carried the COMPLETE number fix (a killed segment got further than the
  pause state credited: `sm_number_param_set` populated at param-alloca creation, the shared
  `stage_suspending_call_arg_bits` helper wired into BOTH the inline-poll (`emit.rs:11118-11121`)
  and heap-boxed (`emit.rs:11412-11415`) staging loops, and `load()`'s param indirection all
  present), and both number repro tests were GREEN on first run against a binary cargo confirmed
  up-to-date with the sources. Zero compiler-code changes were needed this segment — the segment's
  work is VERIFICATION + seal. **Step 3 exit (IR):** number fixture IR read end-to-end — parent
  flushes `x`'s i128 to frame slots (offsets 32/40), arg staging emits
  `getelementptr i8, ptr %frame, i64 32` + `ptrtoint` into the child slot (dangling fresh-alloca
  staging GONE; pre-fix pattern inherited from seg-4 receipts + probe-confirmed 0.000); child
  reloads the bits and reads the i128 through `inttoptr` AFTER its own suspend point
  (`sm_post_wait`). Slot math confirmed read from the authoritative `sm_crossing_slot_indices`
  (first-of-2-slots, `emit.rs:4219-4239`) — no re-derived twin. **Step 4:** Phase 1 fixtures 4/4
  GREEN (`v03_m6_ufcs_suspension.rs`, fixture (b) included); Phase 1b suite 6/6 GREEN; clippy
  `-D warnings` clean; `cargo fmt --all --check` clean; false-positive sweep clean via fresh probes
  `target/probe_fp_{number,fixed}_nonescaping.ynz` (gitignored): non-escaping number arg prints
  `5.0` and its IR keeps a plain `alloca i128` — zero frame flushes, zero crossing machinery;
  non-escaping fixed<T> arg compiles + runs clean (no Check 2b false fire). M3a/M4/M5 invariant
  re-read of the diff: additive crossing source into the ONE names vector; one shared staging rule
  (removes a would-be twin by unifying both loops); no frame-layout/flush/slot-size math changed.
  **Step 5:** determinism N=10 in-test for shape AND number, GREEN across 4 independent suite
  invocations this segment. **DEVIATION SURFACED (not self-decided):** full-workspace
  `cargo test --workspace` shows 521/522 with `v03_m3e_alias_local_name_collision_runs_correctly`
  failing 3/3 — isolated to LOAD INTERFERENCE, not regression: 9/9 isolated passes, 522/522 when
  the integration binary runs alone, AND full workspace goes green when only the 6 new Phase 1b
  tests are skipped (discriminating run) — the ~26 new concurrent `ynz` spawns push the M3e test's
  pre-existing fixed-300ms shutdown race (its own comment concedes prints may be lost at exit)
  past its margin. Orthogonality IR-proven: the fixture (all-int locals, literal args, no-arg
  background spawns) contains zero of the diff's instruction patterns. Routed to the
  deviation-judge → FRAGO seam; candidate remedy recorded in the plan note (deterministic
  completion for the M3e fixture). Test NOT weakened, NOT skipped in any committed artifact.
  **Close-out:** plan.md Phase 1b completion note added; session-id appended; R13 CLOSED, R14
  number portion CLOSED, R14 maybe/union + anon OPEN → Phase 1c; `handoff-phase-1b.md` DELETED as
  the final act (phase's final return). Also noted for reviewers (pre-existing, out of scope, not
  touched): the synchronous driver wrapper returns `ret ptr %ret_dec_slot` — a pointer to its own
  alloca (`emit.rs:5037` convention, present since before this phase). Known-not-mine files
  (`CLAUDE.md`, `crates/ynz-watch/**`) untouched. Nothing staged/committed (conductor seals at the
  CONFIRM gate).

- `conductor-2026-07-09-m6-exec` — 2026-07-09 — **SESSION-END HANDOFF (conductor context full →
  execution continues in a NEW chat via a fresh `/execute-plan` cold-resume).**
  - **Committed:** Phase 0 (`96bff95`) + Phase 1 9-site UFCS fix (`f921efe`). HEAD = `f921efe`.
  - **Phase 1b — WORK DONE + executor-verified green, but NOT yet boundary-reviewed and NOT committed.**
    Scope (FRAGO-007-narrowed): shape (frame-backed) / `fixed<T>` (Check-2b compile-error) / number
    (frame-backed). Executor gates all green (Phase 1 4/4, Phase 1b 6/6, integration 522/522, N=10
    determinism, false-positive sweep, clippy/fmt). R13 CLOSED; R14 number portion CLOSED. The
    segment-7 entry says "SEALED" meaning WORK-COMPLETE, **not committed** — no Phase-1b boundary commit
    exists, so cold-resume correctly treats it as ticked-but-uncommitted. **NEXT CHAT'S FIRST ACTION:**
    run the Phase 1b boundary review (cheap gates + reviewer fan-out over the uncommitted `ynz-typeck`/
    `ynz-codegen`/`ynz-driver` diff) → commit gate → seal. THEN Phase 1c.
  - **Phase 1c (next unsealed after 1b):** the three new-machinery UAF frame-backing fixes — anonymous-
    aggregate + `maybe` + `union` args across suspension (R14 open portion; FRAGOs 006/007) — PLUS the
    **M3e fixture-determinism fix as gating step-0** (FRAGO 008: a REAL sync primitive, not a bigger
    sleep). This is the signature-prone phase (will likely fire a mid-execution HIGH-residual signature
    gate, like 1b did twice).
  - **Known-not-mine (NEVER stage):** `CLAUDE.md`, `crates/ynz-watch/src/{error,rebuild}.rs`.
  - **EXECUTOR MODEL OVERRIDE (re-state in the next chat):** **fable for ALL code execution**;
    reviewers/gates/recon unchanged.
  - **Run mode this session:** CONFIRM. **`--auto` does NOT let the conductor self-sign risk** — it
    only removes the per-phase COMMIT confirmation (behind a gitleaks fail-closed guard); the
    risk-raising signature gate + completion gate stay human even unattended.
  - **CONDUCTOR OVERNIGHT RECOMMENDATION (Patrick to decide in the next chat):** run the mechanical/
    independent phases — Phase 2 (block_on guard), 3 (ABA purge), 4 (lost-wakeup), 6 (two mechanical
    fixes) — UNATTENDED via `--auto` (they're plan-scored LOW/MEDIUM, won't hit signature gates → they
    complete). Save **Phase 1c for ATTENDED time** (signature-prone). Reordering 1c after the mechanical
    phases is a risk-neutral sequencing FRAGO — 1c still lands before Phase 6b (the sanitizer lane must
    scan ALL the frame-backing fixes). 7 FRAGOs filed to date; 2 signed HIGH overrides (R13/R14).

- `conductor-2026-07-10-m6-exec2` — 2026-07-10 — **COLD-RESUME opened (fresh `/execute-plan --auto`,
  new chat).** Step-0 git-authoritative reconcile against ground truth (not prose):
  - **Phase 0 (`96bff95`) + Phase 1 (`f921efe`) → DONE/sealed.** Verified by full-body read + clean
    Phase-1-exclusive surface (`may_block.rs`/`cpu_admission.rs` not dirty). **Reconcile caveat
    (recorded, not fixed — committed history is out of charter to amend):** both prior boundary commits
    carry their `Plan-Phase:` trailer in a paragraph SEPARATED from `Co-Authored-By:` by a blank line,
    so git's `%(trailers:key=Plan-Phase,valueonly)` accessor returns EMPTY for them (the §8.1
    split-trailer bug). They ARE real boundary commits (subjects name the phases; `f921efe` body carries
    `Plan-Phase: 2026-07-04-v0-3-m6-concurrency-hotfix#1`). Consequence for the completion gate (§9.0.2):
    the whole-plan diff range cannot be derived from these two via the trailer accessor — range-start
    will be sourced manually from the known first M6 commit (`96bff95^`). MY commits from Phase 1b
    onward use §8.1-correct single-paragraph trailers, so they ARE accessor-legible.
  - **Phase 1b → isolated half-commit window** (ticked COMPLETE seg-7, surface dirty
    `check.rs`/`emit.rs` + 5 untracked test files, NO boundary commit, NO handoff file). Resume action:
    re-verify (cheap gates + reviewer fan-out over the uncommitted diff) → Step-8 seal. Staging sourced
    from `git status --porcelain` names minus known-not-mine.
  - **Known-not-mine (NEVER stage):** `CLAUDE.md`, `crates/ynz-watch/src/{error,rebuild}.rs` (surfaced
    by the Step-0 clean-tree preflight; carried forward from prior sessions).
  - **Remaining after 1b:** 1c → 2 → 3 → 3b → 4 → 5 → 6 → 6b → 7 → 8 (plan Concept sequence, run as-is;
    NO reorder FRAGO — 1c-before-2 is Decision D1's real dependency, and the handoff's "save 1c for
    attended" concern is dissolved by the delegated-signature mechanism below).
  - **DELEGATED-SIGNATURE MECHANISM (Patrick, this session, explicit — supersedes the standing
    "risk-raising signature gate stays human even under `--auto`" rule FOR THIS RUN ONLY).** Patrick is
    asleep and has delegated his signature to the conductor with a checks-and-balances harness: on ANY
    risk-raising FRAGO / HIGH-residual signature gate that would normally halt for a human, the conductor
    (1) makes the call, (2) documents it as a FRAGO block here with the accepted residual + reasoning,
    (3) dispatches an INDEPENDENT sonnet agent to adversarially review the self-signature for bias
    (is the override actually justified, or is the conductor rubber-stamping to keep the run moving?),
    and (4) proceeds ONLY on a clean bias-review verdict — a sonnet "NOT justified / biased" verdict
    HALTS for Patrick, never a self-override of the check. The sonnet bias-review is the separation-of-
    powers substitute for the human signature; the conductor never both signs AND clears its own sign.
    All golden rules + `no-duct-tape` enforced on every dispatched agent (executor model override:
    **fable for ALL code execution**; reviewers/gates/recon unchanged).

- `conductor-2026-07-10-m6-exec2` — 2026-07-10 — **USER-DIRECTED SCOPE ADDITION (Patrick, this session,
  interactive; Mission-level scope call — the user's authority, not a conductor/executor deviation).**
  Goal restated by Patrick: "as much concurrency work done the RIGHT way — no duct tape, no bugs."
  After walking the full deferral ledger, Patrick selected **"Full plan + P2-7 + P1-2"**:
  - **Un-defer P2-7** (`handle_recv_poll` panic-then-pending hang, Future Requirements #7 / R11) — a real
    concurrency hang, fix mirrors Phase 4's register-before-poll discipline. → new fix phase, sequenced
    right after Phase 4. To be filed as **FRAGO 009** (risk-neutral: un-defers a LOW bug; no signature).
  - **Un-defer P1-2** (twin type-walker unification, Future Requirements #2) — dormant, but the exact
    twin-derivation class that shipped silent miscompiles across M3a/M3d/M3e/M3g; unify behind one
    authoritative resolution. → new fix phase, sequenced before Phase 6b so the sanitizer lane scans the
    unified walker. To be filed as **FRAGO 010** (risk-neutral: dormant authoritative-derivation cleanup;
    no signature).
  - **Explicitly LEFT DEFERRED (conductor held the line, Patrick concurred — fixing these would ADD duct
    tape / scope-drift, violating the stated goal):** FRAGO 002 dynamic-dispatch × suspension (dead code —
    every `dynamic Contract` site hard-errors at codegen, no RED repro constructible), P2-3 closed-send
    (dead until channel-close ships), `background.cpuBound` (speculative unused override), the row-441
    int→number ICE + `fixed<T>` param-iter ICE (real crashes but ORTHOGONAL to the concurrency charter),
    channel-close semantics (a feature needing its own design pass), preemption back-edge (M7's optimizer).
    Each keeps its existing four-field Future-Requirements record.
  - **DURABLE-HOME REQUIREMENT (Patrick, this session — the deferrals must survive this plan's archival).**
    Patrick flagged that every LEFT-DEFERRED item currently lives ONLY in this plan's `plan.md` +
    `audit.md`, which `git mv` to `done/` at completion → the deferrals become invisible to future
    sessions. Durable home confirmed present: the roadmap `2026-05-21-v0-3-concurrency-perf/` (stays
    `active` until the whole v0.3 campaign finishes) carries a **Capability Ledger** ("SSOT for capability
    → milestone ownership", roadmap.md:417/471) + its own **audit.md**. → **FRAGO 011** amends **Phase 8**
    (already the roadmap-reconciliation phase) to LIFT every surviving deferral into that durable store:
    four-field WHAT/WHY/COST/TRIGGER payloads → roadmap `audit.md`; pointer rows → the roadmap Capability
    Ledger, owner-tagged (preemption→M7; channel-close + P2-3→M8; the two ICEs + dynamic-dispatch→`unscoped
    → needs a milestone`, row-441 flagged for Patrick's Gate-4 home call). Made a REVIEWED Phase-8
    deliverable (reviewer fan-out checks it landed), not a promise — §9.0.6-shaped, but pre-authored
    deferrals aren't auto-surfaced by the cumulative gate, so Phase 8 lifts them explicitly. Risk-neutral
    (docs/tracking reconciliation; no signature).
  - **Filing sequence:** FRAGOs 009/010/011 are filed by a re-dispatched executor (plan-body amendment: two
    new fix phases + Phase-8 durable-home deliverable + Concept/sequencing/Future-Requirements/risk-row
    updates) AFTER the Phase 1b boundary seal, to avoid concurrent `plan.md` writers. This note is the
    promptly-captured Mission-scope decision; the formal FRAGO 009/010/011 blocks below follow post-seal.

- `executor-2026-07-10-m6-phase1b-fixloop1` — 2026-07-10 — Phase 1b FIX-LOOP round 1
  (code-reviewer BLOCKER): the THIRD suspending-call arg-staging loop —
  `emit_io_member_init` (emit.rs ~9754, serving BOTH `emit_independent_group_poll` and
  `emit_fused_group_spawn_poll`) — still staged args via pre-fix `lower_expr` +
  `to_i64_bits`, leaving the FRAGO-006/007 number-arg UAF LIVE on the auto-parallelized
  I/O-group paths. **Sweep first (authoritative-derivation):** enumerated every
  `store_local_slot` caller + `to_i64_bits` site in emit.rs — exactly THREE child-frame
  arg-staging loops exist (inline-poll :11121, recursive heap-boxed :11415, io-group
  :9759); all three now route through the ONE `stage_suspending_call_arg_bits` helper;
  no fourth loop. Background-spawn arg staging (:15838/:16063 via
  `prepare_bg_arg_for_ctx`) and conduit send-value staging (:11809) are DIFFERENT
  lifetime classes (task/runtime outlives the parent frame — the frame-GEP helper would
  be WRONG there) and are NOT converted; both have potential ORTHOGONAL decimal128
  by-pointer exposure — surfaced to the seam for their own probe/FRAGO, not self-folded.
  **RED→GREEN (verify-first, both group callers):** independent group
  (`v0_3_m6_number_arg_parallel_group.ynz`) pre-fix printed
  `0.000000000000000000000000000000000000000000000` twice, 3/3 runs → post-fix
  `2.5`/`4.5`; fused group (`v0_3_m6_number_arg_fused_group.ynz`) via temp-revert repro
  printed `1226` + `0.000...` 3/3 → post-fix `1226`/`4.5`. Both locked with value tests
  + in-test N=10 determinism gates. **Batched hardening:** committed false-positive
  sweep fixture (`v0_3_m6_non_escaping_args_false_positive_sweep.ynz` — non-escaping
  number NOT wrongly frame-backed, non-escaping fixed<int> NOT Check-2b-rejected;
  previously gitignored-probe-only) + transitive-chain N=10 determinism gate.
  **Findings surfaced (not self-decided):** (1) the BASE read-after-suspension scan
  flags a fixed<T>/maybe local declared AFTER the function's only `wait` and read later
  (declaration-position-insensitive) — reproduced on the PRE-Phase-1b tree (HEAD
  f921efe, phase diff temporarily set aside), so pre-existing coarse-but-sound
  behavior, NOT a Phase 1b classifier regression; constrains false-positive-sweep
  fixture shapes (documented in the fixture). (2) Two adjacent non-suspending
  CPU-ABI calls form an M3d spike pair whose join IS a genuine suspension — a fixed<T>
  arg into a spiked member is a TRUE crossing (Check 2b correctly fires; verified while
  authoring the sweep fixture). **Gates:** workspace 2300 passed / 0 failed (the known
  M3e load-flake did NOT fire this run — its Phase 1c step-0 routing stands), clippy
  `-D warnings` clean, `cargo fmt --check` clean. Phase 1b test binary 12/12. No
  commit (conductor seals at the boundary gate). Scope guard held: maybe/union +
  non-Ident anon args untouched (Phase 1c); CLAUDE.md / ynz-watch dirty files untouched.

- `conductor-2026-07-10-m6-exec2` — 2026-07-10 — **Phase 1b BOUNDARY REVIEW + SEAL (cold-resume
  re-verify of the isolated half-commit).** Ran the full gate→review pipeline over the uncommitted
  Phase 1b work (Step-0 third-row re-verify path). **Round 1 fleet:** graveyard-auditor GREEN,
  green-check GREEN — BUT code-reviewer (opus) found a **BLOCKER**: a THIRD suspending-call
  arg-staging loop (`emit_io_member_init`, emit.rs, serving the auto-parallelized independent+fused
  I/O-group paths) was left staging a `number` arg via the pre-fix dying-stack path while the other
  two loops used `stage_suspending_call_arg_bits` — the exact FRAGO-006/007 number-arg UAF, LIVE on
  the parallelization path, untested (an authoritative-derivation.md parallel-derivation recurrence).
  acceptance-verifier met (1 should-fix: no committed false-positive fixture), rules-compliance clean,
  deviation-judge on-plan, test-quality clean (1 minor). **Fix-loop round 1** (executor
  `executor-2026-07-10-m6-phase1b-fixloop1`, fable): enumerated ALL staging loops (exactly three, no
  fourth — 4 independent sources concur), converted the third to the ONE helper (drop-in `frame_ptr`),
  RED→GREEN proven on BOTH parallel + fused callers (pre-fix `0.000` zeroed-freed-memory → `2.5`/`4.5`,
  N=10); batched the two quality items (committed false-positive-sweep fixture + transitive N=10 gate).
  **Round 2 fleet (full re-review, escalated tier):** green-check GREEN (12/12 1b tests, workspace
  0-failed, `secret-scanner: gitleaks`), graveyard-auditor GREEN (the two implicated corpses —
  Parallel-Per-Type-Dispatch + Authoritative-Output-Ignored — genuinely CLEARED), code-reviewer
  **Part 1 CLEAN** (fix correct, exactly 3 loops all converted, fixtures non-vacuously RED pre-fix),
  acceptance-verifier met, rules-compliance 0-blockers, deviation-judge on-plan, test-quality clean.
  **ZERO blockers → Phase 1b SEALED** on shape (frame-backed) / `fixed<T>` (Check-2b teaching error) /
  `number` (frame-backed) across ALL THREE staging loops (inline-poll, heap-boxed, io-member).
  - **MAJOR NEW FINDING — sibling decimal128-across-a-concurrency-boundary UAF class (code-reviewer
    Part 2, traced not asserted; expands the fix-loop executor's surfaced deviation).** Both surfaced
    sites judged **PLAUSIBLY-LIVE UAFs** with concrete repros: (A) **background-spawn decimal128 arg**
    — `prepare_bg_arg_for_ctx` (emit.rs:15386) has NO `Number` arm → falls to `_` pass-through
    (:15596-15601), no i128 heap-copy; CPU path (:15840, `ynz_rt_spawn_blocking`) derefs a
    stack-dangling pointer after the spawner returns; SM path (:16060) stores the dangling pointer into
    the heap frame and the spawned resume fn chases it via `sm_number_param_set` inttoptr — **and this
    phase's child-half fix makes that deref DETERMINISTIC (reliably into dead stack)**. (B)
    **conduit-send decimal128** (emit.rs:11809) — `ptr_to_int` of a stack temp sent as a raw i64 into
    `mpsc<i64>`; a receiver on another frame reconstructs a pointer into the sender's dead resume-fn
    stack. Both PRE-EXISTING latent defects, NOT M6 regressions; the fix-loop executor correctly
    DECLINED the frame-GEP helper there (wrong mechanism — spawn/send context outlives the parent
    frame; the real fix is an eager i128 heap-copy = hard-new-machinery, Phase-1c grade). (C)
    **Out-of-lane bonus** (code-reviewer, not graded): `emit_cpu_member_spawn` (emit.rs:9506-9509)
    `build_load(i64)` TRUNCATES a decimal128 local's i128 to its low 64 bits — a distinct wrong-VALUE
    bug (CPU members non-suspending). **DISPOSITION (deviation-judge: JUSTIFIED / risk-neutral FRAGO
    candidate):** did NOT block the 1b seal (out of 1b's arg-to-suspending-callee scope, surfaced+routed
    — same pattern as R13/R14 in Phase 1). NEXT: **empirically probe A/B/C** (verify-before-fix, exact
    repros supplied) → confirmed-live ones get a new fix phase via FRAGO 009 (risk-neutral). To be
    formalized as **FRAGO 009** in the post-seal amendment (rules-compliance flagged the not-yet-filed
    FRAGO as a should-fix on the conductor — correct; being addressed).
  - **Minor nits to tidy in the post-seal amendment (not blockers):** the stale "R14 number CLOSED"
    plan line (accurate now, written pre-fix-loop — re-affirm); the false-positive test's number-half
    WHY comment slightly overstates its causal mechanism (1-line clarification); no self-recursive
    heap-boxed repro (structurally covered by the shared helper — acceptable per §6.1).
  - **Seal commit:** explicit pathspec (check.rs, emit.rs, the 8 Phase-1b test files, plan.md, audit.md,
    _index.md); known-not-mine (CLAUDE.md, ynz-watch/{error,rebuild}.rs) excluded; `--auto` behind the
    gitleaks fail-closed guard; single-paragraph trailer (NOT the split-trailer bug the prior commits
    hit). Phase 1b checkbox ticked+committed atomic. **Post-seal order:** FRAGO 009 (probe→fix sibling
    UAF class) + FRAGOs 010/011/012 (P2-7, P1-2, durable-home) amendment → probe A/B/C → then Phase 1c.

- `executor-2026-07-10-m6-frago008-012` — 2026-07-10 — **Plan-amendment producer dispatch: applied
  FRAGOs 008/009/010/011/012 to `plan.md` (transcription only — no re-adjudication, no code, no
  commit).** Per the Numbering note above the FRAGO log (FRAGO 008 already existed; final numbering
  009 = sibling decimal128 spawn-boundary UAF, 010 = P2-7, 011 = P1-2, 012 = durable-home lift).
  Materialized:
  - **FRAGO 008** — added Phase 1c **step 0** (a new gating step, all subsequent steps unchanged in
    number, 1-5 stay 1-5 since 0 is new — no renumbering needed): replaces the M3e fixture's
    fixed-timeout shutdown-race margin (`v03_m3e_alias_local_name_collision_runs_correctly`,
    `integration.rs:2268-2308`) with a real synchronization primitive per the FRAGO's GUARDRAIL
    (remove the race, do not widen the sleep); Phase 1c's exit criteria + reviewer fan-out extended
    to cover it. **Self-correction, this same dispatch:** an earlier pass through this materialization
    note wrongly asserted the step was already present from a prior amendment; a grep against the
    live `plan.md` before returning showed it was NOT — corrected in the same dispatch before
    returning (verify-before-you-fix, applied to my own prior claim).
  - **FRAGO 009** — new **Phase 1d** inserted between Phase 1c and Phase 2 (the third
    hard-new-machinery decimal128 phase): closes background-spawn `number`-arg UAF (A) and
    cpu-member `number`-arg ICE (C); step 1 is written as the phase executor's own DESIGN DECISION
    (gate-consistent-reject vs. eager i128 heap-copy, both options + the `channel<number>` gate
    precedent + the Phase-1b `fixed<T>` precedent presented as inputs, NOT pre-decided here); item
    B (conduit-send number) recorded as new Future-Requirements deferral #12. Reviewer fan-out +
    Model tag `(coding, high, medium)`; Invariants block (Safety/Performance/Teaching/Runtime
    Dependencies/Kernel-Mode/Demo & Gallery/Feature Registry) extended with Phase 1d entries.
  - **FRAGO 010** — new **Phase 4b** inserted immediately after Phase 4: fixes P2-7
    (`handle_recv_poll` panic-then-pending hang) via the same register-before-poll discipline Phase
    4 applied; Future Requirements #7 updated from "deferred" to "un-deferred, FIXED"; R11 risk row
    adjusted in place (was scored as an accepted-LOW deferral; now scored as a mitigated,
    gate-passing fix). Reviewer fan-out + Model tag `(coding, standard, small)`; Invariants block
    extended.
  - **FRAGO 011** — new **Phase 5b** inserted immediately after Phase 5, before Phase 6b: unifies
    the twin type-walkers (`emit.rs:8276`/`:8364`) behind one authoritative resolution; Future
    Requirements #2 updated from "deferred, verify dormancy" to "un-deferred, FIXED in Phase 5b."
    Reviewer fan-out + Model tag `(coding, standard, small)`; Invariants block extended. New risk
    row R16 added.
  - **FRAGO 012** — Phase 8 extended with a new REVIEWED deliverable (step 4, renumbering the prior
    steps 4-5 to 5-6): lift every surviving Future-Requirements deferral (P2-3; channel-close;
    preemption; `background.cpuBound`; the two orthogonal ICEs [row 441 + `fixed<T>` param-iter];
    dynamic-dispatch #10; FRAGO-009's conduit-send-number #12) into the roadmap's durable store
    (`2026-05-21-v0-3-concurrency-perf/`) — four-field payloads → roadmap `audit.md`; pointer rows →
    roadmap Capability Ledger, owner-tagged. Phase 8's exit criteria + reviewer fan-out extended to
    require this landed, not merely attempted.
  - **Whole-plan consistency sweep performed** (per the dispatch brief — every sibling reference
    swept, not just the trigger citations): §3.2 Concept rewritten (phase count 13→16; full
    sequencing string `P0 → P1 → P1b → P1c → P1d → P2 → P3 → P3b → P4 → P4b → P5 → P5b → P6 → P6b →
    P7 → P8` stated explicitly); the stale `P1 → P1b → P1c → P2` fragment inside Phase 1's own task
    text corrected to include P1d; Phase 6b's own task+purpose sequencing prose AND the
    Coordinating-Instructions sequencing paragraph both updated (Phases 1d/4b/5b threaded into the
    Phase-6b hard-prerequisite list); Risk Assessment table (R11 adjusted in place for Phase 4b;
    R15 added for Phase 1d; R16 added for Phase 5b); Key Outcomes (new "1d." item; item 9 rewritten
    to drop P1-2/P2-7 from the deferred-list and state their Phase 5b/4b fix disposition; new item
    "11." for the FRAGO-012 durable-home lift); Future Requirements (#2 and #7 rewritten from
    "deferred" to "un-deferred, FIXED," retained under their original numbers per the P2-5/#3
    precedent so the audit-finding numbering still resolves; new #12 for FRAGO-009's item B).
  - **Also recorded (dispatch brief's two explicit asks):** the test-quality-flagged 1-line
    comment-clarification for `v03_m6_shape_arg_frame_backing.rs`'s false-positive test's
    number-half WHY comment is now noted directly in Phase 1c's plan text (fold in next time that
    phase, or Phase 1d, edits the test file). The "R14 number portion CLOSED" line (Phase 1b's seal
    note, written pre-fix-loop) is re-affirmed accurate post-fix-loop with an inline note pointing
    at the fix-loop + Round-2 boundary-review entries that actually re-verified it.
  - **Session-id appended to `plan.md` frontmatter in this same action.** No code touched; no
    commit made (rides into Phase 1c's boundary commit per the dispatch brief). Known-not-mine files
    (`CLAUDE.md`, `crates/ynz-watch/**`) untouched.

### `executor-2026-07-10-m6-phase1c-seg1` — 2026-07-10 — Phase 1c segment 1 (steps 0–1), PARTIAL checkpoint

- **Step 0 (FRAGO 008 gating) DONE — M3e fixture determinism.** Replaced
  `v0_3_m3e_alias_local_name_collision`'s fixed `sleep(300)` shutdown-race margin with a REAL
  synchronization primitive: a channel barrier (`channel<int>(3)`; each of the 3 spawned tasks
  `done.send(1)` after its print; the parent `done.receive()` ×3 BEFORE printing `main-done`).
  Race REMOVED (no timing margin anywhere), not widened — per the deviation-judge guardrail.
  Every ingredient verified constructible against `v0_3_m4_channel_composed.ynz` before writing.
  Local decoy keeps an identical signature (name collision, not arity, stays the dispatch
  diagnostic) and also sends (wrong dispatch terminates + fails, never hangs). Test assertion
  STRENGTHENED to exactly-3 IMPORTED-OK (was ≥1 with shutdown-timing tolerance). Verified: 10/10
  isolated + full `cargo test --workspace` under parallel load GREEN (integration 522/522,
  `CARGO_EXIT:0` captured inside the container — an earlier pipeline-exit read was rejected as a
  false-green trap and re-run).
- **Step 1 (CCIR-1) DONE.** All Phase-1c cited lines re-verified against the live tree
  (HEAD `47abd29` + step-0 edits); receipts with corrected drifted line numbers recorded in
  `handoff-phase-1c.md` (notably: `value_to_i64_bits` now `emit.rs:12401`, plan cited :12323).
- **PARTIAL checkpoint** at the step-1/step-2 boundary (context budget): resume at
  **`phase-1c/step-2`** (the "Lock the RED repros" step; positional ordinal 3 of 6). Handoff:
  `handoff-phase-1c.md` (receipts, settled design direction for steps 2–3, open recon items).
- **Deviations surfaced (for deviation-judge; NOT self-decided):** (1) Phase 1c carries no
  `**CHECKPOINT**` marks despite tripping REF-plan-format's >5-steps trigger — checkpointed under
  the conductor dispatch's explicit checkpoint authorization; marks not self-inserted. (2)
  Mechanism deviation flagged IN ADVANCE for steps 3(b)/(c): union frame-embed is structurally
  impossible under the current union ABI (none-case = NULL value pointer checked by
  `build_is_null`; a frame-region pointer is never null → breaks `is`-none semantics) — proposed
  sound mechanism is bind-time promotion to counted heap cells via the EXISTING
  `maybe_to_heap_cell` funnel + a new `union_to_heap_cell` (one uniform mechanism, aliasing
  preserved, default ptr flush/reload + the one staging helper unchanged); evidence in the
  handoff. Not yet implemented — surfaced for ratification before the next segment builds it.

### `executor-2026-07-10-m6-phase1c-seg2` — 2026-07-10 — Phase 1c segment 2: FRAGO 013 applied to plan.md (first action)

- **FRAGO 013 delta applied to Phase 1c's plan text** (ratified JUSTIFIED/RISK-NEUTRAL, conductor
  auto-apply, no signature — this plan's FRAGO-005/006/007 executor-applies pattern): (a) step 3
  rewritten — union/maybe frame-embed mechanism replaced with **bind-time promotion to counted
  heap cells** (reuse `maybe_to_heap_cell` `emit.rs:3077` + symmetric `union_to_heap_cell`; ONE
  uniform mechanism; route PAST Check 2b, not into it; default pointer-flush path + the single
  `stage_suspending_call_arg_bits` helper across all three staging loops UNCHANGED — no second
  derivation; loop leak-parity recon carried to build time under the phase's own gates, not
  assumed safe); (b) `**CHECKPOINT**` marks added under steps 1, 2, and 3 of Phase 1c's 6-step
  list (>5-step REF-plan-format trigger; marks are standalone lines, structurally disjoint from
  step numbering — segment 1's recorded `phase-1c/step-2` resume pointer unaffected).
- **Session-id appended to `plan.md` frontmatter in this same action.**
- **Step 2 (RED repros) DONE.** Three fixtures + six locking tests in
  `v03_m6_shape_arg_frame_backing.rs`, each confirmed RED pre-fix for the documented UAF reason,
  Paper-Traced: maybe → nondeterministic 15-digit pointer garbage vs 42 (3 distinct values / 3
  runs); union → deterministic wrong variant `circle` 3/3 vs `square`; anon → `4240380` 3/3 vs 7
  (matches the plan's probe value). Suite: 12 pre-existing GREEN / 6 new RED. The tracked
  Phase-1b test-quality minor (false-positive test's number-half WHY comment) folded in.
- **PARTIAL checkpoint** at the step-2/step-3 boundary — the planned `**CHECKPOINT**` mark FRAGO
  013 added (context budget past threshold): resume at **`phase-1c/step-3`** (positional ordinal
  4 of 6). Handoff `handoff-phase-1c.md` rewritten in place: step-3 build recipe settled from
  live-code reads (store_binding funnel `emit.rs:19836` = maybe promotion point; union-ctor Let
  arm :12641-12691 clobbers the sm_entry crossing alloca via a fresh `outer_slot` — the key
  gotcha; strategy table needs ZERO edits — Maybe/Union already default to the ptr-alloca +
  default-pointer-flush strategy; heap-cell ownership doc :3243-3246 = never-drop-locals design,
  loop-leak verdict still to be RECORDED under step 3's parity gate, not assumed). Check 2b
  provenance design settled (provenance-returning core in the ONE producer; skip Maybe/Union
  rejection only for arg-escape-only names; fixed<T> + read-after-wait rejections byte-identical).
  Tree green-building: fmt clean, this phase's files clippy-clean, only documented RED (the 6
  RED-repro locks).

### `executor-2026-07-10-m6-phase1c-seg4` — 2026-07-10 — Phase 1c segment 4: FRAGO 014 applied to plan.md (first action)

- **FRAGO 014 delta applied to Phase 1c's plan text** (ratified JUSTIFIED; risk-neutral AFTER the
  deviation-judge's mandated boundary correction — Union deferred entirely to 3c; conductor
  auto-apply, no signature — same executor-applies pattern as FRAGOs 005/006/007/013): (a) step 3
  replaced with the CORRECTED 4-part sub-step split — 3a typeck routing Maybe + anon-StructLit
  ONLY / 3b codegen maybe + anon / 3c codegen union incl. its OWN classifier-widen + the
  emit.rs:4493 annotation-aware override (via `ast_type_to_typeck_type`, one resolution, no twin)
  / 3d parity verdict + IR proof — with `**CHECKPOINT**` marks at each sub-step boundary (each
  boundary green-building by design); (b) ITEM 2: Check 2 (nested-shape, `check.rs:817-870`)
  named alongside Check 2b throughout step 3's "route past" wording, PLUS the sibling occurrences
  of the same fact swept per plan-source-of-truth (the task+purpose bold Check-2b obligation and
  the exit-criteria "routed PAST" line — both now name Check 2 AND Check 2b).
- **Resume-at pointer repaired to `phase-1c/step-3a`** in `handoff-phase-1c.md` (canonical over
  the post-FRAGO sub-step list, per REF-plan-format's FRAGO pointer-repair rule).
- **Session-id appended to `plan.md` frontmatter in this same action.**
- **Step 3a (typeck routing, Maybe + anon-StructLit ONLY) DONE.** All edits in `ynz-typeck`, zero
  codegen edits: `mark_aggregate_arg` widened with `Type::Maybe` (NOT Union — deferred to 3c per
  FRAGO 014, doc comment records why); `crossing_local_names_with_cpu_spike` refactored into a
  provenance-returning core `crossing_local_names_with_provenance` → `CrossingNames { names,
  arg_escape_only }` (snapshot window around the arg-escape collector — by-construction split, one
  producer, no twin scan; `with_cpu_spike` is a thin `.names` wrapper, byte-identical output);
  Check 2b skips rejection iff arg-escape-only ∧ effective type is Maybe (`check.rs:953`); Check 2
  needed no 3a edit (Maybe never matches its Shape arm — Union skip lands in 3c per Decision 13);
  **the M3d decline-to-promote probe (`suspension_guards_fire_for_fn`) mirrored with the same
  provenance + skip** — without it the probe would decline CPU promotion for hosts the checker now
  accepts (silent-envelope-narrowing class; recorded as handoff Decision 18 — 3c must mirror its
  Union skip there too).
- **3a exit criteria ALL verified:** workspace builds; maybe repro compiles exit-0 + nondeterministic
  pointer garbage across runs (still UAF-RED, per plan); anon repro unchanged (compiles, `4240380`
  vs 7; no typeck surface — handoff Decision 17: 3a's "anon" is a scope label, the anon fix is
  entirely 3b's codegen staging arm); union repro FULLY RED with the byte-identical pre-fix tell
  (`circle`, NOT misclassified, NOT an ICE); fixed<T> + both m3a read-after-wait rejection tests
  GREEN; positive widen evidence via IR (`m` frame slot + inttoptr reload in
  `target/ir-check/v0_3_m6_maybe_arg_pure_call.ll`). Gates: integration **522/522**; ynz-typeck
  suites green; frame-backing suite 12 GREEN / 6 documented-RED; fmt clean; clippy `-p ynz-typeck
  -D warnings` clean.
- **PARTIAL checkpoint** at the plan's 3a `**CHECKPOINT**` mark (context budget past threshold):
  resume at **`phase-1c/step-3b`**. Handoff `handoff-phase-1c.md` rewritten in place (segment-4
  anchors, receipts deltas, Decisions 17–18).

### `executor-2026-07-10-m6-frago015` — 2026-07-10 — FRAGO 015 applied to plan.md (tracking/docs reconciliation, no code logic change)

- **FRAGO 015 delta applied to `plan.md`** (ratified JUSTIFIED/RISK-NEUTRAL, conductor auto-apply,
  no signature — transcribed verbatim from the authoritative delta already recorded at `### FRAGO
  015` below, not re-adjudicated): (a) added the Phase 1c per-iteration maybe/union heap-cell LOOP
  leak as new numbered entry **#13** in `## Future Requirements / Revisit`, four fields verbatim
  from the Phase 1c completion note (WHAT/WHY/COST/TRIGGER, pinned via
  `v03_m6_p1c_heap_cell_loop_parity_pins_documented_per_iteration_leak`); (b) amended Key Outcome
  #11 AND Phase 8 step 4's FRAGO-012 durable-home lift-list (both enumeration sites swept per
  plan-source-of-truth's sibling-sweep discipline) to include deferral #13, owner-tagged
  `unscoped → needs the drop-story milestone`; (c) comment-accuracy fix in
  `crates/ynz-typeck/src/check.rs` (~line 826-844, the maybe/union Check-2b arg-escape-only skip
  comment) crediting `store_field`'s (`emit.rs:20154`, v0.3-M5 P2) per-shape-field heap-celling as
  the real reason a nested-shape variant payload is safe under the flat one-level ABI-size
  promotion memcpy — refutes a stale pre-M5 code-review premise (inline stack sub-structs) a
  Phase 1c verification probe already disproved; comment-only, zero logic change (`cargo check -p
  ynz-typeck` reconfirmed clean; Phase 1c's own 2307/0 full-workspace green bill is unaffected).
- **Session-id appended to `plan.md` frontmatter in this same action.**
- No new deviation surfaced; this dispatch transcribes an already-adjudicated delta per the
  dispatching brief's explicit instruction, not a fresh classification.

## FRAGO log

### FRAGO 001 — 2026-07-09 — session-id: `conductor-2026-07-09-m6-exec`

- **Trigger.** Phase 0 dormancy verification falsified assumption **A4**: P2-5 is **LIVE**,
  not dormant. No mutual-exclusion gate exists for a **nested branch-arm** CPU-parallel group
  in a zero-param self-recursive suspending host — the SCC exclusion in
  `find_mutual_suspension_cycles` only drops components of `len() >= 2` (self-loops survive;
  `may_block.rs:1617`), nested admission is block-local (`cpu_admission.rs:508-534` scans only
  the branch arm), and v0.3-M3g Phase 3 removed the co-resident-suspension decline. On
  cancellation, `SpawnStateFnFuture::drop` cleans spike CPU handles on the ROOT frame only
  (`runtime.rs:607`); the recursion-chain walk (`runtime.rs:659-680`) frees each child's sleep
  handle + frame but never its `CpuJoinHandle`s — a chain child suspended at its CPU join leaks.
- **Corroboration.** Independently CONFIRMED by the adversarial gate-checker (code-reviewer,
  agent `ad449ef123ad5f5c5`): every link in the evidence chain holds against the live tree; it
  also surfaced that `queries.rs:942-943` carries a **stale comment** asserting structural
  inertness via the exact Phase-3-removed decline — the artifact that seeded the wrong A4.
- **Classification.** deviation-judge (agent `af475454570475d5c`): **JUSTIFIED / RISK-NEUTRAL.**
  In M6's charter (a confirmed concurrency resource-cleanup-on-cancellation leak — same class as
  P3-1/P2-4). Exactly the outcome risk row **R9** pre-scored (C×III → MEDIUM residual, "trigger:
  Phase 0 verdict itself") and Future-Requirements #3 pre-shaped (~1 session, "extend cleanup to
  recursion-chain children"). No Floor-B trigger; pre-v1.0, zero-users, git-reversible → no
  signed RISK OVERRIDE required (CCIR-5 not tripped).
- **Delta (applied to `plan.md` by re-dispatched executor, per charter — conductor logs, does
  not hand-edit the body).** Insert **Phase 3b — P2-5: recursion-chain × spike CPU-handle
  cleanup leak**, sequenced immediately after Phase 3 (same `runtime.rs:591-693` drop-ladder
  region, minimizes merge collision), NOT folded into Phase 3 (per Phase-0 Step-6 pre-written
  routing + to keep Phase 3's ABA/orphan acceptance gate un-blurred). Phase 3b carries its own
  RED-repro-before-fix (verify-before-you-fix), extends the recursion-chain drop walk to free
  child spike CPU handles via the same root-frame `cleanup_spike_cpu_handles` path
  (authoritative-derivation: one cleanup choke point), AND corrects the stale
  `queries.rs:942-943` comment (docs-must-not-lie, M6 charter). Future-Requirements #3 updated
  from "verify dormancy" to "confirmed LIVE — fixed in Phase 3b." Assumption A4 marked FALSIFIED.
- **Authority.** deviation-judge classified JUSTIFIED/RISK-NEUTRAL; conductor
  (`conductor-2026-07-09-m6-exec`) ratified auto-apply per Step-7 risk-neutral flow (no signature
  gate). Applied to `plan.md` by executor `executor-2026-07-09-m6-phase0b-frago` (re-dispatch).

### FRAGO 002 — 2026-07-09 — session-id: `conductor-2026-07-09-m6-exec`

- **Trigger.** Phase 0's dynamic-dispatch × suspension coverage check found a **GAP** at the
  predicate layer (typeck's `check_follows_contracts` never reads `suspends`; all four suspension
  predicates are MethodCall-blind — the exact shape as P1-1's UFCS gap) that is simultaneously
  **provably UNREACHABLE today**: every `dynamic Contract` call site hard-errors unconditionally
  at codegen (`emit.rs:14622-14625`, "not yet lowered in M4 P4").
- **Corroboration.** CONFIRMED by the adversarial gate-checker (code-reviewer): the hard-error is
  universal for the dispatch form, the emitted vtables are dead/unused, and an existing test
  (`m2_state_machine_integration.rs:757-768`) proves a non-suspending dynamic-dispatch relay
  reaches the codegen error, never a silent sync lowering. A second typeck gate
  (`check.rs:4317-4345`) catches suspending callers even earlier.
- **Classification.** deviation-judge: **JUSTIFIED / RISK-NEUTRAL.** Routing it to the seam
  rather than mechanically self-applying Phase-0 Step-6's binary "GAP → FRAGO Phase 1" was
  correct — the state is hybrid (real gap, provably moot). On the merits: **deferral-with-trigger,
  NOT FRAGO-now**, on the direct in-plan precedent of Decision **D4** (P2-3 stays deferred as
  unreachable-until-a-feature-ships dead code — the YAGNI ceiling). Zero live exposure window
  (loud compile error, never silent mis-suspension), so no cheap-mitigation-now question arises.
- **Delta (applied to `plan.md` by re-dispatched executor).** Add **Future Requirements #10 —
  dynamic-dispatch × suspension predicate blindness**, four-field: WHAT (typeck permits a
  suspending fn to satisfy `follows`; all four predicates MethodCall-blind for the vtable form);
  WHY deferred (every `dynamic Contract` call site hard-errors at codegen today — no reachable
  test, fixing now is speculative work against dead code, same YAGNI shape as D4/P2-3); COST
  (small — reuses Phase 1's shared authoritative-resolution threading; should land in the same
  future phase that lowers dynamic-dispatch codegen, not as separate follow-on); TRIGGER
  (`dynamic Contract` call-site codegen lowering ships — the remaining M4 P4 work; owning
  milestone TBD, flagged to Patrick at Gate-4 rather than named "someday").
- **Authority.** deviation-judge classified JUSTIFIED/RISK-NEUTRAL; conductor ratified auto-apply
  per Step-7 risk-neutral flow (no signature gate). Applied to `plan.md` by executor
  `executor-2026-07-09-m6-phase0b-frago` (re-dispatch).

- `executor-2026-07-09-m6-phase1` — 2026-07-09 — Phase 1 segment 1 (steps 1–2 of 8;
  `STATUS: PARTIAL` at the plan's first CHECKPOINT mark). Step 1: CCIR-1 re-verified all
  Phase-1 citations against the live tree — ALL MATCH (`check.rs:4357-4394` UFCS arm /
  `:4358` `sig_table.fns.get` / `:4360-4361` first-param match / `:4384` kernel guard;
  `may_block.rs:1296` MethodCall arm edge-less; `cpu_admission.rs:823-828`;
  `emit.rs:653-658`; `emit.rs:8433-8441`). Threading design settled on the house
  precedent (`suspension_source.rs` sibling arm + consumer-supplied receiver knowledge,
  mirroring `channel_method_suspends`) — full decision record in `handoff-phase-1.md`.
  Step 2: RED fixture class authored
  (`crates/ynz-driver/tests/fixtures/v0_3_m6_ufcs_{transitive,explicit_wait,mixed_subexpr,background}.ynz`
  + `crates/ynz-driver/tests/v03_m6_ufcs_suspension.rs`) and CONFIRMED failing for the
  documented P1-1 reason: (a)/(b)/(d) panic at `runtime.rs:982` "Cannot start a runtime
  from within a runtime" (the sync block_on wrapper reached on a runtime thread, then
  abort); (c) the Call-leg `fetchBase` subexpr teaching error fires while the UFCS-leg
  `crew` error is absent (MethodCall-blind walker). Pre-fix mode toward step 8:
  **PANIC-then-abort, not silent block** (evidence from this pre-fix RED run). DEVIATION
  surfaced (plan-said-4-sites / reality-has-more, same seam): `count_suspension_expr`
  (`emit.rs:5149`), `emit_suspending_call_inline_poll` (`emit.rs:10921` Call-only
  destructure), `callee_name_from_call_expr` (`emit.rs:7671`),
  `emit_suspending_call_heap_boxed`, `lower_expr_background` (`emit.rs:15569`), and
  `suspending_calls_in_subexpr_position` (`check.rs:704`) are also MethodCall-blind and
  sit on the same UFCS path — details in `handoff-phase-1.md`. Tree state: green-building
  (workspace clippy gate green, fmt clean); only the 4 planned RED tests fail, by design,
  per the CHECKPOINT mark. No git commits (conductor seals the phase). Resume at
  `phase-1/step-3` via `handoff-phase-1.md`.

- `executor-2026-07-09-m6-phase1-seg2` — 2026-07-09 — Phase 1 segment 2 (FRAGO 003
  application + step 3 of 8; `STATUS: PARTIAL`, EARLY checkpoint at the step-3/step-4
  boundary on context budget — not a plan mark). First action: applied FRAGO 003's delta to
  `plan.md` (task line, Key Outcome #1, exit criteria → full 9-site enumeration) PLUS the
  plan-source-of-truth sibling sweep of the same forward-looking fact (step-6 CHECKPOINT
  line and the Invariants "RED→GREEN fixture class" line, "all 4"→"all 9"); historical "4
  sites" mentions (§1 Situation, R1 risk row, Phase-0 text) left as accurate history.
  Receipt-inheritance delta check: HEAD moved `46bab6d`→`96bff95` (Phase-0 seal), verified
  planning-docs-only — all seg-1 receipts stand. Step 3: `may_block.rs`
  `collect_calls_in_expr` MethodCall arm now adds the UFCS call-graph edge mirroring the
  Call arm (local_fns shadowing-first → direct edge; imported∧suspending → intrinsic flag;
  `!is_background_call` gated; unresolved method names set no flag; name-keyed by design —
  fixpoint runs pre-typing). Green-tree evidence: `cargo test -p ynz-typeck` 28/28 binaries
  green; house clippy gate (`--workspace -- -D warnings`) green; fmt applied. RED fixture
  class remains the planned documented RED (sites 2, 4–9 pending). New deviation CANDIDATE
  recorded for empirical probing next segment (NOT self-adjudicated, outside FRAGO 003's
  ratified set): typeck's crossing/SM classifiers (`check.rs:8275-8308` `this_stmt_suspends`
  + siblings) are Call/conduit-only for BARE UFCS suspending statements — probe spec +
  site-by-site threading receipts in `handoff-phase-1.md`. Resume at `phase-1/step-4`.

- `executor-2026-07-09-m6-phase1-seg3` — 2026-07-09 — Phase 1 segment 3 (step 4 of 8;
  `STATUS: PARTIAL`, EARLY checkpoint at the step-4/step-5 boundary on context budget — not a
  plan mark). Materialized the two settled helpers WITH their first consumer (Decision 2):
  `suspension_source::ufcs_method_call_suspends` (the ONE authoritative UFCS classifier arm)
  + `check::expr_is_ufcs_suspending_call` (span-keyed AST helper, exported via `lib.rs`).
  Step 4: `cpu_admission.rs` `expr_contains_suspending_call` MethodCall arm now classifies
  UFCS suspending calls via the shared helper; `expr_types` threaded through the whole
  documented fan-out (`stmt_contains_suspending_call_deep` + 7 admission entry points; new
  `ExprTypes` alias), `inlay_hint_passes.rs` ×3 call sites (pass
  `check_out.typed_module.expr_types`), `emit.rs` delegating wrappers
  (`stmt_contains_suspending_call`/`stmt_needs_sm_walker`, `spike_*`,
  `count_cpu_groups_all_depths`, `stmt_contains_cpu_group`, `stmt_block_has_direct_cpu_group`,
  result-names/extract inner fns — all pass `typed.expr_types`/`cg.typed.expr_types`), emit
  unit tests (`no_types()` empty map — Call-only fixtures, exact) and
  `tests/golden.rs`/`tests/parallel_group_hint_parity.rs` (real `check_out` expr_types).
  Codegen SM routing free-rides per the seg-2 receipt (one shared definition). Green-tree
  evidence: `cargo check --workspace --all-targets` clean; `cargo test -p ynz-typeck` green
  (incl. parity suite); `cargo test -p ynz-codegen --lib` 13/13; house clippy gate green; fmt
  applied. RED fixture class remains the planned documented RED (sites 3–9 pending).
  Bare-UFCS-crossing probe (Decision 3): recorded **DEFERRED-CONFOUNDED (pre-threading)** —
  pre-fix the probe program routes through the unfixed sites to the sync wrapper and panics at
  `runtime.rs:982` for the already-documented P1-1 reason (seg-1 fixture (a) evidence), so it
  cannot discriminate LIVE/DORMANT for the crossing classifiers until sites 4–9 are threaded;
  the decisive run stays scheduled post-threading per the handoff spec. NOT self-adjudicated;
  still outside FRAGO 003's ratified set. Resume at `phase-1/step-5`.

- `executor-2026-07-09-m6-phase1-seg4` — 2026-07-09 — Phase 1 segment 4 (steps 5–6 of 8 done;
  step 8 recorded; step 7 partial; `STATUS: PARTIAL`, EARLY checkpoint at the step-6/step-7
  boundary on context budget + a surfaced deviation gating step 7). **Steps 5–6:** all 9 FRAGO-003
  sites now thread the ONE authoritative resolution — site 3 `collect_callees_in_expr` family
  (expr_types threaded; UFCS callees embed sub-frames); site 4 `is_direct_suspending_call`
  (+ its 5 SM-walker arm call sites, conduit arm still first); site 5 `count_suspension_expr`
  (conduit OR UFCS `own`); sites 6/8 `emit_suspending_call_inline_poll` (Call|MethodCall
  extraction, receiver = arg 0) + `_heap_boxed` (`args: &[&Expr]`); site 7
  `callee_name_from_call_expr` (MethodCall → method); site 8 `synthesize_ufcs_call_expr`
  normalization at `lower_expr_background` + `lower_let_background_handle` entries; site 9
  subexpr walker (expr_types threaded, MethodCall violation arm, six UFCS direct-statement Safe
  arms, check 3 reordered AFTER check_stmts — FRAGO 003's diagnostic-ordering should-fix
  satisfied: mixed fixture asserts both teaching errors present and passes; no ordering-sensitive
  suite failure). **Grep gate PASS** — `grep -rn "ufcs_method_call_suspends" crates/` → one
  classifier (suspension_source.rs:134) + one AST wrapper (check.rs
  `expr_is_ufcs_suspending_call`); consumers = check.rs/cpu_admission.rs/emit.rs/lib.rs;
  negative sweep
  (`grep -rn "contains(method\|suspends(method\|contains(&method\|suspends(&method" crates/`)
  → only `may_block.rs:1315-1319` (the ratified name-keyed fixpoint edge PRODUCER, pre-typing by
  design), all other hits non-suspend (soa intrinsics, ownership hints, EC methods, registry
  parity, span math). **RED class: 3/4 GREEN** ((a) transitive 11/exit-0, (c) both-legs teaching
  errors, (d) background Mon/done). **(b) explicit-wait stays RED — NEW surfaced deviation
  (outside FRAGO 003's set, for the deviation-judge → FRAGO seam, NOT self-fixed):** shape-value
  arg to a suspending callee is staged in the parent resume fn's STACK alloca (`%ship_own`), the
  child frame stores `ptrtoint` of it, parent returns Pending → stack dies → child resumes on a
  dangling `self` → garbage cargo (nondeterministic address values). IR-proven
  (target/tmp_v_interp.ll: parent frame 72B = 32 header + 40 child, ZERO parent local slots);
  reproduces in PURE CALL form (`const c = wait crew(ship)`) → pre-existing latent gap (no
  9-site change touches Call staging/crossing/layout; no pre-existing fixture passes a shape to
  a suspending callee), exposed now because UFCS is the natural shape-passing form. Fixture
  (a) + the crossing probe pass by dangling-but-intact luck (same mechanism, receiver side).
  Root-cause locus: crossing/frame-layout classification ("read AT the suspending statement" =
  non-crossing; wrong for stack-backed shape aggregates whose pointer crosses into the child
  frame). **Probe verdicts (both owed probes run):** bare-UFCS-crossing (Decision 3) —
  LOCAL-crossing half RESOLVED-BY-THREADING (probe prints crossing/10; `msg` frame-backed,
  104B frame), receiver half = the surfaced deviation; name-collision (Decision 1) — SOUND
  (typeck dispatches `ship.copy()` to the builtin — probe A loud teaching error proves
  `dup: Ship`; probe B `${dup.cargo}` prints 10 exit 0 — no silent misroute). **Step 8 RECORDED
  (closes the audit TBD):** pre-fix mode = PANIC-then-abort at `runtime.rs:982` ("Cannot start
  a runtime from within a runtime"), evidence seg-1's pre-fix RED run — not a silent block.
  **R1 so far: zero pre-existing regression** — full-suite failures = `cross_impl_consistency`
  ×2, both 100% fixture (b) (435/436 + 452/453 other corpus files pass); house clippy GREEN,
  fmt applied, `cargo check --workspace --all-targets` clean. Resume at `phase-1/step-7` via
  `handoff-phase-1.md` (step 7 gated on the seam ruling for the surfaced deviation).

### FRAGO 003 — 2026-07-09 — session-id: `conductor-2026-07-09-m6-exec`

- **Trigger.** Phase 1 segment 1 (executor `executor-2026-07-09-m6-phase1`) found, mid-fix, that
  the plan's enumerated "**4 broken predicate sites**" UNDER-COUNTS the UFCS-suspension seam: ~5
  more MethodCall-blind sites must ALSO thread the one authoritative resolution or Phase 1's exit
  criteria (transitive / explicit / mixed / background `wait x.method()` actually suspends) cannot
  be met — two of the plan's OWN pre-authored RED fixtures ((c) mixed-subexpr, (d) background)
  route through `suspending_calls_in_subexpr_position` (`check.rs:704`) and `lower_expr_background`
  (`emit.rs:15569`), both MethodCall-blind. The additional sites: `count_suspension_expr`
  (`emit.rs:5117+`), `emit_suspending_call_inline_poll`/`_heap_boxed` (`emit.rs:10906+`),
  `callee_name_from_call_expr` (`emit.rs:7671`), `lower_expr_background` (+ SM variant :15801, +
  `Stmt::Let` background path :12431), and `suspending_calls_in_subexpr_position` (reorder to run
  after `check_stmts` so it stays type-aware — mirrors existing checks 1a-1c).
- **Classification.** deviation-judge (agent `a702c3a1287f83e8f`): **JUSTIFIED / RISK-NEUTRAL.**
  The plan under-enumerated the very seam its own purpose clause + exit criteria + RED fixtures
  already committed to closing; all ~9 sites stay in the already-declared crates
  (`ynz-typeck`/`ynz-codegen`), on the same seam, threading the ONE `suspension_source` classifier
  arm (CCIR-2 satisfied — plumbing consumers onto one source, not a second derivation). Fix
  mechanism unchanged → R1's residual (MEDIUM) unchanged; the existing RED-fixture-class +
  full-regression gate already covers exactly these fixtures. Shipping 4-of-9 would be WORSE
  (correctly-classified-but-mishandled → miscount/codegen-crash vs today's clean synchronous
  fallback). No Floor trigger; no signature (CCIR-5 not tripped). Same class as FRAGO 001/002.
  The `suspending_calls_in_subexpr_position` reorder is a benign consistency change (flagged as a
  boundary should-fix: confirm no diagnostic-ordering shift vs another same-span check).
- **Delta (applied to `plan.md` by re-dispatched executor).** Phase 1 task line, Key Outcome #1,
  and Phase 1 exit criteria: replace "4 broken predicate sites" with the full enumerated site set
  (the original 4 + the 5 coupled sites named above), preserving the "all threaded from the ONE
  authoritative resolution" language. Phase 1 continues across all sites in the same phase (no new
  phase).
- **Authority.** deviation-judge classified JUSTIFIED/RISK-NEUTRAL; conductor ratified auto-apply
  per Step-7 risk-neutral flow (no signature gate). Applied to `plan.md` by the segment-2 executor
  (`executor-2026-07-09-m6-phase1-seg2`) as its first action before continuing the fix.

### FRAGO 004 — 2026-07-09 — session-id: `conductor-2026-07-09-m6-exec` — **RISK-RAISING, SIGNED**

- **Trigger.** Phase 1 segment 4 surfaced a confirmed memory-safety miscompile blocking fixture (b)
  and Phase 1's "all 4 GREEN" exit criterion: a **shape-value argument to a suspending callee** is
  staged in the PARENT resume fn's **stack alloca**; the child frame stores a `ptrtoint` of it;
  parent returns Pending → parent stack dies → child resumes on a **dangling `self` → silent
  nondeterministic garbage (use-after-free)**.
- **Corroboration (adversarial code-reviewer, agent `ae5ab98a2e94c4182`): CONFIRMED.** Reproduces in
  pure `Call` form (`wait crew(ship)`) AND auto-inserted transitive suspension; **PRE-EXISTING in
  shipped v0.3.0** — proven by stashing ALL M6 WIP back to byte-identical `main` and still getting
  garbage with identical staging IR (parent frame 72B = header+child, zero local slot; shape in
  `%ship_own` stack alloca; `ptrtoint` into child self-slot). **NOT introduced by M6** — UFCS merely
  made the shape-passing form natural to exercise. Likely also affects `fixed<T>`; string/array/map
  are safe (heap-backed, stable pointer). Root cause: `locals_crossing_wait` /
  `collect_crossings_in_stmts` (`check.rs:8122+`) is a lexical "read-AFTER-suspension" source scan
  that misses escape-through-a-callee-frame (the suspending callee holds the arg by pointer and reads
  it after its own suspend point, so it DOES cross, but the source scan never flags it → no
  `shape_embed`, plain stack alloca). One sub-claim refuted: fixture (a) is genuinely frame-backed,
  not "luck."
- **Classification (deviation-judge, agent `a7d77f37c1c12bf87`): JUSTIFIED / RISK-RAISING.**
  Re-score on REF-risk-engine: **Prob A** (Frequent — mainline pattern: zero pre-existing fixtures
  pass a shape to a suspending callee; 100% deterministic failure when exercised) **× Sev II**
  (silent-miscompile anchor per D7; no Floor-A/B — pre-v1.0, zero-users, git-reversible,
  runtime-only-within-one-execution). **Initial EH (A×II)** → after standard B2 mitigation
  (RED-repro + full-regression gate, Prob −1) → **Residual HIGH (B×II)**. Unlike R1/R2/R4 (which
  score their trigger at C-Occasional and land MEDIUM), this is a raw shipped mainline miscompile, so
  it does not clear to MEDIUM. **CCIR-5 fires → RISK OVERRIDE required, routed to Patrick, never
  self-signed.**
- **RISK OVERRIDE — accepted residual: HIGH. SIGNED.**
  - **Risk:** shape-value argument to a suspending callee stages a dangling stack pointer across
    Pending — confirmed UAF / silent-garbage miscompile; pre-existing in shipped v0.3.0; reproduces
    in `wait fn(shapeArg)` Call form + auto-inserted transitive suspension; likely also `fixed<T>`.
  - **Why not mitigable to LOW now:** standard B2 only shifts Prob A→B; Sev II is the established
    silent-miscompile anchor (D7); real elimination needs the crossing-classifier fix itself
    (Phase 1b), not yet built.
  - **Accepted by: Patrick — 2026-07-09** (interactive CCIR-5 sign gate; disposition: "Sign + fix 1b
    immediately next").
  - **Trigger to revisit / close:** Phase 1b's RED→GREEN fixture + full-regression proof lands
    (converts accepted-HIGH interim risk → closed, fixed bug — before Phase 6b sanitizer lane and
    before release).
- **Delta (applied to `plan.md` by re-dispatched executor).** (1) Insert **Phase 1b — shape-arg
  frame-backing miscompile** immediately after Phase 1, and — per Patrick's signed sequencing —
  sequenced to run as the NEXT phase (before Phase 2): P1(seal) → P1b → P2 → P3 → P3b → P4 → P5 →
  P6 → P6b → P7 → P8. Phase 1b: own RED-repro (fixture (b) + a pure-`Call` repro, both locked),
  fix the crossing/frame-layout classification so a stack-backed shape (and `fixed<T>`) aggregate
  passed by pointer to a suspending callee is frame-backed via the existing `shape_embed` machinery
  (authoritative-derivation: extend the ONE crossing classifier, no second path), full-regression +
  M3a-class caution, Model tag `(coding, high, medium)`. (2) Amend Phase 1 task line / Key Outcome #1
  / exit criteria: Phase 1 delivers the 9-site threading + 3/4 fixtures (a,c,d) green + R1
  zero-regression; **fixture (b) is carved out as Phase 1b's locked RED-repro** (a legitimate
  planned-RED per no-duct-tape's inverse — documented, locked by the failing test, closed by the
  immediately-next phase, never shipped alone). (3) Concept 11→12 phases; Coordinating Instructions
  sequencing updated (Phase 1b before Phase 2; still before Phase 6b). (4) Add the Phase 1b Safety
  invariant. (5) Record this SIGNED RISK OVERRIDE block in the plan's Risk Assessment section as a
  new row/override with Patrick's acceptance.
- **Authority.** deviation-judge classified JUSTIFIED/RISK-RAISING/HIGH; conductor drafted the
  override (never self-signed); **Patrick signed it at the interactive CCIR-5 gate, 2026-07-09**, and
  chose the "Phase 1b immediately next" sequencing. Applied to `plan.md` by executor
  `executor-2026-07-09-m6-frago004`.

### Minor observation — Phase 1 boundary review (conductor, 2026-07-09)

- **`synthesize_ufcs_call_expr` structural-check duplication (non-blocker, sub-should-fix).**
  graveyard-auditor (agent `adee86f9e7e4a2f16`) noted `synthesize_ufcs_call_expr` (emit.rs,
  background-spawn normalization) duplicates only the structural "is receiver a shape" check — NOT
  the suspend predicate — so it is **not** an authoritative-derivation violation (the ONE suspend
  source is intact; grep gate + all six reviewers confirm). Purely a cosmetic un-DRY. Not fixed in
  Phase 1. **Fold-if-trivial flag for Phase 1b** (the immediately-next phase, same `emit.rs`
  frame-backing/crossing neighborhood): if 1b's executor touches that region, consolidate the
  shape-check; otherwise it is a cosmetic follow-up with no correctness impact. Recorded here (durable)
  rather than via a heavyweight four-field deferral — YAGNI ceiling: a named follow-up in a
  same-neighborhood immediately-next phase, no correctness or safety dimension.

### FRAGO 005 — 2026-07-09 — session-id: `conductor-2026-07-09-m6-exec` — pending deviation-judge

- **Trigger.** Phase 1b segment 1 found the plan's stated `fixed<T>` fix mechanism inaccurate: the
  Phase 1b text says shape AND `fixed<T>` aggregates are "frame-backed via the existing `shape_embed`
  machinery," but **no embed machinery exists for `fixed<T>`.** The compiler's established design
  (Check 2b `UnsupportedCrossingLocalType`, `check.rs:872-984`) already deliberately BLOCKS
  non-embeddable aggregates crossing a suspension with a teaching error. The executor's settled fix
  routes `fixed<T>`-arg-escape-to-suspending-callee into that SAME existing guard → the UAF is closed
  as a deterministic COMPILE ERROR (not frame-backing), consistent with the existing read-after-wait
  twin's handling.
- **Classification (deviation-judge, agent `a84d4ab1d5539b2bc`): JUSTIFIED / RISK-NEUTRAL-to-RISK-
  LOWERING.** The plan assumed a `fixed<T>` frame-embed mechanism that was never built and (per Check
  2b's own design commentary) never intended for `fixed<T>`; the correction reuses the ONE existing
  guard (authoritative-derivation — no second frame-layout path). Phase 1b's intent (close the UAF for
  shape + `fixed<T>` args) is preserved; only the `fixed<T>` closing mechanism changes from
  (nonexistent) frame-backing to (existing) deterministic compile error. **No program works correctly
  today with this pattern** (100% deterministic garbage), so converting it to a compile error regresses
  nothing — strictly safer than a silent UAF, and lower-engineering-risk than inventing new embed
  machinery in the ~10-round-whack-a-mole M3a subsystem. **No re-signature of the R13 HIGH-residual
  override** (the closing mechanism changed, not the risk or its acceptance).
- **Delta (applied to `plan.md` by re-dispatched executor).** Phase 1b task/steps: **shape<T> arg** →
  frame-backed via the existing `shape_embed` machinery (unchanged); **`fixed<T>` arg** → routed into
  the EXISTING Check 2b `UnsupportedCrossingLocalType` guard (`check.rs:872-984`) → UAF closed as a
  deterministic teaching compile error, consistent with the read-after-wait twin. Add to Phase 1b exit
  criteria / step 4: an explicit **false-positive sweep** for the newly-fired `fixed<T>` compile error
  (mirror Phase 2's corpus false-positive discipline) — confirm the extended classifier routes only
  genuine escape-to-suspending-callee cases into Check 2b, not every `fixed<T>` argument. Residuals
  (anonymous-aggregate / loop-var / number-maybe-union by-pointer args) recorded as **post-fix probes**
  (probe after the fix; if any is LIVE it needs its OWN FRAGO, never a quiet Phase-1b scope-add — the
  signed R13 override was scoped to shape + likely-`fixed<T>`, not the full type matrix). The orthogonal
  pre-existing backend ICE on `fixed<T>` PARAM iteration ("cannot iterate fixed array with unknown
  size") → recorded as its own four-field **Future Requirements** deferral (different bug, not the UAF
  class; D6/P2-7 precedent).
- **Authority.** deviation-judge classified JUSTIFIED/RISK-NEUTRAL; conductor ratified auto-apply per
  Step-7 risk-neutral flow (no signature gate; R13 override unaffected). Applied to `plan.md` by the
  segment-2 executor (`executor-2026-07-09-m6-phase1b-seg2`) before applying step 3.

### FRAGO 006 — 2026-07-09 — session-id: `conductor-2026-07-09-m6-exec` — **RISK-RAISING, SIGNED**

- **Trigger.** Phase 1b's post-fix residual probes (FRAGO 005 step 6) confirmed the shape-arg UAF is a
  WIDER class than R13 scoped: (1) **`number`/`maybe`/`union` args** to a suspending callee stage a
  dangling pointer across suspension → silent garbage (`number` prints 0.000 vs 2.5; they share the
  `value_to_i64_bits` by-pointer staging arm, `emit.rs:12323`); (2) **anonymous struct-literal args**
  → silent garbage (no LET name for the classifier to anchor on). Same escape-through-callee-frame
  root cause as the shape UAF; both pre-existing in shipped v0.3.0. Non-LIVE (correct): indexed args
  (typeck-rejected), loop-var shape args (heap array storage survives).
- **Classification (deviation-judge, agent `a61541fb73c35fe16`): JUSTIFIED / RISK-RAISING / HIGH.**
  Both land **Residual HIGH** (Sev II per D7; number/maybe/union Prob A→B×II=H; anon Prob B→C×II=H —
  converges to HIGH either way after B2 mitigation). **OUTSIDE R13's signed scope** ("shape… likely
  also `fixed<T>`") → require Patrick's OWN fresh signature, never inherited from R13. In no case ship
  either silently (no-duct-tape live-exposure rule + Mission).
- **RISK OVERRIDE (R14) — accepted residual: HIGH. SIGNED.**
  - **Risk:** the shape-arg UAF class is wider than R13 — `number`/`maybe`/`union` args AND anonymous
    struct-literal args to a suspending callee also stage a dangling pointer across suspension → silent
    garbage; pre-existing v0.3.0.
  - **Why not mitigable to LOW now:** same silent-miscompile class as R13; real elimination needs the
    frame-backing fix itself (extend Phase 1b for number/maybe/union; new Phase 1c for anon).
  - **Accepted by: Patrick — 2026-07-09** (interactive gate; disposition: **"Fully fix the entire class
    now"** — full frame-backing for the whole class, nothing rejected, nothing deferred).
  - **Trigger to close:** the number/maybe/union fix (Phase 1b) AND the anonymous-aggregate fix (Phase
    1c) each land RED→GREEN + full-regression + determinism proof before Phase 6b's sanitizer lane and
    before release.
- **Delta (applied to `plan.md` by re-dispatched executor).** (1) **Grow Phase 1b scope**: extend the
  fix + repros to frame-back `number`/`maybe`/`union` args that escape to a suspending callee (the
  `value_to_i64_bits` staging arm) so they WORK across suspension (NOT rejected) — folded into Phase 1b
  before it seals; widen the classifier's candidate type-match arm + add the consumer-side plumbing for
  the scalar-boxing path. (2) **Insert Phase 1c — anonymous-aggregate arg frame-backing across
  suspension** (new anchoring for unnamed temporaries — near-new-design; own RED-repro + full-regression
  + determinism proof), sequenced after Phase 1b, before Phase 2 (continuing the "shipped memory-safety
  UAF, fix first" priority) and before Phase 6b. (3) Concept 12→13 phases; Coordinating Instructions
  sequencing (P1→P1b→P1c→P2→…; P6b after 1,1b,1c,3,3b,4,5); Safety invariants for the number/maybe/union
  + anon cases. (4) Record R14 signed override in the Risk Assessment section (alongside R13).
- **Authority.** deviation-judge classified JUSTIFIED/RISK-RAISING/HIGH; conductor drafted the R14
  override (never self-signed); **Patrick signed it, 2026-07-09**, disposition "fully fix entire class
  now." Applied to `plan.md` by executor `executor-2026-07-09-m6-phase1b-seg3`.

### FRAGO 007 — 2026-07-09 — session-id: `conductor-2026-07-09-m6-exec`

- **Trigger.** Phase 1b segment 4's verify-first recon falsified FRAGO 006's premise that `maybe`/`union`
  are `number`-like (bounded, in-Phase-1b) work. Both are confirmed LIVE UAFs, but **NEEDS-NEW-MACHINERY**
  (fixed<T>/anon category): no Maybe/Union arm in the crossing strategy table (`emit.rs:4492-4556`); Maybe
  needs envelope+payload ownership machinery; Union repr is non-uniform (documented `value_to_stable_bits`
  known-hole); any fix must route past Check 2b without a second classification path. `number` remains
  genuinely feasible/number-like → stays in Phase 1b.
- **Classification (deviation-judge, agent `a8867c932ee309751`): JUSTIFIED / RISK-NEUTRAL / R14-intent-
  PRESERVED / NO new signature.** The vulnerable CLASS is unchanged (R14 already named number/maybe/union
  + anon as one override); this only re-assigns which phase-label performs the maybe/union work — internal
  sequencing, same risk/severity/disposition, same closing gate (both 1b and 1c precede Phase 6b/release).
  Distinct from the R13→R14 jump (which widened the class → needed a fresh signature); this is the
  FRAGO-001/002/003/005 shape (conductor auto-applies, no signature). maybe/union still get FULL
  frame-backing (never rejected, never deferred) — just under Phase 1c. 1b could never have sealed sooner
  with maybe/union nominally in-scope (they need the same new-machinery either way), so the interim-exposure
  window relative to release is unchanged.
- **Delta (applied to `plan.md` by re-dispatched executor).** (1) **Re-home `maybe`/`union` frame-backing
  from Phase 1b to Phase 1c**, making Phase 1c "hard new-machinery frame-backing: anonymous-aggregate +
  maybe + union." **Carry maybe/union's FULL proof obligations verbatim into 1c** (RED-repro-before-fix;
  N≥10 determinism; false-positive sweep; explicit verification the fix routes maybe/union PAST Check 2b's
  rejection, not into it — never a compile-error reject, per R14). (2) **Narrow Phase 1b** to its feasible
  cases: shape (frame-backed ✅), fixed<T> (Check-2b compile-error ✅), number (frame-backed, in progress).
  Phase 1b seals once number lands. (3) **plan-source-of-truth sibling sweep (required):** every Phase 1b
  exit-criteria / Key-Outcome-1b mention attributing maybe/union to Phase 1b, AND the R14 override's
  "Trigger to close" phrasing, move maybe/union → Phase 1c. (4) Phase 1c Safety invariant grown to name
  maybe/union.
- **Heads-up (informational, NOT a gate — delivered to Patrick):** Phase 1c's effort tripled (anon → anon
  + maybe + union, three distinct near-new-design problems). "Fully fix, nothing rejected" still holds for
  all three. No new signature required (deviation-judge-confirmed).
- **Authority.** deviation-judge classified JUSTIFIED/RISK-NEUTRAL, R14-intent preserved, no new signature;
  conductor ratified auto-apply per Step-7 risk-neutral flow. Applied to `plan.md` by executor
  `executor-2026-07-09-m6-phase1b-seg5`.

### FRAGO 008 — 2026-07-09 — session-id: `conductor-2026-07-09-m6-exec`

- **Trigger.** Phase 1b's new concurrent tests exposed a PRE-EXISTING full-workspace-load flake:
  `v03_m3e_alias_local_name_collision_runs_correctly` (`integration.rs:2268-2308`) fails 3/3 under
  full-workspace parallel load, passes 9/9 isolated + 522/522 integration-alone. Its own fixture comment
  (`integration.rs:2299-2301`) concedes a fixed-shutdown-timing race margin; Phase 1b's ~26 new concurrent
  `ynz` spawns tip it past that margin.
- **Classification (deviation-judge, agent `a37d34eb596a9a7cb`): PRE-EXISTING flake, exposed-not-caused
  (IR-proven orthogonal + isolation + discriminating-skip triangulation); FIX-NOW; LOW risk; AUTO-APPLY
  (test-fixture-only, zero product code, strictly monotonic reliability); NO signature.** No genuine WHY
  to defer (same fix size now/later; deferring poisons every Phase 2-8 full-suite gate). **Patrick
  disposition: "ok tacking it onto the next phase."**
- **Delta (to be applied to `plan.md` by the NEXT chat's executor when Phase 1c starts).** Add an **early
  gating step to Phase 1c (step 0)**: replace the M3e fixture's fixed-timeout shutdown-race margin with a
  **real synchronization primitive** (a join/barrier/channel the background task closes BEFORE the test
  asserts) so the full suite is deterministically green under load. **GUARDRAIL (deviation-judge):** must
  REMOVE the race (real sync), NOT widen the sleep (a bigger number is the same race with a longer fuse →
  duct tape). After it lands, the full workspace suite is 522/522 under load and every downstream gate is
  clean.
- **Authority.** deviation-judge classified RISK-NEUTRAL/LOW/no-signature; conductor + Patrick ratified
  (fold into Phase 1c as its gating step-0). Application to `plan.md` deferred to the next chat (this
  conductor's context is full — see the SESSION-END HANDOFF note in the Session log).
- **[2026-07-10 materialization note — `conductor-2026-07-10-m6-exec2`]** This conductor IS the next chat.
  FRAGO 008's plan.md delta (Phase 1c step-0) is applied by the amendment executor bundled with FRAGOs
  009–012 below (deviation-judge re-confirmed JUSTIFIED/risk-neutral at the Phase-1b fix-loop re-review).

> **Numbering note (`conductor-2026-07-10-m6-exec2`, 2026-07-10):** an earlier Session-log scope-addition
> entry tentatively labeled the user scope-adds "FRAGO 009 (P2-7) / 010 (P1-2) / 011 (durable-home)."
> Superseded: FRAGO 008 already existed, and a fresh execution finding (sibling decimal128 UAF class)
> earns the next number. FINAL: **009 = sibling decimal128 spawn-boundary UAF**, **010 = P2-7**,
> **011 = P1-2**, **012 = durable-home lift**.

### FRAGO 009 — 2026-07-10 — session-id: `conductor-2026-07-10-m6-exec2`

- **Trigger.** Phase 1b's fix-loop + boundary review surfaced a SIBLING to the arg-UAF class: a `number`
  (decimal128) value crossing a CONCURRENCY boundary by-pointer without an eager heap-copy. code-reviewer
  (opus) traced three sites; a fable verification probe (HEAD 47abd29, gitignored scratch) empirically
  resolved each (Paper-Trace):
  - **(A-CPU + A-SM) background-spawn decimal128 arg — LIVE runtime UAF.** `prepare_bg_arg_for_ctx`
    (emit.rs:15386) has no `Number` arm → number falls through as `BgArgFreeKind::None`, staging
    spawner-stack bits; both spawn arms (`ynz_rt_spawn_blocking` :15840, `ynz_rt_spawn` :16060) print
    `0.000...` (5/5 deterministic) vs `2.5`; IR-confirmed both arms.
  - **(C) cpu-member decimal128 arg — LIVE compile-time ICE.** `emit_cpu_member_spawn` (emit.rs:9506-9509)
    `build_load(i64)` mismatches the pointer-typed number param → LLVM verifier failure surfaced as
    "compiler bug" on valid code (3/3, fused + pure-CPU); `YNZ_NO_AUTO_PARALLEL=1` baseline is correct.
    Loud crash, not silent.
  - **(B) conduit-send decimal128 — VERIFIED-SAFE-BY-GATE (unreachable today).** `channel<number>` is
    compile-gated by typeck (check.rs:3369-3398) with a teaching error naming this exact UAF class; the
    emit.rs:11809 Number-send path is unreachable from any current syntax.
- **Corroboration.** code-reviewer Part 2 (plausibly-live trace) → fable probe empirical confirm; deviation-judge
  (Phase-1b fix-loop re-review) classified the sibling class JUSTIFIED / risk-neutral.
- **Classification.** JUSTIFIED / RISK-NEUTRAL. A/C are confirmed-live pre-existing defects in the
  concurrency/auto-parallel charter (silent garbage + ICE on valid code — the user's "no bugs" bar). B is
  provably-unreachable dead code (YAGNI ceiling — fixing emit.rs:11809 now = the same "guard for dead
  code" declined at FRAGO 002). Verify-before-fix honored (probe confirmed each before scoping). No
  signature.
- **Delta (to `plan.md`, by re-dispatched executor).** Insert a NEW fix phase after Phase 1c (both
  hard-new-machinery decimal128 phases) closing A (background-spawn) + C (cpu-member)
  decimal128-across-task-boundary defects, RED-repro-before-fix (committed repros from the probe shapes).
  **Fix APPROACH is the phase executor's DESIGN CALL, not conductor-pre-decided** (charter: route-not-design):
  the live precedent is the EXISTING channel<number> compile-gate (check.rs:3369-3398) — one policy for
  "aggregate/number crosses a task boundary" — so the executor weighs gate-consistently (reject with the
  same teaching error; cheap, consistent, matches Phase 1b's fixed<T> precedent) vs eager i128 heap-copy
  (hard machinery, would also unlock channel<number>), against IMP-concurrency / IMP-no-function-coloring.
  B → Future-Requirements deferral-with-trigger (verified-safe; revisit when channel<number> heap-copy
  ships). Update Concept count + sequencing; add risk row; Future-Req entry for B.
- **Authority.** deviation-judge JUSTIFIED/RISK-NEUTRAL; conductor auto-apply per Step-7 risk-neutral flow
  (no signature). Probe: fable, HEAD 47abd29.

### FRAGO 010 — 2026-07-10 — session-id: `conductor-2026-07-10-m6-exec2`

- **Trigger.** USER-DIRECTED scope addition (Patrick, this session): un-defer **P2-7**
  (`handle_recv_poll` panic-then-pending hang, Future-Requirements #7 / R11) and fix it in M6. A real
  concurrency hang: if the panic path (`handle.rs:297-303`) fires before waker registration, the task may
  never wake.
- **Classification.** JUSTIFIED / RISK-NEUTRAL. Un-defers a LOW-scored real bug in the concurrency
  charter; fix mirrors Phase 4's register-before-poll discipline (~0.5 session). User is the Mission-scope
  authority; no signature.
- **Delta (to `plan.md`, by re-dispatched executor).** Insert a new fix phase right after Phase 4 (same
  register-before-poll region), RED-repro-before-fix (panic-before-registration repro), extend the fix to
  the handle poll path. Move P2-7 out of Future Requirements #7 (fixed, not deferred); update Concept
  count + sequencing; adjust R11 (→ fixed).
- **Authority.** User-directed Mission-scope; risk-neutral; conductor auto-apply + log.

### FRAGO 011 — 2026-07-10 — session-id: `conductor-2026-07-10-m6-exec2`

- **Trigger.** USER-DIRECTED scope addition (Patrick, this session): un-defer **P1-2** (twin type-walker
  unification, Future-Requirements #2). Confirmed DORMANT (Phase 0), but it is the exact twin-derivation
  class that shipped silent miscompiles across M3a/M3d/M3e/M3g — unify the two walkers (`emit.rs:8276`
  typeck vs `:8364` substituted; field `Cg.type_subst`) behind one authoritative resolution.
- **Classification.** JUSTIFIED / RISK-NEUTRAL. Dormant authoritative-derivation hardening (~0.5 session);
  aligned with authoritative-derivation.md. User Mission-scope authority; no signature.
- **Delta (to `plan.md`, by re-dispatched executor).** Insert a new cleanup phase sequenced BEFORE Phase
  6b (so the sanitizer lane scans the unified walker); fold the two walkers behind one shared resolution;
  regression-gate the SM-resume + frame-layout suites. Move Future Requirements #2 (→ fixed); update
  Concept count + sequencing.
- **Authority.** User-directed Mission-scope; risk-neutral; conductor auto-apply + log.

### FRAGO 012 — 2026-07-10 — session-id: `conductor-2026-07-10-m6-exec2`

- **Trigger.** USER-DIRECTED (Patrick, this session): the LEFT-DEFERRED items currently live ONLY in this
  plan's `plan.md`/`audit.md` → they vanish when the plan `git mv`s to `done/`. They need a durable home
  outside the archiving dir.
- **Classification.** JUSTIFIED / RISK-NEUTRAL (docs/tracking reconciliation). Durable home = the roadmap
  `2026-05-21-v0-3-concurrency-perf/` (stays `active` until the whole v0.3 campaign finishes): Capability
  Ledger (roadmap.md:417/471) + roadmap `audit.md`.
- **Delta (to `plan.md`, by re-dispatched executor).** Extend **Phase 8** (already the roadmap-reconciliation
  phase) with a REVIEWED deliverable: LIFT every surviving deferral (P2-3; channel-close; preemption;
  `background.cpuBound`; the two orthogonal ICEs; dynamic-dispatch #10; `fixed<T>` param-iter #11; and
  FRAGO-009's B conduit-send number) into the roadmap durable store — four-field payloads → roadmap
  `audit.md`; pointer rows → roadmap Capability Ledger, owner-tagged (preemption→M7; channel-close/P2-3→M8;
  ICEs + dynamic-dispatch→`unscoped → needs a milestone`, row-441 flagged for Patrick's Gate-4 call).
  Reviewer fan-out checks it landed.
- **Authority.** User-directed; risk-neutral; conductor auto-apply + log.

### FRAGO 013 — 2026-07-10 — session-id: `conductor-2026-07-10-m6-exec2`

- **Trigger.** Phase 1c segment 1 found the plan's step-3 prescription (frame-back maybe/union args via
  the crossing strategy-table arms, paralleling Phase 1b's shape `shape_embed`) is STRUCTURALLY IMPOSSIBLE
  for union: the `T | nothing` none-case is a NULL value-pointer checked via `build_is_null`
  (`emit.rs:17509-17539`, doc `:3327-3350`); a frame-region pointer is never null → frame-embed breaks
  `is`-none semantics.
- **Corroboration.** deviation-judge independently re-confirmed against live code (`emit.rs:3327-3350` doc
  + `:17522` `build_is_null` in the union print arm; `maybe_to_heap_cell` funnel live `:3077→3091`).
  Paper-Trace complete (observed NULL-checked ptr / expected stable non-null frame slot / irreconcilable
  residual).
- **Classification.** JUSTIFIED / RISK-NEUTRAL. Verified structural ABI incompatibility, not a shortcut.
  The alternative PRESERVES the signed R14 intent (behavioral commitment = maybe/union WORK across
  suspension, not rejected — R14 signed the behavior, not one technique). Stays inside R14's already-signed
  HIGH residual + its unchanged trigger-to-close; adds one function mirroring a shipped sibling; no new
  irreversible op. No fresh signature (same shape as FRAGO 001/002/008).
- **Delta (to `plan.md`, applied by the re-dispatched Phase-1c seg-2 executor as its FIRST action — this
  plan's established FRAGO-005/006/007 pattern).** (a) Phase 1c step 3: replace the implied union/maybe
  frame-embed mechanism with **bind-time promotion to counted heap cells** — reuse `maybe_to_heap_cell`
  (`emit.rs:3077`) + add a symmetric `union_to_heap_cell`; ONE uniform mechanism, route PAST Check 2b (not
  into it), the default pointer-flush path + the single `stage_suspending_call_arg_bits` staging helper (all
  three loops) UNCHANGED — no second derivation. Open recon carried to build time under the phase's OWN
  gates (full-regression + false-positive sweep), NOT assumed safe: heap-cell ownership/free semantics in
  loops (per-iteration leak?) — handoff item 5(b). (b) Add `**CHECKPOINT**` marks to Phase 1c's 6-step list
  (>5-step REF-plan-format trigger; plan authoring omission). Both bundled per the FRAGO-008 precedent.
- **Authority.** deviation-judge JUSTIFIED/RISK-NEUTRAL (both deltas); conductor auto-apply per Step-7
  risk-neutral flow (no signature). Applied by the re-dispatched executor as its first action.

### FRAGO 014 — 2026-07-10 — session-id: `conductor-2026-07-10-m6-exec2`

- **Trigger.** Phase 1c segment 3 surfaced an OVER-FAT-STEP proposal: step 3 ("The fixes") spans 2 crates +
  8+ edit sites + 6 RED→GREEN + an alloc-parity probe — genuinely > one window (seg-2 spent a full window
  on step 2; seg-3 spent its full window on step-3's mandated verify-recipe pre-work landing zero edits, then
  bounced BLOCKED — no completed-step boundary behind it, PARTIAL illegal). Two coupled findings: (ITEM 2)
  Check 2 (nested-shape, check.rs:817-870) is a SECOND rejection surface the plan's "route PAST Check 2b"
  wording doesn't name; (Decision 12) union-annotated lets resolve to `Shape{variant}` via
  `crossing_local_type_from_body` (emit.rs:8418), NOT the assumed default ptr-alloca.
- **Corroboration.** deviation-judge classified against live code: split premise JUSTIFIED (matches
  REF-plan-format's over-fat-step legal floor; BLOCKED-not-PARTIAL correct — no completed-step boundary
  behind seg-3); ITEM 2 routing-past-both-gates WITHIN R14's already-signed "nothing rejected" scope (Check 2
  + 2b consume the SAME crossing set, check.rs:811; R14 is class/outcome-scoped → no fresh signature);
  Decision 12 a legitimate verify-before-fix correction. Confirmed the ONE classifier
  (`crossing_local_names_with_cpu_spike`) feeds both typeck Check 2/2b AND codegen frame-layout
  (emit.rs:31/327/774/4136) — no twin.
- **Classification.** JUSTIFIED. The split AS LITERALLY PROPOSED is RISK-RAISING (deviation-judge should-fix):
  landing 3a's Union classifier-widen alone would make emit.rs's unmodified loop misclassify the union
  crossing local into `shape_embed_set` (Decision 12) → the locked union RED fixture fails DIFFERENTLY
  (misclassification/ICE) at the 3a→3b boundary before 3c's fix exists, violating RED-repro discipline.
  **Corrected per deviation-judge: DEFER Union entirely (classifier-widen AND codegen) to 3c; 3a widens Maybe
  + anon-StructLit ONLY.** With that correction every boundary is green-building → RISK-NEUTRAL, auto-apply,
  NO signature. ITEM 2 risk-neutral/in-scope. No signature gate trips.
- **Delta (to plan.md, by the re-dispatched seg-4 executor as FIRST action; resume-at repaired to
  `phase-1c/step-3a`).** Replace Phase 1c step 3 with the CORRECTED 4-part sub-step split:
  - **3a — typeck routing (Maybe + anon ONLY; Union DEFERRED to 3c):** widen `mark_aggregate_arg`
    (check.rs:7913-7918) with `Type::Maybe` + anon-`StructLit` (NOT Union); provenance-returning core
    (`arg_escape_only`); Check 2 (:817-870) AND Check 2b (:898-984) skip rejection iff arg-escape-only AND
    effective type is Maybe. Exit: builds; maybe+anon repros COMPILE (still UAF-RED); union repro FULLY RED
    (NOT misclassified); fixed<T> + m3a read-after-wait rejections byte-identical green.
  - **3b — codegen maybe + anon:** `store_binding` (emit.rs:19836) crossing-maybe → `maybe_to_heap_cell`;
    `stage_suspending_call_arg_bits` (:11274) routes `Expr::StructLit` args (all three loops). Exit:
    maybe+anon GREEN (4/6); union RED.
  - **3c — codegen union (incl. ITS classifier-widen + annotation override):** add Union to the classifier
    widen WITH the annotation-aware override (via `ast_type_to_typeck_type` — one resolution, no twin) so it
    does NOT misclassify into shape_embed; add Check 2/2b Union skip; new `union_to_heap_cell` (null→null,
    tag-switched payload deep-copy); union-ctor Let arm (:12641-12691) crossing branch stores the cell into
    the pre-created sm_entry alloca (never a fresh `outer_slot`); `store_binding` Union arm; KNOWN-HOLE doc
    (no `value_to_stable_bits` Union arm). Exit: all 6 GREEN.
  - **3d — parity verdict + IR proof:** exact-gap alloc=free loop probe (per-iteration crossing maybe
    binding), verdict RECORDED either way; per-case IR (arg staged from surviving allocation). Then steps 4-5.
  - Also: name Check 2 alongside Check 2b throughout Phase 1c step 3's "route past" wording (ITEM 2).
- **Authority.** deviation-judge JUSTIFIED; ITEM 2 within-R14-scope (no fresh signature); the
  risk-raising-as-proposed neutralized by the mandated boundary correction (Union → 3c) → risk-neutral →
  conductor auto-apply per Step-7 flow. Applied by re-dispatched executor as first action.

### FRAGO 015 — 2026-07-10 — session-id: `conductor-2026-07-10-m6-exec2`

- **Trigger.** Phase 1c fleet review (deviation-judge + rules-compliance both flagged) — the confirmed
  per-iteration heap-cell leak deferral (a JUSTIFIED/risk-neutral four-field deferral matching M5's
  Future-Req #6 never-drop-locals precedent) lives ONLY in the Phase 1c completion note + audit history —
  it is NOT enumerated in plan.md's `## Future Requirements / Revisit` list AND NOT in FRAGO 012's Phase-8
  durable-home lift-list → it would be LOST on the plan's `git mv` to `done/` (the exact failure FRAGO 012
  exists to prevent; the durable-home requirement Patrick explicitly set this session).
- **Corroboration.** deviation-judge VERDICT (leak deferral JUSTIFIED/risk-neutral, no signature) named the
  durable-home GAP as "the one actionable gap before Phase 8"; rules-compliance minor independently flagged
  the same missing FRAGO/Future-Req cross-listing.
- **Classification.** JUSTIFIED / RISK-NEUTRAL (docs/tracking reconciliation — same shape as FRAGO 012). No
  signature.
- **Delta (to plan.md, by re-dispatched executor).** (a) Add the per-iteration maybe/union heap-cell loop
  leak as a NEW numbered entry in `## Future Requirements / Revisit` (four fields verbatim from the Phase 1c
  completion note: WHAT per-iter heap-cell leak for crossing maybe/union LOOP bindings held to process exit;
  WHY needs the ownership drop story, out of charter, same never-drop-locals class as M5 Future-Req #6; COST
  drop-story milestone 1-2 sessions + update the 2 parity pins; TRIGGER drop story lands OR real unbounded
  suspension-loop-over-maybe/union workload; pinned loud via `v03_m6_p1c_heap_cell_loop_parity_pins_*`). (b)
  Amend FRAGO 012's Phase-8 durable-home lift-list (and Phase 8's deliverable text) to INCLUDE this new
  deferral so the Phase-8 lift carries it to the roadmap durable store. (c) Comment-accuracy fix (Phase 1c
  fleet nit — code-reviewer's refuted nested-shape finding): update the maybe/union Check-2b skip comment
  (check.rs ~830) to CREDIT `store_field`'s per-field heap-celling (M5 P2) as the reason nested-shape
  payloads are safe under one-level promotion — so a future reviewer doesn't re-raise the same
  probe-refuted false positive. Rides into the Phase 1c boundary commit.
- **Authority.** deviation-judge + rules-compliance surfaced; risk-neutral; conductor auto-apply + log.

## Context-segment log

### 2026-07-09 — Phase 1, segment 1
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1-segment-1

- **segment number:** 1
- **session-id:** `executor-2026-07-09-m6-phase1`
- **subagent_tokens actual:** 264408
- **checkpoint reason:** planned mark (stopped at the plan's first authored `**CHECKPOINT**`, after step 2 — RED fixture class authored + confirmed failing for the documented reason).
- **canonical resume-at pointer:** `phase-1/step-3`
- **segment verdict:** `STATUS: PARTIAL` (loop continues). Also surfaced a plan-vs-reality deviation (plan's "4 sites" under-counts the MethodCall-blind seam by ~5 coupled sites) — routed to the deviation-judge → Step-7 FRAGO seam by the conductor; see FRAGO 003 (below).

### 2026-07-09 — Phase 1, segment 2
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1-segment-2

- **segment number:** 2
- **session-id:** `executor-2026-07-09-m6-phase1-seg2`
- **subagent_tokens actual:** 250348
- **checkpoint reason:** executor's own early-checkpoint judgment call (context budget; stopped at the step-3/step-4 boundary — a step boundary, not one of the plan's authored `**CHECKPOINT**` marks — on a green-building tree).
- **canonical resume-at pointer:** `phase-1/step-4` (advanced from segment 1's `phase-1/step-3` — no stall).
- **segment verdict:** `STATUS: PARTIAL` (loop continues). Landed: FRAGO 003 applied to plan.md (9-site enumeration + sibling sweep); step 3 done (`may_block.rs:1296` UFCS call-graph edge, `ynz-typeck` 28/28 green). Surfaced a NEW deviation CANDIDATE (unconfirmed — left an empirical probe spec, did not self-fold): typeck crossing/SM classifiers (`this_stmt_suspends` `check.rs:8275-8308`, `block_contains_inferred_suspension` `check.rs:6560-6578`, `is_suspending_call` `check.rs:9287`) appear Call/conduit-only for BARE un-`wait`ed UFCS suspending statements → a local crossing a bare UFCS suspension may never be frame-backed (M3a class). Outside FRAGO 003's ratified 9-site set; segment 3 probes it first, surfaces the verdict for the seam.

### 2026-07-09 — Phase 1, segment 3
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1-segment-3

- **segment number:** 3
- **session-id:** `executor-2026-07-09-m6-phase1-seg3`
- **subagent_tokens actual:** 287175
- **checkpoint reason:** executor's own early-checkpoint judgment call (context budget; step-4/step-5 boundary, not a plan mark; green-building tree).
- **canonical resume-at pointer:** `phase-1/step-5` (advanced from segment 2's `phase-1/step-4` — no stall).
- **segment verdict:** `STATUS: PARTIAL` (loop continues). Landed: step 4 done — site 2 (`cpu_admission.rs` MethodCall arm) + the two settled shared helpers (`ufcs_method_call_suspends` in `suspension_source.rs`; span-keyed `expr_is_ufcs_suspending_call` in `check.rs`, exported via `lib.rs`); `expr_types` threaded through the admission + emit caller chains; `ynz-typeck` + `ynz-codegen --lib` green; house clippy green. Statement-position UFCS now routes to the SM walker (free-rides on the one shared definition). **Bare-UFCS-crossing probe verdict: DEFERRED-CONFOUNDED** — pre-threading the probe program panics for the upstream P1-1 reason regardless, so it cannot discriminate LIVE/DORMANT yet; by-inspection still looks live (theory, not verdict); decisive run scheduled after step 6 lands (per handoff Decision 3). Still outside FRAGO 003's set, not self-folded.

### 2026-07-09 — Phase 1, segment 4
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1-segment-4

- **segment number:** 4
- **session-id:** `executor-2026-07-09-m6-phase1-seg4`
- **subagent_tokens actual:** 329667
- **checkpoint reason:** step-7 gated on a surfaced deviation (executor could not complete step 7 — fixture (b) + "all 4 GREEN" — because a confirmed out-of-scope miscompile blocks it; surfaced for the seam rather than self-fixed).
- **canonical resume-at pointer:** `phase-1/step-7` (advanced from segment 3's `phase-1/step-5` — no stall).
- **segment verdict:** `STATUS: PARTIAL`. Landed: steps 5–6 — ALL 9 FRAGO-003 sites threaded; **grep gate PASS** (one classifier + one AST wrapper; only name-keyed producer is the ratified pre-typing `may_block.rs` fixpoint edge; zero second derivation); check-3 reordered after `check_stmts`. RED class **3/4 GREEN** (a/c/d); fixture (b) still RED for a NEW reason. **R1: zero regression** in pre-existing Call-based suspension fixtures. House clippy green. Step 8 recorded (pre-fix mode PANIC-then-abort). Probes: name-collision SOUND; bare-UFCS-crossing local half RESOLVED-BY-THREADING. **SURFACED DEVIATION → FRAGO 004 (below):** shape-value arg to a suspending callee staged in the parent resume fn's stack alloca; child frame holds `ptrtoint`; parent Pending → stack dies → child resumes on dangling `self` → silent garbage (UAF). **CONFIRMED by adversarial code-reviewer** (agent `ae5ab98a2e94c4182`): reproduces in pure `Call` form AND auto-inserted transitive suspension; **PRE-EXISTING in shipped v0.3.0** (stash-to-`main` still garbage, identical IR), NOT introduced by M6; likely also affects `fixed<T>`; safe for string/array/map (heap-backed). Root cause: `locals_crossing_wait`/`collect_crossings_in_stmts` lexical read-after scan misses escape-through-a-callee-frame. deviation-judge (agent `a7d77f37c1c12bf87`): JUSTIFIED; re-score Prob A × Sev II → Initial EH → **Residual HIGH** (B×II) after B2 mitigation → **CCIR-5 fires, drafted RISK OVERRIDE routed to Patrick, never self-signed.** Recommended routing: new Phase 1b (mirror Phase 3b, before Phase 6b); close Phase 1 on its verified 9-site scope with fixture (b) carved out. **AWAITING PATRICK'S SIGNATURE DECISION** (see FRAGO 004 pending in FRAGO log). **[RESOLVED: Patrick signed the R13 override 2026-07-09, disposition "Phase 1b immediately next"; Phase 1 sealed at commit `f921efe`.]**

### 2026-07-09 — Phase 1b, segment 1
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1b-segment-1

- **segment number:** 1
- **session-id:** `executor-2026-07-09-m6-phase1b`
- **subagent_tokens actual:** 252784
- **checkpoint reason:** executor's own early-checkpoint judgment call (context budget ~150k+ mid-step-2; landed step 2, stopped at the step-2/step-3 boundary on a green-building tree).
- **canonical resume-at pointer:** `phase-1b/step-3` (first segment of Phase 1b — no prior pointer, no stall).
- **segment verdict:** `STATUS: PARTIAL`. Landed steps 1–2: CCIR-1 verified (`check.rs:8122` `locals_crossing_wait`, `:8242` `collect_crossings_in_stmts`; `is_shape → shape_embed_set` at `emit.rs:4504-4529`). RED repros locked + Paper-Traced: fixture (b) → 10 runs, 10 DISTINCT garbage values (UAF tell, nondeterministic); NEW `v0_3_m6_shape_arg_pure_call.ynz` (expect 7 → garbage), `v0_3_m6_shape_arg_transitive_chain.ynz` (auto-suspension, expect 8 → garbage), `v0_3_m6_fixed_arg_suspending_call_rejected.ynz` (expect compile error → wrongly compiles). Locked by `crates/ynz-driver/tests/v03_m6_shape_arg_frame_backing.rs` (4 tests incl. N=10 determinism gate; 0/4 pre-fix). Fix design settled (extend the ONE classifier `crossing_local_names_with_cpu_spike` `check.rs:7553` with an arg-escape-to-suspending-callee collector mirroring `collect_conduit_locals`; all consumers thread from it; zero codegen changes expected). Surfaced the `fixed<T>` mechanism correction → **FRAGO 005** (risk-neutral, ratified). Residuals recorded for post-fix probe.

### 2026-07-09 — Phase 1b, segment 7 (SEAL)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1b-segment-7

- **segment number:** 7 (segments 5 killed by user, 6 killed by spend-limit — both mid-work, no net edits; segment 7 = resume + seal). Also note prior Phase-1b entries in this log are seg 1/2/3/4; seg 5/6 left no completed-segment entry (killed).
- **session-id:** `executor-2026-07-09-m6-phase1b-seg7`
- **subagent_tokens actual:** 214789
- **checkpoint reason:** DONE — Phase 1b completed on its FRAGO-007-narrowed scope (shape/fixed/number); final segment, handoff deleted.
- **canonical resume-at pointer:** n/a (phase complete).
- **segment verdict:** `STATUS: COMPLETE`. Ground-truth correction: the `number` fix was ALREADY COMPLETE on disk (a killed segment got further than the PAUSE note credited); seg 7 verified-before-trusting (both number tests GREEN on first run against an up-to-date binary), **zero compiler-code changes**. Number fix (all pre-existing on disk, verified): ONE classifier `mark_aggregate_arg` widened to `Shape|BuiltinFixed|Number{p≤34}` (`check.rs:~7576-7920`); ONE shared `stage_suspending_call_arg_bits` (`emit.rs:11266-11316`) wired into inline-poll (:11118) + heap-boxed (:11412), slot math from authoritative `sm_crossing_slot_indices` (no twin); child `load()` via `inttoptr` from the staged pointer (`emit.rs:19923-19940`, `sm_number_param_set`). **IR before/after PASS** (`target/ir-check/v0_3_m6_number_arg_pure_call.ll`): dangling-alloca+ptrtoint GONE → GEP into surviving parent decimal128 frame region. **Gates:** Phase 1 4/4 GREEN (incl. fixture b now); Phase 1b 6/6 GREEN (shape 7, transitive 8, number 2.5 exact, fixed<T> Check-2b error); determinism N=10 shape+number; false-positive sweep clean (IR-verified non-escaping number stays plain alloca); integration 522/522; clippy `-D warnings` + fmt clean. **R13 CLOSED; R14 number portion CLOSED; R14 maybe/union + anon → Phase 1c (OPEN).** **DEVIATION SURFACED → FRAGO 008 (pending):** `v03_m3e_alias_local_name_collision_runs_correctly` fails 3/3 under FULL-WORKSPACE parallel load (passes 9/9 isolated, 522/522 integration-alone; full-workspace green with only the 6 new Phase-1b tests skipped — discriminating run). Diagnosis: pre-existing fixed-300ms shutdown-race margin in the M3e fixture, tipped by ~26 new concurrent `ynz` spawns. **IR-proven ORTHOGONAL** (all-int fixture, zero new instruction patterns). Test NOT weakened/skipped in anything committed. Candidate remedy: make the M3e fixture's background completion deterministic. Will recur on every future full-suite gate until dispositioned. Routed to deviation-judge → seam.

### 2026-07-09 — Phase 1b, segment 4
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1b-segment-4

- **segment number:** 4
- **session-id:** `executor-2026-07-09-m6-phase1b-seg4`
- **subagent_tokens actual:** 239255
- **checkpoint reason:** executor's own early-checkpoint judgment call (context budget; step-2/step-3 boundary) with the maybe/union nature verdict surfaced to the seam.
- **canonical resume-at pointer:** `phase-1b/step-3` (advanced from segment 3's `phase-1b/step-2` — no stall).
- **segment verdict:** `STATUS: PARTIAL`. **Part-A verify-first verdicts (probe-confirmed on pre-fix tree):** `maybe` = **LIVE UAF** (compiles — Check 2b never fires on arg-escape; 4 distinct garbage values vs 42); `union` = **LIVE UAF** (deterministic wrong tag: `circle` 5/5 vs control `square`). **Feasibility = NEEDS-NEW-MACHINERY** (fixed<T>/anon category, NOT number-like): no Maybe/Union arm in the crossing strategy table (`emit.rs:4492-4556`); Maybe needs envelope+payload ownership machinery; Union repr non-uniform (documented `value_to_stable_bits` known-hole); any fix must route past Check 2b without a second classification path. **DEVIATION SURFACED (not self-decided):** R14's signed "frame-back maybe/union IN Phase 1b" is falsified — they're near-new-design like anon, so belong in the new-machinery phase (1c), not 1b. Routed to deviation-judge → seam; see FRAGO 007 (pending). **Number half:** RED repro locked (`v0_3_m6_number_arg_pure_call.ynz` + value + N=10 determinism tests; RED for the documented reason 0.000 vs 2.5); fix design verified against live code (arg-staging loop `emit.rs:11103-11110` → Number `load()` copies bits to a fresh resume-fn stack alloca `emit.rs:19851-19865`; fix = widen `mark_aggregate_arg` to Number + stage a GEP into the parent frame's 2-slot decimal128 region keyed off `sm_crossing_decimal128_set`, ONE classifier; heap-boxed twin `emit.rs:11263+` in scope). Zero compiler-code changes this segment (design + RED-lock only). Steps 3–5 + close-out open.

### 2026-07-09 — Phase 1b, segment 3
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1b-segment-3

- **segment number:** 3
- **session-id:** `executor-2026-07-09-m6-phase1b-seg3`
- **subagent_tokens actual:** 238842
- **checkpoint reason:** executor's own early-checkpoint judgment call at the FRAGO-006-application boundary (context budget); the SIGNED FRAGO 006 re-opened steps 2–3's number/maybe/union halves in the amended plan, so the step ledger itself changed this segment.
- **canonical resume-at pointer:** `phase-1b/step-2` (NOTE: numerically lower than segment 2's `phase-1b/step-4`, but NOT a stall — the amended plan re-scoped step 2 to the new number/maybe/union RED-repro half; exact-string compare `step-4` ≠ `step-2` = advanced; the shape+fixed<T> halves of steps 1–3 stay DONE). Advancement this segment = the full FRAGO 006 plan amendment + fix-design recon.
- **segment verdict:** `STATUS: PARTIAL`. **FRAGO 006 / R14 applied in full**: Phase 1b scope grown (number/maybe/union frame-backing); **Phase 1c inserted** (anonymous-aggregate, new anchoring); Concept 12→13; sequencing P1→P1b→P1c→P2, Phase 6b after 1,1b,1c,3,3b,4,5; R14 signed override recorded beside R13 ("two signed HIGH residuals"); Safety invariants + sibling sweep done. **Fix-design recon (recorded, unresolved — next segment verifies + may surface):** (a) `number` crossing machinery is a bits-copy (i128 alloca + 2 frame slots, `emit.rs:6022-6045`) → classifier membership alone doesn't fix number arg-escape; consumer plumbing required (feasible, per FRAGO 006). (b) **Check 2b HARD-REJECTS `maybe`/`union` crossing locals** (`check.rs:919-933`) → they have NO crossing-embed machinery (same category as fixed<T>); "frame-back maybe/union" per R14 is therefore potentially near-new-design, not consumer-plumbing, AND it is UNCONFIRMED whether maybe/union arg-escape is a live UAF vs. already-Check-2b-rejected (moot) — probes were parser-confounded. Design constraint recorded, NOT resolved: next segment verifies maybe/union's actual nature (LIVE-UAF vs moot; frame-backable-like-number vs new-machinery-like-anon) and SURFACES to the seam rather than blind-frame-backing.

### 2026-07-09 — Phase 1b, segment 2
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1b-segment-2

- **segment number:** 2
- **session-id:** `executor-2026-07-09-m6-phase1b-seg2`
- **subagent_tokens actual:** 229873
- **checkpoint reason:** executor's own early-checkpoint judgment call (context budget; step-3/step-4 boundary; green-building tree).
- **canonical resume-at pointer:** `phase-1b/step-4` (advanced from segment 1's `phase-1b/step-3` — no stall).
- **segment verdict:** `STATUS: PARTIAL`. **FRAGO 005 applied** (shape→shape_embed; fixed<T>→Check-2b compile error; step-4 false-positive-sweep line; Future-Req #11 fixed<T>-param-iteration ICE; sibling sweep). **Step 3 fix DONE** — extended the ONE classifier `crossing_local_names_with_cpu_spike` with `collect_aggregate_args_to_suspending_calls` (+ `mark_aggregate_arg`, walkers) in `check.rs` (≈7642-7900); LET-bound Shape/BuiltinFixed idents that escape to a suspending callee; params/loop-vars excluded; **ZERO codegen changes** (all consumers delegate through the one classifier — authoritative-derivation verified structurally). **IR before/after PASS**: dying-stack-alloca + `ptrtoint`-into-child pattern GONE for shape args → replaced by `getelementptr` into the surviving heap frame. Locking suite **4/4 GREEN** (was 0/4) incl. N=10 determinism gate + fixed<T>-escape-rejected; Phase 1 fixtures **a/b/c/d all GREEN**. **TWO NEW LIVE deviations surfaced (post-fix residual probes; OUTSIDE R13's signed shape+fixed<T> scope; NOT self-fixed):** (1) **anonymous struct-literal arg** to a suspending callee → garbage (no LET name for the classifier to mark); (2) **`number` arg** → 0.000 vs 2.5, and `maybe`/`union` share the same `value_to_i64_bits` by-pointer staging arm (`emit.rs:12323`, presumed same class). Non-LIVE (correct): indexed arg (typeck-rejected), loop-var shape arg (heap array storage survives). Routed to deviation-judge → Step-7 seam; see FRAGO 006 (pending). Steps 4–5 + close-out still open.

### 2026-07-10 — Phase 1c, segment 1
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1c-segment-1

- **segment number:** 1
- **session-id:** `executor-2026-07-10-m6-phase1c-seg1`
- **subagent_tokens actual:** 256275
- **checkpoint reason:** executor's own early-checkpoint judgment call (context budget; clean step-1/step-2 boundary; full-suite-green base).
- **canonical resume-at pointer:** `phase-1c/step-2` (first segment of Phase 1c — no prior pointer to stall-compare against).
- **segment verdict:** `STATUS: PARTIAL`. **Step 0 (FRAGO 008, M3e determinism) DONE + verified** — fixed `sleep(300)` race replaced with a REAL channel-barrier sync primitive (each task `done.send(1)`, parent `receive()`×3 before assert); race REMOVED not widened (guardrail honored); constructibility verified against `v0_3_m4_channel_composed.ynz` first; 10/10 isolated + full `cargo test --workspace` 522/522 GREEN under load (rejected a false-green pipeline-`tail`-exit trap and re-ran). Step 1 (CCIR-1) DONE — drift noted (`value_to_i64_bits` now `emit.rs:12401`). **DEVIATION SURFACED (routed to deviation-judge, NOT self-decided):** union frame-embed is STRUCTURALLY IMPOSSIBLE (none = NULL value-ptr via `build_is_null`, `emit.rs:3327-3350`; a frame-region ptr is never null → embedding breaks `is`-none) — plan step-3's prescribed "frame-backed via strategy-table arms" is falsified for union; proposed sound alternative (bind-time heap-cell promotion via `maybe_to_heap_cell` funnel + new `union_to_heap_cell`, one uniform mechanism, staging helper unchanged) recorded in `handoff-phase-1c.md`, unbuilt, awaiting ratification. Second deviation: Phase 1c has no `**CHECKPOINT**` marks despite the >5-step trigger (plan defect). Resume at `phase-1c/step-2` via handoff.

- **[CONDUCTOR FINDING — `conductor-2026-07-10-m6-exec2`, 2026-07-10] UNSANCTIONED Dockerfile modification —
  QUARANTINED, flagged for Patrick.** `git status` after Phase 1c seg1 surfaced `Dockerfile` dirty (` M`) —
  NOT dirty at Step-0 preflight, NOT in any phase's declared surface. Diff adds a `cargo-nextest` prebuilt
  install (`RUN curl -LsSf https://get.nexte.st/... | tar ...`) with prose referencing a "shared global
  gate-resolver (lib-gate-resolver.sh)" that prefers nextest over `cargo test` when present. Provenance
  UNCERTAIN: the Phase 1c seg1 executor's return explicitly listed Dockerfile as "untouched as ordered"
  (a discrepancy) — likely a global gate-resolver auto-patch or an unreported executor/tooling edit.
  **Disposition:** (1) Dockerfile ADDED to the known-not-mine never-stage set (alongside CLAUDE.md,
  ynz-watch/**) — it will NOT ride any boundary commit. (2) NOT reverted (a `git checkout` on Patrick's
  build infra is his call, not an unattended one). (3) **Verification integrity CONFIRMED UNCONTAMINATED:**
  every green-check dispatch + the Phase-1c executor ran `cargo test --workspace` explicitly (per their own
  reports + the dispatch commands), NOT nextest — so no M3e/concurrency verdict this session rode a
  process-per-test isolation profile that could mask a shared-process concurrency flake. (4) Concern for a
  CONCURRENCY milestone: nextest's process isolation changes the exact load profile that exposes flakes
  (the M3e class) — adopting it must be a DELIBERATE sanctioned decision, not a silent mid-flight edit.
  **FLAGGED FOR PATRICK (morning):** decide keep-nextest-deliberately vs revert. If it re-appears after a
  future gate run, that confirms an auto-patching infra behavior to address at its source.

### 2026-07-10 — Phase 1c, segment 2
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1c-segment-2

- **segment number:** 2
- **session-id:** `executor-2026-07-10-m6-phase1c-seg2`
- **subagent_tokens actual:** 235431
- **checkpoint reason:** planned `**CHECKPOINT**` mark (step-2/step-3 boundary; green-building tree; context past threshold).
- **canonical resume-at pointer:** `phase-1c/step-3` (advanced from segment 1's `phase-1c/step-2` — no stall).
- **segment verdict:** `STATUS: PARTIAL`. **FRAGO 013 applied** as first action (union/maybe → bind-time heap-cell promotion; `**CHECKPOINT**` marks added to Phase 1c). **Step 2 DONE — RED repros locked, Paper-Traced:** `v0_3_m6_maybe_arg_pure_call.ynz` (15-digit nondeterministic garbage vs `42`), `v0_3_m6_union_arg_pure_call.ynz` (`circle` 3/3 vs `square`, deterministic dangling-tag), `v0_3_m6_anon_struct_arg_pure_call.ynz` (`4240380` 3/3 vs `7`); 6 locking tests (value + N=10) RED / 12 pre-existing GREEN; all three constructible, none skipped. Tracked WHY-comment minor folded in. **Step-3 recipe settled (handoff):** `store_binding` (emit.rs:19836) = maybe promotion point; the union-ctor Let arm (emit.rs:12641-12691) mints a fresh `outer_slot` that CLOBBERS the sm_entry crossing alloca (the key gotcha); strategy table needs ZERO edits (Maybe/Union already default to ptr-alloca + default pointer flush); Check-2b provenance via a provenance-returning core in the ONE producer. Heap-cell loop-leak verdict must be RECORDED under step-3 parity gate, NOT assumed (handoff item 5b). Two pre-existing clippy warnings (integration.rs:672, cross_impl_consistency) will hit step-4 `-D warnings` — dispositioned there, not silently ridden. No new deviation. Resume at `phase-1c/step-3` via handoff.

### 2026-07-10 — Phase 1c, segment 3
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1c-segment-3

- **segment number:** 3
- **session-id:** `executor-2026-07-10-m6-phase1c-seg3`
- **subagent_tokens actual:** 257955
- **checkpoint reason:** **over-fat-step proposal** — step 3 ("The fixes") genuinely exceeds one window (2 crates, 8+ edit sites, 6 RED→GREEN, alloc-parity probe); the segment spent its window on step-3's MANDATED verify-recipe pre-work (which falsified a receipt) with no completed-step boundary behind it → BLOCKED bounce, routed to deviation-judge → Step-7 seam per the Step-3a pre-compare check.
- **canonical resume-at pointer:** `phase-1c/step-3` (UNCHANGED — a bounce; nothing landed. Expected/harmless: the pre-compare check intercepted this return BEFORE the stall compare, so the unchanged pointer never trips a stall; the next post-FRAGO segment advances over the newly-inserted sub-steps).
- **segment verdict:** **over-fat-step proposal** (carrier `STATUS: BLOCKED`). No code landed (by design of the bounce). Produced: settled step-3 design (handoff Decisions 11-16) + a proposed 4-part sub-step split (3a typeck routing / 3b codegen maybe+anon / 3c codegen union / 3d parity+IR), each boundary green-building on crate/mechanism seams. **Deviations surfaced (routed, not self-applied):** (1) the over-fat split; (2) RECEIPT FALSIFICATION (verify-before-fix) — `crossing_local_type_from_body` (emit.rs:8418) resolves the RHS type, so union-annotated lets classify as `Shape{variant}` (NOT the assumed default ptr-alloca) → folded into sub-step 3c; (3) Check 2 (nested-shape, check.rs:817-870) is a SECOND rejection surface the plan's "route PAST Check 2b" wording doesn't name but R14's "nothing rejected" implies → flagged for deviation-judge scope-reading confirmation. RED repros left intact (planned RED). Resume at `phase-1c/step-3` via handoff pending the FRAGO ratifying the split.

### 2026-07-10 — Phase 1c, segment 4
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1c-segment-4

- **segment number:** 4
- **session-id:** `executor-2026-07-10-m6-phase1c-seg4`
- **subagent_tokens actual:** 223844
- **checkpoint reason:** planned `**CHECKPOINT**` mark (3a boundary; green-building tree).
- **canonical resume-at pointer:** `phase-1c/step-3b` (advanced from seg-3's `phase-1c/step-3` — no stall).
- **segment verdict:** `STATUS: PARTIAL`. **FRAGO 014 applied** as first action (corrected 3a-3d split + CHECKPOINT marks + Check-2-named-alongside-2b incl. 2 sibling occurrences). **Step 3a DONE** (typeck only, zero codegen): `mark_aggregate_arg` widened for Maybe + anon (NOT Union — deferred to 3c); `crossing_local_names_with_provenance` core → `CrossingNames{names, arg_escape_only}` (one producer, wrapper byte-identical); Check 2b skip (arg-escape-only ∧ Maybe). **Decision 18 (authoritative-derivation catch):** the M3d decline-probe `suspension_guards_fire_for_fn` (check.rs:9270) RE-SPELLS Check 2b's predicate → mirrored the provenance+skip there too (else silent CPU-admission narrowing, twin-drift class); 3c must mirror its Union skip at BOTH touch points. **Exit verified:** builds; maybe repro compiles + UAF-RED (IR widen evidence); union FULLY RED + NOT misclassified (the corrected-split trap avoided — no ICE); fixed<T>+m3a rejections byte-identical green; integration 522/522, clippy `-p ynz-typeck` clean. Decision 17: no typeck surface for anon StructLit (recorded interpretation, exit holds). No new deviation. Resume `phase-1c/step-3b` via handoff.

### 2026-07-10 — Phase 1c, segment 5
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1c-segment-5

- **segment number:** 5
- **session-id:** `executor-2026-07-10-m6-phase1c-seg5`
- **subagent_tokens actual:** 227244
- **checkpoint reason:** planned `**CHECKPOINT**` mark (3b boundary; green-building tree).
- **canonical resume-at pointer:** `phase-1c/step-3c` (advanced from seg-4's `phase-1c/step-3b` — no stall).
- **segment verdict:** `STATUS: PARTIAL`. **Step 3b DONE** (codegen maybe+anon, emit.rs, zero typeck edits): `store_binding` (:19869) promotes a crossing-maybe envelope to a counted heap cell via `maybe_to_heap_cell`, crossing membership from the one threaded `cg.sm_crossing_names` (no second set); `stage_suspending_call_arg_bits` (:11285) routes `Expr::StructLit` args through `value_to_stable_bits` (all three loops by construction). Non-crossing/non-SM paths byte-identical. **Exit verified:** maybe pair + anon pair GREEN → **4/6 locked repros GREEN**; union pair stays RED with the byte-identical pre-fix `circle`-vs-`square` tell (3c's job, NOT a regression/ICE); fixed<T>+m3a rejections green; full workspace 2304 passed / 2 failed (the 2 = exactly the union pair); integration 522/522 under load; clippy `-p ynz-codegen` + fmt clean. No new deviation. Decisions 19-21 recorded (3c Union crossing-check pattern; default flush zero-edit for cell ptrs; StructLit routing safety). Resume `phase-1c/step-3c` via handoff.

### 2026-07-10 — Phase 1c, segment 6
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1c-segment-6

- **segment number:** 6
- **session-id:** `executor-2026-07-10-m6-phase1c-seg6`
- **subagent_tokens actual:** 266140
- **checkpoint reason:** planned `**CHECKPOINT**` mark (3c boundary; green-building tree).
- **canonical resume-at pointer:** `phase-1c/step-3d` (advanced from seg-5's `phase-1c/step-3c` — no stall).
- **segment verdict:** `STATUS: PARTIAL`. **Step 3c DONE — the hard union codegen, all 6/6 repros GREEN.** typeck: `mark_aggregate_arg` widened for Union; Check 2 + Check 2b + the M3d decline-probe `suspension_guards_fire_for_fn` ALL mirror the Union skip (Decision 18, both halves — nested-shape + unsupported-type); `find_let_annotation_type_in_stmts` exported (ONE annotation scanner, no twin). codegen: annotation override routes union-annotated crossing local to ptr-alloca not `shape_embed_set` (Decision 12 trap avoided); new `union_to_heap_cell` (null→null phi preserving `is`-none; tag-switched payload deep-copy, sizes from the one `shape_abi_sizes`); union-ctor Let arm stores the cell into the pre-created sm_entry alloca (clobber gotcha avoided); `store_binding` Union arm; KNOWN-HOLE doc refreshed (no `value_to_stable_bits` Union arm, Decision 15). **Exit verified:** 6/6 repros GREEN (union RED→GREEN, deterministic `square`, N=10); frame-backing suite 18/18; **full workspace 2306 passed / 0 failed**; integration 522/522 under load (FRAGO 008 holding); clippy `-p ynz-typeck -p ynz-codegen -D warnings` + fmt clean. `is`-none preserved (Decision 24). No new deviation; no RED repros remain. Remaining: 3d (loop alloc=free parity verdict + IR proof) + steps 4-5 (full gates, genuinely FIX integration.rs:672 unused-stderr + cross_impl_consistency collapsible-if, false-positive sweep, WHY-comment minor) + close-out (delete handoff). Resume `phase-1c/step-3d` via handoff.

### 2026-07-10 — Phase 1c, segment 7 (DONE)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1c-segment-7

- **segment number:** 7
- **session-id:** `executor-2026-07-10-m6-phase1c-seg7`
- **subagent_tokens actual:** 244901
- **checkpoint reason:** phase DONE (steps 3d + 4 + 5 + close-out all complete; handoff deleted as the final act).
- **canonical resume-at pointer:** n/a — Phase 1c COMPLETE (advanced from seg-6's `phase-1c/step-3d`).
- **segment verdict:** **DONE.** **Step 3d parity verdict — LEAK-BY-SHIPPED-DESIGN CONFIRMED (Paper-Trace, not assumed):** 5-iter crossing-maybe + 3-iter crossing-union loop probe; predicted gap 11 (5×1 maybe + 3×2 union cells) BEFORE first run; observed alloc=12/free=1, residual 0, stable 4/4. Pinned as `v03_m6_p1c_heap_cell_loop_parity_pins_documented_per_iteration_leak` (integration.rs, M5-E8 exact-gap convention `alloc = free + 11`). Per-case IR proof (maybe/union/anon all staged from a SURVIVING allocation, never a dying stack temp). **Step 5:** false-positive sweep extended to non-escaping maybe/union/anon (renamed test, all prior assertions retained, IR-verified zero promotion sites). **Step 4 gates:** full workspace **2307 passed / 0 failed** (run twice; M3e determinism holding under load both times); fmt + `clippy --workspace -D warnings` clean; the TWO pre-existing warnings GENUINELY fixed (integration.rs unused-`stderr` → strengthened assertion; cross_impl_consistency collapsible-if → `&&`) — no `#[allow]`, no `--no-verify`. WHY-comment minor confirmed already folded (seg-2). **DEVIATION SURFACED (for the seam — NOT self-decided):** the per-iteration heap-cell leak is a REAL finding (exact gap=11), fixing needs the ownership drop story (out of hotfix charter) — proposed four-field deferral: WHAT per-iteration heap-cell leak for crossing maybe/union LOOP bindings (1-2 cells/iter, held to process exit); WHY freeing needs the drop story (same class as M5 FRAGO-009 / Future-Req #6 — maybe/union joining the existing never-drop-locals regime); COST drop-story milestone (1-2 sessions) + update the 2 parity pins; TRIGGER drop story lands OR a real unbounded-suspension-loop-over-maybe/union workload. Exposure pinned LOUD in-suite meanwhile. Disposition = seam's call (fleet deviation-judge). Phase 1c completion note written to plan.md.
