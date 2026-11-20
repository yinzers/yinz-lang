---
name: "IMP-generics"
description: "Design decisions for Yinz's generics system, covering both type generics (shape<T>) and function generics using the same bracket-after-name syntax."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Generics — Design Decisions

User spec: covered across [`docs/reference/REF-generics.md`](../../reference/REF-generics.md) (type generics) and inline in [`docs/reference/REF-functions.md`](../../reference/REF-functions.md) (function generics, to be added).

---

## Two kinds of generics

Yinz has both **type generics** and **function generics**, both using the same bracket-after-name syntax.

**Type generics** (already in spec):
```ynz
shape Pair<A, B> {
  first: A
  second: B
}

shape Box<T> {
  value: T
}
```

**Function generics** (this decision):
```ynz
function identity<T>(give value: T) -> T {
  return value
}

function pair<A, B>(give first: A, give second: B) -> Pair<A, B> {
  return { first, second }
}
```

Both ship in v0.1. Type generics are unusable without function generics — collections (`Array<T>`, `Map<K,V>`) need generic methods (`.filter`, `.map`, `.sort`) that work on any T, and the stdlib needs to ship a generic `sort`, `find`, `groupBy`, etc.

---

## Syntax: brackets after the name

Same bracket convention used everywhere else parameterized types appear:

| Use | Syntax |
|-----|--------|
| Generic type | `shape Pair<A, B> { ... }` |
| Generic function | `function pair<A, B>(...) -> Pair<A, B>` |
| Parameterized type instance | `Array<Player>`, `Map<String, Int>`, `Number<70>` |
| Type parameter constraint | `function sort<T follows Comparable>(...)` |

One syntax. Angle brackets everywhere for type parameters.

`<>` always means type parameter and `[]` always means index (`arr[0]`), so a call like `json.parse<User>(raw)` is never read as indexing. Golden Rule 12 (familiar syntax) also favours `<>`.

---

## Constraints: `follows` inline

Constraints use `follows` (the same keyword as type contracts) and are written **inline** with the type parameter:

```ynz
function sort<T follows Comparable>(share items: Array<T>) -> Array<T> { ... }

function process<T follows Comparable, Serializable>(share item: T) -> String { ... }
```

There is no separate `where` clause: the constraint sits next to the type parameter it limits, so the reader finds it without looking elsewhere (Golden Rules 4 and 13).

---

## Type inference at call sites

The compiler infers type parameters from argument types at the call site. Users almost never write the type parameter explicitly:

```ynz
// Inference (99% of calls):
let x = identity(5)                    // T inferred as Int
let p = pair(`alice`, 42)              // A=String, B=Int inferred
let sorted = sort(players)             // T inferred as Player

// Explicit only when no argument carries T (createList is a function you define):
let empty = createList<Player>()       // no args, T must be specified
```

This matches Rule 8 (compiler does the hard work) — jr devs don't write `<T>` for normal calls. The angle-bracket type-param syntax exists primarily for the *function signature*, not the call site.

---

## Compile errors

A constraint is checked at each call. The compiler text lives in `crates/ynz-typeck/src/check.rs` (the `follows` constraint loop, near "Verify follows constraints"):

```ynz
function sort<T follows Comparable>(share items: Array<T>) -> Array<T> { ... }

shape Player { name: String, health: Number }

let players: Array<Player> = [...]
let sorted = sort(players)
// COMPILE ERROR: Type `Player` does not follow contract `Comparable`.
//   WHAT INSTEAD: To use `Player` here, add `follows Comparable` to its declaration AND implement the required methods.
//   WHY: `sort<T follows Comparable>` requires the concrete type to satisfy the `Comparable` contract.
```

A type argument that is not a shape (for example `Array<Int>` where `T follows Comparable`) fails with ``Type `Int` does not follow contract `Comparable` — only shapes can follow contracts.``

A type parameter no argument can bind is an error too:

```ynz
function identity<T>(give value: T) -> T { return value }

let x = identity()
// COMPILE ERROR: Cannot work out the type parameter `T` for function `identity` — pass a value or annotate explicitly.
//   WHAT INSTEAD: Examples: `identity(5)` (T = int) or `identity<Int>()`
//   WHY: Yinz figures out type parameters from the argument types. If there are no arguments, specify the type explicitly.
```

The text above is copied from the compiler source; it was not run through `ynz build` in this pass (the elided `{ ... }` bodies are not compilable as written).

---

## First built-in generic: `Maybe<T>`

See [`docs/internal/implementation/IMP-maybe.md`](IMP-maybe.md) for `Maybe<T>` — the first built-in generic primitive shipped via M5's generics engine. It's the return type of `.get()` on every built-in collection (`Array<T>`, `Fixed<T>`, `Map<K, V>`, `String`), and demonstrates how built-in generics use the same engine that user-defined generics do. M5 ships `Maybe<T>` alongside the engine itself rather than waiting for M6 — the cleanest API (`.get()` returns `Maybe<T>` from day 1, no rename later).

---

## What's NOT in v0.1

**Higher-kinded types** (generic over generics, e.g. `Functor[F[_]]`): Not needed for v0.1. Probably never — Yinz isn't a Haskell-flavored language. Defer indefinitely.

**Lifetime parameters** (Rust-style `'a` lifetimes): Not exposed. Ownership in Yinz is fully inferred — users write `share`/`lend`/`give`/`copy` on values, never on type parameters. The compiler handles the equivalent of lifetimes internally.

**Const generics with arbitrary values:** Yinz has no general-purpose const generics in v0.1. General-purpose const generics are deferred. (Use `Fixed<T>` with type inference instead.)

**Associated types:** A type contract that requires another type as part of the contract. Not in v0.1. Workaround: use explicit type parameters when needed.

These deferrals are not documented in [`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md) because users won't notice their absence in v0.1 — they're language-design features for advanced cases.
