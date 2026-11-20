---
name: "REF-destructuring"
description: "Pull fields out of a type into named variables in one line."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Destructuring

Pull fields out of a type into named variables in one line.

---

## Basic destructuring

```
let player: Player = { name: "Alice", health: 100, score: 50 }

let { name, health } = player
print(name)    // "Alice"
print(health)  // 100
```

The compiler knows the type of `player` at compile time, so it knows every field is guaranteed to exist. No runtime risk.

---

## Rename while destructuring — `as`

```
let { name, health as hp } = player
print(hp)    // 100 — same value, different variable name
```

Use `as` when the field name would conflict with an existing variable or when a shorter name reads better.

---

## Nested destructuring

```
shape Position { x: number, y: number }
shape Player { name: string, health: number, position: Position }

let { name, position: { x, y } } = player
print(x)    // player.position.x
print(y)    // player.position.y
```

---

## In function parameters

Destructure directly in the parameter list instead of assigning inside the function body:

```
function greet({ name, health }: Player) -> string {
  return `${name} has ${health} HP`
}

// Calling it works the same way
greet(player)
```

---

## From function returns

```
let { quotient, remainder } = divmod(10, 3)
print(quotient)   // 3
print(remainder)  // 1
```

---

## No array destructuring

Array indices are not compile-time guaranteed. Destructuring would bypass the `.get()` safety check. Use `.get()` instead:

```
// NOT allowed
let [first, second] = items
// COMPILE ERROR: Array destructuring is not allowed. Use .get() for safe index access.

// Safe — .get() returns maybe
let foundFirst = items.get(0)             // a Maybe: the item, or nothing
let first = foundFirst.or(defaultItem)    // a plain item, defaultItem if missing
let foundSecond = items.get(1)
let second = foundSecond.or(defaultItem)
```

Object fields are guaranteed by the type system. Array indices are not. Different safety profile, different access pattern.
