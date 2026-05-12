# Documentation Rules & Checklist

## Structure

Two parallel doc trees with distinct audiences:

- **`spec/`** — for language users. Written for a HS grad who knows JavaScript. Explains what to write and what it does. Examples-heavy, plain English. No design rationale.
- **`design/`** — for language contributors and architects. Explains WHY decisions were made, implementation approach, tradeoffs. Audience is the compiler team.

Every topic gets its own dedicated file. No threshold judgment about "is this major enough." Related things can be grouped within a single file. `design/decisions.md` is the index — it links to everything, contains nothing itself.

---

## Adding a Language Feature — Checklist

- [ ] Create or update `spec/feature.md` — user-facing (HS-grad readable, examples-heavy, compiler errors shown)
- [ ] Create or update `design/feature.md` — design rationale (what was decided, alternatives considered, why this won)
- [ ] Add `spec/feature.md` to `spec/overview.md` table of contents
- [ ] Add `design/feature.md` to `design/decisions.md` index table
- [ ] Close relevant items in `design/open-questions.md` — move resolved content to the design file
- [ ] Sweep existing spec files for examples that reference the new feature — update them

---

## Making a Design Decision — Checklist

- [ ] Add to the relevant `design/feature.md` file (create it if it doesn't exist)
- [ ] If it resolves an open question, remove from `design/open-questions.md`
- [ ] Add a row to `design/decisions.md` index if this is a new design file
- [ ] If the decision changes existing spec content, update the spec file

---

## File Content Rules

### `spec/` files
- Written for a HS grad who knows JavaScript
- Short sections, lots of code examples
- Show the actual compiler error message when demonstrating mistakes
- No design rationale (that goes in `design/`)
- No unresolved questions (those go in `design/open-questions.md`)
- Follow `.claude/rules/spec-writing.md` for tone and format

### `design/` files
- Written for compiler engineers and language architects
- Each decision: what was decided, alternatives considered, why this one won
- Link to the relevant `spec/` file at the top
- Can reference open questions — link to `design/open-questions.md`

---

## What Goes Where — Quick Reference

| Content | Location |
|---------|----------|
| How to use a language feature | `spec/feature.md` |
| Why a design decision was made | `design/feature.md` |
| Design decision index | `design/decisions.md` |
| Unresolved design questions | `design/open-questions.md` |
| The 13 golden rules with rationale | `design/golden-rules.md` |
| Standard library module design | `design/stdlib/module.md` |
| Naming conventions (renamed keywords, casing) | `.claude/rules/naming.md` |
| How to write spec files | `.claude/rules/spec-writing.md` |
| How to make design decisions | `.claude/rules/language-design.md` |

---

## One Rule, No Exceptions

Every topic gets its own dedicated file. There is no threshold for "is this big enough to deserve a file." If you're writing a design decision, it goes in a file. Group related things within a single file if they belong together. Never add design content to `design/decisions.md` — that file is an index only.
