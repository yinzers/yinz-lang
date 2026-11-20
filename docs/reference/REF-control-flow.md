---
name: "REF-control-flow"
description: "Two forms of if. Loops. Early returns. That's it."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Control Flow

Two forms of `if`. Loops. Early returns. That's it.

---

## Simple `if` — conditions and early returns

```
if (player.health <= 0) {
  return
}

if (player.banned) {
  return
}
```

Parentheses around the condition are required. Curly braces always required.

There is no `else` block. Instead, handle one case and return, then let the code carry on for the other case:

```
// Return early
function describe(share player: Player) -> string {
  if (player.health <= 0) {
    return `dead`
  }
  return `alive`
}
```

When you can't return (you are not in a function, or you need to keep going after both cases), set a starting value first and change it inside the `if`:

```
// Set a starting value, then change it
let message = `in danger`
if (player.health > 50) {
  message = `healthy`
}
print(message)
```

Two patterns, zero `else` blocks. Your code stays flat and easy to read.

---

## Early returns — the main way to handle branches

Handle the unusual cases first and return. The normal path then stays at the left edge with no extra indenting:

```
function processPlayer(lend player: Player) -> nothing {
  if (player.health <= 0) {
    return
  }

  if (player.banned) {
    return
  }

  player.score = player.score + 10
}
```

---

## Boolean operators

```
if (player.active && player.health > 0) {
  // both must be true
}

if (player.banned || player.suspended) {
  // either can be true
}

if (!player.active) {
  // negation
}

// Combined
if (active && (isAdmin || isModerator) && !banned) {
  grantFullAccess()
}
```

Boolean conditions use three symbols:

| Symbol | Meaning |
|--------|---------|
| `&&` | and: both sides must be true |
| `\|\|` | or: at least one side must be true |
| `!` | not: flips true to false and false to true |

A single `|` is for union types (`shape Shape = Circle | Square`), not for "or". In conditions, use `||` for "or".

---

## Multi-case `if` — compare one value to several cases

When you want to compare one value against several cases, use a multi-case `if`, with `=>` arrows inside the block. There is no `switch` and no `match`. It is the same `if` keyword in a different form.

The compiler tells them apart: `=>` inside the block means multi-case.

**Comparing `options` values:**

```
if (status) {
  active => print(`online`)
  inactive => print(`offline`)
  banned => print(`banned`)
}
```

**Comparing the kinds in a union — `is` tells the compiler which kind it is:**

```
if (shape) {
  is Circle => return math.PI * shape.radius * shape.radius
  is Square => return shape.side * shape.side
  is Triangle => return (shape.base * shape.height) / 2
}
```

After `is Circle`, the compiler knows `shape` is a `Circle`. Use `.radius` directly — no conversion and no `.value`.

**Comparing plain values — use `else =>` for everything else:**

```
if (statusCode) {
  200 => print(`ok`)
  404 => print(`not found`)
  500 => print(`server error`)
  else => print(`unexpected: ${statusCode}`)
}
```

`else =>` is only valid inside multi-case `if` blocks. It is never a standalone `else { }` block.

**A case with several lines — use curly braces:**

```
if (shape) {
  is Circle => {
    let radius = shape.radius
    let area = math.PI * radius * radius
    return area
  }
  is Square => {
    return shape.side * shape.side
  }
  is Triangle => {
    return (shape.base * shape.height) / 2
  }
}
```

---

## Every case must be handled — for options and unions

For `options` and for unions, the compiler checks that you handled every case:

```
if (shape) {
  is Circle => return circleArea(shape)
  is Square => return squareArea(shape)
  // COMPILE ERROR: not all Shape variants handled. Missing: Triangle.
  // Add a Triangle case or add an else => catch-all.
}
```

For plain values (numbers, strings), the compiler can't list every possible value, so it doesn't check. Use `else =>` to cover everything else.

---

## for — loop over a collection

```
for (player in players) {
  print(player.name)
}
```

`player` is a new variable that exists only inside the loop. The compiler works out its type from the collection.

---

## while — loop until a condition is false

```
let health = 100
while (health > 0) {
  health = health - 10
}
```
