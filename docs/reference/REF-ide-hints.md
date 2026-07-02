---
name: "REF-ide-hints"
description: "Canonical spec for the muted-text IDE hint protocol showing what the compiler inferred, its click-to-explicit and hover-tooltip behavior, and the v0.2 LSP implementation target."
tags:
  - "yinz-compiler"
created_at: "2026-05-14"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# IDE Hints — Muted-Text Protocol

User spec: none — this is an internal IDE behavior specification for the v0.2 LSP.

**Status**: locked. v0.2 LSP implements against this spec.

---

## What

The IDE hint protocol is the user-facing teaching surface for everything the compiler figures out automatically. Per the [uniform inference rule](../../.claude/rules/inference.md), if the compiler can determine a semantic from context, the developer doesn't have to type it — and the IDE shows what was determined as **muted text** the developer can read, click to make explicit, and hover for a three-part explanation.

This is the protocol every Yinz LSP implementation must follow.

---

## Why this is load-bearing

The teaching mission ([`docs/reference/REF-teaching-mission.md`](REF-teaching-mission.md)) depends on this protocol. Without muted hints, proactive teaching (showing what the compiler decided on valid code) doesn't exist — only reactive teaching (compile errors) remains. Reactive teaching catches misunderstandings AFTER the developer makes them. Proactive teaching prevents them in the first place.

The protocol must be locked NOW (v0.1) so v0.2 LSP work has a target to implement against, not a moving requirement.

---

## What gets hinted

Every semantic the compiler figures out from context gets a muted hint:

| Domain | Example source | Muted hint shows |
|---|---|---|
| Variable type inference | `let x = 42` | `: int` after `x` |
| Function param types (where context allows) | depends on call site | the figured-out type |
| Ownership at call sites (informational only — no body-level syntax) | `foo(player)` | `share` or `lend` keyword after `player` — purely teaching; click jumps to foo's signature where the modifier IS declared |
| Wait points on I/O | `db.fetch()` | `wait` keyword before the call |
| Lifetimes | always figured out | the lifetime (only shown on request — usually hidden) |
| Allocator | inside `arena scratch { let a: array<int> = [] }` | `.in(scratch)` after the constructor |
| Implicit copy points | trivially-copyable values | `.copy` where the copy happens |

This list extends as new compiler-figured-out semantics emerge (effect annotations, capability tracking, etc.). New entries follow the same protocol — no exceptions.

---

## Visual styling

Two tiers of muted text, distinguished by danger:

- **Neutral muted (gray)**: benign hints — type inference, lifetime inference, allocator inference. These add NO new behavior; they just show what's already happening.
- **Cautionary muted (red-tinted)**: hints involving mutation, ownership transfer, or thread crossing. Examples: inferred `lend` at a call site (mutation happens through this call), inferred `give` (ownership transfer is permanent), auto-`Arc` for cross-thread shared state (reference counting added).

The styling is part of the teaching: cautionary hints visually flag "something more is happening here than just inference; pay attention." Compile errors use a third, completely separate styling (red squiggly + error panel) and are NEVER expressed as muted hints.

---

## Hint text + click action

Most muted hints complete to syntactically-valid Yinz the developer COULD have typed — click-to-make-explicit inserts that source. The exception is ownership-at-call-sites, which is purely informational (no body-level syntax exists for `share`/`lend`/`give` — those modifiers only appear in signatures); click there JUMPS to the function's signature instead.

Example (typeable hint — Addition category):

```ynz
let x = 42                    // muted ": int (from 42)" after x
                              // click → becomes `let x: int = 42`
```

Example (informational hint — Ownership at call sites):

```ynz
const player: Player = { name: "Patrick", health: 100 }
foo(player)                   // muted "share (matches foo's signature)" after player
                              // click → IDE jumps to foo's signature where `share` is declared
                              // (or could be made explicit if foo's signature was bare)
```

The hint always documents what's happening. The click action varies by category — see [`.claude/rules/inference.md`](../../.claude/rules/inference.md) for the three categories (Addition, Replacement, Informational).

---

## Hover tooltip format

Every muted hint, on hover, shows a three-part tooltip in the same WHAT / WHAT-INSTEAD / WHY format Golden Rule 11 requires for compiler diagnostics.

### Canonical example — inferred `share` on a `const` binding at a call site

Source:
```ynz
const player: Player = { name: "Patrick", health: 100 }
foo(player)               // muted "share (matches foo's signature)" appears after player
```

Hover tooltip on the muted `share`:

