---
name: "REF-iterables"
description: "for loops work on arrays, fixed arrays, maps, ranges, strings and any shape with a next function."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Iterables

`for` loops work on arrays, fixed arrays, maps, ranges, strings and any shape that has a `next` function. For everyday code you never need to think about how.

---

## How for loops work

```
let names: Array<String> = [`Alice`, `Bob`]

for (name in names) {
  print(name)
}
```

The loop asks the collection for its next value again and again. It stops when there are no more.

You can loop over:

| You're looping over | Each pass gives you |
|---------------------|---------------------|
| `Array<T>`, `Fixed<T>` | the next item |
| `Map<K, V>` | a key and a value: `for ((key, value) in scores)` |
| `range(start, end)` | the next `Int` |
| a `String` | the next character |
| a shape with a `next` function | whatever `next` returns (see below) |

---

## Pulling fields out of shapes in a for loop

When you loop over a collection of shapes, you can pull the fields out right in the loop header instead of reading them from the loop variable:

```
shape Player {
  name: String
  health: Int
  score: Int
}

function entrypoint() -> Nothing {
  let players: Fixed<Player> = [
    { name: `Alice`, health: 100, score: 50 },
    { name: `Bob`, health: 80, score: 30 }
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
}
```

Both forms do the same thing. Pulling fields out is shorthand for reading `player.name` and `player.health` yourself.

You can rename a field with `as`:

```
for ({ name as playerName, health as hp } in players) {
  print(playerName)
  print(hp)
}
```

You don't have to pull out every field — list only the ones you need:

```
for ({ name } in players) {
  print(name)   // health and score exist, but you didn't ask for them
}
```

See [Destructuring](REF-destructuring.md).

---

## Range — numbers without creating an array

`range()` accepts one or two arguments:

```
// Two-argument form: start (inclusive) to end (exclusive)
for (num in range(1, 10)) {
  print(num)    // 1, 2, 3, ..., 9
}

// One-argument form: 0 (inclusive) to end (exclusive)
for (num in range(5)) {
  print(num)    // 0, 1, 2, 3, 4
}
```

`range()` is built in. Both arguments must be `Int`. It makes each number only when the loop asks for it, so it never builds an array.

---

## Building your own iterable

Declare a shape, with its data fields and any `hidden` fields it needs to remember where it is. Then write a standalone `next` function that takes the shape as `lend self` and returns `Maybe<T>`. The loop calls `next(counter)` until it returns `none`.

```
shape CountDown {
  end: Int
  hidden current: Int = 0
}

function next(lend self: CountDown) -> Maybe<Int> {
  if (self.current >= self.end) {
    return none
  }
  let value = self.end - self.current
  self.current = self.current + 1
  return value
}

function entrypoint() -> Nothing {
  let counter: CountDown = { end: 5 }
  for (num in counter) {
    print(num)    // 5, 4, 3, 2, 1
  }
}
```

A shape with no `next` function cannot be looped over:

```
shape Roster {
  title: String
}

function entrypoint() -> Nothing {
  let roster: Roster = { title: `Pirates` }
  for (item in roster) {
    print(item)
  }
}
// COMPILE ERROR: `Roster` cannot be iterated — it does not follow `Iterable<T>`.
// Add `function next(lend self: Roster) -> Maybe<T>` to make it iterable.
// For a `for` loop to work on a custom shape, the shape needs a standalone `next` function returning `Maybe<T>`. When the iterator is done, return `none`.
```

---

## Who needs custom iterables

Most developers never build one. The built-in iterables cover everyday code:

- `Array<T>` and `Fixed<T>` — go through the items
- `Map<K, V>` — go through the keys and values
- `range(start, end)` — a sequence of numbers

Custom iterables are for library authors and for cases like making values on demand.
