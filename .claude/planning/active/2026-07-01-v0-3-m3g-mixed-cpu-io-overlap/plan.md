---
name: "v0-3-m3g-mixed-cpu-io-overlap"
plan-id: "2026-07-01-v0-3-m3g-mixed-cpu-io-overlap"
status: "active"
roadmap-id: "2026-05-21-v0-3-concurrency-perf"
session-id: ["1b30f567-887b-42a7-990a-0eb5323207a9"]
created_at: "2026-07-01"
updated_at: "2026-07-01"
metadata:
  type: "plan"
---

# PLAN: v0.3-M3g — Mixed CPU+I/O Overlap (poll-path fusion)

> **Status note (recorded decision).** The OPORD body below is complete — Intent/End-State, phases,
> and risks are real, not TBD. Frontmatter stays `stub` because the orchestrator owns the human
> approval gate and flips `stub → active` there (same file, same plan-id, per the plan-format
> schema's graduation rule). Nothing else is missing.

## 1. Situation

### Terrain (landscape — recon-verified file:line; spot-check before building on any anchor)

Two disjoint, non-fused lowering paths exist today, and their non-fusion is the root of the
verified 4c deadlock:

- **I/O inline-poll path (M3b):** `emit_independent_group_poll` at
  `crates/ynz-codegen/src/emit.rs:7812`, routed from the block walker at `emit.rs:3941`. Polls
  child sub-frames in declaration order; yields Pending only while members remain unfinished.
  I/O sub-frames are keyed by **callee name** (one slot per unique suspending callee).
- **CPU join-poll path (M3d):** `emit_cpu_group_spawn_join` at `emit.rs:7162`;
  `ynz_rt_join_poll` call at `emit.rs:7612`. Handle/result slots keyed by
  **(group, member-index)** with per-invocation ctx (same-callee CPU members already parallelize).
- **The seam:** the block walker (`emit.rs:3901-3963`) uses `partition_independent_groups` — NOT
  the classified partition — and CPU groups route through a separate `m3d_spike` recursion trigger
  (`emit.rs:3919-3921`). The two paths share **no continuation**. This is the 4c deadlock root: a
  nested CPU spawn yields Pending into a continuation the outer suspension's resume never
  re-drives, so the spawned handle is never re-polled.
- **Typeck front half SHIPPED, deliberately unconsumed:** `partition_groups_classified` at
  `crates/ynz-typeck/src/independence.rs:369` produces `ClassifiedGroup` with
  `MemberClass::{Suspending, Cpu}` (`independence.rs:203`); its doc at `independence.rs:199-201`
  states both classes "share one continuation" — authored for exactly this fusion.
  `compute_cpu_promotions` at `crates/ynz-typeck/src/queries.rs:696` is computed but unconsumed
  for this case (`queries.rs:553`).
- **Two decline seams to lift (single-sourced in typeck):** (1) the admission gate
  `crates/ynz-typeck/src/cpu_admission.rs:69` `admitted_cpu_group` — the **single decision
  authority**: codegen AND the `parallel_groups` inlay hint both read it
  (`cpu_admission.rs:15-23`). Its co-resident-suspension mixed-decline (`cpu_admission.rs:102-108`)
  guards the NESTED-group branch ONLY; the top-level branch (`cpu_admission.rs:85-95`) returns
  `Some` after checking only `param_read_after_join`, and the member-gate's post-group
  suspension decline exempts intrinsic `wait sleep` (`cpu_admission.rs:131-132`) — see A9 and
  Phase 2's temporary top-level decline. (2) the promotion skip at
  `queries.rs:761` (`base_suspends` hosts never enter the CPU-promotion set) and the guard-probe
  skip of `base_suspends` hosts at `queries.rs:684-692`.
- **Runtime ABI (stabilized M3d P1):** `ynz_rt_spawn_blocking_joinable` /
  `ynz_rt_join_poll(handle, waker_ctx, result_out) -> i32` (0=Ready, 1=Pending; panics on null
  handle per `emit.rs:7143`) / `ynz_rt_join_handle_free`. Codegen decls at
  `crates/ynz-codegen/src/runtime_decls.rs:220-226`, `:674`. Runtime handle cleanup is already
  count-driven and N-general (`cleanup_spike_cpu_handles`, per M3d sub-slice 4d).
- **Frame layout:** M3d reserves a single group-0 CPU handle/result region (32/40/48/64 bytes);
  M3b embeds I/O child sub-frames. M3g needs a **dual-kind group frame** carrying BOTH.
  Cross-module frame reconstruction goes through the M3e `frame_layouts_query` (salsa,
  LLVM-accurate) — a mixed group crossing a module boundary touches this path and **no fixture
  exercises it today** (named gap; Phase 4 closes it).
- **Tests:** golden IR corpus `crates/ynz-codegen/tests/golden.rs` (≈43 test fns, 14
  parallel-related) + ≈13 insta `.snap`; timing-based overlap fixtures (elapsed ≈ max vs ≈ sum)
  are the M3b/M3d proof style; alloc==free accounting fixtures.
- **Teaching surfaces:** `parallel_groups` muted-hint domain (`registry/features.toml` ~:2097 —
  promises hint==binary via the shared admission query), `wait_points` (:2079),
  `background_routing` (:2087). Registry SSOT discipline applies
  ([feature-registry rule](../../../rules/feature-registry.md)).
- **Corpses to honor:** the **M2-HALT synchronous-join corpse** — the fused continuation must be
  genuine async poll-yield, NEVER a `block_on` bridge; **corpse-a unified frame dispatch** — route
  result binding through the canonical binders (`bind_sm_result_and_flush`), never forked loads;
  the **`CPU_GROUP_MEMBER_COUNT == 2` hardcode corpse** M3d already exhumed — no member-count
  hardcodes in the fused path.

### Weather (external constraints)

- **Tokio substrate:** `spawn_blocking` tasks are detached and non-cancellable once dispatched
  ([IMP-concurrency](../../../../docs/internal/implementation/IMP-concurrency.md) "Cancellation
  Detach Constraint"); the blocking pool is FINITE — exhaustion under N mixed groups / recursion
  is a real failure mode (risk E8).
- **LLVM 18** via the `dev` docker-compose service; all build/test/fixture work runs in-container
  per the project [CLAUDE.md](../../../../CLAUDE.md).
- **CI contention:** wall-clock-deadline concurrency fixtures flake under full-`--workspace` load
  (precedent: `.claude/todos.md` `v03_m3e_alias_local_name_collision` entry). Timing-overlap
  proofs must be ratio-based with generous budgets, never tight absolute deadlines.
- **No calendar deadline** — correctness paces (consistent with M3a–M3f; flagged for the human
  approval gate to veto). M3g sequences **before the v0.3.0 tag** (mandatory per Patrick
  2026-06-19).

### Friendly forces

- **Roadmap:** `2026-05-21-v0-3-concurrency-perf` §Milestone 3g (this plan's authority) and its
  Capability Ledger row "Mixed CPU+I/O overlap → M3g". Depends on M3b (I/O path, shipped) + M3d
  (CPU path, shipped `v0.3.0-m7`).
- **M3f** (pre-existing codegen correctness) also blocks the `v0.3.0` final tag — both bugs
  shipped 2026-06-09; no ordering dependency on M3g.
- **M3d's safe DECLINE** (mixed → sequential, byte-identical) is the correctness floor this
  milestone must never regress — it exists precisely so M3g can't silently break it.

### Assumptions (verify before relying)

| # | Assumption | Status |
|---|---|---|
| A1 | The three M3d DECLINE fixtures live at `crates/ynz-driver/tests/integration.rs:6287` / `:6301` / `:6355` | **verified** (this session) |
| A2 | `partition_groups_classified` / `MemberClass` docs state the shared-continuation intent (`independence.rs:197-209`) | **verified** (this session) |
| A3 | `compute_cpu_promotions` skips `base_suspends` hosts at `queries.rs:761`; guard-probe skips them at `queries.rs:684-692` | **verified** (this session) |
| A4 | `admitted_cpu_group` at `cpu_admission.rs:69` is the single decision authority read by codegen AND the inlay hint | **verified** (this session) |
| A5 | Exact golden/insta regression-snapshot count + locations (≈43 fns / ≈13 snaps per recon) | **verified** (Phase 1, 2026-07-01) — baseline commit `a8e11fa`. `crates/ynz-codegen/tests/` = **43** `#[test]` fns exactly (golden.rs 34 + frame_layouts_query.rs 8 + audit_hidden_field_defaults.rs 1); golden.rs carries **13** `assert_snapshot!` calls, matching **13** committed `.snap` files under `crates/ynz-codegen/tests/snapshots/` (all `golden__*.snap`). Workspace-wide insta `.snap` count = **35** (13 ynz-codegen + 11 ynz-parser + 6 ynz-driver + 5 ynz-diagnostics). Full `cargo test --workspace` baseline on `a8e11fa`: **2076 passed / 6 failed** — all 6 failures are **pre-existing, confirmed via `git stash` bisection against clean `main`, unrelated to this plan**: (1) `ynz-diagnostics::jargon_audit::no_banned_jargon_in_deferred_feature_user_facing_fields` (registry wording, banned word "implementation"); (2) `ynz-driver::integration::v03_m3e_alias_local_name_collision_runs_correctly` (the documented CI-contention flake — passes 1/1 in isolation, confirmed); (3) `ynz-parser::parse::parser_precedence_table_matches_spec` + (4) `ynz-registry::design_future_sync::every_future_doc_has_a_registry_entry_or_is_skipped` + (5-6) `ynz-registry::schema_smoke::deferred_language_feature_lookup`/`deferred_tooling_feature_lookup` — all four of (3)-(6) are stale hardcoded `spec/`/`design/` paths left by the 2026-07-01 docs-taxonomy migration (`3bdaa1e`/`5e8e722`), tracked at `.claude/todos.md` "docs-migration / stale hardcoded paths." **Phase 1's own diff introduces zero new failures and zero golden-IR churn** (all insta snapshot tests pass byte-identical; zero `.snap.new` files produced). |
| A6 | `wait_on_non_may_block` diagnostic text is single-sourced (14 files reference it) | **verified — was hand-duplicated, now fixed** (Phase 1, 2026-07-01). Audited all ~13 referencing files; the actual emission was hand-duplicated in exactly TWO `ynz-typeck/src/check.rs` call sites (the CPU-only-intrinsic arm and the transitive-user-fn arm), with THREE divergent wordings across those two sites + the `registry/features.toml` `[[diagnostic_template]]` `WaitOnNonMayBlockWarning` entry (which nothing in the codebase actually renders through — `render_template`/`diagnostic_template_lookup` have zero consumers workspace-wide, confirmed by grep). Fixed via the "one Rust home" option: a new `wait_on_non_may_block_warning()` free fn in `check.rs` builds the canonical WHAT/WHY (now byte-identical to the registry's `what_template`/`why_template`); both call sites route through it, keeping only their pre-existing WHAT-INSTEAD call-rendering difference (intrinsic call sites have no accessible arg list at that point; user-fn call sites render formal param names) as an explicit parameter rather than folded into the shared text. **Correction (cheap-gate review, 2026-07-01, graveyard-auditor should-fix):** the original claim above ("none pinned the exact divergent text") was WRONG for one test — `ynz-typeck/tests/check.rs::transitive_no_wait_compiles_clean_under_inference` POSITIVE-assertion-2 filtered stale warnings on `d.what.contains("never suspends")`, the exact substring that only existed in the transitive-callee arm's PRE-unification text. After unification that substring appears nowhere in diagnostic-producing code, so the assertion started passing unconditionally — a real regression-guard silently gutted (Test-Weakening corpse), not caught until cheap-gate review. Fixed by re-pointing the filter at `"no effect"`, the substring the unified WHAT text actually contains (matching the pattern already used by three other assertions in the same file, e.g. line ~3693). Verified with a temporary source-level fault injection (`queries.rs` `local_suspends` forced `false`, simulating a dead may-block fixpoint): the corrected assertion fails red as expected; reverting to the OLD `"never suspends"` filter with the same injected fault reproduces the false-negative (passes green) — proof the finding was real and the fix restores genuine detection power. Both temporary edits reverted; `cargo test -p ynz-typeck` green (all suites, 0 failures) on the final state. |
| A7 | Ratio-based timing fixtures are a viable overlap proof under CI contention | **verified — protocol confirmed AND refined from codebase precedent** (Phase 1, 2026-07-01). The established, working pattern (M2 `v03_m2_concurrent_waits_proof`, M3b/M3d fixtures) is NOT a pure elapsed-ratio comparison — it is a **deterministic ORDERING assertion** as the primary proof (e.g. "all N `START` lines appear before any `DONE` line" — core-count-independent, immune to CI contention) with **generous, loose wall-clock bounds as a secondary sanity check only** (a floor to catch a no-op, a high ceiling to catch a hang — never a tight sum-vs-max ratio). The contention caveat + isolation fallback is directly evidenced by the `v03_m3e_alias_local_name_collision_runs_correctly` flake (`.claude/todos.md`): a FIXED wall-clock deadline test flakes under full-`--workspace` parallel load (confirmed failing in the full run, confirmed passing 1/1 in isolation this session) while being deterministically correct standalone — its own documented fix options are "widen the budget" or "replace the deadline with a deterministic completion signal," i.e. exactly the ordering-assertion pattern above. **Recommendation for M3g's mixed-group overlap fixtures**: prefer a deterministic ordering/interleaving assertion over the members as the primary proof; keep any wall-clock ratio check as a loose secondary guard with a wide margin, and never as the sole pass/fail signal. |
| A8 | The N=2 codegen member-count guard (`children.len() != 2` class) may still be present on `main` | **unverified** — Phase 4 handles either way (the fused path carries no count hardcode regardless) |
| A9 | The top-level admission branch (`cpu_admission.rs:85-95`) has NO co-resident-suspension decline — the `:102-108` check is nested-branch-only, and the post-group gate exempts intrinsic `wait sleep` (`:131-132`); the emit-side spike fire site (`emit.rs:3787-3850`, BEFORE the `:3901-3963` partition loop in the same `lower_sm_block`) is gated per-function by module-global `m3d_spike` (`emit.rs:873`) + `spike_cpu_candidates`→`admitted_cpu_group` (`emit.rs:2297`, `:2583`, `:6730`) with no per-function promotion check (`promoted.contains` lives only in `spike_host_subset`, `emit.rs:6947-6959`, feeding frame sizing / suspend-set union, not the fire site); the hint pass independently skips suspending hosts (`inlay_hint_passes.rs:1935`) | **verified** (this session) |

### Risk Assessment (scored via the frozen risk engine — tiers fixed by the matrix; mitigations woven into phases)

| Risk | Prob | Sev | Initial | Mitigations (bucket / axis / step) | Residual | Gate |
|------|------|-----|---------|------------------------------------|----------|------|
| **E1** 4c fused-continuation **deadlock** (built twice, deadlocked twice — todos "mixed CPU+I/O overlap" residual) | A | II | EH | RED adversarial deadlock gate + 3 flip-fixtures block build (B2 / prob / −1) · `--no-auto-parallel` kill-switch (B2 / sev / −1) — Phase 3 | **M** | record |
| **E2** silent miscompile flipping the DECLINE floor | B | II | H | byte-identity oracle fixtures (B2 / prob / −1) · kill-switch (B2 / sev / −1) — Phases 2–4 | **M** | record |
| **E3** regress shipped M3b/M3d paths | C | III | M | golden-IR snapshot gate (B2 / prob / −1) — Phase 1 baseline, enforced every phase | **L** | pass |
| **E4** frame-header ABI drift — offsets dual-defined codegen↔runtime, no compile binding (todos frame-header entry) | B | II | H | **PLAN OBLIGATION (Phase 1):** shared-constant single home + compile-forced binding (B1 / prob / −2; proof = the shared home + source-scan gate committed) | **M** | record |
| **E5** teaching surfaces lie about mixed groups | B | III | M | single-source admission gate + hint==spawn parity tests (B1 / prob / −2) — Phases 3, 5 | **L** | pass |
| **E6** promotion opens un-guard-probed CPU-join crossings (`queries.rs:684-692`) | B | II | H | **PLAN OBLIGATION (Phase 2):** extend guard-probe to `base_suspends` hosts (B1 / prob / −2; proof = adversarial decline fixtures) | **M** | record |
| **E7** wide-EC UAF admitted into fused groups (todos wide-EC-staging-slot entry; `emit.rs:6604`) | C | II | H | **PLAN OBLIGATION (Phase 3):** ratchet test — the decline-around survives fusion (B2 / prob / −1) | **M** | record |
| **E8** blocking-pool exhaustion (N mixed groups / recursion) | C | II | H | **PLAN OBLIGATION (Phase 3):** exhaustion-stress fixture gates build (B2 / prob / −1) | **M** | record |

No Floor-B class fires (no security/PII/money/irreversible-operation surface). **No HIGH residual —
no override block needed.** The four PLAN OBLIGATIONS are load-bearing: without their proof
artifacts the mitigation step is 0 and E4/E6 revert to HIGH. Every M residual is parked in
[Future Requirements / Revisit](#future-requirements--revisit) with its trigger.

### Cross-cutting factor sweep (mandatory factors — addressed or N/A with why)

| Factor | Disposition |
|---|---|
| Security | N/A as a trust-boundary concern — pre-release compiler, audience of one, no injection/authn/secret surface. The domain's analog (silent miscompile, Golden Rule 5) is scored as E2. |
| Perf / BigO | The milestone IS the perf work. Fused poll is O(members) per resume (mem: one dual-kind frame per group, no per-resume allocation); partition/promotion analyses stay O(F)-dominated per existing docs. Overlap proven by ratio fixtures (Phase 3 step 10). |
| Accessibility | N/A — no UI beyond IDE hints, which follow the locked placement categories in the [inference rule](../../../rules/inference.md); no new surface category. |
| PII / privacy | N/A — no user data touched anywhere in the toolchain. |
| Compliance | N/A — no regulated surface. |
| SEO | N/A — no web surface. |
| Docs | Addressed — Phase 5 amends [IMP-concurrency](../../../../docs/internal/implementation/IMP-concurrency.md) (the 4c decline divergences flip), extends demo + gallery, updates hint/hover text. |
| Reusability / DRY | Addressed — single-source admission gate stays the one authority (E5); canonical binders reused (corpse-a); frame-header offsets get ONE home (E4); diagnostic text single-sourced (A6, Phase 1). |
| Type-safety | Addressed — new match sites over `MemberClass` / return classes carry NO `_` catch-all (the M3d `parity_case` compile-forced-exhaustive pattern); a new variant is a build error, not a silent decline. |
| Idempotency | Addressed — the fused poll must be re-entrant: re-driving an already-Ready handle or re-polling a completed I/O sub-frame must never double-bind a result (Phase 3 step 3; fixture-asserted). |
| Error-handling | Addressed — panic re-raise semantics (first panic wins, after all members settle) preserved across BOTH classes (Phase 4 step 4); `errors`-capable members exercised in the matrix. |
| Observability / logging | Addressed at the compile-time layer — the muted-hint domain is the user's visibility into what fired (E5), and `--no-auto-parallel` is the diagnostic bisect tool. No runtime tracing added: a print-based fixture corpus + byte-identity oracle is the established debug surface; adding runtime instrumentation is out of scope (would perturb the timing proofs). |
| Race / TOCTOU | **The core of the milestone** — NOT N/A. Deadlock gate (E1), byte-identity oracle (E2), detach semantics, shutdown-abort race interaction (Phase 4 step 5), watchdog harness on every concurrency fixture. |
| Resource-cleanup | Addressed — count-driven handle free extended/verified for dual-kind frames; alloc==free accounting on every new fixture; early-return (detach) shapes leak nothing (Phase 4 step 3). |

## 2. Mission

Fuse the M3b I/O inline-poll path and the M3d CPU join-poll path in the Yinz compiler — before
the v0.3.0 tag — so that ONE `Parallel` group's shared continuation drives N CPU members and M
I/O members concurrently instead of sequentially, **because** mixed-group sequential execution is
the last gap in "all independent operations auto-parallelize" and the most-efficient-language
mission (Patrick, 2026-06-19) makes closing it mandatory.

## 3. Execution

### 3.1 Intent & End State

**Purpose.** Yinz's whole concurrency story is "the compiler overlaps everything it can prove
independent, with zero user syntax." Today a body mixing one heavy CPU call and one I/O call runs
them in sequence — correct, but a lie against the positioning. M3g makes the compiler's promise
true for mixed work-classes, without ever trading away the safety discipline (safe DECLINE,
byte-identity, no sync bridge) that got M3a–M3f shipped without a single silent concurrency
miscompile reaching `main`.

**Key outcomes.**

1. **The acceptance signal:** the three M3d DECLINE fixtures
   (`v03_m3d_mixed_cpu_io_group_declines_byte_identical`,
   `v03_m3d_nested_group_with_outer_wait_declines_byte_identical`,
   `v03_m3d_nested_group_with_suspending_callee_no_abort_byte_identical`) FLIP from
   decline-asserting to fire-asserting, and pass.
2. **General fusion within one group:** N CPU members + M I/O members share one continuation —
   no 1+1 hardcode, no member-count cap (the `CPU_GROUP_MEMBER_COUNT == 2` corpse stays buried).
3. **The floor never regresses:** every shape M3g still declines (loop-body, multi-group ≥2,
   wide-EC members, guard-tripping crossings, kernel mode) declines to sequential,
   **byte-identical** to `--no-auto-parallel`; `default == --no-auto-parallel` output identity
   holds everywhere.
4. **No deadlock:** the 4c failure mode is dead — every resume of the shared continuation
   re-drives every live CPU handle AND every pending I/O sub-frame; the adversarial deadlock gate
   and the exhaustion-stress fixture gate the build.
5. **Teaching surfaces stay truthful:** the `parallel_groups` hint and the emitted binary agree
   (shared admission query), proven by parity tests over mixed shapes.
6. Demo (`pirates-roster` mixed section) + error gallery (`v0_3_m3g_errors.ynz`) extended;
   [IMP-concurrency](../../../../docs/internal/implementation/IMP-concurrency.md) amended;
   registry entries enumerated; release tagged.

**Definition of done:** all six outcomes hold on `main` with the full workspace green
(`cargo test --workspace`, clippy `-D warnings`), and the v0.3.0-m{next} tag is cut.

**Disciplined initiative — when steps and reality diverge, this is the priority order:**

1. **Never a synchronous bridge.** If a fusion step only seems achievable via a blocking join /
   `block_on`-shaped construct, the step is wrong, not the design — STOP and surface (M2-HALT
   corpse; [IMP-no-function-coloring](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
   is the governing doc and it wins).
2. **Never regress the floor.** Byte-identity and the safe-DECLINE discipline outrank any firing
   shape. When in doubt, decline the shape safely and record it — with ONE exception:
3. **The three flip fixtures are non-negotiable.** They ARE the milestone. A "safe decline" of
   any of those three shapes is milestone failure, not disciplined initiative — if one cannot be
   made to fire safely, that is a HALT-and-surface, not a quiet decline.
4. Edge shapes beyond the three (exotic member mixes, cross-module corners) MAY decline safely
   if firing them is proven unsafe mid-execution — each such decline gets a locked decline-test +
   a four-field deferral in Future Requirements, M3d-style.

### 3.2 Concept

Five phases, five PRs, strictly sequenced. Phase 1 hardens the ABI seam and pins the regression
baseline BEFORE anything moves (cheapest-stage discipline). Phase 2 builds the typeck front half
**behavior-neutral** (machinery dark, admission still declines) so the byte-identity oracle proves
it inert. Phase 3 is the fusion core — codegen dual-kind frame + shared-continuation poll — and
flips the admission gate **in the same change** so hint==binary parity never has a window where it
lies; the deadlock gate, the three fixture flips, the exhaustion stress, and the wide-EC ratchet
all gate Phase 3's merge. Phase 4 widens to the generality/boundary matrix (N+M shapes,
cross-module, cleanup/panic, interaction sweeps). Phase 5 closes teaching surfaces, docs,
registry, and the release. Handoff between phases is the merged PR + this plan's checkbox state.

### 3.3 Phases

#### Phase 1 — Pre-fusion floor: frame-ABI single home + regression baseline + info-gap recon

- **Task + purpose:** eliminate the E4 drift surface *before* dual-kind frame work multiplies
  header touches; pin the concrete baseline the later "no regression" exit criteria compare
  against; resolve assumptions A5–A7. No user-visible behavior change.
- **Steps:**
  - [x] Move the general SM frame-header offsets (`resume_point@0`, `sleep_handle@8`,
        `return_slot@16`, `FRAME_HEADER_SIZE`) into `ynz-abi` as the single home BOTH
        `ynz-codegen` and `ynz-runtime` consume (the exact pattern M3d sub-slice 4d established
        for the spike-frame constants); delete the dual definitions
        (`runtime.rs:53` `FRAME_SLEEP_HANDLE_OFFSET` vs `state_machine.rs:63`
        `FRAME_OFFSET_SLEEP_HANDLE`); mechanical call-site sweep (~15 `FRAME_HEADER_SIZE` sites).
        *Recorded decision:* the todos frame-header entry deferred this to "the next time the SM
        frame-header layout is touched" — M3g's dual-kind frame IS that trigger, so the full
        single-home move (not just const-asserts) is the durable answer; local const-asserts
        alone still allow a both-sides-forgotten drift.
        **DONE:** `crates/ynz-abi/src/lib.rs` now defines `FRAME_OFFSET_RESUME_POINT`,
        `FRAME_OFFSET_SLEEP_HANDLE`, `FRAME_OFFSET_RETURN_SLOT`, `FRAME_HEADER_SIZE`; both local
        `pub const` definitions deleted (`state_machine.rs`, `runtime.rs`); all call sites swept
        to `ynz_abi::` (13 sites in `emit.rs`, 1 in `queries.rs`, 1 test-import in
        `frame_layouts_query.rs`, 3 pointer-arithmetic sites in `runtime.rs`).
  - [x] Extend the `spike_frame_abi_no_bare_offsets` source-scan gate to cover bare
        `.add(8)`/`.add(16)` and `const_int(8|16)` uses of these offsets. **E4 proof artifact =
        the shared home + the extended gate, committed.** Demonstrate the guarantee with a
        mutation check (change one offset → build/test fails) recorded in the PR.
        **DONE:** `.add(8)`/`.add(16)` folded into the existing simple substring gate
        (`no_bare_spike_frame_offset_literals`) — safe because, post-sweep, zero legitimate
        non-header `.add(8)`/`.add(16)` calls remain in `runtime.rs`/`lib.rs` (verified). A NEW
        gate `no_bare_general_frame_header_offset_in_codegen_gep` handles the codegen side, where
        the literal values 8/16 are NOT unique (ctx-size args, alloc-size args, and the
        return-slot's own internal field stride all legitimately use them) — it scans for the
        `.build_gep(ctx.i8_type(), <base>, &[const_int(N, false)], ...)` shape and flags a bare
        8/16 literal only when the GEP base is not a documented already-offset exception. BOTH
        mutation checks demonstrated this session: (1) source-level — reverted a named-constant
        GEP offset to a bare `8` literal in `state_machine.rs`; the new gate failed with the
        exact violation line, then passed again after revert; (2) value-level — changed
        `ynz_abi::FRAME_HEADER_SIZE` from 32 to 40; `cargo build -p ynz-codegen` failed on the
        `SPIKE_HANDLE_BASE_OFFSET == FRAME_HEADER_SIZE` compile-time assertion, then built clean
        after revert.
  - [x] Inventory the regression corpus: exact golden-IR test count + insta `.snap` count +
        locations; record the baseline commit; FRAGO the concrete numbers into ¶1 A5 (this is
        what makes E3's "no regression" checkable instead of asserted).
        **DONE — see ¶1 A5** (43 test fns / 13 golden.rs snaps / 35 workspace .snap files /
        baseline commit `a8e11fa` / 2076 passed, 6 pre-existing-unrelated failed).
  - [x] Confirm the overlap-proof protocol (A7): ratio-based timing fixtures (elapsed <
        (sum-of-members × margin), and ≥ the max member) with generous budgets; document the
        contention caveat and the isolation fallback per the todos flake precedent.
        **DONE — see ¶1 A7** (protocol confirmed AND refined: prefer a deterministic ordering
        assertion as primary proof, per the actual established M2/M3b/M3d pattern; wall-clock
        bounds stay loose/secondary).
  - [x] Audit `wait_on_non_may_block` diagnostic text across its ~14 referencing files (A6). If
        hand-duplicated → single-source it (registry `[[diagnostic_template]]` or one Rust home)
        NOW, before Phase 2/3 change when `wait` is legal on a CPU-group host. Hand-duplication
        caused an M3d gate block; do not re-live it.
        **DONE — see ¶1 A6** (WAS hand-duplicated across 2 code call sites + the registry entry,
        3-way-divergent wording; single-sourced via one Rust home, `wait_on_non_may_block_warning()`
        in `check.rs`; cheap-gate review caught one test that DID pin the old divergent text and
        had gone blind post-unification — fixed, see ¶1 A6 correction note).
- **Exit criteria:** workspace + clippy green; drift-mutation check demonstrated; baseline
  inventory FRAGO'd into ¶1; diagnostic text single-sourced or confirmed already-DRY; zero
  behavior change (golden corpus byte-identical).
  **STATUS: ALL MET.** `cargo build --workspace` clean; `cargo clippy --workspace -- -D warnings`
  clean; `cargo fmt --all -- --check` clean; both mutation checks demonstrated (source-level +
  value-level, both reverted after confirming red→green); A5/A6/A7 FRAGO'd to verified above;
  `wait_on_non_may_block` single-sourced; golden-IR corpus confirmed byte-identical (all 13
  `golden.rs` insta snapshot assertions pass, zero `.snap.new` produced, `ir_text_snapshot` +
  `m4_player_ir_snapshot` + all `v03_*_ir_snapshot` tests green). One pre-existing, unrelated
  gap surfaced and left unfixed (out of Phase-1 scope; tracked): `cargo test --workspace` is not
  100% green on `main` independent of this phase — 6 failures, all confirmed pre-existing via
  `git stash` bisection (5 stale-path fallout from the 2026-07-01 docs migration + 1 documented
  CI-contention flake + 1 unrelated jargon-wording gap); see `.claude/todos.md`
  "docs-migration / stale hardcoded paths" entry (added this session) and the pre-existing
  `v03_m3e_alias_local_name_collision` flake entry. Phase 1's own diff adds zero new failures.
- **Reviewer fan-out:** code-reviewer (cross-crate ABI seam + call-site sweep completeness).
- **Model tag:** `(coding, standard, medium)`

#### Phase 2 — Typeck front half: `base_suspends` host promotion + guard-probe extension (behavior-neutral)

- **Task + purpose:** build the analysis machinery fusion needs — suspending hosts entering the
  CPU-promotion set, guard-probing their CPU-join crossings, classified groups plumbed to
  codegen's inputs — while the admission gate still declines mixed shapes, so the phase is
  provably inert. Closes E6 at the cheapest stage.
- **Steps:**
  - [x] Lift the `base_suspends` skip in `compute_cpu_promotions` (`queries.rs:761`) so a host
        that already suspends (own `wait` or suspending callee) can still host a CPU group —
        **and, in the SAME change, add a temporary co-resident-suspension decline to
        `admitted_cpu_group`'s TOP-LEVEL branch** (the mirror of the nested branch's
        `cpu_admission.rs:102-108` check), removed by Phase 3's admission flip.
        *Why this exact mechanism (codebase-verified, A9):* NO existing gate keeps this phase
        neutral for a top-level CPU group in a suspending host (`crunch(x); crunch(y);
        wait sleep(5)`). The `:102-108` decline is nested-branch-only; the top-level branch
        (`:85-95`) admits after checking only `param_read_after_join`, with intrinsic
        `wait sleep` exempt from the post-group decline (`:131-132`). The unchanged block-walker
        routing is NOT the guarantor either: the spike fire site (`emit.rs:3787-3850`) runs
        BEFORE the `partition_independent_groups` loop (`emit.rs:3901-3963`) in the same
        `lower_sm_block`, and its per-function gate (`emit.rs:2297` → `:2583`) is module-global
        `m3d_spike` + `spike_cpu_candidates` → `admitted_cpu_group`, with no per-function
        promotion check. So with `:761` lifted, that host is promoted → module `m3d_spike`
        flips true → the spike FIRES: the `:761` skip being removed IS today's only binary-side
        decline for the class. The temporary decline lives in the single decision authority, so
        hint==binary parity holds by construction (the hint pass independently skips suspending
        hosts at `inlay_hint_passes.rs:1935` until Phase 3/5 extend it for mixed groups).
        *Recorded decision:* the admission gate's mixed-flip still does NOT happen in this
        phase — machinery lands dark; the nested mixed-decline AND this temporary top-level
        decline are removed together in Phase 3, traveling with the fused codegen they enable
        (flipping earlier would either crash or make the hint lie — E5). The byte-identity exit
        gate remains the PROOF of neutrality; this step is the mechanism that makes it pass.
        **DONE:** `crates/ynz-typeck/src/cpu_admission.rs` `admitted_cpu_group` top-level branch
        now declines when `f.body.stmts.iter().any(stmt_contains_wait_deep ||
        stmt_contains_suspending_call_deep)`, mirroring the nested branch exactly, with an inline
        `TEMPORARY (v0.3-M3g Phase 2)` comment naming the Phase-3 removal. `queries.rs`'s
        candidate-identification skip (`base_suspends.contains(&f.name) || ...`) now only excludes
        `cyclic_members`.
  - [x] **Neutrality fixtures for the wrong-premise class (pinned by test, not prose):**
        (a) a top-level CPU group in a suspending host — `crunch(x); crunch(y); wait sleep(5)`
        — decline-asserting (0 spawns) + byte-identical; (b) the same host in a module that
        ALSO contains a separately promoted pure-CPU function (module-global `m3d_spike` true
        per `emit.rs:873` — the A9 gating asymmetry makes this variant reachable TODAY,
        independent of the `:761` lift). Probe (b) against the Phase 1 baseline FIRST: if it
        already fires on `main`, that is a pre-existing latent misfire — Paper-Trace it, record
        it in the PR + a FRAGO, and the temporary decline deliberately closes it (a justified,
        recorded divergence, not silent churn); if it declines today, pin it decline-asserting.
        Caution: dormant fixtures `v0_3_m3d_spike_g_mixed.ynz` / `v0_3_m3d_spike_h_two_waits.ynz`
        describe this shape FIRING — they are unreferenced Phase-0-spike leftovers (no test
        consumes them; verified this session); do not treat their comments as current truth.
        **DONE:** fixture (a) = `v0_3_m3g_top_level_group_in_suspending_host_declines.ynz` +
        `v03_m3g_top_level_group_in_suspending_host_declines_byte_identical` — declines both
        before and after this phase's change (0 spawns), confirmed by direct build probe against
        the unmodified Phase 1 baseline commit `d184934` before any code was touched this
        session. Fixture (b) = `v0_3_m3g_mixed_host_with_promoted_sibling_declines.ynz` +
        `v03_m3g_mixed_host_with_promoted_sibling_declines_byte_identical` — **Paper-Traced: this
        DID already fire on the Phase 1 baseline `d184934`, unmodified** (4 spawn calls observed:
        2 legitimate from `pureCpuHost`, 2 from the pre-existing latent misfire in `mixedHost`,
        confirmed via a direct `--emit-ir` build probe before any Phase 2 code change). Recorded
        as FRAGO 002 in `audit.md`. The Phase 2 temporary decline closes it (now exactly 2
        spawns, both `pureCpuHost`'s).
  - [x] **E6 obligation:** extend the guard-probe (`queries.rs:684-692`) to `base_suspends`
        hosts — a guard-tripping local crossing a CPU join inside a suspending host must
        DECLINE. **Proof artifact = adversarial fixtures**: guard-tripping shapes inside
        suspending hosts, decline-asserting + byte-identical (these are permanent safety
        declines, NOT flip candidates).
        **DONE:** `compute_cpu_promotions`'s rollback loop now also probes every `base_suspends`
        host that is itself a direct promotion candidate (`is_base_suspends_direct_candidate`),
        not just the newly-SM closure. Proof artifacts: (1) two `ynz-typeck` unit tests —
        `base_suspends_host_with_clean_cpu_group_promotes` (positive control) and
        `base_suspends_host_with_subexpr_position_guard_declines_promotion` (the E6 proof,
        fault-injection-verified red→green: reverting the extension makes the test fail with
        `entrypoint` wrongly promoted, confirming the test is non-vacuous); (2) the integration
        fixture `v0_3_m3g_guard_tripping_crossing_in_suspending_host_declines.ynz` +
        `v03_m3g_guard_tripping_crossing_in_suspending_host_declines_byte_identical` (0 spawns,
        byte-identical). **Deviation from the plan's literal guard-type suggestion:** the plan's
        prose example (a nested-shape crossing) turned out to be UNREACHABLE as an E6-isolated
        fixture — ANY nested-shape `let` anywhere in ANY function containing a real `wait`
        ALREADY hard-errors at ordinary (pre-promotion) `check_query` time, regardless of
        position relative to the wait (verified empirically) — so a nested-shape guard can never
        isolate the CPU-join-specific E6 hazard from the pre-existing whole-function hard error.
        Used the "suspending call in sub-expression position" guard instead (a third, separate
        call to the CPU-group's own callee in subexpr position — invisible to ordinary check,
        visible only once the guard-probe augments `suspending_fns` with the candidate's own CPU
        callees) — a different guard TYPE from the plan's example, but the same E6 MECHANISM
        (probing `base_suspends` direct candidates). Surfaced here per dispatch instructions
        (plan-said-nested-shape / reality-is-unreachable-for-this-purpose).
  - [x] Plumb `partition_groups_classified` / `ClassifiedGroup` through to the codegen input
        surface (types/queries only; the block walker still routes the old paths).
        **DONE (recorded interpretation):** `CheckOutput::does_real_work_set` — the one input
        `ynz_typeck::independence::CpuCandidacy` needs beyond what codegen already has
        (`sig_table`, `suspend_set`, `imported_fns`) to call `partition_groups_classified` — is
        now threaded through the real production call chain (`codegen_query` →
        `emit_artifact` → `build_module` → `lower_function` → `lower_function_with_waits` →
        `Cg::does_real_work`), mirroring the exact established pattern for sibling fields.
        Structurally wired but genuinely UNCONSUMED this phase (`#[allow(dead_code)]` + WHY
        comment, mirroring the pre-existing `Cg::wait_cache` precedent) — no lowering path reads
        it; the block walker is untouched. `ClassifiedGroup`/`MemberClass`/
        `partition_groups_classified` themselves were already `pub` and crate-reachable from
        `ynz-codegen` (no typeck-side visibility change needed) — `does_real_work` was the one
        missing plumbing gap. Mirrors the "computed but unconsumed" pattern M3d's own Phase 2
        established for `PromotionOutput`.
  - [x] Unit-test the promotion of suspending hosts (the `compute_cpu_promotions` test harness
        at `queries.rs:1046+` already takes `base_suspends` explicitly — extend it).
        **DONE** — see the two new `promotion_tests` in `queries.rs` cited above.
- **Exit criteria:** workspace green; **golden corpus byte-identical to the Phase 1 baseline**
  (the proof of behavior-neutrality); the three M3d DECLINE fixtures still decline (floor
  intact); the top-level-group-in-suspending-host neutrality fixtures (both variants) committed
  and green (decline-asserting + byte-identical, or the variant-(b) Paper-Traced divergence
  recorded); new guard-probe adversarial fixtures committed and green; no `_` catch-all in any
  new `MemberClass`/classification match.
  **STATUS: ALL MET.** `cargo build --workspace` clean; `cargo clippy --workspace -- -D
  warnings` clean (the project's documented gate — a broader `--all-targets` sweep surfaces
  ~25 pre-existing warnings across `ynz-numerics`/`ynz-fmt`/`ynz-registry`/`ynz-lsp`/
  `ynz-runtime` test targets, confirmed via `git stash` bisection to exist unchanged on the
  Phase 1 baseline `d184934`, zero new); `cargo fmt --all -- --check` clean. Golden-IR corpus:
  all 34 `ynz-codegen` tests pass including all `*_ir_snapshot` insta assertions, zero
  `.snap.new` produced anywhere in the tree. The three M3d DECLINE fixtures re-run individually
  and still decline. Both neutrality fixtures + the E6 adversarial fixture green. One
  PRE-EXISTING `ynz-codegen` test
  (`spike_host_subset_bare_admits_effective_declines_on_imported_post_pair`) broke as a direct,
  foreseeable consequence of this phase's OWN temporary decline (Paper-Traced below) — fixed by
  changing its INPUT CONSTRUCTION (inline local-function source + synthetic suspend-set
  split) to isolate its original divergence again, without weakening any assertion; see the
  session-log entry in `audit.md` for the full Paper-Trace. `cargo test --workspace
  --no-fail-fast`: **2084 passed / 5 failed** — the 5 failures are BY NAME the same 5 of A5's 6
  pre-existing failures that are NOT the documented flake (`no_banned_jargon_in_deferred_
  feature_user_facing_fields`, `parser_precedence_table_matches_spec`, `every_future_doc_has_a_
  registry_entry_or_is_skipped`, `deferred_language_feature_lookup`,
  `deferred_tooling_feature_lookup`); the 6th A5 failure
  (`v03_m3e_alias_local_name_collision_runs_correctly`, the documented CI-contention flake) PASSED
  this run — consistent with its known flaky nature, not a fix. Zero new failures introduced by
  this phase's diff; the +8 net new passes over A5's 2076 include this phase's 5 new tests (3
  integration fixtures + 2 `queries.rs` unit tests) plus the flake's this-run pass plus normal
  run-to-run doctest/count variance.
- **BLOCKER FIX (code-reviewer, post-landing):** the reviewer fleet found the temporary top-level
  co-resident-suspension decline (`admitted_cpu_group`, `cpu_admission.rs:92-137` as landed) missed
  a BARE may-block-intrinsic call (`sleep(0)` with no `wait` keyword) — the decline re-derived
  "does this host suspend?" via an AST scan (`stmt_contains_wait_deep` — literal `wait` node only —
  OR `stmt_contains_suspending_call_deep` — which explicitly exempts may-block intrinsics for a
  DIFFERENT, unrelated reason) that never recognized a bare `sleep()` call as a suspension point,
  even though `may_block::analyze` genuinely seeds `calls_may_block_intrinsic` for it independent
  of `wait` syntax. A host with `crunch(x); crunch(y); sleep(0)` (no `wait`) would have admitted
  and FIRED its CPU group at codegen — breaking the "provably inert" neutrality promise for a legal
  Yinz shape, untested by any of this phase's three fixtures (all three use explicit
  `wait sleep(...)`). **FIXED**: the decline now reads `base_suspends.contains(&f.name)` — the
  SAME authoritative pre-promotion suspend set `compute_cpu_promotions` already reads as
  `base_suspends` — instead of a second AST scan, closing the bare-intrinsic hole (and any future
  may-block-intrinsic or bare-suspension shape) without a second detector to keep in sync.
  `base_suspends` is threaded as a genuinely separate parameter from `suspend_set` at every call
  site down to `admitted_cpu_group` (typeck: `admitted_cpu_group`, `inlay_hint_passes.rs`; codegen:
  `spike_cpu_candidates` → `cpu_group_slots_and_reserve` / `spike_host_cpu_supported` /
  `compute_frame_size` / `build_frame_layouts_with_resolver` / `spike_host_subset` →
  `lower_function_with_waits` → `lower_function` → `build_module` → `emit_artifact`; queries.rs
  computes the pure pre-promotion set ONCE per query boundary and never mutates it, passing the
  union-with-spike-hosts set separately as `suspend_set`) — NOT reused from `suspend_set`, because
  at every codegen call site `suspend_set` is `base_suspends ∪ spike_hosts` (every function codegen
  may spike-host, including `f` itself once admitted), so checking `f`'s own name against THAT
  union would self-decline a legitimate pure-CPU host the instant it is admitted. New regression
  fixture `v0_3_m3g_top_level_group_in_suspending_host_bare_intrinsic_declines.ynz` (bare-intrinsic
  twin of neutrality fixture (a)) added to `ynz-driver/tests/integration.rs`; empirically confirmed
  (temporary revert to the old AST-scan predicate) to show 2 spawns pre-fix, 0 spawns post-fix.
  One pre-existing `ynz-codegen` test
  (`spike_host_subset_bare_admits_effective_declines_on_imported_post_pair`) broke AGAIN as a
  direct, foreseeable consequence of this fix (the same class of consequence Phase 2's own landing
  already hit once, above) — split into two single-purpose tests documenting the NEW, STRONGER
  invariant (a correct `base_suspends` now makes `spike_host_subset` insensitive to a bare-vs-
  effective `suspend_set` split, structurally rather than by convention) plus an artificial
  isolation test proving WHY `base_suspends` correctness is the one input that still matters. Full
  Paper-Trace in `audit.md`.
- **Reviewer fan-out:** code-reviewer + adversarial deviation-judge (typeck soundness — can a
  guard-tripping crossing sneak past the extended probe?).
- **Model tag:** `(coding, high, large)`

#### Phase 3 — Fusion core: dual-kind frame + shared-continuation poll + admission flip + deadlock gate + fixture flip

- **Task + purpose:** the milestone's heart. One group frame carries both work-classes; one
  continuation re-drives both on every resume; the admission gate opens mixed groups in the same
  change; the three DECLINE fixtures flip to fire-asserting. Every non-negotiable safety gate
  (E1, E2, E7, E8) lands here and blocks this phase's merge.
- **Steps:**
  - [ ] **Dual-kind group frame layout:** reserve BOTH the CPU handle/result region (keyed
        (group, member-index), per-invocation ctx — the M3d keying, no count hardcode) AND the
        I/O child sub-frames in the same group's frame; discriminator updated so the
        count-driven runtime cleanup covers mixed frames.
  - [ ] **Route the block walker off the classified partition:** replace the
        `partition_independent_groups` + separate-`m3d_spike`-trigger seam (`emit.rs:3901-3963`)
        with `partition_groups_classified` for grouped lowering; retire the forked routing for
        fused groups.
  - [ ] **Fused poll in the shared continuation:** on EVERY resume, re-drive every live CPU
        handle via `ynz_rt_join_poll` AND inline-poll every pending I/O sub-frame; yield Pending
        (genuine async yield — NEVER a blocking join) only while members remain unfinished;
        re-polling a Ready member is idempotent (no double-bind). Bind every member result
        through the canonical binders (`bind_sm_result_and_flush` — corpse-a discipline, no
        forked loads).
  - [ ] **Flip the admission gate** — remove the nested mixed-decline (`cpu_admission.rs:102-108`)
        AND Phase 2's temporary top-level co-resident-suspension decline in this same change —
        hint==binary parity holds through the flip because both read the one query (E5). The
        Phase 2 neutrality fixtures flip with it: the top-level-group-in-suspending-host shape
        becomes fire-asserting (its CPU group spawns; the trailing `wait sleep` suspends through
        the fused continuation).
  - [ ] **E1 obligation — RED-first adversarial deadlock gate:** commit fixtures reproducing the
        three 4c deadlock shapes RED on the branch (mixed adjacent pair; nested CPU group +
        outer `wait`; CPU group + suspending callee), then green via the fusion. Wrap every
        concurrency fixture in a watchdog timeout harness so a hang fails fast instead of
        wedging CI. **Proof artifact = the failing-first fixtures in the branch history.**
  - [ ] **FLIP the three M3d DECLINE fixtures** (`integration.rs:6287` / `:6301` / `:6355`)
        from decline-asserting to fire-asserting — the acceptance signal.
  - [ ] **E2 — byte-identity oracle:** `--no-auto-parallel` output byte-identical to the Phase 1
        baseline on the whole corpus; every still-declined shape byte-identical in default mode.
  - [ ] **E8 obligation — blocking-pool exhaustion stress:** a fixture driving many concurrent
        mixed groups (and a recursion-spawning shape) must complete without deadlock; **gates
        the build.**
  - [ ] **E7 obligation — wide-EC ratchet:** a `-> number errors` (wide-value-EC) callee in a
        mixed group NEVER spawns as a CPU member (the M3d decline-around survives fusion —
        `emit.rs:6604` UAF class); decline-test asserts 0 CPU spawns for the wide-EC member +
        byte-identity. The shared CPU-supported-return-class predicate stays the single source
        in both typeck and codegen.
  - [ ] **Overlap proof:** ratio-based timing fixture — mixed group elapsed ≈ max(member), not
        sum (protocol from Phase 1).
  - [ ] **Demo & error gallery (this phase adds executable surface):** extend
        `examples/pirates-roster/entrypoint.ynz` with a mixed CPU+I/O section doing realistic
        work (a heavy crunch + a `sleep`-backed fetch overlapping); create
        `examples/primantis-orders/v0_3_m3g_errors.ynz` with intentional triggers (with
        `// WHY:` comments) for every new/changed diagnostic class this phase introduces; insta
        stdout/stderr snapshots for both.
- **Exit criteria:** ALL of — 3 flip fixtures fire-asserting and green; deadlock fixtures green
  under the watchdog; exhaustion stress green and build-gating; wide-EC ratchet green;
  byte-identity oracle green corpus-wide; overlap ratio fixture green; alloc==free on every new
  fixture; workspace + clippy green; demo + gallery extended and snapshotted. **This phase does
  not merge with any gate missing.**
- **Reviewer fan-out:** adversarial deviation-judge ×2 + code-reviewer (M2-HALT-corpse-adjacent —
  the roadmap mandates the adversarial gate). Reviewers MUST diff the diff against
  [IMP-no-function-coloring](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
  (any bridge-shaped construct = BLOCK with citation) and
  [IMP-concurrency](../../../../docs/internal/implementation/IMP-concurrency.md), not only
  against this plan.
- **Model tag:** `(coding, high, large)`

#### Phase 4 — Generality + boundary matrix: N+M shapes, cross-module, cleanup/panic, interaction sweeps

- **Task + purpose:** prove the fusion is general (the brief's scope: N CPU + M I/O in one group)
  and that every boundary it touches — module seams, cancellation/detach, panics, sibling
  machinery — holds. This is where "works on the three fixtures" becomes "works."
- **Steps:**
  - [ ] **N+M member matrix:** 2 CPU + 1 I/O, 1 CPU + 2 I/O, 3 CPU + 2 I/O; same-callee CPU
        members ×2 alongside an I/O member; order permutations (I/O-first vs CPU-first
        declaration order); an `errors`-capable I/O member + CPU member mix. Grep-gate for
        member-count hardcodes in the fused path; if the N=2 codegen guard (A8) still exists,
        lift it here with N=3 live fixtures (the runtime cleanup is already N-general).
  - [ ] **Cross-module mixed group (the named gap):** a mixed group whose I/O callee is imported
        (exercises `frame_layouts_query` reconstruction of a dual-kind frame across the module
        boundary), M3e danger-matrix style: direct + re-export chain + errors-capable variants.
        Cross-module dual-kind frame reconstruction is a **first-class byte-identity target**,
        not a smoke test: every cell in this sub-matrix carries the full oracle (still-declined
        variants byte-identical between modes; fired variants' `--no-auto-parallel` lowering
        byte-identical to the Phase 1 baseline) alongside output equality — a frame sized by one
        query boundary and laid out by the other is exactly the silent-corruption class the
        oracle exists to catch.
  - [ ] **Resource cleanup:** early-return-before-join shapes on mixed frames (detach semantics —
        tasks finish, results discarded, nothing leaks); count-driven handle free verified on
        dual-kind frames; alloc==free accounting fixtures.
  - [ ] **Panic re-raise:** a CPU member panics while an I/O member is in flight → panic
        re-raised on the driving coroutine after all members settle; first-panic-wins preserved
        per [IMP-concurrency](../../../../docs/internal/implementation/IMP-concurrency.md)
        "Panic Re-Raise".
  - [ ] **Interaction sweeps:** spike host-set reconciliation (todos over-allocation class — not
        worsened by fused routing); `background` + mixed-group shutdown-abort race (todos entry —
        pre-existing, verify the rate is unchanged, decline-around stands); **kernel-mode
        fixture:** a mixed group under `--kernel` declines to sequential (promotion is off
        entirely without a scheduler), byte-identical, fixture-asserted.
  - [ ] **Cross-impl consistency sweep** across all mixed fixtures (default vs
        `--no-auto-parallel` semantic equivalence — outputs equal, only overlap differs).
- **Exit criteria:** matrix green; still-declined shapes byte-identical; any deliberate golden-IR
  churn justified hunk-by-hunk in the PR against the Phase 1 baseline; gallery extended if the
  matrix surfaced new error classes; alloc==free everywhere.
- **Reviewer fan-out:** adversarial deviation-judge + code-reviewer (round-based, M3d Phase-3
  precedent).
- **Model tag:** `(coding, high, large)`

#### Phase 5 — Teaching surfaces, docs, registry, release

- **Task + purpose:** make every surface that *describes* the behavior match the behavior that
  now exists, then ship.
- **Steps:**
  - [ ] `parallel_groups` muted-hint truth for mixed groups: hint text reflects a mixed group's
        members and classes; hint==spawn **parity tests** extended over the mixed shapes (E5
        proof — both surfaces read the one admission query).
  - [ ] **Feature-registry entries** (see the invariants subsection below): apply the
        `parallel_groups` wording modification; enumerate any `[[diagnostic_template]]` added in
        Phases 1–4; state explicitly what was NOT added. `ynz-registry` build green; jargon
        audit green.
  - [ ] Amend [IMP-concurrency](../../../../docs/internal/implementation/IMP-concurrency.md):
        the mixed-decline divergence entries flip to "fires as designed"; remaining declines
        (loop-body, multi-group, same-callee-I/O, wide-EC members) re-stated with their homes.
        Amend in place — living design doc, not a changelog.
  - [ ] `wait_on_non_may_block` wording sweep via the Phase 1 single-sourced template, if
        promotion changed where `wait` is legal on CPU-group hosts; WHAT/WHAT-INSTEAD/WHY
        format; no banned jargon.
  - [ ] VSCode/LSP teaching parity: hover docs for the mixed-group hint; if
        `tooling/vscode-ynz/` is touched, the release attaches BOTH `.vsix` assets
        (versioned + `yinz-latest.vsix --clobber`) per project convention.
  - [ ] `/pr`, merge, then `/release` for the `v0.3.0-m{next}` tag.
- **Exit criteria:** parity tests green; registry + jargon gates green; IMP amended; release
  tagged with CHANGELOG section.
- **Reviewer fan-out:** code-reviewer (docs/registry consistency) + plan-invariants check
  (7-subsection block satisfied end-to-end).
- **Model tag:** `(coding, standard, medium)`

### 3.4 Coordinating Instructions

- **Sequencing is strict:** 1 → 2 → 3 → 4 → 5; each phase ships via `/pr`; Phase 3 merges with
  zero gates outstanding (its exit list is conjunctive).
- **CCIR — the executor STOPS and surfaces immediately on any of:**
  1. Any fixture hang (watchdog fire) — treat as E1; never "fix" a hang by making the join
     blocking or by weakening the fixture.
  2. Any `default ≠ --no-auto-parallel` divergence — treat as E2; Paper-Trace
     (observed/expected/residual/hypothesis/evidence-path) before any fix.
  3. Any design pressure toward a synchronous bridge / `block_on`-shaped construct — M2-HALT
     corpse; surface as "design doc X says A; the plan says B" per
     [plan-invariants](../../../rules/plan-invariants.md).
  4. Any of the three flip fixtures resisting a safe fire — HALT-and-surface, never a quiet
     decline (¶3.1 initiative rule 3).
- **Scope tripwires (OUT of M3g — decline safely + record, do not drag in):** multi-group ≥2 per
  function; loop-body CPU groups; same-callee-I/O sequential reversal; wide-EC CPU-ABI lift
  (the decline-around MUST survive — E7); M3c shadow parity (deferred post-v0.3.0). Each is
  already tracked (todos / roadmap Capability Ledger); a phase that starts building one has left
  its box.
- **Verify-before-complete:** every checkbox claims completion only with its named proof artifact
  (test, fixture, snapshot, mutation check) in the diff.
- **FRAGO discipline:** Phase 1's baseline numbers and any mid-execution divergence update this
  plan in place + append the delta record to `audit.md`.

## 4. Sustainment

- **Environment:** the `dev` docker-compose service (LLVM 18, Rust stable, inkwell). All builds
  and tests in-container: `docker compose run --rm dev cargo test --workspace`,
  `... cargo clippy --workspace -- -D warnings`, `... cargo fmt --all`. Cargo registry cache in
  the named volume; `target/` on the host bind mount.
- **Test tooling:** insta snapshots (golden IR + demo stdout/stderr); the integration fixture
  corpus in `crates/ynz-driver/tests/integration.rs`; timing fixtures need the ratio-assert
  protocol (Phase 1) and generous budgets under `--workspace` contention.
- **Kill switch:** `--no-auto-parallel` / `YNZ_NO_AUTO_PARALLEL` — both the E1/E2 severity
  mitigation and the bisect tool; it must keep working untouched.
- **Registry:** `registry/features.toml` edits regenerate typed constants via `ynz-registry`
  `build.rs` — build the workspace after any registry change.

## 5. Command & Signal

- **Ownership:** each phase is one dispatched executor session (model per the phase's Model tag);
  reviewer gates per the phase's fan-out; Patrick owns the approval gate (stub→active flip), the
  Design-Doc Alignment sign-off, and the release call.
- **Succession:** a fresh session resumes from plan-id `2026-07-01-v0-3-m3g-mixed-cpu-io-overlap`
  + the session-id chain + the phase checkbox state. Read
  [`.claude/state.md`](../../../state.md) and the roadmap's M3g section first.
- **Audit trail:** [audit.md](./audit.md) (append-only session log + FRAGO log), same directory.

## Invariants This Milestone Must Preserve

### Safety

- `default == --no-auto-parallel` program **output** is semantically identical on every fixture
  (only overlap/timing may differ); for every shape M3g does not fire, the emitted IR is
  **byte-identical** between modes.
- The M3d safe-DECLINE floor never regresses: loop-body groups, multi-group-≥2 functions,
  wide-EC-member groups, guard-tripping crossings, and kernel-mode builds all decline to
  sequential, locked by decline-tests.
- No fused shape can deadlock: every resume of the shared continuation re-drives every live CPU
  handle and every pending I/O sub-frame; asserted by the RED-first adversarial gate + watchdog
  harness + the exhaustion-stress fixture.
- No synchronous join exists anywhere in the fused path (M2-HALT corpse) — the continuation
  yields Pending to the scheduler, never blocks a thread on a handle.
- No wide-value-EC callee is ever admitted as a CPU member (UAF class) — the shared
  return-class predicate declines it in typeck AND codegen, ratchet-tested.
- Frame-header offsets have exactly one home (`ynz-abi`); drift between codegen and runtime is a
  build error, not a prose comment.
- A `.give`/ownership-model regression is out of this milestone's blast radius (no ownership
  rules change); the type-based write-effect floor transfers unchanged — mutable-heap-argument
  members still never parallelize.

### Performance

- A mixed group's elapsed time ≈ max(member costs), not sum — ratio-asserted timing fixture.
- Pure-I/O groups (M3b path) and pure-CPU groups (M3d path) keep their shipped codegen —
  golden-IR gate against the Phase 1 baseline; any churn is deliberate and justified per hunk.
- The fused poll adds O(members) work per resume and no per-resume heap allocation; frame growth
  is one dual-kind region per group (CPU slots + I/O sub-frames), sized at compile time.
- **Auto-promotion analysis** (mandatory per the [auto-promotion rule](../../../rules/auto-promotion.md)):
  - M3g **is itself an auto-promotion** — the compiler proves a mixed group independent and
    picks the concurrent form automatically. **Surfaces:** codegen auto-promotion YES;
    muted hint YES via the existing `parallel_groups` domain (Informational category — there is
    no typeable Yinz syntax for "schedule these in parallel", so no click-to-make-explicit and
    **no new Tier 3 lint**; consistent with M3b/M3d's treatment of the same domain). No new
    lint rule name needed; the hint's inline text and hover follow the domain's existing
    WHAT/WHAT-INSTEAD/WHY wording, extended for mixed classes (Phase 5).
  - **Override directions:** force-sequential exists via existing syntax — a per-site explicit
    `wait` orders the pair, and `--no-auto-parallel` is the program-wide off switch (no new API;
    documented, satisfies the existing-syntax check). Force-parallel is deliberately omitted:
    the compiler fires exactly when independence + admission are proven; a user override
    cannot manufacture safety the analysis lacks (same omission rationale M3b/M3d recorded).
  - No other new feature in this milestone introduces a stricter/faster promotable form — the
    typeck promotion of `base_suspends` hosts is an enabler of this same promotion, not a
    separate candidate. Stated so reviewers know it was considered.

### Teaching

- The `parallel_groups` muted hint and the emitted binary NEVER disagree (both read
  `admitted_cpu_group` / the shared admission query) — parity tests extended over mixed shapes.
- Any new or reworded diagnostic (`wait_on_non_may_block` sweep, any gallery-triggered class)
  follows WHAT/WHAT-INSTEAD/WHY with a call-site-specific WHY, and passes the banned-jargon
  audit (no `thread`, `async`, `spawn`-jargon leaks into user-facing text beyond the approved
  vocabulary).
- Hint placement categories are not mixed: `parallel_groups` stays in its locked category; no
  new hint surface is invented.
- The diagnostic text is single-sourced (Phase 1) so a wording change cannot half-apply.

### Runtime Dependencies

- The fused path depends on: the Tokio runtime (scheduler for the resumed continuation), the
  **finite blocking pool** (`ynz_rt_spawn_blocking_joinable`), the joinable-handle ABI
  (`ynz_rt_join_poll` / `ynz_rt_join_handle_free`), and malloc (group frames, handle slots).
- Detach semantics are a dependency, not an option: spawned CPU tasks are non-cancellable once
  dispatched; early exit discards results (never leaks).
- No NEW runtime primitive is added — M3g composes the two shipped ABIs.

### Kernel-Mode Behavior

- `--kernel` has no scheduler and no blocking pool → auto-parallel promotion is **off entirely**
  (existing behavior); a mixed group under `--kernel` compiles to plain sequential lowering,
  byte-identical to `--no-auto-parallel`, asserted by a Phase 4 fixture.
- No new compile-error class is needed in kernel mode (sequential lowering is always available);
  if a fixture proves otherwise mid-execution, the error follows WHAT/WHAT-INSTEAD/WHY pointing
  at [IMP-no-runtime-mode](../../../../docs/internal/implementation/IMP-no-runtime-mode.md), and
  this subsection gets FRAGO'd.

### Demo & Error Gallery

- `examples/pirates-roster/entrypoint.ynz` gains a mixed CPU+I/O section showing a heavy compute
  call overlapping a suspending call in realistic roster context (not `print(feature())`) —
  Phase 3, with insta stdout snapshot.
- `examples/primantis-orders/v0_3_m3g_errors.ynz` is **created** (does not exist yet) with
  intentional triggers + `// WHY:` comments for every new/changed compile-error class Phases 1–4
  introduce; if the milestone genuinely adds zero new error classes, the file is created with
  the mixed-group teaching triggers that DO exist (e.g. guard-probe declines surfaced as hints)
  and a header note — stated explicitly rather than silently skipped.
- Both files get insta snapshots in the owning phase's verification step.

### Feature Registry Entries

- **Modify** `[[muted_hint_domain]]` `parallel_groups` — wording extended to cover mixed
  (CPU + I/O) group members truthfully; placement category unchanged.
- **Possible** new `[[diagnostic_template]]` entries: only if the `wait_on_non_may_block`
  single-sourcing (Phase 1) or the gallery work (Phases 3–4) promotes hand-written text into a
  canonical template — enumerated in the Phase 5 checklist when known; the plan adds **no**
  speculative templates.
- **Explicitly none:** no new `[[keyword]]`, `[[banned_declaration_keyword]]`,
  `[[banned_jargon]]`, `[[primitive_intrinsic]]`, `[[type_attached_constant]]`,
  `[[deferred_language_feature]]`, or `[[deferred_tooling_feature]]` entries — M3g adds zero
  user-typeable surface. Stated so reviewers know it was considered, not forgotten.

## Design-Doc Alignment

**Governing docs read for this plan:**
[IMP-concurrency](../../../../docs/internal/implementation/IMP-concurrency.md) and
[IMP-no-function-coloring](../../../../docs/internal/implementation/IMP-no-function-coloring.md).

1. **The plan's model matches the design.** IMP-concurrency's locked Model A ("independent
   operations run concurrently — this is the default and the maximal-performance choice", with
   **no member-count or work-class cap**) is exactly what M3g implements for mixed classes —
   this milestone *closes a gap against* the design, it does not diverge from it. The typeck
   front half (`independence.rs:199-201`) was authored stating both classes "share one
   continuation"; codegen finally consumes it.
2. **The no-coloring model is the hard boundary.** IMP-no-function-coloring mandates
   whole-program may-block analysis + genuine suspension — no sync bridge anywhere. The fused
   continuation is an async poll-yield; **any fusion approach that reintroduces a
   `block_on`-shaped bridge is a design violation and a reviewer BLOCK with citation** (the
   v0.3-M2 HALT is the incident; reviewers must diff the diff against the doc, not the plan
   against itself — [plan-invariants](../../../rules/plan-invariants.md) Design-Doc Alignment
   obligation).
3. **Inherited divergences (pre-existing, documented in IMP-concurrency's Design Divergences —
   M3g preserves, does not widen):** (a) same-callee *suspending* members still run sequentially
   (I/O sub-frames keyed by callee name; reversal path documented at IMP-concurrency's
   divergence entry — OUT of M3g); (b) the type-based write-effect conservative floor transfers
   unchanged (mutable-heap-argument members never parallelize — Golden Rule 5 over Rule 10);
   (c) loop-body and multi-group-≥2 declines stand (todos-tracked future slices); (d) wide-EC
   members stay declined (UAF class, E7). Each is already documented with cost + reversal path;
   none is new divergence introduced by this plan.
4. **Milestone-boundary assumptions:** M3g depends on M3b + M3d being complete (both shipped —
   roadmap Capability Ledger). The out-of-scope list above matches the roadmap's M3d "explicitly
   NOT" list and the Capability Ledger ownership rows — no capability owned elsewhere is
   delivered here, and no capability the design marks load-bearing for THIS milestone is
   deferred (the 4c lesson: the fusion is the milestone, whole, in one plan).
5. **SCOPE ruling recorded for Patrick's sign-off:** the directive "whatever the long-term
   correct answer is per no-duct-tape + the IMP docs" resolves to **general fusion WITHIN one
   group** (N CPU + M I/O sharing one continuation) — Model A has no count cap, the classified
   partition was authored for mixed N-member groups, and a 1+1 hardcode would be the
   `CPU_GROUP_MEMBER_COUNT == 2` corpse again. Multi-group-≥2, loop-body, same-callee-I/O
   reversal, M3c shadow parity, and the wide-EC ABI lift stay out (YAGNI ceiling; each has its
   own documented slice).

## Future Requirements / Revisit

Risk-engine outputs (recorded MEDIUMs) and scope deferrals, each with its trigger — durable, not
session-todos:

| What | Why deferred / residual | Cost later | Trigger |
|---|---|---|---|
| **E1 residual (M):** deadlock class survives at MEDIUM even with the gate — new fused shapes can reintroduce it | mitigations cap at B2 (tests + kill-switch; can't eliminate a concurrency class by construction) | a hang escaping to a user build; bisect via kill-switch | any new group-lowering feature (M4 channels, loop-body slice) re-runs the deadlock gate |
| **E2 residual (M):** silent-miscompile class on the decline floor | same B2 cap; byte-identity is a detector, not an eliminator | corpus-wide re-baseline | every future auto-parallel phase keeps the oracle as a standing exit criterion |
| **E4 residual (M):** ABI drift beyond the header offsets (future frame fields) | Phase 1 hardens the known set; future fields must join the `ynz-abi` home | one drift = heap corruption on drop | the source-scan gate + reviewer check whenever `FrameLayout`/header changes |
| **E6 residual (M):** guard-probe completeness on suspending hosts | adversarial fixtures cover known tripping shapes; the shape space is open | a UAF-class admit | any new crossing-slot kind or guard rule re-runs the Phase 2 adversarial set |
| **E7 residual (M):** wide-EC members stay sequential (perf miss, not a bug) | UAF root fix (heap-stabilize the ok-word / wider CPU-result ABI) is its own slice, todos-tracked | 1–2 sessions, shared with `ec-wrapper-collect-on-completion` lift | when wide-EC CPU parallelism is wanted — do both together |
| **E8 residual (M):** pool exhaustion under extreme fan-out | stress fixture bounds it; the pool is finite by substrate | tuning/backpressure work | user reports of stalls under heavy mixed fan-out; M4 channels work touching the pool |
| Multi-group ≥2 per function fires | per-group slot reservation is its own slice (todos multi-group entry) | per-group slot keying + fired-set | when multi-group CPU/mixed parallelism is wanted |
| Loop-body groups fire | loop-carried frame-backing + multi-level synthetic indices (todos loop-body entry) | dedicated loop-placement-matrix slice | when loop-body parallelism is wanted |
| Same-callee suspending members overlap | I/O sub-frame keying by (callee, invocation-index) (IMP-concurrency divergence entry) | `build_frame_layouts` extension | when same-callee I/O fan-out matters to a real workload |
| M3c shadow parity | deferred post-v0.3.0 (roadmap) | per its own milestone | roadmap sequencing after v0.3.0 |
| `background` + CPU-group shutdown-abort panic noise | pre-existing, benign, timing-dependent (todos shutdown-race entry) | graceful drain on `ynz_rt_shutdown` | background-CPU hardening slice |
| `YNZ_NO_AUTO_PARALLEL` as a salsa input (not env var) | pre-existing deferral (IMP-concurrency) | thread the flag as an explicit input | when `ynz watch`/LSP need flag-aware codegen |