> **WHAT**: The compiler inferred `share` at this call site. `foo`'s signature declares its parameter as `share`, and `player` is `const` (which can only be shared, never lent or given). The function gets read-only access; you keep ownership.
>
> **WHAT INSTEAD**: To see the contract, view `foo`'s signature — `function foo(share p: Player)`. There is no body-level syntax for ownership modifiers at call sites; the signature is the explicit form.
>
> **WHY**: `const` bindings can only grant read-only access. If you need a function that mutates `player`, declare `player` with `let` AND change `foo`'s signature to `lend p: Player`. (Trying to call a function whose signature is `lend p: Player` with a `const` player here would produce a compile error: "cannot infer lend for a const binding.")

### Shared wording rule

The same canonical text MUST be used wherever this concept surfaces:
- Compiler error: "cannot infer lend for a const binding"
- IDE tooltip on the muted inferred-`share`: explains WHY const can't be lent
- Spec example in [`docs/reference/REF-ownership.md`](REF-ownership.md) explaining const's behavior at call sites
- Design rationale in [`docs/internal/implementation/IMP-ownership.md`](../internal/implementation/IMP-ownership.md) §`const` Deep Immutability section

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

## v0.2-M5 Implementation Plan

M5 will wire data to 5 of the 9 registry `[[muted_hint_domain]]` entries. The remaining 4 will have protocol handlers that return empty hint lists — they fire automatically when v0.3+ delivers the underlying analysis data. No LSP code change is needed when those features ship.

| Domain | M5 Status | What triggers data | Deferred-entry reference |
|---|---|---|---|
| `variable_type` | **Firing** — `variable_type_hints` pass in `inlay_hint_passes.rs` | Every unannotated `let x = expr` | N/A |
| `ownership_call_site` | **Firing** — `ownership_call_site_hints` pass | Every call site with a typed callee signature | N/A |
| `copy_points` | **Firing** — `copy_point_hints` pass | Trivially-copyable arg still live after call | N/A |
| `array_to_fixed_promotion` | **Firing** — `array_to_fixed_promotion_hints` pass | `let x: array<T> = [...]` with no mutation/growth detected | N/A |
| `let_to_const_promotion` | **Firing** — `let_to_const_promotion_hints` pass | `let x = ...` with no reassignment/mutation/`.lend` detected | N/A |
| `function_param_type` | **Protocol-only** (empty) | First-class lambdas with inferred param types | [`registry/features.toml`](../../registry/features.toml) `lsp-inlay-hint-function-param-type` |
| `wait_points` | **Protocol-only** (empty) | `wait`-auto-insertion analysis (I/O suspension) | [`registry/features.toml`](../../registry/features.toml) `lsp-inlay-hint-wait-points` |
| `lifetimes` | **Protocol-only** (empty) | Explicit lifetime UI (may stay fully implicit) | [`registry/features.toml`](../../registry/features.toml) `lsp-inlay-hint-lifetimes` |
| `allocators` | **Protocol-only** (empty) | `arena scratch { }` keyword | [`registry/features.toml`](../../registry/features.toml) `lsp-inlay-hint-allocators` |

All 9 protocol handlers will be registered in `crates/ynz-lsp/src/inlay_hint.rs::inlay_hint_response` (planned Phase 6). Protocol-only handlers return `Vec<InlayHint>::new()` — not an error, not a panic, just an empty list.

The 5 detection passes will live in `crates/ynz-typeck/src/inlay_hint_passes.rs` as salsa-tracked queries. The hover content for each firing hint will be sourced from the registry via `ynz-registry::lsp_inlay_hint_hover_for(domain, context)` — same SSOT that drives autocomplete and keyword hover.

---

## Cross-references

- [`.claude/rules/inference.md`](../../.claude/rules/inference.md) (the uniform inference rule this protocol implements)
- [`docs/reference/REF-golden-rules.md`](REF-golden-rules.md) Rule 11 (extended to all teaching surfaces)
- [`docs/reference/REF-teaching-mission.md`](REF-teaching-mission.md) "IDE as a Teaching Surface" section
- [`docs/reference/REF-compiler-errors.md`](REF-compiler-errors.md) (the three-part diagnostic format hints inherit)
- [`docs/internal/implementation/IMP-lsp.md`](../internal/implementation/IMP-lsp.md) "Inlay Hints" section (M5 implementation status in full-detail table)
- [`registry/features.toml`](../../registry/features.toml) `[[muted_hint_domain]]` entries (canonical domain definitions)
- [`docs/internal/scratchpad/SCRATCH-future-designs-index.md`](../internal/scratchpad/SCRATCH-future-designs-index.md) (where this file sits in the future-list ordering, even though `docs/reference/REF-ide-hints.md` is the protocol itself, not in `design/future/`)
