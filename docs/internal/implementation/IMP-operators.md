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

## Operator Overloading via `follows` Contracts

Operators are syntactic sugar for methods on standard library contracts. No magic method names, no special dispatch rules — the same `follows` system used everywhere else.

**Why not magic methods**: Languages like Python use `__add__`, `__eq__`, `__str__`. These are invisible to autocomplete, require knowing the naming convention, and feel like magic. The `follows` approach is fully transparent — type `.add` and see the method. The operator is sugar for a regular method.

**Why stdlib-defined contracts vs built-in operator keywords**: User types follow stdlib contracts (`Addable`, `Equatable`). This means the operator system is extensible without compiler changes — new operator contracts can be added to the stdlib like any other type. The compiler maps `+` → `add()` at the syntax level; everything else is regular type system.

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

**The split is readable**: `if (active && verified)` is a condition check. `FLAG_A | FLAG_B` is bit manipulation.

---

## No `===` Triple Equals

`==` is always type-safe. Comparing incompatible types is a compile error.

**Why**: `===` exists in JavaScript to compensate for `==` doing implicit type coercion (`"5" == 5` is `true`). Yinz has no implicit coercion. The type system prevents comparing a `string` to a `number` at compile time. `===` solves a problem that doesn't exist in a statically-typed language.
