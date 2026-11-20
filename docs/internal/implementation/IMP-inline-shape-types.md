---
name: "IMP-inline-shape-types"
description: "Design decisions for inline/anonymous shape types in Yinz — single-use structural types declared with '{ }' directly at the use site instead of a named shape."
tags:
  - "yinz-compiler"
created_at: "2026-05-19"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Inline / Anonymous Shape Types — Single-Use Structural Types

> **Status**: Shipped.
> **Spec**: [`docs/reference/REF-types.md`](../../reference/REF-types.md) → "Inline shapes (anonymous types)" section.

---

## The rule

`shape Foo { ... }` is for a type used more than once. A one-off type is defined at its use site, so its definition sits next to the only code that uses it.

## Syntax

`{` in any type-annotation position means an inline shape. Fields are comma-separated or newline-separated. Same construction syntax as named shapes.

```ynz
// Allowed positions:
let point: { x: Int, y: Int } = { x: 3, y: 4 }
function area(rect: { w: Int, h: Int }) -> Int { return rect.w * rect.h }
const intervals: Fixed<{ minutes: Int }> = [{ minutes: 5 }, { minutes: 15 }]
shape Bar { spread: { bid: Number, ask: Number } }
```

## Design decisions (locked)

### 1. Structural typing for anonymous shapes only

Two `{ a: Int, b: String }` in different files are the same type. Field order does not matter for equivalence.

**Why**: a `{ a: Int, b: Int }` annotation on a `let` accepts any value whose fields match, so one-off types interchange without a `shape` declaration.

**Named shapes remain nominal**: `shape Foo { a: Int }` and `{ a: Int }` are different types. The user picks one or the other; they don't interconvert.

### 2. Implementation: canonical-name hoisting

Anonymous shapes are NOT a new `Type` variant. Instead, they are **hoisted to synthetic named shapes** with content-based canonical names at the `collect_shapes` pre-pass. Two identical anon shapes get the same canonical name and therefore share the same `ShapeTable` entry.

Canonical name format: `__anon__fieldname__TypeName__fieldname__TypeName__...` (fields sorted by name, joined by `__`).

Example: `{ bid: Number, ask: Number }` → `__anon__ask__Number__bid__Number`.

This keeps the rest of the compiler (codegen, type checker, vtable) handling anon shapes identically to named shapes — no new code paths needed.

### 3. Hidden fields are not allowed in inline shapes

`hidden` fields require a named `shape` declaration (file-private visibility is meaningless for anonymous, one-off types). The parser emits a specific diagnostic when `hidden` appears inside an inline shape type.

### 4. Nested inline shapes work

`{ outer: { inner: Int } }` is valid. The inner anon shape is hoisted to its own synthetic named shape and the outer field type resolves to `Type::Shape { name: "__anon__inner__Int" }`.

### 5. Extends and follows not allowed

Inline shapes are pure data — they have no inheritance or contract requirements. The parser never accepts `extends` or `follows` inside a `{ ... }` type position (they're not in the field grammar).

## Implementation

- **Parser** (`crates/ynz-parser/src/parser.rs`): `parse_type_with_depth` handles `Token::LBrace` in type position — parses fields via `parse_field_decl`, supports comma separators, rejects `hidden`.
- **Shapes pre-pass** (`crates/ynz-typeck/src/shapes.rs`): `collect_anon_shapes_in_type` and `collect_anon_shapes_in_stmts` walk all type positions (shape fields, function params, return types, let-binding annotations) and register canonical synthetic `ShapeDecl` entries. Deduplication ensures identical anon shapes register once.
- **Type checker** (`crates/ynz-typeck/src/check.rs`): `ast_type_to_type` for `AstType::AnonShape` computes the canonical name and returns `Type::Shape { name }`. All downstream paths (field access, struct literal, for-destructuring) treat the synthetic shape identically to a named shape.
- **Type names** (`crates/ynz-typeck/src/types.rs`): `type_name` renders `__anon__...` names as `{ field: type, ... }` for user-facing diagnostics.

## Auto-promotion lint

When the same inline type appears in 2+ places, a Tier 3 lint warns at each use site and suggests extracting a named `shape`. Implemented in `check.rs` `lint_repeated_inline_shapes`; 3 tests in `tests/check.rs`.

## Known gaps

1. **Cross-file structural equivalence**: passing a correctly typed struct literal to an imported function works (tested). Passing a struct literal with wrong fields to an imported function does not yet produce an error, because the parameter's expected type is not threaded into untyped call-argument checking in `check_call`. Test `cross_file_inline_shape_field_mismatch_documents_known_gap` in `tests/check.rs` tracks it.

2. **Inline shapes in `follows` contracts**: not supported. Inline shapes work in annotation positions (after `:`) only.

## Related design

- [`docs/reference/REF-types.md`](../../reference/REF-types.md) — "Inline shapes (anonymous types)" section (user-facing spec)
- [`.claude/rules/non-oop.md`](../../../.claude/rules/non-oop.md) — data shapes model (inline shapes are also pure data)
- [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md) — the "2+ uses → extract to named shape" lint pattern
- [`docs/internal/implementation/IMP-type-system.md`](IMP-type-system.md) — nominal typing decision (named shapes remain nominal)
