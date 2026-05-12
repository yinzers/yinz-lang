# Collections — Design Decisions

User spec: `spec/collections.md`

---

## `fixed[T]` and `array[T]`

`fixed[T]` = stack-allocated, size-locked at creation. `array[T]` = heap-allocated, growable.

**Why**: Golden Rule 10 — the fast default should be the default path. Developers who don't think about performance automatically write fast code. `fixed[T]` requires no annotation — it's what you get when you declare a collection with known content. `array[T]` is the explicit opt-in for when you need growth.

**Removed**: `collection[T]` type removed — unnecessary complexity. Two collection types are enough. "Collections" still exists as an English umbrella term covering `fixed[T]`, `array[T]`, `map[K,V]` — but it's not a TYPE, just a category name.

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
- `.get(index)` over `items[n]` — safe access returning `maybe T`; brackets are sugar for `.get()`
- `.last()` added — mirrors `.first()`, returns `maybe T`

**Why**: Golden Rule 12. Every method name that requires FP background knowledge is a barrier. `.limit`, `.filter`, `.prepend` are plain English instructions anyone can guess.

---

## Bracket Sugar for `.get()` and `.set()`

Brackets at a value position desugar to `.get()` (read) or `.set()` (write):

```yinz
let p = players[3]              // sugar for players.get(3)         → maybe Player
let s = scores["alice"]         // sugar for scores.get("alice")    → maybe number
let c = name[0]                 // sugar for name.get(0)            → maybe string

players[2] = newPlayer          // sugar for players.set(2, newPlayer)
scores["bob"] = 75              // sugar for scores.set("bob", 75)
```

Works on: `array[T]`, `fixed[T]`, `map[K,V]`, `string` (read only — strings are immutable).

Does NOT work on: `type` instances. Types use dot access for fields and methods — brackets at a value position on a type are a compile error. This forces the visual distinction between "compile-time-known field name" (dot) and "runtime value lookup" (bracket).

**Why bracket sugar:**
- **Familiarity:** Every JS/Python/C-family dev expects `arr[0]`. Forcing `.get(0)` everywhere creates needless friction.
- **Safety preserved:** The sugar still returns `maybe T`. The type system enforces handling. Surface looks familiar; semantics stay safe.
- **No syntax conflict with type parameters:** Same brackets, different position. `array[Player]` is a TYPE (left of equals or in a type annotation); `players[0]` is a VALUE (right of equals or in an expression). TypeScript proved this distinction works for human readers.

**Reverses an earlier decision:** the original `design/collections.md` rejected `map["key"]` notation citing visual ambiguity with `array[Player]` type syntax. That argument was weaker than originally stated — TS does both fine. Reversing for consistency: brackets work universally on all four collection types for read; on map/array/fixed for write.

**Why dot doesn't work on map keys (the real reason):**

Maps have methods (`.count()`, `.get()`, `.set()`, `.keys()`, `.values()`, `.filter()`, etc.). If keys could be accessed via dot, a key named `count` would collide with the `.count()` method — exactly the JavaScript bug class around `obj.constructor`, `obj.toString`, etc.

Types don't have this problem because the user defines both fields and methods in one place, and the compiler can verify no collision. Map keys are runtime user data; method names are compile-time built-ins. The two namespaces can't be unified.

Bracket access keeps the namespaces visually separated: `m["count"]` is unambiguously a key lookup; `m.count()` is unambiguously a method call. No collision possible.

**Out-of-bounds on `.set()`:**

```yinz
let arr: array[number] = [1, 2, 3]
arr[10] = 99
// RUNTIME ERROR: index 10 out of bounds (arr.count() == 3).
//   .set() replaces existing elements only.
//   To append: use .add(99).
//   To grow with defaults: use .resize(newSize, defaultValue).
```

Sparse-array growth (JS's `arr[100] = "x"` creating `<97 empty items>`) is rejected — it's a footgun, not a feature.

---

## Methods Added: `.set()` on `array[T]` and `fixed[T]`

Originally `.set()` only existed on maps. For the bracket-write sugar to work consistently, arrays and fixed arrays also need `.set(index, value)`:

- `array[T].set(lend self, index: int, value: T) -> nothing` — replace at index. Runtime error if out of bounds.
- `fixed[T].set(lend self, index: int, value: T) -> nothing` — same. Index out of bounds is a compile error when the index is a literal AND the size is known; runtime error otherwise.

These replace existing elements only. Use `.add()` to append; `.insertAt(i, value)` (planned, may not be v0.1) to insert.

---

## Map `.update({...})`, `.filter()`, `.find()`

Three additions to the map API:
- `.update({...})` — bulk add/update from an object literal. Merges keys, adds new ones, updates existing ones.
- `.filter(fn)` — returns new map with only matching entries. Callback receives `entry` with `.key` and `.value`.
- `.find(fn)` — returns `maybe` entry. Same callback shape.

**Why**: `.set(key, value)` one at a time is verbose when updating multiple keys. `.update({...})` follows the same object-literal syntax used for type construction. Map filter/find fills an obvious gap — there was no way to search a map without calling `.entries()` first.

---

## String Methods: `.byteAt()`, `.graphemeAt()`

Default indexing on strings (`.get(n)` / `s[n]`) is by **Unicode code point**. Two escape valves exist for explicit access modes:

- `.byteAt(n)` — `maybe int` — n-th UTF-8 byte. For parsers and protocol handling.
- `.graphemeAt(n)` — `maybe string` — n-th grapheme cluster (what a human sees as "one character"). For text rendering and cursor positioning.

Companion length methods:
- `.count()` — code point count
- `.byteCount()` — UTF-8 byte count
- `.graphemeCount()` — grapheme cluster count

**Why default to code points:**
- For typical text (ASCII, simple multilingual), code points match user intuition of "characters."
- Bytes give wrong results on multi-byte UTF-8 (`"café".get(3)` would return a partial code unit).
- Graphemes are correct for human-perceived characters but expensive — most code doesn't need that level of correctness.

**Why no `char` type:**
- Adds a new concept jr devs must learn (and convert to/from strings).
- Unicode complications surface immediately: is a `char` a code point? A grapheme? UTF-8 byte? Each answer wrong for some use case.
- A 1-length string covers every place a "char" would be useful, without the conceptual baggage.
