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

Most examples on this page use this shape:

```ynz
shape Player {
  name: String
  health: Int
  score: Int
  active: Boolean
  banned: Boolean
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Ana`, health: 80, score: 120, active: true, banned: false }
  print(player.name)
}
```

Output:

```text
Ana
```

---

## Simple `if` — conditions and early returns

```ynz
shape Player {
  name: String
  health: Int
  score: Int
  active: Boolean
  banned: Boolean
}

function check(lend player: Player) -> Nothing {
  if (player.health <= 0) {
    return
  }

  if (player.banned) {
    return
  }

  print(`${player.name} may play`)
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Ana`, health: 80, score: 120, active: true, banned: false }
  check(player)
}
```

Output:

```text
Ana may play
```

Parentheses around the condition are required. Curly braces always required.

There is no `else` block. Instead, handle one case and return, then let the code carry on for the other case:

```ynz
shape Player {
  name: String
  health: Int
  score: Int
  active: Boolean
  banned: Boolean
}

// Return early
function describe(share player: Player) -> String {
  if (player.health <= 0) {
    return `dead`
  }
  return `alive`
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Ana`, health: 80, score: 120, active: true, banned: false }
  print(describe(player))
}
```

Output:

```text
alive
```

When you can't return (you need to keep going after both cases), set a starting value first and change it inside the `if`:

```ynz
shape Player {
  name: String
  health: Int
  score: Int
  active: Boolean
  banned: Boolean
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Ana`, health: 80, score: 120, active: true, banned: false }

  // Set a starting value, then change it
  let message = `in danger`
  if (player.health > 50) {
    message = `healthy`
  }
  print(message)
}
```

Output:

```text
healthy
```

Two patterns, zero `else` blocks. Your code stays flat and easy to read.

---

## Early returns — the main way to handle branches

Handle the unusual cases first and return. The normal path then stays at the left edge with no extra indenting:

```ynz
shape Player {
  name: String
  health: Int
  score: Int
  active: Boolean
  banned: Boolean
}

function processPlayer(lend player: Player) -> Nothing {
  if (player.health <= 0) {
    return
  }

  if (player.banned) {
    return
  }

  player.score = player.score + 10
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Ana`, health: 80, score: 120, active: true, banned: false }
  processPlayer(player)
  print(player.score)
}
```

Output:

```text
130
```

---

## Boolean operators

```ynz
shape Player {
  name: String
  health: Int
  score: Int
  active: Boolean
  banned: Boolean
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Ana`, health: 80, score: 120, active: true, banned: false }

  if (player.active && player.health > 0) {
    print(`both must be true`)
  }

  if (player.banned || !player.active) {
    print(`either can be true`)
  }

  if (!player.active) {
    print(`negation`)
  }

  // Combined
  if (player.active && (player.score > 100 || player.health > 50) && !player.banned) {
    print(`full access`)
  }
}
```

Output:

```text
both must be true
full access
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

```ynz
options Status { active, inactive, banned }

function entrypoint() -> Nothing {
  let status: Status = Status.inactive
  if (status) {
    active => print(`online`)
    inactive => print(`offline`)
    banned => print(`banned`)
  }
}
```

Output:

```text
offline
```

**Comparing the kinds in a union — `is` tells the compiler which kind it is:**

```ynz
shape Circle {
  radius: Number
}

shape Square {
  side: Number
}

shape Figure = Circle | Square

function getArea(share figure: Figure) -> Number {
  if (figure) {
    is Circle => return figure.radius * figure.radius * 3.14159
    is Square => return figure.side * figure.side
  }
  return 0.0
}

function entrypoint() -> Nothing {
  let square: Square = { side: 3.0 }
  let figure: Figure = square
  print(getArea(figure))
}
```

Output:

```text
9.00
```

After `is Circle`, the compiler knows `figure` is a `Circle`. Use `.radius` directly — no conversion and no `.value`. A function that returns a value needs a `return` after the multi-case `if`. See [Union Types](REF-unions.md).

**Comparing plain values — use `else =>` for everything else:**

```ynz
function report(statusCode: Int) -> Nothing {
  if (statusCode) {
    200 => print(`ok`)
    404 => print(`not found`)
    500 => print(`server error`)
    else => print(`unexpected: ${statusCode}`)
  }
}

function entrypoint() -> Nothing {
  report(404)
  report(418)
}
```

Output:

```text
not found
unexpected: 418
```

`else =>` is only valid inside multi-case `if` blocks. It is never a standalone `else { }` block.

**A case with several lines — use curly braces:**

```ynz
shape Circle {
  radius: Number
}

shape Square {
  side: Number
}

shape Figure = Circle | Square

function getSide(share figure: Figure) -> Number {
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

function entrypoint() -> Nothing {
  let circle: Circle = { radius: 1.5 }
  let figure: Figure = circle
  print(getSide(figure))
}
```

Output:

```text
3.00
```

---

## Every case must be handled — for options and unions

For `options` and for unions, the compiler checks that you handled every case:

```ynz
shape Circle {
  radius: Number
}

shape Square {
  side: Number
}

shape Figure = Circle | Square

function sideCount(share figure: Figure) -> Int {
  if (figure) {
    is Circle => return 0
  }
  return -1
}

function entrypoint() -> Nothing {
  let square: Square = { side: 1.0 }
  let figure: Figure = square
  print(sideCount(figure))
}
```

The compiler stops with:

```text
Error: Non-exhaustive union multi-case — 1 variant is not handled: Square.
    ╭─[ main.ynz:12:7 ]
    │
 12 │   if (figure) {
    │       ───┬──
    │          ╰──── Add the missing arms (e.g. `is Square =>`) or add an `else =>` default arm.
    │
    │ Note: The compiler knows every union variant at compile time. A missing arm means some values silently fall through — likely a bug.
────╯
```

For `options`, the error names the options type and the missing cases:

```ynz
options Status { active, inactive, banned }

function entrypoint() -> Nothing {
  let status: Status = Status.active
  if (status) {
    active => print(`online`)
  }
}
```

The compiler stops with:

```text
Error: Non-exhaustive options multi-case — `Status` has 3 variants; 2 are not handled: inactive, banned.
   ╭─[ main.ynz:5:7 ]
   │
 5 │   if (status) {
   │       ───┬──
   │          ╰──── Add the missing arms (e.g. `inactive =>`) or add an `else =>` default arm.
   │
   │ Note: The compiler knows every variant at compile time. A missing arm means some values would silently fall through — likely a bug.
───╯
```

For plain values (numbers, strings), the compiler can't list every possible value, so it doesn't check. Use `else =>` to cover everything else.

---

## for — loop over a collection

```ynz
function entrypoint() -> Nothing {
  let names: Array<String> = [`Alice`, `Bob`]
  for (name in names) {
    print(name)
  }
}
```

Output:

```text
Alice
Bob
```

`name` is a new variable that exists only inside the loop. The compiler works out its type from the collection.

---

## while — loop until a condition is false

```ynz
function entrypoint() -> Nothing {
  let health = 100
  while (health > 0) {
    health = health - 40
    print(health)
  }
}
```

Output:

```text
60
20
-20
```
