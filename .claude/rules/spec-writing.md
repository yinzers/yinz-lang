---
name: "spec-writing"
description: >
  Tone, structure, audience, and code-example rules for the user-facing docs/reference/REF-*.md
  language-spec files — written for a developer who just graduated high school, knows
  JavaScript, and has never done systems programming.
tags:
  - "yinz-compiler"
  - "docs"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "rule"
---

# Spec Writing Rules (docs/reference/REF-*.md language-spec files)

Rules for writing and editing the **user-facing language spec** files, which live at `docs/reference/REF-*.md` under the global `docs/` taxonomy (migrated 2026-07-01 from the old `/spec/` directory — same audience and rules, new location). Not every `docs/reference/REF-*.md` file is a language-spec file (some are cross-cutting compiler principles like `REF-golden-rules.md`) — this rule applies to the ones documenting a language feature for end users (collections, ownership, control-flow, etc.).

---

## Audience

Write for: a developer who just graduated high school, knows JavaScript, and has never done systems programming.

They understand: variables, functions, loops, arrays, objects, callbacks.
They do NOT understand: pointers, memory management, type systems, compiler theory, ownership, or any CS jargon.

Test every sentence: "Would an 18-year-old JS developer understand this without Googling?"

---

## File Structure

Every spec file follows this order:
1. One-sentence description of what the section covers
2. Simplest possible code example
3. Plain English explanation
4. More examples showing variations and edge cases
5. Compiler error examples (always show the actual error message)

---

## Tone

- Short sentences. Active voice.
- "You" — not "the developer" or "the programmer."
- Define any technical term immediately when you use it.
- Don't say "unlike Rust" or "like TypeScript" — explain it fresh.
- Don't explain WHY a design decision was made in the spec — that belongs in the feature's `docs/internal/implementation/IMP-<feature>.md` file (per [`.claude/rules/docs-checklist.md`](docs-checklist.md)). Spec files just show how to use it.

---

## Code Examples

- Use realistic names: `Player`, `User`, `score`, `health` — not `foo`, `bar`, `x`, `y`
- Show the compiler error message when demonstrating mistakes
- No method chaining — step-by-step, one operation per line with a named variable
- Arrow functions only inside method calls (`.filter()`, `.map()`, etc.) — never as standalone functions

---

## Compiler Error Format

When showing a mistake, always include the error message the compiler would show:

```
rgb.add(50)
// COMPILE ERROR: Cannot add to a fixed array.
// fixed<number> is size-locked. Use array<number> if it needs to grow.
```

---

## What NOT to Include

- No implementation details (how the compiler works internally)
- No performance benchmarks (until there are real numbers)
- No comparisons to other languages in the main flow
- No unresolved design questions — move those to `docs/internal/scratchpad/SCRATCH-open-questions.md`
- No TODO comments — open questions live in `docs/internal/scratchpad/`
