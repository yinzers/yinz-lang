---
name: "REF-control-flow"
description: "Two forms of if. Loops. Early returns. That's it."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Control Flow

Two forms of `if`. Loops. Early returns. That's it.

The examples on this page use this shape:

```
shape Player {
  name: String
  health: Int
  score: Int
  active: Boolean
  banned: Boolean
}
```

---

## Simple `if` — conditions and early returns

```
function check(lend player: Player) -> Nothing {
  if (player.health <= 0) {
    return
  }

  if (player.banned) {
    return
  }
}
```

Parentheses around the condition are required. Curly braces always required.

There is no `else` block. Instead, handle one case and return, then let the code carry on for the other case:

```
// Return early
function describe(share player: Player) -> String {
  if (player.health <= 0) {
    return `dead`
  }
  return `alive`
}
```

When you can't return (you need to keep going after both cases), set a starting value first and change it inside the `if`:

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
function processPlayer(lend player: Player) -> Nothing {
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

if (player.banned || !player.active) {
  // either can be true
}

if (!player.active) {
  // negation
}

// Combined
if (player.active && (player.score > 100 || player.health > 50) && !player.banned) {
  print(`full access`)
}
```

Boolean conditions use three symbols:

| Symbol | Meaning |
|--------|---------|
| `&&` | and: both sides must be true |
| `\|\|` | or: at least one side must be true |
| `!` | not: flips true to false and false to true |

A single `|` is for union types (`shape Figure = Circle | Square`), not for "or". In conditions, use `||` for "or".

---

## Multi-case `if` — compare one value to several cases

When you want to compare one value against several cases, use a multi-case `if`, with `=>` arrows inside the block. It is the same `if` keyword in a different form.

The compiler tells them apart: `=>` inside the block means multi-case.

**Comparing `options` values:**

```
options Status { active, inactive, banned }

function show(status: Status) -> Nothing {
  if (status) {
    active => print(`online`)
    inactive => print(`offline`)
    banned => print(`banned`)
  }
}
```

**Comparing the kinds in a union — `is` tells the compiler which kind it is:**

```
shape Circle {
  radius: Float
}

shape Square {
  side: Float
}

shape Figure = Circle | Square

function getArea(share figure: Figure) -> Float {
  if (figure) {
    is Circle => return figure.radius * figure.radius * 3.14159
    is Square => return figure.side * figure.side
  }
  return 0.0
}
```

After `is Circle`, the compiler knows `figure` is a `Circle`. Use `.radius` directly — no conversion and no `.value`. A function that returns a value needs a `return` after the multi-case `if`. See [Union Types](REF-unions.md).

**Comparing plain values — use `else =>` for everything else:**

```
function report(statusCode: Int) -> Nothing {
  if (statusCode) {
    200 => print(`ok`)
    404 => print(`not found`)
    500 => print(`server error`)
    else => print(`unexpected: ${statusCode}`)
  }
}
```

`else =>` is only valid inside multi-case `if` blocks. It is never a standalone `else { }` block.

**A case with several lines — use curly braces:**

```
function getSide(share figure: Figure) -> Float {
  if (figure) {
    is Circle => {
      let radius = figure.radius
      return radius * 2.0
    }
    is Square => {
      return figure.side
    }
  }
  return 0.0
}
```

---

## Every case must be handled — for options and unions

For `options` and for unions, the compiler checks that you handled every case:

```
function sideCount(share figure: Figure) -> Int {
  if (figure) {
    is Circle => return 0
  }
  return -1
}
// COMPILE ERROR: Non-exhaustive union multi-case — 1 variant is not handled: Square.
// Add the missing arms (e.g. `is Square =>`) or add an `else =>` default arm.
// The compiler knows every union variant at compile time. A missing arm means some values silently fall through — likely a bug.
```

For `options`, the error names the options type and the missing cases:

```
function show(status: Status) -> Nothing {
  if (status) {
    active => print(`online`)
  }
}
// COMPILE ERROR: Non-exhaustive options multi-case — `Status` has 3 variants; 2 are not handled: inactive, banned.
// Add the missing arms (e.g. `inactive =>`) or add an `else =>` default arm.
// The compiler knows every variant at compile time. A missing arm means some values would silently fall through — likely a bug.
```

For plain values (numbers, strings), the compiler can't list every possible value, so it doesn't check. Use `else =>` to cover everything else.

---

## for — loop over a collection

```
let names: Array<String> = [`Alice`, `Bob`]
for (name in names) {
  print(name)
}
```

`name` is a new variable that exists only inside the loop. The compiler works out its type from the collection.

---

## while — loop until a condition is false

```
let health = 100
while (health > 0) {
  health = health - 10
}
```
