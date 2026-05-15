# Collections — Design Decisions

User spec: `spec/collections.md`

---

## `fixed<T>` and `array<T>`

`fixed<T>` = stack-allocated, size-locked at creation. `array<T>` = heap-allocated, growable.

**Why**: Golden Rule 10 — the fast default should be the default path. Developers who don't think about performance automatically write fast code. `fixed<T>` requires no annotation — it's what you get when you declare a collection with known content. `array<T>` is the explicit opt-in for when you need growth.

**Removed**: `collection<T>` type removed — unnecessary complexity. Two collection types are enough. "Collections" still exists as an English umbrella term covering `fixed<T>`, `array<T>`, `map<K,V>` — but it's not a TYPE, just a category name.

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

Works on: `array<T>`, `fixed<T>`, `map<K,V>`, `string` (read only — strings are immutable).

Does NOT work on: `type` instances. Types use dot access for fields and methods — brackets at a value position on a type are a compile error. This forces the visual distinction between "compile-time-known field name" (dot) and "runtime value lookup" (bracket).

**Why bracket sugar:**
- **Familiarity:** Every JS/Python/C-family dev expects `arr[0]`. Forcing `.get(0)` everywhere creates needless friction.
- **Safety preserved:** The sugar still returns `maybe T`. The type system enforces handling. Surface looks familiar; semantics stay safe.
- **No syntax conflict with type parameters:** Different syntax, different position. `array<Player>` is a TYPE (left of equals or in a type annotation); `players[0]` is a VALUE (right of equals or in an expression). TypeScript proved this distinction works for human readers.

**Reverses an earlier decision:** the original `design/collections.md` rejected `map["key"]` notation citing visual ambiguity with `array<Player>` type syntax. That argument was weaker than originally stated — TS does both fine. Reversing for consistency: brackets work universally on all four collection types for read; on map/array/fixed for write.

**Why dot doesn't work on map keys (the real reason):**

Maps have methods (`.count()`, `.get()`, `.set()`, `.keys()`, `.values()`, `.filter()`, etc.). If keys could be accessed via dot, a key named `count` would collide with the `.count()` method — exactly the JavaScript bug class around `obj.constructor`, `obj.toString`, etc.

Types don't have this problem because the user defines both fields and methods in one place, and the compiler can verify no collision. Map keys are runtime user data; method names are compile-time built-ins. The two namespaces can't be unified.

Bracket access keeps the namespaces visually separated: `m["count"]` is unambiguously a key lookup; `m.count()` is unambiguously a method call. No collision possible.

**Out-of-bounds on `.set()`:**

```yinz
let arr: array<number> = [1, 2, 3]
arr[10] = 99
// RUNTIME ERROR: index 10 out of bounds (arr.count() == 3).
//   .set() replaces existing elements only.
//   To append: use .add(99).
//   To grow with defaults: use .resize(newSize, defaultValue).
```

