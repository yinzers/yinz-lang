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
let name: string = `Patrick`
let health: number = 100
let score: number = 0
```

You can give a `let` variable a new value after you create it:

```
let health: number = 100
health = health - 25    // fine — health is now 75
```

---

## const — a variable that never changes

```
const maxPlayers: number = 10
const serverName: string = `prod-01`
```

You cannot give a `const` a new value after you create it:

```
const maxPlayers: number = 10
maxPlayers = 20
// COMPILE ERROR: maxPlayers is const and cannot be reassigned.
```

Use `const` by default. Reach for `let` only when you need to change the value later.

### What const blocks — the full picture

`const` means "this value never changes, in any way." That covers:

- **Giving it a new value**: `maxPlayers = 20` — blocked (the example above)
- **Changing a field**: `player.health = 50` on a `const player` is blocked
- **Letting a function change it**: you can't pass a `const` value to a function whose signature declares `lend` or `give`, because those words let the function change or take the value, and the compiler won't allow that

```ynz
const player: Player = { name: `Patrick`, health: 100 }
player.health = 50            // COMPILE ERROR: player is const — fields can't change.
healPlayer(player)            // COMPILE ERROR: player is const — healPlayer's signature is
                              //   `lend player: Player` which needs mutable access.
                              //   Declare with `let` if the function needs to modify it.
```

The compiler rejects all three at compile time. `const` is a complete promise — no exceptions.

`const` and ownership work together to keep your program safe and fast. See [Ownership](REF-ownership.md) for the full picture.

---

## Types the compiler fills in

Inside a function body, you don't have to write a type when it is obvious from the value:

```
let x = 42              // the compiler knows: int
let name = `Patrick`    // the compiler knows: string
let active = true       // the compiler knows: boolean
```

The IDE still shows the type when you hover. You just don't have to write it.

You must write types at the edges of a function: its parameters, its return type, and shape definitions. Inside the body you can leave them out:

```
function add(a: number, b: number) -> number {
  let result = a + b    // no type needed — fine
  return result
}
```

---

## Every variable must start with a value

You cannot declare a variable without giving it a value:

```
let name: string
// COMPILE ERROR: name has no value.
// Every variable must be initialized. Use maybe<string> if the value is optional.
```

If a value might not exist, use a `maybe` type — see [Maybe Types](REF-maybe.md).
