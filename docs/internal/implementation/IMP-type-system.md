---
name: "IMP-type-system"
description: "Design decisions for Yinz's type system: the single 'shape' declaration keyword, data-only single inheritance via 'extends', non-instantiable 'base' shapes, and related type-system rules."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Type System — Design Decisions

User spec: [`docs/reference/REF-types.md`](../../reference/REF-types.md), [`docs/reference/REF-maybe.md`](../../reference/REF-maybe.md), [`docs/reference/REF-unions.md`](../../reference/REF-unions.md), [`docs/reference/REF-options.md`](../../reference/REF-options.md), [`docs/reference/REF-generics.md`](../../reference/REF-generics.md)

---

## Built-in Type Names Are PascalCase

Every type starts with a capital letter, built-ins included: `Int`, `Float`, `Number`, `Boolean`, `String`, `Range`, `Maybe<T>`, `Array<T>`, `Fixed<T>`, `Map<K, V>`, `Channel<T>`, `Nothing`. `MapEntry`, `Frame`, `SourceLoc` and `Self` keep their names. Reason: capital letter = type (Golden Rule 10), so no type name needs an exception.

- **Functions, modules, methods and variables stay lowercase.** `range(a, b)` is a function that returns a `Range`; `.map(fn)` is a method.
- **Type-attached constants** are read off the type name: `Int.max`, `Number.epsilon`.
- **`Nothing` is an ordinary type name.** There is no `nothing` keyword.
- **`Boolean`, not `Bool`.** Golden Rule 5: plain words.
- **No aliases.** Old lowercase spellings in a type position (`int`, `string`, ...) are a compile error: WHAT "`int` is not a type name.", WHAT INSTEAD "Write `Int`.", WHY "Every type in Yinz starts with a capital letter, built-ins included.", plus an LSP quick-fix (see [`IMP-lsp.md`](IMP-lsp.md)). Backwards-compatibility shims are refused (see [`IMP-versioning.md`](IMP-versioning.md)).
- **Reserved names.** A user shape named like a built-in (`String`, `Map`, `Range`, ...) is a compile error: "`{Name}` is a reserved built-in type name."
- **Runtime messages** use the same names: `Int overflow in '+'`, `division by zero (Int)`, `division by zero (Number)`.

---

## One Keyword: `shape`

`shape` is the only declaration keyword. No `interface`, `struct`, `class`, `type`.

**Why**: one word covers plain data shapes, contracts (bare-signature declarations only, no bodies) and shapes that extend other shapes. The declaration keyword `type` is rejected at the lexer, and the diagnostic redirects to `shape`.

> Yinz is **NOT object-oriented** — see [`.claude/rules/non-oop.md`](../../../.claude/rules/non-oop.md). Shapes hold DATA + (optionally) CONTRACT METHOD-SIGNATURE DECLARATIONS only. Method implementations live as standalone `function` declarations at file/module level. `value.method()` is parser-level sugar for `method(value)` (UFCS — Uniform Function Call Syntax). The sections below describe the type system within this non-OOP model.

---

## Single Inheritance with `extends` (data-only)

Single inheritance only. `extends` reuses parent's DATA FIELDS — behavior comes from standalone functions, not from inherited methods (because there are no methods on shapes to inherit). No multiple inheritance.

**Why**: single inheritance has no ambiguous resolution. `extends` reuses data only; shared behavior comes from `follows`, which takes any number of contracts. Overloading by argument type is not built (see the overloading section below).

**Example** (distinct function names, since same-named functions in one file are a compile error):
```ynz
shape Entity { name: String, health: Int }
shape Warrior extends Entity { weapon: String, armor: Int }

function greet(share self: Entity) -> String {
  return `Hello, I am ${self.name}`
}

function greetWarrior(share self: Warrior) -> String {
  return `Hello, I am ${self.name} the warrior, wielding ${self.weapon}`
}

const w: Warrior = { name: `Aragorn`, health: 100, weapon: `sword`, armor: 50 }
w.greetWarrior()    // UFCS sugar for greetWarrior(w) — distinct name, not an overload
```

---

