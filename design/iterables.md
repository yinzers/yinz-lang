# Iterables — Design Decisions

User spec: `spec/iterables.md`

---

## `follows Iterable[T]` with `next()` — Consistent with the Type System

Custom iteration uses the existing `follows` contract system. No special iterator syntax, no magic methods, no separate iterator trait.

**Why**: Consistent with operator overloading (which also uses `follows` contracts) and the `follows` system throughout the language. One mechanism — contracts — handles all cases where you want to customize language behavior for a type. A developer who knows `follows` already knows how to make a type iterable.

---

## `next(lend self) -> maybe T` — Ownership and Maybe

`next()` takes `lend self` (it modifies the iterator's internal state — current position, buffer, etc.) and returns `maybe T` (`none` signals end-of-sequence).

**Why `maybe T` over a separate sentinel**: `maybe T` is the language's universal way to express "this might not exist." Using it for end-of-sequence is consistent. `none` = no more items = natural English.

**Why `lend self`**: Iterators are stateful by nature — they track a current position. `lend self` is the correct ownership annotation for a method that modifies the instance.

---

## Hidden Fields for Iterator State

Iterator state (current position, buffers, page numbers) is tracked with `hidden` fields with defaults. This is what `hidden` was designed for — implementation details that the caller shouldn't see or set.

```
type Range follows Iterable[int] {
  start: int
  end: int
  hidden current: int = 0    // iterator state — caller never sets this
  ...
}
```

---

## Open Question — `Iterable` with Errors

Some iterators do I/O (`file.lines()`, `PaginatedResults`). The current contract `next(lend self) -> maybe T` has no error path. If `next()` needs to do fallible I/O, the options are:

- **Option A**: `Iterable` contract becomes `next(lend self) -> maybe T errors`, making for loops over fallible iterables require an `errors` context
- **Option B**: Fallible iterators return `maybe T` and swallow errors internally (logging them but not propagating)
- **Option C**: Two separate contracts — `Iterable[T]` for infallible and `FallibleIterable[T]` for fallible

The `PaginatedResults` example in the spec does `wait http.get(...)` inside `next()` without marking `next()` as `errors`. This is an inconsistency that needs resolution. See `design/open-questions.md`.
