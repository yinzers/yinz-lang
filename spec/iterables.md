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

## The contract

```
type Iterable[T] {
  function next(lend self) -> maybe T
}
```

Return the next value or `none` when there are no more.

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
for (line in file.lines("massive-log.txt")) {
  if (line.contains("ERROR")) {
    print(line)
  }
}
```

`file.lines()` returns an iterable that reads one line at a time. The file is never fully loaded.

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

---

## Who needs custom iterables

Most developers never build one. Standard iterables cover all common cases:

- `array[T]` and `fixed[T]` — iterate elements
- `map[K, V]` — iterate entries (each with `.key` and `.value`)
- `range(start, end)` — number sequences
- `file.lines(path)` — file lines without loading into memory

Custom iterables are for library authors and advanced patterns like lazy generation or streaming data.