Sparse-array growth (JS's `arr[100] = "x"` creating `<97 empty items>`) is rejected — it's a footgun, not a feature.

---

## Methods Added: `.set()` on `array<T>` and `fixed<T>`

Originally `.set()` only existed on maps. For the bracket-write sugar to work consistently, arrays and fixed arrays also need `.set(index, value)`:

- `array<T>.set(lend self, index: int, value: T) -> nothing` — replace at index. Runtime error if out of bounds.
- `fixed<T>.set(lend self, index: int, value: T) -> nothing` — same. Index out of bounds is a compile error when the index is a literal AND the size is known; runtime error otherwise.

These replace existing elements only. Use `.add()` to append; `.insertAt(i, value)` (planned, may not be v0.1) to insert.

---

## Map `.update({...})`, `.filter()`, `.find()`

Three additions to the map API:
- `.update({...})` — bulk add/update from an object literal. Merges keys, adds new ones, updates existing ones.
- `.filter(fn)` — returns new map with only matching entries. Callback receives `entry` with `.key` and `.value`.
- `.find(fn)` — returns `maybe` entry. Same callback shape.

**Why**: `.set(key, value)` one at a time is verbose when updating multiple keys. `.update({...})` follows the same object-literal syntax used for type construction. Map filter/find fills an obvious gap — there was no way to search a map without calling `.entries()` first.

---

## `map<K,V>` Implementation — Swiss Tables, Four-Tier Hashing

Swiss Tables (open-addressing with SIMD metadata scan) are the locked implementation for `map<K,V>`. No separate chaining, no linked lists. Modeled after Google Abseil, Rust hashbrown, Go 1.24+. Documented because the "easy" implementer choice would be separate chaining; this lock prevents that.

### Why Swiss Tables (and not separate chaining)

- **Cache locality**: entries live in the bucket array directly, not in heap-allocated linked-list nodes. Lookup avoids pointer chasing — every probe stays in one or two cache lines instead of jumping to whatever heap address holds a node.
- **SIMD metadata scan**: a parallel byte array of "control bytes" (one per slot, holding empty/deleted/hash-fingerprint) is searched 16 at a time using a single SSE2/NEON instruction. Match-or-prove-absent in 1-2 cycles instead of N comparisons.
- **Memory overhead**: no per-entry linked-list infrastructure. ~16 bytes saved per entry. Datadog reported ~70% memory reduction switching Go to Swiss Tables in 1.24.

The 60% microbenchmark win Go reported is mostly (a) cache locality and (b) SIMD scan. Real-world apps see ~1.5% geometric mean — but the win compounds in serialization-heavy code (JSON parsing, index building) where maps dominate.

### Four-tier hashing strategy

Different scenarios get different hashing. The compiler picks based on what it knows at compile time. Users don't pick — the IDE shows which tier was used as a muted hint.

| Scenario | Compiler emits | Lookup cost |
|---|---|---|
| `shape Player { name: string; health: int }` (typed access — not a map at all) | Direct memory offset | 1 instruction |
| `map<string, T>` literal with all-static keys: `{ "alice": 1, "bob": 2 }` | **Perfect hashing** (compile-time-generated) — zero collisions, zero probing | ~3 instructions |
| `map<int, T>` (any integer key type) | Identity hash or single multiplication — no hash function overhead | ~2 instructions |
| `map<K, V>` runtime keys (general case — DEFAULT) | Swiss Tables + SipHash-2-4 (DoS-safe with random per-process key) | ~30-50 instructions |
| Trusted-keys variant (for non-adversarial workloads — surface syntax TBD at M4, candidate name `.fast`) | Swiss Tables + xxhash3 (fast non-cryptographic) | ~10-20 instructions |

### Default hash function: SipHash-2-4 (safe-by-default)

For the general case (runtime keys, no opt-in), the default is **SipHash-2-4** with a per-process random key — DoS-safe against attacker-controlled input. This matches industry consensus established after the 2011 HashDoS attacks: Python switched to SipHash for `hash()` in 3.4 (2014); Rust HashMap defaults to SipHash 1-3 since 1.0; Ruby switched in 2.0; Perl in 5.18.

Three reasons specific to Yinz to default safe (not fast):

1. **The cases where SipHash overhead matters are the same cases where it's needed.** Yinz's compile-time tiers already eliminate hash function cost from the cases where DoS-safety doesn't matter: shape access (1 instruction, no hash), perfect hashing for static-key maps (no SipHash), identity hash for int-keyed maps (no SipHash). Only `map<string, V>` with RUNTIME string keys hits the SipHash overhead — and that's overwhelmingly the case where keys come from external input (JSON parsing, HTTP headers, query params, config files = attacker-controlled).

2. **Yinz's target audience is jr devs writing web APIs.** Asking them to know about HashDoS BEFORE they get bitten is teaching at the wrong moment. Defaulting safe means they're protected from a CVE class they don't yet know exists. The fast opt-in is the right teaching moment for senior devs who know their workload's threat model.

3. **Industry consensus.** Rust, Python, Ruby, Perl all default to DoS-safe hashing. The major outlier (Java pre-9) shipped CVE-2011-4858 (denial of service via hash collisions). The 4-84% slowdown Rust devs sometimes complain about (and reach for the `fxhash` crate to escape) is real, but the alternative — every web service exploitable for years — is much worse.

### Fast opt-in: xxhash3 — for explicitly-trusted workloads

For perf-critical maps where the user can prove the keys are NOT attacker-controlled (in-memory caches, internal indexes, intermediate algorithm data structures, compiler-internal symbol tables), there's an explicit opt-in for the fast variant. Backed by xxhash3 — fast, well-distributed, not DoS-safe.

**Surface syntax is deliberately NOT locked here** — this section locks the BEHAVIOR (Swiss Tables + xxhash3) but the user-facing way to opt in is a language design decision for M4 implementation. Candidate name `.fast` (concise, describes intent), or keyword form like `trusted map<K,V>`, or a construction flag — TBD when the broader question of variant-syntax for collections is decided. Yinz currently has no precedent for dot-on-type-name modifiers, so don't lock placeholder syntax that doesn't exist anywhere else in the language.

### IDE teaching surface

The IDE muted-hint protocol surfaces which tier each map uses, so devs SEE whether their map is the safe default or the fast opt-in — no silent perf characteristics.

When the compiler can prove the keys are NOT externally-controlled (e.g., the map is constructed and used entirely within a function whose inputs are themselves not externally-derived, OR all keys are compile-time literals → handled by perfect hashing instead), the IDE emits a Tier 3 suggestion: "this map's keys appear trusted. Consider the fast variant — saves ~3× on hash cost." The lint goes the SAFER direction by default; user opts in to fast when they're sure.

When the compiler picks perfect hashing or identity hash for a literal, the IDE shows a muted hint: `// perfect hash — zero collisions for these compile-time-known keys`.

### Why not let the user pick from N hashers

C++ template parameters explode when stdlib offers many hashers (`std::unordered_map<K, V, Hash, KeyEqual, Allocator>` with 4 type parameters). Yinz keeps it simple: the safe default covers 95% of use cases (and the cases where overhead matters are exactly the cases that need safety), the fast opt-in covers the security-irrelevant 5%, and the compiler auto-selects perfect/identity hash where it can. No template-parameter explosion.

---

## `map<K,V>` Iteration Order — Insertion Order

Locked: iteration order is INSERTION ORDER. Same as Python 3.7+, JavaScript Map, Java LinkedHashMap. NOT random (Go, Rust default).

**Why**: random iteration was meant to prevent devs from relying on order. In practice, after 10+ years in production, it caused the "test passes locally, fails in CI" bug class without preventing any meaningful misuse. Every language designed in 2020+ that's revisited the question picked insertion order. Yinz doesn't relitigate.

**Cost**: one pointer per entry to track insertion order. Trivial overhead (~8 bytes per entry on 64-bit). Implementation: keep a parallel insertion-order index alongside the Swiss Table bucket array.

If a user genuinely doesn't care about iteration order and wants to skip the overhead, they can opt into an unordered variant (surface syntax TBD at M4 alongside the adversarial-variant syntax — same syntax-design problem). Random iteration is still NOT a default — even unordered, the iteration order is implementation-defined but deterministic per-build.

---

## `shape` Field Layout — Compiler Auto-Reorders for Optimal Packing

When the compiler emits the memory layout for a `shape` declaration, it auto-reorders fields for optimal packing (largest-alignment first, smallest last). User code is unaffected — `shape` access syntax (`player.health`) and field semantics stay identical. Only the in-memory byte layout changes.

```yinz
shape Event {
  flag: bool      // user wrote: 1 byte
  timestamp: int  // user wrote: 8 bytes
  count: bool     // user wrote: 1 byte
}
// Compiler emits: timestamp (8) + flag (1) + count (1) + 6 bytes end-padding = 16 bytes
// (NOT what the user wrote in source order, which would have been 24 bytes with 14 bytes of padding)
```

Same shape, same semantics, ~33% less memory for this example. Multiply by N entries in an `array<Event>` and the cache benefit compounds.

**Why auto-reorder by default**: Yinz's principle is "fast by design even for inexperienced developers" (Golden Rule 10 + auto-promotion corollary). C/C++ require the user to manually order fields for packing — a perf trap that beginners universally fall into. Rust auto-reorders by default (FFI structs opt out via `#[repr(C)]`); Yinz inherits that pattern but goes further by NOT having an opt-out keyword for pure Yinz code.

**Why no opt-out keyword in Yinz**: the only legitimate reasons to pin field layout are:
1. **FFI to C** — handled by FFI bindings at the boundary, not by polluting the shape declaration. (FFI is v2+ per `design/mvp-scope.md`.)
2. **Wire protocols / file formats** — handled by serializer codegen (per `.claude/rules/stdlib-design.md` Rule 6 — compiler-generated specialized serializers per shape) which decides wire layout independently from internal layout.
3. **Memory-mapped hardware** — handled by `--kernel` mode infrastructure when v0.3 ships (per `design/future/no-runtime-mode.md`).

None of these require a `layout: c` modifier on the shape itself — the layout constraint comes from the BOUNDARY (FFI, wire, hardware), not from the shape. Yinz can do better than Rust here by handling layout at the boundary, not in the shape declaration. Locked: no user-facing layout-pinning syntax for shapes.

**IDE teaching surface**: per `.claude/rules/auto-promotion.md`, when the compiler reorders fields, the IDE shows a muted hint on the shape declaration:

```
shape Event {                     // muted: // reordered: timestamp, flag, count
  flag: bool                      //         saved 8 bytes per value
  timestamp: int
  count: bool
}
```

Hover tooltip:
- **WHAT**: This `shape` is laid out as `timestamp (8) + flag (1) + count (1) + 6 bytes end-padding = 16 bytes` instead of source-order (which would be 24 bytes with 14 bytes of padding waste).
- **WHAT INSTEAD**: There's no source-level syntax to pin a different layout for pure Yinz code. If you need a specific layout for FFI (v2+), the FFI binding handles conversion at the boundary; if you need it for a wire format, the serializer codegen produces the correct wire bytes regardless of internal layout.
- **WHY**: Largest-alignment-first packing eliminates padding bytes between mismatched-alignment fields. The savings compound when the shape is stored in `array<Shape>` collections — better cache utilization for hot field-access loops.

This pattern qualifies as auto-promotion (per `.claude/rules/auto-promotion.md`) — the codegen surface always applies; the muted hint applies because there IS a "what would my source-order layout have been" form the user could mentally reason about; no Tier 3 lint suggestion because there's no explicit-form rewrite that would help (the user's source order is already what they wrote — the compiler just rearranges internally).

