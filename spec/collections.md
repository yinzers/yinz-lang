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

Reading by index uses brackets, which is shorthand for `.get()`. Both forms return `maybe T` — never a raw value, so you can't accidentally crash on out-of-bounds:

```
let player = players[3]              // sugar for players.get(3)
let player = players.get(3)          // same thing, longer form
// both return: maybe Player

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

## Dot methods — available on both fixed and array

```
.filter(fn)         // filter — returns new collection where fn returns true
.sort(fn, order)    // sorted copy — order is asc or desc (see Options)
.map(fn)            // transform — returns new collection of results
.get(index)         // item at position → maybe T (safe, returns none if out of bounds)
.set(index, value)  // replace item at position — runtime error if out of bounds
.first()            // first item → maybe T
.last()             // last item → maybe T
.find(fn)           // first item matching condition → maybe T
.count()            // number of items → number
.unique()           // deduplicated copy
.limit(n)           // cap to the first N items — returns new collection of at most N
.contains(fn)       // does any item match? → bool
.concat(other)      // combined copy — does not modify original
.append(item)       // new collection with item added at the end
.prepend(item)      // new collection with item added at the front
```

`.concat()`, `.append()`, and `.prepend()` work on `fixed<T>` because they return new collections. The original stays untouched.

`.set()` works on `fixed<T>` because it replaces an existing element without changing size.

---

## Dot methods — array only

These mutate in place, which requires dynamic sizing:

```
.add(item)              // add item at end
.remove(index)          // remove by index
.removeFirst()          // remove and return first item → maybe T
.removeLast()           // remove and return last item → maybe T
```

---

## map<K, V> — key-value pairs

Use maps when keys aren't known at compile time. For known fields, define a `type` — it's faster.

```
let wordCounts: map<string, number> = {}
for (word in words) {
  let current = wordCounts[word].or(0)
  wordCounts[word] = current + 1
}
```

Map dot methods:

```
.get(key)             // → maybe V (might not exist — see Maybe Types)
.set(key, value)      // add or update one key
.update({...})        // add or update multiple keys at once
.has(key)             // does the key exist? → bool
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
scores["alice"]              // sugar for scores.get("alice") → maybe number
scores["alice"] = 75         // sugar for scores.set("alice", 75)
```

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
| `arr[i]` | Index lookup → `maybe T` (sugar for `.get(i)`) |
| `m["key"]` | Key lookup → `maybe V` (sugar for `.get(key)`) |
| `s[i]` | Code point lookup → `maybe string` (sugar for `.get(i)`) |

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
//   If you actually have a fixed set of known keys, consider using a type
//   instead — it's faster AND gives you dot access:
//
//     type Scores { alice: number, bob: number }
//     scores.alice         // works — alice is a compile-time field
//
//   See: spec/collections.md#use-types-instead-of-maps-for-known-fields
```

The simple rule: **dot is for compile-time-known names; brackets are for runtime lookups.**

---

## Use types instead of maps for known fields

If you know all the keys at compile time, a `type` is faster (direct field access vs hash lookup):

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
