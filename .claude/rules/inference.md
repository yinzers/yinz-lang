# Inference Rule — Compiler Infers, IDE Teaches

This file governs how Yinz handles the work the compiler can figure out automatically vs. what the developer types.

---

## Dual-Audience Disclaimer (READ FIRST)

This rule file uses the words `infer`, `inferred`, and `inference` deliberately. **These words are banned in user-facing compiler diagnostics** per `crates/ynz-diagnostics/src/banned_jargon.rs` and `design/compiler-errors.md` — those audiences need plain-English replacements ("figure out automatically", "the compiler can tell").

The divergence is intentional and load-bearing:

| Audience | Vocabulary |
|---|---|
| Engineers reading this rules file, design docs, internal spec notes | `infer` / `inference` is correct |
| End users reading a Yinz compile error in their terminal | `infer` / `inference` is BANNED — use "figure out automatically" etc. |

**Do not "fix" the divergence by renaming this file or stripping `infer`/`inference` from banned-jargon.** They serve different audiences. If a future audit flags this file for using "inference," cite this disclaimer and move on.

---

## The Rule

> **If the compiler can figure it out from context, the developer doesn't type it. The IDE shows the inferred information as muted text with a hover-tooltip explaining what was inferred AND why. The muted hint completes to syntactically-valid Yinz the developer COULD have typed.**

Click-to-make-explicit must produce real Yinz syntax. The muted hint is what the dev would have typed if they typed everything.

---

## Domains This Applies To

The compiler infers across these domains; the IDE shows muted hints in all of them:

| Domain | Example | Muted hint shows |
|---|---|---|
| Variable types | `let x = 42` | `: int` after `x` |
| Function param types (where context allows) | TBD per case | the inferred type |
| Ownership at call sites | `foo(player)` | `.share` or `.lend` based on signature |
| Wait points on I/O | `db.fetch()` | `wait` keyword before the call |
| Lifetimes | always inferred | the lifetime (only shown on request) |
| Allocators | `let temp = array<int>()` inside an arena block | the active arena |
| Copy points (trivially-copyable types) | implicit copy | `.copy` where the copy happens |

If a new domain emerges (e.g., effect annotations, capability tracking), it joins this list.

---

## Muted-Text Styling Rules

Two visual tiers of muted text:

- **Neutral muted (gray)**: benign inference — type inference, lifetime inference, allocator inference. Hover-tooltip explains what and why.
- **Cautionary muted (red-tinted)**: inference involving mutation, ownership transfer, or thread crossing. Examples: `.lend` on a `let` binding, `.give`, auto-`Arc` for cross-thread shared state. Same hover format; visual styling flags the higher-stakes inference.

**Compile errors are NOT muted hints.** Errors use standard error styling (red squiggly + diagnostic in error panel). The two styles are separate — never collapse them.

---

## Hover Tooltip Format

Every muted hint, on hover, gives a three-part explanation matching Golden Rule 11's WHAT / WHAT-INSTEAD / WHY format:

- **WHAT** is happening here (what the inferred thing means)
- **WHAT INSTEAD** the developer could write to make it explicit
- **WHY** the compiler chose this (with the contextual reason — not generic, tied to THIS call site)

Canonical example — hovering muted `.share` on a call passing a `const player`:

> **WHAT**: This is inferred as `.share` because `player` is declared `const`. The function gets read-only access; you keep ownership.
>
> **WHAT INSTEAD**: You could write `foo(player.share)` to make it explicit. The behavior is identical.
>
> **WHY**: `const` bindings can only grant read-only access. If you need mutation, declare `player` with `let` instead. (Trying to write `foo(player.lend)` here would produce a compile error: "cannot lend a const binding.")

Same wording reused wherever this concept surfaces — compiler diagnostics, hover tooltips, spec examples. One canonical explanation per concept.

---

## Inverse Anti-Pattern (DO NOT DO)

**Requiring explicit ownership/type annotation at call sites that could be inferred is the inverse of this rule and is wrong.**

Examples of the anti-pattern:
- "Callers MUST write `.share` at every call site"
- "Type annotations REQUIRED on every variable"
- "All wait points must be typed explicitly"

If you see this language in a PR, spec, or design doc, treat it as a regression. The graveyard has an entry for this — Bouncer will warn on diffs that introduce it.

**Function signatures are different.** Signatures explicitly declare `share`/`lend`/`give` for the same reason Rust signatures do — the function's contract is visible at the definition. That's CORRECT. The anti-pattern is requiring annotation at CALL sites where the signature already tells the compiler what's needed.

---

## Why This Rule Is Load-Bearing

The Yinz teaching mission depends on it:
- Forcing explicit syntax everywhere creates noise; developers learn to ignore it.
- Hiding inference entirely creates magic; developers can't reason about what the compiler does.
- **Muted hints split the difference**: the dev sees what the compiler decided, can click to make it real, and over time learns the rules by reading the hints.

It also depends on IDE quality. The hints aren't a nice-to-have — they're the central teaching mechanism. v0.2 LSP work implements them per `design/ide-hints.md`.

---

## Cross-References

- `design/golden-rules.md` Rule 11 (the compiler is a teacher — extended to IDE surfaces)
- `design/teaching-mission.md` (IDE as teaching surface)
- `design/ide-hints.md` (the protocol spec — v0.2 LSP implementation target)
- `design/compiler-errors.md` (banned-jargon list for user-facing diagnostics, distinct from this internal-vocabulary file)
- `.claude/rules/vocabulary.md` (official Yinz user-facing terms)
- `.claude/graveyard.md` Entry 2 (inverse anti-pattern: required explicit annotation at call sites)
