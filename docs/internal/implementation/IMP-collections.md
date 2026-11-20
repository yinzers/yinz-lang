---
name: "IMP-collections"
description: "Design rationale for Yinz's collection types (Fixed<T> vs Array<T>, Map<K,V>), the no-method-chaining rule, collection method naming conventions, by-value inline array element storage, and the auto-SoA layout transform (both v0.3-M5)."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Collections — Design Decisions

User spec: [`docs/reference/REF-collections.md`](../../reference/REF-collections.md)

---

## Collection Type Names

The collection types are written `Array<T>`, `Fixed<T>`, `Map<K, V>` and `Channel<T>`. Every type name starts with a capital letter, built-ins included, so a reader tells a type from a function or variable by its first letter (Golden Rule 10). `.map(...)` and `range(a, b)` stay lowercase: they are a method and a function, not types.

`Channel<T>` also works in call position, like a shape literal: `Channel<Int>(8)` creates a channel with room for 8 values. Old lowercase spellings (`array<T>`, `fixed<T>`, `map<K, V>`, `channel<T>`) are a compile error with a quick-fix, with no alias (IMP-versioning). A user shape named `Array`, `Fixed`, `Map` or `Channel` is refused as a reserved built-in type name.

---

## `Fixed<T>` and `Array<T>`

`Fixed<T>` = stack-allocated, size-locked at creation. `Array<T>` = heap-allocated, growable.

**Why**: Golden Rule 7 — the fast default should be the default path. Developers who don't think about performance automatically write fast code. `Fixed<T>` requires no annotation — it's what you get when you declare a collection with known content. `Array<T>` is the explicit opt-in for when you need growth.

**Removed**: `collection<T>` type removed — unnecessary complexity. Two collection types are enough. "Collections" still exists as an English umbrella term covering `Fixed<T>`, `Array<T>`, `Map<K,V>` — but it's not a TYPE, just a category name.

**No spread operator**: Not included. Compiler handles optimization internally.

---

## No Method Chaining

Each operation gets its own line with a named variable.

**Why**: Golden Rule 9. Named intermediate results are documentation — the variable name says what that step produces. Every step is debuggable — you can inspect any intermediate value. The compiler fuses sequential operations into a single optimized pass anyway, so there's no performance cost for the readability benefit.

---

## Collection Method Naming

Several method names chosen for plain-English readability over functional programming convention:
- `.filter(fn)` over `.where(fn)` — more standard, clearer to non-FP developers
- `.limit(n)` over `.take(n)` — "limit to N results" reads like English; `.take` is FP jargon
- `.prepend(item)` added — mirrors `.append()`, non-mutating, works on both `Fixed` and `Array`
- `.get(index)` over `items[n]` — safe access returning `Maybe<T>`; brackets are sugar for `.get()`
- `.last()` added — mirrors `.first()`, returns `Maybe<T>`

**Why**: Golden Rule 5. Every method name that requires FP background knowledge is a barrier. `.limit`, `.filter`, `.prepend` are plain English instructions anyone can guess.

---

## Bracket Sugar for `.get()` and `.set()`

Brackets at a value position desugar to `.get()` (read) or `.set()` (write):

```ynz
let p = players[3]              // sugar for players.get(3)         → Maybe<Player>
let s = scores["alice"]         // sugar for scores.get("alice")    → Maybe<Number>
let c = name[0]                 // sugar for name.get(0)            → Maybe<String>

players[2] = newPlayer          // sugar for players.set(2, newPlayer)
scores["bob"] = 75              // sugar for scores.set("bob", 75)
```

Works on: `Array<T>`, `Fixed<T>`, `Map<K,V>`, `String` (read only — strings are immutable).

Does NOT work on: shape values. Shapes use dot access for fields, and functions are called with dot sugar — brackets at a value position on a shape value are a compile error. This forces the visual distinction between "compile-time-known field name" (dot) and "runtime value lookup" (bracket).

**Bracket sugar:**
- It still returns `Maybe<T>`, so the type system enforces handling the absent case.
- It does not conflict with type parameters: `Array<Player>` is a type (in a type annotation), `players[0]` is a value (in an expression).

Brackets work on all four collection types for read, and on Map/Array/Fixed for write. `map["key"]` is allowed.

**Map keys use brackets, never dot.** Maps have methods (`.count()`, `.get()`, `.set()`, `.keys()`, `.values()`, `.filter()`, etc.), so a key named `count` accessed by dot would collide with `.count()`. Map keys are runtime data and method names are compile-time built-ins; brackets keep them apart: `m["count"]` is a key lookup, `m.count()` is a method call. Shapes use dot because the compiler checks their declared field names against the functions callable through dot sugar.

**Out-of-bounds on `.set()`:**

```ynz
let arr: Array<Number> = [1, 2, 3]
arr[10] = 99
// RUNTIME ERROR: index 10 out of bounds (arr.count() == 3).
//   .set() replaces existing elements only.
//   To append: use .add(99).
//   To grow with defaults: use .resize(newSize, defaultValue).
```

Writing past the end never grows the array (no sparse growth); out-of-bounds `.set()` is a runtime error.

---

## Methods Added: `.set()` on `Array<T>` and `Fixed<T>`

`.set()` exists on maps, arrays and fixed arrays, so the bracket-write sugar works the same everywhere; arrays and fixed arrays take `.set(index, value)`:

- `Array<T>.set(lend self, index: Int, value: T) -> Nothing` — replace at index. Runtime error if out of bounds.
- `Fixed<T>.set(lend self, index: Int, value: T) -> Nothing` — same. Index out of bounds is a compile error when the index is a literal AND the size is known; runtime error otherwise.

These replace existing elements only. Use `.add()` to append; `.insertAt(i, value)` (planned, may not be v0.1) to insert.

---

## Map `.update({...})`, `.filter()`, `.find()`

Three additions to the map API:
- `.update({...})` — bulk add/update from an object literal. Merges keys, adds new ones, updates existing ones.
- `.filter(fn)` — returns new map with only matching entries. Callback receives `entry` with `.key` and `.value`.
- `.find(fn)` — returns `Maybe` entry. Same callback shape.

