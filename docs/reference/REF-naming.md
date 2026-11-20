---
name: "REF-naming"
description: "The plain-English keywords Yinz uses (nothing, none, shape, follows and more), what each one means, and how names are capitalized."
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
| `nothing` | A function returns no value | `-> nothing` |
| `none` and `maybe` | A value might be absent | `maybe<string>`: a string, or `none` |
| `shape` | Declares a named group of fields | `shape Player { name: string }` |
| `options` | A fixed set of named choices | `options Status { active, banned }` |
| `base` | A shape that other shapes build on and that you never create directly | `base shape` |
| `follows` | A shape provides the functions a contract lists (a contract is a list of function signatures) | "Player follows Damageable" |
| `\|` | A value can be one of several shapes | `Circle \| Square` |
| `is` | Asks which kind a union value is (a union is a value that can be one of several shapes) | "if shape is Circle" |
| `function` | Declares a function | `function heal(...)` |
| `share` and `lend` | How a function uses a value it receives (`share` reads it, `lend` may change it); written in function signatures only | `share player: Player` |
| `errors` | A function can fail, and the failure goes up to the caller automatically | `-> string errors` |

---

## Capital Letter = Type (Golden Rule 10)

PascalCase is exclusively for types. Everything else is lowercase camelCase — modules, functions, variables, keywords. No exceptions.

```
Player          // type
player          // variable
request         // module
request.get()   // module function
fetchUser       // function name
let userName    // variable
```

Modules and types can share the same base name: `Date` is the type, `date` is the module. `Response` is the type, `response` is the module.

**Why**: Instant scannability. Any line of code — see a capital letter, it's a type. Don't see one, it's not. Zero ambiguity, no context-reading required. The rule is mechanical and absolute: no threshold, no judgment calls.

---

## Comments Syntax

```
// regular comment, one line
/// doc comment, shown in IDE hover
```

See [`REF-doc-comments.md`](REF-doc-comments.md) for the doc-comment rules.