---

## Sort — Compiler Auto-Picks Based on Element Type

The primary API is `.sort()`. The compiler picks stable or unstable codegen based on the element type — beginners never have to think about this.

| Element type | Compiler picks | Reasoning |
|---|---|---|
| `array<int>`, `array<float>`, `array<bool>`, `array<string>` | Unstable (fast in-place quicksort family) | Equal primitives/strings are interchangeable — there's no "original order" between two `5`s or two `"hello"`s to preserve. Pure speed and memory win. |
| `array<Shape>` (any composite type) | Stable (TimSort or equivalent) | Equal-keyed shapes might have other fields the user wants preserved in order. Default to safe. |
| `array<array<T>>`, `array<map<K,V>>`, `array<Entry<K,V>>` | Stable | Same — nested or composite types. |

This is the auto-promotion pattern (per `.claude/rules/auto-promotion.md`):
- **Codegen**: compiler picks per element type, always. No runtime cost to the decision.
- **Muted IDE hint**: shows what was picked, e.g., `// unstable sort (int — equal values interchangeable)` or `// stable sort (Shape — equal keys preserve original order)`. Click does NOT have a typeable equivalent for the auto-pick itself, but the explicit override forms below ARE typeable.
- **Tier 3 lint suggestion**: not generally applicable — the auto-pick is right for ~99% of cases.