`.update({...})` takes the same object-literal syntax used for type construction, and `.filter()` / `.find()` search a map without going through `.entries()` first.

---

## `Map<K,V>` Implementation — Swiss Tables, Four-Tier Hashing

Swiss Tables (open-addressing with SIMD metadata scan) are the implementation for `Map<K,V>`; there is no separate chaining and no linked list.

### Swiss Tables layout

- **Cache locality**: entries live in the bucket array directly, so a probe stays in one or two cache lines.
- **SIMD metadata scan**: a parallel byte array of "control bytes" (one per slot, holding empty/deleted/hash-fingerprint) is searched 16 at a time using a single SSE2/NEON instruction.
- **Memory overhead**: no per-entry linked-list infrastructure, about 16 bytes saved per entry.

### Four-tier hashing strategy

Different scenarios get different hashing. The compiler picks based on what it knows at compile time. Users don't pick — the IDE shows which tier was used as a muted hint.

| Scenario | Compiler emits | Lookup cost |
|---|---|---|
| `shape Player { name: String; health: Int }` (typed access — not a map at all) | Direct memory offset | 1 instruction |
| `Map<String, T>` literal with all-static keys: `{ "alice": 1, "bob": 2 }` | **Perfect hashing** (compile-time-generated) — zero collisions, zero probing | ~3 instructions |
| `Map<Int, T>` (any integer key type) | Identity hash or single multiplication — no hash function overhead | ~2 instructions |
| `Map<K, V>` runtime keys (general case — DEFAULT) | Swiss Tables + SipHash-2-4 (DoS-safe with random per-process key) | ~30-50 instructions |
| Trusted-keys variant (for non-adversarial workloads — surface syntax TBD at M4, candidate name `.fast`) | Swiss Tables + xxhash3 (fast non-cryptographic) | ~10-20 instructions |

### Default hash function: SipHash-2-4 (safe-by-default)

For the general case (runtime keys, no opt-in), the default is **SipHash-2-4** with a per-process random key, which is DoS-safe against attacker-controlled input. The compile-time tiers already remove the hash cost where DoS-safety does not matter (shape access, perfect hashing for static-key maps, identity hash for `Int` keys), so only `Map<String, V>` with runtime string keys pays it, and those keys usually come from external input (JSON, HTTP headers, query params, config files). Safe is the default so a junior developer is protected from hash-collision attacks before learning they exist; the fast variant is the opt-in for developers who know their threat model.

### Fast opt-in: xxhash3 — for explicitly-trusted workloads

For perf-critical maps where the user can prove the keys are NOT attacker-controlled (in-memory caches, internal indexes, intermediate algorithm data structures, compiler-internal symbol tables), there's an explicit opt-in for the fast variant. Backed by xxhash3 — fast, well-distributed, not DoS-safe.

**Surface syntax is deliberately NOT locked here** — this section locks the BEHAVIOR (Swiss Tables + xxhash3) but the user-facing way to opt in is a language design decision for M4 implementation. Candidate name `.fast` (concise, describes intent), or keyword form like `trusted Map<K,V>`, or a construction flag — TBD when the broader question of variant-syntax for collections is decided. Yinz currently has no precedent for dot-on-type-name modifiers, so don't lock placeholder syntax that doesn't exist anywhere else in the language.

### IDE teaching surface

The IDE muted-hint protocol surfaces which tier each map uses, so devs SEE whether their map is the safe default or the fast opt-in — no silent perf characteristics.

When the compiler can prove the keys are NOT externally-controlled (e.g., the map is constructed and used entirely within a function whose inputs are themselves not externally-derived, OR all keys are compile-time literals → handled by perfect hashing instead), the IDE emits a Tier 3 suggestion: "this map's keys appear trusted. Consider the fast variant — saves ~3× on hash cost." The lint goes the SAFER direction by default; user opts in to fast when they're sure.

When the compiler picks perfect hashing or identity hash for a literal, the IDE shows a muted hint: `// perfect hash — zero collisions for these compile-time-known keys`.

Hashers are never user-selectable: the safe default, the fast opt-in and compiler-chosen perfect/identity hashing cover every case.

---

## `Map<K,V>` Iteration Order — Insertion Order

Iteration order is INSERTION ORDER, never random, so a test that passes locally gives the same order in CI.

**Cost**: one pointer per entry to track insertion order. Trivial overhead (~8 bytes per entry on 64-bit). Implementation: keep a parallel insertion-order index alongside the Swiss Table bucket array.

If a user genuinely doesn't care about iteration order and wants to skip the overhead, they can opt into an unordered variant (surface syntax TBD at M4 alongside the adversarial-variant syntax — same syntax-design problem). Random iteration is still NOT a default — even unordered, the iteration order is implementation-defined but deterministic per-build.

---

## `Set<T>` — Unique-Value Collection (DESIGN NOT YET BUILT)

