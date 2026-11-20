---
name: "REF-types"
description: "A shape defines the structure of your data — what fields it has and what types those fields hold."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Types

A `shape` defines the structure of your data — what fields it has and what types those fields hold.

> Yinz is not object-oriented. A shape holds data and, if you want, contract signatures (lists of the functions a shape must provide) — never function bodies. Functions are declared on their own. `value.method()` is a shortcut for `method(value)`, and both forms work.

---

## Defining a shape

```ynz
shape Player {
  name: string
  health: number
  score: number
}
```

The fields sit next to each other in memory, so reading them is fast.

---

## Creating a shape value

Write the variable's type, then assign a `{ ... }` value:

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

Writing the name before the braces, as in `let p = Player { ... }`, is NOT allowed. The type you write on the variable tells the compiler which shape the `{ ... }` value must match.

---

## Functions for a shape are standalone

In Yinz, the functions that work on a shape are normal `function` declarations at the top level of a file or module. They take the value as the first parameter (usually named `self`). When you call one, you can write either `value.method()` (dot form) or `method(value)` (plain form). Both are legal and mean the same thing.

```ynz
shape Player {
  name: string
  health: number
}

// These functions live on their own — NOT inside the shape body
function takeDamage(lend self: Player, amount: number) -> nothing {
  self.health = self.health - amount
}

function isAlive(share self: Player) -> boolean {
  return self.health > 0
}

const player: Player = { name: `Patrick`, health: 100 }

player.takeDamage(25)    // dot form — a shortcut for takeDamage(player, 25)
takeDamage(player, 25)    // plain form — same effect
player.isAlive()          // dot form — a shortcut for isAlive(player)
```

The `lend self: Player` parameter says "this function changes a Player." `share self: Player` says "this function only reads it." See [REF-ownership.md](REF-ownership.md).

The IDE understands both forms. Typing `player.` shows every function whose first parameter is a `Player`, so autocomplete helps you discover functions even though they are not stored inside the shape.

---

## Extending shapes — sharing fields

`extends` reuses the parent's FIELDS. Behavior comes from standalone functions, and each function name must be **unique in its file**. Give the Warrior's behavior its own name instead of reusing the parent's function name:

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

// Warrior version — armor absorbs damage. It needs a different name, because
// two functions with the same name in one file are not allowed.
function takeWarriorDamage(lend self: Warrior, amount: number) -> nothing {
  self.health = self.health - (amount - self.armor)
}

const warrior: Warrior = {
  name: `Patrick`,
  health: 100,
  weapon: `sword`,
  armor: 15
}

warrior.takeWarriorDamage(20)
// Damage applied: 20 - 15 = 5
```

A shape can `extends` only one other shape. The fields are reused, but each shape's own behavior needs its own function name.

There is no `override` keyword in Yinz. Two `function` declarations with the same name in the same file are a compile error, even when their first parameters have different types:

```ynz
function takeDamage(lend self: Entity, amount: number) -> nothing {
  self.health = self.health - amount
}

function takeDamage(lend self: Warrior, amount: number) -> nothing {
  self.health = self.health - (amount - self.armor)
}
// COMPILE ERROR: A function named `takeDamage` is already defined in this file.
// Rename one of the two functions — each function in a file must have a unique name.
```

Picking a same-named function by its argument's type (so `warrior.takeDamage(20)` and `entity.takeDamage(20)` each call the version written for their own shape) is planned but not built yet.

---

## base shapes — never created directly

Mark a shape `base` if it exists only to be extended and should never be created directly:

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

If two unrelated shapes need to be used interchangeably, write a contract and use `follows`. A contract lists function signatures with no `function` keyword and no body. Every shape that follows the contract must have matching standalone functions.

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

// Standalone functions supply the behavior.
// The compiler checks that they match Damageable's signature.
function takeDamage(lend self: Player, amount: number) -> nothing {
  self.health = self.health - amount
}

function takeDamage(lend self: Building, amount: number) -> nothing {
  self.health = self.health - (amount / 2)    // buildings take half damage
}

function dealDamage(lend target: Damageable, amount: number) -> nothing {
  target.takeDamage(amount)    // dot form — works for any shape that follows Damageable
}

dealDamage(player, 50)        // Player follows Damageable — works
dealDamage(building, 30)      // Building follows Damageable — works
```

`follows` is optional. If the matching functions exist, the shape can be used wherever the contract is asked for. But writing `follows` makes the compiler report a mismatch where you declare the shape instead of at the first call, and it tells readers what the shape is meant to do.

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

## Matching by fields

You don't have to name the shape when you return a `{ ... }` value. Its fields only have to match the declared return type:

```ynz
shape DivResult {
  quotient: number
  remainder: number
}

function divmod(a: number, b: number) -> DivResult {
  return { quotient: a / b, remainder: a % b }    // matches DivResult — valid
}
```

The compiler checks that the fields exist and have the right types.

---

## Hidden fields — invisible outside the same file

Mark a field `hidden` to make it unreachable from code in OTHER FILES. A hidden field needs a default value, because code in other files can't provide one when it creates the shape.

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

Code in other files only provides the visible fields when it creates the value:

```ynz
// File: entrypoint.ynz
import { Player, takeDamage } from `./player`

const player: Player = { name: `Alice`, health: 100 }
// damageMultiplier starts at 1.0, internalCache starts empty
// (defaults from player.ynz)

takeDamage(player, 25)              // ✅ allowed — takeDamage is exported
print(player.damageMultiplier)
// COMPILE ERROR: damageMultiplier is hidden — not accessible outside player.ynz.
```

`hidden` controls who can see a field, not whether it can change. Functions in the same file can both read and write a hidden field. Other files that import the shape can't see it.

You can export a shape and still keep some fields private. Code in other files cannot write `player.damageMultiplier = 10` and skip the rules that `takeDamage` follows.

---

## Inline shapes (shapes without a name)

If a shape is used in only one place, you can write it right where you use it instead of scrolling up to a `shape` declaration at the top of the file.

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
// Variable
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

### Same fields means same type

Two inline shapes with the same fields are the same type, wherever they appear and in whatever order the fields are listed:

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

Use an inline shape when it is truly one-off — written and used in the same small block. Use a named shape when:

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
- **No `extends` or `follows`.** Inline shapes hold plain data only, so they can't share fields with a parent or follow a contract.
- Named shapes and inline shapes with identical fields are **different types** and do not convert to each other. If you annotate a variable `let x: Foo`, you cannot assign an `{ a: int }` value to it even if `Foo` has exactly field `a: int`.
