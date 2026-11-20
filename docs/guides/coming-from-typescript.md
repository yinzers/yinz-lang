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

If you write TypeScript, most of Yinz already looks familiar. You still use `let`, `const`, arrow functions in callbacks, template strings with `${}`, and `|` for unions. A few things work differently. This guide lists them, shows both sides, and shows the compiler message you get when an old habit slips in.

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
| Data structures | `interface Player { name: string }`<br>`class Player { name: string }` | `shape Player { name: string }` |
| Functions | `function heal(p: Player): void { }` | `function heal(lend player: Player) -> nothing { }` |
| Missing values | `string \| null`, `undefined` | `maybe<string>`, `none` |
| Failures | `throw new Error('no file')`<br>`try { } catch (e) { }` | `function load() -> string errors { }`<br>`content.failed()`, `content.or(...)` |
| Choosing between kinds of values | `type Shape = Circle \| Square`<br>`if (shape instanceof Circle)` | `shape Shape = Circle \| Square`<br>`if (shape) { is Circle => ... }` |
| Named value sets | `enum Status { Active, Banned }` | `options Status { active, banned }` |
| Collections and reading by position | `string[]`, `Array<string>`<br>`Record<string, number>`<br>`scores[3]` is a number | `array<string>`, `fixed<string>`<br>`map<string, number>`<br>`scores[3]` is a `maybe<number>` |
| Numbers | `0.1 + 0.2` is `0.30000000000000004` | `0.1 + 0.2` is `0.3` |
| Operators | `===`, `score += 5`, `count++` | `==`, `score = score + 5`, `count = count + 1` |
| Branching | `if () { } else { }`<br>`switch (code) { case 200: }` | early `return`<br>`if (code) { 200 => ... else => ... }` |
| Chaining | `players.filter(f).map(g)` | one named step per line |
| Files and imports | `import { x } from './utils/helpers'`<br>`export default` | `` import { x } from `utils/helpers` ``<br>`export function` |
| Slow work | `async function`, `await` | plain `function`; `wait` and `background` only when you need them |
| Copies | `const copy = original` points at the same object | storing a shape in a collection stores a copy |

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
// Use backtick strings instead: `hello` or `Hello ${name}`
```

Single quotes get the same error.

### Line endings: no semicolons

A newline ends a statement.

```
let health = 100;
// COMPILE ERROR: Semicolons are not used in Yinz.
// Remove the `;` — Yinz uses newlines to end statements.
```

### Comments: `//` and `///` only

There is no `/* */` block comment. A `///` block directly above a declaration is its doc comment, and it shows in editor hover.

```
// regular comment, one line
/// doc comment, shown in IDE hover
```

### Functions: methods are not inside shapes

A `shape` holds data. Behavior lives in standalone functions, and `player.heal(20)` is shorthand for `heal(player, 20)`.

```
shape Player {
  name: string
  health: int
}

function heal(lend self: Player, amount: int) -> nothing {
  self.health = self.health + amount
}

let player: Player = { name: `Alice`, health: 50 }
player.heal(20)
print(player.health)    // 70
```

`lend` says "this function changes the value." `share` says "this function only reads it." You write these words in the function signature, never at the call.

### Missing values: `null` and `undefined` do not exist

A value that might be missing has the type `maybe<T>`. You cannot use it until you handle the `none` case.

```
let nickname: maybe<string> = none
print(nickname.value)
// COMPILE ERROR: `maybe.value` requires you to first check `m.exists()`.
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
let score: int? = none
// COMPILE ERROR: The `?` optional suffix is not valid in Yinz.
// Use `maybe<T>` instead: `maybe<int>` not `int?`.
```

### Failures: Yinz has no `throw` and no `try`/`catch`

A function that can fail says so in its signature with `errors`. The caller either passes the failure up, supplies a fallback, or checks it.

```
function readFile(path: string) -> string errors {
  return `file contents`
}

function loadConfig() -> string {
  let content = readFile(`config.txt`)
  let text = content.or(`{}`)       // `{}` if the file can't be read
  return text
}
```

To pass the failure up instead, mark the calling function `errors` too. Calling an `errors` function without handling it is a compile error.

### Choosing between kinds of values: `is`, not `instanceof`

A union uses `|`, as in TypeScript. You check the kind with `is` inside an `if` that has one arm per kind. Inside an arm, the value already has the right kind, so you read its fields directly.

```
shape Shape = Circle | Square | Triangle

function describe(share outline: Shape) -> nothing {
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

### Collections: lowercase names, and reading returns a `maybe`

Built-in collection names are lowercase.

```
let scores: Array<int> = []
// COMPILE ERROR: `Array` is not a type — built-in collection types are lowercase in Yinz.
// Use `array` (lowercase): `array<...>`
```

Reading by position never crashes. `scores[3]` gives a `maybe`, so you handle the missing case:

```
let scores: array<int> = [70, 85, 92]
let found = scores[5]
let score = found.or(0)
print(score)    // 0
```

A shape value is not a collection. Use `[...]` to make one:

```
shape Player {
  name: string
  health: int
}

let team: array<Player> = { name: `Alice`, health: 100 }
// COMPILE ERROR: `{ ... }` creates a single `Player` value, not an `array<Player>`.
// Put it inside `[...]` to make an array: `[{ ... }]`
```

### Numbers: `number` is exact

In TypeScript, `0.1 + 0.2` gives `0.30000000000000004`. In Yinz, `number` stores decimal digits exactly, so you get the answer you expect:

```
let total = 0.1 + 0.2
print(total)    // 0.3
```

Use `int` for whole numbers and `float` only when you want fast binary arithmetic and can accept tiny rounding differences.

### Operators: `==` only, and no shortcuts

`==` compares values and never converts types silently. Comparing different types is an error:

```
let label: string = `5`
let count: number = 5.0
if (label == count) { }
// COMPILE ERROR: `==` cannot be used with `string` and `number`.
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
function describe(share player: Player) -> string {
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

Writing `switch` gives the warning "`switch` is not a keyword in Yinz." and tells you to use a multi-case `if` with `=>` arms.

### Chaining: one step per line

Give each step a name:

```
let active = players.filter(player => player.health > 0)
let topTen = active.limit(10)
let names = topTen.map(player => player.name)
```

The compiler joins the steps into one fast pass, so the named version costs nothing.

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

``import users from `services/users` `` looks like a default import, but it brings in the whole module under the name `users`. You then write `users.fetchUser()`.

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

### Copies: storing a shape copies it

In TypeScript, an array and a variable can point at the same object. In Yinz, `add` stores a copy of the shape value as it is at that moment:

```
shape Position {
  x: int
  y: int
}

let path: array<Position> = []
let spot: Position = { x: 1, y: 2 }
path.add(spot)
spot.x = 99

let first = path.first()
if (first.exists()) {
  print(first.value.x)    // 1
}
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