> **Status**: not shipped. `Set<T>` is not a type the compiler accepts today, and no release is committed for it ([`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md) does not list it). The design below is the direction for a core collection **language feature**, sibling to `Map<K,V>` — not a stdlib module.

A `Set<T>` holds unique values of one type and answers "is value `X` present?" in O(1). It is `Map<K,V>` with keys only — same machinery, no values.

### A distinct type, not `Array` plus a rule

`Fixed`/`Array` find by **position** (`thing[5]` is O(1); "is `X` present?" is an O(n) scan). A `Set` finds by **value** ("is `X` present?" is O(1); there is no position). Its hash storage makes both uniqueness and membership O(1), where an `Array` would need an O(n) scan per insert.

### No `Fixed` set

Sets are inherently growable (hash-table backed, like `Array`/`Map`), so there is no stack-allocated size-locked set. `Set<T>` is the only form. (A compile-time-constant set of literals MAY get a perfect-hash codegen optimization — see map's four-tier hashing — but that's codegen, not a separate `Fixed`-set type.)

### Implementation — shares everything with `Map<K,V>`

A set IS a map with keys only, so it reuses map's locked decisions verbatim:
- **Swiss Tables** backing store (see `Map<K,V>` section above).
- **Four-tier hashing** (SipHash-2-4 safe default → xxhash3 trusted → perfect-hash compile-time-constant → identity for ints). Same auto-pick, same override story.
- **Insertion-order iteration** (same rationale as map — predictable, no "passes locally / fails in CI").

### Operations (ILLUSTRATIVE — proposed, exact names not settled)

- `set.has(value) -> Boolean` — O(1) membership (pure, no `errors`).
- `set.add(value)` — inserts; duplicate add is a no-op.
- `set.remove(value)` — removes if present.
- `set.size` — count (field access, no parens, per dot-postfix rule).
- Set algebra: `a.union(b)`, `a.intersection(b)`, `a.difference(b)`, `a.isSubsetOf(b)`.

### `Array.unique()` companion

The one-time "dedupe this list" case is not a set — it is the shipped `Array` method `array.unique()`, which returns a new array with duplicates removed (insertion order preserved). Rule of thumb: `.unique()` for one-shot dedup; `Set<T>` (once built) when you keep the collection and repeatedly query membership.

### Open questions

- Set-algebra methods: return new sets vs mutate in place (lean: return new, immutable-by-default per stdlib philosophy).
- Set literal syntax — `{1, 2, 3}` collides with shape/map `{...}` literals; needs disambiguation (same family of problems as map literals).
- Whether the unordered opt-out variant (map's TBD-M4 syntax) applies to set too (it should — same overhead tradeoff).

### Auto-promotion

`Array<T>` → `Set<T>` when the array is used only for membership — see "Auto-promotion: `Array<T>` → `Set<T>`" below.

---

## `shape` Field Layout — Compiler Auto-Reorders for Optimal Packing

When the compiler emits the memory layout for a `shape` declaration, it auto-reorders fields for optimal packing (largest-alignment first, smallest last). User code is unaffected — `shape` access syntax (`player.health`) and field semantics stay identical. Only the in-memory byte layout changes.

```ynz
shape Event {
  flag: Boolean      // user wrote: 1 byte
  timestamp: Int  // user wrote: 8 bytes
  count: Boolean     // user wrote: 1 byte
}
// Compiler emits: timestamp (8) + flag (1) + count (1) + 6 bytes end-padding = 16 bytes
// (NOT what the user wrote in source order, which would have been 24 bytes with 14 bytes of padding)
```

Same shape, same semantics, ~33% less memory for this example. Multiply by N entries in an `Array<Event>` and the cache benefit compounds.

Reordering is the default (Golden Rule 7 and the auto-promotion corollary: fast by design for inexperienced developers).

**There is no keyword to pin a shape's field layout.** The layout constraint comes from the boundary, not from the shape:
1. **FFI to C** — handled by FFI bindings at the boundary. (FFI is v2+ per [`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md).)
2. **Wire protocols / file formats** — handled by compiler-generated specialized serializers per shape ([`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) Rule 6), which decide wire layout independently of internal layout.
3. **Memory-mapped hardware** — handled by `--kernel` mode infrastructure (see [`docs/internal/implementation/IMP-no-runtime-mode.md`](IMP-no-runtime-mode.md)).

**IDE teaching surface**: per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md), when the compiler reorders fields, the IDE shows a muted hint on the shape declaration:

```
shape Event {                     // muted: // reordered: timestamp, flag, count
  flag: Boolean                   //         saved 8 bytes per value
  timestamp: Int
  count: Boolean
}
```

Hover tooltip:
- **WHAT**: This `shape` is laid out as `timestamp (8) + flag (1) + count (1) + 6 bytes end-padding = 16 bytes` instead of source-order (which would be 24 bytes with 14 bytes of padding waste).
- **WHAT INSTEAD**: There's no source-level syntax to pin a different layout for pure Yinz code. If you need a specific layout for FFI (v2+), the FFI binding handles conversion at the boundary; if you need it for a wire format, the serializer codegen produces the correct wire bytes regardless of internal layout.
- **WHY**: Largest-alignment-first packing eliminates padding bytes between mismatched-alignment fields. The savings compound when the shape is stored in `Array<Shape>` collections — better cache utilization for hot field-access loops.

This pattern qualifies as auto-promotion (per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md)) — the codegen surface always applies; the muted hint applies because there IS a "what would my source-order layout have been" form the user could mentally reason about; no Tier 3 lint suggestion because there's no explicit-form rewrite that would help (the user's source order is already what they wrote — the compiler just rearranges internally).

---

## Sort — Compiler Auto-Picks Based on Element Type

The primary API is `.sort()`. The compiler picks stable or unstable codegen based on the element type — beginners never have to think about this.

| Element type | Compiler picks | Reasoning |
|---|---|---|
| `Array<Int>`, `Array<Float>`, `Array<Boolean>`, `Array<String>` | Unstable (fast in-place quicksort family) | Equal primitives/strings are interchangeable — there's no "original order" between two `5`s or two `"hello"`s to preserve. Pure speed and memory win. |
| `Array<Shape>` (any composite type) | Stable (TimSort or equivalent) | Equal-keyed shapes might have other fields the user wants preserved in order. Default to safe. |
| `Array<Array<T>>`, `Array<Map<K,V>>`, `Array<Entry<K,V>>` | Stable | Same — nested or composite types. |

This is the auto-promotion pattern (per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md)):
- **Codegen**: compiler picks per element type, always. No runtime cost to the decision.
- **Muted IDE hint**: shows what was picked, e.g., `// unstable sort (Int — equal values interchangeable)` or `// stable sort (Shape — equal keys preserve original order)`. Click does NOT have a typeable equivalent for the auto-pick itself, but the explicit override forms below ARE typeable.
- **Tier 3 lint suggestion**: not generally applicable — the auto-pick is right for ~99% of cases.

### Explicit overrides (rare, opt-in)

For the cases where the auto-pick is wrong, two explicit forms:

- **`.sortFast()`** — forces unstable regardless of element type. Use when the user knows equal items are interchangeable even though they're composite (e.g., `Array<Shape>` where the sort key is a UUID and there are no other relevant fields).
- **`.sortStrict()`** — forces stable regardless of element type. Use when the user knows they need stable behavior even on primitives (the canonical case: multi-step / radix sort, see "Gotcha" below).

Naming follows Golden Rule 5 (human-readable over jargon): `Fast` describes user intent, `Strict` describes the property. Neither uses the word "unstable" (jargon — sounds like "broken" to a non-programmer).

### Multi-step sort detection — compiler upgrades unstable to stable

The compiler also detects in-function multi-step sort patterns and upgrades the auto-pick from unstable to stable. Pattern: two-or-more `.sort()` calls on the same variable, in sequence, with no intervening modifications to the array.

```ynz
let nums: Array<Int> = [...]
nums.sort(n => n % 10)        // muted hint: // unstable sort (Int — equal values interchangeable)
nums.sort(n => n / 10)        // muted hint: // stable sort (multi-step pattern — preserving previous .sort() order)
                              // Compiler upgraded the auto-pick because nums was already sorted in this function.
```

This is single-function data-flow analysis — same machinery as `Array<T>` → `Fixed<T>` proof. Adds the smarts where they pay off and stays out of the way otherwise.

### Cross-function multi-step — explicit `.sortStrict()` needed

The compiler can't easily trace multi-step patterns across function boundaries. If you sort in one function and re-sort in another, the in-function detector won't catch it:

```ynz
function sortByLow(lend nums: Array<Int>) {
  nums.sort(n => n % 10)        // unstable — fine in isolation
}

function processData(lend nums: Array<Int>) {
  sortByLow(nums)
  nums.sort(n => n / 10)        // unstable — but caller intended this as step 2!
                                // Compiler can't know — cross-function intent isn't local
}
```

For cross-function multi-step, the user must reach for `.sortStrict()` explicitly on the second-and-later sorts. The IDE muted hint showing "unstable" is the visible warning when this matters; cross-function multi-step is rare enough that the explicit-opt-in cost is acceptable.

### Type-based auto-pick

The compiler picks stable or unstable from what the element type permits (table above): composite elements (shapes, arrays, maps) sort stably, because equal keys may differ in other fields. Primitives and strings sort unstably, because equal values are interchangeable.

### Performance characteristics

- **Stable (TimSort family) on nearly-sorted data**: O(n) — detects existing runs and merges them. 16M-element benchmark: 0.15s vs 2.21s for introsort = 14× faster.
- **Stable (TimSort family) on random data**: O(n log n) with ~20-30% overhead vs an in-place quicksort due to auxiliary memory and merge work.
- **Unstable (introsort / dual-pivot quicksort)**: O(n log n), in-place, no auxiliary allocation, fastest on random primitive data.

### Glossary — what "stable" actually means

If you're not sure: when two items compare equal, **stable** keeps their original relative order. **Unstable** makes no guarantee — equal items can end up in any order, and different runs can produce different results.

```ynz
let users = [
  { name: "Alice", age: 30 },
  { name: "Bob",   age: 25 },
  { name: "Carol", age: 30 },
]

users.sort(u => u.age)
// STABLE     (always):     [Bob, Alice, Carol]   — Alice before Carol because input order said so
// UNSTABLE   (possible):   [Bob, Alice, Carol]   — but could also be [Bob, Carol, Alice]; undefined
```

When stability matters in real code:

1. **Multi-step sorting**: sort by name first, then by age. Stable keeps people alphabetical WITHIN each age group; unstable scrambles them.
2. **UI rendering**: re-sorting a table by clicking a column header. Stable means equal-valued rows don't jump around — visually less jarring.
3. **Test reproducibility**: stable sort gives identical output every run on every machine.
4. **Audit trails**: sorting log entries by timestamp. Two events with the same timestamp must stay in the order they were logged.

When stability doesn't matter (so unstable wins on speed AND memory):
- Sorting primitives — equal numbers are interchangeable, no original order between two `5`s
- Sorting then immediately discarding (e.g., `.sort().first()` to find the minimum)
- Any collection where equal items are truly fungible

The auto-promotion pattern (per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md)) applies: when the compiler can detect a sort target is a `Fixed<T>` of a primitive type AND the sorted result isn't passed to anything depending on stability, the IDE shows a Tier 3 lint suggestion `prefer-unstable-sort-for-numeric-collections` recommending `.sortUnstable()` for the perf win. No codegen auto-swap (semantics differ — different equal-keyed element ordering); user makes the call.

---

## Map Literals Pre-Size at Compile Time

When a `Map<K, V>` is constructed with a literal initializer where the entry count is known at compile time, the compiler emits codegen that pre-sizes the internal Swiss Table to fit the literal's entries with no-resize headroom. No runtime resize storm.

### Effect

Each resize copies all existing entries. Pre-sizing to the known count removes every resize, and it is pure compile-time work because the literal form has no capacity parameter to write by hand.

### Auto-promotion (codegen-only)

Per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md):
- **Codegen**: compiler counts entries in the literal initializer, allocates the Swiss Table at `ceil(N / load_factor)` buckets directly. Always applies when the count is statically knowable.
- **Muted IDE hint**: not applicable — there is no surface syntax for "pre-sized map" (no `Map<K, V>(capacity: N)` form exists in v0.1, and adding one would create a parallel API per [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) Rule 2). The user can't write the explicit form, so the muted-hint protocol doesn't fit.
- **Tier 3 lint suggestion**: `prefer-presized-map` fires when a map is constructed via `.set()` calls in a loop with a known iteration count (e.g., a literal range or array length) — suggests rebuilding as a literal initializer or noting that the loop pattern misses the pre-size optimization.

