---
slug: v0-3-concurrency-perf
type: roadmap
owner: Patrick Rizzardi
status: active
created: 2026-05-21
last_updated: 2026-05-31
milestones:
  - v0-3-m1-runtime-and-background
  - v0-3-m2-wait-and-state-machines
  - v0-3-m3-auto-parallelization
  - v0-3-m4-channels-soa-release
---

# Roadmap: v0.3 — Auto-Parallelization + Auto-SoA

## Vision

By the end of v0.3, existing v0.1 Yinz code that uses `wait` / `background` actually runs concurrently instead of sequentially. Independent operations in a function body auto-parallelize without any user action. `array<Shape>` hot loops that access only 1-2 fields get SoA layout automatically — 10-40× faster on cache-heavy workloads — with zero syntax change.

From the user's perspective: code they already wrote just gets faster. No rewrite required. No new keywords to learn (the user already wrote `wait` and `background` in v0.1 — they just didn't run concurrently yet). No opt-in flags. The compiler does the hard work.

This is the "Rust-level performance, TypeScript-level readability" promise delivered at runtime, not just at compile-time type safety. After v0.3, a junior Yinz developer writing sequential-looking code gets multi-core parallelism and SIMD-ready data layouts for free.

---

## Why Now

v0.1 shipped the full language surface (tag `v0.1.0`, 830 tests, M1–M8). v0.2 shipped the dev-loop tooling (LSP, fmt, watch — tag `v0.2.0`). Per `design/mvp-scope.md`, v0.3 is the concurrency + performance slot — the compiler's dependency-graph analysis and auto-SoA optimization.

Three things are now in place that weren't possible in v0.1:

1. **Ownership system is working.** M4's `share`/`lend`/`give` + `noalias` LLVM attributes give the compiler static aliasing proofs — the essential ingredient for both safe auto-parallelization (no data races by construction) and auto-SoA (safe layout transformation because field aliasing is provably non-overlapping).

2. **`wait`/`background` have correct sequential semantics.** v0.1 implemented `wait` as a passthrough and `background` as a synchronous call (confirmed: `emit.rs` line 3089 runs the inner expression to completion). The syntax is locked, the type checker validates ownership at spawn boundaries — only the runtime concurrency is missing.

3. **Salsa-backed incremental compiler.** The salsa query graph gives us the dependency information we need. Module-level queries are already tracked. The v0.3 may-block analysis is a new salsa-tracked pass on top of the existing `check_query`/`signature_query` infrastructure.

The "why now vs later" check from `design/mvp-scope.md`: delaying past v0.3 means users write `background doThing(data)` thinking it runs concurrently, but it actually runs sequentially — a correctness illusion. Every version past v0.1 that doesn't fix this extends that illusion.

---

## Constraints

- **No new user-facing syntax.** v0.3 is a compiler-internal milestone. The language surface (keywords, type system, ownership model) doesn't change. `wait`, `background`, `array<T>`, `shape` all stay exactly as they are.
- **Existing programs must produce identical output.** The only change is PERFORMANCE (faster) and SCHEDULING (concurrent execution of independent operations). Semantics are preserved: `wait` still means "complete before continuing"; `background` still means "run independently"; ownership still prevents data races.
- **No GC.** Auto-parallelization uses ownership semantics to eliminate races at compile time. No reference counting introduced for sequential code; `Arc`-equivalent only added cross-thread where ownership requires it.
- **Tokio is an internal implementation detail.** The work-stealing scheduler is provided by Tokio, bundled inside `libynz_rt.a`. Users never see Tokio, never write Rust async, never annotate functions with `async`. The no-function-coloring design (per `design/future/concurrency.md`) is preserved: every function is still just `function`.
- **State machines are compiled to LLVM IR by `ynz-codegen`, not by Rust async/await.** Yinz controls the state machine transformation; Tokio provides the scheduler that polls them. Clean separation — the scheduler can be swapped without touching codegen.
- **All v0.2's rules carry forward:** WHAT/WHAT-INSTEAD/WHY diagnostics, plan invariants per `.claude/rules/plan-invariants.md`, feature registry entries for any new muted-hint domains or lint rules.
- **No kernel-mode concurrency.** `--kernel` mode programs (per `design/future/no-runtime-mode.md`) get a compile error if they use `wait` or `background` — the Tokio-backed scheduler doesn't run in kernel mode. Same rule as v0.1 (currently sequential-only anyway).
- **Full teaching surface ships in the same milestone as the feature — no exceptions.** Every milestone that adds a new compiler behavior MUST include ALL of the following in the same milestone (not a follow-up): (1) registry entry for any new muted-hint domain or lint rule in `registry/features.toml`; (2) LSP inlayHint handler extension or new lint emission for the feature; (3) WHAT/WHAT-INSTEAD/WHY text for every new compile error or warning introduced; (4) updated hover docs in the registry for any keyword whose behavior changes (`wait` and `background` both change meaning in v0.3 — hover text must be updated before the milestone tag ships); (5) VSCode extension version bump + at least one screenshot showing the new IDE surface; (6) `examples/pirates-roster/entrypoint.ynz` extended with a section demonstrating the new feature with inline comments pointing to the IDE behavior. If any of these surfaces can't fit in the milestone, they require a named `[[deferred_tooling_feature]]` entry in the registry with an explicit trigger for when they ship — NOT a silent omission. This constraint exists because forgetting one surface means the feature is invisible in the editor until someone remembers to fix it, which historically doesn't happen until a user complains.

---

## Architectural Decisions Made

These are locked before any v0.3 execution plan starts. Each milestone's execution plan must conform.

