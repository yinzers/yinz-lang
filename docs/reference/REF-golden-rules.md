---
name: "REF-golden-rules"
description: "The 13 rules with full reasoning. This file adds the 'why' behind each one."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-19"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Golden Rules — Extended Reference

The 13 rules with full reasoning. This file adds the "why" behind each one.

When two rules conflict, first find a design that satisfies both, usually by having the compiler do the work and show it. If none exists, the lower number wins. Readability against speed, and any safety opt-out for kernel code, need Chief's confirmation.

---

## Cross-cutting principle: Yinz is NOT object-oriented

Before you read the 13 rules, know this: **Yinz is data shapes plus standalone functions, with a dot-call shortcut.** It is not object-oriented.

- A shape holds data fields and, if you want, contract signatures. **A shape never holds function bodies.**
- Functions are declared on their own, at the top level of a file or module. `value.method()` is a shortcut for `method(value)`. Both forms are legal and mean the same thing.
- `extends` shares data fields only. A child shape gets its parent's fields. Behavior comes from standalone functions, and each function needs its own name.
- **There is no `override` keyword.** Choosing between same-named functions by argument type is planned but not built yet. Today, two functions with the same name in one file are a compile error, whatever their parameter types.
- `follows` is checked by matching function signatures. A shape follows a contract when standalone functions with matching signatures exist.
- `dynamic Foo` chooses the function at run time, through a table kept for each shape and contract pair. When the compiler knows the exact shape, it picks the function while compiling instead.

Values store no functions, so nothing extra is stored per value and ownership stays simple. The function is chosen while compiling unless you ask for `dynamic`. `player.heal(20)` compiles to the standalone function `heal(player, 20)`.

Where the 13 rules below say "method" or "dispatch", read it this way: a method is a function, and today the function is found by its name alone (each name is unique per file), or through the contract table for `dynamic Foo`.

---

**1. No garbage collector**
Ownership-based memory management only. One memory model, no two-tier complexity.

*Why*: A garbage collector (a background cleaner that frees memory for you) adds unpredictable pauses, often uses two to three times the memory, and gives you a second way of thinking: "I write the code, but something cleans up sometimes." Ownership is predictable and adds no cost while the program runs.

---

**2. Compile-time safety**
Every check that can run at compile time does. A failure that can't (file and network problems, bugs like division by zero) is explicit: `errors` or a task-isolated panic. No silent runtime failure.

*Why*: A crash in production at 3am costs far more than a compile error while you are writing the code. Every safety check that can move to compile time should.

---