### Tradeoff

A pre-sized map for a 3-entry literal is slightly larger than the smallest possible bucket array (typically 16 buckets even for 3 entries). The "wasted" headroom is negligible for small maps and saves real resize work for medium-to-large literals.

---

## `Array<T>` Growth Factor — 1.5×

When `Array<T>` runs out of capacity, it grows by **1.5×**. Growth that is smaller (1.125× needs about 3× more reallocations, each copying every entry) wastes CPU; growth that is larger (2× makes each new block bigger than all freed blocks combined) keeps the allocator from reusing freed memory. 1.5× lets later allocations fit into space freed by earlier ones.

**`Fixed<T>` covers known-size**: if the user knows the final size, they should use `Fixed<T>` from the start (stack-allocated, size-locked, zero growth). The `Array<T>` growth factor only matters when the size is genuinely dynamic. There is no `array.withCapacity(n)` API — `Fixed<T>` IS the answer for "I know the size."

### Auto-promotion: `Array<T>` → `Fixed<T>` (hybrid model — three things happen)

When the compiler can prove an `Array<T>` declaration is never grown (no `.add()`, `.remove()`, `.resize()`, no call to a function whose signature declares `lend` AND may grow the array), THREE things happen:

1. **Codegen auto-promotion (silent perf win)**: the compiler emits `Fixed<T>` codegen for the binding — stack-allocated, no heap, no growth tracking. The user gets the perf benefit automatically without rewriting source.

