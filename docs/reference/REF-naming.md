---
name: "REF-naming"
description: "The plain-English keywords Yinz uses (none, shape, follows and more), what each one means, and how names are capitalized."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Naming

Yinz uses plain English words for its keywords, and capital letters to mark types.

---

## Keywords in plain English

| Yinz keyword | What it means | Reads as |
|--------------|---------------|----------|
| `none` | A value is absent (only a `Maybe` can hold it) | `Maybe<String>`: a `String`, or `none` |
| `shape` | Declares a named group of fields | `shape Player { name: String }` |
| `options` | A fixed set of named choices | `options Status { active, banned }` |
| `base` | A shape that other shapes build on and that you never create directly | `base shape` |
| `follows` | A shape provides the functions a contract lists (a contract is a list of function signatures) | "Player follows Damageable" |
| `\|` | A value can be one of several shapes | `Circle \| Square` |
| `is` | Asks which kind a union value is (a union is a value that can be one of several shapes) | "if shape is Circle" |
| `function` | Declares a function | `function heal(...)` |
| `share` and `lend` | How a function uses a value it receives (`share` reads it, `lend` may change it); written in function signatures only | `share player: Player` |
| `errors` | A function can fail, and the failure goes up to the caller automatically | `-> String errors` |

---

## Capital Letter = Type (Golden Rule 10)

Every type is PascalCase, built-in types included. Everything else is lowercase camelCase — modules, functions, variables, keywords. No exceptions.

```
Player          // type you wrote
Int             // built-in type
Array<Player>   // built-in type
player          // variable
request         // module
request.get()   // module function
fetchUser       // function name
let userName    // variable
```

A name can exist in both casings. `Date` is the type, `date` is the module. `Response` is the type, `response` is the module. `Range` is the type, `range` is the function: `range(1, 10)` returns a `Range`.

### Built-in types

| Type | Holds |
|------|-------|
| `Int` | A whole number |
| `Float` | A fast binary decimal number |
| `Number` and `Number<N>` | An exact decimal number, optionally with `N` digits |
| `Boolean` | `true` or `false` |
| `String` | Text |
| `Range` | A sequence of whole numbers, made by `range(start, end)` |
| `Maybe<T>` | A `T` that might be absent |
| `Array<T>` | A list that can grow |
| `Fixed<T>` | A list whose size is set when you create it |
| `Map<K, V>` | Values looked up by key |
| `Channel<T>` | A line for passing values between tasks |
| `Nothing` | What a function returns when it returns no value: `-> Nothing` |

`Nothing` is an ordinary type name, not a keyword. A function, module or variable can use the lowercase spelling of a type name. A variable called `number` is fine.

### Old lowercase type names

Writing a built-in type in lowercase is a compile error:

```
let health: int = 100
// COMPILE ERROR: `int` is not a type name.
//   Write `Int`.
//   Why: Every type in Yinz starts with a capital letter, built-ins included.
```

The editor offers a quick-fix titled ``Write `Int` instead of `int` `` that rewrites the name for you.

### Shapes cannot reuse a built-in type name

A shape or union alias named like a built-in type is a compile error. The reserved names are `Int`, `Float`, `Number`, `Boolean`, `String`, `Range`, `Maybe`, `Array`, `Fixed`, `Map`, `Channel`, `Nothing`, `MapEntry`, `Frame` and `SourceLoc`:

```
shape String {
  value: Int
}
// COMPILE ERROR: `String` is a reserved built-in type name.
//   Give this shape another name, such as `MyString` or one that says what it holds.
//   Why: Yinz already has a built-in type called `String`. A shape with the same name would make every `String` in this file ambiguous between your shape and the built-in.
```

**Why**: Instant scannability. Any line of code — see a capital letter, it's a type. Don't see one, it's not. Zero ambiguity, no context-reading required. The rule is mechanical and absolute: no threshold, no judgment calls.

---

## Comments Syntax

```
// regular comment, one line
/// doc comment, shown in IDE hover
```

See [`REF-doc-comments.md`](REF-doc-comments.md) for the doc-comment rules.
