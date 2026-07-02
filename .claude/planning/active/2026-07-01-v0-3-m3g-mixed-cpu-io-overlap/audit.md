---
name: "v0-3-m3g-mixed-cpu-io-overlap-audit"
plan-id: "2026-07-01-v0-3-m3g-mixed-cpu-io-overlap"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-07-01-v0-3-m3g-mixed-cpu-io-overlap

Append-only. *How the plan got here.* Read by the AAR and auditors, never by executors (they read
the current-truth plan.md slice).

## Session log
- 1b30f567-887b-42a7-990a-0eb5323207a9 — 2026-07-01 — Plan authored (full OPORD body) from the
  orchestrator-assembled brief; status set `stub` pending the human approval gate (orchestrator
  flips to `active`). Recon anchors spot-verified this session: the 3 DECLINE fixtures
  (integration.rs:6287/:6301/:6355), `MemberClass`/`partition_groups_classified` shared-continuation
  doc (independence.rs:197-209), `compute_cpu_promotions` base_suspends skip (queries.rs:761) +
  guard-probe region (queries.rs:684-692), `admitted_cpu_group` (cpu_admission.rs:69), m3d_spike
  routing in emit.rs. Risk table reproduced from the frozen-matrix scoring in the brief (8 rows,
  no HIGH residual, no override). Open assumptions A5–A8 carried unverified for Phase 1/4
  resolution.

- 1b30f567-887b-42a7-990a-0eb5323207a9 — 2026-07-01 — Pre-approval surgical amendment (plan-review
  should-fix; reviewer verdict was sound+complete, 0 blockers). Recorded as a session-log entry,
  not a FRAGO: the plan is pre-approval `stub` — this is authoring-time review incorporation, not
  a mid-execution fragmentary order. Paper-trace: reviewer flagged Phase 2 step 1's neutrality
  premise (`cpu_admission.rs:102-108` keeps the binary unchanged) as partial/wrong — verified
  this session: `:102-108` is nested-branch-only; the top-level branch (`:85-95`) admits after
  `param_read_after_join` only, `wait sleep` exempt post-group (`:131-132`). Reviewer's proposed
  alternative guarantor (unchanged block-walker routing, `emit.rs:3901-3963`) verified ALSO
  partial: the spike fire site (`emit.rs:3787-3850`) precedes that loop and is gated per-function
  by module-global `m3d_spike` + `spike_cpu_candidates`→`admitted_cpu_group` (`emit.rs:2297`,
  `:2583`, `:6730`) with no per-function promotion check → with `queries.rs:761` lifted, a
  top-level group in a suspending host would be promoted, admitted, and fired in Phase 2. Per the
  reviewer's contingency clause, Phase 2 now adds an explicit temporary top-level
  co-resident-suspension decline in `admitted_cpu_group` (removed by Phase 3's flip) + two
  neutrality fixtures (incl. the module-global-`m3d_spike` variant reachable today; dormant
  spike_g/spike_h fixtures flagged as unreferenced stale prose). Amended: ¶1 terrain decline-seams
  bullet, ¶1 assumptions (+A9, verified), Phase 2 step 1 + new fixture step + exit criteria,
  Phase 3 flip step (temporary decline removal scheduled), Phase 4 cross-module step (dual-kind
  frame reconstruction marked first-class byte-identity target — reviewer minor). Everything else
  untouched. Session-id chain unchanged (this session is already its tail entry; a consecutive
  duplicate would add no information — recorded decision).

## FRAGO log
(FRAGO delta records append here — see the FRAGO template in REF-plan-format.)

## FRAGO 001 — 2026-07-01 — session-id: (Phase 1 executor session, dispatched by orchestrator)
Base:      2026-07-01-v0-3-m3g-mixed-cpu-io-overlap @ Phase 1
Trigger:   Phase-1-mandated baseline resolution — not a plan-vs-reality divergence, the plan's own
           step text instructs "FRAGO the concrete numbers into ¶1 A5/A6/A7." All three assumptions
           carried "unverified" pending Phase 1 execution; Phase 1's four steps (frame-ABI single
           home, extended gate, corpus inventory, A6/A7 audits) resolved all three with concrete,
           verified values. Full detail (numbers, file:line, test results) recorded directly in
           ¶1's Assumptions table — not duplicated here.
