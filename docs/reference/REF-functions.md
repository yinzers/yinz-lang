---
name: "REF-functions"
description: "Functions are blocks of code that do one job and return a result."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Functions

Functions are blocks of code that do one job and return a result.

---

## Defining a function

```
function add(a: Number, b: Number) -> Number {
  return a + b
}
```

Every function:
- Starts with `function`
- Lists its parameters with their types
- Declares what it returns with `->`
- Always has a return type — no exceptions

---

## Return types are always required

The compiler enforces that every code path actually returns the declared type:

```
function getScore(share player: Player) -> Number {
  return player.score
}
```

---

## Nothing — functions that don't return a value

If a function does something but gives nothing back, say `-> Nothing`:

```
function logMessage(share msg: String) -> Nothing {
  print(msg)
}
```

`Nothing` reads like English. The function runs and returns... nothing.

Using `return` inside a `Nothing` function is fine for early exits:

```
function processPlayer(lend player: Player) -> Nothing {
  if (player.health <= 0) {
    return    // early exit — done
  }
  player.health = player.health - 10
}
```

Returning a value from a `Nothing` function is a compile error:

```
function logMessage(share msg: String) -> Nothing {
  return msg
}
// COMPILE ERROR: `return` with a value in a `-> Nothing` function.
//   Remove the value: write `return` with no expression.
//   Functions declared `-> Nothing` do not produce a value. A `return` inside them ends the function — no value allowed.
```

---

## Parameters — share, lend, give

Every parameter says how the function uses the value it receives. See [Ownership](REF-ownership.md) for the full picture.

```
function greet(share name: String) -> Nothing      // just reading name
function rename(lend player: Player) -> Nothing    // going to modify player
function consume(give data: Data) -> Nothing       // taking ownership
```

The caller never types the ownership modifier — the compiler infers it from the function's signature. The IDE shows the inferred modifier as muted text:

```ynz
greet(playerName)    // IDE shows muted "share" — read-only access inferred
rename(player)       // IDE shows muted "lend" (red-tinted) — function modifies player
consume(data)        // IDE shows muted "give" (red-tinted) — function takes ownership
```

There is no body-level syntax for `.share()` / `.lend()` / `.give()` — those modifiers live only in function signatures. See [Ownership](REF-ownership.md).

---

## Every function has a name

Write each function with `function` and a name. To use one function inside another, call it by name.

---

## No tuples — define a shape instead

If you need to return multiple values, define a shape. Named fields are always clearer than positional slots:

```
shape DivResult {
  quotient: Number
  remainder: Number
}

function divmod(a: Number, b: Number) -> DivResult {
  return { quotient: a / b, remainder: a % b }
}
```