### Explicit overrides (rare, opt-in)

For the cases where the auto-pick is wrong, two explicit forms:

- **`.sortFast()`** — forces unstable regardless of element type. Use when the user knows equal items are interchangeable even though they're composite (e.g., `array<Shape>` where the sort key is a UUID and there are no other relevant fields).
- **`.sortStrict()`** — forces stable regardless of element type. Use when the user knows they need stable behavior even on primitives (the canonical case: multi-step / radix sort, see "Gotcha" below).

Naming follows Golden Rule 12 (human-readable over jargon): `Fast` describes user intent, `Strict` describes the property. Neither uses the word "unstable" (jargon — sounds like "broken" to a non-programmer).

### Multi-step sort detection — compiler upgrades unstable to stable

The compiler also detects in-function multi-step sort patterns and upgrades the auto-pick from unstable to stable. Pattern: two-or-more `.sort()` calls on the same variable, in sequence, with no intervening modifications to the array.

```yinz
let nums: array<int> = [...]
nums.sort(n => n % 10)        // muted hint: // unstable sort (int — equal values interchangeable)
nums.sort(n => n / 10)        // muted hint: // stable sort (multi-step pattern — preserving previous .sort() order)
                              // Compiler upgraded the auto-pick because nums was already sorted in this function.
```

