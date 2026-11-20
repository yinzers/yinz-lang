---
name: "coming-from-python"
description: "A guide for Python developers: what you write in Python, what you write in Yinz, and the habits the compiler will correct."
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

# Coming from Python

If you write Python, most of Yinz already reads like plain English. You still write `for`, `while`, `if`, `return`, `print` and `range`, and you still use `in` to loop over a collection. A few things work differently: blocks use braces instead of indentation, you write types at the edges of a function, and there are no classes. This guide lists them, shows both sides, and shows the compiler message you get when an old habit slips in.

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

## In Python you write → In Yinz you write

| Concept | In Python you write | In Yinz you write |
|---|---|---|
| Variables | `health = 100`<br>no `const`; `MAX_PLAYERS = 10` by convention | `let health = 100`<br>`const maxPlayers = 10` |
| Text | `'Hi'`, `"Hi"`, `f"Hi {name}"` | `` `Hi ${name}` `` (backticks only) |
| Comments and line endings | `# note`, `"""docstring"""`, no `;` needed | `// note`, `/// doc comment`, no `;` |
| Data structures | `class Player:` with `__init__`<br>`@dataclass` | `shape Player { name: String }` |
| Functions | `def heal(player, amount):`<br>`return None` | `function heal(lend self: Player, amount: Int) -> Nothing { }` |
| Missing values | `None`, `Optional[str]`<br>`if nickname is None:` | `Maybe<String>`, `none`<br>`if (nickname.exists()) { }` |
| Failures | `raise ValueError("no file")`<br>`try:` … `except:` | `function load() -> String errors { }`<br>`content.failed()`, `content.or(...)` |
| Choosing between kinds of values | `Shape = Circle \| Square`<br>`isinstance(shape, Circle)` | `shape Shape = Circle \| Square`<br>`if (shape) { is Circle => ... }` |
| Named value sets | `class Status(Enum):`<br>`ACTIVE = 1` | `options Status { active, banned }` |
| Collections and reading by position | `list[str]`, `dict[str, int]`<br>`scores[3]` raises `IndexError` | `Array<String>`, `Fixed<String>`, `Map<String, Int>`<br>`scores[3]` is a `Maybe<Int>` |
| Numbers | `0.1 + 0.2` is `0.30000000000000004`<br>`7 / 2` is `3.5`, `7 // 2` is `3` | `0.1 + 0.2` is `0.3`<br>`7 / 2` is `3`; `//` starts a comment |
| Operators | `and`, `or`, `not`<br>`score += 5` | `&&`, `\|\|`, `!`<br>`score = score + 5` |
| Branching | `if:` / `elif:` / `else:`<br>`match code:` | early `return`<br>`if (code) { 200 => ... else => ... }` |
| Chaining | `[p.name for p in players if p.health > 0]` | a `for` loop, one named step per line |
| Files and imports | `from services.users import fetch_user`<br>`from .users import fetch_user`<br>`if __name__ == "__main__":` | `` import { fetchUser } from `services/users` ``<br>`export function`<br>`function entrypoint()` |
| Slow work | `async def`, `await`, `asyncio.gather(...)` | plain `function`; `wait` and `background` only when you need them |
| Copies | `b = a` makes two names for one list<br>`a.copy()` | `let b = a` hands the value over<br>`a.copy()` |

## Traps

### Variables: declare every name with `let` or `const`

In Python, `health = 100` creates the variable. In Yinz, a new name needs `let` (can change) or `const` (never changes). Assigning to a name that was never declared is an error:

```
function startRound() -> Nothing {
  health = 100
}
// COMPILE ERROR: `health` is not defined.
// Declare it first: `let health = ...`
```

Python has no `const`; you write `MAX_PLAYERS = 10` and promise not to change it. In Yinz the compiler keeps the promise. A capital letter always means a type, so a constant is written in camelCase like every other name:

```
function startRound() -> Nothing {
  const maxPlayers = 10
  maxPlayers = 20
}
// COMPILE ERROR: `maxPlayers` cannot be changed — it was declared with `const`.
// Change `const maxPlayers = ...` to `let maxPlayers = ...` if you need to reassign it.
```

Use `const` by default. Use `let` when the value needs to change.

### Text: backticks only, and `${}` replaces f-strings

Python has `'...'`, `"..."`, `f"..."` and `"""..."""`. Yinz has one text form: backticks. It supports `${}` and runs over several lines.

