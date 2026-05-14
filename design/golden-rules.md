# Golden Rules — Extended Reference

The 12 rules with full reasoning. The rules themselves also live in `CLAUDE.md` (loaded every turn). This file adds the "why" behind each one.

---

**1. Dot-first design**
If something can be `.method()` with autocomplete, it should be. No cryptic symbols, no keywords to memorize.

*Why*: Autocomplete teaches the language. A developer who types `.` and sees their options doesn't need documentation. Rust's `&'a mut` requires memorization; `name.lend` shows up when you type `.`.

---

**2. Self-documenting syntax**
Every keyword, function, and pattern should be readable by a junior developer who has never seen Yinz. If someone needs docs to understand what a line does, the design failed.

*Why*: The single biggest barrier to Rust adoption is its syntax. `name.lend` vs `&mut name`. Both express the same concept — one is guessable, one isn't. Every design decision should aim for guessable.

---

**3. No garbage collector**
Ownership-based memory management only. One memory model, no two-tier complexity.

*Why*: GCs add unpredictable pauses, 2-3x memory overhead, and a second conceptual model ("I write the code but the GC cleans up sometimes"). Ownership is deterministic and has zero runtime overhead. It's also one of the things that makes Go and Java unsuitable for low-latency systems.

---

**4. Compiler does the hard work**
Smart defaults everywhere. Type inference inside function bodies, operation fusion, optimal memory layout.

*Why*: The gap between "what you write" and "what runs" should be the compiler's job. Developers shouldn't have to hand-optimize for performance that the compiler could figure out automatically.

---

**5. Compile-time safety**
Wrong code fails at compile time, never at runtime.

*Why*: A runtime crash at 3am in production costs orders of magnitude more than a compile error during development. Every safety check that can be moved to compile time should be.

---

**6. Familiar syntax**
Borrow from TypeScript/JavaScript wherever possible.

*Why*: There are tens of millions of JS/TS developers. If they feel at home on day one, adoption starts from a much larger base. `let`, `const`, arrow functions, backtick interpolation — free familiarity.

---

**7. Step-by-step over chaining**
No method chaining. Each operation gets its own line with a named variable. Compiler fuses sequential operations into a single optimized pass.

*Why*: `players.filter(p => p.health > 0).sort(p => p.health).limit(10)` is undebuggable — you can't inspect the middle. `let active = ...; let ranked = ...` gives every intermediate result a name and an inspection point. The compiler eliminates any performance difference.

---

**8. Zero-cost abstractions**
High-level syntax compiles to the same machine code as hand-written low-level code.

*Why*: If convenience costs performance, developers will bypass the readable path in hot code. Zero-cost means the right choice and the fast choice are always the same choice.

**Clarification — do not interpret as relaxation**:

- "Zero-cost ABSTRACTIONS" means the abstraction itself adds no overhead beyond hand-written code. It does NOT mean "no features have any cost ever."
- Features themselves (Arc reference counting, async runtime scheduler, arena allocator bookkeeping) cost what they inherently cost. Those costs apply whether the developer writes them manually or the compiler infers them.
- Compiler inference happens at compile time → zero RUNTIME cost from the inference itself. The compiler choosing `.share` for you produces identical machine code to you writing `.share` explicitly.
- The "hand-written low-level" benchmark is "what would a careful Rust/C++/Zig programmer write," not "the absolute theoretical minimum." Memory safety has a cost — that cost is the bar; not zero.
- **Any future design tradeoff that costs MORE than hand-written code requires Patrick's explicit approval AND documentation in `design/decisions.md`.** This clarification block exists to prevent future drift — interpreting Rule 8 as "soften zero-cost" is wrong.

This clarification was added 2026-05-14 after the design-lockdown conversation surfaced the risk that auto-inferred features (auto-Arc for cross-thread shared state, auto-wait for I/O) might be mistaken for zero-cost violations. They aren't — the cost is in the underlying feature, not the inference.

---

**9. Fast to type**
Quick to write without sacrificing readability.

*Why*: Developer experience matters. A language that's pleasant to write gets used. `function` over `fn` is worth the extra characters because it's immediately readable — but ceremony that adds nothing (like `Optional<T>` vs `maybe T`) should be cut.

---

**10. Efficiency first, dynamic after**
The default behavior is always the most performant option. Developers opt INTO slower/flexible behavior explicitly.

