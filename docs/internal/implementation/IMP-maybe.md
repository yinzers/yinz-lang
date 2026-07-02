---
name: "IMP-maybe"
description: "Design decisions for Yinz's 'maybe<T>' built-in generic primitive — its API (.exists(), .value, .or()) and why it shipped in M5 with generics instead of M6 with narrowing."
tags:
  - "yinz-compiler"
created_at: "2026-05-17"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Maybe — Design Decisions

User spec: [`docs/reference/REF-maybe.md`](../../reference/REF-maybe.md)

---

## What `maybe<T>` is

`maybe<T>` is a built-in generic primitive shipped in M5 alongside the generics engine. It expresses "a value of type T that might not exist." Operations:

- `none` — the absent value (built-in keyword)
- `m.exists() -> boolean` — method (parens — action)
- `m.value` — virtual field (no parens — access; flow-sensitive proof of `.exists()` required)
- `m.or(default: T) -> T` — method (parens — action)

See [`docs/reference/REF-maybe.md`](../../reference/REF-maybe.md) for the user-facing surface.

---

## Why `maybe<T>` ships in M5, not M6

The earlier milestone plan put `maybe T` in M6 alongside `options`, union types, and `is Type` narrowing — all "things that affect type narrowing." But `maybe<T>` is structurally different: it's the return type of `.get()` on every built-in collection. Without `maybe<T>` in M5, `array<int>.get(0)` either has to return `int` (with a runtime panic on out-of-bounds — wrong shape, eventually requires a rename) or `.get()` ships in M6 (M5 collections would have no safe access until M6 lands).

Pulling `maybe<T>` into M5 produces the cleanest API: `.get()` returns `maybe<T>` from day 1; no rename, no breaking change. M6 still ships unions / options / narrowing — but `maybe<T>` is structurally a primitive of the type system, not a narrowing case.

Locked 2026-05-17 in [`.claude/planning/done/2026-05-17-m5-generics/plan.md`](../../../.claude/planning/done/2026-05-17-m5-generics/plan.md).

---

## LLVM Lowering Decision Table

The lowering is chosen per concrete T at monomorphization time. The decision is mechanical from T's storage class; no heuristics, no per-binding override.

| Concrete T | Encoding | LLVM type | Why |
|---|---|---|---|
| `int`, `float`, `number`, `boolean` (primitives) | Tagged union | `struct { i1 has_value, T value }` | Fits in 2 words; tag is cheaper than reserving a sentinel value of T |
| Heap-allocated shape pointer (`ynz_alloc`-backed shape) | Null-pointer | `T*` (null = none) | One word; no tag byte; matches Rust's `Option<Box<T>>` optimization |
| `fixed<U, N>` (stack-allocated array) | Tagged union | `struct { i1 has_value, [N x U] value }` | The whole array is inline; null-pointer encoding doesn't apply |
| `array<U>` (heap array header) | Null-pointer | `*Array<U>` (null = none) | Pointer = discriminator |
| `map<K, V>` (heap map header) | Null-pointer | `*Map<K, V>` | Same as `array<U>` |
| `Pair<A, B>` and other generic shapes with no pointer field | Tagged union | `struct { i1 has_value, Pair<A, B> value }` | No pointer to repurpose; explicit tag required |
| `dynamic Foo` (fat pointer: data + vtable) | Null-pointer on DATA slot | `struct { *T data, *Vtable vt }` (data == null → none) | Data-pointer is the natural sentinel; vtable ignored when none |
| `string` (heap-allocated UTF-8 bytes) | Null-pointer | `*String` (null = none) | Same as heap shape pointer |
| `maybe<maybe<T>>` (nested) | **REJECTED at typeck** — compile error | — | Nested maybe forces a 2-bit tag; almost always a code smell. Compile error suggests flattening. |
| All other generic-shape instantiations | Tagged union | `struct { i1 has_value, ShapeT value }` | Default — safe but slightly more memory than null-pointer when applicable |

**Why no per-binding override**: the encoding is implementation-detail; users see only `maybe<T>` and `none`. Adding a `dense<T>` / `pointer-niche<T>` opt-in surface is duct tape — the compiler picks the right one automatically.

**Why reject `maybe<maybe<T>>`**: allowing it forces a 2-bit tag (some-some / some-none / none); that distinction is almost never what the user meant. M5 ships the compile error; if a real use case emerges, this section gets a v0.2+ amendment.

IR-snapshot tests in `crates/ynz-codegen/tests/snapshots.rs` assert each row of the table produces the expected LLVM type.

---

## `none` type inference rules

The `none` literal produces a type `maybe<T>` for some unknown T. T is resolved from context:

| Context | T resolution |
|---|---|
| `let x: maybe<U> = none` | T = U (from binding annotation) |
| `function foo() -> maybe<U> { return none }` | T = U (from return type) |
| `arr.add(none)` where `arr: array<maybe<U>>` | T = U (from parameter type) |
| `if (cond) { some_value } else { none }` | T = U from sibling branch type |
| `let m: map<string, maybe<U>> = { "a": some(5), "b": none }` | T = U (from map value-type annotation) |
| `let x = none` (no annotation, no enclosing call/return) | **Compile error** — "Cannot work out which type `none` should be here. Annotate the binding." |
| `identity(none)` (single-arg generic call, no other parameters constrain T) | **Compile error** — "Cannot work out the type parameter T from a `none` argument alone." |
| `pair(none, 5)` (multi-arg generic call, one arg constrains T) | T = int for first param if the parameter declares `maybe<A>`; otherwise compile error per slot |
| `foo(none)` where `foo(give x: maybe<U>) -> ...` (non-generic) | T = U |

