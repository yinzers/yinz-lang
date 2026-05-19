# Iterables

`for` loops work on any type that follows the `Iterable` contract. Arrays, fixed arrays, and maps all work by default — you never need to think about this for everyday code.

---

## How for loops work under the hood

```ynz
for (player in players) {
  print(player.name)
}
```

The compiler calls `next(players)` repeatedly until it returns `none`. (Equivalent to `players.next()` via UFCS — both forms work the same.) That's the entire `Iterable` contract.

---

## Two contracts — in-memory and fallible

Most iterables can never fail mid-step. Some iterables (reading a file, paging through an API) can — disk errors, network timeouts, etc. Yinz has a contract for each. Contracts use bare-signature form (no `function` keyword, no body — see `.claude/rules/non-oop.md`):

```ynz
shape Iterable<T> {
  next(lend self) -> maybe T
}

shape FallibleIterable<T> {
  next(lend self) -> maybe T errors
}
```

Return the next value or `none` when there are no more. For `FallibleIterable`, `next` can also fail.

You almost never think about which contract is which — the compiler picks based on the iterator you're using:

| You're iterating over | Contract |
|-----------------------|----------|
| `array<T>`, `fixed<T>` | `Iterable<T>` |
| `map<K, V>.entries()` | `Iterable<Entry<K, V>>` |
| `range(start, end)` | `Iterable<int>` |
| `file.lines(path)` | `FallibleIterable<string>` |
| `http.stream(url)` | `FallibleIterable<Response>` |
| Paginated API clients | `FallibleIterable<T>` |

The `for` syntax is the same either way. What changes is that a fallible loop requires your function to be marked `errors` (or you handle the failures explicitly).

---

## Range — numbers without creating an array

`range()` accepts one or two arguments:

```ynz
// Two-argument form: start (inclusive) to end (exclusive)
for (num in range(1, 10)) {
  print(num)    // 1, 2, 3, ..., 9
}

// One-argument form: 0 (inclusive) to end (exclusive)
for (num in range(5)) {
  print(num)    // 0, 1, 2, 3, 4
}

// Large range — almost zero memory
for (num in range(1, 1000000)) {
  print(num)
}
```

`range()` is built into the standard library. Each number is generated on demand — no array created. Under the hood it's a `Range` type that follows `Iterable<int>`.

---

## File lines — read huge files without loading them

```
// Process a 50GB log file with only one line in memory at a time
function findErrors(path: string) -> array<string> errors {
  let errors: array<string> = []
  for (line in file.lines(path)) {     // each read can fail — auto-propagates
    if (line.contains("ERROR")) {
      errors.add(line)
    }
  }
  return errors
}
```

`file.lines()` returns a `FallibleIterable<string>` — it reads one line at a time, and any read can fail (disk errors, mid-read closure, etc.). Because the enclosing function is `errors`, failures auto-propagate just like a direct `errors` function call.

If you want to keep reading past failures (for example, log them and continue), use one of the adapters:

```ynz
// Skip lines that failed to read — silently discards failures, continues to next
for (line in file.lines(path).orSkipFailures()) {
  if (line.contains(`ERROR`)) { print(line) }
}

// Skip and log each failure to stderr — compose .logSkippedFailuresTo() with .orSkipFailures()
for (line in file.lines(path).logSkippedFailuresTo(terminal.stderr).orSkipFailures()) {
  if (line.contains(`ERROR`)) { print(line) }
}

// Get each step as a success-or-failure value, decide per line
for (result in file.lines(path).withErrors()) {
  if (result.failed()) {
    log.warn(`bad line: ${result.message}`)
  } else {
    if (result.value.exists()) {
      print(result.value.value)
    }
  }
}
```

With any adapter, the enclosing function no longer needs to be `errors`.

**`.orSkipFailures()` is silent** — it drops failed iterations without any I/O. If you want logging, chain `.logSkippedFailuresTo(sink)` before it. `terminal.stderr` and `terminal.stdout` are built-in log sinks. You can also pass any value that follows the `LogSink` contract (has a `.write(message)` method).

---

## Building a custom iterable

Declare a shape that `follows Iterable<T>` (data fields + `hidden` state). Provide a standalone `next` function whose signature matches the contract — the compiler verifies the match.

```ynz
shape CountDown follows Iterable<int> {
  end: int
  hidden current: int = 0
}

function next(lend self: CountDown) -> maybe int {
  if (self.current > self.end) {
    return none
  }
  const value = self.end - self.current
  self.current = self.current + 1
  return value
}

const counter: CountDown = { end: 5 }
for (num in counter) {
  print(num)    // 5, 4, 3, 2, 1, 0
}
```

If your iteration step can fail (I/O, network), follow the fallible contract instead:

```ynz
shape ApiPager<T> follows FallibleIterable<T> {
  baseUrl: string
  hidden cursor: maybe string = none
  hidden done: bool = false
}

function next(lend self: ApiPager<T>) -> maybe T errors {
  if (self.done) {
    return none
  }
  const response = http.get(self.buildUrl())   // can fail — errors propagates
  self.cursor = response.nextCursor
  self.done = response.nextCursor.exists() == false
  return response.item
}
```

The choice — `Iterable<T>` vs `FallibleIterable<T>` — comes down to one question: can a single step fail at runtime? If yes, use `FallibleIterable<T>`. If no, use `Iterable<T>`. The compiler will catch you trying to do I/O inside an infallible `next`.

Note: Yinz is not object-oriented — `next` is a standalone function, not a method "on" the iterator. The for-loop machinery calls `next(iterator)` (or equivalently `iterator.next()` via UFCS dot-call sugar).

---

## Who needs custom iterables

Most developers never build one. Standard iterables cover all common cases:

- `array<T>` and `fixed<T>` — iterate elements
- `map<K, V>` — iterate entries (each with `.key` and `.value`)
- `range(start, end)` — number sequences
- `file.lines(path)` — file lines without loading into memory

Custom iterables are for library authors and advanced patterns like lazy generation or streaming data.
