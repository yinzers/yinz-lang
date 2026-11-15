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

## Stored union slot (v0.3-M9 C1f)

The lowering table above names the logical representation. What a union value is when it is
*stored* (an array element, a map value, a shape field, a binding that crosses a `wait`, a
return, a staged argument) is one layout, computed once.

**Slot.** An *envelope union* (one with a shape variant) is stored as `{ i64 tag, payload }`, laid
out like a Rust `enum`: the payload area is as large as the largest variant (at least 8 bytes for
the non-shape variants), at the shape alignment (8, or 16 with a `number` field), and the slot size
is a multiple of that alignment. `Thing = Tiny | Big` is 24 bytes when `Big` is 16. `none` (a union
that lists `nothing`, or a `maybe` of the union) is the tag `u64::MAX`. The layout is
`UnionLayout` in `crates/ynz-codegen/src/shape_types.rs`; the frame layout, codegen and any
release helper read that one answer.

**Value form.** Inside an expression a union is a pointer to a per-site `{tag, data}` view envelope
(`data` = the payload's address for a shape variant, the value's bits otherwise), null for `none`.
`union_slot_to_value` builds the view over a slot; `union_persist_into` copies a value into a slot.
A view is valid as long as the slot it views.

**By value, no heap home.** Every store copies the payload into the slot: a container element, a
map value and a shape field (a hidden default into a union field included), a binding (an entry
slot owned by the binding, so rebinding the source never changes the copy), a `return` (the slot
comes back by value), an `errors` result (`{ err, slot }`) and a `maybe<Thing>` return (the slot,
`none` the none tag), and a `wait`-staged or `background` argument (a `Param` region of the
callee's own frame). A suspending function returns the slot through a `Return` region of its frame;
the waiting caller copies it out before the frame is freed. A binding that crosses a `wait` lives in
a region of the function's frame (`crossing_local_slot_count`); every resume rebuilds the view from
it, so no pointer into a resume function's stack survives a suspension. None of these allocates.

**Recursive unions** (a variant reaches the union through shape fields, so no finite inline size)
are the one exception: the slot is `{ tag, word }` and a shape variant's bytes live in a heap cell
the slot owns, one per store, as `Box<T>` in a Rust enum. Copying such a union into a binding
clones its cell too.

**Ownership.** A `share` union parameter cannot be stored (`ParamNeedsGive`); a `give` union is
copied into the slot. A slot owns its payload: a shape variant owns the shape's fields (and, boxed,
its cell), a `string`, array or map variant owns that value, scalar variants and `none` own nothing.

**Equality.** Two union fields are equal when their tags match and the stored variants are equal
as values of that variant (field by field for a shape variant, the payload word otherwise).

**Not stored.** A `maybe<Thing>` binding held across a `wait`, and a union indexed-set into an
array, are refused by typeck.

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
