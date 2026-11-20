---
name: "IMP-no-function-coloring"
description: "Locks Yinz's no-function-coloring concurrency model: no async/sync split, compiler does whole-program may-block analysis and auto-inserts wait, taught via IDE muted hints."
tags:
  - "yinz-compiler"
created_at: "2026-05-14"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Concurrency — No Function Coloring

**Status**: Locked. Each section below says what is built.

User spec: [`docs/reference/REF-concurrency.md`](../../reference/REF-concurrency.md).

---

## The Decision

Yinz has **no function coloring**. There is no type-level distinction between async and sync functions: every function is just `function`. The compiler does whole-program may-block analysis from the call graph, auto-inserts suspension points, and the IDE shows each inserted `wait` as a muted hint per [`docs/reference/REF-ide-hints.md`](../../reference/REF-ide-hints.md).

**Why it works**: Yinz ships a runtime (`libynz_rt.a`) with a scheduler to suspend on, which makes whole-program may-block analysis tractable (kernel-mode builds are covered by [`IMP-no-runtime-mode.md`](IMP-no-runtime-mode.md)). And the IDE is required infrastructure, so muted hints carry what explicit `await` syntax would.

---

## How it works

### Compile time

1. The compiler builds a call graph for every function in the program (Yinz code + Yinz packages — see [`docs/internal/scratchpad/SCRATCH-future-packages.md`](../scratchpad/SCRATCH-future-packages.md) for the binary metadata that makes cross-package analysis work).
2. For each function, the compiler determines whether it transitively calls any I/O intrinsic, FFI function marked `may-block`, or `wait`-expression. The "may-block" property propagates up the call graph.
3. At every call site to a may-block function, the compiler emits a **suspension point** in the codegen. This is the no-coloring mechanism — automatic, never typed by the user.
4. The IDE protocol shows the inferred suspension as the muted `wait_points` hint before the call expression.

> **Suspension ≠ the `wait` keyword.** The auto-emitted suspension point above is about *suspension correctness* (handing the thread back to the scheduler) — the user never writes `wait` for it. The user-written `wait` keyword is a separate, additional thing: an **ordering barrier** the user adds to force a happens-before between otherwise-independent operations the compiler can't infer. Both are spelled `wait`, but suspension is automatic and ordering is the keyword's only manual job. The authoritative model is [`docs/internal/implementation/IMP-concurrency.md`](IMP-concurrency.md) → "Suspension vs. Ordering — What's Automatic and What `wait` Does (LOCKED)".

### Runtime

1. `wait` desugars to a state-machine suspension (stackless coroutines: low memory, fast spawn, cheap context switch).
2. The runtime scheduler (in `libynz_rt.a`) drives suspended state machines forward as I/O completes.
3. `background` spawns a new task onto the scheduler. A task costs state-machine memory and no per-task OS stack.
4. Cross-thread shared state crosses a `background` boundary via auto-inferred reference-counted sharing, live for a GROUP of spawns: two or more spawns in one block passing the same whole shape binding, nothing changing it between them, every spawned function proven read-only (`effective_ownership`), and a shape of `Int`/`Float`/`Boolean`/`String` fields share ONE block (`ynz_arc_new(size, glue)` at the first spawn, with a copy of the shape's strings that the block owns; `ynz_arc_clone` per task; the caller's transient released after the last spawn; each task's reference released at retire; the last release runs the glue that frees the strings, then frees the 16-byte-header block: `emit.rs:24601-24643`, `crates/ynz-runtime/src/arc.rs:83-91,147-167`). A single spawn, a spawn group straddling a suspension point, a spawn inside a loop, and pointer-carrying or `Number`-carrying values still cross via the independent-copy path (the compiler infers `.copy`) — correct and race-free; that residual is what the registry entry `auto-arc-codegen-emission` now names. The IDE shows the `auto_arc` muted hint at every member spawn (normal muted style; the cautionary red tint stays under `auto-arc-cautionary-tint` — the caution is in the hover's words). The sharing topology, the beneficial-emission condition, the read-only proof, and the override analysis are specified in [`docs/internal/implementation/IMP-ownership.md`](IMP-ownership.md) "Auto-Arc — Sharing Topology Across `background` Boundaries"; the transfer rule for values that LEAVE a frame is the same document's "Transfer — Who Else Holds This Value".

   Topology: one shared copy, N task references, the caller keeps its original (`IMP-ownership.md` "The topology, decided"). The registry entry `auto-arc-codegen-emission` (`[[deferred_language_feature]]`) names only the residual above, and `auto-arc-cautionary-tint` (`[[deferred_tooling_feature]]`) defers the hint's red tint. Anchor: `git log --grep=m8-p5`.

