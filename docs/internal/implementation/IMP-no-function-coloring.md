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

**Status**: Locked, v0.2 implementation.

User spec target: [`docs/reference/REF-concurrency.md`](../../reference/REF-concurrency.md) (currently has the v0.1 surface syntax; v0.2 fleshes out semantics).

---

## The Decision

Yinz's async model has **no function coloring**. There is no type-level distinction between "async fn" and "sync fn" — every function is just `function`. The compiler does whole-program may-block analysis from the call graph, auto-inserts `wait` at suspension points, and the IDE shows the inserted `wait` as a muted hint per [`docs/reference/REF-ide-hints.md`](../../reference/REF-ide-hints.md).

This is genuinely novel — Rust can't do it (locked into type-level async by 1.39's design), Go won't do it (committed to stackful goroutines), Zig is close but doesn't have the IDE teaching layer.

---

## Why Yinz can do this when Rust can't

Three things Rust gave up that Yinz keeps:

1. **Yinz ships a runtime.** Rust deliberately doesn't (so it can target microcontrollers and kernels). Without a language-controlled runtime, Rust can't do whole-program may-block analysis — there's no scheduler to suspend on. Yinz HAS a runtime (`libynz_rt.a`), so the analysis is tractable. Kernel-mode support is handled separately via [`docs/internal/implementation/IMP-no-runtime-mode.md`](IMP-no-runtime-mode.md), not by skipping the runtime everywhere.

2. **Yinz controls the IDE.** Rust's async syntax has to work without IDE support (some embedded environments edit Rust in vim with no rust-analyzer). Yinz's teaching mission means the IDE is REQUIRED infrastructure — muted hints carry the load that explicit `await` syntax carries in Rust.

3. **No backward-compat constraint.** Rust 1.0 shipped without async. The community built `futures-rs` as a library. Rust 1.39 added `async`/`await` as syntax around that pre-existing ecosystem. The type-level `Future` was inevitable given those constraints. Yinz is 0.1 — we're not stuck.

---

## How it works

### Compile time

1. The compiler builds a call graph for every function in the program (Yinz code + Yinz packages — see [`docs/internal/scratchpad/SCRATCH-future-packages.md`](../scratchpad/SCRATCH-future-packages.md) for the binary metadata that makes cross-package analysis work).
2. For each function, the compiler determines whether it transitively calls any I/O intrinsic, FFI function marked `may-block`, or `wait`-expression. The "may-block" property propagates up the call graph.
3. At every call site to a may-block function, the compiler emits a **suspension point** in the codegen. This is the no-coloring mechanism — automatic, never typed by the user.
4. The IDE protocol shows the inferred suspension as the muted `wait_points` hint before the call expression.

> **Suspension ≠ the `wait` keyword.** The auto-emitted suspension point above is about *suspension correctness* (handing the thread back to the scheduler) — the user never writes `wait` for it. The user-written `wait` keyword is a separate, additional thing: an **ordering barrier** the user adds to force a happens-before between otherwise-independent operations the compiler can't infer. Both are spelled `wait`, but suspension is automatic and ordering is the keyword's only manual job. The authoritative model is [`docs/internal/implementation/IMP-concurrency.md`](IMP-concurrency.md) → "Suspension vs. Ordering — What's Automatic and What `wait` Does (LOCKED 2026-06-05)".

### Runtime

1. `wait` desugars to a state-machine suspension (stackless coroutines, like Rust's async — low memory, fast spawn, minimal context-switch cost).
2. The runtime scheduler (in `libynz_rt.a`) drives suspended state machines forward as I/O completes.
3. `background` spawns a new task onto the scheduler. Tasks are cheap (state-machine memory, no per-task OS stack).
4. Cross-thread shared state is DESIGNED to cross a `background` boundary via auto-inferred `Arc<T>` wrapping — **the codegen emission that does this is deferred to v0.4+** (registry entry `auto-arc-codegen-emission`; see the P2-6 disposition note below and the honesty note under "False Sharing Auto-Padding — Locked Pre-v0.3"). Today, a read-only value shared across a `background` boundary crosses safely via the independent-copy path instead (the compiler infers `.copy`, so each task reads its own copy) — correct and race-free, just not the zero-copy Arc-share form. The IDE shows the auto-Arc muted hint (cautionary red-tinted styling because reference counting has cost) for the teaching surface regardless of which path runs underneath. See [`docs/internal/implementation/IMP-ownership.md`](IMP-ownership.md) for share/lend semantics across thread boundaries.

   **P2-6 disposition (v0.3-M6, confirmed no action needed):** the 2026-07-04 concurrency-release audit's P2-6 finding ("auto-Arc unwired") needs no fix this milestone — it is already correctly registry-deferred to v0.4+ via `auto-arc-codegen-emission` (`[[deferred_language_feature]]`) and `auto-arc-cautionary-tint` (`[[deferred_tooling_feature]]`, the hint's red-tint styling), both self-diagnosing their own WHY (the caller/task Arc-sharing topology `IMP-ownership.md` is cited for but does not actually specify). This is a documentation note, not new registry work — re-confirmed against the live `registry/features.toml` entries during the v0.3-M6 Phase 7 docs/registry honesty sweep, so a reviewer does not mistake the pre-existing deferral for a silently-carried-forward gap.

