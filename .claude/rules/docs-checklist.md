---
name: "docs-checklist"
description: >
  Documentation structure rules and checklists for adding a language feature or making a
  design decision within Yinz's four-bucket docs/ taxonomy (docs/reference/REF-*.md,
  docs/internal/implementation/IMP-*.md, docs/internal/decisions/ADR-*.md,
  docs/internal/scratchpad/SCRATCH-*.md).
tags:
  - "yinz-compiler"
  - "docs"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "rule"
---

# Documentation Rules & Checklist

## Structure

This project follows the **global `docs/` taxonomy** (`~/.claude/docs/internal/implementation/IMP-documentation-system.md`), migrated 2026-07-01 off the old two-tree `spec/`+`design/` layout. Four buckets carry this project's content:

- **`docs/reference/REF-*.md`** — the shared, cited-as-authority source of truth. Two kinds: (1) **user-facing language spec** (formerly `/spec/`) — written for a HS grad who knows JavaScript, examples-heavy, no design rationale, see [`.claude/rules/spec-writing.md`](spec-writing.md); (2) **cross-cutting compiler principles** other docs cite as the one canonical home (golden rules, teaching mission, naming, compiler-errors, mvp-scope, ide-hints).
- **`docs/internal/implementation/IMP-*.md`** — living engineering blueprints (formerly `/design/`). Contributor/architect audience. What was decided, how it's implemented, alternatives considered, tradeoffs. Amended in place as the compiler evolves — NOT an immutable log (that's ADRs, below).
- **`docs/internal/decisions/ADR-*.md`** — one-time, immutable, sequentially-numbered architecture decision records (tech-stack choices, governance policy). Never edit an accepted ADR's decision — a new ADR supersedes it.
- **`docs/internal/scratchpad/SCRATCH-*.md`** — unresolved design questions (formerly `design/open-questions.md`) and locked-but-not-yet-built future/stdlib design ideas (formerly `design/future/`, `design/stdlib/`). Exempt from frontmatter/linking gates per the global standard — a frictionless sandbox.

Every topic gets its own dedicated file. No threshold judgment about "is this major enough." Related things can be grouped within a single file. [`docs/README.md`](../../docs/README.md) is the index — it links to everything, contains nothing itself.

---

## Adding a Language Feature — Checklist

- [ ] Create or update `docs/reference/REF-feature.md` — user-facing (HS-grad readable, examples-heavy, compiler errors shown)
- [ ] Create or update `docs/internal/implementation/IMP-feature.md` — design rationale (what was decided, alternatives considered, why this won)
- [ ] Add `docs/reference/REF-feature.md` to [`docs/reference/REF-language-overview.md`](../../docs/reference/REF-language-overview.md) table of contents
- [ ] Add `docs/internal/implementation/IMP-feature.md` to [`docs/README.md`](../../docs/README.md) index table
- [ ] Close relevant items in [`docs/internal/scratchpad/SCRATCH-open-questions.md`](../../docs/internal/scratchpad/SCRATCH-open-questions.md) — move resolved content to the implementation file
- [ ] Sweep existing `docs/reference/REF-*.md` spec files for examples that reference the new feature — update them
- [ ] If the feature graduates a `docs/internal/scratchpad/SCRATCH-future-*.md`/`SCRATCH-stdlib-*.md` idea from "future" to "shipping now," delete or trim the scratch file and point it at the new `IMP-*.md`/`REF-*.md` home instead of leaving a stale duplicate

---

## Making a Design Decision — Checklist

