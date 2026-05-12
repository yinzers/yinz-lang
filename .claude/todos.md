# Todos: ynz

Global cross-workstream items only. Granular per-chat work lives in:
- `.claude/plans/active/{slug}.md` for planned work
- `.claude/work/{slug}/todos.md` for unplanned chat-scoped work

---

## Now (active cross-workstream items)

- [x] **Compiler error-message audit (2026-05-12)** — DONE:
  - **A.** Wrote `design/compiler-errors.md` — the style spec with required three-part WHAT/WHAT-INSTEAD/WHY format, banned-jargon list, tone guide, multi-error strategy.
  - **B.** Swept `spec/**/*.md` for jargon. Rewrote: `spec/errors.md`, `spec/control-flow.md`, `spec/unions.md`, `spec/type-conversion.md`, `spec/main.md`, `spec/testing.md`, `spec/types.md`, `spec/functions.md`.
  - "Auto-propagation" kept as Yinz's official feature name but must be explained in plain English on first use.
  - `spec/linting.md` notes its catalog examples are abbreviated — the real compiler output uses the full three-part format.
  - Future error messages added by new versions must follow `design/compiler-errors.md`.

## Soon (committed, not started)

- [ ] {item}

## Later (idea bin — not committed)

- [ ] {item}

## Done (recent)

- [x] {item} (2026-05-11)
