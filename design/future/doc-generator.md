# `ynz doc` — Documentation Generator

**Status**: Parking lot — direction confirmed, implementation deferred (v0.3+ or v0.4+)
**Trigger**: first real Yinz library that another project depends on, OR Yinz stdlib lands (v0.5+)

---

## What It Does

`ynz doc` reads a Yinz project and produces structured documentation for every exported declaration — shapes, functions, options types, constants. It combines two sources of truth that already exist in the compiler:

1. **The type signature** — params, types, return type, ownership modifiers — pulled directly from the AST. Always correct; cannot drift from the code.
2. **The leading `//` doc comment** — prose description written by the author above the declaration (see `design/doc-comments.md`).

The output is a navigable reference that looks like `rustdoc`, `godoc`, or TypeDoc, but tailored for Yinz's type system. Hosted anywhere — static HTML by default.

---

## Why This Design Is Better Than JSDoc/JavaDoc

Every JSDoc/JavaDoc-style system has the same failure mode: `@param` descriptions get out of sync with actual parameter names and types. The type changes, the param gets renamed, the comment says something that's no longer true. The user trusts the comment; the code does something different.

Yinz sidesteps this entirely because the type signature IS the structured documentation. `ynz doc` derives the structure from the compiler's type-checked AST — same source the compiler uses. If a param changes name or type, the docs update automatically on the next `ynz doc` run. No tags to maintain.

The `//` comment adds the PROSE — the "what it does" and "why you'd use it." The signature adds the STRUCTURE — "what goes in, what comes out." Both are always correct by construction.

---

## Output Format

For a function:

```
fetchUser(id: UserId) -> maybe User errors

Fetches a user by their unique ID.
Returns none if the user doesn't exist.
Errors on database connection failure.

Parameters
  id: UserId — the unique identifier to look up

Returns
  maybe User — a User value, or none if not found

Errors
  DatabaseError — on connection failure or query timeout
```

The "Parameters" and "Returns" sections are DERIVED from the signature — no tags needed. If the author wants to annotate a specific parameter, they use inline trailing comments on the param:

```ynz
export function fetchUser(
  id: UserId    // the unique identifier to look up
) -> maybe User errors {
```

The doc generator picks up inline trailing comments on parameters and includes them in the "Parameters" section.

For a shape:

```
shape Player

Represents a player in the game world.

Fields
  name:     string    — The player's display name.
  health:   int       — Current HP, clamped 0–100. Never negative.
  position: Position  — World-space coordinates in meters.
```

Field descriptions come from inline trailing comments on the field declarations (or leading `//` blocks above the field).

For an options type:

```
options AssetClass

The category of a tradable asset on Alpaca.

Variants
  us_equity   — U.S. equity (stock)
  us_option   — U.S. options contract
  crypto      — Cryptocurrency  (display: "Cryptocurrency")
  ipo         — IPO indication of interest  (display: "IPO Indication of Interest")
```

---

## CLI Interface

```bash
ynz doc                    # generate docs for the current project
ynz doc --out ./docs       # output directory (default: ./doc)
ynz doc --format html      # output format: html (default), json, markdown
ynz doc --open             # open in browser after generation
ynz doc --watch            # rebuild on file change (pairs with ynz watch)
```

`ynz doc --format json` emits a structured JSON API of all exported symbols, suitable for:
- Third-party doc hosts
- IDE plugins that want rich type info
- AI tooling (a Yinz-aware LLM can read the JSON API to understand a library)

---

## What Gets Documented

- All `export`-prefixed declarations: `export shape`, `export function`, `export options`, `export const`
- `hidden` fields are excluded — they are implementation details not visible to users
- Non-exported items are excluded from the generated output (but still show in LSP hover within the project)
- Inherited shape fields (via `extends`) are shown in the child shape's docs, attributed to the parent

---

## Hover Integration (already ships)

The doc generator shares infrastructure with the LSP hover system. The same `doc: Option<String>` field on every AST declaration node feeds both:
- **LSP hover**: shows prose + colored signature in the editor
- **`ynz doc`**: formats the same prose + signature into the generated docs

This means hover works immediately once `//` comments are written — no separate step. `ynz doc` is the same information formatted for a browser instead of a tooltip.

---

## Implementation Notes (for when this milestone is planned)

- The doc comment `//` attachment logic lives in `ynz-parser` (attaches during parse, stores in `doc: Option<String>` on AST nodes). Already implemented for M5.
- The inline trailing-comment-on-param/field attachment does NOT exist yet — that's new parser work for this milestone.
- The HTML generator is a new crate (`crates/ynz-doc`), consuming the parsed AST + type-checked signature table.
- JSON format can be a thin serialization of the existing `SignatureOutput` + doc fields — minimal new work.
- `ynz doc --watch` reuses `ynz-watch` file-watching infrastructure.
- Target: < 100ms for a single-file project; < 2s for a 10,000-line stdlib.

---

## Cross-References

- `design/doc-comments.md` — the Go-model `//` comment convention (the source of the prose)
- `spec/doc-comments.md` — user-facing spec for writing doc comments
- `design/lsp.md` — doc-comment attachment in the parser, LSP hover rendering
- `design/mvp-scope.md` — version target for this feature
- `design/future/index.md` — this doc's entry in the future-designs index
