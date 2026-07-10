---
name: "v0-3-m6-concurrency-hotfix-audit"
plan-id: "2026-07-04-v0-3-m6-concurrency-hotfix"
created_at: "2026-07-09"
updated_at: "2026-07-09"
metadata:
  type: "audit"
---

# Audit sidecar — v0.3-M6 Concurrency Hotfix

Append-only execution history for the M6 hotfix plan. `plan.md` is current truth;
this file is the session + FRAGO + context-segment log.

## Session log

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
