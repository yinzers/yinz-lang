# Arena Allocators — Scoped Memory Pools

**Status**:
- **Locked, v0.2 implementation**: A1 (named scope) + A2 (anonymous scope) — the default arena patterns
- **Deferred to v0.3+**: Option B (explicit `Arena()` + `.reset()`) — power-user pattern, requires lifecycle tracking

User spec target: `spec/memory.md` (new file when implemented).

---

## The Decision

Yinz supports arena allocators as scope-bounded memory pools. Allocations inside the scope use the arena (one-add-instruction allocation, zero-cost per-object cleanup); the entire arena is wiped in a single CPU instruction at scope exit. **10-100x faster than malloc/free for scope-bounded workloads.**

Default pattern: scope blocks (A1/A2). Power-user pattern: explicit Arena values (B, deferred to v0.3+).

---

## The Concept (for the engineering audience)

Default per-object allocation:
- Create a Player: allocator finds free memory, marks chunk as used (~20-100ns per allocation, plus bookkeeping memory)
- Delete the Player: allocator marks chunk as free (~20-100ns per deallocation)
- For millions of small allocations, bookkeeping dominates

Arena allocation:
- Ask for one big buffer upfront (say 10MB)
- Each allocation just bumps a pointer forward (~1ns)
- Per-object deletions are no-ops (don't track)
- Wipe the entire arena at scope end in one CPU instruction (memset or just reset the bump pointer)

**When this wins**: workloads with scope-bounded allocation lifetimes. Per-request data in web servers. Per-file parse trees in compilers. Per-frame state in games. Per-batch data in batch processors. **The Yinz compiler itself should use arenas internally** for parse/typeck/codegen — see "Yinz compiler should eat its own dogfood" below.

**When this doesn't fit**: long-lived objects (global config, database connection, cached lookup tables). These need traditional heap allocation that survives across many scopes.

---

## A1 — Named scope (the most common pattern)

```yinz
arena scratch {
  let users = array<User>()              // allocated in 'scratch'
  let nameMap = map<string, User>()      // allocated in 'scratch'
  let buffer = string()                  // allocated in 'scratch'

  process(users.share)
}  // scratch (and everything in it) freed in one operation
```

Inside an `arena <name> { ... }` block, the default allocator IS the arena. Every constructor call within the block uses the arena unless explicitly overridden. The arena lives for the scope; everything in it dies when the scope exits.

The IDE shows muted `.in(scratch)` after each constructor — per the [uniform inference rule](../../.claude/rules/inference.md), the allocator choice is figured out from context and rendered as a muted hint.

---

## A2 — Anonymous scope (when you don't need to reference the arena)

```yinz
arena {
  let temp = array<int>()
  let bigBuffer = string()
  computeStuff(temp, bigBuffer)
}  // arena wiped here
```

Identical to A1 except no name. Use when no code inside the scope needs to reference the arena explicitly (passing it to other functions, etc.). Cleaner syntax for the simple case.

---

## Option B (DEFERRED to v0.3+) — Explicit Arena values

```yinz
let scratch = Arena()                    // manually create an arena
let users = array<User>.in(scratch)      // explicit per-allocation
let names = map<string, User>.in(scratch)
process(users.share)
scratch.reset()                          // manually wipe
```

This is the power-user pattern. Use cases A1/A2 don't cover:

- **Cross-function arenas**: a per-connection arena passed to many handlers. The arena outlives any one function's scope.
- **Pooled arenas**: reuse the same arena across iterations to avoid the allocator overhead of `Arena()` itself. (Per-iteration: `scratch.reset()` instead of dropping and recreating.)
- **Multi-arena patterns**: several arenas in flight, allocate to specific ones based on lifetime requirements.

### Why Option B is deferred

The compiler safety story for Option B requires **lifecycle tracking comparable to the borrow checker**:

- An `Arena()` value can be passed to other functions. The compiler must track ownership and ensure exactly one `.reset()` or drop per arena.
- Allocations inside the arena hold references to it (otherwise the arena could be dropped while values are still live). The borrow checker must enforce arena-lifetime ≥ allocation-lifetime.
- On every code path through the function (every branch, every early return, every panic), the analysis must verify: does `.reset()` happen before the arena goes out of scope, OR is the arena dropped (which also wipes)?

This is non-trivial. A1/A2 sidestep the analysis entirely because the scope IS the lifetime — no tracking needed. The compiler just checks "scope started" → "scope ended" → wipe.

Option B is documented now so v0.3+ planning has the design. v0.2 ships with A1/A2; if real user workloads emerge that A1/A2 can't serve (per-connection arenas in a web framework, e.g.), Option B becomes the next priority.

---

## Performance characteristics

A1/A2 allocation:
- Bump-pointer allocation: ~1ns (one ADD)
- Per-object delete: 0ns (no-op)
- Scope-exit wipe: 1 instruction (reset the bump pointer)
- Memory overhead: arena buffer pre-allocated; unused tail is wasted memory
- Cache: contiguous allocations get cache-friendly layout

Compared to malloc/free:
- Malloc per object: ~20-100ns (allocator searches for free space)
- Free per object: ~20-100ns
- For 1000 small objects: malloc/free ≈ 20-200μs total; arena ≈ 1-2μs total. **10-100x speedup.**

The win is largest for many-small-allocations workloads. For a few large allocations, malloc isn't much worse.

---

## Yinz compiler should eat its own dogfood

The Yinz compiler is a textbook arena workload:

- **Parse phase**: AST nodes for one file live until the next file's parse. → one arena per file, wipe at end.
- **Type check phase**: intermediate types and inference state for one function live until next function. → one arena per function.
- **Codegen phase**: LLVM IR builders for one module live until next module. → one arena per module.

Rust's compiler uses arenas heavily internally. Salsa (our incremental framework) plays well with arena patterns.

**Adding arenas to the Yinz compiler internals is a v0.1 M8 polish task** — likely 2-5x compile-speed improvement for typical projects. Documented in `.claude/todos.md` after this plan lands.

This is internal compiler work; it has nothing to do with the user-facing arena syntax above. The user-facing syntax is v0.2.

---

## Kernel-mode compatibility

In `--kernel` mode (see [`no-runtime-mode.md`](no-runtime-mode.md)), the default allocator doesn't exist. `arena {}` blocks still WORK — they need a base allocator to get their buffer from, which the user injects via `runtime.setAllocator(myKernelAllocator)`. Once that's set, `arena {}` blocks function normally.

This is one of the kernel-mode wins: the bump-pointer allocator pattern is perfect for kernels (deterministic, fast, no fragmentation). Arenas work AS WELL in kernel mode as in regular mode — no kernel-specific limitations.

---

## v0.2 Implementation notes

- Arena lifetime: scoped to the block. Compiler generates "create arena" at block entry and "wipe arena" at block exit (including all panic-unwind paths).
- The arena's underlying buffer is itself heap-allocated via the default allocator (or user-provided in kernel mode). The arena trades a single big allocation for many small ones.
- Buffer sizing: starts at some default (e.g., 4KB). On overflow, allocates an additional chunk via the default allocator. Multiple chunks all wipe together at scope end.
- Borrow checking: values allocated in the arena cannot escape the arena's scope. Compile error if you try to return one or store it somewhere outliving the arena.
- Compiler should be able to do escape analysis to PROVE values don't escape, allowing the user to forget about it most of the time.

The v0.2 milestone plan must address: thread safety (arenas are typically single-threaded for performance; what's the cross-thread story?), error messages for escape violations, and the IDE hint protocol for muted `.in(arena)` annotations.

---

## v0.3 Implementation notes (Option B)

If/when Option B is implemented:

- Add `Arena()` constructor in stdlib
- Add `.reset()` method
- Implement the lifecycle/borrow checker analysis
- Compile error if Arena is dropped without `.reset()` (or with allocations still live)
- IDE hints for arena ownership patterns

---

## Cross-references

- [`design/future/no-runtime-mode.md`](no-runtime-mode.md) (arenas work in kernel mode if user provides base allocator)
- [`design/ide-hints.md`](../ide-hints.md) (muted `.in(arena)` rendering protocol)
- [`design/ownership.md`](../ownership.md) (escape-analysis rules apply to arena-allocated values)
- `.claude/todos.md` (Yinz compiler internals should use arenas — M8 polish task)
- [`.claude/plans/active/design-lockdown-from-gemini-review.md`](../../.claude/plans/active/design-lockdown-from-gemini-review.md) (originating decision)
