---
name: "IMP-destructuring"
description: "Design rationale for Yinz's destructuring support: why object/type destructuring is allowed but array destructuring is not, plus the 'as' renaming syntax and destructuring in function parameters."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Destructuring — Design Decisions

User spec: [`docs/reference/REF-destructuring.md`](../../reference/REF-destructuring.md)

---

## Object Destructuring Only — No Array Destructuring

Object/type destructuring is supported. Array destructuring is not.

**Why object destructuring is safe**: The compiler knows every field of a type at compile time. `let { name, health } = player` is guaranteed to succeed — `name` and `health` are proven to exist by the type system. No runtime risk.

**Array destructuring is not allowed**: array indices are not known at compile time, and `let [first, second] = items` would bypass the `Maybe` that `.get(index)` returns to force handling of the absent case.

**The pattern is consistent**: Objects = compile-time guaranteed fields = safe to destructure. Arrays = runtime-determined indices = use `.get()` which returns `Maybe`.

---

## `as` for Renaming

`let { health as hp } = player` follows the same `as` keyword used in import aliasing (`import { X as Y } from "module"`). One keyword for renaming in all contexts.

---

## Destructuring in Function Parameters

`function greet({ name, health }: Player) -> String` puts the type annotation after the destructured pattern. Same as TypeScript — familiar to JS/TS developers (Golden Rule 12).
