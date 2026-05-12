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

## The Teaching Test

Ask: "Does this teach the user something, or does it just hide complexity?"

Every feature, rule, error message, and example should INFORM the developer about why something works the way it does. Features that make it easier to write code WITHOUT learning anything are anti-Yinz — they create dependent developers instead of capable ones.

**Features that PASS this test:** error messages with the three-part WHAT/WHAT-INSTEAD/WHY format, lint suggestions with reasoning attached, spec examples that show realistic code AND explain the design decision, IDE hints that teach performance and idiomatic patterns.

**Features that FAIL this test:** implicit conversions with no diagnostic, automatic behaviors that happen invisibly, abstractions that hide what the machine is doing, "magic" that works without explanation.

See `design/teaching-mission.md` for the full mission and the required three-part diagnostic format. This is a load-bearing criterion — Yinz's positioning as a teaching language depends on it.

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
