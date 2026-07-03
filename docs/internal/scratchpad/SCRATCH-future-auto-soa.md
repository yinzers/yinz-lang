---
name: "SCRATCH-future-auto-soa"
description: "Future-milestone design notes for auto-transforming array<Shape> from Array-of-Structs to Struct-of-Arrays layout in hot field-access loops, transparent to the user-facing API."
tags:
  - "yinz-compiler"
created_at: "2026-05-15"
updated_at: "2026-07-03"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Auto-SoA Transformation for Hot Field-Access Loops

**Status**: Locked (commitment), v0.3+ implementation.

> **Owning plan (2026-07-03): `2026-07-03-v0-3-m5-auto-soa`** (v0.3-M5 — split from M4 2026-07-02; the
> `v0-3-m3c-array-by-value` by-value element storage folded in 2026-07-03, so ONE elem_size-aware array
> representation carries both by-value AoS and SoA-as-layout-variant). That plan is authoritative where it
> sharpens or diverges from this doc (its Design-Doc Alignment section enumerates the divergences — e.g.
> "codegen-only" is corrected: SoA needs the new array representation). Full graduation/trim of this doc
> into `IMP-collections.md` happens in that plan's Phase 7 step 5 — do NOT trim earlier.

User spec target: none — this is an internal compiler optimization. The user-facing surface stays identical (`array<Player>`, `arr[i].health`); only the memory layout differs.

---

## The Decision

When the compiler can prove a hot loop over `array<Shape>` accesses only a small subset of fields (typically 1-2), it transforms the layout from Array-of-Structs (AoS) to Struct-of-Arrays (SoA) transparently. The shape's external API is unchanged; only the underlying memory layout differs.

This is genuinely novel — no mainstream language attempts this transformation automatically. Yinz can do it because the ownership system already proves field aliasing statically; the missing piece is the optimization pass that decides when to apply it.

---

## The Problem (and why other languages can't easily solve it)

Object-oriented languages model entities as objects:

```
shape Player {
  x: float
  y: float
  health: int
  name: string
}

let players: array<Player> = ...
```

The default memory layout is AoS:

```
[x0, y0, health0, name0, x1, y1, health1, name1, ...]
```

A loop that processes only `x` values (e.g., physics):

```
for (player in players) {
  player.x = player.x + velocity.x
}
```

...has to load entire cache lines containing `y, health, name` just to get `x`. SIMD vectorization is impossible because the `x` values are 32+ bytes apart in memory. Real-world cost: 10-40× slower than the same operation on a `float[]` array.

The fix is SoA:

```
[x0, x1, x2, ..., xN]   ← x array
[y0, y1, y2, ..., yN]   ← y array
[health0, health1, ...]
[name0, name1, ...]
```

Loop processing only `x` reads contiguous memory, fits in cache, vectorizes cleanly. But existing languages either (a) make the user write SoA manually (ugly, error-prone) or (b) provide a library type like Rust's `soa-derive` crate (opt-in, learned-not-default).

Yinz's ownership system is the missing ingredient — the compiler already proves which fields a function reads vs writes via `share`/`lend`/`give` annotations, and the `noalias` LLVM attribute is already emitted for ownership-tracked references. That metadata is exactly what's needed to prove a SoA transform doesn't break aliasing.

---

## The Transform — User Code Stays the Same

Critically: **the user writes the same Yinz code**. The shape declaration, the array creation, the field access syntax — all unchanged. The compiler picks the layout per-array based on access patterns.

```
shape Player { x: float; y: float; health: int; name: string }

let players: array<Player> = ...

for (player in players) {
  player.x = player.x + 0.1   // hot loop, accesses only x
}
// Compiler emits SoA layout for this array's storage.
// IDE shows muted hint on the array declaration:
//   // SoA layout: hot loop accesses only `x` — saved 24 bytes/entry on cache reads
```

Other code accessing the same array gets transparent fan-out. Reading `player.name` from a SoA-laid-out array involves one extra pointer dereference (the `name[]` array is in a different region) — slightly slower than AoS for that field, but the gain on the hot field's loop dwarfs the cold-field cost.

---

## When the Compiler Applies It

