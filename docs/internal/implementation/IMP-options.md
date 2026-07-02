---
name: "IMP-options"
description: "Design decisions for the options type (enum replacement), including its LLVM lowering to integer tags, exhaustiveness checking, and variant-count limits."
tags:
  - "yinz-compiler"
created_at: "2026-05-18"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Options — Design Decisions

User spec: [`docs/reference/REF-options.md`](../../reference/REF-options.md)

---

## What `options` is

`options Status { active, inactive, banned }` declares a type with a finite, named set of values. It replaces `enum` — the word "options" is plain English readable by a non-programmer (Golden Rule 12). The compiler knows every variant at compile time and enforces exhaustiveness in multi-case `if` blocks.

See [`docs/reference/REF-options.md`](../../reference/REF-options.md) for the user-facing surface.

---

## LLVM Lowering

Every `options` type lowers to an `i8` tag (or smallest fitting type for the variant count). The variants are assigned sequential integer values starting at 0 in declaration order.

| Variant count | LLVM type | Why |
|---|---|---|
| 2 to 255 variants | `i8` | One byte covers up to 256 distinct variants |
| 256+ variants | `i16`, but **COMPILE ERROR** with teaching hint | Avoid 256-variant options in v0.1; hint suggests refactoring. 256+ is almost certainly a design smell — more than the human eye can hold. |
| 0 or 1 variant | **COMPILE ERROR** | See "Single/empty-variant rejection" below |

`==` between two values of the same options type lowers to `icmp eq i8` — no vtable, no string comparison, no hash. Near-zero cost.

Multi-case `if` over an options scrutinee lowers to LLVM `switch` on the tag byte — the same dense jump-table optimization used for `int` multi-case (per [`docs/internal/implementation/IMP-control-flow.md`](IMP-control-flow.md)).

`.toString()` on an options value: the compiler emits a per-type LLVM global `[N x *const u8]` array of string literals indexed by tag. The `.toString()` method indexes by tag and calls `ynz_string_from_static(ptr, len)` to return a heap-owned Yinz string.

**Display strings** (added 2026-05-18): the `variantName: \`Display String\`` syntax lets the author attach a separate display string to any variant. When present, the display string populates the string table slot instead of the variant identifier. Variants without a display string fall back to the identifier name. This is purely a string-table substitution — the tag value, comparison semantics, and ABI are unchanged. The `OptionsEntry` in the typeck table carries a parallel `display_strings: Vec<Option<String>>` field; `lower_options_to_string` reads `display_strings[i].unwrap_or(&variants[i])` when building the LLVM string global per variant.

**Why not a separate `.displayString()` method**: one string representation per options type is cleaner and avoids the question of which method to call in interpolation. The display string IS the string — if you need both the identifier and the label, access the identifier via the variant name in code (which is free, compile-time) and call `.toString()` for display.

---

## Built-in Options

These are always in scope — no import needed:

```ynz
options SortOrder { asc, desc }
options Comparison { equal, greater, less }
```

Registered at typeck startup via `OptionsTable`.

---

## Exhaustiveness Enforcement

A multi-case `if` over an options scrutinee MUST cover every variant, or include an `else =>` catch-all. Missing variants are named individually in the diagnostic:

```
COMPILE ERROR: Non-exhaustive options multi-case — Status has 3 variants; only 2 are handled.

  Missing variant: banned

  WHAT INSTEAD: Add the missing arm:
    is Status.banned => ...
  or add a catch-all:
    else => ...

  WHY: The compiler knows every variant at compile time. A missing arm means
  some Status values would silently fall through — likely a bug.
```

---

## Ambiguous Shorthand Resolution

When using a bare identifier in a position where an options type is expected, the compiler resolves it via shorthand:

```ynz
players.sort(p => p.health, desc)    // shorthand — compiler infers SortOrder.desc
```

**Ambiguous shorthand** (two visible options types define the same variant name): compile error with locked diagnostic text:

> **WHAT**: `desc` is ambiguous — it's a variant of both `SortOrder` and `Direction` in scope here.
>
> **WHAT INSTEAD**: Use the qualified form: `SortOrder.desc` or `Direction.desc`.
>
> **WHY**: Shorthand resolution requires a unique match against the expected type. When two visible options types define the same variant name, the compiler refuses to guess.

**Function vs options-shorthand priority**: when a bare identifier resolves to BOTH a function in scope AND an options variant via shorthand, **the function wins**. If the function's type doesn't match the expected parameter type, the resulting type error names both candidates:

```
COMPILE ERROR: Type mismatch at call site — `desc` resolves to function `desc(int) -> string`,
not a SortOrder variant.

  WHAT INSTEAD: To use the options variant, write `SortOrder.desc` explicitly.

  WHY: In-scope functions take priority over options-variant shorthand. Qualify the
  variant to disambiguate.
```

---

## `is` Namespace Resolution for `options`

The `OptionName` form in a multi-case arm (`active =>`) resolves against the scrutinee's declared options type — the types-only namespace. A same-name binding in the values namespace does NOT shadow the variant lookup:

```ynz
let active = 5         // binding named 'active' — values namespace
if (status) {
  active => ...        // resolves Status.active — types namespace; the binding is irrelevant here
}
```

When the scrutinee is a union of two options types (`OptionsA | OptionsB`), the bare `OptionName` arm form is rejected; the user must use the qualified `is OptionsA.variantName =>` form, or refactor to nested multi-cases.

---

## Single/Empty-Variant Rejection

`options Foo { }` (empty) and `options Foo { only_one }` (single variant) are compile errors:

```
COMPILE ERROR: options types need at least 2 variants.

  WHAT INSTEAD: Add a second variant, or use a const if you only need one named value:
    const ONLY_ONE: int = 0

  WHY: A single-variant options type has only one possible value — it carries no
  information. It's almost always the wrong tool.
```

Symmetric with single-variant union rejection (`shape S = A` is also an error).

---

## `options` vs union types — Design Rationale

| | `options` | Union (`\|`) |
|---|---|---|
| Variants have different data shapes? | No — all variants are just labels | Yes — each variant is a distinct shape |
| Payload at runtime | None (tag is the whole value) | Yes (tag + payload) |
| Use when | Finite set of named modes/states | Value can be one of several structurally-different types |

See [`docs/internal/implementation/IMP-unions.md`](IMP-unions.md) for the union counterpart.

---

## Cross-References

- [`docs/reference/REF-options.md`](../../reference/REF-options.md) — user-facing surface (how to write options types and use them)
- [`docs/internal/implementation/IMP-unions.md`](IMP-unions.md) — union types (the structural-variant counterpart)
- [`docs/internal/implementation/IMP-narrowing.md`](IMP-narrowing.md) — flow-sensitive narrowing (applied to union `is` checks; options multi-case uses exhaustiveness, not narrowing)
- [`docs/internal/implementation/IMP-control-flow.md`](IMP-control-flow.md) — exhaustiveness enforcement for multi-case `if`; jump-table lowering
- [`docs/reference/REF-golden-rules.md`](../../reference/REF-golden-rules.md) Rule 12 — why "options" over "enum"
- `crates/ynz-typeck/src/options_table.rs` — OptionsTable registry (M6 implementation)
- `crates/ynz-codegen/src/options.rs` — options codegen helpers (M6 implementation)