---

## FFI annotation requirement

**Design for when `foreign` ships (v2+ — see [`IMP-ffi.md`](IMP-ffi.md) and its `[[deferred_language_feature]]` registry entry). No `foreign` keyword exists in the compiler today**, so nothing below runs yet — it is the locked plan for the may-block analysis to extend cleanly onto FFI boundaries once they exist.

The compiler can analyze pure Yinz code. It CANNOT analyze C code linked via FFI — we can't know whether `printf` blocks without knowing what's behind it. So FFI boundaries must declare `may-block` explicitly:

```ynz
foreign function printf(format: String) -> Int may-block
foreign function read(fd: Int, buf: pointer, n: Int) -> Int may-block
foreign function memcpy(dst: pointer, src: pointer, n: Int) -> pointer    // not annotated → doesn't block
```

This is one line per C function declared. Far less burden than Rust's `async fn` propagation. The compiler treats `may-block` foreign functions as if they were Yinz functions calling I/O intrinsics — call graph propagation works the same way.

---

## Compiled-Yinz package metadata

The compiled-package format is not built. When it ships, it embeds `may-block` metadata per exported function, and the format reserves space for it. A downstream project reads that metadata and adds the package's functions to its call graph, so `wait` insertion works across package boundaries. Format: [`docs/internal/scratchpad/SCRATCH-future-packages.md`](../scratchpad/SCRATCH-future-packages.md).

---

## Cross-Module Frame Layouts — `frame_layouts_query` (shipped)

A function that suspends (it reaches a `wait`) keeps its locals in a heap frame. When file A calls a suspending function from file B, A must reserve room for B's frame inside its own and write B's arguments into it, so A needs B's real frame size and local count. `frame_layouts_query(source)` in `crates/ynz-codegen/src/queries.rs` is the one place that computes them: a Salsa query, memoized per source file, returning `Arc<HashMap<String, FrameLayout>>` (function name to layout).

How it works:

- **Codegen computes it, not the type checker.** The size of a shape-typed local is the target's padded layout, which only LLVM's data layout knows; `ynz-typeck` stays free of LLVM. Each source file is its own LLVM module and object file, so the importer cannot read the callee's module; it asks the query for the callee's file instead.
- **One target-machine constructor** serves both `emit_artifact` and the query, so importer and exporter use byte-identical ABI sizes.
- **Recursive for re-export chains:** for an imported suspending callee, the size resolver calls `frame_layouts_query(callee_file)`; A to B to C recurses and ends because the import graph has no cycle that survives (Salsa cycle recovery returns an empty map for a circular import).
- **What it fixes:** exact slot offsets for every crossing local, the full sub-frame tree through re-exports, the `errors` staging slot next to a child sub-frame, the two-slot `Number` crossing local, and transitive composition with the caller's frame.
- **Still refused:** edges the compiler cannot analyze (a call through a `dynamic Contract` vtable, a foreign call into a suspending function) keep failing through the existing may-block unresolvable-edge path.

When the compiled-package format ships, it must serialize these layouts into the package artifact; nothing in the in-process query blocks that.

---

## What this is NOT

- **Not stackful coroutines**: tasks are stackless state machines with no per-task stack.
- **Not a hidden `wait`**: the IDE muted hint makes every suspension visible; the user can click to make it explicit and hover to learn why.
- **Not "everything is async"**: pure-CPU functions (no I/O, no FFI may-block, no `wait` inside) get no `wait` at their call sites. Only call chains that reach a suspension point get suspension code.

---

## Channel/Queue Primitives — Bounded by Default, Capacity Auto-Inferred

