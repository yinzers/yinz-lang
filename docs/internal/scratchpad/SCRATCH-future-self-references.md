---
name: "SCRATCH-future-self-references"
description: "Design notes locking how Yinz will support self-referential shapes safely by default via relative/offset pointers (Approach A), targeted for v0.3+."
tags:
  - "yinz-compiler"
created_at: "2026-05-14"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Self-Referential Shapes — Relative Pointers (Approach A)

**Status**: Locked — Approach A (relative/offset pointers). Patrick confirmed during the design-lockdown conversation (2026-05-14, round 3 of the plan).

**Target**: v0.3+ implementation. Specific milestone TBD when v0.2 ships.

User spec target: [`docs/reference/REF-types.md`](../../reference/REF-types.md) (gets a "self-referential shapes" section when implemented).

---

## The Decision

Shapes can have internal references — fields pointing into other fields of the same shape — and they are SAFE BY DEFAULT in Yinz. The compiler detects self-references automatically, applies **Approach A** (relative/offset pointers), and renders the affected shape as `self-referential` in the IDE as a muted hint.

Per the [uniform inference rule](../../../.claude/rules/inference.md), the developer doesn't have to write `self-referential` — the compiler figures it out from field types. The IDE shows what was figured out.

This is genuinely better than Rust. Rust requires `Pin<T>` (Approach C in this doc's terminology) because they're locked into bitwise-memcpy moves; Yinz isn't.

---

## What "self-referential" means

A shape is self-referential when one of its fields holds a pointer or reference that points INTO another field of the same shape instance. Classic example: a node that holds data and a pointer into its own data buffer.

```ynz
shape Node {                         // muted ".self-referential" appears here in IDE
  data: int
  ref: pointer-to-self.data          // hypothetical syntax — exact form TBD in v0.3 spec
}
```

(The exact source syntax for declaring the self-reference is a v0.3 spec decision; this design doc establishes the SEMANTICS the compiler must enforce, not the surface syntax.)

---

## Why this is dangerous WITHOUT special handling

When a value MOVES (e.g., returned from a function, stored in an array that reallocates), the compiler typically does a bitwise-memcpy:

```
Before move:  [data at 0x200] [ref at 0x208 holding value "0x200"]
After move:   [data at 0x500] [ref at 0x508 holding value "0x200"]   ← STALE
                                                       ^^^^^^
                                            still pointing to the OLD location
                                            which may now be free or reused
```

Result: dangling pointer → use-after-free → memory corruption → security vulnerability or crash.

This is why Rust uses `Pin<T>` — pin the value to one heap location so it can't move. Awkward but safe.

---

## Approach A — Relative Pointers (Yinz's choice)

Store the internal reference as a **byte offset from the shape's base address**, not as an absolute pointer. When the shape moves, the offset stays valid because it's relative.

```
Before move:  [data at base+0]  [ref at base+8 holding offset value "+0"]
After move:   [data at NEW+0]   [ref at NEW+8 holding offset value "+0"]
                                                       ^^
                                          offset is still correct because
                                          it's relative to base, not absolute
```

Access cost: one ADD instruction (base + offset) instead of one direct dereference. ~1 CPU cycle. Negligible in any real workload.

**Approach A is exactly what Zig allocator-relative containers and the rkyv crate do.** It's a known-good technique, not Yinz inventing something novel — Yinz just makes it the language-level default for self-referential shapes.

---

## Rejected alternatives (kept for institutional memory)

### Approach B — Fix-up on move

When the compiler generates a move, it also generates code that walks through the shape's fields and updates internal pointers to the new location.

**Why rejected**:
- Complex move semantics throughout the language — every move becomes a multi-step operation
- Bugs in fix-up code cause memory corruption (the worst kind of bug)
- Larger code size for every move (even moves of non-self-referential shapes pay the cost in code-bloat terms)
- The optimizer can't always eliminate the fix-up overhead

### Approach C — Pin in place (Rust's solution)

Allocate the shape on the heap. Pin it there. Never move it. Internal pointers are absolute but the shape's location is permanent.

**Why rejected**:
- Forces heap allocation — conflicts with Yinz's stack-by-default preference
- Cannot put self-referential shapes in arrays (arrays may reallocate, which moves their elements)
- Adds API constraints — the type can't be passed by value, can't be returned from a function, can't be moved anywhere
- This is Rust's `Pin` and it's the single biggest source of `Pin`-related friction in Rust development

---

## Compiler-detected vs explicit declaration

The compiler can detect self-references automatically by looking at field types and inferred lifetimes. The IDE shows the inferred `self-referential` modifier as a muted hint on the shape declaration.

The developer can type `self-referential` explicitly to document intent (per the [uniform inference rule](../../../.claude/rules/inference.md) — every inferred attribute is also typeable):

```ynz
shape Node self-referential {        // explicit — same behavior as inferred
  data: int
  ref: pointer-to-self.data
}
```

Click-to-make-explicit on the muted hint produces this exact syntax. Removing the explicit keyword and letting the compiler re-infer produces identical compiled output.

---

## When does the cost actually apply

Only shapes that ARE self-referential pay the 1-cycle-per-access cost. Non-self-referential shapes (the vast majority — 99%+ of types in real codebases) use normal absolute pointers and pay zero overhead.

The compiler tracks which fields of which shapes are internal-references. For other fields, it emits direct memory access. The cost is opt-in by virtue of declaring the self-referential field — there's no global toggle, no per-program performance tax.

---

## What this enables

Patterns that are awkward or impossible in Rust become natural in Yinz:

- **Intrusive linked lists** — a list node embeds the next-pointer directly, no separate Box allocation per node
- **Cyclic graphs with internal pointers** — graph node owning its edges, edges pointing to other nodes by reference
- **Self-referential parser/lexer state** — a parser holding pointers into its own input buffer
- **Custom data structures** — anything where logical structure mirrors physical layout

The traditional Yinz pattern (index-into-an-array) is still preferred for most cases because it's simpler and indices don't get invalidated by data movement. But for the cases where direct references are the right model, Yinz makes them safe instead of awkward.

---

## v0.3+ Implementation notes

- The compiler MUST detect self-references via type-system analysis (not source-level pattern matching). New ways to express references will work automatically.
- Move codegen must emit base-relative offset computation for self-referential fields.
- The borrow checker must verify that internal references can't outlive their shape — same lifetime rules as normal references, but anchored to the shape's identity.
- IDE support: the muted `self-referential` hint requires the LSP to expose shape-level inference, which is more granular than current shape-attribute hints. v0.3 LSP work.

The v0.3 milestone plan must include:
- Exact source syntax for declaring a self-referential field
- Borrow checker rules for internal references
- LLVM codegen patterns
- IDE hint integration

---

## Cross-references

- [`docs/internal/implementation/IMP-ownership.md`](../implementation/IMP-ownership.md) (general ownership semantics — self-references extend these)
- [`docs/reference/REF-ide-hints.md`](../../reference/REF-ide-hints.md) (muted `self-referential` hint protocol)
- [`docs/internal/scratchpad/SCRATCH-future-designs-index.md`](SCRATCH-future-designs-index.md) (status: locked, v0.3+)
- [`.claude/planning/done/2026-05-14-design-lockdown-from-gemini-review/plan.md`](../../../.claude/planning/done/2026-05-14-design-lockdown-from-gemini-review/plan.md) (originating discussion + Patrick's Approach A confirmation)
