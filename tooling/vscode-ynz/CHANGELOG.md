# Changelog

All notable changes to the Yinz Language extension are documented here.

## [0.3.0-m1] — 2026-05-21

### Added

- **`background` runs on a separate thread** — hover over the `background` keyword to see updated v0.3-M1 docs (WHAT: "Runs the function on a separate thread"; WHAT INSTEAD: correct call forms; WHY: prior v0.2 behavior was sequential).
- **`wait` hover docs updated** — hover doc explains M1 semantics (synchronous; state-machine suspension arrives in v0.3-M2).
- **New compile errors**:
  - `background fn(lend-param)` → lend-cross-thread safety error with `.give`/`.copy()` fix suggestion.
  - `background fn(...)` in `--kernel` mode → error with explanation.
  - `wait expr` in `--kernel` mode → error with explanation.
- **New inlay hint** — `background fn(largeStruct.copy())` where estimated copy size > 64 bytes shows `.give (transfers ownership; no copy)` muted annotation inline.
- **Large-copy warning** — Tier 3 lint warning (yellow) on `.copy()` args > 64 bytes at `background` call sites.

### Screenshots

See `screenshots/background-concurrent.png` for the hover doc and inline hint in action.

## [0.2.0-m2] — 2026-05-20

Initial release (preview).

### Added

- Syntax highlighting for `.ynz` files: keywords, deferred features (illegal), banned declaration keywords (deprecated), strings, numbers, comments
- Inline diagnostics: Yinz compiler errors displayed as red squiggles with full WHAT/WHAT-INSTEAD/WHY teaching content
- Autocomplete: keywords, primitive methods filtered by receiver type, type-attached constants, deferred features (shown as deprecated)
- Hover docs: registry-sourced WHY content for every keyword, primitive intrinsic, type constant, deferred feature, and banned keyword
- TextMate grammar derived automatically from the Yinz feature registry — new keywords and features appear in the editor on rebuild
- Language association for `.ynz` files
- `yinz.server.path` configuration setting to point at a custom `ynz-lsp` binary location
