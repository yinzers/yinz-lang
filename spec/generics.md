# Generics

Sometimes you want a type that works with any type. That's what generics are for.

---

## You already know the syntax

You've been using generics this whole time:

```
array[Player]           // an array that holds Players
map[string, number]     // a map with string keys and number values
fixed[string]           // a fixed array of strings
```

The `[T]` part is the generic parameter — it says what type the collection holds.

---

## Defining your own generic types

Use the same `name[T]` pattern:

```
type Box[T] {
  value: T
}

let playerBox: Box[Player] = { value: somePlayer }
let numberBox: Box[number] = { value: 42 }
```

`Box[Player]` is a Box that holds a `Player`. `Box[number]` holds a `number`. One definition, works with any type.

---

## Multiple type parameters

```
type Pair[A, B] {
  first: A
  second: B
}

let coord: Pair[number, number] = { first: 10, second: 20 }
let entry: Pair[string, number] = { first: "score", second: 100 }
```

---

## Consistent with built-in collections

The pattern is the same everywhere — your types and the built-in types work the same way:

```
array[Player]           // built-in — holds Players
Box[Player]             // yours — holds a Player
Pair[string, number]    // yours — holds a string and a number
map[string, number]     // built-in — maps strings to numbers
```

One pattern. No special cases.

---

## Generic functions

The spec for generic functions (functions that accept any type as a parameter) is coming in a future update. See `/design/open-questions.md`.