- **M2/M3 boundary correction — may-block analysis ENGINE belongs in M2, not M3 (DECIDED 2026-05-31, Patrick)**: The original split (M2 = state machines; M3 = may-block analysis) was unbuildable: a CORRECT state-machine layer requires knowing which functions suspend, and without that analysis M2 was forced into a runtime `block_on` sync bridge as a stopgap. That bridge (a) crashed at runtime (the M2 Phase-5 MILESTONE-INTEGRITY HALT — "Cannot start a runtime from within a runtime") and (b) contradicted `design/future/concurrency.md` ("No Function Coloring": whole-program transitive may-block analysis + auto-inserted `wait`, stackless state machines, NO bridge anywhere). Three rounds of adversarial plan-review + a P0 gate + five per-phase gates never caught it because every review checked the plan against itself, never against the design doc. **Correction:** the intra-compilation-unit transitive may-block fixpoint (may-block set = `{sleepAsync}` for M2) moves M3→M2 — it's load-bearing for M2 correctness; M3 EXTENDS it cross-module (M8 prereq) + adds auto-parallelization + I/O-auto-`wait`. — Rejected: keeping the bridge (ships a known runtime crash + violates the design + "back to JS/Rust thread-holding"). Rejected: requiring explicit `wait` / function coloring in M2 (reverses the locked no-coloring decision — `design/future/concurrency.md` line 11). WHY this is the right answer: it IS the documented design; it ships zero slow paths; it builds the exact substrate M3 needs (vindicating the original HALT decision "do the right long-term thing — M3 builds on M2's substrate"). Enforcement going forward: `.claude/rules/plan-invariants.md` "Design-Doc Alignment" + project CLAUDE.md "design docs are governing" + graveyard "Plan Contradicts a Governing Design Doc" — added 2026-05-31 so no plan again drifts from a design doc unreviewed.

- **State-machine frame allocation = COMPOSITION, not per-call heap allocation (DECIDED 2026-05-31, Patrick — "absolute fastest" requirement)**: A child state machine's live-across-suspension state composes INTO the parent's frame, computed at compile time (the same mechanism Rust uses to nest `async fn` futures). Heap allocation happens ONLY at (a) `background` spawn points — one allocation per spawned task tree — and (b) recursion points (break the otherwise-infinite frame size, à la Rust's `Box::pin` for recursive async). — Rejected: heap-allocating a frame per state-machine call (simpler but leaves allocations on the floor and misses `design/future/concurrency.md`'s "low memory, fast spawn — like Rust's async" target; violates Golden Rule 8 zero-cost-abstractions). This is the allocation model M2's `lower_function_with_waits` must implement. Asterisks NOT taken in v0.3 (tracked, not lost): work-stealing scheduler stays (thread-per-core revisited only if a workload demands it); `io_uring`/`kqueue`/IOCP I/O backend is captured in `design/stdlib/filesystem.md` for the v0.5 file module. Bottom line locked: stackless state machines + compiler-inferred coloring + composed frames = the known-fastest task-concurrency model for analyzable code (Rust's performance ceiling, Go's ergonomics).

- **Tokio as embedded scheduler**: Tokio's multi-thread work-stealing runtime is bundled inside `libynz_rt.a`. Generated Yinz binaries call `ynz_rt_init()` at startup (the Yinz equivalent of `#[tokio::main]`) which boots the Tokio runtime. The Tokio runtime stays alive for the program's lifetime. — Rejected: writing a work-stealing scheduler from scratch. Rationale: Tokio is 5+ years of production hardening (epoll/kqueue/IOCP, work-stealing). The novel part of v0.3 is the COMPILER analysis, not the scheduler. Spending 4-8 phases building a scheduler from scratch is the wrong use of time.

- **Tokio preemption model vs. design doc compile-time checkpoints**: `design/future/concurrency.md` "Scheduler Preemption Model" locks compile-time-assisted safe-point insertion at loop back-edges and function call sites. Tokio 1.x's budget system yields at `await` points only — it does NOT insert preemption checks at loop back-edges. This means: for the initial v0.3 implementation, Yinz MUST emit its own preemption check calls at loop back-edges and function call sites per the design doc, using a runtime helper `ynz_rt_check_preempt()` that calls into Tokio's cooperative yield mechanism. Tokio's budget system alone is insufficient for pure-CPU loops. The design-doc time quantum (~10ms) maps to a Tokio budget override at runtime init. — This is resolved in M1 P0 research spike; if the cost of check insertion is too high, the design doc's loop back-edge requirement gets relaxed with explicit rationale.

- **State machine transformation in `ynz-codegen`**: Functions containing `wait` are transformed into state machines in LLVM IR. The state machine struct holds the function's live variables across `wait` points. Each `wait` point desugars to a Tokio-compatible `waker.wake()` + `Poll::Pending` return from the state machine's `resume()` function. The transformation happens in `crates/ynz-codegen/src/emit.rs` — a new `lower_function_with_waits()` path parallel to the existing `lower_function()`. — Rationale: Yinz can't use Rust's `async`/`await` desugaring (Rust-specific compiler feature; Yinz compiles to C-ABI LLVM IR). But Yinz CAN emit the exact state machine IR that Rust's async desugars to, interfacing with Tokio's `Future` polling protocol via the `ynz_rt_spawn(fn_ptr, ctx)` bridge.

- **`ynz_rt_spawn` ABI bridge**: The runtime exposes `extern "C" fn ynz_rt_spawn(fn_ptr: *const fn(*mut c_void) -> i32, ctx: *mut c_void)` — a C-ABI function that wraps the Yinz state machine in a Tokio `Future` and spawns it onto the work-stealing scheduler. The `ctx` pointer carries the state machine's heap-allocated frame (owned by the task; dropped when the task completes). — Design detail: resolved in v0.3-M1 P0 research spike.

- **`background` semantics in M1 — blocking thread pool, not I/O scheduler**: In v0.3-M1, `background fn(args)` compiles to `ynz_rt_spawn_blocking(fn_ptr, args_copy)` — using Tokio's `spawn_blocking` (the blocking thread pool), NOT `spawn` (the I/O work-stealing scheduler). Why: M1 ships before state machine desugaring (M2). A background function that contains `wait` calls internally would run those `wait` calls synchronously (blocking behavior), which must NOT run on the I/O scheduler threads (it would starve I/O completions). `spawn_blocking` routes to a separate OS-thread pool designed for blocking work. After M2 ships state machines, background functions whose call graph is may-block get routed to the I/O scheduler; CPU-bound functions stay on spawn_blocking (per the CPU/I/O routing decision). The M1 value statement: "`background fn()` runs on a separate thread from main — the main thread continues immediately." True for ALL function types in M1; I/O-blocking functions don't block main, they block their blocking thread.

