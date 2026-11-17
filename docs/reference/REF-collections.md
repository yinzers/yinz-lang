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
fixed<string>           // fixed size, stack-allocated — size locked at creation, fastest
array<Player>           // growable, heap-allocated — add and remove freely
map<string, number>     // key-value pairs with dynamic keys
```

All collections are typed. Every element must be the same type:

```
let stuff = [42, "hello", true]
// COMPILE ERROR: Cannot mix types in a collection. Pick one type.
```

The syntax is the same for all three: `kind<type>`. No exceptions.

---

## fixed<T> — fixed-size arrays

`fixed<T>` is stack-allocated and size-locked at creation. The fastest option — no heap, no growth tracking.

Use `fixed<T>` when you know the size up front:

```
let rgb: fixed<number> = [255, 128, 0]
let names: fixed<string> = ["Alice", "Bob", "Charlie"]
let players: fixed<Player> = [
  { name: "Alice", health: 100 },
  { name: "Bob", health: 80 }
]
```

You cannot add or remove elements:

```
rgb.add(50)
// COMPILE ERROR: Cannot add to a fixed array.
// fixed<number> is size-locked. Use array<number> if it needs to grow.
```

### `fixed<T>` and `fixed<T, N>` in function signatures

A function can take and return a `fixed` list. Its length is a number the compiler knows, so passing or returning a list costs what passing or returning an array costs in Rust (`[T; N]`): the cells, nothing else.

```
function total(share nums: fixed<int>) -> int { ... }       // any length: one compiled version per length you pass
function sum4(share nums: fixed<int, 4>) -> int { ... }     // exactly 4 items: any other length is a compile error
function makeRow() -> fixed<int> {                          // the length is the one every `return` gives
  let row: fixed<int> = [1, 2, 3]
  return row
}
```

- Write the length as a whole number, `fixed<int, 4>`, on the whole type of a **parameter, a return value or a shape field** only. A local (`let row: fixed<int> = [...]`) takes its length from the list on the right.
- `fixed<int>` as a parameter works for lists of any length: the compiler builds one version of the function for each length the program passes.
- `fixed<int>` as a return type takes the length of the lists the function returns. Every `return` must give the same length.
- An `export function` must write the length of every `fixed` it takes or returns (`fixed<int, 4>`): another file sees only the signature.
- A list returned from a function that waits (a suspending function), or from a `background` task you `receive()`, comes back the same way.
- A function that waits, or that the compiler may run as a task of its own, is built only once: if its body uses the length of a `fixed` parameter (`count`, a loop, an index, a `return`), write the length (`fixed<int, 4>`).

#### A `fixed` list in a shape field

A shape keeps a `fixed<T, N>` field inside its own bytes, as a Rust struct keeps a `[T; N]`:

```
shape Roster {
  title: string
  names: fixed<string, 2>
}
```

- The length is required: `names: fixed<string>` is a compile error (the shape's size has to be known). Use `array<string>` for a list whose length varies.
- This holds for a shape written inline (`{ trail: fixed<int, 3> }`) too, and a different length is a different type.
- Reading the field in a `let` gives the binding its own copy when the binding is written; the shape's `.copy()` copies the list and its items.

You CAN replace existing elements — the size stays the same:

```
rgb[1] = 200            // OK — size unchanged
rgb.set(1, 200)         // same thing, longer form
```

---

## array<T> — growable arrays

`array<T>` is heap-allocated and can grow and shrink:

```
let roster: array<Player> = []
roster.add({ name: "Alice", health: 100 })
roster.add({ name: "Bob", health: 80 })
roster.remove(0)        // remove by index
roster[0] = newPlayer   // replace element at index 0
```

---

## Safe access — brackets are sugar for .get()

Reading by index uses brackets, which is shorthand for `.get()`. Both forms return `maybe<T>` — never a raw value, so you can't accidentally crash on out-of-bounds:

```
let player = players[3]              // sugar for players.get(3)
let player = players.get(3)          // same thing, longer form
// both return: maybe<Player>

if (player.exists()) {
  print(player.value.name)
}

