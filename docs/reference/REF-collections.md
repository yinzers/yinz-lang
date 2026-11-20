---
name: "REF-collections"
description: "Three ways to hold multiple values."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Collections

Three ways to hold multiple values.

---

## The three collection types

```
fixed<string>           // fixed size: the size is set when you create it. Fastest.
array<Player>           // growable: add and remove items freely
map<string, number>     // key-value pairs: look up a value by its key
```

All collections are typed. Every element must be the same type:

```
let stuff = [42, `hello`, true]
// COMPILE ERROR: Cannot mix types in a collection. Pick one type.
```

The syntax is the same for all three: `kind<type>`. No exceptions.

---

## fixed<T> — fixed-size lists

`fixed<T>` has a size that is set when you create it and never changes. It is the fastest option because it lives on the stack (the quick memory a function uses for its own variables) and never has to grow.

Use `fixed<T>` when you know the size up front:

```
let rgb: fixed<number> = [255, 128, 0]
let names: fixed<string> = [`Alice`, `Bob`, `Charlie`]
let players: fixed<Player> = [
  { name: `Alice`, health: 100 },
  { name: `Bob`, health: 80 }
]
```

You cannot add or remove elements:

```
rgb.add(50)
// COMPILE ERROR: Cannot add to a fixed array.
// fixed<number> is size-locked. Use array<number> if it needs to grow.
```

Pick `array<T>` when the list can grow.

### `fixed<T>` and `fixed<T, N>` in function signatures

A function can take a `fixed` list and give one back. Write the number of items after the type to say exactly how many it holds, or leave the number out and the compiler works it out.

```
function total(share scores: fixed<int>) -> int {
  let sum = 0
  for (score in scores) {
    sum = sum + score
  }
  return sum
}

function sumOfFour(share scores: fixed<int, 4>) -> int {
  return total(scores)
}

function makeRow() -> fixed<int> {
  let row: fixed<int> = [1, 2, 3]
  return row
}
```

- Write the length as a whole number, `fixed<int, 4>`, on the whole type of a **parameter, a return value or a shape field** only. A local (`let row: fixed<int> = [1, 2, 3]`) takes its length from the list on the right.
- `fixed<int>` as a parameter takes a list of any length: `total` above works for three scores or thirty.
- `fixed<int>` as a return type takes the length of the lists the function returns. Every `return` must give the same length.
- An `export function` must write the length of every `fixed` it takes or returns (`fixed<int, 4>`), because another file sees only the signature.
- A list returned from a function that waits, or from a `background` task you `receive()`, comes back the same way.
- A function that waits, or that the compiler may run as a task of its own, needs the length written (`fixed<int, 4>`) when its body asks for it (`count`, a loop, an index, a `return`).

Passing a list of the wrong length is a compile error:

```
let scores: fixed<int> = [1, 2, 3]
let result = sumOfFour(scores)
// COMPILE ERROR: `scores` holds 3 items, but `sumOfFour` takes exactly 4 for `scores`.
// Pass a list of 4 items, or change `scores` to `fixed<int>` so `sumOfFour` takes a list of any length.
```

#### A `fixed` list in a shape field

A shape keeps a `fixed<T, N>` field inside itself:

```
shape Roster {
  title: string
  names: fixed<string, 2>
}
```

- The length is required: `names: fixed<string>` is a compile error, because the shape's size has to be known. Use `array<string>` for a list whose length varies.
- This holds for a shape written inline (`{ trail: fixed<int, 3> }`) too, and a different length is a different type.
- Reading the field in a `let` gives the binding its own copy when the binding is written; the shape's `.copy()` copies the list and its items.

You CAN replace existing elements — the size stays the same:

```
rgb[1] = 200            // OK — size unchanged
rgb.set(1, 200)         // same thing, longer form
```

---

## array<T> — growable lists

`array<T>` can grow and shrink. It lives in the heap (flexible memory that can change size):

```
let roster: array<Player> = []
roster.add({ name: `Alice`, health: 100 })
roster.add({ name: `Bob`, health: 80 })
roster.remove(0)        // remove by index
roster[0] = newPlayer   // replace element at index 0
```

---

## Safe access — brackets are shorthand for .get()

Reading by index uses brackets, which is shorthand for `.get()`. Both forms return `maybe<T>` — never a bare value, so an index that is too big can't crash your program:

```
let thirdPlayer = players[3]         // shorthand for players.get(3)
let samePlayer = players.get(3)      // same thing, longer form
// both return: maybe<Player>

if (thirdPlayer.exists()) {
  print(thirdPlayer.value.name)
}
```

