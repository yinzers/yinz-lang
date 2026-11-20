---
name: "REF-collections"
description: "Three ways to hold multiple values."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---


# Collections

Three ways to hold multiple values.

---

## The three collection types

```ynz
function entrypoint() -> Nothing {
  let colors: Fixed<String> = [`red`, `green`]    // fixed size: the size is set when you create it. Fastest.
  let roster: Array<String> = [`Alice`]           // growable: add items freely
  let scores: Map<String, Int> = { `alice`: 90 }  // key-value pairs: look up a value by its key
  print(colors.count())
  print(roster.count())
  print(scores.count())
}
```

Output:

```text
2
1
1
```

All collections are typed. Every element must be the same type.

The syntax is the same for all three: a capitalized name, then the type of the items in `<>`. No exceptions.

Most examples on this page use this shape:

```ynz
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Alice`, health: 100 }
  print(player.name)
}
```

Output:

```text
Alice
```

---

## Fixed<T> — fixed-size lists

`Fixed<T>` has a size that is set when you create it and never changes. It is the fastest option because it lives on the stack (the quick memory a function uses for its own variables) and never has to grow.

Use `Fixed<T>` when you know the size up front:

```ynz
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let rgb: Fixed<Int> = [255, 128, 0]
  let names: Fixed<String> = [`Alice`, `Bob`, `Charlie`]
  let players: Fixed<Player> = [
    { name: `Alice`, health: 100 },
    { name: `Bob`, health: 80 }
  ]
  print(rgb.count())
  print(names.count())
  print(players.count())
}
```

Output:

```text
3
3
2
```

A `Fixed<T>` has no `.add()`. Use `Array<T>` when the list can grow.

### `Fixed<T>` and `Fixed<T, N>` in function signatures

A function can take a `Fixed` list and give one back. Write the number of items after the type to say exactly how many it holds, or leave the number out and the compiler works it out.

```ynz
function total(share scores: Fixed<Int>) -> Int {
  let sum = 0
  for (score in scores) {
    sum = sum + score
  }
  return sum
}

function sumOfFour(share scores: Fixed<Int, 4>) -> Int {
  return total(scores)
}

function makeRow() -> Fixed<Int> {
  let row: Fixed<Int> = [1, 2, 3]
  return row
}

function entrypoint() -> Nothing {
  let fourScores: Fixed<Int> = [1, 2, 3, 4]
  let row = makeRow()
  print(sumOfFour(fourScores))
  print(total(row))
}
```

Output:

```text
10
6
```

- Write the length as a whole number, `Fixed<Int, 4>`, on the whole type of a **parameter, a return value or a shape field** only. A local (`let row: Fixed<Int> = [1, 2, 3]`) takes its length from the list on the right.
- `Fixed<Int>` as a parameter takes a list of any length: `total` above works for three scores or thirty.
- `Fixed<Int>` as a return type takes the length of the lists the function returns. Every `return` must give the same length.
- An `export function` must write the length of every `Fixed` it takes or returns (`Fixed<Int, 4>`), because another file sees only the signature.
- A list returned from a function that waits, or from a `background` task you `receive()`, comes back the same way.
- A function that waits, or that the compiler may run as a task of its own, needs the length written (`Fixed<Int, 4>`) when its body asks for it (`count`, a loop, an index, a `return`).

Passing a list of the wrong length is a compile error:

```ynz
function total(share scores: Fixed<Int>) -> Int {
  let sum = 0
  for (score in scores) {
    sum = sum + score
  }
  return sum
}

function sumOfFour(share scores: Fixed<Int, 4>) -> Int {
  return total(scores)
}

