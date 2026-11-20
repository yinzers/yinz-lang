---
name: "IMP-fmt"
description: "Architecture of the 'ynz fmt' formatter: its zero-config/safe/idempotent/registry-driven/teachable goals and the lex-parse-format pipeline."
tags:
  - "yinz-compiler"
created_at: "2026-05-20"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# `ynz fmt` — Formatter Architecture

> Spec counterpart: _none yet (formatter is tooling; no language spec needed)_
> Related: [`docs/internal/implementation/IMP-lsp.md`](IMP-lsp.md) (format-on-save wiring, shipped in v0.2-M5), [`docs/internal/implementation/IMP-feature-registry.md`](IMP-feature-registry.md) (registry consumer)

---

## Goals

1. **Zero config.** One canonical output per AST. No `.ynzfmt.toml`, no opt-in flags that change formatting behavior. Disagreement is between the user and the formatter, not between formatter and formatter-config.
2. **Safe.** `parse(fmt(x)).ast == parse(x).ast` modulo trivia — the formatter never changes what a program does.
3. **Idempotent.** `fmt(fmt(x)) == fmt(x)` for every input the parser accepts.
4. **Registry-driven.** Keyword spellings, banned-jargon, and reserved identifier names come from `ynz-registry` — never a forked copy of the keyword list.
5. **Teachable.** Error messages use the same `ynz-diagnostics` machinery as `ynz build` — WHAT/WHAT-INSTEAD/WHY format, no opaque error codes.

---

## Architecture

```
Source text
    │
    ▼
lex_with_trivia()  ─────────────────────────────────────────────────►  Vec<Comment>
    │                                                                        │
    ▼                                                                        │
parse()  ──────────────────────────────────────────────────────────►  Module (AST)
    │                                                                        │
    ▼                                                                        │
comment_merge::attach()  ◄───────────────────────────────────────────────────┘
    │
    ▼
walker::emit_module()
    │
    ▼
Formatted source text
```

**Key layers:**

| Layer | Crate | Role |
|-------|-------|------|
| `lex_with_trivia` | `ynz-parser` | Additive trivia-capturing lexer pass — produces the same token stream as `lex()` PLUS a `Vec<Comment>` with byte-spans for every `//` and `///` token |
| `parse` | `ynz-parser` | Existing recursive-descent parser — unchanged |
| `comment_merge` | `ynz-fmt` | Attaches each `Comment` to the nearest AST node by byte-position |
| `walker` | `ynz-fmt` | AST-to-string emitter; one method per node type; consumes attached comments |
| `render` | `ynz-fmt` | Primitive renderers: indent, separator, line-break budget |
| `line_break` | `ynz-fmt` | Long-line handler: when an expression's emitted form exceeds `LINE_WIDTH_LIMIT` (100 chars), split at natural points |

---

## Library API Contract

```rust
// crates/ynz-fmt/src/lib.rs
pub fn format(source: &str) -> Result<String, FmtError>;
pub fn check(source: &str) -> Result<CheckResult, FmtError>;

pub enum CheckResult {
    AlreadyCanonical,
    WouldChange { preview: String },
}

pub enum FmtError {
    ParseError(DiagnosticBucket),   // re-uses ynz-diagnostics types
    InvalidInput(String),           // infra errors (non-UTF-8 stdin, etc.)
}
```

`format(source)` is the only function the LSP `textDocument/formatting` handler needs. It computes a single full-file `TextEdit` (replace `[0..end]` with the formatted output) and returns it. There is no range support.

**Semver stability**: the API above is frozen at the v0.2-M3 tag. Backwards-incompatible changes require a major-version bump once v0.2.0 ships.

---

## CLI Interface

```
ynz fmt <path>           Format a single .ynz file in-place (atomic write)
ynz fmt --all [dir]      Walk the yinz.toml project and format every .ynz file
ynz fmt --check <path>   Read-only: exit 0 if canonical, exit 1 + print which
ynz fmt --stdin          Read source on stdin, write formatted to stdout
```

**Flag conflict matrix** (enforced via clap `conflicts_with`):

| Flag combination | Result |
|---|---|
| `--all` + `--check` | ACCEPT — read-only check across whole project, common CI use case |
| `--all` + `--stdin` | REJECT — incompatible: `--all` needs a project root; `--stdin` has none |
| `--check` + `--stdin` | REJECT — no CI pipeline that pipes stdin to a checker without a file |
| `<path>` + `--stdin` | REJECT — `--stdin` takes no path argument |

**Exit codes** (consistent with `ynz build`):

| Code | Meaning |
|---|---|
| 0 | Success / already canonical |
| 1 | Source has parse errors OR (for `--check`) file(s) would change |
| 2 | Infrastructure error: can't read/write file, missing `yinz.toml` for `--all` |

---

## Comment Handling

The lexer (`crates/ynz-parser/src/lexer.rs:73`) currently skips `//` line comments via `skip_whitespace_and_comments()` — only `///` doc-comments survive into the AST as `Token::DocComment`. The formatter must reconstruct `//` comment positions.

**Approach**: additive `lex_with_trivia()` function in `ynz-parser` — same lexer logic but a `Vec<Comment>` captures every `//` + `///` token with its byte-span. Implementation: Internal `Lexer::lex_capturing(trivia: bool)` method shared by both `lex()` (trivia=false) and `lex_with_trivia()` (trivia=true). Single source of truth; no parallel copy.

```rust
// crates/ynz-parser/src/trivia.rs
pub struct Comment {
    pub kind: CommentKind,
    pub text: String,
    pub span: SourceSpan,
}

pub enum CommentKind {
    LineComment,   // //
    DocComment,    // ///
}
```