Or give it a default, so you get a plain `Player` back:

```
let foundPlayer = players.get(5)                // a maybe: the Player, or nothing
let player = foundPlayer.or(defaultPlayer)      // always a Player
```

The same rule applies to all collections — `array<T>`, `fixed<T>`, `map<K, V>`, and `string`. Every access that might not exist returns `maybe`. An index past the end never crashes a read.

**Brackets and `.get()` both work:** brackets are the everyday shorthand. `.get()` is the long form — use it when you want the code to say exactly what it does.

---

## Writing by index

Writes use the same shorthand — brackets turn into `.set()`:

```
players[2] = newPlayer       // shorthand for players.set(2, newPlayer)
scores[`alice`] = 75          // shorthand for scores.set(`alice`, 75)
```

Writing past the end is an error when the program runs. It does not leave empty slots behind:

```
let scores: array<number> = [1, 2, 3]
scores[10] = 99
// RUNTIME ERROR: index 10 out of bounds (scores.count() == 3).
//   .set() replaces existing elements only.
//   To append: use .add(99).
//   To grow with default values: use .resize(newSize, defaultValue).
```

Fixed arrays catch out-of-bounds at compile time when the index is a literal:

```
let rgb: fixed<number> = [255, 128, 0]
rgb[5] = 100
// COMPILE ERROR: index 5 out of bounds. rgb has 3 elements.
```

---

## Storing a shape hands it over

When you store a shape value — into an array, a map, or another shape's field — the collection
becomes its owner. The name you stored it from cannot be used after that line:

```
shape Position {
  x: number
  y: number
}

let path: array<Position> = []
let start: Position = { x: 1.0, y: 2.0 }
path.add(start)
start.x = 99.0
// COMPILE ERROR: `start` was stored in `path` and cannot be used here — storing a value hands it over.
//   If you still need `start` after the store, store a copy instead: `path.add(start.copy())`.
//   If you only need it before the store, put this line above the store.
```

If you want to keep changing `start`, store a copy. The stored copy keeps the values it had when
you stored it:

```
path.add(start.copy())
start.x = 99.0
// The element inside path still has x == 1.0.
```

If you're coming from JavaScript, watch out: in JavaScript, the array and `start` would share one
object, so setting `start.x = 99.0` would change what the array sees. In Yinz, one value never has
two names, so you choose: hand it over, or store a copy.

To change a stored element, write through the collection:

```
path[0] = { x: 99.0, y: 2.0 }
```

---

## Functions you call with a dot — on both fixed and array

Write these after the collection, like `players.count()`:

```
.filter(fn)         // filter — returns new collection where fn returns true
.sort(fn, order)    // sorted copy — order is asc or desc (see Options)
.map(fn)            // transform — returns new collection of results
.get(index)         // item at position → maybe<T> (safe, returns none if out of bounds)
.set(index, value)  // replace item at position — runtime error if out of bounds
.first()            // first item → maybe<T>
.last()             // last item → maybe<T>
.find(fn)           // first item matching condition → maybe<T>
.count()            // number of items → int
.unique()           // deduplicated copy
.limit(n)           // cap to the first N items — returns new collection of at most N
.contains(value)    // is this value in the collection? → boolean
.concat(other)      // combined copy — does not modify original
.append(item)       // new collection with item added at the end
.prepend(item)      // new collection with item added at the front
```

`.concat()`, `.append()`, and `.prepend()` work on `fixed<T>` because they return new collections. The original stays untouched.

`.set()` works on `fixed<T>` because it replaces an existing element without changing size.

### Checking if a value is in a collection — `.contains(value)`

`.contains(value)` tells you whether a value is in the collection:

```
let scores: fixed<number> = [70, 85, 92]
let hasPerfect = scores.contains(100)
// hasPerfect is false
```

It works on shape values too. A shape value counts as "in the collection" when every field
matches — but the two kinds of fields match differently:

- **number and boolean fields** match when their values are equal.
- **string fields** — and fields holding another shape or a collection — match only when both
  fields hold the very same stored value, not just one that reads the same.

When every field is a number, it works the way you'd expect:

```
shape Position {
  x: number
  y: number
}

let path: array<Position> = [
  { x: 0, y: 0 },
  { x: 5, y: 3 },
]

let step: Position = { x: 5, y: 3 }
let visited = path.contains(step)
// visited is true — step's fields match the second entry,
// even though step was built separately
```