```
let greeting = 'Hello'
// COMPILE ERROR: Single-quoted strings don't exist in Yinz.
// Use backtick strings instead: `` `hello world` ``
```

```
let greeting = "Hello"
// COMPILE ERROR: Double-quoted strings don't exist in Yinz.
// Use backtick strings instead: `` `hello` `` or `` `Hello ${name}` ``
```

Write what you would put after `f"` inside backticks instead:

```
let name = `Alice`
let greeting = `Hello ${name}`
print(greeting)    // Hello Alice
```

### Comments and line endings: `//`, `///`, and no `;`

Python comments start with `#`. Yinz comments start with `//`.

```
# this is a comment
// COMPILE ERROR: Yinz uses `//` for comments, not `#`.
// Change `#` to `//`: `// this is a comment`
```

A Python docstring becomes a `///` doc comment placed directly above the declaration. It shows in the editor when you hover over the name.

```
/// Adds `amount` to the player's health.
function heal(lend self: Player, amount: Int) -> Nothing {
  self.health = self.health + amount
}
```

A newline ends a statement, as in Python. A `;` is an error:

```
let health = 100;
// COMPILE ERROR: Semicolons are not used in Yinz.
// Remove the `;` — Yinz uses newlines to end statements.
```

Python uses a colon and indentation to group lines. Yinz uses braces `{ }`, and indentation is only for readers. See Branching below.

### Data structures: `shape` holds the fields, and there is no `class`

A `shape` lists fields and nothing else. There is no `__init__`: you build a value by writing its fields.

Python:

```python
class Player:
    def __init__(self, name, health):
        self.name = name
        self.health = health

player = Player("Alice", 50)
```

Yinz:

```
shape Player {
  name: String
  health: Int
}

let player: Player = { name: `Alice`, health: 50 }
```

Writing `class` gives errors, and among them you also get this warning:

```
class Player {
  name: String
}
// COMPILE WARNING: `class` is not a keyword in Yinz.
// Use `shape` for data and standalone functions for behavior: `shape Player { name: String }` then `function greet(share self: Player) -> String`
```

A `@dataclass` is the closest Python match: fields only. Behavior lives in standalone functions, which the next trap shows.

### Functions: types on the edges, and no methods inside shapes

A parameter needs a type and the function needs a return type. `-> Nothing` is the Yinz word for "returns no value" (Python's `None` return). Inside the body you write no types: the compiler works them out.

Behavior lives in standalone functions outside the shape, and `player.heal(20)` is shorthand for `heal(player, 20)`. The first parameter is named `self`, as in Python, but you write it yourself and it is not special.

Python:

```python
class Player:
    def __init__(self, name, health):
        self.name = name
        self.health = health

def heal(player, amount):
    player.health += amount

player = Player("Alice", 50)
heal(player, 20)
print(player.health)    # 70
```

Yinz:

```
shape Player {
  name: String
  health: Int
}

function heal(lend self: Player, amount: Int) -> Nothing {
  self.health = self.health + amount
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Alice`, health: 50 }
  player.heal(20)
  print(player.health)    // 70
}
```

In Python, any function can change what you pass it, and nothing in the signature says so. In Yinz, `lend` says "this function changes the value", and `share` says "this function only reads it". You write these words in the signature, never at the call.

`print` takes exactly one value. Python's `print("Health:", health)` does not work:

```
print(`Health:`, health)
// COMPILE ERROR: `print` takes 1 argument, but 2 were given.
// Call it with one value: `print(value)`
```

Put the value in the text instead: ``print(`Health: ${health}`)``.

### Missing values: `none` and `Maybe<T>` replace `None`

A value that might be missing has the type `Maybe<T>`. You cannot use it until you handle the `none` case. In Python, `nickname.upper()` on a `None` crashes when the program runs. In Yinz, the compiler stops you first.

Python:

```python
nickname = None
display = nickname or "Anonymous"
if nickname is not None:
    print(nickname.upper())
```

Yinz:

```
let nickname: Maybe<String> = none
print(nickname.value)
// COMPILE ERROR: `Maybe.value` requires you to first check `m.exists()`.
// Add a check: `if (m.exists()) { print(m.value) }`. Or use a default: `m.or(0)`.
```

Two correct ways:

```
let nickname: Maybe<String> = none
let display = nickname.or(`Anonymous`)

