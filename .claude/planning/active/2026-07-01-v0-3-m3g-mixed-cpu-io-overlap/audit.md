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

## FRAGO 002 — 2026-07-01 — session-id: (Phase 2 executor session, dispatched by orchestrator)
Base:      2026-07-01-v0-3-m3g-mixed-cpu-io-overlap @ Phase 2 (Typeck front half, behavior-neutral)
Trigger:   Phase 2's own step 2 text explicitly directs this exact action: "Probe (b) against the
           Phase 1 baseline FIRST: if it already fires on `main`, that is a pre-existing latent
           misfire — Paper-Trace it, record it in the PR + a FRAGO." Not a self-decided scope
           change — the plan pre-authorizes this specific record for this specific probe.
Changes:
  - **Paper-Trace (neutrality fixture variant b, the A9 gating-asymmetry case):**
    Observed — a throwaway module (`crunch`/`pureCpuHost`/`mixedHost`/`entrypoint`, isomorphic to
    the committed `v0_3_m3g_mixed_host_with_promoted_sibling_declines.ynz`) built against the
    UNMODIFIED Phase 1 baseline commit `d184934` (zero Phase 2 code touched) via
    `./target/debug/ynz build ... --emit-ir` shows **4** `call ptr
    @ynz_rt_spawn_blocking_joinable(...)` instructions: 2 with trampoline
    `__ynz_spike_trampoline_pureCpuHost_crunch_{0,1}` and 2 with trampoline
    `__ynz_spike_trampoline_mixedHost_crunch_{0,1}`.
    Expected — `mixedHost` contains `wait sleep(0)`, so it is a `base_suspends` host; per
    `compute_cpu_promotions`'s pre-Phase-2 `queries.rs:761` skip, it can never enter
    `cpu_promoted`, so only `pureCpuHost`'s legitimate 2 spawns should appear (2 total, not 4).
    Residual — 2 extra spawn calls, both attributable to `mixedHost`.
    Hypothesis — `spike_cpu_candidates` (`emit.rs`, pre-Phase-2 numbering `:6706-6733`) is gated
    in `lower_function_with_waits` purely by the MODULE-WIDE `m3d_spike` boolean
    (`!cpu_promoted.is_empty()`), NOT by whether the function currently being lowered is itself a
    member of `cpu_promoted`. `pureCpuHost`'s legitimate promotion alone flips the module-wide
    flag; every OTHER suspend_set-lowered function in the module (including `mixedHost`, which is
    already in `suspend_set` for unrelated reasons — its own `wait`) then gets probed by
    `admitted_cpu_group` regardless. Pre-Phase-2, that function's top-level branch had no
    co-resident-suspension check (only the nested branch did), so it structurally admitted.
    Evidence path — `crates/ynz-codegen/src/emit.rs` (pre-Phase-2: `:2295-2296` module-wide gate,
    `:6731` `admitted_cpu_group` call) → `crates/ynz-typeck/src/cpu_admission.rs` (pre-Phase-2
    `:85-95`, top-level branch, no co-resident-suspension check).
    Verdict — real, reproducible, pre-existing (independent of any Phase 2 code), exactly matching
    A9's prediction. The Phase 2 temporary co-resident-suspension decline (step 1) closes it as a
    side effect of the SAME mechanism that keeps fixture (a) neutral, since both read the one
    admission authority (`admitted_cpu_group`). Verified after the fix: spawn count drops to
    exactly 2 (both `pureCpuHost`'s); `mixedHost` contributes 0.
  - `crates/ynz-typeck/src/cpu_admission.rs`, `crates/ynz-typeck/src/queries.rs`: Phase 2's actual
    diff (temporary top-level co-resident-suspension decline; `base_suspends` candidate-skip
    lift; E6 guard-probe extension to `base_suspends` direct candidates; 2 new unit tests,
    fault-injection-verified non-vacuous).
  - `crates/ynz-codegen/src/emit.rs`, `crates/ynz-codegen/src/queries.rs`: `does_real_work_set`
    threaded through the real production call chain (`codegen_query` → `emit_artifact` →
    `build_module` → `lower_function` → `lower_function_with_waits` → `Cg::does_real_work`),
    structurally wired but unconsumed (`#[allow(dead_code)]`, mirroring the `wait_cache`
    precedent) — satisfies the plan's "plumb to the codegen input surface" step; no lowering
    logic touched.
  - `crates/ynz-driver/tests/fixtures/v0_3_m3g_{top_level_group_in_suspending_host_declines,
    mixed_host_with_promoted_sibling_declines,guard_tripping_crossing_in_suspending_host_
    declines}.ynz` + 3 matching `crates/ynz-driver/tests/integration.rs` tests: the two
    neutrality fixtures (a)/(b) and the E6 adversarial fixture. All green.
  - **Deviation surfaced (E6 fixture guard type):** the plan's step-2 prose example for a
    guard-tripping crossing is a nested-shape local. Verified this is UNREACHABLE as an
    E6-isolated fixture: any nested-shape `let` anywhere in ANY function that ALSO contains a
    real `wait` hard-errors at ordinary, pre-promotion `check_query` time (`` `w` is a `Outer`
    value that crosses a `wait` `` — the whole-function-conservative crossing analysis flags it
    regardless of position relative to the wait, confirmed by direct probe: `wait sleep(0); let
    w: Outer = {...}; print(w.tag)` still hard-errors even with `w` declared strictly AFTER the
    only wait). That pre-existing hard error fires BEFORE the promotion-layer guard-probe ever
    runs, so a nested-shape crossing can never isolate the E6-specific hazard (a crossing over
    ONLY the invisible CPU join) from the ordinary compile-time guard. Substituted the
    "suspending call in sub-expression position" guard instead (same underlying
    `suspension_guards_fire_for_fn` mechanism, a different one of its checks) — verified this one
    compiles clean under the REAL suspend set (the callee never suspends) and is caught ONLY by
    the guard-probe's per-candidate `suspending_fns` augmentation. Not a self-decided scope
    change to the E6 MECHANISM (still "extend the guard-probe to `base_suspends` hosts," exactly
    as directed) — only to which of `suspension_guards_fire_for_fn`'s several checks the fixture
    exercises, because the plan's suggested one was structurally unusable for this isolation
    purpose. Surfaced per dispatch instructions for review.
  - **Discovered interaction + fix (pre-existing test, not a plan-vs-reality divergence in the
    step-text sense — a foreseeable consequence of Phase 2's own directed mechanism):**
    `crates/ynz-codegen/tests/frame_layouts_query.rs`'s pre-existing (Phase-1-baseline)
    `spike_host_subset_bare_admits_effective_declines_on_imported_post_pair` started failing
    after Phase 2's step 1 landed. Paper-Trace — Observed: `hosts_bare.contains("entrypoint")`
    assertion fails (empty set) where the test expects the BARE probe to still ADMIT. Expected:
    the test isolates a NARROW divergence (`cpu_group_member_indices`'s post-group
    suspending-callee check, which is suspend-set-membership-dependent) via a shared
    cross-module fixture whose `entrypoint` calls the imported suspending `ioWork` with an
    EXPLICIT `wait` keyword. Hypothesis: the Phase 2 temporary decline's `stmt_contains_wait_deep`
    half fires on the LITERAL `wait` AST node regardless of suspend-set membership — broader than,
    and firing BEFORE, the narrow gate this test isolates — so BOTH the bare and effective probes
    now decline for this NEW, independent reason, masking the divergence the test exists to keep
    visible. Evidence path: `cpu_admission.rs`'s new top-level branch check (this phase) →
    `frame_layouts_query.rs:410` (the shared fixture's `wait ioWork()` call). Confirmed via two
    dead-end fixes before settling on the real one: (1) tried removing the `wait` keyword from the
    shared cross-module fixture — hit an UNRELATED, pre-existing gap (a bare, non-`wait`,
    non-`let` statement-position call to an IMPORTED function does not resolve — `` `name` is not
    defined `` — reproduced independently with a trivial non-suspending imported callee, confirming
    it is a general cross-module bare-call resolution gap, not anything to do with suspension);
    (2) confirmed a bare LOCAL suspending call (no `wait`) DOES resolve and suspend correctly.
    Fix: rewrote the test to use an INLINE single-file source with a LOCAL `ioWork` (sidesteps the
    unrelated import-resolution gap) called BARE (no `wait`, sidesteps the Phase 2 temporary
    decline's wait-AST check), with the "bare"/"effective" `SuspendSet`s constructed MANUALLY
    (removing/keeping `"ioWork"` from `check.suspends_set`) rather than relying on real
    cross-module absence — the mechanism under test (`spike_host_subset` respecting whatever
    suspend set it is given) is identical either way. No assertion was weakened; a sanity
    assertion was added (`check.suspends_set.contains("ioWork")`) confirming the synthetic split
    removes/keeps a REAL suspending name, not a no-op. Verified: all 8 `frame_layouts_query.rs`
    tests green, including this one and the two other tests sharing the ORIGINAL cross-module
    fixture (untouched, still passing — `imported_suspending_after_pair_declines_consistently_
    across_boundaries` and `ynz-driver`'s `v03_m3d_imported_suspending_after_pair_byte_identical_
    and_clean`).
Unchanged: ¶1, ¶2, ¶3.1, ¶3.2, Phase 1 (already closed), Phases 3-5, ¶4, ¶5, the Invariants
  section, Design-Doc Alignment, Future Requirements — Phase 3's fusion core, admission flip, and
  deadlock gate are untouched; the admission gate still declines every mixed shape (behavior-
  neutral, as required).
Override:  N/A — no risk residual rose to HIGH; E6's mitigation step (guard-probe extension +
  adversarial fixtures, fault-injection-verified) is complete per the plan's own PLAN OBLIGATION
  framing for Phase 2.

- (Phase 2 executor session, dispatched by orchestrator for code-reviewer blocker follow-up) —
  2026-07-02 — Fix to Phase 2's own record, not a new FRAGO (mirrors the Phase 1 cheap-gate-fix
  correction pattern above: a correction to the phase's own work found by the reviewer fleet, not
  a plan-vs-reality divergence or a new design decision).
  code-reviewer (static trace, high confidence) flagged a blocker: the Phase-2 temporary top-level
  co-resident-suspension decline (`admitted_cpu_group`, `cpu_admission.rs:92-137` as landed by
  FRAGO 002) re-derived "does this host suspend?" via `stmt_contains_wait_deep(s) ||
  stmt_contains_suspending_call_deep(s, suspend_set)` — a per-statement AST scan of `f`'s OWN
  body. This missed a bare may-block-intrinsic call (`sleep(0)` with no `wait` keyword): such a
  call still sets `calls_may_block_intrinsic = true` on the host in `may_block::analyze`
  (`may_block.rs:916-921`, independent of any `wait` wrapper), but produces no `Expr::Wait` node
  (`stmt_contains_wait_deep` → false) AND `sleep` is in `M2_MAY_BLOCK_INTRINSICS`, explicitly
  EXEMPTED from `stmt_contains_suspending_call_deep`'s scan (`cpu_admission.rs:553-554` as landed).
  So the decline predicate stayed false, `admitted_cpu_group` returned `Some`, and the host's CPU
  group would FIRE at codegen (`spike_cpu_candidates` → `admitted_cpu_group`, no `base_suspends`
  gate at the fire site) — breaking Phase 2's "provably inert / behavior-neutral" promise for a
  legal Yinz shape. Untested: all 3 of Phase 2's fixtures use explicit `wait sleep(...)`, none a
  bare intrinsic call. It also broke hint==binary parity: `parallel_group_hints` independently
  skips a suspending host's `admitted_cpu_group` call entirely (it never reaches the decline), so
  the muted hint would show the host as NOT parallelized while the binary fired it.
  Paper-Trace — Observed (pre-fix, temporary revert of the decline body to the old AST-scan
  predicate, `cargo test -p ynz-driver --test integration
  v03_m3g_top_level_group_in_suspending_host_bare_intrinsic_declines_byte_identical`): 2 spawn
  calls (`left: 2 right: 0` — the `count_spawn_calls(&ir), 0` assertion failed). Expected: 0
  spawns (sequential, byte-identical to `--no-auto-parallel`, oracle "9907"). Residual: 2 spawns
  attributable to the bare-`sleep(0)` host's CPU group firing when it must decline. Hypothesis:
  the AST-scan decline's `stmt_contains_suspending_call_deep` exempts `M2_MAY_BLOCK_INTRINSICS`
  (correct for its OTHER callers — an intrinsic wait has no embedded child sub-frame to alias —
  but wrong reused here for "does the host suspend at all"), and `stmt_contains_wait_deep` only
  matches a literal `Expr::Wait` node, which a bare `sleep(0)` call never produces. Evidence path:
  `crates/ynz-typeck/src/cpu_admission.rs:108-110` (as landed) ×
  `crates/ynz-typeck/src/may_block.rs:916-921` (the seeding this scan never consulted). Verdict:
  real, reproducible, exactly matching the code-reviewer's static-trace finding. Reverted the
  temporary probe edit immediately after confirming (zero net diff from the probe itself).
  Fix: keyed the decline off `base_suspends.contains(&f.name)` instead of the AST re-scan —
  `base_suspends` is the SAME authoritative pre-CPU-promotion suspend set
  `compute_cpu_promotions` already reads as `base_suspends` (exactly the set Phase 2's own lift
  widened), computed once per query boundary via `build_effective_suspend_set(&check.suspends_set,
  &sig_output.imported_fns)` and never re-derived from a second, narrower AST notion. This closes
  the bare-intrinsic hole AND any future may-block intrinsic or bare-suspension shape without a
  second detector to keep in sync — the exact property a re-derived (even corrected) AST scan
  could not offer. `base_suspends` had to be threaded as a genuinely SEPARATE parameter from
  `suspend_set` at every call site down to `admitted_cpu_group` (NOT reused from `suspend_set`,
  which — at every codegen call site downstream of `codegen_query`/`frame_layouts_query` — is
  `base_suspends ∪ spike_hosts`; checking `f`'s own name against that union would self-decline a
  legitimate pure-CPU host the instant it is admitted, since its own name lands in the union the
  moment `spike_host_subset` accepts it). Verified this precisely with the golden-corpus
  `pureCpuHost`/`mixedHost` fixture (b) — its "exactly 2 spawns" assertion (only `pureCpuHost`'s)
  still holds after the fix; a naive `suspend_set`-based decline would have wrongly suppressed
  `pureCpuHost`'s legitimate promotion on its second `admitted_cpu_group` probe inside
  `emit_artifact`.
  Confirmed empirically (fault-injection, not prose-only): the new bare-intrinsic fixture's
  integration test shows 2 spawns with the OLD AST-scan predicate temporarily restored, 0 spawns
  with the real `base_suspends` fix restored — both runs captured live, the temporary edit
  reverted immediately after.
  Changes: `crates/ynz-typeck/src/cpu_admission.rs` (`admitted_cpu_group` gains a `base_suspends:
  &SuspendSet` parameter; the temporary decline body + its doc comment rewritten); `crates/ynz-
  typeck/src/inlay_hint_passes.rs` (`parallel_group_hints`'s call site passes `&effective_suspends`
  for both `suspend_set` and `base_suspends` — identical value at this call site, since the hint
  pass never unions in spike-host names); `crates/ynz-codegen/src/emit.rs` (`base_suspends`
  threaded through `spike_cpu_candidates`, `cpu_group_slots_and_reserve`,
  `spike_host_cpu_supported`, `compute_frame_size`, `build_frame_layouts_with_resolver`,
  `spike_host_subset`, `lower_function_with_waits`, `lower_function`, `build_module`,
  `emit_artifact`); `crates/ynz-codegen/src/queries.rs` (`frame_layouts_query` and `codegen_query`
  each keep an unmutated `base_suspend_set` alongside the unioned `effective_suspend_set` /
  `suspends_with_promotions`, passing both to their respective callees); new fixture
  `crates/ynz-driver/tests/fixtures/v0_3_m3g_top_level_group_in_suspending_host_bare_intrinsic_
  declines.ynz` + matching `crates/ynz-driver/tests/integration.rs` test
  `v03_m3g_top_level_group_in_suspending_host_bare_intrinsic_declines_byte_identical` (bare-
  intrinsic twin of neutrality fixture (a), 0 spawns, oracle "9907").
  **Discovered interaction + fix (same class of foreseeable consequence as the one recorded in
  FRAGO 002 above, now triggered a second time by this fix, not a plan-vs-reality divergence):**
  `crates/ynz-codegen/tests/frame_layouts_query.rs`'s
  `spike_host_subset_bare_admits_effective_declines_on_imported_post_pair` (itself already a
  FRAGO-002-era rewrite) broke AGAIN. Paper-Trace — Observed: `hosts_bare.contains("entrypoint")`
  fails (empty). Expected (per that test's own construction): the bare probe admits, the effective
  probe declines. Hypothesis: `entrypoint` in that fixture calls a genuinely suspending `ioWork`
  SOMEWHERE in its body (bare, no `wait`), so `entrypoint` is ALREADY a member of the real
  `check.suspends_set` — independent of whether "ioWork" itself is present in the (unrelated)
  `suspend_set` argument the test varies. The NEW `base_suspends`-keyed decline reads exactly that
  membership, so it now declines `entrypoint` regardless of the bare/effective `suspend_set` split
  — collapsing the OLD divergence the test polices via a DIFFERENT, coarser mechanism than the one
  it isolates. Confirmed: `hosts_bare` is empty with a correctly-computed `base_suspends`,
  regardless of `suspend_set`. Verdict: not a regression — a STRENGTHENING. Because both real
  query boundaries (`frame_layouts_query`, `codegen_query`) compute `base_suspends` identically
  (same `build_effective_suspend_set` call, same salsa-memoized `check_query`/
  `module_signatures_query` outputs), the bare-vs-effective divergence this test file exists to
  catch is now structurally unreachable for any function that is itself transitively suspending —
  not merely masked by convention, as the OLD literal-`wait`-AST-scan version of the decline
  incidentally also did (per FRAGO 002's Paper-Trace above) but for the wrong reason (a re-derived
  scan that could diverge, not an authoritative-set read that cannot).
  Fix: split the one test into two single-purpose tests (no assertion weakened — the underlying
  claim strengthened): (1)
  `spike_host_subset_base_suspends_decline_masks_bare_vs_effective_divergence` proves the NEW,
  correct, production-realistic invariant — a correctly-computed `base_suspends` makes
  `spike_host_subset` decline `entrypoint` regardless of the `suspend_set` variant, `hosts_bare ==
  hosts_effective` (both empty); (2)
  `spike_host_subset_post_pair_gate_still_suspend_set_sensitive_when_base_suspends_is_wrong` keeps
  the ORIGINAL isolation alive for its teaching value, via an explicitly-labeled ARTIFICIAL
  construction (an empty `base_suspends` — not a value any real caller would ever pass) proving the
  pre-existing, unrelated `cpu_group_member_indices` post-pair suspending-callee gate is still
  alive underneath and still `suspend_set`-sensitive — documenting WHY every real caller computing
  `base_suspends` correctly is the one thing standing between "safe" and the exact heap-under-
  allocation class this file guards against. Verified: all 9 `frame_layouts_query.rs` tests green
  (7 pre-existing untouched + the 2 new tests replacing the 1 broken one), including the two other
  tests sharing the ORIGINAL cross-module fixture (untouched, still passing).
  Also updated (compile-only, no logic/assertion change, same value passed for the new parameter
  as production always uses at each call site): `crates/ynz-codegen/tests/golden.rs`
  (`admitted_group_for` helper), `crates/ynz-typeck/tests/parallel_group_hint_parity.rs` (3
  `admitted_cpu_group` call sites), `crates/ynz-codegen/tests/frame_layouts_query.rs`'s two
  `build_frame_layouts_with_resolver` call sites in `reexport_chain_b_total_size_includes_a_sub_
  frame` and its two `spike_host_subset` call sites in
  `imported_suspending_after_pair_declines_consistently_across_boundaries`.
  Verified (full sweep): `cargo test -p ynz-typeck -p ynz-codegen` — 100% green (including all 34
  `golden.rs` tests, both `queries.rs` `promotion_tests::base_suspends_*` unit tests, all 5
  `parallel_group_hint_parity.rs` tests, all 9 `frame_layouts_query.rs` tests);
  `cargo test -p ynz-driver --test integration v03_m3g` — 4/4 green (3 pre-existing Phase 2
  fixtures + the 1 new bare-intrinsic fixture); `cargo test -p ynz-driver --test integration
  v03_m3d` — 71/71 green (the `_fires_` fixtures confirm no over-declining regression alongside
  the `_declines_` fixtures confirming the floor holds).
  Changes (plan/audit): `.claude/planning/active/2026-07-01-v0-3-m3g-mixed-cpu-io-overlap/plan.md`
  (Phase 2 exit-criteria block amended in place with a BLOCKER FIX note — current-truth body, not
  a new FRAGO entry per dispatch framing); this `audit.md` entry.
  Unchanged: `crates/ynz-typeck/src/cpu_admission.rs`'s nested-branch co-resident-suspension check
  (`:129-135` — pre-existing, NOT part of Phase 2's temporary top-level decline, out of the
  blocker's stated scope; still reads `suspend_set` via the AST scan, unchanged); everything else
  in ¶1, ¶2, ¶3.1, ¶3.2, Phases 1/3-5, ¶4, ¶5, the Invariants section, Design-Doc Alignment, Future
  Requirements.
  Override: N/A — no risk residual rose to HIGH; this is a same-phase correction to Phase 2's own
  landed work, not a new design decision.