**Comment attachment rules** (the main cases):

- **Leading**: a `//` block whose end-byte is immediately before a decl (separated by whitespace + ≤1 blank line) attaches to that decl and moves with it.
- **Inline**: a `//` comment on the same line as code, after the code — stays on that line (2 spaces before `//`).
- **Block**: consecutive `//` lines with no blank between → grouped as one `CommentBlock`.
- **Floating**: comment too far from the next decl (>1 blank line) — emitted at original byte position, not attached.

Doc comments (`///`) are handled separately: the AST already carries them; the walker emits them from the AST directly to avoid double-emission.

---

## Algorithm Choice

**Decision in force: full AST reflow.** Original whitespace is discarded, and the same program always formats to the same output, whatever layout the author wrote (the "one canonical output" goal above).

### Algorithm Description

Prettier-style: full AST reflow. Original whitespace is discarded. Output is a pure function of the AST.

- **Function signatures**: if the single-line form (all params on one line) is ≤100 chars, emit single-line. If >100 chars, emit each param on its own line at +2 indent from the function keyword.
- **Body statements**: 2-space indent. Each statement on its own line.
- **Comment attachment**: leading comments (within 2 lines above a stmt, no blank line between) stay with their stmt. Inline comments (on the same source line as code) emit inline with 2 spaces between code and `//`.
- **Backtick strings**: opaque units. Never re-flowed. Reconstructed from AST parts (preserves byte content of all interpolated expressions).
- **Blank lines**: ≤1 blank line between top-level declarations. Preserved from original if user had 1+; suppressed if user had 0. **Inside function bodies (between statements): blank lines are NOT preserved.** The canonical form produces zero blank lines between statements. Top-level blank lines separate logical sections, so they are kept; blank lines inside bodies are personal style, so the canonical form has none.
- **Trailing newline**: always added if missing.

---

## Line-Width Handling

Long-line constant: `LINE_WIDTH_LIMIT = 100`.

When `emit_expr` produces a string longer than the budget:
- Function call with N args: each arg on its own line, indented +2.
- Array / map / shape literal with N elements: each element on its own line.
- Operator chain (`a + b + c`): break before each operator at +2 indent.
- Backtick strings: **never re-flowed** — the entire literal is treated as an opaque unit.
- Single identifier longer than the limit: **emitted unbroken** — no arbitrary break.

---

## Registry Use

The formatter consumes `ynz-registry` (crate `crates/ynz-registry`) for:
- Keyword spellings — formatter always emits the canonical spelling from the registry, immune to drift if a keyword is renamed in a future vN.
- Banned-jargon list — any diagnostic text the formatter emits is checked against `banned_jargon()` in CI.
- Reserved-identifier protection — `deferred_language_feature_lookup(name)` guards against the formatter ever emitting an identifier that collides with a deferred feature name.

The formatter is a registry consumer only; it adds no registry entries.

---

## Atomic Writes

Single-file rewrites use the same-directory tempfile + rename pattern:

```
1. write formatted bytes to `<dir>/<file>.ynz.tmp.<pid>`
2. rename (atomic on POSIX) `<file>.ynz.tmp.<pid>` → `<file>.ynz`
```

The tempfile MUST be in the same directory as the target (not `/tmp/`) to avoid `EXDEV` on cross-mount renames. If `formatted == source`, the file is not touched (no spurious mtime bump).

`--check` mode reads files in memory only — no filesystem writes under any path.

---

## Symlink Behavior

`ynz fmt` resolves symlinks (consistent with rustfmt / prettier defaults). `std::fs::read_to_string` follows symlinks transparently. The tempfile+rename write replaces the symlink with a regular file at the symlink path. Documented in `ynz fmt --help` output: "follows symlinks; rewriting a symlinked file replaces the symlink with a regular file." Users who want to preserve symlinks pre-resolve with `realpath` and format the target directly.

---

## Concurrency Model

`ynz fmt` is designed for one process at a time per file. Two `ynz fmt` processes racing on the same file → last-rename-wins (idempotent in steady state because both processes write the same canonical output). No file locking is used. Documented in CLI help.

---

## Future Proofing

- **`textDocument/formatting` LSP wiring** — shipped in v0.2-M5; the LSP calls the library API (`format(source: &str) -> Result<String, FmtError>`).
- **`format_range(source, range)`** — not built; whole-file formatting is what format-on-save uses.
- **Embedded SQL formatting** — deferred to database stdlib milestone (v0.5+).
- **Embedded Markdown / regex / JSON** — not designed; v1+ at earliest.
- **Import sorting** — belongs to Tier 3 lint suggestions (v0.4), not the formatter.
- **Self-hosting migration** — formatter rewritten in Yinz when self-hosting lands (v2+).

---

## Cross-References

- [`docs/internal/implementation/IMP-compiler-language.md`](IMP-compiler-language.md) — Rust compiler stack
- [`docs/internal/implementation/IMP-feature-registry.md`](IMP-feature-registry.md) — registry schema + carve-out policy (formatter is consumer-only)
- [`docs/internal/implementation/IMP-lsp.md`](IMP-lsp.md) — format-on-save wiring (`textDocument/formatting`, shipped in v0.2-M5)
- [`.claude/rules/inference.md`](../../../.claude/rules/inference.md) — muted-hint protocol
- `crates/ynz-registry/src/lib.rs` — registry consumer adapters
- `crates/ynz-parser/src/lexer.rs` — comment-handling refactor point
