# LSP UX Friction Report

**Analyzed**: 2026-05-21
**Scope**: `crates/ynz-lsp/`, `crates/ynz-typeck/src/inlay_hint_passes.rs`, `crates/ynz-typeck/src/check.rs`
**Friction Issues Found**: 10 (Blocker: 2, Annoyance: 5, Nit: 3)
**Opportunities Identified**: 4 (High: 2, Medium: 2)

## Blocker findings

1. **Replacement hints render at `span.start`** (`inlay_hint_passes.rs:463,514`) — comment overlaps `let` keyword instead of trailing the statement.
2. **`array_to_fixed_promotion` has no `text_edits`** (`inlay_hint.rs:216-226`) — click-to-make-explicit silently dead. Asymmetric with `let_to_const_promotion` which works correctly.

## Annoyance findings

3. **Keyword hovers return only "Introduced in M4"** (`lsp_adapter.rs:218-223`) — no teaching content for `follows`, `extends`, `base`, `dynamic`, `hidden`, `wait`, `background`, `errors`, `sensitive`.
4. **Shape/options hover returns None** (`hover.rs:124-152`) — only function lookups, no shape_table check.
5. **`collect_maybe_mutated` over-suppresses promotion hints** (`inlay_hint_passes.rs:163-170`) — every call arg marked mutated regardless of callee ownership. Hints silently MIA on virtually every real binding.
6. **Space is a completion trigger** (`capabilities.rs:38`) — fires popup on every space in comments/strings.
7. **`BannedJargon` has no quick-fix** (`code_action.rs:57-76`) — only `BannedKeyword` is wired; jargon replacement data exists in the registry but unused.

## Nit findings

8. **Rename progress threshold only fires `>3 files`** — single-file renames with many references give no feedback.
9. **Format-on-save parse-error uses `MessageType::INFO`** instead of `WARNING` — undersells urgency.
10. **Cross-file completion items lack visual indicator** — `Player (import)` suffix would prevent accidental double-import.

## High opportunities

- **Field hover on dot-access** — `p.health` cursor on `health` returns None; should look up via `expr_types` + shape_table.
- **Type-mismatch quick-fix** — wrap `int → string` mismatch with `.toString()` automatically. Common error class has no lightbulb.

## Medium opportunities

- **`prepareRename` on keyword silently does nothing** — should emit "cursor is on keyword X — place on identifier Y" message.
- **Multi-symbol import quick-fix destructively deletes whole line** instead of pruning unused name.

## What's working well

- Rename error messages follow WHAT/WHAT-INSTEAD/WHY perfectly with concrete examples
- Format-on-save emits visible signal on formatter failure (right pattern)
- `let_to_const_promotion` click-to-make-explicit is well-built
- Cross-file completion deduplication is correct
- Atomic rename guarantee is explicit and enforced
- `$0` cursor placement in snippets follows quality conventions
