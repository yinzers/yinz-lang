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

Both ship in v0.1. Type generics are unusable without function generics — collections (`array<T>`, `map<K,V>`) need generic methods (`.filter`, `.map`, `.sort`) that work on any T, and the stdlib needs to ship a generic `sort`, `find`, `groupBy`, etc.

---

## Syntax: brackets after the name

Same bracket convention used everywhere else parameterized types appear:

| Use | Syntax |
|-----|--------|
| Generic type | `type Pair<A, B> { ... }` |
| Generic function | `function pair<A, B>(...) -> Pair<A, B>` |
| Parameterized type instance | `array<Player>`, `map<string, int>`, `number<70>` |
| Type parameter constraint | `function sort<T follows Comparable>(...)` |

One syntax. Angle brackets everywhere for type parameters.

**Why `<>` not `[]`**: `[]` is already used for index access (`arr[0]`, `map["key"]`). Using `[]` for type parameters too creates ambiguity — `json.parse[User](raw)` reads as array indexing, not a type parameter. `<>` is unambiguous: `<>` always means type parameter, `[]` always means index. Also consistent with TypeScript (Golden Rule 6 — borrow familiar syntax).

---

## Constraints: `follows` inline

Constraints use `follows` (the same keyword as type contracts) and are written **inline** with the type parameter:

```ynz
function sort<T follows Comparable>(share items: array<T>) -> array<T> { ... }

function process<T follows Comparable, Serializable>(share item: T) -> string { ... }
```

**Why inline over a separate `where` clause:**

```ynz
// Inline (chosen):
function sort<T follows Comparable>(share items: array<T>) -> array<T> { ... }

// Where clause (rejected):
function sort<T>(share items: array<T>) -> array<T> where T follows Comparable { ... }
```

The inline form keeps the constraint visible right next to the type parameter — the reader's eyes don't have to jump to a separate location. Rust adopted `where` clauses because Rust constraints got very complex (lifetimes + multiple traits + associated types + higher-ranked bounds). Yinz won't have that complexity in v0.1 or even v1.0 — keep it inline.

Golden Rule 2 (self-documenting syntax) and Golden Rule 9 (fast to read/type) both favor inline.

If/when Yinz constraints become complex enough to warrant a where clause (deep into v2+), we can revisit. Until then, inline.

---

## Type inference at call sites

The compiler infers type parameters from argument types at the call site. Users almost never write the type parameter explicitly:

```ynz
// Inference (99% of calls):
let x = identity(5)                    // T inferred as int
let p = pair("alice", 42)              // A=string, B=int inferred
let sorted = sort(players)             // T inferred as Player
let unique = dedupe([1, 2, 1, 3])      // T inferred as int

// Explicit only when ambiguous or no args:
let empty = createList<Player>()       // no args, T must be specified
```

This matches Rule 4 (compiler does the hard work) — jr devs don't write `<T>` for normal calls. The angle-bracket type-param syntax exists primarily for the *function signature*, not the call site.

---

## Compile errors

```ynz
function sort<T follows Comparable>(share items: array<T>) -> array<T> { ... }

shape Player { name: string, health: number }

let players: array<Player> = [...]
let sorted = sort(players)
//
// COMPILE ERROR: Type Player does not follow contract Comparable.
//
//   sort<T follows Comparable> requires T to follow Comparable, but Player
//   does not implement it. To make Player sortable, add a follows clause:
//
//     type Player follows Comparable { ... }
//
//   Then implement the required compare() method. See docs/reference/REF-operators.md.
```

```ynz
let x = identity()
//
// COMPILE ERROR: Cannot infer type parameter T for function identity.
//
//   identity<T>(value: T) -> T needs a value to infer T from, but no
//   arguments were passed. Either pass a value:
//
//     let x = identity(5)              // T inferred as int
//
//   Or specify T explicitly:
//
//     let x: int = identity<int>()     // explicit type — but identity()
//                                       // with no args has no meaning
```

---

## First built-in generic: `maybe<T>`

See [`docs/internal/implementation/IMP-maybe.md`](IMP-maybe.md) for `maybe<T>` — the first built-in generic primitive shipped via M5's generics engine. It's the return type of `.get()` on every built-in collection (`array<T>`, `fixed<T>`, `map<K, V>`, `string`), and demonstrates how built-in generics use the same engine that user-defined generics do. M5 ships `maybe<T>` alongside the engine itself rather than waiting for M6 — the cleanest API (`.get()` returns `maybe<T>` from day 1, no rename later).

---

## What's NOT in v0.1

**Higher-kinded types** (generic over generics, e.g. `Functor[F[_]]`): Not needed for v0.1. Probably never — Yinz isn't a Haskell-flavored language. Defer indefinitely.

**Lifetime parameters** (Rust-style `'a` lifetimes): Not exposed. Ownership in Yinz is fully inferred — users write `share`/`lend`/`give`/`copy` on values, never on type parameters. The compiler handles the equivalent of lifetimes internally.

**Const generics with arbitrary values:** Yinz has no general-purpose const generics in v0.1. General-purpose const generics are deferred. (Use `fixed<T>` with type inference instead.)

**Associated types:** A type contract that requires another type as part of the contract. Not in v0.1. Workaround: use explicit type parameters when needed.

These deferrals are not documented in [`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md) because users won't notice their absence in v0.1 — they're language-design features for advanced cases.
