---
name: "IMP-control-flow"
description: "Design rationale for Yinz's control-flow constructs, including why standalone else blocks are disallowed and the early-return/pre-assignment replacement patterns."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Control Flow — Design Decisions

User spec: [`docs/reference/REF-control-flow.md`](../../reference/REF-control-flow.md)

---

## No Standalone `else` Block

`else { }` is not valid syntax. The only `else` in the language is `else =>` as a catch-all inside a multi-case `if` block.

**Two replacement patterns:**

Early return (inside functions):
```
if (condition) {
  doThing()
  return
}
doOtherThing()
```

Pre-assignment (when you can't return):
```
let value = "default"
if (condition) {
  value = "other"
}
```

**Why**: Forces flat code. Every `else` block adds a level of nesting. Early returns keep the happy path at the left margin with edge cases handled first. Pre-assignment is two lines and universally readable. The language philosophy is step-by-step, flat code — eliminating `else` enforces it at the syntax level.

**No `else if` chains either**: Use multi-case `if` instead. Chained `else if` is what multi-case was designed to replace.

---

## Multi-Case `if` — No `switch`, No `match`

Multi-case matching uses the existing `if` keyword with `=>` arrows inside the block. The compiler distinguishes simple `if` from multi-case `if` by the presence of `=>` inside the block.

**Why no `switch`**: switch has fall-through bugs (famously confusing in C/Java/JS), requires `break` statements, and is a distinct keyword to learn. Multi-case `if` has no fall-through, no `break`, and uses the keyword developers already know.

**Why no `match`**: `match` is Rust/functional jargon. `if` reads like English — developers already know it checks conditions. Multi-case `if` is a natural extension of what `if` already means.

**Zero new keywords added**: The compiler distinguishes the two forms based on `=>` inside the block. One keyword, two forms, no ambiguity.

---

## `else =>` as Catch-All Inside Multi-Case

The `else =>` inside a multi-case block is the catch-all case. It reads like English ("else, do this"). `else` only exists in this context — never standalone.

**Alternatives considered:**
- `default =>` — more familiar from switch/match; rejected because `else` is more English (Golden Rule 12) and `default` feels like switch syntax
- `_ =>` — common in Rust/Haskell; rejected because `_` is programmer notation, not plain English

---

## Exhaustiveness Enforcement

For `options` types and union types (`type Shape = Circle | Square`), the compiler verifies all cases are handled. Missing a variant is a compile error.

For value matching (numbers, strings), exhaustiveness is not enforced — the set of possible values is infinite. `else =>` serves as the required catch-all when used.

**Why enforce for options/unions but not values**: The compiler knows every variant of an `options` type or union at compile time. It can verify completeness. For arbitrary integers or strings, it cannot. The distinction is based on what the compiler can actually prove.

---

## Flow-Sensitive Narrowing in `if` Conditions

When the condition of an `if` block proves something about a binding's type (e.g., `x is Circle`, or `m.exists()`), the compiler narrows the binding's type inside the then-block. This is flow-sensitive analysis — the narrowed type is only valid in the scope where the proof holds.

The full narrowing rules table (all forms, including early-return narrowing, `&&`/`||` propagation, reassignment invalidation, and closure non-propagation) lives in [`docs/internal/implementation/IMP-narrowing.md`](IMP-narrowing.md).

---

## Compiler Optimization

Multi-case `if` blocks compile to jump tables internally — same performance as a traditional switch/match statement. No runtime cost for the readable syntax.
