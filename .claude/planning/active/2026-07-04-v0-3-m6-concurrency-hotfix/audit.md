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

## Context-segment log

_None yet._
