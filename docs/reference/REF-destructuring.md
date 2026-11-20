---
name: "REF-destructuring"
description: "Pull fields out of each value in a `for` loop header into named variables."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---


# Destructuring

Pull fields out of each value in a `for` loop header into named variables. Destructuring is written in the loop header only.

---

## Shape fields in a `for` loop

Write the fields you want inside `{ }`. Each pass of the loop binds one variable per field:

```ynz
shape Player {
  name: String
  health: Int
  score: Int
}

function entrypoint() -> Nothing {
  let roster: Array<Player> = [
    { name: `Alice`, health: 100, score: 50 },
    { name: `Bob`, health: 80, score: 70 }
  ]

  for ({ name, health } in roster) {
    print(name)                  // Alice, then Bob
    print(health.toString())     // 100, then 80
  }
}
```

Output:

```text
Alice
100
Bob
80
```

The compiler knows which shape the loop holds before the program runs, so every field is guaranteed to exist. Nothing can go wrong while the program runs.

---

## Rename while destructuring — `as`

```ynz
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let roster: Array<Player> = [
    { name: `Alice`, health: 100 },
    { name: `Bob`, health: 80 }
  ]

  for ({ name, health as hp } in roster) {
    print(hp.toString())    // same value as health, different variable name
  }
}
```

Output:

```text
100
80
```

Use `as` when the field name would conflict with an existing variable or when a shorter name reads better.

---

## Key and value in a map loop

A map loop names the key and the value in a pair of parentheses:

```ynz
function entrypoint() -> Nothing {
  let scores: Map<String, Int> = { `alice`: 90, `bob`: 85 }

  for ((name, score) in scores) {
    print(`${name}=${score.toString()}`)
  }
}
```

Output:

```text
alice=90
bob=85
```

---

## No array destructuring

The compiler can't know before the program runs whether an array has an item at a given position. Destructuring would skip the safe `.get()` check. Loop with a single name and read the fields directly, or use `.get()`:

```ynz
function entrypoint() -> Nothing {
  let pairs: Array<Array<Int>> = [[1, 2], [3, 4]]
  for ([first, second] in pairs) {
    print(first)
  }
}
```

The compiler stops with:

```text
Error: Array destructuring `[a, b]` isn't supported in `for` loop headers.
   ╭─[ main.ynz:3:8 ]
   │
 3 │   for ([first, second] in pairs) {
   │        ┬
   │        ╰── Loop with a single binding and access fields directly:
  for (config in intervals) {
      config.minutes
      config.timeframe
  }
   │
   │ Note: Array indices aren't compile-time verified — `[a, b]` could silently miss elements after a refactor. Named shape fields are statically guaranteed. The shape iterator compiles to the same pointer-arithmetic loop a tuple array would.
───╯
```

Safe access with `.get()` returns a `Maybe`:

```ynz
function entrypoint() -> Nothing {
  let items: Array<Int> = [4, 8]
  let foundFirst = items.get(0)    // a Maybe: the item, or nothing
  let first = foundFirst.or(0)     // a plain Int, 0 if missing
  print(first)
}
```

Output:

```text
4
```

A shape's fields always exist. An array position might not. That is why shapes can be destructured and arrays can't.
