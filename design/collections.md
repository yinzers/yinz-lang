# Collections — Design Decisions

User spec: `spec/collections.md`

---

## `fixed[T]` and `array[T]`

`fixed[T]` = stack-allocated, size-locked at creation. `array[T]` = heap-allocated, growable.

**Why**: Golden Rule 10 — the fast default should be the default path. Developers who don't think about performance automatically write fast code. `fixed[T]` requires no annotation — it's what you get when you declare a collection with known content. `array[T]` is the explicit opt-in for when you need growth.

**Removed**: `collection[T]` type removed — unnecessary complexity. Two collection types are enough.

**No spread operator**: Not included. Compiler handles optimization internally.

---

## No Method Chaining

Each operation gets its own line with a named variable.

**Why**: Golden Rule 7. Named intermediate results are documentation — the variable name says what that step produces. Every step is debuggable — you can inspect any intermediate value. The compiler fuses sequential operations into a single optimized pass anyway, so there's no performance cost for the readability benefit.

---

## Collection Method Naming

Several method names chosen for plain-English readability over functional programming convention:
- `.filter(fn)` over `.where(fn)` — more standard, clearer to non-FP developers
- `.limit(n)` over `.take(n)` — "limit to N results" reads like English; `.take` is FP jargon
- `.prepend(item)` added — mirrors `.append()`, non-mutating, works on both `fixed` and `array`
- `.get(index)` over `items[n]` — safe access returning `maybe T`; direct indexing is a compile error
- `.last()` added — mirrors `.first()`, returns `maybe T`

**Why**: Golden Rule 12. Every method name that requires FP background knowledge is a barrier. `.limit`, `.filter`, `.prepend` are plain English instructions anyone can guess.

---

## Map `.update({...})`, `.filter()`, `.find()`

Three additions to the map API:
- `.update({...})` — bulk add/update from an object literal. Merges keys, adds new ones, updates existing ones.
- `.filter(fn)` — returns new map with only matching entries. Callback receives `entry` with `.key` and `.value`.
- `.find(fn)` — returns `maybe` entry. Same callback shape.

**Why**: `.set(key, value)` one at a time is verbose when updating multiple keys. `.update({...})` follows the same object-literal syntax used for type construction. Map filter/find fills an obvious gap — there was no way to search a map without calling `.entries()` first.

**No `map["key"]` index notation**: Dot-first design (Golden Rule 1). The `[]` syntax is already used for type annotations (`array[Player]`) — using it at the value level too would create visual ambiguity.
