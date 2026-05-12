# Yinz — Claude Project Rules

Yinz is a compiled systems programming language. Goal: Rust-level performance, TypeScript-level readability. Memory-safe, zero-cost abstractions, approachable by junior developers.

File extension: `.ynz`. Compiler target: LLVM native machine code.

---

## Golden Rules (check every design decision against these — lower number wins)

1. **Dot-first design.** If something can be `.method()` with autocomplete, do it that way. No cryptic symbols, no keywords to memorize.
2. **Self-documenting syntax.** Every keyword, function, and pattern should be readable by a jr dev who's never seen Yinz. If docs are required to understand a line, the design failed.
3. **No garbage collector.** Ownership-based memory management only. One memory model. The compiler handles allocation and freeing.
4. **Compiler does the hard work.** Smart defaults everywhere. Type inference inside function bodies. Developer writes simple code; compiler makes it fast.
5. **Compile-time safety.** Wrong = caught at compile time, never at runtime.
6. **Familiar syntax.** Borrow from TypeScript/JavaScript wherever possible.
7. **Step-by-step over chaining.** No method chaining. Each operation gets its own line with a named variable. Compiler fuses into a single optimized pass.
8. **Zero-cost abstractions.** High-level syntax compiles to the same machine code as hand-written low-level code.
9. **Fast to type.** Quick to write without sacrificing readability. `function` over `fn` is worth it; don't add ceremony where it adds nothing.
10. **Efficiency first, dynamic after.** Default = most performant. `fixed[T]` by default, opt into `array[T]` when you need growth.
11. **The compiler is a teacher.** Compile errors explain what went wrong and suggest the fix.
12. **Human-readable over programmer jargon.** `options` not `enum`. `follows` not `implements`. `nothing` not `void`. If a non-programmer can guess the meaning, the naming is right. (Note: union types use `|` for consistency with TypeScript — `or` was triple-overloaded.)
13. **Capital letter = type. Everything else = lowercase.** Scan any line of code — capital letter means type, no capital means everything else (function, variable, keyword, module). `Player` is a type. `player` is a variable. `http` is a module. `Http` would be a type. Zero ambiguity.

---

## Project Layout

| Path | Purpose |
|------|---------|
| `/spec/` | Language specification — for users of Yinz |
| `/design/` | Design decisions and open questions — for contributors |
| `.claude/rules/` | Detailed rule files (loaded on demand) |
| `CLAUDE.md` | This file — rules for Claude |

---

## Rules Files

| File | Load when |
|------|-----------|
| `.claude/rules/naming.md` | Any time renamed concepts come up (`options` vs `enum`, etc.) |
| `.claude/rules/spec-writing.md` | Writing or editing `/spec/` files |
| `.claude/rules/language-design.md` | Making or reviewing language design decisions |

---

## When Working on This Project

- Check every proposed language feature against all 12 golden rules before suggesting it
- Always use Yinz terms — see `.claude/rules/naming.md` for the full reference
- Spec files are written for a HS grad — short sections, example-heavy, plain English
- No method chaining in code examples; step-by-step with named variables
- Design decisions go in `/design/decisions.md` with the WHY captured
- Open questions go in `/design/open-questions.md`
- When a design decision is made, move it from open questions to decisions

---

## Tech Stack (fill in as compiler is built)

**Compiler target**: LLVM (planned)
**Package manager**: TBD
**Compiler implementation language**: TBD

```bash
# CLI commands (spec — not yet implemented)
ynz build main.ynz
ynz run main.ynz
```
