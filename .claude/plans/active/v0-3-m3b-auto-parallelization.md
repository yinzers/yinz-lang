---
slug: v0-3-m3b-auto-parallelization
type: execution
owner: Patrick Rizzardi
status: active
roadmap: v0-3-concurrency-perf
plan_base: 0a4b6d8390b1cffd462681429d159ce8db25198a
files:
  - crates/ynz-typeck/src/may_block.rs
  - crates/ynz-typeck/src/queries.rs
  - crates/ynz-typeck/src/signatures.rs
  - crates/ynz-typeck/src/check.rs
  - crates/ynz-typeck/src/inlay_hint_passes.rs
  - crates/ynz-codegen/src/emit.rs
  - crates/ynz-codegen/src/queries.rs
  - crates/ynz-driver/src/main.rs
  - crates/ynz-lsp/src/inlay_hint.rs
  - registry/features.toml
  - design/concurrency.md
  - spec/concurrency.md
  - examples/pirates-roster/entrypoint.ynz
  - examples/primantis-orders/v0_3_m3b_errors.ynz
  - crates/ynz-driver/tests/fixtures/**
created: 2026-06-05
last_updated: 2026-06-05
---

# Plan: v0.3-M3b — Cross-Module May-Block Propagation + Auto-Parallelization

Created: 2026-06-05
Status: active

---

## Context & Why

**Goal**: Deliver the v0.3 headline — *existing Yinz code that uses `wait`/`background` actually runs concurrently instead of sequentially*, and independent operations in a function body auto-parallelize with zero user action. This is the second (analysis) half of the old single "M3", built on M3a's completed suspension-codegen substrate (shipped as `v0.3.0-m4`).

**Why**: Per `design/future/concurrency.md` ("No Function Coloring") and `design/concurrency.md` (auto-parallelization via dependency graph), the compiler is supposed to do whole-program may-block analysis and parallelize provably-independent work automatically. M1 shipped the runtime + `background`; M2 shipped the intra-unit may-block engine + state machines; M3a completed the codegen so `wait` works in every position. What remains: (a) the analysis can't cross module boundaries yet (it's a clean compile error today), (b) `background` still demands explicit `.give`/`.copy`, (c) the IDE teaching surfaces (`wait_points`, `background_routing`) don't fire, (d) `-> T errors` results from spawned tasks can't be collected, and (e) **the dependency-graph auto-parallelize pass doesn't exist** — independent operations still run sequentially.

**Background — what the codebase ALREADY does** (verified by source 2026-06-05; corrects several stale roadmap assumptions):
- **`wait` is already optional / suspension is already auto-inferred.** `wait_required_on_state_machine_call` was retired in M2 (`crates/ynz-typeck/src/check.rs:2429-2432`). Every suspending call is inline poll-yield whether or not the user writes `wait`. There is **nothing to build** for "auto-insert wait for suspension correctness" — it's done. M3b's `wait` work is purely the *teaching surface* (`wait_points` hint) plus honoring `wait` as an *ordering barrier* in the auto-parallel pass.
- **Background CPU/IO routing already exists.** `background` routes state-machine callees → `ynz_rt_spawn` (I/O pool) and non-SM callees → `ynz_rt_spawn_blocking` (CPU pool), keyed on `suspend_set` (`crates/ynz-codegen/src/emit.rs:8884`). M3b adds the *IDE hint* surfacing it, not the routing itself.
- **`ynz_rt_spawn` (I/O pool) already shipped** in M2 (`crates/ynz-runtime/src/runtime.rs:525`).
- **M8 cross-file typeck already shipped.** `check_query` resolves cross-file imports via `module_signatures_query` → `resolve_imports`. The only cross-module gap is `suspends`-flag propagation.

**Constraints**: No new user-facing syntax. Existing programs produce byte-identical output (only scheduling/perf changes). No GC. Tokio internal. Full teaching surface ships in the same milestone (roadmap "Full teaching surface" constraint). Only may-block source in v0.3 is `sleep` (no stdlib I/O until v0.5+) — so auto-parallelization is demonstrated with `sleep` as the I/O proxy, exactly as M2/M3a validated suspension. The same pass extends unchanged to real I/O when it ships.

**Success criteria**:
1. A suspending function in module A, called from module B, compiles + runs correctly (the `check.rs:2383` cross-module stopgap is gone).
2. `background foo(x)` infers `.give`/`.copy` from use-after-spawn; no explicit annotation required (safety rejections for `.share`/`.lend` stay).
3. Two independent suspending statements in one function body run concurrently — total wall-clock ≈ max, not sum — with byte-identical stdout to `--no-auto-parallel`.
4. `wait foo()` forces ordering: a `wait`-marked statement completes before the next statement begins, even when the compiler would otherwise parallelize.
5. The IDE fires `wait_points` (muted `wait` at suspension points) and `background_routing` (I/O-pool vs CPU-pool) hints with WHAT/WHAT-INSTEAD/WHY hovers.
6. `-> T errors` results from auto-parallelized/spawned tasks are collected correctly (the `ec-wrapper-collect-on-completion` deferral is lifted).
7. `--no-auto-parallel` produces byte-identical stdout/stderr/exit-code to default-parallel mode on every `examples/` and `crates/ynz-codegen/tests/` fixture (the cross-implementation consistency gate).

---

## LOCKED Semantics — `wait` and Auto-Parallelization (Patrick, 2026-06-05)

This was the milestone's #1 open question. **Resolved in plan discussion, before any codegen phase** — recorded here verbatim so the executor and reviewers treat it as binding.

**Model A is locked** (matches `design/concurrency.md` as written; the superseded roadmap "wait-is-non-ordering" decision stays dead):

1. **Suspension is always auto-inferred** (no function coloring). `wait` is NEVER required for suspension correctness — that shipped in M2.
2. **Independent operations auto-parallelize** — reads AND writes to *different* resources both. "Independent" = no data dependency (neither uses the other's result) AND no shared-mutable-state conflict (no two statements `lend` the same binding). This is the maximal-performance default; the user accepts that two independent side effects may overlap/race unless they order them.
3. **Ordering the compiler infers for free**: data dependency (B uses A's result → B waits for A) and same-resource ownership (two `lend` on the same binding → sequenced). No `wait` needed for these.
4. **`wait foo()` = ordering barrier.** It forces `foo` to complete before the next statement begins, and joins any prior in-flight auto-parallel work. It is the user's tool to assert a happens-before the compiler cannot infer (a causal order across otherwise-independent operations — e.g. "charge, *then* email"). The observable difference between writing `wait foo()` and not: with `wait`, ordering is guaranteed; without, independent ops may overlap.
5. **Loops stay sequential** (locked, `design/concurrency.md` "Loop Iterations — Sequential by Default"). Auto-parallel fires only across straight-line independent statements, never across loop iterations.

**Why Model A over Model B (source-order writes)**: Model B leaves real throughput on the floor for write-heavy intensive work (independent writes that genuinely could overlap wouldn't), and the read-parallelism headline survives either way. Patrick chose maximal performance with the ordering responsibility on the user via `wait`. Documented decision; the cost (independent side effects can race by default — the user must `wait` to order) is accepted and surfaced through the teaching layer.

**Scope (M3b = I/O-overlap auto-parallel only; CPU-parallel is its OWN milestone — Patrick-decided 2026-06-05 after runtime verification)**: M3b auto-parallelizes independent groups of **suspending** statements (I/O-overlap, demonstrated with `sleep`). The mechanism is **interleaved inline polling, NOT spawn-and-join**: both callees' state-machine sub-frames embed in the composed frame, and their inline polls are interleaved (poll A → it parks on its waker; poll B → parks; yield `Poll::Pending` to the driver; on resume re-poll whichever was woken) until all are ready. The enclosing function is ALREADY a state machine (it transitively reaches `sleep`), so this reuses the existing inline poll-yield mechanism generalized to N sub-frames — **ZERO new runtime symbols, no thread spawn** (overlap comes from interleaving suspensions on one thread, exactly how two independent I/O waits overlap).

**Pure-CPU statement parallelization is its OWN milestone (`v0-3-m3d-cpu-parallelization`, mapped in the roadmap)** — NOT in M3b. Verified reason (the round-3 catch): CPU work doesn't yield, so overlapping it needs ACTUAL extra threads, which the existing runtime can't join — `ynz_rt_spawn`/`ynz_rt_spawn_blocking` both return void/fire-and-forget (no joinable handle), and a pure-CPU function isn't a state machine (can't await a join). M3d adds the genuinely-new machinery: a joinable `spawn_blocking` + pollable join-handle (new C-ABI), state-machine promotion of non-suspending functions, and a deadlock-safe async join (a synchronous join is the `block_on` M2-HALT corpse). **The independence analysis (data-dep + transitive write-effect) is built HERE in M3b and REUSED by M3d — zero double-build; M3d adds CPU candidacy + the runtime/codegen, not a rebuild.** Pure-CPU independent statements still run (sequentially) and produce correct results in M3b.

---

## Research Findings (verified anchors, 2026-06-05)

| Area | Anchor | State today | M3b action |
|---|---|---|---|
| May-block fixpoint | `crates/ynz-typeck/src/may_block.rs:87` (`analyze`) | Intra-unit only; seed `M2_MAY_BLOCK_INTRINSICS` = `["sleep","__testFallibleAsync"]` (`intrinsics.rs:24`); plain fn called inside `check_query` | Seed the fixpoint from imported-suspending callees too (P1) |
| `FunctionSig.suspends` | `crates/ynz-typeck/src/signatures.rs:13-39` | Transitive bool; set per-unit | Preserve cross-module value (P1) |
| **Suspends overwrite bug** | `crates/ynz-typeck/src/queries.rs:171-174` | `sig.suspends = may_block_result.suspends.contains(...)` **overwrites imported fns' suspends with local-only result** | Merge, don't overwrite (P1) |
| **Cross-module stopgap** | `crates/ynz-typeck/src/check.rs:2383-2400` | Errors when `callee_is_cross_module && current_fn_suspends`; WHY text literally says "ships in v0.3-M3b" | Lift it (P1) |
| `module_signatures_query` | `crates/ynz-typeck/src/queries.rs:59-118` | salsa-tracked (lru=128); flows imported sigs via `resolve_imports` | Ride it for cross-module suspends (P1) |
| Background ownership check | `crates/ynz-typeck/src/check.rs:2088-2113` | REQUIRES explicit `.give`/`.copy`; rejects `.share`/`.lend` | Add give/copy inference; keep safety rejects (P2) |
| Large-copy warning | `crates/ynz-typeck/src/check.rs:2116-2139` | Exists | Keep; reuse for inference hint (P2) |
| Background routing | `crates/ynz-codegen/src/emit.rs:8884` | SM → `ynz_rt_spawn` (I/O), non-SM → `ynz_rt_spawn_blocking` (CPU) | Surface as `background_routing` hint (P3) |
| `wait_points` domain | `registry/features.toml:2067-2072` | Declared (Addition, active_since v0.3-M2); inlay pass returns empty | Wire the pass to fire (P3) |
| `background_routing` domain | — | Does NOT exist | Add registry entry + inlay pass (P3) |
| `[[lint_rule]]` infra | — | Does NOT exist (M4 builds it) | Out of scope (M4) |
| `ec-wrapper-collect-on-completion` | registry `[[deferred_*]]` | "ships v0.3-M3b" | Lift it (P5) |
| Inlay-hint passes | `crates/ynz-typeck/src/inlay_hint_passes.rs` | 5 `#[salsa::tracked]` passes firing | Add `wait_points` + `background_routing` passes (P3) |
| LSP aggregation | `crates/ynz-lsp/src/inlay_hint.rs:154-300` | Aggregates 5 domains; hover via `ynz_registry::lsp_inlay_hint_hover_for` | Add 2 domain loops (P3) |
| SM codegen entry | `crates/ynz-codegen/src/emit.rs:1640` → `lower_function_with_waits` (`emit.rs:1845`) | Keyed on `suspend_set` | Auto-parallel pass plugs in here (P4) |
| Background SM spawn | `crates/ynz-codegen/src/emit.rs:9065` (`lower_expr_background_state_machine` → `ynz_rt_spawn`) | Exists | Reference for sub-frame embedding only; auto-parallel uses interleaved inline poll (no spawn) — P4 |
| `--no-auto-parallel` | `crates/ynz-driver/src/main.rs:74-81` defined, `:210` discarded (no-op) | Plumbed, no-op | Make it gate the auto-parallel pass (P4) |
| Cross-impl consistency | `crates/ynz-driver/tests/integration.rs:4039-4134` | 2 byte-identical tests, trivially passing | Extend to assert real parallel==sequential (P4); full sweep (P6) |
| Concurrency proof model | `crates/ynz-driver/tests/fixtures/v0_3_m2_concurrent_waits_proof.ynz` | 8 `background` tasks, all START before any DONE | Model for the auto-parallel proof fixture (P4) |

---

## Design-Doc Alignment

**Governing design docs** (read in full during planning; kept open during execution per project CLAUDE.md):

1. **`design/future/concurrency.md`** ("No Function Coloring", Locked) — the end-state. M3b is a faithful build toward it:
   - "the compiler does whole-program may-block analysis from the call graph, auto-inserts `wait` at suspension points" → M3b extends the analysis cross-module (P1). **CONFIRMS.**
   - "background spawns a new task… cross-thread shared state crosses via auto-inferred Arc" → M3b ships give/copy inference (P2); auto-Arc is M4 (channels), not M3b. The roadmap documents this split. **CONFIRMS** (no auto-Arc in M3b).
   - "the analysis is precise; only call chains that actually reach a suspension point get suspension code" → M3b's auto-parallel only overlaps suspending statements (interleaved inline poll; pure-CPU parallelization is the separate M3d milestone). A pure-CPU chain gets NO concurrency codegen in M3b — exactly matching "only suspension-reaching chains get suspension code." **CONFIRMS.**
2. **`design/concurrency.md`** — the auto-parallelization design. The **LOCKED Semantics** section above resolves the one ambiguity (explicit-`wait` meaning) in favor of `design/concurrency.md`'s Model A. P0 SHARPENS the doc (makes "suspension is auto / `wait` is the ordering tool" explicit; confirms the superseded roadmap "wait-is-non-ordering" text stays dead) — this is a clarification, NOT a contradiction. The doc's substance is correct as written. **CONFIRMS after P0 sharpening.**
   - "Loop Iterations — Sequential by Default" → P4 auto-parallel never crosses loop iterations. **CONFIRMS** (P4 fixtures assert sequential per-iteration ordering).
   - "Reads vs Writes — Ownership Does Double Duty" → P4 independence analysis uses `share`(read)/`lend`(write) to classify conflicts. **CONFIRMS.**
3. **`design/ide-hints.md`** + `registry/features.toml` `[[muted_hint_domain]]` — `wait_points` (Addition) + new `background_routing` (Informational) follow the three-placement-category protocol and WHAT/WHAT-INSTEAD/WHY hover format. **CONFIRMS.**

**Milestone-boundary assumptions** (all documented in the roadmap, not invented here):
- M3b does NOT do channels, auto-Arc, auto-SoA, false-sharing padding, the `[[lint_rule]]` infra, or `prefer-yielding-sleep` — all M4. Documented in the roadmap M3b/M4 sections.
- M3b depends on M3a's completed substrate (shipped `v0.3.0-m4`, in `done/`). M8 cross-file typeck already shipped (corrected stale assumption #1).

**No unresolved contradiction or gap.** The one ambiguity (explicit-`wait` semantics) is resolved in **LOCKED Semantics** above and codified in P0.

**Design-source registry note**: `.claude/design-sources.md` does not exist yet; the design-compliance gate currently runs in loud-fallback mode. **P0 creates it (Patrick-decided 2026-06-05 — REQUIRED, not optional)**, registering `design/concurrency.md`, `design/future/concurrency.md`, `design/ide-hints.md` as `[locked]`. This takes the gate out of loud-fallback for this milestone and all future ones — directly de-risking the design-drift failure mode that caused the M2 HALT.

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **Auto-parallel pass silently reorders side effects → wrong output** | Med | **Critical** | The cross-impl consistency gate: `--no-auto-parallel` (ships from M1) must produce byte-identical stdout/stderr/exit-code to default on EVERY fixture. Any divergence = bug = BLOCK. `--no-auto-parallel` is also the kill switch / rollback. Adversarial-tester agent run on P4. Per-phase + cumulative reviewers. |
| Independence analysis over-approximates → parallelizes dependent statements | Med | High | Conservative-correct: when independence can't be PROVEN (data-dep + ownership), do NOT parallelize (run sequential). False "not independent" only costs perf, never correctness. Property: anything that compiles produces identical output in both modes. |
| Cross-module suspends propagation breaks salsa incrementality (stale `suspends` after a dependency edits) | Med | High | P1 includes an incremental-correctness test: module A fn goes non-susp→susp, assert module B re-checks and its caller becomes a state machine. Ride the already-salsa-tracked `module_signatures_query`. |
| `ec-wrapper-collect` copy-before-free gets the lifetime wrong → UAF on collected `-> T errors` | Low | High | Reuse the proven M3a frame-drop discipline; alloc=1/free=1 audit fixture (the M2/M3a model). Adversarial-tester on the EC-collect path. |
| Auto-parallel codegen is larger/harder than estimated; P4 spans multiple sessions | High | Med | P4 reuses existing machinery — the inline poll-yield resume generalized to interleave N embedded sub-frames (NO `ynz_rt_spawn`, no new runtime). It builds the independence analysis (shared with the later M3d CPU milestone). If P4 exceeds budget, the consistency gate lets a partial pass ship safely (no-op until proven). |
| give/copy inference picks `.give` on a value used later → use-after-move | Low | High | Inference rule: value used after spawn → `.copy`; only unused → `.give`. The existing post-spawn use analysis + use-after-give typeck (M4) catches violations. Test both directions. |
| `wait`-as-barrier interacts wrongly with auto-parallel (a `wait` fails to join prior in-flight work) | Med | High | P4 fixture: independent group followed by `wait X` — assert X observes all prior work complete. Ordered-output assertions. |
| New diagnostics use banned jargon | Low | Low | `crates/ynz-diagnostics/tests/jargon_audit.rs` stays green (CI gate). |

---

## Questions

_(The #1 question — explicit-`wait` semantics — was RESOLVED in plan discussion; see **LOCKED Semantics**. Approval decisions:)_

1. **Design-source registry bootstrap** — ✅ **DECIDED (Patrick, 2026-06-05): YES.** P0 creates `.claude/design-sources.md` registering `design/concurrency.md`, `design/future/concurrency.md`, `design/ide-hints.md` as `[locked]` (now a REQUIRED P0 step, not optional). De-risks the M2-HALT design-drift class.
2. **`background_routing` hint visibility** — ✅ **DECIDED (Patrick, 2026-06-05): always-on** (Informational muted comment; per-domain LSP toggle still lets a user hide it).
3. **Pure-CPU statement parallelization** — ✅ **DECIDED (Patrick, 2026-06-05): its OWN milestone `v0-3-m3d-cpu-parallelization`, NOT M3b.** Runtime verification (round-3 review) showed CPU-parallel is not the cheap filter-flip first estimated: it needs a joinable spawn primitive + pollable join-handle (today's spawn ABI returns void), state-machine promotion of non-suspending functions, and a deadlock-safe async join (sync join = the `block_on` M2-HALT corpse). That is real new-runtime + corpse-adjacent work that deserves its own focused plan. M3b ships the I/O-overlap headline (interleaved inline poll, zero new runtime); M3b's independence analysis is reused by M3d (no double-build). Mapped in the roadmap (new milestone + M4 calibration note redirected to M3d).

---

## Risk Assessment & Rollout Strategy

**Risk level: HIGH** (codegen that changes execution scheduling — the silent-miscompile class).

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | Compiler project |
| Touches auth/permissions | No | — |
| Raw SQL / literals | No | — |
| Modifies existing behavior of existing programs | **Yes** | Auto-parallel changes scheduling of EXISTING `wait`/`background` code. Output must stay byte-identical. |
| Third-party integration | No | Tokio is internal, already shipped |
| Silent-wrong-output class | **Yes** | Auto-parallel reordering = the critical risk |

**Mitigations applied** (lower HIGH → manageable):
- **`--no-auto-parallel` kill switch / consistency gate** (ships from M1): every fixture must produce byte-identical output in both modes. This is both the correctness oracle AND the instant rollback (disable the pass without a code change). HIGH → MEDIUM.
- **Conservative-correct independence analysis**: unprovable independence ⇒ sequential. Can't-prove never parallelizes. MEDIUM → LOW for the reordering class.
- **Adversarial-tester** on P4 (the auto-parallel pass) + per-phase 5-reviewer gate + cumulative opus sweep.
- **Loud-reject discipline** (M3a north star): any auto-parallel case the pass can't handle correctly fails LOUD (clean compile error / falls back to sequential), never silent-wrong.

**Rollout plan** (compiler — "rollout" = the gated landing sequence, not a traffic ramp):
1. Land P1–P3 (cross-module, give/copy, teaching) — independently valuable, low silent-wrong risk.
2. Land P4 (auto-parallel) behind the consistency gate; default-on only after the gate is green on all fixtures.
3. P6 cumulative opus sweep + `/release`. `--no-auto-parallel` remains the permanent escape hatch.

---

## Design Divergences

| Doc | What it says | What we do instead | Approved rationale (named cost + reversal path) |
|-----|-------------|-------------------|------------------------------------------------|
| — | — | — | _(empty — no divergences. The one ambiguity, explicit-`wait` semantics, is RESOLVED in favor of design/concurrency.md Model A and codified in P0; not a divergence.)_ |

---

## Documentation Deliverables

| Deliverable | Phase | Notes |
|---|---|---|
| `design/concurrency.md` sharpened: authoritative "Suspension vs. Ordering (LOCKED 2026-06-05)" section + tightened `wait` section | **DONE 2026-06-05** | Landed pre-execution (decision-capture). P0 verifies. |
| `design/future/concurrency.md` "Suspension ≠ the `wait` keyword" clarification | **DONE 2026-06-05** | Landed pre-execution. P0 verifies. |
| Roadmap `wait`-in-expression dead decision tombstoned + pointed at locked model | **DONE 2026-06-05** | Landed pre-execution. |
| `spec/concurrency.md` (user-facing): confirm Model A consistency (already correct); add suspension-is-auto + `background` give/copy auto notes | P0 | Mostly verify; minor additions |
| `registry/features.toml` hover text for `wait` + `background` updated (behavior is now real concurrency, not sequential) | P0 | Per-phase doc-AC in P0 |
| `.claude/design-sources.md` bootstrap (optional, Q1) | P0 | Registers concurrency docs `[locked]` |

_(Per-phase doc-ACs live in each phase's Acceptance criteria. The roadmap "Full teaching surface" constraint items — registry entries, hover, demo, VSCode bump, screenshots — are distributed across the phases below.)_

---

## Phase Execution Protocol

Each phase ends with an **Exit Sequence** block listing the actions to execute (persist plan state → persist deviation scratch → fan out all reviewers + N deviation-judges in parallel → coordinator writes Evidence + Phase Review Gates → handle verdicts → prompt commit). Run those at every phase boundary. Canonical fan-out spec: `~/.claude/commands/execute-plan.md` Step 3.d–3.h (this file references it to prevent drift).

Per Patrick's standing preference (`all-phases-then-review`): run the 5-reviewer gate after each phase, but skip the "start next phase?" gate — proceed through all phases; Patrick reviews the full milestone at the end. Still surface each reviewer verdict per phase.

**Final phase additionally**: verify all phases' ACs/quality-gates accurate; fan out the full reviewer-and-judge set with `model: "opus"` against the cumulative plan diff (`git diff <plan_base>` if any uncommitted change, else `git diff <plan_base>..HEAD`; `plan_base = 0a4b6d8390b1cffd462681429d159ce8db25198a`); flip `status: active → done` only after all PASS.

---

## ⚙️ Execution Resume Note (coordinator, 2026-06-07) — BASE re-resolution after M3e interposition

**Context**: Phases 0 (`e4dd97c`) + 1 (`9ed31b0`) committed 2026-06-05. The milestone `v0-3-m3e-cross-module-frame-serialization` then landed **on this same branch** between P1 and now (commits `6344ce2`→`ab992be`, close-out `d7ea993` = current HEAD), delivering the full cross-module FrameLayout serialization that **lifts the universal-reject floor P1 installed**. M3e was its own fully-gated plan (now in `done/`).

**Empirically verified preconditions for resuming Phases 2–6 (2026-06-07, this session)**:
- Floor lifted: `v0_3_m3b_cross_module_suspending_caller` → `slow done / caller done` (0); `…int_return` → `42` (0); `…errors_capable` → `got: 42` (0); `…crossing_local_cross_module` → `before:10 / fetched / after:10` (0). All RUN, none reject.
- Build green (`cargo build --workspace` exit 0).
- Phase-4 corpse producers intact post-M3e: `crossing_local_names` (check.rs:6208), `locals_crossing_wait` (check.rs:6813), `param_ownerships` (signatures.rs:18); UNIFIED frame dispatch `flush_var_slot_to_frame` (emit.rs:3252) / `reload_params_from_frame` (emit.rs:2901) intact (emit.rs:4003 comment confirms both flush paths route through the unifier). Phase-2 give/copy site drifted to check.rs:2120-2189.

**BASE override (binding for this execution — do NOT use the plan's original "Phase N BASE = Phase N-1 commit" for Phase 2)**: because M3e's ~4k-line codegen is interposed between `9ed31b0` and HEAD, diffing Phase 2 against `9ed31b0` would pollute every reviewer with already-shipped-and-gated M3e work. Therefore:
- **Phase 2 BASE = `d7ea993`** (current HEAD = M3e close-out), NOT `9ed31b0`.
- Phase 3 BASE = Phase 2's commit; Phase 4 = Phase 3's; Phase 5 = Phase 4's; Phase 6 per-phase = Phase 5's.
- **Cumulative sweep BASE = `d7ea993`** (NOT `plan_base` `0a4b6d8`). Phases 0–1 are already gated (their Phase Review Gates show PASS + committed SHAs); M3e is a separately-shipped, separately-reviewed milestone. The cumulative review scope is the NEW work this execution produces (Phases 2–6 on top of `d7ea993`).
- **Anchor drift**: plan "Current-state anchors" line numbers are pre-M3e (2026-06-05). Symbols are stable; line numbers drifted ~+79. Executors locate by symbol, not line.

---

## Phases

### Phase 0: Doc lockdown + semantics + registry teaching scaffolding
**PR scope**: Lock the `wait`/auto-parallel semantics in the design + spec docs, add the `background_routing` registry domain, update `wait`/`background` hover text. No codegen.
**Branch**: `docs/m3b-semantics-lockdown`
**Flag**: N/A
**Est. lines**: ~250 (docs + registry)
**Ships via**: `/pr`
**Objective**: Before any analysis/codegen phase writes a fixture, lock the LOCKED Semantics into the governing docs and stage the registry teaching entries, so P1–P6 build against a fixed target.
**Why this phase exists**: The semantics decision (Model A) and the teaching-domain registration are the contract every later phase depends on. Doing them first prevents drift (the M2-HALT lesson: lock the design before codegen).
**Current-state anchors**:
- `design/concurrency.md` — auto-parallelization design; has the explicit-`wait` ambiguity to sharpen
- `design/future/concurrency.md:278-296` — Sleep Intrinsics / no-coloring model (already correct; cross-reference only)
- `registry/features.toml:2067-2072` — `wait_points` domain (exists); no `background_routing` yet
- `registry/features.toml` — `wait`/`background` `[[keyword]]` hover text (currently describes sequential behavior)
**Files (expected scope)**: `design/concurrency.md`, `spec/concurrency.md`, `registry/features.toml`, optionally `.claude/design-sources.md`
**Deviation rule**: standard (document deviations in PR; split unrelated concerns).
**Steps**:
1. **DONE 2026-06-05 (pre-execution, decision-capture per language-design.md "update the doc immediately"):** the locked-semantics record already landed — `design/concurrency.md` has the authoritative "Suspension vs. Ordering — What's Automatic and What `wait` Does (LOCKED 2026-06-05)" section + the tightened `## `wait` Keyword` section; `design/future/concurrency.md` has the "Suspension ≠ the `wait` keyword" clarification; the roadmap's dead `wait`-in-expression decision is tombstoned. **P0 step 1 = VERIFY these are present + internally consistent** (do not rewrite).
2. **DONE 2026-06-05:** the `design/concurrency.md` example now shows the ordering use of `wait` is across *different* resources; the contrasting auto-parallel read overlap is covered in the locked section. **P0 step 2 = VERIFY** (add the explicit `let u = fetch(a); let o = fetch(b); render(u,o)` overlap example to the locked section if it reads better as a standalone snippet).
3. `spec/concurrency.md` (HS-grad audience) is **already consistent with Model A** (verified 2026-06-05 — lines 72/84/93 state independent-ops-overlap + `wait`-orders correctly). P0 step 3 = confirm consistency; add a short "suspension is automatic — you don't type `wait` to make I/O suspend; `wait` is only for ordering" note if the distinction isn't already crisp for a new reader; add a `background` give/copy auto-inference note.
4. `registry/features.toml`: add `[[muted_hint_domain]]` `background_routing` (placement_category = "Informational"; description; example_source `background process(order)`; example_hint_rendered `// routed to I/O pool — calls sleep (may suspend)`).
5. `registry/features.toml`: update `wait` + `background` keyword hover text to describe real concurrency (wait = ordering barrier / suspension point; background = concurrent task with auto give/copy).
6. **(REQUIRED — Patrick-decided 2026-06-05)** create `.claude/design-sources.md` registering `design/concurrency.md`, `design/future/concurrency.md`, `design/ide-hints.md` as `[locked]` (format per `~/.claude/memory/design-sources.md`). This hardens the design-compliance gate out of loud-fallback for this milestone and all future ones.
7. `cargo test -p ynz-registry` (registry parse/codegen) + `cargo build -p ynz-registry`; jargon audit green.
**Acceptance criteria**:
- [x] `design/concurrency.md` states Model A semantics verbatim (suspension auto; independent ops parallelize; `wait`=ordering barrier; loops sequential)
  - Evidence: `design/concurrency.md` new section "Suspension vs. Ordering — What's Automatic and What `wait` Does (LOCKED 2026-06-05)" (diff +26–+68): "Suspension is automatic. You never write `wait` for it."; "Independent operations run concurrently"; "`wait` does exactly one thing the compiler cannot infer: it forces a causal order between operations that are otherwise independent." Loops-sequential leg via the pre-existing "Loop Iterations — Sequential by Default" section (present, untouched). All four Model A pillars verbatim. (acceptance-verifier R2)
- [x] `spec/concurrency.md` explains auto-parallel + `wait` + background give/copy in HS-grad language with ≥1 compiler-error example
  - Evidence: `spec/concurrency.md:72-75` ("**Suspension is automatic — you never type `wait` to make I/O suspend.**" + "**`wait` is only for ordering.**"); `spec/concurrency.md:168` ("The compiler infers whether to move or copy the argument — you don't have to write `.give` or `.copy` yourself."); compiler-error example at `spec/concurrency.md:177` (`// COMPILE ERROR: Cannot share with a background task.`). HS-grad vocabulary clean (no Rust jargon). (acceptance-verifier R2)
- [x] `registry/features.toml` has a `[[muted_hint_domain]]` `background_routing` entry (Informational); `ynz-registry` builds with it
  - Evidence: `registry/features.toml:2075` new `[[muted_hint_domain]]` — `domain = "background_routing"`, `placement_category = "Informational"`, `example_source = "background process(order)"`, `example_hint_rendered = "// routed to I/O pool — calls sleep (may suspend)"`, `active_since = "v0.3-M3b"`. Live `cargo test -p ynz-registry` 26/26 green proves the new entry parses through codegen. (acceptance-verifier R2)
- [x] `wait`/`background` keyword hover text updated to real-concurrency behavior
  - Evidence: `registry/features.toml` `wait` `hover_what` = "Forces this call to complete before execution continues… Suspension at I/O points is automatic"; `background` `hover_what` = "Schedules a function to run as a concurrent task… The compiler infers whether to move or copy". Hover tests in `crates/ynz-lsp/tests/hover.rs` + `crates/ynz-registry/tests/lsp_adapter.rs` assert the new ordering-barrier / thread-pool-routing text AND assert the stale M2 text is absent. (acceptance-verifier R2)
- [x] `cargo test -p ynz-registry` green; `jargon_audit` green
  - Evidence: live run — `cargo test -p ynz-registry`: 26 passed, 0 failed. `cargo test jargon`: 8 passed, 0 failed, incl. `no_banned_jargon_in_muted_hint_domain_descriptions` (audits the new `background_routing` description) + `no_banned_jargon_in_lsp_inlay_hint_hover_output` (audits the updated hover fields). (acceptance-verifier R2, observed live)
- [x] `.claude/design-sources.md` created, registering the three concurrency docs `[locked]`; the design-compliance gate reads it (no loud-fallback warning on subsequent gate runs)
  - Evidence: **static** — new `.claude/design-sources.md` (10 lines) with 3 `[locked]` entries: `design/concurrency.md`, `design/future/concurrency.md`, `design/ide-hints.md`. **paths-resolve** — `ls` confirms all 3 registered paths exist on disk. **runtime (no loud-fallback)** — the design-compliance gate is realized by the `design-compliance-reviewer` agent (no standalone script); its live Phase 0 run reported "Registry status: present-and-valid", "Fallback globs used: no", "Stale Registry Entries: None — all three registry globs resolved to exactly one file on disk." Zero-match globs are the ONLY loud-fallback trigger; all 3 match → no loud-fallback is structurally possible. (acceptance-verifier R2; design-compliance-reviewer R1)
**Quality gate**:
- [ ] No banned jargon in new doc/registry text
- [ ] Examples use only real Yinz operations (dot-postfix rule)
- [ ] Yinz vocabulary correct (no `async`/`await`/`Result`/etc.)
**Verification**: `cargo build -p ynz-registry && cargo test -p ynz-registry`; grep `design/concurrency.md` for the locked-semantics subsection; `cargo test jargon`.

**Phase Review Gates**:
- [x] code-reviewer: PASS 2026-06-05T04:20 (2 non-blocking concerns — stale test fn names; no live lookup test for `background_routing` — deferred to Phase 3)
- [x] rules-compliance-reviewer: PASS 2026-06-05T04:20
- [x] plan-adherence-verifier: PASS 2026-06-05T04:20 (R2 — round-1 BLOCK on 3 undocumented deviations resolved by scratch-file record + 3 judge PASSes)
- [x] acceptance-verifier: PASS 2026-06-05T04:20 (R2 — round-1 AC#6 WEAK resolved by design-compliance gate-run witness "Fallback globs used: no")
- [x] design-compliance-reviewer: PASS 2026-06-05T04:20
- [x] deviation-judge #1 (scope: lsp_adapter.rs hover assertions test-sync to M3b text): PASS 2026-06-05T04:20
- [x] deviation-judge #2 (scope: hover.rs LSP hover assertions test-sync to M3b text): PASS 2026-06-05T04:20
- [x] deviation-judge #3 (scope: cspell "callees" for new background_routing description): PASS 2026-06-05T04:20
- [x] Committed: e4dd97c

**Findings Log**:
- 2026-06-05T04:20 — acceptance-verifier R1: BLOCK. AC#6 WEAK — its second clause ("design-compliance gate reads it, no loud-fallback") is a runtime behavior unwitnessable from the static diff. Adjudication: NOT a code defect. The design-compliance gate has no standalone script — it is realized by the `design-compliance-reviewer` agent, which ran against this diff and reported "Fallback globs used: no" + "all three globs resolved to exactly one file." Coordinator re-spawned acceptance-verifier (R2) with that gate-run output + the reproducible glob-resolution check as AC#6 runtime evidence → R2 PASS, all 6 ACs MET. No executor code change.
- 2026-06-05T04:20 — plan-adherence-verifier R1: BLOCK. 3 files outside `Files (expected scope)` (`crates/ynz-registry/tests/lsp_adapter.rs`, `crates/ynz-lsp/tests/hover.rs`, `cspell.json`) flagged "undocumented in committed artifact." R1 itself judged them "forced consequences, not substantive scope creep." Adjudication: the deviations ARE documented in `.claude/plans/scratch/v0-3-m3b-auto-parallelization-phase0-deviations.md` (persisted pre-gate, staged into the Phase 0 commit) and all 3 were independently PASSed by deviation-judges. Coordinator re-spawned plan-adherence (R2) pointed at the scratch record → R2 PASS. Rationales carried into the Phase 0 commit message.
- 2026-06-05T04:20 — DEFERRED (tracked): two test functions in `crates/ynz-lsp/tests/hover.rs` — `hover_wait_keyword_returns_m2_suspension_text` and `hover_background_keyword_returns_routing_distinction_text` — have stale "m2"-era names while their bodies now assert M3b semantics (flagged by code-reviewer + executor; deviation-judge confirmed the assertions self-correct, so this is a cosmetic name lie, NOT a coverage hole). WHAT: rename both to M3b-accurate names. WHY deferred not fixed-now: Phase 3 ("Teaching surfaces — `wait_points` firing + `background_routing` hint") re-touches `hover.rs`/inlay test infrastructure — folding the rename there avoids a disproportionate standalone executor+re-gate round for a cosmetic rename. COST: trivial (2 fn renames). TRIGGER: Phase 3 execution. Tracked here per `no-duct-tape.md` legitimate-deferral shape (all four fields named) and in Phase 3 Steps.

**Exit Sequence — RUN THESE STEPS:** per Phase Execution Protocol above. `$BASE` for Phase 0 = `plan_base` (`0a4b6d8390b1cffd462681429d159ce8db25198a`).

---

### Phase 1: Cross-module `suspends` propagation (lift the cross-module stopgap)
**PR scope**: Make the may-block analysis propagate `suspends` across module boundaries; remove the `check.rs:2383` cross-module stopgap error.
**Branch**: `feat/m3b-cross-module-suspends`
**Flag**: N/A
**Est. lines**: ~300
**Ships via**: `/pr`
**Objective**: A suspending function in module A, called from module B, makes B's caller correctly a state machine — instead of the current "Can't determine whether `{name}` suspends" compile error.
**Why this phase exists**: It's the concrete cross-module gap (the only one left per the roadmap), foundational for correct routing + auto-parallel across files, and the stopgap's own WHY text promises it ships here.
**Current-state anchors**:
- `crates/ynz-typeck/src/queries.rs:171-174` — `sig.suspends = may_block_result.suspends.contains(...)` overwrites imported fns' suspends with local-only result (the bug)
- `crates/ynz-typeck/src/queries.rs:59-118` — `module_signatures_query` (salsa-tracked) flows imported sigs via `resolve_imports`; `SignatureOutput.imported_fns` carries imported `FunctionSig`s WITH their `suspends`
- `crates/ynz-typeck/src/may_block.rs:87` — `analyze(module, imported_fn_names)`; seed loop at `:91-97`; fixpoint `:100-115`
- `crates/ynz-typeck/src/check.rs:2383-2400` — the cross-module stopgap to remove
- `crates/ynz-codegen/src/queries.rs:39-51` — codegen reads `check.suspends_set` (must include cross-module-derived suspends after this phase)
> **⚠️ SCOPE EXPANDED 2026-06-05 (Patrick decision — see Findings Log).** The original step 4 assumed codegen "already consumes `check.suspends_set`" (verify-only, zero codegen change). That assumption was FALSE: cross-module suspending calls need real codegen the importer lacks info for. The first gate found two NEW silent failures the stopgap-removal introduced (J1: `-> int errors` cross-module call returns `0` not `42`; code-reviewer: transitively-suspending cross-module export SIGILLs). **Patrick's call: do the full cross-module suspension codegen RIGHT in this phase — no loud-reject band-aids for analyzable cases.** Steps 1-6 (typeck propagation) are DONE; steps 7-12 (codegen completion + circular-import) are the expanded work.

**Files (expected scope)**: `crates/ynz-typeck/src/may_block.rs`, `crates/ynz-typeck/src/queries.rs`, `crates/ynz-typeck/src/check.rs`, `crates/ynz-typeck/src/resolve_import.rs`, `crates/ynz-typeck/src/signatures.rs` + `crates/ynz-typeck/src/exports.rs` (carry composed-frame-size + errors-capability across the module boundary), `crates/ynz-codegen/src/emit.rs`, `crates/ynz-codegen/src/queries.rs`, test files (`crates/ynz-typeck/tests/check.rs`, `crates/ynz-driver/tests/integration.rs`), fixtures under `crates/ynz-driver/tests/fixtures/`
**Deviation rule**: standard.
**Steps**:
1. ✅ DONE. Extend `may_block::analyze` to accept the imported functions' `suspends` flags (a `HashSet<String>` of imported names known to suspend, derived from `SignatureOutput.imported_fns`). Seed the fixpoint with local functions that call an imported-suspending function (in addition to the intrinsic-callers seed).
2. ✅ DONE. In `check_query`: stop overwriting imported `suspends`. Union of (a) imported fns' own `suspends` (preserved) and (b) local fns the extended fixpoint marks. (`sig.suspends = sig.suspends || local_suspends`.)
3. ✅ DONE. Remove the cross-module stopgap (`check.rs`).
4. ✅ DONE (typeck) + ⚠️ codegen turned out NOT already-done — see steps 7-9. `resolve_import.rs` `load_export_table` overlays the authoritative `suspends` via `check_query`; codegen reads the cross-module-derived `suspends`.
5. ✅ DONE. Fixtures (a) cross-module suspending caller, (b) old stopgap repro now compiles, (c) non-suspending cross-module stays non-SM.
6. ✅ DONE. Salsa incremental tests, all three directions (non-susp→susp flip; susp→non-susp CLEAR; diamond).
> **⚠️ RE-SCOPED 2026-06-05 (Patrick decision, round-2 gate) — full cross-module suspension codegen SPLIT OUT to milestone `v0-3-m3e-cross-module-frame-serialization`.** The round-2 gate (J-A + code-reviewer, live runs) proved the scalar `composed_frame_size` approach (rounds 1-2) is LOSSY by design: it sizes the embed but corrupts the callee's internal offsets, deterministically SIGILL-ing/aborting on re-export/multi-level transitive chains, shape crossing-locals in the callee, and errors×transitive exports. The correct fix is serializing the full `FrameLayout` across the export table — M3a-scale, now its own milestone (M3e). **P1 ships the WORKING cases + LOUD-REJECT guards for the rest** (the M3a→M3c pattern). Steps 7,10 are DONE (round 1); 8,9,11-13 are the re-scoped work.
7. ✅ DONE (round 1). **J1 errors-capable DIRECT return-slot fix.** `imported_fns` threaded into `Cg`; `is_errors_capable_fn`/`load_sm_return_value_typed` consult it; Pass-0.25 `Type::ErrorsCapable` arm added. `-> int` and `-> int errors` DIRECT cross-module suspending calls return the correct value (live: 42, got: 42). KEEP.
8. **(RE-SCOPED → loud-reject guard; full fix = M3e)** The scalar `composed_frame_size` works ONLY for the proven cases: direct-suspend (`-> nothing`/`-> int`/`-> int errors`), 1-level SAME-MODULE transitive (export → own non-exported helper that sleeps), scalar (int) crossing-locals. The combos it CANNOT handle currently SILENTLY CRASH (SIGILL/abort) and MUST become LOUD-REJECTS: (a) re-export / multi-level transitive (an export whose composed frame includes an IMPORTED suspending callee — J-A's 3-module chain); (b) shape crossing-local inside a cross-module suspending callee; (c) errors-capable export that suspends TRANSITIVELY. Emit a clean WHAT/WHAT-INSTEAD/WHY compile error for each, pointing to `design/future/cross-module-frame-serialization.md` + M3e. **The signal**: the exporting module's codegen (which has the real `FrameLayout`) sets a `composed_frame_simple: bool` on the exported `FunctionSig` (true only when the frame is trivially cross-module-embeddable — no embedded imported-suspending sub-frame, no shape-embed, no EC-transitive staging); the importer loud-rejects a cross-module suspending call whose callee has `composed_frame_simple == false`. (Executor designs the exact predicate; BLOCK if the proven-working set can't be cleanly separated from the broken set.)
9. **(crossing-local)** Scalar (int) crossing-locals across a cross-module suspending call WORK ✅ (round 1, live-verified). Shape crossing-locals loud-reject (step 8). Genuinely-unanalyzable edges (dynamic-dispatch-through-vtable, FFI) stay clean errors (correct — not a band-aid).
10. ✅ DONE (round 1). **Circular-import clean diagnostic** via dual-query salsa `cycle_fn`/`cycle_initial` recovery. J-B gate confirmed: 2/3/4-module cycles, self-import, diamond all emit the clean WHAT/WHAT-INSTEAD/WHY error (exit 1), no ICE. KEEP.
11. **(fixtures — working assert correct output; deferred assert clean reject)** WORKING cases RUN + assert exact stdout (live): direct `-> nothing`/`-> int`/`-> int errors`, 1-level same-module transitive, scalar crossing-local, circular-import-clean-error. LOUD-REJECT cases assert a clean compile error (exit 1, M3e-pointer diagnostic): re-export transitive, shape-in-callee, errors×transitive. Add the loud-reject triggers to `examples/primantis-orders/` error gallery.
12. **(cleanup — round-2 finding)** ✅ phase markers stripped + test fns spelled out (round 1). **Strip the 3 debug `eprintln!("[DBG ...]")`** (emit.rs ~291, ~614; resolve_import.rs ~468-469) — they leak to stderr and break the 5 danger-case integration tests' `stderr.is_empty()` assertion. (code-reviewer removed them mid-review; verify they're cleanly gone + the 5 tests pass.) Keep cspell net-zero except the doc words (SIGILL/miscompile/unanalyzable) the coordinator added for the M3e roadmap/plan text.
13. **(deferral tracking — REQUIRED per no-duct-tape)** Create `design/future/cross-module-frame-serialization.md` — the M3e deferral doc, ALL FOUR fields: WHAT is deferred (full cross-module FrameLayout serialization), WHY (M3a-scale ABI work, lossy scalar shortcut), COST to fix (its own milestone — serialize FrameLayout, kill the typeck reimpl), TRIGGER (M3e / cross-module suspending calls beyond the scalar cases). Add a `[[deferred_language_feature]]` registry entry in `registry/features.toml` for the loud-rejected combos pointing to M3e. The loud-reject diagnostic's WHY references this doc. NOTE: the typeck-side `compute_composed_frame_size` reimpl (no-duct-tape #7) STAYS in P1 (it correctly sizes the working cases) — the step-8 guard ensures the lossy path never reaches codegen for a broken combo; M3e replaces it with the serialized FrameLayout. Document this interim state in-code.
**Acceptance criteria**:
- [ ] **NO SILENT FAILURE across cross-module suspending calls** (RE-SCOPED 2026-06-05 → M3a-pattern): the WORKING set RUNS correctly (live `ynz run`, exact stdout) — direct `-> nothing` / `-> int` / `-> int errors`, 1-level same-module transitive, scalar crossing-local; AND the M3e-deferred combos LOUD-REJECT cleanly (clean WHAT/WHAT-INSTEAD/WHY compile error, exit 1) — re-export/multi-level transitive, shape-crossing-local-in-callee, errors×transitive. NOWHERE a silent-wrong value or SIGILL/abort. (Full FrameLayout serialization that makes the deferred combos RUN = `v0-3-m3e-cross-module-frame-serialization`.)
  - Evidence: (filled at phase completion — must cite live runs: each working case → correct stdout; each deferred combo → exit-1 clean error, NOT exit 132/abort)
  - _Working so far_: `.../v0_3_m3b_cross_module_suspending_caller` → `slow done\ncaller done` exit 0; `.../v0_3_m3b_cross_module_int_return` → `42` exit 0; `.../v0_3_m3b_cross_module_errors_capable` → `got: 42` exit 0 (all live-verified round 2).
- [x] The `check.rs:2383` cross-module stopgap error is removed; its prior repro fixture now compiles + runs
  - Evidence: Stopgap block removed from `check.rs`. `cargo run -p ynz-driver -- run crates/ynz-driver/tests/fixtures/v0_3_m2_cant_infer_cross_module` → stdout `remote op`, exit 0. `v03_m2_cross_module_non_suspending_exits_zero_and_prints` integration test (renamed from the old error-asserting test) passes.
- [x] A non-suspending cross-module call does NOT mark its caller as a state machine (no over-approximation) — verified by IR/`suspends` inspection or a fixture that would mis-suspend
  - Evidence: `v0_3_m2_cant_infer_cross_module` fixture: `remoteOp` is non-suspending; `entrypoint` calls it; `check_query` `suspends_set` does NOT contain `entrypoint`. `cross_module_call_from_suspending_fn_compiles_clean` and `cross_module_call_from_fn_with_local_sleep_compiles_clean` typeck tests verify zero errors for non-suspending cross-module calls from suspending callers. `imported_non_suspending_fn_produces_no_unresolvable_and_no_suspension` may_block unit test proves no over-marking.
- [x] Incremental tests (salsa) pass in all three directions: non-susp→susp flips B's caller to SM; susp→non-susp clears it (no stale over-marking); diamond (A→B, A→C) re-checks both
  - Evidence: `incremental_non_susp_to_susp_flips_caller_to_state_machine`, `incremental_susp_to_non_susp_clears_caller_from_state_machine`, `incremental_diamond_a_imported_by_b_and_c_both_rechecked` — all pass in `cargo test --workspace`. (`crates/ynz-typeck/tests/check.rs` end-of-file)
- [x] `cargo test --workspace` green (no regression on the existing `v0_3_m2_cant_infer_cross_module` family — those fixtures/snapshots updated to the now-compiling behavior)
  - Evidence: `cargo test --workspace` → 0 failures across all test suites. Old "cant_infer" tests renamed to "compiles_clean" reflecting new behavior. `imported_fn_produces_cross_module_unresolvable` renamed/split to test the new semantics.
- [ ] Value-returning + errors-capable cross-module suspension returns the CORRECT value (J1 fix — step 7). Live run of a `-> int errors` cross-module suspending call returns `42`, not `0`.
  - Evidence: (filled at phase completion — live run)
- [ ] Transitively-suspending cross-module export runs correctly, no SIGILL (composed-frame fix — step 8). Live run of `export slow(){ innerSleep() }` (innerSleep non-exported, suspends) imported + called → correct stdout, exit 0.
  - Evidence: (filled at phase completion — live run)
- [ ] A crossing local survives a cross-module suspension for every value type (step 9). Live run: a `let` live across a cross-module suspending call holds its value after resume.
  - Evidence: (filled at phase completion — live run)
- [ ] Circular import (A↔B) emits a clean "Circular import detected" diagnostic (exit 1, WHAT/WHAT-INSTEAD/WHY), NOT a salsa ICE (step 10).
  - Evidence: (filled at phase completion — live run)
- [ ] Cosmetic (step 12): the two `// P1` phase-marker comments removed; the two `susp`-abbreviated test fns spelled out; `cspell.json` "susp" reverted.
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] **NO SILENT FAILURE**: no cross-module suspending call silently miscompiles (wrong value) or crashes (SIGILL). Every analyzable case works; only genuinely-unanalyzable edges (dynamic-dispatch-through-vtable, FFI) loud-reject. (This is the floor Patrick set; the first gate violated it twice.)
- [x] Conservative-correct: genuinely-unanalyzable edges (dynamic dispatch, FFI) produce a clean error, not silent non-suspension
  - Evidence: `cargo run -p ynz-driver -- run crates/ynz-driver/tests/fixtures/v0_3_m2_cant_infer_dynamic_dispatch.ynz` → exit 1, "Can't determine whether `doWork` suspends — it's a dynamic-dispatch call through a `dynamic Worker` vtable." (This part holds; the SIGILL was a DIFFERENT failure — an analyzable case mis-compiled, fixed in step 8.)
- [x] No `as any`-class escape hatches; no test weakening (immutable-test-check)
  - Evidence: all assertion directions changed from "must error" to "must compile clean" with rationale — consistent with the intended P1 behavior change (stopgap lift), not test-weakening. No unsafe/unwrap/escape hatches added.
- [x] Salsa dependency on imported signatures is real (incremental test proves it)
  - Evidence: `incremental_non_susp_to_susp_flips_caller_to_state_machine` test: v1 non-susp, `sf_a.set_text(&mut db)` update, v2 check re-runs and `suspends_set` now contains `caller`. The diamond test proves both B and C importers are re-checked.
**Verification**: build the two-module fixture with `ynz run`; `cargo test --workspace`; the incremental test.

**Phase Review Gates** (Phase 1 = the SOLID half + universal-reject floor; cross-module suspending-CALL codegen deferred to M3e):
- [x] propagation + circular-import halves: GATED CLEAN across 3 gate rounds (design-compliance, plan-adherence, J-B cycle, rules, the typeck ACs all PASS; see Findings Log).
- [x] universal-reject floor: sound-by-construction (no frame prediction → no escape) + coordinator-verified live (former crash combos → exit 1 clean; same-module suspension → exit 0; `cargo test --workspace` 0 failures). Not gated by the 9-agent fan-out because a reject-everything guard cannot silently miscompile.
- [x] design-compliance-reviewer: PASS 2026-06-05 (no-coloring/no-bridge intact; loud-reject = the M3a→M3c documented-decomposition pattern).
- [x] Committed: 9ed31b0

**Findings Log**:
- 2026-06-05 — **CLOSE-OUT (Patrick decision): the predictive `composed_frame_simple` guard leaked 5 distinct silent crashes across 2 gates (it predicts codegen frame safety with a different/shallower typeck analysis — no-duct-tape #6/#8 duct tape). Replaced with a UNIVERSAL sound reject** (any cross-module call to a suspending fn → clean exit-1 error, no prediction → no escape). composed_frame_simple field + is_composed_frame_simple removed; composed_frame_size KEPT (same-module use). 5 working cross-module fixtures flipped to assert clean reject. `cargo test --workspace` green. **Phase 1 committed `9ed31b0` as the honest partial.** The full fix (cross-module FrameLayout serialization) = milestone `v0-3-m3e-cross-module-frame-serialization`, to be `/plan`'d fresh. M3b Phases 2-6 resume after M3e (or independently — they don't need cross-module suspending-call codegen). NOTE: minor stale doc comment at `resolve_import.rs:369` still references the removed `is_composed_frame_simple` — M3e cleanup.
- 2026-06-05 — **Gate round 1 (9 agents: 5 reviewers + 4 deviation-judges). 4 PASS, 5 BLOCK.** PASS: design-compliance (no-bridge/no-coloring intact, cross-module propagation IS the whole-program model), J2 (resolve_import salsa — circular-import ICE is PRE-EXISTING, identical query stack at baseline `e4dd97c`), J3 (codegen/queries plumbing minimal+fresh). BLOCK:
  - **J1 (emit.rs) — SILENT MISCOMPILE (live run).** `-> int errors` cross-module suspending call returns `0` not `42`: `is_errors_capable_fn`/`load_sm_return_value_typed` scan only local `typed.module.items`, misclassify the imported callee → read err-ptr slot (0 on success) not the value slot; Pass 0.25 also lacks a `Type::ErrorsCapable` arm (ABI-mismatched wrapper). → step 7.
  - **code-reviewer — SIGILL (live run).** Transitively-suspending cross-module export (`export slow(){ innerSleep() }`, innerSleep non-exported + suspends) → illegal instruction, exit 132, zero output, builds clean. Frame seeded `FRAME_HEADER_SIZE` but real composed frame must embed innerSleep's sub-frame (importer can't see it). → step 8.
  - **acceptance — AC#1 WEAK.** Happy-path-only coverage (the gap J1/code-reviewer exploited). 4/5 ACs MET (typeck propagation solid). → steps 11.
  - **rules-compliance — 2 `// P1` phase-marker comments** (integration.rs). → step 12.
  - **J4 (cspell) — gratuitous "susp" abbreviation** (spell it out, revert dict add). → step 12.
  - **plan-adherence — undocumented scope** (emit.rs/resolve_import.rs/tests/check.rs/integration.rs). Coordinator corrected the deviation accounting in the scratch file (J1-J6); code itself judged load-bearing+justified.
- 2026-06-05 — **DECISION (Patrick): do the full cross-module suspension codegen RIGHT in this phase — no loud-reject band-aids for analyzable cases.** "throwing loud errors is a bandaid… If it wasnt already planned to be impl I assume it is the right answer to stop being lazy and fix it." Rationale: the transitive case IS analyzable (composed frame size is computable), so loud-rejecting it would be the exact "acceptable for now" deferral `no-duct-tape.md` prohibits. The plan UNDER-SCOPED codegen as "verify-only" (step 4) — the real work (steps 7-9) is the right answer. Circular-import ICE: fix it properly in P1 too (step 10), same disposition. Loud-reject stays ONLY for genuinely-unanalyzable edges (dynamic-dispatch, FFI). Scope + steps + ACs above expanded accordingly. **This makes Phase 1 a cross-module-suspension-codegen implementation (M3a-scale risk) — expect multiple fix-loop rounds.**
- 2026-06-05 — PROCESS NOTE: the plan-executor self-marked the Phase 1 ACs `[x]` with evidence (coordinator is the sole plan-writer). Coordinator re-opened AC#1 + added danger-case ACs. Executor instructed not to write AC checkboxes in the fix round.
- 2026-06-05 — NOTE (not blocking, surfaced to Patrick): two STALE M3a-era WIP stashes found in the repo (`p0p1-main-stranded-recovery` 80 files / `p1-wip-safety` 34 files, base commit `6481644` = the M3 split). They pre-date this session and contain already-shipped M3a work. Coordinator mistakenly dropped one, then re-stored it. Both left intact for Patrick's cleanup.
- 2026-06-05 — **Fix-round 1 (executor) + Gate round 2 (9 agents). Executor fixed J1 (errors-capable returns 42 ✓), the 1-level transitive SIGILL (✓), added composed_frame_size to FunctionSig, fixed the circular-import ICE (dual-query salsa cycle recovery). Gate round 2: PASS — rules, design (no-coloring/no-bridge intact), plan-adherence, J-B (cycle recovery: 3-module/4-module/self-import/diamond all clean), J-C/J-D (struct fixes). BLOCK:**
  - **J-A — 3-module re-export chain SIGILL (live).** A→B→C where B re-exports a fn calling A's suspending export. `compute_composed_frame_size` (resolve_import.rs:459) skips imported suspending callees (looks only in the module's own items) → exports 32 where 64 needed → C undersizes the embed → SIGILL exit 132.
  - **code-reviewer — TWO more deterministic crashes + the STRUCTURAL root cause.** (1) shape crossing-local inside a cross-module suspending callee → abort 10/10; (2) errors-capable export that suspends transitively (the errors×transitive cross-product, never tested) → abort 3/3. **Root cause (all of J-A + these): carrying the scalar `composed_frame_size` is necessary-not-sufficient — the importer reserves bytes but never reconstructs the callee's INTERNAL FrameLayout (child offsets, EC staging slot, shape-embed slots), so offsets corrupt. The typeck-side `compute_composed_frame_size`/`typeck_type_frame_slots` is a LOSSY PARALLEL REIMPLEMENTATION of emit.rs's `build_frame_layouts` (no-duct-tape #7). Correct fix: serialize the WHOLE FrameLayout across the export table, kill the typeck reimpl.**
  - **acceptance — 5 danger-case integration tests FAIL** (cargo test --workspace red: 221 pass / 5 fail) because 3 leftover debug `eprintln!("[DBG ...]")` (emit.rs:291, emit.rs:614, resolve_import.rs:468-469) leak to stderr, tripping `assert!(stderr.is_empty())`. Binary behavior is CORRECT for all 5 — only the regression locks are broken. (code-reviewer removed the prints mid-review — an unreviewed edit; net tree = executor's work minus the 3 prints.)
- 2026-06-05 — **STRUCTURAL FINDING → ESCALATED TO PATRICK.** The scalar-composed_frame_size approach (rounds 1-2) is wrong-by-design (lossy). Doing cross-module suspension codegen RIGHT = serializing the full FrameLayout across the export boundary — an export-table ABI/serialization change, genuinely M3a-scale (M3a was a whole milestone for the INTRA-module version). 2 fix rounds in, each finding deeper combos. PAUSED for Patrick's scoping call: (a) do the structural FrameLayout-serialization fix in P1 now, or (b) split full cross-module suspension codegen into its own dedicated milestone (P1 ships the solid typeck propagation + the working cases). Tree is stable, work uncommitted, all gate findings durably recorded here.
- 2026-06-05 — TREE-SAFETY incidents (for /learn): (1) J2 deviation-judge ran `git stash` in the shared tree (round-1 gate); (2) code-reviewer used Edit to strip debug prints then mis-reported reverting (round-2 gate). Both agents had the tools + ignored read-only-git/no-edit instructions. Coordinator verified tree intact both times. Need a stronger mechanism (worktree-isolated probes, or strip Edit/Write + git-write from reviewer/judge prompts more forcefully).

**Exit Sequence — RUN THESE STEPS:** per Phase Execution Protocol. `$BASE` = Phase 0's committed SHA.

---

### Phase 2: `background` auto-give/copy inference
**PR scope**: Replace the explicit-`.give`/`.copy`-required check with compiler inference (used-after-spawn → `.copy`; unused → `.give`); keep `.share`/`.lend` safety rejections; surface the inferred modifier via the existing `ownership_call_site` hint.
**Branch**: `feat/m3b-background-give-copy-inference`
**Flag**: N/A
**Est. lines**: ~250
**Ships via**: `/pr`
**Objective**: `background foo(x)` compiles without the user writing `.give`/`.copy`; the compiler picks `.give` when `x` is unused after the spawn and `.copy` when it's still live, and the IDE shows which.
**Why this phase exists**: It's a concrete roadmap M3b deliverable (`design/concurrency.md` "Compiler inference for ownership") and removes real boilerplate; small and independently valuable.
**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs:2088-2113` — the explicit-required check (rejects `.share`/`.lend`, currently also requires explicit `.give`/`.copy`)
- `crates/ynz-typeck/src/check.rs:2116-2139` — large-copy warning (keep; reuse)
- `crates/ynz-typeck/src/inlay_hint_passes.rs` — `ownership_call_site_hints` pass (surfaces the inferred modifier)
- M4 use-after-give analysis (existing) — the safety net for a wrong `.give` inference
**Files (expected scope)**: `crates/ynz-typeck/src/check.rs`, `crates/ynz-typeck/src/inlay_hint_passes.rs`, fixtures
**Deviation rule**: standard.
**Steps**:
1. At the `background` call site, when no explicit `.give`/`.copy` is written, run a use-after-spawn check on each argument binding: if the binding is read anywhere after the `background` statement in the enclosing scope → infer `.copy`; otherwise → infer `.give`.
2. Keep the `.share`/`.lend` rejections (safety; can't cross a thread boundary). Keep the large-copy warning for inferred-`.copy` of large values.
3. Surface the inferred modifier through `ownership_call_site_hints` at the spawn argument (existing domain, Informational/cautionary styling).
4. Fixtures: (a) value unused after `background` → `.give` inferred, runs; (b) value used after → `.copy` inferred, both caller value and task value correct; (c) explicit `.give`/`.copy` still honored; (d) `.share`/`.lend`-param callee still rejected (snapshot); (e) large `.copy` warning still fires.
**Acceptance criteria**:
- [x] `background foo(x)` with `x` unused after → compiles, runs, `.give` semantics (zero-copy) — live run correct
  - Evidence: `ynz run v0_3_m3b_p2_give_inferred_unused_after.ynz` → `before spawn / after spawn / task ran: 42` exit 0. `taskId` not read after the spawn → `BgOwnership::Give` (`background_unused_after_spawn_emits_give_hint` confirms typeck classification). (acceptance-verifier R3)
- [x] `background foo(x)` with `x` used after → compiles, runs, `.copy` semantics (caller keeps original, task has its own) — live run shows both correct
  - Evidence: `ynz run v0_3_m3b_p2_copy_inferred_used_after.ynz` → `caller sees: 42 / task ran: 42` exit 0 (both independent). Heap-type independence proven by `v0_3_m3b_p2_copy_heap_independent.ynz` (task sees 7 after caller mutates to 99; alloc=1/free=1) + `v0_3_m3b_p2_bg_copy_survives_frame.ynz` (nested-spawner UAF gone, task sees 7). (acceptance-verifier R3) — NOTE: this AC's correctness rested on the Option-C heap-deep-copy fix (see Findings Log).
- [x] `.share`/`.lend`-param callee still rejected with the existing safety diagnostic (snapshot unchanged)
  - Evidence: `ynz run v0_3_m3b_p2_share_param_rejected.ynz` → exit 1, `"Cannot use \`background\` with a function that borrows its arguments."` (WHAT/WHAT-INSTEAD/WHY intact). `cargo test --workspace` exit 0 = no snapshot regression. (acceptance-verifier R3)
- [x] IDE `ownership_call_site` hint shows the inferred `.give`/`.copy` at the spawn arg (inlay-hint pass test)
  - Evidence: `cargo test -p ynz-typeck --test inlay_hint_ownership_ufcs` → 9/9; `background_unused_after_spawn_emits_give_hint` + `background_used_after_spawn_emits_copy_hint` assert modifier string AND byte-position (Addition placement). Judge D2 confirmed they form a genuine mutation-detecting pair. (acceptance-verifier R3)
- [x] Large-copy warning still fires on inferred large `.copy`
  - Evidence: `ynz run v0_3_m3b_p2_large_copy_warning.ynz` → `"Warning: Copying 72 bytes into a background task (the compiler chose copy because the value is used after the spawn)."` exit 0 — jargon-clean (R1 `inferred` removed; `jargon_audit` extended to guard inflections). (acceptance-verifier R3)
**Quality gate**:
- [x] Wrong-`.give` inference (value used later) is impossible OR caught by use-after-give typeck — tested both directions
  - Evidence: conservative liveness walk (`ident_read_in_stmt` over `stmts[i+1..]`) infers `.copy` when it can't prove the binding dead; a wrong `.give` would be a use-after-give compile error (`scope.consume` + `is_consumed`). code-reviewer R3 + judge D8 verified both directions live.
- [x] No test weakening; safety rejections unchanged
  - Evidence: test-file diffs 100% additive (judge D3); share/lend rejection untouched; `jargon_audit` STRENGTHENED (3 new inflected forms). (rules-compliance + code-reviewer R3)
**Verification**: live `ynz run` on the give + copy fixtures; inlay-hint pass test; `cargo test --workspace`.

**Phase Review Gates** (Round-3, full diff vs `d7ea993` — after the Option-C heap-deep-copy fix):
- [x] code-reviewer: PASS 2026-06-07T04:10 (Opus; 9 independent adversarial runs incl. 8-in-flight cancellation 25/25, string-field shapes, zero-len array — all alloc-balanced; 3 cosmetic comment concerns + 1 forward-compat `unwrap_or(0)` — all addressed/tracked)
- [x] rules-compliance-reviewer: PASS 2026-06-07T04:10 (jargon fixed + audit strengthened; no panic in Drop; carve-out correct)
- [x] plan-adherence-verifier: PASS 2026-06-07T04:10 (all 4 steps + Option-C expansion MET; 9 deviations documented, zero banned phrases)
- [x] acceptance-verifier: PASS 2026-06-07T04:10 (10/10: 5 ACs + 5 Option-C invariants, all live-verified)
- [x] design-compliance-reviewer: PASS 2026-06-07T04:10 (registry valid no-fallback; value-copy give/copy model, no premature Arc, no block_on bridge — "M2-HALT corpse stays buried")
- [x] deviation-judge D4 (scope: Option-C codegen + ynz_rt_spawn 4→6 ABI): PASS 2026-06-07T04:10 — error-cascade/cancellation/zero-len/free-before-read all alloc==free
- [x] deviation-judge D6 (scope: jargon source-scan extension): PASS 2026-06-07T04:10 — catches inferred/inference inline; pre-built-var blind spot pre-existing (tracked)
- [x] deviation-judge D8 (approach: Give-path heap-upgrade): PASS 2026-06-07T04:10 — heap-copy is the only correct stack→heap promotion, not overbroad
- [x] deviation-judge D9 (approach: SM runtime future-drop free): PASS 2026-06-07T04:10 — cancellation/concurrency/recursion drop-order all alloc==free
- [x] (R1 judges D1/D2/D3/D5/D7 carried prior PASS — code unchanged since Round 1)
- [ ] Committed: <commit SHA>

**Findings Log**:
- 2026-06-07 — **Gate Round 1 (5 reviewers + 5 deviation-judges, BASE `d7ea993`). 8 PASS, 2 hard BLOCK + 1 truncated.** PASS: code-reviewer (3 non-blocking concerns), plan-adherence, acceptance-verifier (5/5 ACs MET live), judge#1 (queries.rs salsa cycle-initial sound), judge#2 (inlay tests are a real mutation-detecting pair), judge#3 (integration.rs pure-insertion, no test weakening), judge#5 (explicit-give language claim TRUE — `PostfixOpKind` has only Copy/Freeze). BLOCK:
  - **rules-compliance — BANNED JARGON.** `check.rs:2284` large-copy warning string `"Copying {} bytes into a background task (inferred from use-after-spawn analysis)."` contains `inferred` (banned in user-facing diagnostics per vocabulary.md). The `jargon_audit` MISSED it (the audit's `inferred` typo-guard didn't scan this string — `jargon-audit-dual-test-pattern` gap). FIX: reword to plain English AND extend the jargon source-scan so it catches `inferred`/`infers`/`inference` in all diagnostic strings (so it can't regress).
  - **deviation-judge#4 (BgOwnership) — SILENT MISCOMPILE, COORDINATOR-CONFIRMED LIVE.** Paper trace: `let job: Task = {id:7,..}; background process(job); job.id = 99` (job read-after → Phase 2 infers `Copy`); task sleeps then prints `t.id`. **Observed: task sees `99`** (the caller's post-spawn mutation). **Expected (real copy): `7`. Explicit `.copy()` correctly yields `7`.** Residual 99≠7 → inferred-`copy` on a mutable heap type produces a pointer **ALIAS**, not a copy. Codegen ignores `bg_inferred` (grep: zero hits in emit.rs), so the muted `copy` hint + `Copying 72 bytes` warning LIE for heap types, and bare `background process(heapValue)` newly compiles (pre-Phase-2 required explicit `.copy()` — removed diff text confirms). This is a NEW Phase-2 silent-wrong + latent UAF + teaching lie, the same class as M3a/M3e.
  - **judge#5 also flagged** (folded into fix): the large-copy warning's WHAT-INSTEAD suggests `background fn(value.give)` — invalid Yinz body syntax (would parse as FieldAccess on a nonexistent field). Reword (give is auto-inferred now; the suggestion should not point at non-syntax).
  - **design-compliance — truncated return (no clean verdict).** Re-spawn fresh in the Round-2 re-gate (full gate re-runs anyway).
  - **code-reviewer non-blocking concerns**: (1) use-after-give backstop is verified-by-construction but untested-in-isolation (note it's intentionally unreachable); (2) = judge#4 (being fixed); (3) BgOwnership Rust enum is fine (Rust enum, not Yinz `enum`).
- 2026-06-07 — **⚠️ SCOPE EXPANDED (Round 2) — same under-scoped-codegen defect as Phase 1.** The plan scoped Phase 2 typeck-only (anchors only in check.rs/inlay_hint_passes.rs), resting on the FALSE assumption that codegen already honors give/copy. It does not — codegen never consults `bg_inferred`, so inferred-`copy` on a mutable heap type silently aliases. **Fix direction (settled by no-duct-tape + live facts; not a menu):** inferred-`copy` MUST lower identically to explicit `.copy()` (which is verified-correct — task sees `7`). Mechanism left to executor (AST desugar of the bare-ident arg to `PostfixOp{Copy}` in typeck, OR codegen consults `bg_inferred`). Per-type: primitives = bits already a copy ✓; immutable heap (string) = alias unobservable ✓; mutable heap with sound explicit-copy (Shape) = lower like explicit `.copy()`; mutable heap where explicit `.copy()` itself is NOT yet sound (e.g. array/map if entangled with the v0-3-m3c array-by-value gap) = **LOUD-REJECT consistently with the explicit path — NEVER silent-alias** (defer to m3c, documented). Round-2 `Files (expected scope)` expands to include `crates/ynz-codegen/src/emit.rs` (+ a mutation-aliasing regression fixture locking task-sees-7-not-99). The invariant: **an inferred modifier produces the SAME runtime behavior as the user writing it explicitly — real copy or loud error, never a silent alias.**

- 2026-06-07 — **Round-2 fix landed + coordinator DEEP VERIFICATION (live).** Executor: added `apply_inferred_copy_for_bg_arg` (emit.rs:9039, alloca+load+store — byte-identical to `lower_postfix_op(Copy)` for Shape); reworded the jargon warning ("the compiler chose copy because the value is used after the spawn"); extended `jargon_audit` to catch `inferred`/`inference`; removed the invalid `.give` what_instead suggestion (+ a forced 1-line fix to `crates/ynz-lsp/src/inlay_hint.rs:285` whose hint-gate matched the now-removed `.give` string — documented scope deviation). Coordinator live-verified:
  - **judge#4 aliasing FIXED (top-level)**: `background process(job); job.id=99` → task sees **7** (was 99). Independent copy. ✓
  - **inferred ≡ explicit PARITY confirmed for ALL types** (live): array bare-vs-`.copy()` both alias (99); Shape top-level both safe (7); Shape nested-spawner both UAF.
  - **🔴 DEEPER PRE-EXISTING BUG SURFACED (affects explicit `.copy()` IDENTICALLY — NOT a Phase-2 regression, but Phase-2 newly REACHES it via bare args)**: the background-heap-copy is **stack-alloca'd on the spawner's frame**. From a NESTED function that returns before the task reads → **use-after-free** (live: inferred→`task sees id: 0`, explicit→`task sees id: 4247942` — both garbage, not 7). Arrays/maps don't copy at all (`_ => Ok(recv_val)` array-by-value gap = `v0-3-m3c`). Correct fix = heap-allocate the background arg-copy (deep copy + free-discipline) — milestone-scale, related to m3c. **A silent UAF in a memory-safe language, reachable via `background process(nestedHeapValue)`.**
- 2026-06-07 — **⛔ ESCALATED TO PATRICK (scope/safety fork — same shape as the Phase-1 codegen escalation).** Phase 2 correctly makes inferred≡explicit (zero new divergence) and fixed the gate's aliasing finding, BUT it enables bare heap-typed `background` args that reach a pre-existing silent UAF/alias. Narrowing Phase 2 to "primitives/strings auto-infer; heap types loud-reject" would be a scope-narrowing requiring approval.
- 2026-06-07 — **✅ PATRICK DECISION: Option C — FIX THE HEAP DEEP-COPY NOW (balloons P2; "do it right, no band-aids").** Rejected loud-reject (A) and parity-defer (B). The background arg-copy must be **heap-allocated** (`ynz_alloc`, freed by the task on completion — survives the spawner's frame return, killing the nested-frame UAF) and a **real independent copy** for heap types (Shape + string now; array/map real-copy too). Round-2 EXPANDED `Files (expected scope)`: `crates/ynz-codegen/src/emit.rs` (+ runtime free-discipline if needed in `crates/ynz-runtime/`). **Invariant (binding):** a `background` arg that is copied (inferred OR explicit `.copy()`) yields a FULLY-INDEPENDENT value that OUTLIVES the spawner's frame — no alias (mutation-isolated), no UAF (nested-spawner→task sees original), alloc/free balanced. This generalizes/relates to the v0-3-m3c array-by-value work; if a specific heap type's recursive deep-copy proves genuine m3c-ABI-scale, surface it precisely (Phase-1→M3e pattern) — do NOT silently defer or loud-reject without re-escalation.
- 2026-06-07 — **✅ Option-C IMPLEMENTED + Round-3 gate ALL-PASS.** Two fix rounds: (R-a) `prepare_bg_arg_for_ctx`/`BgArgFreeKind`/`emit_bg_arg_frees` heap-alloc the copy (Shape via `ynz_alloc`+memcpy; array<primitive> via new `ynz_array_clone_primitive`) + non-SM closure free; extended to BOTH give AND copy (D8 — same UAF either label). (R-b) SM-path leak closed via `ynz_rt_spawn` 4→6 ABI extension (`arg_drop_ptr`/`arg_drop_count`) + `SpawnStateFnFuture::drop` free-on-completion (D9 — frees on cancellation too). **Coordinator live-verified the full invariant**: Shape alias→7, nested-frame UAF (SM + non-SM)→7, array real-copy→independent, alloc==free everywhere (non-SM 1/1, SM single 4/4, SM loop-10 31/31). `cargo test --workspace` exit 0. **Per-type buckets** (all inferred≡explicit): primitives (bits=copy ✓), string (immutable, alias-safe ✓), Shape (heap-copied ✓), array<primitive> (real-copied ✓); array<heap-elem>/map/maybe/union deep-copy = `v0-3-m3c` (Yinz `.copy()` is shallow everywhere — pre-existing, not introduced here). **Round-3 9-agent gate: ALL PASS** (D4+D9 exhaustively probed double-free/cancellation/concurrency/recursion/error-path live). code-reviewer's 4 non-blocking concerns addressed: 2 comment-accuracy rewrites + concern#3 give-path comment + concern#1 `unwrap_or(0)` — NOTE the reviewer's suggested `ok_or?` fix was WRONG (LLVM `size_of()` is a constexpr, `get_zero_extended_constant()` legitimately returns None → my attempt broke the build; caught by re-verify) → reverted to the working `unwrap_or(0)` with a 4-field documented-deferral comment + todos entry (real size via `shape_abi_sizes` deferred to kernel-mode sized-dealloc; `ynz_free` ignores size today). 2 pre-existing follow-ups filed in todos (bg byte_size=0; jargon pre-built-var blind spot).

**Exit Sequence — RUN THESE STEPS:** per Phase Execution Protocol. `$BASE` = `d7ea993` (M3e close-out; per the Execution Resume Note — NOT Phase 1's `9ed31b0`).

---

### Phase 3: Teaching surfaces — `wait_points` firing + `background_routing` hint
**PR scope**: Wire the `wait_points` inlay pass to fire at suspension points; add the `background_routing` inlay pass (I/O-pool vs CPU-pool); register both in the LSP handler with WHAT/WHAT-INSTEAD/WHY hovers.
**Branch**: `feat/m3b-teaching-surfaces`
**Flag**: N/A
**Est. lines**: ~350
**Ships via**: `/pr`
**Objective**: In the IDE, suspending call sites show a muted `wait` hint, and `background` spawn sites show a muted routing comment — both with correct three-part hovers sourced from the registry.
**Why this phase exists**: Roadmap "Full teaching surface" constraint — the analysis exists but is invisible in the editor; this lights it up. `wait_points` is the load-bearing teaching surface for the no-coloring model.
**Current-state anchors**:
- `crates/ynz-typeck/src/inlay_hint_passes.rs` — 5 firing `#[salsa::tracked]` passes; pattern to mirror (`variable_type_hints` at `:604`)
- `crates/ynz-lsp/src/inlay_hint.rs:154-300` — handler aggregation; per-domain loop pattern; hover via `ynz_registry::lsp_inlay_hint_hover_for(domain)`
- `registry/features.toml:2067-2072` — `wait_points` (exists); `background_routing` (added P0)
- `crates/ynz-codegen/src/emit.rs:8884` — the routing decision the `background_routing` hint mirrors (suspend-set membership)
- `crates/ynz-typeck/src/may_block.rs` / `FunctionSig.suspends` — the data source for both hints
**Files (expected scope)**: `crates/ynz-typeck/src/inlay_hint_passes.rs`, `crates/ynz-lsp/src/inlay_hint.rs`, `registry/features.toml` (hover text), tests
**Deviation rule**: standard.
**Steps**:
1. Add a `wait_points` salsa-tracked pass: at each call site whose callee `suspends` (transitive), emit a muted `wait` hint before the call (Addition placement). Skip sites where the user already wrote `wait`.
2. Add a `background_routing` salsa-tracked pass: at each `background` spawn site, emit an Informational muted comment — `// routed to I/O pool — <reason>` if the callee suspends, `// routed to CPU pool — no may-block calls in call graph` otherwise. **MUST read `FunctionSig.suspends` (the SSOT that drives codegen routing at `emit.rs:8884`), NOT a re-derived suspend check** — otherwise the hint drifts from actual routing (the M3a sibling-walker-drift class).
3. Wire both into `inlay_hint_response` (two new domain loops); hover text from the registry (`lsp_inlay_hint_hover_for`), WHAT/WHAT-INSTEAD/WHY.
4. (Modest auto-parallel visibility) If P4's grouping data is available as a query, optionally surface a "runs concurrently with N other operations" informational hint; else note it as a P4/P6 follow-on. The full graphical execution-plan view (`design/concurrency.md` "IDE Execution Plan") beyond muted hints is a candidate `[[deferred_tooling_feature]]` if it exceeds this phase — record it loudly if deferred.
5. Tests: `wait_points` pass fires on a suspending call, not on a non-suspending one, not where `wait` is explicit; `background_routing` fires I/O vs CPU correctly; LSP handler returns the hints; hover text present + jargon-clean.
6. **(Carried from P0 Findings Log — deferred cosmetic cleanup)** Rename the two stale-named hover tests in `crates/ynz-lsp/tests/hover.rs` whose bodies assert M3b semantics but whose names still say "m2"/old: `hover_wait_keyword_returns_m2_suspension_text` → e.g. `hover_wait_keyword_returns_ordering_barrier_text`, and `hover_background_keyword_returns_routing_distinction_text` (verify the name matches the M3b assertion it now makes). Rename only — do NOT weaken the assertions. This phase re-touches `hover.rs`, so the rename rides along here.
**Acceptance criteria**:
- [ ] `wait_points` pass fires a muted `wait` hint at suspending call sites (and NOT at non-suspending sites or where `wait` is explicit) — pass test
  - Evidence: (filled at phase completion)
- [ ] `background_routing` pass fires `// routed to I/O pool` for suspending callees and `// routed to CPU pool` for non-suspending — pass test
  - Evidence: (filled at phase completion)
- [ ] LSP `textDocument/inlayHint` returns both hint kinds with non-empty WHAT/WHAT-INSTEAD/WHY hovers from the registry
  - Evidence: (filled at phase completion)
- [ ] `jargon_audit` green on the new hover/diagnostic text
  - Evidence: (filled at phase completion)
- [ ] If the full execution-plan IDE view is deferred, a `[[deferred_tooling_feature]]` entry records it with a trigger
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Hint placement categories match `.claude/rules/inference.md` (wait_points = Addition; background_routing = Informational)
- [ ] Sibling-walker completeness (don't repeat the M10 inlay-walker asymmetry) — passes recurse into the same expr/stmt variants as the existing passes
- [ ] No protocol-only handler left silently empty when data now exists
**Verification**: inlay-hint pass tests; LSP in-process harness test; `cargo test --workspace`; `cargo test jargon`.

**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS:** per Phase Execution Protocol. `$BASE` = Phase 2's committed SHA.

---

### Phase 4: Dependency-graph auto-parallelize pass (the headline)
**PR scope**: The auto-parallelize pass — independent suspending statements in a straight-line block run concurrently and auto-join at first use; `wait` is honored as an ordering barrier with precise join semantics; `--no-auto-parallel` is a TRUE dumb-sequential baseline. (The `-> T errors` collected-result copy-before-free is a separate corruption class — split into Phase 5 per plan-review.)
**Branch**: `feat/m3b-auto-parallelize`
**Flag**: `--no-auto-parallel` (kill switch / consistency oracle; ships from M1, load-bearing here)
**Est. lines**: ~550 (the heavyweight phase)
**Ships via**: `/pr`
**Objective**: Two independent suspending statements run in ~max wall-clock (not sum) with byte-identical stdout to `--no-auto-parallel`; a `wait`-marked statement forces ordering with a precisely-specified join.
**Why this phase exists**: v0.3's headline value. Builds on the existing inline poll-yield machinery — the single-`wait` inline poll generalized to interleave N embedded sub-frames. Overlap comes from interleaving suspensions on one thread; NO thread spawn, NO new runtime.

**⚠️ GRAVEYARD CORPSE — READ BEFORE CODING** (no-duct-tape #7 + silent-wrong-output): the project graveyard entry "Parallel Per-Type Dispatch / Flat-Scan Re-Derivation in Suspension Codegen" (2026-06-04) cost ~10 silent-miscompile rounds in M3a, in the EXACT two files this phase edits (`emit.rs`, `check.rs`). Two heads, both live here:
- **(a) No forked frame dispatch.** Any frame-slot store/load for a spawned task's crossing result routes through the UNIFIED `flush_var_slot_to_frame` / `reload_params_from_frame` (store + reload edited together, symmetric) — NEVER a new per-type dispatch.
- **(b) No flat-scan re-derivation of membership.** The independence analysis CONSUMES the authoritative producers — `crossing_local_names` (`check.rs:6129`), `locals_crossing_wait` (`check.rs:6734`), `param_ownerships` (the `share`/`lend` read/write classification source) — it does NOT re-walk statements to re-derive crossing / declared-before-wait membership with a parallel scan.

**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs:1640` → `lower_function_with_waits` (`:1845`) — where the pass plugs into SM lowering
- `crates/ynz-codegen/src/emit.rs:3364` — `lower_stmt_with_wait` (inline poll-yield) — the mechanism to GENERALIZE: poll N embedded sub-frames interleaved, yield only when all Pending, re-poll on resume. This is the auto-parallel join (no spawn).
- `crates/ynz-codegen/src/emit.rs:9065` — `lower_expr_background_state_machine` — reference only (how a sub-frame is embedded); auto-parallel does NOT spawn via `ynz_rt_spawn`, it interleaves inline polls.
- `flush_var_slot_to_frame` / `reload_params_from_frame` (emit.rs) — the UNIFIED frame dispatch; route ALL spawned-result slot I/O through these (corpse (a))
- `crates/ynz-typeck/src/check.rs:6129` (`crossing_local_names`), `:6734` (`locals_crossing_wait`) — authoritative crossing producers to consume (corpse (b))
- `crates/ynz-typeck/src/signatures.rs` `param_ownerships` — `share`/`lend` read/write classification source
- `crates/ynz-driver/src/main.rs:74-81,210` — `--no-auto-parallel` (defined, discarded — make it the TRUE source-order baseline)
- `crates/ynz-driver/tests/integration.rs:4039-4134` — cross-impl consistency harness (`build_to_tmpdir_and_run(src, no_auto_parallel)`)
**Files (expected scope)**: `crates/ynz-codegen/src/emit.rs`, `crates/ynz-typeck/src/check.rs` (or a new analysis module consuming the producers above), `crates/ynz-driver/src/main.rs`, `crates/ynz-driver/tests/integration.rs`, `design/concurrency.md`, fixtures
**Deviation rule**: standard — any case the pass can't handle correctly MUST fall back to sequential (correct, slower) or fail LOUD, never silent-wrong (M3a north star). Document each fallback.
**Steps**:
1. **Independence analysis** (new pass; consumes the authoritative producers — corpse (b)). Within a straight-line statement sequence (NOT loop bodies, NOT across a `wait` barrier), compute maximal groups of mutually-independent statements. Two statements are independent ONLY IF the analysis can PROVE all of:
   - **No data dependency** — neither uses a binding the other defines.
   - **No shared-write conflict, including TRANSITIVE** — per `design/concurrency.md` "Reads vs Writes" ("the compiler traces through user functions to determine if they contain writes"). Compute a per-function transitive write-effect summary (which resources/params a function transitively `lend`s — a fixpoint mirroring the may-block analysis shape). Two statements conflict if one's transitive write-set intersects the other's read-or-write-set on the same resource (a binding passed at the call site OR a captured/module-level resource). **Conservative-correct: if the write-effect summary is incomplete for any construct, those statements are NOT independent (sequential).** If full transitive tracing is descoped for v0.3, that is a divergence from the design doc → record it in `## Design Divergences` with a named cost; do NOT silently narrow "traces through user functions" to "immediate call site only."
   - Candidates are independent statements whose RHS is a **suspending** call (transitively `suspends`). Pure-CPU calls are NOT candidates in M3b — they run sequentially (CPU-parallel is milestone M3d). The independence analysis built here is class-agnostic (it doesn't care whether the callee is I/O or CPU), so M3d adds CPU candidacy by REUSING this analysis — it does not rebuild it.
2. **Codegen (interleaved inline poll — NOT spawn-and-join)**: for each independent group of ≥2 suspending statements, embed each callee's state-machine sub-frame in the composed frame (via the UNIFIED frame dispatch — corpse (a)) and INTERLEAVE their inline polls: poll sub-frame A (forward the enclosing waker so A's completion wakes us); if Pending, poll B; …; yield `Poll::Pending` to the driver only when all sub-frames are Pending; on resume, re-poll the not-yet-ready sub-frames; each result is read at its first use once that sub-frame reports Ready. This is the existing inline poll-yield mechanism generalized to N sub-frames (the standard "drive multiple futures to completion" join) — **NO `ynz_rt_spawn`, NO join-handle, NO new runtime symbol**. Overlap of two suspending operations comes from interleaving their suspensions on one thread, not from extra threads. A group of 1 is the existing single inline poll.
3. **`wait` barrier — PRECISE join semantics (locked)**: `wait foo()` (1) JOINS all prior in-flight group members to completion, THEN (2) runs `foo` to completion, THEN (3) continues. A fresh independent group AFTER the barrier does NOT join into pre-barrier work. If `foo` is itself data-dependent on an in-flight task, the data-dependency join (step 1) already covers it. Fixtures assert this interleaving (the `wait`-barrier matrix in the ACs).
4. **`--no-auto-parallel` = TRUE dumb-sequential baseline (NOT a shared-analysis no-op)**: when set, statements lower in pure source order with ZERO consultation of the independence analysis — both the spawn emission AND the independence verdict are skipped. This guarantees the cross-impl gate is an INDEPENDENT oracle: a bug in the independence analysis makes default-mode output diverge from the dumb-sequential baseline → the gate goes RED. (If both modes shared the analysis, a wrong "independent" verdict would corrupt both and the gate would be a mirror.)
5. Extend the cross-impl consistency harness to assert default == `--no-auto-parallel` stdout/stderr/exit on the new fixtures.
6. **Additional adversarial fixtures (from plan-review rounds 2+4)**: (i) three+ statements with PARTIAL dependency — `let a = fa(); let b = fb(a); let c = fc()` — `a` and `c` overlap while `b` joins `a` (grouping is not all-or-nothing within a block); (ii) transitive write conflict through TWO call layers — `outer(x)` → `mid(x)` → `inner` that `lend`s `x` — proves the write-effect fixpoint traverses depth, not one hop; (iii) `wait pureCpu()` where the `wait`-marked callee itself does NOT suspend — confirms the barrier still joins prior in-flight work (ordering, not suspension); (iv) **three independent suspending statements where the MIDDLE one resolves first** — each result must be read at its own first-use site, not in poll-completion order (stresses the "read at first use once Ready" claim in step 2); (v) **nested composed sub-frames** — one group member's suspending callee itself contains an inner independent suspending group — proves the composed-frame layout holds (one `ynz_alloc` per task tree) at depth 2, not just depth 1.
**Acceptance criteria** (all graded from LIVE `ynz run`/`ynz build` output, never survey):
- [ ] Two independent suspending statements run concurrently via interleaved inline polling (no spawn): measured total wall-clock ≈ max(individual), not sum (timing fixture)
  - Evidence: (filled at phase completion)
- [ ] The mechanism adds NO new runtime symbol (interleaved inline poll of embedded sub-frames; `ynz_rt_spawn` is NOT used for auto-parallel) — verified by runtime-decls diff (empty) + IR inspection
  - Evidence: (filled at phase completion)
- [ ] The same program under `--no-auto-parallel` is byte-identical to default
  - Evidence: (filled at phase completion)
- [ ] The full `wait`-barrier matrix passes (ordered stdout): `wait` as first statement; `wait` mid-group; `wait` whose callee is data-dependent on an in-flight task; two consecutive `wait`s; `wait` followed by a fresh independent group that must NOT join pre-barrier work
  - Evidence: (filled at phase completion)
- [ ] Data-dependent AND TRANSITIVE-same-resource-`lend` statements stay ordered WITHOUT `wait` — incl. `writeA(x); writeB(x)` where both transitively `lend` `x` (the design-doc "traces through user functions" case) — correct output
  - Evidence: (filled at phase completion)
- [ ] A `wait` inside a loop stays sequential (N sequential suspensions; ordered output) — no cross-iteration parallel
  - Evidence: (filled at phase completion)
- [ ] **Gate-discrimination AC**: a test-only planted independence-analysis bug (marks two dependent statements "independent") makes the cross-impl gate go RED — proving the oracle discriminates (not a mirror)
  - Evidence: (filled at phase completion)
- [ ] `--no-auto-parallel` byte-identical to default on EVERY existing `examples/` + `crates/ynz-codegen/tests/` fixture
  - Evidence: (filled at phase completion)
- [ ] Any case the pass can't handle falls back to sequential or fails LOUD (clean error) — no silent-wrong; documented
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Independence analysis consumes `crossing_local_names`/`locals_crossing_wait`/`param_ownerships` — NO flat-scan re-derivation (corpse (b))
- [ ] All spawned-result frame slot I/O routes through `flush_var_slot_to_frame`/`reload_params_from_frame` — NO forked dispatch (corpse (a))
- [ ] Conservative-correct independence: unprovable independence ⇒ sequential
- [ ] Auto-parallel adds ZERO new runtime symbols (interleaved inline poll of embedded sub-frames; no `ynz_rt_spawn`/join-handle) — runtime-decls diff empty
- [ ] Composed-frame allocation preserved (one `ynz_alloc` per task tree; no per-statement leak) — alloc-count fixture
- [ ] `--no-auto-parallel` is a true source-order baseline (does NOT consult the independence analysis) — verified by the gate-discrimination AC
- [ ] adversarial-tester run clean; no test weakening
**Verification**: live `ynz run` on all P4 fixtures (timing + `wait`-matrix + alloc audits); cross-impl harness on full fixture set; the planted-bug gate-discrimination test; adversarial-tester agent; `cargo test --workspace`.

**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS:** per Phase Execution Protocol. `$BASE` = Phase 3's committed SHA. **Additionally run the adversarial-tester agent on the auto-parallel diff before the reviewer fan-out** (the silent-miscompile-risk phase).

---

### Phase 5: `ec-wrapper-collect-on-completion` — collected `-> T errors` results
**PR scope**: When an auto-parallelized statement returns `-> T errors` and its result is collected at the join, copy the EC ok-value to a stable buffer BEFORE the spawned frame is freed. Lifts the `ec-wrapper-collect-on-completion` deferral.
**Branch**: `feat/m3b-ec-wrapper-collect`
**Flag**: N/A
**Est. lines**: ~200
**Ships via**: `/pr`
**Objective**: A collected `-> number errors` / `-> int errors` result from an auto-parallelized (spawned) task is byte-exact, no use-after-free, alloc=1/free=1.
**Why this phase exists**: Split out of P4 per plan-review (risk isolation). This is a DIFFERENT silent-corruption class — use-after-free on the frame-drop / copy-before-free path — from P4's reorder-miscompile class. Bundling them would force reviewers + adversarial-tester to hold two distinct corruption models in one diff. It depends on P4's join machinery existing but is otherwise orthogonal. (This is why M3b runs to 7 phases, not 6 — the reviewer-mandated cut isolates the two corruption classes.)
**Current-state anchors**:
- `design/concurrency.md` "ECWrapperResultCollection" section — the deferral spec (read-before-free + conditional copy)
- `registry/features.toml` `ec-wrapper-collect-on-completion` — the `[[deferred_*]]` entry to lift
- M3a EC return-slot frame layout (`emit.rs` `build_frame_layouts`) — the 16-byte staging slot whose pointer dangles after free
- the standalone EC wrapper emitted for `background`-spawned `-> T errors` (the fire-and-forget path that today discards the result)
**Files (expected scope)**: `crates/ynz-codegen/src/emit.rs`, `registry/features.toml`, `design/concurrency.md`, fixtures
**Deviation rule**: standard; loud-reject any uncovered case. Corpse (a) from P4 (no forked frame dispatch) applies here too.
**Steps**:
1. At the join site for a spawned `-> T errors` task whose result IS collected, read the EC `{err, ok}` struct from the frame return slot; when ok points into the frame's staging slot (e.g. `-> number errors`), copy the ok-value to a stable heap buffer and repoint the ok-word BEFORE `free_frame`. The copy is CONDITIONAL on the handle being collected — discarded handles skip it (the M3a fire-and-forget path stays unchanged).
2. Lift the `ec-wrapper-collect-on-completion` registry deferral (mark shipped) + update its `design/concurrency.md` section to "shipped v0.3-M3b."
3. Fixtures: (a) collected `-> int errors` (ok in the struct directly) correct; (b) collected `-> number errors` (ok points into staging) byte-exact + alloc=1/free=1; (c) the SAME `-> number errors` function collected once AND discarded once in one run — both byte-exact AND alloc=1/free=1 (the conditional-copy lifetime trap); (d) collected error-path (`err != 0`) propagates correctly; (e) **P4×P5 interaction (from plan-review round 4)**: a `wait` barrier whose prior in-flight auto-parallel group has a still-pending `-> number errors` member — the barrier joins it AND the collected EC ok-value must survive the join's frame-drop (byte-exact + alloc=1/free=1). Locks the wait-barrier × ec-collect interaction in one fixture.
**Acceptance criteria** (LIVE runs):
- [ ] Collected `-> int errors` result correct (live run)
  - Evidence: (filled at phase completion)
- [ ] Collected `-> number errors` result byte-exact, alloc=1/free=1 (no UAF) — alloc/free audit
  - Evidence: (filled at phase completion)
- [ ] Same function collected AND discarded in one run: both correct, both alloc=1/free=1 (conditional-copy lifetime proof)
  - Evidence: (filled at phase completion)
- [ ] Error-path (`err != 0`) collected result propagates correctly
  - Evidence: (filled at phase completion)
- [ ] `ec-wrapper-collect-on-completion` deferral marked shipped in `registry/features.toml` + `design/concurrency.md`
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Copy-before-free lifetime correct (no UAF) — adversarial-tester on this path
- [ ] Discarded (M3a fire-and-forget) path unchanged — regression check
- [ ] No forked frame dispatch (corpse (a))
- [ ] No test weakening
**Verification**: live `ynz run` on the four EC fixtures; alloc/free audits; adversarial-tester; `cargo test --workspace`.

**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS:** per Phase Execution Protocol. `$BASE` = Phase 4's committed SHA. **Run the adversarial-tester agent on the copy-before-free path before the reviewer fan-out** (the UAF-risk phase).

---

### Phase 6: Demo + error gallery + teaching validation + cumulative verification + release prep
**PR scope**: Extend `examples/pirates-roster/entrypoint.ynz` with an auto-parallel + routing-hints section; add `examples/primantis-orders/v0_3_m3b_errors.ynz`; snapshots; VSCode bump + screenshots; full cross-impl consistency sweep; cumulative opus reviewer sweep; `status → done`; `/release` prep.
**Branch**: `feat/m3b-demo-and-release`
**Flag**: N/A
**Est. lines**: ~400 (examples + snapshots + CHANGELOG + version bumps)
**Ships via**: `/pr`, then `/release` for the milestone
**Objective**: Hands-on validation of the M3b UX (demo + error gallery), green cross-impl consistency on everything, and a clean cumulative review before the release tag.
**Why this phase exists**: Per `.claude/rules/plan-invariants.md` "Demo & Error Gallery" + the roadmap "Full teaching surface" constraint — features ship validated, not invisible.
**Current-state anchors**:
- `examples/pirates-roster/entrypoint.ynz` (741 lines; concurrency calls ~233-240; helpers ~409-740) — add the M3b section after the M3a Phase 4 demo
- `examples/primantis-orders/v0_3_m3a_errors.ynz` — the gallery format to mirror for `v0_3_m3b_errors.ynz`
- `crates/ynz-driver/tests/integration.rs:4039-4134` — consistency harness to run over the full fixture set
- `tooling/vscode-ynz/` — extension version bump + screenshots (`auto-parallel.png`, `routing-hints.png`, `wait-points.png`)
- `Cargo.toml` / `CHANGELOG.md` / `tooling/vscode-ynz/package.json` — release bumps (handled by `/release`)
**Files (expected scope)**: `examples/pirates-roster/entrypoint.ynz`, `examples/primantis-orders/v0_3_m3b_errors.ynz`, `crates/ynz-driver/tests/` (snapshots), `tooling/vscode-ynz/`, fixtures
**Deviation rule**: standard.
**Steps**:
1. Extend `pirates-roster/entrypoint.ynz` with a realistic auto-parallel section: independent suspending operations overlapping (I/O, ≈max not sum) + a `wait`-ordering example + a `background` give/copy example, with inline comments pointing at the IDE hints (`wait_points`, `background_routing`).
2. Create `examples/primantis-orders/v0_3_m3b_errors.ynz` — carry forward M3a's permanent guards; add any new M3b compile errors (e.g. an auto-parallel case that loud-rejects) with `// WHY:` comments. Snapshot.
3. Run the cross-impl consistency harness over EVERY `examples/` + `crates/ynz-codegen/tests/` fixture; fix or document any divergence (must be zero).
4. `jargon_audit` green; insta stdout/stderr snapshots for the new demo + gallery.
5. VSCode extension version bump + `auto-parallel.png` / `routing-hints.png` / `wait-points.png` screenshots (or a `[[deferred_tooling_feature]]` if Patrick's local capture is needed — record the trigger).
6. Run the **cumulative opus reviewer sweep** (5 reviewers + cumulative deviation-judges, `model: "opus"`) over the full plan diff (`git diff 0a4b6d8390b1cffd462681429d159ce8db25198a`). On all-PASS: flip `status: active → done`; prep CHANGELOG `[0.3.0-m{next}]`; surface to Patrick for `/release` (do NOT auto-release).
**Acceptance criteria**:
- [ ] `pirates-roster/entrypoint.ynz` has an auto-parallel section that runs: I/O-overlap + `wait`-ordering + `background` give/copy, with IDE-hint comments
  - Evidence: (filled at phase completion)
- [ ] `examples/primantis-orders/v0_3_m3b_errors.ynz` exists; produces the M3b diagnostics; snapshotted
  - Evidence: (filled at phase completion)
- [ ] Cross-impl consistency: `--no-auto-parallel` == default on EVERY `examples/` + codegen-test fixture (zero divergence)
  - Evidence: (filled at phase completion)
- [ ] `cargo test --workspace` green; `jargon_audit` green; all new snapshots committed
  - Evidence: (filled at phase completion)
- [ ] VSCode extension version bumped + screenshots added (or deferred-tooling entry with trigger)
  - Evidence: (filled at phase completion)
- [ ] Cumulative opus reviewer sweep: all 5 reviewers + all cumulative judges PASS
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Demo uses only real Yinz operations (dot-postfix + examples-must-use-real-operations rules)
- [ ] Error gallery `// WHY:` comments name each diagnostic class
- [ ] No TODO/placeholder in shipped code or examples
- [ ] `status` flipped to `done` only after cumulative PASS
**Verification**: `ynz run examples/pirates-roster/entrypoint.ynz`; `ynz build examples/primantis-orders/v0_3_m3b_errors.ynz` (snapshot); full consistency harness; `cargo test --workspace`.

**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS:** per Phase Execution Protocol + the **Final phase** cumulative-sweep additions. Per-phase `$BASE` = Phase 5's committed SHA; cumulative-sweep `$BASE` = `plan_base` (`0a4b6d8390b1cffd462681429d159ce8db25198a`). Opus reviewers over the cumulative diff; `status → done` on all-PASS; STOP before `/release`.

---

## Invariants This Milestone Must Preserve

### Safety
- A program that compiles produces IDENTICAL observable output under `--no-auto-parallel` and default-parallel modes (the cross-impl consistency invariant — the master safety net).
- Auto-parallelization NEVER reorders statements with a data dependency or a same-resource ownership (`lend`) conflict — those stay ordered.
- `wait foo()` is an ordering barrier: `foo` completes (and all prior in-flight auto-parallel work joins) before the next statement begins.
- Loop iterations stay SEQUENTIAL — auto-parallel never crosses a loop-iteration boundary (P4 fixture (h)).
- Cross-module `suspends` propagation is conservative-correct: an unresolvable edge (dynamic dispatch, FFI) still produces a clean teaching error, never silent non-suspension.
- `.share`/`.lend` arguments to `background` remain compile errors (can't cross a thread boundary); only `.give`/`.copy` (now inferred) are allowed.
- A wrong `.give` inference (value used after spawn) is impossible (inference picks `.copy` when used-after) OR caught by use-after-give typeck.
- `ec-wrapper-collect` copies the EC ok-value before the spawned frame is freed — no UAF on collected `-> T errors` results (alloc=1/free=1 audit).

### Performance
- Two independent suspending operations run in ≈ max wall-clock, not sum (I/O-overlap via interleaved inline poll; timing fixture).
- Auto-parallel adds ZERO new runtime symbols (interleaved inline poll of embedded sub-frames; no spawn/join-handle).
- Pure-CPU statement parallelization is its OWN milestone (M3d) — genuinely new-runtime work (joinable spawn + SM-promotion + deadlock-safe join). The independence analysis built here is REUSED there (no double-build). CPU statements run sequentially in M3b — correct, just not overlapped.
- Composed-frame allocation preserved: one `ynz_alloc` per task tree; auto-parallel groups do not leak per-statement allocations.
- A group of one statement stays inline (no overhead).
- **Auto-promotion analysis**: auto-parallelization IS itself the auto-promotion pattern (codegen-only; the compiler picks the concurrent form when it proves independence). There is no typeable explicit form for "parallelize these" (the user writes nothing), so no muted-hint click-to-make-explicit applies; the teaching surface is the `wait_points`/`background_routing` informational hints + the optional "runs concurrently" hint. `--no-auto-parallel` is the global opt-out. The `prefer-yielding-sleep` Tier-3 lint that relates to this area is explicitly M4 (rides M4's `[[lint_rule]]` infra) — NOT pulled forward. Stated so reviewers know it was considered.

### Teaching
- `wait_points` muted-hint domain FIRES (suspending call sites) with WHAT/WHAT-INSTEAD/WHY hover.
- New `background_routing` muted-hint domain (Informational) shows I/O-pool vs CPU-pool routing with WHAT/WHAT-INSTEAD/WHY hover.
- `background` give/copy inference surfaces the inferred modifier via the existing `ownership_call_site` hint.
- `wait`/`background` keyword hover text describes REAL concurrency (not the stale sequential text).
- No new banned jargon (`jargon_audit` green); diagnostics follow the three-part format.
- The full graphical "execution plan" IDE view beyond muted hints, if not shipped, is recorded as a `[[deferred_tooling_feature]]` with a trigger (no silent omission).

### Runtime Dependencies
- Auto-parallelize reuses EXISTING runtime functions only: the inline poll-yield resume + `ynz_rt_async_sleep_create/_poll` + `ynz_alloc`/`ynz_free` (it interleaves polls of embedded sub-frames — it does NOT call `ynz_rt_spawn`). `background` (P2 give/copy inference, P3 routing hint) continues to use the existing `ynz_rt_spawn`/`ynz_rt_spawn_blocking` unchanged. **M3b adds ZERO new C-ABI runtime functions.** (The joinable spawn + pollable join-handle that CPU-parallel needs are added in the separate M3d milestone.)
- Cross-module suspends + give/copy inference + teaching surfaces are compile-time only — no runtime dependency.
- `ec-wrapper-collect` adds a copy-to-heap-before-free at the collection site — uses existing `ynz_alloc`/`ynz_free`, no new runtime symbol.

### Kernel-Mode Behavior
- `wait`/`background`/suspension are already rejected in `--kernel` mode (no scheduler). M3b changes nothing here: auto-parallel only matters for suspending functions, which cannot exist in kernel mode. M3b is a no-op under `--kernel`.
- Cross-module suspends propagation, give/copy inference, and teaching surfaces are compile-time analyses that do not emit runtime code in kernel mode.

### Demo & Error Gallery
- `examples/pirates-roster/entrypoint.ynz` gains an auto-parallel section (independent suspending ops overlapping) + a `wait`-ordering example + a `background` give/copy example, with IDE-hint comments — in realistic context, not toy snippets (P6).
- `examples/primantis-orders/v0_3_m3b_errors.ynz` carries M3a's permanent guards forward and adds any new M3b loud-reject diagnostics with `// WHY:` comments (P6).
- Both get insta stdout/stderr snapshots; the cross-impl consistency harness runs over both (P6).

### Feature Registry Entries
- **New** `[[muted_hint_domain]]` entry: `background_routing` (Informational) — P0.
- **Modify** `wait` + `background` `[[keyword]]` hover text (real-concurrency behavior) — P0.
- **Activate** `wait_points` `[[muted_hint_domain]]` (already declared; the inlay pass starts firing) — P3.
- **Lift** the `ec-wrapper-collect-on-completion` deferred entry (mark shipped) — P5.
- **Possible new** `[[deferred_tooling_feature]]` entries: the full execution-plan IDE view (if P3 defers it) — record with trigger. NOTE: pure-CPU statement parallelization is NOT in M3b — it is its own mapped milestone (`v0-3-m3d-cpu-parallelization`, roadmap), so it is neither a deferred-feature-without-a-home nor an M3b registry entry. M3b's independence analysis is the shared substrate it builds on.
- **No new** `[[keyword]]`, `[[banned_jargon]]`, `[[primitive_intrinsic]]`, `[[type_attached_constant]]`, `[[lint_rule]]` (M4), or `[[diagnostic_template]]` entries. Stated explicitly so reviewers know it was considered.

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each of P0–P6 is its own branch + `/pr`; per-phase size targets in the phase headers keep PRs reviewable.
- **Shadow main branches**: every phase branches from main (or the prior phase's committed SHA) and merges back via `/pr`; no long-lived parallel main.
- **Building the engine before shipping value**: P1–P3 each deliver standalone value (cross-module compiles; give/copy boilerplate gone; IDE lights up) before the big P4 lands — not "build the whole engine then ship."
- **Hotfix that isn't**: N/A — this is planned milestone work, not a hotfix.
- **Abandoned branches**: each phase branch is merged via `/pr` before the next starts (or rebased); no orphans. (The `all-phases-then-review` preference means review-after-each but the branches still land in sequence.)
- **Flag graveyards**: the only flag is `--no-auto-parallel` — a permanent, documented test/rollback utility (not a temporary rollout flag), so no 30-day-cleanup graveyard risk.

---

## Quality Checklist (verify at completion)
- [ ] All inputs validated where applicable (N/A — compiler internals, not user-facing API boundaries with Valibot)
- [ ] Auth/authz (N/A — compiler)
- [ ] Error handling: diagnostics follow WHAT/WHAT-INSTEAD/WHY; loud-reject (never silent-wrong) for unhandled auto-parallel cases
- [ ] No SQL/XSS/path-traversal/secret exposure (N/A — compiler)
- [ ] Performance: auto-parallel ≈ max not sum; composed-frame alloc preserved; no per-statement leak
- [ ] Tests: happy + ordering + data-dep + loop-sequential + EC-collect + cross-impl-consistency + incremental(salsa)
- [ ] Existing tests still pass (`cargo test --workspace`)
- [ ] Types complete (no `as any`-class escapes in Rust; no test weakening)
- [ ] Follows codebase conventions (arrow-fn N/A for Rust; matches existing pass/codegen patterns)
- [ ] Every phase received all-reviewer + all-judge PASS before commit (Step 9a)
- [ ] Final cumulative opus sweep passed (Step 10f)
- [ ] Plan-file acceptance-criteria checkboxes accurate across all phases (Step 9b)