*Why*: Junior developers don't know which choices are fast. If fixed arrays are default, map lookups have a visible type change, and the compiler suggests typed objects over maps — junior code is fast by default. Experienced developers can always opt in to dynamic behavior when they need it.

---

**11. The compiler is a teacher**
Errors and IDE teaching surfaces explain what went wrong (or what's happening) AND why. Suggestions explain why one approach is better than another — performance, clarity, or idiomatic Yinz. Every diagnostic AND every IDE tooltip answers three questions: WHAT happened (or is happening), WHAT to do instead (or how to make it explicit), and WHY. The compiler is not a checker — it's a senior developer mentoring a junior developer through every interaction.

The WHY must be **specific and contextual**, not generic. The compiler knows the types, variable usage, and surrounding code — it should use that knowledge. "Avoids allocation" is generic. "scores isn't used again after this line — sortInPlace() skips the allocation because you only need the sorted version" is contextual. Generic WHYs are a fallback for when context genuinely isn't available, not the standard.

**This rule applies to every teaching surface**, not just compiler diagnostics:

- Terminal compile errors and warnings
- IDE muted-text hints (inferred types, inferred ownership, inferred wait points — see `.claude/rules/inference.md`)
- IDE hover tooltips on muted text or any other annotated element
- Lint suggestions (typo corrections, style hints, performance hints)
- Doc-generation output (cargo doc-style or whatever Yinz ends up with)

**Shared wording rule**: one canonical explanation per concept, used wherever it surfaces. Don't write a different explanation for the same concept in the compiler vs the IDE tooltip vs the spec. The canonical form lives in one place (the rule file or design doc) and other surfaces re-use the same text.

**Canonical example** — hovering muted `.share` on a call passing a `const player`:

> **WHAT**: This is inferred as `.share` because `player` is declared `const`. The function gets read-only access; you keep ownership.
>
> **WHAT INSTEAD**: You could write `foo(player.share)` to make it explicit. The behavior is identical.
>
> **WHY**: `const` bindings can only grant read-only access. If you need mutation, declare `player` with `let` instead. (Trying to write `foo(player.lend)` here would produce a compile error: "cannot lend a const binding.")

*Why*: Learning happens at the moment of feedback. A developer who just hit an ownership violation is primed to understand ownership. A developer who just wrote `map<string, number>` with all-static keys is primed to learn why a `shape` is faster. That's the optimal moment to teach — not in a doc they read two weeks earlier, not after the bug ships. IDE hints extend that mentorship to every keystroke, not just error cases.

The teaching mission is a first-class language goal — see `design/teaching-mission.md` for the full rationale, the required three-part diagnostic format, and the long-term aspiration that Yinz becomes a CS-101 teaching language. The inference protocol that powers IDE hints lives in `.claude/rules/inference.md`.

---

**13. Capital letter = type. Everything else = lowercase.**
Scan any line of code. Capital letter = type. No capital = not a type. Modules are lowercase (`http`, `file`, `math`). Types are PascalCase (`Player`, `Response`, `Date`). Functions, variables, keywords — all lowercase. The same base name can exist in both casings: `Date` is the type, `date` is the module.

*Why*: Zero-cost scannability. Reading code is faster when the type system is visually encoded in casing. No context-reading required to distinguish a module call from a type annotation. Any pair of eyes — experienced or new — instantly knows what category they're looking at.

---

**12. Human-readable over programmer jargon**
`options` not `enum`. `follows` not `implements`. `nothing` not `void`. If a non-programmer could guess a keyword's meaning, the naming is right.

*Why*: The stated goal is accessibility to junior developers. Every term that requires CS background knowledge is a barrier to entry. Plain English words are memorable, guessable, and require no prior knowledge.

**Exception — union syntax uses `|`, not `or`**: Yinz writes union types as `shape Result = Success | Failure`, matching TypeScript convention. The `or` keyword was considered (and previously documented here) but rejected because `or` is triple-overloaded: boolean operator (`if (a or b)`), union type syntax, and the word in prose. `|` is unambiguous as a type-syntax symbol and reads naturally to TypeScript developers. Locked 2026-05-14 per Patrick's call during the design-lockdown conversation. This is the ONE place Yinz prefers a symbol over a word — every other operator/keyword stays as a word.