function entrypoint() -> Nothing {
  let scores: Fixed<Int> = [1, 2, 3]
  let result = sumOfFour(scores)
  print(result)
}
```

The compiler stops with:

```text
Error: `scores` holds 3 items, but `sumOfFour` takes exactly 4 items for `scores`.
    ╭─[ main.ynz:15:26 ]
    │
 15 │   let result = sumOfFour(scores)
    │                          ───┬──
    │                             ╰──── wrong number of items
    │
    │ Note: Pass a list of 4 items, or change `scores` to `Fixed<Int>` so `sumOfFour` takes a list of any length. — A list written `Fixed<Int, 4>` holds exactly 4 items, and the code compiled for it reads exactly that many. A list of another length would be read past its end, or leave items unread, so the compiler checks the length at every call, every `return` and every place a list is stored.
────╯
```

#### A `Fixed` list in a shape field

A shape keeps a `Fixed<T, N>` field inside itself:

```ynz
shape Roster {
  title: String
  names: Fixed<String, 2>
}

function entrypoint() -> Nothing {
  let roster: Roster = { title: `Pirates`, names: [`Alice`, `Bob`] }
  print(roster.title)
  print(roster.names.count())
}
```

Output:

```text
Pirates
2
```

- The length is required: `names: Fixed<String>` is a compile error, because the shape's size has to be known. Use `Array<String>` for a list whose length varies.
- This holds for a shape written inline (`{ trail: Fixed<Int, 3> }`) too, and a different length is a different type.
- Reading the field in a `let` gives the binding its own copy when the binding is written; the shape's `.copy()` copies the list and its items.

You CAN replace existing elements — the size stays the same:

```ynz
function entrypoint() -> Nothing {
  let rgb: Fixed<Int> = [255, 128, 0]
  rgb[1] = 200            // OK — size unchanged
  rgb.set(1, 100)         // same thing, longer form
  print(rgb[1].or(0))
}
```

Output:

```text
100
```

---

## Array<T> — growable lists

`Array<T>` can grow. It lives in the heap (flexible memory that can change size):

```ynz
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let roster: Array<Player> = []
  roster.add({ name: `Alice`, health: 100 })
  roster.add({ name: `Bob`, health: 80 })
  roster[0] = { name: `Carol`, health: 90 }    // replace the element at index 0
  let rosterSize = roster.count()              // 2
  print(rosterSize)
}
```

Output:

```text
2
```

---

## Safe access — brackets are shorthand for .get()

Reading by index uses brackets, which is shorthand for `.get()`. Both forms return `Maybe<T>` — never a bare value, so an index that is too big can't crash your program:

```ynz
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let roster: Array<Player> = [{ name: `Alice`, health: 100 }]

  let thirdPlayer = roster[3]          // shorthand for roster.get(3)
  let samePlayer = roster.get(3)       // same thing, longer form
  // both return: Maybe<Player>

  if (thirdPlayer.exists()) {
    print(thirdPlayer.value.name)
  }
  print(samePlayer.exists())
}
```

Output:

```text
false
```

Or give it a default, so you get a plain `Player` back:

```ynz
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let roster: Array<Player> = [{ name: `Alice`, health: 100 }]
  let defaultPlayer: Player = { name: `Guest`, health: 1 }
  let foundPlayer = roster.get(5)                 // a maybe: the Player, or nothing
  let player = foundPlayer.or(defaultPlayer)      // always a Player
  print(player.name)
}
```

Output:

```text
Guest
```

The same rule applies to all collections — `Array<T>`, `Fixed<T>`, `Map<K, V>`, and `String`. Every access that might not exist returns `Maybe`. An index past the end never crashes a read.

**Brackets and `.get()` both work:** brackets are the everyday shorthand. `.get()` is the long form — use it when you want the code to say exactly what it does.

---

## Writing by index

Writes use the same shorthand — brackets turn into `.set()`:

```ynz
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let roster: Array<Player> = [{ name: `Alice`, health: 100 }]
  let scores: Map<String, Int> = { `alice`: 50 }

  roster[0] = { name: `Dana`, health: 70 }      // shorthand for roster.set(0, ...)
  scores[`alice`] = 75                          // shorthand for scores.set(`alice`, 75)

  print(roster[0].or({ name: `none`, health: 0 }).name)
  print(scores[`alice`].or(0))
}
```

Output:

```text
Dana
75
```

`.set()` replaces an element that already exists. Writing past the end of an `Array<T>` stops the program when it runs; use `.add()` to put an item at the end.

---

## Storing a shape hands it over

When you store a shape value — into an array, a map, or another shape's field — the collection
becomes its owner. The name you stored it from cannot be used after that line:

```ynz
shape Position {
  x: Number
  y: Number
}

