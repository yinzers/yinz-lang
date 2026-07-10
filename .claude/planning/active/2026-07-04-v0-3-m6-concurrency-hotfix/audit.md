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
- **segment verdict:** `STATUS: PARTIAL`. Landed: steps 5–6 — ALL 9 FRAGO-003 sites threaded; **grep gate PASS** (one classifier + one AST wrapper; only name-keyed producer is the ratified pre-typing `may_block.rs` fixpoint edge; zero second derivation); check-3 reordered after `check_stmts`. RED class **3/4 GREEN** (a/c/d); fixture (b) still RED for a NEW reason. **R1: zero regression** in pre-existing Call-based suspension fixtures. House clippy green. Step 8 recorded (pre-fix mode PANIC-then-abort). Probes: name-collision SOUND; bare-UFCS-crossing local half RESOLVED-BY-THREADING. **SURFACED DEVIATION → FRAGO 004 (below):** shape-value arg to a suspending callee staged in the parent resume fn's stack alloca; child frame holds `ptrtoint`; parent Pending → stack dies → child resumes on dangling `self` → silent garbage (UAF). **CONFIRMED by adversarial code-reviewer** (agent `ae5ab98a2e94c4182`): reproduces in pure `Call` form AND auto-inserted transitive suspension; **PRE-EXISTING in shipped v0.3.0** (stash-to-`main` still garbage, identical IR), NOT introduced by M6; likely also affects `fixed<T>`; safe for string/array/map (heap-backed). Root cause: `locals_crossing_wait`/`collect_crossings_in_stmts` lexical read-after scan misses escape-through-a-callee-frame. deviation-judge (agent `a7d77f37c1c12bf87`): JUSTIFIED; re-score Prob A × Sev II → Initial EH → **Residual HIGH** (B×II) after B2 mitigation → **CCIR-5 fires, drafted RISK OVERRIDE routed to Patrick, never self-signed.** Recommended routing: new Phase 1b (mirror Phase 3b, before Phase 6b); close Phase 1 on its verified 9-site scope with fixture (b) carved out. **AWAITING PATRICK'S SIGNATURE DECISION** (see FRAGO 004 pending in FRAGO log).
