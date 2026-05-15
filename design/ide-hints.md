# IDE Hints — Muted-Text Protocol

User spec: none — this is an internal IDE behavior specification for the v0.2 LSP.

**Status**: locked. v0.2 LSP implements against this spec.

---

## What

The IDE hint protocol is the user-facing teaching surface for everything the compiler figures out automatically. Per the [uniform inference rule](../.claude/rules/inference.md), if the compiler can determine a semantic from context, the developer doesn't have to type it — and the IDE shows what was determined as **muted text** the developer can read, click to make explicit, and hover for a three-part explanation.

This is the protocol every Yinz LSP implementation must follow.

---

## Why this is load-bearing

The teaching mission (`design/teaching-mission.md`) depends on this protocol. Without muted hints, proactive teaching (showing what the compiler decided on valid code) doesn't exist — only reactive teaching (compile errors) remains. Reactive teaching catches misunderstandings AFTER the developer makes them. Proactive teaching prevents them in the first place.

The protocol must be locked NOW (v0.1) so v0.2 LSP work has a target to implement against, not a moving requirement.

---

## What gets hinted

Every semantic the compiler figures out from context gets a muted hint:

| Domain | Example source | Muted hint shows |
|---|---|---|
| Variable type inference | `let x = 42` | `: int` after `x` |
| Function param types (where context allows) | depends on call site | the figured-out type |
| Ownership at call sites | `foo(player)` | `.share` or `.lend` after `player` |
| Wait points on I/O | `db.fetch()` | `wait` keyword before the call |
| Lifetimes | always figured out | the lifetime (only shown on request — usually hidden) |
| Allocator | inside `arena scratch { let a: array<int> = [] }` | `.in(scratch)` after the constructor |
| Implicit copy points | trivially-copyable values | `.copy` where the copy happens |

This list extends as new compiler-figured-out semantics emerge (effect annotations, capability tracking, etc.). New entries follow the same protocol — no exceptions.

---

## Visual styling

Two tiers of muted text, distinguished by danger:

- **Neutral muted (gray)**: benign hints — type inference, lifetime inference, allocator inference. These add NO new behavior; they just show what's already happening.
- **Cautionary muted (red-tinted)**: hints involving mutation, ownership transfer, or thread crossing. Examples: `.lend` on a `let` binding (mutation happens through this call), `.give` (ownership transfer is permanent), auto-`Arc` for cross-thread shared state (reference counting added).

The styling is part of the teaching: cautionary hints visually flag "something more is happening here than just inference; pay attention." Compile errors use a third, completely separate styling (red squiggly + error panel) and are NEVER expressed as muted hints.

---

## Hint text must mirror typeable syntax

The muted text MUST complete to syntactically-valid Yinz the developer COULD have typed. Click-to-make-explicit produces real code.

Example:

```yinz
foo(player)                   // muted ".share" after player
                              // click → becomes foo(player.share)
                              // which is valid Yinz and equivalent
```

The hint is what the dev would have typed if they typed everything. It is NOT an arbitrary annotation that doesn't appear in the source language.

---

## Hover tooltip format

Every muted hint, on hover, shows a three-part tooltip in the same WHAT / WHAT-INSTEAD / WHY format Golden Rule 11 requires for compiler diagnostics.

### Canonical example — `.share` on a `const` binding

Source:
```yinz
const player = Player { name: "Patrick", health: 100 }
foo(player)               // muted ".share" appears after player
```

Hover tooltip on the muted `.share`:

> **WHAT**: This is figured out as `.share` because `player` is declared `const`. The function gets read-only access; you keep ownership.
>
> **WHAT INSTEAD**: You could write `foo(player.share)` to make it explicit. The behavior is identical.
>
> **WHY**: `const` bindings can only grant read-only access. If you need mutation, declare `player` with `let` instead. (Trying to write `foo(player.lend)` here would produce a compile error: "cannot lend a const binding.")

### Shared wording rule

The same canonical text MUST be used wherever this concept surfaces:
- Compiler error: "cannot lend a const binding"
- IDE tooltip on the muted `.share`: explains WHY const can't be lent
- Spec example in `spec/ownership.md` explaining const's behavior at call sites
- Design rationale in `design/ownership.md` `const Deep Immutability` section

One canonical explanation lives in one source (typically the rule file or design doc) and every surface re-uses it. This prevents the "doc says X, error says Y, tooltip says Z" drift that plagues every IDE-language ecosystem.

---

## Configuration

The user can toggle hints globally and per-domain:

- **All hints off** — text-only editing mode (advanced users who know what the compiler will do)
- **All hints on** — full teaching mode (default for new users)
- **Per-domain toggle** — e.g., hide type hints once you've internalized type inference, keep ownership hints because mutation visibility is still valuable

These are LSP-level user preferences, not language-level decisions. The protocol defines what hints exist and how they behave; the user chooses which ones to see.

---

## What this protocol is NOT

- **Not a comment system**: hints are LSP-rendered, not part of source files. Two developers viewing the same file see hints based on THEIR LSP settings, not stored in the file.
- **Not auto-completion**: completion suggests what you could type; hints show what was figured out. Different mechanisms.
- **Not arbitrary annotations**: hints must be valid Yinz syntax (per "mirror typeable syntax" above). The LSP cannot invent display strings that wouldn't compile.
- **Not a replacement for errors**: compile errors stay in the error panel with standard error styling. The hint protocol is for VALID code where the compiler made a decision the user can learn from.

---

## v0.2 LSP implementation notes (forward-compat)

- The LSP must expose hints via the standard LSP `textDocument/inlayHint` capability so any LSP-compliant editor can render them.
- Hint kind: `Type` for type/lifetime/allocator hints; need a custom kind or use `Other` for ownership/wait hints (LSP spec has limited kinds; verify against current LSP version when implementing).
- Tooltip text must be generated by the SAME diagnostic-rendering code as compiler errors so the WHY field stays in sync.
- Performance: hints update on every keystroke; the LSP must do incremental computation per Salsa queries, not full-program re-analysis.

---

## Cross-references

- `.claude/rules/inference.md` (the uniform inference rule this protocol implements)
- `design/golden-rules.md` Rule 11 (extended to all teaching surfaces)
- `design/teaching-mission.md` "IDE as a Teaching Surface" section
- `design/compiler-errors.md` (the three-part diagnostic format hints inherit)
- `design/future/index.md` (where this file sits in the future-list ordering, even though `design/ide-hints.md` is the protocol itself, not in `design/future/`)