if (nickname.exists()) {
  print(nickname.value)
}
```

Python's `is` checks identity. In Yinz, `is` checks which kind of value a union holds (see Choosing between kinds of values).

### Failures: `errors` replaces `raise`, `try` and `except`

A function that can fail says so in its signature with `errors`. There is no `raise`, `try` or `except`. The caller supplies a fallback, checks the result, or passes the failure up.

Python:

```python
def batting_average(hits, at_bats):
    return hits * 1000 // at_bats

try:
    average = batting_average(127, 400)
except ZeroDivisionError:
    average = 0
print(average)    # 317
```

Yinz:

```
function battingAverage(hits: Int, atBats: Int) -> Int errors {
  return (hits * 1000) / atBats
}

function entrypoint() -> Nothing {
  let attempt = battingAverage(127, 400)
  let average = attempt.or(0)    // the fallback value, used when the call fails
  print(average)    // 317
}
```

Python's `ZeroDivisionError` is an exception you can catch with `except`. Yinz has no such catch. Dividing by zero is not a failure you can catch: it stops the program with `RUNTIME ERROR: division by zero (Int)`. Check the number before you divide.

To look at the failure yourself, use `.failed()`:

```
let checked = battingAverage(50, 200)
if (checked.failed()) {
  print(`battingAverage failed`)
}
```

To pass the failure up instead, mark the calling function `errors` too. A failure left unhandled is a compile error, so it cannot slip through:

```
let attempt = battingAverage(127, 400)
print(attempt)
// COMPILE ERROR: This function can fail, but the failure is not handled here.
// Three options: (1) Mark this function `-> T errors` to pass failures up. (2) Use `.or(default)` for a fallback. (3) Check `.failed()` explicitly.
```

### Choosing between kinds of values: `is` replaces `isinstance`

A union uses `|`, as in Python 3.10 and later. You check the kind with `is` inside an `if` that has one arm per kind. Inside an arm, the value already has the right kind, so you read its fields directly.

Python:

```python
from dataclasses import dataclass

@dataclass
class Circle:
    radius: int

@dataclass
class Square:
    side: int

Shape = Circle | Square

def describe(outline: Shape) -> None:
    if isinstance(outline, Circle):
        print("circle")
    elif isinstance(outline, Square):
        print("square")
```

Yinz:

```
shape Circle {
  radius: Int
}

shape Square {
  side: Int
}

shape Triangle {
  baseLength: Int
  height: Int
}

shape Shape = Circle | Square | Triangle

function describe(outline: Shape) -> Nothing {
  if (outline) {
    is Circle => {
      print(`circle`)
    }
    is Square => {
      print(`square`)
    }
    is Triangle => {
      print(`triangle`)
    }
  }
}
```

Python does not notice a forgotten kind. The Yinz compiler names it:

```
if (outline) {
  is Circle => { print(`circle`) }
  is Square => { print(`square`) }
}
// COMPILE ERROR: Non-exhaustive union multi-case — 1 variant is not handled: Triangle.
// Add the missing arms (e.g. `is Triangle =>`) or add an `else =>` default arm.
```

### Named value sets: `options` replaces `Enum`

An `options` declaration lists the allowed names. You write no numbers and no `Enum` import. You pick a name with `Status.active`.

Python:

```python
from enum import Enum

class Status(Enum):
    ACTIVE = 1
    BANNED = 2

status = Status.ACTIVE
if status == Status.BANNED:
    print("banned")
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

### Collections and reading by position: reading never raises `IndexError`

Every Yinz type starts with a capital letter, built-in collections included:

| Python | Yinz |
|---|---|
| `list[int]`, grows | `Array<Int>`, grows |
| (no direct match) | `Fixed<Int>`, size set at creation |
| `dict[str, int]` | `Map<String, Int>` |

Reading by position or by key gives a `Maybe`, so a missing item never crashes the read. Python's `scores[5]` raises `IndexError` and `hit_leaders["stargell"]` raises `KeyError`. Python's `hit_leaders.get("stargell", 0)` is the closest match to what Yinz does:

```
let scores: Array<Int> = [70, 85, 92]
let foundScore = scores[5]
let score = foundScore.or(0)
print(score)    // 0

let hitLeaders: Map<String, Int> = { `clemente`: 3000, `wagner`: 3420 }
let foundHits = hitLeaders[`stargell`]
let hits = foundHits.or(0)
print(hits)    // 0
```

The everyday collection functions have new names:

| Python | Yinz |
|---|---|
| `len(scores)` | `scores.count()` |
| `scores.append(88)` | `scores.add(88)` |
| `88 in scores` | `scores.contains(88)` |
| `scores[-1]` | `scores.last()`, a `Maybe` |