This is single-function data-flow analysis — same machinery as `array<T>` → `fixed<T>` proof. Adds the smarts where they pay off and stays out of the way otherwise.

### Cross-function multi-step — explicit `.sortStrict()` needed

The compiler can't easily trace multi-step patterns across function boundaries. If you sort in one function and re-sort in another, the in-function detector won't catch it:

```yinz
function sortByLow(lend nums: array<int>) {
  nums.sort(n => n % 10)        // unstable — fine in isolation
}

function processData(lend nums: array<int>) {
  sortByLow(nums)
  nums.sort(n => n / 10)        // unstable — but caller intended this as step 2!
                                // Compiler can't know — cross-function intent isn't local
}
```

For cross-function multi-step, the user must reach for `.sortStrict()` explicitly on the second-and-later sorts. The IDE muted hint showing "unstable" is the visible warning when this matters; cross-function multi-step is rare enough that the explicit-opt-in cost is acceptable.

### Why type-based auto-pick is the right default

JavaScript's sort was unstable-by-spec from 1995 to ES2019 — 24 years. V8 used QuickSort for arrays >10 elements; SpiderMonkey used a different algorithm. Same code produced different sort orders across browsers AND across array sizes. Applications relying on sort order for deterministic rendering or test comparisons silently broke when array size crossed the threshold or browser changed (https://v8.dev/features/stable-sort).

Java got it right by accident — `Collections.sort()` has been stable since at least JDK 1.4 because objects HAVE hidden state worth preserving. `Arrays.sort(int[])` is unstable because primitives don't. Yinz formalizes this intuition: the compiler picks based on what the type permits.

### Performance characteristics

- **Stable (TimSort family) on nearly-sorted data**: O(n) — detects existing runs and merges them. 16M-element benchmark: 0.15s vs 2.21s for introsort = 14× faster.
- **Stable (TimSort family) on random data**: O(n log n) with ~20-30% overhead vs an in-place quicksort due to auxiliary memory and merge work.
- **Unstable (introsort / dual-pivot quicksort)**: O(n log n), in-place, no auxiliary allocation, fastest on random primitive data.

### Glossary — what "stable" actually means

If you're not sure: when two items compare equal, **stable** keeps their original relative order. **Unstable** makes no guarantee — equal items can end up in any order, and different runs can produce different results.

