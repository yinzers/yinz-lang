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

> **If the compiler can figure it out from context, the developer doesn't type it. The IDE shows the inferred information as muted text with a hover-tooltip explaining what was inferred AND why. Click action: where the muted hint corresponds to typeable Yinz syntax, click-to-make-explicit inserts that source; where the inference is purely informational (no typeable equivalent), click jumps to the relevant declaration site instead.**

Most domains complete to real Yinz syntax (type annotations, allocator placement, `wait` insertion, `let`→`const` rewrite). The exception is ownership at call sites: those modifiers exist only in function signatures — there is no body-level syntax to insert. The muted hint there is informational only; click jumps to the function signature where the modifier IS visible (or can be made explicit if the signature was bare).

---

## Domains This Applies To

The compiler infers across these domains; the IDE shows muted hints in all of them:

Each row shows the muted text the IDE renders directly inline. The text is informative-at-a-glance: enough context for the dev to understand WHAT was inferred and WHY without hovering, while staying concise enough not to clutter the editor. Hover tooltips go deeper (full WHAT/WHAT-INSTEAD/WHY per Golden Rule 11 — see "Hover Tooltip Format" below).

| Domain | Example source | Muted hint rendered inline (informative at a glance) |
|---|---|---|
| Variable types | `let x = 42` | `: int (from 42)` after `x` — shows the type AND what the compiler inferred it from |
| Function param types (where context allows) | `foo(x => x + 1)` where `foo: (int) -> int` is known | `: int` on `x` — shows the type the call site requires |
| Ownership at call sites | `foo(player)` where `player` is `const` and signature is `share` | `share (read-only — matches foo's signature)` after `player` — shows the modifier AND why this one was picked. **Informational category** (no body-level syntax to insert — call-site ownership modifiers don't exist as Yinz source; only signatures carry the modifier; the muted hint is purely teaching). Click jumps to foo's signature. |
| Ownership at call sites — mutation | `bar(player)` where `player` is `let` and signature is `lend` | `lend (function will mutate — see bar's signature)` after `player` — same pattern, cautionary styling. **Informational category** (same rationale as above). |
| Wait points on I/O | `db.fetch("users")` | `wait (db.fetch may suspend on I/O)` before the call — shows the keyword AND why suspension happens here |
| Lifetimes | always inferred (only shown on user request) | `'request_scope` — shows the lifetime; on request because lifetime hints are usually noise |
| Allocators | `let temp: array<int> = []` inside `arena scratch { ... }` | `.in(scratch) — current arena` after the constructor — shows the allocator AND that it's the active scope's arena |
| Copy points (trivially-copyable types) | implicit copy at a call site, e.g., passing `let n: int` to two functions | `.copy (8 bytes, trivially copyable)` — shows the action AND why it's free |
| `array<T>` → `fixed<T>` promotion | `let nums: array<int> = [1, 2, 3]` (never grown) | `// promoted to fixed<int, 3> — never grown` after the binding; click rewrites annotation to `fixed<int>` |
| `let` → `const` (binding never reassigned/mutated/lent) | `let count = 5` (never written after) | `// effectively const — never reassigned` after the binding; click rewrites `let` to `const` |

If a new domain emerges (e.g., effect annotations, capability tracking), it joins this list. Each new entry must follow the same "informative at a glance" pattern: show what the compiler decided AND a one-clause "why" that the dev can read without hovering.

---

## Two Surfaces for the Same Decision (Hybrid Model)

Some auto-promotions surface in BOTH the muted-hint protocol (this file) AND the Tier 3 lint protocol (`design/linting.md`). They're different surfaces with different teaching jobs:

| Surface | Purpose | Always-on? | Action required? |
|---|---|---|---|
| **Muted hint** (this protocol) | Show what the compiler decided. Click to make explicit produces typeable source. | Yes — every keystroke | No — informational |
| **Tier 3 lint suggestion** (yellow squiggle) | Teach best practice. Recommends user rewrite for source-level clarity. | Yes — visible in IDE + compile output | Suggestion — user can ignore |

For `array<T>` → `fixed<T>` and `let` → `const`, BOTH apply: the muted hint shows the inference happened AND the lint suggestion teaches the explicit-form best practice. Both pay off:
- The muted hint surfaces the perf decision the compiler made (so the user knows the optimization happened, doesn't think they're paying for the slower form).
- The lint suggestion teaches the explicit form (so reviewers see intent in source and a future `.add()` becomes a compile error rather than a silent codegen change).

### Auto-promotions that get BOTH surfaces

The criterion for using both is: **the explicit form is typeable Yinz syntax**. If the user could write the stricter form in source by hand, both surfaces apply.

- `array<T>` → `fixed<T>` ✓ (`fixed<T>` IS typeable)
- `let` → `const` ✓ (`const` IS typeable)

### Auto-promotions that get ONLY the Tier 3 lint surface

When the explicit form has no typeable syntax, the muted-hint protocol does NOT apply (it requires click-to-make-explicit to produce real Yinz). Use Tier 3 lint suggestion alone in those cases.

- **Auto-SoA layout transform** (`design/future/auto-soa.md`): no source-level syntax for SoA exists. Tier 3 lint suggestion only; no muted hint.

### Rule of thumb

Compiler made a decision the user could have made themselves in source → both surfaces. Compiler made a decision with no equivalent user-typeable form → Tier 3 lint only.

See `design/collections.md` "Auto-promotion: `array<T>` → `fixed<T>`" section for the canonical hybrid-model rationale.

---

## Three Placement Categories — Each Has Exactly One Correct Surface

Muted hints don't all use the same IDE surface. The category determines which surface the LSP MUST render. **Mixing surfaces within a category is forbidden** — consistency is what makes the protocol learnable.

### Test for which category applies

Ask: "could the user have typed something in source to make this explicit?"
- **At a specific position** → Addition
- **By replacing existing text** → Replacement
- **No, it's a pure compiler/runtime choice** → Informational

Pick once at design time. The category locks the surface.

### 1. Addition — in-position muted text

User could have typed the explicit form IN A SPECIFIC POSITION but didn't. The muted text appears at that exact position. Click → the text gets typed into source.

```ynz
let i = 4                              // muted `: int` between `i` and `=`
foo(player)                            // muted `.share` after `player`
let queue = channel<Order>()           // muted `64` INSIDE the empty parens
db.fetch("users")                      // muted `wait` BEFORE the call
arena scratch { let temp = array<int>() }   // muted `.in(scratch)` after the constructor
```

Zero source-vs-render ambiguity. Click-to-make-explicit is trivial — the muted bytes get inserted at the position they're rendered. Visibility is passive (you see the info while reading the file; no hover needed).

### 2. Replacement — visual decoration on existing token + hover (LOCKED: Option D)

User picked one form, compiler picked another. The explicit form would REPLACE source bytes, not add to them.

Examples:
- `array<T>` → `fixed<T>` promotion: source has `let nums: array<int> = ...`, alternative form is `fixed<int, 3>`
- `let` → `const`: source has `let x = 5`, alternative form is `const`

**Locked rendering**: the existing token (`let`, `array<T>`) gets a **visual decoration** — dotted underline, subtle color shift, or small marker — indicating "alternative form available." The alternative form lives in the **hover tooltip**, not as inline text.

```ynz
let count = 5
// IDE renders `let` with a dotted underline.
// Hover on `let`:
//   WHAT: This `let` is treated as `const` because count is never reassigned/mutated/lent.
//   WHAT INSTEAD: Click to convert `let` → `const` in source.
//   WHY: const enables stronger compiler optimizations (readonly attribute) and signals
//        intent to readers; auto-promote already happened so no perf change.
```

**Why decoration + hover, NOT inline-replace, NOT side-by-side bracket annotation, NOT comment-after**:
- Inline-replace breaks cursor positioning (where does the cursor go in `let` vs `const`?) and creates "what I see ≠ what's in the file" confusion
- Side-by-side bracket (`let [const] = 5`) looks like double-keyword syntax that doesn't exist
- Side-by-side at end of line (`let count = 5  [const]`) puts the annotation far from what it modifies; gets lost on long lines
- Comment-style is too much for one-keyword info — adds visible width without proportionate teaching value
- Decoration matches existing IDE conventions (deprecated APIs, unused imports, lint squiggles); users already know "decoration = look here"

**Fallback for editors without decoration support** (rare in 2026 — bare `vim` without LSP, etc.): the IDE may fall back to end-of-line annotation `let count = 5  [const]` or simply omit the hint. Bare-text editors (`cat`, `less`) see only the source bytes, which is correct — they're not rendering anything anyway.

Visibility is ACTIVE (requires hover). For replacements, this is the right tradeoff — the info is one keyword and the user can scan for decorations.

### 3. Informational — comment-style passive annotation

The compiler made a decision that has NO equivalent source form the user could have written. The annotation appears as a muted comment near the relevant expression.

```ynz
findMax(players)              // muted: // static dispatch (T = Player) — .compare() inlined
background process(data)      // muted: // routed to CPU pool — no may-block calls in call graph
```

There's no Yinz syntax for "static dispatch" or "route to CPU pool" — these are codegen and scheduler choices, not typeable forms. Comment-style is the only protocol-compatible surface.

**Why comment-style for informational, NOT decoration-only**:
- Informational decisions carry MULTI-CLAUSE info ("static dispatch (T = Player) — .compare() inlined, ~1 cycle")
- Decoration-only would require ACTIVE hover to discover any of that text
- The teaching mission depends on PASSIVE visibility — the dev learns by reading their own annotated code
- Replacement's one-keyword info fits in a tooltip; informational's multi-clause info does not

Visibility is PASSIVE. The whole point is the dev sees the compiler's reasoning while reading.

### Consistency rule — one category, one surface

If you propose a new inference domain, pick ONE category and use ONLY that category's surface. Don't mix (e.g., "use comment-style sometimes and decoration other times for the same domain"). The whole point of these three categories is the LSP implementer can write one renderer per category and apply it everywhere.

If a domain seems to fit multiple categories, the test in "Test for which category applies" above resolves it. Any genuine ambiguity gets escalated and the rule gets clarified — not papered over with mixed surfaces.

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
