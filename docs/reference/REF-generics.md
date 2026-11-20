---
name: "REF-generics"
description: "Sometimes you want a shape or function that works with any type. That's what generics are for."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Generics

Sometimes you want a shape or function that works with any type. That's what generics are for.

---

## You already know the syntax

You've been using generics this whole time:

```
Array<Player>           // an array that holds Players
Map<String, Int>        // a map with string keys and Int values
Fixed<String>           // a fixed array of strings
```

The `<T>` part is the generic parameter — it says what type the collection holds.

---

## Defining your own generic shapes

Use the same `name<T>` pattern:

```ynz
shape Box<T> {
  value: T
}

function entrypoint() -> Nothing {
  let numberBox: Box<Int> = { value: 42 }
  let nameBox: Box<String> = { value: `Ana` }
  print(numberBox.value.toString())
  print(nameBox.value)
}
```

`Box<Int>` is a Box that holds an `Int`. `Box<String>` holds a `String`. One definition works with any type.

---

## Multiple type parameters

```ynz
shape Pair<A, B> {
  first: A
  second: B
}

function entrypoint() -> Nothing {
  let coord: Pair<Int, Int> = { first: 10, second: 20 }
  let entry: Pair<String, Int> = { first: `score`, second: 100 }
  print(coord.first.toString())
  print(entry.first)
}
```

---

## Generic functions

Functions can have type parameters too — same `name<T>` syntax, after the function name:

```ynz
function identity<T>(give value: T) -> T {
  return value
}

function entrypoint() -> Nothing {
  let n = identity(5)            // T is Int
  let s = identity(`hello`)      // T is String
  print(n.toString())
  print(s)
}
```

The compiler works out `T` from the argument you pass. You almost never write it yourself.

---

## One `T` is one type per call

When two parameters share a type parameter, every argument for them must have that one type. The first argument that sets `T` (or the explicit `<Type>` you wrote) decides it:

```ynz
shape Tiny {
  flag: Boolean
}

shape Big {
  label: String
  count: Int
}

function same<T>(share first: T, share second: T) -> T {
  return second
}

function entrypoint() -> Nothing {
  let big: Big = { label: `a`, count: 1 }
  let big2: Big = { label: `b`, count: 2 }
  let tiny: Tiny = { flag: true }
  same(big, big2)        // OK — T is Big for both
  same(big, tiny)        // COMPILE ERROR
  same<Big>(big, tiny)   // COMPILE ERROR
}
```

The error names the argument, the type `T` was made, and what made it:

```
`tiny` has type `Tiny`, but `same` needs `Big` for `second`: `second` and `first` share the
type parameter `T`, and the argument `big` for `first` already made it `Big`.
```

The function is compiled for `Big`, so it would read the `Tiny` as a `Big`: wrong values, or memory past the end of the `Tiny`. Pass a `Big` for `second`, or give `first` a `Tiny` too.

The same comparison applies to every argument of every call form: `f(a, b)`, `a.f(b)` and generic calls. An argument must be the type its parameter declares.

---

## Multiple type parameters on a function

```ynz
shape Pair<A, B> {
  first: A
  second: B
}

function pair<A, B>(give first: A, give second: B) -> Pair<A, B> {
  return { first, second }
}

function entrypoint() -> Nothing {
  let coord = pair(10, 20)            // Pair<Int, Int>
  let entry = pair(`score`, 100)      // Pair<String, Int>
  print(coord.first.toString())
  print(entry.first)
}
```

---

## Constraints — require the type to follow a contract

If your function needs the type to support certain functions, write `follows` inside the angle brackets. A contract is a `shape` that lists function signatures:

```ynz
shape Scorable {
  score(share self) -> Int
}

shape Player follows Scorable {
  name: String
  points: Int
}

function score(share player: Player) -> Int {
  return player.points
}

function bestScore<T follows Scorable>(share first: T, share second: T) -> Int {
  return score(first)
}
```

If you call the function with a shape that does not follow the contract:

```ynz
shape Rookie {
  name: String
  points: Int
}

function entrypoint() -> Nothing {
  let first: Rookie = { name: `Ana`, points: 3 }
  let second: Rookie = { name: `Bo`, points: 5 }
  print(bestScore(first, second).toString())
}
```

the compiler reports:

```
Type `Rookie` does not follow contract `Scorable`.
```

Add `follows Scorable` to the shape's declaration and write the functions the contract lists.

---

## Explicit type parameters — only when needed

When no argument tells the compiler what `T` is, write it at the call site with angle brackets:

```ynz
function createList<T>() -> Array<T> {
  return []
}

function entrypoint() -> Nothing {
  let empty = createList<Int>()       // explicit — there's nothing to work it out from
  print(empty.count().toString())
}
```

Without it the compiler reports:

```
Cannot work out the type parameter `T` for function `createList` — pass a value or annotate explicitly.
```