String fields are stricter. A string built at runtime is its own stored value, so it does not
match — even when the text is identical:

```
shape Player {
  name: string
  health: int
}

let roster: array<Player> = [
  { name: `Alice`, health: 100 },
]

let prefix: string = `Ali`
let lookalike: Player = { name: `${prefix}ce`, health: 100 }
let found = roster.contains(lookalike)
// found is false — lookalike's name was built at runtime, so it is not
// the same stored string as the roster entry's name, even though both
// read `Alice`
```

To match text by what it says, use `.find(fn)` instead — `==` on strings compares the text
itself:

```
let alice = roster.find(player => player.name == `Alice`)
// alice.exists() is true
```

Want to match on a condition instead of a whole value? Use `.find(fn)` the same way.

---

## Functions you call with a dot — array only

These change the array itself, so they only work on a list that can change size:

```
.add(item)              // add item at end
.remove(index)          // remove by index
.removeFirst()          // remove and return first item → maybe<T>
.removeLast()           // remove and return last item → maybe<T>
```

---

## map<K, V> — key-value pairs

Use maps when you don't know the keys until the program runs. For fields you know in advance, define a `shape` — it's faster.

```
let wordCounts: map<string, number> = {}
for (word in words) {
  let foundCount = wordCounts[word]         // a maybe: the count, or nothing
  let current = foundCount.or(0)            // a plain number, 0 if the word is new
  wordCounts[word] = current + 1
}
```

Functions you call on a map:

```
.get(key)             // → maybe<V> (might not exist — see Maybe Types)
.set(key, value)      // add or update one key
.update({...})        // add or update multiple keys at once
.has(key)             // does the key exist? → boolean
.remove(key)          // delete by key
.keys()               // → array<K>
.values()             // → array<V>
.entries()            // → array of key-value pairs (each has .key and .value)
.sort(fn, order)      // sorted copy
.filter(fn)           // new map with only matching entries — fn receives entry with .key and .value
.find(fn)             // first matching entry → maybe entry (has .key and .value)
.count()              // → number of entries (int)
```

**Brackets work on maps too:**

```
scores[`alice`]              // shorthand for scores.get(`alice`) → maybe<number>
scores[`alice`] = 75         // shorthand for scores.set(`alice`, 75)
```

**A `number` key matches the number as stored.** A map keeps a `number` key in its own slot and finds a key by comparing the stored digits, so `1.0` set twice is one key, while `1.0` and `1.00` are two (`1.0 == 1.00` is still `true`; a map key does not use `==`). A key computed to the same value and scale, such as `0.5 + 0.5` for `1.0`, is the same key. Matching `1.0` and `1.00` as one key is not decided yet. `.contains` on an `array<number>` still compares with `==`, so it finds `1.00` for `1.0`.

**A map keeps each key in one slot.** The slot is 8 bytes for an `int`, a `boolean`, a `float` or a `string`, and 16 bytes for a `number` of up to 34 digits. Any other key type is refused at compile time. A `number` of more than 34 digits takes as much memory as its digits need, kept outside the map, so it does not fit the slot; a shape, an array and the other types are made of other values, so a slot could hold only where they live (a shape or array key is not supported yet). Either way a lookup with a key equal to one you stored would miss, because it lives at another address:

```
let totals: map<number<50>, int> = {}
// COMPILE ERROR: A `map` cannot be keyed by `number<50>`.
// Key the map by an `int` or a `string` id, or by a `number` of at most 34 digits (`number<34>`).

let labels: map<Point, string> = {}
// COMPILE ERROR: A `map` can't be keyed by `Point` yet.
// Key the map by one of its fields (an `int` id or a `string` name) and keep the `Point`
// as the map's value: `map<string, Point>`.

let sessions: map<sensitive string, int> = {}
// COMPILE ERROR: A `map` cannot be keyed by `sensitive string`.
// Key the map by a plain `string` or an `int` that is not a secret, such as an id or a name,
// and keep the secret as the map's value: `map<string, sensitive string>`.
```

A `sensitive string` is refused for a different reason than its slot: a map would free the key as an ordinary string and compare keys with a comparison that is not constant-time. A `sensitive string` key stays refused: that is by design, not a missing feature.

**Bulk update with `.update({...})`:**

```
let scores: map<string, number> = { alice: 50, bob: 60 }

scores.update({ alice: 75, charlie: 40 })
// Result: alice=75 (updated), bob=60 (untouched), charlie=40 (added)
```

