---
slug: v0-3-m3f-codegen-correctness-fixes
type: execution
owner: Patrick Rizzardi
roadmap: v0-3-concurrency-perf
status: active
created: 2026-06-09
last_updated: 2026-06-09
plan_base: d24a5f45e2c1485416f694eea6a3cdb3edddadb8
files:
  - crates/ynz-codegen/src/emit.rs
  - crates/ynz-codegen/src/state_machine.rs
  - crates/ynz-driver/tests/fixtures/v0_3_m3f_*.ynz
  - crates/ynz-driver/tests/integration.rs
  - examples/pirates-roster/entrypoint.ynz
  - examples/pirates-roster/expected_stdout.txt
  - design/concurrency.md
  - .claude/todos.md
  - .claude/plans/roadmaps/v0-3-concurrency-perf.md
---

# Plan: v0.3-M3f — Pre-Existing Codegen Correctness Fixes

Created: 2026-06-09
Status: pending_approval
Ships via: `/pr` per phase; `/release` for standalone `v0.3.0-m6` tag at milestone end.

## Context & Why

**Goal**: close two confirmed silent-miscompile bugs in the Yinz compiler's state-machine / auto-parallelization codegen, both surfaced during v0.3-M3b adversarial review and CONFIRMED live against the current `main` compiler (`d24a5f4`, the released `v0.3.0-m5` build). Both are Golden Rule 5 violations (wrong answer at runtime instead of caught at compile time). They **block the final `v0.3.0` release** (roadmap M3f section, line 277): "v0.3.0 must not ship with a known silent miscompile."

**Why now**: `v0.3.0-m5` shipped with these holes. Bug 1 produces a silently-wrong decimal128 value; Bug 2 produces a silently-wrong integer **AND breaks M3b's load-bearing cross-implementation consistency invariant** (`default == --no-auto-parallel`). Shipping the correctness fix promptly beats holding it for the multi-session M4. M4 then branches off a clean base. Standalone tag `v0.3.0-m6` (Patrick decision, 2026-06-09).

**Background — what exists today (verified by live repro on `d24a5f4`)**:

- **Bug 1 — Same-callee wide-`errors` staging-slot value-aliasing** (todos.md:180; roadmap M3f bug 1). HIGH — silent wrong value.
  - Repro: `let p1 = fetchPrice(0); let p2 = fetchPrice(1)` where `fetchPrice -> number errors` returns an arg-dependent decimal128.
  - Observed (both `default` AND `--no-auto-parallel`): `31.75` / `31.75`. Expected: `24.50` / `31.75`.
  - Root: a `-> T errors` return whose ok-value is too wide for the EC ok-word (number/decimal128, shape) puts the ok-value in the **callee's 16-byte staging slot**, and the EC `{i64,i64}` struct's ok-word **points into that slot** rather than copying the value out. A second call to the same callee reuses the staging slot, clobbering the first binding's value before it's read. Same staging-slot-dangling class as the `ec-wrapper-collect-on-completion` deferral, but in the inline-poll / sequential same-callee-reuse path (NOT the `background` path).
  - Both-modes-identical → it is **base M3a/EC-return codegen, NOT an auto-parallel regression**. The M3b cross-impl oracle (`default == --no-auto-parallel`) HOLDS here — both modes are identically wrong.

- **Bug 2 — Parallel-group result frame-backing across a subsequent suspension** (todos.md:178 named this the "nested-CF read-scan gap"; **VERIFIED REFRAME below**). HIGH + **mode-divergent**.
  - Repro (MIN-1): two independent suspending bindings get auto-parallelized into one group; a group-result binding crossing a subsequent `wait` reads garbage when the group contains a boolean result that is live across that wait.
    ```ynz
    let a = slowCall()      // int, suspends
    let cond = other()      // boolean, suspends — grouped with slowCall (independent)
    wait sleep(1)           // subsequent suspension
    if (cond) { print(a.toString()) }   // a reads 0, not 42
    ```
  - Observed: `default` → `0`; `--no-auto-parallel` → `42`. Deterministic (5/5 runs each). **Modes DIVERGE** → breaks the M3b cross-impl consistency invariant.
  - **Verified trigger boundary** (live bisection, see Research Findings): the bug fires only when auto-parallel **groups** the bindings (a data dependency that prevents grouping → correct in both modes) AND ≥2 group results are live across a subsequent suspension AND at least one is a **boolean used after the wait**. int+int (any use position) is correct; int+bool where the bool is unused-after-wait is correct.
  - Root (candidate, to be pinned by Phase-1 Paper-Trace): the boolean crossing-local frame-slot path (i1 alloca → zext i1→i64 on flush, trunc on reload — `sm_crossing_bool_set`) corrupts a sibling int result's frame slot when both are parallel-group results crossing the same later wait. The grouping DECISION is correct; the bug is in materializing/frame-backing the group results across the later suspension.

