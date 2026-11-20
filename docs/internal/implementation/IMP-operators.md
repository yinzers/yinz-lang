---
name: "IMP-operators"
description: "Design decisions for Yinz operator overloading via follows contracts (Addable, Equatable, etc.) instead of Python-style magic methods, plus the Self keyword and print() semantics."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Operators — Design Decisions

User spec: [`docs/reference/REF-operators.md`](../../reference/REF-operators.md)

---

## Operator Overloading via `follows` Contracts (designed, not built)

**Status:** `+`, `==` and the other operators on a user shape are not built. No `Addable`, `Equatable` or similar contract exists in the compiler or the registry (grep over `crates/` and `registry/`), so a program using `a + b` on two shapes does not compile. Until it ships, write the function and call it by name (`add(a, b)`).

Operators are syntactic sugar for standalone functions on standard library contracts. No magic method names, no special dispatch rules — the same `follows` system used everywhere else.

**Why stdlib-defined contracts vs built-in operator keywords**: User types follow stdlib contracts (`Addable`, `Equatable`). This means the operator system is extensible without compiler changes — new operator contracts can be added to the stdlib like any other type. The compiler maps `+` → `add()` at the syntax level; everything else is regular type system.

**The contracts (designed).** A contract is a shape of bare signatures; the functions are written at the top level of a file, never inside the shape:

```ynz
shape Addable {
  add(share self, share other: Self) -> Self
}

shape Equatable {
  equals(share self, share other: Self) -> Boolean
}

shape Comparable follows Equatable {
  compareTo(share self, share other: Self) -> Int
}

shape Printable {
  toString(share self) -> String
}
```

A shape opts in with `follows`, and standalone functions supply the behavior; the compiler checks each signature against the contract:

```ynz
shape Vector2D follows Addable, Equatable, Printable {
  x: Number
  y: Number
}

function add(share self: Vector2D, share other: Vector2D) -> Vector2D {
  return { x: self.x + other.x, y: self.y + other.y }
}

function equals(share self: Vector2D, share other: Vector2D) -> Boolean {
  return self.x == other.x && self.y == other.y
}

function toString(share self: Vector2D) -> String {
  return `(${self.x}, ${self.y})`
}

const a: Vector2D = { x: 1, y: 2 }
const b: Vector2D = { x: 3, y: 4 }

const c = a + b          // calls add(a, b)
const same = a == b      // calls equals(a, b)
print(a)                 // calls toString(a)
```

**Which function each operator calls (designed):**

| Operator | Contract | Function name |
|----------|----------|---------------|
| `+` | `Addable` | `add` |
| `-` | `Subtractable` | `subtract` |
| `*` | `Multipliable` | `multiply` |
| `/` | `Divisible` | `divide` |
| `==` | `Equatable` | `equals` |
| `<` `>` `<=` `>=` | `Comparable` | `compareTo` |
| `print(x)` | `Printable` | `toString` |

The compiler finds the function by its name and the type of its first parameter, so `a + b` means exactly `add(a, b)`.

---

## `Self` Keyword

`Self` (capital S) is a reserved type keyword meaning "the type that follows this contract."

**Why capital S**: Follows the naming convention — capital letter = type. `self` (lowercase) = the instance. `Self` (uppercase) = the type of the instance. Consistent, no new rule needed.

**How it resolves**: When `Vector2D follows Addable`, the compiler substitutes `Vector2D` for `Self` in all method signatures. `function add(share self, share other: Self) -> Self` becomes `function add(share self, share other: Vector2D) -> Vector2D`.

---

## `print()` Always Works — No `Printable` Required

Every type is printable. `print()` never errors.

**Default representation**: Type name + visible fields. Hidden fields excluded.

**Why a default**: Requiring `follows Printable` for `print()` to work would make debugging painful — you'd need to implement `toString()` before you could inspect any custom type. Default representation gives you something useful immediately. `Printable` is for when you want to customize the format, not for when you want `print()` to work at all.

**Hidden fields excluded from default**: Hidden fields are invisible to outside code. Printing them in the default representation would expose implementation details that the type author intentionally hid. Custom `toString()` can include hidden fields if the author chooses.

---

## Boolean Operators: `&&`, `||`, `!`

Symbol-based boolean operators, as in JavaScript and TypeScript. `and`, `or` and `not` are not operators.

**Why symbols**: Golden Rule 12 (familiar syntax): JavaScript and TypeScript developers already know `&&`, `||` and `!`.

**`|` and `||` stay apart by position**: `|` is a union type in type position (`shape Shape = Circle | Square`) and bitwise OR in an expression; `||` is boolean OR in an expression. The `.or(default)` call is a function, not an operator.

---

## Bitwise Operators — Symbols

Bitwise operators use single symbols (`&`, `|`, `^`, `~`, `<<`, `>>`) while boolean operators use double symbols (`&&`, `||`, `!`). The two sets never overlap.

**Why symbols for bitwise**: Golden Rule 12 (familiar syntax): every language with bitwise operations spells them this way.

**The split is readable**: `if (active && verified)` is a condition check. `flagA | flagB` is bit manipulation.

---

## No `===` Triple Equals

`==` is always type-safe. Comparing incompatible types is a compile error.

**Why**: `===` exists in JavaScript to compensate for `==` doing implicit type coercion (`"5" == 5` is `true`). Yinz has no implicit coercion. The type system prevents comparing a `String` to a `Number` at compile time. `===` solves a problem that doesn't exist in a statically-typed language.