- [ ] **Living/amendable decision** (the normal case — a feature's design evolves with the compiler): add to the relevant `docs/internal/implementation/IMP-feature.md` file (create it if it doesn't exist)
- [ ] **One-time, locked, unlikely-to-be-revisited decision** (tech-stack choice, versioning policy — see `.claude/rules/docs-checklist.md` "What Goes Where" below): write a new `docs/internal/decisions/ADR-NNN-short-name.md` instead. Never edit an accepted ADR's Decision section — write a new ADR that supersedes it.
- [ ] If it resolves an open question, remove from [`docs/internal/scratchpad/SCRATCH-open-questions.md`](../../docs/internal/scratchpad/SCRATCH-open-questions.md)
- [ ] Add a row to [`docs/README.md`](../../docs/README.md) index if this is a new file
- [ ] If the decision changes existing user-facing behavior, update the matching `docs/reference/REF-*.md` spec file

---

## File Content Rules

### `docs/reference/REF-*.md` (language-spec files)
- Written for a HS grad who knows JavaScript
- Short sections, lots of code examples
- Show the actual compiler error message when demonstrating mistakes
- No design rationale (that goes in `docs/internal/implementation/`)
- No unresolved questions (those go in [`docs/internal/scratchpad/SCRATCH-open-questions.md`](../../docs/internal/scratchpad/SCRATCH-open-questions.md))
- Follow [`.claude/rules/spec-writing.md`](spec-writing.md) for tone and format

### `docs/internal/implementation/IMP-*.md` (design files)
- Written for compiler engineers and language architects
- Each decision: what was decided, alternatives considered, why this one won
- Link to the relevant `docs/reference/REF-*.md` spec file at the top, when a user-facing counterpart exists
- Can reference open questions — link to [`docs/internal/scratchpad/SCRATCH-open-questions.md`](../../docs/internal/scratchpad/SCRATCH-open-questions.md)

### `docs/internal/decisions/ADR-*.md` (decision records)
- Immutable once accepted — a new ADR supersedes, never edits, an old one
- Follows the ADR template: Status / Context / Decision / Consequences
- Reserved for genuinely one-time, hard-to-reverse calls — not every feature design belongs here (most do NOT; see `IMP-*.md` above)

### `docs/internal/scratchpad/SCRATCH-*.md` (open questions + future ideas)
- Exempt from frontmatter/linking/kebab-case gates per the global standard — write loosely
- `SCRATCH-open-questions.md` — one entry per unresolved question; move to its dedicated `IMP-*.md`/`ADR-*.md` and delete the entry once resolved
- `SCRATCH-future-*.md` / `SCRATCH-stdlib-*.md` — locked-but-unbuilt design ideas; states what's deferred, why, target milestone, what's locked vs still open

---

## What Goes Where — Quick Reference

| Content | Location |
|---------|----------|
| How to use a language feature | `docs/reference/REF-feature.md` |
| Why a design decision was made (living, amendable) | `docs/internal/implementation/IMP-feature.md` |
| A one-time, locked, immutable architecture call | `docs/internal/decisions/ADR-NNN-short-name.md` |
| Design decision index | [`docs/README.md`](../../docs/README.md) |
| Unresolved design questions | [`docs/internal/scratchpad/SCRATCH-open-questions.md`](../../docs/internal/scratchpad/SCRATCH-open-questions.md) |
| Locked-but-unbuilt future feature design | `docs/internal/scratchpad/SCRATCH-future-topic.md` |
| Locked-but-unbuilt stdlib module design (v0.5+) | `docs/internal/scratchpad/SCRATCH-stdlib-module.md` |
| The 13 golden rules with rationale | [`docs/reference/REF-golden-rules.md`](../../docs/reference/REF-golden-rules.md) |
| Naming conventions (renamed keywords, casing) | [`.claude/rules/vocabulary.md`](vocabulary.md) |
| How to write language-spec files | [`.claude/rules/spec-writing.md`](spec-writing.md) |
| How to make design decisions | [`.claude/rules/language-design.md`](language-design.md) |

---

## One Rule, No Exceptions

Every topic gets its own dedicated file. There is no threshold for "is this big enough to deserve a file." If you're writing a design decision, it goes in a file. Group related things within a single file if they belong together. Never add design content to [`docs/README.md`](../../docs/README.md) — that file is an index only.
