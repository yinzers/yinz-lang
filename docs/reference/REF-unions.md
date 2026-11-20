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
shape Circle { radius: number }
shape Square { side: number }
shape Triangle { base: number, height: number }

shape Shape = Circle | Square | Triangle
```

`|` separates the variants. It reads as "Circle or Square or Triangle."

---

## Checking which variant you have — `is` tells the compiler which kind

```
function getArea(share shape: Shape) -> number {
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

`Admin` does NOT match `is User` in a union, even though `Admin extends User`. Each variant is always separate, so you can predict which line runs.

---

## Outside a union, an `extends` shape can be used as its parent

```
function greet(share user: User) -> string {
  return `Hello ` + user.name
}

let admin: Admin = { name: `Alice`, email: `a@b.com`, permissions: [`all`] }
greet(admin)    // fine — Admin extends User, so Admin IS a User here
```

The rule: `is` in a union matches the exact type only. Outside a union, a shape that extends another can be used wherever its parent is expected.

---

## Unions of shapes that extend each other

```
shape User { name: string, email: string }
shape Admin extends User { permissions: fixed<string>, role: string }
shape Guest extends User { expiresAt: number }

shape AnyUser = Admin | Guest | User

function getAccess(share user: AnyUser) -> string {
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

## Every variant must be handled

When you use a multi-case `if` on a union, you must cover every variant, or add an `else =>` line for the rest:

```
function getArea(share shape: Shape) -> number {
  if (shape) {
    is Circle   => return math.PI * shape.radius * shape.radius
    is Square   => return shape.side * shape.side
    is Triangle => return (shape.base * shape.height) / 2
  }
}
```

Leaving out a variant is a compile error. The compiler tells you which ones are missing:

```
// COMPILE ERROR: Non-exhaustive union multi-case — Shape has 3 variants; only 2 are handled.
// Missing variant: Triangle
// Add: is Triangle => ...  or add: else => ...
```

---

## `maybe<T>` is a union

`maybe<string>` is the same type as `string | none`. Every `maybe` type is a union. See [Maybe Types](REF-maybe.md).

---

## Write unions with `|`

```
shape Shape = Circle | Square | Triangle
```

Use the `|` symbol between variants. Words such as `or` are not accepted.