The transform is profitable only when:

1. **The array is large** (> ~64 entries). Small arrays don't benefit; the SoA setup overhead outweighs the cache wins.
2. **A loop accesses ≤ 2 fields per iteration** (the most common SIMD-amenable shape). Loops accessing all fields don't benefit — AoS is already optimal for "touch the whole struct."
3. **The loop runs many iterations** (the body is hot — detected via static analysis or PGO if PGO is later added).
4. **No external code requires the AoS layout** (no FFI export, no serialization that depends on byte layout — the serialization codegen per [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) Rule 6 produces wire format independent of internal layout, so this is usually automatic).

When all four hold, SoA is emitted. When any fails, AoS is the default. The compiler picks per-array, not globally — different arrays in the same program can have different layouts.

---

## Why This Is a Yinz Differentiator

Manual SoA is the standard advice in performance-sensitive C++/Rust circles ("data-oriented design," EnTT/flecs ECS frameworks, etc.). The complaint is always: it's ugly, requires rewriting your data structures, and loses the OOP-style readability of `player.x` syntax. Some teams adopt it anyway and pay the readability cost; most don't and leave the perf on the table.

Yinz is the first language where the compiler can deliver SoA's perf wins without forcing the user to rewrite. The ownership system pays for the analysis; the user gets the speedup transparently.

ECS frameworks become a stdlib pattern, not a third-party rewrite. Game engines, physics simulations, data pipelines — all the workloads that justify SoA in C++ become natural in Yinz.

---

## Why v0.3 (Not v0.1 or v0.2)

v0.1 ships the basic `shape`/`array` infrastructure. v0.2 ships LSP + watch + fmt. The SoA pass is a CODEGEN optimization that requires:

- The ownership system to be working (M4, v0.1)
- The `array<T>` implementation to be stable (v0.1)
- The compiler IR to be mature enough to support layout transformations (v0.1+ baseline)
- An optimization-pass framework that can decide per-array (v0.2 LSP work surfaces some of this)
- A way to express the transformed layout in LLVM IR while keeping debugger-visible field semantics (v0.3 work)

v0.3 is the auto-parallelization milestone — independent operations get scheduled concurrently. SoA fits the same milestone family: both are "compiler does ambitious cross-cutting analysis to make naive code fast." Bundling them keeps the optimization-team work in one milestone.

Engineering estimate: 4-8 weeks of focused compiler engineering once v0.1 ships. Not v0.1 territory; not "5 years from now" either. v0.3 is the right slot.

---

## Compile-Time Cost

The SoA analysis pass scales linearly with code size. Realistic estimate: +3-7% on `ynz build --release` for code bases with many `array<shape>` hot loops. Negligible on `ynz build` (debug) because the optimization pass is skipped in debug builds (debug-vs-release cost separation per [`docs/internal/implementation/IMP-compiler.md`](../implementation/IMP-compiler.md)).

This is bounded — the analysis doesn't have exponential or quadratic blowup. Comparable to other release-mode optimization passes (escape analysis, bounds-check elimination).

---

## IDE Teaching Surface

