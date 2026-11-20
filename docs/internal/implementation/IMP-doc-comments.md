---
name: "IMP-doc-comments"
description: "Design decisions for Yinz's '///' doc comment convention: a /// block directly above a declaration or field."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-19"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Doc Comments — Design Decisions

User spec: [`docs/reference/REF-doc-comments.md`](../../reference/REF-doc-comments.md)

---

## `///` Leading Comments

**The rule**: a `///` comment block immediately above a declaration — with no blank line between the last comment line and the declaration — is the doc comment for that declaration. A regular `//` comment is skipped by the lexer and invisible to tooling.

```ynz
/// Fetches a user by their unique ID.
/// Returns none if the user doesn't exist.
export function fetchUser(id: UserId) -> Maybe<User> errors {
  // implementation note — never appears in hover
}
```

**Why `///`**: the lexer can tell a doc comment from a note by the third slash alone, so a note never turns into documentation by accident.

**How it attaches**: the lexer emits one `DocComment` token per `///` line, with a flag for whether a blank line follows it. The parser collects consecutive `///` lines, joins them with newlines, and gives the result to the next function, shape, options or `base shape` declaration (exported or not). A blank line after a `///` block discards it. An import or an unrecognized top-level token also discards pending `///` lines.

- Multiple consecutive `///` lines with no blank lines between them form one doc block.
- `//` comments inside function bodies never attach to anything.

Decided: JSDoc `/** */`, Python-style docstrings and block doc syntax rejected.

---

## Field Documentation

A `///` block directly above a shape field documents that field.

```ynz
export shape Player {
  /// The player's display name.
  name: String
  /// Current HP, clamped 0–100. Never negative.
  health: Int
}
```

**Why**: shape fields are part of the public API, so a field doc keeps its meaning next to its declaration.

---

## Hover Integration

Doc comments attach to the declaration's `doc: Option<String>` field in the AST and surface in:

1. **LSP hover**: shows as Markdown prose above the colored type signature.
2. **`ynz doc` generated output** (not shipped): see [`docs/internal/scratchpad/SCRATCH-future-doc-generator.md`](../scratchpad/SCRATCH-future-doc-generator.md).

The type signature always renders as a fenced ` ```ynz ``` ` code block in hover, which gets syntax highlighting from the Yinz TextMate grammar. This makes the signature self-documenting without `@param` or `@returns` tags.

---

## Cross-References

- [`docs/reference/REF-doc-comments.md`](../../reference/REF-doc-comments.md) — user-facing spec (the "how to write them")
- [`docs/internal/scratchpad/SCRATCH-future-doc-generator.md`](../scratchpad/SCRATCH-future-doc-generator.md) — `ynz doc` command design
- [`docs/reference/REF-ide-hints.md`](../../reference/REF-ide-hints.md) — LSP hover format spec
- [`docs/internal/implementation/IMP-lsp.md`](IMP-lsp.md) — doc-comment attachment in the parser
