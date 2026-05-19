# Adversarial Test Analysis — Yinz Compiler

**Scope**: lexer, parser, project loader, import resolver, banned-jargon scanner.
**Findings**: 15 scenarios — 1 fixed Batch 4b, 6 fixed Batch 5a (Critical: 0 remain, High: 1 remain, Medium: 3 remain, Low: 3 remain)

---

~~## CRITICAL — Finding 2: Deeply nested expressions → parser stack overflow~~ **FIXED (Batch 5a)**

`expr_depth` field added to `Parser`; checked at 256 in `parse_expr`; overflow emits WHAT/WHAT-INSTEAD/WHY diagnostic and drains to statement boundary. Test in `crates/ynz-parser/tests/parse.rs` (`hardening_expression_nesting_depth_cap`).

## ~~CRITICAL — Finding 4: Symlink loop in project tree → infinite recursion~~ FIXED (Batch 4b)

`collect_ynz_files` uses `symlink_metadata()` to detect symlinks without following them. Canonical-path `HashSet` prevents re-visiting directories (catches hard-link cycles). Teaching diagnostic on detection per WHAT/WHAT-INSTEAD/WHY format.

~~## CRITICAL/HIGH — Finding 11: NUL byte in string literal → silent runtime truncation~~ **FIXED (Batch 5a — lexer side)**

Lexer now rejects `\0` escape with WHAT/WHAT-INSTEAD/WHY diagnostic instead of pushing the NUL byte. Codegen-side is moot — the lexer refuse ensures the AST never contains a NUL. Future `{ptr, len}` string representation (per `design/strings.md`) can revisit if there is a use case for embedded NULs.

~~## HIGH — Finding 1: Multi-byte Unicode chars produce N errors per codepoint~~ **FIXED (Batch 5a)**

`lex_one` fallthrough now advances by `utf8_codepoint_len(first_byte)` and emits one diagnostic per codepoint. `日本語` produces 3 diagnostics, not 9. Test: `hardening_unicode_error_one_diagnostic_per_codepoint`.

~~## HIGH — Finding 3: Extremely long identifier (10 MB) → unbounded heap allocation~~ **FIXED (Batch 5a)**

`lex_identifier_or_keyword` checks `pos - start > 1024` after consuming characters, emits diagnostic, emits empty Identifier token for recovery. Test: `hardening_identifier_too_long_produces_diagnostic`.

## ~~HIGH — Finding 9: Concurrent `ynz build` races on same `obj_path`~~ FIXED (Batch 4b)

Cross-references reliability.md Finding 3. All intermediates in `tempfile::tempdir()`.

## HIGH — Finding 10: `ariadne` render with out-of-range span → `.expect()` panics

- **File**: `crates/ynz-diagnostics/src/render.rs:85`
- **Trigger**: Diagnostic constructed with a span past EOF (e.g., bug in a recovery path)
- **Outcome**: `.expect("ariadne render failed")` panics; no diagnostic bucket entry; user sees raw panic
- Cross-references forensics.md High finding.

## MEDIUM — Finding 5: yinz.toml `entry` path traversal (currently inert)

- **Trigger**: yinz.toml with `entry = "../../../etc/passwd"`
- **Outcome**: Currently safe because read-only and constrained to project root, but no explicit validation
- **Fix**: Reject any `entry` not resolving inside the project root; emit error

## MEDIUM — Finding 7: yinz.toml unreadable → silent fallback to defaults

- **Trigger**: `chmod 000 yinz.toml` then `ynz build`
- **Trace**: `crates/ynz-driver/src/load.rs` silently uses defaults instead of erroring
- **Fix**: Distinguish "no manifest" from "manifest exists but unreadable" — error on the latter

## MEDIUM — Finding 13: Import resolver re-reads same files (O(n·m) disk reads)

- **File**: `crates/ynz-typeck/src/resolve_import.rs:278-279` (acknowledged at L273-276)
- **Trigger**: 100 files importing the same `shared/types.ynz`
- **Trace**: `load_export_table` creates a fresh `SourceFile` per call, defeating Salsa memoization
- **Fix**: Look up canonical `SourceFile` from the pre-registered driver registry instead of `SourceFile::new`. Comment already documents the deferred optimization.

## LOW — Finding 6: Circular import reported at second level not first

- **Trigger**: `a` imports `b`; `b` imports `a`
- **Outcome**: Error fires when resolving `a` from `b`'s context, not on the original first-level edge — slightly confusing
- **Fix**: Pre-traverse the import graph with a visited set before resolving exports

## LOW — Finding 8: Project-build UTF-8 error path inferior to single-file path

- **Trigger**: Source file with invalid UTF-8 bytes
- **Trace**: Single-file path emits a clean diagnostic; project path collapses into a generic load error
- **Fix**: Unify the load path so both routes produce the same diagnostic

## LOW — Finding 12: `DiagnosticBucket::push` O(n) error count (bounded by cap)

- **File**: `crates/ynz-diagnostics/src/bucket.rs:30-34`
- Cross-references performance.md Critical finding. Acceptable in practice given the 50-error cap but a design smell. Fix together.

## LOW — Finding 14: `build.rs` collects errors into Vec just to check `is_empty()`

- **File**: `crates/ynz-driver/src/build.rs:55-62`
- **Fix**: use existing `DiagnosticBucket::has_errors()` instead of `iter().filter().collect::<Vec<_>>().is_empty()`

~~## LOW — Finding 15: Multi-line `"..."` string stops at newline → cascade errors~~ **FIXED (Batch 5a)**

`lex_double_quote_error` now recovers to the next closing `"`, blank line, or EOF — whichever comes first. Emits ONE diagnostic pointing at the opening `"` (span start+1 only). Test: `hardening_double_quote_multiline_one_error`.

---

## Priorities

1. **Critical 2 & 4 & 11** — deterministic-crash bugs (deep nesting, symlink loop, NUL truncation). Any of these can be triggered by a single source file or a one-line `ln -s`.
2. **High 3 & 10** — DoS via giant identifier; ariadne render panic
3. **High 1** — Unicode error cascade
4. **High 9** — concurrent build race (already in reliability.md)
5. Medium / Low items per file
