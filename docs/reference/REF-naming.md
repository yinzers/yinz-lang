---
name: "REF-naming"
description: "Design rationale for Yinz's human-readable keyword naming choices (e.g. nothing, none, shape, follows) over traditional CS jargon, per Golden Rule 5."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Naming — Design Decisions

User spec: [`docs/reference/REF-language-overview.md`](REF-language-overview.md) (golden rules), [`.claude/rules/vocabulary.md`](../../.claude/rules/vocabulary.md)

---

## Human-Readable Keywords over Programmer Jargon

Use plain English words instead of CS terms throughout the language:

| CS term | Yinz term | Why |
|---------|-----------|-----|
| `void` | `nothing` | Reads like English |
| `null` / `undefined` | `none` with `maybe` | Explicit optionality |
| `struct` / `class` / `interface` | `shape` | One concept, one word |
| `enum` | `options` | Non-programmers understand it |
| `abstract` | `base` | "Base type you build on" |
| `implements` | `follows` | "Player follows Damageable" |
| `\|` (union) | `\|` | Same as TypeScript — `Circle \| Square` |
| `typeof` / `instanceof` | `is` | "if shape is Circle" |
| `fn` | `function` | Spell it out |
| `&T` / `&mut T` | `share` / `lend` | Keywords written in function signatures only |
| `Result<T,E>` / `?` | `errors` keyword | Auto-propagation |

**Why**: Golden Rule 5. Every keyword that requires CS background knowledge is a barrier to junior developers. Keywords that read like plain English lower the learning curve and make code self-documenting.

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
