---
name: "REF-variables"
description: "Variables store values. Declare them with let or const."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Variables

Variables store values. Declare them with `let` or `const`.

---

## let — a variable you can change

```
let name: String = `Patrick`
let health: Number = 100.0
let score: Number = 0.0
```

You can give a `let` variable a new value after you create it:

```
let health: Number = 100.0
health = health - 25.0    // fine — health is now 75.0
```

---

## const — a variable that never changes

```
const maxPlayers: Number = 10.0
const serverName: String = `prod-01`
```

You cannot give a `const` a new value after you create it:

```
const maxPlayers: Number = 10.0
maxPlayers = 20.0
// COMPILE ERROR: `maxPlayers` cannot be changed — it was declared with `const`.
// Change `const maxPlayers = ...` to `let maxPlayers = ...` if you need to reassign it.
// `const` declares a value that never changes. Use `let` when the value needs to be updated.
```

Use `const` by default. Reach for `let` only when you need to change the value later.

### What const blocks — the full picture

`const` means "this value never changes, in any way." That covers:

- **Giving it a new value**: `maxPlayers = 20.0` — blocked (the example above)
- **Changing a field**: `player.health = 50` on a `const player` is blocked
- **Letting a function change it**: you can't pass a `const` value to a function whose signature declares `lend` or `give`, because those words let the function change or take the value, and the compiler won't allow that

```ynz
shape Player {
  name: String
  health: Int
}

function healPlayer(lend player: Player) -> Nothing {
  player.health = 100
}

function entrypoint() -> Nothing {
  const player: Player = { name: `Patrick`, health: 100 }

  player.health = 50
  // COMPILE ERROR: `player` is `const` and its fields cannot be changed.
  // Declare it with `let` instead: `let player: ShapeType = { ... }`
  // `const` bindings are fully read-only — no reassignment, no field mutation. Use `let` for values that need to change.

  healPlayer(player)
  // COMPILE ERROR: `player` is `const` — `healPlayer` needs to mutate it but `const` blocks mutation.
  // Declare `player` with `let` if you need `healPlayer` to modify it.
  // `const` bindings cannot be lent for mutation. The `lend` modifier means the function will write to the value.
}
```

The compiler rejects all three at compile time. `const` is a complete promise — no exceptions.

`const` and ownership work together to keep your program safe and fast. See [Ownership](REF-ownership.md) for the full picture.

---

## Types the compiler fills in

Inside a function body, you don't have to write a type when it is obvious from the value:

```
let playerCount = 42    // the compiler knows: Int
let name = `Patrick`    // the compiler knows: String
let active = true       // the compiler knows: Boolean
```

The IDE still shows the type when you hover. You just don't have to write it.

You must write types at the edges of a function: its parameters, its return type, and shape definitions. Inside the body you can leave them out:

```
function addScores(firstScore: Number, secondScore: Number) -> Number {
  let total = firstScore + secondScore    // no type needed — fine
  return total
}
```

---

## Every variable must start with a value

You cannot declare a variable without giving it a value:

```
let name: String
// COMPILE ERROR: `name` declaration is missing `=`.
// Write: `let name = value`
// Variable declarations must assign an initial value.
```

If a value might not exist, use a `Maybe` type — see [Maybe Types](REF-maybe.md).
