---
name: "REF-linting"
description: "The compiler catches problems at three levels. Errors block compilation. Warnings compile but flag something to fix. Suggestions are informational — shown in the IDE, not in the terminal by default."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Linting

The compiler catches problems at three levels. Errors block compilation. Warnings compile but flag something to fix. Suggestions are informational — shown in the IDE, not in the terminal by default.

Every rule prevents a real problem. Nothing is enforced just because "that's how it's usually done."

Each message the compiler prints has three parts: WHAT happened, WHAT to do instead, and WHY. This page shows the first part, the WHAT line, for each example. See [REF-compiler-errors.md](REF-compiler-errors.md) for the full format.

---

## Errors — code won't compile

**A value you gave away:**

```ynz
shape Player {
  name: String
  health: Int
}

function retire(give player: Player) -> Nothing {
  print(player.name)
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Ana`, health: 100 }
  retire(player)
  print(player.name)
}
```

```
`player` was already given away and cannot be used here.
```

Call `.copy()` before giving the value away if you need it in both places.

**A path that does not return:**

```ynz
shape Player {
  name: String
  active: Boolean
}

function getName(share player: Player) -> String {
  if (player.active) {
    return player.name
  }
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Ana`, active: true }
  print(getName(player))
}
```

```
`getName` must return a `String` on every path, but some paths fall off the end without returning.
```

**A value of the wrong type:**

```ynz
function entrypoint() -> Nothing {
  let count: Int = `hello`
}
```

```
This value is `String`, but `count` is declared as `Int`.
```

**A failure that nothing handles:**

```ynz
function loadScore() -> Int errors {
  return 42
}

function entrypoint() -> Nothing {
  print(loadScore())
}
```

```
This function can fail, but the failure is not handled here.
```

Mark the calling function `errors`, give a fallback with `.or(default)`, or check `.failed()`.

**A `Maybe` used as if it always had a value:**

```ynz
shape User {
  name: String
}

function greet(share user: Maybe<User>) -> Nothing {
  print(user.name)
}

function entrypoint() -> Nothing {
  let user: Maybe<User> = none
  greet(user)
}
```

```
`Maybe<User>` does not have a field called `name`.
```

Check `user.exists()` first and then read `user.value`, or give a fallback with `user.or(default)`.

**Changing a `const`:**

```ynz
function entrypoint() -> Nothing {
  const name = `Patrick`
  name = `Alice`
}
```

```
`name` cannot be changed — it was declared with `const`.
```

**Mixed types in a collection:**

```ynz
function entrypoint() -> Nothing {
  let stuff = [42, `hello`]
}
```

```
Element 2 has type `String`, but the array expects `Int`.
```

All elements of an array must have the same type.

**Adding to a fixed array:**

```ynz
function entrypoint() -> Nothing {
  let nums: Fixed<Int> = [1, 2, 3]
  nums.add(4)
}
```

```
`Fixed<Int>` does not have a method called `add`.
```

`Fixed<Int>` is size-locked. Use `Array<Int>` if the list needs to grow.

**Sharing with a background task:**

```ynz
shape Report {
  title: String
}

function processReport(share report: Report) -> Nothing {
  print(report.title)
}

function entrypoint() -> Nothing {
  let report: Report = { title: `Weekly` }
  background processReport(report)
}
```

```
Cannot use `background` with a function that borrows its arguments.
```

A `background` task can outlive the function that started it. Take the parameter as `give`, or pass `report.copy()` to a function that takes `give`. See [REF-concurrency.md](REF-concurrency.md).

**A lowercase built-in type name:**

```ynz
function entrypoint() -> Nothing {
  let health: int = 100
}
```

```
`int` is not a type name.
```

Write `Int`. See [REF-naming.md](REF-naming.md).

---

## Warnings — compiles, but fix this

**Unreachable code:**

```ynz
function finish() -> String {
  return `done`
  print(`after return`)
}

function entrypoint() -> Nothing {
  print(finish())
}
```

```
This code will never run.
```

Remove the code after `return`, or move the `return` below it.

**Unused import:**

```ynz
// services/users.ynz
export function fetchUser(id: Int) -> String {
  return `user ${id.toString()}`
}

export function createUser(id: Int) -> String {
  return `new user ${id.toString()}`
}
```

```ynz
// entrypoint.ynz
import { fetchUser, createUser } from `services/users`

function entrypoint() -> Nothing {
  print(fetchUser(1))
}
```

```
`createUser` is imported but never used.
```

Remove the name from the import, or use it.

**A banned keyword:**

```ynz
function entrypoint() -> Nothing {
  let type = `player`
  print(type)
}
```

```
`type` is not a keyword in Yinz.
```

The compiler warns each time the word appears, then reads it as an ordinary name. Declare data with `shape`, not `type`. A declaration that starts with `type Player { ... }` also fails to parse. See [REF-compiler-errors.md](REF-compiler-errors.md).

---

## Suggestions — IDE only by default

Suggestions are informational. The IDE shows them. The terminal does not.

**An array that never grows, and a variable that never changes.** When nothing grows an `Array<T>` after it is created, the compiler stores it as `Fixed<T>`. When nothing reassigns a `let`, the compiler treats it as `const`. The IDE underlines the `Array` or the `let` you wrote with a dotted line. Hover for the explanation. The lightbulb action rewrites the word for you. See [REF-ide-hints.md](REF-ide-hints.md).

**Blocking sleep.** The rule `prefer-yielding-sleep` fires on `sleepBlocking` in programs that have a scheduler:

```ynz
function entrypoint() -> Nothing {
  sleepBlocking(100)
}
```

```
`sleepBlocking` parks the current thread — it sits idle and runs nothing else until the time is up.
```

Write `wait sleep(100)` instead. Your function pauses just the same, and the thread stays free to run other tasks.

**Shapes used by different background tasks.** The rule `cross-thread-fields-not-padded` fires when different `background` tasks use fields of a shape whose layout the compiler cannot pad because another file can see it:

```ynz
export shape Counters {
  hits: Int
  misses: Int
}

function record(give counters: Counters) -> Nothing {
  wait sleep(1)
  print(counters.hits.toString())
}

function entrypoint() -> Nothing {
  let counters: Counters = { hits: 3, misses: 1 }
  background record(counters)
}
```

```
Fields of `Counters` are used by different background tasks, but the compiler could not add cache-line padding to it.
```

Keep hot cross-task state in a shape declared in the same file and not exported. The compiler pads that kind of shape automatically.

**Arrays stored as separate per-field arrays.** The rule `array-using-soa-layout` tells you when the compiler stores a large, never-grown `Array` of shape values as one block of memory per field because your loops read at most two fields. There is nothing to change. The program behaves the same either way.