**Constraints**: no new user-facing syntax (pure codegen correctness); existing programs that were correct stay byte-identical; the fixes RESTORE the documented suspension/cross-impl invariants (they don't introduce new behavior). No new keywords, intrinsics, or registry entries.

**Success criteria**:
1. Bug-1 repro prints `24.50` / `31.75` in BOTH modes; N live bindings of the same wide-EC callee are all distinct; alloc == free.
2. Bug-2 repro prints `42` in BOTH modes; `default == --no-auto-parallel` restored across the full trigger matrix and the existing suite.
3. No regression: `cargo test --workspace` green; cross-impl consistency holds on every fixture; jargon/clippy/fmt clean.
4. The design-doc claim Bug 1 contradicted (`design/concurrency.md` ECWrapperResultCollection — "the inline-poll path is correct and complete") is corrected.

## Research Findings

All findings are from **live repro against `./target/debug/ynz` on `d24a5f4`** (the released `v0.3.0-m5` build), not from reading code alone.

**Bug 2 reframe — the most important finding.** todos.md:178 described Bug 2 as a "crossing-local READ-scan gap that crashes at compile time with LLVM 'does not dominate all uses', identical in both `default` and `--no-auto-parallel` modes." That description does **not** reproduce on current `main`:
- ~10 non-grouped variations (plain binding before a wait; suspending binding; if / while / for / match body; if-inside-if; string / shape / number types; ≥2 crossing locals) were tried — **all correct**. The codegen read-detection (`crossing_local_names` → `locals_crossing_wait`, check.rs:6527/6533) and the typeck read-scan (`collect_ident_refs_in_stmt`, check.rs:7502) both already recurse into nested CF bodies. The "read-scan recursion gap" appears already closed.
- What IS live is a **mode-divergent** miscompile that fires only under auto-parallel **grouping**. Bisection table:

  | Case | Group members | Use of `a` | Subsequent wait? | default | nopar |
  |---|---|---|---|---|---|
  | B | — (no later wait) | top-level | no | 42 | 42 ✅ |
  | H1 / C | int + int | top-level | yes | 42 | 42 ✅ |
  | H2 | int + int | nested-only | yes | 42 | 42 ✅ |
  | H3 | int + bool (**bool unused after wait**) | top-level | yes | 42 | 42 ✅ |
  | data-dep | int + bool but `other(a)` (no grouping) | nested | yes | 42 | 42 ✅ |
  | **MIN-1** | int + **bool (used after wait)** | nested-only | yes | **0** | 42 ❌ |
  | **MIN-2** | int + **bool (used after wait)** | top-level + nested | yes | **0 0** | 42 42 ❌ |

  → trigger is precisely **{auto-parallel group} × {≥2 results crossing a later wait} × {a boolean result live across that wait}**. The bool is the differentiator; the int sibling reads `0`. CRASH-hunt with `number` returned `0.00` (same class, decimal128 sibling). The exact mechanism (bool zext/trunc writing the wrong slot vs. slot-index shift when a bool slot is interleaved) is the Phase-1 Paper-Trace job.

**Bug 1** root path confirmed in source:
- `emit.rs:214` `number_errors_staging_offset`; `emit.rs:220-223`/`338-347`/`599` — the 16-byte staging slot, and the comment stating the SM EC-return path "stores the raw i128 decimal there and **points the EC `ok` word at the slot**."
- `emit.rs:5059` `bind_sm_return_value` StructValue (EC) arm — the **non-crossing** bind path; stores the struct but the ok-word still points into the staging slot (no copy-out). This is the bug site for the SM path.
- `emit.rs:5316-5320` — the non-crossing branch that calls `bind_sm_return_value` without copy-on-bind.
- `emit.rs:5122-5198` `bind_sm_result_and_flush` — the **crossing** EC arm (5154-5190) ALREADY extracts both `{i64,i64}` fields and stores copies into per-binding companion storage. **This is the fix template** — the non-crossing path must do the same copy-out for wide-value ok-words.
- `emit.rs:12723-12737` `lower_errors_capable_call_result` (non-SM inline-poll path) — also just stores the struct (dangling ok-ptr); must also copy-on-bind for wide-EC.
- `state_machine.rs:387-418` `load_return_value_errors` — returns the ok_i64 that, for `number errors`, is the staging-slot pointer.
- `registry/features.toml` `ec-wrapper-collect-on-completion` + `design/concurrency.md:450-464` `ECWrapperResultCollection` — the SAME staging-slot-dangling class but for the `background` wrapper path (gated on background-handle collection, still vacuous in M3b). Bug 1 is the **inline-poll / sequential same-callee** sibling, which is reachable today. The doc's claim "the inline-poll path ... is correct and complete" (concurrency.md:458) is **false** in the same-callee-reuse case and must be corrected.

**Deferred (out of M3f, confirmed)**: aliased shape + `lend`-across-`wait` write-back (todos.md:170) — `let b = a` shape alias, both mutated via lend across a wait → prints `n:10` not `n:110`. Verified BOTH modes identical (`10`) → base shallow-shape-alias write-back, a **different root cause** (m3c-array-by-value family), NOT the crossing-local/parallel-group family. Patrick decision 2026-06-09: defer to `v0-3-m3c-array-by-value`. Stays open in todos.md.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Bug-2 fix tempts a duct-tape "stop grouping bool-containing pairs" shortcut | Med | High | EXPLICIT no-duct-tape note in Phase 3: grouping is CORRECT (bindings are independent); disabling a correct optimization to dodge a codegen bug is `no-duct-tape.md` duct tape. Fix the frame-backing/slot path, not `independence.rs`. Reviewer + acceptance must confirm the headline (independent-statement overlap) still works after the fix. |
| Bug-1 copy-on-bind missed in one of the two paths (SM non-crossing AND non-SM inline-poll) | Med | High | Phase-2 acceptance runs the repro through BOTH a state-machine caller AND a non-SM inline-poll caller; matrix of 3+ live same-callee bindings; alloc==free assertion. |
| Bug-1 fix double-copies / leaks (per-binding buffer not freed, or wide-EC value freed twice) | Med | High | alloc==free instrumentation on every fixture (the M3a/M3b discipline); deviation-judge probes double-free / cancellation paths. |
| Fix changes byte output of an already-correct program | Low | High | Cross-impl + full-suite green is the gate; insta golden IR snapshots must stay byte-identical for unaffected functions; `--no-auto-parallel` is the oracle for every fixture. |
| Bug-2 root cause is deeper than the bool-slot path (e.g. slot-index allocation across mixed-width group results) | Med | Med | Phase-1 Paper-Trace bisects the exact slot the int sibling reads from; Phase 3 fixes the proven mechanism, not the hypothesis. The trigger matrix is the regression net regardless of which slot is wrong. |
| RED repros stay red across Phase-1 boundary and trip the no-duct-tape detector | High (expected) | Low | Both declared in `## Planned RED Repros`; the orchestrator green-lights them at the Phase-1 gate per `no-duct-tape.md` §Intentional Test-First Breaks (all 4 conditions met: same-plan fixing phase named, RED test locks the correct contract, zero independent prod exposure on a feature branch, in-code honesty comment). |
| Cross-module / EC-result-in-a-parallel-group interaction surfaces a third variant | Low | Med | Phase-4 matrix includes an EC-result-in-group fixture; if a new variant appears, it's added to the same RED-then-fix loop, not deferred silently (deferrals-must-be-tracked). |

## Questions

Both resolved by Patrick at plan time (2026-06-09):

1. **Aliased-shape + lend-across-wait bug (todos:170) in M3f scope?** → **No — defer to `v0-3-m3c-array-by-value`.** Verified different root cause (shallow shape-alias write-back, both modes identical = base codegen, not the crossing-local/parallel-group family). Roadmap "fold if cheap" does not apply — it's not cheap (m3c by-value-storage territory).
2. **Release shape?** → **Standalone `v0.3.0-m6` tag.** GR5 silent-miscompile fixes in an already-released version; ship promptly, M4 branches off a clean base.

No open questions remain.

## Risk Assessment & Rollout Strategy

**Risk level: MEDIUM** (codegen correctness in a pre-1.0 compiler; no money/auth/user-data; no production-traffic rollout — this is a developer-tool release).

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | |
| Touches auth/permissions | No | |
| Raw SQL / literals | No | |
| Modifies existing data | No | compiler codegen only |
| Third-party integration | No | |
| Changes existing endpoints / behavior | Yes (intended) | changes behavior of programs that were SILENTLY WRONG → now correct. No correct program changes output. |

**Mitigations applied**:
- Cross-impl consistency oracle (`default == --no-auto-parallel`) on every fixture → catches any new scheduling/codegen divergence. (MEDIUM → LOW)
- Full RED-repro-first discipline; fixes verified against the original repros + extended matrix. (MEDIUM → LOW)
- Backward compatible: only output of previously-wrong programs changes; rollback is a single revert (no data, no migration).

**Rollout plan** (LOW effective risk after mitigations): standard pre-1.0 — land the fixes, full local gate (`cargo test --workspace` + cross-impl + clippy/fmt/jargon), cumulative opus reviewer sweep, then `/release v0.3.0-m6` on Patrick's go. No staged percentage rollout (developer tool, not a traffic-serving service).

## Design Divergences

_(No divergence from any `[locked]` design doc. Both fixes RESTORE documented invariants. Bug 1's fix CORRECTS a now-false claim in `design/concurrency.md` (ECWrapperResultCollection:458) — that is a doc update, recorded as a Documentation Deliverable, not a divergence.)_

| Doc | What it says | What we do instead | Approved rationale (named cost + reversal path) |
|-----|-------------|-------------------|------------------------------------------------|
| — | — | — | _(empty — no divergences)_ |

## Documentation Deliverables

| Deliverable | Phase | Notes |
|---|---|---|
| `design/concurrency.md` ECWrapperResultCollection (≈line 458) claim "the inline-poll path … is correct and complete" corrected to note the same-callee wide-EC reuse hole + its M3f fix | Phase 4 | Cross-cutting: the claim is wrong because of Bug 1; the correction belongs to the whole-milestone narrative, not a single fix phase. |
| `.claude/todos.md` entries :178 + :180 closed (ticked + dated); roadmap M3f "Tracked bugs" marked shipped; aliased-shape :170 left open (deferred to m3c) | Phase 4 | Deferrals-must-be-tracked: the closed bugs leave the live list; the deferred one stays with its trigger. |

_(Per-phase doc-ACs: none beyond the above — the fixes touch no other documented subsystem. The gallery obligation does NOT produce a doc/error-gallery entry because M3f introduces no new compile-error class — see `### Demo & Error Gallery` Invariant below for the explicit rationale.)_

## Planned RED Repros

_(Declared so the orchestrator green-lights these at the Phase-1 boundary per `no-duct-tape.md` §Intentional Test-First Breaks. Both fixing phases are in THIS plan; both RED tests assert the CORRECT contract; zero prod exposure (feature branch, unreleased); in-code `// RED until Phase N (M3f)` honesty comment required.)_

| What's intentionally broken (file + function/symbol + line-range) | Locking RED test (path::test name) | Asserted contract (correct behavior, not the broken one) | Fixing phase (Phase N / Step — same plan) | Prod-exposure note (how condition 3 holds) |
|---|---|---|---|---|
| Bug 1 — wide-EC same-callee bind copies nothing out of the staging slot (`emit.rs` `bind_sm_return_value` EC arm ~5059 + `lower_errors_capable_call_result` ~12723) | `integration.rs::v0_3_m3f_ec_same_callee_aliasing_distinct_values` | Two live `let` bindings of the same `-> number errors` callee hold their OWN arg-dependent values: stdout == `24.50\n31.75` in BOTH modes | Phase 2 | Feature branch `fix/m3f-codegen-correctness`, unreleased; Phase 2 lands the fix before any tag. |
| Bug 2 — parallel-group bool result corrupts sibling int frame slot across a later wait (`emit.rs` bool crossing-local flush/reload `sm_crossing_bool_set` ~1455-1458 + `flush_var_slot_to_frame` ~3363 + `bind_sm_result_and_flush` ~5122) | `integration.rs::v0_3_m3f_parallel_group_bool_sibling_survives_wait` | Grouped int+bool results both survive a subsequent `wait`: stdout == `42` in BOTH modes AND `default == --no-auto-parallel` | Phase 3 | Same feature branch, unreleased; Phase 3 lands the fix before any tag. |

## Design-Doc Alignment

**Governing docs** (all `[locked]` in `.claude/design-sources.md`):
- `design/concurrency.md` — "Suspension vs. Ordering" (suspension preserves values; independent ops auto-parallelize; `default == --no-auto-parallel` is the cross-impl invariant); `ECWrapperResultCollection` (the staging-slot ok-pointer class).
- `design/future/concurrency.md` — no-coloring model, inline poll-and-yield.

**Conformance**: both fixes make the code MATCH these docs.
- Bug 2 fix RESTORES the cross-impl invariant the roadmap mandates ("`--no-auto-parallel` produces identical stdout/stderr/exit-code to default-parallel mode for EVERY fixture"). It is currently violated; the fix is conformance, not divergence. The fix must NOT be a grouping-suppression hack — the doc's auto-parallel model intends independent suspending statements to overlap; the codegen must materialize their results correctly.
- Bug 1 fix makes the inline-poll EC path actually "correct and complete" as `concurrency.md:458` claims — and amends that line to record the hole that existed (the claim was aspirational, not true, for same-callee reuse).

**Milestone-boundary check**: M3f defers the aliased-shape write-back to m3c (documented in the roadmap + todos with trigger). M3f does NOT depend on any unbuilt milestone — both fixes are in already-shipped codegen surface (M3a EC-return + M3b parallel-group), so no boundary is cut at the wrong line.

## Invariants This Milestone Must Preserve

### Safety
- A `let` binding of a `-> T errors` call holds the value produced by THAT call; a subsequent call to the same callee never mutates an already-bound result. (Bug 1)
- Every parallel-group result binding that is live across a later suspension reads back the value it was assigned, regardless of the other group members' types. (Bug 2)
- `default` and `--no-auto-parallel` produce byte-identical stdout/stderr/exit-code for every fixture in the suite (cross-impl consistency restored). (Bug 2)
- No use-after-free / double-free introduced by copy-on-bind: alloc == free on every fixture.

### Performance
- The auto-parallel headline is preserved: two independent suspending statements still overlap (interleaved inline poll) — the Bug-2 fix is in result frame-backing, NOT a grouping-suppression that would serialize them. Verified by the existing M3b overlap-timing fixture staying ≈max-not-sum.
- Bug-1 copy-on-bind is a bounded per-binding copy of one wide-EC ok-value (≤16 bytes for decimal128) executed once per binding — not per access; no hot-loop cost. It only fires for `-> T errors` wide-value returns, not for int/bool/ptr EC results that fit the ok-word.
- **Auto-promotion analysis**: M3f adds no new language feature, stdlib type, or optimization — it fixes existing codegen. No auto-promotion candidates. (Stated explicitly so reviewers know it was considered, not forgotten.)

### Teaching
- M3f introduces NO new diagnostic, muted-hint domain, or lint (both bugs are silent miscompiles being made correct, not new compile errors). No WHAT/WHAT-INSTEAD/WHY text to add.
- `design/concurrency.md` ECWrapperResultCollection corrected so the design narrative stops claiming a path is complete when it had a hole (teaching-by-honest-docs).

### Runtime Dependencies
- Bug 1 fix: copy-on-bind uses the existing frame/stack allocation already present for EC bindings (the crossing path's companion storage is the template) — no new runtime symbol; reuses `ynz_alloc`/`ynz_free` only if the wide-EC buffer is heap-backed (decide in Phase 2; stack alloca preferred where lifetime permits). No new C-ABI surface.
- Bug 2 fix: frame-slot assignment / bool zext-trunc only — no new runtime symbol.

### Kernel-Mode Behavior
- No change. Both fixes are in the suspension/parallel codegen path which is already unavailable in `--kernel` mode (Tokio-backed scheduler rejected per the roadmap's no-kernel-concurrency constraint). `--kernel` programs cannot reach either bug site (they can't use `wait`/`background`). No new `--kernel` diagnostic.

### Demo & Error Gallery
- **`examples/pirates-roster/entrypoint.ynz`**: extend with a section that exercises BOTH fixed patterns in realistic context — (a) two same-callee wide-EC price lookups returning distinct decimal128 values; (b) a parallel group pairing an int result with a boolean flag, both crossing a `wait`, used afterward — and assert correct values via `expected_stdout.txt`. This is the human-eyes-on proof the silent-wrong values are gone.
- **`examples/primantis-orders/` error gallery**: **NO new file/entry.** M3f introduces no new compile-error class — both bugs were SILENT (no diagnostic) and remain non-erroring once fixed (they just produce correct values). The gallery is for compile errors; there is nothing to trigger. This is the documented "deliberate omission with rationale" the invariant permits — recorded here so reviewers see it was considered, not forgotten.
- Both new behaviors get permanent regression fixtures under `crates/ynz-driver/tests/fixtures/v0_3_m3f_*.ynz` with insta stdout snapshots + the cross-impl (`default == --no-auto-parallel`) assertion. The fixtures ARE the automated net the demo can't replace.

### Feature Registry Entries
- **None.** M3f adds no new keyword, banned-jargon word, primitive intrinsic, type-attached constant, deferred feature, diagnostic template, or muted-hint domain. Stated explicitly so reviewers know it was considered. (It REMOVES nothing from the registry either; the `ec-wrapper-collect-on-completion` deferral stays — it's the `background`-path sibling, still gated on background-handle collection, distinct from Bug 1's inline-poll path.)

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each phase is one PR via `/pr`; the milestone is 4 phases / ≤4 PRs, sized per `branching.md`. Not one giant uncommitted blob.
- **Shadow main branches**: single feature branch `fix/m3f-codegen-correctness` off `main@d24a5f4`; merged via `/pr` → main, no long-lived parallel main.
- **Building the engine before shipping value**: each fix phase delivers a standalone correctness win (Phase 2 alone makes wide-EC bindings correct; Phase 3 alone restores cross-impl). No infrastructure-before-value.
- **Hotfix that isn't**: these ARE confirmed silent miscompiles with live repros + Paper-Trace, not speculative. Phase 1 locks the repros RED before any fix.
- **Abandoned branches**: branch merges at milestone end; `/release v0.3.0-m6` cuts the tag; branch deleted post-merge.
- **Flag graveyards**: no feature flags — correctness fixes ship unconditionally (a wrong-answer fix is not gated). `--no-auto-parallel` is a pre-existing test oracle, not a new flag.

## Phase Execution Protocol

Each phase ends with an **Exit Sequence** block — run those instructions at every phase boundary (persist plan state → persist deviation scratch → fan out all reviewers + N deviation-judges in parallel → coordinator writes Evidence + Phase Review Gates → handle verdicts → prompt commit). Canonical fan-out spec: `~/.claude/commands/execute-plan.md` Step 3.d–3.h (referenced, not duplicated). BASE resolution per phase: Phase 1 BASE = `plan_base` (`d24a5f4`); Phase N≥2 BASE = prior phase's committed SHA. Diff-form selection: `git diff <BASE>` if `git status --porcelain` reports any uncommitted change, else `git diff <BASE>..HEAD`.

**Final phase additionally**: verify all phases' AC + quality-gate checkboxes; fan out the full reviewer-and-judge set with `model: "opus"` against the cumulative plan diff (`git diff <BASE>` form-selected); flip `status: active` → `done` only after all reviewers AND cumulative judges PASS; then STOP and present `/release v0.3.0-m6` for Patrick's go (do not auto-release per standing instruction).

## Phases

### Phase 1: Reproduce both silent miscompiles in failing tests (RED)
**PR scope**: Add two RED fixtures + two integration assertions that FAIL on `d24a5f4`. No fix.
**Branch**: `fix/m3f-codegen-correctness`
**Flag**: N/A
**Est. lines**: ~120 (2 fixtures + 2 tests + harness wiring)
**Ships via**: `/pr`
**Objective**: lock both bug contracts with reproductions that fail on the current commit and will pass once Phases 2 & 3 land. Externalize the Paper-Trace for each before writing the assertions.
**Why this phase exists**: per the bug-fix discipline (`verification.md` + plan skill) — you cannot weaken a test you wrote 30 seconds ago to reproduce a bug. The RED tests are the binding contract the fixes must satisfy.
**Current-state anchors**:
- `crates/ynz-driver/tests/fixtures/` — fixture dir; convention `v0_3_m3{x}_*.ynz`.
- `crates/ynz-driver/tests/integration.rs` — integration harness + the existing cross-impl consistency test (the `--no-auto-parallel` oracle lives here).
- `YNZ_NO_AUTO_PARALLEL=1 ./target/debug/ynz run <file>` is the verified sequential oracle (the `--no-auto-parallel` flag is on `ynz build`; the env var works on `ynz run`).
**Files (expected scope)**: 2 new fixtures (`v0_3_m3f_ec_same_callee_aliasing.ynz`, `v0_3_m3f_parallel_group_bool_sibling.ynz`); `crates/ynz-driver/tests/integration.rs`.
**Deviation rule**: may touch adjacent harness helpers if needed to assert both-mode equality; document each deviation with a one-line reason. Unrelated cleanup → separate PR.
**Steps**:
1. **Paper-Trace Bug 1**: Observed `31.75`/`31.75` (both modes). Expected `24.50`/`31.75` (each binding holds its own call's value — value semantics; same-callee calls are independent). Residual: `p1` reads `p2`'s value. Hypothesis: ok-word points into the reused 16-byte staging slot (`emit.rs:220-223`, `bind_sm_return_value` EC arm `~5059`). Evidence path: `emit.rs:5059` non-crossing EC bind stores struct without copy-out; `state_machine.rs:387` returns staging-slot pointer.
2. Write `v0_3_m3f_ec_same_callee_aliasing.ynz` (the verified repro: `fetchPrice(which: int) -> number errors` with `if (which == 0) return 24.50` else `return 31.75`; bind `p1=fetchPrice(0)`, `p2=fetchPrice(1)`; print both via `.or(0.0).toString()`). Add `// RED until Phase 2 (M3f) — wide-EC same-callee staging-slot aliasing` header comment.
3. **Paper-Trace Bug 2**: Observed `default 0` / `nopar 42`. Expected `42` both modes (binding survives suspension; `default == --no-auto-parallel`). Residual: grouped int sibling reads `0` when a bool group-result is live across the later wait. Hypothesis: bool crossing-local i1→i64 flush/reload (`sm_crossing_bool_set`, `emit.rs:1455-1458`) corrupts the int sibling's frame slot. Evidence path: bisection table (Research Findings) isolates `{group}×{bool result used after wait}×{subsequent wait}`.
4. Write `v0_3_m3f_parallel_group_bool_sibling.ynz` (MIN-1: `let a = slowCall()` int + `let cond = other()` boolean, `wait sleep(1)`, `if (cond) { print(a.toString()) }`). Add `// RED until Phase 3 (M3f) — parallel-group bool sibling frame-slot corruption` header comment.
5. Add `integration.rs::v0_3_m3f_ec_same_callee_aliasing_distinct_values`: assert default-mode stdout == `"24.50\n31.75\n"` (derive expected from value semantics, NOT current output).
6. Add `integration.rs::v0_3_m3f_parallel_group_bool_sibling_survives_wait`: assert default-mode stdout == `"42\n"` AND default-mode stdout == nopar-mode stdout (cross-impl).
7. Run both → confirm RED with the documented residuals (`31.75/31.75`; `0` vs `42`).
**Acceptance criteria**:
- [ ] Two fixtures exist at `crates/ynz-driver/tests/fixtures/v0_3_m3f_*.ynz` with RED-until-Phase-N header comments
  - Evidence: (filled at phase completion)
- [ ] `v0_3_m3f_ec_same_callee_aliasing_distinct_values` FAILS on `d24a5f4` with observed `31.75\n31.75` vs expected `24.50\n31.75`
  - Evidence: (filled at phase completion)
- [ ] `v0_3_m3f_parallel_group_bool_sibling_survives_wait` FAILS on `d24a5f4` with default `0` ≠ expected `42` AND default ≠ nopar
  - Evidence: (filled at phase completion)
- [ ] Each test's expected value is derived from a stated semantic rule (value-binding / suspension-preservation / cross-impl), not from running the current code
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Fixtures use only real Yinz operations in current scope (no invented APIs)
- [ ] Expected outputs derived from spec/semantics, not current behavior
- [ ] Both tests are RED (fail) on the base commit, demonstrably for the documented reason
- [ ] No fix code in this phase
**Verification**: `cargo test -p ynz-driver --test integration v0_3_m3f` → both new tests FAIL with the documented residuals; `git diff` touches only fixtures + integration.rs.

**Phase Review Gates** (filled at phase completion by coordinator):
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS:**
1. Pre-reviewer bookkeeping only (tick Quality gate items the diff verified; bump `last_updated:`; do NOT pre-tick ACs/Evidence/Phase Review Gates).
2. Resolve `$BASE` = `d24a5f4` (plan_base). Diff-form select. Fan out the 5 reviewers + 0 deviation-judges (unless executor documents deviations) per `~/.claude/commands/execute-plan.md` Step 3.d. **This phase's RED tests are declared in `## Planned RED Repros`** — a no-duct-tape BLOCK on the intentionally-RED tests is expected; the orchestrator green-lights it (all 4 conditions met).
3. Coordinator writes Evidence + Phase Review Gates from the verifiers' reports.
4. Handle verdicts (fix loop, max 5 rounds).
5. Prompt user with every reviewer verdict individually. "Phase 1 done. Ready to commit and move to Phase 2?"
6. On confirmation: commit, write SHA into Committed.

### Phase 2: Fix Bug 1 — copy-on-bind for wide-EC same-callee returns
**PR scope**: A `-> T errors` binding whose ok-value is too wide for the EC ok-word copies that value into the binding's own stable storage before a subsequent same-callee call can clobber the shared staging slot.
**Branch**: `fix/m3f-codegen-correctness`
**Flag**: N/A
**Est. lines**: ~80-150 (codegen)
**Ships via**: `/pr`
**Objective**: turn the `v0_3_m3f_ec_same_callee_aliasing` RED test GREEN — each wide-EC binding holds its own value — in BOTH modes, with alloc == free.
**Why this phase exists**: closes the HIGH-severity silent-wrong-decimal128 hole; the inline-poll EC path becomes actually "correct and complete."
**Current-state anchors**:
- `emit.rs:5059` `bind_sm_return_value` StructValue (EC) arm — non-crossing bind; **fix site** (stores struct, ok-word still points into staging slot).
- `emit.rs:5316-5320` — non-crossing branch routing to `bind_sm_return_value` without copy-on-bind.
- `emit.rs:5122-5198` `bind_sm_result_and_flush` EC crossing arm (5154-5190) — **fix template** (already extracts + copies `{i64,i64}` fields into per-binding companion storage).
- `emit.rs:12723-12737` `lower_errors_capable_call_result` (non-SM inline-poll path) — **second fix site** (also stores struct with dangling ok-ptr).
- `emit.rs:214/220-223/338-347/599` — staging slot definition + the comment stating ok-word points into it.
- `state_machine.rs:387-418` `load_return_value_errors` — source of the staging-slot ok pointer.
**Files (expected scope)**: `crates/ynz-codegen/src/emit.rs`; possibly `crates/ynz-codegen/src/state_machine.rs`.
**Deviation rule**: may touch the EC-result helpers if the copy-out is cleanest there; document deviations. A grouping/independence change is OUT of scope here (that's Bug 2) — STOP and split if tempted.
**Steps**:
1. Paper-Trace which storage the ok-word points into for `-> number errors` and exactly when the second same-callee call overwrites it (binary-search with a print of the ok-pointer address if needed).
2. Implement copy-on-bind: when binding an EC result whose ok-value is a wide-value staging-slot pointer (decimal128 / shape — anything not fitting the i64 ok-word), allocate per-binding stable storage and copy the wide ok-value out of the staging slot into it; repoint the binding's ok-word at the per-binding buffer. Mirror the crossing arm's extract+copy (5154-5190). Stack-alloca where the binding's lifetime permits; heap (`ynz_alloc`/`ynz_free`) only if the value must outlive the resume frame (decide from the live-range; prefer stack).
3. Apply to BOTH the SM non-crossing path (`emit.rs:5316`/`bind_sm_return_value`) AND the non-SM inline-poll path (`lower_errors_capable_call_result`). Leave the already-correct crossing path untouched.
4. Verify alloc == free (no leaked per-binding buffer; no double-free of the wide value).
**Acceptance criteria**:
- [ ] `v0_3_m3f_ec_same_callee_aliasing` prints `24.50\n31.75` in BOTH `default` and `YNZ_NO_AUTO_PARALLEL=1` modes (live run)
  - Evidence: (filled at phase completion)
- [ ] A 3+ same-callee-binding variant (`p1=fetchPrice(0); p2=fetchPrice(1); p3=fetchPrice(0)`) yields 3 independent correct values (live run)
  - Evidence: (filled at phase completion)
- [ ] **Failed-branch interleave**: a same-callee mix where one binding takes the `failed` branch (`fetchPrice(2)` → error) and a later same-callee call succeeds — the failed binding surfaces its error and does NOT read the later call's ok-value, and the later ok-binding is unaffected. (The EC struct is `{i64,i64}`; copy-on-bind must be correct for the error word too, not just the ok word.) (live run)
  - Evidence: (filled at phase completion)
- [ ] Both a state-machine caller AND a non-SM inline-poll caller of the wide-EC fn are correct (both fix sites exercised)
  - Evidence: (filled at phase completion)
- [ ] alloc == free on the bug-1 fixtures (no leak, no double-free)
  - Evidence: (filled at phase completion)
- [ ] `cargo test --workspace` green; golden IR snapshots for unaffected fns byte-identical
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Copy-on-bind fires only for wide-value EC ok-words (int/bool/ptr EC results unaffected — no needless copy)
- [ ] No use-after-free / double-free (alloc==free instrumented)
- [ ] No grouping/independence code touched (Bug 2 is Phase 3)
- [ ] Comments are durable (Perf/constraint tier per comments.md), no changelog phrasing
- [ ] Follows the crossing-arm copy pattern, not a parallel reinvention
**Verification**: run both fixtures both modes; run a 3-binding variant; `cargo test --workspace`; alloc==free check on the fixtures.

**Phase Review Gates** (filled at phase completion by coordinator):
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS:**
1. Pre-reviewer bookkeeping only.
2. `$BASE` = Phase-1 committed SHA. Diff-form select. Fan out 5 reviewers + N deviation-judges per `/execute-plan` Step 3.d. (Memory-safety judges should probe double-free / cancellation / N-binding aliasing live, per the M3b precedent for EC-copy fixes.)
3. Coordinator writes Evidence + Phase Review Gates.
4. Handle verdicts (max 5 rounds). **Re-verify the ORIGINAL repro + the extended N-binding sweep after every "fixed"**, not just the minimal case (per `reverify-original-repro`).
5. Prompt user with every verdict individually. "Phase 2 done. Ready to commit and move to Phase 3?"
6. On confirmation: commit, write SHA.

### Phase 3: Fix Bug 2 — parallel-group result frame-backing across a subsequent suspension
**PR scope**: A parallel-group result binding survives a subsequent `wait` regardless of the group's member types — a boolean group-result live across the wait no longer corrupts a sibling result's frame slot. Restores `default == --no-auto-parallel`.
**Branch**: `fix/m3f-codegen-correctness`
**Flag**: N/A
**Est. lines**: ~60-150 (codegen)
**Ships via**: `/pr`
**Objective**: turn the `v0_3_m3f_parallel_group_bool_sibling` RED test GREEN — grouped int+bool results both survive the later wait, `42` in BOTH modes — and restore cross-impl consistency across the full trigger matrix.
**Why this phase exists**: closes the mode-divergent auto-parallel soundness hole that breaks M3b's load-bearing `default == --no-auto-parallel` invariant. A released version (`v0.3.0-m5`) currently miscompiles this; M4 must branch off a clean base.
**Current-state anchors**:
- `check.rs:6527` `crossing_local_names` → `:6533` `locals_crossing_wait` — confirm parallel-group result bindings live across a later wait ARE in the crossing set (they should be; the corruption is in materialization, not detection).
- `emit.rs:1455-1458` `sm_crossing_bool_set` — i1 alloca; flush zext i1→i64, reload trunc i64→i1. **Prime corruption suspect.**
- `emit.rs:3363` `flush_var_slot_to_frame` + the slot-index assignment (int/bool = 1 slot; decimal128/EC = 2 slots, `emit.rs:1460-1464`/`2247-2249`) — **suspect: slot-index shift when a bool slot is interleaved among mixed-width group results.**
- `emit.rs:5122-5198` `bind_sm_result_and_flush` — the parallel-group result binding path.
- `crates/ynz-codegen/src/independence.rs` — the grouping decision. **DO NOT MODIFY to "fix" by refusing to group bool-containing pairs.** The grouping is correct; that would be `no-duct-tape.md` duct tape (disabling a correct optimization to dodge a codegen bug).
**Files (expected scope)**: `crates/ynz-codegen/src/emit.rs`; possibly `crates/ynz-codegen/src/state_machine.rs` and/or `crates/ynz-typeck/src/check.rs` (only if the crossing-set membership is the gap).
**Deviation rule**: may touch the slot-assignment + bool flush/reload helpers; document deviations. Touching `independence.rs` to suppress grouping is an out-of-scope concern — STOP.
**Steps**:
1. Paper-Trace the exact slot the int sibling reads from when it returns `0`. Instrument: print the slot index assigned to `a` (int) and `cond` (bool), and the offset each flush/reload uses. Bisect the matrix (int+int ✓, int+bool-unused ✓, int+bool-used ✗) to confirm whether the bug is (a) the bool zext/trunc writing the wrong slot, (b) a slot-index shift when a 1-slot bool is interleaved, or (c) the int sibling never being frame-backed in the grouped+bool case.
2. Fix the proven mechanism so each group-result gets a correct, non-overlapping frame slot and the bool's zext/trunc reads/writes ITS slot only. Do not reorder or skip the int sibling's flush/reload.
3. Confirm the auto-parallel headline still works: the two statements still overlap (independent interleaved poll), not serialized.
**Acceptance criteria**:
- [ ] `v0_3_m3f_parallel_group_bool_sibling` prints `42` in BOTH modes AND `default == --no-auto-parallel` (live run)
  - Evidence: (filled at phase completion)
- [ ] Full trigger matrix correct in both modes (live runs): int+int (top + nested use), int+bool (bool used after wait — top + nested), **bool-first then int (reverse member order — flushes out any slot-ordering-matches-declaration-order assumption)**, bool+bool, ≥3 group members, a `number`/decimal128 sibling, and **a `-> number errors` wide-EC result (2-slot) paired with the bool (1-slot) both crossing the same later wait (the 2-slot×1-slot interleaving — most likely slot-index-shift hiding spot — with an explicit expected decimal asserted)**
  - Evidence: (filled at phase completion)
- [ ] The previously-passing cases (B, C, H1, H2, H3, data-dep from the Research matrix) still pass — no regression
  - Evidence: (filled at phase completion)
- [ ] The M3b overlap-timing fixture still shows ≈max-not-sum (headline preserved — grouping not suppressed)
  - Evidence: (filled at phase completion)
- [ ] `independence.rs` unchanged (or, if touched, justified as NOT grouping-suppression); `cargo test --workspace` green
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Fix is in frame-slot materialization, NOT grouping suppression (no-duct-tape)
- [ ] Each group-result frame slot is non-overlapping and correctly sized (bool=1, int=1, decimal128/EC=2)
- [ ] bool zext/trunc touches only the bool's slot
- [ ] Cross-impl oracle holds on every new + existing fixture
- [ ] Durable comments only (constraint/Perf tier), no changelog phrasing
**Verification**: run the full trigger matrix both modes; run the M3b overlap-timing fixture; `cargo test --workspace`; cross-impl sweep.

**Phase Review Gates** (filled at phase completion by coordinator):
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS:**
1. Pre-reviewer bookkeeping only.
2. `$BASE` = Phase-2 committed SHA. Diff-form select. Fan out 5 reviewers + N deviation-judges per `/execute-plan` Step 3.d. (A deviation-judge should probe whether the fix narrows to bool-only or generalizes; and whether grouping was secretly suppressed.)
3. Coordinator writes Evidence + Phase Review Gates.
4. Handle verdicts (max 5 rounds). Re-verify the ORIGINAL MIN-1 repro + the full matrix + the M3b overlap fixture after every "fixed."
5. Prompt user with every verdict individually. "Phase 3 done. Ready to commit and move to Phase 4?"
6. On confirmation: commit, write SHA.

### Phase 4: Regression fixtures + demo + cross-impl sweep + doc/todos closeout
**PR scope**: Permanent regression net for both fixes; demo extension; design-doc correction; todos/roadmap closeout; cumulative cross-impl sweep; release readiness.
**Branch**: `fix/m3f-codegen-correctness`
**Flag**: N/A
**Est. lines**: ~200 (fixtures + demo + docs)
**Ships via**: `/pr`
**Objective**: lock the matrix as permanent fixtures, prove the fixes in the canonical demo, correct the now-false design-doc claim, close the tracked bugs, and confirm the whole milestone passes the cross-impl gate.
**Why this phase exists**: the Demo & Error Gallery invariant + the documentation-deliverable obligations + the milestone-completion gate.
**Current-state anchors**:
- `crates/ynz-driver/tests/fixtures/` + `integration.rs` — fixture home + cross-impl harness.
- `examples/pirates-roster/entrypoint.ynz` (879 lines, single-entry) + `expected_stdout.txt` + `expected_stdout.txt.regenerate.sh` + `services/` — the canonical demo.
- `examples/primantis-orders/` — error gallery (NO new file — M3f adds no compile-error class).
- `design/concurrency.md:450-464` ECWrapperResultCollection — the claim to correct.
- `.claude/todos.md:178` (Bug 2) + `:180` (Bug 1) + `:170` (aliased-shape, stays open); `.claude/plans/roadmaps/v0-3-concurrency-perf.md` M3f section.
**Files (expected scope)**: fixtures + `integration.rs`; `examples/pirates-roster/entrypoint.ynz` + `expected_stdout.txt`; `design/concurrency.md`; `.claude/todos.md`; roadmap file.
**Deviation rule**: standard; document deviations.
**Steps**:
1. Promote the Phase-3 trigger matrix into permanent fixtures (`v0_3_m3f_parallel_group_*` for int+int, int+bool top/nested, bool-first-then-int, bool+bool, 3-member, and a concrete `-> number errors` 2-slot result paired with the bool 1-slot crossing the same wait) with insta stdout snapshots + cross-impl assertions. Add `v0_3_m3f_ec_*` fixtures for Bug 1: the multi-binding ok-path variant AND the failed-branch-interleave variant (one binding errors, a later same-callee call succeeds).
2. Extend `examples/pirates-roster/entrypoint.ynz` with a section demonstrating BOTH fixed patterns in realistic context (two same-callee wide-EC price lookups returning distinct decimal128 values; a parallel group pairing an int with a bool flag, both crossing a `wait`, used after). Regenerate `expected_stdout.txt` via the regenerate script; verify the runtime output matches.
3. Gallery: add NO new `primantis-orders` file — record in the PR description + a one-line note in the gallery README why (M3f introduces no new compile-error class; both bugs are silent miscompiles made correct).
4. Correct `design/concurrency.md` ECWrapperResultCollection (≈line 458): amend "the inline-poll path … is correct and complete" to note the same-callee wide-EC reuse hole existed and was fixed in M3f (cross-ref the copy-on-bind fix); keep the `background`-wrapper deferral text (still gated on background-handle collection).
5. Close `.claude/todos.md:178` + `:180` (tick + date + "shipped M3f"); mark roadmap M3f "Tracked bugs" 1 & 2 shipped; leave `:170` (aliased-shape) open with its m3c deferral. **When closing :178, state the reframe explicitly** so nobody reconciling the roadmap thinks a crash was fixed: ":178 described an LLVM 'does not dominate all uses' compile crash; that symptom no longer reproduces on `main` (the crossing-local read-scan already recurses — incidentally closed by earlier M3 work). What M3f actually fixed in this area is the live mode-divergent silent miscompile (parallel-group bool sibling reads 0; default ≠ --no-auto-parallel) that the same root area surfaced. Closing :178 as the area's tracked bug, fixed via the Bug-2 reframe."
6. Cumulative cross-impl sweep: assert `default == YNZ_NO_AUTO_PARALLEL=1` across ALL fixtures + the existing suite; `cargo clippy --workspace -- -D warnings`; `cargo fmt --all --check`; jargon audit.
**Acceptance criteria**:
- [ ] Permanent regression fixtures for the full Bug-2 matrix + Bug-1 multi-binding exist with insta snapshots + cross-impl assertions; all pass both modes (live)
  - Evidence: (filled at phase completion)
- [ ] `pirates-roster/entrypoint.ynz` demonstrates both fixed patterns; `expected_stdout.txt` regenerated and matches the live run
  - Evidence: (filled at phase completion)
- [ ] `design/concurrency.md` ECWrapperResultCollection claim corrected (inline-poll path hole + M3f fix noted)
  - Evidence: (filled at phase completion)
- [ ] todos.md :178 + :180 closed; roadmap M3f bugs 1&2 marked shipped; :170 left open with m3c deferral
  - Evidence: (filled at phase completion)
- [ ] Cumulative `default == --no-auto-parallel` holds on every fixture; `cargo test --workspace` green; clippy/fmt/jargon clean
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Every new fixture has a cross-impl (`default == --no-auto-parallel`) assertion
- [ ] Demo uses only real Yinz operations; expected_stdout regenerated, not hand-edited
- [ ] Design-doc correction is durable (states current reality), not a changelog note
- [ ] Aliased-shape bug (:170) NOT silently dropped — stays tracked with trigger
- [ ] No new compile-error gallery entry (documented why)
**Verification**: full `cargo test --workspace`; cross-impl sweep script; `examples/pirates-roster` run vs `expected_stdout.txt`; clippy/fmt/jargon.

**Phase Review Gates** (filled at phase completion by coordinator):
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS (FINAL PHASE):**
1. Pre-reviewer bookkeeping; verify ALL phases' AC + quality-gate checkboxes accurate; update the Quality Checklist below.
2. `$BASE` = `d24a5f4` (plan_base — CUMULATIVE). Diff-form select (`git diff <BASE>` if any uncommitted, else `git diff <BASE>..HEAD`). Fan out all 5 reviewers + D_cumulative deviation-judges with `model: "opus"` against the cumulative diff per `~/.claude/commands/execute-plan.md` Step 4.a.
3. Coordinator writes Evidence + Phase Review Gates from the reports.
4. Handle verdicts (max 5 rounds).
5. All reviewers + cumulative judges PASS → flip `status: active` → `done` in front-matter.
6. Prompt user with every verdict individually. "M3f complete — all 4 phases green, cumulative opus sweep PASS. Ready to `/release v0.3.0-m6`?" **STOP — do not auto-release** (standing instruction). Present for Patrick's go.

## Quality Checklist (verify at completion)
- [ ] Both RED repros written first (Phase 1) and turned GREEN by their fixing phases
- [ ] Bug 1: wide-EC same-callee bindings hold distinct correct values; alloc==free; both fix sites (SM + inline-poll)
- [ ] Bug 2: parallel-group results survive a subsequent wait; `default == --no-auto-parallel` restored; headline overlap preserved
- [ ] No grouping-suppression duct tape (independence.rs intact or justified)
- [ ] Cross-impl consistency holds on every fixture (the M3b invariant)
- [ ] Types complete (no `any`-equivalent; Rust types proper); arrow/style rules N/A (Rust crate)
- [ ] Existing tests still pass (`cargo test --workspace`); golden IR byte-identical for unaffected fns
- [ ] clippy `-D warnings` + `cargo fmt --check` + jargon audit clean
- [ ] Demo (`pirates-roster`) extended + expected_stdout regenerated; no new error-gallery entry (documented why)
- [ ] `design/concurrency.md` ECWrapperResultCollection corrected; todos :178/:180 closed; :170 left open (m3c)
- [ ] Every phase received all-reviewer + all-judge PASS before committing (Step 9a)
- [ ] Final cumulative opus reviewer sweep passed (Step 10f)
- [ ] Plan-file AC checkboxes accurate across all phases (Step 9b)
- [ ] `/release v0.3.0-m6` presented to Patrick (NOT auto-run)
