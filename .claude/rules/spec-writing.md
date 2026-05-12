# Spec Writing Rules

Rules for writing and editing files in `/spec/`.

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
- Don't explain WHY a design decision was made in the spec — that belongs in `/design/decisions.md`. Spec files just show how to use it.

---

## Code Examples

- Use realistic names: `Player`, `User`, `score`, `health` — not `foo`, `bar`, `x`, `y`
- Show the compiler error message when demonstrating mistakes
- No method chaining — step-by-step, one operation per line with a named variable
- Arrow functions only inside method calls (`.where()`, `.map()`, etc.) — never as standalone functions

---

## Compiler Error Format

When showing a mistake, always include the error message the compiler would show:

```
rgb.add(50)
// COMPILE ERROR: Cannot add to a fixed array.
// fixed[number] is size-locked. Use array[number] if it needs to grow.
```

---

## What NOT to Include

- No implementation details (how the compiler works internally)
- No performance benchmarks (until there are real numbers)
- No comparisons to other languages in the main flow
- No unresolved design questions — move those to `/design/open-questions.md`
- No TODO comments — open questions live in the design folder
