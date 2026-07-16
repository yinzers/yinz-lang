---
name: "v0-3-m7-optimizer-pipeline-audit"
plan-id: "2026-07-04-v0-3-m7-optimizer-pipeline"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-07-04-v0-3-m7-optimizer-pipeline

Append-only. *How the plan got here.* Read by the AAR, auditors, and the execution conductor's
Step-3a / Step-0 reconcile; never by executors (they read the current-truth plan.md slice).

## Session log

- `plan-author-2026-07-04-m7-optimizer` — 2026-07-04 — Authored the original OPORD draft (¶1–¶5,
  9 phases, risk table R1–R6, Future Requirements, Roadmap Reconciliation). No `audit.md` was created
  at this time (plan had not yet been dispatched for execution).
- `plan-amend-2026-07-04-m7-blockers` — 2026-07-04 — Pre-execution amendment pass resolving all
  plan-reviewer findings (this plan has not started Phase 0; no phase has been dispatched, so no
  FRAGO delta-record applies — these are authoring-time corrections to the still-unexecuted OPORD,
  recorded here as a session-log entry per the amendment discipline). Changes:
  - Authored the missing `## Invariants This Milestone Must Preserve` section (all 7 subsections:
    Safety, Performance, Teaching, Runtime Dependencies, Kernel-Mode Behavior, Demo & Error Gallery,
    Feature Registry Entries), placed after ¶3.4 Coordinating Instructions and before ¶4 Sustainment
    (M5-plan precedent placement).
  - Authored the missing `## Design-Doc Alignment` section (citing `IMP-no-function-coloring.md`'s
    "Scheduler Preemption Model" section and `authoritative-derivation.md`, with citation-depth
    verification, divergence enumeration, and the M6/M7 milestone-boundary flag for P4-1), placed
    after ¶1 Risk Assessment and before ¶2 Mission (M5-plan precedent placement).
  - Fixed the false spike-evidence claim: the cited worktree
    (`.claude/worktrees/agent-abba2c8babbd9ea21`) is gitignored, uncommitted, lives in a different
    clone, and its branch was reused — it no longer preserves anything. Created a checked-in unified
    diff, `spike-o0-flip.patch` (this directory), reconstructed and byte-verified against the current
    tree's `crates/ynz-codegen/src/emit.rs:876-882` and `crates/ynz-codegen/src/state_machine.rs:752-758`.
    Rewrote every plan reference (¶1 Terrain, ¶4 Sustainment "Reference artifact," Phase 1 Step 2) to
    cite `spike-o0-flip.patch` as the preserved mechanism-reproduction artifact and
    `.claude/audits/2026-07-04-concurrency-release-audit.md`'s "Phase-0 spike" section as the primary
    durable evidence of the spike's RED verdict (6/470 failures, failing-fixture list, direct-repro
    SIGSEGV). Grep-confirmed zero remaining `abba2c8babbd9ea21` references in `plan.md`.
  - Resolved the Phase 3/4 ordering contradiction: hard-sequenced Phase 3 → Phase 4 (was
    "either order"). Updated ¶3.2 Concept (added an explicit Ordering note + rewrote the phase-summary
    sentence), ¶3.4 Coordinating Instructions' Sequencing bullet, and Phase 4's Task+purpose text to
    state the hard sequence and its reason (Phase 4 Step 1 re-confirms the crash under Phase 3's
    now-live pipeline). Grep-confirmed zero remaining "either order" references.
  - Added `**CHECKPOINT**` marks to Phase 6 (6 steps trips the >5-step trigger; added after step 2 and
    after step 4, splitting into 2/2/2-step segments) and annotated its Model tag.
  - Re-nested every existing `**CHECKPOINT**` mark (Phases 1, 2, 3, 5, 7) from its own numbered list
    item into a standalone line nested under the preceding step, per
    `REF-plan-format.md`'s checkpoint-mark syntax — renumbered the now-shorter Steps lists in each of
    those phases accordingly.
  - Added risk row R7 (optimizer/golden non-determinism breaking the Phase 5 byte-identical 2-run
    gate) to the ¶1 risk table: C×III initial MEDIUM, mitigated by the Phase 5 two-independent-run
    gate itself (B2, probability, −1) → residual LOW (D×III), per REF-risk-engine.md.
  - Named both duplicate roadmap `## Capability Ledger` tables explicitly (roadmap.md lines ~365 and
    ~417) in Phase 8 Step 2 and the `## Roadmap Reconciliation` section intro, and required both be
    updated in lockstep (matching the sibling M6 plan's independent commitment to the same
    both-tables convention).
  - Rewrote the frontmatter-deviation blockquote to state the real convention: `paused` is the
    conductor-set pre-approval state (Gate 4 pending + the M6-merge precondition), removing the
    prior self-contradictory "should be active per charter" framing.
  - Added a "Civil considerations" line to the ¶1 cross-cutting factor sweep: N/A — compiler-internal
    backend work, no user-facing surface change.
  - Appended `session-id: "plan-amend-2026-07-04-m7-blockers"` to the frontmatter chain
    (append-only — `plan-author-2026-07-04-m7-optimizer` preserved).