function entrypoint() -> Nothing {
  let path: Array<Position> = []
  let start: Position = { x: 1.0, y: 2.0 }
  path.add(start)
  start.x = 99.0
}
```

The compiler stops with:

```text
Error: `start` was stored in `path` and cannot be used here — storing a value hands it over.
    ╭─[ main.ynz:10:3 ]
    │
 10 │   start.x = 99.0
    │   ──┬──
    │     ╰──── handed over
    │
    │ Note: If you still need `start` after the store, store a copy instead: `path.add(start.copy())`. If you only need it before the store, put this line above the store. — Every value has one owner. Storing `start` in `path` makes `path` its owner, and when `path` is done with it, it frees it. If `start` could still read the same value, two names would reach one value and one of them would free it under the other. So a store hands the value over, and the compiler refuses any line that reads `start` afterward. Nothing happens at runtime — `start` is not cleared or set to `none`; the program simply does not compile until this read is placed above the store or the store takes a copy.
────╯
```

If you want to keep changing `start`, store a copy. The stored copy keeps the values it had when
you stored it:

```ynz
shape Position {
  x: Number
  y: Number
}

function entrypoint() -> Nothing {
  let path: Array<Position> = []
  let start: Position = { x: 1.0, y: 2.0 }
  path.add(start.copy())
  start.x = 99.0
  print(path[0].or({ x: 0.0, y: 0.0 }).x)    // the element inside path still has x == 1.0
}
```

Output:

```text
1.0
```

In Yinz, one value never has two names, so you choose: hand it over, or store a copy.

To change a stored element, write through the collection:

```ynz
shape Position {
  x: Number
  y: Number
}

function entrypoint() -> Nothing {
  let path: Array<Position> = [{ x: 1.0, y: 2.0 }]
  path[0] = { x: 99.0, y: 2.0 }
  print(path[0].or({ x: 0.0, y: 0.0 }).x)
}
```

Output:

```text
99.0
```

---

## Functions you call with a dot — on both fixed and array

Write these after the collection, like `roster.count()`:

- `.get(index)` — item at position. Gives a `Maybe<T>`: safe, it is `none` if the position is out of bounds.
- `.set(index, value)` — replace the item at that position.
- `.first()` — first item. Gives a `Maybe<T>`.
- `.last()` — last item. Gives a `Maybe<T>`.
- `.count()` — number of items. Gives an `Int`.
- `.contains(value)` — is this value in the collection? Gives a `Boolean`. Works on `Array<T>`.

`.set()` works on `Fixed<T>` because it replaces an existing element without changing size.

### Checking if a value is in a collection — `.contains(value)`

`.contains(value)` tells you whether a value is in an `Array<T>`:

```ynz
function entrypoint() -> Nothing {
  let scores: Array<Int> = [70, 85, 92]
  let hasPerfect = scores.contains(100)
  print(hasPerfect)
}
```

Output:

```text
false
```

It works on shape values too. A shape value counts as "in the collection" when every field
matches — but the two kinds of fields match differently:

- **`Number`, `Boolean` and `String` fields** match when their values are equal. Two strings match when they hold the same text.
- **Fields holding another shape or a collection** match only when both fields hold the very same stored value, not just one that reads the same.

When every field is a number, it works the way you'd expect:

```ynz
shape Position {
  x: Number
  y: Number
}