The IDE surface for auto-SoA is **NOT a muted-hint inference** (the protocol in [`.claude/rules/inference.md`](../../../.claude/rules/inference.md) requires every muted hint to complete to typeable Yinz syntax — SoA has no opt-in syntax for the user to type, so the muted-hint protocol doesn't apply).

Instead, auto-SoA uses the same hybrid model as `array<T>` → `fixed<T>` promotion ([`docs/internal/implementation/IMP-collections.md`](../implementation/IMP-collections.md)): silent codegen + Tier 3 lint suggestion. The transform happens; the IDE surfaces a Tier 3 suggestion (yellow squiggle) on the array declaration that explains the optimization in hover.

```
let players: array<Player> = ...   // yellow squiggle
                                    // (compiler already emitted SoA codegen)
```

Hover tooltip on the squiggle:
- **WHAT**: This `array<Player>` is stored Struct-of-Arrays internally (separate arrays per field) because the loop on lines X-Y accesses only `player.x` and `player.velocity.x`. Same external behavior — `players[i].x` works the same — but the layout is now SIMD-friendly.
- **WHAT INSTEAD**: There is no source-level opt-in or opt-out syntax for SoA in v0.3. The compiler picks per-array based on access patterns. If you need a specific layout for FFI or a wire format, FFI bindings (v2+) and the serializer codegen (per [`docs/internal/implementation/IMP-collections.md`](../implementation/IMP-collections.md) "Auto-promotion" section) handle the conversion at the boundary, not at the source.
- **WHY**: For hot loops over large `array<shape>` collections, SoA enables SIMD vectorization and reduces cache miss count. Cold field accesses (`player.name`, `player.health`) get a single extra dereference — slightly slower but dwarfed by the hot-loop win. The compiler picks SoA only when the cost analysis favors it.

This is consistent with the array→fixed hybrid model — auto-codegen + visible diagnostic — but uses Tier 3 (suggestion) styling rather than muted-hint styling because there's no equivalent typeable source form. For v0.3 implementation, the Tier 3 lint rule is something like `array-using-soa-layout` (final naming TBD).

If it turns out users want explicit control for edge cases (testing, embedded), v0.3 implementation can add an opt-in modifier syntax (e.g., `soa array<Player>` or `array<Player>(layout: .soa)`) — but the default model is "compiler picks, lint suggestion explains." Same as the array→fixed precedent.

---

## Forward-Compatibility for FFI (v2+)

When FFI lands in v2+ and a user wants to pass a Yinz `shape` to a C function expecting a specific layout, the FFI binding converts AoS↔SoA at the boundary. The Yinz array can stay in whatever internal layout the compiler picked; the FFI binding allocates a temporary AoS buffer for the C call and reads back any mutations.

This means SoA does NOT need to be opt-out-able from the shape declaration. The FFI side handles layout conversion; the Yinz side stays in its optimized layout.

---

## Open Questions for v0.3 Implementation

These get resolved in the v0.3 milestone plan, not now:

- **Threshold tuning**: at what array size does SoA start winning? Probably ~64 entries but should be measured.
- **Cross-function analysis**: if a function takes `share array<Player>` and accesses only `.x`, does the SoA decision propagate to all callers, or per-call-site?
- **Mixed-access loops**: loop accesses `.x` in line 1 and `.name` in line 2. Worth SoA or not? Heuristic TBD.
- **Stable iteration order**: does SoA break iteration order guarantees? (Should not — `map` iteration order is locked, `array` iteration is sequential by index.)
- **Debug experience**: lldb/gdb integration — when the user inspects `players[i]`, does the debugger show the unified Player view (helpful) or the raw SoA arrays (confusing)? Needs DAP work.

These are implementation-time decisions, not design-time blockers. The COMMITMENT to ship auto-SoA is locked here NOW; the implementation details get figured out at v0.3 plan time.

---

## Cross-References

- [`docs/reference/REF-golden-rules.md`](../../reference/REF-golden-rules.md) Rule 4 (compiler does the hard work)
- [`docs/reference/REF-golden-rules.md`](../../reference/REF-golden-rules.md) Rule 8 (zero-cost abstractions — high-level syntax compiles to optimal layout)
- [`docs/reference/REF-golden-rules.md`](../../reference/REF-golden-rules.md) Rule 10 (efficiency first — fast layout is the default, no opt-in needed)
- [`docs/reference/REF-golden-rules.md`](../../reference/REF-golden-rules.md) Rule 11 (compiler is teacher — IDE hint explains the transform)
- [`docs/internal/implementation/IMP-ownership.md`](../implementation/IMP-ownership.md) (the aliasing proof system that makes auto-SoA possible)
- [`docs/internal/implementation/IMP-concurrency.md`](../implementation/IMP-concurrency.md) (auto-parallelization — same v0.3 milestone family)
- [`docs/internal/implementation/IMP-collections.md`](../implementation/IMP-collections.md) (`array<T>` and `shape` semantics this builds on)
- [`.claude/rules/inference.md`](../../../.claude/rules/inference.md) (the muted-hint protocol the IDE surface uses)
- `lockin-cpu-bigo.md` Finding #6 (the AoS vs SoA perf gap research)
