---
name: "IMP-maybe"
description: "Design decisions for Yinz's 'Maybe<T>' built-in generic primitive — its API (.exists(), .value, .or()), its lowering table and its flow-sensitive '.value' rules."
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

## What `Maybe<T>` is

`Maybe<T>` is a built-in generic primitive shipped in M5 alongside the generics engine. It expresses "a value of type T that might not exist." Operations:

- `none` — the absent value (built-in keyword)
- `m.exists() -> Boolean` — method (parens — action)
- `m.value` — virtual field (no parens — access; flow-sensitive proof of `.exists()` required)
- `m.or(default: T) -> T` — method (parens — action)

See [`docs/reference/REF-maybe.md`](../../reference/REF-maybe.md) for the user-facing surface.

---

## `Maybe<T>` is a type-system primitive

`Maybe<T>` is the return type of `.get()` on every built-in collection, so it belongs to the generics engine and not to the narrowing features (unions, `options`, `is Type`). `Array<Int>.get(0)` returns `Maybe<Int>`, and an out-of-bounds index gives `none`, not a runtime panic.

---

## LLVM Lowering Decision Table

The lowering is chosen per concrete T at monomorphization time. The decision is mechanical from T's storage class; no heuristics, no per-binding override.

| Concrete T | Encoding | LLVM type | Why |
|---|---|---|---|
| `Int`, `Float`, `Number`, `Boolean` (primitives) | Tagged union | `struct { i1 has_value, T value }` | Fits in 2 words; tag is cheaper than reserving a sentinel value of T |
| Heap-allocated shape pointer (`ynz_alloc`-backed shape) | Null-pointer | `T*` (null = none) | One word; no tag byte; matches Rust's `Option<Box<T>>` optimization |
| `Fixed<U, N>` (stack-allocated array) | Tagged union | `struct { i1 has_value, [N x U] value }` | The whole array is inline; null-pointer encoding doesn't apply |
| `Array<U>` (heap array header) | Null-pointer | `*Array<U>` (null = none) | Pointer = discriminator |
| `Map<K, V>` (heap map header) | Null-pointer | `*Map<K, V>` | Same as `Array<U>` |
| `Pair<A, B>` and other generic shapes with no pointer field | Tagged union | `struct { i1 has_value, Pair<A, B> value }` | No pointer to repurpose; explicit tag required |
| `dynamic Foo` (fat pointer: data + vtable) | Null-pointer on DATA slot | `struct { *T data, *Vtable vt }` (data == null → none) | Data-pointer is the natural sentinel; vtable ignored when none |
| `String` (heap-allocated UTF-8 bytes) | Null-pointer | `*String` (null = none) | Same as heap shape pointer |
| `Maybe<Maybe<T>>` (nested) | **REJECTED at typeck** — compile error | — | Nested maybe forces a 2-bit tag; almost always a code smell. Compile error suggests flattening. |
| All other generic-shape instantiations | Tagged union | `struct { i1 has_value, ShapeT value }` | Default — safe but slightly more memory than null-pointer when applicable |

**No per-binding override**: the encoding is an implementation detail; users see only `Maybe<T>` and `none`, and the compiler picks the encoding from the table.

**`Maybe<Maybe<T>>` is a compile error**: it would need a 2-bit tag (some-some / some-none / none), a distinction a program almost never means.

IR-snapshot tests in `crates/ynz-codegen/tests/snapshots.rs` assert each row of the table produces the expected LLVM type.

---

## `none` type inference rules

The `none` literal produces a type `Maybe<T>` for some unknown T. T is resolved from context:

| Context | T resolution |
|---|---|
| `let x: Maybe<U> = none` | T = U (from binding annotation) |
| `function foo() -> Maybe<U> { return none }` | T = U (from return type) |
| `arr.add(none)` where `arr: Array<Maybe<U>>` | T = U (from parameter type) |
| `if (cond) { some_value } else { none }` | T = U from sibling branch type |
| `let m: Map<String, Maybe<U>> = { "a": some(5), "b": none }` | T = U (from map value-type annotation) |
| `let x = none` (no annotation, no enclosing call/return) | **Compile error** — "Cannot work out which type `none` should be here. Annotate the binding." |
| `identity(none)` (single-arg generic call, no other parameters constrain T) | **Compile error** — "Cannot work out the type parameter T from a `none` argument alone." |
| `pair(none, 5)` (multi-arg generic call, one arg constrains T) | T = Int for first param if the parameter declares `Maybe<A>`; otherwise compile error per slot |
| `foo(none)` where `foo(give x: Maybe<U>) -> ...` (non-generic) | T = U |

