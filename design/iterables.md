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

```yinz
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
| `http.stream(url)` | `FallibleIterable<Response>` |
| Paginated API clients | `FallibleIterable<T>` |

**Compiler behavior at for-loop sites:**

The for loop syntax is identical for both contracts. The compiler checks the iterator's type:
- If it follows `Iterable<T>`, the loop runs anywhere.
- If it follows `FallibleIterable<T>`, the loop requires an `errors` context (or an explicit adapter). The compiler auto-propagates per-iteration errors using the same flow-sensitive narrowing rule as direct `errors` function calls (see `design/errors.md`).

**Stdlib adapters for ergonomic fallible-to-infallible conversion:**

```yinz
// Skip failed iterations silently (logs the error, continues to next item)
let lines = file.lines(path).orSkipFailures()        // Iterable<string>

// Get each step's success-or-failure as a value
let results = file.lines(path).withErrors()          // Iterable<Result<string>>
```

These let users iterate fallible sources from non-errors functions when they have a specific recovery strategy.

**Why two contracts instead of one with optional `errors`:**

Yinz's `follows` contracts require method signatures to match exactly. There's no language-level mechanism for "this method returns `maybe T` OR `maybe T errors`" — adding one would complicate every `follows` contract for one feature. Two explicit contracts is simpler and matches the user's mental model: "is this iterator infallible or fallible?"

**Why not Option A (single contract that always returns `errors`):**

Forces every for loop (even `for (x in [1,2,3])`) into an `errors` context. Pollutes pure code with annotations for a failure mode that can't actually happen on in-memory data. Rejected.

**Why not Option B (swallow errors and return `none`):**

Violates Yinz's core error principle: failures are visible and structured. A `for (line in file.lines(path))` that silently stops on an I/O error and returns a truncated result is the exact bug pattern Yinz exists to prevent. Rejected outright.

**Writing a custom iterable — the choice is clarifying:**

```yinz
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
  hidden done: bool = false
}

function next(lend self: ApiPager<T>) -> maybe T errors {
  if (self.done) { return none }
  const response = http.get(self.buildUrl())   // can fail
  self.cursor = response.nextCursor
  self.done = response.nextCursor.exists() == false
  return response.item
}
```

The implementor answers ONE clarifying question — "can my iteration step fail?" — and picks the matching contract. It's a forcing function for thinking about failure modes, not a burden.

Each standalone `next` function is found by the compiler when verifying `follows Iterable<T>` (or `FallibleIterable<T>`) — see `.claude/rules/non-oop.md` for the structural-function-signature-matching mechanism.