// Or with a default
let player = players.get(5).or(defaultPlayer)   // always returns a Player
```

The same rule applies to all collections — `array<T>`, `fixed<T>`, `map<K, V>`, and `string`. Every access that might not exist returns `maybe`. No out-of-bounds crashes anywhere in the language.

**Why brackets and `.get()` both exist:** brackets are the everyday shorthand familiar to anyone who's used JS, Python, or any C-family language. `.get()` is the long form — useful when you want the operation to be explicit, or when chaining with `.or(default)`.

---

## Writing by index

Writes use the same sugar — brackets desugar to `.set()`:

```
players[2] = newPlayer       // sugar for players.set(2, newPlayer)
scores["alice"] = 75          // sugar for scores.set("alice", 75)
```

Writing past the end is a runtime error, not a sparse-array creation:

```
let arr: array<number> = [1, 2, 3]
arr[10] = 99
// RUNTIME ERROR: index 10 out of bounds (arr.count() == 3).
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
let p: Position = { x: 1.0, y: 2.0 }
path.add(p)
p.x = 99.0
// COMPILE ERROR: `p` was stored in `path` and cannot be used here — storing a value hands it over.
//   If you still need `p` after the store, store a copy instead: `path.add(p.copy())`.
//   If you only need it before the store, put this line above the store.
```

If you want to keep changing `p`, store a copy. The stored copy keeps the values it had when
you stored it:

```
path.add(p.copy())
p.x = 99.0
// The element inside path still has x == 1.0.
```

If you're coming from JavaScript, watch out: in JavaScript, the array and `p` would share one
object, so setting `p.x = 99.0` would change what the array sees. In Yinz, one value never has
two names, so you choose: hand it over, or store a copy.

To change a stored element, write through the collection:

```
path[0] = { x: 99.0, y: 2.0 }
```

---

## Dot methods — available on both fixed and array

```
.filter(fn)         // filter — returns new collection where fn returns true
.sort(fn, order)    // sorted copy — order is asc or desc (see Options)
.map(fn)            // transform — returns new collection of results
.get(index)         // item at position → maybe<T> (safe, returns none if out of bounds)
.set(index, value)  // replace item at position — runtime error if out of bounds
.first()            // first item → maybe<T>
.last()             // last item → maybe<T>
.find(fn)           // first item matching condition → maybe<T>
.count()            // number of items → number
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
// read "Alice"
```

To match text by what it says, use `.find(fn)` instead — `==` on strings compares the text
itself:

```
let alice = roster.find(p => p.name == `Alice`)
// alice.exists() is true
```

Want to match on a condition instead of a whole value? Use `.find(fn)` the same way.

---

## Dot methods — array only

These mutate in place, which requires dynamic sizing:

```
.add(item)              // add item at end
.remove(index)          // remove by index
.removeFirst()          // remove and return first item → maybe<T>
.removeLast()           // remove and return last item → maybe<T>
```

---

## map<K, V> — key-value pairs

Use maps when keys aren't known at compile time. For known fields, define a `shape` — it's faster.

```
let wordCounts: map<string, number> = {}
for (word in words) {
  let current = wordCounts[word].or(0)
  wordCounts[word] = current + 1
}
```

Map dot methods:

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
.count()              // → number of entries
```

**Bracket sugar works on maps too:**

```
scores["alice"]              // sugar for scores.get("alice") → maybe<number>
scores["alice"] = 75         // sugar for scores.set("alice", 75)
```

**A `number` key matches the number as stored.** A map keeps a `number` key in its own slot and finds a key by comparing the stored digits, so `1.0` set twice is one key, while `1.0` and `1.00` are two (`1.0 == 1.00` is still `true`; a map key does not use `==`). A key computed to the same value and scale, such as `0.5 + 0.5` for `1.0`, is the same key. Matching `1.0` and `1.00` as one key is not decided yet. `.contains` on an `array<number>` still compares with `==`, so it finds `1.00` for `1.0`.

**A map keeps each key in one slot.** The slot is 8 bytes for an `int`, a `boolean`, a `float` or a `string`, and 16 bytes for a `number` of up to 34 digits. Any other key type is refused at compile time. A `number` of more than 34 digits takes as much memory as its digits need, kept outside the map, so it does not fit the slot; a shape, an array and the other types are made of other values, so a slot could hold only where they live. Either way a lookup with a key equal to one you stored would miss, because it lives at another address:

```
let totals: map<number<50>, int> = {}
// COMPILE ERROR: A `map` cannot be keyed by `number<50>`.
// Key the map by an `int` or a `string` id, or by a `number` of at most 34 digits (`number<34>`).

let labels: map<Point, string> = {}
// COMPILE ERROR: A `map` cannot be keyed by `Point`.
// Key the map by one of its fields (an `int` id or a `string` name) and keep the `Point`
// as the map's value: `map<string, Point>`.

let sessions: map<sensitive string, int> = {}
// COMPILE ERROR: A `map` cannot be keyed by `sensitive string`.
// Key the map by a plain `string` or an `int` that is not a secret, such as an id or a name,
// and keep the secret as the map's value: `map<string, sensitive string>`.
```

A `sensitive string` is refused for a different reason than its slot: a map would free the key as an ordinary string and compare keys with a comparison that is not constant-time. A `sensitive string` key is not supported yet.

**Bulk update with `.update({...})`:**

```
let scores: map<string, number> = { alice: 50, bob: 60 }

scores.update({ alice: 75, charlie: 40 })
// Result: alice=75 (updated), bob=60 (untouched), charlie=40 (added)
```

**Filtering and finding in a map:**

```
let expensive = prices.filter(e => e.value > 100)       // new map — only items over $100
let aliceEntry = scores.find(e => e.key == "alice")      // maybe entry

if (aliceEntry.exists()) {
  print(`${aliceEntry.value.key}: ${aliceEntry.value.value}`)
}
```

---

## Dot is for types; brackets are for collections

There's one rule that determines whether to use `.` or `[]`:

| Access | What it does |
|--------|--------------|
| `obj.fieldName` | Field on a type — compile-time known name |
| `obj.methodName()` | Method on a type or collection |
| `arr[i]` | Index lookup → `maybe<T>` (sugar for `.get(i)`) |
| `m["key"]` | Key lookup → `maybe<V>` (sugar for `.get(key)`) |
| `s[i]` | Code point lookup → `maybe<string>` (sugar for `.get(i)`) |

Types and collections are different concepts. Trying to use brackets on a type, or dot to access a runtime key on a map, both fail:

```
shape Player { name: string, health: number }
let p: Player = { name: "Alice", health: 100 }
let n = p["name"]
// COMPILE ERROR: Bracket access is for collections (array, fixed, map, string).
//                Use dot access on types: p.name

let scores: map<string, number> = { alice: 50 }
scores.alice
// COMPILE ERROR: Dot access is for methods, not map keys.
//   Use scores["alice"] for the value at the "alice" key.
//   If you actually have a fixed set of known keys, consider using a shape
//   instead — it's faster AND gives you dot access:
//
//     shape Scores { alice: number, bob: number }
//     scores.alice         // works — alice is a compile-time field
//
//   See: docs/reference/REF-collections.md#use-shapes-instead-of-maps-for-known-fields
```

The simple rule: **dot is for compile-time-known names; brackets are for runtime lookups.**

---

## Use shapes instead of maps for known fields

If you know all the keys at compile time, a `shape` is faster (direct field access vs hash lookup):

```
// Slower — known fields in a map
let stats: map<string, number> = { health: 100, attack: 50 }

// Faster — typed object with fixed offsets
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
let active = players.filter(p => p.health > 0)
let ranked = active.sort(p => p.health, desc)
let top = ranked.limit(10)
let names = top.map(p => p.name)
```

The compiler sees that each result is only used once and fuses all four steps into a single optimized pass with zero intermediate allocations. You get readable code AND fast code.

Early returns fit naturally into this pattern:

```
function getTopNames(share players: fixed<Player>, count: number) -> array<string> {
  let active = players.filter(p => p.health > 0)

  if (active.count() == 0) {
    return []
  }

  let ranked = active.sort(p => p.health, desc)
  let top = ranked.limit(count)
  return top.map(p => p.name)
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

**A shape type is not a collection type:**

```
let team: Player = []
// COMPILE ERROR: `[]` is an array literal, but `Player` is a shape — a single value, not a collection.
// Use `array<Player>` if you want a list: `let team: array<Player> = []`
```