**General rule**: `none` resolves T by walking up the AST one node at a time, looking for a context that types it as `Maybe<U>` for a concrete U. The walk terminates at: (a) immediate type annotation, (b) enclosing call's parameter type, (c) enclosing return type, (d) sibling branch with `Maybe<U>` type. If the walk exhausts these without finding a U, compile error with a suggested-annotation diagnostic.

---

## Flow-sensitive `.value` enforcement rules

`.value` on a `Maybe<T>` binding requires compile-time proof that the value is not none. M5 ships a NARROW subset of flow narrowing (full narrowing including negation/short-circuit/early-return is M6's `if (x is Type)` work).

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

**Scope of the subset**: positive, negative and AND forms are narrowed. Early-return narrowing needs negation-narrowing infrastructure that the `if (x is Type)` work (M6) owns. Reassignment invalidation is mechanical. Closures do not carry the flag.

---

## Documented v0.1 limitation: cycle leak through `Maybe<Self>` mutation

`shape Node<T> { value: T, next: Maybe<Node<T>> }` permits cycle creation through field mutation:

```ynz
let n1: Node<Int> = { value: 1, next: none }
let n2: Node<Int> = { value: 2, next: none }
n1.next = some(n2)   // (annotation-driven literal; some() is the constructor for Maybe<T>)
n2.next = some(n1)   // CYCLE — both nodes leak on scope exit
```

The v0.1 borrow checker does NOT detect reference cycles. The user gets a memory leak (`ynz_free` never reaches the cycled nodes). This is **intentional v0.1 behavior**, not a bug:

- **What**: cycles through `Maybe<Self>` field mutation leak both endpoints
- **Why**: cycle-detection requires global reference-graph analysis, which is borrow-checker work scoped for v0.2+ when the LSP enables interactive cycle visualization
- **Cost to fix later**: 1-2 sessions of borrow-checker work + IDE wiring — not blocking for v0.1's "compiler works end-to-end" goal
- **Trigger to revisit**: when v0.2 LSP work begins, OR when a real workload trips a production leak

Documented in fixture `crates/ynz-driver/tests/fixtures/m5_cycle_leak.ynz` with a comment citing this section.

---

## `Maybe<T>` is built into the compiler

`Maybe<T>` is the return type of `.get()` on every built-in collection (`Array<T>`, `Fixed<T>`, `Map<K, V>`, `String`) and of `.first()`, `.last()`, `.find()`, `.removeFirst()`, `.removeLast()` and similar methods. It is defined inside the compiler and needs no import, like `Array<T>`, `Fixed<T>` and `Map<K, V>`.

`some(value)` constructor: `some(value)` is sugar for "wrap this value as a `Maybe<T>`." Treated like a built-in function but resolved at typeck (no actual runtime call — codegen lowers `some(x)` to the tagged-union construction or pointer-wrap directly).

---

## Cross-References

- [`docs/reference/REF-maybe.md`](../../reference/REF-maybe.md) — user-facing surface
- [`docs/internal/implementation/IMP-generics.md`](IMP-generics.md) — generics engine that `Maybe<T>` is built on top of
- [`docs/internal/implementation/IMP-collections.md`](IMP-collections.md) — collections whose `.get()` returns `Maybe<T>`
- [`docs/reference/REF-golden-rules.md`](../../reference/REF-golden-rules.md) — Rule 2 (compile-time safety) governs the flow-sensitive `.value` check
- [`.claude/rules/dot-postfix.md`](../../../.claude/rules/dot-postfix.md) — `.value` is access (no parens), `.exists()` and `.or()` are actions (parens)
- [`.claude/rules/vocabulary.md`](../../../.claude/rules/vocabulary.md) — `maybe T` (user) and `Maybe<T>` (since M5 syntax-lock) terminology