- `plan-amend-2026-07-04-m7-links` — 2026-07-04 — Mechanical amendment pass, no execution has started
  (pre-Phase-0). Fixed two classes of citation defects: (1) R7's mitigation-proof citation said "Phase 5
  step 4" for the golden second-regeneration work; verified against Phase 5's actual step list and
  corrected to "step 3" (the second-independent-run diff is Phase 5 Step 3, not Step 4). (2) Every
  broken relative-link depth in the file: 8 `IMP-no-function-coloring.md` links using a wrong 5-up form
  (lines 221, 344, 471, 479, 494, 499, 608, 798 — 608 found during the mechanical sweep, not in the
  original enumerated list) corrected to the 4-up form already used at lines 142/664; 3
  `authoritative-derivation.md` links using a wrong 4-up form missing the `.claude` segment (lines 95,
  343, 609) corrected to the 3-up form used at 144/154; 4 links to `Cargo.toml`/`ci.yml`/`features.toml`
  using a wrong 5-up form (lines 37, 68, 573, 741) corrected to 4-up; Phase 8 Step 1's `roadmap.md`
  citation (line 554) missing the `active/` segment, corrected to
  `../../active/2026-05-21-v0-3-concurrency-perf/roadmap.md`. Every one of the corrected targets
  re-verified to exist on disk at its new relative path after the edit. Links to global `~/.claude`
  specs (`REF-plan-format.md`, `REF-risk-engine.md`, `plan-source-of-truth.md`,
  `plan-spike-discipline.md`) were deliberately left untouched — a recorded systemic
  global-vs-project-link convention gap out of this plan's scope. Appended
  `session-id: "plan-amend-2026-07-04-m7-links"` to the frontmatter chain (append-only).

