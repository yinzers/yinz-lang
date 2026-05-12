# Collections

Three ways to hold multiple values.

---

## The three collection types

```
fixed[string]           // fixed size, stack-allocated — size locked at creation, fastest
array[Player]           // growable, heap-allocated — add and remove freely
map[string, number]     // key-value pairs with dynamic keys
```

All collections are typed. Every element must be the same type:

```
let stuff = [42, "hello", true]
// COMPILE ERROR: Cannot mix types in a collection. Pick one type.
```

The syntax is the same for all three: `kind[type]`. No exceptions.

---

## fixed[T] — fixed-size arrays

`fixed[T]` is stack-allocated and size-locked at creation. The fastest option — no heap, no growth tracking.

Use `fixed[T]` when you know the size up front:

```
let rgb: fixed[number] = [255, 128, 0]
let names: fixed[string] = ["Alice", "Bob", "Charlie"]
let players: fixed[Player] = [
  { name: "Alice", health: 100 },
  { name: "Bob", health: 80 }
]
```

You cannot add or remove elements:

```
rgb.add(50)
// COMPILE ERROR: Cannot add to a fixed array.
// fixed[number] is size-locked. Use array[number] if it needs to grow.
```

---

## array[T] — growable arrays

`array[T]` is heap-allocated and can grow and shrink:

```
let roster: array[Player] = []
roster.add({ name: "Alice", health: 100 })
roster.add({ name: "Bob", health: 80 })
roster.remove(0)    // remove by index
```

---

## Dot methods — available on both fixed and array

```
.filter(fn)         // filter — returns new collection where fn returns true
.sort(fn, order)    // sorted copy — order is asc or desc (see Options)
.map(fn)            // transform — returns new collection of results
.get(index)         // item at position → maybe T (safe, returns none if out of bounds)
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

`.concat()`, `.append()`, and `.prepend()` work on `fixed[T]` because they return new collections. The original stays untouched.

---

## Safe access — no direct indexing

You cannot access a collection by index using `[]`:

```
let item = players[5]
// COMPILE ERROR: Direct index access is not allowed.
// Use players.get(5) — it returns maybe Player and handles out-of-bounds safely.
```

Use `.get(index)` instead. It returns `maybe T`, forcing you to handle the case where the index doesn't exist:

```
let item = players.get(5)             // → maybe Player

if (item.exists()) {
  print(item.value.name)             // compiler knows it's safe here
}

// Or with a default
let item = players.get(5).or(defaultPlayer)   // always returns a Player
```

This applies to all collection types — `fixed`, `array`, and `map`. Every access that might not exist returns `maybe`. No out-of-bounds crashes anywhere in the language.

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

## map[K, V] — key-value pairs

Use maps when keys aren't known at compile time. For known fields, define a `type` — it's faster.

```
let wordCounts: map[string, number] = {}
for (word in words) {
  let current = wordCounts.get(word).or(0)
  wordCounts.set(word, current + 1)
}
```

Map dot methods:

```
.get(key)             // → maybe V (might not exist — see Maybe Types)
.set(key, value)      // add or update one key
.update({...})        // add or update multiple keys at once
.has(key)             // does the key exist? → bool
.remove(key)          // delete by key
.keys()               // → array[K]
.values()             // → array[V]
.entries()            // → array of key-value pairs (each has .key and .value)
.sort(fn, order)      // sorted copy
.filter(fn)           // new map with only matching entries — fn receives entry with .key and .value
.find(fn)             // first matching entry → maybe entry (has .key and .value)
```

**Bulk update with `.update({...})`:**

```
let scores: map[string, number] = { alice: 50, bob: 60 }

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

## Use types instead of maps for known fields

If you know all the keys at compile time, a `type` is faster (direct field access vs hash lookup):

```
// Slower — known fields in a map
let stats: map[string, number] = { health: 100, attack: 50 }

// Faster — typed object with fixed offsets
type Stats { health: number, attack: number }
let stats: Stats = { health: 100, attack: 50 }
```

The compiler will suggest this when it sees a map with all-literal keys.

---

## Nested collections — name the inner shape

Don't nest collection types inline. Name the inner shape:

```
// Hard to read
let data: map[string, map[string, number]] = { ... }

// Clear — name each layer
type PlayerScores {
  kills: number
  deaths: number
  assists: number
}
let scoreboard: map[string, PlayerScores] = {
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
function getTopNames(share players: fixed[Player], count: number) -> array[string] {
  let active = players.filter(p => p.health > 0)

  if (active.count() == 0) {
    return []
  }

  let ranked = active.sort(p => p.health, desc)
  let top = ranked.limit(count)
  return top.map(p => p.name)
}
```
