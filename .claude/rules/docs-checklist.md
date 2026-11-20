---
name: Yinz Docs Protocol
description: "Applies when adding a language feature, making a design decision, or writing a file under docs/: which doc holds what and what to update."
trigger: relevance
---

## What each doc holds

One topic, one file; group related things inside it. Why: a topic with one home cannot contradict itself.

- **`docs/reference/REF-*.md`:** user-facing, shows how to use a feature. No design reasons, no open questions. Why: users need usage, not rationale.
- **`docs/internal/implementation/IMP-*.md`:** living and amended in place. It holds the decision in force plus a one-clause reason; alternatives and evidence go in the commit message. Why: the doc states what holds now, and the commit keeps the history.
- **`docs/internal/scratchpad/SCRATCH-*.md`:** open questions, and locked-but-unbuilt designs. Exempt from doc gates. Why: a sandbox stays frictionless.
- **`docs/README.md`:** an index only, never design content. Why: content there has no owner and goes stale.

## Adding a language feature

Touch all of these. Why: a missed doc leaves a stale or undiscoverable duplicate.

- Create or update `docs/reference/REF-<feature>.md`.
- Create or update `docs/internal/implementation/IMP-<feature>.md`.
- Add the REF file to the contents of `docs/reference/REF-language-overview.md`.
- Add the IMP file to the index table in `docs/README.md`.
- Close the matching entry in `docs/internal/scratchpad/SCRATCH-open-questions.md` and move its resolved content to the IMP file. Why: an open entry for a settled question misleads.
- Sweep the existing `docs/reference/REF-*.md` files for examples that use the feature and update them. Why: old examples silently contradict the new behavior.
- If the feature graduates a `SCRATCH-future-*.md` or `SCRATCH-stdlib-*.md` idea to shipping, delete or trim that file so the IMP/REF file is the only home. Why: a leftover copy goes stale.

## Making a design decision

- Put it in the matching IMP file, creating it if missing.
- If it resolves an open question, remove that entry from `SCRATCH-open-questions.md`.
- Add a `docs/README.md` index row if the IMP file is new.
- If it changes user-facing behavior, update the matching `REF-*.md` file. Why: REF is what users read.
- Write no ADR file. Why: IMP docs hold the decision in force, and the commit holds the alternatives.