**General rule**: `none` resolves T by walking up the AST one node at a time, looking for a context that types it as `maybe<U>` for a concrete U. The walk terminates at: (a) immediate type annotation, (b) enclosing call's parameter type, (c) enclosing return type, (d) sibling branch with `maybe<U>` type. If the walk exhausts these without finding a U, compile error with a suggested-annotation diagnostic.

---

## Flow-sensitive `.value` enforcement rules

`.value` on a `maybe<T>` binding requires compile-time proof that the value is not none. M5 ships a NARROW subset of flow narrowing (full narrowing including negation/short-circuit/early-return is M6's `if (x is Type)` work).

| Form | `.value` permitted? | Notes |
|---|---|---|
| `if (m.exists()) { m.value }` | YES — flag set on `m` inside then-block | The bread-and-butter case. |
| `if (!m.exists()) { ... } else { m.value }` | YES — flag set on `m` inside else-block | Symmetric to positive form. |
| `if (m.exists() && other) { m.value }` | YES — short-circuit AND propagates the flag | Both RHS and body see `m` narrowed. |
| `if (other && m.exists()) { m.value }` | YES — same | Order doesn't matter for AND. |
| `if (m.exists() \|\| other) { m.value }` | NO — OR doesn't guarantee `m` narrows | Compile error suggests `.or(default)`. |
| `if (a.exists() && b.exists()) { b.value }` | YES — independent flags per binding | Independent maybes each get their own flag. |
| `if (m.exists()) { return ... } m.value` | NO — early-return narrowing is M6 | Teaching error points to M6. Workaround: `if (m.exists()) { use(m.value) } else { return ... }`. |
| `for (i in range(3)) { if (m.exists()) { m.value } }` | YES — flag scoped to if-block | Loop doesn't change `m`. |
| `if (m.exists()) { m = newMaybe(); m.value }` | NO — reassignment invalidates flag | Compile error citing the offending reassign site. |
| `if (m.exists()) { closureCapture(() => m.value) }` | NO — closures don't carry the flag | Forward-defensive; closures are v0.3+ anyway. |
| `m.value` with no surrounding `.exists()` check | NO — compile error | Base case. Suggests both `.exists()` check and `.or(default)`. |

**Why only this subset**: the simple positive/negative/AND cases cover 95% of real usage. Early-return narrowing requires negation-narrowing infrastructure that M6 owns. Reassignment invalidation is mechanical. Closures are deferred.

---

## Documented v0.1 limitation: cycle leak through `maybe<Self>` mutation

`shape Node<T> { value: T, next: maybe<Node<T>> }` permits cycle creation through field mutation:

```ynz
let n1: Node<int> = { value: 1, next: none }
let n2: Node<int> = { value: 2, next: none }
n1.next = some(n2)   // (annotation-driven literal; some() is the constructor for maybe<T>)
n2.next = some(n1)   // CYCLE — both nodes leak on scope exit
```

The v0.1 borrow checker does NOT detect reference cycles. The user gets a memory leak (`ynz_free` never reaches the cycled nodes). This is **intentional v0.1 behavior**, not a bug:

- **What**: cycles through `maybe<Self>` field mutation leak both endpoints
- **Why**: cycle-detection requires global reference-graph analysis, which is borrow-checker work scoped for v0.2+ when the LSP enables interactive cycle visualization
- **Cost to fix later**: 1-2 sessions of borrow-checker work + IDE wiring — not blocking for v0.1's "compiler works end-to-end" goal
- **Trigger to revisit**: when v0.2 LSP work begins, OR when a real workload trips a production leak

Documented in fixture `crates/ynz-driver/tests/fixtures/m5_cycle_leak.ynz` with a comment citing this section.

---

## Why `maybe<T>` is built-in, not a stdlib generic

`maybe<T>` is the return type of `.get()` on every built-in collection: `array<T>`, `fixed<T>`, `map<K, V>`, `string`. It's also the signature of `.first()`, `.last()`, `.find()`, `.removeFirst()`, `.removeLast()`, and similar collection methods. The type appears EVERYWHERE in the language surface.

If `maybe<T>` were a stdlib generic (defined in `std/maybe.ynz` and imported), every program would need an implicit prelude that imports it. M5 prefers the simpler model: `maybe<T>` is a primitive of the type system, defined inside the compiler, with no import needed. This matches `array<T>` / `fixed<T>` / `map<K, V>` (also primitives, also no-import).

`some(value)` constructor: `some(value)` is sugar for "wrap this value as a `maybe<T>`." Treated like a built-in function but resolved at typeck (no actual runtime call — codegen lowers `some(x)` to the tagged-union construction or pointer-wrap directly).

---

## Cross-References

- [`docs/reference/REF-maybe.md`](../../reference/REF-maybe.md) — user-facing surface
- [`.claude/planning/done/2026-05-17-m5-generics/plan.md`](../../../.claude/planning/done/2026-05-17-m5-generics/plan.md) — M5 implementation plan (where all of the above tables were originally locked)
- [`docs/internal/implementation/IMP-generics.md`](IMP-generics.md) — generics engine that `maybe<T>` is built on top of
- [`docs/internal/implementation/IMP-collections.md`](IMP-collections.md) — collections whose `.get()` returns `maybe<T>`
- [`docs/reference/REF-golden-rules.md`](../../reference/REF-golden-rules.md) — Rule 5 (compile-time safety) governs the flow-sensitive `.value` check
- [`.claude/rules/dot-postfix.md`](../../../.claude/rules/dot-postfix.md) — `.value` is access (no parens), `.exists()` and `.or()` are actions (parens)
- [`.claude/rules/vocabulary.md`](../../../.claude/rules/vocabulary.md) — `maybe T` (user) and `maybe<T>` (since M5 syntax-lock) terminology
