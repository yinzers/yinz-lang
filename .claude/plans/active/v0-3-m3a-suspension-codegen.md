---
slug: v0-3-m3a-suspension-codegen
type: execution
owner: Patrick Rizzardi
roadmap: v0-3-concurrency-perf
status: active
depends_on: [v0-3-m2-wait-and-state-machines]
plan_base: 24d7fee081d96ab6eb04dfa493649f0435ae6a79
files:
  - crates/ynz-codegen/src/emit.rs
  - crates/ynz-codegen/src/state_machine.rs
  - crates/ynz-typeck/src/check.rs
  - crates/ynz-typeck/src/queries.rs
  - crates/ynz-typeck/src/may_block.rs
  - crates/ynz-typeck/src/intrinsics.rs
  - crates/ynz-typeck/src/inlay_hint_passes.rs
  - registry/features.toml
  - crates/ynz-driver/tests/fixtures/**
  - examples/pirates-roster/**
  - examples/primantis-orders/**
  - design/concurrency.md
created: 2026-06-01
last_updated: 2026-06-01
---

# Plan: v0.3-M3a — Suspension Codegen Completion

Created: 2026-06-01
Status: pending_approval

## Context & Why

**Goal**: Finish the `wait`/state-machine codegen substrate so a `wait` suspension works in **every** position a Yinz program can put one — specifically, lift the two M2 compile-error guards that block the common painful cases: a local crossing a suspension boundary, and a `wait` inside a loop or `match` arm. Also land the decided `sleepAsync`→`sleep` / `sleepMs`→`sleepBlocking` rename, and permanently re-document the two guards we are deliberately keeping.

**Why**: This is the first half of the old v0.3-M3, split out on 2026-06-01 (see roadmap `v0-3-concurrency-perf.md`). M3 bundled two unrelated concerns — suspension-codegen completion (this plan, M3a) and dependency-graph auto-parallelization (M3b). The codegen half is a **hard prerequisite** for the analysis half: meaningful auto-parallelization (`let a = wait fetchA(); let b = wait fetchB(); combine(a, b)`) trips the `LocalCrossesWait` guard — `a` is a local that crosses the second suspension. Building M3b on a substrate that can't keep a local across a `wait` is the **same unbuildable-boundary mistake that caused the M2 HALT** (a plan shipped a `block_on` bridge because the milestone boundary was cut at an unbuildable line). Completing the substrate first, as its own shippable slice, is the fix.

**Background — what exists today (v0.3-M2, verified against source 2026-06-01)**:
- State machines work for `wait` at the **top level of a function and inside `if` blocks** only. The composed-frame model (one `ynz_alloc` per task tree, child sub-frames embedded at compile-time offsets) and the typed return slot are in place (`crates/ynz-codegen/src/state_machine.rs`).
- Four positional guards reject everything else at **typeck** (so the codegen never sees the unsupported shape):
  1. `LocalCrossesWait` — a `let`/`const` declared before a suspension and read after it (`check.rs:460-477`, analysis `locals_crossing_wait` at `check.rs:4880`). **← M3a lifts this.**
  2. `WaitInsideLoop` — a `wait` inside a `while`/`for`/`match` body (`check.rs:434-445`, detector `wait_in_loop_or_match_body` at `check.rs:4714`). **← M3a lifts this (loop + match).**
  3. `SubExprSuspendViolation` — a suspending call nested inside a larger expression (`check.rs:493-512`, detector `suspending_calls_in_subexpr_position` at `check.rs:5232`). **← M3a KEEPS this (permanent — Golden Rule 7).**
  4. `MutualSuspensionCycle` — two-or-more different suspending functions mutually calling each other (`queries.rs:212`, detector `find_mutual_suspension_cycles` at `may_block.rs:747`). **← M3a KEEPS this (permanent — rare + restructure-able).**
- The frame **local-slot infrastructure already exists** but is used only for parameters today: `load_local_slot`/`store_local_slot` (`state_machine.rs:391`/`:421`), locals section at `FRAME_OFFSET_LOCALS_START = 32` (`state_machine.rs:69`), 8-byte i64 slots. P1's job is to wire the *existing* `locals_crossing_wait` set into this *existing* slot machinery — not build frame infra from scratch.
- The sleep intrinsics are still named `sleepAsync` (yielding, may-block) and `sleepMs` (blocking) — the rename to `sleep`/`sleepBlocking` is decided (roadmap + `design/future/concurrency.md` "Sleep Intrinsics") but not done.

**Constraints**:
- **No new user-facing syntax.** `wait` stays `wait`. We lift *positional* restrictions on it; we add no keywords.
- **Existing programs produce identical output.** The only change: programs that previously errored now compile; programs that compiled before behave identically.
- **Composed-frame allocation model preserved** (Golden Rule 8, zero-cost). Frame-backed locals reuse the one-`ynz_alloc`-per-task-tree model; loops do NOT allocate per iteration.
- **Loop iterations stay sequential** (`design/concurrency.md` "Loop Iterations — Sequential by Default"). A `wait` inside a `for` body is N *sequential* suspensions, never parallel. M3a does NOT parallelize anything (that's M3b).
- **No I/O intrinsics exist yet** (v0.5 file module). The only may-block source remains `sleep` (renamed). Every M3a fixture exercises suspension via `wait sleep(...)`.

**Success criteria** (the milestone is done + right when):
1. A local can be declared before a `wait` and correctly read/mutated after it (frame-backed: value survives suspension).
2. `wait sleep(ms)` works inside `while`, `for` (over array AND map), and `match` arms — loop counter/iterator preserved across the pause; iteration count correct; output identical to a hand-unrolled sequential version.
3. `LocalCrossesWait` + `WaitInsideLoop` no longer exist; `SubExprSuspendViolation` + `MutualSuspensionCycle` still fire, with diagnostics reworded to state the *design rationale* (no more false "ships in v0.3-M3").
4. `sleep`/`sleepBlocking` are the only names; `sleepAsync`/`sleepMs` appear nowhere in source, fixtures, registry, or docs. `jargon_audit` passes (the rename REMOVES the `Async` jargon).
5. One `ynz_alloc` per task tree holds even for loop/local-heavy suspending functions (alloc-count fixture proves it).
6. `--no-auto-parallel` produces byte-identical stdout/stderr/exit-code to default mode on every fixture (the cross-impl consistency harness — still a no-op gate in M3a, but it must stay green).

## Research Findings

(Verified against source 2026-06-01 by three parallel research agents — anchors are current.)

1. **Frame layout (`state_machine.rs:59-73`)**: per (sub-)frame — `resume_point` i32 @0, pad @4, `sleep_handle` ptr @8, 16-byte `return_slot` @16, own locals @32 (8 bytes each), then embedded child sub-frames. `alloc_frame` (`state_machine.rs:501`) uses `ynz_alloc_zeroed`. Recursion edges heap-box a child frame (`emit_suspending_call_heap_boxed`, `emit.rs:2973`). **Self-recursion already works** — only *mutual* recursion is guarded.
2. **`lower_function_with_waits` (`emit.rs:1587`)** currently slots only **parameters** into the frame (`emit.rs:1602-1606`); locals are not frame-backed. This is the exact gap P1 closes.
3. **`locals_crossing_wait` (`check.rs:4880`)** already computes the precise set of crossing locals (name + def-span + use-span), excluding parameters. Today it drives the *error*; P1 repurposes it to drive *codegen*.
4. **`wait_in_loop_or_match_body` (`check.rs:4714`)** matches `Stmt::While` / `Stmt::For` / `Stmt::Match` bodies and recurses into `Stmt::If`. So lifting `WaitInsideLoop` = handle suspension inside all three (P2 = while; P3 = for + match).
5. **May-block analysis (`may_block.rs:87`, `analyze`)** seeds from `M2_MAY_BLOCK_INTRINSICS = ["sleepAsync", "__testFallibleAsync"]` (`intrinsics.rs:24`, mirrored at `emit.rs:511`). Runs inside salsa `check_query` (`queries.rs:143`), populates `FunctionSig.suspends` (`queries.rs:171-174`). **M3a does NOT touch the analysis** beyond the rename — it's codegen-only. (Cross-module propagation = M3b.)
6. **Runtime bridge — all present** (`runtime.rs`): `ynz_rt_init:67`, `ynz_rt_spawn_blocking:140`, `ynz_rt_spawn:525` (I/O pool, already shipped M2), `ynz_rt_check_preempt:228` (M1 no-op stub), `ynz_rt_shutdown:247`, `ynz_rt_async_sleep_create:592`, `ynz_rt_async_sleep_poll:636`. **M3a adds no runtime functions** — frame-backed locals + loops reuse the existing frame + sleep-poll machinery.
7. **`--no-auto-parallel`** parsed at `main.rs:81` (`hide = true`), destructured-and-discarded at `main.rs:210` — a deliberate no-op until M3b. M3a keeps it a no-op; the consistency harness compares the two modes and they're trivially identical (no parallel pass yet).
8. **Registry** (`registry/features.toml`): `sleepMs` @630-639, `sleepAsync` @641-652 (both `[[primitive_intrinsic]]`). `wait_points` muted-hint domain @1949 exists but is **protocol-only** (LSP returns `[]`, `inlay_hint.rs:305-308`) — firing it is M3b, NOT M3a. **Zero `[[lint_rule]]` entries exist** — lint infra is genuinely M4, so M3a has NO lint obligation.
9. **Teaching gap to fix**: the kept-guard diagnostics' WHY text literally says "ships in v0.3-M3" (`check.rs:506-509`; `MutualSuspensionCycle` in `queries.rs`) and the code comment at `check.rs:491-492` calls SubExpr "the M3 feature." Leaving these re-creates the M2-style confusion. P0 rewords them.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Loop-body suspension codegen is harder than anticipated (resume must re-enter a loop body mid-iteration; the state-machine switch must reconstruct loop control flow) | High | Medium | P2 is scoped to `while` only (simplest induction state); P3 adds `for`/`match`. Each is its own PR with its own fixtures. If P2's `while` case takes >1 session, re-spike before committing to P3 — same accept/reject discipline the roadmap mandates. |
| Frame-backed local wider than the 8-byte slot truncates silently (Tier-A silent-wrong-output) — `number` is decimal128 (16 bytes); `{i64,i64}` errors-values are 16 bytes | Medium | **Critical** | Slots are i64 (8 bytes); int/bool z-extend, float `bitcast`, pointer `ptr_to_int`. For >8-byte types use TWO consecutive slots (mirror the 16-byte return-slot scheme `state_machine.rs:224-259`) or pointer-back — decided per-type at P1 entry, never truncated. Dedicated `number`-crossing fixture (P1 (i)) asserts exact decimal round-trip. Classify every value type fits-8/needs-16/ptr-backed BEFORE writing codegen. |
| Lifting a guard but missing a code path → the old runtime abort ("Cannot start a runtime from within a runtime") returns silently | Medium | High | The M2 HALT lesson: **every fixture runs through the REAL compiler** (`./target/debug/ynz run`), never a hand-written model. Each lifted case gets an end-to-end fixture asserting correct stdout BEFORE the phase claims done. The two kept guards get error-gallery fixtures asserting they STILL fire. |
| Loop suspension accidentally allocates per iteration (breaks the zero-cost frame model) | Medium | Medium | Alloc-count fixture: a suspending `for` over N items must show exactly ONE `ynz_alloc` (extend `v0_3_m2_alloc_proof_*` pattern). Verified via the existing alloc-counting test harness. |
| Loop suspension accidentally parallelizes iterations (violates "loop iterations sequential by default") | Low | High | P2/P3 fixtures assert *sequential ordering* of per-iteration side effects (print order). Auto-parallel does not exist in M3a, so this can only happen via a codegen bug; the ordered-output assertion catches it. |
| `sleep` rename misses a fixture/snapshot → CI red or stale jargon | Medium | Low | P0 greps the whole tree for `sleepAsync`/`sleepMs` (source + fixtures + snapshots + docs + registry); `cargo test --workspace` (not just `--check`) per `test-after-rename` memory — fixture strings break silently. `jargon_audit` is the backstop. |
| Cancellation mid-loop-`wait` leaks the frame (frame-backed locals add to the frame the drop-guard frees) | Low | High | Frame-backed locals live INSIDE the existing composed frame, which the `SpawnStateFnFuture::Drop` guard already frees. Extend `v0_3_m2_recursive_cancel.ynz` with a loop+local case; assert no leak (the existing cancel test harness). |
| Removing the `LocalCrossesWait`/`WaitInsideLoop` analysis functions breaks the kept guards (shared helpers) | Low | Medium | KEEP the analysis functions where shared (`block_contains_wait`, `suspending_fns` set are used by multiple guards). Only the guard *emission* is removed; `locals_crossing_wait` is *repurposed* (drives codegen), not deleted. Audit shared helpers in P1/P2 before deleting anything. |

## Questions

_(None blocking — the two scope decisions are resolved: split confirmed, kept-guards confirmed by Patrick 2026-06-01. Open implementation detail surfaced if it arises: whether match-arm suspension warrants its own phase if P3 grows too large — decide at P3 entry, not now.)_

## Risk Assessment & Rollout Strategy

**Risk level: MEDIUM** (compiler codegen correctness; a miscompiled suspending function produces wrong runtime behavior or a runtime abort. No payments/auth/user-data/SQL — it's a compiler-internal milestone.)

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | |
| Touches auth/permissions | No | |
| Raw SQL / literals | No | |
| Modifies existing data | No | |
| Third-party integration | No | Tokio is internal; no new bridge |
| Changes existing endpoints | No | Compiler; changes which programs compile (strictly more compile now) |
| New behavior with no equivalent | Yes (+LOW) | Frame-backed locals + loop suspension are new codegen paths |

**Mitigations applied** (lower MEDIUM → effectively LOW for ship-gating):
- Every success criterion has an **end-to-end fixture run through the real compiler** before the phase claims it — directly closes the M2 false-ACCEPT hole (mitigation: real-product validation, not a hand-written model).
- `--no-auto-parallel` cross-impl consistency harness as a regression guard.
- Backward compatible: programs that compiled before behave identically (read-only over existing behavior); only new compiles are added.

**Rollout plan**: This is a compiler correctness milestone — there is no production traffic and no feature flag. "Rollout" = test-gated. Ship sequence: (1) all phases pass the per-phase 5-reviewer + deviation-judge gate; (2) cumulative reviewer sweep (Step 10f); (3) demo review by Patrick via `examples/pirates-roster` (the human-eyes-on layer); (4) `/release` for the next `v0.3.0-m{n}` tag. No staged percentage rollout (N/A for a compiler).

## Design Divergences

| Doc | What it says | What we do instead | Approved rationale (named cost + reversal path) |
|-----|-------------|-------------------|------------------------------------------------|
| `v0-3-concurrency-perf.md` roadmap — old "`wait`-in-expression semantics" Architectural Decision | `let x = wait a() + wait b()` should compile and run `a()`/`b()` concurrently (expression-position suspension) | KEEP `SubExprSuspendViolation` — nested-in-expression `wait` stays a compile error | **Approved by Patrick 2026-06-01.** Named cost: users must write the step-by-step form (`let a = wait fa()` / `let b = wait fb()` / `combine(a,b)`) — which is *already* the Golden-Rule-7 preferred style and gets the same concurrency for free once M3b's statement-level auto-parallel ships. So the cost is zero real expressiveness. The superseded roadmap decision has been struck in the roadmap itself (see "Kept M2 positional guards" Architectural Decision). Reversal path: if a real use case ever demands expression-position suspension, it's an additive codegen feature (lower the sub-expression to a temp + suspension) — no data migration, no ABI change. |

_(No divergence from the governing `[locked]`-tier design docs `design/future/concurrency.md` / `design/concurrency.md` — see Design-Doc Alignment below. The single entry above is a divergence from a superseded *roadmap* decision, recorded for transparency.)_

## Design-Doc Alignment

**Governing design docs for this milestone** (read in full during planning; kept open during execution per project CLAUDE.md):

1. **`design/future/concurrency.md`** ("No Function Coloring", Status: Locked) — the end-state model. M3a is a **faithful subset-completion** of it, not a divergence:
   - The doc: "`wait` desugars to a state-machine suspension (stackless coroutines… low memory, fast spawn)." M3a completes exactly this — frame-backed mutable locals + loop suspension are the missing pieces that make the stackless state machine work in all positions. The composed-frame model (one alloc per task tree) is preserved (Golden Rule 8). **CONFIRMS alignment.**
   - The doc: "the analysis is precise; only call chains that actually reach a suspension point get suspension code." M3a does NOT change the analysis (M2's engine, extended cross-module in M3b). It only changes codegen for already-identified suspending functions. **No analysis drift.**
   - The two **kept guards** (`SubExprSuspendViolation`, `MutualSuspensionCycle`) do NOT contradict the doc: the doc mandates suspension *correctness* at may-block call sites, not expression-position suspension or mutual-recursion support specifically. Keeping them is a style/scope choice (one enforces Golden Rule 7; one defers a near-zero-payoff codegen case), not a contradiction of the locked model.

2. **`design/concurrency.md`** ("Loop Iterations — Sequential by Default") — M3a's loop suspension makes `wait` inside a `for`/`while` run as **N sequential suspensions**, never parallel. This is exactly what the doc locks. Auto-parallelization of independent statements is M3b. **CONFIRMS alignment** — and the P2/P3 fixtures assert sequential per-iteration ordering to lock it.

3. **`design/future/concurrency.md`** "Sleep Intrinsics" (DECIDED 2026-06-01) — mandates the `sleepAsync`→`sleep` / `sleepMs`→`sleepBlocking` rename "at M3 kickoff or standalone." M3a P0 does it. **CONFIRMS alignment.** (The `prefer-yielding-sleep` lint + `KernelModeRejectsWait` amendment from that same section are explicitly M4 / post-v0.3 — NOT pulled into M3a.)

**Milestone-boundary assumptions this plan depends on** (all confirmed documented in the roadmap, not invented here):
- M3a deliberately defers cross-module propagation, auto-parallelization, give/copy inference, routing, and the `wait_points`/`background_routing` IDE surfaces to **M3b** — documented in the roadmap "M3 SPLIT" Architectural Decision + the Milestone 3a/3b sections.
- M3a depends on M2's completed state-machine substrate (shipped, in `done/`).

**No contradiction or gap found between this plan and the governing design docs.** The plan completes the locked model; it does not override it.

## Phase Execution Protocol

Each phase ends with an **Exit Sequence** block listing actions to execute (persist plan state → persist deviation scratch file → fan out all reviewers + N deviation-judges in parallel → coordinator writes Evidence + Phase Review Gates → handle verdicts → prompt commit). The canonical fan-out spec is `~/.claude/commands/execute-plan.md` Step 3.d–3.h; this file references it rather than duplicating (drift hazard per `no-duct-tape.md` #7).

**Final phase (P4) additionally**: verify all phases' acceptance/quality checkboxes; fan out the full reviewer-and-judge set with `model: "opus"` against the cumulative plan diff (`git diff <plan_base>..HEAD` or `git diff <plan_base>` if uncommitted — porcelain selection per `/execute-plan` Step 4.a); flip `status: active`→`status: done` only after all reviewers + cumulative judges PASS.

## Phases

### Phase 0: Doc lockdown + sleep rename + kept-guard re-documentation
**PR scope**: Rename the two sleep intrinsics, reword the two kept-guard diagnostics, lock the design-doc + registry surface. NO codegen behavior change.
**Branch**: `chore/m3a-doc-lockdown-sleep-rename`
**Flag**: N/A
**Est. lines**: ~600+ touched (the rename is wider than first estimated — ~231 `sleepAsync`/`sleepMs` occurrences across ~17 source files: `check.rs` ~33, `emit.rs` ~27, `integration.rs` ~15, `golden.rs` ~15, `check.rs` tests ~85 — plus fixtures, snapshots, registry, docs). Mechanical but broad.
**Ships via**: `/pr`
**Objective**: Lock every name and every guard-diagnostic before any codegen phase writes new fixtures, so P1–P4 build on final names and final diagnostic text. This is the standard P0 doc-lockdown pattern (M5/M7 precedent).
**Why this phase exists**: The sleep rename is decided but undone — codegen phases must not write fixtures against soon-to-be-renamed intrinsics. The kept-guard diagnostics still falsely promise "ships in v0.3-M3"; leaving that re-creates the exact M2 confusion Patrick called out.
**Current-state anchors**:
- `crates/ynz-typeck/src/intrinsics.rs:24` — `M2_MAY_BLOCK_INTRINSICS = ["sleepAsync", "__testFallibleAsync"]`
- `crates/ynz-codegen/src/emit.rs:511` — mirrored constant; `:1716`, `:2061` — `sleepMs` codegen refs
- `crates/ynz-typeck/src/check.rs:1744` — `"sleepMs" => self.check_sleep_ms_call(call)` dispatch; sleepAsync dispatch nearby
- `crates/ynz-typeck/src/check.rs:1709-1730` — the **"`wait` has no effect" redundant-wait warning's CPU-only-intrinsic list** hardcodes `"sleepMs"` (`:1716`). This is NOT a dispatch arm — it's a separate `matches!` list and easy to miss. If not renamed to `"sleepBlocking"`, the warning silently STOPS firing for `wait sleepBlocking(...)` — exactly a case we WANT flagged (`sleepBlocking` blocks the thread; the `wait` does nothing). (Sibling warning for user fns at `:1890` uses the `suspends` predicate, no hardcoded name — unaffected by the rename.)
- `crates/ynz-typeck/src/check.rs:493-512` — `SubExprSuspendViolation` emission (reword WHY @506-509; reword comment @481-492)
- `crates/ynz-typeck/src/queries.rs:212-228` — `MutualSuspensionCycle` emission (reword WHY)
- `registry/features.toml:630-639` (`sleepMs`), `:641-652` (`sleepAsync`)
- `crates/ynz-typeck/src/inlay_hint_passes.rs:43` — `sleepAsync` reference
- `design/concurrency.md`, `design/future/concurrency.md` "Sleep Intrinsics" — already use the new names in prose; verify consistency
**Files (expected scope)**: `intrinsics.rs`, `emit.rs`, `check.rs`, `may_block.rs`, `queries.rs`, `inlay_hint_passes.rs`, `registry/features.toml`, `design/concurrency.md`, ~15 `crates/ynz-driver/tests/fixtures/v0_3_m*.ynz`, their insta snapshots, `CHANGELOG.md`.
**Deviation rule**: standard (see top-level). Document any file touched outside this list with a one-line reason.
**Steps**:
1. Rename `sleepAsync`→`sleep`, `sleepMs`→`sleepBlocking` in `M2_MAY_BLOCK_INTRINSICS` (both copies: `intrinsics.rs:24` + `emit.rs:511`), typeck dispatch arms (`check.rs`), the **redundant-wait CPU-only-intrinsic list at `check.rs:1716`** (`"sleepMs"`→`"sleepBlocking"` — so the "`wait` has no effect" warning keeps firing on `wait sleepBlocking(...)`), `may_block.rs` references, `inlay_hint_passes.rs:43`, and the two `[[primitive_intrinsic]]` registry entries (name + hover text — drop the "Async" framing; `sleepBlocking` hover names its danger per `stdlib-design.md` Rule 1).
2. Rename across all fixtures + their insta snapshots; grep the whole tree for residual `sleepAsync`/`sleepMs` (source, `examples/`, `design/`, `spec/`, snapshots, AND doc comments — e.g. `state_machine.rs:61` uses `sleepAsync` as a verb). Run `cargo test --workspace` (NOT just `cargo check`) per `test-after-rename` memory — fixture/snapshot strings break silently and `check` won't catch them.
3. Reword `SubExprSuspendViolation` WHY (`check.rs:506-509`) + code comment (`check.rs:481-492`): remove "ships in v0.3-M3"; state the Golden-Rule-7 rationale (step-by-step is the preferred style; `let r = call(...)` then use `r`). Keep WHAT/WHAT-INSTEAD/WHY shape.
4. Reword `MutualSuspensionCycle` WHY (`queries.rs:222-226`): remove "ships in v0.3-M3"; state the rare+restructure-able rationale; note self-recursion IS supported.
5. Add a "Permanent positional constraints on `wait`" section to `design/concurrency.md` documenting the two kept guards as deliberate, with the rationale.
6. Audit `registry/features.toml` for any `[[deferred_language_feature]]`/`[[deferred_tooling_feature]]` entry naming the four guards. Reclassify: lifted-two (`LocalCrossesWait`/`WaitInsideLoop`) → remove deferred entry (they ship in M3a P1-P3); kept-two → ensure they are NOT framed as "deferred to M3" anywhere.
7. `cargo test --workspace` + `cargo fmt --all` + `cargo clippy --workspace -- -D warnings`.
**Acceptance criteria**:
- [ ] `grep -rn "sleepAsync\|sleepMs" crates/ examples/ design/ spec/ registry/` returns ZERO hits (excluding CHANGELOG history entries that document the rename itself)
  - Evidence: (filled at phase completion)
- [ ] `sleep`/`sleepBlocking` fixtures run correctly through `./target/debug/ynz run` (rename is behavior-preserving)
  - Evidence: (filled at phase completion)
- [ ] `wait sleepBlocking(100)` STILL triggers the "`wait` has no effect" warning post-rename (fixture asserting the warning fires — the `check.rs:1716` list was updated, not orphaned)
  - Evidence: (filled at phase completion)
- [ ] `SubExprSuspendViolation` + `MutualSuspensionCycle` diagnostics no longer contain the string "v0.3-M3" / "ships in"; they state the design rationale
  - Evidence: (filled at phase completion)
- [ ] `design/concurrency.md` has a "Permanent positional constraints on `wait`" section naming both kept guards + rationale
  - Evidence: (filled at phase completion)
- [ ] `jargon_audit` test passes (the rename removed the `Async` jargon; no new jargon introduced)
  - Evidence: (filled at phase completion)
- [ ] `cargo test --workspace` green
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] No `sleepAsync`/`sleepMs` residue anywhere (grep clean)
- [ ] Reworded diagnostics keep the three-part WHAT/WHAT-INSTEAD/WHY shape
- [ ] No behavior change (pure rename + doc/diagnostic text)
- [ ] Follows existing registry + diagnostic patterns
**Verification**: `grep -rn "sleepAsync\|sleepMs" crates/ examples/ design/ spec/ registry/` (expect empty); `cargo test --workspace`; run a renamed fixture and confirm identical output.

**Phase Review Gates** (filled at phase completion by coordinator):
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS**: per the canonical fan-out at `~/.claude/commands/execute-plan.md` Step 3.d–3.h. Resolve `$BASE` (Phase 0 → `plan_base` front-matter = `24d7fee…`). Pre-reviewer bookkeeping only (Quality gate ticks, `last_updated`); do NOT pre-tick Acceptance criteria or pre-write Evidence. Fan out all 5 reviewers + N deviation-judges. Coordinator writes Evidence + gates after they return. Prompt commit.

### Phase 1: Frame-backed mutable locals (lift `LocalCrossesWait`)
**PR scope**: A `let`/`const` local declared before a `wait` and read/mutated after it is preserved across the suspension via a frame slot. Remove the `LocalCrossesWait` guard.
**Branch**: `feat/m3a-frame-backed-locals`
**Flag**: N/A
**Est. lines**: ~500 (codegen + analysis wiring + fixtures)
**Ships via**: `/pr`
**Objective**: Wire the existing `locals_crossing_wait` set into the existing frame local-slot machinery so crossing locals survive suspension. This is the M3a hard core and the prerequisite for everything M3b will do with multi-`wait` data flow.
**Why this phase exists**: Today only parameters get frame slots; locals crossing a `wait` are rejected. M3b's auto-parallel patterns are unbuildable without this.
**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs:1587` — `lower_function_with_waits` entry; `:1602-1606` — only params slotted today
- `crates/ynz-codegen/src/state_machine.rs:391`/`:421` — `load_local_slot`/`store_local_slot` (exist, used for params)
- `crates/ynz-codegen/src/state_machine.rs:69-88` — `FRAME_OFFSET_LOCALS_START`, slot sizing, `frame_size_flat`
- `crates/ynz-typeck/src/check.rs:4880` — `locals_crossing_wait` (the set to repurpose)
- `crates/ynz-typeck/src/check.rs:460-477` — the guard emission to remove
**Files (expected scope)**: `emit.rs`, `state_machine.rs`, `check.rs` (remove guard emission, keep + possibly export the analysis), new fixtures under `crates/ynz-driver/tests/fixtures/`, codegen golden tests.
**Deviation rule**: standard. The crossing-locals analysis must move from typeck-only to a form codegen can consume (it may need to be surfaced on the typed AST / a side-table) — if that requires touching `queries.rs`/`signature` plumbing, document it.
**Steps**:
1. Make the crossing-locals set available to codegen: either thread `locals_crossing_wait`'s output onto the typed function representation `lower_function_with_waits` already receives, or recompute it in codegen from the same logic. Pick the form that keeps ONE source of truth (no parallel reimplementation — `no-duct-tape.md` #7).
2. In `lower_function_with_waits`, assign frame local-slot indices to crossing locals (after the parameter slots) and grow the composed frame size accordingly (`frame_size_flat` + child sizes).
3. Codegen the flush/reload: `store_local_slot` at the local's definition AND after every mutation; `load_local_slot` at every use that is reachable after a suspension point. Non-crossing locals stay in SSA/registers (do NOT slot them — perf).
4. **Per-type slot handling — NO truncation, and the slot width must fit the value.** First determine each crossing-local type's codegen representation, then: int/bool → i64 (z-extend, as `store_local_slot` already does); `float` (f64, 8 bytes) → `bitcast` f64↔i64; pointer (string/shape/array) → `ptr_to_int`/`int_to_ptr`. **CRITICAL — types wider than 8 bytes:** `number` is decimal128 (**16 bytes**) and the `{i64,i64}` errors-value is 16 bytes — a single i64 slot would SILENTLY TRUNCATE (Tier-A silent-wrong-output). For any crossing local wider than `FRAME_LOCAL_SLOT_SIZE` (8), use TWO consecutive slots (mirroring the 16-byte return-slot scheme at `state_machine.rs:224-259`) OR store the value by pointer — pick one, document it. At P1 entry, enumerate every Yinz value type and classify each as fits-8 / needs-16 / pointer-backed before writing codegen; if `number` turns out pointer-backed already, the 8-byte slot is fine and that's noted — but it must be VERIFIED, not assumed.
5. Remove the guard emission at `check.rs:460-477`. Keep `locals_crossing_wait` (now drives codegen). Keep shared helpers (`block_contains_wait`, `suspending_fns`). Verify the crossing analysis handles a local defined inside a nested `if`/`match` branch and read after a top-level `wait` (scoping correctness).
6. Fixtures (all run through `./target/debug/ynz run`): (a) int local crossing one wait; (b) local mutated between two waits then read; (c) two crossing locals; (d) string-typed crossing local; (e) shape-typed crossing local; (f) value-returning fn (`-> int`) with a crossing local; (g) `-> T errors` fn with a crossing local; (h) alloc-count: crossing locals add ZERO extra `ynz_alloc` (still one per task tree); (i) **`number`-typed crossing local — assert exact decimal128 value round-trip** (the truncation guard); (j) **conditionally-defined crossing local** — local declared inside an `if` arm, read after a top-level `wait` (scoping/framing correctness, or a correct diagnostic if out of scope).
**Acceptance criteria**:
- [ ] A local mutated across a `wait` reads back the mutated value after resume (fixture (b) prints the post-mutation value)
  - Evidence: (filled at phase completion)
- [ ] Pointer/float crossing locals round-trip without truncation (fixtures (d),(e))
  - Evidence: (filled at phase completion)
- [ ] A 16-byte `number` (decimal128) crossing local round-trips its EXACT value across suspension — no truncation to 8 bytes (fixture (i)); slot-width classification documented at P1 entry
  - Evidence: (filled at phase completion)
- [ ] A crossing local defined inside a nested `if`/`match` branch is correctly framed or correctly diagnosed (fixture (j))
  - Evidence: (filled at phase completion)
- [ ] A value-returning suspending fn with a crossing local returns the correct typed value (fixtures (f),(g))
  - Evidence: (filled at phase completion)
- [ ] `LocalCrossesWait` no longer emitted — a program that previously errored now compiles AND runs correctly
  - Evidence: (filled at phase completion)
- [ ] Crossing locals add no extra heap allocation (fixture (h): alloc count unchanged vs the param-only baseline)
  - Evidence: (filled at phase completion)
- [ ] Non-crossing locals are NOT frame-slotted (inspect emitted IR for a fn with a non-crossing local — no `store_local_slot` for it)
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] One source of truth for the crossing-locals set (no parallel analysis reimplementation)
- [ ] Every crossing-local type handled explicitly (int/bool/ptr/number/float) — no silent `as`/truncation
- [ ] Composed-frame allocation model intact (one alloc per task tree)
- [ ] Cancellation drop-guard still frees the (now larger) frame — no leak
- [ ] Follows existing `lower_function_with_waits` + slot-helper patterns
**Verification**: run fixtures (a)-(h) through `./target/debug/ynz run`; codegen golden test for a representative crossing-local fn; alloc-count assertion for (h).

**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS**: canonical fan-out (`execute-plan.md` Step 3.d–3.h). Resolve `$BASE` = Phase 0's `Committed:` SHA. Same protocol as P0.

### Phase 2: While-loop suspension (lift `WaitInsideLoop`, while case)
**PR scope**: `wait sleep(ms)` works inside a `while` body — loop state preserved across suspension, iterations run sequentially.
**Branch**: `feat/m3a-while-loop-suspension`
**Flag**: N/A
**Est. lines**: ~450 (the riskiest codegen — loop control-flow reconstruction in the state machine)
**Ships via**: `/pr`
**Objective**: Make the state-machine resume function re-enter a `while` loop body mid-iteration after a suspension, with the loop condition/counter frame-backed (built on P1's slot machinery). Iterations are N sequential suspensions.
**Why this phase exists**: `wait` inside a loop is one of the two painful guards; `while` is the simplest loop (no iterator state, just a condition + frame-backed counter).
**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs:434-445` — `WaitInsideLoop` guard emission (the `while`/`for`/`match` branch)
- `crates/ynz-typeck/src/check.rs:4714` — `wait_in_loop_or_match_body` (matches `Stmt::While` @4717)
- `crates/ynz-codegen/src/emit.rs:1587` — `lower_function_with_waits` (where the resume-switch is built); P1's frame-backed-local machinery
- `crates/ynz-codegen/src/state_machine.rs:136-147` — `store_resume_point`/`load_resume_point` (the re-entry mechanism)
**Files (expected scope)**: `emit.rs`, `check.rs` (narrow the guard to exclude `while`), fixtures, codegen golden tests.
**Deviation rule**: standard. If `while`-suspension reveals that the resume-switch needs structural changes (e.g., per-loop continuation states), document the codegen approach taken.
**Steps**:
1. Codegen: when a `while` body contains a suspension point, emit a continuation state that re-enters the loop's condition check, with any loop-carried local (counter, accumulator) frame-backed via P1's slot mechanism.
2. Ensure the suspension inside the body returns `Poll::Pending` up to the driver and resumes back INTO the loop body at the correct point (not at function entry).
3. Narrow the `check.rs:434-445` guard to NOT fire for `while` (still fire for `for`/`match` until P3). Update `wait_in_loop_or_match_body` accordingly (or split it).
4. Fixtures (real compiler): (a) `while` with a counter, `wait sleep` each iteration, prints 0..N in order; (b) `while` with a frame-backed accumulator mutated each iteration; (c) value-returning fn that loops-then-returns; (d) alloc-count: a `while` over N iterations = ONE `ynz_alloc` (no per-iteration alloc); (e) ordering: per-iteration prints appear in strict sequence (proves NOT parallelized); (f) **conditional wait inside the loop** — `while (cond) { if (x) { wait sleep(5) } step() }` — only some iterations suspend; resume re-enters at the correct sub-position; assert exact iteration count + ordering.
**Acceptance criteria**:
- [ ] `wait sleep` inside a `while` body runs N times in order, counter correct (fixture (a))
  - Evidence: (filled at phase completion)
- [ ] A loop-carried accumulator is correct after the loop (fixture (b))
  - Evidence: (filled at phase completion)
- [ ] A suspending `while` loop allocates ONCE, not per iteration (fixture (d))
  - Evidence: (filled at phase completion)
- [ ] Iterations are sequential — per-iteration side effects are strictly ordered (fixture (e); locks `design/concurrency.md` "loop iterations sequential")
  - Evidence: (filled at phase completion)
- [ ] `WaitInsideLoop` no longer fires for `while`; still fires for `for`/`match` (deferred to P3)
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Resume re-enters the loop body, not function entry (verified in emitted IR)
- [ ] One alloc per task tree even with a loop
- [ ] Sequential iteration ordering (no accidental parallelism)
- [ ] Guard correctly narrowed (for/match still guarded)
- [ ] Follows P1's frame-backed-local conventions
**Verification**: fixtures (a)-(e) through `./target/debug/ynz run`; alloc-count assertion; ordered-output assertion.

**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS**: canonical fan-out. `$BASE` = Phase 1's `Committed:` SHA.

### Phase 3: For-loop + match-arm suspension (lift `WaitInsideLoop`, remaining cases)
**PR scope**: `wait` works inside `for` (over array AND map) and inside `match` arms. Fully removes the `WaitInsideLoop` guard.
**Branch**: `feat/m3a-for-match-suspension`
**Flag**: N/A
**Est. lines**: ~500
**Ships via**: `/pr`
**Objective**: Extend P2's loop-suspension to `for` (iterator/index state frame-backed) and `match` (resume into the correct arm — structurally closer to the already-working `if` case). Remove the guard entirely.
**Why this phase exists**: Completes the `WaitInsideLoop` lift; `for` and `match` are distinct codegen paths from `while`.
**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs:4718` — `Stmt::For` branch; `:4719-4728` — `Stmt::Match` branch of `wait_in_loop_or_match_body`
- P2's while-loop continuation-state machinery (the pattern to extend)
- for-loop codegen (the existing AoS array / map iteration lowering — locate in `emit.rs`; the executor confirms the exact site at phase start)
**Files (expected scope)**: `emit.rs`, `check.rs` (remove the guard + retire `wait_in_loop_or_match_body`), fixtures, golden tests.
**Deviation rule**: standard. If match-arm suspension proves materially harder than the if-case (e.g., narrowed-binding state across suspension), document the approach; if it threatens to balloon the phase, surface a split before proceeding (per Questions).
**Steps**:
1. `for`-over-array suspension: index + collection pointer frame-backed; resume re-enters the loop body for the current index.
2. `for`-over-map suspension: iterator/cursor state frame-backed (maps have a defined iteration order per locked decision).
3. `match`-arm suspension: resume into the correct arm; any narrowed binding live across the suspension is frame-backed (P1 machinery).
4. Remove the `WaitInsideLoop` guard emission (`check.rs:434-445`) entirely; retire/remove `wait_in_loop_or_match_body`.
5. Fixtures (real compiler): (a) `for (x in array)` with `wait sleep`, ordered output; (b) `for ((k,v) in map)` with `wait`, all entries visited; (c) `match` with a `wait` in one arm; (d) `for` with a crossing local AND a wait; (e) alloc-count for a suspending `for` = ONE alloc; (f) sequential ordering for `for`; (g) **empty / zero-iteration loop** — `for (x in emptyArray) { wait sleep(1) }` then read a local declared before the loop (frame slot correctly preserved when the suspending body never runs — never-stored-but-loaded guard); (h) **mutual-recursion negative case** — a self-recursive suspending fn that ALSO calls a second suspending fn non-recursively (NOT a cycle); assert `MutualSuspensionCycle` does NOT false-positive after the loop/local plumbing changes the suspending-set.
**Acceptance criteria**:
- [ ] `wait` inside `for`-over-array runs once per element in index order (fixture (a))
  - Evidence: (filled at phase completion)
- [ ] `wait` inside `for`-over-map visits every entry (fixture (b))
  - Evidence: (filled at phase completion)
- [ ] `wait` inside a `match` arm resumes into the correct arm and produces correct output (fixture (c))
  - Evidence: (filled at phase completion)
- [ ] `WaitInsideLoop` guard fully removed — no `wait`-position case remains except the two permanently-kept guards
  - Evidence: (filled at phase completion)
- [ ] Suspending `for` allocates once, iterations sequential (fixtures (e),(f))
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] for-over-array AND for-over-map both covered
- [ ] match-arm resume correct (no fall-through to wrong arm)
- [ ] One alloc per task tree; sequential ordering
- [ ] Guard + dead detector fully removed (no orphaned code)
- [ ] Follows P2 conventions
**Verification**: fixtures (a)-(f) through `./target/debug/ynz run`; golden tests; alloc + ordering assertions.

**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS**: canonical fan-out. `$BASE` = Phase 2's `Committed:` SHA.

### Phase 4: Demo + error gallery + teaching + cumulative verification
**PR scope**: Extend the canonical demo + error gallery; verify the full milestone end-to-end; release prep.
**Branch**: `feat/m3a-demo-gallery-verification`
**Flag**: N/A
**Est. lines**: ~300 (mostly `.ynz` + snapshots + CHANGELOG)
**Ships via**: `/pr`, then `/release` for the next `v0.3.0-m{n}` tag after merge
**Objective**: Land the human-eyes-on layer (demo + gallery) and the cumulative cross-impl consistency proof; bump VSCode/CHANGELOG.
**Why this phase exists**: Per `plan-invariants.md` "Demo & Error Gallery" — features that ship without hands-on demo + error-experience review go unvalidated. This is how Patrick reviews the UX.
**Current-state anchors**:
- `examples/pirates-roster/entrypoint.ynz` — the growing v0.1–v0.3 demo (add a loop-with-`wait` + local-crossing-`wait` section)
- `examples/primantis-orders/` — per-milestone error gallery (the now-compiling cases move OUT; the two kept-guard cases stay/are added)
- the cross-impl consistency harness (`--no-auto-parallel` vs default) — locate the existing test
**Files (expected scope)**: `examples/pirates-roster/entrypoint.ynz`, `examples/primantis-orders/*errors*.ynz`, snapshots, `CHANGELOG.md`, `tooling/vscode-ynz/package.json` (version bump), `crates/ynz-codegen/tests/` (cross-impl assertions).
**Deviation rule**: standard.
**Steps**:
1. Extend `pirates-roster/entrypoint.ynz` with a realistic section: a roster-processing loop that `wait sleep`s per item, with a local accumulator crossing the suspension (shows P1+P2/P3 in real context, not a toy). Inline comments point at the behavior.
2. Update `primantis-orders` error gallery: REMOVE the now-compiling `LocalCrossesWait`/`WaitInsideLoop` triggers (they're no longer errors); ensure the two KEPT guards (`SubExprSuspendViolation`, `MutualSuspensionCycle`) have intentional triggers with `// WHY:` comments naming the (reworded) diagnostic class. Refresh insta snapshots.
3. Cross-impl consistency: assert `--no-auto-parallel` and default produce byte-identical stdout/stderr/exit-code on every `pirates-roster` + `crates/ynz-codegen/tests/` fixture (still trivially identical in M3a — no parallel pass — but the gate must be green and wired).
4. Teaching surface check: confirm `sleep`/`sleepBlocking` hover docs updated (P0); confirm NO new muted-hint domain / lint was needed (state explicitly in the PR — M3a adds no new IDE surface; `wait_points` firing is M3b). Bump VSCode `package.json` version (renamed-intrinsic hover carries forward; no new screenshot — no new surface).
5. `jargon_audit`, `cargo test --workspace`, `cargo clippy -- -D warnings`, `cargo fmt --all`.
6. CHANGELOG `[v0.3.0-m{n}]` section.
**Acceptance criteria**:
- [ ] `pirates-roster/entrypoint.ynz` has a loop-with-`wait` + local-crossing-`wait` section that compiles and runs (snapshot of stdout)
  - Evidence: (filled at phase completion)
- [ ] `primantis-orders` error gallery: `LocalCrossesWait`/`WaitInsideLoop` triggers removed; `SubExprSuspendViolation` + `MutualSuspensionCycle` triggers present with reworded diagnostics in the snapshot
  - Evidence: (filled at phase completion)
- [ ] `--no-auto-parallel` == default on every fixture (consistency gate green)
  - Evidence: (filled at phase completion)
- [ ] `cargo test --workspace` green; `jargon_audit` green; clippy `-D warnings` clean
  - Evidence: (filled at phase completion)
- [ ] CHANGELOG + VSCode version bumped; PR explicitly states M3a adds no new IDE muted-hint/lint surface (with reason)
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Demo shows the feature in REAL context (not `print(featureName())`)
- [ ] Error gallery accurately reflects the post-M3a guard set
- [ ] Cross-impl consistency wired + green
- [ ] No jargon; all gates green
**Verification**: run `pirates-roster` + error gallery through the compiler; snapshot review; `cargo test --workspace`; consistency harness.

**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence (FINAL PHASE) — RUN THESE STEPS**: per-phase fan-out (`$BASE` = Phase 3's `Committed:` SHA) PLUS the final cumulative sweep per `execute-plan.md` Step 4.a — fan out all 5 reviewers + cumulative deviation-judges with `model: "opus"` against the cumulative diff (`git diff <plan_base>` if uncommitted else `git diff <plan_base>..HEAD`). Flip `status: active`→`status: done` only after all return PASS. Then `/release`.

## Invariants This Milestone Must Preserve

### Safety
- A mutable local crossing a `wait` is flushed before suspension and reloaded after resume; its post-resume value equals its pre-suspension value plus any pre-suspension mutation. (Fixtures P1 (a)-(g).)
- A frame-backed local is never read uninitialized: the analysis guarantees def-before-use; codegen stores at the definition site.
- `wait` inside `while`/`for`/`match` preserves loop counter / iterator / accumulator across suspension; iteration count is correct. (Fixtures P2/P3.)
- Loop iterations run SEQUENTIALLY — a suspending `for`/`while` is N sequential suspensions, never parallel. (Ordered-output fixtures P2 (e), P3 (f).)
- `LocalCrossesWait` + `WaitInsideLoop` are removed; programs that previously errored compile AND produce correct output.
- `SubExprSuspendViolation` + `MutualSuspensionCycle` STILL fire (kept guards), verified by error-gallery fixtures. Self-recursion still works (unchanged).
- A task cancelled mid-loop-`wait` frees its frame (including frame-backed locals) with no leak — the existing `SpawnStateFnFuture::Drop` guard covers the larger frame. (Extended `recursive_cancel` fixture.)

### Performance
- Only locals that ACTUALLY cross a suspension are frame-slotted; non-crossing locals stay in SSA/registers. (IR-inspection acceptance criterion P1.)
- Frame composition preserved: ONE `ynz_alloc` per spawned task tree, even for loop/local-heavy suspending functions. Loops allocate ZERO per iteration. (Alloc-count fixtures P1 (h), P2 (d), P3 (e).)
- Pointer/number/float locals round-trip through i64 slots via `ptr_to_int`/`bitcast`, never truncation.
- **Auto-promotion analysis**: M3a introduces NO feature with a stricter/faster typeable form — it completes existing codegen for already-identified suspending functions. There is no `array→fixed`-style promotion candidate here. The `prefer-yielding-sleep` Tier-3 lint that relates to `sleepBlocking` is explicitly **M4** (rides M4's `[[lint_rule]]` infra, which does not exist yet) per the roadmap — NOT pulled forward. Stated explicitly so reviewers know it was considered, not forgotten.

### Teaching
- Reworded `SubExprSuspendViolation` + `MutualSuspensionCycle` diagnostics follow WHAT/WHAT-INSTEAD/WHY and state the *design rationale* (no false "ships in v0.3-M3").
- `sleep`/`sleepBlocking` hover docs in `registry/features.toml` updated; `sleepBlocking` names its danger (`stdlib-design.md` Rule 1).
- No new banned jargon (the rename REMOVES the `Async` jargon); `jargon_audit` passes.
- M3a introduces NO new muted-hint domain and NO new lint rule. `wait_points` firing + `background_routing` are M3b. Stated explicitly.

### Runtime Dependencies
- Frame-backed locals: NO new runtime dependency — reuse the existing heap-allocated composed frame (`ynz_alloc`/`ynz_free`) + `load_local_slot`/`store_local_slot`.
- Loop/match suspension: NO new runtime dependency — reuse `ynz_rt_async_sleep_create`/`_poll` + the resume-switch.
- Sleep rename: NO runtime dependency change (same `ynz_rt_async_sleep_*` functions; only the Yinz-surface name changes).
- M3a adds ZERO new C-ABI runtime functions.

### Kernel-Mode Behavior
- `wait`/suspension is already rejected in `--kernel` mode (every guard checks `!self.kernel_mode`). M3a does NOT change this — frame-backed locals + loop suspension only matter for suspending functions, which cannot exist in kernel mode. So M3a is a no-op under `--kernel`.
- `sleepBlocking` (renamed from `sleepMs`) is the kernel-appropriate sleep (no scheduler needed); `sleep` (yielding) requires the scheduler. The `KernelModeRejectsWait` WHAT-INSTEAD amendment pointing at `sleepBlocking` is explicitly **post-v0.3** (roadmap "Sleep Intrinsics" point 3) — NOT in M3a. Current kernel-mode behavior unchanged.

### Demo & Error Gallery
- `examples/pirates-roster/entrypoint.ynz` gains a realistic loop-with-`wait` + local-crossing-`wait` section (P4) — in context, not a toy snippet.
- `examples/primantis-orders/` error gallery: the now-compiling `LocalCrossesWait`/`WaitInsideLoop` triggers are REMOVED; the two kept-guard triggers (`SubExprSuspendViolation`, `MutualSuspensionCycle`) are present with `// WHY:` comments naming the reworded diagnostic class.
- Both files get insta stdout/stderr snapshots in P4.

### Feature Registry Entries
- **Modify** 2 `[[primitive_intrinsic]]` entries: `sleepAsync`→`sleep` (yielding, may-block member), `sleepMs`→`sleepBlocking` (blocking) — name + hover text (P0).
- **Audit + reclassify** any `[[deferred_language_feature]]`/`[[deferred_tooling_feature]]` entry naming the four guards: remove deferred entries for the two LIFTED guards (they ship in M3a); ensure the two KEPT guards are not framed as "deferred to M3" (P0).
- **No new** `[[keyword]]`, `[[banned_jargon]]`, `[[muted_hint_domain]]`, `[[lint_rule]]`, `[[deferred_*]]`, or `[[diagnostic_template]]` entries. (Reworded kept-guard diagnostics are text edits to existing emission sites, not new templates — confirm during P0 whether either is registry-backed; if so, modify in place.) Stated explicitly so reviewers know it was considered.

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each of the 5 phases is its own PR with its own branch + review gate; no "one big uncommitted blob."
- **Shadow main branches**: repo is main-only (clean slate); each phase branches off the prior phase's committed tip and merges via `/pr`. No long-lived parallel main.
- **Building the engine before shipping value**: M3a is itself the value-first slice of the old M3 — it ships "wait works everywhere" without waiting for M3b's analysis. Within M3a, P1 (the substrate) is the first codegen phase and every later phase delivers a user-visible lift.
- **Hotfix that isn't**: N/A — this is planned milestone work, not a hotfix.
- **Abandoned branches**: each phase branch merges immediately on review-PASS + commit; none left dangling.
- **Flag graveyards**: no feature flags introduced (compiler correctness milestone; `--no-auto-parallel` is a pre-existing test utility, not a new flag).

## Quality Checklist (verify at completion)
- [ ] All inputs validated — N/A (compiler-internal; no user input boundary added)
- [ ] Auth/authz — N/A
- [ ] Error handling: reworded guard diagnostics keep WHAT/WHAT-INSTEAD/WHY; no false "ships in M3"
- [ ] No SQL/XSS/path-traversal/secret exposure — N/A
- [ ] Performance: one alloc per task tree; non-crossing locals not slotted; no per-iteration alloc
- [ ] Tests: per-type crossing-local fixtures + per-loop-kind fixtures + alloc-count + ordering + cancellation + error-gallery — all through the REAL compiler
- [ ] Existing tests still pass (`cargo test --workspace`)
- [ ] Types complete (no `as any`-equivalent; explicit per-type slot handling)
- [ ] Follows existing codegen + diagnostic conventions
- [ ] Every phase received all-reviewer + all-judge PASS before committing (Step 9a)
- [ ] Final cumulative reviewer sweep passed (Step 10f, opus)
- [ ] Plan-file acceptance-criteria checkboxes accurate across all phases
