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
    is Admin => return "Admin: " + user.name
    is User => return "User: " + user.name
  }
}
```

`Admin` does NOT match `is User` in a union, even though `Admin extends User`. This makes union checking predictable — each variant is always distinct.

---

## Outside a union, inheritance works normally

```
function greet(share user: User) -> string {
  return "Hello " + user.name
}

let admin: Admin = { name: "Alice", email: "a@b.com", permissions: ["all"] }
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
    is Admin => return user.permissions.first().or("none")
    is Guest => return "read-only"
    is User => return "basic"
  }
}
```

---

## `maybe T` is a union

`maybe string` is the same type as `string | none`. Every `maybe` type is a union. See [Maybe Types](maybe.md).

---

## `|` not words

```
shape Shape = Circle | Square | Triangle    // correct
shape Shape = Circle | Square | Triangle  // COMPILE ERROR: use | for union types
```