## `base` for Non-Instantiable Shapes

`base shape Entity` instead of `abstract class Entity`.

**Why**: "base shape" reads as plain English, and `abstract` is object-oriented vocabulary (Golden Rule 5).

`base` shapes cannot be instantiated via shape value literal — attempting `let e: Entity = { ... }` on a `base shape Entity` is a compile error. They exist purely to be extended.

---

## `follows` for Contracts

`follows` instead of `implements`. Optional in structurally-typed code but recommended for catching contract mismatches at definition time.

**Why**: "Player follows Damageable" reads as a sentence (Golden Rule 5). Structural typing means `follows` is not required for compatibility; writing it reports a missing function at the shape declaration instead of at the first call site.

**Contract verification**: when a shape declares `follows Foo`, the compiler verifies that standalone functions exist whose signatures match each of Foo's bare-signature method declarations. No method implementations live inside the contract OR the implementing shape — both sides declare signatures; standalone functions provide the behavior.

**Multiple allowed**: `shape Warrior extends Entity follows Damageable, Attackable` — single `extends`, any number of `follows`.

---

## No `override` Keyword; Function Overloading by Argument Type Is Not Built

There is no `override` keyword: methods don't live inside shapes ([`.claude/rules/non-oop.md`](../../../.claude/rules/non-oop.md)), so there is nothing to override.

**Function overloading by argument type** (several standalone functions with the same name and different first-parameter types) is not built. The typeck collision check (`crates/ynz-typeck/src/signatures.rs`) keys the signature table by function name alone and rejects a second same-named function in the same file, whatever its parameter types:

```ynz
shape Entity { name: String }
shape Warrior extends Entity { weapon: String }

function attack(share self: Entity) -> String { return `${self.name} attacks!` }
function attack(share self: Warrior) -> String { return `${self.name} swings ${self.weapon}!` }
// COMPILE ERROR: A function named `attack` is already defined in this file.
```

`extends` gives a child shape its parent's fields only, so call sites get no polymorphic dispatch over a shared function name. Polymorphism comes from **generics** (a specialized version per concrete type) and **`dynamic Contract`** (runtime dispatch, see [Dynamic dispatch](#dynamic-dispatch--when-and-how)). Overload-by-argument-type is the `function-overload-by-argument-type` `[[deferred_language_feature]]` entry in [`registry/features.toml`](../../../registry/features.toml). Until it ships, give such functions distinct names (`attackEntity`, `attackWarrior`) or use generics.

---

## Structural Typing

If a value's fields match the expected shape, the value is valid with no explicit constructor. `return { quotient: a / b, remainder: a % b }` works when the return type is `DivResult`.

**Why**: Golden Rule 12 (familiar syntax) and less ceremony at every construction site.

---

## Static Dispatch Default for `follows` Constraints

When a function is generic over a `follows` constraint and the concrete type at the call site is known, the compiler generates a specialized version per concrete type and inlines the called functions (no call overhead, no runtime lookup). This is static dispatch: the decision is made at compile time. Dynamic dispatch (runtime lookup, no inlining) is the explicit opt-in for when the concrete type is unknown until runtime.

Locked: static dispatch is the default; dynamic dispatch requires explicit syntax. User-facing surfaces and docs say "specialized version per type", never the compiler-literature term for it (Golden Rule 5).

### Concrete example

```ynz
// Define a contract — bare-signature declarations only (no `function` keyword, no body)
shape Comparable {
  compare(share self, share other: Self) -> Int
}

// Two shapes that follow the contract — data fields only
shape Player follows Comparable {
  name: String
  health: Int
}

shape Item follows Comparable {
  name: String
  weight: Int
}

// Standalone functions provide the implementations.
// Compiler verifies these match Comparable's signature when checking `follows`.
// A file holds one function per name, so Item's `compare` lives in the file that declares Item.
function compare(share self: Player, share other: Player) -> Int {
  return self.health - other.health
}

// A generic function — works on anything that follows Comparable
function findMax<T follows Comparable>(share items: Array<T>) -> Maybe<T> {
  // ... walk the array, keep the largest per compare()
}

// CASE A — concrete type known → static dispatch (auto-picked)
let players: Array<Player> = [...]
let best = findMax(players)
// The compiler generates a specialized findMax for Player; compare() is inlined.

// CASE B — heterogeneous collection → dynamic dispatch (opt-in via `dynamic`)
let mixedThings: Array<dynamic Comparable> = [player1, item1, player2]
let maxThing = findMax(mixedThings)
// No specialized version exists: the array holds different concrete types, so each
// compare() is a runtime lookup plus an indirect call.
```

