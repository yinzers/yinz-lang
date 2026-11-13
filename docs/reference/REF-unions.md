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

A union type is a value that can be one of several different types.

---

## Defining a union

```
shape Circle { radius: number }
shape Square { side: number }
shape Triangle { base: number, height: number }

shape Shape = Circle | Square | Triangle
```

`|` separates the variants. Same as TypeScript. Reads as "Circle | Square | Triangle."

---

## Checking which type it is — `is` tells the compiler which kind

```
function getArea(share shape: Shape) -> number {
  if (shape) {
    is Circle => return math.PI * shape.radius * shape.radius
    is Square => return shape.side * shape.side
    is Triangle => return (shape.base * shape.height) / 2
  }
}
```

After `is Circle`, the compiler knows `shape` is a `Circle`. Access `.radius` directly — no cast, no `.value`.

---

## `is` checks the exact type in a union

In a union, `is` matches the exact runtime type. Inheritance doesn't change this:

```
shape User { name: string, email: string }
shape Admin extends User { permissions: fixed<string> }

shape AnyUser = Admin | User

function describe(share user: AnyUser) -> string {
  if (user) {
    is Admin => return `Admin: ` + user.name
    is User => return `User: ` + user.name
  }
}
```

`Admin` does NOT match `is User` in a union, even though `Admin extends User`. This makes union checking predictable — each variant is always distinct.

---

## Outside a union, inheritance works normally

```
function greet(share user: User) -> string {
  return `Hello ` + user.name
}

let admin: Admin = { name: `Alice`, email: `a@b.com`, permissions: [`all`] }
greet(admin)    // fine — Admin extends User, so Admin IS a User here
```

The rule: `is` inside a union = exact type match. Inheritance outside unions = normal subtype rules.

---

## Unions with inheritance — shared variants

```
shape User { name: string, email: string }
shape Admin extends User { permissions: fixed<string>, role: string }
shape Guest extends User { expiresAt: number }

shape AnyUser = Admin | Guest | User

function getAccess(share user: AnyUser) -> string {
  if (user) {
    is Admin => return user.permissions.first().or(`none`)
    is Guest => return `read-only`
    is User => return `basic`
  }
}
```

---

## A union is not one of its variants

A `Circle` can go where a `Shape` is expected. A `Shape` cannot go where a `Circle` is expected: it might hold a `Square`, so the compiler refuses it until an `is` arm has checked.

```
function describeCircle(share circle: Circle) -> string {
  return `a circle`
}

function describe(share figure: Shape) -> string {
  return describeCircle(figure)
}

// COMPILE ERROR: `figure` is a `Circle | Square | Triangle` — it might hold any one of those — but `describeCircle` expects a `Circle` here.
// Check which variant it holds first, then use it inside that arm: if (figure) { is Circle => { … } }
```

Inside `is Circle =>`, `figure` is a `Circle`, so `describeCircle(figure)` compiles there. This applies everywhere a value must have one exact type: arguments, `return`, annotated `let`, and shape fields.

---

## Exhaustiveness — all variants must be handled

When using multi-case `if` on a union, you must cover every variant — or include `else =>`:

```
function getArea(share shape: Shape) -> number {
  if (shape) {
    is Circle   => return math.PI * shape.radius * shape.radius
    is Square   => return shape.side * shape.side
    is Triangle => return (shape.base * shape.height) / 2
  }
}
```

Missing a variant is a compile error. The compiler names which ones are missing:

```
// COMPILE ERROR: Non-exhaustive union multi-case — Shape has 3 variants; only 2 are handled.
// Missing variant: Triangle
// Add: is Triangle => ...  or add: else => ...
```

A variant that is not a shape (`int`, `string`, …) counts too: `is` arms name shape variants only, so for `Circle | int` the missing `int` is handled with `else =>`. `nothing` is the one exemption: in `T | nothing` a match with only the arms for `T` is complete, because no arm can name `nothing` and skipping the absent case drops no data.

---

## `maybe<T>` is a union

`maybe<string>` is the same type as `string | none`. Every `maybe` type is a union. See [Maybe Types](REF-maybe.md).

---

## `|` not words

```
shape Shape = Circle | Square | Triangle    // correct
shape Shape = Circle | Square | Triangle  // COMPILE ERROR: use | for union types
```
