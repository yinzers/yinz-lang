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

**LOCKED 2026-05-20: Prettier-style (full AST reflow).**

Both spikes passed all numeric gates. Prettier-style was chosen for canonicality — a requirement the LOC tie-break cannot override.

### Decision Evidence

**Gate 1 (idempotency)**: both spikes produced byte-identical output over 5 iterations on all 5 fixtures (10/10 fixture × style combinations). Both algorithms converge.

**Gate 2 (comment placement)**: both spikes scored 50/50 = 100% exact placement on 50 curated `//` comments across the fixture suite (> 95% gate threshold).

**Tie-break (LOC)**: rustfmt slightly smaller (376 vs 421 LOC, 10.7% difference — just outside the 10% default-to-prettier window). Nominally favors rustfmt.

**Canonicality override**: the LOC tie-break was designed for genuinely equivalent approaches. These approaches are NOT equivalent on the key metric that matters for Yinz:

| Scenario | Prettier output | Rustfmt output |
|----------|-----------------|----------------|
| 103-char signature written on one line | Multi-line (>100 break) | Single-line (original preserved) |
| Same signature written multi-line by a different author | Multi-line | Multi-line |

Same program, same formatter, **two different outputs** under rustfmt. This violates Yinz's foundational "zero config, one canonical output" mandate. The tie-break is irrelevant when one approach violates the design requirement.

**Prettier chosen.**

### Algorithm Description (Locked)

Prettier-style: full AST reflow. Original whitespace is discarded. Output is a pure function of the AST.

- **Function signatures**: if the single-line form (all params on one line) is ≤100 chars, emit single-line. If >100 chars, emit each param on its own line at +2 indent from the function keyword.
- **Body statements**: 2-space indent. Each statement on its own line.
- **Comment attachment**: leading comments (within 2 lines above a stmt, no blank line between) stay with their stmt. Inline comments (on the same source line as code) emit inline with 2 spaces between code and `//`.
- **Backtick strings**: opaque units. Never re-flowed. Reconstructed from AST parts (preserves byte content of all interpolated expressions).
- **Blank lines**: ≤1 blank line between top-level declarations. Preserved from original if user had 1+; suppressed if user had 0. **Inside function bodies (between statements): blank lines are NOT preserved.** The canonical form produces zero blank lines between statements. Rationale: top-level blank lines are logical section separators (between exported functions, shapes, etc.) and are meaningful to preserve; blank lines inside function bodies are stylistic whitespace-preferences that vary per developer and would create non-canonical output. The formatter's job is one canonical form; inside bodies, that form has no inter-statement blanks.
- **Trailing newline**: always added if missing.

### Why This Was Empirically Verified (Not Assumed)

The Phase 1 spike built both algorithms (~400 LOC each) against a representative fixture suite and ran the decision gates numerically. The canonicality argument was known in advance (it's a first-principles consequence of the design mandate), but the spike produced the empirical evidence:
1. Gate 1 proves both approaches CAN achieve idempotency — neither has an inherent idempotency deficit on Yinz-sized programs.
2. Gate 2 proves comment placement is achievable with either approach — the comment-attachment algorithm design in Phase 3 is the bottleneck, not the choice of algorithm.
3. The only observable difference is the canonicality property — which is precisely what the algorithm choice affects.

Future re-litigation of this decision would need to present a Yinz use case where full AST reflow produces an unacceptable user experience AND where preserving user intent is essential. No such case exists under Yinz's "zero config" constraint.

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

Empirical evidence from the Phase 1 spike. Full raw data in `crates/ynz-fmt/_spike/MEASUREMENTS.md` (preserved in git history; deleted after Phase 2 supersedes the spike).

### Fixture Suite

| Fixture | Comments |
|---------|---------|
| `long_signature.ynz` | 6 |
| `nested_expr.ynz` | 6 |
| `comment_heavy.ynz` | 17 |
| `multiline_string.ynz` | 8 |
| `shape_decl.ynz` | 13 |
| **Total** | **50** |

### Gate 1: Idempotency (5 iterations, byte-identical check)

All 10 fixture × style combinations: PASS.

### Gate 2: Comment Placement (50 comments, exact/near/wrong)

Both spikes: 50/50 = 100% exact placement.

### Tie-break

LOC: prettier 421 / rustfmt 376 (10.7% difference — nominally favors rustfmt). Overridden by canonicality argument (see Algorithm Choice above).

### Key Output Difference

`long_signature.ynz` has a 103-char function signature. Prettier breaks it to multi-line (>100 threshold). Rustfmt preserves single-line (original was single-line). Full diff:

```diff
- function computeScore(
-   name: string,
-   health: int,
-   attack: int,
-   defense: int,
-   speed: int,
-   level: int
- ) -> int {
+ function computeScore(name: string, health: int, attack: int, defense: int, speed: int, level: int) -> int {
```

Prettier output (top) is canonical for any user. Rustfmt output (bottom) is only canonical if the original was single-line — a different user writing the same function multi-line would get the top form.

---

## Cross-References

- `design/compiler-language.md` — why Rust was chosen for the compiler implementation
- `design/feature-registry.md` — registry schema + carve-out policy (formatter is consumer-only)
- `design/lsp.md` — format-on-save wiring (textDocument/formatting deferred to v0.2-M5)
- `.claude/rules/inference.md` — muted-hint protocol (consumption deferred to v0.2-M5)
- `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` — parent roadmap
- `crates/ynz-registry/src/lib.rs` — registry consumer adapters
- `crates/ynz-parser/src/lexer.rs` — comment-handling refactor point