- **`background` ownership inference timing = M3**: `design/concurrency.md` "Compiler inference for ownership" locks: value unused after background call → auto-`.give`; value used after → auto-`.copy`. This auto-inference requires the call-graph analysis that ships in M3 (to determine whether a value is used in a transitive path after the `background` call). Until M3 ships, users MUST write `.give` or `.copy` explicitly at every `background fn()` call site — the v0.1 explicit-annotation typeck check (`check.rs:1216`) stays in place through M1 and M2. M3 execution plan must specify "lift auto-give/copy inference for background call sites" as a named phase step.

- **`wait`-in-expression semantics**: Multiple `wait` calls within a single expression are treated as INDEPENDENT concurrent operations when no data dependency exists between them. Example: `let x = wait a() + wait b()` — both `a()` and `b()` are scheduled concurrently; the `+` operation executes only when both complete (implicit join barrier at the expression evaluation). This is consistent with the auto-parallelization premise: the compiler parallelizes independent operations whenever the ownership system proves no shared mutable state. Explicit `wait` at a call site is a hint to the compiler ("I need this result"), not an ordering constraint. User-required ORDERING between side effects is expressed by placing the `wait` calls on separate statements with a data dependency (or using explicit `wait` sequencing per `design/concurrency.md` "When `wait` is necessary").

- **Auto-`wait` insertion scope (M3) = suspension correctness only, not ordering**: M3 auto-inserts `wait` at call sites where the callee is may-block AND the call site doesn't already have an explicit `wait`. This ensures the compiler-generated suspension point prevents blocking the I/O scheduler thread. This is DISTINCT from user-written `wait` for ORDERING: `wait chargePayment(order)` before `sendConfirmationEmail(order)` where the user needs sequential side-effect ordering. Auto-inserted `wait` does NOT guarantee ordering between independent may-block calls — those run concurrently. The compiler only inserts `wait` to ensure the function body can suspend correctly; ordering is the user's responsibility via data dependencies or explicit `wait`. — Consequence: `M3.auto-wait` and `M3.auto-parallelize` are the SAME feature: auto-`wait` + auto-`background` for independent groups. They ship together, not as separate passes.

- **Auto-SoA decision criteria** (from `design/future/auto-soa.md`): SoA layout applied when: (1) array length > SIZE_THRESHOLD entries at the analysis-time proof point — initial constant is 64; M4 P0 must run a benchmark spike (workload: physics-update loop over `array<Player>` with `x`/`y` field access, measured on the CI machine) and either confirm 64 or update with a new constant + benchmark evidence, (2) hot loop accesses ≤ 2 fields, (3) no FFI export of the array, (4) the shape has NO `lend self` methods that access more than the hot-loop fields (if multiple fields are accessed via mutable methods, SoA splits what the method expects to be contiguous — suppress transform for that shape). Codegen-only change; Tier 3 lint suggestion surfaces the transform decision. Debugger integration for v0.3 is best-effort (may ship as a `[[deferred_tooling_feature]]` entry if DAP work exceeds 3 phases).

