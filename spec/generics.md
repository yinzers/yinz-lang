# Generics

Sometimes you want a type that works with any type. That's what generics are for.

---

## You already know the syntax

You've been using generics this whole time:

```
array<Player>           // an array that holds Players
map<string, number>     // a map with string keys and number values
fixed<string>           // a fixed array of strings
```

The `<T>` part is the generic parameter — it says what type the collection holds.

---

## Defining your own generic types

Use the same `name<T>` pattern:

```
shape Box<T> {
  value: T
}

let playerBox: Box<Player> = { value: somePlayer }
let numberBox: Box<number> = { value: 42 }
```

`Box<Player>` is a Box that holds a `Player`. `Box<number>` holds a `number`. One definition, works with any type.

---

## Multiple type parameters

```
shape Pair<A, B> {
  first: A
  second: B
}

let coord: Pair<number, number> = { first: 10, second: 20 }
let entry: Pair<string, number> = { first: "score", second: 100 }
```

---

## Consistent with built-in collections

The pattern is the same everywhere — your types and the built-in types work the same way:

```
array<Player>           // built-in — holds Players
Box<Player>             // yours — holds a Player
Pair<string, number>    // yours — holds a string and a number
map<string, number>     // built-in — maps strings to numbers
```

One pattern. No special cases.

---

## Generic functions

Functions can have type parameters too — same `name<T>` syntax, after the function name:

```
function identity<T>(give value: T) -> T {
  return value
}

let n = identity(5)            // T inferred as int
let s = identity("hello")      // T inferred as string
```

The compiler picks `T` from the argument you pass. You almost never write it explicitly.

---

## Multiple type parameters on a function

```
function pair<A, B>(give first: A, give second: B) -> Pair<A, B> {
  return { first, second }
}

let coord = pair(10, 20)            // A=int, B=int — Pair<int, int>
let entry = pair("score", 100)      // A=string, B=int — Pair<string, int>
```

---

## Constraints — require the type to follow a contract

If your function needs the type to support certain operations, use `follows` right inside the angle brackets:

```
function sort<T follows Comparable>(share items: array<T>) -> array<T> {
  // T is guaranteed to follow Comparable, so .compare() works
  // ...
}

let sorted = sort(players)         // works because Player follows Comparable
```

Multiple constraints — separate with commas:

```
function process<T follows Comparable, Serializable>(share item: T) -> string {
  // T follows both Comparable AND Serializable
}
```

If you try to call the function with a type that doesn't follow the contract, the compiler tells you exactly what's missing:

```
shape Player { name: string, health: number }   // no follows clause

let sorted = sort(players)
// COMPILE ERROR: Type Player does not follow contract Comparable.
//
//   sort<T follows Comparable> requires T to follow Comparable, but Player
//   does not implement it. To make Player sortable, add a follows clause:
//
//     type Player follows Comparable { ... }
//
//   Then implement the required compare() method. See spec/operators.md.
```

---

## Explicit type parameters — only when needed

In rare cases the compiler can't infer `T` (usually when no arguments use the type). Specify it manually with angle brackets at the call site:

```
function createList<T>() -> array<T> {
  return []
}

let empty = createList<Player>()       // explicit — there's nothing to infer from
```

99% of calls don't need this. If you find yourself writing `<T>` at a call site often, the function signature probably has a design issue.