**Built today**: the specialized-version codegen for generics. `dynamic Contract` parses and type-checks, but codegen rejects calls through it (`emit.rs:23987-23990`, "dynamic dispatch call sites not yet lowered"), so Case B does not build.

### When you need `dynamic`

Only when heterogeneity is the feature:
- **Heterogeneous collections**: a UI tree whose leaves are all `Drawable` but are buttons, sliders and text boxes, stored in `Array<dynamic Drawable>`.
- **Plugin architectures**: types loaded from packages at runtime.
- **Stored callback collections**: arrays of different function-shaped values.

### Teaching surfaces (designed, not built)

Per Golden Rule 3 and [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md), both dispatch cases show what was chosen at the call site, plus a lint when `dynamic` was unnecessary. None of these hints or the lint exist in the compiler yet.

#### Static dispatch: neutral muted hint

```ynz
let best = findMax(players)        // muted: // static dispatch (T = Player)
```

Hover tooltip:
- **WHAT**: a specialized version of `findMax` was generated for `Player`, and `compare()` is inlined into the loop body.
- **WHAT INSTEAD**: to force dynamic dispatch, wrap the input as `Array<dynamic Comparable>`.
- **WHY**: the compiler knew the concrete type at this call site, so no runtime lookup is needed.

#### Dynamic dispatch: cautionary muted hint (red-tinted per [`.claude/rules/inference.md`](../../../.claude/rules/inference.md))

```ynz
let maxThing = findMax(mixedThings)   // muted (red-tinted): // dynamic dispatch — runtime lookup per compare()
```

Hover tooltip:
- **WHAT**: this call uses a runtime lookup because `mixedThings` is `Array<dynamic Comparable>`, so the concrete type of each element is unknown at compile time.
- **WHAT INSTEAD**: if all elements have the same concrete type, switch to `Array<Player>` for static dispatch.
- **WHY**: you wrote `dynamic Comparable` to allow mixed storage, which costs one runtime lookup and indirect call per `compare()`.

#### Lint when `dynamic` could have been static

```ynz
let things: Array<dynamic Comparable> = []   // yellow squiggle from `prefer-static-dispatch-when-monotype`
things.add(player1)
things.add(player2)
// All elements are Player. The dynamic wrapper is unnecessary.
```

The lint fires when the compiler proves the `dynamic` collection only ever holds one concrete type. Suggested fix: change to `Array<Player>`.

#### Compile errors when a contract is not followed

```
COMPILE ERROR: Item is not a Player.
  Player and Item are different types — Array<Player> only holds Players.

  To store a mix of Comparable shapes, declare:
    let things: Array<dynamic Comparable> = []

  This adds runtime dispatch cost per compare() call but allows mixed types.
```

```
COMPILE ERROR: Foo does not follow Comparable.
  findMax requires elements to follow the Comparable contract.

  To make Foo work with findMax:

    1. Declare Foo follows Comparable:
       shape Foo follows Comparable {
         name: String
       }

    2. Provide a standalone compare function matching Comparable's signature:
       function compare(share self: Foo, share other: Foo) -> Int {
         // ... return negative, zero, or positive
       }

  Yinz is not object-oriented — contract implementations are standalone
  functions, not methods inside the shape. See docs/reference/REF-operators.md.
```

All diagnostics follow what / what-instead / why and use plain words ("specialized version per type", "runtime lookup").

### Auto-promotion (per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md))

Codegen only: when the compiler proves the concrete type at a call site, it emits the static-dispatch version. The muted hint above is the optional confirmation; there is no lint, because the user wrote idiomatic generic code.

