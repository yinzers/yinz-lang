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

## Stored union slot

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

**Heap-variant unions** (`string | array<int>`, `string | channel<int>`, `string | int`: no shape variant, two or more variants beside `nothing`, one of which may own heap; `is_envelope_union`) are envelope unions with no shape variant, so they take every path above (wrap, parameter, return, store, frame region) with the same slot: `{ i64 tag, 8-byte payload }`, 16 bytes, `UnionLayout` with no shape variants. The cost of the tag is one word, as a Rust `enum` discriminant. `string | int` is tagged because a scope exit must tell the int's bits from a string pointer; a union of value variants only (`int | float`) owns no heap, so it gets no tag word and no release function. The payload word is the value's bits or the heap pointer the slot owns. A wrap owns its payload from the start (a string is copied, a fresh or consumed array, map or channel moves in); a COPY of a slot (a binding made from a place, a `return` of a parameter) copies a string, array or map variant and shares a channel variant (`union_deepen_fn`, called by `union_persist_into_as` unless the source moves), so every owner releases its own. A channel variant is refused in a union with a shape variant (`envelope_refusal`). A union with one variant beside `nothing` (`map<..> | nothing`) stays the variant's raw pointer, null for `nothing`: it is released, and a shape field of it is copied, as that variant (`raw_union_variant`); a union of value variants releases nothing.

**Recursive unions** (a variant reaches the union through shape fields, so no finite inline size)
are the one exception: the slot is `{ tag, word }` and a shape variant's bytes live in a heap cell
the slot owns, one per box, as `Box<T>` in a Rust enum. A store MOVES such a value when it is
a move and COPIES it deep otherwise (next section), exactly where Rust moves or clones. A moved union BINDING or call result hands its box cell to the slot it is stored into (a field, a `let`) (`Cg::box_cell_views`): no new cell, no emptied cell left behind. The same
holds for a non-recursive union whose shape variant has such a field.

**Moving and copying a value that owns boxes.** A shape, union or `maybe` that owns
a recursive union's boxes is never copied by bytes alone, because the copy would share its boxes
with the source and a release would free each box twice. Typeck records which stores are MOVES
(`TypedModule::heap_moves`, `crates/ynz-typeck/src/heap_moves.rs`): the checker's admitted transfer
of a fresh value (a literal, a call whose every return is fresh, a `.copy()`) or of an owned
binding the store consumed (a later read is a compile error), and a `return` of either. Codegen
reads the record and decides nothing (`lower_expr` marks the value, `Cg::is_moved_cells` asks):

- a MOVED store takes the boxes as they are: one new cell for the value's own box, none for the
  boxes it owns (building a chain link by link makes one cell per link);
- every other store deep-copies, like `Clone`: `.copy()` (always, even when its source is not
  read again), a binding made from a place that stays where it is and is not a view (`let copied = outer.inner` with
  the field written while the binding is read, a `maybe` read of an element, a returned parameter's
  alias binding) and an element store of a value
  that is not moved. The copy clones each box the value owns (`deepen_unless_moved`,
  `union_deepen_fn`): O(n) cells for n boxes.

One seam makes the choice for every copy of a shape's bytes: union slot persist
(`union_persist_into_as`), `shape_bytes_to_owned`, `shape_bytes_to_heap_cell`,
`maybe_payload_stable_bits`, the embed slot and the staged element of an array or map store. A
value absent from the record is copied deep, so a hole costs cells and never a double free. A
`return` of a parameter, a field or an element copies it, so a function hands its caller an owned
value (`IMP-ownership.md` "Non-owned reads").

**Views.** A `let` of a place read (`let rest = head.next`) names a place that stays where it
is. Copying it into the binding is a clone, one box for every box the value owns, so a recursive
walk that peels one link per call would copy the whole tail at every step: O(n²) cells where a Rust
borrow costs none. A binding that only READS what it names is a read-only VIEW instead: it points at
the place (a union field's slot, a shape field's bytes), owns nothing and clones nothing, and the
language's value semantics are unchanged. Typeck proves it (`TypedModule::non_owned_reads`,
`crates/ynz-typeck/src/non_owned_reads.rs`, which also judges loop variables and call arguments), inside the same liveness walk as `string_moves`, and
codegen only reads the record:

- (a) the view is never written, stored, returned, given or sent;
- (b) the root of the place is not assigned, written through a field or index, or passed to a
  position that may write or consume it, anywhere in the view's live range; a parameter the function
  only reads (`share`) satisfies (b) for the whole body;
- (c) the view never crosses a suspension: typeck refuses a written `wait` or `background` in the
  live range, and codegen refuses a binding in the crossing set (`sm_crossing_names`, the set that
  sizes the frame, which counts a call that suspends without a written `wait`). A state-machine
  function whose binding does not cross views like any other.

Every other binding keeps the deep copy, as Rust needs `.clone()` where it refuses the borrow. The
forms that are views:

- a field-read chain from a parameter or a plain `let` (`head.next`, `outer.inner.chain`);
- a read THROUGH a view (`let b = a.g` with `a` a view of `x`): `b` is a view of `x`, checked against
  `x`, and only while `a` is a view;
- an element read of a built-in array, fixed list or map: `arr[i]`, `arr[i].f`, `arr.get(i)`,
  `.first()`, `.last()`. An element read is a copy of the element's bytes into per-site staging
  (a union element is a pointer into the buffer), so it shares the element's boxes and stays valid
  while the collection is not written. Any write to the collection breaks (b): the in-place mutators
  (`add`, `set`, `remove`, `clear`, `removeFirst`, `removeLast`, from the same
  `*_method_is_mutating` tables the checker uses to refuse them on a `share` parameter) and growth,
  a `lend`/`give`/`send` of the collection, and an assignment to it.

An annotated `let` (a possible union wrap) is never a view. Which call positions only read comes
from `effective_ownership::call_position_reads`, derived from the effective-ownership fixpoint, so
a view analysis and the checker cannot disagree about a callee.

Override directions (`auto-promotion.md`). Force the OTHER pick (an independent copy where a view
fits): `let own = head.next.copy()`, existing syntax, always a deep copy. Force the AUTO pick (a
view where the compiler keeps the copy): deliberately no override, because every refusal above is
a soundness refusal (a write, an escape, a suspension, an unknown callee), and a forced view there
would read a changed or freed place. A view has no source form (no keyword takes a binding "by
view"), so there is no muted hint to make explicit.

**Ownership.** A `share` union parameter cannot be stored (`ParamNeedsGive`); a `give` union is
copied into the slot. A slot owns its payload: a shape variant owns the shape's fields (and, boxed,
its cell), a `string`, array or map variant owns that value, scalar variants and `none` own nothing.

**Equality.** Two union fields are equal when their tags match and the stored variants are equal
as values of that variant (field by field for a shape variant, the payload word otherwise).

**Not stored.** A `maybe<Thing>` binding (or an `errors` result of one) held across a `wait`,
and a union indexed-set into an array, are refused by typeck.

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