function entrypoint() -> Nothing {
  let path: Array<Position> = [
    { x: 0.0, y: 0.0 },
    { x: 5.0, y: 3.0 },
  ]

  let step: Position = { x: 5.0, y: 3.0 }
  let visited = path.contains(step)
  print(visited)    // step's fields match the second entry, even though step was built separately
}
```

Output:

```text
true
```

String fields match by their text, so a string built at runtime matches too:

```ynz
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let team: Array<Player> = [
    { name: `Alice`, health: 100 },
  ]

  let prefix: String = `Ali`
  let lookalike: Player = { name: `${prefix}ce`, health: 100 }
  let found = team.contains(lookalike)
  print(found)    // both names read `Alice`
}
```

Output:

```text
true
```

---

## Functions you call with a dot — array only

This one changes the array itself, so it only works on a list that can change size:

- `.add(item)` — add an item at the end.

---

## Map<K, V> — key-value pairs

Use maps when you don't know the keys until the program runs. For fields you know in advance, define a `shape` — it's faster.

```ynz
function entrypoint() -> Nothing {
  let wordCounts: Map<String, Int> = {}
  let words: Array<String> = [`ahead`, `behind`, `ahead`]
  for (word in words) {
    let foundCount = wordCounts[word]         // a maybe: the count, or nothing
    let current = foundCount.or(0)            // a plain number, 0 if the word is new
    wordCounts[word] = current + 1
  }
  let aheadCount = wordCounts[`ahead`]
  print(aheadCount.or(0))
}
```

Output:

```text
2
```

Functions you call on a map:

- `.get(key)` — gives a `Maybe<V>` (might not exist — see [Maybe Types](REF-maybe.md)).
- `.set(key, value)` — add or update one key.
- `.has(key)` — does the key exist? Gives a `Boolean`.
- `.count()` — number of entries. Gives an `Int`.

**Brackets work on maps too:**

```ynz
function entrypoint() -> Nothing {
  let scores: Map<String, Int> = { `alice`: 50 }

  let before = scores[`alice`]    // shorthand for scores.get(`alice`) → Maybe<Int>
  scores[`alice`] = 75            // shorthand for scores.set(`alice`, 75)
  let after = scores.get(`alice`)
  print(before.or(0))
  print(after.or(0))
  print(scores.has(`bob`))
}
```

Output:

```text
50
75
false
```

**A `Number` key matches the number as stored.** A map keeps a `Number` key in its own slot and finds a key by comparing the stored digits, so `1.0` set twice is one key, while `1.0` and `1.00` are two (`1.0 == 1.00` is still `true`; a map key does not use `==`). A key computed to the same value and scale, such as `0.5 + 0.5` for `1.0`, is the same key. `.contains` on an `Array<Number>` still compares with `==`, so it finds `1.00` for `1.0`.

**A map keeps each key in one slot.** The slot is 8 bytes for an `Int`, a `Boolean`, a `Float` or a `String`, and 16 bytes for a `Number` of up to 34 digits. Any other key type is refused at compile time. A `Number` of more than 34 digits takes as much memory as its digits need, kept outside the map, so it does not fit the slot; a shape, an array and the other types are made of other values, so a slot could hold only where they live. Either way a lookup with a key equal to one you stored would miss, because it lives at another address:

```ynz
function entrypoint() -> Nothing {
  let totals: Map<Number<50>, Int> = {}
  print(totals.count())
}
```

The compiler stops with:

```text
Error: A `Map` cannot be keyed by `Number<50>`.
   ╭─[ main.ynz:2:15 ]
   │
 2 │   let totals: Map<Number<50>, Int> = {}
   │               ──────────┬─────────
   │                         ╰─────────── can't be a map key
   │
   │ Note: Key the map by an `Int` or a `String` id, or by a `Number` of at most 34 digits (`Number<34>`). — A map keeps a `Number` key inside its own 16-byte slot, which holds up to 34 digits. A `Number<50>` can have up to 50 digits and takes as much memory as its digits need, kept outside the map, so it does not fit in the slot. The slot could hold only its address, and looking up a `Number<50>` equal to one you stored would miss, because it lives at another address.
