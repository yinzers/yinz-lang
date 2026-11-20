---
name: "REF-unions"
description: "A union type is a value that can be one of several different types."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
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
shape Circle { radius: Number }
shape Square { side: Number }
shape Triangle { base: Number, height: Number }

shape Shape = Circle | Square | Triangle
```

`|` separates the variants. It reads as "Circle or Square or Triangle."

---

## Checking which variant you have — `is` tells the compiler which kind

```
function getArea(share shape: Shape) -> Number {
  if (shape) {
    is Circle => return math.PI * shape.radius * shape.radius
    is Square => return shape.side * shape.side
    is Triangle => return (shape.base * shape.height) / 2
  }
}
```

After `is Circle`, the compiler knows `shape` is a `Circle`. Use `.radius` directly — no conversion and no `.value`.

---

## `is` checks the exact type in a union

In a union, `is` matches only the exact type the value has. Sharing fields with `extends` doesn't change this:

```
shape User { name: String, email: String }
shape Admin extends User { permissions: Fixed<String> }

shape AnyUser = Admin | User

function describe(share user: AnyUser) -> String {
  if (user) {
    is Admin => return `Admin: ` + user.name
    is User => return `User: ` + user.name
  }
}
```

`Admin` does NOT match `is User` in a union, even though `Admin extends User`. Each variant is always separate, so you can predict which line runs.

---

## Outside a union, an `extends` shape can be used as its parent

```
function greet(share user: User) -> String {
  return `Hello ` + user.name
}

let admin: Admin = { name: `Alice`, email: `a@b.com`, permissions: [`all`] }
greet(admin)    // fine — Admin extends User, so Admin IS a User here
```

The rule: `is` in a union matches the exact type only. Outside a union, a shape that extends another can be used wherever its parent is expected.

---

## Unions of shapes that extend each other

```
shape User { name: String, email: String }
shape Admin extends User { permissions: Fixed<String>, role: String }
shape Guest extends User { expiresAt: Number }

shape AnyUser = Admin | Guest | User

function getAccess(share user: AnyUser) -> String {
  if (user) {
    is Admin => {
      let firstPermission = user.permissions.first()   // a maybe: the permission, or nothing
      return firstPermission.or(`none`)
    }
    is Guest => return `read-only`
    is User => return `basic`
  }
}
```

---

## A union is not one of its variants

A `Circle` can go where a `Shape` is expected. A `Shape` cannot go where a `Circle` is expected: it might hold a `Square`, so the compiler refuses it until an `is` arm has checked.

```
function describeCircle(share circle: Circle) -> String {
  return `a circle`
}

function describe(share figure: Shape) -> String {
  return describeCircle(figure)
}

// COMPILE ERROR: `figure` is a `Circle | Square | Triangle` — it might hold any one of those — but `describeCircle` expects a `Circle` here.
// Check which variant it holds first, then use it inside that arm: if (figure) { is Circle => { … } }
```

Inside `is Circle =>`, `figure` is a `Circle`, so `describeCircle(figure)` compiles there. This applies everywhere a value must have one exact type: arguments, `return`, annotated `let`, and shape fields.

The suggestion changes with what can be checked. An `is` arm checks a name and names a shape variant, so:

- A value that is not a name (`holder.inner`, `pick()`): the error asks you to bind it first, `let checked = holder.inner`, then check `checked`.
- A variant that is not a shape (`Int`, `String`), or a union with no shape variant (`Map<String, String> | Nothing`): no `is` arm can name the variant, so the error suggests making the receiving function accept the union, or keeping the value as the variant from where it is created.

---

## Every variant must be handled

When you use a multi-case `if` on a union, you must cover every variant, or add an `else =>` line for the rest:

```
function getArea(share shape: Shape) -> Number {
  if (shape) {
    is Circle   => return math.PI * shape.radius * shape.radius
    is Square   => return shape.side * shape.side
    is Triangle => return (shape.base * shape.height) / 2
  }
}
```

Leaving out a variant is a compile error. The compiler tells you which ones are missing:

```
// COMPILE ERROR: Non-exhaustive union multi-case — 1 variant is not handled: Triangle.
// Add the missing arms (e.g. `is Triangle =>`) or add an `else =>` default arm.
```

A variant that is not a shape (`Int`, `String`, …) counts too: `is` arms name shape variants only, so for `Circle | Int` the missing `Int` is handled with `else =>`. `Nothing` is the one exemption: in `T | Nothing` a match with only the arms for `T` is complete, because no arm can name `Nothing` and skipping the absent case drops no data.

---

## `Maybe<T>` is a union

`Maybe<String>` is the same type as `String | none`. Every `Maybe` type is a union. See [Maybe Types](REF-maybe.md).

---

## Write unions with `|`

```
shape Shape = Circle | Square | Triangle
```

Use the `|` symbol between variants. Words such as `or` are not accepted.
