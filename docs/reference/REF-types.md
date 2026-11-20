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

```ynz
shape Player {
  name: String
  health: Int
  score: Int
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Patrick`, health: 100, score: 0 }
  print(player.name)
}
```

Output:

```text
Patrick
```

The fields sit next to each other in memory, so reading them is fast.

Write one field per line.

---

## Creating a shape value

Write the variable's type, then assign a `{ ... }` value:

```ynz
shape Player {
  name: String
  health: Int
  score: Int
}

function entrypoint() -> Nothing {
  const player: Player = {
    name: `Patrick`,
    health: 100,
    score: 0
  }
  print(player.health)
}
```

Output:

```text
100
```

Every field must be provided. Forgetting one is a compile error:

```ynz
shape Player {
  name: String
  health: Int
  score: Int
}

function entrypoint() -> Nothing {
  const player: Player = {
    name: `Patrick`,
    health: 100
  }
  print(player.health)
}
```

The compiler stops with:

```text
Error: Missing field `score` in `Player` construction.
    ╭─[ main.ynz:8:26 ]
    │
  8 │ ╭─▶   const player: Player = {
    ┆ ┆
 11 │ ├─▶   }
    │ │
    │ ╰───────── Add `score: value` to the shape value.
    │
    │     Note: Every visible field of a shape must be provided when constructing a value — the compiler cannot fill them in for you.
────╯
```

Writing the name before the braces, as in `let p = Player { ... }`, is NOT allowed. The type you write on the variable tells the compiler which shape the `{ ... }` value must match.

---

## Functions for a shape are standalone

In Yinz, the functions that work on a shape are normal `function` declarations at the top level of a file or module. They take the value as the first parameter (usually named `self`). When you call one, you can write either `value.method()` (dot form) or `method(value)` (plain form). Both are legal and mean the same thing.

```ynz
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
  print(player.health)
}
```

Output:

```text
true
50
```

The `lend self: Player` parameter says "this function changes a Player." `share self: Player` says "this function only reads it." See [REF-ownership.md](REF-ownership.md).

The IDE understands both forms. Typing `player.` shows every function whose first parameter is a `Player`, so autocomplete helps you discover functions even though they are not stored inside the shape.

---

## Extending shapes — sharing fields

`extends` reuses the parent's FIELDS. Behavior comes from standalone functions, and each function name must be **unique in its file**. Give the Warrior's behavior its own name instead of reusing the parent's function name:

```ynz
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
  print(warrior.health)    // damage applied: 20 - 15 = 5
}
```

Output:

```text
95
```

A shape can `extends` only one other shape. The fields are reused, but each shape's own behavior needs its own function name.

There is no `override` keyword in Yinz. Two `function` declarations with the same name in the same file are a compile error, even when their first parameters have different types:

```ynz
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

function takeDamage(lend self: Warrior, amount: Int) -> Nothing {
  self.health = self.health - (amount - self.armor)
}