**3. The compiler is a teacher**
Errors and IDE teaching hints explain what went wrong (or what's happening) AND why. Suggestions explain why one approach is better than another: speed, clarity, or the usual Yinz style. Every compiler message AND every IDE hint answers three questions: WHAT happened (or is happening), WHAT to do instead (or how to write it out), and WHY. The compiler is not just a checker. It acts like a senior developer coaching a junior one.

The WHY must be **specific to the code in front of you**, not generic. The compiler knows the types, how each variable is used, and the code around it, so it should use that. "Avoids allocation" is generic. "scores isn't used again after this line — sortInPlace() skips the allocation because you only need the sorted version" is specific. A generic WHY is a fallback for when the compiler has nothing more specific, not the standard.

**This rule applies to every teaching surface**, not just compiler messages:

- Compile errors and warnings in the terminal
- IDE muted-text hints (types, ownership words and wait points the compiler worked out for you)
- IDE hover tooltips on muted text or any other marked-up element
- Lint suggestions (typo fixes, style hints, speed hints)
- Generated documentation

**One wording per idea**: each concept has one standard explanation, used wherever it appears. Don't write one explanation for the compiler, another for the IDE tooltip, and a third for the docs. The standard wording lives in one place, and every other surface reuses it.

**Example** — hovering the muted `share` hint on a call that passes a `const player`:

> **WHAT**: The compiler chose `share` here because `player` is declared `const`. The function gets read-only access, and you keep ownership.
>
> **WHAT INSTEAD**: Nothing to type here — `share` is written only in a function's signature, never at the call. Click to jump to `foo`'s signature, where the `share` keyword is written (or can be added if the signature has none yet).
>
> **WHY**: A `const` variable can only give read-only access. If `foo`'s signature declared `lend` (borrow to change it) instead, passing this `const` variable would be a compile error: "`player` is `const` — `foo` needs to mutate it but `const` blocks mutation."

*Why*: People learn at the moment of feedback. A developer who just broke an ownership rule is ready to understand ownership. A developer who just wrote `map<string, number>` with keys that never change is ready to learn why a `shape` is faster. That is the best moment to teach — not in a doc read two weeks earlier, and not after the bug ships. IDE hints bring that coaching to every keystroke, not only to errors.

Teaching is a core goal of the language. For the full reasoning, the required three-part message format, and the long-term hope that Yinz becomes a language for learning to program, see [REF-teaching-mission.md](REF-teaching-mission.md).

---

**4. Self-documenting syntax**
Every keyword, function, and pattern should be readable by a junior developer who has never seen Yinz. If someone needs docs to understand what a line does, the design failed.

*Why*: A keyword you can guess beats one you must look up. `lend` in a signature says "this function borrows the value to change it" in one plain word. Every design decision should aim for guessable.

---

**5. Human-readable over programmer jargon**
`options` not `enum`. `follows` not `implements`. `nothing` not `void`. If a non-programmer could guess a keyword's meaning, the naming is right.

*Why*: The stated goal is to be welcoming to junior developers. Every term that needs computer-science background is a barrier. Plain English words are easy to remember and to guess, and need no prior knowledge.

**Exception — union syntax uses `|`, not `or`**: Yinz writes union types as `shape Outcome = Success | Failure`. `or` is a prose word, while `|` is unambiguous. Boolean operators are symbols too: `&&`, `||`, `!`.

---

**6. Zero-cost abstractions**
High-level syntax compiles to what a careful Rust/C++ programmer would write by hand. The features themselves cost what they inherently cost.

*Why*: If convenience costs speed, developers will skip the readable path in hot code. Zero-cost means the right choice and the fast choice are always the same choice.

**Clarification — this is not a relaxation**:

- "Zero-cost ABSTRACTIONS" means the abstraction adds no cost beyond hand-written code. It does NOT mean "no feature has any cost."
- A feature costs what it costs: counting how many owners share a value, scheduling background work, and bookkeeping for memory pools all have a price. You pay it whether you write that code by hand or the compiler writes it for you.
- Work the compiler does while compiling adds no cost while the program runs. When the compiler chooses `share` for a call, the machine code is identical to writing `share` in the function's signature yourself.
- The comparison is "what would a careful low-level programmer write by hand," not "the smallest cost anyone could imagine." Memory safety has a cost, and that cost is the bar, not zero.
- **A design that costs more than hand-written code needs Chief's confirmation, recorded in the commit and the feature's IMP design doc.** Rule 6 is not a softer "zero-cost".

Choices the compiler makes for you (sharing a value across threads with an owner count, waiting on slow calls) are not zero-cost violations: the cost belongs to the feature underneath, not to the compiler choosing it.

---

**7. Efficiency first, dynamic after**
The default behavior is always the most performant option. Developers opt INTO slower/flexible behavior explicitly.

*Why*: Junior developers don't know which choices are fast. If fixed arrays are the default, a map lookup is visibly a different type, and the compiler suggests shapes over maps, then beginner code is fast by default. Experienced developers can always opt in to dynamic behavior when they need it.

**Corollary — automatic upgrade**: when the compiler can prove that a stricter or faster form of your code fits how you actually use it, it uses that form automatically AND shows you the choice. Three things happen: (1) the compiled code silently uses the faster form, (2) a muted IDE hint shows what was chosen (when you could type the explicit form), (3) a lint suggestion teaches you to write the explicit form. Examples: an `array<T>` that never grows is compiled as `fixed<T>`; a `let` that is never reassigned is treated as `const`; hot field-access loops get a faster memory layout; a value shared across threads gets an owner count. Yinz promises to be fast by design, even for beginners, and this pattern delivers it. An upgrade with no teaching is magic, and a suggestion with no upgrade punishes laziness. Yinz does both.

---

**8. Compiler does the hard work**
Smart defaults everywhere. Type inference inside function bodies, operation fusion, optimal memory layout.

*Why*: Turning "what you write" into "what runs" should be the compiler's job. Developers shouldn't have to hand-tune speed that the compiler can work out itself.

---

**9. Step-by-step over chaining**
No method chaining. Each operation gets its own line with a named variable. The compiler merges sequential steps into a single fast pass.

*Why*: `players.filter(p => p.health > 0).sort(p => p.health).limit(10)` is hard to debug because you can't look at the middle. `let active = ...; let ranked = ...` gives every intermediate result a name and a place to inspect it. The compiler removes any speed difference.

---

**10. Capital letter = type. Everything else = lowercase.**
Scan any line of code. Capital letter = type. No capital = not a type. Modules are lowercase (`request`, `file`, `math`). Types are PascalCase (`Player`, `Response`, `Date`). Functions, variables, keywords — all lowercase. The same base name can exist in both casings: `Date` is the type, `date` is the module; `Request` is the type, `request` is the module.

*Why*: Zero-cost scannability. Reading code is faster when casing shows what a name is. No context-reading required to tell a module call from a type annotation. Any pair of eyes — experienced or new — instantly knows what category they're looking at.

---

**11. Dot-first design**
If something can be `.method()` with autocomplete, it should be. No cryptic symbols, no keywords to memorize.

*Why*: Autocomplete teaches the language. A developer who types `.` after a value sees every function whose first parameter matches that value. No documentation lookup needed. `player.heal(20)` (a shortcut for `heal(player, 20)`) shows up when you type `.` after a Player.

---

**12. Familiar syntax**
Borrow from TypeScript/JavaScript wherever possible.

*Why*: There are tens of millions of JS/TS developers. If they feel at home on day one, adoption starts from a much larger base. `let`, `const`, arrow functions, backtick interpolation — free familiarity.

---

**13. Fast to type**
Quick to write without sacrificing readability.

*Why*: Developer experience matters. A language that's pleasant to write gets used. `function` over `fn` is worth the extra characters because it's immediately readable — but ceremony that adds nothing (like `Optional<T>` vs `maybe T`) should be cut.
