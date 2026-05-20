# Changelog

All notable changes to the Yinz Language extension are documented here.

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
