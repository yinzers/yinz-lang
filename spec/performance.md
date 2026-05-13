# Performance

Yinz is fast. Here's why.

---

## No interpreter, no JIT

Yinz compiles ahead of time to native machine code via LLVM. The compiler has unlimited time to optimize — there's no overhead from an interpreter or just-in-time compiler running alongside your program.

---

## No garbage collector

Memory is freed when the owner goes out of scope. Ownership is tracked at compile time — no background tracking, no pause-and-scan, no 2-3x memory overhead from a runtime GC. Memory management is deterministic and free.

---

## Flat memory layout

When you define a `type`, fields are laid out contiguous in memory. Nested types are embedded inline — not scattered as pointers to other parts of the heap:

```
type Position { x: number, y: number }
type Player { name: string, health: number, position: Position }
```

`position` lives directly inside `Player` — not a pointer to somewhere else. When you access `player.position.x`, the CPU finds it in the same cache line. On data-heavy workloads, this is 10-50x faster than pointer-chasing.

---

## Efficiency first by default

`fixed<T>` arrays are stack-allocated and size-locked. Stack allocation is faster than heap allocation. The default path — the one you reach without thinking — is the fast path.

Typed objects (`type Player`) are faster than maps (`map<string, number>`) because fields are at fixed offsets — direct access. Maps use hash tables — slower.

A developer who never thinks about performance automatically writes fast code.

---

## Compiler operation fusion

Step-by-step operations on collections get fused by the compiler into a single optimized pass:

```
let active = players.filter(p => p.health > 0)   // step 1
let ranked = active.sort(p => p.health, desc)    // step 2
let top = ranked.limit(10)                       // step 3
let names = top.map(p => p.name)                 // step 4
```

The compiler sees that each intermediate result is only used by the next step and fuses all four into a single loop with zero intermediate allocations. You write readable steps; the compiler writes fast code.

---

## What you get for free

The compiler handles all of this automatically — you don't have to think about it:

- Type inference (you skip obvious annotations; the compiler fills them in)
- Operation fusion (sequential collection operations become single passes)
- Memory layout (nested types embedded inline, not as pointers)
- Allocation strategy (stack when possible, heap only when needed)
- Ownership tracking (memory freed exactly when it's done, no GC needed)
