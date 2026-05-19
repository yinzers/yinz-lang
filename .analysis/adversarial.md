# Adversarial Test Analysis — Yinz Compiler

**Scope**: lexer, parser, project loader, import resolver, banned-jargon scanner.
**Findings**: 15 scenarios — 1 fixed Batch 4b (Critical: 2 remain, High: 4, Medium: 3, Low: 5)

---

## CRITICAL — Finding 2: Deeply nested expressions → parser stack overflow

- **Trigger**: A `.ynz` file containing `((((...((x))...))))` with deep nesting
- **Trace**: Recursive descent `parse_primary` / `parse_paren_expr` (no depth budget)
- **Outcome**: Stack overflow, process crash, no output
- **Fix**: Track depth via `&mut self.depth`, cap (e.g. 256), emit `Diagnostic::error("expression nesting too deep")`

## ~~CRITICAL — Finding 4: Symlink loop in project tree → infinite recursion~~ FIXED (Batch 4b)

`collect_ynz_files` uses `symlink_metadata()` to detect symlinks without following them. Canonical-path `HashSet` prevents re-visiting directories (catches hard-link cycles). Teaching diagnostic on detection per WHAT/WHAT-INSTEAD/WHY format.

## CRITICAL/HIGH — Finding 11: NUL byte in string literal → silent runtime truncation

- **Trigger**: `` `hello\0world` `` in source
- **Trace**: `crates/ynz-parser/src/lexer.rs:782-784` accepts `\0` escape; `crates/ynz-codegen/src/emit.rs:2850-2851` emits as C string with terminator; runtime `print()` stops at first NUL
- **Outcome**: Silent data loss — `print(x)` outputs `hello`, drops `world`. No compile error, no runtime error.
- **Fix**: Either (a) reject `\0` in string literals at lexer, or (b) represent strings as `{ptr, len}` slices throughout. Aligns with `design/strings.md` UTF-8/SSO model.

## HIGH — Finding 1: Multi-byte Unicode chars produce N errors per codepoint

- **Trigger**: Source containing `日本語` outside an identifier/string
- **Trace**: Lexer error path emits a diagnostic per byte rather than per codepoint; ariadne renders garbled spans
- **Fix**: Advance lexer by full codepoint length on error; emit one diagnostic per invalid codepoint

## HIGH — Finding 3: Extremely long identifier (10 MB) → unbounded heap allocation

- **Trigger**: Single identifier `aaa...a` of 10 MB
- **Trace**: `crates/ynz-parser/src/lexer.rs:486-639` accumulates chars into `String` without bound
- **Fix**: Cap identifier length (suggest 1024 bytes), emit diagnostic on overflow

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

## LOW — Finding 15: Multi-line `"..."` string stops at newline → cascade errors

- **File**: `crates/ynz-parser/src/lexer.rs:652`
- **Trigger**: `"hello\nworld"` with a literal newline
- **Outcome**: Lexer stops at `\n`; second line is parsed as code, producing cascade of unrelated errors
- **Acceptable but improvable**: emit a single "unterminated string literal" pointing at the opening quote

---

## Priorities

1. **Critical 2 & 4 & 11** — deterministic-crash bugs (deep nesting, symlink loop, NUL truncation). Any of these can be triggered by a single source file or a one-line `ln -s`.
2. **High 3 & 10** — DoS via giant identifier; ariadne render panic
3. **High 1** — Unicode error cascade
4. **High 9** — concurrent build race (already in reliability.md)
5. Medium / Low items per file
