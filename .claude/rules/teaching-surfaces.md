---
name: "teaching-surfaces"
description: >
  The consolidated checklist every user-facing diagnostic, hover tooltip, lint suggestion, and
  muted hint must clear: the three-slot WHAT/WHAT-INSTEAD/WHY test, the 18-year-old-JS-dev
  audience test, the banned-vocabulary pointer, in-example naming conventions, and the
  no-internal-paths/no-milestone-tags rules. Load whenever a diff touches Diagnostic:: construction,
  registry teaching fields, or inlay_hint/hover code.
tags:
  - "yinz-compiler"
  - "teaching"
created_at: "2026-07-16"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "rule"
---

# Teaching Surfaces — The Checklist Every User-Facing Teaching String Must Clear

Load this rule whenever a diff touches `Diagnostic::` construction, `registry/features.toml` teaching
fields (`hover_what`/`hover_what_instead`/`hover_why`, lint `what`/`what_instead`/`why`,
`[[diagnostic_template]]`), or `inlay_hint`/`hover` code. Consolidates the checklist that used to be
scattered across six documents into one grading pass a reviewer applies directly — the judgment tier no
mechanical check can fully cover.

---

## The three-slot test (Golden Rule 11)

Every user-facing diagnostic, hover tooltip, lint suggestion, and muted hint answers three slots — see
[`docs/reference/REF-teaching-mission.md`](../../docs/reference/REF-teaching-mission.md) for the
canonical mission. Grade each slot independently; a string that nails WHAT but fumbles WHY is still
non-conformant:

- **WHAT** states the problem the user actually hit, in plain English — not a category label and never
  a raw-output dump (a linker-failure WHY that was literally `"Linker stderr:\n{...}"` is the
  anti-example — tool output belongs in the message body, never the teaching slot).
- **WHAT-INSTEAD** is copyable and actionable — the test is "can the user DO this?" A rule restatement
  ("Each parameter must have a unique name") is a WHY wearing WHAT-INSTEAD's clothes; the real
  WHAT-INSTEAD is the fix itself ("Rename one of them, e.g. `count2`").
- **WHY** is contextual and non-circular, and **cites no internals** — never a runtime name (`Tokio`,
  `ynz_rt_init`), an LLVM term (`alloca`, `frame-backed`), or a restatement of the WHAT ("this fails
  because it's invalid" is circular, not a reason).

---

## The audience test

Every slot clears: **"Would an 18-year-old JS developer understand this without Googling?"** — the same
bar [`spec-writing.md`](spec-writing.md) sets for language-spec prose, applied here to diagnostics,
hovers, and lint text. If a slot needs a CS-jargon term to land, replace the term or drop the sentence —
don't gate the explanation behind vocabulary the audience doesn't have.

---

## Banned vocabulary

Check every teaching string against [`vocabulary.md`](vocabulary.md)'s Banned Legacy Terms table — the
SSOT the registry's `[[banned_jargon]]` entries enforce mechanically via `jargon_audit.rs`. `infer`/
`inference` is the recurring miss: correct inside this rules corpus (engineer audience), banned the
moment it reaches a user-facing hover or diagnostic — see [`inference.md`](inference.md)'s
Dual-Audience Disclaimer for the full split.

---

## Naming conventions inside examples

A diagnostic that teaches the wrong style trains the mistake into the user's next line of code:

- **camelCase identifiers** in every suggested rename or example (`updatedCount`, not
  `updated_count`) — never snake_case.
- **`.copy()` — parens, always.** It's a body action per [`dot-postfix.md`](dot-postfix.md); never bare
  `.copy` in an example or a hover string.
- **No SCREAMING_SNAKE constants.** `const maxHealth = 100`, never `const MAX_HEALTH = 100`, per
  [`vocabulary.md`](vocabulary.md)'s constants-naming ruling. GR13 ("capital letter = type") stays
  absolute; there is no ratified constants exception today. A future design that genuinely needs one is
  ratified in `vocabulary.md` first — never introduced silently by an example.

---

## No internal paths, no milestone tags

A user-facing WHY never cites something the user can't reach or has no context for:

- **No internal doc paths** (`design/concurrency.md`, `IMP-no-runtime-mode.md`) — the user has no repo
  to open. Cite a stable feature-registry name instead ("see
  `background-handle-nonsuspending-callee` in the feature registry"), or drop the citation entirely.
- **No milestone tags** (`M5`, `v0.3-M2`) — say "ships in a later version"; a milestone number carries
  no meaning for someone outside the project.

---

## Worked good/bad pair

```
❌ WHY: "sleep requires the Tokio runtime (started by ynz_rt_init)."
✅ WHY: "sleep needs the scheduler runtime that Yinz starts for you automatically — this call can't
        run before that runtime exists."
```

---

## Cross-References

- [`docs/reference/REF-teaching-mission.md`](../../docs/reference/REF-teaching-mission.md) — the
  canonical WHAT/WHAT-INSTEAD/WHY mission (Golden Rule 11)
- [`vocabulary.md`](vocabulary.md) — banned vocabulary, the constants-naming ruling, Capital Letter Rule
- [`dot-postfix.md`](dot-postfix.md) — parens-for-actions (`.copy()`)
- [`spec-writing.md`](spec-writing.md) — the audience test's shared origin (language-spec prose)
- [`inference.md`](inference.md) — the internal-vs-user-facing `infer`/`inference` dual-audience split