2. **Muted IDE hint (informational, always-on)**: per [`.claude/rules/inference.md`](../../../.claude/rules/inference.md), the IDE shows a neutral-gray muted hint after the binding (e.g., `// promoted to fixed — never grown`). Click-to-make-explicit converts the source to `Fixed<Int> = [1, 2, 3]`. This surface tells the user the inference happened — they SEE that the compiler made a perf-positive decision, even if they don't act on it. Same protocol as type-inference and ownership-inference muted hints.

3. **Tier 3 lint suggestion (visible teaching surface)**: a Tier 3 lint suggestion (yellow squiggle in IDE, surfaced in compile output) recommends rewriting the source as `Fixed<T>` explicitly for code-review clarity and future-proofing. Per [`docs/internal/implementation/IMP-linting.md`](IMP-linting.md) rule `prefer-fixed-when-immutable`. This surface is the teaching nudge — the lint says "best practice is to write the stricter form yourself."

```
let nums: Array<Int> = [1, 2, 3]   // muted hint: // promoted to fixed — never grown
                                    // yellow squiggle from prefer-fixed-when-immutable
                                    // (compiler already emitted Fixed<Int> codegen)
```

The three surfaces answer different questions:
- The **muted hint** answers "what did the compiler decide?" — visible always, click to materialize.
- The **lint suggestion** answers "what should I write in source?" — actionable, recommends rewrite.
- The **codegen** answers "what runs at runtime?" — invisible, auto-applied.

Hover tooltip on the muted hint:
- **WHAT**: The compiler is treating this `Array<Int>` as `Fixed<Int>` because it proved no growth happens. Stack-allocated, no heap, no growth tracking.
- **WHAT INSTEAD**: Click to convert the source to `Fixed<Int> = [1, 2, 3]`. Behavior identical.
- **WHY**: `Fixed<T>` is the stricter form when the size is known. The compiler picks it automatically when it's safe; the explicit source form makes the choice visible in code review and prevents a future `.add()` from silently switching the codegen back.

Hover tooltip on the yellow lint squiggle:
- **WHAT**: This `Array<Int>` is auto-promoted to `Fixed<Int>` codegen — the perf optimization already happened. This lint is about source-level clarity, not perf.
- **WHAT INSTEAD**: For explicit code as a best practice, write `Fixed<Int> = [1, 2, 3]` so the choice is visible to readers.
- **WHY**: Writing `Fixed<T>` explicitly makes the perf characteristic obvious in code review (the next reader sees "stack-allocated, can't grow" without having to trace the access pattern) and prevents a future `.add()` from silently forcing the codegen back to heap allocation — if growth is unintended, the explicit `Fixed<T>` makes that future `.add()` a compile error instead of a silent codegen change.

This hybrid model — auto-promote for perf, muted hint for visibility, lint suggestion for explicit-form teaching — is the canonical Yinz pattern for "stricter form fits AND explicit form is typeable." Same model used by `mutable-when-const-suffices` for `let → const` ([`docs/internal/implementation/IMP-linting.md`](IMP-linting.md)).

The hybrid keeps all three: perf is automatic, the inference is visible (muted hint), and the explicit form is taught (lint suggestion). Compiler-internal choices such as auto-Arc and auto-`wait` stay silent because the user cannot write an explicit form.