- **Work-stealing scheduler thread count = CPU cores**: Tokio's default thread count is `num_cpus::get()`. No user-facing configuration in v0.3 (that's a potential v1.x lint-config surface if demand surfaces). CPU-bound tasks auto-route to `tokio::task::spawn_blocking` (Tokio's blocking thread pool) per the call-graph analysis.

- **Atomic ordering for channels and auto-Arc = acquire-release default**: Per `design/future/concurrency.md` "Atomic Ordering Default" (locked pre-v0.2). Channel `.send()` / `.receive()` operations and auto-Arc reference-count operations use acquire-release memory ordering. Sequential-consistency is not the default. The user-facing opt-in API for the seq-cst global-order case is named TBD at M4 execution-plan time; it ships as a `[[deferred_language_feature]]` entry if the API surface is too broad for M4. — Consequence: code relying on seq-cst assumptions without the explicit opt-in WILL produce wrong results under acquire-release. The no-jargon user surface does not expose `memory_order_*` — the opt-in is via a named method (candidate: `.withGlobalOrdering()` per design doc, final name TBD at M4 P0).

- **`background` task panic behavior = best-effort discard**: Per `design/concurrency.md` "Error Cancellation." When a background task panics, the panic does NOT propagate to the spawning scope — it's caught by the Tokio task wrapper and logged (or silently discarded if no log module is available). The spawning scope never sees the panic; it only sees the join barrier complete. This matches Tokio's `JoinHandle::await` behavior on panic (returns `Err(JoinError)`), but Yinz's fire-and-forget form does NOT expose a result — the panic information is discarded. The supervisor pattern (`design/future/supervisor.md`) is deferred to a future version. — Consequence for v0.3: if a `background` task panics, the program continues. This is documented in the error gallery and WHAT/WHAT-INSTEAD/WHY for the `background` keyword hover in the LSP.

- **Cross-implementation consistency test harness**: All execution plans from M1 onward MUST include a fixture-based semantic equivalence test that runs every `examples/pirates-roster/` program and every `crates/ynz-codegen/tests/` fixture in BOTH (a) a forced-sequential mode (`ynz build --no-auto-parallel` flag, added in M1) and (b) default auto-parallel mode, and asserts identical stdout/stderr/exit-code. This flag ALSO serves as the regression guard if auto-parallelization introduces a scheduling bug. `--no-auto-parallel` must be supported from M1 onward as a mandatory test utility even if it only matters from M3 when actual auto-parallelization begins.

- **Reordering verification technique**: Property tests for "lend-sequenced operations are never reordered" use the `--no-auto-parallel` flag to establish the sequential baseline, then run under `--deterministic-schedule` mode (a separate Tokio runtime flag that uses a round-robin scheduler for reproducible task ordering). Concrete test: a program where operation A writes to a binding and operation B reads from it — B must always see A's write. If the ownership system allows both to compile, the `lend`/`share` semantic guarantees ordering; if both compile correctly the ownership system would have rejected the concurrent form already. The actual property test is: any program that compiles under M3+ produces identical output in `--no-auto-parallel` and default modes.

- **Feature Registry Entries mandate for all v0.3 execution plans**: Each v0.3 milestone execution plan MUST include `### Feature Registry Entries` per `.claude/rules/plan-invariants.md`. Known entries across the milestone: M1 may add `[[deferred_tooling_feature]]` entries for background-handle-form; M3 MUST activate the `wait_points` muted-hint domain (currently in `registry/features.toml` as protocol-only); M4 MUST activate the `channel_capacity` muted-hint domain + add Tier 3 lint rules `array-using-soa-layout` and `cross-thread-fields-not-padded` to the registry.

- **Required Pre-Work before v0.3-M1 starts — DECIDED 2026-05-21**: Two pre-work items ship as dedicated PRs before M1 begins:
  1. **Parser infinite-loop bug**: `v0_2_m1_errors.ynz` and `m1_errors.ynz` cause the parser to hang on error recovery. **Decision: fix it** (dedicated PR before M1 P1). Workaround exclusion is duct tape per `no-duct-tape.md` Rule 10.
  2. **M8 cross-file typeck**: `m8-typeck-cross-file-resolution` (`plans/active/`, `pending_approval`) must be APPROVED and EXECUTED before M3 starts. **Decision: M8 is a hard prerequisite for M3** — no LSP-workaround. Rationale: the compiler's `check_query` hits `Item::ImportDecl(_) => {}` silently (verified in `crates/ynz-typeck/src/check.rs`); the LSP's `resolve_imports`/`ExportTable` is invoked only by LSP queries, not by `check_query`. Without M8, may-block analysis can't propagate across module boundaries — the analysis under-approximates and silently misses `wait` insertions at cross-file I/O call sites. M8 must complete before M3-P2 (analysis implementation phase). Recommend approving M8 immediately after v0.2.0 ships.

- **Auto-SoA debugger integration — DECIDED 2026-05-21**: DAP integration ("show unified Player view when inspecting SoA-laid-out array in lldb/gdb") ships in M4 as a REQUIRED phase, not best-effort. If DAP integration scope proves unexpectedly large (>3 phases), escalate at M4 planning time — do NOT pre-emptively defer.

- **Sleep intrinsic naming + blocking-vs-yielding teaching — DECIDED 2026-06-01, Patrick**: the two sleep intrinsics shipped in M1/M2 are mis-named with programmer jargon. Locked corrections:
  1. **RENAME (cheapest now — no external users; do at M3 kickoff or as a standalone PR):** `sleepAsync` → `sleep` (the yielding/cooperative sleep, the default, used as `wait sleep(ms)`); `sleepMs` → `sleepBlocking` (the thread-holding sleep, the labeled exception). WHY: "Async" is exactly the function-coloring jargon Yinz hides (Golden Rule 12 + `vocabulary.md` — Yinz uses `wait`, never `async`/`await`); `wait sleepAsync` says "suspend" twice; and `sleepMs`/`sleepAsync` mix naming bases (unit vs mechanism). The yielding form is the idiomatic default (`sleep`), the blocking form names its danger (`sleepBlocking`) per `stdlib-design.md` Rule 1. Scope: `registry/features.toml`, `M2_MAY_BLOCK_INTRINSICS` in `emit.rs`, typeck dispatch, the `v0_3_m*` fixtures + insta snapshots, hover/CHANGELOG text.
  2. **NON-KERNEL TEACHING — `prefer-yielding-sleep` Tier 3 lint — SHIPS IN M4 (REQUIRED).** When `sleepBlocking(ms)` is used in a non-kernel program, a Tier 3 lint (yellow squiggle, suggestion, dismissable — NOT an error) nudges toward `wait sleep(ms)`. WHAT/WHAT-INSTEAD/WHY: "wait sleep frees the thread to run other tasks; sleepBlocking holds it idle. Use sleepBlocking only when you need to hold the thread (e.g. --kernel mode)." **Belongs in M4, not earlier, for TWO reasons:** (a) M4 builds the `[[lint_rule]]` registry mechanism anyway (`array-using-soa-layout`, `cross-thread-fields-not-padded`) — `prefer-yielding-sleep` rides the same machinery; building it as a one-off check before that infra exists is duct tape per `no-duct-tape.md`. (b) M4's `background` handle-form removes the ONE legit non-kernel blocking-sleep pattern — the `sleepMs(N)` main-thread keepalive (used today in `v0_3_m2_concurrent_waits_proof.ynz`, whose comment already says "M4 handle-form will eliminate the need for this"). So "almost 0 non-kernel uses" only becomes true AFTER M4; linting earlier would nag a still-legitimate pattern. — Must NOT be an error (rare legit uses + respect explicit intent per `auto-promotion.md`); must be a suggestion.
  3. **KERNEL TEACHING — `KernelModeRejectsWait` WHAT-INSTEAD amendment — ships whenever `--kernel` mode is wired to EMIT (post-v0.3 kernel milestone; the diagnostic is registry-reserved but wired in 0 code sites today).** When `wait sleep(ms)` (or any suspension point) is used in `--kernel` mode → compile error per `no-runtime-mode.md`. The existing `KernelModeRejectsWait` template tells you what you CAN'T do but not what to do instead — amend its `what_instead` to redirect: "Use `sleepBlocking(ms)` — it pauses without needing the scheduler." Golden Rule 11 WHAT-INSTEAD must be actionable.
  - Symmetry locked: **kernel mode pushes you toward `sleepBlocking` (error on `wait`); normal mode pushes you toward `wait sleep` (lint on `sleepBlocking`).** Design note: `design/future/concurrency.md` "Sleep Intrinsics" + `design/future/no-runtime-mode.md`.

---

## Open Architectural Questions

These BLOCK the milestones listed next to them. They must be resolved (and the resolution locked into the execution plan) before coding the blocking milestone starts.

- **State machine ABI with Tokio's `Future`**: The exact layout of the generated LLVM IR state machine and how it interfaces with `tokio::task::spawn` / `Poll<Output>` must be locked before M2 coding begins. A research spike in M2 P0 prototypes the simplest possible `wait`-able function (one `wait` point, no branching) and validates end-to-end execution. Blocks: v0-3-m2-wait-and-state-machines.

- **`background` ownership validation at spawn boundary**: `background fn(args)` requires that `args` are either `.give` (ownership transfer) or `.copy` (trivially copyable). The typeck check exists in v0.1 (`check.rs:1214`). Does the v0.3 auto-give inference extend the check, or does the explicit check stay as-is? Blocks: v0-3-m1-runtime-and-background M1 P1.

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| State machine LLVM IR is more complex than anticipated; M2 spans multiple sessions | High | Medium | M2 P0 is a dedicated spike with a clear accept/reject gate. If the simplest case takes >3 days, we revisit the approach before committing to full implementation. |
| Tokio ABI bridge has unsound behavior on task cancellation or stack overflow | Medium | High | M1 P0 prototype covers: spawn + join (verify main continues before fn completes), spawn + drop (cancel — panic discarded per best-effort design), spawn + panic (runtime continues; panic discarded). Each path must be verified before M1 ships. |
| May-block analysis over-approximates; every function is marked may-block | Medium | Medium | Ground truth: stdlib I/O intrinsics are the source of may-block. The upward propagation through user code is a fixed-point computation. Conservatively correct is acceptable; false positives add `wait` overhead but don't break correctness. Measurement: count `may_block = true` function ratio in `pirates-roster`; flag if >30% of user functions are over-marked. |
| Auto-parallelization silently reorders side-effect-ordered operations | Medium | High | Auto-parallelize only fires for operations with NO shared mutable state (ownership typeck is the safety gate). Property test technique: `--no-auto-parallel` produces identical stdout/stderr/exit-code to default-parallel mode for EVERY fixture in `examples/` and `crates/ynz-codegen/tests/`. Any divergence = bug. Ordering semantics are per the "auto-`wait` scope" decision: user must write explicit `wait` for ordered side effects; compiler only auto-parallels provably-independent operations. |
| `m8-typeck-cross-file-resolution` is a hard prerequisite for may-block analysis | **Decided** | High | **M8 required — ships before M3 P2.** Decision locked 2026-05-21 (see Required Pre-Work in Architectural Decisions). Schedule M8 immediately after v0.2.0 ships. |
| Auto-SoA silently breaks shapes with `lend self` methods | Low | Critical | Per Architectural Decisions: SoA suppressed for any shape with `lend self` methods that access more than the hot-loop fields. Detection: M4 execution plan must enumerate all shapes in `examples/` + test fixtures and verify SoA candidates are correctly filtered. `--no-auto-parallel` mode also disables SoA for the cross-implementation consistency test. |
| `background` + auto-Arc wraps a `lend`-borrowed value across thread boundary | Low | High | Ownership typeck already prevents `share` across `background` boundary (`check.rs:1216`). `lend` (exclusive mutable access) also cannot cross a thread boundary — the compiler must reject `background fn(lend val)` with "cannot lend across thread boundary; use `.give` or `.copy`." M1 typeck must cover this. |
| `wait` inside `for` loop auto-paralelizes incorrectly | Low | High | Loop iterations remain sequential (per `design/concurrency.md`). `wait` inside a `for` body is N sequential waits. The dependency graph analysis MUST NOT auto-parallelize across loop iteration boundaries. M3 integration test: `for (item in items) { wait process(item) }` — assert single-threaded sequential execution. |
| Auto-SoA layout breaks serialization | Low | High | No FFI in v0.3 (FFI is v2+). Serialization codegen is compile-time-generated per `design/stdlib-design.md` Rule 6 and reads field layout from the compiler's representation — SoA-aware serialization is a M4 prerequisite check. Assert: serialize + deserialize a SoA-laid-out `array<Player>` → roundtrip produces identical field values. |
| Compile-time cost of analysis passes exceeds acceptable limits | Low | Medium | Analysis passes (may-block, dependency graph, SoA candidate) are profiled in each milestone's verification step. Target: <10% increase in `ynz build --release` wall-clock on `examples/pirates-roster/`. |
| Channel send to a closed channel / dropped receiver | Medium | Medium | M4 must specify behavior: `send()` to a dropped receiver returns a typed `errors` result — the user handles or propagates. NOT a silent drop. Design detail locked in M4 P0. |

---

## Milestones

### Milestone 1: v0.3-M1 — Runtime Bootstrap + Working `background` — multi-session
**Value delivered**: `background process(data)` runs on a separate thread — main continues immediately. Previously it ran synchronously and main waited for it to complete (`emit.rs:3089` stub). After M1: `background saveAnalytics(event)` after `return response` returns the response before analytics runs. `wait` calls inside the background function still block that blocking-pool thread (state machines are M2), but they don't block main. The program is correct; background I/O work may take longer than optimal until M2.
**Execution plan**: `v0-3-m1-runtime-and-background` (status: planned)
**Depends on**: nothing — first up. Pre-condition: v0.2.0 released; parser infinite-loop bug fixed (Required Pre-Work above).
**Rough scope**:
- Add `tokio` + `num_cpus` to `ynz-runtime` Cargo.toml (bundled in `libynz_rt.a`)
- `ynz_rt_init()` boots Tokio multi-thread runtime + blocking thread pool; called from generated `main`
- `ynz_rt_spawn_blocking(fn_ptr, ctx_ptr, ctx_size)` C-ABI bridge via `tokio::task::spawn_blocking` (M1 routes ALL background tasks to blocking pool; I/O routing upgrades in M3)
- `ynz_rt_check_preempt()` C-ABI helper emitted at loop back-edges + function call sites
- `ynz_rt_shutdown()` for clean teardown after `main` returns
- `ynz build --no-auto-parallel` flag: forces sequential execution mode (required for the cross-implementation consistency test harness from M1 onward)
- `ynz-codegen` emits `ynz_rt_spawn_blocking(...)` for `Expr::Background`; `ynz_rt_check_preempt()` at loop back-edges
- Large-copy warning: `background fn(largeStruct)` where `largeStruct` is used after spawn → compile warning "copying N bytes into background task — consider passing ownership" (per `design/concurrency.md` IDE warning on large copies)
- Integration tests: main continues before background fn completes (timing-verified); `.give` and `.copy` accepted; `.share` rejected (`check.rs:1216`); `--no-auto-parallel` produces identical output to sequential baseline
- Demo: `examples/pirates-roster/entrypoint.ynz` extended with `background` section; `examples/primantis-orders/v0_3_m1_errors.ynz`
- **Teaching surface (REQUIRED in M1, not deferred)**:
  - Registry: update `wait` and `background` keyword hover docs in `registry/features.toml` — current text says they run sequentially, which is now wrong
  - New compile errors with WHAT/WHAT-INSTEAD/WHY: `background fn(lend val)` → "cannot lend across thread boundary; use `.give` or `.copy`"
  - Large-copy compiler warning: WHAT/WHAT-INSTEAD/WHY text locked before M1 ships
  - VSCode extension: bump version, add `background-concurrent.png` screenshot
- **Registry entries**: one `[[deferred_tooling_feature]]` entry for `background-handle-form`
**Ships via**: `/pr` per phase, `/release` for `v0.3.0-m1` tag

### Milestone 2: v0.3-M2 — `wait` Suspension + State-Machine Codegen + Intra-Unit May-Block Inference — multi-session
> **⚠️ BOUNDARY CORRECTED 2026-05-31 (see Architectural Decisions "M2/M3 boundary correction").** This milestone originally deferred ALL may-block analysis to M3 and used a runtime `block_on` bridge as the M2 stopgap. That bridge both crashed at runtime (the M2 Phase-5 HALT) AND contradicted `design/future/concurrency.md`'s documented no-coloring model. The intra-compilation-unit may-block analysis is LOAD-BEARING for a correct state-machine layer and therefore moves M3→M2; M3 keeps cross-module propagation + auto-parallelization.
**Value delivered**: Functions that (transitively) reach a suspension point actually suspend and resume instead of blocking — and you do NOT have to write `wait` for that to happen. Per `design/future/concurrency.md` ("No Function Coloring"), the compiler infers which functions are state machines from the call graph and shows the inferred `wait` as a muted hint; every caller of a suspender is itself a state machine and uses inline poll-and-yield — no function coloring, no thread-holding bridge. This makes existing `wait` code both correct AND fast (Rust-async-level efficiency).
**Execution plan**: `v0-3-m2-wait-and-state-machines` (status: planned)
**Depends on**: v0.3-M1 (Tokio runtime must be running before state machines can suspend into it)
**Rough scope**:
- M2 P0: research spike validated against the REAL compiler — compile real `.ynz` through `./target/debug/ynz`, NOT hand-written Rust/IR. (The original spike validated a hand-written model that diverged from the emitted codegen → false ACCEPT → the Phase-5 HALT. Never re-introduce that.)
- New `lower_function_with_waits()` codegen path in `emit.rs`; transforms any function the may-block analysis marks as suspending into an LLVM state machine struct + `resume(frame, waker)` function
- **Intra-compilation-unit transitive may-block analysis (NEW — moved from M3):** a call-graph fixpoint with the may-block set = `{sleepAsync}` (M2's only suspension source). A function is a state machine iff it transitively reaches a may-block call. This eliminates the runtime bridge entirely — because every caller of a suspender is itself a state machine, every suspending call is inline poll-and-yield; the only `block_on` is the legitimate `main→entrypoint` top-level driver.
- **Frame composition (zero-cost-abstraction allocation model):** a child state machine's live state composes INTO the parent's frame (computed at compile time), like Rust nests `async fn` futures. Heap-allocate only at `background` spawn points and at recursion points (break the infinite-size cycle, à la Rust's `Box::pin`). NOT a heap allocation per state-machine call — that would miss the design's "low memory, fast spawn" target.
- **Typed return slot:** value-returning state machines (`-> int` / `-> string` / `-> T errors` with `wait`) thread the return value through a properly-typed frame slot (holds i64 / pointer / the `{i64,i64}` errors ABI).
- **Can't-infer cases → clean compile error (per the design's "externals are on the user"), NOT a bridge:** cross-module calls before M8 lands; dynamic dispatch through a `dynamic Contract` vtable; FFI (none in M2 yet). The compiler emits a teaching error directing the user to make the boundary explicit; M3 lifts the cross-module case via M8 + cross-module propagation.
- `Expr::Wait(inner, _)` desugars to inline poll-and-yield: poll `inner`'s resume fn forwarding the same `waker_ctx` → ready: read its typed return slot + continue; pending: return `Poll::Pending` up to the driver.
- Integration tests (ALL validated through `./target/debug/ynz run` on real `.ynz`): one `wait`, sequential `waits`, `wait` inside `if`, NESTED state machines (SM-from-SM), `background` from inside a state machine, value-returning + errors-cascade through suspension, cancellation-no-leak.
- **Boundary note (what stays M3+):** CROSS-MODULE may-block propagation (needs M8 cross-file typeck), auto-parallelization of independent statement groups, and I/O auto-`wait` insertion (when stdlib I/O intrinsics ship and join the may-block set). M2 builds the analysis ENGINE over the `{sleepAsync}` set; later milestones feed it more may-block sources without changing the engine.
- **Teaching surface (REQUIRED in M2, not deferred)**:
  - New compile warning WHAT/WHAT-INSTEAD/WHY: "`wait` on a function that can never block — the `wait` has no effect" + suggestion to remove it
  - New compile errors for `wait` on bad expression types (non-call) if any surface
  - `wait_points` domain: protocol handler exists from v0.2-M5; M2 implementation spike must validate the handler fires correctly for explicit `wait` before M3 adds auto-`wait`
  - Demo: `pirates-roster` mock I/O section showing `wait` suspending; `v0_3_m2_errors.ynz`
  - VSCode extension: bump version, add `wait-suspension.png` screenshot showing the coroutine suspending behavior
**Ships via**: `/pr` per phase, `/release` for `v0.3.0-m2` tag

### Milestone 3: v0.3-M3 — Cross-Module May-Block Propagation + Auto-Parallelization — multi-session
> **⚠️ SCOPE NARROWED 2026-05-31 (boundary correction):** the intra-compilation-unit may-block analysis ENGINE moved to M2 (it's load-bearing for a correct state-machine layer). M3 EXTENDS that engine across module boundaries (needs M8) and adds the parallelization + I/O-auto-`wait` features. M3 does NOT build the engine from scratch — it feeds the M2 engine more may-block sources (cross-module callees, I/O intrinsics) and adds the dependency-graph parallelization pass.
**Value delivered**: Independent operations in a function body auto-parallelize without the user writing `background` or `wait`. The compiler's dependency graph analysis identifies statement groups with no shared mutable state and schedules them concurrently. May-block detection (built intra-unit in M2) now propagates ACROSS module boundaries, and the compiler inserts `wait` at I/O call sites automatically once stdlib I/O intrinsics exist and join the may-block set. The IDE's muted `wait` hint surface (per `design/ide-hints.md` `wait_points` domain) fires on real cross-module + I/O data.
**Execution plan**: `v0-3-m3-auto-parallelization` (status: planned)
**Depends on**: v0.3-M2 (state machines must work before auto-parallelization can schedule them); M8 cross-file typeck (REQUIRED — must be complete before M3 P2; see Required Pre-Work in Architectural Decisions)
**Rough scope**:
- Cross-module may-block propagation: EXTEND M2's intra-unit `may_block` fixpoint (salsa-tracked) to propagate across file/module boundaries via M8's compile-time cross-file typeck (M8 ships before M3 P2 — decided 2026-05-21). Also ADD the I/O stdlib intrinsics to the may-block set as they ship (M2's set was `{sleepAsync}` only). The fixpoint algorithm itself is M2's; M3 widens its inputs (cross-module edges + I/O intrinsics) and lifts M2's "cross-module call = clean error" stopgap.
- Auto-`wait` and auto-parallelize are the SAME pass (not two separate passes): the dependency graph analysis identifies independent statement groups; groups with may-block callees get auto-`wait` + run-concurrently; groups with no may-block callees run on CPU pool. Both happen in the same IR rewrite pass.
- IDE `wait_points` muted-hint domain: the `wait_points` registry domain (currently protocol-only) starts firing. Muted `wait` appears before I/O calls; hover tooltip: WHAT/WHAT-INSTEAD/WHY per Golden Rule 11.
- CPU-bound task routing: calls with no may-block transitive callees route to `tokio::task::spawn_blocking` (from M1) vs I/O scheduler (new in M3).
- `ynz_rt_spawn(fn_ptr, ctx)` (NON-blocking, I/O pool) added in M3 alongside existing `ynz_rt_spawn_blocking` — M3 is when the routing distinction matters.
- `background` ownership auto-give/copy inference: value unused after background call → auto-`.give`; value used after → auto-`.copy`. Required call-graph analysis ships here alongside may-block analysis.
- IDE routing hint: background tasks get muted `// routed to I/O pool` or `// routed to CPU pool` per auto-inference.
- `--no-auto-parallel` flag (added M1): verify that `--no-auto-parallel` and default-parallel produce identical stdout/stderr/exit-code on every `examples/pirates-roster/` fixture AND every `crates/ynz-codegen/tests/` fixture. This is the cross-implementation consistency gate.
- **Teaching surface (REQUIRED in M3, not deferred)**:
  - New `[[muted_hint_domain]]` registry entry: `background_routing` (Informational category: `// routed to I/O pool — calls db.fetch (may suspend)` / `// routed to CPU pool — no may-block calls in call graph`). Handler wired in LSP inlayHint.
  - `wait_points` domain fires for the first time with real auto-inserted `wait` data — validate hover tooltip text is correct WHAT/WHAT-INSTEAD/WHY (the domain was protocol-only until now)
  - New compile errors with WHAT/WHAT-INSTEAD/WHY: `wait` inside `for` body that the compiler cannot auto-parallelize → clear diagnostic explaining loop-iterations-sequential rule
  - `background` ownership auto-give/copy inference: when inference changes behavior, the IDE muted hint shows what was inferred (`.give` or `.copy`) at the call site using the existing `ownership_call_site` domain
  - Demo: `pirates-roster` auto-parallelization section with muted routing hints visible; `v0_3_m3_errors.ynz`
  - VSCode extension: bump version, add `auto-parallel.png` + `routing-hints.png` screenshots
**Ships via**: `/pr` per phase, `/release` for `v0.3.0-m3` tag

### Milestone 4: v0.3-M4 — Channels + Auto-Arc + Auto-SoA + v0.3.0 Release — multi-session
**Value delivered**: `channel<T>()` for bounded task communication (the `background` handle-form). Cross-thread shared state gets auto-`Arc` wrapping. `array<Shape>` hot loops that access ≤2 fields get SoA layout automatically (10-40× cache improvement on large arrays). Cuts the `v0.3.0` release tag — first version where Yinz code actually runs concurrently.
**Execution plan**: `v0-3-m4-channels-soa-release` (status: planned)
**Depends on**: v0.3-M3 (auto-parallelization must be working before channels add meaningful communication patterns)
**Rough scope**:
- `channel<T>()` type: bounded (capacity default 64, configurable); `ynz_channel_send` / `ynz_channel_recv` C-ABI bridge via Tokio's `mpsc` channel internals
- Channel muted-hint domain: `channel_capacity` registry domain fires inline capacity hint `⟨64⟩` per `design/future/concurrency.md` "Channel/Queue Primitives"
- Background handle-form: `let h = background fn()` → `h` is a task handle with `.send()` / `.receive()` (wraps a Tokio `JoinHandle` + optional channel)
- Auto-Arc: when a value is shared across a `background` spawn boundary and ownership rules require reference counting (shared immutable state accessed from multiple tasks), codegen emits `Arc::new(value)` + `Arc::clone()`. IDE muted hint shows the auto-Arc (cautionary red-tinted styling per `design/future/concurrency.md`).
- False sharing auto-padding: shapes with fields accessed from different `background` tasks get 64-byte cache-line alignment + inter-field padding. Codegen-only; Tier 3 lint `cross-thread-fields-not-padded` fires if padding can't be applied automatically.
- Auto-SoA: new analysis pass `soa_candidate_query(db, source) -> Vec<SoaCandidate>`. Each candidate is an `array<Shape>` binding whose primary loop accesses ≤2 fields. Codegen emits SoA layout instead of AoS. Tier 3 lint `array-using-soa-layout` fires on the binding with hover explaining the transform (per `design/future/auto-soa.md` IDE Teaching Surface).
- **`prefer-yielding-sleep` Tier 3 lint** (per "Sleep intrinsic naming + teaching" architectural decision): fires on `sleepBlocking(ms)` in non-kernel programs, suggesting `wait sleep(ms)` (suggestion, dismissable — NOT an error). Rides the same `[[lint_rule]]` machinery built for the SoA/padding lints. This is the M4 home for the non-kernel half of the blocking-vs-yielding teaching; M4 is also where the handle-form removes the last legit non-kernel blocking-sleep use (the `sleepMs` keepalive), so the lint stops nagging a valid pattern. NOTE: depends on the `sleepAsync`→`sleep` / `sleepMs`→`sleepBlocking` rename being done first (M3 kickoff or standalone) — if the rename hasn't landed by M4, do it as M4 P0.
- Debugger integration: best-effort for v0.3 (if DAP integration is >3 phases of work, defer to `[[deferred_tooling_feature]]` entry and ship v0.3 without it).
- Cargo.toml bump 0.2.x → 0.3.0, CHANGELOG, `/release` for `v0.3.0` tag (no `-mN` suffix)
- Demo: full `pirates-roster` showcase with channels, auto-SoA hint annotations; `v0_3_m4_errors.ynz`
- **Teaching surface (REQUIRED in M4, not deferred)**:
  - New `[[muted_hint_domain]]` registry entries: `channel_capacity` (Addition category: `⟨64⟩` inline hint) AND `auto_arc` (Informational category: cautionary red-tinted `// Arc<T> — shared across tasks, ref-counted` hint)
  - New `[[lint_rule]]` registry entries: `array-using-soa-layout` + `cross-thread-fields-not-padded` + **`prefer-yielding-sleep`** (non-kernel `sleepBlocking` → `wait sleep` nudge) — each with WHAT/WHAT-INSTEAD/WHY hover text
  - Channel error messages: `send()` to closed channel, channel-full backpressure trigger — WHAT/WHAT-INSTEAD/WHY for each
  - SoA debugger DAP integration: `players[0]` in lldb shows unified `Player { x, y, health, name }` view even under SoA layout
  - VSCode extension: full v0.3 polish pass — version bump, updated `wait`/`background` hover docs (updated text from M1 carries forward), screenshots for channels, auto-Arc hint, SoA lint
  - `pirates-roster` must demonstrate ALL four M4 surfaces: channel communication, auto-Arc hint, SoA lint squiggle, false-sharing padding
**Ships via**: `/pr` per phase, `/release` for `v0.3.0` tag

---

## Out of Scope

These were considered for v0.3 and explicitly deferred:

- **Custom user `Iterable<T>` implementations** — deferred to v1.0 per `design/mvp-scope.md`. No change from plan.
- **Background handle-form before M4** — `let h = background fn()` is M4 (channels); M1-M3 only has fire-and-forget.
- **Cross-loop parallelization** — loop iterations remain sequential. Per `design/concurrency.md` "Loop Iterations — Sequential by Default."
- **Deadlock detection at runtime** — complex; deferred to later. `design/future/concurrency.md` flags this as an Open Question for the v0.2 implementation milestone; it remains open for v0.3.
- **Debugger SoA view** — best-effort in M4; deferred to a future tooling release if DAP integration scope exceeds 3 phases.
- **Sized integer variants, FFI, GPU dispatch** — v2+ per `design/mvp-scope.md`. No change.
- **`ynz lint` / v0.4 linting tier** — v0.4 is next after v0.3. The Tier 3 lint rules introduced in v0.3 (SoA, false-sharing, mutable-when-const) are part of the normal compile output per `design/linting.md` (compiler IS the linter).
- **v0.3.x patch releases** — possible for critical bugs; not pre-planned. Same policy as v0.2.x.
- **Batch utilities for parallel loop processing** — `design/concurrency.md` notes "developers will use batch utilities from the standard library" for parallel loop processing. No such batch utilities ship in v0.3 — that's a stdlib module (likely bundled with a future iteration of v0.5+ file or a concurrency stdlib). Users who need parallel loop processing in v0.3 write explicit `background` calls with a manual join barrier. Documented in Out of Scope so the gap is visible.

---

## Open Questions for Patrick

*(All three original open questions resolved 2026-05-21 — see Required Pre-Work and Auto-SoA debugger decision in Architectural Decisions above.)*

- No remaining open questions at roadmap level. Execution plan open questions will surface in each milestone's `/plan` session.
