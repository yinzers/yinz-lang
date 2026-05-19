# Inline / Anonymous Shape Types — Single-Use Structural Types

> **Status**: Shipped — v0.1-polish (2026-05-19).
> **Spec**: `spec/types.md` → "Inline shapes (anonymous types)" section.
> **Trigger**: Patrick hit verbosity friction while testing — defining a one-off shape outside a function just to type a single binding scrolled the eye too far from the use site.

---

## The problem

Named `shape Foo { ... }` declarations are the right tool when a type is used in multiple places. But single-use types — a config struct for one function, an intermediate result that never leaves a loop — force you to scroll to the top of the file to see a type definition that is only meaningful in one spot. That's a coupling smell: the type's definition is physically separated from the only code that uses it.

## The rule

`shape Foo { ... }` only makes sense when `Foo` is used more than once. For one-off types, the definition should live at the call site.

## Syntax

`{` in any type-annotation position means an inline shape. Fields are comma-separated or newline-separated. Same construction syntax as named shapes.

```ynz
// Allowed positions:
let point: { x: int, y: int } = { x: 3, y: 4 }
function area(rect: { w: int, h: int }) -> int { return rect.w * rect.h }
const intervals: fixed<{ minutes: int }> = [{ minutes: 5 }, { minutes: 15 }]
shape Bar { spread: { bid: number, ask: number } }
```

## Design decisions (locked 2026-05-19)

### 1. Structural typing for anonymous shapes only

Two `{ a: int, b: string }` in different files are the same type. Field order does not matter for equivalence.

**Why**: The use case is one-off data definitions. Users expect `{ a: int, b: int }` annotated on a `let` to accept any value whose fields match — that's TypeScript's model and the natural expectation. Nominal typing for anonymous shapes would force users to write `shape` declarations anyway to make the types interchangeable.

**Named shapes remain nominal**: `shape Foo { a: int }` and `{ a: int }` are different types. The user picks one or the other; they don't interconvert.

### 2. Implementation: canonical-name hoisting

Anonymous shapes are NOT a new `Type` variant. Instead, they are **hoisted to synthetic named shapes** with content-based canonical names at the `collect_shapes` pre-pass. Two identical anon shapes get the same canonical name and therefore share the same `ShapeTable` entry.

Canonical name format: `__anon__fieldname__typename__fieldname__typename__...` (fields sorted by name, joined by `__`).

Example: `{ bid: number, ask: number }` → `__anon__ask__number__bid__number`.

This keeps the rest of the compiler (codegen, type checker, vtable) handling anon shapes identically to named shapes — no new code paths needed.

### 3. Hidden fields are not allowed in inline shapes

`hidden` fields require a named `shape` declaration (file-private visibility is meaningless for anonymous, one-off types). The parser emits a specific diagnostic when `hidden` appears inside an inline shape type.

### 4. Nested inline shapes work

`{ outer: { inner: int } }` is valid. The inner anon shape is hoisted to its own synthetic named shape and the outer field type resolves to `Type::Shape { name: "__anon__inner__int" }`.

### 5. Extends and follows not allowed

Inline shapes are pure data — they have no inheritance or contract requirements. The parser never accepts `extends` or `follows` inside a `{ ... }` type position (they're not in the field grammar).

## Implementation

- **Parser** (`crates/ynz-parser/src/parser.rs`): `parse_type_with_depth` handles `Token::LBrace` in type position — parses fields via `parse_field_decl`, supports comma separators, rejects `hidden`.
- **Shapes pre-pass** (`crates/ynz-typeck/src/shapes.rs`): `collect_anon_shapes_in_type` and `collect_anon_shapes_in_stmts` walk all type positions (shape fields, function params, return types, let-binding annotations) and register canonical synthetic `ShapeDecl` entries. Deduplication ensures identical anon shapes register once.
- **Type checker** (`crates/ynz-typeck/src/check.rs`): `ast_type_to_type` for `AstType::AnonShape` computes the canonical name and returns `Type::Shape { name }`. All downstream paths (field access, struct literal, for-destructuring) treat the synthetic shape identically to a named shape.
- **Type names** (`crates/ynz-typeck/src/types.rs`): `type_name` renders `__anon__...` names as `{ field: type, ... }` for user-facing diagnostics.

## Open questions (not addressed in v0.1-polish)

1. **Auto-promotion lint**: when the same inline type appears in 2+ places, a Tier 3 lint should suggest extracting to a named `shape`. Not implemented yet — covered by `.claude/rules/auto-promotion.md`.

2. **Cross-file structural equivalence**: currently two `{ a: int }` in different files produce the same canonical name and should work correctly. Integration tested only via same-file tests; cross-file case is untested.

3. **Inline shapes in `follows` contracts**: deferred. The current implementation focuses on annotation positions only (`:` position). Contract signatures with inline shapes remain unsupported.

## Related design

- `spec/types.md` — "Inline shapes (anonymous types)" section (user-facing spec)
- `.claude/rules/non-oop.md` — data shapes model (inline shapes are also pure data)
- `.claude/rules/auto-promotion.md` — the "2+ uses → extract to named shape" lint pattern
- `design/type-system.md` — nominal typing decision (named shapes remain nominal)
