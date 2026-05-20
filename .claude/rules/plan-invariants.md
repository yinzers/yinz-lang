# Plan Invariants Rule

This file governs what every milestone plan from **M4 onward** must declare.

---

## The Rule

> Every milestone plan from M4 onward must include a section titled exactly:
>
> `## Invariants This Milestone Must Preserve`
>
> with six required sub-sections:
>
> `### Safety` · `### Performance` · `### Teaching` · `### Runtime Dependencies` · `### Kernel-Mode Behavior` · `### Demo & Error Gallery`
>
> **From v0.2-M2 onward**, a seventh subsection is also required:
>
> `### Feature Registry Entries`
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

### `### Demo & Error Gallery`

Every phase that adds executable surface MUST extend two canonical files as part of its acceptance criteria:

1. **`examples/basics/src/entrypoint.ynz`** — the single growing demo project covering EVERY v0.1 language feature (M1–M8). Each phase adds the new feature in context (not as an isolated snippet). By the end of M8, this one project demonstrates the entire pre-stdlib language. After v0.1 ships, stdlib modules (v0.5+) get their OWN per-module example projects — but `examples/basics/` is the v0.1 showcase.

2. **`examples/errors/m{N}_errors.ynz`** — the per-milestone error gallery. Each phase that adds new compile-error classes adds intentional triggers to the milestone's gallery file. One run of the file produces every diagnostic Yinz can emit for that milestone (Yinz multi-errors up to 50/compile per `design/compiler-errors.md`, so a single file can demonstrate many simultaneously). Used for hands-on validation of the teaching diagnostic quality.

**Why this is a milestone invariant**: features that ship without hands-on demo + error-experience review go un-validated until users hit them. Patrick reviews each phase's UX via these files — without them, the language ships diagnostics nobody human has read. The two files are the human-eyes-on layer that automated tests can't replace.

**Required content per phase**:
- New executable feature → add to `examples/basics/src/entrypoint.ynz` showing the feature in a small but realistic context (not just `print(featureName())` — show it doing real work)
- New compile-error class → add to `examples/errors/m{N}_errors.ynz` as an intentional trigger with a `// WHY:` comment naming the diagnostic class
- Both files get `insta` stdout/stderr snapshots in the phase's verification step

**Deferred-feature handling**: features locked for v0.2+ (arenas per `design/future/arena.md`), v0.3+ (self-references per `design/future/self-references.md`, `verified { }` blocks per vocabulary.md), or later get a placeholder comment in `examples/basics/src/entrypoint.ynz` (`// arena scratch { ... } — v0.2 feature, see design/future/arena.md`) until they ship. When they ship, they get added to the demo for real.

**Cross-reference to project CLAUDE.md**: this requirement is also stated in `<project>/CLAUDE.md` "When Working on This Project" so plans drafted in fresh chats see the requirement immediately.

### `### Feature Registry Entries` *(applies to plans from v0.2-M2 onward — v0.2-M1 is exempt as the plan creating this rule)*

Every plan from v0.2-M2 onward that adds a new language keyword, banned-jargon word, primitive method, type-attached constant, reserved/deferred feature, diagnostic template, or muted-hint domain MUST enumerate the registry entries it adds or modifies.

**Required content per entry type:**

- For new entries: `[[entry_kind]] name = "<name>"` — one line per entry, listing the kind and name.
- For modified entries: `modify [[entry_kind]] name = "<name>"` — include which fields change and why.
- For entries the plan explicitly DOES NOT add (e.g., "this plan adds no new keywords"): state that explicitly so reviewers know it was considered.

**Examples (for a plan that adds a new keyword `verify` in v0.2-M3):**
```
### Feature Registry Entries
- New `[[keyword]]` entry: `verify` (token = `Verify`, since = "v0.2")
- New `[[banned_declaration_keyword]]` entry: `assert` (redirects to `verify`)
- No new banned_jargon, primitive_intrinsic, type_attached_constant, deferred_* entries for this milestone.
```

**Why this is a mandatory subsection**: the feature registry SSOT discipline (v0.2-M1) requires that adding a feature to the compiler and adding it to the registry happen in the SAME plan. Without this subsection, plans drift — the code ships but the registry entry is forgotten until the LSP tries to read it (v0.2-M2) and gets an incomplete autocomplete list or stale muted-hint domain.

**Enforcement**: `.claude/graveyard.md` "missing-feature-registry-subsection" Bouncer entry (to be added after v0.2-M2 ships the first plan under this rule — until then, checked at plan-review time).

---

## Enforcement

This rule is enforced mechanically by the Bouncer:

- `.claude/graveyard.md` Entry 1 catches M4+ plans missing the const-deep-immutability invariants in `### Safety`
- `.claude/graveyard.md` Entry 3 catches M4+ plans missing any of the (now 7) required sub-sections
- `.claude/graveyard.md` Entry 4 catches plans that touch `crates/**` without declaring runtime dependencies and kernel-mode behavior
- `### Demo & Error Gallery` subsection: future Bouncer entry catches plans that touch `crates/**` without including the `examples/basics/` + `examples/errors/` extension obligations (entry to be added once the first M4+ plan ships under the rule — until then, this requirement is checked at plan-review time)
- `### Feature Registry Entries` subsection: future Bouncer entry catches plans that add language features without enumerating registry entries (to be added after v0.2-M2 ships)

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