A value written with `{ ... }` is a single shape value, not a collection. Use `[...]` to make a collection:

```
let team: Array<Player> = { name: `Alice`, health: 100 }
// COMPILE ERROR: `{ ... }` creates a single `Player` value, not an `Array<Player>`.
// Put it inside `[...]` to make an array: `[{ ... }]`
```

Writing past the end by position stops the whole program with no message, so use `.add(item)` to grow an `Array`.

### Numbers: `/` on whole numbers stays whole, and `//` is a comment

Python's `/` always gives a decimal, and `//` rounds down. In Yinz, `/` on two `Int` values gives an `Int`, and `//` starts a comment:

```
let half = 7 / 2
print(half)    // 3

let rounded = 7 // 2
print(rounded)    // 7, because `// 2` is a comment
```

For a result with decimals, use decimal numbers: `7.0 / 2.0`. There is no `**` operator.

Python's `0.1 + 0.2` gives `0.30000000000000004`. In Yinz, `Number` stores decimal digits exactly, so you get the answer you expect:

```
let total = 0.1 + 0.2
print(total)    // 0.3
```

Use `Int` for whole numbers, `Number` for anything where the exact value matters, and `Float` only when you want fast binary arithmetic and can accept tiny rounding differences. A Python `int` has no size limit. A Yinz `Int` holds about ±9.2 × 10^18, and a result that does not fit stops the whole program with `RUNTIME ERROR: Int overflow in '<op>' at …`.

### Operators: `&&`, `||`, `!`, `==` checks types, and no `+=`

Python's `and`, `or` and `not` are `&&`, `||` and `!`:

```
function canEnter(score: Int, banned: Boolean, isAdmin: Boolean) -> Boolean {
  if (isAdmin) {
    return true
  }
  return score >= 10 && !banned
}
```

A single `|` is for unions, not for "or".

`==` compares values and never converts types silently. Python's `"5" == 5` is `False` and nothing warns you. In Yinz, comparing different types is an error:

```
let input: String = `5`
let count: Int = 5
if (input == count) { print(1) }
// COMPILE ERROR: `==` cannot be used with `String` and `Int`.
// Check that both sides have the same type.
```

Python's `int(input)` raises `ValueError` on bad text. In Yinz, `.toInt()` gives a `Maybe<Int>`:

```
let input: String = `5`
let parsed = input.toInt()
let count = parsed.or(0)
print(count)    // 5
```

`+=`, `-=`, `*=`, `/=` and `++` do not exist. Write the assignment out:

```
let score = 10
score += 5
// COMPILE ERROR: `+=` is not supported in Yinz.
// Use `x = x + n` instead.

score = score + 5
```

### Branching: no `else`, `elif` or `match`

Handle one case with an early `return`, then carry on. This replaces an `if` / `elif` / `else` chain.

Python:

```python
def describe(health):
    if health <= 0:
        return "dead"
    elif health < 30:
        return "in danger"
    else:
        return "healthy"
