---
slug: v0-3-m3d-cpu-parallelization
type: execution
owner: Patrick Rizzardi
status: active
roadmap: v0-3-concurrency-perf
depends_on: [v0-3-m3b-auto-parallelization]
plan_base: 8b99ac9c626468ff7ce7c123f2e061e3a8df22cd
files:
  - crates/ynz-runtime/src/**
  - crates/ynz-typeck/src/**
  - crates/ynz-codegen/src/**
  - crates/ynz-lsp/src/**
  - crates/ynz-driver/**
  - registry/features.toml
  - design/concurrency.md
  - spec/concurrency.md
  - examples/pirates-roster/**
  - examples/primantis-orders/**
  - tooling/vscode-ynz/**
created: 2026-06-11
last_updated: 2026-06-11
---

# Plan: v0.3-M3d — Pure-CPU Statement Parallelization

Created: 2026-06-11
Status: approved — in execution (Phase 0, ROUND-8 re-gate in flight 2026-06-12 — Patrick authorized round 8 at arbitration #3 ("Round 8: one-clause fix (Recommended)"). Round-7 closed 3 PASS / 1 BLOCK (judge #15 silent-narrowing — see Findings Log 16:30). Round-8 executor applied judge #15's named fix exactly: stripped `stmt_contains_wait` from the POST-PAIR check at all three gate sites (emit.rs:6574/6752/6940), keeping only `stmt_contains_suspending_call`; pre-pair gates unchanged (ISSUE-A); stale prune comment at emit.rs:2613 rewritten; fixture headers verified accurate. FIRES table IR-verified: (g)/(h)/(i)/(j)/(n) went 0→2 `ynz_rt_spawn_blocking_joinable` call instructions each — the cross-suspension reload machinery is LIVE again; (m)/(p)/(q) still decline at 0. All 17 fixtures byte-identical both modes, full workspace green, clippy/fmt clean, zero new deviations. Scratch deviation #15 amended (round 8) with recomputed identity hash fdc8dc2. Post-R8 tree pre-verified (delegation intact, 9/9 m3d tests green, no probe artifacts). ROUND-8 re-gate CLOSED ALL GREEN 2026-06-12T17:03: judge #15 PASS (fresh adversarial inputs on the narrowed predicate all held) + acceptance-verifier PASS (all 4 ACs MET on own live runs; FIRES independently IR-verified; 1924 workspace tests green). Phase 0 gate fully green after 8 rounds. Plan writeback complete: 4 ACs ticked with R8 evidence, Phase Review Gates ticked (5 reviewers + 17 judges at latest verdicts; #1/#11 deferred to cumulative 4.a per authorization), FIRES/DECLINES envelope table added to AC1 evidence, fixture (m) note corrected. Phase 0 COMMITTED dcc1432 (2026-06-12, Patrick-confirmed; 38 files, +4067/-45, explicit-path staging). Post-phase /learn completed (2026-06-12): new graveyard corpse "Silent Envelope Narrowing" in .claude/graveyard.md (UNCOMMITTED — rides with next commit; Bouncer checks: decline-widening needs fire-evidence, fixture FIRE claims verified vs IR, gated-path plans need FIRE/DECLINE envelope table) + generic principle "Assert the Mechanism Fired" in engineering-principles.md + project memory gated-path-fire-assertions + mutation-probe serialization folded into reviewer-judge-read-only-git memory. Next: Phase 1 via /execute-plan in a future session (phase-by-phase mode); P1 plans MUST declare the spike admission envelope as a FIRE/DECLINE table up front per the new corpse. Cumulative 4.a at end-of-plan re-judges all 17 deviations incl. the #1/#11 deferred re-judges. P1 carry-forwards logged in plan: end-to-end C-unwind resume-fn ABI; emit_suspending_call_heap_boxed stale frame_layouts; null-from-spawn silently skips; spike-on oracle is manual-only (needs CI gate when productionized).)

## Context & Why

**Goal**: Two (or more) independent heavy pure-CPU operations in a function body run on separate cores — real multicore speedup — with zero user syntax and byte-identical results to `--no-auto-parallel`. This is the "your code just got faster" win for compute-bound code, complementing M3b's I/O-overlap win.

**Why**: M3b shipped I/O-overlap auto-parallelization by INTERLEAVING suspensions on one thread (zero new runtime — both child sub-frames embed in the parent frame and inline-poll). CPU work doesn't yield, so overlapping it needs ACTUAL extra threads with a joinable handle. The existing spawn ABI cannot provide that: `ynz_rt_spawn` (`runtime.rs:591`) and `ynz_rt_spawn_blocking` (`runtime.rs:140`) both return void / fire-and-forget. A pure-CPU function isn't a state machine today, so it can't await a join either. This was SPLIT out of M3b on 2026-06-05 precisely because it is genuinely-new runtime + codegen work sitting one wrong implementation choice (a synchronous join) from the `block_on` M2-HALT corpse.

**Background — what exists today**:
- M3b's independence analysis (`crates/ynz-codegen/src/independence.rs`) partitions straight-line statement lists into `Singleton` / `Parallel` groups with class-agnostic data-dep + type-based write-effect criteria — but candidacy criterion 3 currently requires all group members to be SUSPENDING calls to DISTINCT callee names.
- The interleaved inline-poll join (`emit.rs:6061` `emit_independent_group_poll`) polls embedded child frames; it has no concept of a thread-spawned child.
- The SM-lowering trigger is `suspend_set.contains(&f.name)` (`emit.rs:1847` → `lower_function_with_waits` at `emit.rs:2063`); the suspend set is computed by typeck's may-block fixpoint (`may_block.rs:96 analyze`) and carried cross-module via `FunctionSig.suspends` (`signatures.rs:38`).
- The sleep-handle poll protocol (`ynz_rt_async_sleep_create` → handle stored in frame slot → `ynz_rt_async_sleep_poll(handle, waker_ctx)` until Ready, freed-on-Ready, freed-by-drop-shim on cancellation) is the proven precedent the joinable CPU spawn mirrors.

**Constraints** (from the roadmap + design docs, all locked):
- No new user-facing syntax. Existing programs produce IDENTICAL output (`--no-auto-parallel` is the oracle).
- The join MUST be a poll-based suspension through the existing waker protocol — a synchronous join (`block_on`, thread-park) is the M2-HALT corpse and is banned at every call path.
- No function coloring (design/future/concurrency.md "Invariant"); promotion must be invisible at signature level.
- Auto-promotion must never turn previously-valid code into a compile error (`.claude/rules/auto-promotion.md` — promotion only when the compiler can PROVE the fit).
- Adversarial gate mandatory per the roadmap milestone entry.

**Success criteria**: a demo with two independent heavy CPU calls (`let a = work(40)` / `let b = work(41)`) measurably overlaps on two cores (wall-clock < sequential), byte-identical output vs `--no-auto-parallel`, zero regressions across the full fixture corpus, full teaching surface shipped (registry + LSP hint + VSCode + demo), released as `v0.3.0-m7`.

## Research Findings

1. **Tokio 1.49 (verified via context7, 2026-06-11)**: `tokio::task::spawn_blocking(closure) -> JoinHandle<R>`. `JoinHandle<T>` implements `Future<Output = Result<T, JoinError>>`; polling registers the real waker (no fabricated wakers needed — same forwarding discipline as the sleep poll). **`spawn_blocking` tasks cannot be aborted once started** — `JoinHandle::abort()` only prevents not-yet-started tasks; dropping the handle DETACHES (task runs to completion). Pool saturation (default `max_blocking_threads` = 512) QUEUES additional `spawn_blocking` calls — they run as threads free up. Panic inside the closure is caught and surfaces as `JoinError::is_panic()` on the handle.

2. **Deadlock-safety consequence**: because the join is an inline poll that returns `Pending` up to the scheduler (never holds a thread), pool saturation cannot deadlock the joiner — the joining state machine is suspended, not parked on a pool thread; queued spawns always eventually run. The ONLY way to deadlock is a synchronous join — which is banned (corpse). The stress fixture still proves this empirically.

3. **Cancellation + borrowed heap args = UAF hazard**: if a parent frame is dropped mid-join (task cancellation), the detached `spawn_blocking` child keeps running. A string argument passed by pointer into the child would dangle the moment the parent frame's drop frees it. **Fix: reuse M1's existing arg-copy + `BgArgDropEntry` machinery** (`runtime.rs:89` ctx-copy region; `ynz_rt_spawn`'s `arg_drop_ptr` discipline) — heap args are COPIED into the spawned task's ctx; the child owns its copies; nothing dangles regardless of cancellation timing.

4. **SM-promotion can transitively break compiles — must decline, never error**: promoting non-suspending `F` to a state machine makes every caller of `F` a state machine too (normal may-block propagation). Any newly-SM function can then trip the existing suspension guards on previously-compiling code: `ShadowsCrossingLocal`, `UnsupportedCrossingLocalType` (union/maybe/dynamic/fixed/range/MapEntry crossing the join), `WideValueSuspendingReturn` (`-> Shape` / `-> Shape errors`), `MutualSuspensionCycle`, `FixedArrayIterWithWait`, `StoredRangeWithWait`, `ExpressionIterWithWait`, `ArrayShapeRuntimeFieldWithWait`. The promotion pass MUST probe all of these across the full transitive closure of newly-SM functions and ROLL BACK any promotion whose propagation would fire a guard. Decline = sequential lowering (silent perf opt-out), NEVER a new compile error.

5. **Suspend-set single source of truth**: registry entries for `wait_points` (`features.toml:2078`) and `background_routing` (`features.toml:2086`) explicitly promise "the same set that drives codegen routing, so the hint and the binary always agree." Promotion therefore must be computed in TYPECK (extending the suspend set `may_block.rs` produces) so guards, inlay hints, cross-module `FunctionSig.suspends`, and codegen all agree. Consequence: the independence partition (currently `ynz-codegen/src/independence.rs`, deps only on `ynz_ast` + `ynz_typeck` types) moves into `ynz-typeck`; codegen consumes it from there. Not a parallel implementation — a relocation.

6. **Same-callee CPU calls CAN parallelize** (unlike M3b's I/O members): the I/O same-callee restriction exists because composed sub-frames are keyed by callee NAME in `build_frame_layouts_with_resolver` (`emit.rs:254`). CPU children don't embed a child frame — each spawn gets its own heap ctx, and the parent frame needs only a handle slot + result slot PER GROUP-MEMBER INDEX. `let a = fib(40); let b = fib(41)` — the single most common CPU-parallel pattern — works. This is also the headline demo.

7. **Result ABI**: 16 bytes covers every supported return class — i64 scalars (int/bool), f64 (float), heap pointer (string/array/map), i128 (number/decimal128), `{i64,i64}` EC pair (`-> T errors`). Closure returns a 16-byte POD; `ynz_rt_join_poll(handle, waker_ctx, result_out: *mut u8) -> i32` writes it on Ready. `-> Shape` / `-> Shape errors` are DECLINED (consistent with `WideValueSuspendingReturn` — the non-suspending shape-return base bug is a named pre-existing deferral, not this milestone's to fix).

8. **Panic semantics locked by the identical-output gate**: sequential execution of a panicking CPU callee aborts the program. Parallel execution must match → on `ynz_rt_join_poll` Ready(panic), the parent RE-RAISES (aborts with the same diagnostic path sequential execution takes). Best-effort-discard applies to `background` (fire-and-forget) only — a JOINED result is load-bearing, discard would be a silent wrong answer.

9. **Worth-it heuristic**: spawn ONLY when the callee transitively contains a loop or recursion (proxy for "does real work"); trivial/leaf/arithmetic calls run inline. Implemented as a `does_real_work` fixpoint beside `may_block.rs`'s call-graph walker, carried cross-module via a new `FunctionSig` field (rides the existing `module_signatures_query` propagation exactly like `suspends` — building intra-module-only and retrofitting later would be build-twice). The benchmark-calibrated cost threshold that refines this proxy is this milestone's own perf-spike (Phase 5), per the roadmap ("ship a safe constant, calibrate with evidence" — same protocol as auto-SoA's SIZE_THRESHOLD).

10. **stdout interleaving inside parallel members is locked Model A behavior** (design/concurrency.md "Suspension vs. Ordering", LOCKED 2026-06-05): independent side effects may race; ordering is the user's job via `wait`. Fixtures must assert via aggregates or post-join prints — never interleaving-dependent output. Same policy M3b shipped under.

11. **`wait_on_non_may_block` diagnostic becomes stale** (`features.toml:1553`): its why-text says `wait` on a CPU-bound callee has "identical runtime semantics" — after M3d, an explicit `wait` before a CPU call in a promoted function is a real ORDERING BARRIER (resets the parallel group, per the partition's existing barrier rule). Text must be reworded in Phase 4.

## Design-Doc Alignment

**Governing docs read in full before this plan was drafted**:
- `design/concurrency.md` `[locked]` — auto-parallelization model, Model A ordering semantics, loop-iterations-sequential, M3b design divergences (type-based write-effect floor, same-callee), permanent positional constraints.
- `design/future/concurrency.md` `[locked]` — no-coloring invariant, CPU-bound task routing ("tasks whose call graph contains zero may-block calls route to a separate blocking thread pool"), Task Cancellation ("CPU-bound tasks have no `wait` points, so cancellation cannot be injected mid-computation… the same behavior as Rust's `spawn_blocking`" — documented constraint, not a gap), Scheduler Preemption Model, work-stealing scheduler lock.
- `design/ide-hints.md` `[locked]` — Informational placement category for the new hint domain.

**Conformance statement**: this plan implements exactly the documented model — poll-based suspension at the join (no coloring, no bridge), blocking-pool routing for zero-may-block work, detach-on-cancel for in-flight CPU tasks, IDE visibility of compiler scheduling decisions ("IDE Execution Plan — Non-Negotiable" in `design/concurrency.md`).

**Divergences carried forward from M3b (already recorded in `design/concurrency.md` "Design Divergences", not new)**: (a) type-based write-effect floor — transfers unchanged to CPU candidacy per the doc's own note ("M3d CPU-parallel reuses this same type-based write-effect source — the floor transfers unchanged"); (b) same-callee sequential — REMAINS for suspending (I/O) members; this plan LIFTS it for CPU members only (per-invocation ctx, no name-keyed sub-frame — see Research Finding 6). The doc's divergence entry gets a scope amendment in Phase 4.

**Milestone-boundary assumptions**: `background` handle-form, channels, auto-Arc, auto-SoA are M4 (roadmap-documented). The borrow-checker narrowing of the write-effect floor is M4 (doc-documented reversal path). Nothing this plan defers is load-bearing for M3d's correctness.

**One gap being closed beyond the milestone's nominal scope, surfaced loudly**: `design/concurrency.md` "IDE Execution Plan — Non-Negotiable" requires the IDE to show which operations run concurrently. M3b shipped `wait_points` + `background_routing` but NO hint at parallel-GROUP sites (verified: no such pass in `inlay_hint_passes.rs`, no registry domain). M3d's new `parallel_groups` Informational hint domain covers BOTH classes (M3b's I/O-overlap groups AND M3d's CPU groups), closing the gap rather than widening it. This is a deliberate scope addition (~1 inlay pass + registry entry + LSP wiring) — flagged in Questions for explicit approval.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| A synchronous join sneaks into ANY path (the `block_on` M2-HALT corpse) | Medium | Critical | P0 spike validates poll-based join end-to-end through the REAL compiler before production code; P5 adversarial gate stress-runs every join path; grep-able invariant: no new `block_on`/`Handle::block_on`/`thread::park` call sites in the diff outside the existing main→entrypoint driver |
| SM-promotion fires an existing suspension guard on previously-compiling code (silent compile break) | High (without the decline design) | Critical | Decline-to-promote with transitive rollback (Research Finding 4) is a named P2 deliverable with dedicated tests: every guard-triggering shape compiled BOTH before and after M3d with identical results |
| Cancellation mid-join dangles a borrowed heap arg in the detached child (UAF) | Medium | Critical | Heap args copied into ctx via existing `BgArgDropEntry` machinery (Research Finding 3); P5 cancellation fixtures run under the alloc=free accounting harness |
| Promotion fixpoint diverges or is order-dependent (nondeterministic builds) | Medium | High | P0 locks the algorithm: intrinsic-rooted may-block fixpoint FIRST, then bottom-up promotion pass in deterministic call-graph order, then guard-probe rollback loop (monotone downward — converges); salsa-tracked so it's also incremental-safe |
| Blocking-pool exhaustion deadlock (N spawns ≫ pool, or recursion spawning groups) | Low (poll-join makes it structurally impossible) | High | Poll-based join never holds a thread (Research Finding 2); P5 stress fixture: recursion fan-out ≫ 512 spawns must complete |
| Same-callee per-invocation keying corrupts frame slots (the M3b name-keying assumption violated) | Medium | High | P0 spike includes the same-callee case explicitly; P3 keys handle/result slots by (group, member-index), never by callee name; danger matrix covers same-callee × every return type |
| Panic in CPU child silently discarded → wrong value bound | Medium | High | Re-raise semantics locked (Research Finding 8); P5 panic fixture asserts identical behavior vs sequential |
| stdout interleaving makes the cross-impl consistency gate flaky | Medium | Medium | Fixture discipline: aggregate/post-join assertions only (Research Finding 10); any fixture printing inside a parallel member must assert order-independent output |
| Spawn overhead exceeds the win for small workloads (perf regression) | Medium | Medium | Worth-it proxy (loop/recursion required) ships from day one; P5 perf spike measures spawn overhead on the CI machine and calibrates; `--no-auto-parallel` is the user escape hatch; explicit `wait` is the per-site escape hatch |
| Moving `independence.rs` to ynz-typeck breaks codegen consumers | Low | Medium | Pure relocation with re-export, all existing unit tests move with it, zero behavior change asserted by the full suite before any M3d feature code lands on top |
| Compile-time cost of new analyses (does_real_work + promotion probe) | Low | Medium | Both ride the existing call-graph walk; roadmap target <10% `ynz build` wall-clock increase on pirates-roster, measured in P5 |

## Questions

1. **Mixed groups (CPU member + I/O member in ONE parallel group)** — e.g. `let data = fetchThing()` (suspending) overlapping `let score = crunch(n)` (CPU). The poll loop handles per-child class naturally (inline-poll vs join-poll); declining them would need MORE code (a class-homogeneity check). **Recommendation: in scope.** Adds adversarial surface — the danger matrix covers it.
2. **`parallel_groups` hint domain retroactively covering M3b's I/O groups** (scope addition per Design-Doc Alignment). **Recommendation: yes** — it's the design doc's non-negotiable IDE surface, and the marginal cost over a CPU-only hint is near zero since both read the same partition.
3. **Same-callee CPU parallelization** (lifting M3b's restriction for CPU members only). **Recommendation: in scope** — it's the headline demo pattern and structurally unblocked (Research Finding 6).

## Risk Assessment & Rollout Strategy

**Risk level: MEDIUM** (compiler project — no payments/auth/PII/SQL; the "data" at risk is miscompiled user programs).

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | — |
| Touches auth/permissions | No | — |
| Raw SQL / literals | No | — |
| Modifies existing data | No | Codegen-only behavior change, gated by identical-output oracle |
| Third-party integration | Yes (Tokio) | Internal implementation detail; pinned `tokio = "1"`, verified against 1.49 docs |
| Changes existing behavior | Yes | Existing programs get FASTER but must stay byte-identical (`--no-auto-parallel` oracle on every fixture) |

**Mitigations applied:**
- Cross-impl consistency oracle (every fixture, default vs `--no-auto-parallel`, byte-identical stdout/stderr/exit) → the equivalent of a kill switch + full regression harness → MEDIUM stays MEDIUM with high confidence
- Decline-to-promote design → no new compile errors possible from this milestone
- Adversarial gate mandatory (P5) per roadmap

**Rollout plan**: compiler milestone — ships as tagged pre-release `v0.3.0-m7` (local-install VSCode artifact, no marketplace). No staged percentage rollout applies; the staging IS the `-mN` pre-release tag sequence before the `v0.3.0` final (M4).

## Design Divergences

| Doc | What it says | What we do instead | Approved rationale (named cost + reversal path) |
|-----|-------------|-------------------|------------------------------------------------|
| `design/concurrency.md` "Design Divergences → Write-effect type-based floor" | (already-recorded M3b divergence) traces actual writes | CPU candidacy reuses the same type-based floor: mutable-heap-argument calls never parallelize | Pre-approved in the doc itself ("M3d CPU-parallel reuses this same type-based write-effect source — the floor transfers unchanged"). Cost: read-only mutable-heap-arg CPU calls don't overlap. Reversal: M4 borrow-checker narrowing (already documented there). |
| `design/concurrency.md` "Same-callee concurrent calls run sequentially" | (already-recorded M3b divergence) | Lifted for CPU members ONLY (per-invocation ctx + member-index slot keying); REMAINS for suspending members | Cost of keeping it for I/O: unchanged from M3b. Phase 4 amends the doc entry's scope line. No new divergence — a partial reversal of a recorded one. |

_(No NEW divergences from any `[locked]` doc. The plan implements the documented model.)_

## Documentation Deliverables

| Deliverable | Phase | Notes |
|---|---|---|
| `design/concurrency.md` — new "CPU Statement Parallelization (M3d)" section: promotion + decline rules, panic re-raise semantics, cancellation/detach constraint, worth-it proxy, same-callee scope amendment to the M3b divergence entry | Phase 4 | Cross-cutting: documents decisions locked in P0 and implemented in P1–P3 |
| `spec/concurrency.md` — short user-facing note: independent heavy computations run on separate cores automatically; the IDE shows the overlap; `wait` forces order | Phase 4 | HS-grad tone per `.claude/rules/spec-writing.md` |
| `CHANGELOG.md` `[0.3.0-m7]` section | Release (post-P5) | Generated via `/release` |

## Planned RED Repros

| What's intentionally broken (file + function/symbol + line-range) | Locking RED test (path::test name) | Asserted contract | Fixing phase | Prod-exposure note |
|---|---|---|---|---|
| — | — | — | — | _(empty — no planned RED repros; this is feature work, not a bug-fix plan, and no phase ships intentionally-failing tests)_ |

## Invariants This Milestone Must Preserve

### Safety
- A synchronous join (`block_on`, `Handle::block_on`, thread-park, busy-wait) appears in ZERO new code paths; the only `block_on` in the binary remains the pre-existing main→entrypoint driver. (Checkable: grep the diff.)
- Every program that compiles before M3d compiles after M3d with the same diagnostics. Promotion NEVER introduces a compile error — guard-conflicting candidates decline to sequential lowering. (Checkable: guard-shape test corpus compiled pre/post.)
- Every fixture produces byte-identical stdout/stderr/exit-code under default mode vs `--no-auto-parallel`. (Checkable: cross-impl consistency harness.)
- Heap arguments to CPU-spawned children are copied into the child's ctx (`BgArgDropEntry` discipline); no pointer owned by the parent frame is reachable from a detached child. (Checkable: cancellation fixtures under alloc=free accounting.)
- A panicking CPU child re-raises in the parent — identical observable behavior to sequential execution. (Checkable: panic fixture diff vs `--no-auto-parallel`.)
- alloc=free on every new fixture (frames, ctx blocks, handle boxes, arg copies).

### Performance
- Two independent heavy CPU calls overlap on separate cores: demo wall-clock measurably under sequential baseline on the CI machine (P5 records numbers).
- Non-promoted functions pay ZERO new cost: no frame, no spawn, no preemption change — straight-line codegen unchanged (checkable: IR diff on a non-promoted fixture is empty).
- Trivial/leaf callees never spawn (worth-it proxy: transitive loop/recursion required).
- Compile-time: <10% `ynz build` wall-clock increase on `examples/pirates-roster/` (roadmap target; measured P5).
- **Auto-promotion analysis** (mandatory per `.claude/rules/auto-promotion.md`): this milestone IS an auto-promotion. Stricter/faster form = thread-parallel execution of independent CPU statements; compiler proves fit via independence + worth-it + guard-probe. Surfaces: codegen auto-promotion YES; muted hint — NO TYPEABLE explicit form exists (there is no "run in parallel" syntax), so per the inference rule the hint is the **Informational** `parallel_groups` domain (comment-style), not click-to-make-explicit; Tier 3 lint — NO (nothing to rewrite toward; consistent with auto-SoA precedent). Override directions: force-the-OTHER-pick (sequential) = existing syntax — explicit `wait` between statements (per-site) or `--no-auto-parallel` (program-wide); force-the-auto-pick (parallel) = existing syntax — explicit `background` spawns. Both directions handled by existing syntax → no new API (documented omission per the rule).

### Teaching
- New `parallel_groups` Informational muted-hint domain fires on every member of every Parallel group (CPU and I/O classes), with WHAT/WHAT-INSTEAD/WHY hover text; wired through registry → LSP inlayHint → VSCode.
- `wait_on_non_may_block` why-text reworded: explicit `wait` before a CPU call is an ordering barrier (group reset), not a no-op (in promoted contexts).
- No new compile-error classes ship (decline is silent by design) — explicitly documented in the error-gallery note so reviewers know it was considered, not forgotten.
- No banned jargon in any new user-facing text (`jargon_audit` green; no "thread", "spawn", "join" jargon leaks into hints — naming per `.claude/rules/vocabulary.md`, e.g. "runs at the same time as line N").

### Runtime Dependencies
- `ynz_rt_spawn_blocking_joinable` / `ynz_rt_join_poll` / handle-free shim: require the Tokio runtime (`ynz_rt_init`) and malloc — same dependency class as existing spawn primitives.
- Promotion analysis, independence partition, worth-it fixpoint: compile-time only, zero runtime dependency.
- Non-promoted code: zero new runtime dependencies.

### Kernel-Mode Behavior
- `--kernel` mode: the CPU-parallel pass is DISABLED entirely (no scheduler exists to spawn onto). Functions lower sequentially — identical output, no new diagnostics. This is the "decline" path, not an error: auto-promotion never errors on valid code, and kernel mode simply fails the runtime-dependency fit. (Decision locked here per the roadmap's "clean reject vs. inline-sequential fallback — decide at plan time": **inline-sequential fallback**. A clean reject would turn valid sequential-looking code into an error because the OPTIMIZER wanted to fire — that violates the auto-promotion rule and the "no new user-facing syntax / identical semantics" constraint.)
- `wait`/`background` remain compile errors in kernel mode (unchanged, `check.rs:142-186`).

### Demo & Error Gallery
- `examples/pirates-roster/entrypoint.ynz` gains a CPU multicore section (two independent heavy computations — realistic roster-stats crunch, not `print(featureName())`) with inline comments pointing at the `parallel_groups` IDE hint. insta stdout snapshot.
- `examples/primantis-orders/`: M3d introduces NO new compile-error classes (decline is silent). The milestone gallery file gets a header note recording that fact explicitly (the considered-not-forgotten requirement). Existing gallery snapshots re-verified against the reworded `wait_on_non_may_block` text.
- Both files snapshot-verified in P5.

### Feature Registry Entries
- New `[[muted_hint_domain]]`: `parallel_groups` (placement_category = "Informational", fires on members of auto-parallel groups, both classes).
- Modify `[[diagnostic_template]]` `wait_on_non_may_block` (`features.toml:1553` region): why-text reworded for ordering-barrier semantics.
- Modify `[[muted_hint_domain]]` `background_routing` description if wording drifts from the new shared analysis home (verify in P4; likely unchanged).
- No new keywords, banned_jargon, primitive_intrinsics, type_attached_constants. No new `[[deferred_language_feature]]`/`[[deferred_tooling_feature]]` expected; if the P5 perf spike defers the calibrated cost threshold, it lands as a documented constant + design-doc note, not a registry deferral (it refines an internal heuristic, not a user-facing feature).

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each phase is an independently-reviewable PR on `feat/m3d-cpu-parallelization` via `/pr`; phase boundaries are commit+review gates per the Exit Sequence.
- **Shadow main branches**: one feature branch off `main`, merged per phase or at milestone end via PR — no long-lived parallel main.
- **Building the engine before shipping value**: P0–P1 are deliberately thin (spike + ABI); the first user-visible value (working CPU overlap on real fixtures) lands in P3, teaching in P4 — no phase builds speculative infrastructure beyond what the next phase consumes.
- **Hotfix that isn't**: no hotfixes planned; any pre-existing bug discovered (e.g., the non-suspending `-> Shape` return base bug) is DECLINED-AROUND (decline-to-promote) and stays on its existing roadmap/todos track — not quietly half-fixed here.
- **Abandoned branches**: the branch lives exactly as long as the milestone; `/release` for `v0.3.0-m7` closes it.
- **Flag graveyards**: no feature flags — the oracle flag `--no-auto-parallel` is a permanent test utility (M1-mandated), not a rollout flag needing cleanup.

## Phase Execution Protocol

Each phase ends with the Exit Sequence: persist plan bookkeeping → resolve `$BASE` (Phase N≥2: prior phase's `Committed:` SHA; Phase 0: `plan_base:` from front-matter) → fan out the full reviewer set + one deviation-judge per documented deviation in parallel → coordinator writes Evidence + Phase Review Gates → handle verdicts → prompt commit. Canonical fan-out spec: `~/.claude/commands/execute-plan.md` Step 3.d–3.h (referenced, not duplicated, per `no-duct-tape.md` #7). Final phase additionally runs the cumulative Opus sweep per Step 4.a.

## Phases

### Phase 0: Spike — joinable-join vertical slice on the REAL compiler + decision lockdown
**PR scope**: Throwaway-tolerant vertical slice proving the poll-based CPU join end-to-end through `./target/debug/ynz run` on real `.ynz`, plus the locked decision record. Accept/reject gate.
**Branch**: `feat/m3d-cpu-parallelization`
**Flag**: N/A
**Est. lines**: ~400 (spike code, may be reshaped in P1–P3)
**Ships via**: `/pr` (draft; the PR carries the spike verdict + locked decisions)
**Objective**: Validate, against the real compiler and runtime, that (a) a prototype `ynz_rt_spawn_blocking_joinable` + `ynz_rt_join_poll` pair drives a 2-member CPU group through spawn → suspend-at-join → resume → result-bind with correct values; (b) the same-callee case works with per-invocation keying; (c) pool saturation queues without deadlock; (d) panic re-raise and cancellation-detach behave as locked. Lock the promotion-fixpoint algorithm.
**Why this phase exists**: M2's HALT happened because its spike validated a hand-written model that diverged from emitted codegen. This spike compiles real `.ynz` through `./target/debug/ynz` (spike-artifact-validity rule). The CPU join is M2-corpse-adjacent — one synchronous-join mistake reintroduces the crash — so the riskiest mechanism is proven before any production phase builds on it.
**Current-state anchors**:
- `crates/ynz-runtime/src/runtime.rs:591` — `ynz_rt_spawn` (existing fire-and-forget SM spawn; the future-wrapper pattern to mirror)
- `crates/ynz-runtime/src/runtime.rs:140` — `ynz_rt_spawn_blocking` (ctx-copy discipline at `runtime.rs:89`)
- `crates/ynz-runtime/src/runtime.rs:663` region — `ynz_rt_async_sleep_create`/`_poll` (the handle-in-frame-slot poll protocol the join mirrors)
- `crates/ynz-codegen/src/emit.rs:6061` — `emit_independent_group_poll` (the join loop the CPU child class extends)
- `crates/ynz-codegen/src/emit.rs:1847` — SM-lowering dispatch (`suspend_set.contains`)
**Files (expected scope)**: `crates/ynz-runtime/src/runtime.rs`, `crates/ynz-codegen/src/emit.rs` (spike-gated path), one spike fixture dir under `crates/ynz-driver/tests/fixtures/`, this plan file (decision record).
**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work (lint fix in adjacent code, blocking bug, missing dependency). Document each deviation in the PR description with a one-line reason. If a deviation is its own concern, STOP — split into a separate PR or revise this plan.
**Steps**:
1. Prototype runtime pair: `ynz_rt_spawn_blocking_joinable(fn_ptr: extern "C" fn(*mut u8) -> YnzCpuResult, ctx_ptr, ctx_size) -> *mut u8` (heap handle boxing `JoinHandle<YnzCpuResult>` where `YnzCpuResult = [i64; 2]`), `ynz_rt_join_poll(handle, waker_ctx, result_out: *mut u8) -> i32` (1=Pending with waker registered, 0=Ready with 16 bytes written + handle box dropped; Ready(panic) → re-raise), and a handle-free shim for the frame drop path.
2. Hardwire a minimal promotion + lowering path for ONE fixture shape (two int-returning calls to a recursive `fib`-style callee), gated behind env `YNZ_M3D_SPIKE=1` so nothing leaks into default builds.
3. Spike fixtures, all run via `./target/debug/ynz run`: (a) distinct-callee 2-group, correct values bound; (b) SAME-callee 2-group (`fib(35)` / `fib(36)`), correct distinct values; (c) timing proof: wall-clock < sequential sum (coarse — e.g. two ~1s workloads complete in <1.6s); (d) saturation: ≥600 spawned joins complete (queueing proven, no deadlock); (e) panic-in-child re-raises; (f) cancellation: parent dropped mid-join → no UAF/leak under the alloc accounting; (g) MIXED group (one CPU child + one `sleep`-backed I/O child sharing the continuation state) — including the CPU-child-finishes-FIRST ordering, since a Ready join-poll must not corrupt the still-Pending inline-poll state (reviewer-flagged highest-risk interaction; proving it here, not first in P3).
4. Lock and record in this plan + P0 PR description: the promotion fixpoint algorithm (intrinsic-rooted may-block fixpoint first → bottom-up promotion pass in deterministic call-graph order → guard-probe transitive rollback loop, monotone downward), the decline-rule enumeration (all guards from Research Finding 4 + Shape returns + kernel mode + `--no-auto-parallel` + call-cycle membership), result ABI, panic re-raise, heap-arg copy discipline, mixed-group support.
5. **Accept/reject gate**: if the simplest distinct-callee case cannot work without a synchronous join anywhere, or fixture (c)'s overlap doesn't materialize, HALT the milestone and re-plan — do NOT proceed to P1 with a workaround.
**Acceptance criteria**:
- [x] Spike fixtures (a)–(f) all pass through `./target/debug/ynz run` with documented output in the PR
  - Evidence: All six fixtures live-verified by R8 acceptance-verifier 2026-06-12 in devcontainer (LLVM 18) — outputs below re-confirmed; (c) timing re-measured 1218ms (spike) vs 1780ms (default), < 1600ms threshold ✓; saturation_600_joins re-run green (640 joins, no deadlock); drop-witness tests re-run green. Original documented outputs (2026-06-11):
    - (a) distinct callee (`v0_3_m3d_spike_a_distinct.ynz`): `6765\n10946\n`, exit 0 — fib(20)=6765, fib2(21)=10946 ✓
    - (b) same callee (`v0_3_m3d_spike_b_same_callee.ynz`): `6765\n10946\n`, exit 0 — fib(20), fib(21) distinct values ✓
    - (c) timing proof (`v0_3_m3d_spike_c_timing.ynz`): `102334155\n165580141\n`, exit 0 — parallel wall-clock 1317ms vs sequential 1913ms (<1600ms threshold) ✓
    - (d) saturation (`v0_3_m3d_spike_d_saturation.ynz`): fib(30)/fib(31) pair; output `832040\n1346269\n`, exit 0 — 2 joins, 0 failures ✓. ≥600-join saturation proof: `saturation_600_joins` unit test in `crates/ynz-runtime/src/lib.rs` spawns 640 individual joinable tasks via `ynz_rt_spawn_blocking_joinable` (JOIN_COUNT=640, above the 512-thread default pool), polls all 640 to Ready, asserts all results correct — no SpawnStateFnFuture involved.
    - (e) panic re-raise (`v0_3_m3d_spike_e_panic.ynz`): `RUNTIME ERROR: division by zero (int)`, exit 1 — identical sequential vs spike ✓
    - (f) cancellation (`v0_3_m3d_spike_f_cancel.ynz`): `RUNTIME ERROR: division by zero (int)`, exit 1 — will_panic(0) panics, in-flight fib(35) handle freed by frame drop ✓
  - Bonus fixture (g) (`v0_3_m3d_spike_g_mixed.ynz`): `55\n89\ndone\n`, exit 0 — CPU group (fib(10)+fib(11)) joins, then `wait sleep(50)` runs sequentially; prior "continuation state 3 out of range" error resolved by R3-2 (sm_scope_depth guard) + R3-3 (reload exclusion). Fixture (g) is NOT in ACs.
  - Bonus fixture (h) (`v0_3_m3d_spike_h_two_waits.ynz`): `55\n89\n`, exit 0 — CPU group (fib(10)+fib(11)) then two sequential `wait sleep(0)` calls; proves spike_reload_cpu_results_from_frame correctly reloads frame bytes into pre-allocated sm_entry allocas across each suspension (LLVM SSA dominance satisfied: allocas are in the function entry block, dominate all state blocks). Fixture (h) is NOT in ACs; added in R3, dominance fix landed in R5 post-gate.
  - Regression fixture (i) (`v0_3_m3d_spike_i_mixed_locals.ynz`): `99\n55\n89`, exit 0 — `let c = 99` pre-pair, CPU group (fib(10)+fib(11)), then `wait sleep(0)` then `print(c)`, `print(a)`, `print(b)`. Pre-pair local `c` is lowered before spawn (F6-2); crossing-local slot base shifted past 6-slot spike reserve (SPIKE_SLOT_RESERVE=6, locals start at byte 80+) so flush of `c` no longer collides with spike result slots. Added R4 (frame-slot collision regression); filename corrected R6. Fixture (i) is NOT in ACs.
  - Regression fixture (j) (`v0_3_m3d_spike_j_locals_after_join.ynz`): `55\n89\n144`, exit 0 — CPU group (fib(10)+fib(11)) then `let c = a + b` then `wait sleep(0)` then `print(a)`, `print(b)`, `print(c)`; confirms crossing-local after join is correctly flushed and reloaded across the suspension. Added R4; filename corrected R6. Fixture (j) is NOT in ACs.
  - Admission fixture (k) (`v0_3_m3d_spike_k_param_host.ynz`): host function has ≥1 params; CPU pair is present but spike is declined (zero-param admission invariant). Both modes byte-identical (sequential output). Added R5. Fixture (k) is NOT in ACs.
  - Admission fixture (l) (`v0_3_m3d_spike_l_arity_decline.ynz`): CPU calls have arity ≠ 1 (`add(10,20)` or zero-arg `getval()`); spike is declined by `args_lowerable` guard. Both modes byte-identical. Added R5. Fixture (l) is NOT in ACs.
  - Mutation fixture (m) (`v0_3_m3d_spike_m_mutated_result.ynz`): CPU group produces `a=fib(10)`, `b=fib(11)`, then `a=999` in rest_stmts; both modes print `999` then `89`. DECLINE fixture: result-name reassignment in rest_stmts refuses spike admission (R6 replaced the R5 prune mechanism with admission decline), so both modes lower sequentially — byte-identical trivially. IR-verified 0 spawn-call instructions (R7/R8). Added R5; mechanism description corrected R8. Fixture (m) is NOT in ACs.
  - Nested-wait fixture (n) (`v0_3_m3d_spike_n_nested_wait.ynz`): nested wait inside an if-block in rest_stmts; CPU results remain correct across the nested suspension (reload_params_from_frame calls spike_reload at every sm_scope_depth via reload_crossing:true; result allocas pre-allocated in sm_entry dominate all state blocks). Both modes byte-identical. Added R5. Fixture (n) is NOT in ACs.
  - FIRES/DECLINES envelope (IR-verified R8 by executor AND independently by acceptance-verifier): (a)/(b)/(c)/(d)/(e)/(f)/(g)/(h)/(i)/(j)/(n) each emit 2 `call @ynz_rt_spawn_blocking_joinable` instructions under `YNZ_M3D_SPIKE=1` — the spike FIRES, including through post-pair intrinsic waits, so the cross-suspension reload machinery is live-exercised. (k)/(l)/(m)/(o)/(p)/(q) emit 0 — admission correctly declined (params, arity, mutation, non-entrypoint, pre-pair wait, post-pair suspending callee respectively); byte-identical via sequential lowering.
- [x] No `block_on`/thread-park in any spike path (grep of diff cited)
  - Evidence: R8 acceptance-verifier: `git diff 8b99ac9 -- crates/ynz-codegen/src/emit.rs crates/ynz-runtime/src/runtime.rs | grep -E "^\+.*(block_on|thread::park|spin_wait|busy.wait)"` → two comment lines only (the invariant declaration comment at runtime.rs:176-177). Zero actual call sites in production code. The sole `block_on` in the full diff is inside the `#[tokio::test]` `panic_reraises_in_parent` in lib.rs (test harness needing a synchronous poll to catch the panic payload) — not a spike path.
- [x] Decision record (fixpoint algorithm + decline rules + ABI + panic + cancellation + mixed groups) written into this plan's Phase 0 notes and the PR description
  - Evidence: Phase 0 Decision Record section below (items 1–10), locked 2026-06-11; R8 acceptance-verifier confirmed all six AC-named categories present: fixpoint algorithm (item 8), decline rules / admission envelope (item 9), ABI (items 1–2), panic (item 6), cancellation (items 4–5), mixed groups (item 10).
- [x] Accept/reject verdict stated explicitly with the timing numbers
  - Evidence: Phase 0 Step 5 Verdict below — explicit "ACCEPT. Proceed to Phase 1." with timing numbers (parallel ~1256ms vs sequential ~1795ms, < 1600ms threshold), all 6 gate conditions enumerated. R8 live corroboration: 1218ms (spike) vs 1780ms (default) — still under threshold.
**Quality gate**:
- [x] Spike code clearly gated (`YNZ_M3D_SPIKE`) — zero default-build behavior change (full existing suite green)
  - Evidence: `cargo test --workspace` (spike env var absent) → all test-result lines `ok`, 0 failed across all crates (verified 2026-06-11).
- [x] Spike exercised the REAL compiler artifact, not a hand-written model
  - Evidence: All fixtures run via `./target/debug/ynz run <fixture.ynz>` — real `.ynz` source through the full pipeline (parser → typeck → codegen → LLVM → binary → execute).
- [x] Cancellation fixture ran under alloc=free accounting
  - Evidence: `SpawnStateFnFuture::drop` step 1.5 calls `cleanup_spike_cpu_handles` which reads the discriminator at frame offset 4; if `SPIKE_FRAME_MAGIC` is present it frees and nulls each non-null handle slot. Test evidence: `discriminator_drop_frees_spike_handles` (direct helper test — slot-0 live, slot-1 null, normal-frame no-op) and `spawn_state_fn_future_drop_before_first_poll_frees_handles` (end-to-end delegation: SpawnStateFnFuture created with spike-magic frame + live handle, dropped without polling — blocking task completes after detach).
**Verification**: `cargo test --workspace` green (spike gated off); `YNZ_M3D_SPIKE=1 ./target/debug/ynz run <spike fixtures>` outputs recorded; timing numbers in PR.

**Phase Review Gates** (filled at phase completion by coordinator; 8 gate rounds total — each line shows the agent's LATEST verdict; full per-round history in Findings Log):
- [x] code-reviewer: PASS 2026-06-12T16:30 (R7 — ISSUE-A/B confirmed resolved via 6 adversarial inputs; non-blocking concerns logged for P1)
- [x] rules-compliance-reviewer: PASS 2026-06-12T05:37 (R6 — zero violations; R4/R5 not-confirmed findings adjudicated per Rule 11, recorded in Findings Log for Patrick's overrule)
- [x] plan-adherence-verifier: PASS 2026-06-12T05:37 (R6 — all R5 items resolved)
- [x] acceptance-verifier: PASS 2026-06-12T17:03 (R8 — all 4 ACs MET on own live runs; FIRES independently IR-verified; 1924 workspace tests green)
- [x] design-compliance-reviewer: PASS 2026-06-12T05:37 (R6 — no [locked]-doc contradictions; M2-HALT corpse clean: no block_on/park in production paths)
- [x] deviation-judge #1 (scope: lib.rs m3d_join_shims test module): last direct verdict R5 BLOCK (ISSUE-1 drop-test blindness) — fix verified R6 by judges #13/#17 + drop-witness mutation test; direct re-judge deferred to cumulative 4.a per Patrick's focused-re-gate authorization 2026-06-12
- [x] deviation-judge #2 (scope: runtime_decls.rs + 13 golden snapshots): PASS 2026-06-11T23:30 (R5)
- [x] deviation-judge #3 (scope: state.md build-env note): PASS 2026-06-11T23:30 (R5)
- [x] deviation-judge #4 (approach: extern "C-unwind" ABI honesty): PASS 2026-06-11T23:30 (R5 — honest C-unwind docs verified at every constructible call site)
- [x] deviation-judge #5 (approach: CpuJoinHandle private field + constructor): PASS 2026-06-11T23:30 (R5)
- [x] deviation-judge #6 (approach: detection-scope gate agreement): PASS 2026-06-12T05:37 (R6)
- [x] deviation-judge #7 (approach: spawn→poll extraction + adjacency/data-dep/arity guards): PASS 2026-06-12T05:37 (R6)
- [x] deviation-judge #8 (approach: null-guard on spawn return): PASS 2026-06-11T23:30 (R5)
- [x] deviation-judge #9 (approach: F7 reload / spike-owned crossing exclusion): PASS 2026-06-12T05:37 (R6 — 7 pure-read adversarial shapes clean)
- [x] deviation-judge #10 (approach: Cg-field spike_crossing carry): PASS 2026-06-12T05:37 (R6)
- [x] deviation-judge #11 (approach: spike frame discriminator): last direct verdict R5 BLOCK (ISSUE-1, same root as #1) — fix verified R6 by judges #13/#17 + drop-witness mutation test; direct re-judge deferred to cumulative 4.a per Patrick's authorization 2026-06-12
- [x] deviation-judge #12 (approach: locals-base shift past spike reserve): PASS 2026-06-12T05:37 (R6 — heap corruption gone)
- [x] deviation-judge #13 (approach: cleanup_spike_cpu_handles helper extraction): PASS 2026-06-12T05:37 (R6 — delegation deletion → both tests fail 0≠1)
- [x] deviation-judge #14 (approach: sm_entry result-alloca pre-allocation): PASS 2026-06-11T23:30 (R5)
- [x] deviation-judge #15 (approach: prune→admission-decline + pre-pair lowering + pre/post-pair suspending gates, R8-narrowed): PASS 2026-06-12T17:03 (R8 — fresh adversarial inputs on the narrowed predicate all held; identity hash fdc8dc2)
- [x] deviation-judge #16 (approach: entrypoint-only admission gate): PASS 2026-06-12T16:30 (R7 — background post-pair admitted+harmless; ISSUE-B repro declines byte-identically)
- [x] deviation-judge #17 (approach: #[cfg(test)] drop-witness probe in CpuJoinHandle): PASS 2026-06-12T05:37 (R6 — layout-safe, no spurious increments)
- [x] Committed: dcc1432

**Findings Log**:
- 2026-06-11T18:52 — code-reviewer round 1: BLOCK. Panic fixture (e) and cancellation fixture (f) validate by process-abort coincidence — Yinz div-by-zero calls `process::abort()` from the child thread, so `ynz_rt_join_poll`'s `resume_unwind` branch never fires end-to-end, and `ynz_rt_join_handle_free` is declared but NEVER called by spike codegen (no drop-shim wiring); Step 5 ACCEPT gate condition 5 overstated. `crates/ynz-runtime/src/runtime.rs` fn_ptr path, `crates/ynz-codegen/src/emit.rs` spike path, fixtures e/f. Also: wrong "LLVM normalizes allocas" comment ~emit.rs:6640; 13 snapshots gained 3 unconditional declares ("IR diff empty" invariant literally false).
- 2026-06-11T18:52 — plan-adherence-verifier round 1: BLOCK. (1) `crates/ynz-codegen/src/runtime_decls.rs` is Phase 1 declared scope, touched in Phase 0 silently (3 unconditional declarations); (2) 13 golden snapshot updates cascade, undocumented; (3) Step 3(g) "proving it here, not first in P3" — spike FAILS fixture (g) ("continuation state 3 out of range") and the failure was reclassified as out-of-scope observation without deviation entry or user sign-off (silent scope narrowing per no-duct-tape.md).
- 2026-06-11T18:52 — acceptance-verifier round 1: BLOCK (AC1 WEAK, AC-quality-gate-cancellation WEAK). Saturation fixture (d) runs exactly 2 joins (one fib(30)/fib(31) pair, 2 output lines), NOT the ≥600 spawned joins Step 3(d) requires — plan Evidence claim "300 instances × 2 = 600 joins" factually wrong. Cancellation "alloc=free accounting" claim has no allocation ledger behind it (Arc-witness unit test only). Other ACs MET by live runs: (a)(b)(c)(e)(f) confirmed, timing 1199ms parallel vs 1906ms sequential.
- 2026-06-11T18:52 — deviation-judge #2 (approach: extern "C-unwind" ABI) round 1: BLOCK. `fn_ptr` is `extern "C" fn` at runtime.rs:924 — a panicking worker aborts the process at the spawn-closure boundary (RFC 2945) BEFORE Tokio's catch_unwind can form a JoinError; the C-unwind fix on `ynz_rt_join_poll` fixes the exit but the entry is welded shut. Sibling pattern: `ynz_rt_spawn_blocking` wraps fn_ptr in catch_unwind at runtime.rs:198-200.
- 2026-06-11T18:52 — deviation-judge #3 (approach: CpuJoinHandle pub(crate)) round 1: BLOCK. Inner field `pub(crate)` lets any crate code call `.abort()`/`.poll()` on a handle the doc declares opaque with mutually-exclusive consumption paths. Narrow fix: private field + `pub(crate) fn new()` constructor (~4 lines), test uses `CpuJoinHandle::new(...)`. runtime.rs:884-898.
- 2026-06-11T18:52 — deviation-judge #4 (approach: detection scope) round 1: BLOCK. `spike_extract_cpu_group` (emit.rs:6258+) lacks the `int_returning_callees` filter `spike_cpu_candidates` has — a suspending parent calling one int + one non-int callee gets NO extra frame bytes (gate 1 None) but the spawn path fires anyway (gate 2 Some) → out-of-bounds frame writes at offsets 32-79 + LLVM type mismatch on the trampoline. Fix: same membership check in both gates.
- 2026-06-11T18:52 — deviation-judge #5 (approach: spawn→poll_state) round 1: BLOCK. Extraction takes any two non-suspending calls BY INDEX with no data-dep check: `let a = fib(20); let b = a + 1; let c = fib(b)` extracts calls 0+2, intervening `let b` lands in rest (lowered after spawn) → fib(b) spawns with missing/stale b. Fix: adjacency guard `second_idx == first_idx + 1` at emit.rs:~6290.
- 2026-06-11T19:50 — ROUND 2 (post F1–F9 fix round; all round-1 findings verified fixed by respective reviewers/judges). 6 PASS: rules-compliance, design-compliance, judges #1 (lib.rs scope), #2 (runtime_decls scope), #3 (C-unwind + catch_unwind), #4 (CpuJoinHandle constructor). 7 BLOCK, all NEW findings:
- 2026-06-11T19:50 — code-reviewer round 2: BLOCK. `spike_reload_cpu_results_from_frame` emits invalid LLVM IR when >1 suspension follows the CPU group (live repro: "Instruction does not dominate all uses" on reload alloca); Deviation #8 rationale + doc comment claim generality ("after each suspension") the code lacks. emit.rs:6367-6411. Concerns: Decision Record item 6 has self-contradicting "enables true end-to-end resume_unwind propagation" clause; detach_ctx_alloc_balanced doc oversells "alloc=1, free=1".
- 2026-06-11T19:50 — plan-adherence-verifier round 2: BLOCK. `.claude/state.md` executor-written build-env note is an undocumented scope deviation (not in any of the 8 scratch entries). Cross-flags: fixture (g) Evidence text at plan ~line 244 is stale (still describes the old failure); no-block_on grep evidence cites `git diff HEAD` not `git diff 8b99ac9`. Round-1 BLOCKs #1-#3 all verified RESOLVED.
- 2026-06-11T19:50 — acceptance-verifier round 2: BLOCK (AC1 WEAK only). All fixtures (a)-(g) pass live (verifier's own runs; (c) 1265ms parallel vs 1742ms sequential); saturation_600_joins + detach_ctx_alloc_balanced + drop_detach_no_uaf pass live; quality gates MET incl. alloc-ledger. Sole fix: fixture (d) Evidence sub-bullet (plan ~line 241) still claims "300 concurrent instances × 2 spawns = 600 joins" — false; fixture runs 2 joins; ≥600-join proof lives in saturation_600_joins (lib.rs:~3150). Correct the text.
- 2026-06-11T19:50 — deviation-judge #5 identity 7a6f20e (detection scope) round 2: BLOCK (fresh input). Filter unified but SCOPE not: gate 1 (spike_cpu_candidates) scans top-level stmts non-recursively; gate 2 fires in lower_sm_block's RECURSION into if/loop bodies. Nested adjacent CPU pair in an if-body of an already-suspending fn → gate 1 allocates 0 extra frame/state, gate 2 spawns anyway → state-index collision + OOB frame GEPs. Fix: guard spike path with `cg.sm_scope_depth == 0`.
- 2026-06-11T19:50 — deviation-judge #6 identity bba0e83 (spawn→poll) round 2: BLOCK (fresh input; round-1 input confirmed excluded). ADJACENT data-dependent pair `let a = fib(20); let b = fib(a)` passes adjacency guard → child 1 arg eval at spawn time hits unbound `a` → codegen Err on valid program. Fix: in spike_extract_cpu_group, return None when second call's args reference first call's bind name (~5-8 lines).
- 2026-06-11T19:50 — deviation-judge #7 identity 73c553f (null-guard) round 2: BLOCK (fresh input; round-1 orderings still hold). Cancellation leak: SpawnStateFnFuture::drop reads only FRAME_SLEEP_HANDLE_OFFSET=8 (runtime.rs:448-454); spike CPU handle slots 32/40 never cleaned → cancel parent mid-join with a live handle leaks the CpuJoinHandle box. Rationale's "matches sleep-handle protocol" is half-true (poll leg yes, drop leg missing). Fix: cleanup leg for spike handle slots — MUST NOT misread non-spike frames (use per-frame drop-shim mechanism or spike-frame discrimination).
- 2026-06-11T19:50 — deviation-judge #8 identity 8f8ffa4 (F7 reload) round 2: BLOCK. Same multi-suspension dominance failure as code-reviewer, deeper diagnosis: standard crossing slots for CPU results never written (reload_params_from_frame loads garbage into them at first wait), and second wait's reload_params_from_frame stores to the stale post_wait_bb_1 reload alloca from non-dominated cont_state_bb_2 → LLVM verifier abort. Surgical fix: exclude spike-owned CPU-result names from reload_params_from_frame's crossing loop (emit.rs:3182 region); spike_reload owns them exclusively.
- 2026-06-11T20:56 — ROUND 3 (post R3-1–R3-8; all round-2 findings verified fixed by respective reviewers/judges; D_count=11 incl. coordinator-identified #11 frame discriminator). 12 PASS: rules, design, plan-adherence (all R2 items resolved), acceptance-verifier (all ACs + quality gates MET by live runs), judges #2 (runtime_decls ABI), #3 (state.md), #4 (C-unwind; noted "Box leaks on panic path" comment now stale — discriminator rescues spike frames), #5 (constructor; #[cfg(test)] would be tighter — P1 note), #6 (gate divergence → harmless over-allocation), #8 (null-guard; TOCTOU closed by Tokio cooperative scheduling; P1 note: null-from-spawn silently skips instead of aborting), #10 (Cg-field exclusion tight), #11 (discriminator sound — offset 4 unreachable by non-spike writes, allocator verified zeroing). 4 BLOCK:
- 2026-06-11T20:56 — code-reviewer round 3: BLOCK. SILENT ORACLE DIVERGENCE (live repro): spike result/handle slots hardcoded at frame offsets 32/40/48/64 COLLIDE with crossing-local slots (FRAME_OFFSET_LOCALS_START=32 + idx*8). Mixed body (CPU pair + `let c = a + b` between two waits) → flush of `c` physically overwrites spike result region → prints 144 where 55 belongs, exit 0. emit.rs:2189 + 6238-6241, state_machine.rs:496. The R3 name-exclusion patched reloads; nothing stops the FLUSH from clobbering. Fix: spike slots must not overlap the crossing-local region (shift one region); committed mixed-shape regression fixture required. Concern: discriminator drop branch (runtime.rs:490-500) has zero test coverage.
- 2026-06-11T20:56 — deviation-judge #9 identity 8f8ffa4 (F7 reload) round 3: BLOCK. Same root cause as code-reviewer, independent live repro: 3rd crossing local `c` lands at byte 48 = SPIKE_RESULT_0_OFFSET; flush clobbers fib(10) result; output `99\n99\n89` vs expected `99\n55\n89`. Fixes named: dynamic spike-slot base after all crossing-local slots, OR decline-to-spike when non-spike crossing locals exist.
- 2026-06-11T20:56 — deviation-judge #7 identity bba0e83 (spawn→poll) round 3: BLOCK (fresh input; R2 input confirmed excluded). Data-dep guard is a FLAT Expr::Ident match — `fib(a + 1)` / `fib(max(a, 2))` pass the guard, then crash at the unsupported-arg arm (emit.rs:6710) instead of declining to sequential. Fix: recursive expr_contains_ident walk, or (narrower) extraction admits ONLY args the spike arg-evaluator can lower (IntLit / Ident-not-matching-bind-name) and declines everything else.
- 2026-06-11T20:56 — deviation-judge #1 identity 1b473f7 (lib.rs scope) round 3: BLOCK (fresh angle after 2 PASSes). Test `detach_ctx_alloc_balanced` name asserts alloc balance; its body proves only "child ran once, no UAF" — and the R3 doc trim explicitly admits "Does NOT directly measure malloc/free." False contract. Fix: rename to match assertion (e.g. detach_child_runs_after_handle_free) OR add real alloc/free instrumentation.
- 2026-06-11T20:56 — acceptance-verifier round 3: PASS with one coordinator-actionable text correction: fixture (d) Evidence still says "300 concurrent SpawnStateFnFuture instances × 2 = 600" — actual test is 640 individual joinable spawns via ynz_rt_spawn_blocking_joinable (JOIN_COUNT=640), no SpawnStateFnFuture involved. Replacement language provided in verifier report.
- 2026-06-11T21:48 — ROUND 4 (post F4-1–F4-5; D_count=13). 9 PASS: design, acceptance-verifier (all ACs + gates MET live; fixtures (i)/(j) byte-identical), judges #1 (rename clean), #2, #3, #5 (field privacy E0616-verified), #6 (declined-but-shifted frames consistent, 4 live inputs), #8 (poll/null/drop ordering intact), #11 (discriminator holds; slot floor=byte 32). 9 BLOCK:
- 2026-06-11T21:48 — code-reviewer round 4: BLOCK. PARAM-SLOT COLLISION (live repros): param slots use raw indices 0..n_params → bytes 32+ collide with fixed spike region. 1-param host: silent wrong output (seed 7→0, b 89→88). 2-param host: param value 13 forged as Box pointer → `free(): invalid pointer`. emit.rs:2704-2711, 3198-3199. All 10 fixtures use 0-param entrypoint — dimension never exercised. Fix: shift params past reserve OR decline-to-spike on n_params>0 + param'd-host fixtures. Concern: frame-layout comment documents the overlap without flagging it.
- 2026-06-11T21:48 — deviation-judge #12 identity 8fee9ca (locals-base shift) round 4: BLOCK. Same param collision, independent live repro (compute(42) prints a pointer value). Narrow fix named: `if !f.params.is_empty() { return None; }` in spike_cpu_candidates (~3 lines); all fixtures stay green (all 0-param).
- 2026-06-11T21:48 — deviation-judge #7 identity bba0e83 round 4: BLOCK (fresh input). args_lowerable checks arg FORM not COUNT: `add(10, 20)` passes (both IntLit) → one-arg trampoline calls two-param callee → LLVM verifier abort (live). Zero-arg passes vacuously → Err arm. Fix: `call.args.len() == 1 &&` at emit.rs:~6428 (doc comment already says "exactly one").
- 2026-06-11T21:48 — deviation-judge #9 identity 8f8ffa4 round 4: BLOCK (fresh input; R3 collision input now passes). MUTATION STALENESS (live repro): `let a = fib(10); ... wait; a = 999; wait; print(a)` → prints 55 not 999. Mutation flushes to crossing slot (byte 80) but spike_reload re-reads frozen SPIKE_RESULT slot (byte 48) on next wake. Fix: prune name from spike_crossing + un-exclude on Stmt::Assign in rest_stmts; crossing machinery takes over.
- 2026-06-11T21:48 — deviation-judge #10 identity dc62ffd round 4: BLOCK (fresh input). DEPTH ASYMMETRY: exclusion fires at ALL sm_scope_depths in reload_params_from_frame, but spike_reload only runs in the depth-0 rest_stmts loop → nested wait in an if-block leaves CPU results as stale allocas (neither reload path fires). Fix: carry (name, frame_offset) pairs in the Cg field; call spike_reload from reload_params_from_frame's reload_crossing:true path at any depth. MUST be implemented coherently with judge #9's pruning fix (one source of truth for which names are spike-reloaded vs crossing-reloaded).
- 2026-06-11T21:48 — deviation-judge #4 identity 6a66433 round 4: BLOCK (fresh trace after 2 PASSes). Deviation rationale claims "SM poll functions are themselves extern C-unwind (existing M2 discipline)" — FALSE: declare_resume_fn emits plain extern "C" resume fns (state_machine.rs:9, 101-112), so a real unwinding panic through emitted codegen aborts at the SM frame boundary BEFORE Tokio's catch_unwind; runtime.rs:1120-1122 comment is factually wrong. Unobserved because Yinz panics abort (not unwind) and the unit test bypasses SM frames. Fix: correct the rationale + Decision Record item 2 + runtime.rs comment honestly (through-codegen unwind path aborts by construction; C-unwind end-to-end is P1's, named), OR flip resume-fn ABI to C-unwind now.
- 2026-06-11T21:48 — deviation-judge #13 identity 01e0de3 round 4: BLOCK. Drop delegation at runtime.rs:513 is UNTESTED — delete the line, all tests stay green ("transitive coverage" claim false). Plus: helper lacks slot-nulling after free (double-free on hypothetical re-call) and size guard (UB on <48-byte buffer with colliding magic). Fixes: drop-path test via ynz_rt_spawn + drop-before-first-poll (no resume-fn needed — judge-verified constructable); null slots after free; debug-assert/document min frame size.
- 2026-06-11T21:48 — plan-adherence-verifier round 4: BLOCK (3 plan-text items): Decision Record item 8 stale ("no drop-shim wiring exists" — false post-discriminator); quality-gate evidence should cite discriminator_drop_frees_spike_handles; fixtures (i)/(j) undocumented in plan evidence.
- 2026-06-11T21:48 — rules-compliance-reviewer round 4: BLOCK (7 findings). COORDINATOR ADJUDICATION per CLAUDE.md Rule 11 (verify-real-then-fix): findings 1/2 (SPIKE_SLOT_RESERVE provenance doc placement), 4 (spike_cpu_candidates called 3× — cache it), 5 (Tier-2 doc comments on the 4 large spike helpers) = CONFIRMED REAL → routed to fix round 5. Findings 3/6/7 = NOT CONFIRMED: (3)+(6) demand test-execution-evidence comments in code/state.md ("Verified at <timestamp>") — that is changelog-style commentary banned by comments.md, and the execution evidence lives where the persistence model puts it (plan-file Evidence + acceptance-verifier live-run reports, rounds 2-4); (7) claims the spike codegen is "untested in the actual compiler context" — factually contradicted by fixtures (a)-(j) run through ./target/debug/ynz (full pipeline) and live-verified by the acceptance-verifier in rounds 2, 3, AND 4 (same round as this finding). These three findings are not routed; adjudication recorded here for Patrick's review — overrule available at the commit prompt.
- 2026-06-11T23:30 — ROUND 5 (FINAL gate; D_count=14; round cap reached). 11 PASS: design, plan-adherence (all R4 items resolved; cosmetic fixture (i)/(j) names+output stale in plan evidence — coordinator fixes at commit), acceptance-verifier (all 7 AC/gate entries MET by live runs: 14 fixtures byte-identical both modes, timing 1185ms vs 1771ms, 71 tests green), judges #2, #3, #4 (honest C-unwind docs verified at every constructible call site), #5 (production caller at runtime.rs:1109 justifies ungated pub(crate); prior cfg(test) note invalid), #6 (3-gate agreement, 4 live adversarial runs), #8 (dual null paths disjoint + correct), #10 (depth asymmetry fixed; sibling/while/partial-prune all live-verified), #14 (sm_entry pre-allocation tight; ok_or_else guards; fresh HashMap per Cg). 8 BLOCK → 4 distinct issues + adjudication:
- 2026-06-11T23:30 — [R5 ISSUE-1] code-reviewer + judges #1, #11, #13 (4 agents, each independently mutation-tested): drop-delegation test `spawn_state_fn_future_drop_before_first_poll_frees_handles` asserts task completion, which spawn_blocking's detach satisfies whether the handle box was freed or LEAKED — delete the delegation at runtime.rs:551, test stays green. Sibling `discriminator_drop_frees_spike_handles` case-1 free path equally blind. Fix (all four agents converge): Drop-witness — wrapper whose Drop flips an Arc<AtomicBool>, assert THE FLAG, in both tests.
- 2026-06-11T23:30 — [R5 ISSUE-2] deviation-judge #9 identity 8f8ffa4: BLOCK (live repro `0\n999\n89` vs oracle `55\n999\n89`). Prune-on-Assign fires UPFRONT for any assign anywhere in rest_stmts — a READ between the first wait and a later assignment reads the never-populated (zeroed) crossing slot. Fix: prune only when the assignment is the name's first use, or prune inline at the assignment site after lowering+flush; simplest spike-scope alternative: DECLINE-to-spike when rest_stmts assigns any CPU-result name.
- 2026-06-11T23:30 — [R5 ISSUE-3] deviation-judge #7 identity bba0e83: BLOCK (live repro `16994814376953` vs oracle `45`). spike_extract_cpu_group routes PRE-pair statements into rest_stmts (lowered AFTER the spawn) — an Ident arg naming a pre-pair crossing local reads an uninitialized sm_entry alloca at spawn time. Fix: lower pre-pair statements before emit_cpu_group_spawn_join (semantically right), or decline when first_idx != 0.
- 2026-06-11T23:30 — [R5 ISSUE-4] deviation-judge #12 identity 8fee9ca: BLOCK (live repro `55 88` off-by-one / `inconsistent park state` crash). NON-ENTRYPOINT spike host: frame_layouts_query runs BEFORE spike promotion, so a caller heap-allocating the promoted callee's frame via emit_suspending_call_heap_boxed (emit.rs:8156-8160) falls back to FRAME_HEADER_SIZE=32 bytes while the spike resume writes at bytes 48/64/80+ → heap corruption past the allocation. Fix: spike-aware fallback size at the call site, or gate spike admission to entrypoint-only (decline path, simplest).
- 2026-06-11T23:30 — rules-compliance-reviewer round 5: BLOCK (6 findings). COORDINATOR ADJUDICATION vs artifacts: finding 3 (offset constants lack names) NOT CONFIRMED — `SPIKE_HANDLE_0_OFFSET` etc. exist as documented consts in BOTH crates (emit.rs:6410-6412, runtime.rs:70); finding 4 (runtime fns lack Tier-3 docs) NOT CONFIRMED — docstrings verified by R1/R3/R4 reviewers and multiple judges; finding 6 (state.md note = duct-tape) NOT CONFIRMED — environment facts in state.md are exactly CLAUDE.md Rule 6's prescription, judge #3 PASSed it 3× incl. this round. Finding 2 CONFIRMED-TRIVIAL (no comment directly above the m3d_spike env read at emit.rs:727; the Cg field at 1564-1565 documents it — add a one-liner). Findings 1 (env-var parse helper at 2 occurrences — below the rule's own 3+ threshold, reviewer self-describes as "borderline") and 5 (spike_cpu_candidates at 2 cross-pass call sites; 2163 could consult the extended suspend set) = real-but-cheap improvements, routed with ISSUE fixes if a round 6 is authorized.
- 2026-06-11T23:30 — Working-tree integrity verified post-gate: delegation call intact at runtime.rs:551; modified-file set matches known phase scope (4 agents' temporary mutations all restored).
- 2026-06-12T05:37 — ROUND 6 (focused re-gate, user-authorized; D_count=17; stable judges #1-#5, #8, #11, #14 deferred to cumulative 4.a). 11 PASS: design, rules (zero violations), plan-adherence (all R5 items resolved; cosmetic: consolidate admission envelope into Decision Record item 9; fixture (m) header still describes removed prune), acceptance-verifier (all 7 AC/gate entries MET live: 15 fixtures byte-identical, 1048ms parallel, 71 tests green, drop-witness verified real), judges #6 (3-gate agreement, type-system-bounded asymmetry), #7 (pre-pair mutation propagates correctly), #9 (7 pure-read adversarial shapes clean), #10 (Cg field single-write verified; stale prune comments at emit.rs:3234/3566), #12 (heap corruption gone; P3 note: emit_suspending_call_heap_boxed still reads stale layouts — gated unreachable), #13 (3-round saga closed: delegation deletion → both tests fail 0≠1), #17 (probe layout-safe, no spurious increments). 3 BLOCK → 2 root causes:
- 2026-06-12T05:37 — [R6 ISSUE-A] code-reviewer + deviation-judge #15 (identity 3df244e, convergent live repros): SUSPENDING PRE-PAIR STATEMENT (`wait sleep(0)` before the CPU pair) passes all gates; pre-stmts loop calls lower_sm_stmt_with_wait, then emit_cpu_group_spawn_join repositions past the unterminated post_wait_bb → LLVM verifier crash ("Basic Block does not have terminator") on valid code; 3 of 4 wait-bearing variants broke. Fix (both agents converge): decline admission when pre_stmts contains any wait/suspending call (one line, helpers exist in the same loop, applied in all 3 gates) + committed leading-wait regression fixture.
- 2026-06-12T05:37 — [R6 ISSUE-B] deviation-judge #16 (identity 3a091cf, live repro `55\n1\n89` vs oracle `55\n55\n89`): SUSPENDING CALLEE under spike — `wait worker()` in post_stmts embeds worker's child sub-frame at byte 48 per the PRE-SPIKE frame_layouts (own_base computed without the 48-byte reserve) = SPIKE_RESULT_0; worker's resume_point write clobbers the joined result `a`. Same stale-frame_layouts root class as its R5 finding, different door (embedded children vs caller allocation). Fix options (judge-named): (A, immediate) force heap-boxed path for suspending callees when cg.m3d_spike (emit_suspending_call_inline_poll skip-embedded); (B, P3) include the reserve in build_frame_layouts. Plus committed suspending-callee regression fixture.
- 2026-06-12T05:37 — Post-gate tree verification (probe-window hazard from this round): delegation intact at runtime.rs:551, zero MUTATION-PROBE markers in crates/, both drop tests green on final tree, modified-file set matches phase scope. Process lesson recorded: mutation probes by parallel gate agents on the shared tree can expose sibling reviewers to inconsistent states — future gates must serialize mutation probes (coordinator-run post-gate, or single designated prober).
- 2026-06-12T16:30 — ROUND 7 (focused gate, user-authorized at arbitration #2; code-reviewer + acceptance-verifier + judges #15 #16 on the R7 decline-gate fixes). 3 PASS / 1 BLOCK. code-reviewer PASS: R6 ISSUE-A + ISSUE-B both confirmed resolved via 6 adversarial inputs (leading wait, nested pre-pair wait, post-pair suspending stmt/expr-arg, post-pair pure-compute correctly still FIRES with output 144, ISSUE-B repro declines 55/55/89); concerns logged (stale prune comment emit.rs:2613, spike-on oracle is manual-only, reload machinery unreachable). acceptance-verifier PASS: all 7 ACs MET on live runs (17 fixtures byte-identical, timing 1283ms<1600ms, 382 driver + 31 codegen + 9 runtime tests green, block_on grep clean in production paths, drop-witness mutation-verified); critical coverage note matches judge #15. judge #16 PASS: entrypoint+post-pair gate conservatively correct — `background worker()` post-pair admitted and harmless (own task, own frame, no ISSUE-B path), R6 repro declines byte-identically, no admitted program reaches emit_suspending_call_inline_poll from post-pair.
- 2026-06-12T16:30 — [R7 SILENT-NARROWING] deviation-judge #15 (identity 76cbd9f): BLOCK. The post-pair gate's `stmt_contains_wait` clause also catches `wait sleep(0)` INTRINSIC waits — sleep uses the inline-poll path via ynz_rt_async_sleep_create/poll and embeds NO child sub-frame, so it poses zero ISSUE-B aliasing risk, yet it declines the group. Consequence (IR-verified): only fixtures (a)-(f) fire the spike; (g)-(q) — 11 of 17 — silently decline (zero ynz_rt_spawn_blocking_joinable call sites in their IR); spike_reload_cpu_results_from_frame (emit.rs:3572-3574) + wait branches (emit.rs:3744/3787-3803) are unreachable in the admitted envelope — the reload machinery built across deviations #9/#14 has zero firing-fixture coverage; fixture (g)/(h)/(n) headers + deviation #15 record factually overstate what they test. All fixtures stay green because sequential fallback is always correct (the silent-narrowing pattern the round was primed to detect). Judge-named narrow fix: strip `stmt_contains_wait` from the POST-PAIR gate only (3 sites: emit.rs:6571/6748/6852), keep `stmt_contains_suspending_call` — sleep is already excluded from that predicate via the M2_MAY_BLOCK_INTRINSICS guard at emit.rs:3264, so (g)/(h)/(i)/(j)/(n) re-admit while the real ISSUE-B class (user-defined suspending callees) still declines. ISSUE-A is pre-pair territory; the pre-pair gate keeps `stmt_contains_wait` unchanged. Round cap exceeded → arbitration #3 to Patrick (round-8 one-clause fix vs accept narrowed envelope with honest documentation).
- 2026-06-12T16:35 — Post-R7 tree verification: delegation intact at runtime.rs:551, zero probe markers, 9/9 m3d runtime tests green (both drop-probes), no stray .ll/binary artifacts, untracked set = 17 fixtures + plan + scratch only.
- 2026-06-12T17:03 — ROUND 8 (user-authorized at arbitration #3: "Round 8: one-clause fix (Recommended)"). Executor applied judge #15's named fix exactly: post-pair gate predicate narrowed to `stmt_contains_suspending_call` only at all three gate sites (emit.rs:6574/6752/6940); pre-pair gates unchanged; stale prune comment at emit.rs:2613 rewritten; fixture headers verified. FIRES table (executor IR-grep, independently re-verified by R8 acceptance-verifier): (g)/(h)/(i)/(j)/(n) 0→2 `call @ynz_rt_spawn_blocking_joinable` instructions each; (m)/(p)/(q) still 0 (decline). Zero new deviations. Focused re-gate (authorized scope): **judge #15 PASS** (fresh adversarial inputs: `wait __testFallibleAsync(false)` post-pair — the OTHER M2_MAY_BLOCK_INTRINSICS member, distinct synchronous-inline codegen path through arm 4426/lower_stmt vs sleep's emit_wait_point — produced correct spike results with and without a following sleep; orphan state slot patched by terminator loop at emit.rs:3652-3667, LLVM verifies; `print(suspendingFn())` arg-position suspending call correctly declines via recursive arg check at emit.rs:3273-3275; Stmt::If recursion identical between both predicates — no structural gap). **acceptance-verifier PASS** (all 4 ACs MET on own live runs: timing 1218ms vs 1780ms < 1600ms; saturation_600_joins green; drop-witness tests green; own IR-level FIRES verification confirms reload machinery now live-exercised — fixtures (h)/(n) fire AND complete post-join waits byte-identically; cargo test --workspace 1924 passed / 0 failed after one harness-race flake re-ran clean; clippy -D warnings clean). R7 silent-narrowing RESOLVED. Phase 0 gate fully green.

---

#### Phase 0 Decision Record (Step 4 — locked 2026-06-11)

**Status**: COMPLETE — All 5 steps done (2026-06-11, devcontainer, LLVM 18). Steps 1 + 4 landed in the prior WSL-host session; Steps 2, 3, 5 completed in the devcontainer continuation session.

**Runtime shims shipped** (`crates/ynz-runtime/src/runtime.rs`):
- `YnzCpuResult([i64; 2])` — the 16-byte POD return ABI (repr C, Copy)
- `CpuJoinHandle(tokio::task::JoinHandle<YnzCpuResult>)` — heap-boxed handle newtype (pub(crate))
- `ynz_rt_spawn_blocking_joinable(fn_ptr, ctx_ptr, ctx_size) -> *mut u8` — copies ctx bytes (FrameDropGuard, mirrors ynz_rt_spawn_blocking discipline), spawns on blocking pool, returns Box::into_raw(CpuJoinHandle)
- `ynz_rt_join_poll(handle, waker_ctx, result_out) -> i32` — `extern "C-unwind"` (permits resume_unwind to cross the frame), casts waker_ctx to `&mut Context<'_>`, pins and polls JoinHandle; Pending=1, Ready(Ok)=writes 16 bytes + drops Box + returns 0, Ready(Err(panic))=resume_unwind
- `ynz_rt_join_handle_free(handle)` — drops Box (detaches); null-safe; called by frame drop shim when parent cancelled mid-join

**Unit tests** (`crates/ynz-runtime/src/lib.rs`, module `m3d_join_shims`):
- `value_roundtrip_int` — [42, 0] survives ABI
- `value_roundtrip_saturation` — [i64::MIN, i64::MAX] survives ABI (decimal128 lo/hi path)
- `ctx_copy_contents_reach_child` — ctx bytes faithfully copied and visible to child
- `drop_detach_no_uaf` — ynz_rt_join_handle_free drops without UAF; child runs to completion (Arc<AtomicBool> witness)
- `panic_reraises_in_parent` — ynz_rt_join_poll re-raises via resume_unwind (extern "C-unwind" allows propagation); catch_unwind in test catches the original panic payload

**Locked decisions**:

1. **Result ABI**: `YnzCpuResult = [i64; 2]` (16 bytes, C repr, Copy).
   Mapping: int/bool → [bits, 0]; float → [f64_bits as i64, 0]; string/array/map → [heap_ptr as i64, 0]; number/decimal128 → [lo_word, hi_word]; T errors → [err_tag, ok_bits].
   Shape and Shape-errors returns are declined by the promotion pass (WideValueSuspendingReturn decline rule).

2. **Poll ABI**: `ynz_rt_join_poll` is `extern "C-unwind"` — allows `resume_unwind` to propagate from within its own body. The codegen-emitted SM resume functions are `extern "C"` (not `extern "C-unwind"`); an unwind that escapes `ynz_rt_join_poll` back into an SM resume function will abort the process at that ABI boundary (RFC 2945). Full end-to-end `C-unwind` propagation — resume functions emitted as `extern "C-unwind"` so unwind reaches Tokio's `catch_unwind` — is a P1 deliverable. `ynz_rt_join_poll` is called directly from `SpawnStateFnFuture::poll` (pure Rust, no C boundary) in the unit test path, where the `extern "C-unwind"` ABI is exercised correctly.

3. **Waker forwarding**: `waker_ctx: *mut u8` is a type-erased `&mut Context<'_>` from the enclosing SM's poll call — identical discipline to `ynz_rt_async_sleep_poll`. No fabricated wakers. The JoinHandle<T> implements Future and is polled with the real waker, so Tokio registers the correct waker and wakes the SM when the blocking task completes.

4. **Ctx-copy discipline**: `ynz_rt_spawn_blocking_joinable` copies `ctx_size` bytes to a heap buffer via `FrameDropGuard` (same as `ynz_rt_spawn_blocking` at runtime.rs:170-181). The child owns its copy; the parent frame may be dropped at any time without dangling the child's args (UAF-on-cancellation prevention from Research Finding 3).

5. **Handle ownership**:
   - Spawn → Box::into_raw → caller stores in frame handle slot
   - Poll Ready → ynz_rt_join_poll drops Box (result already written to frame result slot)
   - Poll Pending → handle unchanged; SM saves resume_point, yields Pending
   - Frame drop (cancellation) → ynz_rt_join_handle_free drops Box; detaches task

6. **Panic re-raise**: `Ready(Err(join_err))` → `resume_unwind(join_err.into_panic())`. Non-panic JoinError (abort) → `panic!` with clear message. The Box intentionally leaks on the panic path (bounded: one per panicking child) — matches Tokio's own JoinHandle panic propagation behavior.
   **Spike scope note**: in the spike, `fn_ptr` is `extern "C" fn`, so a Yinz div-by-zero panic (which calls `process::abort()` from the child thread) aborts the process at the `extern "C"` ABI boundary — BEFORE Tokio's internal `catch_unwind` can form a `JoinError`. The `resume_unwind` branch in `ynz_rt_join_poll` is validated at the Rust unit-test level only (`panic_reraises_in_parent` test uses a pure-Rust panic that crosses `extern "C-unwind"` correctly). The spike fixtures (e)/(f) pass because div-by-zero happens to abort the process on both the sequential and parallel paths, not because the re-raise contract fires end-to-end. P1 production hardening changes `fn_ptr` to `extern "C-unwind"` — that ABI change is what enables the re-raise path to fire end-to-end; the `catch_unwind` wrapper added in the F1 fix is the other half of that contract.

7. **No synchronous join**: poll returns `Poll::Pending` when the child hasn't finished; the SM suspends. No `block_on`, no `thread::park`, no spin-wait anywhere in the join path. The Tokio task harness registers the real waker; the blocking pool signals completion via `JoinHandle`'s internal channel, which wakes the SM.

8. **Promotion fixpoint algorithm** (locked from Research Findings 7–9) — Cancellation cleanup: spike frame handle slots are freed by `cleanup_spike_cpu_handles` (called from `SpawnStateFnFuture::drop` step 1.5). The discriminator at frame offset 4 (`SPIKE_FRAME_MAGIC`) gates the cleanup so normal frames are never misread. `ynz_rt_join_handle_free` is declared and unit-tested (`drop_detach_no_uaf`) but is called via the discriminator path in `SpawnStateFnFuture::drop`, not by spike codegen directly — spike codegen writes handle pointers into fixed slots at spawn time; `Drop` reads them and frees. Evidence: `discriminator_drop_frees_spike_handles` + `spawn_state_fn_future_drop_before_first_poll_frees_handles` tests.
   (a) Intrinsic-rooted may-block fixpoint: mark all `sleep`/`http`/I/O-equivalent calls may-block; propagate transitively bottom-up through the call graph (monotone, terminates at fixpoint).
   (b) Bottom-up promotion pass: in deterministic call-graph order (post-order DFS), for each function, identify independent pure-CPU statement pairs that meet ALL: both callees are NOT may-block, return type maps to YnzCpuResult, not in a cycle, no SM guard fires on either call, `--no-auto-parallel` not set.
   (c) Guard-probe transitive rollback loop: after promoting a callee, re-run the SM suspension guards on every caller of that callee; if any guard fires on previously-compiling code, roll back the promotion (decline silently — no new compile error). Repeat until fixpoint.

9. **Decline rules** (SM-promotion declines when any of these match):
   - Callee return type is Shape or `Shape errors` (WideValueSuspendingReturn)
   - `ShadowsCrossingLocal`, `UnsupportedCrossingLocalType`, `WideValueSuspendingReturn`, or `OwnershipConflictAtSuspend` guard fires on the candidate pair
   - Callee is in a call cycle (recursive group — inter-procedural fixpoint excludes cycles)
   - `--no-auto-parallel` flag set
   - `--kernel` mode (no blocking pool)
   - Either callee is may-block (promotes to I/O SM instead, existing path)

   **Spike admission envelope** (consolidated, locks the spike's proven-safe range; P3 lifts these via typeck analysis + frame_layouts integration):
   - Entrypoint function only (`f.name == "entrypoint"`) — non-entrypoint spike hosts have their frame sized by `emit_suspending_call_heap_boxed` reading the pre-spike `frame_layouts` entry, which is 32 bytes too small (missing the 48-byte spike reserve), causing heap corruption.
   - Zero function parameters — param slots start at FRAME_HEADER_SIZE = byte 32, the same offset as `SPIKE_HANDLE_0_OFFSET`; a param reload would overwrite the handle pointer.
   - Single adjacent pair of non-suspending int-returning direct calls (`second_idx == first_idx + 1`).
   - Exactly one arg per call, `IntLit` or `Ident` only — the trampoline ctx holds one i64; zero-arg and multi-arg callees are not packable.
   - No data-dependency between pair members — the second call's argument must not reference the first call's bind name (evaluated at spawn time before the first result is available).
   - No post-pair rest statement assigns a CPU-result bind name at any nesting depth — the upfront-prune approach left the initial read sourcing from an unpopulated crossing slot; admission decline is the correct fix.
   - No pre-pair statement contains a `wait` or suspending call — a wait in pre_stmts advances `current_state` and positions the builder in a new `post_wait_bb`; the subsequent spawn block has no predecessor branch from that block, leaving `post_wait_bb` without a terminator (LLVM verifier crash).
   - No post-pair statement contains a `wait` or suspending call — after the join, a suspending post-pair callee would be lowered with its child sub-frame embedded at the pre-spike offset (computed before the 48-byte spike reserve). For a 0-local entrypoint that offset equals `SPIKE_RESULT_0_OFFSET` (byte 48), causing the callee's `resume_point` write to alias the joined result (silent wrong value). Routing through `emit_suspending_call_heap_boxed` is not viable because the spike host's `recursion_slot` is `None` (the callee appears in `children`, not in `recursion_slot`), so the heap-boxed path's re-entry code uses the original `child_frame` SSA value across basic blocks, violating LLVM SSA dominance ("Instruction does not dominate all uses"). Declining at the gate routes the whole body through sequential lowering, which is always correct. P3 integrates the spike reserve into `frame_layouts` so embedded sub-frames carry the correct offset and the restriction is lifted.

10. **Mixed groups** (CPU child + I/O sleep child sharing one continuation): the CPU JoinHandle slot and the sleep handle slot are independent fields in the frame. The inline-poll group join loop polls each independently. CPU-finishes-first is the highest-risk case: the Ready join-poll must write result bytes to the CPU result slot and null the handle slot WITHOUT touching the still-Pending sleep handle slot. This is naturally correct: each poll returns its own result independently. **Spike finding (fixture g) — RESOLVED in fix round (2026-06-11)**: the spike's original mixed-body codegen failed with "continuation state 3 out of range" because `spike_extra_states` was set to 1 (accounting only for spawn=0 and poll=1) instead of 2 (spawn=0, poll=1, plus one additional slot for the subsequent sleep continuation). Fixed by: (a) changing `spike_extra_states` to 2, (b) returning `Vec<(String, u64)>` crossing results from `emit_cpu_group_spawn_join`, (c) adding `spike_reload_cpu_results_from_frame` which reloads frame slots into fresh allocas after each subsequent suspension. Fixture (g) now passes: output `55\n89\ndone\n`, exit 0 with `YNZ_M3D_SPIKE=1`. The per-handle independent-poll design is correct; the fix was purely a state-index accounting and cross-suspension reload gap. P3 production lowering generates the canonical unified state table.

**Environment blocker** (resolved): Steps 2/3/5 required LLVM 18. Host (Debian bookworm, LLVM 15) was unworkable; devcontainer (Ubuntu 24.04, LLVM 18) was used for the continuation session.

#### Phase 0 Step 5 — Accept/Reject Verdict (locked 2026-06-11)

**ACCEPT. Proceed to Phase 1.**

**Timing numbers** (fixture c, `v0_3_m3d_spike_c_timing.ynz`, fib(40)/fib(41) on devcontainer):
- Sequential (spike off): ~1795 ms wall-clock (representative run; earlier run measured ~1913 ms)
- Parallel (spike on): ~1256 ms wall-clock (representative run; earlier run measured ~1317 ms)
- Overlap factor: 1256ms < 1600ms threshold (1.6× sequential-each = two ~1.0s workloads in under 1.6s) ✓
- Output byte-identical both paths: `102334155\n165580141\n`, exit 0 ✓

**Gate conditions — all satisfied**:
1. The simplest distinct-callee case (fixture a) works without any synchronous join — poll-based suspension drives the full spawn→suspend→resume→result-bind cycle. ✓
2. The timing overlap materializes (1317ms < 1600ms threshold). ✓
3. No `block_on`, `Handle::block_on`, `thread::park`, or spin-wait appears in any spike code path (grep confirmed). ✓
4. Pool saturation (600 concurrent joins, fixture d) queues without deadlock — the poll-based join structurally cannot deadlock (joiner is suspended, not parked on a thread). ✓
5. Panic re-raise (fixture e) and cancellation-detach (fixture f) behave identically to sequential execution. ✓ (Process-level coincidence: Yinz's div-by-zero calls `process::abort()` from the child thread, which aborts identically on both sequential and parallel paths. The `resume_unwind` end-to-end contract is validated at Rust unit-test level only — `panic_reraises_in_parent` tests the ABI path correctly; the spike trampoline uses `extern "C"` not `extern "C-unwind"`, so the re-raise branch is unreachable through the spike fixtures. P1 production hardens the trampoline ABI to close this.)
6. Same-callee case (fixture b) produces distinct correct values per invocation (per-invocation ctx keying, not callee-name keying). ✓

**Spike limitation — RESOLVED in fix round (2026-06-11)**: the spike's continuation-state table gap (fixture g "continuation state 3 out of range") was fixed by correcting `spike_extra_states` from 1 to 2 and adding cross-suspension result reloading. Fixture (g) now produces `55\n89\ndone\n`, exit 0 with `YNZ_M3D_SPIKE=1`. No remaining known spike limitations that block Phase 0 acceptance.

#### Phase 0 Session Handoff (written 2026-06-11 by the /execute-plan coordinator — local WSL chat → devcontainer chat)

The original chat ran on the WSL host and hit the LLVM 18 environment blocker above. Chats do not transfer into the devcontainer; the working tree DOES (same mounted workspace). Everything a fresh chat needs is in this file + the scratch file. **Nothing is committed yet** — all Phase 0 work is uncommitted on `feat/m3d-cpu-parallelization`. `BASE` for the Phase 0 diff = `plan_base` front-matter (`8b99ac9`).

**Execution settings already chosen by Patrick (do not re-ask)**: phase-by-phase mode, in-place on `feat/m3d-cpu-parallelization` (no worktree), Phase 0 first.

**State at handoff**:
- DONE — Step 1 (runtime shims in `crates/ynz-runtime/src/runtime.rs` +292 lines; re-exports + 5 unit tests in `crates/ynz-runtime/src/lib.rs` +302 lines) and Step 4 (Decision Record above). Verified on host: `cargo test -p ynz-runtime` → 96 green; `cargo clippy -p ynz-runtime -- -D warnings` clean.
- NOT DONE — Step 2 (`YNZ_M3D_SPIKE=1`-gated promotion+lowering path in `crates/ynz-codegen/src/emit.rs`), Step 3 (spike fixtures (a)–(g) via `./target/debug/ynz run`), Step 5 (accept/reject verdict + timing numbers).
- Deviations so far (D_count = 3: 1 scope, 2 approach) persisted verbatim with identity hashes to `.claude/plans/scratch/v0-3-m3d-cpu-parallelization-phase0-deviations.md`. The completed-portion list is PARTIAL — the resumed executor may add deviations from Steps 2/3/5; the resuming coordinator must merge + overwrite the scratch file before the reviewer fan-out (per /execute-plan 3.d.0).
- Executor follow-up observation parked for P1: `CpuJoinHandle` Box intentionally leaks on the panic path (matches Tokio's own JoinHandle panic behavior, documented in the doc comment); P1 evaluates `ManuallyDrop` + explicit free before `resume_unwind`.

**Resume instructions for the new chat (inside the devcontainer)**:
1. Claim the slug (`/init-chat v0-3-m3d-cpu-parallelization` or the write-slug canonical writer), then run `/execute-plan v0-3-m3d-cpu-parallelization`.
2. Step 2 resume detection will classify Phase 0 as partial-state (working-tree changes + scratch file = Case A). IMPORTANT: the executor work is INCOMPLETE — do NOT jump straight to reviewers. First fresh-spawn a `plan-executor` with resume context: "Steps 2, 3, 5 of Phase 0 remain; Step 1 + Step 4 already landed in the working tree (see `git status` + this handoff) — do not re-do them." Devcontainer has LLVM 18; `cargo build --workspace` works there.
3. After the executor completes (including the Step 5 accept/reject verdict — HALT the milestone if the gate fails), merge its new deviations into the scratch file, then run the full 5-reviewer + per-deviation-judge fan-out on the COMPLETE Phase 0 diff (`git diff 8b99ac9` — working tree vs BASE, porcelain will be dirty).
4. Phase-by-phase mode: pause for Patrick's OK before committing Phase 0 and before starting Phase 1.

### Phase 1: Runtime ABI — production joinable spawn + poll + drop shim
**PR scope**: The three production C-ABI runtime functions with full doc comments, runtime-crate unit tests, and `runtime_decls.rs` declarations. No codegen consumer yet.
**Branch**: `feat/m3d-cpu-parallelization`
**Flag**: N/A
**Est. lines**: ~450
**Ships via**: `/pr`
**Objective**: `ynz_rt_spawn_blocking_joinable`, `ynz_rt_join_poll`, `ynz_rt_join_handle_free` hardened to production quality from the spike prototypes: init/shutdown-ordering failure modes (mirroring `ynz_rt_spawn`'s discard-with-warning paths at `runtime.rs:617-628`), panic capture → re-raise contract, waker forwarding identical to `ynz_rt_async_sleep_poll`'s discipline, handle ownership rules documented (Ready consumes; drop-shim detaches + frees).
**Why this phase exists**: the runtime ABI is the foundation P3's codegen emits calls against; landing it standalone keeps the P3 diff reviewable and lets runtime unit tests pin the contract before LLVM IR depends on it.
**Current-state anchors**:
- `crates/ynz-runtime/src/runtime.rs:591` — `ynz_rt_spawn` (error-path patterns to mirror)
- `crates/ynz-runtime/src/runtime.rs:89` — ctx-copy helper (reused for arg copies)
- `crates/ynz-codegen/src/runtime_decls.rs` — extern declarations consumed by codegen
**Files (expected scope)**: `crates/ynz-runtime/src/runtime.rs`, `crates/ynz-runtime/src/lib.rs`, `crates/ynz-codegen/src/runtime_decls.rs`.
**Deviation rule**: (same as Phase 0)
**Steps**:
1. Finalize `YnzCpuResult` = 16-byte POD; document which Yinz return classes map to it (int/bool/float/number/string/array/map/`T errors`) and that Shape returns are out of contract (declined upstream).
2. `ynz_rt_spawn_blocking_joinable`: spawn via `Handle::try_current()` → fallback RUNTIME-mutex path (exact same ladder as `ynz_rt_spawn:591`); closure owns ctx + arg-copies, frees them after the call returns; returns boxed handle ptr. Pre-init/post-shutdown: return null + warning (caller treats null handle as "run inline sequentially" — codegen guards this in P3? NO — codegen always runs after `ynz_rt_init` in generated main; null only occurs in hand-written misuse; document + poll on null = abort with message).
3. `ynz_rt_join_poll(handle, waker_ctx, result_out) -> i32`: poll the `JoinHandle` future with the forwarded real waker; Pending→1; Ready(Ok)→write 16 bytes, drop box, 0; Ready(Err panic)→re-raise (resume_unwind) so the Tokio task wrapper of the PARENT surfaces it exactly as a sequential panic would.
4. `ynz_rt_join_handle_free(handle)`: drop the box (detach). Idempotence rules documented (never called after a Ready poll — slot nulled, mirroring sleep-handle discipline).
5. Runtime unit tests: spawn+join value roundtrip (all 16-byte payload shapes — with an EXPLICIT i128/decimal128 alignment assertion: the `result_out` write path and the frame result slot must satisfy 16-byte alignment; a misaligned i128 store is a silent-wrong-value/SIGBUS class), saturation queueing (≥600 concurrent), drop-detach (task completes after handle freed; ctx freed by closure — asserted via allocation counter), panic surfacing, pre-init discard.
**Acceptance criteria**:
- [x] All three functions exported with `#[no_mangle]` C ABI + doc comments covering Flow/Failure modes/Side effects/Safety (house pattern in runtime.rs)
  - Evidence: acceptance-verifier R4 (live): `ynz_rt_spawn_blocking_joinable` (runtime.rs:~1100, `#[no_mangle] extern "C"`), `ynz_rt_join_poll` (~1247, `extern "C-unwind"`), `ynz_rt_join_handle_free` (~1339, `extern "C"`) — each carries # Flow / # Failure modes / # Side effects / # Safety (+ # Idempotence on free), matching the runtime.rs house pattern; all three `#[no_mangle]`. Big-O lines added inside # Side effects on all 5 concurrency FFI fns for cluster consistency (Patrick-directed).
- [x] Runtime unit tests cover: value roundtrip per payload class, saturation, drop-detach with alloc accounting, panic re-raise, pre-init/post-shutdown discard
  - Evidence: acceptance-verifier R4 ran `cargo test -p ynz-runtime --lib m3d_join_shims` (live). (1) roundtrip per class: value_roundtrip_int/float/ptr/decimal128_aligned(asserts slot%16==0 + aligned i128 read)/error_pair; (2) saturation: saturation_600_joins (640 joins > 512 pool); (3) drop-detach with REAL alloc accounting: handle_free_detaches_and_frees_box (Box free_count==1 drop-probe) + joinable_spawn_frees_ctx_copy_exactly_once (ctx free_count==1 through the REAL ynz_rt_spawn_blocking_joinable + FrameDropGuard); (4) panic: panic_reraises_in_parent (resume_unwind); (5) pre-init: spawn_before_init_returns_null + post-shutdown: spawn_after_shutdown_returns_null (tests/m2_spike.rs). All green.
- [x] `runtime_decls.rs` declares all three for codegen consumption
  - Evidence: acceptance-verifier R4: runtime_decls.rs declares ynz_rt_spawn_blocking_joinable / ynz_rt_join_poll / ynz_rt_join_handle_free as FunctionValue fields + declare_fn wiring (signatures match runtime); P1 diff de-spiked the comments only. Workspace links clean (no missing-symbol errors).
- [x] Zero behavior change to existing exports (full workspace suite green)
  - Evidence: acceptance-verifier R4 live `cargo test --workspace` → 1958 passed / 0 failed (all crates); `cargo clippy --workspace -- -D warnings` clean. All production edits are doc-comment-only or `#[cfg(test)]`-gated (FrameDropGuard.free_probe / CTX_FREE_PROBE / arm_ctx_free_probe / CpuJoinHandle::set_drop_probe) — zero release-build surface change.
**Quality gate**:
- [x] No `block_on`/park anywhere in the new code — code-reviewer + design-compliance R4 grep clean in production paths; the only `block_on` in the diff is in a `#[tokio::test]` panic harness.
- [x] Waker forwarding matches the sleep-poll discipline (no fabricated wakers) — `ynz_rt_join_poll` polls JoinHandle with the forwarded `&mut Context` (cast from waker_ctx), identical to ynz_rt_async_sleep_poll; verified code-reviewer + plan-adherence R4.
- [x] Ownership of ctx/handle/result documented at every transfer point — doc sections + the three-state handle lifecycle; plan-adherence R4 MET.
- [x] Follows existing runtime.rs doc-comment + error-path conventions — Handle::try_current()→RUNTIME-mutex ladder mirrors ynz_rt_spawn; null+warning on pre-init/post-shutdown; plan-adherence R4 MET.
**Verification**: `cargo test -p ynz-runtime` green incl. new tests; `cargo clippy --workspace -- -D warnings`; `cargo test --workspace` green.

**Phase Review Gates**:
- [x] code-reviewer: PASS 2026-06-12T (R4 — Big-O accuracy + ctx-free probe layout-safe, mutation-verified; M2-HALT clean)
- [x] rules-compliance-reviewer: PASS 2026-06-12T (R4 — R3 Big-O BLOCK resolved + deferral-tracked citation closes the Concern; 1 accepted warning-advisory: same-deferral adjacency in a body comment)
- [x] plan-adherence-verifier: PASS 2026-06-12T (R4 — Steps 1-5 MET; m2_spike.rs touch authorized+minimal; banned-phrase grep clean)
- [x] acceptance-verifier: PASS 2026-06-12T (R4 — all 4 ACs MET on live runs; 1958/0; clippy clean)
- [x] design-compliance-reviewer: PASS 2026-06-12T (R4 — no [locked]-doc contradiction; no-synchronous-join invariant intact)
- [ ] Committed: <commit SHA>

**Findings Log**:
- 2026-06-12 — coordinator tier-classification (no Executor tier field): Phase 1 → complex (trigger: "production C-ABI runtime functions … panic capture → re-raise contract, waker forwarding" + M2-HALT-corpse adjacency). Executor dispatched at Opus.
- 2026-06-12 — Phase 1 BASE = 6a183d2 (chore commit isolating Phase-0 /learn bookkeeping; clean boundary so the Phase 1 diff is pure runtime-ABI).
- 2026-06-12 — coordinator probes (pre-gate), all REFUTED: (1) scope — `git diff --name-only 6a183d2` → exactly the 3 expected code files (plan-file change is coordinator writeback). (2) drop-detach contract — `handle_free_detaches_and_frees_box` asserts `free_count == 1` via per-handle drop-probe (real Box-FREE ledger, not an Arc<bool> 'ran' witness — closes the Phase-0 5-round false-contract trap). (3) no-block_on — production diff grep clean. Routing: proceeding to full 5-reviewer fan-out (D_count=0).
- 2026-06-12 — ROUND 1 gate (D_count=0; 5 reviewers). 2 PASS / 3 BLOCK. PASS: code-reviewer (M2-HALT corpse clean; drop-test asserts real Box-FREE; doc honesty holds; 2 non-blocking concerns — pre-existing clippy dup-attr warning in out-of-diff m2_runtime.rs integration test, and the load-bearing "no unit test calls ynz_rt_init" assumption behind spawn_before_init_returns_null), design-compliance (no synchronous join in any production path; CPU routing/detach/panic-reraise all match design/future/concurrency.md; no function coloring). 3 BLOCK → 4 distinct findings:
- 2026-06-12 — rules-compliance round 1: BLOCK. [crates/ynz-runtime/src/lib.rs:2960] `spawn_join_collect` is a Tier-2 test helper with a poll `loop {}` and no Big-O annotation — violates comments.md:361 (Big-O on Tier 2+). (Cleared: 200ms sleep = codebase convention w/ provenance at lines 3164/3295/3522/3648; C-unwind "later phase" deferral language = plan-tracked exemption.)
- 2026-06-12 — plan-adherence round 1: BLOCK. [crates/ynz-runtime/src/lib.rs:3172] Step 5 silent deviation — `handle_free_detaches_and_frees_box` constructs CpuJoinHandle directly via `CpuJoinHandle::new`, BYPASSING `ynz_rt_spawn_blocking_joinable`, so there is no ctx copy under test; Step 5's "ctx freed by closure — asserted via allocation counter" is unmet (Box-free is proven; ctx-free is not). Cross-flag: post-shutdown discard branch (runtime.rs:1086-1090) untested — only pre-init covered. Steps 1-4 MET; scope clean (no emit.rs/state_machine.rs).
- 2026-06-12 — acceptance-verifier round 1: BLOCK (OVERALL). 2 MET (runtime_decls declares all three; cargo test --workspace 1956/0 + clippy clean — verifier's own live run, zero regressions). 2 WEAK: (AC1) `ynz_rt_join_handle_free` (runtime.rs:~1287) has only `# Idempotence` + `# Safety` — missing `# Flow`, `# Failure modes`, `# Side effects` of the four-section house pattern; (AC2) post-shutdown discard branch has zero test coverage (only pre-init `spawn_before_init_returns_null`). Verifier-suggested fix: `spawn_after_shutdown_returns_null` (init→shutdown→spawn→assert null), likely in tests/ integration binary since ynz_rt_init/shutdown are integration-only.
- 2026-06-12 — coordinator probes (fix-loop R1, all LANDED): FIX1 Big-O present at lib.rs:2960; FIX4 all 5 house-pattern headings (Flow/Failure modes/Side effects/Idempotence/Safety) on ynz_rt_join_handle_free; FIX2 `joinable_spawn_frees_ctx_copy_exactly_once` passes (drives real ynz_rt_spawn_blocking_joinable, asserts ctx_free_count==1, executor mutation-proven RED on neutered probe); FIX3 `spawn_after_shutdown_returns_null` passes in tests/m2_spike.rs (executor mutation-proven RED on faked non-null). Re-spawning full gate on updated diff.
- 2026-06-12 — SCOPE NOTE (coordinator-authorized, not a silent deviation): FIX 3 added one test fn to existing `crates/ynz-runtime/tests/m2_spike.rs`, outside Phase 1's declared 3-file scope. Authorized by coordinator's FIX-3 instruction (post-shutdown branch requires ynz_rt_init/shutdown which are integration-binary-only; instruction preferred extending an existing integration file over a new one). Executor reported D_count=0 (rationalized as existing-file extension). Coordinator surfaces it explicitly; plan-adherence asked to confirm the touch is minimal (one test fn, nothing else in m2_spike.rs changed). No deviation-judge spawned — coordinator-directed, exactly-as-wide-as-needed.
- 2026-06-12 — ROUND 2 gate (D_count=0). 4 PASS / 1 BLOCK. PASS: code-reviewer (ctx-free probe test-gated+layout-safe+mutation-verified to bite on a real leak; 2 non-blocking concerns: probe-after-free coupling is not a realistic regression surface, deferral-tracking cross-flag = plan-tracked), design-compliance (no synchronous join; new poll-loops yield via tokio sleep/yield_now; post-shutdown .join() is a raw OS-thread join returning null synchronously — M2-HALT clean), plan-adherence (both R1 BLOCKs closed cold; m2_spike.rs touch minimal + coordinator-authorized; banned-phrase grep clean), acceptance-verifier (all 4 ACs MET on live runs: ynz_rt_join_handle_free has all 5 house headings; post-shutdown + ctx-free pinned with real counters; cargo test --workspace all-green, clippy clean).
- 2026-06-12 — rules-compliance round 2: BLOCK → COORDINATOR ADJUDICATION (CLAUDE.md Rule 11 verify-real-then-fix): NOT CONFIRMED. Finding: [runtime.rs:1310] ynz_rt_join_handle_free gained Tier-3 sections (Flow/Failure modes/Side effects via FIX 4) but lacks `Time: O(1) Space: O(1)` per comments.md:345/361. Verification probe (coordinator, read-only): 0 of 5 FFI functions in runtime.rs carry a Time:/Space: line (ynz_rt_spawn_blocking_joinable, ynz_rt_join_poll, ynz_rt_spawn_blocking, ynz_rt_spawn, ynz_rt_async_sleep_poll all omit it — grep -cE Time: O\( → 0). Grounds for NOT-CONFIRMED: (1) Hard Rule 7's actual trigger is loops/recursion/accumulating-data — ynz_rt_join_handle_free is an O(1) single-statement box-drop with none; (2) the "house pattern in runtime.rs" the AC explicitly references is 0/5 FFI fns with Big-O; (3) round-1 rules-compliance PASSED this same Tier-3 FFI function (Idempotence+Safety) without flagging missing Big-O — the function's tier did not change in R2, only 3 headings were added; (4) complying would put Big-O on 1 of 5 siblings = a NEW inconsistency, or obligate retrofitting all 5 (scope creep in a runtime-ABI phase). Routed to Patrick for overrule at the commit prompt (NOT the fix loop), per the plan's Phase-0 R4/R5 adjudication precedent. No-progress check: R1 hash [lib.rs:2960] ≠ R2 hash [runtime.rs:1310] → genuine progress, not a thrash.
- 2026-06-12 — PATRICK DECISION on the rules adjudication: NOT overrule — chose "Comply across all 5 FFI fns + re-gate" (strict letter-compliance done consistently to avoid a 1-of-5 inconsistency; explicitly accepted pulling the non-M3d sibling FFI fns into the diff). Fix round 2 (round-3 gate): add accurate per-function `Time:/Space:` doc lines to the 5 concurrency-runtime FFI fns — ynz_rt_spawn_blocking_joinable, ynz_rt_join_poll, ynz_rt_join_handle_free (M3d), ynz_rt_spawn/ynz_rt_spawn_blocking, ynz_rt_async_sleep_poll (siblings). Per-function accuracy required (spawn family copies ctx = O(n); poll/free = O(1)). Touching the sibling fns is Patrick-authorized consistency compliance (same-file, in declared scope) — not silent creep.
- 2026-06-12 — coordinator probe (fix-loop R2, LANDED): runtime.rs Time: doc lines 0→6; all 5 named FFI fns carry a Big-O line (ynz_rt_spawn computed O(1) — frame moved not copied; spawn-blocking family O(n) ctx-copy; poll/free/sleep-poll O(1)). Workspace 1958/0, clippy/fmt clean. Re-spawning round-3 gate (delta = 6 doc lines only).
- 2026-06-12 — ROUND 3 gate (D_count=0). 4 PASS / 1 BLOCK. PASS: design ("delta inert"), plan-adherence (doc-only; Steps 1-5 intact; sibling touches authorized), code-reviewer (Big-O accuracy verified vs bodies — spawn-MOVES vs spawn-blocking-COPIES distinction dead-accurate; 1 non-blocking concern re the SM-promotion-phase comment), acceptance-verifier (all 4 ACs MET on live run, 1958/0, clippy clean). rules-compliance R2 finding (ynz_rt_join_handle_free Big-O) RESOLVED. BLOCK: rules-compliance round 3 — NEW finding [lib.rs joinable_spawn_frees_ctx_copy_exactly_once]: poll-loop test fn lacks Big-O (comments.md:361). CONFIRMED-real + consistency-aligned (sibling poll-loop helper spawn_join_collect already carries Big-O). Plus warning-severity Concern: ynz_rt_join_poll "SM-promotion lowering phase" deferral comment — asks coordinator to confirm plan-tracked.
- 2026-06-12 — coordinator confirms the deferral IS plan-tracked: Phase 3 ("Codegen — SM-promotion lowering + CPU children") IS the SM-promotion lowering phase; the end-to-end C-unwind resume-fn ABI is logged as a P1 carry-forward in the plan status (line 28). Exemption satisfied. Round 4 will add an inline `// deferral-tracked:` citation to close the warning permanently.
- 2026-06-12 — coordinator precise probe (read, not crude grep): the ONLY new loop-containing fn lacking Big-O is joinable_spawn_frees_ctx_copy_exactly_once. echo_ctx_first_word (reads one i64) + returns_float/ptr/decimal128/error_pair (return a constant) are loop-free trampolines (Tier 1, no Big-O needed); saturation_600_joins is pre-existing (committed dcc1432, not in the 6a183d2 diff). Round 4 fix is bounded to 1 Big-O line + 1 deferral citation — rules cannot find a 4th.






### Phase 2: Typeck — independence relocation, worth-it analysis, promotion query with decline rules
**PR scope**: Move the independence partition into `ynz-typeck`; add the `does_real_work` fixpoint + `FunctionSig` field with cross-module propagation; implement the promotion query (suspend-set extension) with the full decline-to-promote rule set and transitive guard rollback. No codegen change yet — promoted functions are computed but codegen consumes them only in P3 (suspend-set extension is feature-gated off until P3 wires the lowering, so P2 ships zero behavior change).
**Branch**: `feat/m3d-cpu-parallelization`
**Flag**: N/A
**Est. lines**: ~900 (≈300 is the relocation diff)
**Ships via**: `/pr`
**Objective**: One typeck-owned source of truth for "which functions are state machines and which statement groups parallelize" that guards, inlay hints, cross-module `FunctionSig.suspends`, and codegen all read — per the registry's existing "hint and binary always agree" contract.
**Why this phase exists**: Research Findings 4 + 5 — promotion computed anywhere else either breaks the hint/binary agreement or lets guards fire as errors on previously-valid code. The relocation must land BEFORE feature logic so the move itself is a reviewable no-behavior-change diff.
**Current-state anchors**:
- `crates/ynz-codegen/src/independence.rs:126` — `partition_independent_groups` (moves; deps are only `ynz_ast` + `ynz_typeck` types)
- `crates/ynz-codegen/src/emit.rs:3473` — the codegen call site (re-pointed to the typeck re-export)
- `crates/ynz-typeck/src/may_block.rs:96` — `analyze` (the fixpoint the promotion pass extends; `calls_may_block_intrinsic` plumbing at `may_block.rs:108,168`)
- `crates/ynz-typeck/src/signatures.rs:14,38` — `FunctionSig` / `suspends` (new sibling field `does_real_work`)
- `crates/ynz-typeck/src/check.rs:497,560` — suspending-function guard sites (the predicates the promotion probe reuses)
- `crates/ynz-driver/src/main.rs:81,210,219` — `--no-auto-parallel` plumbing (promotion must respect it)
- `crates/ynz-typeck/src/check.rs:142-186` — kernel-mode flag (promotion disabled under it)
**Files (expected scope)**: `crates/ynz-typeck/src/independence.rs` (new home), `crates/ynz-typeck/src/may_block.rs`, `crates/ynz-typeck/src/signatures.rs`, `crates/ynz-typeck/src/check.rs`, `crates/ynz-typeck/src/queries.rs`, `crates/ynz-typeck/src/exports.rs`, `crates/ynz-codegen/src/independence.rs` (deleted/re-export), `crates/ynz-codegen/src/emit.rs` (import path only).
**Deviation rule**: (same as Phase 0)
**Steps**:
1. **Relocation commit (isolated)**: move `independence.rs` to `ynz-typeck` verbatim (module path + imports only), unit tests move with it; `ynz-codegen` imports from typeck. Full suite green — zero behavior change.
2. `does_real_work` fixpoint beside `may_block::analyze`: a function does real work iff its body contains a loop (`while`/`for`) or it participates in recursion (self or mutual), transitively through calls. New `FunctionSig.does_real_work: bool`, set in the same pre-pass→analysis flow as `suspends` (`signatures.rs:26-38` pattern); propagated cross-module through `module_signatures_query`/`exports.rs` exactly as `suspends` is.
3. Class-aware candidacy in the partition: a group member is EITHER a suspending call (existing criteria, same-callee restriction KEPT) OR a CPU call — non-suspending direct ident call whose callee `does_real_work`, passes the type-based write-effect floor, returns a supported class (NOT Shape/Shape-errors), and is not the enclosing function itself. Same-callee allowed for CPU members (per-invocation identity = statement index). Mixed groups permitted.
4. Promotion query (salsa-tracked): after the intrinsic-rooted may-block fixpoint, walk functions bottom-up in deterministic order; a non-suspending function containing ≥1 CPU-parallelizable group (≥2 members after partition) becomes promotion-candidate; extend the effective suspend set; iterate until stable.
5. Guard-probe transitive rollback: for the candidate set, probe EVERY function that becomes newly-SM (candidates + their transitive callers) against all suspension-guard predicates (`ShadowsCrossingLocal`, `UnsupportedCrossingLocalType`, `WideValueSuspendingReturn`, `MutualSuspensionCycle`, `FixedArrayIterWithWait`, `StoredRangeWithWait`, `ExpressionIterWithWait`, `ArrayShapeRuntimeFieldWithWait`). Any violation → remove the promotion(s) whose propagation caused it; repeat until no violations (monotone downward, converges). Functions in call cycles are never promoted (mutual-suspension prevention); self-recursion is fine (existing heap-boxed child frame support).
6. Gating: promotion disabled under `kernel_mode` and `--no-auto-parallel` (both checked at the query entry).
7. Tests: every guard shape from Research Finding 4 as a fixture that compiled pre-M3d — assert it STILL compiles with zero new diagnostics and is NOT promoted; promotion-positive cases (clean 2-member group → promoted, suspend set extended, callers promoted transitively); cross-module `does_real_work` propagation; cycle decline; `--no-auto-parallel` + kernel decline; determinism (two runs, identical promotion sets).
**Acceptance criteria**:
- [ ] Relocation commit is verbatim-move + import-path-only (reviewable as such) with full suite green before any feature logic lands
  - Evidence: (filled at phase completion)
- [ ] `does_real_work` propagates cross-module (fixture: importer parallelizes calls to an imported loop-containing callee)
  - Evidence: (filled at phase completion)
- [ ] Every guard-shape fixture compiles with IDENTICAL diagnostics pre/post promotion logic and declines silently
  - Evidence: (filled at phase completion)
- [ ] Promotion query is deterministic and respects kernel-mode + `--no-auto-parallel`
  - Evidence: (filled at phase completion)
- [ ] P2 ships zero default behavior change (suspend-set extension not yet consumed by codegen — full fixture corpus byte-identical)
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] No parallel implementation: partition has ONE home (typeck); codegen imports it
- [ ] Probe predicates REUSE the guard implementations (`check.rs`) — not re-derived copies
- [ ] Salsa-tracked queries (incremental-safe); no global mutable state
- [ ] No `unwrap()` on fallible paths; clippy clean
**Verification**: `cargo test -p ynz-typeck` + `cargo test --workspace` green; cross-impl consistency harness on full corpus (must be trivially green since codegen unconsumed); determinism check run twice.

**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty)_

### Phase 3: Codegen — SM-promotion lowering + CPU children in the group poll
**PR scope**: Promoted functions lower through the SM path; the group poll gains the CPU-child class (spawn at group entry, join-poll, result bind, panic re-raise); frame layout gains per-member handle + result slots; trampolines emitted; drop shim frees live handles. The feature goes LIVE in default builds here.
**Branch**: `feat/m3d-cpu-parallelization`
**Flag**: N/A
**Est. lines**: ~1100
**Ships via**: `/pr`
**Objective**: End-to-end working CPU parallelization on real fixtures: distinct-callee, same-callee, mixed CPU+I/O groups, all supported return classes, inside `if`/`match` arms and single loop-iteration bodies (the partition's existing scope), under both modes (`--no-auto-parallel` sequential-identical).
**Why this phase exists**: this is the milestone's working core — everything before it de-risked the mechanism; everything after it is teaching + adversarial hardening.
**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs:1847` — SM dispatch (promoted fns flow here automatically once the P2 suspend set is consumed)
- `crates/ynz-codegen/src/emit.rs:2063` — `lower_function_with_waits`
- `crates/ynz-codegen/src/emit.rs:254` — `build_frame_layouts_with_resolver` (handle/result slot layout extension; keyed (group, member-index), NOT callee name)
- `crates/ynz-codegen/src/emit.rs:6061` — `emit_independent_group_poll` (CPU-child arm)
- `crates/ynz-codegen/src/emit.rs:3473` — partition consumption in `lower_sm_block`
- `crates/ynz-runtime/src/runtime.rs:89` — ctx/arg-copy helper discipline (BgArgDropEntry)
**Files (expected scope)**: `crates/ynz-codegen/src/emit.rs`, `crates/ynz-codegen/src/state_machine.rs`, `crates/ynz-codegen/src/runtime_decls.rs` (if signatures shift), `crates/ynz-driver/tests/fixtures/` + `crates/ynz-driver/tests/integration.rs`.
**Deviation rule**: (same as Phase 0)
**Steps**:
1. Wire P2's extended suspend set into codegen's `cg.suspend_set` (the P2 gate flips on). Promoted functions route through `lower_function_with_waits` unchanged — verify the no-explicit-`wait`, no-sleep-child case lowers cleanly (frame with zero sleep sub-frames, ≥1 CPU group).
2. Frame layout: for each Parallel group with CPU members, allocate per-member (handle slot: i64 ptr; result slot: 16 bytes) keyed (group-id, member-index). Composed-frame size math extended; alloc still once per task tree.
3. Trampoline emission: per (callee, arg-signature) a C-ABI `extern fn(ctx: *mut u8) -> YnzCpuResult` that unpacks args from ctx (scalars by value; heap args from copied pointers), calls the plain compiled function, packs the return into 16 bytes.
4. Group-entry spawn: build ctx (arg evaluation happens BEFORE any spawn, in source order — argument expressions are not parallelized), copy heap args via the BgArgDropEntry-equivalent for joined children, call `ynz_rt_spawn_blocking_joinable`, store handle in the frame slot.
5. CPU-child arm in `emit_independent_group_poll`: per poll visit, `ynz_rt_join_poll(handle, waker_ctx, result_slot_ptr)`; Pending → mark outstanding; Ready → null the handle slot, bind via `load_sm_return_value_typed`-equivalent read from the 16-byte result slot through the existing `bind_sm_result_and_flush` (including the i64→i1 bool truncation discipline from the M3f fix at `emit.rs:5702-5726`).
6. Drop shim: frame drop frees any non-null handle slots via `ynz_rt_join_handle_free` (mirrors sleep-handle cleanup), then arg-copy entries.
7. Mixed groups: the poll loop dispatches per-child class (embedded-frame inline poll vs join poll) — one continuation state for the whole group, same as today.
8. Integration fixtures (all via `./target/debug/ynz run`, each with a `--no-auto-parallel` twin assertion): distinct-callee int; same-callee int (distinct values!); float/number/string/array/map/`T errors` returns; mixed CPU+I/O; group inside `if` arm; group inside a single for-iteration body (iterations still sequential); explicit `wait` barrier splits a would-be group; trivial leaf callee NOT spawned (worth-it respected — assert via IR or debug output); promoted-caller chain (caller of promoted fn is SM, output correct).
**Acceptance criteria**:
- [ ] All step-8 fixtures pass with byte-identical default vs `--no-auto-parallel` output
  - Evidence: (filled at phase completion)
- [ ] Same-callee fixture binds DISTINCT correct values (the per-invocation keying proof)
  - Evidence: (filled at phase completion)
- [ ] Timing fixture: two heavy CPU calls complete in measurably less than sequential sum
  - Evidence: (filled at phase completion)
- [ ] alloc=free on every new fixture (frames, ctx, handles, arg copies)
  - Evidence: (filled at phase completion)
- [ ] Full existing corpus green — zero regressions (`cargo test --workspace`)
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] No synchronous join anywhere (diff grep)
- [ ] Slot I/O routes through the canonical helpers (`flush_var_slot_to_frame`/`reload_params_from_frame`/`bind_sm_result_and_flush`) — corpse guard (a) from M3b respected
- [ ] No flat-scan re-derivation — partition consumed from typeck only (corpse guard (b))
- [ ] Arg evaluation order = source order, pre-spawn (no observable reorder of argument side effects)
**Verification**: `cargo test --workspace`; cross-impl harness over full corpus; `valgrind`/alloc-counter run on the new fixtures per existing M3a/M3b practice.

**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty)_

### Phase 4: Teaching surface + docs + demo
**PR scope**: `parallel_groups` registry domain + LSP inlay pass + hover text; `wait_on_non_may_block` reword; VSCode bump + screenshot; pirates-roster CPU section; design/spec doc updates; error-gallery considered-note.
**Branch**: `feat/m3d-cpu-parallelization`
**Flag**: N/A
**Est. lines**: ~600
**Ships via**: `/pr`
**Objective**: The full teaching surface mandated by roadmap Constraint "Full teaching surface ships in the same milestone": every compiler scheduling decision this milestone makes is visible in the IDE with WHAT/WHAT-INSTEAD/WHY, the demo shows it in context, and the docs describe the locked model.
**Why this phase exists**: invisible auto-promotion is magic; magic is anti-Yinz (`.claude/rules/auto-promotion.md` Banned Anti-Pattern 1). This phase also closes the M3b-era gap where parallel groups had NO inline IDE surface (design/concurrency.md "IDE Execution Plan — Non-Negotiable").
**Current-state anchors**:
- `registry/features.toml:2086` — `background_routing` entry (format precedent for the new domain)
- `registry/features.toml:1553` — `wait_on_non_may_block` why_template (reword target)
- `crates/ynz-typeck/src/inlay_hint_passes.rs:1495` — `background_routing_hints` (pass pattern to mirror; new pass consumes the typeck partition directly)
- `crates/ynz-lsp/` — inlayHint handler wiring (mirror the M3b domain hookup)
- `examples/pirates-roster/entrypoint.ynz` — demo growth point
**Files (expected scope)**: `registry/features.toml`, `crates/ynz-registry/` (regenerated constants), `crates/ynz-typeck/src/inlay_hint_passes.rs`, `crates/ynz-lsp/src/`, `tooling/vscode-ynz/` (version bump + screenshot), `examples/pirates-roster/entrypoint.ynz`, `examples/primantis-orders/v0_3_m3d_errors.ynz` (considered-note header), `design/concurrency.md`, `spec/concurrency.md`.
**Deviation rule**: (same as Phase 0)
**Steps**:
1. Registry: `[[muted_hint_domain]] parallel_groups` (Informational; example_hint_rendered like `// runs at the same time as line 12 — separate core` for CPU members / `// runs at the same time as line 12` for I/O members — final wording jargon-audited, no "thread"/"spawn"/"pool" leakage per vocabulary.md).
2. Inlay pass `parallel_group_hints` consuming the typeck partition (the same query codegen reads — agreement guaranteed by construction); hover: WHAT (these statements overlap because no data flows between them), WHAT-INSTEAD (write `wait` before the second call to force order), WHY (contextual — names the actual bindings/lines).
3. Reword `wait_on_non_may_block` why_template: explicit `wait` on a CPU-bound callee is an ordering barrier (prevents overlap with surrounding independent statements); update the fixture snapshots that carry the old text.
4. `design/concurrency.md`: new "CPU Statement Parallelization (M3d)" section (promotion + decline rules verbatim from the P0 decision record, panic re-raise, cancellation detach constraint, worth-it proxy, same-callee scope amendment to the M3b divergence entry). `spec/concurrency.md`: HS-grad-tone subsection.
5. `examples/pirates-roster/entrypoint.ynz`: CPU multicore section (realistic two-stat crunch over the roster) with comments pointing at the hint; insta snapshot. `examples/primantis-orders/v0_3_m3d_errors.ynz`: header note — M3d ships no new compile-error classes (decline is silent by design); file exists so the per-milestone gallery convention holds.
6. VSCode extension: version bump, `cpu-parallel-hints.png` screenshot, hover-doc text carried via registry regeneration.
**Acceptance criteria**:
- [ ] `parallel_groups` hints fire on the pirates-roster demo section in a live LSP session (screenshot evidence)
  - Evidence: (filled at phase completion)
- [ ] Hint/binary agreement: the inlay pass and codegen consume the SAME partition query (cited import paths)
  - Evidence: (filled at phase completion)
- [ ] `wait_on_non_may_block` reworded + snapshots updated; jargon_audit green over all new user-facing text
  - Evidence: (filled at phase completion)
- [ ] `design/concurrency.md` + `spec/concurrency.md` sections landed; M3b divergence entry amended
  - Evidence: (filled at phase completion)
- [ ] VSCode version bumped, screenshot added, registry constants regenerated
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Hint placement category is Informational ONLY (no mixed surfaces — inference.md consistency rule)
- [ ] WHY text is contextual (names actual lines/bindings), not generic
- [ ] Registry entry precedes code consuming it (feature-registry.md discipline)
**Verification**: `cargo test --workspace` (snapshot suites green); manual LSP session against the demo; jargon audit test green.

**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty)_

### Phase 5: Adversarial gate + perf spike + verification sweep
**PR scope**: The mandatory adversarial gate (danger matrix + stress + cancellation/panic hostile fixtures), the worth-it perf calibration spike with recorded evidence, the cross-impl consistency run over the entire corpus, compile-time budget check, and the Step-10 verification sweep (TODO sweep, quality checklist, cumulative review).
**Branch**: `feat/m3d-cpu-parallelization`
**Flag**: N/A
**Est. lines**: ~700 (mostly fixtures + tests)
**Ships via**: `/pr`, then `/release` for `v0.3.0-m7`
**Objective**: Prove the milestone hostile-input-clean before release. The roadmap marks this gate MANDATORY: the CPU join is M2-HALT-corpse-adjacent.
**Why this phase exists**: M3a P2's shadow saga (6 fix-rounds, round 6 a silent miscompile) and M3e's adversarial round (3 live SIGILLs found) prove this codegen area is whack-a-mole-prone; a structured matrix catches what feature-development testing doesn't.
**Current-state anchors**:
- M3e's 19-fixture danger-matrix pattern (plans/done/v0-3-m3e — structure precedent)
- `crates/ynz-driver/tests/integration.rs` — fixture harness
**Files (expected scope)**: `crates/ynz-driver/tests/fixtures/`, `crates/ynz-driver/tests/integration.rs`, this plan file (evidence write-back), `design/concurrency.md` (calibration constant note if the spike adjusts the proxy).
**Deviation rule**: (same as Phase 0). Any bug found routes to a fix commit within this phase if it's an M3d defect; CONFIRMED pre-existing bugs get the M3f treatment — tracked loudly on the roadmap/todos, not silently fixed here.
**Steps**:
1. Danger matrix fixtures (every cell via real `ynz run`, multiple runs to catch garbage reads, alloc=free): return classes {int, bool, float, number, string, array, map, int errors, number errors} × {distinct-callee, same-callee} × {CPU-only group, mixed group} × position {top-level, if-arm, match-arm, single loop-iteration body} — pruned to the ~25 highest-risk cells with the pruning rationale recorded (full cross-product is ~288; pruning keeps every unique mechanism pairing).
2. Hostile fixtures: blocking-pool exhaustion (recursive promoted fn fanning out groups, total spawns ≫ 512 — must complete); cancellation mid-join (parent task cancelled while ≥1 CPU child outstanding — no UAF, no leak, detached child completes harmlessly); panic-in-child (re-raise identical to sequential); CPU child that itself calls `background` (spawn-from-blocking-pool-thread path); group whose members finish in reverse order — INCLUDING the mixed variant where the CPU child completes before the I/O child suspends (shared continuation state, different child classes, out-of-declaration-order completion); a SELF-RECURSIVE promoted function containing a CPU group (recursion heap-boxing × per-member handle/result slots intersecting — the two allocation mechanisms the frame-composition decision treats separately); a worth-it FALSE-NEGATIVE leaf (heavy straight-line callee with no loop/recursion — assert the decline is silent and output identical, confirming the proxy is a perf gate, never a correctness gate); zero-iteration and 10k-iteration loop bodies containing groups; deep promoted-caller chains (A→B→C all promoted by propagation).
3. Worth-it perf spike (CI machine): measure spawn+join overhead vs workload size; confirm the loop/recursion proxy leaves no pathological small-workload regression (a loop of 3 iterations spawning) — if it does, add a calibrated constant (e.g. minimum estimated iteration weight) with benchmark evidence recorded here + design doc, same protocol as auto-SoA SIZE_THRESHOLD.
4. Cross-impl consistency: EVERY `examples/` program + EVERY `crates/ynz-codegen/tests/` + driver fixture, default vs `--no-auto-parallel`, byte-identical stdout/stderr/exit.
5. Compile-time budget: `ynz build` wall-clock on pirates-roster, pre vs post — <10% increase (roadmap target).
6. Step-10 sweep: TODO/FIXME/phase-comment grep; todos.md cross-check; shortcut detection; quality checklist with evidence; plan-file final persistence; cumulative Opus reviewer fan-out per the protocol.
**Acceptance criteria**:
- [ ] Danger matrix green (cell list + pruning rationale recorded in the PR)
  - Evidence: (filled at phase completion)
- [ ] All hostile fixtures green incl. exhaustion, cancellation, panic, reverse-completion
  - Evidence: (filled at phase completion)
- [ ] Perf evidence recorded: overlap speedup numbers + worth-it calibration verdict
  - Evidence: (filled at phase completion)
- [ ] Cross-impl consistency green over the ENTIRE corpus
  - Evidence: (filled at phase completion)
- [ ] Compile-time increase <10% on pirates-roster (numbers recorded)
  - Evidence: (filled at phase completion)
- [ ] Step-10 sweep complete; cumulative reviewer fan-out PASS
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] No fixture asserts interleaving-dependent stdout (aggregate/post-join discipline)
- [ ] Multiple-run garbage detection applied to every matrix cell touching frame slots
- [ ] Any pre-existing bug found is tracked loudly (roadmap/todos), not silently patched or silently skipped
**Verification**: full matrix + hostile suite via `cargo test --workspace`; timing/perf numbers in PR + plan; `/release` readiness check (Cargo.toml bump to `0.3.0-m7`, CHANGELOG, VSCode `.vsix` pair incl. `yinz-latest.vsix` clobber).

**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log**:
_(empty)_

## Quality Checklist (verify at completion)
- [ ] All inputs validated (N/A web-style — compiler: all new analyses conservative-correct, decline on uncertainty)
- [ ] No synchronous join anywhere in the milestone diff (grep evidence)
- [ ] Promotion never errors: guard-shape corpus identical diagnostics pre/post
- [ ] Cross-impl consistency: full corpus byte-identical default vs `--no-auto-parallel`
- [ ] alloc=free on all new fixtures
- [ ] Teaching surface complete: registry + LSP + hover + VSCode + demo + docs
- [ ] Tests: happy path + error cases + edge cases (danger matrix + hostile fixtures)
- [ ] Existing tests still pass (full workspace, 1929+ baseline)
- [ ] Types complete (no `unwrap()` on fallible paths, clippy `-D warnings` clean)
- [ ] Follows existing codebase conventions (corpse guards a+b, canonical slot helpers, doc-comment house style)
- [ ] Every phase received all-reviewer + all-judge PASS before committing
- [ ] Final cumulative reviewer sweep passed (Step 10f)
- [ ] Plan-file acceptance-criteria checkboxes accurate across all phases
