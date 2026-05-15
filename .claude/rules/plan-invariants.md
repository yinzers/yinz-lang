# Plan Invariants Rule

This file governs what every milestone plan from **M4 onward** must declare.

---

## The Rule

> Every milestone plan from M4 onward must include a section titled exactly:
>
> `## Invariants This Milestone Must Preserve`
>
> with five required sub-sections:
>
> `### Safety` · `### Performance` · `### Teaching` · `### Runtime Dependencies` · `### Kernel-Mode Behavior`
>
> Each sub-section lists testable assertions, not vague aspirations.

Pre-M4 milestone plans (M1, M2, M3) are exempt — the rule kicks in for M4 (types + ownership) where the invariants get load-bearing. M3 has a retroactive partial Invariants section per the `design-lockdown-from-gemini-review` plan as proof-of-concept.

---

## What Goes In Each Sub-Section

### `### Safety`

What memory-safety, type-safety, and ownership guarantees must hold after this milestone ships? Each guarantee is a testable assertion.

**Examples (for M4 types + ownership)**:
- `const` bindings cannot be reassigned
- `const` bindings cannot be lent for mutation (`.lend` rejected at compile time)
- `const` bindings cannot be given away (`.give` rejected at compile time)
- `const` bindings cannot have their fields mutated
- A `.give`'d value cannot be used afterward (use-after-give caught at compile time)

**NOT examples** (too vague):
- "Memory is safe" — what does that mean? How would you test it?
- "Ownership works correctly" — too generic to enforce.

### `### Performance`

What codegen properties or compile-time guarantees must hold? Includes LLVM attribute emission, monomorphization, optimization-pass requirements, AND auto-promotion analysis (per `.claude/rules/auto-promotion.md`).

**Examples (for M4)**:
- Function parameters with `share` declaration emit LLVM `readonly` attribute
- Function parameters with `lend` declaration emit LLVM `noalias` + writable
- `const` bindings passed to functions emit `readonly` regardless of explicit annotation
- Field access on a `shape` value compiles to a direct memory offset (no indirection)

**Auto-promotion analysis (mandatory subsection from M4 onward)**:

For each new feature, stdlib type, or compiler optimization the milestone introduces, the plan MUST answer:
- Is there a stricter or faster form the compiler could prove fits in some cases?
- If yes: which surfaces apply (codegen auto-promotion / muted IDE hint / Tier 3 lint suggestion)?
- What's the lint rule name (convention: `prefer-X-when-Y`)?
- What does the muted hint render inline (must be informative-at-a-glance per `.claude/rules/inference.md`)?
- What's the hover tooltip text (must follow WHAT/WHAT-INSTEAD/WHY per Golden Rule 11)?

If a feature has no auto-promotion candidates, state that explicitly so reviewers know it was considered, not forgotten. Full project-creation checklist lives in `.claude/rules/auto-promotion.md`.

This subsection is mandatory because Yinz's "fast by design even for inexperienced developers" positioning depends on consistently applying the auto-promotion pattern. A feature that ships without considering auto-promotion candidates either leaves perf on the table OR creates the inverse anti-pattern (user must opt in to the fast form). Either failure mode is structural, not cosmetic.

### `### Teaching`

What new diagnostics or IDE-hint patterns does this milestone introduce, and do they follow the three-part format?

**Examples (for M4)**:
- Diagnostics for cannot-lend-const, cannot-give-const, cannot-mutate-field-of-const all follow WHAT/WHAT-INSTEAD/WHY format
- IDE muted hint for `.share`/`.lend` at call sites is implemented (or scheduled with explicit cross-reference)
- No new banned-jargon words slip into user-facing errors (audited by `tests/jargon_audit.rs`)

### `### Runtime Dependencies`

What does this milestone's code DEPEND ON at runtime? Heap allocator? Scheduler? OS file I/O? None?

**Examples (for M4)**:
- `shape` declarations: none (compile-time only)
- Heap-allocating instances of `shape Foo { ... }`: requires malloc (libc) — kernel-mode users must provide custom allocator
- `array<T>` field on a shape: requires malloc; same kernel-mode story

**Why this matters**: kernel-mode support (v0.3+) depends on every feature declaring what it requires. Without declarations, we ship features that secretly need an allocator and break in kernel mode.

### `### Kernel-Mode Behavior`

For each runtime dependency listed above: what is the behavior in `--kernel` mode? Compile error? Works with user-provided primitive? Always works?

**Examples (for M4)**:
- `shape` declarations: always work in `--kernel` mode (compile-time only)
- Heap-allocating shape instances: COMPILE ERROR in `--kernel` mode unless user provides an allocator via `... .in(myKernelAllocator)`
- Error message format: WHAT/WHAT-INSTEAD/WHY pointing to `design/future/no-runtime-mode.md` for the plug-in allocator API

---

## Enforcement

This rule is enforced mechanically by the Bouncer:

- `.claude/graveyard.md` Entry 1 catches M4+ plans missing the const-deep-immutability invariants in `### Safety`
- `.claude/graveyard.md` Entry 3 catches M4+ plans missing any of the 5 sub-sections
- `.claude/graveyard.md` Entry 4 catches plans that touch `crates/**` without declaring runtime dependencies and kernel-mode behavior

Bouncer checks are runnable shell commands. False-positives are fixed by tightening the regex in the graveyard entry, not by exempting the plan.

---

## Why This Rule Exists

The Gemini code review on 2026-05-14 flagged that the M3 plan said "ownership system" without enumerating what `const` blocks at call sites or which LLVM attributes get emitted. The gap WAS real and would have shipped a less-safe + less-performant M4 if not caught. The rule prevents recurrence.

See `.claude/plans/active/design-lockdown-from-gemini-review.md` for the originating incident and the locked decisions.

---

## Cross-References

- `~/.claude/skills/plan/SKILL.md` (the global /plan skill — its plan template can/should be extended to include this section by default once /plan is project-aware)
- `.claude/graveyard.md` (Entries 1, 3, 4 enforce this rule)
- `.claude/plans/active/v0-1-compiler.md` `## Forward-Compatibility Constraints` (cites this rule)
- `design/future/no-runtime-mode.md` (defines the kernel-mode behavior the rule references)
