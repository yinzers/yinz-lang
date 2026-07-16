---
name: "language-design"
description: >
  Rules for making and reviewing Yinz language design decisions — the readability, duplication,
  performance, OOP-drift, and teaching tests every proposed feature or keyword must pass before
  it ships, plus where decisions get documented.
tags:
  - "yinz-compiler"
  - "language-design"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "rule"
---

# Language Design Rules

Rules for making and reviewing language design decisions.

---

## Before Adding Anything New

Check every proposed feature against all golden rules (in [`CLAUDE.md`](../../CLAUDE.md)). If it violates any rule, don't add it. If there's tension between two rules, the lower-numbered rule wins.

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

## The OOP Drift Test

Ask: "Does this design assume object-oriented patterns?"

If yes → reconsider. Yinz is data shapes + standalone functions + UFCS dot-call sugar — NOT OOP. Two
of the most common tells: reaching for `override`, or storing function-typed fields to simulate a
method. The full enumerated list of drift signals (and which ones are compile errors vs. review
warnings) is the [`.claude/rules/non-oop.md`](non-oop.md) Banned Anti-Patterns table — one home, not
restated here.

See [`.claude/rules/non-oop.md`](non-oop.md) for the full model + the dual-style diagnostic format for UFCS errors. Locked r10–r13.

---

## The Teaching Test

Ask: "Does this teach the user something, or does it just hide complexity?"

Every feature, rule, error message, and example should INFORM the developer about why something works the way it does. Features that make it easier to write code WITHOUT learning anything are anti-Yinz — they create dependent developers instead of capable ones.

**Features that PASS this test:** error messages with the three-part WHAT/WHAT-INSTEAD/WHY format, lint suggestions with reasoning attached, spec examples that show realistic code AND explain the design decision, IDE hints that teach performance and idiomatic patterns.

**Features that FAIL this test:** implicit conversions with no diagnostic, automatic behaviors that happen invisibly, abstractions that hide what the machine is doing, "magic" that works without explanation.

See [`docs/reference/REF-teaching-mission.md`](../../docs/reference/REF-teaching-mission.md) for the full mission and the required three-part diagnostic format. This is a load-bearing criterion — Yinz's positioning as a teaching language depends on it.

---

## Documenting Decisions

Every decision goes in its `docs/internal/implementation/IMP-<feature>.md` file (create it if it doesn't exist) per [`.claude/rules/docs-checklist.md`](docs-checklist.md), with:
1. What was decided
2. Alternatives that were considered
3. Which golden rule(s) drove the decision and why

If a decision can't be explained by any golden rule, either a new golden rule is needed or the decision should be reconsidered.

---

## Open Questions

Unresolved design questions go in `/docs/internal/scratchpad/SCRATCH-open-questions.md`. Don't leave them as vague language in spec files — make it explicit that they're open.

---

## Spec Updates

When a decision is made, update the relevant `docs/reference/REF-*.md` file immediately. Spec files are the user-facing truth — they should never be out of date with the decisions log.
