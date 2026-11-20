---
name: "REF-types"
description: "A shape defines the structure of your data — what fields it has and what types those fields hold."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
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

```
shape Player {
  name: String
  health: Int
  score: Int
}
```

The fields sit next to each other in memory, so reading them is fast.

Write one field per line.

---

## Creating a shape value

Write the variable's type, then assign a `{ ... }` value:

```
const player: Player = {
  name: `Patrick`,
  health: 100,
  score: 0
}
```

Every field must be provided. Forgetting one is a compile error:

```
const player: Player = {
  name: `Patrick`,
  health: 100
}
// COMPILE ERROR: Missing field `score` in `Player` construction.
// Add `score: value` to the shape value.
// Every visible field of a shape must be provided when constructing a value — the compiler cannot fill them in for you.
```

Writing the name before the braces, as in `let p = Player { ... }`, is NOT allowed. The type you write on the variable tells the compiler which shape the `{ ... }` value must match.

---

## Functions for a shape are standalone

In Yinz, the functions that work on a shape are normal `function` declarations at the top level of a file or module. They take the value as the first parameter (usually named `self`). When you call one, you can write either `value.method()` (dot form) or `method(value)` (plain form). Both are legal and mean the same thing.

```
shape Player {
  name: String
  health: Int
}

// These functions live on their own — NOT inside the shape body
function takeDamage(lend self: Player, amount: Int) -> Nothing {
  self.health = self.health - amount
}

function isAlive(share self: Player) -> Boolean {
  return self.health > 0
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Patrick`, health: 100 }

  player.takeDamage(25)     // dot form — a shortcut for takeDamage(player, 25)
  takeDamage(player, 25)    // plain form — same effect
  let alive = player.isAlive()    // dot form — a shortcut for isAlive(player)
  print(alive)
}
```

The `lend self: Player` parameter says "this function changes a Player." `share self: Player` says "this function only reads it." See [REF-ownership.md](REF-ownership.md).

The IDE understands both forms. Typing `player.` shows every function whose first parameter is a `Player`, so autocomplete helps you discover functions even though they are not stored inside the shape.

---

## Extending shapes — sharing fields

`extends` reuses the parent's FIELDS. Behavior comes from standalone functions, and each function name must be **unique in its file**. Give the Warrior's behavior its own name instead of reusing the parent's function name:

```
shape Entity {
  name: String
  health: Int
}

shape Warrior extends Entity {
  weapon: String
  armor: Int
}

function takeDamage(lend self: Entity, amount: Int) -> Nothing {
  self.health = self.health - amount
}

// Warrior version — armor absorbs damage. It needs a different name, because
// two functions with the same name in one file are not allowed.
function takeWarriorDamage(lend self: Warrior, amount: Int) -> Nothing {
  self.health = self.health - (amount - self.armor)
}

function entrypoint() -> Nothing {
  let warrior: Warrior = {
    name: `Patrick`,
    health: 100,
    weapon: `sword`,
    armor: 15
  }

  warrior.takeWarriorDamage(20)
  // Damage applied: 20 - 15 = 5
}
```

A shape can `extends` only one other shape. The fields are reused, but each shape's own behavior needs its own function name.

There is no `override` keyword in Yinz. Two `function` declarations with the same name in the same file are a compile error, even when their first parameters have different types:

```
function takeDamage(lend self: Entity, amount: Int) -> Nothing {
  self.health = self.health - amount
}

function takeDamage(lend self: Warrior, amount: Int) -> Nothing {
  self.health = self.health - (amount - self.armor)
}
// COMPILE ERROR: A function named `takeDamage` is already defined in this file.
// Rename one of the two functions — each function in a file must have a unique name.
// Yinz does not allow two functions with the same name in the same file.
```

---

## base shapes — never created directly

Mark a shape `base` if it exists only to be extended and should never be created directly:

```
base shape Vehicle {
  speed: Int
}

shape Car extends Vehicle {
  brand: String
}

let vehicle: Vehicle = { speed: 50 }
// COMPILE ERROR: `Vehicle` is a `base shape` and cannot be constructed directly.
// Create a shape that extends `Vehicle`, then construct that instead.
// `base shape` declarations are meant to be extended — they provide shared fields for child shapes but cannot be instantiated on their own.

let car: Car = { speed: 120, brand: `Ford` }    // fine
```

---

## follows — behavior contracts

A contract is a shape that lists function signatures with no `function` keyword and no body. A shape that `follows` the contract must have a matching standalone function for every signature.

```
shape Damageable {
  takeDamage(lend self, amount: Int) -> Nothing      // bare signature — no body
}

shape Player follows Damageable {
  name: String
  health: Int
}