**Filtering and finding in a map:**

```
let expensive = prices.filter(entry => entry.value > 100)       // new map — only items over $100
let aliceEntry = scores.find(entry => entry.key == `alice`)      // maybe entry

if (aliceEntry.exists()) {
  print(`${aliceEntry.value.key}: ${aliceEntry.value.value}`)
}
```

---

## Dot is for shapes; brackets are for collections

One rule decides whether to use `.` or `[]`:

| Access | What it does |
|--------|--------------|
| `player.fieldName` | Reads a field of a shape. The name is fixed when you write the code. |
| `player.functionName()` | Calls a function on a shape or collection |
| `players[index]` | Reads by position → `maybe<T>` (shorthand for `.get(index)`) |
| `` scores[`key`] `` | Reads by key → `maybe<V>` (shorthand for `.get(key)`) |
| `word[index]` | Reads one character of a string → `maybe<string>` (shorthand for `.get(index)`) |

Shapes and collections are different things. Using brackets on a shape, or a dot to read a map key, both fail:

```
shape Player { name: string, health: number }
let player: Player = { name: `Alice`, health: 100 }
let name = player[`name`]
// COMPILE ERROR: Bracket access is for collections (array, fixed, map, string).
//                Use dot access on types: player.name

let scores: map<string, number> = { alice: 50 }
scores.alice
// COMPILE ERROR: Dot access is for methods, not map keys.
//   Use scores[`alice`] for the value at the `alice` key.
//   If you actually have a fixed set of known keys, consider using a shape
//   instead — it's faster AND gives you dot access:
//
//     shape Scores { alice: number, bob: number }
//     scores.alice         // works — alice is a compile-time field
//
//   See "Use shapes instead of maps for known fields" below.
```

The simple rule: **use a dot for names you write in the code; use brackets for keys and positions you look up while the program runs.**

---

## Use shapes instead of maps for known fields

If you know all the keys when you write the code, use a `shape`. Reading a field is faster than looking up a key:

```
// Slower — known fields in a map
let stats: map<string, number> = { health: 100, attack: 50 }

// Faster — a shape
shape Stats { health: number, attack: number }
let stats: Stats = { health: 100, attack: 50 }
```

The compiler will suggest this when it sees a map with all-literal keys.

---

## Nested collections — name the inner shape

Don't nest collection types inline. Name the inner shape:

```
// Hard to read
let data: map<string, map<string, number>> = { ... }

// Clear — name each layer
shape PlayerScores {
  kills: number
  deaths: number
  assists: number
}
let scoreboard: map<string, PlayerScores> = {
  alice: { kills: 10, deaths: 3, assists: 7 },
  bob: { kills: 5, deaths: 8, assists: 12 }
}
```

---

## Step-by-step operations — no chaining

Each operation gets its own line with a named variable. The name documents what that step produces:

```
let active = players.filter(player => player.health > 0)
let ranked = active.sort(player => player.health, desc)
let top = ranked.limit(10)
let names = top.map(player => player.name)
```

The compiler sees that each result is only used once, so it combines the four steps into one fast pass and builds no temporary lists. You get readable code AND fast code.

Early returns fit naturally into this pattern:

```
function getTopNames(share players: fixed<Player>, limitCount: int) -> array<string> {
  let active = players.filter(player => player.health > 0)

  if (active.count() == 0) {
    return []
  }

  let ranked = active.sort(player => player.health, desc)
  let top = ranked.limit(limitCount)
  return top.map(player => player.name)
}
```

---

## Common mistakes

**Collection types are lowercase — `Array` and `Fixed` are not valid:**

```
let scores: Array<int> = []
// COMPILE ERROR: `Array` is not a type — built-in collection types are lowercase in Yinz.
// Use `array` (lowercase): `array<int>`
```

Capital letter = user-defined shape. Lowercase = built-in. `array`, `fixed`, `map` are all lowercase.

**A shape value is not an array — use `[...]` to make a collection:**

```
shape Player { name: string, health: int }

let team: array<Player> = { name: `Alice`, health: 100 }
// COMPILE ERROR: `{ ... }` creates a single `Player` value, not an `array<Player>`.
// Put it inside `[...]` to make an array: [{ name: `Alice`, health: 100 }]
```

**A shape is not a collection:**

```
let team: Player = []
// COMPILE ERROR: `[]` is an array literal, but `Player` is a shape — a single value, not a collection.
// Use `array<Player>` if you want a list: `let team: array<Player> = []`
```
