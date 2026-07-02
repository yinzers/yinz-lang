---
name: "IMP-unions"
description: "Design decisions for Yinz union types ('A | B | C' shapes), including the LLVM lowering decision table (pointer-niche vs tagged-struct representation) and single-variant rejection."
tags:
  - "yinz-compiler"
created_at: "2026-05-18"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Unions — Design Decisions

User spec: [`docs/reference/REF-unions.md`](../../reference/REF-unions.md)

---

## What a union type is

`shape Shape = Circle | Square | Triangle` declares a type whose values can be any of the listed variants. Each variant is a concrete shape (with its own fields). The `|` operator in type position separates variants — the same syntax as TypeScript union types (Golden Rule 6 — borrow familiar syntax).

Union types are the structural complement to `options`: `options` carries only a tag label; unions carry a tag PLUS a typed payload for each variant.

See [`docs/reference/REF-unions.md`](../../reference/REF-unions.md) for the user-facing surface.

---

## LLVM Lowering Decision Table

The lowering is chosen per concrete variant set at codegen time. The decision is mechanical from the variant properties; no heuristics, no user override.

| Variant set | LLVM type | Why |
|---|---|---|
| All variants are heap-allocated shapes (`ynz_alloc`-backed) AND no `none` variant | Pointer-niche: `{ i8 tag, ptr data }` where tag identifies which shape `data` points to | One pointer + one tag byte; no max-payload waste. Used when every payload is a single pointer. |
| Mixed value-type and heap variants, OR any variant has a value-type payload >1 pointer | Tagged struct: `{ i8 tag, [maxPayloadSize x i8] payload, padding }` aligned to max(alignof variant) | Single representation; payload area sized for largest variant; alignment correct for all. |
| `T \| none` (any T) | NOT a union per M6 codegen — this IS `maybe<T>` and uses M5's lowering table | Avoids double-encoding; the `T | none` form at typeck time is rewritten to `maybe<T>`. See [`docs/internal/implementation/IMP-maybe.md`](IMP-maybe.md). |
| Single-variant union (`shape S = A`) | **REJECTED at typeck** — compile error | Degenerate; see "Single-variant rejection" below. |
| 2 to 255 variants | Tag is `i8` | One byte covers up to 256 distinct variants |
| 256+ variants | Tag is `i16`, but **COMPILE ERROR** with teaching hint | 256+ union variants is almost certainly a code smell. Hint suggests refactoring. |

IR-snapshot tests in `crates/ynz-codegen/tests/snapshots.rs` assert each row of this table produces the expected LLVM type.

### Pointer-niche detail

For the all-heap-shapes case, `data` is an untyped pointer. The tag byte indicates which concrete shape it points to:
- tag = 0 → cast `data` to `*Circle`
- tag = 1 → cast `data` to `*Square`
- etc.

The payload is a single pointer (8 bytes on 64-bit); no additional payload space needed. This is the smallest possible representation when all variants are heap-allocated.

### Tagged-struct detail

`{ i8 tag, [maxPayloadSize x i8] payload }` where:
- `maxPayloadSize` = max of `sizeof(variant)` for all variants, rounded up to `alignof(largest variant)`
- alignment of the whole struct = max of `alignof(variant)` for all variants

LLVM emits explicit padding bytes to maintain alignment. The largest variant's value is stored inline in the payload bytes; smaller variants leave trailing bytes unused (the tag discriminates access).

---

## No User Override on Layout

There is no syntax for `dense<S>` or `tagged<S>` to force a particular layout. Rationale: layout is an implementation detail that users do not need to observe. The only legitimate reasons to pin layout (FFI, wire format, memory-mapped hardware) are handled at the boundary — not in the union declaration. Adding a layout modifier would create work for users who would almost never need it. Deliberate omission, symmetric with SSO threshold and auto-SoA (both also have no override).

---

## `is` Checks the Exact Type — Not the Subtype

In a union context, `is` performs exact-type matching. Inheritance does NOT change this:

```ynz
shape User { name: string }
shape Admin extends User { permissions: fixed<string> }

shape AnyUser = Admin | User

if (anyUser) {
  is Admin => ...    // exact match — only values whose tag = Admin's tag
  is User  => ...    // exact match — only values whose tag = User's tag; Admin does NOT fall through here
}
```

`Admin extends User` means Admin IS a User outside unions (normal subtype rules). Inside a union, `is` is purely tag-based — each variant is always distinct. This makes union exhaustiveness predictable: the set of variants is fixed and non-overlapping.

**Why exact-match**: if `is User` matched Admin (because Admin extends User), then `is Admin` and `is User` could overlap, exhaustiveness checking would be ambiguous, and adding a new `is User =>` arm would silently steal Admin cases. Exact-match eliminates the entire class of ambiguity.

---

## Exhaustiveness Enforcement

A multi-case `if` over a union scrutinee MUST cover every declared variant, or include an `else =>` catch-all. Missing variants are named individually:

```
COMPILE ERROR: Non-exhaustive union multi-case — Shape has 3 variants; only 2 are handled.

  Missing variant: Triangle

  WHAT INSTEAD: Add the missing arm:
    is Triangle => ...
  or add a catch-all:
    else => ...

  WHY: When you add a new variant to Shape later, the compiler will tell you every
  place that needs updating. Without this check, new variants are silently ignored.
```

---

## Single-Variant Rejection

`shape S = A` is a compile error:

```
COMPILE ERROR: Union types need at least 2 variants.

  WHAT INSTEAD: If you want a type alias, use shape directly:
    // S and A are the same type; just use A
  If you want a union with room to grow, add a second variant:
    shape S = A | B

  WHY: A single-variant union is just a type alias with extra steps. It compiles to
  the same layout as A and adds confusion. Use A directly, or a real union.
```

Symmetric with single-variant `options` rejection.

---

## Cross-References

- [`docs/reference/REF-unions.md`](../../reference/REF-unions.md) — user-facing surface (how to write unions and use `is`)
- [`docs/internal/implementation/IMP-options.md`](IMP-options.md) — options types (the label-only counterpart)
- [`docs/internal/implementation/IMP-narrowing.md`](IMP-narrowing.md) — flow-sensitive `is` narrowing (how the compiler tracks which variant is proven inside a block)
- [`docs/internal/implementation/IMP-maybe.md`](IMP-maybe.md) — `maybe<T>` lowering (the `T | none` special case; unions reuse M5's maybe encoding for that form)
- [`docs/internal/implementation/IMP-control-flow.md`](IMP-control-flow.md) — multi-case `if` exhaustiveness; jump-table lowering
- [`docs/reference/REF-golden-rules.md`](../../reference/REF-golden-rules.md) Rule 6 — borrow familiar syntax (`|` from TypeScript)
- `crates/ynz-typeck/src/unions_table.rs` — UnionLayoutTable cache (M6 implementation)
- `crates/ynz-codegen/src/unions.rs` — union codegen helpers (M6 implementation)
