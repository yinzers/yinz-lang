---
name: "REF-iterables"
description: "for loops work on any type that follows the Iterable contract. Arrays, fixed arrays, and maps all work by default — you never need to think about this for everyday code."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Iterables

`for` loops work on any type that follows the `Iterable` contract. Arrays, fixed arrays, and maps all work by default — you never need to think about this for everyday code.

---

## How for loops work

```ynz
for (player in players) {
  print(player.name)
}
```

The loop asks `players` for its next value again and again, using the function `next(players)`. It stops when `next` returns `none`. You can also write the call as `players.next()`; both forms do the same thing. That one function is the whole `Iterable` contract.

---

## Two contracts — one that cannot fail, one that can

Most iterables never fail while giving you the next value. Some can: reading a file or paging through a web API can hit a disk error or a network timeout. Yinz has one contract for each. A contract lists only the function signatures a shape must provide, with no `function` keyword and no body:

```ynz
shape Iterable<T> {
  next(lend self) -> Maybe<T>
}

shape FallibleIterable<T> {
  next(lend self) -> Maybe<T> errors
}
```

`next` returns the next value, or `none` when there are no more. For `FallibleIterable`, `next` can also fail.

You almost never need to choose a contract. The compiler picks one from what you loop over:

| You're iterating over | Contract |
|-----------------------|----------|
| `Array<T>`, `Fixed<T>` | `Iterable<T>` |
| `Map<K, V>.entries()` | `Iterable<Entry<K, V>>` |
| `range(start, end)` | `Iterable<Int>` |
| `file.lines(path)` | `FallibleIterable<String>` |
| `request.stream(url)` | `FallibleIterable<Response>` |
| Paginated API clients | `FallibleIterable<T>` |

The `for` syntax is the same either way. The difference: a loop that can fail needs your function to be marked `errors`, or you handle the failures yourself.

---

## Pulling fields out of shapes in a for loop

When you loop over a collection of shapes, you can pull the fields out right in the loop header instead of reading them from the loop variable:

```ynz
shape Player { name: String, health: Int, score: Int }

const players: Fixed<Player> = [
    { name: `Alice`, health: 100, score: 50 },
    { name: `Bob`,   health: 80,  score: 30 },
]

// One variable — read fields from it
for (player in players) {
    print(player.name)
    print(player.health)
}

// Pull the fields out — name and health become variables
for ({ name, health } in players) {
    print(name)
    print(health)
}
```

Both forms do the same thing. Pulling fields out is shorthand for reading `player.name` and `player.health` yourself.

You can rename a field with `as`:

```ynz
for ({ name as playerName, health as hp } in players) {
    print(playerName)
    print(hp)
}
```

You don't have to pull out every field — list only the ones you need:

```ynz
for ({ name } in players) {
    print(name)   // health and score exist, but you didn't ask for them
}
```

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

`range()` is built in. It makes each number only when the loop asks for it, so it never builds an array. It follows the `Iterable<Int>` contract.

---

## File lines — read huge files without loading them

```
// Process a 50GB log file with only one line in memory at a time
function findErrors(path: String) -> Array<String> errors {
  let errorLines: Array<String> = []
  for (line in file.lines(path)) {     // each read can fail — the failure goes up automatically
    if (line.contains(`ERROR`)) {
      errorLines.add(line)
    }
  }
  return errorLines
}
```

`file.lines()` returns a `FallibleIterable<String>`. It reads one line at a time, and any read can fail (disk error, file closed mid-read, and so on). Because the function is marked `errors`, a failed read ends the function and passes the failure to the caller, just like a failed call to any other `errors` function.

If you want to keep reading past failures (for example, log them and carry on), use one of these adapters (functions that sit between the file and your loop and change what happens when a read fails):

```ynz
// Skip lines that failed to read — drops each failure and moves on
const firstPassLines = file.lines(path)
const readableLines = firstPassLines.orSkipFailures()
for (line in readableLines) {
  if (line.contains(`ERROR`)) { print(line) }
}

// Skip and log each failure to the error output — call .logSkippedFailuresTo() first, then .orSkipFailures()
const secondPassLines = file.lines(path)
const loggedLines = secondPassLines.logSkippedFailuresTo(terminal.stderr)
const survivingLines = loggedLines.orSkipFailures()
for (line in survivingLines) {
  if (line.contains(`ERROR`)) { print(line) }
}

// Get each step as a success-or-failure value, decide per line
const thirdPassLines = file.lines(path)
const lineResults = thirdPassLines.withErrors()
for (result in lineResults) {
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

**`.orSkipFailures()` is silent** — it drops failed lines and writes nothing. If you want a log, call `.logSkippedFailuresTo(sink)` first and then `.orSkipFailures()` on its result. `terminal.stderr` and `terminal.stdout` are built-in places to log to. You can also pass any value that follows the `LogSink` contract (one with a `.write(message)` function).

---

## Building your own iterable

Declare a shape that `follows Iterable<T>`, with its data fields and any `hidden` fields it needs to remember where it is. Then write a standalone `next` function whose signature matches the contract. The compiler checks the match.

```ynz
shape CountDown follows Iterable<Int> {
  end: Int
  hidden current: Int = 0
}

function next(lend self: CountDown) -> Maybe<Int> {
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

If getting the next value can fail (a file or the network), follow the `FallibleIterable` contract instead:

```ynz
shape ApiPager<T> follows FallibleIterable<T> {
  baseUrl: String
  hidden cursor: Maybe<String> = none
  hidden done: Boolean = false
}

function next(lend self: ApiPager<T>) -> Maybe<T> errors {
  if (self.done) {
    return none
  }
  const url = self.buildUrl()
  const response = request.get(url)   // can fail — the failure goes up automatically
  self.cursor = response.nextCursor
  self.done = response.nextCursor.exists() == false
  return response.item
}
```

To choose between `Iterable<T>` and `FallibleIterable<T>`, ask one question: can getting the next value fail while the program runs? If yes, use `FallibleIterable<T>`. If no, use `Iterable<T>`. The compiler reports an error if you read a file or call the network inside a `next` that is not allowed to fail.

`next` is a standalone function, not something stored inside the iterable. The `for` loop calls `next(iterator)`, which you can also write as `iterator.next()`.

---

## Who needs custom iterables

Most developers never build one. The built-in iterables cover everyday code:

- `Array<T>` and `Fixed<T>` — go through the items
- `Map<K, V>` — go through the entries (each with `.key` and `.value`)
- `range(start, end)` — a sequence of numbers
- `file.lines(path)` — the lines of a file, without loading the whole file

Custom iterables are for library authors and for cases like making values on demand or reading a stream of data.