```

Yinz:

```
function describe(health: Int) -> String {
  if (health <= 0) {
    return `dead`
  }
  if (health < 30) {
    return `in danger`
  }
  return `healthy`
}
```

The condition always has parentheses and the block always has braces.

When you cannot return, set a value first and change it. This replaces `"healthy" if health > 50 else "in danger"`:

```
let message = `in danger`
if (health > 50) {
  message = `healthy`
}
print(message)
```

To compare one value against several cases, use a multi-case `if`. `else =>` is the catch-all. This replaces Python's `match`:

```
function describeStatus(statusCode: Int) -> Nothing {
  if (statusCode) {
    200 => {
      print(`ok`)
    }
    404 => {
      print(`not found`)
    }
    else => {
      print(`unexpected: ${statusCode}`)
    }
  }
}
```

Writing `match` gives errors, and among them you also get this warning:

```
match (statusCode) {
// COMPILE WARNING: `match` is not a keyword in Yinz.
// Use multi-case `if` with `=>` arms: `if (value) { 1 => ...; 2 => ...; else => ... }`
```

### Chaining: a comprehension becomes named steps

Yinz code puts one step on each line and gives each result a name (golden rule: *step-by-step over chaining*), so you can inspect every value. A list comprehension becomes a `for` loop that builds the result one item at a time.

Python:

```python
active_names = [player.name for player in players if player.health > 0]
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

In Yinz, write each step on its own line and give each result a name, as above.

### Files and imports: paths start at the project root

Python imports by package name, and a leading dot means "relative to this file". Yinz import paths start at your project root and are written in backticks. A leading `./` or `../` is an error:

```
import { fetchUser } from `../services/users`
// COMPILE ERROR: Import paths must be project-root relative, not relative paths.
// Write `services/users` instead of `../services/users`.
```

The correct form, with the exporting file above it:

```
// services/users.ynz
export function fetchUser(userId: Int) -> String {
  return `user ${userId}`
}
```

```
// entrypoint.ynz
import { fetchUser } from `services/users`
```

In Python, every name in a file can be imported. In Yinz, only names marked `export` can be imported, and everything else is private. There are no default exports.

Python's `if __name__ == "__main__":` has no Yinz twin. Every Yinz program starts at `function entrypoint()`:

```
function entrypoint() -> Nothing {
  print(`hello, yinz`)
}
```

In Python, a second `from ... import Player` quietly replaces the first. Yinz stops you and asks you to rename one:

```
import { Player } from `models/game`
import { Player } from `external/legacy`
// COMPILE ERROR: Import name `Player` is already bound by a previous import.
// Use a different local name: `import { Player as otherName } from "..."`
```

### Slow work: no `async def`, no `await`

You do not mark functions as `async`, and there is no `await` before each call. When a call has to pause, such as a network call, the compiler pauses your function by itself. Independent calls run at the same time without extra code, so there is no `asyncio.gather`.

Python:

```python
import asyncio

async def fetch_lower_bowl_price():
    await asyncio.sleep(0.06)
    return 24.50

async def fetch_upper_deck_price():
    await asyncio.sleep(0.06)
    return 31.75

async def main():
    lower_bowl, upper_deck = await asyncio.gather(
        fetch_lower_bowl_price(), fetch_upper_deck_price()
    )
    print(lower_bowl, upper_deck)

asyncio.run(main())
```

Yinz:

```
function fetchLowerBowlPrice() -> Number {
  wait sleep(60)
  return 24.50
}

function fetchUpperDeckPrice() -> Number {
  wait sleep(60)
  return 31.75
}

function entrypoint() -> Nothing {
  let lowerBowl = fetchLowerBowlPrice()
  let upperDeck = fetchUpperDeckPrice()
  print(`lower bowl ticket: ${lowerBowl}`)
  print(`upper deck ticket: ${upperDeck}`)
}
```

The two fetches have nothing to do with each other, so they overlap: the wait is about 60 milliseconds, not 120. The `sleep` takes milliseconds, where Python's `asyncio.sleep` takes seconds.

Your editor shows a muted `wait` hint where a call pauses. Write `wait` yourself only when two calls must finish in a set order and the compiler cannot see why. Write `background` to start a task that keeps running after your function returns.

Writing the Python words gives errors, and among them you also get these warnings:

```
async function fetchPrice() -> Number {
// COMPILE WARNING: `async` is not a keyword in Yinz.
// Write `wait foo()` at the call site — no declaration-level annotation needed.
```

```
let price = await fetchPrice()
// COMPILE WARNING: `await` is not a keyword in Yinz.
// Write `wait foo()` at the call site.
```

### Copies: `b = a` hands the value over

In Python, `b = a` gives one list two names, and a change through one name shows through the other. Yinz never lets one value have two names. Storing a value in another name, an array or a shape hands it over, and the old name cannot be read afterward.

Python:

```python
scores = [70, 85, 92]
backup = scores
backup.append(100)
print(len(scores))    # 4, because both names are one list
```

Yinz:

```
let scores: Array<Int> = [70, 85, 92]
let backup: Array<Int> = scores
let total = scores.count()
// COMPILE ERROR: `scores` was stored in `backup` and cannot be used here — storing a value hands it over.
// If you still need `scores` after the store, store a copy instead: `let backup = scores.copy()`. If you only need it before the store, put this line above the store.
```

Ask for a second value with `.copy()`:

```
let scores: Array<Int> = [70, 85, 92]
let backup = scores.copy()
backup.add(100)
let total = scores.count()
print(total)    // 3
```

The same rule applies when you store a shape value in an array:

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

To change a stored element, write through the collection: `path[0] = { x: 99, y: 2 }`.

## Where to go next

Each topic has a full reference page in [`docs/reference/`](../reference/REF-language-overview.md). Start with [Variables](../reference/REF-variables.md), [Maybe Types](../reference/REF-maybe.md) and [Errors](../reference/REF-errors.md).