───╯
```

```ynz
shape Position {
  x: Number
  y: Number
}

function entrypoint() -> Nothing {
  let labels: Map<Position, String> = {}
  print(labels.count())
}
```

The compiler stops with:

```text
Error: A `Map` can't be keyed by `Position` yet.
   ╭─[ main.ynz:7:15 ]
   │
 7 │   let labels: Map<Position, String> = {}
   │               ──────────┬──────────
   │                         ╰──────────── can't be a map key
   │
   │ Note: Key the map by one of its fields (an `Int` id or a `String` name) and keep the `Position` as the map's value: `Map<String, Position>`. — Today a map keeps each key in one slot of 8 bytes (for an `Int`, a `Boolean`, a `Float`, or a `String`, which is compared by its text) or of 16 bytes (for a `Number` of up to 34 digits). A `Position` is a shape, which holds several fields rather than one value, so its slot could hold only the address where it lives, and looking up a `Position` equal to one you stored would miss, because it lives at another address. Keys like this are not supported yet.
───╯
```

```ynz
function entrypoint() -> Nothing {
  let sessions: Map<sensitive String, Int> = {}
  print(sessions.count())
}
```

The compiler stops with:

```text
Error: A `Map` cannot be keyed by `sensitive String`.
   ╭─[ main.ynz:2:17 ]
   │
 2 │   let sessions: Map<sensitive String, Int> = {}
   │                 ─────────────┬────────────
   │                              ╰────────────── can't be a map key
   │
   │ Note: Key the map by a plain `String` or an `Int` that is not a secret, such as an id or a name, and keep the secret as the map's value: `Map<String, sensitive String>`. — A `sensitive String` is a secret. A map would free a key as an ordinary string and would compare keys with a comparison that is not constant-time: it takes longer the more leading bytes two keys share, so how long a lookup takes could reveal part of a secret.
───╯
```

A `sensitive String` is refused for a different reason than its slot: a map would free the key as an ordinary string and compare keys with a comparison that is not constant-time. A `sensitive String` key stays refused.

---

## Dot is for shapes; brackets are for collections

One rule decides whether to use `.` or `[]`:

| Access | What it does |
|--------|--------------|
| `player.fieldName` | Reads a field of a shape. The name is fixed when you write the code. |
| `player.functionName()` | Calls a function on a shape or collection |
| `roster[index]` | Reads by position → `Maybe<T>` (shorthand for `.get(index)`) |
| `` scores[`key`] `` | Reads by key → `Maybe<V>` (shorthand for `.get(key)`) |
| `word[index]` | Reads one character of a string → `Maybe<String>` (shorthand for `.get(index)`) |

Shapes and collections are different things. Using brackets on a shape fails:

```ynz
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Alice`, health: 100 }
  let name = player[`name`]
  print(name)
}
```

The compiler stops with:

```text
Error: `Player` does not support bracket access.
   ╭─[ main.ynz:8:14 ]
   │
 8 │   let name = player[`name`]
   │              ───────┬──────
   │                     ╰──────── Bracket access works on `Array<T>`, `Fixed<T>`, `Map<K, V>`, and `String`.
   │
   │ Note: Use `.get(index)` on built-in collections or access shape fields with dot notation.
───╯
```

So does a dot used to read a map key:

```ynz
function entrypoint() -> Nothing {
  let scores: Map<String, Int> = { `alice`: 50 }
  let aliceScore = scores.alice
  print(aliceScore)
}
```

The compiler stops with:

