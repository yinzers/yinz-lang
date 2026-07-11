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

- `conductor-2026-07-10-m6-exec2` — 2026-07-10 — **SESSION-END HANDOFF (conductor context deep →
  execution continues in a NEW chat via a fresh `/execute-plan --auto` cold-resume).** Paused at a
  CLEAN SEALED boundary (Phase 1c committed), deliberately NOT mid-phase — no half-built state to
  reconcile.
  - **COMMITTED (HEAD = `759bd9b`):** Phase 0 (`96bff95`) · Phase 1 (`f921efe`) · Phase 1b (`47abd29`,
    shape/fixed/number arg-UAF) · Phase 1c (`759bd9b`, maybe/union/anon arg-UAF + M3e determinism).
    **Reconcile caveat:** the Phase 0/1 commits carry a SPLIT-TRAILER (blank line before `Co-Authored-By`)
    so `%(trailers:key=Plan-Phase,valueonly)` returns EMPTY for them — they ARE real boundary commits
    (verified full-body). MY commits (1b `47abd29`, 1c `759bd9b`) carry PROPER single-paragraph trailers
    (accessor-legible `#1b`/`#1c`). A cold-resume must corroborate 0/1 by full-body read, not the accessor.
  - **NEXT PHASE = Phase 1d** (FRAGO 009 — the sibling decimal128-across-a-concurrency-boundary UAF class,
    EMPIRICALLY VERIFIED this session): (A) background-spawn decimal128 arg = **LIVE** silent-garbage UAF
    (`prepare_bg_arg_for_ctx` no Number arm, both spawn arms → `0.000`); (C) cpu-member decimal128 arg =
    **LIVE** compile-time ICE (`emit_cpu_member_spawn` i64/ptr mismatch). Both to fix. (B) conduit-send
    decimal128 = **VERIFIED-SAFE-BY-GATE** (channel<number> compile-gated, unreachable) → Future-Req #12,
    do NOT fix (dead code). **FIX APPROACH is Phase 1d's own step-1 DESIGN decision (route-not-design):**
    gate-consistently-with-the-existing-channel<number>-compile-gate (check.rs:3369-3398) vs eager i128
    heap-copy — the executor decides against IMP-concurrency, NOT pre-decided.
  - **REMAINING (plan Concept sequence):** 1d → 2 → 3 → 3b → 4 → 4b(P2-7) → 5 → 5b(P1-2) → 6 → 6b(sanitizer
    lane) → 7 → 8(roadmap durable-home lift). 16 phases total, 4 sealed.
  - **FRAGOs filed this session (all risk-neutral OR within already-signed R13/R14 — NONE tripped a fresh
    signature gate):** 008 (materialized M3e→1c step-0), 009 (sibling decimal128→Phase 1d), 010 (P2-7→Phase
    4b), 011 (P1-2→Phase 5b), 012 (durable-home lift→Phase 8), 013 (union heap-cell mechanism), 014
    (over-fat-step 3a-3d split + Check-2 naming), 015 (heap-cell loop-leak durable-home). Plus a mid-Phase-1c
    fix-loop (a hidden THIRD staging loop caught by code-reviewer in Phase 1b, fixed pre-seal) and a
    code-reviewer nested-shape finding REFUTED by empirical probe (stale pre-M5 premise).
  - **DELEGATED-SIGNATURE MECHANISM (Patrick's, this run):** on any risk-raising HIGH-residual signature
    gate the conductor may self-sign IF it (1) documents the FRAGO, (2) dispatches an INDEPENDENT sonnet
    agent to adversarially bias-review the self-sign, (3) proceeds ONLY on a clean bias-review (a "biased/
    unjustified" verdict HALTS for Patrick). **NEVER TRIPPED this session** — every deviation was
    risk-neutral or fell inside the already-signed R13 (shape) / R14 (maybe/union/number/anon) HIGH
    overrides. STILL IN FORCE for the next session (re-state it in the fresh chat).
  - **⚠ DOCKERFILE QUARANTINE — FLAGGED FOR PATRICK (decide keep-vs-revert):** an UNSANCTIONED
    `cargo-nextest` install was added to `Dockerfile` mid-session (provenance uncertain — likely a global
    gate-resolver auto-patch; the Phase-1c executor claimed it untouched). QUARANTINED to known-not-mine
    (never staged into any commit — verified excluded from `47abd29` + `759bd9b`); NOT reverted (a
    `git checkout` on Patrick's build infra is his call). Verification integrity CONFIRMED UNCONTAMINATED
    (every green-check + executor ran `cargo test`, NOT nextest). For a CONCURRENCY milestone nextest's
    process-isolation changes the flake-exposure profile — adopting it must be DELIBERATE. If it re-appears
    after a future gate run, that confirms auto-patching infra to fix at source.
  - **KNOWN-NOT-MINE (NEVER stage):** `CLAUDE.md`, `Dockerfile`, `crates/ynz-watch/src/{error,rebuild}.rs`.
  - **EXECUTOR MODEL OVERRIDE (re-state next chat):** **fable for ALL code execution**; reviewers/gates/
    recon unchanged (their normal models). Run mode **`--auto`** (host `gitleaks` clears the 8.0b commit
    guard).
  - **OPEN DEFERRALS (recorded, lifted to the roadmap at Phase 8 via FRAGO 012+015):** the per-iteration
    maybe/union heap-cell loop leak (Future-Req #13, pinned loud via `v03_m6_p1c_heap_cell_loop_parity_*`),
    plus the pre-existing Future-Reqs (P2-3, channel-close, preemption, background.cpuBound, the 2 ICEs,
    dynamic-dispatch #10, fixed<T> param-iter #11, conduit-send-number #12). Gitignored probe scratch left
    for reference under `target/scratch-1c*/` and `target/probe_*` (safe to clean).

### `executor-2026-07-10-m6-phase1d` — 2026-07-10 — Phase 1d segment 1 (steps 1–2), PARTIAL checkpoint

- **Step 1 DONE.** CCIR-1: all FRAGO-009 citations re-verified against the live tree (content
  matches; line drift recorded in `handoff-phase-1d.md` receipts — `prepare_bg_arg_for_ctx` now
  emit.rs:15616, cpu-member i64 load :9686-9697, channel gate check.rs:3417-3451). Design decision
  made and recorded as **plan.md Recorded Decision D8**: **Option 2 — eager decimal128 heap-copy at
  the spawn boundary** (channel<number> gate untouched; Future-Req #12 unchanged). Load-bearing
  reasons: IMP-concurrency's background-args give/copy design contradicts a rejection; Option 1 is
  structurally unavailable for defect C (cpu-member spawn is auto-promotion — a teaching error on
  code the user never opted into is architecturally wrong, and an admission decline is a silent
  permanent de-parallelization); Phase 1b's signed-R14 "number WORKS across suspension — NOT
  rejected" precedent; the "hard machinery" framing dissolved on re-read (MapEntry pre-gate
  precedent + existing heap-cell core family + both spawn arms' balanced free ladders).
- **Step 2 DONE.** Four RED repros committed-shape + harness
  `crates/ynz-driver/tests/v03_m6_number_spawn_boundary.rs` (8 tests: value + N=10 determinism per
  repro), **all 8 confirmed RED for the documented reasons** on the pre-fix tree: A (bg-spawn, CPU
  and SM arms) Paper-Traced `0.000000...` vs staged 2.5/4.5, 3/3 deterministic per arm; C
  (cpu-member, pure-spike AND fused) verbatim LLVM verifier ICE `Call parameter type does not match
  function signature! ... call ptr @heavyGrow(i64 %spike_arg)` surfaced as "compiler bug".
  Repro-shape discovery recorded in the handoff: a bare fire-and-forget last-statement spawn does
  NOT reproduce (CPU arm reads the not-yet-clobbered dead alloca; SM arm's task is cancelled at
  shutdown) — the proven shape suspends the spawner after the spawn (`wait sleep(60)`).
- **Checkpoint (context budget):** conductor-authorized PARTIAL at the step-2→3 boundary (the
  Phase-1 seg-1 precedent: RED class locked, fix next). Tree green-building: builds clean; the only
  reds are the 8 locked Phase-1d RED repros (documented planned RED, step 3 on record to green
  them). Handoff `handoff-phase-1d.md` written (settled step-3 blueprint: one
  `number_to_heap_cell` mechanism; `prepare_bg_arg_for_ctx` unconditional Number pre-gate returning
  `HeapShape{16}`; `callee_takes_bare_number` predicate consumed by both `emit_cpu_member_spawn`
  and `build_cpu_trampoline`; free-after-pack ordering invariant; one OPEN probe item —
  IntLit-vs-number-param typecheck reachability). Resume-at **`phase-1d/step-3`**. Nothing
  staged/committed (conductor seals). KNOWN-NOT-MINE (CLAUDE.md, crates/ynz-watch/**, Dockerfile)
  untouched.

### `executor-2026-07-10-m6-phase1d-seg2` — 2026-07-10 — Phase 1d segment 2 (step 3 + OPEN probe), PARTIAL checkpoint

- **Step 3 DONE.** Option-2 (D8) fix implemented in `crates/ynz-codegen/src/emit.rs` via ONE
  authoritative mechanism (authoritative-derivation): new `Cg::number_to_heap_cell` (16-byte counted
  cell, load i128 → store; beside `shape_bytes_to_heap_cell`). **Defect A (both arms):** an
  UNCONDITIONAL `Type::Number{precision<=34}` pre-gate in `prepare_bg_arg_for_ctx` (after the MapEntry
  pre-gate) returns `(cell, HeapShape{16})` — shared by the CPU-spawn arm (`ynz_rt_spawn_blocking`,
  free via `emit_bg_arg_frees` HeapShape) and the SM-spawn arm (`ynz_rt_spawn`, free via
  `BgArgDropEntry` kind-0); SM child-side read unchanged (`load()`'s `sm_number_param_set` indirection
  now derefs the heap cell). **Defect C:** new `callee_takes_bare_number` predicate (first-param twin
  of `callee_returns_bare_number`) consumed by BOTH `emit_cpu_member_spawn` (Ident number arg → read
  via authoritative `lower_expr`/`load()` → `number_to_heap_cell` → ptr_to_int ctx word; new `callee`
  param, both callers pass `&child.callee`) AND `build_cpu_trampoline` (int_to_ptr the ctx word → pass
  the pointer to the callee; free the cell AFTER result packing — the identity-callee aliasing
  invariant). One alloc / one free.
- **OPEN probe RESOLVED (verify-before-fix, scratch fixture + `ynz run`).** `grow(5)` (int literal →
  `number` param) **typechecks** (typeck coerces IntLit→Number{34}, check.rs:2224 via the hint at
  :3813) but **ICEs at codegen EVEN SYNCHRONOUSLY**: `Call parameter type does not match function
  signature! ... call ptr @grow(i64 5)`. Root cause: `lower_expr(IntLit)` yields an i64 (emit.rs:14514)
  and NO call site coerces int→number (normal call loop :14986-14990). So int-literal-to-`number`-param
  is a **pre-existing GENERAL codegen gap orthogonal to FRAGO 009's concurrency-boundary charter** — it
  is NOT fixed here, and the cpu-member IntLit sub-case is deliberately guarded to a clean compile error
  (never a new segfault from `int_to_ptr(5)`) rather than given a partial fix that would make
  `background f(5)` work while `f(5)` stays broken. **SURFACED as a deviation** for the deviation-judge
  → conductor seam (candidate Future-Requirements deferral; NOT self-decided, NOT absorbed).
- **Verification (my new work, unconditional).** `cargo build -p ynz-driver` clean; the 8 Phase-1d RED
  repros (`v03_m6_number_spawn_boundary.rs`) now **8/8 GREEN** incl. N=10 determinism per repro
  (value + determinism arms, A CPU/SM + C pure/fused). Scratch probe fixture removed.
- **Checkpoint (context budget) at the step-3→4 boundary.** Tree green-building (builds clean; no
  planned RED remaining — the 8 repros are green). Handoff `handoff-phase-1d.md` rewritten in place;
  resume-at **`phase-1d/step-4`** (full `cargo test --workspace` + clippy `-D warnings` + `fmt --check`
  + false-positive sweep, then step 5 Future-Req #12 confirmation, then completion note + handoff
  delete). Nothing staged/committed (conductor seals). KNOWN-NOT-MINE untouched.

### 2026-07-10 — `executor-2026-07-10-m6-phase1d-fixloop1` — Phase 1d fix-loop round 1 (post-review should-fixes)

Correctness already verified clean by code-reviewer (0 blockers); this round addressed the fleet's
should-fix cluster. (1) **Guard diagnostic rerouted** — the IntLit→`number` cpu-member guard's raw
`Err(String)` (which hit `queries.rs:421`'s generic "compiler bug" ICE wrapper, leaking "FRAGO 009"
+ a `{span:?}` dump) replaced by a typeck-level WHAT/WHAT-INSTEAD/WHY teaching error in
`check_user_fn_call` (`crates/ynz-typeck/src/check.rs`), mirroring the `channel<number>` gate — the
codebase's established home for known-limitation compile errors (codegen has NO user-facing
diagnostic path; investigated `queries.rs`/`build.rs` wrappers + emit.rs before deciding). Fires
uniformly on every call site (`f(5)` synchronous AND spawn boundary — inconsistent-teaching-story
concern moot). Codegen arm kept as an unreachable internal backstop (offsets, no FRAGO id).
(2) **Guard tested, RED→GREEN** — RED reproduced live against the pre-change binary (ICE banner +
FRAGO leak verbatim); new fixture `v0_3_m6_int_literal_number_arg_cpu_member.ynz` (admitted
spike-group, `heavyGrow(5)`) + test
`v03_m6_int_literal_to_number_param_cpu_member_is_clean_teaching_error` (non-zero exit, no hang,
teaching message present, NO "compiler bug", NO "FRAGO"). (3) **Narrative overclaims corrected** —
plan.md completion note rewritten (sweep = code-path disjointness only; no in-boundary
false-positive case exists by design under the unconditional pre-gate); the seg-3 entry annotated
in place (below); the IR proof PERSISTED as committed test
`v03_m6_non_escaping_number_ir_stays_by_pointer` (zero heap-cell markers + by-pointer `@bump(ptr`
pinned). (4) **FRAGO 016 filed** — #14 ratified (fix routed to the
`2026-07-04-v0-3-hotfix-int-literal-number` stub plan, expansion = conductor→human coordination,
stub NOT edited); #15/#16 polish minors filed four-field; Phase 8 lift-list amended to carry
#14/#15/#16. Gates: full-workspace nextest + clippy `-D warnings` + fmt (results in the return).
Observation (not fixed, pre-existing, shared renderer): the diagnostic renderer's printed line/col
maps byte offset 950 (line 31 col 21, verified) to a displayed `:32:2` — an off-by-one in the
DISPLAY mapping common to all diagnostics, not introduced by this change; noted for a future
diagnostics-touching milestone. Did NOT commit/stage — conductor seals the boundary.

### 2026-07-10 — `executor-2026-07-10-m6-phase1d-fixloop2` — Phase 1d fix-loop round 2 (post-review should-fixes)

Correctness verified clean (0 blockers, all 5 reviewers); this round addressed three should-fixes.
(1) **Guard extended to ALL call forms (verify-then-fix)** — code-reviewer proved round 1's "every
call site uniformly" claim FALSE: UFCS `p.scale(5)` and generic-concrete-param `scale(5, tag)` both
bypassed the `check_user_fn_call`-only gate and ICE'd. Both locked RED pre-fix (new fixtures
`v0_3_m6_int_literal_number_arg_{ufcs,generic_fn}.ynz`; UFCS: LLVM verifier reject
`call ptr @scale(ptr %p1, i64 5)` → "This is a compiler bug.", exit 1; generic: IntValue-vs-
PointerValue panic at `emit.rs:20176` → panic banner, exit 2). Fix: gate extracted into ONE shared
`reject_int_literal_number_arg` helper (`check.rs`, authoritative-derivation) consumed by all three
arg loops — `check_user_fn_call`, `check_method_call`'s shape/UFCS arm (args now threaded through
its signature; non-receiver args zipped against `sig.params[1..]`), and `check_generic_fn_call`
(gate BEFORE the discarded `unify_param`). RED→GREEN: byte-identical teaching diagnostic across all
three forms (non-oop.md dual-style convention restored), pinned by
`v03_m6_int_literal_to_number_param_{ufcs,generic_fn}_is_clean_teaching_error` + the round-1
plain-form test refactored onto one shared assertion helper. False-positive key unchanged:
exactly `(Type::Number, Expr::IntLit)`. `emit.rs` backstop comment + plan.md round-1 note +
Future-Req #14 all corrected to the now-true uniform claim. (2) **Future-Req #17 filed (FRAGO
017)** — the trampoline staged decimal128 arg-cell shutdown-drop leak (round-1's "noted, not fixed
here" comment at `emit.rs:~9708`) formalized four-field + added to Phase 8's FRAGO-012 lift-list;
the code comment now points at #17. (3) **IR-proof test hardened** (test-quality) — positive
controls added: each negative marker asserted PRESENT in a boundary fixture's IR (probe-verified:
`bg_number`/`_num_ld` in the background-CPU fixture; `_num_bits_`/`spike_num_arg_ptr` in the
cpu-member fixture), inert `number_to_heap_cell` dropped (Rust fn name, never IR), `spike_num_free`
probe-proven ALSO inert (void-call names dropped by LLVM) and rejected as replacement. Gates:
full-workspace nextest + clippy `-D warnings` + fmt (results in the return). Did NOT commit/stage —
conductor seals the boundary.

### 2026-07-10 — Phase 1d fix-loop round 3 — the guard COMPLETION sweep (FRAGO 018)

Three segments: `executor-2026-07-10-m6-phase1d-fixloop3` (seg 1, enumeration),
`executor-2026-07-10-m6-phase1d-fixloop3-seg2` (seg 2, RED lock + implement),
`executor-2026-07-10-m6-phase1d-fixloop3-seg3` (seg 3, gates + close-out). Human-directed SCOPE
EXPANSION over defer, justified by enumeration finding real user-reachable danger beyond rounds 1–2's
three call-argument forms.

**Seg 1 (enumeration).** Exhaustive slot enumeration DONE with a grep-completeness argument (27
`infer_expr(_, Some())` sites classified; no `Paren` AST node; zero `number`-param intrinsics; all
named/cross-module calls route through the gated fns; `channel<number>` construction-gated). Root
mechanism: the literal-hint rule (`check.rs:2223-2227`) types an IntLit hinted `Number{34}` as number
→ codegen lowers a raw i64. 25-row slot table; new confirmed-RED slots beyond the 3 gated include
`array<number>.add(5)` **SEGFAULT (exit 139)** and `contains(5)` **SILENT wrong `false` (exit 0)**.
Fix recipe settled. Zero code landed (PARTIAL, tree byte-identical to HEAD `759bd9b`).

**Seg 2 (RED lock + implement).** 24 committed RED fixtures (`v0_3_m6_int_literal_number_*.ynz`, each
header recording its probe-confirmed pre-fix signature) + 25 committed tests (24 slot tests + false-
positive sweep) in `crates/ynz-driver/tests/v03_m6_number_spawn_boundary.rs`. Implemented the extended
shared gate + all three mechanisms (all `crates/ynz-typeck/`, authoritative-derivation — one gate, no
per-form twins): `reject_int_literal_number_arg` → thin wrapper over role-parameterized
`reject_int_literal_number_slot(NumberSlotRole, …)` (adds a `-IntLit` `UnaryOp{Neg, IntLit}` arm);
the generic case moved to ONE post-arg-loop `apply_substitution` pass (concrete + explicit
`pass<number>(5)` + sibling-bound `pick(price, 5)`, unresolved TypeParams never match Number); the
one authoritative `collection_method_arg_slots(receiver, method)` table in `builtins.rs` drives one
gate loop over collection-method element positions; hinted construction/statement slots (struct-lit
field + map-hint twin, array/fixed literal elements, map-literal key+value, index assigns incl. the
map KEY, map bracket-key READ, field assign, `return`, multi-case-if arm pattern) gate through the
slot fn. All 24 fixtures flipped RED→GREEN; targeted nextest 29/29.

**Seg 3 (gates + close-out, this entry — DONE).** Step-4 full gates ALL GREEN:
`cargo nextest run --workspace` **2344 passed / 0 failed** (exit 0; M3e `cross_impl_consistency` both
GREEN under load, no flake fired — `long_session` RSS / `contract_3_wait_inside_if` /
`spawn_panic_ctx_no_leak` none tripped); `cargo clippy --workspace -- -D warnings` clean;
`cargo fmt --all --check` clean. No `#[allow]` / `--no-verify` / weakened test. **Step 5 close-out:**
covered-set narrated as COMPLETE for the IntLit / `-IntLit` → `number` argument / construction /
statement class, store-site #9 (`let x: number = 5`, signed-stub territory) excepted — Phase 1d
completion note gains the round-3 sub-note, Future-Req #14 updated, `emit.rs` backstop comment near
`emit_cpu_member_spawn` updated to name the shared slot gate + true covered set. **FRAGO 018 filed**
(this dispatch) recording the scope expansion + the FOUR surfaced deviations (enumeration-completeness
map-bracket-KEY amendment fixed in-class; NEW decimal128 by-value RETURN garbage; NEW `map<number,V>`
real-key silent breakage; the unchanged concat/pick + `array.remove` gaps) — SURFACED for the
deviation-judge → conductor seam, NOT self-adjudicated. Session-ids for all three fixloop3 segments
appended to the frontmatter chain WITH this entry (seg 1 and seg 2 were both absent — appended here to
keep the chain consistent with the plan's own precedent that every PARTIAL executor segment is
recorded). Handoff `handoff-phase-1d.md` DELETED as the final act. Did NOT commit/stage — conductor
seals the boundary.

- `executor-2026-07-10-m6-phase1d-fixloop4` — 2026-07-10 — **Phase 1d fix-loop round 4 — DONE.**
  Closed TWO confirmed honesty-gaps in round 3's "COMPLETE" claim plus formalized deferrals; small,
  precise round (round 3 NOT re-swept). **FINDING 1 (code fix).** `return 5` from a `-> number errors`
  fn missed the round-3 teaching gate — Paper-Trace verified against the live tree: pre-fix it emitted
  the GENERIC mismatch (`return produces int, but this function must return number errors`, exit 1)
  while the plain `-> number` control taught (`Write it as a decimal literal: 5.0`); root cause — the
  return type is `Type::ErrorsCapable{ inner: Number }`, so the shared gate's
  `matches!(expected_ty, Type::Number{..})` was FALSE through the wrapper and the return-site consumer
  (`check.rs:2170`) fell through to the generic path (`check.rs:2185`). Fix: unwrap `ErrorsCapable`
  before the `Type::Number` check inside `reject_int_literal_number_slot` (`crates/ynz-typeck/src/check.rs`)
  — ONE gate, no parallel path (authoritative-derivation). Post-fix Paper-Trace: errors-return now
  teaches (exit 1); plain `-> number` UNCHANGED; `return 5` from `-> int errors` stays clean (exit 0);
  `return 5.0` from `-> number errors` stays clean (exit 0). ONE RED fixture
  (`v0_3_m6_int_literal_number_return_errors.ynz`, header records the pre-fix generic signature) + ONE
  test (`v03_m6_int_lit_number_return_errors_is_teaching_error`) appended to
  `crates/ynz-driver/tests/v03_m6_number_spawn_boundary.rs` via the existing slot-test macro (asserts
  the teaching text, not merely exit≠0). **FINDING 2 (doc only, NO code).** `hidden cache: number = 5`
  ICEs — Paper-Trace confirmed: `Found IntValue(i64 5) but expected PointerValue variant` at
  `emit.rs:20351`, from the field-default lowering site `emit.rs:18233` calling `lower_expr` with no
  type hint → raw i64 → `store_field` into a decimal128 slot. This is STORE-SITE #9 class, structurally
  identical to `let x: number = 5`; NOT gated (gating one store site while the other stays open would
  make the class inconsistent) — carved out of the "COMPLETE" claim and folded into #9/#14's coercion
  stub-plan scope. **Doc corrections:** (a) `check.rs` gate doc-comment + Future-Req #14 + Key Outcome
  1d corrected — guard complete for the class INCLUDING the errors-wrapped return, with BOTH
  declaration-site store facets (`let x: number = 5` AND `hidden f: number = 5`) carved out; (b)
  hidden-field-default folded into #9 (records linkage to the `2026-07-04-v0-3-hotfix-int-literal-number`
  stub plan; did NOT edit the stub plan — conductor→human coordination item); (c) three deviation-judge
  should-fixes promoted to four-field Future-Reqs #18 (sync decimal128 by-value RETURN garbage), #19
  (`map<number,V>` real-number-key silent breakage), #20 (general UFCS number→int arg-validation gap +
  `array.remove` no codegen lowering); (d) Key Outcome 1d amended with the headline-vs-delivered
  reconciliation (scope grew from A/C to the near-compiler-wide IntLit→number rejection guard), and its
  unqualified stale `check.rs:3369-3398` citation corrected to the live `:3417-3451` (an already-recorded
  fixed fact whose headline sibling was never swept — plan-source-of-truth sibling sweep; the historical
  step-text/D8 citations are deliberately-preserved originals whose drift #12 already documents, left
  untouched). **FRAGO 019 filed** (this dispatch). **Gates ALL GREEN:** `cargo nextest run --workspace`
  **2345 passed / 0 failed** (exit 0; +1 vs round 3's 2344 = the new errors-return test; M3e
  `cross_impl_consistency` both GREEN under load at 285s/304s, no flake fired — `long_session` RSS /
  `contract_3_wait_inside_if` / `spawn_panic_ctx_no_leak` none tripped); `cargo clippy --workspace -- -D
  warnings` clean; `cargo fmt --all --check` clean. No `#[allow]` / `--no-verify` / weakened test. Did
  NOT touch the dirty-not-mine files (`CLAUDE.md`, `Dockerfile`, `ynz-watch/src/{error,rebuild}.rs`). Did
  NOT commit/stage — conductor seals the boundary.

- `executor-2026-07-10-m6-store-site-stopgap` — 2026-07-10 — v0.3-M6 store-site stopgap (human-directed
  "no duct tape", FRAGO 020) — **DONE.** Closed the two LIVE raw-ICE store-site exposures Phase 1d left
  un-gated: `let x: number = 5` (local binding) and `hidden f: number = 5` (field default) now REJECT with
  the shared int-literal→`number` teaching error instead of the "Yinz compiler bug" ICE banner — the
  int→number COERCION stays deferred to the `2026-07-04-v0-3-hotfix-int-literal-number` stub. Verify-before-fix
  Paper-Trace (pre-fix ICE at `emit.rs:20268`/`:20436` → post-fix teaching error, controls clean) in FRAGO 020.
  Extended the ONE shared `reject_int_literal_number_slot` gate (new `NumberSlotRole::StoreBinding` for the
  binding, REUSED `NumberSlotRole::Field` for the field default — authoritative-derivation, no twin); 2 RED
  fixtures + 2 slot tests; re-pointed the #9 store-site control comment AND the `int_literal_retypes_as_number_
  with_annotation` typeck control (renamed to `..._store_binding_is_rejected_with_teaching_error`) to the new
  intended behavior + added a decimal false-fire guard. Cross-plan doc reconciliation (M6 plan + stub plan +
  both audit.md, whole-plan sibling sweep). Gates FULL GREEN: nextest --workspace 2349 passed / 0 failed / 6
  skipped (+3 vs 2346 baseline); clippy clean; fmt clean. Deviation surfaced: roadmap row 441/497 "panics"
  phrasing now stale (surfaced for the seam, not self-edited). Did NOT commit/stage — conductor seals.

- `executor-2026-07-10-m6-store-site-stopgap-fixloop1` — 2026-07-10 — store-site stopgap **FIX-LOOP round 1**
  — **DONE.** Closes the ONE confirmed code-review BLOCKER: the new hidden-field-default gate over-reached and
  rejected VALID generic code. Root cause (empirically confirmed): the gate loop in the `ShapeDecl` arm of
  `check_module` (`check.rs:481`) called `ast_type_to_type(&field.ty)` where `type_param_scope` is EMPTY (it is
  only populated in `check_generic_function_body`), so a hidden field whose type references a generic type
  param fell through to the unknown-type path and emitted a spurious `` `T` is not a known type `` — the exact
  trap the adjacent original code (`check.rs` ~453-462) deliberately avoids with the diagnostic-free
  `collect_referenced_names_in_ast_type` walker. **Fix (`check.rs`, hidden-field gate arm):** guard on the raw
  AST type before resolving — `if !matches!(&field.ty, AstType::Number { .. }) { continue; }` — so the gate
  resolves/fires ONLY for a literal `number` annotation and never walks a generic param (`number` is banned from
  aliasing, so a `number` slot is always the literal `AstType::Number`; loses nothing — `maybe<number>` etc.
  already fall through the gate, which matches only bare `Type::Number`). **Paper-Trace (verify before/after):**
  regression case `shape Cache<T> { label: string; hidden buffer: array<T> = [] }` — Observed pre-fix: `Error: `T`
  is not a known type` (exit 1, valid code REJECTED); Expected: clean compile; post-fix: compiles + runs clean
  (exit 0, "generic-hidden-field-ok"), Residual 0. Stopgap facets all still hold post-fix: `hidden cache: number
  = 5` STILL teaches (int-literal→`number` error, no ICE, exit 1); `hidden cache: number = 5.0` stays clean
  (exit 0, "5.0"); `let x: number = 5` STILL teaches (exit 1); `let x: number = 5.0` stays clean (exit 0).
  `check_let` NOT touched — verified its gate runs inside `check_generic_function_body` where `type_param_scope`
  IS populated (a `let x: T = ...` resolves `T` correctly via line 4870), so it carries no analogous exposure.
  **New POSITIVE regression fixture + test:** `crates/ynz-driver/tests/fixtures/v0_3_m6_int_literal_number_generic_hidden_field_ok.ynz`
  (generic shape + type-param-typed hidden field, declaration-site — the over-reach fired at shape-declaration
  time independent of construction; generic-shape *construction* via literal is a separate unshipped feature, so
  the fixture is declaration-scoped to match the exact reproduction) + test
  `v03_m6_generic_type_param_hidden_field_compiles_clean` (in `v03_m6_number_spawn_boundary.rs`) asserting exit
  0, no `is not a known type`, byte-exact stdout. **Gates FULL GREEN:** `nextest --workspace` 2350 passed / 0
  failed / 6 skipped (= 2349 baseline + 1 new positive regression test); `clippy --workspace -- -D warnings`
  clean; `fmt --all --check` clean. No `#[allow]`, no `--no-verify`, no weakened test. Touched ONLY `check.rs` +
  new fixture + `v03_m6_number_spawn_boundary.rs` + this `audit.md`. Did NOT commit/stage — conductor seals. See
  the `maybe<number>` probe note below.
  - **PROBE (record-only, NOT gated this round — sibling coverage gap):** the reviewer flagged `let x: maybe
    number = 5` as possibly un-gated. Probed with corrected syntax (`maybe number` with a space is itself a
    parse error — "`maybe` requires a type argument"; the valid form is `maybe<number>`). Result for `let x:
    maybe<number> = 5`: does **NOT ICE** — it is cleanly REJECTED by the general assignment type-mismatch
    diagnostic (`This value is `int`, but `x` is declared as `maybe<number>``, exit 1), NOT the tailored
    int-literal→`number` teaching error (the shared gate matches only bare `Type::Number`, so a `maybe<number>`
    slot falls through it). No crash, no ICE — so no four-field deferral trigger. **Candidate Future-Req note
    (sibling of the int-literal→`number` gate, pre-existing, uniform across the gate, NOT introduced this
    round):** the int-literal→`number` teaching gate does not extend to `maybe<number>` slots — an int literal
    there gets the generic type-mismatch instead of the tailored decimal-literal teaching guidance; purely a
    teaching-quality coverage gap, safe (no ICE). Routed to the conductor→seam as a candidate coverage-gap note;
    NOT self-adjudicated, NOT gated (different type slot, scope discipline). (Aside, orthogonal: even `let x:
    maybe<number> = 5.0` is rejected — `number` is not auto-wrapped into `maybe<number>` — a separate,
    pre-existing maybe-wrapping matter, out of scope, noted only so the probe result isn't misread.)

- `executor-2026-07-10-m6-phase3-seg3` — 2026-07-10 — Phase 3 segment 3 — **PHASE 3
  DONE** (step 7 full-workspace gates + close-out; steps 1-6 landed segments 1-2, see the
  Context-segment log Phase 3 entries + FRAGO 021). Receipts inherited from
  `handoff-phase-3.md`, delta-verified: `git diff 759bd9b..46906d1 -- crates/ynz-runtime/`
  empty, so segment-1/2 orientation receipts stand unmodified; own new work verified
  unconditionally via the full gates. **Step 7 gates ALL GREEN:**
  `cargo nextest run --workspace` (dev container) — **2354 passed / 0 failed / 6 skipped,
  exit 0** (= 2350 stopgap-fixloop1 baseline + 4 new Phase 3 tests); the 4 new tests
  (`m6_pending_send_aba::cancelled_frame_sender_…`, `…::freed_handle_sender_…`,
  `channel::tests::purge_pending_sends_is_idempotent_and_gen0_is_reserved`,
  `channel::tests::same_token_different_generation_never_collides_and_stale_is_swept`)
  confirmed in the workspace run set via `nextest list` and GREEN both in the full run and a
  targeted 4/4 confirmation run; ZERO flakes (known transients `long_session` RSS /
  `contract_3_wait_inside_if` / `spawn_panic_ctx_no_leak` all passed first attempt — no
  isolated re-run needed); M4 channel/handle suites regression-free. House
  `cargo clippy --workspace -- -D warnings` exit 0 (documented gate shape; the 2 pre-existing
  `--all-targets`-only Phase-1d-era lints at `tests/m2_runtime.rs:275` + `lib.rs:2830`
  untouched, not `#[allow]`'d). `cargo fmt --all --check` exit 0. No `#[allow]`, no
  `--no-verify`, no weakened test; nothing touched outside plan/audit this segment (the fix
  itself is segments 1-2's uncommitted diff in `crates/ynz-runtime/src/{channel.rs,runtime.rs,
  handle.rs,lib.rs}`). Close-out: Phase 3 completion note written to plan.md (after the
  phase's Model-tag line, Phase 1c/1d convention); session-id appended to the frontmatter
  chain in the same action; `handoff-phase-3.md` DELETED as the final act. Did NOT commit/
  stage — conductor seals. FRAGO 021 pends boundary deviation-judge ratification (recorded,
  not adjudicated).

- `executor-2026-07-10-m6-phase3-fixloop1` — 2026-07-10 — Phase 3 review fix-loop 1 — **both
  converged should-fixes CLOSED** (2 reviewers, 0 blockers, 2 should-fixes). **Should-fix 1
  (gen-0 unprotected class): ELIMINATE path taken** — verify-before-fix Paper-Trace enumerated
  every generation producer (mint sites `runtime.rs` `ynz_rt_spawn` + test ctor, `handle.rs`
  child `task_gen` + handle `send_gen` — all NONZERO from the counter starting at 1; gen 0
  reached the key ONLY via the `ynz_channel_send_poll` thread-local read with no `TaskGenGuard`
  active, i.e. every `SyncStateFnFuture` drive) and found the premise WIDER than the reviewers
  stated: not one immortal entrypoint but EVERY sync drive (`main` wrapper + non-entry sync
  wrappers from non-SM contexts, one shared `ynz_rt_run_entrypoint` driver) shared gen 0.
  Fix: `SyncStateFnFuture` now carries its own `task_gen` minted at construction and enters
  `TaskGenGuard` in `poll` — uniform nonzero stamping across all three producers, protection
  compiled into ALL builds (release trading mount included), zero ABI/codegen delta; gen-0
  docs/comments in `channel.rs` reconciled (gen 0 = bare unstamped test calls only; purge
  no-op floor retained). Nothing deferred — no four-field deferral. **Should-fix 2:** new
  deterministic handle-seam test
  `handle::tests::handle_send_same_address_different_generation_never_collides` (real
  `ynz_handle_send_poll` mint, same address, two explicit generations, purge withheld; a broken
  salt delivers the dead 111 and fails the 222 assertion — non-tautological); `lib.rs` repro
  fallback note repointed. **Gates:** nextest workspace 2355/0 failed/6 skipped exit 0 (2354
  baseline + 1 new; 5/5 ABA-suite targeted run green); clippy `-D warnings` exit 0; fmt exit 0.
  First full run had ONE failure — pre-existing `ynz-typeck::symbol_lookup::
  test_cross_file_reference_count_estimate_completes_fast` (<5ms wall-clock assert), passed in
  isolation + full rerun: non-recurring load transient in an untouched crate, surfaced NOT
  silenced. Design-doc check: IMP-concurrency.md (cancel sections) + IMP-no-function-coloring.md
  silent on the runtime-internal generation scheme — no contradiction. Touched ONLY
  `crates/ynz-runtime/src/{runtime,channel,handle,lib}.rs` + plan.md (step-5 + completion-note
  parentheticals reconciled via sibling sweep; fix-loop paragraph) + this audit.md. FRAGO 022
  filed this dispatch (same-dispatch seam); pends boundary deviation-judge ratification. Did
  NOT commit/stage — conductor seals.

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

### FRAGO 016 — 2026-07-10 — session-id: `executor-2026-07-10-m6-phase1d-fixloop1`

- **Trigger.** Phase 1d boundary fan-out: seg-2 surfaced (seg-3 recorded) the IntLit→`number`-param
  CALL-SITE coercion gap as candidate Future-Req #14 — explicitly NOT self-adjudicated, routed to the
  deviation-judge → conductor seam. Separately, the fleet review flagged a should-fix cluster on the
  interim guard: (a) its rejection rendered the generic "compiler bug" ICE banner for a KNOWN
  user-triggerable limitation, leaked the internal ID "FRAGO 009" and a raw `{span:?}` Debug dump into
  user-facing text (rules-compliance + 2 others); (b) the guard was untested new code (test-quality /
  verification.md); (c) two narrative overclaims (unpersisted "IR-level proof"; false-positive-sweep
  wording claiming boundary discrimination a zero-spawn fixture cannot prove); (d) two code polish
  minors (twin-scan plumbing; bare 16-byte cell-size literals).
- **Corroboration.** deviation-judge ruled the #14 scope-out JUSTIFIED / risk-neutral (pre-existing
  general coercion gap, ICEs synchronously with zero concurrency, orthogonal to FRAGO 009's charter;
  boundary-only fix would ship an inconsistent teaching story). Fix-loop round 1 reproduced the guard's
  RED live (pre-change binary: ICE banner + FRAGO leak verbatim) before rerouting it.
- **Classification.** JUSTIFIED / RISK-NEUTRAL (deferral formalization + review should-fixes; same
  shape as FRAGO 015/#13). No signature gate trips.
- **Delta (applied to plan.md by `executor-2026-07-10-m6-phase1d-fixloop1` in this same dispatch).**
  (a) Future-Req #14 RATIFIED as a four-field deferral; FIX routed to the existing stub plan
  `2026-07-04-v0-3-hotfix-int-literal-number`, to be expanded to cover BOTH #9's store-site and #14's
  call-site facets under ONE coercion mechanism — the cross-plan stub expansion is a conductor→human
  coordination item, deliberately NOT applied by this plan's executors. (b) Interim guard rerouted:
  typeck-level WHAT/WHAT-INSTEAD/WHY teaching error in `check_user_fn_call` (the `channel<number>`-gate
  convention — codegen has no user-facing diagnostic path), firing uniformly on every call site; no
  "FRAGO 009", no `{span:?}` in user text; codegen arm retained as an unreachable internal backstop.
  Tested RED→GREEN (`v03_m6_int_literal_to_number_param_cpu_member_is_clean_teaching_error`, new
  spike-group fixture `v0_3_m6_int_literal_number_arg_cpu_member.ynz`). (c) Both narrative overclaims
  corrected (plan.md completion note; seg-3 entry annotated below); IR proof persisted as
  `v03_m6_non_escaping_number_ir_stays_by_pointer`. (d) Polish minors filed as Future-Req #15
  (twin-scan consolidation) + #16 (named decimal128 cell-size const), both four-field, both
  "not debt" per code-reviewer. (e) Phase 8's FRAGO-012 durable-home lift-list amended to carry
  #14/#15/#16 so none is lost on the plan's `git mv` to `done/` (FRAGO 015 discipline).
- **Authority.** deviation-judge JUSTIFIED/risk-neutral verdict relayed via the conductor's fix-loop
  round-1 dispatch; risk-neutral → auto-apply + log per Step-7 flow.

### FRAGO 017 — 2026-07-10 — session-id: `executor-2026-07-10-m6-phase1d-fixloop2`

- **Trigger.** Phase 1d fix-loop round 2 (conductor-routed, fleet-surfaced): (a) graveyard-auditor +
  rules-compliance flagged the trampoline staged decimal128 arg-cell shutdown-drop leak
  (`emit.rs:~9708` `spike_num_free` free site) as an UNTRACKED deferral — only a "noted, not fixed
  here" code comment, no plan-seam entry, so it would be LOST on the plan's `git mv` to `done/`
  (the exact FRAGO 015 failure shape); (b) code-reviewer proved round 1's "fires on every call site
  uniformly" guard claim FALSE (UFCS + generic-concrete-param forms bypassed → still ICE'd);
  (c) test-quality flagged the IR-proof markers as mechanically unlinked to codegen's emitted names
  (one already inert).
- **Corroboration.** Both bypass forms reproduced RED live against the pre-fix binary (verifier
  reject / IntValue panic, banners verbatim); marker inertness probe-confirmed against emitted IR
  (`number_to_heap_cell` 0 hits everywhere; `spike_num_free` 0 hits — void-call names dropped).
- **Classification.** JUSTIFIED / RISK-NEUTRAL (deferral formalization + review should-fixes; same
  shape as FRAGO 015/016). No signature gate trips.
- **Delta (applied to plan.md by `executor-2026-07-10-m6-phase1d-fixloop2` in this same dispatch).**
  (a) NEW Future-Req #17: trampoline staged arg-cell leak on a blocking-pool task dropped un-run at
  runtime shutdown — four-field (WHAT process-exit-only cell leak, one balancing free never runs;
  WHY never-drop-locals class, same as M5 #6 / this plan's #13, needs the drop story; COST small
  once the drop-story milestone lands, exactly-once between trampoline free + shutdown drop path;
  TRIGGER drop story lands OR a real workload accumulates un-run dropped tasks). (b) Phase 8's
  FRAGO-012 lift-list amended to carry #17 (owner-tag `unscoped → needs the drop-story milestone`,
  joining #13). (c) Guard-extension deltas recorded in the Phase 1d completion narrative + #14's
  entry (round-1 uniformity overclaim corrected in place; round-2 note added). (d) `emit.rs` leak
  comment updated to cite #17.
- **Authority.** Fleet-surfaced should-fixes routed via the conductor's fix-loop round-2 dispatch;
  risk-neutral → auto-apply + log per Step-7 flow (FRAGO 015/016 precedent).

### FRAGO 018 — 2026-07-10 — session-id: `executor-2026-07-10-m6-phase1d-fixloop3-seg3`

- **Trigger.** Phase 1d fix-loop round 3 (conductor-routed, human-directed): round 3's exhaustive
  slot enumeration proved the interim IntLit→`number` guard's danger class is far WIDER than the
  three call-argument forms rounds 1–2 covered. Probe-confirmed, pre-fix, on live-reachable Yinz
  source: `array<number>.add(5)` **SEGFAULTS (exit 139)**; `contains(5)` returns a **SILENT wrong
  `false` (exit 0)** — worse than a crash; plus ~24 further arg / construction / statement slots that
  ICE or silently corrupt (generic-explicit `pass<number>(5)`, sibling-bound `pick(price, 5)`,
  array/fixed `.set`, `maybe<number>.or(5)`, struct-lit field, array/fixed/map literal elements,
  index-assign incl. the map KEY, map bracket-key READ, field-assign, `return 5` from `-> number`,
  neg-literal `-5`). The human chose **complete the guard across EVERY IntLit / `-IntLit` → `number`
  slot** over defer, justified by that finding (recorded in the conductor cold-resume note).
- **Corroboration.** All 24 committed RED fixtures re-confirmed RED against the pre-fix binary
  (verbatim signatures in each fixture header: ICE panic exit 2, LLVM-verifier ICE, silent exit 1,
  and the two SILENT-WRONG exit-0 cases); post-fix all 24 flip RED→GREEN (teaching error, non-zero
  exit, no "compiler bug"/"FRAGO" banner); false-positive sweep byte-exact clean; controls
  (`x = 5` reassign, the #9 `let x: number = 5` store-site) confirmed UNCHANGED. Full step-4 gates:
  `cargo nextest run --workspace` 2344 passed / 0 failed (exit 0; M3e `cross_impl_consistency` both
  GREEN under load, no flake); `cargo clippy --workspace -- -D warnings` clean; `cargo fmt --all
  --check` clean.
- **Classification.** SCOPE EXPANSION (defer → complete-the-guard), human-directed; JUSTIFIED /
  RISK-NEUTRAL as a delta — it extends an existing COMPILE-TIME teaching error to more slots,
  converting segfaults / silent corruption / ICEs into clean WHAT/WHAT-INSTEAD/WHY rejections; no
  runtime behavior changes, no new miscompile surface, one authoritative gate (no per-form twins).
  No signature gate trips. (The four deviations in the Delta below are SURFACED for the
  deviation-judge → conductor seam — this FRAGO RECORDS them; it does NOT rule on whether the
  out-of-charter scope-outs were justified.)
- **Delta (applied to plan.md by `executor-2026-07-10-m6-phase1d-fixloop3-seg3` in this same
  dispatch).** (a) Phase 1d completion narrative gains the **fix-loop round 3** note (scope
  expansion + the one-authoritative-gate mechanism: `reject_int_literal_number_arg` → thin wrapper
  over role-parameterized `reject_int_literal_number_slot`; `-IntLit` arm; generic post-loop
  `apply_substitution` pass; the one `collection_method_arg_slots` table in `builtins.rs`; hinted
  construction / statement slots). (b) Future-Req #14 updated to state the guard is now COMPLETE for
  the IntLit / `-IntLit` → `number` argument / construction / statement class, store-site #9
  excepted, and that the WHOLE guard is removed when the coercion ships. (c) `emit.rs` backstop
  comment near `emit_cpu_member_spawn` updated to name the shared slot gate + the true covered set.
  (d) **FOUR deviations surfaced (routed, NOT self-adjudicated):** (1) enumeration-completeness
  amendment — two map-bracket-KEY sibling slots the seg-1 25-row table missed (index-assign
  `names[5] = …` and index-read `names[5]`, both probe-confirmed silent type-corruption), fixed
  IN-CLASS this round + tested (`..._map_bracket_key.ynz`); (2) NEW pre-existing decimal128
  **by-value RETURN** garbage from a synchronous user fn (`print(toll(5.0))` nondeterministic; the
  synchronous return-value sibling of Phase 1d's arg class), out of hotfix charter — candidate
  Future-Req; (3) NEW pre-existing `map<number, V>` REAL-number-literal-key silent breakage (keys
  hash/compare by pointer identity; `set(1.5,…)` then `get(1.5)` → `none`, exit 0), out of charter —
  candidate Future-Req; (4) unchanged prior-round out-of-scope gaps — the general UFCS
  arg-validation gap (`a.concat([5])` / `pick(5, price)`, number→int direction) and `array.remove`
  having no codegen lowering arm for ANY type.
- **Authority.** Human-directed scope expansion relayed via the conductor's fix-loop round-3
  dispatch; risk-neutral delta → auto-apply + log per Step-7 flow (FRAGO 015/016/017 precedent). The
  four surfaced deviations await the deviation-judge → conductor seam's disposition (candidate
  Future-Reqs for #2/#3; the #4 pair pre-recorded, unchanged).

### FRAGO 019 — 2026-07-10 — session-id: `executor-2026-07-10-m6-phase1d-fixloop4`

- **Trigger.** Phase 1d fix-loop round 4 (conductor-routed): a five-lens review of round 3 confirmed
  0 blockers but surfaced TWO honesty-gaps in round 3's "COMPLETE" claim, both reproduced by the
  conductor and re-verified against the live tree this round. **Finding 1 (code):** `return 5` from a
  `-> number errors` fn missed the round-3 teaching gate — pre-fix it emitted the GENERIC mismatch
  (`return produces int, but this function must return number errors`, exit 1) where the plain
  `-> number` control taught (`Write it as a decimal literal: 5.0`). Root cause: the return type is
  `Type::ErrorsCapable{ inner: Number }`, so the shared gate's `matches!(expected_ty, Type::Number{..})`
  is false through the wrapper and the return-site consumer (`check.rs:2170`) falls through to the
  generic path (`check.rs:2185`). Both paths reject cleanly (exit 1) — NO ICE, NO silent-wrong — so a
  diagnostic-QUALITY gap inside round 3's own claimed "return" scope, contradicting the doc-comment;
  Golden Rule 11 makes the inconsistency worth closing. **Finding 2 (doc only):** `hidden cache: number
  = 5` ICEs (`Found IntValue(i64 5) but expected PointerValue variant` at `emit.rs:20351`, from the
  field-default lowering site `emit.rs:18233` calling `lower_expr` with no hint → raw i64 →
  `store_field` into a decimal128 slot) — STORE-SITE #9 class, structurally identical to `let x: number
  = 5`; NOT gated (gating it alone while `let x: number = 5` stays un-gated would make the store-site
  class inconsistent).
- **Corroboration.** Finding 1 Paper-Trace verified against the freshly-built debug binary: pre-fix
  errors-return → generic mismatch (exit 1); post-fix errors-return → teaching error (exit 1); plain
  `-> number` UNCHANGED (teaches); controls stay clean — `return 5` from `-> int errors` → `ok` (exit
  0), `return 5.0` from `-> number errors` → `ok` (exit 0). Finding 2 Paper-Trace confirmed the ICE +
  panic location. Full step-4 gates: `cargo nextest run --workspace` **2345 passed / 0 failed** (exit
  0; +1 vs round 3's 2344 = the new errors-return test; M3e `cross_impl_consistency` both GREEN under
  load, no flake); `cargo clippy --workspace -- -D warnings` clean; `cargo fmt --all --check` clean.
- **Classification.** Finding 1 is a DIAGNOSTIC-QUALITY completion within round 3's own claimed scope
  — extends the existing COMPILE-TIME teaching error to the errors-wrapped return through the ONE
  shared gate (authoritative-derivation: unwrap `ErrorsCapable` before the `Type::Number` check, no
  parallel path); no runtime behavior change, no new miscompile surface. Finding 2 is a DOC-ONLY
  carve-out (no code) of a pre-existing store-site ICE the round-3 "COMPLETE" claim over-swept. The
  three formalized deferrals (#18/#19/#20) are OUT-OF-CHARTER pre-existing gaps promoted from round 3's
  surfaced-deviation list — RECORDED, not ruled on. No signature gate trips (risk-neutral delta).
- **Delta (applied to plan.md + check.rs + the test file by `executor-2026-07-10-m6-phase1d-fixloop4`
  in this same dispatch).** (a) CODE: `reject_int_literal_number_slot` (`crates/ynz-typeck/src/check.rs`)
  unwraps `ErrorsCapable{inner}` before the `Type::Number` check; new RED fixture
  `v0_3_m6_int_literal_number_return_errors.ynz` + test
  `v03_m6_int_lit_number_return_errors_is_teaching_error` in
  `crates/ynz-driver/tests/v03_m6_number_spawn_boundary.rs`. (b) `check.rs` gate doc-comment updated —
  round-4 errors-return added to the round list; BOTH declaration-site store facets (`let x: number =
  5`, `hidden f: number = 5`) named as the #9 store-site carve-out. (c) Future-Req #14 corrected — the
  "COMPLETE" claim now reads complete INCLUDING the errors-wrapped return, with both store facets carved
  out; the new errors-return test added to the tested set. (d) Future-Req #9 folds in the
  hidden-field-default (records the shared store-site root + the linkage to the
  `2026-07-04-v0-3-hotfix-int-literal-number` stub plan under ONE coercion; did NOT edit the stub plan —
  conductor→human coordination item). (e) THREE four-field deferrals formalized: #18 (sync decimal128
  by-value RETURN garbage), #19 (`map<number,V>` real-number-key silent breakage), #20 (general UFCS
  number→int arg-validation gap + `array.remove` no codegen lowering) — promoted from FRAGO 018's
  surfaced-deviation list. (f) Key Outcome 1d amended with the headline-vs-delivered reconciliation, and
  its unqualified stale `check.rs:3369-3398` citation corrected to the live `:3417-3451` (an
  already-recorded fixed fact whose headline sibling was never swept; historical step-text/D8 citations
  left as deliberately-preserved originals per #12's documented drift note).
- **Authority.** Conductor-routed fix-loop round-4 dispatch; risk-neutral deltas → auto-apply + log per
  Step-7 flow (FRAGO 015/016/017/018 precedent). Finding 2's carve-out and the three deferrals are
  RECORDED here, not self-adjudicated — the deviation-judge → conductor seam owns any further ruling;
  the cross-plan stub-plan expansion (#9/#14 coercion) remains a conductor→human coordination item.

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

### 2026-07-10 — Phase 1d, segment 1
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1d-segment-1

- **segment number:** 1
- **session-id:** `executor-2026-07-10-m6-phase1d`
- **subagent_tokens actual:** 298722
- **checkpoint reason:** executor's own early-checkpoint judgment call (context budget past threshold at the step-2→step-3 boundary; conductor-authorized). No `**OVER-FAT-STEP PROPOSAL**` sentinel — a plain PARTIAL on a completed-step boundary, not a split proposal.
- **canonical resume-at pointer:** `phase-1d/step-3` (first segment of the phase — no previous pointer to stall-compare against, so re-dispatch is unconditional).
- **segment verdict:** `STATUS: PARTIAL`. **Steps 1-2 DONE.** Step 1: FRAGO-009 citations re-verified against live tree (line drift only, content matches: `prepare_bg_arg_for_ctx` emit.rs:15616, cpu-member i64 load emit.rs:9686-9697, `channel<number>` gate check.rs:3417-3451); **Recorded Decision D8 = Option 2 (eager decimal128 heap-copy at the spawn boundary; `channel<number>` gate untouched, Future-Req #12 keeps that trigger)** recorded in plan.md BEFORE implementation, weighed against IMP-concurrency/IMP-no-function-coloring (design doc's `.give`/`.copy` background-args model + Option-1 structurally unavailable for defect C's auto-promotion path). Step 2: 8 RED repros locked + Paper-Traced (A-arms `0.000...` vs 2.5/4.5, 3/3 deterministic per arm; C-forms verbatim ICE `Call parameter type does not match function signature!` on pure-spike + fused). Repro-shape discovery recorded: bare fire-and-forget last-statement spawn does NOT repro — proven shape suspends the spawner after the spawn. No code changed; the only reds are the 8 planned-RED repros step 3 is on record to green. No deviation surfaced. One OPEN probe carried in handoff (not a deviation): whether `f(5)` typechecks against a `number` param (decides the cpu-member IntLit sub-case). Resume at `phase-1d/step-3` via handoff.

### 2026-07-10 — Phase 1d, segment 2
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1d-segment-2

- **segment number:** 2
- **session-id:** `executor-2026-07-10-m6-phase1d-seg2`
- **subagent_tokens actual:** 244382
- **checkpoint reason:** executor's own early-checkpoint judgment call (past budget at the clean step-3→step-4 boundary; the step-4 full-workspace run is substantial and warranted a fresh window). No `**OVER-FAT-STEP PROPOSAL**` sentinel — plain PARTIAL on a completed-step boundary.
- **canonical resume-at pointer:** `phase-1d/step-4` (advanced from seg-1's `phase-1d/step-3` — no stall).
- **segment verdict:** `STATUS: PARTIAL`. **Step 3 DONE — the Option-2 (D8) fix, ONE authoritative mechanism, all in emit.rs:** new `Cg::number_to_heap_cell` (single 16-byte boundary-cross copy both defects consume, beside `shape_bytes_to_heap_cell`); Defect A both spawn arms via unconditional `Type::Number{precision<=34}` pre-gate in `prepare_bg_arg_for_ctx` → `(cell, HeapShape{16})`, freed by the CPU-spawn `emit_bg_arg_frees` + SM-spawn `BgArgDropEntry` kind-0 (SM child-side read unchanged); Defect C via new `callee_takes_bare_number` predicate consumed by BOTH `emit_cpu_member_spawn` + `build_cpu_trampoline` (one alloc/one free). **8/8 Phase-1d RED repros GREEN** (value + N=10 determinism, A CPU/SM + C pure/fused); `cargo build -p ynz-driver` clean. **Full workspace suite / clippy / fmt / false-positive sweep NOT yet run — that is step 4.** **OPEN probe RESOLVED (verify-before-fix):** `f(5)` (IntLit→`number` param) typechecks but ICEs at codegen even SYNCHRONOUSLY (`call ptr @grow(i64 5)`; root cause `lower_expr(IntLit)`=i64 emit.rs:14514, no call-site int→number coercion emit.rs:14986-14990) — so the cpu-member IntLit sub-case needed NO bespoke boundary fix; guarded to a clean compile error, never a segfault. **DEVIATION SURFACED (NOT self-decided — for the deviation-judge → conductor seam at the phase boundary):** IntLit→`number`-param is a PRE-EXISTING GENERAL codegen coercion gap (ICEs synchronously), orthogonal to FRAGO-009's concurrency charter; fixing it only at the boundary would ship an inconsistent teaching story (`background f(5)` works, `f(5)` doesn't), so the executor scoped it OUT and guarded it cleanly — candidate Future-Requirements deferral (int-literal→`number` call-site coercion), to be routed through the seam by the step-5 segment / boundary fan-out. Evidence: `lower_expr` emit.rs:14514, normal call loop emit.rs:14986-14990, typeck coercion check.rs:2224/:3813. Resume at `phase-1d/step-4` via handoff.

### 2026-07-10 — Phase 1d, segment 3 (DONE)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1d-segment-3

- **segment number:** 3
- **session-id:** `executor-2026-07-10-m6-phase1d-seg3`
- **subagent_tokens actual:** 261721 (authoritative, from the conductor's task-notification usage block; the executor's own ~186000 was an estimate — this entry was written by the executor, an §3a.1 overstep the conductor reconciled)
- **checkpoint reason:** phase DONE (steps 4 + 5 + close-out all complete; handoff deleted as the final act).
- **canonical resume-at pointer:** n/a — Phase 1d COMPLETE (advanced from seg-2's `phase-1d/step-4`).
- **segment verdict:** **DONE.** **Step 4 gates all GREEN.** Confirming full workspace re-run `cargo test --workspace` **exit 0 — 2315 passed / 0 failed** (M3e determinism holding under load: `cross_impl_consistency` 2/2, 251s). `cargo fmt --all --check` clean (formatted ONE seg-2 leftover line in `emit.rs` via `cargo fmt -p ynz-codegen` — no `#[allow]`, no `--no-verify`); house `cargo clippy --workspace -- -D warnings` clean. 8/8 Phase-1d repros GREEN (value + N=10, A CPU/SM + C pure/fused). **False-positive sweep clean** — committed sweep test GREEN + IR-level proof (`ynz build --emit-ir` on the sweep fixture): ZERO number heap-cell markers (`bg_number`/`_num_ld`/`_num_bits_`/`spike_num_arg_ptr`/`number_to_heap_cell`), non-escaping `bump(x)` → `call ptr @bump(ptr %x1)` (by-pointer, no cell). **[CORRECTED — fix-loop round 1, `executor-2026-07-10-m6-phase1d-fixloop1`, FRAGO 016: (a) that IR proof was a session-local run with NO persisted artifact — now committed as `v03_m6_non_escaping_number_ir_stays_by_pointer`; (b) "sweep clean" overstated — the sweep fixture has ZERO spawn calls, so it proves Phase 1b/1c's disjoint classifier untouched (code-path disjointness), NOT Phase 1d boundary discrimination; no in-boundary false-positive case exists BY DESIGN under Option 2's unconditional Number pre-gate, and in-boundary correctness is carried by the 8 value/determinism repros.]** **Step 5:** Future-Req #12 tightened (stale `check.rs:3369-3398` → live `check.rs:3417-3451`; COST narrowed to D8/Option 2 — reuses `number_to_heap_cell`, still needs its own conduit send/recv marshalling pass; gate untouched per D8 reason 5); new Future-Req #14 = surfaced **IntLit→`number`-param call-site coercion gap** (ICEs even synchronously; sibling call-site facet of #9's store-site ICE, same root class; scoped OUT of FRAGO-009 charter, guarded to a clean cpu-member compile error) recorded as a candidate four-field deferral. **DEVIATIONS SURFACED (NOT self-adjudicated — for the deviation-judge → conductor seam):** (1) IntLit→`number` call-site coercion scope-out (candidate #14 — the seam rules whether the scope-out was justified + formalizes a FRAGO number, mirroring FRAGO 015/#13); (2) first full-suite run had ONE transient flake — `current_rss_bytes_returns_value_on_supported_platforms` (`crates/ynz-watch/tests/long_session.rs`, live-process RSS `mb > 0`), in not-this-executor's `ynz-watch`, UNMODIFIED source (`memory.rs` + test file both clean; dirty `error.rs`/`rebuild.rs` don't touch RSS), structurally unrelated to a codegen change, passed 3/3 isolated AND did not recur on the confirming re-run. **OBSERVATION (not a gate failure):** the stricter `clippy --workspace --all-targets` (NOT the documented gate) surfaces two pre-existing test-code lints in `ynz-numerics` + `ynz-runtime` (`elem[1] = -1` unused-assignment) — not-this-executor's crates, candidate future cleanup. Phase 1d completion note written to plan.md; session-id appended; handoff `handoff-phase-1d.md` deleted as the final act. With Phase 1b + 1c, the whole R15/FRAGO-009 interim risk is closed. Did NOT commit/stage — conductor seals the boundary.

### 2026-07-10 — Phase 1d, fix-loop round 3, segment 1 (enumeration)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#1d-fixloop3-segment-1

- **segment number:** fixloop3-seg1
- **session-id:** `executor-2026-07-10-m6-phase1d-fixloop3` (segment 1)
- **subagent_tokens actual:** 227028
- **checkpoint reason:** executor early-checkpoint judgment (3 context-budget nudges) at the enumeration→implementation boundary; green tree, ZERO code changes landed (byte-identical to HEAD 759bd9b). No `**OVER-FAT-STEP PROPOSAL**` sentinel — plain PARTIAL.
- **canonical resume-at pointer:** `phase-1d/fixloop3-step-2` (first segment of round 3 — no previous fixloop3 pointer to stall-compare).
- **segment verdict:** `STATUS: PARTIAL`. **The #1 deliverable — exhaustive slot enumeration — is DONE**, with a grep-derived completeness argument (27 `infer_expr(_, Some())` sites classified; no `Paren` AST node so `f((5))` can't defeat the IntLit match; zero `number`-param intrinsics in features.toml; all named/cross-module calls route through exactly the 3 gated fns; `channel<number>` construction-gated). **Root mechanism:** the literal-hint rule check.rs:2223-2227 (IntLit hinted `Number{34}` types as number → codegen lowers raw i64). **25-row slot table (handoff).** New confirmed-RED slots beyond the 3 gated: generic-explicit `pass<number>(5)` + sibling-bound `pick(price, 5)` (ICE, the blocker); `array<number>.add(5)` (**SEGFAULT exit 139**, direct-run verified); array/fixed `.set`; `maybe<number>.or(5)` (LLVM select ICE); `contains(5)` (**SILENT wrong `false`, exit 0 — worse than a crash**); struct-lit field, array/fixed literal elements, index-assign, field-assign, `return 5` from `-> number` (all ICE w/ verbatim evidence); neg-literal UFCS `-5` ICE; plain `f(-5)` already clean (control). Excluded-and-verified-safe: `let x: number = 5` (#9 signed territory), reassign, BinOp, intrinsics. **Fix recipe settled** (extend the one shared gate with a `-IntLit` arm + a post-loop `apply_substitution` pass for the generic case + a per-method elem-slot table for the 4 collection arms + construction/stmt-slot gating). **Deviations surfaced (not self-decided):** (1) `a.concat([5])`/`pick(5, price)` reach breakage only via the pre-existing general arg-validation gap (number→int direction) — NOT cleanly separable into the IntLit→number class, noted-not-gated per the exclusion; (2) `array.remove` has no codegen lowering arm at all (pre-existing). Remaining: steps 2 (RED fixtures) → 3 (implement) → 4 (gates) → 5 (close-out + FRAGO 018). Resume `phase-1d/fixloop3-step-2` via handoff.

### 2026-07-10 — Phase 3b, segment 1
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#3b-segment-1

- **segment number:** 1
- **session-id:** `executor-2026-07-10-m6-phase3b-seg1`
- **subagent_tokens actual:** 247305 (authoritative, from the conductor's `Task` tool-result usage block)
- **checkpoint reason:** executor's own early-checkpoint judgment call (context budget crossed ~150k at the step-1→step-2 boundary, before any source was written; the dispatch authorized the handoff convention). No `**OVER-FAT-STEP PROPOSAL**` sentinel — a plain PARTIAL on a completed-step boundary.
- **canonical resume-at pointer:** `phase-3b/step-2` (first segment of the phase — no previous pointer to stall-compare against, so re-dispatch is unconditional).
- **segment verdict:** `STATUS: PARTIAL`. **Step 1 (CCIR-1) DONE** against HEAD `fc7797f`: six cited anchors re-verified — three trivial line-shift drifts corrected (`runtime.rs:607`→`:629` root-frame `cleanup_spike_cpu_handles` call; `runtime.rs:659-680`→`:677-707` chain walk, confirmed it frees each child's sleep handle + frame but never its spike CPU handles; `may_block.rs:1638` SCC `len() >= 2` filter), `cpu_admission.rs:508-534` + `queries.rs:900-917` MATCH, stale `queries.rs:941-944` "structurally inert" comment CONFIRMED present + stale (cpu_admission.rs:109-152 documents the M3g Phase 3 decline removal). **Load-bearing orientation finding:** `YNZ_ALLOC_COUNTER` counts only `ynz_alloc`/`ynz_free` — `Box<CpuJoinHandle>` (Rust allocator) is invisible to it, so the plan's "or handle-count instrumentation" branch is REQUIRED for the RED fixture (env-gated handle counters; expected RED `handle_alloc=4, handle_free=2`). Fully-settled design D-3b-1..6 in `handoff-phase-3b.md` (RED fixture source + timeline, parity + positive-control + `YNZ_SKIP_RECURSION_DROP` negative-control integration tests, a deterministic drop-probe unit test, the one-choke-point fix site `cleanup_spike_cpu_handles(child_ptr)` at `runtime.rs:691-706`, the `queries.rs` comment rewrite). No code touched; tree unchanged (4 pre-existing not-mine dirty files untouched). No deviation surfaced — anchor drifts are the trivial class Phase 0 precedented. One conditional pre-registered in the handoff: if the RED positive-control fails to reproduce the leak, that falsifies Phase 0's reachability claim → surface to the deviation-judge seam, do not self-adjudicate. Resume `phase-3b/step-2` via handoff.

### 2026-07-10 — Phase 3b, segment 2 (DONE)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#3b-segment-2

- **segment number:** 2
- **session-id:** `executor-2026-07-10-m6-phase3b-seg2`
- **subagent_tokens actual:** 261748 (authoritative, from the conductor's `Task` tool-result usage block)
- **checkpoint reason:** phase DONE (steps 2–5 all complete; handoff `handoff-phase-3b.md` deleted as the final act).
- **canonical resume-at pointer:** n/a — Phase 3b work COMPLETE (resumed at seg-1's `phase-3b/step-2`, advanced through to DONE; no stall).
- **segment verdict:** **DONE.** **The fix landed** — chain-walk drop (`runtime.rs:677-707`) now calls the SAME `cleanup_spike_cpu_handles(child_ptr)` the root frame uses (one authoritative cleanup choke point, no second drop path; helper doc names both grains + the never-duplicate rule). **RED→GREEN non-vacuous** via env-gated `CpuJoinHandle` handle counters (alloc-counter-invisible `Box`): RED integration `handle_alloc=4, handle_free=2` + unit probe `0 != 1`; GREEN `4/4` + probe `1` + frame parity `3/3`; **durable negative control** `YNZ_SKIP_RECURSION_DROP=1` still leaks post-fix. **Positive control PASSED** (`handle_alloc >= 4`) → Phase-0 reachability claim CONFIRMED; the pre-registered falsification conditional did NOT fire. New fixtures: `crates/ynz-driver/tests/fixtures/v0_3_m6_recursive_spike_cancel.ynz` + `tests/v03_m6_recursive_spike_cancel.rs`; unit `recursion_chain_child_spike_handles_freed_on_drop`. Stale `queries.rs:941-944` comment corrected (step 4). Debug detour: a first post-fix red was a **stale runtime embed** (driver `include_bytes!`s `libynz_runtime.a`; forced re-embed resolved it; an offset-mismatch hypothesis was checked + falsified). **nextest full workspace: 2360 run / 2359 passed / 1 failed / 6 skipped** (baseline 2355 + 5 new; M3g/M4 recursion + CPU-group + all 3 `recursion_cancellation*` pass); clippy `-D warnings` clean; fmt clean. **DEVIATION SURFACED (NOT self-adjudicated — for the deviation-judge → conductor seam):** the 1 failure `v03_m6_ufcs_background_spawned_method_call_runs` is **bisect-proven PRE-EXISTING** (fails 2/2 with this phase's source diff fully reverted + runtime force-re-embedded; symptom: spawned task prints empty line instead of "Mon"), NOT caused by this diff — the pre-existing dirty `Dockerfile` noted as a candidate suspect, not investigated. Open question for the seam: is the "full suite green" exit criterion met given one bisect-proven pre-existing failure. Did NOT commit/stage — conductor seals the boundary.

## Conductor cold-resume note — 2026-07-10 (context clear mid-Phase-1d-round-3)

Written by conductor `conductor-2026-07-10-m6-exec2`-lineage session at a human-requested context
clear, mid-execution. Phase 1c is sealed (commit 759bd9b); **Phase 1d is NOT sealed** — it is
mid fix-loop **round 3** (the guard-completion sweep). This note is state + history for the next
conductor; it is NOT standing law (do not absorb it as operating rules — verify against current
global charter).

### ⚠️ IN-FLIGHT EXECUTOR AT CLEAR TIME — reconcile BEFORE re-dispatching (anti-collision)
An implement executor (`executor-2026-07-10-m6-phase1d-fixloop3-seg2`) was **dispatched and running**
when this context was cleared, implementing round-3 steps 2→5 from `phase-1d/fixloop3-step-2`. A
cold-resuming conductor MUST reconcile its status before re-dispatching anything from the handoff,
or two producers collide:
- **Check completion:** does `executor-2026-07-10-m6-phase1d-fixloop3-seg2` appear in `plan.md`
  frontmatter's session-id chain (it appends on completion)? Has the handoff pointer advanced past
  `phase-1d/fixloop3-step-2`, or is the handoff deleted (= round-3 DONE)?
- **If completed** → resume from the updated on-disk state (route its return per Step 3a: PARTIAL →
  continue the checkpoint loop; DONE → run the full re-gate, code-reviewer's completeness lens is
  the real test, then seal).
- **If still running** → WAIT for it; do not re-dispatch the same step.

### Round-3 state (guard-completion sweep)
Human chose "complete the IntLit/`-IntLit`→`number` guard across EVERY arg/construction slot" over
defer, justified by the enumeration finding real danger (segfault exit 139 on `array<number>.add(5)`;
silent-wrong `false` on `contains(5)`; ~10+ slots). Enumeration is DONE + grep-completeness-argued
(see the `## Context-segment log` fixloop3-seg1 entry + `handoff-phase-1d.md`'s 25-row slot table).
Excluded: `let x: number = 5` (#9, signed stub-plan territory). Two out-of-scope deviations surfaced
(the general UFCS arg-validation gap; `array.remove` no-lowering). Closing dispatch files **FRAGO 018**.

### NEXTEST — session speed override (continue it)
This session ran ALL gates via `cargo nextest run --workspace` (dispatch-prompt override in executor
+ green-check prompts), NOT the resolver default. Reason: the global resolver probes `cargo-nextest`
on the HOST (absent — all-docker setup), so it emits slow `cargo test`; nextest lives only in the dev
container. The durable fix is an **inbox candidate in CANON** (`inbox/2026-07-10-resolver-nextest-env-mismatch.md`,
covers a hook-author resolver fix + a rule-author corpse). Until that lands, KEEP writing
`cargo nextest run --workspace` into executor + green-check dispatches, else gates revert to slow
`cargo test`. Speed only, not correctness.

### Model — Fable restored (no action needed)
`coding·high` → Fable·medium is live in the CANON REF binding (commit 2719845 + mirrored to
`~/.claude/`), **expiry hard-stop 2026-07-13** (auto-reverts to Opus Monday; revert target 0628136).
`select-model` auto-resolves Fable — no manual model override needed for dispatches.

### Coordination items to surface to the human at 1d seal (candidate findings, NOT yet routed)
1. **Stub-plan expansion** — #14's call-site facet should be added to the `2026-07-04-v0-3-hotfix-int-literal-number`
   stub plan's scope so the store-site (#9) + call-site (#14) coercion lands as ONE mechanism. This is
   a cross-plan edit reserved for conductor→human coordination; deliberately NOT self-applied.
2. **UFCS general arg-validation gap** — pre-existing: general arg-type/arity through `p.method(...)`
   isn't validated beyond the new IntLit gate. Orthogonal to #14; belongs to whoever owns the UFCS
   typeck surface next.

### Fix-loop history (Phase 1d)
R1 = guard diagnostic reroute (typeck WHAT/WHAT-INSTEAD/WHY) + narrative overclaim corrections.
R2 = extend guard to UFCS + generic call forms (one shared helper). R3 = full-slot completion sweep
(in flight). All original 1d exit criteria + defects A/C were verified MET before the fix-loop began.

### 2026-07-10 — Phase 2 — P4-3: block_on-fallback hard-error guard (DONE)
- **session-id:** `executor-2026-07-10-m6-phase2`
- **What landed (all in `crates/ynz-codegen/src/emit.rs`):**
  1. **Pure predicate** `sync_call_fallback_is_codegen_bug(callee_name, genuine_suspend_set)` — co-located
     with its authoritative sibling `is_direct_suspending_call`. Mirrors that predicate's membership test:
     `genuine_suspend_set.contains(name) && !is_base_suspension_intrinsic(name)` (the ONE authoritative
     suspend classifier, threaded — never a second name-keyed derivation, per authoritative-derivation.md).
  2. **Hard-error guard** at the head of `lower_expr`'s `Expr::Call` `name =>` synchronous-fallback arm
     (the block_on-driven wrapper path, plan's `emit.rs:15122-15137` region, now shifted post-Phase-1).
     `return Err(format!("codegen bug: suspending callee ... "))` in WHAT/WHAT-INSTEAD/WHY shape, mirroring
     the sibling recursive-path hard error in `emit_suspending_call_heap_boxed` EXACTLY (a codegen-internal
     `Err(String)` invariant assertion — not a user-facing diagnostic emitted through the diagnostics system).
  3. **New `Cg` field `base_suspends: &'g SuspendSet`** — threaded to all three `Cg` construction sites
     (lower_function straight-line; `cg_resume` SM path; mono-generic = `empty_suspend_set()`). The guard keys
     on `base_suspends`, NOT `suspend_set`. See the paper-trace below.
- **PAPER-TRACE (false positive caught + fixed during the corpus sweep):** first guard draft keyed on
  `cg.suspend_set`. FULL suite → 2 failures (`corpus_byte_identical_across_auto_parallel_modes` +
  `v03_m3d_cpu_child_panic_fires_byte_identical`); many `v0_3_m3d_*` fixtures went MODE-DIVERGENT (default/
  auto-parallel exit 1, sequential exit 0). Direct run of `v0_3_m3d_nested_group_with_suspending_callee.ynz`:
  guard fired on callee `combine`. Observed = default exit 1 (guard err); Expected = byte-identical to
  sequential = `19810`; Residual = `combine` is a **CPU-spike-host** (a pure-CPU parallel host, no wait/sleep),
  promoted into `suspend_set` ONLY in auto-parallel mode (`suspend_set` = `suspends_set_arg` = the
  WITH-CPU-PROMOTIONS set; `base_suspends` = `base_suspends_arg` = the genuine pre-promotion may-block∪imported
  set, per emit_artifact lines 918/926). A CPU-spike host is driven **synchronously** by the poll-join
  mechanism at the top level (the *designed* path — runtime.rs's "non-entry wrapper functions called from
  non-state-machine contexts"), never a block_on async drive, so it must NOT trip the guard. Root cause:
  guard keyed on the wrong set. Fix: thread `base_suspends` (genuine async-suspension set) to `Cg` and key on
  it. Re-verified: `v0_3_m3d_nested_group_with_suspending_callee.ynz` default mode → `19810` (byte-identical).
- **RED fixture (guard's own regression):** unit test `emit::tests::suspending_callee_trips_block_on_fallback_guard`
  — constructs the deliberately-miscategorized condition at the internal level (the guarded path is UNREACHABLE
  from real Yinz source post-Phase-1: the may-block classification routes every genuine suspending call through
  `lower_sm_stmt_with_wait`, so no real program reaches the synchronous block_on fallback with a genuinely-
  suspending callee). Asserts the firing predicate fires on a suspending callee AND does NOT fire on a normal
  non-suspending callee (in-unit false-positive check) NOR on a base suspension intrinsic (`sleep`, mirroring
  `is_direct_suspending_call`'s `!is_base_suspension_intrinsic` exclusion).
- **Registry `[[diagnostic_template]]` check (Step 2) — determination recorded:** the plan's sibling site
  (`emit.rs:11162`, now `emit_suspending_call_heap_boxed`) uses **NO** `diagnostic_template` — it is a
  codegen-internal `return Err(format!("codegen bug: ..."))` invariant assertion, not a user-facing three-part
  diagnostic emitted through the diagnostics system. Per `feature-registry.md` carve-out (mirrored by the note
  at `registry/features.toml` ~L1676-1681 for the can't-infer per-site codegen/check strings), per-site codegen-bug
  messages stay in code, NOT the registry. **Mirroring the sibling exactly ⇒ NO template.** Reuse: none applicable.
  Add: none. **No registry change, therefore no FRAGO** (the plan's "recorded via FRAGO if new" is conditional
  on a NEW template; none was added). This satisfies the Feature Registry Entries note at plan L1898-1900 and the
  milestone's "no new user-facing language-surface" claim (plan L1905-1906).
- **Demo & Error Gallery disposition (plan-invariants) — N/A, with reason:** the guard is structurally UNREACHABLE
  from real Yinz source post-Phase-1 (a codegen-internal invariant assertion the may-block classification prevents
  from firing on real source). Per plan L1591-1593's explicit allowance, the gallery obligation is N/A; coverage is
  the internal regression fixture (the unit test above). No `examples/primantis-orders/` change this phase.
- **GATES (FULL phase-boundary, all GREEN):**
  - `docker compose run --rm dev cargo nextest run --workspace` → **exit 0 — 2346 passed / 0 failed / 6 skipped**
    (3 slow: the two `cross_impl_consistency` corpus tests + fmt idempotency). This run IS the corpus-wide
    **false-positive sweep** (step 4): zero normal `wait x.method()` / `wait fn()` path trips the guard.
  - `docker compose run --rm dev cargo clippy --workspace -- -D warnings` → **exit 0**, clean.
  - `docker compose run --rm dev cargo fmt --all --check` → **exit 0**, clean.
  - No `#[allow]`, no `--no-verify`, no weakened test. No known-flake re-runs needed (none tripped).
- **Design-doc alignment:** this guard is what keeps `IMP-no-function-coloring.md`'s no-bridge invariant ENFORCED
  (not merely documented) — a genuinely-suspending callee mis-classified as non-suspending is now a compile-time
  hard error instead of a silent block_on bridge. No contradiction with the design doc surfaced.
- **Deviations surfaced:** none. (The `base_suspends`-vs-`suspend_set` keying was an in-phase implementation
  correctness fix caught by the corpus sweep, not a plan-vs-reality divergence — the plan's escape-hatch target is
  the genuine block_on async-suspension path, which `base_suspends` is exactly.)
- **Recorded decisions:** (D-a) guard keyed on `base_suspends` (genuine may-block set), not `suspend_set`
  (with-CPU-promotions) — reason: CPU-spike hosts are driven synchronously by the poll-join mechanism, not
  block_on; keying on the promotions set is a false positive proven by the corpus sweep. (D-b) RED fixture is an
  internal unit test on the firing predicate, not a `.ynz` source fixture — reason: the path is unreachable from
  real source post-Phase-1 (plan-sanctioned).
- Did NOT commit/stage — conductor seals the boundary. Session-id appended to `plan.md` frontmatter in the same action.

### 2026-07-10 — Phase 2 fix-up — 3 review cleanups (DONE)
- **session-id:** `executor-2026-07-10-m6-phase2-fixup`
- **Scope:** three small review-cleanups on the REVIEWED-CLEAN Phase 2 guard (0 blockers across 3 lenses).
  Guard logic unchanged — no re-architecture. All in `crates/ynz-codegen/src/emit.rs`.
- **Item 1 — stale comment fix (code-reviewer minor):** the comment above the direct wrapper-fn invoke
  (the plan's `emit.rs:15156-15165` region) still said that path "drives the SM internally via
  RUNTIME.block_on" — untrue post-guard, since a genuinely-suspending callee is errored out by
  `sync_call_fallback_is_codegen_bug` before reaching it. Rewrote to durable phrasing: the direct invoke
  now serves ONLY non-suspending callees + CPU-spike hosts (poll-join-driven), with the genuinely-suspending
  case diverted to the guard above. Describes current-code properties, no changelog phrasing.
- **Item 2 — formula extraction (authoritative-derivation + reusability):** the membership formula
  `set.contains(name) && !is_base_suspension_intrinsic(name)` was literally re-spelled at FOUR sites. Extracted
  to ONE content-neutral helper `fn is_suspending_member(set: &SuspendSet, name: &str) -> bool` (co-located
  just above `is_direct_suspending_call`). Wired all four consumers to it, each passing its OWN set (the
  intentional difference that stays): the router `is_direct_suspending_call` (on `suspend_set`), the guard
  `sync_call_fallback_is_codegen_bug` (on `base_suspends`), plus the two inline copies in
  `collect_callees_in_expr` and the suspension-point counter (both on `suspend_set`). The formula is now
  genuinely single-source. Doc comments on both predicates updated to the "one formula, two sets, never a
  second derivation" story; the guard's own unit-test comment updated to reference the shared helper.
  **RECORDED DECISION (D-c):** the task scoped item 2 to the two named predicates ("have BOTH predicates call
  it"), but the stated goal — "genuinely single-source" — and `authoritative-derivation.md` are only satisfied
  if the helper is the SOLE home of the formula. Leaving the two OTHER inline copies (`collect_callees_in_expr`,
  the point counter) would recreate the exact twin-derivation the rule kills. Wired all four; behavior-identical
  and verified by the full suite (below). Reason on record so the slightly-beyond-"BOTH" scope is auditable.
  **BEHAVIOR-IDENTICAL confirmed:** full nextest → 2346 passed / 0 failed, identical to the pre-fix-up baseline;
  any count regression would have meant the extraction changed behavior. It did not.
- **Item 3 — mono-generic v0.4-revisit cross-ref (code-reviewer minor #2):** the mono-generic `Cg` keys the
  guard OFF via `base_suspends: empty_suspend_set()` (`emit.rs` ~L1519). Vacuously safe today — no generic
  function in the current surface can reach a suspension point through a type parameter
  (`ynz-typeck/src/check.rs:4214-4218`, gated on v0.4 generic+suspension). Added a code comment at that
  `empty_suspend_set()` site noting the guard is vacuously-off for generic bodies and cross-referencing
  `check.rs:4214`. Per the task's preferred route (code cross-ref + audit note, do NOT touch the check.rs
  deferral's logic), recorded here rather than editing `check.rs`: **the block_on-fallback guard's mono-generic
  empty-set assumption is a v0.4-revisit site**, alongside the existing kernel-guard deferral at
  `check.rs:4220-4229` — when v0.4 threads may-block through generic instantiation (`GenericFnSig.suspends`),
  the mono-generic `Cg`'s `base_suspends` must be populated from the real instantiated suspend set so this
  guard fires correctly for suspending generic bodies. `check.rs` deferral logic untouched (comment/note only).
- **GATES (FULL phase-boundary, all GREEN):**
  - `docker compose run --rm dev cargo nextest run --workspace` → **exit 0 — 2346 passed / 0 failed / 6 skipped**
    (3 slow: the two `cross_impl_consistency` corpus tests + fmt idempotency). No count delta from the Phase 2
    baseline of 2346 → the helper extraction is behavior-identical. No known-flake re-runs needed (none tripped).
  - `docker compose run --rm dev cargo clippy --workspace -- -D warnings` → **exit 0**, clean.
  - `docker compose run --rm dev cargo fmt --all --check` → **exit 0**, clean.
  - No `#[allow]`, no `--no-verify`, no weakened test.
- **Deviations surfaced:** none. (Item 2's four-site wiring is a recorded scope decision D-c, not a plan-vs-reality
  divergence — it completes the item's own stated single-source goal.)
- **Touched:** `crates/ynz-codegen/src/emit.rs` (3 fixes + shared helper) + this `audit.md`. `check.rs` NOT touched
  (item 3 recorded here per preferred route). Did NOT commit/stage — conductor seals. Session-id appended to
  `plan.md` frontmatter in the same action.

### FRAGO 020 — 2026-07-10 — session-id: `executor-2026-07-10-m6-store-site-stopgap`

- **Trigger.** Human-directed ("no duct tape") follow-on to the SEALED Phase 1d: close the two LIVE raw-ICE
  store-site exposures Phase 1d deliberately left un-gated (FRAGO 019 Finding 2 + Future-Req #9). This is the
  no-duct-tape live-exposure clause: `let x: number = 5` is "arguably the most common beginner mistake in the
  language" and both facets ICE with the raw "Yinz compiler bug" banner, while a CHEAP in-scope mitigation
  (the shared int-literal→`number` gate already exists and already teaches for every other slot) closes the
  crash now. A standalone guard-extension task with its OWN FRAGO — NOT a Phase 1d reopen. The int→number
  COERCION stays deferred to the `2026-07-04-v0-3-hotfix-int-literal-number` stub plan; this stopgap only
  REJECTS (teaching error), it does not coerce.
- **Corroboration (verify-before-fix Paper-Trace, probe-confirmed against the freshly-built debug binary).**
  PRE-FIX: `let x: number = 5` → ICE banner, panic `Found IntValue "i64 5" but expected PointerValue variant`
  at `emit.rs:20268` (store path); `hidden cache: number = 5` → same panic at `emit.rs:20436` (hidden-default
  store, from `lower_expr`-no-hint at `emit.rs:18318`). POST-FIX: both emit the shared teaching error (exit 1,
  `... is an int literal — passing an int literal to a \`number\` {variable|field} is not supported yet` +
  `Write it as a decimal literal: \`5.0\``, NO "compiler bug"/"FRAGO" banner). Controls stay clean:
  `let x: number = 5.0` → `5.0` exit 0; `number`-typed var → exit 0; `-5` negated store → teaches;
  m5 hidden `int` default (`= 42`) → 42 exit 0 (gate matches `Type::Number` only, never `int`/`float`).
- **Classification.** REJECTION stopgap that closes a live crash exposure via the ONE existing shared gate
  (authoritative-derivation — new `NumberSlotRole::StoreBinding` for the binding; REUSED `NumberSlotRole::Field`
  for the field default; no per-slot twin). Risk-neutral: intercepts a raw ICE with a clean teaching error and
  adds no new miscompile surface. One INTENDED behavior change (documented as such, not a weakened test): the
  store site flips ICE→teach, requiring the `int_literal_retypes_as_number_with_annotation` typeck control to be
  re-pointed to assert the new rejection.
- **Delta (applied to `check.rs` + the two test files + both plans, this same dispatch).**
  (a) CODE — `crates/ynz-typeck/src/check.rs`: new `NumberSlotRole::StoreBinding { name }`; gate call in
  `check_let` (subject `\`x\``, noun "variable") that binds the name at the annotated `number` type when it
  fires so later uses don't cascade; gate call at the `ShapeDecl` arm of `check_module` over hidden-field
  defaults (REUSING `NumberSlotRole::Field`, matching the codegen lowering scope — non-hidden defaults are dead,
  fields must be provided at construction); stale "store sites deliberately NOT gated" doc-comment rewritten.
  (b) TESTS — two RED fixtures (`v0_3_m6_int_literal_number_let_store.ynz`,
  `v0_3_m6_int_literal_number_hidden_field_default.ynz`, headers recording the pre-fix ICE signature) + two slot
  tests via the existing `int_literal_number_slot_test!` macro in `v03_m6_number_spawn_boundary.rs`; the #9
  store-site control COMMENT (L428-429) re-pointed to "ALSO gated / intended behavior change"; the typeck control
  `int_literal_retypes_as_number_with_annotation` renamed to
  `int_literal_number_store_binding_is_rejected_with_teaching_error` asserting the teaching rejection + a new
  `number_literal_stays_clean_in_number_store_binding` false-fire guard (`crates/ynz-typeck/tests/check.rs`).
  (c) DOCS — M6 plan reconciled (Phase 1d deliverable summary, "COMPLETE except #9" claim, Future-Req #9,
  Future-Req #14, round-4 carve-out); stub plan `2026-07-04-v0-3-hotfix-int-literal-number` reconciled (Mission
  intro + SCOPE-WIDENED facets 1&2 flipped from "still ICE" to "now teach"); whole-plan sibling sweep run on both
  (every store-site-ICE/#9-excepted sibling reconciled; clearly-historical step-narrative left as past-tense).
  Roadmap row 441/497 noted as the coercion-assignment sibling (see the Roadmap note below).
- **GATES (FULL, all GREEN).** `docker compose run --rm dev cargo nextest run --workspace --no-fail-fast` →
  **exit 0, 2349 passed / 0 failed / 6 skipped** (+3 vs the Phase-2 baseline of 2346: two new store-site slot
  tests + one new typeck false-fire guard; the pre-existing control was renamed/re-pointed, net 0 for that one).
  The two `cross_impl_consistency` corpus tests GREEN under load (254s/272s); no known-flake re-runs needed.
  `cargo clippy --workspace -- -D warnings` → exit 0 clean. `cargo fmt --all --check` → exit 0 clean.
  No `#[allow]`, no `--no-verify`, no test weakened-to-force-green (the ONE control flip is an intended-behavior
  update, documented above). Did NOT touch `CLAUDE.md` / `Dockerfile` / `ynz-watch/src/*` / roadmap #18/#19 rows.
- **Roadmap note (row 441/497, active v0.3 roadmap).** Row 441 is the coercion-assignment row (still assigned to
  the stub plan; coercion still deferred). Its "then the compiler panics" current-state phrasing is now stale —
  the panic is intercepted by this stopgap's teaching error — but the row's core (coercion missing → assigned to
  the stub) stands. Surfaced as a deviation for the conductor/deviation-judge rather than self-edited into that
  large row: reconciling row 441 is a roadmap edit outside this task's named scope (the task scoped roadmap
  touches to store-site-ICE *sibling statements* and explicitly fenced #18/#19). Flagged for the seam to decide.
- **Deviations surfaced (not self-adjudicated):** (1) the roadmap row 441/497 "panics" phrasing is now stale
  (above) — surfaced for the seam, not self-edited. (2) Recorded decisions: field-default gate scoped to HIDDEN
  fields only (matches codegen lowering scope + confirmed exposure; non-hidden defaults are dead/ignored — a
  non-hidden `number` field must be provided at construction); no demo/error-gallery extension (this reuses the
  EXISTING int-literal→`number` teaching diagnostic — no NEW error class, so the plan-invariants gallery
  obligation does not trigger); no `--release` rebuild performed (a new compile-time REJECTION, not a change to
  the runtime behavior of already-valid programs consumer mounts run — flagged for the conductor's seal step).
- **Touched:** `crates/ynz-typeck/src/check.rs`; `crates/ynz-typeck/tests/check.rs`;
  `crates/ynz-driver/tests/v03_m6_number_spawn_boundary.rs`; two new fixtures under
  `crates/ynz-driver/tests/fixtures/`; `plan.md` (M6) + the stub `plan.md` + both `audit.md`. Did NOT
  commit/stage — conductor seals. Session-id appended to `plan.md` frontmatter in this same action.

### 2026-07-10 — Phase 3 — P3-1/P2-2: pending_sends ABA + orphan purge, segment 1 (PARTIAL — conductor-logged)

- **Conductor-written** (not executor): the segment executor (ran under `executor-2026-07-10-m6-phase3`,
  NOT appended to the frontmatter chain per the no-session-id-on-PARTIAL convention) correctly DECLINED to
  write this Context-segment log entry, citing its charter (`execute-plan §3a.1` — the Context-segment log is
  conductor-owned, written after every segment). The dispatch prompt erroneously asked it to; the executor held
  its charter line rather than overstep. Conductor writes it here; the dispatch template dropped that instruction
  for the continuation segment.
- **Resume-at pointer:** `phase-3/step-2`. **Handoff:** `handoff-phase-3.md` (carries the settled steps-2–5
  design + verification receipts so the continuation implements without re-deriving).
- **What landed (green-building tree carrying the plan-prescribed, documented RED):**
  - **Step 1 DONE** — both cancellation paths confirmed purge-free by direct read: drop-ladder kind-2 arm
    (`crates/ynz-runtime/src/runtime.rs:636-642`) calls only `ynz_channel_free`; `ynz_handle_free`
    (`crates/ynz-runtime/src/handle.rs:337-351`) frees `msg_chan` but never purges the handle-keyed entry.
  - **Step 6 RED repros authored early + proven RED** (verify-before-fix; step 6's own text demands RED-before-fix):
    new `mod m6_pending_send_aba` in `crates/ynz-runtime/src/lib.rs` — frame-path test through the REAL drop
    ladder with forced frame-address reuse, handle-path test through real `ynz_rt_spawn_handle`/`ynz_handle_free`;
    behavior-neutral `#[cfg(test)]` `pending_send_count` accessor added to `channel.rs`. GREEN half pends the fix.
  - **Steps 2–5 design settled in the handoff:** one global generation counter; `(token, gen)` `pending_sends`
    key; `task_gen` on `SpawnStateFnFuture` published via thread-local guard during poll; `send_gen` on
    `YnzTaskHandle`; ONE keyed core + ONE purge-by-generation helper.
- **Paper-Trace (ABA/orphan):** Observed `pending_send_count == 1` after cancel (frame) / free (handle) of a
  sender suspended on a full capacity-1 channel; Expected 0; Residual 1 orphaned boxed send-future per cancelled
  suspended sender (the P2-2 leak + the P3-1 ABA precondition). Evidence: `runtime.rs:641`, `handle.rs:346`,
  `channel.rs:270` (insert site).
- **Gates (this segment):** RED run `nextest -p ynz-runtime -E 'test(m6_pending_send_aba)'` → 2 run / 0 passed /
  2 failed (exactly the defect assertions). `cargo fmt -p ynz-runtime --check` clean. Full workspace gates owed at step 7.
- **Deviations surfaced (for the boundary deviation-judge — the continuation executor files the FRAGO in the same
  dispatch that hardens the design, per plan-source-of-truth seam-before-hardening):** (1) **citation drift** — the
  plan's frame-token mint cite `emit.rs:11651-11654` is stale; live site `emit.rs:12205-12208`
  (`emit_conduit_suspend_point`). (2) **step-5 mechanism refinement** — plan says replace the raw token at BOTH
  mint sites; reality: the generation half must join RUNTIME-side (the frame header is fully packed — codegen-side
  gen storage would require frame-layout ABI surgery, and a salted token must be re-poll-stable). The
  one-counter/one-scheme/both-producers invariant is PRESERVED and coverage extends to embedded-child frame tokens
  the plan never enumerated.
- **Touched by executor:** `crates/ynz-runtime/src/{lib.rs,channel.rs}` (RED suite + test accessor). `plan.md`/`audit.md`
  untouched by executor (this entry is the conductor's). Did NOT commit/stage — conductor seals.

### 2026-07-10 — Phase 3, segment 2 (PARTIAL — conductor-logged)

- **Conductor-written** (executor `executor-2026-07-10-m6-phase3-seg2`, NOT in the frontmatter chain per
  no-session-id-on-PARTIAL; named in FRAGO 021's heading for attribution). Executor correctly asked the conductor
  to log the segment — no overstep this time.
- **Resume-at pointer:** `phase-3/step-7` — full WORKSPACE gates + close-out are the ONLY remaining work; the fix
  is implemented and locally green. **Handoff:** `handoff-phase-3.md` (rewritten).
- **What landed (steps 2–6, per the settled handoff design — not re-derived):**
  - **ONE purge helper** `purge_pending_sends(chan_ptr, generation)` (`crates/ynz-runtime/src/channel.rs`):
    null-safe, gen-0 no-op (reserved unstamped/immortal class), purge-by-generation `retain`, idempotent by
    construction. Wired into BOTH cancellation paths — drop-ladder kind-2 arm (`runtime.rs`, BEFORE `ynz_channel_free`)
    and `ynz_handle_free` (`handle.rs`, BEFORE releasing the conduit ref). Idempotency proven both paths
    (repeated-cancel repro assertions + a dedicated double-purge / purge-empty / purge-null / gen-0 unit test).
  - **ONE runtime-side salting scheme, both producers** — `pending_sends` re-keyed `(caller_token, caller_generation)`;
    one global `next_caller_generation()`; `task_gen` on `SpawnStateFnFuture` (all 3 construction sites) published via
    a `TaskGenGuard` thread-local RAII around every poll (extern-C send ABI byte-identical — **emit.rs untouched, zero
    codegen delta**; covers root + embedded-child + chain-child tokens); `send_gen` on `YnzTaskHandle` passed by
    `ynz_handle_send_poll`; both mints over ONE keyed core `channel_send_poll_guarded`; plus an insert-time
    same-token/different-generation stale sweep (missed-path leak backstop). BOTH mitigations (purge + salted token), D2.
- **RED→GREEN Paper-Trace:** pre-fix `pending_send_count == 1` post-cancel (expected 0; residual 1 orphaned boxed
  send-future); post-fix targeted **4/4** (frame GREEN, handle GREEN, + same-token/different-gen ABA collision proof +
  idempotency unit test); crate `nextest -p ynz-runtime` **134/134**; `fmt -p ynz-runtime --check` clean; documented
  clippy shape exit 0.
- **FRAGO 021 filed** (this dispatch, seam-before-hardening): both seg-1 deviations recorded — citation drift
  `emit.rs:11651-11654` → `:12205-12208` (**sibling sweep: all 7 plan occurrences fixed, grep 0**) + step-5 runtime-side
  salt refinement (**plan step-5 text rewritten in place**); one-counter/one-scheme/both-producers invariant preserved.
  Boundary deviation-judge to ratify.
- **Remaining (step 7):** full workspace `nextest`/`clippy`/`fmt` + close-out. Observation carried in handoff:
  `--all-targets` clippy (NOT the documented gate) hits 2 PRE-EXISTING non-Phase-3 lints (`tests/m2_runtime.rs:275`,
  `lib.rs:2830`) already on record from Phase 1d — untouched, not `#[allow]`ed.
- **Design-doc alignment:** re-checked — IMP-concurrency cancel-via-drop uncontradicted (purge extends the drop ladder);
  IMP-no-function-coloring unaffected (sync atomics + thread-local, no bridge). No new deviation.
- **Touched by executor:** `crates/ynz-runtime/src/{channel.rs,runtime.rs,handle.rs,lib.rs}` + `plan.md` (step-5 text +
  citation sweep) + `audit.md` (FRAGO 021). Did NOT commit/stage — conductor seals.

### FRAGO 021 — 2026-07-10 — session-id: `executor-2026-07-10-m6-phase3-seg2`

- **Trigger.** The two deviations Phase 3 segment 1 surfaced (recorded in that segment's
  Context-segment log entry and `handoff-phase-3.md`), filed by the continuation segment in the
  SAME dispatch that hardens the settled design into code (plan-source-of-truth
  seam-before-hardening). Surfaced-not-adjudicated: the boundary deviation-judge ratifies; this
  FRAGO RECORDS. (Session-id named here for attribution; NOT appended to the frontmatter chain —
  this segment returns PARTIAL, per the no-session-id-on-PARTIAL convention.)
- **Deviation 1 — citation drift.** The plan's frame-token mint cite `emit.rs:11651-11654` is
  stale (that range is now unrelated decimal-staging code); the live mint is
  `emit.rs:12205-12208` in `emit_conduit_suspend_point`
  (`build_ptr_to_int(frame_ptr, …, "conduit_token")`, recomputed per poll site for dominance).
  Handle mint confirmed unchanged at `handle.rs:326`. **Delta applied:** whole-plan sibling sweep
  — all 7 occurrences of the stale cite reconciled (`grep -c 11651` → 0): Situation terrain (×2),
  risk row R2, Key Outcome 3, Phase 3 task text, Phase 3 step 5, Decision D2.
- **Deviation 2 — step-5 mechanism refinement (runtime-side generation salt).** Plan step 5 said
  "replace the raw frame-pointer token (`emit.rs` mint) AND the raw handle-pointer token … at
  both mint sites"; reality: the generation half CANNOT live codegen-side without frame-layout
  ABI surgery — the frame header is fully packed (`ynz-abi`: resume_point@0, spike
  discriminator@4, sleep_handle@8, return_slot@16..32, `FRAME_HEADER_SIZE=32` =
  `SPIKE_HANDLE_BASE_OFFSET`), and a salted token must be stable across re-polls of the same
  suspension (the frame is the only codegen-side durable store). The salt therefore joins
  RUNTIME-side at caller-identity birth: ONE global counter (`channel::next_caller_generation`,
  gen 0 = reserved unstamped/immortal entrypoint class), `(caller_token, caller_generation)`
  key, `task_gen` on `SpawnStateFnFuture` published via a thread-local RAII guard around the
  resume-fn call (extern-C send ABI byte-identical — zero codegen change), `send_gen` on
  `YnzTaskHandle` passed explicitly by `ynz_handle_send_poll`, both through ONE keyed core
  (`channel_send_poll_guarded`) + ONE purge-by-generation helper (`purge_pending_sends`). The
  plan's one-counter/one-scheme/both-producers invariant (D2, authoritative-derivation.md) is
  PRESERVED, and coverage EXTENDS to embedded-child + chain-child frame tokens the plan text
  never enumerated (every token a task mints carries its task_gen). **Delta applied:** Phase 3
  step-5 text rewritten in place to the runtime-side mechanism (cold reader sees current truth).
- **Classification.** Risk-neutral refinements: deviation 1 is a pure citation fix; deviation 2
  changes the salt's JOIN POINT, not the mitigation set (both D2 mitigations land, one scheme,
  both producers, wider token coverage, zero ABI/codegen delta). R2's mitigation shape
  (RED→GREEN pair + idempotent purge at both paths) is unchanged.
- **Evidence.** Receipts in `handoff-phase-3.md` (segment 1, tree `46906d1`): packed frame
  header (receipt 5); live mint site (receipt 3); purge-free cancellation paths (receipts 1-2).
  Implementation this segment: `channel.rs` (counter, thread-local guard, keyed core, purge
  helper, insert-time stale sweep), `runtime.rs` (`task_gen` + poll guard + kind-2 purge),
  `handle.rs` (`send_gen` + explicit-gen send + free-time purge). RED pair flipped GREEN +
  2 new deterministic unit tests (idempotency; same-token/different-generation collision).

### FRAGO 022 — 2026-07-10 — session-id: `executor-2026-07-10-m6-phase3-fixloop1`

- **Trigger.** Phase 3 review fix-loop: code-reviewer + critical-path-integrity converged (0
  blockers, 2 should-fixes); the conductor routed both to this fix-loop dispatch. Should-fix 1
  (gen-0 unprotected class) resolves via the ELIMINATE path, which refines the plan's step-5
  gen-0 wording — filed in the SAME dispatch that lands the code (plan-source-of-truth
  seam-before-hardening). Surfaced-not-adjudicated: this FRAGO records; the boundary
  deviation-judge ratifies.
- **Deviation — step-5 gen-0-class refinement.** Plan step 5 (and FRAGO 021's deviation-2
  record) said "generation 0 reserved as the unstamped entrypoint class, never mass-purged" —
  the entrypoint drive itself rode gen 0, exempt from the purge (gen-0 no-op) and mutually
  unprotected inside its own class (two gen-0 identities at a reused token address share a key;
  the insert sweep retains same-gen entries). Its safety was a prose-only invariant. The
  verify-before-fix Paper-Trace found the class WIDER than the prose: EVERY `SyncStateFnFuture`
  drive minted gen 0 — the codegen `main` wrapper AND every non-entry sync wrapper called from a
  non-state-machine context (`ynz_rt_run_entrypoint` is the shared driver) — all sharing ONE
  unstamped identity. Reality now: the sync driver mints its own NONZERO `task_gen` from the one
  counter at construction and publishes it via the same `TaskGenGuard` around every poll;
  generation 0 remains reserved ONLY for bare unstamped ABI calls (substrate tests), and the
  purge gen-0 no-op is retained as the never-mass-purge floor. **Delta applied:** step-5
  parenthetical + Phase-3 completion-note parenthetical rewritten in place; whole-plan sibling
  sweep for `generation 0`/`gen-0`/`unstamped`/`immortal` (5 hits — step 5 + completion note; no
  risk row or invariant carries the stale wording).
- **Classification.** Risk-neutral (risk-REDUCING) refinement: the one-counter / one-scheme /
  one-keyed-core invariant (D2, authoritative-derivation.md) is preserved and now uniform across
  THREE producers (spawned-task frame tokens, handle tokens, sync-drive frame tokens); zero
  ABI/codegen delta (extern signatures byte-identical, `emit.rs` untouched); no mitigation
  removed or weakened. Eliminate chosen over the debug_assert+defer fallback because the change
  is contained (1 field + 1 mint + 1 guard-enter) and the release binary on the trading mount
  gets the protection compiled in rather than a compiled-out assert — nothing left to defer, so
  no four-field deferral exists or is needed.
- **Evidence.** `runtime.rs` (`SyncStateFnFuture.task_gen`, poll-time `TaskGenGuard`, entrypoint
  mint), `channel.rs` (module/doc/comment reconciliation of the gen-0 class), `handle.rs` (new
  deterministic handle-seam ABA test), `lib.rs` (repro fallback note repointed). Gates: nextest
  workspace 2355 passed / 0 failed / 6 skipped exit 0; targeted ABA suite 5/5; clippy exit 0;
  fmt exit 0.

### FRAGO 023 — 2026-07-10 — session-id: `conductor-2026-07-10-m6-phase3b` (filed at seal; plan.md body edit applied by re-dispatched executor)

- **Trigger.** Phase 3b boundary review. The reviewer fleet on Phase 3b's own work returned **0
  blockers** (code-reviewer clean; critical-path-integrity clean; test-quality MEANINGFUL;
  rules-compliance 0-blocker; graveyard clean) — the leak fix, its non-vacuous RED→GREEN repro, and the
  stale-comment correction are all delivered and exemplary. But two things surfaced that the plan seam
  must record: (a) the exit-criterion "full suite green" is literally NOT met, and (b) the sole failure
  is a corroborated-pre-existing, orthogonal, **flagship** regression the milestone did not know about.
  This FRAGO reframes (a) honestly and records (b) as a tracked milestone finding. deviation-judge ruled
  the executor's scope-out **JUSTIFIED, risk-neutral** — so this is auto-apply + log, no signature.
- **Deviation 1 — Phase 3b exit-criterion #4 "full suite green" reframed to the honest reality.** Plan
  text (Phase 3b Exit criteria, `plan.md` ~:1403-1405) said "full suite green." Reality: the workspace is
  RED on exactly ONE test, `v03_m6_ufcs_background_spawned_method_call_runs`
  (`crates/ynz-driver/tests/v03_m6_ufcs_suspension.rs`), which is **corroborated PRE-EXISTING and
  orthogonal** to Phase 3b's diff. **Independent corroboration (not the executor's word alone):** (i)
  green-check reproduced it 3/3 on the current tree; (ii) a ground-truth pass reproduced the identical
  symptom (`left "\ndone\n"` vs `right "Mon\ndone\n"`, exit fail) at the CLEAN committed baseline
  `fc7797f` with a forced clean rebuild and ZERO Phase-3b changes applied, then restored the tree.
  Phase 3b's diff (runtime drop-walk + env-gated counters + a typeck comment) does not touch the
  UFCS/background-spawn lowering path the failure lives in. **Reframe applied to plan.md:** exit-criterion
  #4 is rewritten to "Phase 3b's own work is green (the recursion-chain spike-handle leak closed, repro
  GREEN); one corroborated-pre-existing orthogonal failure (`v03_m6_ufcs_background_spawned_method_call_runs`)
  remains, tracked as a separate milestone finding (Deviation 2 below) — NOT caused by or curable within
  Phase 3b."
- **Deviation 2 — a flagship milestone regression was discovered at HEAD and is NOT deferred (user-directed:
  hunt it now).** `v03_m6_ufcs_background_spawned_method_call_runs` exercises `background ship.haul()` —
  a `background`-spawned UFCS suspending call with a `give`-transferred receiver — which is **Key Outcomes
  1 & 8** (the flagship "`wait`/`background` x.method() must actually suspend and deliver correct output"
  deliverable M6 exists to make correct against released v0.3.0). At HEAD `fc7797f` the spawned task loses
  its output (prints `""` instead of `"Mon"`). The recorded seal-time baseline "2355 passed / 0 failed"
  (Phase 3's completion note, `plan.md` ~:1327; FRAGO 022 evidence line) was therefore **wrong** — this
  test was already red before Phase 3b began and went unrecorded. Surfaced to the human at the Phase 3b
  boundary; **user directed: seal Phase 3b, then hunt the regression this session** (not a Future-Requirements
  deferral). Its root-cause + fix will be scoped as its own phase/FRAGO once the investigation lands; this
  FRAGO records the discovery + the corroboration so it is not lost.
- **Classification.** Risk-NEUTRAL (documents an already-existing reality honestly; adds no destructive or
  irreversible op; removes/weakens no mitigation). No signature required (deviation-judge: justified +
  risk-neutral). Also filed in this same dispatch: the one in-phase should-fix from the fleet
  (rules-compliance + test-quality `hot-path.md`: `std::env::var("YNZ_SKIP_RECURSION_DROP")` uncached on
  every `SpawnStateFnFuture::drop`) is being fixed in the accompanying fix-loop round by caching the flag
  like `ALLOC_COUNTER_ENABLED`, plus a timing-triage comment on the wall-clock integration test — those
  are code cleanups, not deviations, recorded here only for a complete boundary record.
- **Evidence.** Fleet verdicts this boundary (all this session's dispatches): code-reviewer clean;
  critical-path-integrity 0-blocker/1-minor(pre-existing, out-of-diff); test-quality MEANINGFUL;
  rules-compliance 0-blocker/1-should-fix(hot-path, being fixed); deviation-judge JUSTIFIED+risk-neutral;
  green-check red-on-1-test (secret-scan pass via gitleaks); graveyard clean. Ground-truth corroboration
  of the pre-existing failure at clean `fc7797f` (independent stash+rebuild+run, tree restored clean).

### FRAGO 024 — 2026-07-10 — session-id: `conductor-2026-07-10-m6-phase3b` (adds Phase 3c; plan.md body edit applied by re-dispatched executor)

- **Trigger.** FRAGO-023 Deviation-2 (the flagship `background ship.haul()` regression) was root-caused
  this session by an independent read-only investigation (IR-proven, git-bisected). User directed "hunt
  the regression now" → it is FIXED here, not deferred: this FRAGO inserts **Phase 3c** to close it,
  mirroring how FRAGOs 004/006/010 inserted Phases 1b/1c/4b.
- **Root cause (CONFIRMED, IR-proven).** `background ship.haul()` is an `Expr::MethodCall` (UFCS). The
  statement-level background give/copy ownership inference in `crates/ynz-typeck/src/check.rs:1388` only
  matches `Expr::Call`, so the receiver ident `ship`'s span is NEVER inserted into
  `background_arg_inferred_ownership`. Downstream, `crates/ynz-codegen/src/emit.rs:15896-15913`
  (`prepare_bg_arg_for_ctx`) gates the Shape heap-upgrade on span membership → absent → the receiver is
  passed as a RAW POINTER into `entrypoint`'s resume-fn stack frame (`%ship_own` alloca). When entrypoint
  returns Pending at `wait sleep(120)` that frame dies; the spawned task reads `self.name` from freed
  stack → empty string / garbage / crash. **A use-after-free in the flagship deliverable (Key Outcomes
  1 & 8).** Contrast: the Call-form `background haul(ship)` heap-upgrades correctly (30/30 green) — the
  ONLY codegen delta is the missing receiver upgrade.
- **Regression history.** CONGENITAL to `f921efe` (M6 P1-1) — a latent UAF that was timing-masked and
  genuinely passed 30/30 through `46906d1`. `070beca` (M6 P3) added the `TaskGenGuard` thread-local (zero
  codegen delta), which perturbed stack-reuse timing enough to expose it deterministically at HEAD. So
  the P3-seal "2355/0/6" baseline was a timing-masked pass, not a true green. This is a **genuine
  coverage gap, NOT a regression in Phase 1b/1c/1d** — those hardened arg-TYPE arms, all behind the same
  membership set only populated for `Expr::Call`; the UFCS receiver is a distinct arg slot that never
  enters it.
- **Phase 3c (the fix, applied to plan.md by a re-dispatched executor).** Extend `check.rs:1387-1442`'s
  background give/copy inference to ALSO match `Expr::MethodCall` with a shape receiver — normalize to
  `[receiver, ...args]` (mirroring codegen's existing `synthesize_ufcs_call_expr` at emit.rs:16211) and
  run the identical per-plain-ident inference that registers each ident span. Then the existing codegen
  Shape arm heap-upgrades the receiver exactly as the Call form does — NO codegen change, no new
  machinery (authoritative-derivation.md: reuse the one inference path + the one codegen normalization).
  The existing failing test `v03_m6_ufcs_background_spawned_method_call_runs` IS the locked RED that
  flips GREEN (verify-before-fix satisfied by the pre-existing test); add a give-`fixed<T>`-receiver /
  second-shape-field sibling case if cheap, and confirm the full suite returns to 0 real failures.
- **Classification.** Risk-NEUTRAL / risk-REDUCING: closes a UAF (memory-safety), adds no
  destructive/irreversible op, removes/weakens no mitigation, reuses the authoritative inference +
  codegen paths. Scope addition (a new phase) but squarely in-Mission (the flagship must deliver the
  correctness it was released to deliver). Auto-apply + log; no signature (no HIGH residual, no
  destructive op). The human was surfaced the root cause + fix approach before execution and directed the
  hunt; the Step-8 CONFIRM commit gate remains the human's review before anything seals.
- **Evidence.** Independent root-cause investigation this session (`git-bisect` monotonic boundary
  `46906d1` PASS → `070beca` FAIL, IR of `ynz_sm_entrypoint_resume` showing the raw-stack-pointer store +
  `arg_drop_count=0`; contrast IR of the Call form showing `ynz_alloc` + 24-byte arg-drop descriptor).
  Tree restored clean after the investigation.

### FRAGO 025 — 2026-07-10 — session-id: `conductor-2026-07-10-m6-phase3b` (extends Phase 3c scope; plan.md body edit applied by re-dispatched executor)

- **Trigger.** Phase 3c review fleet found a **LIVE use-after-free blocker** the `security` reviewer
  REPRODUCED in-container, plus a should-fix in the fix's own predicate. Blockers route to the fix loop;
  this FRAGO records the scope extension the blocker forces (Phase 3c statement-form-only → both spawn
  forms of the UFCS-receiver UAF) so the seam matches the fix. Conductor override note: `deviation-judge`
  classified the handle-form as a JUSTIFIED scope-out, but on the explicit factual premise that a silent
  `Type::Error` is "a benign compile-stop, not a runtime UAF." `security`'s **live reproduction** falsifies
  that premise (compiles clean, zero diagnostics, reads freed stack at runtime — a real UAF). Per
  verify-before-trust (a fellow agent's verdict is a claim; a live repro is ground truth), the conductor
  routes it as a confirmed blocker to the fix loop — NOT self-adjudicating a FRAGO's justification, but
  routing a reproduced live memory-safety defect, which is the conductor's job on a blocker.
- **Deviation 1 — Phase 3c scope extended to BOTH spawn forms (blocker fix).** The handle form
  `let h = background ship.haul()` is the SAME FRAGO-024 UAF class: `check_background_handle_spawn`
  (`crates/ynz-typeck/src/check.rs:1699-1811`) resolves the receiver/callee only for `Expr::Call`
  (registers ownership at ~:1723, callee_name at ~:1745) and returns `Type::Error` at ~:1754 on a STALE
  "already diagnosed by the Background arm" comment — but the Background arm (check.rs:2697) now accepts
  `Expr::MethodCall`, so NO diagnostic fires → codegen (`lower_let_background_handle`, emit.rs:~11985)
  spawns the receiver WITHOUT the heap-upgrade → raw pointer into the dead spawner frame. `security`
  reproduced empty `self.name` / zero-output heap-corruption live. **Fix:** thread the SAME statement-form
  normalization into `check_background_handle_spawn` — register the shape receiver span in `bg_inferred`
  AND resolve `callee_name` from the MethodCall method (the one authoritative inference path, no second
  derivation). Add a handle-form RED→GREEN repro.
- **Deviation 2 — narrowed-union receiver predicate (should-fix, fix's own code).** The Phase-3c
  `receiver_is_shape` predicate (check.rs:~1404) reads `self.scope.lookup(rname).ty` — a raw scope read
  that is NOT narrowing-aware, so a union receiver narrowed to a shape variant inside `if (x is Ship)`
  returns the un-narrowed union → predicate false → the same UAF for the narrowed-union subcase
  (authoritative-derivation: it must consume the narrowing-aware authoritative source codegen reads —
  `resolve_ident`/`self.expr_types` — not a second parallel derivation). **Fix:** thread the
  narrowing-aware authoritative type source (or add a parity link). Add a narrowed-union repro if cheap.
- **Deviation 3 — non-plain-ident receivers/args DEFERRED (four-field), flagged for the MILESTONE-seal
  decision.** `background fleet.flagship.haul()` / `ships[0].haul()` (and the Call-form equivalent
  `background haul(fleet.flagship)`) still ride as raw pointers — `is_heap_arg` (emit.rs:~15909) gates on
  `Expr::Ident`/explicit `.copy()`, dropping any field-access/index expr to no-heap-upgrade. **Pre-existing,
  shared by BOTH spawn forms, NOT introduced or widened by Phase 3c.** `security` could NOT reproduce a
  live UAF for the simple field-access case (the base local's storage survived) and `critical-path`
  couldn't confirm the full blast radius — so it is a latent asymmetry, not a confirmed-live blocker.
  Four-field deferral: **WHAT** — heap-upgrade non-plain-ident shape receivers/args in background-spawn
  position (both forms); **WHY** — needs new field-projection give/copy machinery beyond Phase 3c's
  give-transferred-plain-ident-receiver charter; building it now expands the phase for an unconfirmed-live
  exposure; **COST** — a dedicated fix (new codegen give/copy machinery for field/index/return-materialized
  receivers), ~1 phase; **TRIGGER** — a live UAF is reproduced for a non-plain-ident receiver, OR the
  milestone-seal review (per deviation-judge, route like the R13/R14 signed-risk overrides if confirmed
  live). SURFACED to the human at this boundary for the milestone-level call; homed durably here + to be
  echoed to Future Requirements by the fix-loop executor.
- **Deviation 4 — Call-only large-copy Tier-3 warning DEFERRED (minor, teaching-only).** The
  large-copy lint (check.rs:~2841) fires only for `Expr::Call`; a UFCS receiver >64 bytes gets no
  give-vs-copy teaching warning. No correctness/memory-safety impact. Four-field: **WHAT** — extend the
  large-copy lint to the UFCS receiver; **WHY** — teaching-parity only, zero safety impact, and the phase
  already built the `bg_args` normalization it would reuse; **COST** — small (<1 session); **TRIGGER** —
  whichever future phase next touches background-spawn UFCS diagnostics. Homed here + Future Requirements.
- **Classification.** Deviations 1-2: risk-REDUCING (close/​harden a UAF via the one authoritative path,
  no destructive op) → fix loop, no signature. Deviations 3-4: four-field deferrals (3 flagged for the
  milestone-seal human call). rules-compliance "missing fixture" blocker DISMISSED as a false negative
  (`v0_3_m6_ufcs_background_multifield.ynz` verified present on disk, 815 bytes, untracked; green-check ran
  the test green with a forced rebuild; test-quality/code-reviewer/acceptance-verifier all read it).
- **Evidence.** security live in-container repro of the handle-form UAF (empty `self.name`, nondeterministic
  zero-output heap corruption); rules-compliance narrowed-union static trace (`scope.lookup` vs
  narrowing-aware `resolve_ident`); code-reviewer/critical-path non-ident-receiver traces; fleet verdicts
  this boundary (security 1-blocker/1-should-fix/1-minor; rules-compliance 1-blocker[dismissed]/1-should-fix;
  code-reviewer 0-blocker/1-should-fix/1-minor; critical-path 0-blocker/1-should-fix; test-quality
  MEANINGFUL; acceptance-verifier MET; deviation-judge per-candidate; green-check green; graveyard clean).

### FRAGO 026 — 2026-07-10 — session-id: `conductor-2026-07-10-m6-phase3b` (fix-introduced OOB → fail-closed rejection; plan.md body edit + #21 rescope applied by re-dispatched executor)

- **Trigger.** Phase 3c fix-loop round-1 re-review: the memory-safety fleet converged (security + critical-path
  + rules-compliance + deviation-judge) that the narrowed-union background-receiver path FRAGO-025 round-1
  deferred as "silent-wrong (#21)" is in fact a **confirmed, reachable out-of-bounds read (CWE-125)** that
  the round-1 predicate hardening ITSELF introduced. Blocker → this round's fix loop.
- **Deviation — narrowed-union background-spawn shape receiver must be FAIL-CLOSED REJECTED now (in-scope
  blocker), not deferred as silent-wrong.** The round-1 `binding_ty_narrowed` predicate change newly routed a
  union receiver narrowed to a shape variant (`if (fig is Circle) { background fig.haul() }`) into codegen's
  `Type::Shape` heap-upgrade arm (`emit.rs:15918/15946`), which `build_load`s `sizeof(shape)` bytes from what
  is actually the **16-byte `{i64 tag, i64 data}` union envelope** — the shape payload lives behind the
  envelope's `data` pointer, ignored. Minimum shape size (64B, field-padded) > 16B envelope, so **every**
  narrowed-union shape receiver over-reads (security: 48B OOB for a 1-field shape, 176B for 3-field;
  critical-path: OOB for any variant >16B; both reproduced via IR). Compiles + runs today (typeck's Check 2b
  `UnsupportedCrossingLocalType` gate covers a union *crossing a wait*, not this heap-upgrade path). **This is
  a FIX-INTRODUCED gap on Phase 3c's OWN in-charter plain-ident-receiver surface** (`fig` is a plain ident) —
  round-1 converted the old dangling-pointer UAF into a heap-upgraded OOB — so per verification.md's
  mirror-case rule (a hole the last fix just opened is fix-now, not defer) and the plan's OWN signed
  **R14 / FRAGO 005 precedent** (un-embeddable `fixed<T>` crossings route to a deterministic teaching compile
  error — "in no case ship silently"), it is NOT eligible for the milestone-seal deferral #21 wrongly folded
  it into. **Fix (interim, in-scope, small):** at the shape-receiver predicate site, detect that the
  receiver's shape-ness came from union-narrowing (via the `binding_ty_narrowed` primitive round-1 already
  built) and route to a fail-closed teaching compile error (mirror Check 2b / FRAGO 005: "a union value
  narrowed to a shape can't yet be spawned as a background receiver — bind the inner value to a `let` first"),
  instead of the naive Shape heap-upgrade. This closes the OOB now, memory-safe + teaching-consistent.
- **#21 rescoped.** FRAGO-025 Deviation-3/#21 conflated the narrowed-union case (fix-introduced, in-charter)
  with the genuinely-pre-existing non-plain-ident case. Split them: (a) the interim narrowed-union
  **rejection** lands NOW (this FRAGO); (b) the DURABLE full fix — correct union-payload extraction so a
  narrowed-union receiver WORKS (reusing the existing `union_to_heap_cell` envelope+tag-resolved deep-copy at
  `emit.rs:3248/13069`, which already does exactly this for the let-bound union arg-escape case) — stays
  deferred (#21, now correctly scoped to "make it work," not "stop the OOB"); (c) the non-plain-ident
  receiver class (Deviation-3, both spawn forms, genuinely pre-existing) remains its own four-field deferral
  flagged for the milestone-seal call. The #21 text's "silent-wrong / lifetime-safe" framing is corrected to
  "confirmed OOB (CWE-125), interim fail-closed rejection landed FRAGO 026, full extraction deferred."
- **Classification.** Risk-REDUCING: closes a confirmed OOB read (memory-safety) via a fail-closed teaching
  error, no destructive op, reuses the round-1 detection primitive + the established Check-2b rejection
  pattern (authoritative-derivation-clean). In-scope blocker fix for Phase 3c (fix-introduced on its own
  surface), no signature (no HIGH residual — the OOB is CLOSED, not accepted). The `code-reviewer` 2 minors
  (Call-only large-copy warning #22; Call-only false-sharing crossing-shape collection) stay deferred —
  teaching/analysis-only, no correctness/safety impact.
- **Evidence.** security reproduced-IR OOB (`ynz_alloc(64)` + `load {double,[56xi8]}` from a 16-byte
  `{i64,i64}` envelope; 48B/176B over-read; CWE-125); critical-path IR confirmation (>16B variant OOB, free
  side consistent so no double-free/leak — the defect is a source over-read); deviation-judge MUST-REJECT-NOW
  (fix-introduced, in-charter plain-ident surface, R14/FRAGO-005 precedent, `union_to_heap_cell` exists);
  rules-compliance no-duct-tape blocker (deferral WHY written for the wrong sub-case). Handle-form UAF fix
  itself: code-reviewer clean, test-quality MEANINGFUL, security/critical-path handoff SOUND.

## Conductor cold-resume note — 2026-07-10 (context clear at the Phase-3 → Phase-3b boundary)

Supersedes the earlier "mid-Phase-1d-round-3" cold-resume note above (that one is historical).
Written at a clean, nothing-in-flight boundary. State + pointers for the next conductor; NOT standing law.

### State — clean break, nothing in flight
- **HEAD = `9be047d`.** Sealed this session: P1c (`759bd9b`), P1d + guard sweep (`5fd10f2`), planning
  coordination (`e6c80ac`), P2 block_on guard (`6390902`), store-site stopgap (`46906d1`), P3
  ABA/orphan (`070beca`), roadmap freshness (`9be047d`).
- **Done: Phases 0, 1, 1b, 1c, 1d, 2, 3 (+ store-site stopgap).** All the hard novel-machinery work.
- **Next: Phase 3b** — P2-5 recursion-chain × spike CPU-handle cleanup leak (LIVE, confirmed Phase 0 /
  FRAGO 001). Model tag `(coding, high, medium)`. Then 4, 4b, 5, 5b, 6, 6b, 7, 8.
- **Working tree: clean except 4 PRE-EXISTING not-mine dirty files** — `CLAUDE.md`, `Dockerfile`,
  `crates/ynz-watch/src/{error.rs,rebuild.rs}`. Do NOT touch/commit/stage these; they predate the session.

### ⚠️ MODEL — Fable for the EXECUTION agent (this session's hard-won fix)
- The global executor agent def (`claude-setup/agents/executor.md`, mirrors to `~/.claude/`) had its
  `model: "opus"` frontmatter pin **REMOVED this session** — that pin was silently beating the per-call
  override. With it gone, **dispatch executors with `model: "fable"` and they run Fable (VERIFIED via
  the human's UI).** The Agent-tool `model` enum only accepts `sonnet|opus|haiku|fable` — the alias
  `"fable"`, NOT the full `claude-fable-5` (which the schema rejects).
- **Only the EXECUTION agent goes on Fable** (human's explicit call). Review/other agents run on their
  own def defaults — no `model:` override.
- **Cannot self-verify a subagent's model** — the tool result never reports it; only the human's status
  bar shows it. Do not claim "on Fable" as verified without that confirmation.
- Main loop is Opus 4.8 · medium (human's session `/model`).
- **Fable window EXPIRES Mon 2026-07-13** (REF-model-selection hard-stop; reverts to Opus). Monday
  consideration: the executor def pin is still removed — decide whether to restore `model: "opus"` or
  let the REF reversion cover it.

### Gate override — NEXTEST (keep writing it)
All gates run `cargo nextest run --workspace` (+ `clippy --workspace -- -D warnings` no `--tests`, +
`fmt --all --check`), in the dev container: `docker compose run --rm dev ...`. The host has NO
cargo-nextest (all-docker); the global resolver emits slow `cargo test` — keep writing `nextest` into
executor + green-check dispatches. Baseline count at seal: **2355 passed / 0 failed / 6 skipped.**

### Discipline that held all session (keep it)
- **Money-adjacent concurrency → review fleet BEFORE seal.** Reviews caught a real issue every phase
  (P1d 27th slot, P2 mono-generic, stopgap generic over-reach, P3 gen-0 window). Do not trust an
  executor's own "green tree."
- **Conductor seals by name** (`git add <file> ...`, never `git add -A`), and gets the human's word
  before committing. Context-segment log is conductor-owned (executors correctly refuse to write it).

### Backlog / deferred (homed, not lost)
- **Brittle flake:** `ynz-typeck` `test_cross_file_reference_count_estimate_completes_fast`
  (`symbol_lookup.rs:547`, wall-clock `<5ms` assert) flaked once under full parallel load, passed on
  rerun — pre-existing, will flake gates again; candidate cleanup, out of M6 scope.
- **2 pre-existing `--all-targets` clippy lints** (`ynz-runtime/tests/m2_runtime.rs:275`, `lib.rs:2830`)
  — Phase-1d-era, NOT the documented gate; leave alone.
- **FRAGO 021/022** accepted as justified (factual citation fix / forced ABI salt-location / the
  conductor-directed gen-0 eliminate) — recorded.
- **int→number COERCION** deferred to stub plan `2026-07-04-v0-3-hotfix-int-literal-number` (3 facets:
  store-site #9, call-site #14, field-default); **#18 decimal128 by-value return + #19 map<number> key
  hashing** homed on the concurrency-perf roadmap Capability Ledger.

## Session log (continued)

- `executor-2026-07-10-m6-phase3b-seg1` — 2026-07-10 — Phase 3b segment 1 (fresh dispatch;
  checkpointed at the step 1→2 boundary on the context budget; `STATUS: PARTIAL`, resume-at
  `phase-3b/step-2`, relay `handoff-phase-3b.md`). Step 1 (CCIR-1) COMPLETE against HEAD
  `fc7797f`: all six cited anchors re-verified — three trivial line-shift drifts corrected
  (`runtime.rs:607`→`:629` root cleanup call; `runtime.rs:659-680`→`:677-707` chain walk;
  `may_block.rs:1617`→`:1638` SCC `len() >= 2` filter), `cpu_admission.rs:508-534` and
  `queries.rs:900-917` MATCH, and the stale `queries.rs:941-944` comment CONFIRMED present +
  stale (cpu_admission.rs:109-152 documents the M3g Phase 3 decline removal). Leak mechanism
  re-confirmed in the live tree: chain walk frees each child's sleep handle + frame, never its
  spike CPU handles. Key orientation finding: `YNZ_ALLOC_COUNTER` sees only `ynz_alloc`/`ynz_free`
  — `Box<CpuJoinHandle>` (Rust allocator) is invisible to it, so the RED fixture requires the
  plan's handle-count-instrumentation branch (env-gated `handle_alloc=`/`handle_free=` counters
  mirroring the alloc-counter pattern; design settled as D-3b-1..6 in the handoff, including the
  full RED fixture shape/timing, integration + unit test designs, the one-choke-point fix site,
  and the queries.rs comment rewrite). Confirmed `sleepBlocking` ∉ `BASE_SUSPENSION_INTRINSICS`
  (deterministic CPU-member duration is legal). No code touched; tree unchanged (4 pre-existing
  not-mine dirty files untouched). No deviation surfaced — anchor drifts are the trivial class
  Phase 0 precedented.

- `executor-2026-07-10-m6-phase3b-seg2` — 2026-07-10 — Phase 3b segment 2 (resumed at
  `phase-3b/step-2` from `handoff-phase-3b.md`; inherited seg-1 receipts, re-verified nothing
  already receipted). Steps 2–5 COMPLETE; phase DONE; relay deleted as final act.
  **Step 2 (RED, non-vacuous):** landed D-3b-1 (env-gated `CpuJoinHandle` parity counters —
  `YNZ_HANDLE_ALLOC_COUNT`/`YNZ_HANDLE_FREE_COUNT` statics in `lib.rs`, alloc recorded in
  `CpuJoinHandle::new`, `Drop` made UNCONDITIONAL as the one free choke point, `handle_alloc=`/
  `handle_free=` lines appended after `alloc=`/`free=` in the shutdown counter dump — prefix-parser
  safe), D-3b-2 (fixture `v0_3_m6_recursive_spike_cancel.ynz`), D-3b-3 (4 integration tests in
  `v03_m6_recursive_spike_cancel.rs`: parity gate, positive control, `YNZ_SKIP_RECURSION_DROP`
  negative control, frame alloc=free guard), D-3b-4 (deterministic unit test
  `recursion_chain_child_spike_handles_freed_on_drop`; `SpawnStateFnFuture::new` test-ctor
  vestigial `rec_slot: *mut u8` → `recursion_slot_offset: i64`, 3 call sites now pass `-1`).
  RED evidence: unit probe `0 != 1`; integration `handle_alloc=4, handle_free=2`; positive control
  PASSED (`handle_alloc>=4`) → Phase-0 reachability claim CONFIRMED (pre-registered falsification
  conditional did NOT fire). **Step 3 (fix):** `cleanup_spike_cpu_handles(child_ptr)` threaded into
  the chain-walk loop (after child sleep-handle free, before grandchild read) — the SAME helper the
  root uses at drop step 1.5; docs updated (Drop free-order ladder + the helper's "Called from",
  which now names both grains and the never-duplicate rule). GREEN: unit probe `1`; integration
  `4/4`; frame parity `3/3`. Debug detour worth recording: the first post-fix nextest run still
  showed `4/2` — root-caused to a STALE RUNTIME EMBED (`ynz-driver` `include_bytes!`s
  `target/debug/libynz_runtime.a`; the nextest flow had not re-embedded the fixed archive). Fixed
  by forced rebuild (`touch crates/ynz-driver/build.rs && cargo build -p ynz-driver`) — same
  staleness class as the CLAUDE.md `target/release` consumer-mount note, at debug grain. A
  dead-end hypothesis (composed `cpu_group_slots` offsets diverging from the runtime's canonical
  `SPIKE_HANDLE_BASE_OFFSET` scan) was checked and FALSIFIED: `emit.rs` places `cpu_reserve`
  immediately after the frame header, so handle slots ARE canonical. **Step 4:** stale
  `queries.rs:941-944` "structurally inert" comment corrected to current truth (M3g Phase 3 flip
  removed the co-resident-suspension decline in both arms; admitted groups DO fire). **Step 5
  (gates, all in docker):** `cargo fmt --all` clean (trivial rewraps of this phase's own files);
  `cargo clippy --workspace -- -D warnings` clean; full `cargo nextest run --workspace
  --no-fail-fast`: **2360 run / 2359 passed / 1 failed / 6 skipped** (baseline 2355 + this phase's
  5 new tests). M3g/M4 recursion + CPU-group suites and the 3 `recursion_cancellation*` tests all
  pass. **DEVIATION SURFACED (not self-adjudicated):** the single failure,
  `v03_m6_ufcs_background_spawned_method_call_runs` (M6 Phase-1-era test), is PRE-EXISTING —
  plan/dispatch said baseline "2355 passed / 0 failed"; reality is this test fails
  deterministically under nextest on the live tree, BISECT-PROVEN: with this phase's 3 source-file
  diffs reverted (`git checkout`) and the runtime force-re-embedded, it still fails 2/2 runs
  (symptom: spawned `ship.haul()` prints an EMPTY line instead of "Mon"; exit 1 under the harness,
  exit 0 with the same empty print when run manually). Not caused by, and not fixed by, this
  phase's diff — for the deviation-judge → conductor seam to adjudicate (candidate suspects
  include host-environment drift; the pre-existing not-mine dirty `Dockerfile` is noted for the
  seam, not investigated). This phase's touched files for the boundary commit:
  `crates/ynz-runtime/src/lib.rs`, `crates/ynz-runtime/src/runtime.rs`,
  `crates/ynz-typeck/src/queries.rs`, `crates/ynz-driver/tests/v03_m6_recursive_spike_cancel.rs`
  (new), `crates/ynz-driver/tests/fixtures/v0_3_m6_recursive_spike_cancel.ynz` (new), plus
  `plan.md`/`audit.md`. Nothing committed (conductor seals). Session-id appended to `plan.md`
  frontmatter in the same action as this entry.

- `executor-2026-07-10-m6-phase3b-fixloop1` — 2026-07-10 — Phase 3b fix-loop round (review-fix
  cleanups + FRAGO 023 plan.md body edit, per the conductor's classified instruction).
  **Hot-path should-fix (rules-compliance + test-quality):** `SpawnStateFnFuture::drop`'s uncached
  per-drop `std::env::var("YNZ_SKIP_RECURSION_DROP")` read (process-env lock + String alloc on
  every task completion/cancellation) replaced with a once-latched `static AtomicBool`
  (`SKIP_RECURSION_DROP` + `skip_recursion_drop()` + `init_skip_recursion_drop_flag()` in
  `lib.rs`, co-located with sibling `ALLOC_COUNTER_ENABLED`; latched in `ynz_rt_init` right beside
  `init_alloc_counter_flag()` — ONE authoritative cached-flag pattern, not a second scheme).
  Env-var name + semantics unchanged (non-empty ⇒ skip; default false ⇒ walk always runs; tests
  set the var before the child process starts, so init-time latching is behavior-identical).
  **Timing-triage comment** added in `v03_m6_recursive_spike_cancel.rs` ahead of the cancel-timing
  tests: repeated CI flakes there are timing-margin drift (±300ms fixture margins; widen the
  burn/cancel windows), not dismissible — `recursion_chain_child_spike_handles_freed_on_drop` is
  the primary timing-independent proof. Comment only, no assertion change. **FRAGO 023 body edit
  applied:** Phase 3b exit-criterion #4 rewritten from "full suite green" to the honest reality
  (Phase 3b's own work green; one corroborated-pre-existing orthogonal failure
  `v03_m6_ufcs_background_spawned_method_call_runs` tracked per FRAGO 023 Deviation 2, under
  investigation this session, not deferred); clauses 1–3 intact. **Completion note finalized**
  (0-blocker fleet outcome + FRAGO 023 adjudication + this round's cleanups + re-run gate counts).
  **Gates re-run (docker, nextest):** build green; `cargo nextest run --workspace --no-fail-fast`
  **2360 run / 2359 passed / 1 failed / 6 skipped** — sole failure is the tracked pre-existing
  `v03_m6_ufcs_background_spawned_method_call_runs`; clippy `--workspace -- -D warnings` clean;
  `fmt --all` + `--check` clean. Files touched this round: `crates/ynz-runtime/src/lib.rs`,
  `crates/ynz-runtime/src/runtime.rs`, `crates/ynz-driver/tests/v03_m6_recursive_spike_cancel.rs`,
  `plan.md`, `audit.md`. The 4 pre-existing not-mine dirty files untouched; nothing committed
  (conductor seals). Session-id appended to `plan.md` frontmatter in the same action as this entry.

- `conductor-2026-07-10-m6-phase3b` — 2026-07-10 — **Phase 3b SEALED at commit `1b7e567`** (boundary
  commit, `Plan-Phase: …#3b` trailer accessor-verified). Fleet at the boundary: 0 blockers
  (code-reviewer clean; critical-path-integrity 0-blocker; test-quality MEANINGFUL; rules-compliance
  0-blocker after the hot-path fix-loop round; deviation-judge JUSTIFIED+risk-neutral; graveyard clean;
  green-check green modulo the 1 pre-existing red). Two boundary minors dispositioned no-fix (YAGNI
  ceiling): (1) rules-compliance CARVE-OUT restatement — `ynz-runtime` is outside the scattered-registry
  Bouncer grep scope and the sibling alloc-counter statics carry the same rationale; (2)
  critical-path-integrity empty-predicate confirm — resolved (new `!is_empty()` mirrors
  `init_alloc_counter_flag`; negative control passes). **FRAGO 023 filed** (risk-neutral, auto-applied):
  clause-4 "full suite green" reframed + the flagship regression recorded. **PIVOT (user-directed: hunt
  the regression now):** next action is a root-cause investigation of FRAGO-023 Deviation-2 —
  `v03_m6_ufcs_background_spawned_method_call_runs` (`background ship.haul()` loses its output; Key
  Outcomes 1 & 8), corroborated pre-existing at `fc7797f`. NOT advancing to Phase 4 until the regression
  is scoped.

- `executor-2026-07-10-m6-phase3c` — 2026-07-10 — Phase 3c (FRAGO 024: `background ship.haul()`
  UFCS-receiver use-after-free). **Authored Phase 3c into plan.md** (inserted after Phase 3b,
  before Phase 4, per FRAGO 024's classified instruction — same shape as the neighboring inserted
  phases: Task + purpose / Steps / Exit criteria / Reviewer fan-out / Model tag `(coding, high,
  medium)`). **Fix implemented in the ONE authoritative typeck inference path**
  (`crates/ynz-typeck/src/check.rs`, statement-level background give/copy block): the spawn target
  is normalized to its Call-form argument list — `Expr::Call` args as-is; a shape-receiver
  `Expr::MethodCall` (plain-ident receiver whose scope type is `Type::Shape`, the same pre-infer
  scope-lookup source the sibling channel gate already uses) contributes `[receiver, ...args]`,
  the typeck twin of codegen's existing `synthesize_ufcs_call_expr` — then the IDENTICAL
  per-plain-ident inference loop runs over the normalized list, so the receiver's span enters
  `background_arg_inferred_ownership` and codegen's existing Shape arm heap-upgrades it exactly as
  the Call form. ZERO codegen change; no second normalization scheme; no ad-hoc receiver pre-gate
  (authoritative-derivation.md). **Verify-before-fix:** RED re-confirmed pre-fix on the live tree
  at HEAD `1b7e567` with a forced runtime-then-driver rebuild (fresh `libynz_runtime.a` embed) —
  `v03_m6_ufcs_background_spawned_method_call_runs` FAILED (fixture exit 1, lost `Mon` output);
  GREEN post-fix (PASS). The locked RED test untouched/un-weakened. **Sibling coverage added:**
  `v03_m6_ufcs_background_give_receiver_multifield_survives_spawner_frame` + fixture
  `v0_3_m6_ufcs_background_multifield.ynz` (task reads BOTH receiver fields — string + int — after
  its own suspension; locks the whole-struct heap upgrade). **Gates (docker, nextest):**
  `cargo nextest run --workspace --no-fail-fast` **2361 run / 2361 passed / 0 failed / 6 skipped**
  (baseline 2360 + 1 new test; 0 real failures — the FRAGO 023 "1 tracked pre-existing failure"
  reframe is retired; Call-form background fixtures + Phase 3b recursion-spike suite
  regression-free; the known brittle `symbol_lookup` wall-clock flake did not fire); clippy
  `--workspace -- -D warnings` clean; `fmt --all` + `--check` clean. **Three sibling gaps
  SURFACED for the deviation-judge → conductor seam (not self-adjudicated):** (1) handle-form
  `let h = background ship.haul()` silently types the handle `Type::Error`
  (`check_background_handle_spawn` resolves no callee for a MethodCall inner; its "already
  diagnosed by the Background arm" comment is stale — the arm accepts UFCS since P1-1); (2)
  NON-plain-ident shape receivers/args (`background fleet.flagship.haul()`, equally Call-form
  `background haul(fleet.flagship)`) still ride membership-less as raw pointers — a pre-existing
  class shared by BOTH spawn forms, not introduced or widened here, needing field-projection
  give/copy machinery; (3) the large-copy Tier-3 warning loop is `Expr::Call`-only
  (teaching-parity gap for a copy-inferred UFCS receiver). Files touched for the boundary commit:
  `crates/ynz-typeck/src/check.rs`, `crates/ynz-driver/tests/v03_m6_ufcs_suspension.rs`,
  `crates/ynz-driver/tests/fixtures/v0_3_m6_ufcs_background_multifield.ynz` (new), plus
  `plan.md`/`audit.md`. The 4 pre-existing not-mine dirty files untouched; nothing committed
  (conductor seals). Session-id appended to `plan.md` frontmatter in the same action as this entry.

- `executor-2026-07-10-m6-phase3c-fix1` — 2026-07-10 — Phase 3c fix-loop round 1 (FRAGO 025:
  handle-form UAF blocker + narrowed-union predicate should-fix). **Deviation 1 CLOSED:** the spawn
  target normalization is now the ONE shared helper `Checker::background_spawn_call_form`
  (`crates/ynz-typeck/src/check.rs` — the typeck twin of codegen's `synthesize_ufcs_call_expr`),
  consumed by BOTH spawn forms: the statement path's give/copy inference block now calls it, and
  `check_background_handle_spawn` pre-records ownership over the normalized `[receiver, ...args]`
  list (the shape receiver's span enters `background_arg_inferred_ownership`, so codegen's existing
  Shape arm heap-upgrades it — ZERO codegen change) AND resolves `callee_name` from the same
  normalization (UFCS callee = method name), retiring the stale "already diagnosed by the
  Background arm" comment. **Verify-before-fix:** new handle-form repro
  `v03_m6_ufcs_background_handle_receiver_survives_spawner_frame` + fixture
  `v0_3_m6_ufcs_background_handle.ynz` FAILED pre-fix on the live tree with a fresh
  runtime-then-driver rebuild — compiled clean (zero diagnostics, exit 0) and printed
  `"\n7\ndone\n"` (empty `self.name` read from the dead spawner frame — the exact
  security-reproduced signature); PASSES post-fix (`"Mon\n7\ndone\n"`). **Deviation 2 CLOSED at
  both sites:** the shape-receiver predicate reads `Checker::binding_ty_narrowed` (the
  `union_narrowed` overlay over the scope entry — the same overlay order `resolve_ident` applies;
  `resolve_ident`'s narrowing head now delegates its overlay branch to the same helper so the two
  readers cannot drift) instead of the raw `scope.lookup`; both spawn forms consume it through the
  one normalization helper. **Narrowed-union repro: CANDIDATE, not a test** — probed live
  post-fix: the narrowed spawn compiles, registers ownership, and heap-upgrades (deterministic
  output, no dead-frame ride — risk-reduced vs the pre-fix raw-pointer ride), but the heap copy
  duplicates the union's `{tag,data}` envelope bytes misread as the variant shape (probe printed
  `0` for `radius: 5.0`): correct payload needs union-payload-extraction machinery, the same
  new-machinery family as FRAGO 025 deviation 3 — SURFACED for the seam (recorded on Future
  Requirements #21), not silently fixed. **Gates (docker, nextest):**
  `cargo nextest run --workspace --no-fail-fast` **2362 run / 2361 passed / 1 failed / 6 skipped**;
  the 1 failure = `ynz-runtime::m2_spike sync_bridge_overhead_measurement`, a wall-clock
  overhead-measurement test that flaked under full parallel load and PASSES in isolation on rerun
  (verified, not assumed — same brittleness class as the known `symbol_lookup` flake) → **0 real
  failures** (baseline 2361 + 1 new test). Clippy `--workspace -- -D warnings` clean; `fmt --all`
  + `--check` clean. No existing test edited or weakened. **Plan edits (same dispatch):** Phase 3c
  Task+purpose/Steps/Exit-criteria extended to BOTH spawn forms + predicate hardening; fix-loop
  round 1 completion note appended; FRAGO 025 deviations 3 & 4 echoed to Future Requirements
  #21/#22 as four-field entries (#21 stays flagged for the milestone-seal human call). Files
  touched for the boundary commit: `crates/ynz-typeck/src/check.rs`,
  `crates/ynz-driver/tests/v03_m6_ufcs_suspension.rs`,
  `crates/ynz-driver/tests/fixtures/v0_3_m6_ufcs_background_handle.ynz` (new), plus
  `plan.md`/`audit.md`. The 4 pre-existing not-mine dirty files untouched; nothing committed
  (conductor seals). Session-id appended to `plan.md` frontmatter in the same action as this entry.
- `executor-2026-07-10-m6-phase3c-fix2` — 2026-07-10 — Phase 3c fix-loop round 2 (FRAGO 026:
  narrowed-union background receiver = confirmed reachable OOB read, CWE-125, fix-introduced by
  round 1's predicate hardening → interim fail-closed rejection). **CLOSED fail-closed:** the ONE
  spawn normalization (`Checker::background_spawn_call_form`, `crates/ynz-typeck/src/check.rs`)
  now detects a receiver whose shape-ness comes from union-narrowing (the `union_narrowed`
  overlay — the exact source `binding_ty_narrowed` reads; no second detection derivation) and
  emits a WHAT/WHAT-INSTEAD/WHY teaching compile error instead of routing into codegen's
  `Type::Shape` heap-upgrade — BOTH spawn forms through the one shared helper, ZERO codegen
  change. Verify-before-fix: probed live pre-fix — the narrowed spawn compiled clean and ran,
  printing `0` for `radius: 5.0` (64-byte shape loaded from the 16-byte `{tag,data}` union
  storage); post-fix a deterministic teaching error, exactly ONE diagnostic per spawn site.
  RED→GREEN: `v03_m6_ufcs_background_narrowed_union_receiver_rejected_both_forms` + fixture
  `v0_3_m6_ufcs_background_narrowed_union.ynz` (both forms, distinct variants + no-double-emission
  count asserted). Gallery: `examples/primantis-orders/m6_errors.ynz` trigger added (10
  diagnostics) + phrase assertion in `error_galleries.rs`. Registry: per-site dynamic message —
  the `[[diagnostic_template]]` carve-out (Check 2b / M6 teaching-error convention), no entry.
  Gates (docker, nextest, forced runtime→driver rebuild): `cargo nextest run --workspace
  --no-fail-fast` **2363 run / 2363 passed / 0 failed / 6 skipped** (baseline 2362 + 1 new test;
  round-1's `sync_bridge_overhead_measurement` flake passed this run); clippy `-D warnings`
  clean; `fmt --all` + `--check` clean; plain-shape statement/multifield/handle + Call-form
  background fixtures green un-weakened. **Deviation surfaced (FRAGO 026's prescribed
  WHAT-INSTEAD falsified live):** `let ship: Circle = fig` inside the `is` arm does NOT extract
  the payload (the re-bind copies the union storage; prints `0` with no spawn involved) — the
  shipped message steers to a shape-typed binding at the value's creation site (probe-verified
  working) instead. **Two sibling union-payload surfaces probed + SURFACED for the seam
  (pre-existing, same family as FR #21, NOT fixed):** (a) direct narrowed field access
  (`fig.radius` inside `is Circle`) silently prints `0` for `5.0`; (b) Call-form
  `background work(fig)` with a give-transferred UNION arg runs but the task's tag-match
  produces NO output (expected `circle`). Plan edits applied per FRAGO 026's classified
  instruction: round-1 completion-note framing corrected (OOB, not silent-wrong/lifetime-safe),
  round-2 completion note added, FR #21 rescoped to the durable union-payload extraction, the
  non-plain-ident class split to NEW FR #23 (still milestone-seal-flagged), the round-1 note's
  `#21/#22` echo reconciled to `#23/#22`, and one Teaching-invariant bullet added for the new
  diagnostic class. Files touched: `crates/ynz-typeck/src/check.rs`,
  `crates/ynz-driver/tests/v03_m6_ufcs_suspension.rs`,
  `crates/ynz-driver/tests/fixtures/v0_3_m6_ufcs_background_narrowed_union.ynz` (new),
  `crates/ynz-driver/tests/error_galleries.rs`, `examples/primantis-orders/m6_errors.ynz`, plus
  `plan.md`/`audit.md`. The 4 pre-existing not-mine dirty files untouched; nothing committed
  (conductor seals). Session-id appended to `plan.md` frontmatter in the same action as this entry.
- `executor-2026-07-10-m6-phase3c-polish` — 2026-07-10 — Phase 3c final polish round (teaching-text
  polish + honest deferral homing only; ZERO detection/rejection logic change, no test weakened).
  **(1) WHAT-INSTEAD reworded (security should-fix):** the FRAGO 026 narrowed-union rejection's
  WHAT-INSTEAD (`crates/ynz-typeck/src/check.rs`, `background_spawn_call_form`) previously led with
  "Keep the value in a `<Variant>`-typed binding…", misreadable as "re-bind here" —
  `let inner: Circle = fig` inside the `is` arm, which security reproduced as a SIGSEGV (the 16-byte
  union envelope copied into a shape-sized binding → OOB read on a pointer field). New text states
  the probe-verified working pattern (spawn on the original `<Variant>`-typed binding where the
  value is created, BEFORE the union store) AND explicitly warns against the re-bind, naming it
  inline (`let inner: <Variant> = <narrowed>` → "would hold the union's storage, not a `<Variant>`
  value"). WHAT + WHY unchanged; the content-specific asserted phrase ("a union value narrowed to
  `<Variant>` cannot yet be used as a `background` receiver") unchanged — rejection test +
  m6-gallery assertions pass un-edited, nothing weakened. **(2) Deferral cluster homed honestly
  (deviation-judge should-fix):** the round-2 sibling surfaces split by orthogonality — NEW FR #24
  now carries (a) narrowed direct field access silent-wrong (`fig.radius` prints `0` for `5.0`, no
  spawn) and (b) union→shape re-bind OOB/SIGSEGV (CWE-125, no spawn), each with the explicit
  "pre-existing GENERAL union-narrowing bugs, NOT concurrency defects, orthogonal to M6's
  concurrency charter" callout (the #18/#19/#20 mold) so no future owner mis-triages them as
  spawn-family cleanup; (c) the give-transferred union-arg surface (concurrency-adjacent) stays on
  FR #21, which now cross-references #24 (one `union_to_heap_cell` extraction machinery closes
  both). CANDIDATE recorded inside #24, surfaced not self-decided: a cheap interim fail-closed
  rejection of the (b) re-bind (FRAGO 026's precedent) for whoever owns the class — (a)/(b) remain
  reachable in plain non-concurrent code today. **(3) Example cleanliness:** fixture
  `v0_3_m6_ufcs_background_narrowed_union.ynz` + the m6-gallery shapes (`m6_errors.ynz`) swapped
  `float`→`int` fields so a copied example doesn't hit the SEPARATE pre-existing global
  `float.toString` bug (`print(f.toString())` on a float prints `0.0`) — surfaced as a one-line
  candidate for the seam, far out of M6 scope, NOT fixed. **Gates (docker, nextest, forced
  runtime→driver rebuild):** `cargo nextest run --workspace --no-fail-fast` **2363 run /
  2363 passed / 0 failed / 6 skipped** (baseline held, 0 real failures); clippy `--workspace
  -- -D warnings` clean; `fmt --all` + `--check` clean. Files touched:
  `crates/ynz-typeck/src/check.rs`,
  `crates/ynz-driver/tests/fixtures/v0_3_m6_ufcs_background_narrowed_union.ynz`,
  `examples/primantis-orders/m6_errors.ynz`, plus `plan.md`/`audit.md`. No `## Context-segment log`
  entry written (conductor-owned); FRAGO 023–026 + prior session-log content left intact. The 4
  pre-existing not-mine dirty files untouched; nothing committed (conductor seals). Session-id
  appended to `plan.md` frontmatter in the same action as this entry.
- `m6-fr24-crossplan-lift-2026-07-11` — 2026-07-11 — Cross-plan durable-home lift (DOC-ONLY, no
  code): FR #24 (general union-narrowing payload-extraction defect class — narrowed direct field
  access silent-wrong + union→shape re-bind OOB/SIGSEGV, both reproducing with NO concurrency)
  lifted to the roadmap's durable store so it survives this plan's archival to `done/`. Roadmap
  `2026-05-21-v0-3-concurrency-perf`: a pointer row added to BOTH Capability Ledger tables (after
  the two M6 decimal128 rows in each; status **unscoped → needs a milestone**, with the explicit
  "pre-existing GENERAL union-narrowing memory-safety/correctness defects, NOT concurrency
  defects, do not mis-triage as spawn/concurrency cleanup" callout and the `union_to_heap_cell`
  (`emit.rs:3248`) reuse pointer shared with FR #21), and the faithful four-field
  WHAT/WHY/COST/TRIGGER payload appended to the roadmap's `audit.md` as a dated ledger amendment
  under Idempotency-Key `2026-07-04-v0-3-m6-concurrency-hotfix#24: union-narrowing-payload-extraction`
  (the re-run sentinel — a later Phase-8 deferral lift finds it present and skips re-appending).
  The ONLY M6-plan edit: a one-line LIFTED cross-reference appended at the end of plan.md FR #24
  (FR #24 not otherwise restructured; FR #21's cross-refs intact). RECORD-ONLY transcription of an
  already-decided deferral — no adjudication, no FRAGO needed (no plan content changed beyond the
  cross-reference; the deferral itself was decided and homed at the Phase 3c polish round). No
  `## Context-segment log` entry written (conductor-owned); FRAGO + prior session-log content left
  intact. The 4 pre-existing not-mine dirty files untouched; nothing committed (conductor seals).
  Session-id appended to `plan.md` frontmatter in the same action as this entry.

## Conductor cold-resume note — 2026-07-11 (context clear at the Phase-3c → Phase-4 boundary)

Supersedes the earlier P3→P3b cold-resume note. Clean, nothing-in-flight boundary. State + pointers for the
next conductor; NOT standing law.

### State — clean break, nothing in flight
- **HEAD = `4a13241`.** Sealed since the last note: **P3b** (`1b7e567` — recursion-chain spike CPU-handle
  cleanup leak, one-choke-point fix + RED→GREEN); **P3c** (`03273be` — the flagship `background x.method()`
  UFCS-receiver use-after-free, BOTH spawn forms, + narrowed-union OOB fail-closed teaching rejection; 4
  fix-loop rounds); user's out-of-band commits `aabb5bc`/`9893406` (ynz-watch rebuild + docker nextest, the
  ex-"not-mine" files) and `4a13241` (FR#24 roadmap lift).
- **Done: Phases 0, 1, 1b, 1c, 1d, 2, 3, 3b, 3c.** M6 suite is now GENUINELY green — **2363 passed / 0
  failed / 6 skipped**. (The old "2355/0/6" P3 baseline was a timing-masked pass — the flagship UFCS test
  `v03_m6_ufcs_background_spawned_method_call_runs` was already RED at `fc7797f`; P3c is what actually
  closed it. Corroborated by independent clean-HEAD reproduction.)
- **Next: Phase 4** — P3-2 `ynz_channel_recv_poll` lost-wakeup window (register-before-poll). Then **4b, 5,
  5b, 6, 6b, 7, 8** (see plan.md `#### Phase` headers, lines ~1632–1884).
- **Working tree: CLEAN** except this one uncommitted M6 `audit.md` (this note + the FR#24-lift session-log
  line), which the conductor seals next. The 4 formerly-"known-not-mine" dirty files (CLAUDE.md, Dockerfile,
  ynz-watch/{error,rebuild}.rs) are NOW ALL COMMITTED by the user — the next chat no longer needs to avoid them.

### ⚠️ MODEL — Fable for the EXECUTION agent (unchanged); reviewers on their own defaults
- Dispatch executors with `model: "fable"` (the alias, not `claude-fable-5`). ONLY the execution agent; all
  review/gate agents run their own def defaults (no `model:` override). This session honored that throughout.
- **Fable window EXPIRES Mon 2026-07-13** (REF-model-selection hard-stop → reverts to Opus). Two days left.
- Main loop was Opus 4.8 · medium this session.

### Gate override — NEXTEST, ALL IN DOCKER (keep writing it)
Host has NO cargo/nextest. Every gate runs `docker compose run --rm dev cargo nextest run --workspace`
(+ `clippy --workspace -- -D warnings`, + `fmt --all --check`). The driver `include_bytes!`s
`libynz_runtime.a`, so a typeck/runtime change needs a FORCED runtime→driver rebuild or you test a stale
embed (this bit us mid-P3b/P3c — force it). Baseline at seal: **2363 passed / 0 failed / 6 skipped.**

### Discipline that held (keep it)
- **Money-adjacent concurrency → FULL review fleet BEFORE seal.** It caught a real issue every phase this
  session: P3c's flagship UAF, the handle-form twin, the fix-INTRODUCED OOB (CWE-125), and a teaching
  message that would've steered users into a segfault. Do NOT trust an executor's own "green tree" — an
  independent green-check + the fleet is the seal gate.
- **Verify a surprising claim against ground truth** — the "pre-existing" baseline claim, the falsified
  fix-spec, the "missing fixture" false-negative were all resolved by reproduction, not by trusting a
  subagent's word. A router's own authored fix-spec is a claim too (my FRAGO-026 WHAT-INSTEAD was falsified
  live by the executor — good).
- **Conductor seals by name** (`git add <file> …`, never `-A`), human confirms before commit.

### Deferrals / open decisions (homed, not lost)
- **FR #24 — union-narrowing payload-extraction defect CLUSTER: LIFTED + committed to the roadmap**
  (`2026-05-21-v0-3-concurrency-perf/audit.md` + Capability Ledger, key
  `2026-07-04-v0-3-m6-concurrency-hotfix#24: union-narrowing-payload-extraction`, **unscoped → needs a
  milestone**). A GENERAL memory-safety class, ORTHOGONAL to M6 concurrency: (a) narrowed field access
  `fig.radius` silently prints 0; (b) `let inner: Circle = fig` then a pointer-field read SIGSEGVs (CWE-125).
  Reproduce with NO background/spawn. **Patrick's call to schedule its own milestone.**
- **FR #21** (narrowed-union durable extraction so it WORKS, reusing `union_to_heap_cell` emit.rs:3248),
  **#23** (non-plain-ident receivers, both spawn forms), **#22** (Call-only large-copy warning) — all in M6
  plan.md Future Requirements, flagged for the **milestone-seal human call** (Phase 8 lifts the full set to
  the roadmap per Key Outcome 11; #24 already lifted early to de-risk it).
- **Also noted (out of M6 scope, NOT fixed):** a pre-existing global `float.toString()` bug — `let f: float
  = 5.0; print(f.toString())` prints `0.0`. Candidate only; recorded in the P3c polish round.
- Prior deferrals still standing: int→number COERCION (stub plan `2026-07-04-v0-3-hotfix-int-literal-number`);
  #18 decimal128 by-value return + #19 map<number> key hashing (roadmap ledger).