### Dynamic dispatch — when and how

The opt-in is `dynamic Contract` (for example `dynamic Drawable`). It is written `dynamic`, never `dyn` (Golden Rule 5).

### Tradeoff

A specialized version per concrete type grows binary size with the number of concrete types behind one contract. Identical instantiations are deduplicated (generics section of [`docs/internal/implementation/IMP-compiler.md`](IMP-compiler.md)), and `dynamic` recovers code size when that matters.

---

## Generics — `name<T>` Syntax

`shape Box<T> { value: T }` — angle bracket generics, same pattern as built-in collections.

**Why**: one `name<type>` pattern covers built-in (`Array<Player>`, `Map<String, Number>`) and user-defined generic types.

---

## Union Types with `|`

`shape DrawableShape = Circle | Square | Triangle` with `is` for type checking and narrowing.

**Why `|` over `or`**: the Golden Rule 5 exception: `or` is a prose word, while `|` is unambiguous. `is` reads as plain English ("if shape is Circle").

**`is` in a union is an exact-type test**: `Admin` does not match `is User` in a union even if `Admin extends User`. Outside unions, normal subtype rules apply. Each variant stays distinct.

---

## No Null — `Maybe` Types

No `null`. No `undefined`. Absence is expressed as `none` and tracked by the type system with `Maybe<T>`.

**Why**: explicit absence moves null errors to compile time (Golden Rule 2).

**`Maybe<T>` and `T | none`** are designed as one type. Built today, `T | none` stays a plain union and is not rewritten to `Maybe<T>`; write `Maybe<T>` where `Maybe` lowering must apply (see [`IMP-maybe.md`](IMP-maybe.md)).

---

## Hidden Fields — `hidden` Keyword

`hidden` fields are invisible to code OUTSIDE THE SAME FILE as the shape declaration. They require a default value:

```ynz
shape Player {
  name: String
  hidden damageMultiplier: Number = 1.0
  hidden hitCount: Int = 0
}
```

**Why `hidden` exists**: per-field visibility within an exported shape. Exporting a shape exposes all its fields, so without `hidden` external code could write `player.damageMultiplier = 10` and bypass the invariants `takeDamage` maintains.

**Why `hidden` over `private`**: Golden Rule 5; `hidden damageMultiplier` is understood without OOP vocabulary.

**Why defaults are required**: a caller constructing the shape provides only the visible fields, so the initial state of a hidden field must be explicit in the type definition (Golden Rule 4).

**Visibility scope**: a `hidden` field is read and written only by standalone functions declared in the same file as the shape, and is invisible to imports and to functions in other files.

**Hidden vs ownership modifiers** are separate: `share`/`lend`/`give` declare ownership in signatures; `hidden` is field-level visibility. They don't interact.

---

## `options` Keyword

`options Status { active, inactive, banned }` instead of `enum`.

**Why**: "options" is a plain word; `enum` is jargon (Golden Rule 5).

**Built-in options**: `SortOrder { asc, desc }`, `Comparison { equal, greater, less }` (registered in `crates/ynz-typeck/src/options_table.rs`).

**Shorthand**: allowed when the expected type is known at the call site.

---

## Anonymous Inline Shape Types

A shape field can be typed with an inline anonymous shape definition instead of requiring a separate named shape:

```ynz
shape Bar {
    symbol: String
    spread: {
        bid: Number
        ask: Number
    }
}
```

**No keyword**: `{` in type position is unambiguous, so the compiler reads it as an anonymous shape type.

**How it works**: the compiler registers the anonymous shape as a synthetic named shape with a content-based name, so every later pass sees an ordinary named shape. Naming and equivalence rules are in [`IMP-inline-shape-types.md`](IMP-inline-shape-types.md).

**Print output**: the synthesized name is omitted (`emit.rs:27088`), so `print(b)` shows `Bar { symbol: AAPL, spread: { bid: 182.40, ask: 182.50 } }`.

**Anonymous vs named**: use an anonymous shape for a one-off field referenced nowhere else; use a named shape (`shape Spread { ... }`) when it is reused or passed independently.