---

## FFI annotation requirement

**Design for when `foreign` ships (v2+ — see [`docs/reference/REF-ffi.md`](../../reference/REF-ffi.md) and its `[[deferred_language_feature]]` registry entry). No `foreign` keyword exists in the compiler today**, so nothing below runs yet — it is the locked plan for the may-block analysis to extend cleanly onto FFI boundaries once they exist.

The compiler can analyze pure Yinz code. It CANNOT analyze C code linked via FFI — we can't know whether `printf` blocks without knowing what's behind it. So FFI boundaries must declare `may-block` explicitly:

```ynz
foreign function printf(format: string) -> int may-block
foreign function read(fd: int, buf: pointer, n: int) -> int may-block
foreign function memcpy(dst: pointer, src: pointer, n: int) -> pointer    // not annotated → doesn't block
```

This is one line per C function declared. Far less burden than Rust's `async fn` propagation. The compiler treats `may-block` foreign functions as if they were Yinz functions calling I/O intrinsics — call graph propagation works the same way.

---

## Compiled-Yinz package metadata

When the compiler emits a binary Yinz package (`.ynzlib` or whatever format), it MUST embed `may-block` metadata per exported function. This is the BAKE-IN-NOW item: the binary format must reserve space for this metadata from v0.1, even though v0.1 doesn't populate it. Retrofitting later is painful.

See [`docs/internal/scratchpad/SCRATCH-future-packages.md`](../scratchpad/SCRATCH-future-packages.md) for the binary format spec.

When a downstream project consumes a compiled Yinz package, the compiler reads the package's `may-block` metadata and includes the package's functions in its call graph for analysis. Same `wait` insertion works across package boundaries.

---

## What this is NOT

- **Not green threads / stackful coroutines** — those have per-task stack memory overhead. Yinz uses stackless state machines like Rust async, with the function-coloring problem eliminated by the compiler doing the work the user shouldn't have to.
- **Not a hidden `wait`** — the IDE muted hint makes every suspension visible. The user can read the hint, click to make it explicit, hover to learn WHY. This is teaching, not magic.
- **Not "everything is async"** — pure-CPU functions (no I/O, no FFI may-block, no wait inside) have no `wait` inserted at their call sites. The analysis is precise; only call chains that actually reach a suspension point get suspension code.

---

## Channel/Queue Primitives — Bounded by Default, Capacity Auto-Inferred

