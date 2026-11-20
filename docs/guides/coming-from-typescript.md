---
name: "coming-from-typescript"
description: "A guide for TypeScript developers: what you write in TypeScript, what you write in Yinz, and the habits the compiler will correct."
tags:
  - "yinz-compiler"
  - "guide"
created_at: "2026-11-20"
updated_at: "2026-11-20"
status: "active"
author: "yinz"
metadata:
  type: "guide"
---

# Coming from TypeScript

If you write TypeScript, most of Yinz already looks familiar. You still use `let`, `const`, template strings with `${}`, and `|` for unions. A few things work differently. This guide lists them, shows both sides, and shows the compiler message you get when an old habit slips in.

The compiler teaches as you go. Every message below says what is wrong, what to write instead, and why.

## The concept list

The table and the traps below follow this list, in this order:

1. Variables
2. Text
3. Comments and line endings
4. Data structures
5. Functions
6. Missing values
7. Failures
8. Choosing between kinds of values
9. Named value sets
10. Collections and reading by position
11. Numbers
12. Operators
13. Branching
14. Chaining
15. Files and imports
16. Slow work
17. Copies

## In TypeScript you write → In Yinz you write

| Concept | In TypeScript you write | In Yinz you write |
|---|---|---|
| Variables | `let health = 100`<br>`const maxPlayers = 10` | `let health = 100`<br>`const maxPlayers = 10` |
| Text | `'Hi'`, `"Hi"`, `` `Hi ${name}` `` | `` `Hi ${name}` `` (backticks only) |
| Comments and line endings | `/* note */` and `;` | `// note`, `/// doc comment`, no `;` |
| Data structures | `interface Player { name: string }`<br>`class Player { name: string }` | `shape Player { name: String }` |
| Functions | `function heal(p: Player): void { }` | `function heal(lend player: Player) -> Nothing { }` |
| Missing values | `string \| null`, `undefined` | `Maybe<String>`, `none` |
| Failures | `throw new Error('no file')`<br>`try { } catch (e) { }` | `function load() -> String errors { }`<br>`content.failed()`, `content.or(...)` |
| Choosing between kinds of values | `type Shape = Circle \| Square`<br>`if (shape instanceof Circle)` | `shape Shape = Circle \| Square`<br>`if (shape) { is Circle => ... }` |
| Named value sets | `enum Status { Active, Banned }` | `options Status { active, banned }` |
| Collections and reading by position | `string[]`, `Array<string>`<br>`Record<string, number>`<br>`scores[3]` is a number | `Array<String>`, `Fixed<String>`<br>`Map<String, Number>`<br>`scores[3]` is a `Maybe<Number>` |
| Numbers | `0.1 + 0.2` is `0.30000000000000004` | `0.1 + 0.2` is `0.3` |
| Operators | `===`, `score += 5`, `count++` | `==`, `score = score + 5`, `count = count + 1` |
| Branching | `if () { } else { }`<br>`switch (code) { case 200: }` | early `return`<br>`if (code) { 200 => ... else => ... }` |
| Chaining | `players.filter(f).map(g)` | a `for` loop, one named step per line |
| Files and imports | `import { x } from './utils/helpers'`<br>`export default` | `` import { x } from `utils/helpers` ``<br>`export function` |
| Slow work | `async function`, `await` | plain `function`; `wait` and `background` only when you need them |
| Copies | `const b = a` makes two names for one object<br>`structuredClone(a)` | `let b = a` hands the value over<br>`a.copy()` |

## Traps

### Variables: `const` also protects the fields

In TypeScript, `const` stops reassignment but you can still change the fields. In Yinz, a `const` value cannot change at all.

```
const player: Player = { name: `Patrick`, health: 100 }
player.health = 50
// COMPILE ERROR: `player` is `const` and its fields cannot be changed.
// Declare it with `let` instead: `let player: ShapeType = { ... }`
```

Use `let` if the value needs to change.

### Text: one quote style

Yinz has one text form: backticks. It supports `${}` and multiple lines.