```yinz
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
3. **Test reproducibility**: stable sort gives identical output every run on every machine. JavaScript's 24-year unstable-spec window caused a generation of "test passes locally, fails in CI" bugs.
4. **Audit trails**: sorting log entries by timestamp. Two events with the same timestamp must stay in the order they were logged.

When stability doesn't matter (so unstable wins on speed AND memory):
- Sorting primitives — equal numbers are interchangeable, no original order between two `5`s
- Sorting then immediately discarding (e.g., `.sort().first()` to find the minimum)
- Any collection where equal items are truly fungible

The auto-promotion pattern (per `.claude/rules/auto-promotion.md`) applies: when the compiler can detect a sort target is a `fixed<T>` of a primitive type AND the sorted result isn't passed to anything depending on stability, the IDE shows a Tier 3 lint suggestion `prefer-unstable-sort-for-numeric-collections` recommending `.sortUnstable()` for the perf win. No codegen auto-swap (semantics differ — different equal-keyed element ordering); user makes the call.

---

## Map Literals Pre-Size at Compile Time

When a `map<K, V>` is constructed with a literal initializer where the entry count is known at compile time, the compiler emits codegen that pre-sizes the internal Swiss Table to fit the literal's entries with no-resize headroom. No runtime resize storm.

### Why this matters

Java `HashMap` initialized with `new HashMap()` resizes ~16 times when populated with 1M entries (https://www.baeldung.com/java-hashmap-optimize-performance). Each resize copies all existing entries. Pre-sizing to the known count eliminates all resizes — pure compile-time work that beginners never do because the literal-form constructor doesn't expose a capacity parameter in most languages.

### Auto-promotion (codegen-only)

Per `.claude/rules/auto-promotion.md`:
- **Codegen**: compiler counts entries in the literal initializer, allocates the Swiss Table at `ceil(N / load_factor)` buckets directly. Always applies when the count is statically knowable.
- **Muted IDE hint**: not applicable — there is no surface syntax for "pre-sized map" (no `map<K, V>(capacity: N)` form exists in v0.1, and adding one would create a parallel API per `.claude/rules/stdlib-design.md` Rule 2). The user can't write the explicit form, so the muted-hint protocol doesn't fit.
- **Tier 3 lint suggestion**: `prefer-presized-map` fires when a map is constructed via `.set()` calls in a loop with a known iteration count (e.g., a literal range or array length) — suggests rebuilding as a literal initializer or noting that the loop pattern misses the pre-size optimization.

### Tradeoff

A pre-sized map for a 3-entry literal is slightly larger than the smallest possible bucket array (typically 16 buckets even for 3 entries). The "wasted" headroom is negligible for small maps and saves real resize work for medium-to-large literals.

---

## `array<T>` Growth Factor — 1.5×

When `array<T>` runs out of capacity, it grows by **1.5×** (Java/Folly choice). Locked.

**Why 1.5× and not 2×**: with 2× growth, the new allocation is always larger than the sum of all previous allocations — so freed memory from prior reallocations can never be recycled. With 1.5×, after enough reallocations the new allocation can fit into space freed by older ones. Folly's `fbvector` documents this; GCC's `std::vector` (2× growth) does not allow memory reuse.

**Why not 1.125× (Python's choice)**: 1.125× requires ~3× more reallocations than 1.5× to reach the same size (86 vs 29 reallocations to reach 1M elements per Tim Peters' analysis). Each reallocation copies all existing entries — too much CPU waste.

**`fixed<T>` covers known-size**: if the user knows the final size, they should use `fixed<T>` from the start (stack-allocated, size-locked, zero growth). The `array<T>` growth factor only matters when the size is genuinely dynamic. There is no `array.withCapacity(n)` API — `fixed<T>` IS the answer for "I know the size."

### Auto-promotion: `array<T>` → `fixed<T>` (hybrid model — three things happen)

When the compiler can prove an `array<T>` declaration is never grown (no `.add()`, `.remove()`, `.resize()`, no `.lend` to a function with a may-grow signature), THREE things happen:

1. **Codegen auto-promotion (silent perf win)**: the compiler emits `fixed<T, N>` codegen for the binding — stack-allocated, no heap, no growth tracking. The user gets the perf benefit automatically without rewriting source.

2. **Muted IDE hint (informational, always-on)**: per `.claude/rules/inference.md`, the IDE shows a neutral-gray muted hint after the binding (e.g., `// promoted to fixed<int, 3>`). Click-to-make-explicit converts the source to `fixed<int> = [1, 2, 3]`. This surface tells the user the inference happened — they SEE that the compiler made a perf-positive decision, even if they don't act on it. Same protocol as type-inference and ownership-inference muted hints.