For auto-promotions where the explicit form has NO typeable syntax (e.g., auto-SoA, where there's no `soa Array<T>` keyword in v0.3), only the lint-suggestion surface applies — the muted-hint protocol requires click-to-make-explicit to produce real Yinz syntax. See ["Auto-SoA layout (v0.3-M5)"](#auto-soa-layout-v03-m5) below and [`.claude/rules/inference.md`](../../../.claude/rules/inference.md) "Two Surfaces for the Same Decision" section.

---

## Array element storage — by-value inline (v0.3-M5)

`Array<Shape>` elements are stored **by value, inline** in the array's heap buffer — variable slot
width = `sizeof(elem)`, exactly like `Array<Int>` stores i64 values inline. Shipped in v0.3-M5
(plan `2026-07-03-v0-3-m5-auto-soa`, Phases 2–3); shape elements are no longer stored as pointers.

### Ownership

The array OWNS its element bytes: the heap buffer survives a `wait`, so the shapes do too, whether their field values are literals or computed at runtime (see [`IMP-concurrency.md`](IMP-concurrency.md) for the `ArrayShapeRuntimeFieldWithWait` guard that this representation lifted).

### One allocation per buffer (D2)

The whole array is a single contiguous allocation of `cap × elem_size` bytes. The SoA layout
variant (next section) is the SAME single allocation, segmented per field, with segment offsets
computed at compile time from `cap ×` per-field sizes — a layout variant of one representation,
never a second substrate (the twin-substrate drift class is eliminated by construction).

Elements are never separate heap cells or LLVM module globals.

### Element-blind drop, parity-gated (D6)

`ynz_array_drop` is element-blind: it frees the buffer and does not walk elements. It stays that way until the ownership model's drop story guarantees freeing of heap-owned fields. The alloc=free parity check (`YNZ_ALLOC_COUNTER_OUTPUT`) gates this.

### `contains` on `Array<Shape>` = field-wise value equality (D12)

By-value storage has no pointer identity, so `contains` compares by value:
`roster.contains(pirate)` reads as content membership (GR4) and matches every primitive cell's
behavior. Implemented as per-field GEP compares (`shape_value_eq`,
`crates/ynz-codegen/src/emit.rs`), never as pointer identity or a whole-element memcmp.

**String fields compare by content** (`shape_value_eq`; `Array<String>.contains` does the same):
copy-on-store gives a stored string no stable identity, so an identity compare of a stored copy
with the text it was copied from is always false (`IMP-strings.md` "String Ownership and
Release"). This follows D12's value-semantics direction. **Nested shape and collection fields
still compare by identity**; deep value equality for those is a future extension.

### Field-assign = copy-on-persist snapshot semantics (D13)

Storing a shape (or `Maybe`) value into ANY persist surface — a shape field, a map value, an array
element, a `background` spawn descriptor — **snapshots the value's bytes at the assignment**
(counted heap cell / runtime memcpy). The pre-M5 pointer representation ALIASED: a later mutation
of the source stayed visible through the stored reference. By-value storage has no stable pointer
to alias, so the snapshot is forced — and it is consistent with D12's value-semantics direction
(shapes are data, not identities).

**Teaching note (the single most surprising divergence for the target audience):** TypeScript/JS
developers expect aliasing here — in TS, objects are references, so push-then-mutate changes what
the array sees. In Yinz a store hands the source over (`IMP-ownership.md`, "Stores are sinks";
`git log --grep="hands it over"`), so the source name cannot be read or mutated after the store.
To keep working with the source, store a copy; the collection holds a snapshot taken at the store:

```ynz
shape Position {
  x: Number
  y: Number
}

let path: Array<Position> = []
let p: Position = { x: 1.0, y: 2.0 }
path.add(p.copy())
p.x = 99.0
// The stored element still has x == 1.0 — the copy was taken at the call.
// Without `.copy()`, the read of `p` on the next line is refused.
// To change the stored element, write through the collection: path[0] = { x: 99.0, y: 2.0 }
```

The user spec ([`REF-collections.md`](../../reference/REF-collections.md)) must keep carrying this
in HS-grad wording — it is a spec callout, never a silent divergence.

### FFI suppression check — documented vacuous check (D7)

The "no FFI export" admission criterion (roadmap-mandated for the SoA variant) is **vacuously true
today** — FFI is v2+. The check is kept as a named line in the admission criteria rather than
deleted and re-invented at v2. Same posture as the serialization note below.

### Benchmark force-mode is harness-only (D8)

`YNZ_SOA_FORCE` is an internal, test-only env var (mirrors the `YNZ_NO_AUTO_PARALLEL` idiom) used
by the calibration harness for A/B layout forcing. It is **never user-facing syntax** — the locked
design has no source-level opt-in/opt-out for layout in v0.3.

### Serialization forward-compatibility (E10)

Layout metadata is exposed as a real, tested struct — `LayoutDecision`
(`crates/ynz-typeck/src/soa.rs`), consumed by the Tier 3 lint hover, so it exists as exercised
code, not prose. v0.8's compile-time serializer codegen (per
[`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) Rule 6) reconstructs
unified values from either layout by reading it: per-field segment offsets are computable from
`cap ×` field sizes. **No roundtrip test exists until v0.8** — it is vacuous until a serializer
exists (recorded plan decision; the roadmap's original roundtrip assertion was reframed).

---

## Auto-SoA layout (v0.3-M5)

When admission criteria hold, the compiler stores a qualifying `Array<Shape>` in Struct-of-Arrays
(SoA) layout — per-field segments instead of whole-element slots — inside the SAME single
allocation as the by-value representation above (D2). Zero syntax change; byte-identical program
output in both scheduling modes. Shipped in v0.3-M5 (plan `2026-07-03-v0-3-m5-auto-soa`, Phases
4–5), riding the by-value substrate. Graduated here from `SCRATCH-future-auto-soa.md` (now a
pointer stub). *(Terminology: "Struct-of-Arrays"/"SoA" is the engineering term of art and is fine
in this internal doc; ALL user-facing text says "stored as separate per-field arrays" — see D9
below.)*

### Admission criteria (D3, D4, D5)

An array is SoA-admitted only when ALL hold; any failure declines safely to the AoS baseline:

1. **Compile-time-provable length > `SOA_SIZE_THRESHOLD` (64) and NO growth ops** (`.add()`/push
   anywhere on the binding declines) — D3. Growth under segmented layout forces a per-segment
   re-layout on every realloc: real cost, zero proven demand. The provable-length rule keeps the
   "length > threshold at the analysis-time proof point" criterion honest instead of guessing
   runtime sizes. **Rejected:** runtime-length admission (revisit: FR #5, if PGO/adaptive layout
   ever lands) and grown-array admission (FR #4).
2. **Escape-decline** — an array passed as an argument, returned, stored into another value, or
   crossing a module boundary is DECLINED (the analysis cannot see all accesses; a
   partial-visibility layout decision is the silent-wrong class) — D4. The shape-level `lend self`
   method filter is retained as defense-in-depth on top; it becomes load-bearing when
   cross-function propagation lands (FR #3). **Rejected:** cross-function propagation in M5 —
   interprocedural field-access summaries are their own analysis.
3. **≤ 2 fields in the UNION of fields accessed across ALL loops over the array** in the owning
   function; ≥ 3 declines — D5. **Rejected:** per-loop scoring — conflicting per-loop layouts are
   unimplementable (one array has one layout). Revisit: FR #9.
4. **No FFI export** — vacuously true today (D7, previous section).

### One layout authority — padding wins (E3, D11)

Both layout transforms (M4 false-sharing padding and SoA) are resolved by ONE authoritative
source — the layout-decisions query producing `LayoutDecisions`
(`crates/ynz-typeck/src/soa.rs`) — consumed by every codegen path, never re-derived (per
[`.claude/rules/authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md);
the twin-derivation class shipped silent miscompiles four milestones running). When a shape is
simultaneously cross-thread-padded AND an SoA candidate, the authority resolves to
**padding-only** (D11): correctness under false sharing outranks cache-locality perf, and a
cross-thread array is outside SoA's provable-single-thread model anyway (D4 would independently
decline it). A byte-layout-asserting both-candidate fixture locks the precedence.

### Kernel mode: SoA is disabled under `--kernel` (D1)

Mirrors M4 padding's gate idiom (one predicate, same dual-mode testing story). Kernel mode has
zero real programs today; divergent kernel×layout states would multiply the fixture matrix for no
user. Revisit: FR #7.