// A standalone function supplies the behavior.
// The compiler checks that it matches Damageable's signature.
function takeDamage(lend self: Player, amount: Int) -> Nothing {
  self.health = self.health - amount
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Patrick`, health: 100 }
  player.takeDamage(50)
}
```

Writing `follows` makes the compiler report a missing function where you declare the shape:

```
shape Circle follows Drawable {
  radius: Int
}
// COMPILE ERROR: `Circle` follows `Drawable` but is missing function `draw`.
// Add `function draw(share self: Circle) -> ...` to this file.
// `Drawable` requires a function named `draw` — define it as a standalone function whose first parameter is `self: Circle`.
```

A shape can follow several contracts — `extends` comes first, then `follows` with a comma-separated list:

```
shape Warrior extends Entity follows Damageable, Attackable {
  weapon: String
  armor: Int
}
// Warrior must have standalone functions matching every signature in
// Damageable and Attackable.
```

---

## Matching by fields

You don't have to name the shape when you return a `{ ... }` value. Its fields only have to match the declared return type:

```
shape DivResult {
  quotient: Int
  remainder: Int
}

function divmod(a: Int, b: Int) -> DivResult {
  return { quotient: a / b, remainder: a % b }    // matches DivResult — valid
}
```

The compiler checks that the fields exist and have the right types.

---

## Hidden fields — invisible outside the shape's own functions

Mark a field `hidden` to keep outside code from reading it. A hidden field needs a default value, because code in other files can't provide one when it creates the shape. The default must match the field's type (`hidden note: String = 4096` is refused), and every new value starts with it, wherever the shape is created.

```
// File: player.ynz
export shape Player {
  name: String
  health: Int
  hidden bonus: Int = 5
}

// A function whose first parameter is `self: Player` CAN touch hidden fields
export function takeDamage(lend self: Player, amount: Int) -> Nothing {
  let actual = amount - self.bonus
  self.health = self.health - actual
}
```

Code in other files only provides the visible fields when it creates the value:

```
// File: entrypoint.ynz
import { Player, takeDamage } from `player`

function entrypoint() -> Nothing {
  let player: Player = { name: `Alice`, health: 100 }
  // bonus starts at 5 (the default from player.ynz)

  takeDamage(player, 25)    // allowed — takeDamage is exported
  print(player.bonus)
  // COMPILE ERROR: `bonus` is a hidden field of `Player` and cannot be read here.
  // Move this access inside a function whose first parameter is `self: Player`.
  // Hidden fields are only accessible to functions that explicitly operate on that shape — they cannot be read by outside code.
}
```

Setting a hidden field when you create a value in another file is also an error:

```
let player: Player = { name: `Alice`, health: 100, bonus: 50 }
// COMPILE ERROR: `bonus` is hidden — code in this file cannot set it at construction.
// Remove `bonus: <value>` from the shape value. The default from `player.ynz` will be used. To customize the value, expose a setter from the declaring file: `function withBonus(lend self: Player, v: Int) -> Nothing { self.bonus = v }` — then call that function instead.
// Hidden fields are file-private (the spec calls this `visibility, not mutability`). Allowing external construction to set them would bypass whatever invariants the declaring file's functions maintain.
```

`hidden` controls who can see a field, not whether it can change. You can export a shape and still keep some fields private.

---

## Inline shapes (shapes without a name)

If a shape is used in only one place, you can write it right where you use it instead of scrolling up to a `shape` declaration at the top of the file.

```
// Inline shape — right for a one-off config that never leaves this block
let intervals: Fixed<{ minutes: Int }> = [
  { minutes: 5 },
  { minutes: 15 },
  { minutes: 60 }
]
```

`{` in a type annotation always means an inline shape. No extra keyword needed.

### Where inline shapes can appear

Inline shapes work in any type-annotation position:

```
// Variable
let p: { x: Int, y: Int } = { x: 3, y: 4 }

// Function parameter
function area(rect: { w: Int, h: Int }) -> Int {
  return rect.w * rect.h
}

// Inside a generic type
let steps: Fixed<{ minutes: Int }> = [{ minutes: 5 }, { minutes: 15 }]

// Field type inside a named shape
shape Bar {
  symbol: String
  spread: {
    bid: Int
    ask: Int
  }
}
```

### Accessing fields

Field access works the same as for named shapes:

```
let p: { x: Int, y: Int } = { x: 3, y: 4 }
let total = p.x + p.y
print(total)    // prints 7
```

### Same fields means same type

Two inline shapes with the same fields are the same type, wherever they appear and in whatever order the fields are listed:

```
function takesAB(p: { a: Int, b: Int }) -> Nothing {
  print(p.a)
}

function entrypoint() -> Nothing {
  let x: { b: Int, a: Int } = { a: 1, b: 2 }
  takesAB(x)  // OK — same fields, same type
}
```

### Inline vs named: which to use

Use an inline shape when it is truly one-off — written and used in the same small block. Use a named shape when:

- The same type appears in more than one function or file
- You want to give the type a meaningful name for readability
- You need to use `extends`, `follows`, or `hidden` fields

```
// Inline — fine when only this loop uses it
for ({ minutes } in intervals) {
  print(minutes)
}
```

### Restrictions

- **No `hidden` fields.** Hidden fields are file-private and require a named `shape` declaration.
- **No `extends` or `follows`.** Inline shapes hold plain data only, so they can't share fields with a parent or follow a contract.
- Named shapes and inline shapes with identical fields are **different types** and do not convert to each other. If you annotate a variable `let x: Foo`, you cannot assign an `{ a: Int }` value to it even if `Foo` has exactly field `a: Int`.