- `plan-amend-2026-07-04-m7-phase6-yield` — 2026-07-04 — Pre-execution amendment pass (no phase
  dispatched; still `paused`, pre-Phase-0), triggered by Fable's personal plan-audit finding recorded
  in [`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)
  "M7-plan addendum": Phase 6's original framing ("implement real cooperative-yield semantics inside
  `ynz_rt_check_preempt`") was architecturally wrong — a synchronous `extern "C"` callee cannot yield
  the enclosing Tokio task by itself. Verified directly against the code before amending: `runtime.rs:281-299`
  (`ynz_rt_check_preempt` is a synchronous no-op stub), `emit.rs:12356-12365` (`emit_loop_preempt`
  emits a plain, unconditional call at loop back edges), and `state_machine.rs` (the existing
  authoritative suspension machinery — `store_resume_point`/`load_resume_point`,
  `flush_var_slot_to_frame` in `emit.rs` — that a resume function already uses to store `resume_point`
  and return `Pending` at `wait` points). Changes:
  - Rewrote Phase 6's Task+purpose and all 6 Steps into 7 steps: (1) a new DESIGN step specifying the
    codegen back-edge poll-yield transform (which loops qualify — SM functions only; what the yield
    emits — `resume_point` store + crossing-local flush via the EXISTING authoritative suspension
    machinery, reuse not re-derivation, per [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md);
    the budget-check mechanism — `ynz_rt_check_preempt` becomes a cheap synchronous CHECK, never the
    yield itself; and the explicit non-SM residual), (2) implement the codegen transform + the check
    fn, (3) the starvation-proof fixture set — now requiring BOTH an SM-positive fixture (hot loop
    inside a wait-containing function) AND a companion non-SM-residual fixture (identical hot loop
    inside a plain function, confirmed NOT preempted, relying on existing CPU-admission routing) — then
    the pre-existing pre-register-threshold / call-site-measurement / decision / doc-update steps,
    renumbered 4–7. Added `**CHECKPOINT**` marks after Steps 1, 3, and 5 (was after Steps 2 and 4;
    re-anchored to the new step positions). Raised the Model tag from `(coding, standard, medium)` to
    `(coding, high, medium)` — quality-bar raised to match Phase 1/2's bar for the same
    silent-miscompile hazard family. Added an adversarial-gate-checker reviewer clause specific to R8
    (does the RED-fixture set prove frame-layout correctness; does the transform genuinely reuse the
    authoritative machinery rather than re-deriving a parallel one).
  - Added risk row **R8** to the ¶1 Risk Assessment table (back-edge poll-yield codegen transform → new
    frame-layout/crossing-local suspension hazard, same silent-miscompile family as R1 and this repo's
    four-milestone twin-derivation/frame history: M3a/M3d/M3e/M3g). Scored per REF-risk-engine.md:
    initial `lookup(B, II) = HIGH`. Applied the one honestly-provable catalog mitigation — Adversarial/
    RED-repro test (B2, probability, −1; proof: loop-crossing-local suspension fixtures authored and
    committed BEFORE the transform lands) — shifting probability B→C. Re-lookup(C, II) = **HIGH,
    unchanged** (Critical severity does not clear High until probability reaches D). Explicitly
    evaluated and REJECTED two candidate second mitigations as dishonest catalog stretches: (a)
    severity-axis B1 patterns (made-reversible/idempotency) don't map to a compiler miscompile, and
    this plan's severity-anchor selection already prices git-revertibility into Sev II — re-applying it
    would double-count the same fact; (b) canary/staged-exposure's precondition presumes staged
    PRODUCTION exposure, which doesn't exist for compiler-internal pre-release work. Residual: **HIGH**.
    Drafted the RISK OVERRIDE block in place (¶1, immediately below the risk table) with the signature
    line deliberately left blank — this producer never self-signs a HIGH residual; the
    orchestrator/Patrick's signature is required before Phase 6 Step 2 (implementation) begins.
  - Sibling-swept the whole plan for "check_preempt", "back-edge", "yield", "preempt" and reconciled
    every hit with the corrected architecture: Design-Doc Alignment divergence 1 (added the
    architectural-gap clarification + the three-part TRUE-architecture framing), Key Outcome 3
    (rewrote to state preemption is currently ZERO today, not "already partially true," and the
    three-part architecture), the Teaching invariant (three-part doc-rewrite framing), the Runtime
    Dependencies invariant (codegen-transform-not-runtime-magic framing + the named non-SM residual),
    the Kernel-Mode Behavior invariant (added the non-SM-residual kernel-orthogonality clause), and the
    R6 risk row (added a one-line clause distinguishing R6's overhead concern from R8's
    frame-layout-correctness concern — both are codegen-emitted poll-yield sites, never runtime magic).
    Fixed a stale cross-reference in `### Feature Registry Entries` ("Phase 6 step 5" → "Phase 6 step
    6" — the decision step's position shifted when the new Step 1/Step 3 were inserted).
  - Added Future Requirements / Revisit **#8** — the named non-SM-admission-miss residual, as its own
    tracked four-field entry (WHAT/WHY/COST/TRIGGER), since it is a genuinely new named limitation this
    amendment introduces and REF-plan-format.md's doctrine requires every unresolved risk/limitation to
    be an observed future requirement, never a loose note.
  - Appended `session-id: "plan-amend-2026-07-04-m7-phase6-yield"` to the frontmatter chain
    (append-only — all three prior session-ids preserved).

- `gate4-signatures-2026-07-04` — 2026-07-04 — Signature event: Patrick signed R8's RISK OVERRIDE
  (back-edge poll-yield frame-layout hazard, ¶1 Risk Assessment) as part of Gate-4 approval covering
  all three sibling concurrency plans (M6/M7/M8). Filled `Accepted by: Patrick (Gate-4 approval,
  conducted 2026-07-04)` and `Date: 2026-07-04` on the previously-blank signature lines; no other risk
  text changed. Appended `session-id: "gate4-signatures-2026-07-04"` to the frontmatter chain
  (append-only — all four prior session-ids preserved).

- `executor-2026-07-16-patrick-triage-application` — 2026-07-16 — Applied Patrick's M6-completion
  triage decisions to this plan via a targeted Future Requirements amendment (no plan-body phase
  rewrite, no status change — this plan remains in-flight `active` post-Gate-4). M6-completion triage
  disposition: **fr23** (non-plain-ident background-spawn receivers riding as raw pointers under
  `OptimizationLevel::None`) was tracked as "latent, not confirmed-live" in the M6 release; under
  THIS plan's real optimizer pipeline (with LLVM passes enabled), the stack-slot-lifetime shrinkage
  turns the latent raw-pointer ride into confirmed UAF. This plan's own Phase 2/3 optimizer wiring is
  the expiry condition on that latent verdict. **Amendment**: added Future Requirements item #9
  (fr23: non-plain-ident background-spawn-receivers) with full WHAT/WHY/DISPOSITION/TRIGGER, homed to
  M7 because the optimizer flip is the confirming condition that turns the finding live. The item
  prescribes an either-or choice: (a) fix the give/copy machinery for field/index/return-materialized
  spawn receivers early in M7, OR (b) gate and re-run the UAF repro under real optimized pipeline
  (post Phase 2/3), then route a confirmed-live result as a risk override per the R13/R14 precedent
  (signed-risk pattern already established in this plan). Appended `session-id:
  "executor-2026-07-16-patrick-triage-application"` to the frontmatter chain (append-only — all five
  prior session-ids preserved).

- `executor-2026-07-16-phase0-spike` — 2026-07-16 — Executed **Phase 0 (P0 Spike: inkwell / LLVM
  PassBuilder feasibility)**. Verdict: **GREEN** (hard-gate STOP-condition satisfied; R3 retired to
  its residual). Evidence chain:
  - **Step 1 (API discovery):** verified against the vendored source in the dev container's
    cargo-registry volume (`inkwell-0.9.0/src/module.rs:1631`) — `Module::run_passes(&self,
    passes: &str, machine: &TargetMachine, options: PassBuilderOptions) -> Result<(), LLVMString>`,
    gated `#[llvm_versions(13..)]` (live at the pinned `llvm18-1-prefer-dynamic`). Pipeline string
    format is `opt -passes=` new-PM syntax (`"default<O2>"` and friends). `PassBuilderOptions`
    lives at `inkwell-0.9.0/src/passes.rs:1196`; `FunctionValue::run_passes` also exists; legacy
    `PassManager` is deprecated in-source in favor of this exact API.
  - **Steps 2–3 (proof):** throwaway scratch binary `scratch/opt-pipeline-spike/` (standalone
    `[workspace]`, outside the crate tree, same inkwell pin + feature as the workspace) built a
    module with one function / one alloca / one dead store (42 overwritten by 7 before any load),
    ran `run_passes("default<O2>", …)`, and asserted on the printed IR: dead store eliminated,
    alloca promoted (mem2reg), body constant-folded to `ret i64 7` with O2 function attributes
    inferred — a genuinely-ran pipeline, not a no-op `Ok(())`. Run via
    `docker compose run --rm dev bash -c "cd scratch/opt-pipeline-spike && cargo run"`; all four
    assertions passed (llvm-sys 181.3.0 / LLVM 18).
  - **Step 4 (durable artifact, plan-spike-discipline Facet 2):** the API shape + captured
    before/after IR persisted as the checked-in note `scratch/opt-pipeline-spike/api-shape.md` —
    the artifact Phase 3 consumes directly. Kept alongside it as reference: the minimal repro
    source (`src/main.rs`, `Cargo.toml`). Discarded scaffolding: build output (`target/`,
    `Cargo.lock`) is gitignored inside the spike dir; nothing entered `crates/`.
  - The note also records the Phase-3 threading constraint: reuse the authoritative
    `state_machine::default_target_machine()` for `run_passes` (per
    `.claude/rules/authoritative-derivation.md`) — surfaced here so Phase 3's executor does not
    construct a second machine.
  - Plan↔task sync: this plan's phase text carries no `- [ ]` checkboxes (steps are numbered
    prose per REF-plan-format's noted convention drift), and this dispatch's tool grant carries no
    TodoWrite — phase completion is recorded by this session-log entry + the appended session-id;
    no checkbox/task mismatch exists. Appended `session-id: "executor-2026-07-16-phase0-spike"` to
    the frontmatter chain (append-only — all six prior session-ids preserved).

- `conductor-2026-07-16-fable-model-override` — 2026-07-16 — **Model-routing decision, not a plan
  amendment** (execution/dispatch policy, no plan-vs-reality divergence — recorded here per
  plan-source-of-truth for auditability, not as a FRAGO). Per Patrick's explicit, repeated,
  in-conversation instruction (three rounds — the conductor pushed back twice, citing the §4
  alignment-data gap for Fable and R8's signed HIGH residual specifically; Patrick held the
  instruction each round and confirmed it deliberately on the third), **every dispatch for the
  remainder of this plan's execution that would otherwise resolve to Opus 4.8 under any
  `REF-model-selection.md` §3 override (safety-floor, scale=large, security) is instead dispatched
  as Fable 5 at effort `medium`** — scoped to this plan's execution only, NOT a change to the global
  `REF-model-selection.md` binding (no §6 re-derivation was run; Fable's alignment/large-context/
  security-capability data remains ungathered in §4).
  - **Logged objection (restated once, per the Honesty escalation discipline, not re-litigated):**
    Phase 6 Step 2 (the back-edge poll-yield codegen transform) carries R8's signed HIGH residual —
    a silent-miscompile-class risk this repo has been burned by four milestones running
    (M3a/M3d/M3e/M3g). The safety-floor override exists specifically to route risk=high work to the
    model with the best MEASURED alignment score; Fable carries no measured alignment score in this
    binding. Patrick's call, made deliberately, knowing this — not a silent substitution.
  - Effective for: Phases 1–8 of this plan (Phase 0 already ran on Fable·medium per the standing
    `coding·high` binding cell, no override needed there). Applies to EVERY model-selection cell for
    the rest of this execution, including the ones that would otherwise hit Override 1 (safety
    floor), Override 2 (scale=large — Phases 2, 5, 7), and Override 3 (security, if any reviewer
    dispatch in this plan's fan-out hits it).
  - Appended `session-id: "conductor-2026-07-16-fable-model-override"` to the frontmatter chain
    (append-only — all seven prior session-ids preserved).

- `executor-2026-07-16-phase0-fixloop` — 2026-07-16 — Documentation-hygiene fix-loop dispatch,
  resolving two confirmed reviewer findings in plan/audit state post-Phase-0 execution (Phase 0
  itself executed GREEN with no plan-vs-reality divergence; these are documentation-coherence blockers
  unrelated to Phase 0's technical work). Changes:
  - **Finding 1 (confirmed by 3 independent reviewers):** The frontmatter `session-id` array carried
    `"executor-2026-07-16-patrick-triage-application"` orphaned — no matching `audit.md` Session-log
    entry existed for it, violating plan-source-of-truth.md's rule that session-id and audit narrative
    are authored together. **Fix:** authored the missing session-log entry for
    `executor-2026-07-16-patrick-triage-application`, inserted at correct chronological position
    (after `gate4-signatures-2026-07-04`, before `executor-2026-07-16-phase0-spike`), documenting that
    session's M6-completion-triage application: added Future Requirements item #9 (fr23: non-plain-ident
    background-spawn-receivers) with reasoning that Phase 2/3's real optimizer pipeline turns the
    latent UAF finding into a live, confirmed-risk scenario for M7 to resolve early or gate-and-retry
    (disposition: either-or choice (a) fix give/copy machinery or (b) gate and re-run repro post-Phase3
    with risk override per R13/R14 precedent).
  - **Finding 2 (confirmed by 2 independent reviewers):** The status blockquote (plan.md lines 15–25)
    still asserted the plan was `paused` pending two preconditions (Gate 4 approval and M6 merge),
    contradicting the current frontmatter `status: "active"`. Both preconditions have been satisfied:
    Gate 4 signed on 2026-07-04 per audit entry `gate4-signatures-2026-07-04`, and M6 merged to
    `main` as v0.3.2 (commits `0ac76d5` / `10df6d7`, 2026-07-16). **Fix:** rewrote the blockquote to
    state the transition has occurred — both preconditions satisfied, status correctly flipped to
    `active` — while preserving historical context (why it WAS paused, the two preconditions that
    gated it, and the exact citations for when each cleared). Rewrite maintains the blockquote's
    established practice of recording plan-status history in place, not deleting the context.
  - No FRAGO filed: no plan-vs-reality divergence in either finding — the issues were
    documentation-consistency gaps between audit.md and plan.md / frontmatter and body text, both
    corrections to the documentation state itself rather than a response to code/build reality.
  - Appended `session-id: "executor-2026-07-16-phase0-fixloop"` to the frontmatter chain (append-only
    — all eight prior session-ids preserved).

## FRAGO log

(none yet — Phase 0 executed 2026-07-16 with zero plan-vs-reality divergence: the written steps,
STOP-conditions, and exit criteria matched reality as found, so no FRAGO was needed. The first
FRAGO, if any, will be filed when a later phase's reality diverges from the written plan.)

## Context-segment log

(none yet — no phase has been dispatched.)