### Teaching surface — Tier 3 lint only, no muted hint (D9)

Auto-SoA is the canonical "no typeable explicit form" auto-promotion
([`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md)): **Tier 3 lint
YES, muted hint NO** — there is no `soa Array<T>` syntax for click-to-make-explicit to produce.
The lint rule is `array-using-soa-layout` (`registry/features.toml`), firing on the array
declaration. The *identifier* is roadmap-locked registry-internal convention (the "soa" acronym is
fine there); ALL user-facing hover text is jargon-free — "stored as separate per-field arrays",
never "struct"/"Struct-of-Arrays" — and is mechanically gated by
`no_banned_jargon_in_lint_rule_templates` in `crates/ynz-diagnostics/tests/jargon_audit.rs`, which
audits every `[[lint_rule]]` entry's template text. The hover's WHY cites both honest measurements
below, never a generic claim. The DAP debugger unified-view over SoA storage is deferred
(`[[deferred_tooling_feature]] dap-soa-unified-view` — zero DWARF substrate exists; FR #1).

### Honest performance provenance (E14)

**Current shipped reality (v0.3-M7 onward — see the CHANGELOG entry and the
`2026-07-04-v0-3-m7-optimizer-pipeline` plan): `ynz build` now runs a real LLVM `default<O2>` pass
pipeline by default.** The measured -O2 win described below is therefore the win shipped binaries
actually get today, not a projected/unreached one. `--no-optimize` / `YNZ_NO_OPTIMIZE` remain as an
opt-out back to the old unoptimized codegen, for anyone who deliberately wants it.

The correctness win is unconditional: byte-identical dual-mode output at every measured point, all
checksum/IR gates green, on the one elem_size-aware representation. The performance claim carries
a hard caveat, with both measured results stated separately and **never conflated**
(provenance: [`soa-threshold-raw-2026-07-04.md`](../../../crates/ynz-driver/benches/soa-threshold-raw-2026-07-04.md),
"Step 3 calibration verdict"). **These two measurements are historical, pre-M7 methodology** — taken
back when `ynz build` still emitted at `OptimizationLevel::None` with zero LLVM pass pipeline, to
separate the correctness win from the (then-unreached) optimizer win:

- **Baseline measured at the OLD `OptimizationLevel::None` default** (pre-v0.3-M7): net SoA/AoS
  ratios **1.00–1.18** at every N in {8..4096} — no crossover anywhere; 9 of 10 points trend 4–18%
  slower with N=16 at parity (0.997), each individually below the ~15% noise floor. Net effect ≈
  **1.0x — no detectable benefit** at that old baseline.
- **Identical generated IR under `opt-18 -O2`** (diagnostic, N=4096): SoA wins ≈ **3.3x** net of
  spawn (10.77 ms AoS vs 3.29 ms SoA), consistent with the 4x theoretical bandwidth edge for
  2-of-8 hot fields. As of v0.3-M7, this is no longer a diagnostic-only, unreached number — it is
  the class of win `default<O2>` now delivers in shipped binaries by default.

`SOA_SIZE_THRESHOLD = 64` shipped at M5 as a **documented conservative default, NOT a
crossover-calibrated constant** — at the time, there was no O0 crossover to calibrate against, and
the honest verdict beat a derived number wearing unearned precision. Re-deriving this threshold
against real -O2 crossover data (now that the pipeline is live) is tracked as forward work — see
FR #2's revisit trigger below.

Two identified paths toward fully realizing the win, both recorded in plan
`2026-07-03-v0-3-m5-auto-soa`'s Future Requirements (in prose by FR number — the plan directory
moves buckets on completion). FR #14 is now moot for the O0 case but the selective-gather fix in
FR #15 still applies under the live optimizer:

- **FR #15 — selective hot-field-only element materialization**: current codegen gathers ALL
  declared fields per element access, ignoring the analysis's already-computed hot-field set; a
  selective gather is a directly perf-relevant fix independent of the optimizer pipeline (requires
  re-auditing every full-element consumer, so it is its own session).
- **FR #14 — stale-runtime-archive ABI-check footgun**: the driver embeds
  `target/{profile}/libynz_runtime.a` with no ABI/version check, so a stale archive silently
  miscompiles by resolving old-signature symbols by name — an operational guard until the
  versioned-symbol fix lands (it burned a full Phase 6 diagnosis round).

The optimizer-pipeline dependency (risk E14 / FR #2's revisit trigger) has now landed as of
v0.3-M7: shipped builds run the LLVM pass pipeline by default, so the measured -O2 win above IS the
shipped win. The threshold (`SOA_SIZE_THRESHOLD = 64`) still awaits re-derivation from real -O2
crossover data — that re-calibration is the remaining half of FR #2's trigger.

---

## Auto-promotion: `Array<T>` → `Set<T>` (membership-only usage; DESIGN NOT YET BUILT — needs `Set<T>`)

When the compiler proves an `Array<T>` is used ONLY for membership — `.contains()` / `.add()` / `.remove()` / `.size`, with NO indexing, NO reliance on element order, and NO reliance on duplicate storage — the array is doing a set's job at an array's O(n²) cost. Promoting it to `Set<T>` storage takes membership + uniqueness from O(n²) → O(n). Real perf win.

Surfaces (same hybrid model as `Array → Fixed`, with one key difference):
- **Codegen auto-promotion**: when the proof holds, emit `Set<T>` storage. Because the proof needs whole-program escape/usage analysis, it's gated to `--release` (like auto-SoA), not every dev build.
- **Tier 3 lint suggestion (primary surface)**: `prefer-set-for-membership` — recommends declaring `Set<T>` explicitly. This is the *main* surface here, because the real fix is "you reached for the wrong type."
- **Muted IDE hint**: `Set<T>` IS typeable, so the protocol applies — click-to-make-explicit rewrites the declaration to `Set<T>`.

**The proof is stricter than `Array → Fixed`**: a `Set` drops indexing, positional order and duplicate storage, so the compiler must prove none of those three is observed before it swaps. When it can't prove it, the lint still fires (teaching) but the codegen swap does not (safety).

### `Fixed<T>` is not blanket-promoted to `Set<T>`

The membership-only usage analysis also applies to `Fixed`, but the compiler does not promote `Fixed → Set` by default: `Fixed` is small, stack-allocated and cache-hot, and for the small N it is built for, a linear scan over a contiguous stack buffer beats a heap-allocated hash lookup. Auto-promotion only happens when the stricter form is unambiguously better, and this one depends on size.

What the compiler does instead for a membership-checked `Fixed`:
- **Compile-time-constant contents** (`Fixed<Int> = [1, 5, 10, 42]` used for `.contains()`) → a **perfect-hash or branchless comparison / jump table**, computed at compile time, **zero heap** — strictly better than a runtime `set`. (Ties into the perfect-hash tier of map's four-tier hashing.)
- **Genuinely large `Fixed` + heavy membership** → a `set` *could* win, but that's a size-threshold heuristic, not a clean promotion; flagged as a possible `--release` analysis, not a default.

So: `Array → Set` is a real promotion (arrays are heap and variable-size already, so set storage is a lateral-or-better move); `Fixed → Set` is declined in favor of zero-alloc membership strategies for small or constant fixeds.

---

## String Methods: `.byteAt()`, `.graphemeAt()`

Default indexing on strings (`.get(n)` / `s[n]`) is by **Unicode code point**. Two escape valves exist for explicit access modes:

- `.byteAt(n)` — `Maybe<Int>` — n-th UTF-8 byte. For parsers and protocol handling.
- `.graphemeAt(n)` — `Maybe<String>` — n-th grapheme cluster (what a human sees as "one character"). For text rendering and cursor positioning.

Companion length methods:
- `.count()` — code point count
- `.byteCount()` — UTF-8 byte count
- `.graphemeCount()` — grapheme cluster count

**Default is code points**: they match the "characters" of typical text, bytes give a partial code unit on multi-byte UTF-8 (`"café".get(3)`), and graphemes are correct but too expensive to be the default.

**There is no `char` type**: a one-character `String` covers every use, and a `char` would force a choice (code point, grapheme or byte) that is wrong for some use case.

## `Fixed<T, N>` in function signatures (v0.3-M9 C1h)

State in force; the history is in the commits (`git log --grep="Fixed<T, N>"`).

- **Cost rule (Golden Rule 6).** A length is a compile-time constant, never a runtime field and never a pointer-and-length pair: `Fixed<T, N>` costs what Rust's `[T; N]` costs. Measured on the emitted machine code (`build(a, b, c, d) -> Fixed<Int, 4>` and `third(xs) -> Int`): the same instructions as the Rust program (`mov` + four stores + `ret`; one `mov` + `ret`), no allocation. A `Fixed<Shape>` list still keeps one heap cell per item (the existing item representation, unchanged by this work), which Rust's inline `[S; N]` does not.
- **Type.** `Type::BuiltinFixed.size` is a `FixedLength`: `Known(n)`, `Unknown` (the result of `filter`/`unique`/`limit`/`freeze`, `append`/`prepend`/`concat`), or `OfParam` (the length of an unsized `Fixed<T>` parameter: the body is checked once with the placeholder).
- **One reader of a written `Fixed<…>`**: `crates/ynz-typeck/src/fixed_length.rs` (`read_fixed_arguments`). The checker's side (diagnostics, call binding, return-length work) is `check/fixed_lengths.rs`. A length may be written only as the whole type of a parameter, a return value or a shape field (inline shapes included, `check_shape_field_type`); elsewhere, and on any other generic, it is refused (`FixedLengthNotAllowedHere`). A field written without a length is refused (`FixedFieldLengthMissing`).
- **Return by value.** A `Fixed` of known length comes back as `[N x i64]` (plain functions: `abi_return_type`; suspending functions: the cells ride the frame's `Return` region, copied out by the wrapper or the calling state machine). A function whose return type writes no length gets the length every `return` gives, worked out by re-running the body check until no function's length changes (`check`); disagreeing or unknown lengths are refused (`FixedReturnLengthsDiffer`, `FixedReturnLengthUnknown`).
- **Per-length versions.** An unsized `Fixed<T>` parameter makes the function compile once per length its callers pass (symbol `name<3>`). `fixed_length::close_instances` is the ONLY place the set of versions is worked out (`TypedModule::fixed_instances`); codegen declares, lowers and calls exactly those versions and works out no length itself (`Cg::fixed_version_symbol`, `Cg::resolve_fixed_length`). A function that suspends or may be promoted to a task host is compiled once (`why_built_once`, the one predicate), so it has no versions. An unsized `Fixed<T>` parameter on it is refused (`FixedLengthMissingInTaskFunction`) only when its body asks for the length (`fixed_length_use::LengthUses::needs_length`: any use but handing the list to a callee that does not need it, or owning it with heap items to release); a function that only receives the list works, because a spawn's copy is sized and released from the argument's type at the call site (`release_glue_free_kind`).
- **Exports** must write the length of every `Fixed` they take or return (`FixedLengthMissingInExport`): an importer sees only the signature.
- **Shape fields.** A `Fixed<T, N>` field is stored by value: an `[N x i64]` member (`[N x i128]` for a decimal128 item) of the shape's struct (`shape_types.rs`), so the length is in the type and nothing is stored beside the cells. Reading the field yields the address of its first cell; a store copies the cells and a `Fixed<String>`/`Fixed<Shape>` field gets an item of its own per cell unless the source is moved; the shape's release walks the cells in place; every deep copy (`deepen_shape_fields`) copies each cell's item. A `let` of a field read is judged like an array or shape read (`non_owned_reads`): a view when nothing writes it, else a copy in an entry-block slot.
- **Background tasks.** A plain `Fixed<T, N>` result rides the moved cell a union result rides (`HANDLE_RET_KIND_UNION`, `handle_cell_bytes`): `receive()` copies the cells and their items into a block of the receiving frame; an unreceived result releases its items and the cell.
- **Not built / parked.** `freeze`, `filter`, `unique`, `limit` build lists of runtime length and are not lowered in codegen. A list of unknown length (`Unknown`) read from one of them reads `count` 0 as before.
