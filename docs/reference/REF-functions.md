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

```ynz
function add(a: Number, b: Number) -> Number {
  return a + b
}

function entrypoint() -> Nothing {
  print(add(2.0, 3.0))
}
```

Output:

```text
5.0
```

Every function:
- Starts with `function`
- Lists its parameters with their types
- Declares what it returns with `->`
- Always has a return type — no exceptions

---

## Return types are always required

The compiler enforces that every code path actually returns the declared type:

```ynz
shape Player {
  name: String
  score: Number
  health: Number
}

function getScore(share player: Player) -> Number {
  return player.score
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Ana`, score: 120.0, health: 100.0 }
  print(getScore(player))
}
```

Output:

```text
120.0
```

---

## Nothing — functions that don't return a value

If a function does something but gives nothing back, say `-> Nothing`:

```ynz
function logMessage(share message: String) -> Nothing {
  print(message)
}

function entrypoint() -> Nothing {
  logMessage(`Game started`)
}
```

Output:

```text
Game started
```

`Nothing` reads like English. The function runs and returns... nothing.

Using `return` inside a `Nothing` function is fine for early exits:

```ynz
shape Player {
  name: String
  health: Number
}

function processPlayer(lend player: Player) -> Nothing {
  if (player.health <= 0.0) {
    return    // early exit — done
  }
  player.health = player.health - 10.0
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Ana`, health: 100.0 }
  processPlayer(player)
  print(player.health)
}
```

Output:

```text
90.0
```

Returning a value from a `Nothing` function is a compile error:

```ynz
function logMessage(share message: String) -> Nothing {
  return message
}

function entrypoint() -> Nothing {
  logMessage(`Game started`)
}
```

The compiler stops with:

```text
Error: `return` with a value in a `-> Nothing` function.
   ╭─[ main.ynz:2:10 ]
   │
 2 │   return message
   │          ───┬───
   │             ╰───── Remove the value: write `return` with no expression.
   │
   │ Note: Functions declared `-> Nothing` do not produce a value. A `return` inside them ends the function — no value allowed.
───╯
```

---

## Parameters — share, lend, give

Every parameter says how the function uses the value it receives. See [Ownership](REF-ownership.md) for the full picture.

```ynz
shape Player {
  name: String
  health: Number
}

shape Data {
  label: String
}

function greet(share name: String) -> Nothing {      // just reading name
  print(`Hello, ${name}`)
}

function rename(lend player: Player) -> Nothing {    // going to modify player
  player.name = `Ana B`
}

function consume(give data: Data) -> Nothing {       // taking ownership
  print(data.label)
}

function entrypoint() -> Nothing {
  let playerName = `Ana`
  let player: Player = { name: `Ana`, health: 100.0 }
  let data: Data = { label: `save file` }
  greet(playerName)    // IDE shows muted "share" — read-only access inferred
  rename(player)       // IDE shows muted "lend" (red-tinted) — function modifies player
  consume(data)        // IDE shows muted "give" (red-tinted) — function takes ownership
  print(player.name)
}
```

Output:

```text
Hello, Ana
save file
Ana B
```

The caller never types the ownership modifier — the compiler infers it from the function's signature. The IDE shows the inferred modifier as muted text.

There is no body-level syntax for `.share()` / `.lend()` / `.give()` — those modifiers live only in function signatures. See [Ownership](REF-ownership.md).

---

## Every function has a name

Write each function with `function` and a name. To use one function inside another, call it by name.

---

## No tuples — define a shape instead

If you need to return multiple values, define a shape. Named fields are always clearer than positional slots:

```ynz
shape DivResult {
  quotient: Int
  remainder: Int
}

function divmod(a: Int, b: Int) -> DivResult {
  return { quotient: a / b, remainder: a % b }
}

function entrypoint() -> Nothing {
  let result = divmod(7, 2)
  print(result.quotient)
  print(result.remainder)
}
```

Output:

```text
3
1
```
