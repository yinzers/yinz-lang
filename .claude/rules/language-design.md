# Language Design Rules

Rules for making and reviewing language design decisions.

---

## Before Adding Anything New

Check every proposed feature against all 12 golden rules (in `CLAUDE.md`). If it violates any rule, don't add it. If there's tension between two rules, the lower-numbered rule wins.

---

## The Readability Test

Ask: "Could a developer who just graduated high school and knows JavaScript read this without documentation?"

If no → rename it. If there's no good English word, ask the user.

---

## The Duplication Test

Ask: "Does this concept already exist in the language under a different name?"

One concept = one keyword. If something can be expressed with existing syntax, don't add a new keyword.

---

## The Performance Test

Ask: "Is the default behavior the most performant option?"

The developer who doesn't think about performance should automatically write fast code. If the intuitive default is slow, the design is wrong.

---

## Documenting Decisions

Every decision goes in `/design/decisions.md` with:
1. What was decided
2. Alternatives that were considered
3. Which golden rule(s) drove the decision and why

If a decision can't be explained by any golden rule, either a new golden rule is needed or the decision should be reconsidered.

---

## Open Questions

Unresolved design questions go in `/design/open-questions.md`. Don't leave them as vague language in spec files — make it explicit that they're open.

---

## Spec Updates

When a decision is made, update the relevant `/spec/` file immediately. Spec files are the user-facing truth — they should never be out of date with the decisions log.
