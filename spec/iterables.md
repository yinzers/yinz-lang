# Iterables

`for` loops work on any type that follows the `Iterable` contract. Arrays, fixed arrays, and maps all work by default — you never need to think about this for everyday code.

---

## How for loops work under the hood

```
for (player in players) {
  print(player.name)
}
```

The compiler calls `.next()` on `players` repeatedly until it returns `none`. That's the entire `Iterable` contract.

---

## Two contracts — infallible and fallible

Most iterables can never fail mid-step. Some iterables (reading a file, paging through an API) can — disk errors, network timeouts, etc. Yinz has a contract for each:

```
type Iterable[T] {
  function next(lend self) -> maybe T
}

type FallibleIterable[T] {
  function next(lend self) -> maybe T errors
}
```

Return the next value or `none` when there are no more. For `FallibleIterable`, `next()` can also fail.

You almost never think about which contract is which — the compiler picks based on the iterator you're using:

| You're iterating over | Contract |
|-----------------------|----------|
| `array[T]`, `fixed[T]` | `Iterable[T]` |
| `map[K, V].entries()` | `Iterable[Entry[K, V]]` |
| `range(start, end)` | `Iterable[int]` |
| `file.lines(path)` | `FallibleIterable[string]` |
| `http.stream(url)` | `FallibleIterable[Response]` |
| Paginated API clients | `FallibleIterable[T]` |

The `for` syntax is the same either way. What changes is that a fallible loop requires your function to be marked `errors` (or you handle the failures explicitly).

---

## Range — numbers without creating an array

```
// Iterate 1 to 1,000,000 with almost zero memory
for (num in range(1, 1000000)) {
  print(num)
}
```

`range()` is built into the standard library. Each number is generated on demand — no million-element array created. Under the hood it's a `Range` type that follows `Iterable[int]`.

---

## File lines — read huge files without loading them

```
// Process a 50GB log file with only one line in memory at a time
function findErrors(path: string) -> array[string] errors {
  let errors: array[string] = []
  for (line in file.lines(path)) {     // each read can fail — auto-propagates
    if (line.contains("ERROR")) {
      errors.add(line)
    }
  }
  return errors
}
```

`file.lines()` returns a `FallibleIterable[string]` — it reads one line at a time, and any read can fail (disk errors, mid-read closure, etc.). Because the enclosing function is `errors`, failures auto-propagate just like a direct `errors` function call.

If you want to keep reading past failures (for example, log them and continue), use one of the adapters:

```
// Skip lines that failed to read — silently logs and continues
for (line in file.lines(path).orSkipFailures()) {
  if (line.contains("ERROR")) { print(line) }
}

// Get each step as a success-or-failure value, decide per line
for (result in file.lines(path).withErrors()) {
  if (result.failed()) {
    log.warn(`bad line: ${result.message}`)
  } else {
    print(result.value)
  }
}
```

With either adapter, the enclosing function no longer needs to be `errors`.

---

## Building a custom iterable

Implement `follows Iterable[T]` with a `next(lend self) -> maybe T` method. Use `hidden` fields with defaults to track internal state:

```
type CountDown follows Iterable[int] {
  end: int
  hidden current: int = 0

  function next(lend self) -> maybe int {
    if (self.current > self.end) {
      return none
    }
    let value = self.end - self.current
    self.current = self.current + 1
    return value
  }
}

let counter: CountDown = { end: 5 }
for (num in counter) {
  print(num)    // 5, 4, 3, 2, 1, 0
}
```

If your iteration step can fail (I/O, network), follow the fallible contract instead:

```
type ApiPager[T] follows FallibleIterable[T] {
  baseUrl: string
  hidden cursor: maybe string = none
  hidden done: bool = false

  function next(lend self) -> maybe T errors {
    if (self.done) {
      return none
    }
    let response = http.get(self.buildUrl())   // can fail — errors propagates
    self.cursor = response.nextCursor
    self.done = response.nextCursor.exists() == false
    return response.item
  }
}
```

The choice — `Iterable` vs `FallibleIterable` — comes down to one question: can a single step fail at runtime? If yes, use `FallibleIterable[T]`. If no, use `Iterable[T]`. The compiler will catch you trying to do I/O inside an infallible `next()`.

---

## Who needs custom iterables

Most developers never build one. Standard iterables cover all common cases:

- `array[T]` and `fixed[T]` — iterate elements
- `map[K, V]` — iterate entries (each with `.key` and `.value`)
- `range(start, end)` — number sequences
- `file.lines(path)` — file lines without loading into memory

Custom iterables are for library authors and advanced patterns like lazy generation or streaming data.