3. **Tier 3 lint suggestion (visible teaching surface)**: a Tier 3 lint suggestion (yellow squiggle in IDE, surfaced in compile output) recommends rewriting the source as `fixed<T>` explicitly for code-review clarity and future-proofing. Per `design/linting.md` rule `prefer-fixed-when-immutable`. This surface is the teaching nudge — the lint says "best practice is to write the stricter form yourself."

```
let nums: array<int> = [1, 2, 3]   // muted hint: // promoted to fixed<int, 3>
                                    // yellow squiggle from prefer-fixed-when-immutable
                                    // (compiler already emitted fixed<int, 3> codegen)
```

Why three surfaces (not just two):
- The **muted hint** answers "what did the compiler decide?" — visible always, click to materialize.
- The **lint suggestion** answers "what should I write in source?" — actionable, recommends rewrite.
- The **codegen** answers "what runs at runtime?" — invisible, auto-applied.

Hover tooltip on the muted hint:
- **WHAT**: The compiler is treating this `array<int>` as `fixed<int, 3>` because it proved no growth happens. Stack-allocated, no heap, no growth tracking.
- **WHAT INSTEAD**: Click to convert the source to `fixed<int> = [1, 2, 3]`. Behavior identical.
- **WHY**: `fixed<T>` is the stricter form when the size is known. The compiler picks it automatically when it's safe; the explicit source form makes the choice visible in code review and prevents a future `.add()` from silently switching the codegen back.

Hover tooltip on the yellow lint squiggle:
- **WHAT**: This `array<int>` is auto-promoted to `fixed<int, 3>` codegen — the perf optimization already happened. This lint is about source-level clarity, not perf.
- **WHAT INSTEAD**: For explicit code as a best practice, write `fixed<int> = [1, 2, 3]` so the choice is visible to readers.
- **WHY**: Writing `fixed<T>` explicitly makes the perf characteristic obvious in code review (the next reader sees "stack-allocated, can't grow" without having to trace the access pattern) and prevents a future `.add()` from silently forcing the codegen back to heap allocation — if growth is unintended, the explicit `fixed<T>` makes that future `.add()` a compile error instead of a silent codegen change.

This hybrid model — auto-promote for perf, muted hint for visibility, lint suggestion for explicit-form teaching — is the canonical Yinz pattern for "stricter form fits AND explicit form is typeable." Same model used by `mutable-when-const-suffices` for `let → const` (`design/linting.md`).

Why hybrid (not pure-silent or pure-suggestion):
- **Pure-silent** (auto-Arc / auto-`wait` style) hides the optimization completely — fine for compiler-internal stuff like reference counting, but misses the teaching opportunity for choices the user can also make explicit.
- **Pure-suggestion** (lint-only, no auto-promotion) makes the user fix every instance manually before getting the perf win — punishes laziness, slow path stays the default until rewrite.
- **Hybrid** gets all three — perf is automatic, the inference is visible (muted hint), the explicit form is taught (lint suggestion).

For auto-promotions where the explicit form has NO typeable syntax (e.g., auto-SoA, where there's no `soa array<T>` keyword in v0.3), only the lint-suggestion surface applies — the muted-hint protocol requires click-to-make-explicit to produce real Yinz syntax. See `design/future/auto-soa.md` and `.claude/rules/inference.md` "Two Surfaces for the Same Decision" section.

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