**Implementation milestone**: SHIPPED v0.3-M4 — `channel<T>()` / `channel<T>(N)` construction, bounded-by-construction typeck + codegen, the kernel-mode gate, the muted default-capacity hint, and send/recv suspension (bare-channel and `background` handle-form) are all live. The default capacity is the locked constant `64` (`DEFAULT_CHANNEL_CAPACITY` in `ynz-typeck` — the single authoritative source per [`.claude/rules/authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md)).

`channel<T>()` constructs a bounded channel with a compiler-chosen default capacity. `channel<T>(N)` overrides with an explicit number. There is NO unbounded constructor — to express unbounded, write `channel<T>(int.max)` and a comment explaining why.

This is the auto-promotion pattern (per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md)):
- **Codegen**: compiler emits the chosen capacity (default or explicit)
- **Muted IDE hint**: always shows the capacity AND whether it was default or user-set, so the bounded-ness is visible at every channel construction
- **Explicit override**: `channel<T>(N)` when the default is wrong for this workload

```ynz
// Source the user wrote:
let queue: channel<Order> = channel<Order>()

// IDE rendering (muted text appears INSIDE the empty parens — addition placement
// per .claude/rules/inference.md "Three Placement Categories"):
let queue: channel<Order> = channel<Order>(64)     // `64` rendered muted (plain
                                                    // text, like every Addition hint)
                                                    // hover tooltip:
                                                    //   WHAT: capacity = 64 (default)
                                                    //   WHAT INSTEAD: write channel<Order>(N) for a different bound
                                                    //   WHY: 64 is sized for typical workloads — backpressure surfaces
                                                    //        within seconds of sustained overproduction
                                                    // click-to-make-explicit: writes "64" into source

// Override case — source already has the number, no muted hint needed:
let bigQueue: channel<Frame> = channel<Frame>(1000)
```

The muted `64` appears in the position where the user would have typed it, not as a comment after the line. Click → "64" gets typed into the source. This is the addition-placement pattern from [`.claude/rules/inference.md`](../../../.claude/rules/inference.md).

### Default capacity — a constant, not a learned value

The default is a **single constant** (shipped: 64, locked at v0.3-M4 P0 — `DEFAULT_CHANNEL_CAPACITY` in `ynz-typeck`; revisit only with real benchmark evidence). Honest reasoning about what the compiler can and can't infer:

- **Can pick a safe constant**: 64 is small enough that sustained overproduction surfaces as backpressure within seconds (not hours of memory growth before OOM), large enough to absorb normal consumer jitter without spurious suspension.
- **Can't pick the truly optimal value statically**: optimal capacity depends on runtime producer/consumer rates and acceptable buffer-time tradeoffs — none knowable at compile time.
- **Can't reliably auto-detect usage patterns**: detecting "this channel is in an HTTP handler, use 1000" would require whole-program data-flow analysis with high false-positive rate. Constant default is simpler and almost as good. Skipped.

### Why allow a default at all (instead of mandatory like the original lock)

The original write-up made `capacity` mandatory specifically so users SEE the bound — they have to think about it. But the muted IDE hint makes the bound visible even with a default, AND the bound is still REAL (the channel is always bounded; just the number isn't always in source). Net effect:

- **Mandatory + no IDE hint**: high friction, beginners struggle, but the bound is in source
- **Default + always-visible muted hint**: ergonomic, beginners get safe behavior automatically, AND the bound is visible (just in the IDE rather than the source bytes)

The IDE-hint approach is strictly better — same safety property, less friction, the dev SEES the choice and learns through reading their own annotated code. This matches the broader auto-promotion philosophy ("fast by design even for inexperienced developers" + visible teaching surface).

### Why mandatory bounded

Erlang actor mailboxes have been unbounded by design since 1987. When a producer outpaces a consumer, messages accumulate until the node runs out of memory and crashes. OTP 19 (2016) added `max_heap_size` as a reactive mitigation — 29 years later — and even that just kills the process when the heap exceeds a threshold; it does not apply backpressure to senders. Fred Hebert's canonical "Handling Overload" guide (https://ferd.ca/handling-overload.html) calls this "still reactive, not preventive." Rust's `tokio::mpsc::unbounded_channel` and Node.js streams pre-streams2 made the same mistake with the same symptoms.

The pattern: unbounded queues hide backpressure → producer outpaces consumer → memory exhausts → OOM kill in production with no log entry showing the actual cause. Looks like a "random crash."

### Behavior on full channel

Sending to a full channel **suspends the sender** (applies backpressure). The sender's `background` task pauses until capacity opens. This propagates: if the sender has its own caller waiting on its result, the caller suspends too — the natural backpressure cascade.

This is the desired behavior. Documentation must explicitly state that "my producer is suspended" is backpressure working correctly, not a deadlock — to head off the misdiagnosis.

### What `capacity` does NOT constrain

`capacity` is the queue's max size at any moment, not a constraint on producers or consumers:

- **Many producers, one consumer**: fine. All producers send into the same queue; capacity is the total unread items.
- **One producer, many consumers**: fine. Consumers all pull from the same queue; the queue load-balances naturally.
- **Many producers AND many consumers**: fine. Standard work-distribution pattern.

The bounded behavior only kicks in when the queue actually fills. With capacity 100, 5 producers sending at moderate rates and 3 consumers reading at moderate rates produces no suspension as long as the consumers keep up. Suspension is the safety net for sustained-overproduction; it doesn't fire in steady-state balanced workloads.

### Backpressure is for writes, not reads

This rule applies to **writes into a queue** (`.send()` / channel push). It does NOT apply to reading shared memory via `.share` — that's a different mechanism (ownership-based, no queue exists, no buildup possible). When multiple tasks read the same `share`-borrowed value, no backpressure question arises because nothing is being queued. The ownership system handles the coordination at compile time.

### Cross-references

- [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) Rule 4 (bounded by default) — channel/queue is the canonical instance of this rule.
- `lockin-concurrency.md` Findings #12 (Erlang) and #16 (Node.js streams2) for the source pain.

---

## Atomic Ordering Default — Acquire-Release, Not Sequential-Consistency

Yinz's channel and shared-state primitives use **acquire-release** ordering by default for all send/receive operations. Sequential-consistency is available as an explicit opt-in for the rare use case requiring a global total order across multiple concurrent operations.

### Why acquire-release default

C++ atomics default to `memory_order_seq_cst`. On x86 (TSO model), seq-cst overhead over acquire-release is <0.4% geomean — essentially free. On ARM (the dominant mobile and embedded architecture), seq-cst requires explicit barrier instructions on both loads AND stores. Acquire-release requires barriers only at the transition points. Clang/LLVM only added the more efficient `LDAPR` instruction (ARMv8.1 acquire-with-release) in LLVM 16 (March 2023).

For the synchronization patterns that actually appear in user code (handoff via channel, lock acquire/release, reference counting), acquire-release is sufficient and correct. Seq-cst is needed only for unusual patterns where a global total order across multiple concurrent operations is part of the program's correctness contract — rare even in systems code.

### Surface for users

Per Golden Rule 12 (no jargon), Yinz does NOT expose `memory_order_seq_cst` or `memory_order_acquire` as user-visible names. The default ordering applies automatically to channel/atomic operations. Users who genuinely need the global-total-order guarantee opt in via a named API: candidate `atomic.add(n).withGlobalOrdering()` (final naming TBD when concurrency primitives ship in v0.2). The compiler does NOT expose memory ordering as a tunable parameter to non-systems users.

### Tradeoff

Acquire-release is weaker than sequential consistency. Code that relies on seq-cst's global total order (a correctness requirement in certain multi-producer/multi-consumer patterns) will be silently wrong if the programmer reaches for the default and assumes seq-cst. Mitigated by:
1. The Yinz channel model makes cross-thread coordination explicit via typed messages — most patterns don't need raw atomics.
2. The exceptional case (global order needed) is surfaced through an explicit API rather than a default-flip parameter.

### Cross-references

- ARM seq-cst analysis: https://community.arm.com/arm-community-blogs/b/tools-software-ides-blog/posts/armv8-sequential-consistency

---

## False Sharing Auto-Padding — Locked Pre-v0.3

When the compiler detects that a `shape` type crosses a thread boundary at a `background` spawn site, it emits the shape with 64-byte cache-line alignment and inserts padding between independently-accessed fields to prevent false sharing. Detection keys on the LEGAL crossing set — `give`/`copy` arguments, `channel<T>` conduit element types, and callee return types at `background` spawn sites — because `.share`/`.lend` across `background` is a hard compile error as of v0.3-M4 (see [`IMP-concurrency.md`](IMP-concurrency.md) "Ownership with Background Tasks — Why `.share` Fails"); the original intent here ("ownership crossing thread boundaries") is realized on the legal modifier set.

**Why**: Two atomic variables placed in the same 64-byte cache line cause writes to either variable to invalidate the entire cache line for all cores — even though no data is actually shared. Production measurements show a 3.1× throughput collapse from false sharing on lock-free queue head/tail pointers (https://alic.dev/blog/false-sharing). The fix (128-byte isolation) recovers the full 3.1×. The compiler has all the information needed to apply this automatically.

**Surface**: Codegen-only. No user syntax. No typeable form exists (there is no `@cacheLineSeparate` annotation in Yinz), so no muted-hint surface (per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md) — muted hints require a typeable explicit form). A Tier 3 lint `cross-thread-fields-not-padded` fires if the compiler detects a shape with cross-thread-accessed fields that it cannot auto-pad. The v0.3 unpaddable (decline-lint) class is cross-module-visible-layout shapes — exported / imported / `__anon__*` structural shapes — because each module compiles to its own object file, so padding only the spawning module's view would fork one type's layout (`foreign`/FFI is a v2+ `[[deferred_language_feature]]` and cannot occur in v0.3).

**Implementation milestone**: SHIPPED v0.3-M4 — the padding transform and the `cross-thread-fields-not-padded` decline lint are live (both gate off under `--no-auto-parallel`, reading the same authoritative analysis predicate). Honesty note (FRAGO 009, v0.3-M4 plan): every crossing v0.3 can produce today is an exclusive-ownership handoff (`give`/`copy`/channel payload), which structurally cannot false-share — the transform ships as real, tested, forward-looking infrastructure whose throughput benefit arrives when auto-Arc codegen emission ships (v0.4+, registry entry `auto-arc-codegen-emission`) and creates genuinely-shared concurrent instances.

---

## Scheduler Preemption Model — Locked Pre-v0.2

Yinz uses **compile-time-assisted safe-point preemption**: the compiler turns loop back edges inside state-machine functions into real poll-yield suspension points, and the runtime's budget check decides at each one "should I yield to another task" — no async signal-handler infrastructure. The shipped architecture is **three-part** (v0.3-M7 Phase 6, replacing the pre-M7 stub status this section previously documented):

### The three-part shipped architecture (v0.3-M7)

1. **State-machine-function loop back edges — codegen poll-yield (NEW, v0.3-M7).** Inside an *admitted* state-machine (may-suspend) function, every qualifying loop back edge is a genuine suspension point: codegen emits `br (ynz_rt_check_preempt(waker_ctx)), yield, header` — the yield stores the resume point through the existing suspension machinery (`store_resume_point` + the per-statement crossing-local flush discipline) and returns Pending; the continuation state reloads and re-enters at the loop header. The budget check wakes the task from OFF the worker thread before the Pending returns (a remote wake via the blocking pool), so a yield is a fair requeue — Tokio's LIFO fast path would otherwise let the yielding task starve its siblings (the tokio `yield_now` self-wake problem).
   - **Qualifying loops**: `while`, and `for` over a literal `range(...)`, an `array<T>`, or a `map<K, V>`. **Named exclusions** (these loop forms keep pre-M7 behavior — a budget-ticking call with no yield): `for` over `fixed<T>` (stack-backed elements cannot survive a suspension — the same hazard the FixedArrayIterWithWait check rejects for explicit waits), and `for` over a string / shape-iterable / stored-range variable (the state-machine walker's non-frame-backed fallback forms).
   - **Per-function admission (safe default: decline = byte-identical pre-M7 behavior)**: a function is admitted only when treating its loop back edges as suspension points forces no un-frame-backable type (`fixed<T>` / `maybe` / `union` / `dynamic` / nested-shape shape) to cross a suspension, no non-qualifying `for` form wraps a qualifying loop, and no two `for` loops share a loop-variable name across different element types (the name-keyed frame-slot machinery would collide). Declined functions are part of residual (3) below. The admission verdict is computed once in typeck (`back_edge_yield_admitted`) and threaded to every consumer — routing, suspension-point counting, and crossing-local collection all read the ONE producer.
2. **Non-state-machine CPU-bound work — blocking-pool routing (already shipped, unchanged).** A plain synchronous function has no frame, cannot return Pending, and has nothing to resume — no yield mechanism exists or can exist for it under the no-coloring design. Its protection is CPU-admission routing (below): tasks whose call graph contains zero may-block calls run on the blocking pool, off the I/O workers.
3. **The named residual — CPU-heavy code no mechanism covers.** (a) CPU-heavy code inside a non-SM function that a may-suspend task calls: it runs on an I/O worker with no yield until it returns — admission routing is a heuristic over the *task's* call graph, not a per-function guarantee. (b) Loop-free CPU-bound recursion inside a state-machine function (no back edge to yield at) — the shape call-site checks would have covered. (c) Loops in *declined* SM functions and the excluded loop forms above. These are recorded residuals (the v0.3-M7 plan's Future Requirements), not silent gaps.

**Call-site preemption checks: measured and DEFERRED (v0.3-M7 Phase 6).** The fresh measurement under the real default optimizer pipeline — against a pre-registered ≤5% threshold on the fib(30) call-heavy microbenchmark — came in at **~+398%** (toggle-on median 132ms vs ~26.5ms off): far below the O0-era 1190% figure, but still ~80× over the bar, because an opaque per-call-site check defeats exactly the inlining the optimizer delivers. Per the measurement gate they did NOT ship; the four-field deferral lives in registry entry [`preempt-callsite-checks`](../../../registry/features.toml) (`[[deferred_language_feature]]`), with the emission preserved behind a compile-time toggle (`YNZ_PREEMPT_CALLSITE_CHECKS`) and the microbenchmark fixture committed for the next re-measurement. (The earlier [`cooperative-preemption-back-edge-yield`](../../../registry/features.toml) `[[deferred_tooling_feature]]` entry tracked the back-edge half — now shipped by this milestone.)

### Why decided pre-v0.2

Go shipped its 1.0 scheduler (March 2012) with cooperative-only preemption. Goroutines yielded only at function calls, channel ops, or mutex contention. A tight CPU-bound loop with no function calls monopolized its P (logical processor) indefinitely, starving every other goroutine on the same P. The fix (asynchronous SIGURG-based preemption) shipped in Go 1.14 (February 2020) — 8 years later. During those 8 years, every Go production service with mixed CPU+I/O workloads was vulnerable to I/O latency spikes from a single tight CPU-bound goroutine.

Yinz cannot afford the equivalent 8-year window. The preemption model must be locked before v0.2 ships any concurrency primitives, because once user code is written against a cooperative model, the migration is exactly as painful as Go's was.

### Why compile-time-assisted (not signal-based)

Signal-based async preemption (Go 1.14's approach) has zero per-instruction cost but requires careful interaction with the compiler's optimization pipeline, debug symbol tables, and the OS's signal-handling semantics. SIGURG is chosen specifically to avoid debugger conflicts; this kind of cross-cutting concern is fragile.

Compile-time safe-point insertion fits Yinz better:
- The compiler already has visibility into function call graphs and loop back-edges (it needs them for ownership analysis anyway)
- Preemption checks are just an extra `if (scheduler.shouldYield) { suspend(); }` at known points
- LLVM can hoist these checks out of provably-short loop bodies, so per-iteration cost is amortized
- No signal handler complexity

### Time quantum and CPU-bound task routing

The ~10ms quantum is a **wall-clock target, implemented as a deterministic poll count** (v0.3-M7): the runtime's budget is a pure per-worker call count (`2^20` back-edge polls between granted yields, `ynz_rt_check_preempt` in `crates/ynz-runtime/src/runtime.rs`), which lands in the ~2-15ms band for the tight hot loops the mechanism exists to break. **Why count, not clock** (a deliberate, recorded v0.3-M7 divergence from the original clock sketch): a wall-clock budget makes whether a given loop yields depend on run-to-run timing jitter, making compiled-program output NONDETERMINISTIC across runs — reproduced live on the pirates-roster demo the moment a clock-based cut landed. Byte-exact output goldens and cross-mode byte-identity are load-bearing guarantees in this compiler's test architecture, so the budget is a deterministic function of program execution alone. The honest cost: loops with heavy per-iteration bodies yield less often than a strict 10ms clock would; a per-runtime configuration surface for the interval remains future work, and the constant is the whole configuration today.

**CPU-bound task routing is auto-inferred** (per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md)). The compiler's whole-program may-block analysis already determines which functions can suspend. Tasks whose call graph contains zero may-block calls are purely CPU-bound — the compiler routes them to a separate blocking thread pool, off the I/O event-loop threads. This avoids the Tokio failure mode where CPU work on the I/O scheduler starves I/O completions.

The IDE muted hint at the spawn site shows the routing decision:

```ynz
background processOrder(order)        // muted: // routed to I/O pool — calls db.fetch (may suspend)
background calculateRisk(positions)   // muted: // routed to CPU pool — no may-block calls in call graph
```

**Explicit override — spec'd, NOT implemented** (per the auto-promotion rule's "force-the-other-pick" direction): if the auto-inference gets it wrong — e.g., a function that doesn't call any I/O but does heavy parsing/encryption/encoding the compiler can't see is dominant — the design calls for the user to force routing with `background.cpuBound process(data)` (final naming TBD). No such syntax exists in the compiler today; there is no way to override the auto-inferred routing decision above. Per `auto-promotion.md`'s override-direction checklist, building this form is deliberately deferred until a real workload demonstrates the auto-inference getting CPU-bound routing wrong — an unused override built ahead of that evidence is speculative (YAGNI). Tracked as registry entry [`background.cpuBound`](../../../registry/features.toml) (`[[deferred_language_feature]]`).

### Cross-references

- `lockin-concurrency.md` Finding #15 for the Go cooperative-only history.
- Go's non-cooperative preemption proposal: https://go.googlesource.com/proposal/+/master/design/24543-non-cooperative-preemption.md

---

## Invariant — None of These Changes May Reintroduce Function Coloring

The decisions in this file (bounded channels, acquire-release atomics, compile-time-assisted preemption, CPU-bound task routing) are all designed to be **invisible at the function-signature level**. The no-coloring design (whole-program may-block analysis + IDE muted `wait` hints) is the load-bearing contract; nothing in these additions touches it.

Specifically:
- **Channel send/receive**: regular method calls. The fact that `.send()` MIGHT suspend on a full channel is detected by the existing may-block analysis, not by a type-level marker on the channel methods.
- **Atomic ordering**: hardware-level memory barriers; doesn't change function signatures at all.
- **CPU-bound task routing**: a spawn-site annotation (`background.cpuBound foo(x)`), NOT a function-signature change. `foo` itself is a plain function; if it calls `bar`, `bar` doesn't need any tag.

Future additions to concurrency primitives must preserve this invariant. Any proposed feature that requires a function's CALLERS to be marked because the function is marked = function coloring = REJECTED. Re-read `lockin-concurrency.md` (Round 1 source) for the Rust async-cascade pain this design avoids; the trap is real and easy to fall into.

---

## Scheduler Design — Locked Pre-v0.2

Yinz uses a **work-stealing scheduler**: each OS thread runs a local task queue, and idle threads steal tasks from busy threads' queues. This is the same model used by Go, Tokio, Java ForkJoinPool, and .NET ThreadPool — the proven choice for multi-core task scheduling.

**Why work-stealing and not alternatives**:
- Single-threaded: can't utilize multiple cores for concurrent tasks. Incompatible with Yinz's goal of auto-parallelization (v0.3).
- Configurable: violates the one-runtime principle. Rust's "choose Tokio vs async-std vs smol" is exactly the ecosystem split Yinz avoids by shipping one runtime.

The CPU/IO split (already locked above) maps cleanly to work-stealing: I/O tasks live on the work-stealing scheduler threads; CPU-bound tasks (auto-inferred from call graph) route to a separate blocking thread pool and don't compete with I/O completions.

---

## Task Cancellation — Locked Pre-v0.2

**The locked end-state model — cancel-via-drop at the next `wait` point.** When a task handle is dropped (or `.cancel()` is called on it), the runtime injects a cancellation signal at the task's next suspension point. The task receives it as a cancellation error, which propagates via `errors` through the call stack. Every value in scope is dropped in order — cleanup runs via existing drop semantics.

This falls out of what's already locked:
- The compiler already knows every suspension point (`wait` = may-block analysis)
- Values already run cleanup via drop-on-unwind (per [`docs/internal/scratchpad/SCRATCH-future-panic-safety.md`](../scratchpad/SCRATCH-future-panic-safety.md))
- `errors` already propagates failures up the call stack

**Edge case — CPU-bound tasks (blocking thread pool)**: CPU-bound tasks have no `wait` points, so cancellation cannot be injected mid-computation. Dropping the handle sends the signal; the task either runs to completion first or panics. This is a documented constraint, not a design gap — the same behavior as Rust's `spawn_blocking`.

**User-facing model (end-state)**: "Dropping a task handle cancels it. Cleanup always runs."

**Implementation milestone**: SHIPPED-DEFERRED v0.3-M4 — the RUNTIME half is live and substrate-proven: `ynz_handle_free` (`crates/ynz-runtime/src/handle.rs`) aborts the child at its next suspension point via Tokio abort, and the frame + arg-copies + shared-channel refs + the R8 buffer are each freed exactly once (safe-drop: no use-after-free, no leak, alloc=free-gated). The LANGUAGE half — codegen automatically calling `ynz_handle_free` when a handle binding's scope ends (`ynz_handle_free` is declared in codegen's `runtime_decls.rs` but NEVER emitted — zero call sites), the cancellation surfacing inside the child as a typed `errors` value, and any explicit `.cancel()` API — is NOT implemented. **v0.3 shipped semantics**: a task whose handle goes out of scope runs to completion (fire-and-forget) — it is never silently killed mid-work; the un-freed handle object is one small Box per spawn, reclaimed at process exit. Deferred to the future language-wide scope-drop/destructor mechanism (a handle-only drop pass would fork a second cleanup mechanism the eventual general one must unify, per [`.claude/rules/authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md)); four-field deferral recorded in the [v0.3-M4 plan's Future Requirements](../../../.claude/planning/active/2026-07-02-v0-3-m4-channels-arc-release/plan.md) (registry entry `background-handle-cancel-injection`; trigger: the language-wide scope-drop mechanism shipping, OR a real workload needing task cancellation — whichever lands first).

---

## Sleep Intrinsics — Naming & Blocking-vs-Yielding Teaching — DECIDED 2026-06-01

There are two sleep intrinsics, distinguished by what they do to the OS thread:

- **`sleep(ms)`** — the **yielding** sleep (the default). Used as `wait sleep(ms)`; the function suspends and hands the thread back to the scheduler, which runs other tasks during the wait. Resumes when the timer fires. This is a suspension point (`wait` is auto-inferred per the no-coloring model).
- **`sleepBlocking(ms)`** — the **blocking** sleep (the labeled exception). Parks the OS thread for `ms` — it sits idle, runs nothing else. NOT a suspension point.

**Both pause the calling code for `ms`. The difference is whether the THREAD is wasted.** The win from yielding only appears when there's other work to run; in a single-purpose program doing one thing, the observable behavior is identical.

**Naming history:** these shipped in M1/M2 as `sleepMs` (blocking) and `sleepAsync` (yielding). Both are mis-named: `sleepAsync` smuggles the `async` jargon the language exists to hide (Golden Rule 12 + [`.claude/rules/vocabulary.md`](../../../.claude/rules/vocabulary.md) — Yinz uses `wait`, never `async`/`await`), `wait sleepAsync` states "suspend" twice, and `sleepMs`/`sleepAsync` mix naming bases (unit vs mechanism). Renamed to `sleep` (yielding default) + `sleepBlocking` (blocking exception names its danger, per `stdlib-design.md` Rule 1). The rename is cheapest pre-external-users (M3 kickoff or standalone PR).

**Symmetric teaching (the language steers you to the right tool per context):**

| Context | Wrong choice | Diagnostic |
|---|---|---|
| **`--kernel` mode** | `wait sleep(ms)` (no scheduler to yield to) | **COMPILE ERROR** — `KernelModeRejectsWait`. **SHIPPED and confirmed live** (v0.3-M1, re-verified v0.3-M6): the `Expr::Wait` arm (`crates/ynz-typeck/src/check.rs:2726-2732`) rejects `wait <anything>` before recursing into the callee, so `wait sleep(ms)` produces exactly this diagnostic, with WHAT-INSTEAD "Remove the keyword or build without `--kernel`." A bare `sleep(ms)` with no `wait` (a separate, also-live arm, `check.rs:3108-3118`) gets the more specific WHAT-INSTEAD "Use `sleepBlocking` for blocking sleep." The same kernel-mode-suspension rejection also covers every bare auto-suspending user-function call (`check.rs:3234-3241`) and its UFCS dot-call form (`check.rs:4849-4857`) — under the no-coloring model, ALL suspending calls, not just explicit `wait`, are rejected under `--kernel`. See [`docs/internal/implementation/IMP-no-runtime-mode.md`](IMP-no-runtime-mode.md). |
| **Normal mode** | `sleepBlocking(ms)` (holds a thread idle when a scheduler is available) | **Tier 3 lint** `prefer-yielding-sleep` (suggestion, dismissable — NOT an error) → use `wait sleep(ms)`. **SHIPPED v0.3-M4** (rides the `[[lint_rule]]` infra built there, kernel-gated off; M4's `background` handle-form also removed the last legit non-kernel blocking-sleep use — the keepalive pattern — so the lint stops nagging a valid case). Must be a suggestion, not an error: rare legit uses exist + respect explicit intent ([`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md)). |

Execution was tracked in [`.claude/planning/active/2026-05-21-v0-3-concurrency-perf/roadmap.md`](../../../.claude/planning/active/2026-05-21-v0-3-concurrency-perf/roadmap.md) ("Sleep intrinsic naming + blocking-vs-yielding teaching" architectural decision + M4 scope); the normal-mode lint shipped in v0.3-M4, and the `--kernel` compile-error row above is likewise live (corrected v0.3-M6 — it previously read as still-reserved; direct read of `check.rs` this session confirmed the rejection arms are shipped, not pending).

---

## Open questions for v0.2 implementation milestone

These don't need to be answered NOW; the v0.2 milestone plan resolves them:

- Deadlock detection: should the runtime detect deadlocks at runtime? At compile time?
- Channel/queue primitives: Yinz needs typed concurrent queues for tasks to communicate. Design lives in stdlib.

The v0.2 milestone plan must include these in its `### Open questions` section before implementation starts.

---

## Cross-references

- [`docs/reference/REF-ide-hints.md`](../../reference/REF-ide-hints.md) (muted `wait` rendering protocol)
- [`docs/internal/implementation/IMP-ownership.md`](IMP-ownership.md) (auto-`Arc` for cross-thread shared state)
- [`docs/internal/scratchpad/SCRATCH-future-panic-safety.md`](../scratchpad/SCRATCH-future-panic-safety.md) (panics in `background` tasks)
- [`docs/internal/scratchpad/SCRATCH-future-supervisor.md`](../scratchpad/SCRATCH-future-supervisor.md) (stdlib supervisor helpers)
- [`docs/internal/scratchpad/SCRATCH-future-packages.md`](../scratchpad/SCRATCH-future-packages.md) (binary metadata for may-block propagation across packages)
- [`docs/internal/implementation/IMP-no-runtime-mode.md`](IMP-no-runtime-mode.md) (kernel-mode disables this entire system; users provide their own scheduler)
