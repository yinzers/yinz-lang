---
name: "REF-unions"
description: "A union type is a value that can be one of several different types."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Union Types

A union type is a value that can be one of several different types. Each type in the union is called a *variant*.

---

## Defining a union

```
shape Circle {
  radius: Float
}

shape Square {
  side: Float
}

shape Triangle {
  baseLength: Float
  height: Float
}

shape Figure = Circle | Square | Triangle
```

`|` separates the variants. It reads as "Circle or Square or Triangle."

Put a shape value into a union with a `let` that names the union:

```
let circle: Circle = { radius: 2.0 }
let figure: Figure = circle
```

---

## Checking which variant you have — `is` tells the compiler which kind

```
function getArea(share figure: Figure) -> Float {
  if (figure) {
    is Circle => {
      return figure.radius * figure.radius * 3.14159
    }
    is Square => {
      return figure.side * figure.side
    }
    is Triangle => {
      return (figure.baseLength * figure.height) / 2.0
    }
  }
  return 0.0
}
```

After `is Circle`, the compiler knows `figure` is a `Circle`. Use `.radius` directly — no conversion and no `.value`.

A function that returns a value needs a `return` after the `if`, even when every variant has its own arm. The compiler does not count the arms as a return on every path.

---

## `is` checks the exact type in a union

In a union, `is` matches only the exact type the value has. Sharing fields with `extends` doesn't change this:

```
shape User {
  name: String
  email: String
}

shape Admin extends User {
  permissions: Fixed<String, 1>
}

shape AnyUser = Admin | User

function describe(share user: AnyUser) -> String {
  if (user) {
    is Admin => {
      return `Admin: ${user.name}`
    }
    is User => {
      return `User: ${user.name}`
    }
  }
  return `unknown`
}
```

`Admin` does NOT match `is User` in a union, even though `Admin extends User`. Each variant is always separate, so you can predict which line runs.

---

## Unions of shapes that extend each other

```
shape User {
  name: String
  email: String
}

shape Admin extends User {
  permissions: Fixed<String, 1>
  role: String
}

shape Guest extends User {
  expiresAt: Float
}

shape AnyUser = Admin | Guest | User

function getAccess(share user: AnyUser) -> String {
  if (user) {
    is Admin => {
      let firstPermission = user.permissions.first()   // a maybe: the permission, or nothing
      return firstPermission.or(`none`)
    }
    is Guest => {
      return `read-only`
    }
    is User => {
      return `basic`
    }
  }
  return `unknown`
}
```

---

## A union is not one of its variants

A `Circle` can go where a `Figure` is expected. A `Figure` cannot go where a `Circle` is expected: it might hold a `Square`, so the compiler refuses it until an `is` arm has checked.

```
function describeCircle(share circle: Circle) -> String {
  return `a circle`
}

function describeFigure(share figure: Figure) -> String {
  return describeCircle(figure)
}

// COMPILE ERROR: `figure` is a `Circle | Square | Triangle` — it might hold any one of those — but `describeCircle` expects a `Circle` here.
// Check which variant it holds first, then use it inside that arm: `if (figure) { is Circle => { … } }` — inside the arm, `figure` is a `Circle`. Handle the other variants in their own `is` arms, or in an `else =>` arm.
```

Inside `is Circle =>`, `figure` is a `Circle`, so `describeCircle(figure)` compiles there. This applies everywhere a value must have one exact type: arguments, `return`, annotated `let`, and shape fields.

The suggestion changes with what can be checked. An `is` arm checks a name and names a shape variant, so:

- A value that is not a name (`holder.inner`, `pick()`): the error asks you to bind it first, `let checked = holder.inner`, then check `checked`.
- A variant that is not a shape (`Int`, `String`), or a union with no shape variant (`Map<String, String> | Nothing`): no `is` arm can name the variant, so the error suggests making the receiving function accept the union, or keeping the value as the variant from where it is created.

---

## Every variant must be handled

When you use a multi-case `if` on a union, you must cover every variant, or add an `else =>` line for the rest:

```
function sidesOf(share figure: Figure) -> Int {
  if (figure) {
    is Circle => {
      return 0
    }
    is Square => {
      return 4
    }
  }
  return -1
}
// COMPILE ERROR: Non-exhaustive union multi-case — 1 variant is not handled: Triangle.
// Add the missing arms (e.g. `is Triangle =>`) or add an `else =>` default arm.
// The compiler knows every union variant at compile time. A missing arm means some values silently fall through — likely a bug.
```

A variant that is not a shape (`Int`, `String`, …) counts too: `is` arms name shape variants only, so for `Circle | Int` the missing `Int` is handled with `else =>`. `Nothing` is the one exemption: in `T | Nothing` a match with only the arms for `T` is complete, because no arm can name `Nothing` and skipping the absent case drops no data.

---

## Write unions with `|`

```
shape Figure = Circle | Square | Triangle
```

Use the `|` symbol between variants.