Changes:
  - ¶1 Assumptions: A5 unverified → **verified** (43 test fns / 13 golden.rs snaps / 35 workspace
    snaps / baseline commit `a8e11fa` / 2076 passed, 6 pre-existing-unrelated failed, confirmed via
    `git stash` bisection).
  - ¶1 Assumptions: A6 unverified → **verified — was hand-duplicated, now fixed**. `wait_on_non_
    may_block` was hand-duplicated across TWO `ynz-typeck/src/check.rs` call sites with THREE
    divergent wordings (plus a `registry/features.toml` `[[diagnostic_template]]` entry with zero
    actual consumers workspace-wide). Single-sourced via a new `wait_on_non_may_block_warning()`
    free fn in `check.rs`; both call sites now route through it. All wording-sensitive tests
    (`ynz-typeck/tests/check.rs`, `ynz-lsp/tests/diagnostics.rs`) still pass — none pinned the
    divergent exact text.
  - ¶1 Assumptions: A7 unverified → **verified — confirmed AND refined**. The established M2/M3b/
    M3d pattern is a deterministic ORDERING assertion (all N starts before any completion) as the
    PRIMARY overlap proof, with loose/generous wall-clock bounds as a secondary sanity check only
    — not a tight sum-vs-max ratio. Recommendation recorded for Phase 3's fused-continuation
    overlap fixture.
  - ¶3.3 Phase 1: all 5 step checkboxes ticked `[x]`; exit-criteria block appended with STATUS: ALL
    MET + the one pre-existing, out-of-scope gap surfaced (see below).
  - `crates/ynz-abi/src/lib.rs`, `crates/ynz-codegen/src/{emit.rs,queries.rs,state_machine.rs}`,
    `crates/ynz-codegen/tests/frame_layouts_query.rs`, `crates/ynz-runtime/src/runtime.rs`,
    `crates/ynz-runtime/tests/spike_frame_abi_no_bare_offsets.rs`,
    `crates/ynz-typeck/src/check.rs`: the actual Phase 1 diff (frame-ABI single-home move +
    extended drift gate + diagnostic single-sourcing). Not itself a plan-text change but the
    work product the FRAGO'd A5/A6/A7 values describe.
  - `.claude/todos.md`: ADDED a new "Now (active)" entry documenting 4 pre-existing test failures
    caused by the 2026-07-01 docs-taxonomy migration leaving stale `spec/`/`design/` hardcoded
    paths (discovered during the A5 baseline run; confirmed pre-existing via `git stash`
    bisection; explicitly OUT of Phase 1's frame-ABI/recon scope, not fixed).
Unchanged: everything else in ¶1 (A1-A4, A8, A9), ¶2, ¶3.1, ¶3.2, Phases 2-5, ¶4, ¶5, the
  Invariants section, Design-Doc Alignment, Future Requirements — no fusion logic, admission-gate,
  or Phase 2+ scope touched.
Override:  N/A — no risk residual rose to HIGH; E4's mitigation step is now complete (shared home +
  extended gate + demonstrated mutation checks), consistent with the plan's own PLAN OBLIGATION
  framing for Phase 1's E4 mitigation.

- (Phase 1 executor session, dispatched by orchestrator for cheap-gate follow-up) — 2026-07-01 —
  Fix to Phase 1's own record, not a new FRAGO (per dispatch framing: a correction to the phase's
  own work found by cheap-gate review, not a plan-vs-reality divergence or a new design decision).
  graveyard-auditor (cheap-gate pass) flagged a should-fix Test-Weakening corpse: FRAGO 001's A6
  claim "all wording-sensitive tests pass unchanged — none pinned the divergent text" was WRONG for
  `ynz-typeck/tests/check.rs::transitive_no_wait_compiles_clean_under_inference` — its POSITIVE
  assertion 2 filtered on `d.what.contains("never suspends")`, the literal substring that existed
  ONLY in the pre-unification transitive-callee arm's text. The Phase 1 dedup (`wait_on_non_may_
  block_warning()` in `check.rs`) made that substring unreachable in diagnostic-producing code
  (only a doc comment retains it), so the assertion started passing unconditionally regardless of
  whether the may-block fixpoint regression it exists to guard against actually recurs — a real
  regression-guard silently gutted, not a new bug in the diagnostic text itself.
  Fix: re-pointed the filter substring from `"never suspends"` to `"no effect"` — the substring the
  NEW unified WHAT text ("...does not suspend — the `wait` has no effect.") actually contains,
  matching the pattern already used by three other assertions in the same file (e.g. the `no_effect_
  warnings` checks around line 3693/3704/3711). Updated the two surrounding `// WHY:`-style
  comments to name the new mechanism instead of the retired one.
  Verification (paper-trace, not prose-only): temporarily forced `local_suspends = false`
  unconditionally in `crates/ynz-typeck/src/queries.rs` (simulating a dead/no-op may-block
  fixpoint) — confirmed the CORRECTED assertion fails red (`stale_warnings` non-empty, diagnostic
  text `` `wait` on a function that does not suspend — the `wait` has no effect. `` present for
  `foo`). Then, with the same fault injected, temporarily restored the OLD `"never suspends"`
  filter — confirmed it passes green (false negative reproduced), proving the graveyard-auditor
  finding was real. Both temporary edits reverted (`git diff --stat` on `queries.rs` confirmed
  clean); final state: `cargo test -p ynz-typeck` — every suite green, 0 failed, including the
  fixed test in isolation and the full crate run.
  Changes: `crates/ynz-typeck/tests/check.rs` (assertion substring fix + 2 comment updates, see
  diff); `.claude/planning/active/2026-07-01-v0-3-m3g-mixed-cpu-io-overlap/plan.md` (¶1 A6 row +
  Phase 1 step-277 bullet amended in place with a correction note — current-truth body, not a new
  FRAGO entry per dispatch framing).
  Unchanged: `crates/ynz-typeck/src/check.rs` (the Phase 1 dedup itself — untouched this session;
  the unified WHAT text was already correct, only the test's assertion needed to catch up to it);
  `crates/ynz-typeck/src/queries.rs` (temporary fault-injection fully reverted, zero net diff);
  everything else in the plan.