```
print("hello world")
// COMPILE ERROR: Double-quoted strings don't exist in Yinz.
// Use backtick strings instead: `` `hello` `` or `` `Hello ${name}` ``
```

Single quotes get the same error.

### Comments and line endings: `//`, `///`, and no `;`

A newline ends a statement. A `;` is an error:

```
let health = 100;
// COMPILE ERROR: Semicolons are not used in Yinz.
// Remove the `;` — Yinz uses newlines to end statements.
```

There is no `/* */` block comment. A `///` block directly above a declaration is its doc comment, and it shows in editor hover.

```
// regular comment, one line
/// doc comment, shown in IDE hover
```

### Data structures: `shape` replaces `interface` and `class`

A `shape` lists fields and nothing else. There is no constructor: you build a value by writing its fields.

TypeScript:

```ts
interface Player {
  name: string
  health: number
}

const player: Player = { name: 'Alice', health: 50 }
```

Yinz:

```
shape Player {
  name: String
  health: Int
}

let player: Player = { name: `Alice`, health: 50 }
```

A TypeScript `class` with a constructor becomes the same `shape`. Its methods become standalone functions, which the next trap shows.

Writing `class` gives errors, and among them you also get this warning:

```
class Player {
  name: String
}
// COMPILE WARNING: `class` is not a keyword in Yinz.
// Use `shape` for data and standalone functions for behavior: `shape Player { name: String }` then `function greet(share self: Player) -> String`
```

### Functions: methods are not inside shapes

A `shape` holds data. Behavior lives in standalone functions, and `player.heal(20)` is shorthand for `heal(player, 20)`.

```
shape Player {
  name: String
  health: Int
}

function heal(lend self: Player, amount: Int) -> Nothing {
  self.health = self.health + amount
}

let player: Player = { name: `Alice`, health: 50 }
player.heal(20)
print(player.health)    // 70
```

`lend` says "this function changes the value." `share` says "this function only reads it." You write these words in the function signature, never at the call.

### Missing values: `null` and `undefined` do not exist

A value that might be missing has the type `Maybe<T>`. You cannot use it until you handle the `none` case.

```
let nickname: Maybe<String> = none
print(nickname.value)
// COMPILE ERROR: `Maybe.value` requires you to first check `m.exists()`.
// Add a check: `if (m.exists()) { print(m.value) }`. Or use a default: `m.or(0)`.
```

Two correct ways:

```
let display = nickname.or(`Anonymous`)

if (nickname.exists()) {
  print(nickname.value)
}
```

A `?` after a type is not valid either:

```
let score: Int? = none
// COMPILE ERROR: The `?` optional suffix is not valid in Yinz.
// Use `Maybe<T>` instead: `Maybe<Int>` not `Int?`.
```

### Failures: Yinz has no `throw` and no `try`/`catch`

A function that can fail says so in its signature with `errors`. The caller either passes the failure up, supplies a fallback, or checks it.

```
function readFile(path: String) -> String errors {
  return `file contents`
}

function loadConfig() -> String {
  let content = readFile(`config.txt`)
  let text = content.or(`{}`)       // `{}` if the file can't be read
  return text
}
```

To pass the failure up instead, mark the calling function `errors` too. Using the result of an `errors` function without handling it is a compile error.

### Choosing between kinds of values: `is`, not `instanceof`

A union uses `|`, as in TypeScript. You check the kind with `is` inside an `if` that has one arm per kind. Inside an arm, the value already has the right kind, so you read its fields directly.

```
shape Shape = Circle | Square | Triangle

function describe(share outline: Shape) -> Nothing {
  if (outline) {
    is Circle => print(`circle`)
    is Square => print(`square`)
    is Triangle => print(`triangle`)
  }
}
```

If you forget a kind, the compiler names it:

```
// COMPILE ERROR: Non-exhaustive union multi-case — 1 variant is not handled: Triangle.
// Add the missing arms (e.g. `is Triangle =>`) or add an `else =>` default arm.
```