**Built**: `Channel<T>()` / `Channel<T>(N)` construction, bounded-by-construction typeck and codegen, the kernel-mode gate, the muted default-capacity hint, and send/receive suspension (bare channel and `background` handle form). The default capacity is the constant `64` (`DEFAULT_CHANNEL_CAPACITY` in `ynz-typeck`, the single authoritative source per [`.claude/rules/authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md)).

`Channel<T>()` constructs a bounded channel with a compiler-chosen default capacity. `Channel<T>(N)` overrides with an explicit number. There is NO unbounded constructor — to express unbounded, write `Channel<T>(Int.max)` and a comment explaining why.

This is the auto-promotion pattern (per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md)):
- **Codegen**: compiler emits the chosen capacity (default or explicit)
- **Muted IDE hint**: always shows the capacity AND whether it was default or user-set, so the bounded-ness is visible at every channel construction
- **Explicit override**: `Channel<T>(N)` when the default is wrong for this workload

```ynz
// Source the user wrote:
let queue: Channel<Order> = Channel<Order>()

// IDE rendering (muted text appears INSIDE the empty parens — addition placement
// per .claude/rules/inference.md "Three Placement Categories"):
let queue: Channel<Order> = Channel<Order>(64)     // `64` rendered muted (plain
                                                    // text, like every Addition hint)
                                                    // hover tooltip:
                                                    //   WHAT: capacity = 64 (default)
                                                    //   WHAT INSTEAD: write Channel<Order>(N) for a different bound
                                                    //   WHY: 64 is sized for typical workloads — backpressure surfaces
                                                    //        within seconds of sustained overproduction
                                                    // click-to-make-explicit: writes `64` into source

// Override case — source already has the number, no muted hint needed:
let bigQueue: Channel<Frame> = Channel<Frame>(1000)
```

The muted `64` appears where the user would have typed it, not as a comment after the line; a click types it into the source. This is the addition-placement pattern from [`.claude/rules/inference.md`](../../../.claude/rules/inference.md).

### Default capacity — a constant

The default is one constant (64, `DEFAULT_CHANNEL_CAPACITY`). **Why 64**: small enough that sustained overproduction surfaces as backpressure within seconds instead of unbounded memory growth, large enough to absorb normal consumer jitter. The compiler cannot infer a better value statically, because the optimum depends on runtime producer and consumer rates.

### Bounded, with a visible default

Every channel is bounded, and the muted hint keeps the bound visible when the source omits the number. **Why bounded**: an unbounded queue hides backpressure until memory runs out, and the failure looks like a random crash.

### Behavior on full channel

Sending to a full channel **suspends the sender**: its `background` task pauses until capacity opens. If the sender has a caller waiting on its result, the caller suspends too (the backpressure cascade). Documentation says plainly that a suspended producer is backpressure working, not a deadlock.

### What `capacity` does NOT constrain

`capacity` is the queue's maximum size at any moment, not a limit on producers or consumers. Many producers, many consumers, or any mix share one queue; suspension fires only when the queue actually fills, not in balanced steady state.

### Backpressure is for writes, not reads

The rule applies to writes into a queue (`.send()`). Reading a `share`-borrowed value queues nothing, so no backpressure question arises; ownership coordinates it at compile time.

### Cross-references

- [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) Rule 4 (bounded by default); the channel is its canonical instance.
- [`IMP-concurrency.md`](IMP-concurrency.md) "Channel Close — End-of-Stream Semantics": the other half of the channel contract: `.close()`, `receive()` as `Maybe<T>` on a bare channel (`none` = end of stream; the handle's `receive()` stays `T errors` because it carries the task's own failure), `send()` giving its owned-heap payload, and the deferred auto-close.

---

## Atomic Ordering Default — Acquire-Release, Not Sequential-Consistency

Yinz's channel and shared-state primitives use **acquire-release** ordering by default for all send/receive operations. Sequential-consistency is available as an explicit opt-in for the rare use case requiring a global total order across multiple concurrent operations.

### Why acquire-release default

For the patterns that appear in user code (handoff via channel, lock acquire/release, reference counting), acquire-release is sufficient and correct, and it avoids the extra barriers sequential consistency needs on ARM. Sequential consistency is needed only when a global total order across concurrent operations is part of a program's correctness contract.

### Surface for users

Per Golden Rule 5, Yinz exposes no `memory_order_*` names, and memory ordering is not a tunable parameter. The default applies automatically to channel and atomic operations. The global-order opt-in is a named API whose name is not decided (candidate: `let ordered = atomic.add(n)` then `ordered.withGlobalOrdering()`); it is not built.

### Tradeoff

Code that assumes sequential consistency can be silently wrong under the acquire-release default. The channel model makes most cross-thread coordination explicit through typed messages, and the exceptional case goes through an explicit API, not a default-flipping parameter.

---

## False Sharing Auto-Padding

When the compiler detects that a `shape` type crosses a thread boundary at a `background` spawn site, it emits the shape with 64-byte cache-line alignment and inserts padding between independently accessed fields. Detection keys on the legal crossing set (`give`/`copy` arguments, `Channel<T>` conduit element types, and callee return types at `background` spawn sites), because `.share`/`.lend` across `background` is a hard compile error (see [`IMP-concurrency.md`](IMP-concurrency.md) "Ownership with Background Tasks — Why `.share` Fails").

**Why**: two variables in one 64-byte cache line make a write to either invalidate the line for every core even when no data is shared.

**Surface**: codegen only, with no user syntax. No typeable explicit form exists (there is no `@cacheLineSeparate` annotation), so there is no muted hint (per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md), hints need a typeable explicit form). A Tier 3 lint `cross-thread-fields-not-padded` fires when a shape with cross-thread-accessed fields cannot be auto-padded. The unpaddable class is cross-module-visible-layout shapes (exported, imported and `__anon__*` structural shapes), because each module compiles to its own object file and padding one module's view would fork the type's layout. FFI shapes are out of scope (`foreign` is a deferred language feature).

**Built**: the padding transform and the decline lint; both gate off under `--no-auto-parallel` and read the same authoritative analysis predicate. Every crossing the compiler produces today is an exclusive-ownership handoff (`give`/`copy`/channel payload), which cannot false-share, so the transform's throughput benefit arrives with auto-Arc spawn groups ("Runtime" item 4 above), which create genuinely shared instances.

---

## Scheduler Preemption Model

Yinz uses **compile-time-assisted safe-point preemption**: the compiler turns loop back edges inside state-machine functions into poll-yield suspension points, and the runtime's budget check decides at each one whether to yield to another task. There is no async signal-handler infrastructure. The architecture has three parts:

### The three-part architecture

1. **State-machine-function loop back edges: codegen poll-yield.** Inside an *admitted* state-machine (may-suspend) function, every qualifying loop back edge is a genuine suspension point: codegen emits `br (ynz_rt_check_preempt(waker_ctx)), yield, header` — the yield stores the resume point through the existing suspension machinery (`store_resume_point` + the per-statement crossing-local flush discipline) and returns Pending; the continuation state reloads and re-enters at the loop header. The budget check wakes the task from OFF the worker thread before the Pending returns (a remote wake via the blocking pool), so a yield is a fair requeue (Tokio's LIFO fast path would otherwise let the yielding task starve its siblings).
   - **Qualifying loops**: `while`, and `for` over a literal `range(...)`, an `Array<T>`, or a `Map<K, V>`. **Named exclusions** (these loop forms tick the budget with no yield): `for` over `Fixed<T>` (stack-backed elements cannot survive a suspension — the same hazard the FixedArrayIterWithWait check rejects for explicit waits), and `for` over a string / shape-iterable / stored-range variable (the state-machine walker's non-frame-backed fallback forms).
   - **Per-function admission (safe default: a declined function runs without back-edge yields)**: a function is admitted only when treating its loop back edges as suspension points forces no un-frame-backable type (`Fixed<T>` / `Maybe` / `union` / `dynamic` / nested-shape shape) to cross a suspension, no non-qualifying `for` form wraps a qualifying loop, and no two `for` loops share a loop-variable name across different element types (the name-keyed frame-slot machinery would collide). Declined functions are part of residual (3) below. The admission verdict is computed once in typeck (`back_edge_yield_admitted`) and threaded to every consumer — routing, suspension-point counting, and crossing-local collection all read the ONE producer.
2. **Non-state-machine CPU-bound work: blocking-pool routing.** A plain synchronous function has no frame, cannot return Pending, and has nothing to resume — no yield mechanism exists or can exist for it under the no-coloring design. Its protection is CPU-admission routing (below): tasks whose call graph contains zero may-block calls run on the blocking pool, off the I/O workers.
3. **The named residual — CPU-heavy code no mechanism covers.** (a) CPU-heavy code inside a non-SM function that a may-suspend task calls: it runs on an I/O worker with no yield until it returns — admission routing is a heuristic over the *task's* call graph, not a per-function guarantee. (b) Loop-free CPU-bound recursion inside a state-machine function (no back edge to yield at) — the shape call-site checks would have covered. (c) Loops in *declined* SM functions and the excluded loop forms above. These are recorded residuals, not silent gaps.

**Call-site preemption checks are not shipped.** An opaque per-call-site check defeats inlining, and a fib(30) call-heavy microbenchmark ran ~80× over the ≤5% overhead bar. The deferral is registry entry [`preempt-callsite-checks`](../../../registry/features.toml) (`[[deferred_tooling_feature]]`); the emission stays behind the compile-time toggle `YNZ_PREEMPT_CALLSITE_CHECKS`, with the microbenchmark fixture committed for re-measurement.

### Why compile-time-assisted

The compiler already sees call graphs and loop back edges for ownership analysis, a safe-point check is a poll at a known point, and no signal-handler interaction with the optimizer, debug tables or OS is needed.

### Time quantum and CPU-bound task routing

The ~10ms quantum is a wall-clock target implemented as a deterministic poll count: a per-worker count of `2^20` back-edge polls between granted yields (`ynz_rt_check_preempt` in `crates/ynz-runtime/src/runtime.rs`), which lands in the ~2-15ms band for tight hot loops. **Why count, not clock**: a wall-clock budget makes whether a loop yields depend on run-to-run timing, so program output varies between runs, and byte-exact output goldens and cross-mode byte-identity depend on determinism. Loops with heavy per-iteration bodies yield less often than a strict 10ms clock would. The constant is the whole configuration; a per-runtime setting is future work.

**CPU-bound task routing is auto-inferred** (per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md)). Tasks whose call graph contains zero may-block calls are purely CPU-bound, so the compiler routes them to a separate blocking thread pool, off the I/O event-loop threads where they would starve I/O completions.

The IDE muted hint at the spawn site shows the routing decision:

```ynz
background processOrder(order)        // muted: // routed to I/O pool — calls db.fetch (may suspend)
background calculateRisk(positions)   // muted: // routed to CPU pool — no may-block calls in call graph
```

**Explicit override: designed, not built.** The design is `background.cpuBound process(data)` (final naming undecided) to force routing when auto-inference is wrong, for example a function that calls no I/O but does heavy parsing or encryption. No such syntax exists, so the routing decision cannot be overridden today. Decided: the form is deferred until a real workload shows the inference getting routing wrong. Registry entry [`background.cpuBound`](../../../registry/features.toml) (`[[deferred_language_feature]]`).

---

## Invariant — None of These Changes May Reintroduce Function Coloring

The decisions in this file (bounded channels, acquire-release atomics, compile-time-assisted preemption, CPU-bound task routing) are all designed to be **invisible at the function-signature level**. The no-coloring design (whole-program may-block analysis + IDE muted `wait` hints) is the load-bearing contract; nothing in these additions touches it.

Specifically:
- **Channel send/receive**: regular method calls. The fact that `.send()` MIGHT suspend on a full channel is detected by the existing may-block analysis, not by a type-level marker on the channel methods.
- **Atomic ordering**: hardware-level memory barriers; doesn't change function signatures at all.
- **CPU-bound task routing**: a spawn-site annotation (`background.cpuBound foo(x)`), NOT a function-signature change. `foo` itself is a plain function; if it calls `bar`, `bar` doesn't need any tag.

Future concurrency primitives must preserve this invariant: any feature that marks a function so that its callers must be marked is function coloring and is rejected.

---

## Scheduler Design

Yinz uses a **work-stealing scheduler**: each OS thread runs a local task queue, and idle threads steal from busy threads' queues. I/O tasks live on the scheduler threads; CPU-bound tasks (auto-inferred from the call graph) route to a separate blocking thread pool and don't compete with I/O completions.

Decided: a single-threaded scheduler and a configurable scheduler are rejected (Yinz ships one runtime).

---

## Task Cancellation

**The end-state model: cancel-via-drop at the next `wait` point.** When a task handle is dropped (or `.cancel()` is called on it), the runtime injects a cancellation signal at the task's next suspension point. The task receives it as a cancellation error, which propagates via `errors` through the call stack. Every value in scope is dropped in order — cleanup runs via existing drop semantics.

This falls out of what's already locked:
- The compiler already knows every suspension point (`wait` = may-block analysis)
- Values already run cleanup via drop-on-unwind (per [`docs/internal/scratchpad/SCRATCH-future-panic-safety.md`](../scratchpad/SCRATCH-future-panic-safety.md))
- `errors` already propagates failures up the call stack

**Edge case — CPU-bound tasks (blocking thread pool)**: CPU-bound tasks have no `wait` points, so cancellation cannot be injected mid-computation. Dropping the handle sends the signal; the task either runs to completion first or panics. This is a documented constraint, not a design gap.

**User-facing model (end-state)**: "Dropping a task handle cancels it. Cleanup always runs."

**Built**: a handle local that leaves its scope stops its task. The runtime half is `ynz_handle_free` (`crates/ynz-runtime/src/handle.rs:519-548`): it aborts the child at its next suspension point via Tokio abort, and the frame, arg-copies, shared-channel refs, Arc-group refs and R8 buffer are each freed exactly once (safe-drop: no use-after-free, no leak, alloc=free-gated). The language half is the scope-exit release pass: a handle binding is one arm of the one release helper (`Cg::release_fn`, `crates/ynz-codegen/src/emit.rs:29334-29345`; its body `Cg::emit_release_body`, `emit.rs:29484-29583`, calls `ynz_handle_free` at `emit.rs:29572`), which the one release emitter (`Cg::emit_release_edge`, `emit.rs:30830-30871`) calls at the same edges as for every other local — see [`IMP-ownership.md`](IMP-ownership.md) "Scope-exit release in a resume function". Two pieces of the end-state model are NOT implemented: the cancellation surfacing inside the child as a typed `errors` value, and an explicit `.cancel()` API.

**Current semantics**: a task whose handle goes out of scope — at block end, loop-iteration end, or the return of the function that bound it, whether that function is a plain function, a suspending one, or itself a `background` task — is stopped there: it is aborted at its NEXT suspension point, so a child that is mid-`wait sleep` at that moment does not resume and its remaining output never prints. There is no detach or join rule. A task that must finish keeps its handle live until then (`h.receive()`). A fire-and-forget `background f()` has no handle to leave a scope, so it keeps running until it finishes or `entrypoint` returns, when `ynz_rt_shutdown` (`crates/ynz-runtime/src/runtime.rs:605-728`) stops every still-running task at its next suspension point. Pinned by `handle_leaving_its_block_stops_the_child_optimized` and `handle_leaving_its_block_stops_the_child_unoptimized` in `crates/ynz-driver/tests/v03_m8_handle_scope_pin.rs`. What the stopped task's own locals release is "The spawn ladder" in [`IMP-ownership.md`](IMP-ownership.md).

**What the stop releases (the transfer rule at exit).** Stopping a task releases what the task owns at the statement it is parked at, by the same rule every scope exit follows ([`IMP-ownership.md`](IMP-ownership.md) "The transfer rule at an exit edge"): its own locals and the values given to it, once each. A value the task already gave away, sent or stored before it parked is not released again; its new owner holds it. A completion the handle never received is released by the handle, so nothing leaks and nothing is freed twice ([`IMP-ownership.md`](IMP-ownership.md) "A task dropped while Pending"). The list a parked task releases is typeck's `ReleasePlan::suspension_edge` (`crates/ynz-typeck/src/release_plan.rs:1267-1269`); codegen's drop entry is `register_drop_state` (`crates/ynz-codegen/src/emit.rs:10790-10960`).

---

## Sleep Intrinsics — Naming & Blocking-vs-Yielding Teaching

There are two sleep intrinsics, distinguished by what they do to the OS thread:

- **`sleep(ms)`** — the **yielding** sleep (the default). Used as `wait sleep(ms)`; the function suspends and hands the thread back to the scheduler, which runs other tasks during the wait. Resumes when the timer fires. This is a suspension point (`wait` is auto-inferred per the no-coloring model).
- **`sleepBlocking(ms)`** — the **blocking** sleep (the labeled exception). Parks the OS thread for `ms` — it sits idle, runs nothing else. NOT a suspension point.

**Both pause the calling code for `ms`. The difference is whether the THREAD is wasted.** The win from yielding only appears when there's other work to run; in a single-purpose program doing one thing, the observable behavior is identical.

**Naming:** the yielding sleep is `sleep` (no `async` jargon, per Golden Rule 5 and [`.claude/rules/vocabulary.md`](../../../.claude/rules/vocabulary.md): Yinz uses `wait`, never `async`/`await`). The blocking exception is `sleepBlocking`, so its name states the danger (`stdlib-design.md` Rule 1).

**Symmetric teaching (the language steers you to the right tool per context):**

| Context | Wrong choice | Diagnostic |
|---|---|---|
| **Kernel mode** (the typeck kernel-mode setting; the driver has no `--kernel` flag yet) | `wait sleep(ms)` (no scheduler to yield to) | **COMPILE ERROR** — `KernelModeRejectsWait`. Built: the `Expr::Wait` arm (`crates/ynz-typeck/src/check.rs:2726-2732`) rejects `wait <anything>` before recursing into the callee, so `wait sleep(ms)` produces exactly this diagnostic, with WHAT-INSTEAD "Remove the keyword or build without `--kernel`." A bare `sleep(ms)` with no `wait` (a separate, also-live arm, `check.rs:3108-3118`) gets the more specific WHAT-INSTEAD "Use `sleepBlocking` for blocking sleep." The same kernel-mode-suspension rejection also covers every bare auto-suspending user-function call (`check.rs:3234-3241`) and its UFCS dot-call form (`check.rs:4849-4857`) — under the no-coloring model, ALL suspending calls, not just explicit `wait`, are rejected under `--kernel`. See [`docs/internal/implementation/IMP-no-runtime-mode.md`](IMP-no-runtime-mode.md). |
| **Normal mode** | `sleepBlocking(ms)` (holds a thread idle when a scheduler is available) | **Tier 3 lint** `prefer-yielding-sleep` (suggestion, dismissable — NOT an error) → use `wait sleep(ms)`. Built (a `[[lint_rule]]`, off in kernel mode). It is a suggestion, not an error, because rare legitimate uses exist and explicit intent is respected ([`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md)). |

Both rows are built.

---

## Open questions

- Deadlock detection: should the runtime detect deadlocks at runtime? At compile time?
- Queue primitives beyond `Channel<T>`: typed concurrent queues for task communication; the design lives in stdlib.

---

## Cross-references

- [`docs/reference/REF-ide-hints.md`](../../reference/REF-ide-hints.md) (muted `wait` rendering protocol)
- [`docs/internal/implementation/IMP-ownership.md`](IMP-ownership.md) (auto-`Arc` for cross-thread shared state)
- [`docs/internal/scratchpad/SCRATCH-future-panic-safety.md`](../scratchpad/SCRATCH-future-panic-safety.md) (panics in `background` tasks)
- [`docs/internal/scratchpad/SCRATCH-future-supervisor.md`](../scratchpad/SCRATCH-future-supervisor.md) (stdlib supervisor helpers)
- [`docs/internal/scratchpad/SCRATCH-future-packages.md`](../scratchpad/SCRATCH-future-packages.md) (binary metadata for may-block propagation across packages)
- [`docs/internal/implementation/IMP-no-runtime-mode.md`](IMP-no-runtime-mode.md) (kernel-mode disables this entire system; users provide their own scheduler)
