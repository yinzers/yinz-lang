# Concurrency — No Function Coloring

**Status**: Locked, v0.2 implementation.

User spec target: `spec/concurrency.md` (currently has the v0.1 surface syntax; v0.2 fleshes out semantics).

---

## The Decision

Yinz's async model has **no function coloring**. There is no type-level distinction between "async fn" and "sync fn" — every function is just `function`. The compiler does whole-program may-block analysis from the call graph, auto-inserts `wait` at suspension points, and the IDE shows the inserted `wait` as a muted hint per [`design/ide-hints.md`](ide-hints.md).

This is genuinely novel — Rust can't do it (locked into type-level async by 1.39's design), Go won't do it (committed to stackful goroutines), Zig is close but doesn't have the IDE teaching layer.

---

## Why Yinz can do this when Rust can't

Three things Rust gave up that Yinz keeps:

1. **Yinz ships a runtime.** Rust deliberately doesn't (so it can target microcontrollers and kernels). Without a language-controlled runtime, Rust can't do whole-program may-block analysis — there's no scheduler to suspend on. Yinz HAS a runtime (`libynz_rt.a`), so the analysis is tractable. Kernel-mode support is handled separately via [`design/future/no-runtime-mode.md`](no-runtime-mode.md), not by skipping the runtime everywhere.

2. **Yinz controls the IDE.** Rust's async syntax has to work without IDE support (some embedded environments edit Rust in vim with no rust-analyzer). Yinz's teaching mission means the IDE is REQUIRED infrastructure — muted hints carry the load that explicit `await` syntax carries in Rust.

3. **No backward-compat constraint.** Rust 1.0 shipped without async. The community built `futures-rs` as a library. Rust 1.39 added `async`/`await` as syntax around that pre-existing ecosystem. The type-level `Future` was inevitable given those constraints. Yinz is 0.1 — we're not stuck.

---

## How it works

### Compile time

1. The compiler builds a call graph for every function in the program (Yinz code + Yinz packages — see `packages.md` for the binary metadata that makes cross-package analysis work).
2. For each function, the compiler determines whether it transitively calls any I/O intrinsic, FFI function marked `may-block`, or `wait`-expression. The "may-block" property propagates up the call graph.
3. At every call site to a may-block function, the compiler INSERTS a `wait` suspension point in the codegen.
4. The IDE protocol shows the inserted `wait` as muted text before the call expression. The user can type `wait` explicitly to make it visible in source.

### Runtime

1. `wait` desugars to a state-machine suspension (stackless coroutines, like Rust's async — low memory, fast spawn, minimal context-switch cost).
2. The runtime scheduler (in `libynz_rt.a`) drives suspended state machines forward as I/O completes.
3. `background` spawns a new task onto the scheduler. Tasks are cheap (state-machine memory, no per-task OS stack).
4. Cross-thread shared state crosses a `background` boundary via auto-inferred `Arc<T>` wrapping. The IDE shows the auto-Arc as a muted hint (cautionary red-tinted styling because reference counting has cost). See [`design/ownership.md`](../ownership.md) for share/lend semantics across thread boundaries.

---

## FFI annotation requirement

The compiler can analyze pure Yinz code. It CANNOT analyze C code linked via FFI — we can't know whether `printf` blocks without knowing what's behind it. So FFI boundaries must declare `may-block` explicitly:

```yinz
foreign function printf(format: string) -> int may-block
foreign function read(fd: int, buf: pointer, n: int) -> int may-block
foreign function memcpy(dst: pointer, src: pointer, n: int) -> pointer    // not annotated → doesn't block
```

This is one line per C function declared. Far less burden than Rust's `async fn` propagation. The compiler treats `may-block` foreign functions as if they were Yinz functions calling I/O intrinsics — call graph propagation works the same way.

---

## Compiled-Yinz package metadata

When the compiler emits a binary Yinz package (`.ynzlib` or whatever format), it MUST embed `may-block` metadata per exported function. This is the BAKE-IN-NOW item: the binary format must reserve space for this metadata from v0.1, even though v0.1 doesn't populate it. Retrofitting later is painful.

See [`design/future/packages.md`](packages.md) for the binary format spec.

When a downstream project consumes a compiled Yinz package, the compiler reads the package's `may-block` metadata and includes the package's functions in its call graph for analysis. Same `wait` insertion works across package boundaries.

---

## What this is NOT

- **Not green threads / stackful coroutines** — those have per-task stack memory overhead. Yinz uses stackless state machines like Rust async, with the function-coloring problem eliminated by the compiler doing the work the user shouldn't have to.
- **Not a hidden `wait`** — the IDE muted hint makes every suspension visible. The user can read the hint, click to make it explicit, hover to learn WHY. This is teaching, not magic.
- **Not "everything is async"** — pure-CPU functions (no I/O, no FFI may-block, no wait inside) have no `wait` inserted at their call sites. The analysis is precise; only call chains that actually reach a suspension point get suspension code.

---

## Open questions for v0.2 implementation milestone

These don't need to be answered NOW; the v0.2 milestone plan resolves them:

- Scheduler design: work-stealing? Single-threaded? Configurable? (Rust gives you Tokio, async-std, smol — Yinz must pick ONE default and document it.)
- Cancellation: how does a `background` task get cancelled? Cooperative checkpoint? `wait` polls a cancellation token? Hard-kill?
- Deadlock detection: should the runtime detect deadlocks at runtime? At compile time?
- Channel/queue primitives: Yinz needs typed concurrent queues for tasks to communicate. Design lives in stdlib.

The v0.2 milestone plan must include these in its `### Open questions` section before implementation starts.

---

## Cross-references

- [`design/ide-hints.md`](../ide-hints.md) (muted `wait` rendering protocol)
- [`design/ownership.md`](../ownership.md) (auto-`Arc` for cross-thread shared state)
- [`design/future/panic-safety.md`](panic-safety.md) (panics in `background` tasks)
- [`design/future/supervisor.md`](supervisor.md) (stdlib supervisor helpers)
- [`design/future/packages.md`](packages.md) (binary metadata for may-block propagation across packages)
- [`design/future/no-runtime-mode.md`](no-runtime-mode.md) (kernel-mode disables this entire system; users provide their own scheduler)
