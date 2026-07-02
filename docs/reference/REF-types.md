---
name: "REF-types"
description: "A shape defines the structure of your data — what fields it has and what types those fields hold."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Types

A `shape` defines the structure of your data — what fields it has and what types those fields hold.

> Yinz is not object-oriented (see [`.claude/rules/non-oop.md`](../../.claude/rules/non-oop.md)). Shapes hold DATA and (optionally) CONTRACT METHOD-SIGNATURE DECLARATIONS — never method bodies. Methods are standalone `function` declarations. `value.method()` is parser-level sugar for `method(value)` — both call forms work (Uniform Function Call Syntax — UFCS).

---

## Defining a shape

```ynz
shape Player {
  name: string
  health: number
  score: number
}
```

Fields are laid out flat and contiguous in memory. Fast to access, CPU-cache friendly.

---

## Creating an instance

Use annotation-driven literal form — declare the variable's type, then assign a `{ ... }` literal:

```ynz
const player: Player = {
  name: `Patrick`,
  health: 100,
  score: 0
}
```

Every field must be provided. Forgetting one is a compile error:

```ynz
const player: Player = {
  name: `Patrick`,
  health: 100
}
// COMPILE ERROR: Missing field 'score' (required by Player).
```

The `let p = Player { ... }` prefix-form is NOT legal — Yinz uses structural-typed literals exclusively. The annotation on the binding tells the compiler what shape the literal must satisfy.

---

## Methods are standalone functions

In Yinz, methods are normal `function` declarations at file/module level. They take the value as the first parameter (conventionally named `self`). At the call site, you can write either `value.method()` (dot-call) or `method(value)` (function-call) — both are legal and equivalent.

```ynz
shape Player {
  name: string
  health: number
}

// Methods live as standalone functions — NOT inside the shape body
function takeDamage(lend self: Player, amount: number) -> nothing {
  self.health = self.health - amount
}

function isAlive(share self: Player) -> boolean {
  return self.health > 0
}

const player: Player = { name: `Patrick`, health: 100 }

player.takeDamage(25)    // dot-call — sugar for takeDamage(player, 25)
takeDamage(player, 25)    // function-call — same effect
player.isAlive()          // dot-call — sugar for isAlive(player)
```

The `lend self: Player` parameter says "this function modifies a Player." `share self: Player` says "read-only." See [`docs/reference/REF-ownership.md`](REF-ownership.md).

The IDE recognizes both call forms. Typing `player.` shows autocomplete with all standalone functions whose first parameter type is `Player` — autocomplete-as-teaching for free, without methods being bound to the type.

---

## Extending shapes — data-only inheritance

`extends` reuses parent's FIELDS. Behavior comes from standalone functions; the compiler picks the most specific overload at the call site (no `override` keyword needed).

```ynz
shape Entity {
  name: string
  health: number
}

shape Warrior extends Entity {
  weapon: string
  armor: number
}

function takeDamage(lend self: Entity, amount: number) -> nothing {
  self.health = self.health - amount
}

// Warrior-specific version — armor absorbs damage. Same function name.
function takeDamage(lend self: Warrior, amount: number) -> nothing {
  self.health = self.health - (amount - self.armor)
}

const warrior: Warrior = {
  name: `Patrick`,
  health: 100,
  weapon: `sword`,
  armor: 15
}

warrior.takeDamage(20)    // calls takeDamage(Warrior) — more specific overload wins
// Damage applied: 20 - 15 = 5
```

Single inheritance only. A shape can `extends` one other shape; behavior is provided by overloaded standalone functions.

There is no `override` keyword in Yinz — write multiple `function` declarations with the same name and different first-parameter types, and the compiler picks the most specific match. See [`.claude/rules/non-oop.md`](../../.claude/rules/non-oop.md) for the rationale.

---

## base shapes — not directly instantiable

Mark a shape `base` if it's only meant to be extended, never created directly:

```ynz
base shape Entity {
  name: string
  health: number
}

const e: Entity = { name: `test`, health: 50 }
// COMPILE ERROR: Entity is a base shape — you can't create one directly.
//
//   Create a shape that extends Entity instead:
//     shape Warrior extends Entity { weapon: string, armor: number }
//     const w: Warrior = { name: `test`, health: 50, weapon: `axe`, armor: 10 }
//
//   Why: base shapes describe shared fields but aren't meant to stand alone.
//        Creating one directly would give you an incomplete value. Always
//        use a specific shape that extends the base.

const w: Warrior = { name: `test`, health: 50, weapon: `axe`, armor: 10 }   // fine
```

---

## follows — behavior contracts

If two unrelated shapes need to be used interchangeably, define a contract shape and use `follows`. Contracts declare method signatures in bare-signature form (no `function` keyword, no body — the implementing shape must provide a matching standalone function).

```ynz
shape Damageable {
  health: number
  takeDamage(lend self, amount: number) -> nothing      // bare signature — no body
}

shape Player follows Damageable {
  name: string
  health: number
}

shape Building follows Damageable {
  address: string
  health: number
}

// Standalone functions provide the implementations.
// Compiler verifies these match Damageable's signature when checking `follows`.
function takeDamage(lend self: Player, amount: number) -> nothing {
  self.health = self.health - amount
}

function takeDamage(lend self: Building, amount: number) -> nothing {
  self.health = self.health - (amount / 2)    // buildings take half damage
}

function dealDamage(lend target: Damageable, amount: number) -> nothing {
  target.takeDamage(amount)    // dot-call — works for any shape that follows Damageable
}

dealDamage(player, 50)        // Player follows Damageable — works
dealDamage(building, 30)      // Building follows Damageable — works
```

