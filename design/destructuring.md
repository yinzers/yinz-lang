# Destructuring — Design Decisions

User spec: `spec/destructuring.md`

---

## Object Destructuring Only — No Array Destructuring

Object/type destructuring is supported. Array destructuring is not.

**Why object destructuring is safe**: The compiler knows every field of a type at compile time. `let { name, health } = player` is guaranteed to succeed — `name` and `health` are proven to exist by the type system. No runtime risk.

**Why array destructuring is not allowed**: Array indices are not compile-time guaranteed. `let [first, second] = items` would bypass the `.get()` safety check that enforces `maybe` returns for potentially out-of-bounds access. The whole point of `.get(index)` is to force handling of the absent case — destructuring would silently undermine that.

**The pattern is consistent**: Objects = compile-time guaranteed fields = safe to destructure. Arrays = runtime-determined indices = use `.get()` which returns `maybe`.

---

## `as` for Renaming

`let { health as hp } = player` follows the same `as` keyword used in import aliasing (`import { X as Y } from "module"`). One keyword for renaming in all contexts.

---

## Destructuring in Function Parameters

`function greet({ name, health }: Player) -> string` puts the type annotation after the destructured pattern. Same as TypeScript — familiar to JS/TS developers (Golden Rule 6).
