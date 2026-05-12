# Doc Comments — Design Decisions

User spec: `spec/doc-comments.md`

---

## `///` Only — No Block Doc Syntax

Triple-slash only. Multiple `///` lines handle multi-line documentation. No `/** */` block syntax.

**Why no block docs**: Block doc comments require managing open/close delimiters. Adding a line means inserting it between `/**` and `*/`. Removing the last line leaves empty delimiters. Reordering lines is clean. Triple-slash is line-by-line — every doc line is independent. Add, remove, reorder without touching anything else. Easier to write in a text editor, easier to diff in code review.

**Why triple-slash over single-slash**: Regular `//` serves a different purpose — internal implementation notes visible only to readers of the source. `///` is visually distinct and unambiguously marks "this is public API documentation." The doc generator trivially distinguishes them by counting slashes.

---

## Exported Items Only

Doc comments on non-exported items have no effect. `hidden` fields are also excluded.

**Why**: Documentation is for users of the API. Internal helpers and hidden implementation details are not part of the public API. Mixing private and public docs creates noise in generated output. If an item isn't exported, users can't call it — documenting it would be meaningless.

---

## Field Documentation

`///` on type fields is supported and appears in generated docs next to the field.

**Why**: Type fields ARE part of the public API. A `Player` type with undocumented fields forces users to read source code to understand what `health` is clamped to, what units `position` uses, or what `score` represents. Field docs make the type self-contained.
