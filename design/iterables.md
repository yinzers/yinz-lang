# Iterables — Design Decisions

User spec: `spec/iterables.md`

---

## `follows Iterable<T>` with `next()` — Consistent with the Type System

Custom iteration uses the existing `follows` contract system. No special iterator syntax, no magic methods, no separate iterator trait.

**Why**: Consistent with operator overloading (which also uses `follows` contracts) and the `follows` system throughout the language. One mechanism — contracts — handles all cases where you want to customize language behavior for a type. A developer who knows `follows` already knows how to make a type iterable.

---

## `next(lend self) -> maybe T` — Ownership and Maybe

The `next` function takes `lend self` (it modifies the iterator's internal state — current position, buffer, etc.) and returns `maybe T` (`none` signals end-of-sequence).

**Why `maybe T` over a separate sentinel**: `maybe T` is the language's universal way to express "this might not exist." Using it for end-of-sequence is consistent. `none` = no more items = natural English.

**Why `lend self`**: Iterators are stateful by nature — they track a current position. `lend self` is the correct ownership modifier for a function that modifies the iterator value.

Per the non-OOP model (`.claude/rules/non-oop.md`), `next` is a STANDALONE FUNCTION — not a method inside the iterator shape. The contract declares its bare signature; each iterator's implementation lives as a standalone `function next(lend self: MyIterator) -> maybe T { ... }`.

---

## M7 Synthesized Iterator Wrapper Shapes (Locked)

Built-in collections formally follow `Iterable<T>` through compiler-synthesized wrapper shapes. These are real shapes — they get `ShapeTable` entries and monomorphize per concrete T via M5's `MonomorphizationTable`.

| Wrapper | Source | Contract-T | Stack-allocated? |
|---|---|---|---|
| `ArrayIter<T>` | `array<T>` | `T` | Yes — alloca, no `ynz_alloc` |
| `FixedIter<T, N>` | `fixed<T, N>` | `T` | Yes |
| `MapIter<K, V>` | `map<K, V>` | `MapEntry<K, V>` |  Yes |
| `StringCodePointIter` | `string` | `string` (1 code point per step) | Yes |
| `Range` | `range(start, end)` | `int` | Yes (Range IS the iterator — not wrapped) |

**`Range`** is a user-visible shape (not synthesized). It follows `Iterable<int>` directly with hidden state fields. Storable, passable, returnable as of M7 (M3 restriction removed).

**User-defined iterables** are NOT wrapped — they ARE the iterator. The shape follows the contract; the compiler verifies a matching standalone `next()` function exists.

**`next()` is `alwaysinline`** for built-in wrappers in optimized builds: LLVM `alwaysinline` attribute ensures no `call @next_array_iter` remains in `-O2` IR. For-loop over built-in collection emits no heap allocation (`alloca` only for the wrapper struct).

**String iteration default (locked):** `for c in "café"` steps by **code point** (not grapheme). Each `c` is a 1-character `string`. Grapheme iteration is opt-in via `.graphemes()` — deferred to v0.5+. This matches `.get(n)` default semantics.

---

## Hidden Fields for Iterator State

Iterator state (current position, buffers, page numbers) is tracked with `hidden` fields with defaults. This is what `hidden` was designed for — implementation details that the caller shouldn't see or set.

```
shape Range follows Iterable<int> {
  start: int
  end: int
  hidden current: int = 0    // iterator state — caller never sets this
  ...
}
```

---

## Two Contracts — `Iterable<T>` and `FallibleIterable<T>`

Resolved: two separate contracts. In-memory iteration uses `Iterable<T>`; iteration over I/O sources (files, network, paginated APIs) uses `FallibleIterable<T>`. The user almost never sees the distinction directly — the compiler infers it from the iterator's type and propagates errors only when needed.

```ynz
// Contracts use bare-signature form (no `function` keyword, no body)
shape Iterable<T> {
  next(lend self) -> maybe T
}

shape FallibleIterable<T> {
  next(lend self) -> maybe T errors
}
```

**Built-in contract assignments:**

| Source | Contract |
|--------|----------|
| `array<T>.items()` | `Iterable<T>` |
| `fixed<T>.items()` | `Iterable<T>` |
| `map<K,V>.entries()` | `Iterable<Entry<K,V>>` |
| `range(start, end)` | `Iterable<int>` |
| `file.lines(path)` | `FallibleIterable<string>` |
| `request.stream(url)` | `FallibleIterable<Response>` |
| Paginated API clients | `FallibleIterable<T>` |

**Compiler behavior at for-loop sites:**

The for loop syntax is identical for both contracts. The compiler checks the iterator's type:
- If it follows `Iterable<T>`, the loop runs anywhere.
- If it follows `FallibleIterable<T>`, the loop requires an `errors` context (or an explicit adapter). The compiler auto-propagates per-iteration errors using the same flow-sensitive narrowing rule as direct `errors` function calls (see `design/errors.md`).

**Stdlib adapters for ergonomic fallible-to-infallible conversion:**

```ynz
// Skip failed iterations silently (logs the error, continues to next item)
let lines = file.lines(path).orSkipFailures()        // Iterable<string>

// Get each step's success-or-failure as a value
let results = file.lines(path).withErrors()          // Iterable<Result<string>>
```

These let users iterate fallible sources from non-errors functions when they have a specific recovery strategy.

**M7 adapter semantics (locked):**

`.orSkipFailures()` is **PURE** — no I/O side effects. It silently drops failed iterations and continues. This is required by `stdlib-design.md` Rule 1 (pure-named methods must be pure). For users who want to log each skipped failure, compose with the separate `.logSkippedFailuresTo(sink)` method:

```ynz
// Silent drop of failures
iter.orSkipFailures()

// Logged failures + continue (compose explicitly)
iter.logSkippedFailuresTo(terminal.stderr).orSkipFailures()
```

`.logSkippedFailuresTo(sink)` takes a `LogSink` value. The `LogSink` contract:

```ynz
shape LogSink {
  write(lend self, message: string) -> nothing
}
```

In M7, `terminal.stderr` and `terminal.stdout` follow `LogSink`. The v0.5+ stdlib expands this to file sinks and user-defined sinks.

`.withErrors()` returns `Iterable<maybe T errors>` (NOT `Iterable<Result<T>>`). `Result` is on the banned-jargon list. Each iteration step yields an errors-capable maybe-value; the user inspects it with standard `.failed()` / `.message` / `.or()` machinery. Example:

```ynz
for (result in file.lines(path).withErrors()) {
  if (result.failed()) {
    log.warn(`bad line: ${result.message}`)
  } else {
    if (result.value.exists()) {
      process(result.value.value)
    }
  }
}
```

No new shape is needed — this reuses M7's own errors-capable mechanism uniformly.

**Why two contracts instead of one with optional `errors`:**

Yinz's `follows` contracts require method signatures to match exactly. There's no language-level mechanism for "this method returns `maybe T` OR `maybe T errors`" — adding one would complicate every `follows` contract for one feature. Two explicit contracts is simpler and matches the user's mental model: "is this iterator infallible or fallible?"

**Why not Option A (single contract that always returns `errors`):**

Forces every for loop (even `for (x in [1,2,3])`) into an `errors` context. Pollutes pure code with annotations for a failure mode that can't actually happen on in-memory data. Rejected.

**Why not Option B (swallow errors and return `none`):**

Violates Yinz's core error principle: failures are visible and structured. A `for (line in file.lines(path))` that silently stops on an I/O error and returns a truncated result is the exact bug pattern Yinz exists to prevent. Rejected outright.

**Writing a custom iterable — the choice is clarifying:**

```ynz
// In-memory data — implements the infallible contract.
// Shape declaration holds data fields only; standalone function provides the implementation.
shape CircularBuffer<T> follows Iterable<T> {
  items: array<T>
  hidden position: int = 0
}

function next(lend self: CircularBuffer<T>) -> maybe T {
  if (self.items.count() == 0) { return none }
  const value = self.items[self.position]
  self.position = (self.position + 1) % self.items.count()
  return value
}

// I/O data — implements the fallible contract.
shape ApiPager<T> follows FallibleIterable<T> {
  cursor: maybe string
  hidden done: boolean = false
}

function next(lend self: ApiPager<T>) -> maybe T errors {
  if (self.done) { return none }
  const response = request.get(self.buildUrl())   // can fail
  self.cursor = response.nextCursor
  self.done = response.nextCursor.exists() == false
  return response.item
}
```

The implementor answers ONE clarifying question — "can my iteration step fail?" — and picks the matching contract. It's a forcing function for thinking about failure modes, not a burden.

Each standalone `next` function is found by the compiler when verifying `follows Iterable<T>` (or `FallibleIterable<T>`) — see `.claude/rules/non-oop.md` for the structural-function-signature-matching mechanism.

---

## M7 MapEntry Destructuring Forms (Locked)

Two legal forms for map iteration:

**Single-binding form:**
```ynz
for (entry in scores) {
  let k = entry.key
  let v = entry.value
}
```

**Tuple-destructure form (desugar at parser level):**
```ynz
for ((k, v) in scores) {
  // desugars to: for (entry in scores) { let k = entry.key; let v = entry.value; ... }
}
```

Both forms desugar identically at codegen — the tuple-destructure form is parser sugar only. `MapIter<K, V>.next()` still returns `maybe MapEntry<K, V>`; the desugar step inserts the field-access bindings before the loop body.

---

## M7 Contract-T Resolution Table (Locked)

For built-in wrappers, the `T` in `Iterable<T>` or `FallibleIterable<T>` is:

| Source | Wrapper | Contract | T |
|---|---|---|---|
| `array<T>` | `ArrayIter<T>` | `Iterable<T>` | concrete T |
| `fixed<T, N>` | `FixedIter<T, N>` | `Iterable<T>` | concrete T |
| `map<K, V>` | `MapIter<K, V>` | `Iterable<MapEntry<K, V>>` | `MapEntry<K, V>` |
| `string` | `StringCodePointIter` | `Iterable<string>` | `string` (1 code point) |
| `range(start, end)` | `Range` (self) | `Iterable<int>` | `int` |
| `file.lines(path)` | (user-visible) | `FallibleIterable<string>` | `string` |

User shapes follow `Iterable<T>` or `FallibleIterable<T>` directly — no wrapper. The concrete `T` is whatever their `next()` function returns inside the `maybe`.

---

## Cross-References

- `design/errors.md` — `errors` keyword semantics (required for `FallibleIterable`)
- `.claude/rules/non-oop.md` — UFCS and standalone-function contract verification
- `design/collections.md` — `MapEntry<K, V>` shape definition
- `design/narrowing.md` — errors-capable narrowing (used in for-loop over `FallibleIterable`)
- `spec/iterables.md` — user-facing surface for all the above