Write unions with `|`. Words such as `or` do not work.

### Named value sets: `options` replaces `enum`

An `options` declaration lists the allowed names. You write no numbers. You pick a name with `Status.active`.

TypeScript:

```ts
enum Status {
  Active,
  Banned,
}

function describeStatus(status: Status): string {
  if (status === Status.Banned) {
    return 'banned'
  }
  return 'online'
}

console.log(describeStatus(Status.Active))    // online
```

Yinz:

```
options Status {
  active
  banned
}

function entrypoint() -> Nothing {
  let status: Status = Status.active
  if (status) {
    active => {
      print(`online`)
    }
    banned => {
      print(`banned`)
    }
  }
}
```

This prints `online`. Names are camelCase, like every other name that is not a type. The compiler checks that your `if` handles every name in the set.

Writing `enum` gives errors, and among them you also get this warning:

```
enum Status {
// COMPILE WARNING: `enum` is not a keyword in Yinz.
// Use `options` to declare a set of named values: `options Status { active, inactive, banned }`
```

### Collections and reading by position: capitalized names, and reading returns a `Maybe`

Every type starts with a capital letter, built-in collections included.

```
let scores: array<Int> = []
// COMPILE ERROR: `array` is not a type name.
// Write `Array`. — Every type in Yinz starts with a capital letter, built-ins included.
```

Reading by position never crashes. `scores[3]` gives a `Maybe`, so you handle the missing case:

```
let scores: Array<Int> = [70, 85, 92]
let found = scores[5]
let score = found.or(0)
print(score)    // 0
```

A shape value is not a collection. Use `[...]` to make one:

```
shape Player {
  name: String
  health: Int
}

let team: Array<Player> = { name: `Alice`, health: 100 }
// COMPILE ERROR: `{ ... }` creates a single `Player` value, not an `Array<Player>`.
// Put it inside `[...]` to make an array: `[{ ... }]`
```

### Numbers: `Number` is exact

In TypeScript, `0.1 + 0.2` gives `0.30000000000000004`. In Yinz, `Number` stores decimal digits exactly, so you get the answer you expect:

```
let total = 0.1 + 0.2
print(total)    // 0.3
```

Use `Int` for whole numbers and `Float` only when you want fast binary arithmetic and can accept tiny rounding differences.

### Operators: `==` only, and no shortcuts

`==` compares values and never converts types silently. Comparing different types is an error:

```
let label: String = `5`
let count: Number = 5.0
if (label == count) { }
// COMPILE ERROR: `==` cannot be used with `String` and `Number`.
// Check that both sides have the same type.
```

There is no `===`.

`+=`, `-=`, `++` and `--` do not exist. Write the assignment out:

```
let score = 10
score += 5
// COMPILE ERROR: `+=` is not supported in Yinz.
// Use `x = x + n` instead.

score = score + 5
```

### Branching: no `else` block, no `switch`

Handle one case with an early `return`, then carry on:

```
function describe(share player: Player) -> String {
  if (player.health <= 0) {
    return `dead`
  }
  return `alive`
}
```

When you cannot return, set a value first and change it:

```
let message = `in danger`
if (player.health > 50) {
  message = `healthy`
}
print(message)
```

To match a value against several cases, use a multi-case `if`. `else =>` is the catch-all:

```
if (statusCode) {
  200 => print(`ok`)
  404 => print(`not found`)
  else => print(`unexpected: ${statusCode}`)
}
```

Writing `switch (statusCode) {` gives errors, and among them this warning:

```
switch (statusCode) {
// COMPILE WARNING: `switch` is not a keyword in Yinz.
// Use multi-case `if` with `=>` arms: `if (value) { 1 => ...; 2 => ...; else => ... }`
```

### Chaining: a chain becomes named steps

Yinz has no chaining. Each step goes on its own line with a name, so you can inspect every value. The example below uses a `for` loop that builds the result one item at a time.

TypeScript:

```ts
const players = [
  { name: 'Alice', health: 100 },
  { name: 'Bob', health: 0 },
  { name: 'Cara', health: 80 },
]
const activeNames = players.filter(player => player.health > 0).map(player => player.name)
console.log(activeNames.length)    // 2
```

Yinz:

```
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let players: Array<Player> = [
    { name: `Alice`, health: 100 },
    { name: `Bob`, health: 0 },
    { name: `Cara`, health: 80 }
  ]
  let activeNames: Array<String> = []
  for (player in players) {
    if (player.health > 0) {
      activeNames.add(player.name)
    }
  }
  let activeCount = activeNames.count()
  print(activeCount)    // 2
}
```

A chain of calls written on one line is not written in Yinz. Give each result a name on its own line, as above.

### Files and imports: paths start at the project root

Import paths start at your project root. `./` and `../` are errors, and the path is a backtick string.

```
import { fetchUser } from `../services/users`
// COMPILE ERROR: Import paths must be project-root relative, not relative paths.
// Write `services/users` instead of `../services/users`.
```

There are no default exports, and you export by name:

```
export default function fetchUser() { }
// COMPILE ERROR: Yinz does not have default exports.
// Name the export instead: `export function foo()` or `export const PI = 3.14`.
```

Two imports cannot use the same name. Yinz stops you and asks you to rename one:

```
import { Player } from `models/game`
import { Player } from `external/legacy`
// COMPILE ERROR: Import name `Player` is already bound by a previous import.
// Use a different local name: `import { Player as otherName } from "..."`
```

### Slow work: no `async`, no `await`

You do not mark functions as `async`. When a call has to pause, such as a network call, the compiler pauses your function by itself, and your editor shows that as a muted `wait` hint. Independent calls run at the same time without any extra code.

Write `wait` only when two calls must finish in a set order and the compiler cannot see why. Write `background` to start a task that keeps running after your function returns.

### Copies: `const b = a` hands the value over

In TypeScript, `const b = a` gives one array two names, and a change through one name shows through the other. Yinz never lets one value have two names. Storing a value in another name, an array or a shape hands it over, and the old name cannot be read afterward.

TypeScript:

```ts
const scores = [70, 85, 92]
const backup = scores
backup.push(100)
console.log(scores.length)    // 4, because both names are one array
```

Yinz:

```
let scores: Array<Int> = [70, 85, 92]
let backup: Array<Int> = scores
let total = scores.count()
// COMPILE ERROR: `scores` was stored in `backup` and cannot be used here — storing a value hands it over.
// If you still need `scores` after the store, store a copy instead: `let backup = scores.copy()`. If you only need it before the store, put this line above the store.
```

Ask for a second value with `.copy()`. TypeScript's `structuredClone(scores)` is the match:

```
let scores: Array<Int> = [70, 85, 92]
let backup = scores.copy()
backup.add(100)
let total = scores.count()
print(total)    // 3
```

The same rule applies when you store a shape value in an array. In TypeScript, `path.push(start)` stores the same object, and a later change to `start` shows through the array. In Yinz, you ask for the copy:

```
shape Position {
  x: Int
  y: Int
}

let path: Array<Position> = []
let start: Position = { x: 1, y: 2 }
path.add(start.copy())
start.x = 99
```

To change a stored element, write through the collection:

```
path[0] = { x: 99, y: 2 }
```

## Habits from other languages

The compiler also catches syntax from Python, shell and PHP:

```
# this is a comment
// COMPILE ERROR: Yinz uses `//` for comments, not `#`.
// Change `#` to `//`: `// this is a comment`
```

```
let y = $x
// COMPILE ERROR: Variables in Yinz don't use a `$` prefix.
// Remove the `$`: write `x` instead of `$x`.
```

## Where to go next

Each topic has a full reference page in [`docs/reference/`](../reference/REF-language-overview.md). Start with [Variables](../reference/REF-variables.md), [Maybe Types](../reference/REF-maybe.md) and [Errors](../reference/REF-errors.md).
