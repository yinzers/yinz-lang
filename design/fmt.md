# `ynz fmt` — Formatter Architecture

> Spec counterpart: _none yet (formatter is tooling; no language spec needed)_
> Related: `design/lsp.md` (format-on-save wiring lands in v0.2-M5), `design/feature-registry.md` (registry consumer)

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

`format(source)` is the only function v0.2-M5's LSP `textDocument/formatting` handler needs. It computes a single full-file `TextEdit` (replace `[0..end]` with the formatted output) and returns it. No range support in M3 — that's explicitly deferred to M5 if proven necessary.

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

**Approach**: additive `lex_with_trivia()` function in `ynz-parser` — same lexer logic but a `Vec<Comment>` captures every `//` + `///` token with its byte-span. Implementation: internal `Lexer::lex_capturing(trivia: bool)` method shared by both `lex()` (trivia=false) and `lex_with_trivia()` (trivia=true). Single source of truth; no parallel copy.

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

**Comment attachment rules** (locked at plan time — see `.claude/plans/active/v0-2-m3-fmt.md` Phase 3 spec table for the full 15-case matrix):

- **Leading**: a `//` block whose end-byte is immediately before a decl (separated by whitespace + ≤1 blank line) attaches to that decl and moves with it.
- **Inline**: a `//` comment on the same line as code, after the code — stays on that line (2 spaces before `//`).
- **Block**: consecutive `//` lines with no blank between → grouped as one `CommentBlock`.
- **Floating**: comment too far from the next decl (>1 blank line) — emitted at original byte position, not attached.

Doc comments (`///`) are handled separately: the AST already carries them; the walker emits them from the AST directly to avoid double-emission.

---

## Algorithm Choice

_(This section is a placeholder — Phase 1 research spike fills it with the locked decision + empirical measurements from `_spike/MEASUREMENTS.md`.)_

**Candidates under evaluation:**

- **Prettier-style (full reflow)**: discard ALL original whitespace; emit canonical form from AST. Trivially idempotent. Must reconstruct comment positions from byte-spans.
- **Rustfmt-style (preserve-some-intent)**: keep user's blank-line count; keep user's choice between single-line and multi-line forms when both fit in 100 chars. Harder to make idempotent; more edge cases.

**Decision criterion** (locked — numeric, no vibes):

- Gate 1 (binary): idempotency byte-identical over 5 iterations on every fixture.
- Gate 2 (numeric): comment-placement accuracy ≥ 95% exact across ≥50 curated comments.
- Tie-break: smaller spike LOC count; default to prettier-style within 10% LOC.

Decision to be recorded here after Phase 1 spike.

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

No new registry entries are added in M3 (formatter is a consumer-only milestone).

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

- **`textDocument/formatting` LSP wiring** — deferred to v0.2-M5. The library API (`format(source: &str) -> Result<String, FmtError>`) is the only hook M5 needs.
- **`format_range(source, range)`** — deferred to v0.2-M5 if proven necessary. Range formatting is hard; whole-file is enough for format-on-save.
- **Embedded SQL formatting** — deferred to database stdlib milestone (v0.6+).
- **Embedded Markdown / regex / JSON** — not designed; v1+ at earliest.
- **Import sorting** — belongs to Tier 3 lint suggestions (v0.4), not the formatter.
- **Self-hosting migration** — formatter rewritten in Yinz when self-hosting lands (v2+).

---

## Algorithm Spike Measurements (v0.2-M3 Phase 1)

_(To be filled in by Phase 1. Will contain: side-by-side formatted output per fixture, idempotency results, comment-placement accuracy scores, LOC counts, decision rationale.)_

---

## Cross-References

- `design/compiler-language.md` — why Rust was chosen for the compiler implementation
- `design/feature-registry.md` — registry schema + carve-out policy (formatter is consumer-only)
- `design/lsp.md` — format-on-save wiring (textDocument/formatting deferred to v0.2-M5)
- `.claude/rules/inference.md` — muted-hint protocol (consumption deferred to v0.2-M5)
- `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` — parent roadmap
- `crates/ynz-registry/src/lib.rs` — registry consumer adapters
- `crates/ynz-parser/src/lexer.rs` — comment-handling refactor point
