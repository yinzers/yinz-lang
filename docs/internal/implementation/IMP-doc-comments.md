---
name: "IMP-doc-comments"
description: "Design rationale for Yinz's Go-model leading '//' doc comment convention and why it replaced '///' and JSDoc-style alternatives."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Doc Comments — Design Decisions

User spec: [`docs/reference/REF-doc-comments.md`](../../reference/REF-doc-comments.md)

---

## Go-Model `//` Leading Comments (Locked 2026-06-01, replaces `///`)

**The rule**: a `//` comment block immediately above a declaration — with no blank line between the last comment line and the declaration — is the doc comment for that declaration. Anywhere else, `//` is internal implementation notes invisible to tooling.

```ynz
// Fetches a user by their unique ID.
// Returns none if the user doesn't exist.
export function fetchUser(id: UserId) -> maybe User errors {
  // implementation note — never appears in hover or generated docs
}
```

**Why this beats `///`**:

The previous design used `///` (triple-slash) to distinguish doc comments from regular `//` comments. Two reasons overturned that:

1. **Rule 6 violation**: In TypeScript — the language Yinz's target audience knows best — `///` is a compiler directive (`/// <reference path="..." />`), NOT a doc comment. A TypeScript developer seeing `///` in Yinz will be confused or assume it does something completely different. JSDoc (`/** */`) is what TS devs use for docs, but that's its own problem (see below).

2. **Rule 2 violation**: The distinction between `//` (invisible) and `///` (visible to tooling) requires documentation to understand. A jr dev who just graduated and knows JavaScript has never seen `///` as a meaningful marker. They'd have to read the spec to know which one shows in hover. That's anti-Yinz.

**Why not JSDoc (`/** */`)**:

JSDoc is what TypeScript devs know for documentation. But:
- The `@param`, `@returns`, `@example` tag vocabulary needs to be learned and remembered
- Tags become stale when code changes — `@param` descriptions get out of sync with actual types
- In Yinz, the type signature already communicates what JSDoc tags would say: `lend self: Player, amount: int` tells you exactly what `@param` would. With syntax highlighting in hover, the signature IS the colored parameter documentation.
- Multi-line formatting is awkward (`/**`, body, `*/`)
- Diff noise: adding a field requires touching `/**` and `*/` lines

**Why Go-model beats Python docstrings**:

Python puts the doc INSIDE the function body (`"""text"""`). For a compiled language without runtime reflection, there's no natural mechanism to grab that string at compile time without treating it as a special first-statement. Go's leading-comment approach has the doc OUTSIDE the declaration, which is how every other documentation approach works and what a dev naturally writes.

**What counts as "immediately before"**:

- The comment block ends at the line immediately above the declaration
- Any blank line between the last `//` line and the declaration breaks the association — that comment block is free-floating trivia
- Multiple consecutive `//` lines with no blank lines between them form one doc block
- `//` comments inside function bodies never attach to the function declaration (they're inside the body, not above it)

**`///` compatibility**:

The lexer treats `//` and `///` identically as `LineComment` tokens, stripping all leading slash characters and whitespace from the content. Code written with `///` before the Go-model decision keeps working. The spec says `//` is the canonical form; `///` is not wrong, just redundant. No lint suggestion — the difference is cosmetic and both render identically.

---

## No Block Doc Syntax

No `/** */` block comments, not now, not ever. Single-slash `//` lines handle multi-line documentation.

**Why**: Block docs require managing open/close delimiters. Adding a line means inserting it between `/**` and `*/`. Removing the last line leaves empty delimiters. `//` is line-by-line — every doc line is independent. Add, remove, reorder without touching anything else. Easier to write, easier to diff.

---

## Exported Items Only

Doc comments on non-exported items are captured by the compiler but excluded from `ynz doc` generated output. `hidden` fields are also excluded from generated docs.

**Why**: Documentation is for users of the API. Internal helpers and implementation details are not part of the public API surface. They still show in LSP hover within the same project (hover works on all symbols regardless of export status), but the doc generator filters them.

---

## Field Documentation

`//` on a field line (inline trailing comment, immediately after the field declaration on the same line OR as a block above the field) is supported and appears in generated docs next to the field.

```ynz
export shape Player {
  name: string          // The player's display name.
  health: int           // Current HP, clamped 0–100. Never negative.
  position: Position    // World-space coordinates in meters.
}
```

Alternatively, leading block form:

```ynz
export shape Player {
  // The player's display name.
  name: string
  // Current HP, clamped 0–100. Never negative.
  health: int
}
```

**Why**: Type fields ARE part of the public API. A `Player` shape with undocumented fields forces users to read source code to understand what `health` is clamped to or what units `position` uses. Field docs make the type self-contained.

**Inline vs leading**: Both forms work. `ynz fmt` normalizes trailing-comment alignment (see [`docs/internal/scratchpad/SCRATCH-open-questions.md`](../scratchpad/SCRATCH-open-questions.md) formatter section). The `ynz doc` generator accepts both.

---

## Hover Integration

Doc comments attach to the declaration's `doc: Option<String>` field in the AST and surface in three places:

1. **LSP hover**: shows as Markdown prose above the colored type signature
2. **`ynz doc` generated output**: see [`docs/internal/scratchpad/SCRATCH-future-doc-generator.md`](../scratchpad/SCRATCH-future-doc-generator.md)
3. **IDE completion detail**: shows first line of doc comment in the autocomplete detail panel

The type signature always renders as a fenced ` ```ynz ``` ` code block in hover, which gets syntax highlighting from the Yinz TextMate grammar. Parameters are colored by type; ownership modifiers are colored; return types are colored. This makes the signature self-documenting without needing `@param` or `@returns` tags.

---

## Cross-References

- [`docs/reference/REF-doc-comments.md`](../../reference/REF-doc-comments.md) — user-facing spec (the "how to write them")
- [`docs/internal/scratchpad/SCRATCH-future-doc-generator.md`](../scratchpad/SCRATCH-future-doc-generator.md) — `ynz doc` command design
- [`docs/reference/REF-ide-hints.md`](../../reference/REF-ide-hints.md) — LSP hover format spec
- [`docs/internal/implementation/IMP-lsp.md`](IMP-lsp.md) — doc-comment attachment in the parser (Phase 10 of v0.2-M5)
- [`docs/internal/scratchpad/SCRATCH-open-questions.md`](../scratchpad/SCRATCH-open-questions.md) — trailing-comment alignment in `ynz fmt`