function entrypoint() -> Nothing {
  print(`unreachable`)
}
```

The compiler stops with:

```text
Error: A function named `takeDamage` is already defined in this file.
    ╭─[ main.ynz:15:1 ]
    │
 15 │ ╭─▶ function takeDamage(lend self: Warrior, amount: Int) -> Nothing {
    ┆ ┆
 19 │ ├─▶ function entrypoint() -> Nothing {
    │ │
    │ ╰──────────────────────────────────────── Rename one of the two functions — each function in a file must have a unique name.
    │
    ├─[ main.ynz:15:1 ]
    │
 11 │ ╭─▶ function takeDamage(lend self: Entity, amount: Int) -> Nothing {
    ┆ ┆
 15 │ ├─▶ function takeDamage(lend self: Warrior, amount: Int) -> Nothing {
    │ │
    │ ╰─────────────────────────────────────────────────────────────────────── first definition here
    │
    │     Note: Yinz does not allow two functions with the same name in the same file.
────╯
```

---

## base shapes — never created directly

Mark a shape `base` if it exists only to be extended and should never be created directly:

```ynz
base shape Vehicle {
  speed: Int
}

shape Car extends Vehicle {
  brand: String
}

function entrypoint() -> Nothing {
  let car: Car = { speed: 120, brand: `Ford` }    // fine
  print(car.brand)
}
```

Output:

```text
Ford
```

Creating the base shape itself is a compile error:

```ynz
base shape Vehicle {
  speed: Int
}

function entrypoint() -> Nothing {
  let vehicle: Vehicle = { speed: 50 }
  print(vehicle.speed)
}
```

The compiler stops with:

```text
Error: `Vehicle` is a `base shape` and cannot be constructed directly.
   ╭─[ main.ynz:6:26 ]
   │
 6 │   let vehicle: Vehicle = { speed: 50 }
   │                          ──────┬──────
   │                                ╰──────── Create a shape that extends `Vehicle`, then construct that instead.
   │
   │ Note: `base shape` declarations are meant to be extended — they provide shared fields for child shapes but cannot be instantiated on their own.
───╯
```

---

## follows — behavior contracts

A contract is a shape that lists function signatures with no `function` keyword and no body. A shape that `follows` the contract must have a matching standalone function for every signature.

```ynz
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
  print(player.health)
}
```

Output:

```text
50
```

Writing `follows` makes the compiler report a missing function where you declare the shape:

```ynz
shape Drawable {
  draw(share self) -> Nothing
}

shape Circle follows Drawable {
  radius: Int
}

function entrypoint() -> Nothing {
  let circle: Circle = { radius: 4 }
  print(circle.radius)
}
```

The compiler stops with:

```text
Error: `Circle` follows `Drawable` but is missing function `draw`.
   ╭─[ main.ynz:5:7 ]
   │
 5 │ shape Circle follows Drawable {
   │       ───┬──
   │          ╰──── Add `function draw(share self: Circle) -> ...` to this file.
   │
   │ Note: `Drawable` requires a function named `draw` — define it as a standalone function whose first parameter is `self: Circle`.
───╯
```

A shape can follow several contracts — `extends` comes first, then `follows` with a comma-separated list:

```ynz
shape Entity {
  name: String
  health: Int
}

shape Damageable {
  takeDamage(lend self, amount: Int) -> Nothing
}

shape Attackable {
  attack(share self) -> Int
}

shape Warrior extends Entity follows Damageable, Attackable {
  weapon: String
  armor: Int
}

// Warrior must have standalone functions matching every signature in
// Damageable and Attackable.
function takeDamage(lend self: Warrior, amount: Int) -> Nothing {
  self.health = self.health - amount
}

function attack(share self: Warrior) -> Int {
  return 10
}

function entrypoint() -> Nothing {
  let warrior: Warrior = { name: `Patrick`, health: 100, weapon: `sword`, armor: 15 }
  warrior.takeDamage(30)
  print(warrior.health)
  print(warrior.attack())
}
```

Output:

```text
70
10
```

---

## Matching by fields

You don't have to name the shape when you return a `{ ... }` value. Its fields only have to match the declared return type:

```ynz
shape DivResult {
  quotient: Int
  remainder: Int
}

function divmod(a: Int, b: Int) -> DivResult {
  return { quotient: a / b, remainder: a % b }    // matches DivResult — valid
}

function entrypoint() -> Nothing {
  let result = divmod(17, 5)
  print(result.quotient)
  print(result.remainder)
}
```

Output:

```text
3
2
```

The compiler checks that the fields exist and have the right types.

---

## Hidden fields — invisible outside the shape's own functions

Mark a field `hidden` to keep outside code from reading it. A hidden field needs a default value, because code in other files can't provide one when it creates the shape. The default must match the field's type (`hidden note: String = 4096` is refused), and every new value starts with it, wherever the shape is created.

```ynz
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

```ynz
// File: entrypoint.ynz
import { Player, takeDamage } from `player`

function entrypoint() -> Nothing {
  let player: Player = { name: `Alice`, health: 100 }
  // bonus starts at 5 (the default from player.ynz)

  takeDamage(player, 25)    // allowed — takeDamage is exported
  print(player.health)
}
```

Output:

```text
80
```

Reading the hidden field from another file is a compile error:

```ynz
// File: entrypoint.ynz
import { Player, takeDamage } from `player`

function entrypoint() -> Nothing {
  let player: Player = { name: `Alice`, health: 100 }
  takeDamage(player, 25)
  print(player.bonus)
}
```

The compiler stops with:

```text
Error: `bonus` is a hidden field of `Player` and cannot be read here.
   ╭─[ entrypoint.ynz:7:16 ]
   │
 7 │   print(player.bonus)
   │                ──┬──
   │                  ╰──── Move this access inside a function whose first parameter is `self: Player`.
   │
   │ Note: Hidden fields are only accessible to functions that explicitly operate on that shape — they cannot be read by outside code.
───╯
```

Setting a hidden field when you create a value in another file is also an error:

```ynz
// File: entrypoint.ynz
import { Player } from `player`

function entrypoint() -> Nothing {
  let player: Player = { name: `Alice`, health: 100, bonus: 50 }
  print(player.health)
}
```

The compiler stops with:

```text
Error: `bonus` is hidden — code in this file cannot set it at construction.
   ╭─[ entrypoint.ynz:5:54 ]
   │
 5 │   let player: Player = { name: `Alice`, health: 100, bonus: 50 }
   │                                                      ──┬──
   │                                                        ╰──── Remove `bonus: <value>` from the shape value. The default from `player.ynz` will be used. To customize the value, expose a setter from the declaring file: `function withBonus(lend self: Player, v: Int) -> Nothing { self.bonus = v }`— then call that function instead.
   │
   │ Note: Hidden fields are file-private (the spec calls this `visibility, not mutability`). Allowing external construction to set them would bypass whatever invariants the declaring file's functions maintain.
───╯
```

`hidden` controls who can see a field, not whether it can change. You can export a shape and still keep some fields private.

---

## Inline shapes (shapes without a name)

If a shape is used in only one place, you can write it right where you use it instead of scrolling up to a `shape` declaration at the top of the file.

```ynz
function entrypoint() -> Nothing {
  // Inline shape — right for a one-off config that never leaves this block
  let intervals: Fixed<{ minutes: Int }> = [
    { minutes: 5 },
    { minutes: 15 },
    { minutes: 60 }
  ]

  for ({ minutes } in intervals) {
    print(minutes)
  }
}
```

Output:

```text
5
15
60
```

`{` in a type annotation always means an inline shape. No extra keyword needed.

### Where inline shapes can appear

Inline shapes work in a variable annotation, inside a generic type, and as a field type inside a named shape:

```ynz
// Field type inside a named shape
shape Bar {
  symbol: String
  spread: {
    bid: Int
    ask: Int
  }
}

function entrypoint() -> Nothing {
  // Variable
  let p: { x: Int, y: Int } = { x: 3, y: 4 }

  // Inside a generic type
  let steps: Fixed<{ minutes: Int }> = [{ minutes: 5 }, { minutes: 15 }]

  let bar: Bar = { symbol: `ABC`, spread: { bid: 5, ask: 6 } }
  print(p.x)
  print(steps.count())
  print(bar.spread.ask)
}
```

Output:

```text
3
2
6
```

### Accessing fields

Field access works the same as for named shapes:

```ynz
function entrypoint() -> Nothing {
  let p: { x: Int, y: Int } = { x: 3, y: 4 }
  let total = p.x + p.y
  print(total)    // prints 7
}
```

Output:

```text
7
```

### Same fields means same type

Two inline shapes with the same fields are the same type, wherever they appear and in whatever order the fields are listed:

```ynz
function entrypoint() -> Nothing {
  let first: { a: Int, b: Int } = { a: 1, b: 2 }
  let second: { b: Int, a: Int } = first    // OK — same fields, same type
  print(second.a)
}
```

Output:

```text
1
```

### Inline vs named: which to use

Use an inline shape when it is truly one-off — written and used in the same small block. Use a named shape when:

- The same type appears in more than one function or file
- You want to give the type a meaningful name for readability
- You need to use `extends`, `follows`, or `hidden` fields

### Restrictions

- **No `hidden` fields.** Hidden fields are file-private and require a named `shape` declaration.
- **No `extends` or `follows`.** Inline shapes hold plain data only, so they can't share fields with a parent or follow a contract.
- Named shapes and inline shapes with identical fields are **different types** and do not convert to each other. If you annotate a variable `let x: Foo`, you cannot assign an `{ a: Int }` value to it even if `Foo` has exactly field `a: Int`.
