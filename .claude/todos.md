# Todos: ynz

Global cross-workstream items only. Granular per-chat work lives in:
- `.claude/plans/active/{slug}.md` for planned work
- `.claude/work/{slug}/todos.md` for unplanned chat-scoped work

---

## Now (active cross-workstream items)

- [ ] **After current question pass is complete: tackle the compiler error-message audit.** Two-part task (per `design/open-questions.md#compiler-error-format--full-spec`):
  - **A.** Write the plain-English error-message style rule. Likely a new `design/compiler-errors.md` with a jargon ban-list, jr-dev readability test, format spec, and tone guide.
  - **B.** Sweep all error-message examples in `spec/**/*.md` and `design/**/*.md`. Flag and rewrite any programmer jargon ("propagate," "narrow," "infer," etc.) in plain English.
  - **Motivation:** Golden Rule 11 says the compiler is a teacher. If error messages use jargon, the language fails its own promise. Triggered by patrick noticing "propagate" in an example error during the Iterable design discussion (2026-05-12).

## Soon (committed, not started)

- [ ] {item}

## Later (idea bin — not committed)

- [ ] {item}

## Done (recent)

- [x] {item} (2026-05-11)