`follows` is optional — Yinz uses structural typing (if the matching functions exist, the shape can be used). But declaring `follows` catches mismatches at definition time rather than at the first call site, and makes the relationship visible to readers.

A shape can follow multiple contracts — `extends` comes first, then `follows` with a comma-separated list:

```ynz
shape Warrior extends Entity follows Damageable, Attackable, Renderable {
  weapon: string
  armor: number
}
// Warrior must have standalone functions matching every signature in
// Damageable, Attackable, and Renderable.
```

---

## Structural typing — shape matching

You don't have to name the shape when returning a literal. The data has to match the declared return type:

```ynz
shape DivResult {
  quotient: number
  remainder: number
}

function divmod(a: number, b: number) -> DivResult {
  return { quotient: a / b, remainder: a % b }    // matches DivResult — valid
}
```

The compiler checks that the fields exist with the right types. No need to write `new DivResult(...)`.

---

## Hidden fields — invisible outside the same file

Mark a field `hidden` to make it inaccessible to code in OTHER FILES. Hidden fields require a default value because external code can't provide them at construction.

```ynz
// File: player.ynz
shape Player {
  name: string
  health: number
  hidden damageMultiplier: number = 1.0
  hidden internalCache: map<string, number> = {}
}

// Standalone functions in the same file CAN touch hidden fields
function takeDamage(lend self: Player, amount: number) -> nothing {
  const actual = amount * self.damageMultiplier    // ✅ same file, can touch hidden
  self.health = self.health - actual
}
```

External callers only provide visible fields when creating the value:

```ynz
// File: entrypoint.ynz
import { Player, takeDamage } from `./player`

const player: Player = { name: `Alice`, health: 100 }
// damageMultiplier starts at 1.0, internalCache starts empty
// (defaults from player.ynz)

takeDamage(player, 25)              // ✅ public API
print(player.damageMultiplier)
// COMPILE ERROR: damageMultiplier is hidden — not accessible outside player.ynz.
```

`hidden` is about visibility, not mutability. Hidden fields can be both read and written by standalone functions in the same file. They're invisible to imports.

**Why this exists**: per-field visibility within an exported shape. Without `hidden`, external code could write `player.damageMultiplier = 10` bypassing whatever invariants `takeDamage` maintains. Module-level "don't export the field" doesn't work because you can't export a shape without exposing all its fields.

---

## Inline shapes (anonymous types)

> Design rationale: [`docs/internal/implementation/IMP-inline-shape-types.md`](../internal/implementation/IMP-inline-shape-types.md)

If a type is only used in one place, you can define it right where you use it instead of scrolling to a `shape` declaration at the top of the file.

```ynz
// Named shape — right when used in more than one function
shape IntervalConfig {
    minutes: int
    timeframe: Timeframe
}

// Inline type — right for a one-off config that never leaves this block
const intervals: fixed<{ minutes: int }> = [
    { minutes: 5 },
    { minutes: 15 },
    { minutes: 60 },
]
```

`{` in a type annotation always means an inline shape. No extra keyword needed.

### Where inline shapes can appear

Inline shapes work in any type-annotation position:

```ynz
// Variable binding
let p: { x: int, y: int } = { x: 3, y: 4 }

// Function parameter
function area(rect: { w: int, h: int }) -> int {
    return rect.w * rect.h
}

// Inside a generic type
const intervals: fixed<{ minutes: int }> = [{ minutes: 5 }, { minutes: 15 }]

// Field type inside a named shape
shape Bar {
    symbol: string
    spread: {
        bid: number
        ask: number
    }
}
```

### Accessing fields

Field access works the same as for named shapes:

```ynz
let p: { x: int, y: int } = { x: 3, y: 4 }
print(`${p.x + p.y}`)  // prints 7
```

### Structural equivalence

Two inline shapes with the same fields are the same type, regardless of where they appear or what order the fields are listed:

```ynz
function takesAB(p: { a: int, b: int }) -> nothing {
    print(`${p.a}`)
}

function entrypoint() -> nothing {
    let x: { b: int, a: int } = { a: 1, b: 2 }
    takesAB(x)  // OK — same fields, same type
}
```

### Inline vs named: which to use

Use an inline shape when the type is truly one-off — defined and consumed in the same small block. Use a named shape when:

- The same type appears in more than one function or file
- You want to give the type a meaningful name for readability
- You need to use `extends`, `follows`, or `hidden` fields

```ynz
// Inline — fine when only this loop uses it
for ({ minutes } in intervals) {
    print(`${minutes}`)
}

// Named — better when the type is used in multiple places
shape IntervalConfig {
    minutes: int
    timeframe: Timeframe
}
```

### Restrictions

- **No `hidden` fields.** Hidden fields are file-private and require a named `shape` declaration.
- **No `extends` or `follows`.** Inline shapes are pure data — they have no inheritance or contract requirements.
- Named shapes and inline shapes with identical fields are **different types** and do not convert to each other. If you annotate a variable `let x: Foo`, you cannot assign an `{ a: int }` value to it even if `Foo` has exactly field `a: int`.