```text
Error: Cannot use `.alice` to look up a map key.
   ╭─[ main.ynz:3:27 ]
   │
 3 │   let aliceScore = scores.alice
   │                           ──┬──
   │                             ╰──── Map keys are runtime values — use `m["alice"]` to look up a key. For checking existence, use `m.has("alice")`.
   │
   │ Note: Dot access is for shape fields with compile-time-known names. Map keys are dynamic — use bracket syntax.
───╯
```

The simple rule: **use a dot for names you write in the code; use brackets for keys and positions you look up while the program runs.**

---

## Use shapes instead of maps for known fields

If you know all the keys when you write the code, use a `shape`. Reading a field is faster than looking up a key:

```ynz
shape Stats {
  health: Int
  attack: Int
}

function entrypoint() -> Nothing {
  // Known fields in a map
  let mapStats: Map<String, Int> = { `health`: 100, `attack`: 50 }

  // A shape
  let stats: Stats = { health: 100, attack: 50 }
  print(mapStats.count())
  print(stats.health)
}
```

Output:

```text
2
100
```

---

## Nested collections — name the inner shape

Don't nest collection types inline. Name the inner shape:

```ynz
shape PlayerScores {
  kills: Int
  deaths: Int
  assists: Int
}

function entrypoint() -> Nothing {
  let scoreboard: Map<String, PlayerScores> = {
    `alice`: { kills: 10, deaths: 3, assists: 7 },
    `bob`: { kills: 5, deaths: 8, assists: 12 }
  }
  print(scoreboard.count())
}
```

Output:

```text
2
```

---

## Step-by-step operations — no chaining

Each operation gets its own line with a named variable. The name documents what that step produces:

```ynz
function entrypoint() -> Nothing {
  let scores: Array<Int> = [45, 80, 92, 60]
  let passing: Array<Int> = []
  for (score in scores) {
    if (score >= 60) {
      passing.add(score)
    }
  }
  let passingCount = passing.count()    // 3
  print(passingCount)
}
```

Output:

```text
3
```

---

## Common mistakes

**Collection types start with a capital letter — `array`, `fixed` and `map` are not type names:**

```ynz
function entrypoint() -> Nothing {
  let scores: array<Int> = []
  print(scores.count())
}
```

The compiler stops with:

```text
Error: `array` is not a type name.
   ╭─[ main.ynz:2:15 ]
   │
 2 │   let scores: array<Int> = []
   │               ──┬──
   │                 ╰──── write `Array`
   │
   │ Note: Write `Array`. — Every type in Yinz starts with a capital letter, built-ins included.
───╯
```

A capital letter always means a type, whether it is built in (`Array`, `Fixed`, `Map`) or a shape you wrote.

**A shape value is not an array — use `[...]` to make a collection:**

```ynz
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let team: Array<Player> = { name: `Alice`, health: 100 }
  print(team.count())
}
```

The compiler stops with:

```text
Error: `{ ... }` creates a single `Player` value, not an `Array<Player>`.
   ╭─[ main.ynz:7:29 ]
   │
 7 │   let team: Array<Player> = { name: `Alice`, health: 100 }
   │                             ───────────────┬──────────────
   │                                            ╰──────────────── Put it inside `[...]` to make an array: `[{ ... }]`
   │
   │ Note: `{ ... }` creates one value. `[...]` creates a collection. Use `Array<Player>` when you need multiple values.
───╯
```

**A shape is not a collection:**

```ynz
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let team: Player = []
  print(team.name)
}
```

The compiler stops with:

```text
Error: `[]` is an array literal, but `Player` is a shape — a single value, not a collection.
   ╭─[ main.ynz:7:22 ]
   │
 7 │   let team: Player = []
   │                      ─┬
   │                       ╰── Use `Array<Player>` if you want a list: `let ... : Array<Player> = []`
   │
   │ Note: `Player` holds one value. To store multiple `Player` values, use `Array<Player>`.
───╯
```
