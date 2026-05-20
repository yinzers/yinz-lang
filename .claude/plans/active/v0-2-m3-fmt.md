---
slug: v0-2-m3-fmt
type: execution
owner: Patrick Rizzardi
status: active
roadmap: v0-2-dev-loop-tooling
created: 2026-05-20
last_updated: 2026-05-20
review_rounds:
  - round: 1
    reviewer: plan-reviewer
    verdict: BLOCK
    required_fixes_addressed: 8/8
    concerns_addressed: 4/5  # MEASUREMENTS.md persistence; tempfile location; --check --stdin conflict; jargon audit scope. (Per-phase rule-reminder block in code-reviewer prompts noted, deferred — now in todos.md as `per-phase-rule-reminder-block-in-code-reviewer-prompts`.)
  - round: 2
    reviewer: plan-reviewer
    verdict: BLOCK
    required_fixes_addressed: 5/5  # Cargo.toml version contingency; entrypoint.ynz path discrepancy + new examples/fmt_demo/messy.ynz demo; assert_cmd workspace dep; Phase 4 mass-rewrite uses library not CLI (atomic-write ordering hole closed); Demo & Error Gallery rule fully met with fmt_demo fixture.
    concerns_addressed: 5/5  # ast_eq sibling-order mutation test #7; Phase 1 fixture comment-counts pre-counted to exactly 50; per-phase rule-reminder moved to todos.md; Phase 5 jargon AC restates scope; UTF-8 behavior locked NOW (trust invariant; convert at boundary).
    adversarial_addressed: 3/3  # concurrent --all processes (single-process assumption locked); symlink rewrite (follows + rename — locked); dangling doc-comment (walker emits as orphan trivia — Phase 2 test added).
  - round: 3
    reviewer: plan-reviewer
    verdict: PASS
    required_fixes_addressed: 0  # none requested
    concerns_addressed: 1/5  # MEASUREMENTS.md retention timing — picked deletion at Phase 2. Other 4 concerns deferred to executor judgment (alphabetical Cargo.toml placement, double-CHANGELOG safety, line-break recursion depth, proptest_smoke threshold calibration).
    adversarial_addressed: 3/3  # inline-comment-EOF, two-adjacent-inline-on-split, long-identifier-no-break — all added to comment-merge spec table + acceptance criteria (now 15 fixture rows).
status_after_round_3: ready_for_patrick_approval
files:
  - crates/ynz-fmt/**
  - crates/ynz-parser/src/lexer.rs
  - crates/ynz-driver/src/**
  - design/fmt.md
  - design/mvp-scope.md
  - CLAUDE.md
  - examples/basics/entrypoint.ynz
  - examples/errors/v0_2_m3_errors.ynz
  - Cargo.toml
depends_on: [v0-2-m1-feature-inventory-sync, v0-2-m2-lsp-thin-slice]
---

# Plan: v0.2-M3 — `ynz fmt`

Created: 2026-05-20
Status: pending_approval

## Context & Why

**Goal**: Ship `ynz-fmt` — a zero-config formatter for Yinz source files — as both a library crate (consumed by v0.2-M5's LSP `textDocument/formatting` handler) AND a CLI subcommand (`ynz fmt <path>`, `ynz fmt --all`, `ynz fmt --check`, `ynz fmt --stdin`). One opinionated style. No `.ynzfmt.toml`. Disagreement is between the user and the formatter, not between formatter and itself.

**Why now**:
- v0.2-M1 shipped the SSOT registry (`crates/ynz-registry/src/lib.rs`) with keyword, banned-jargon, and deferred-feature data the formatter consumes — making the formatter immune to drift when v0.3+ adds new keywords or renames anything.
- v0.2-M2 (LSP Thin Slice) is in verification sweep; once tagged it will surface inline diagnostics, autocomplete, and hover. The next missing piece for editor users is **format-on-save**, which the LSP wires in v0.2-M5 — but only if v0.2-M3 ships the `ynz-fmt` library API first.
- For terminal users (CI, pre-commit hooks, vim users without LSP), `ynz fmt --check` is the canonical formatter gate.
- The roadmap (`v0-2-dev-loop-tooling`) explicitly puts fmt in M3 parallelizable with M2/M4. M2 is wrapping up; M3 starts the moment M2 tag cuts (Phase 0 begins from `main` at the M2 tag commit).

**Background**:
- 11 workspace crates (M2 added `ynz-lsp` and `ynz-tmgrammar`). `Cargo.toml:18` is currently at `0.2.0-m1` (verified 2026-05-20). v0.2-M2's final phase (Phase 9 per `.claude/plans/active/v0-2-m2-lsp-thin-slice.md`) bumps to `0.2.0-m2` and cuts the tag; M3 Phase 0 begins from `main` AFTER that bump lands. **Contingency**: if for any reason M2's tag-cut DOES NOT include the version bump when M3 Phase 0 starts, Phase 0 takes over the `0.2.0-m1 → 0.2.0-m2` bump as a prerequisite step (added to Phase 0 acceptance criteria conditionally — checked at start of Phase 0). Phase 6 of M3 always does the `<previous> → 0.2.0-m3` bump regardless.
- The Yinz parser is hand-written recursive-descent + Pratt for expressions (`crates/ynz-parser/src/parser.rs`, 4118 lines). The lexer (`crates/ynz-parser/src/lexer.rs`, 1314 lines) **skips `//` line comments entirely** — only `///` doc-comments survive into the AST as `Token::DocComment`. The formatter must reconstruct `//` comment positions via a separate trivia-emitting lex pass.
- `crates/ynz-ast/src/nodes.rs` (830 lines) defines every node carrying a `SourceSpan` — byte-offsets the formatter uses to align comments with surrounding nodes.
- Existing source convention (verified by reading `examples/basics/entrypoint.ynz`): 2-space indent, backtick strings, `//` line comments + `///` doc-comments only (no `/* */`), no trailing semicolons.
- `crates/ynz-registry/src/lib.rs:16-80` exposes `keywords()`, `banned_jargon()`, `deferred_language_features()`, etc. — the formatter consumes these for keyword spellings and reserved-name protection (no fork of the keyword list).

**Constraints (locked from roadmap + this planning session)**:
- **Zero config.** No `.ynzfmt.toml`, no opt-in flags that change formatting behavior. One canonical output per AST.
- **CLI flag scope (locked this session)**: `ynz fmt <path>`, `--all` (project mode — walks `yinz.toml` project), `--check` (CI gate — exits non-zero if any file would change, prints which), `--stdin` (read source on stdin, write formatted to stdout — used by LSP/editor "format-on-save" once v0.2-M5 wires it).
- **Comment handling (locked this session)**: re-lex trivia pass — additive `lex_with_trivia()` function in `ynz-parser` returns the existing token stream PLUS a parallel vec of `Comment { kind, text, span }`. Formatter merges comments back into AST output at their original position. No parser changes; no AST changes.
- **Registry use (locked this session)**: formatter consults `ynz-registry` for keyword spellings (so renames in vN+ flow through automatically), banned-jargon avoidance in any formatter-emitted text, and reserved-name protection (formatter won't emit identifiers that collide with a deferred feature).
- **Algorithm choice deferred to Phase 1 research spike** — prettier-style (full reflow, discard original whitespace) vs rustfmt-style (preserve some author intent, normalize edge cases). Spike measures both against the existing examples + a curated set of "hard cases" (long function signatures, deeply nested expressions, comment-heavy code) and locks the choice.
- **Formatter NEVER alters program semantics.** `parse(fmt(x)).ast == parse(x).ast` modulo trivia is the load-bearing safety invariant. Verified via property test: round-trip every example.
- **Idempotency is non-negotiable.** `fmt(fmt(x)) == fmt(x)` for every input the parser accepts. Verified via property test + golden files.
- **All compile errors continue WHAT/WHAT-INSTEAD/WHY format** — formatter errors (only kind: "source has parse errors; fix those first") use the same Diagnostic constructor as the rest of the compiler.
- **No new language features.** Formatter is pure tooling — adds zero tokens, zero AST nodes, zero typeck/codegen behavior.
- **Existing 830+ tests must still pass.** New tests added; no existing tests weakened.
- **Compiler binary's behavior on `.ynz` files unchanged** — `ynz build` / `ynz run` byte-identical to pre-M3 for every existing fixture. `ynz fmt` is a new subcommand; the others don't change.

**Out of M3 scope (deferred — see Deferrals table at end)**:
- `textDocument/formatting` / `textDocument/rangeFormatting` LSP handler wiring — that's v0.2-M5's job; M3 ships the library API M5 will call.
- `format_range(source, range)` API for LSP range-formatting — M3 ships whole-file formatting only. Range formatting is genuinely hard (where do you re-flow? Where do you stop?) and the LSP doesn't need it for format-on-save (which is whole-file). Deferred to v0.2-M5 if proven necessary.
- Embedded SQL formatting inside `sql`...`` template literals — explicitly out of v0.2 per roadmap (deferred to database stdlib milestone, v0.6+).
- Embedded Markdown / regex / JSON inside string literals — not even designed; v1+ at earliest.
- Sorting imports — a Tier 3 lint suggestion concern (v0.4), NOT a formatter behavior. M3 leaves import order untouched.
- Format-as-you-type partial reformatting in the LSP — v0.2-M5 (or later) if at all.

**Success criteria**:
- `ynz fmt examples/basics/entrypoint.ynz` rewrites the file in canonical form; running it again is a no-op (idempotent).
- `ynz fmt --all` in a project dir formats every `.ynz` file under the project root.
- `ynz fmt --check examples/basics/entrypoint.ynz` exits 0 if already canonical, exits 1 with a list of files that would change otherwise.
- `cat foo.ynz | ynz fmt --stdin` writes the formatted result to stdout, exits 0 on success or 1 on parse error.
- `ynz_fmt::format(source: &str) -> Result<String, FmtError>` is the library API. v0.2-M5's LSP calls it from the `textDocument/formatting` handler with no logic in between.
- Every example in `examples/basics/` + `examples/errors/` + `crates/ynz-driver/tests/fixtures/` round-trips: `fmt(file) == file` (the existing examples ARE in canonical form after Phase 4 normalizes them once, then stay canonical).
- `cargo test --workspace` passes (830+ existing + new fmt-specific tests).
- Tag cut: `v0.2.0-m3` (intermediate; v0.2.0 final ships at v0.2-M5).

## Research Findings

**Parser comment-handling (verified 2026-05-20 against `crates/ynz-parser/src/lexer.rs:73-100`)**:
- `skip_whitespace_and_comments()` (line 73): on encountering `//`, scans to end-of-line and discards. On encountering `///`, calls `lex_doc_comment()` which emits `Token::DocComment { content, span }`.
- Implication: `//` comments are LEXED-AWAY before the parser ever sees them. Doc comments `///` survive as tokens and end up on AST nodes (function/shape declarations, per parser.rs:149 and :3615).
- The formatter must reconstruct `//` positions via a separate trivia-emitting pass. **Locked approach**: add a `pub fn lex_with_trivia(source: &str) -> (Vec<SpannedToken>, Vec<Comment>)` to `ynz-parser` — same lexer logic but the `Comment` vec captures every `//` + `///` token with its byte-span. No effect on the existing `lex(source)` path used by parser. Additive.

**Existing source convention (verified 2026-05-20 from `examples/basics/entrypoint.ynz`)**:
- 2-space indent for blocks (functions, if/while/for bodies)
- Backtick strings (`` `text` ``), NOT double-quote strings
- `//` line comments only; no `/* */` block comments observed in any source file (verified `grep '/\*' examples/ crates/*/tests/fixtures/`)
- `///` doc comments on function declarations only (per M8 doc-comment work)
- No trailing semicolons
- Block-bracket placement: opening `{` on same line as keyword (function/if/while/for/shape declaration), closing `}` on its own line
- Operator spacing: `a + b`, not `a+b`
- One blank line between top-level declarations; no blank line at start/end of block bodies
- Comments before a declaration "belong to" that declaration (move with it)
- Inline comments (after code on same line) STAY inline

**These are observations of the existing code**, not yet locked-in style rules. Phase 1's research spike confirms which to keep, which to normalize.

**Registry consumer surface (verified against `crates/ynz-registry/src/lib.rs`)**:
- `keywords() -> impl Iterator<Item = &'static KeywordEntry>` — formatter uses this for keyword spelling validation (the parser already validates input, but the formatter must EMIT correct keyword spellings if any transformation ever requires it).
- `banned_jargon()` — if the formatter ever emits diagnostic text (e.g., for `--check` mode: "would rewrite foo.ynz"), the text must pass the jargon audit.
- `deferred_language_feature_lookup(name)` — used by Phase 1 spike's reserved-name protection: the formatter doesn't rename identifiers, but a future feature (auto-rename / quick-fix) might; locking the protection now saves work later.
- No NEW registry adapter needed for M3 (formatter is a consumer of EXISTING data; adapters are sufficient).

**Algorithm comparison (preliminary, locked in Phase 1 spike)**:
- **Prettier-style (full reflow)**: discard ALL original whitespace; emit canonical form from AST. Pros: trivially idempotent (output is a pure function of AST); no edge cases around "what counts as original intent." Cons: must reconstruct comment positions from byte-spans (we have spans, so OK); long expressions need an explicit "break here if line > N" rule.
- **Rustfmt-style (preserve-some-intent)**: keep user's blank-line count, keep user's choice between single-line and multi-line forms when both are valid. Pros: preserves "I formatted this multi-line on purpose because it's logically grouped" intent. Cons: idempotency is harder (you can re-stabilize, but the rules to do so are subtle); more edge cases.
- For Yinz's "one style, no config" mandate, prettier-style is the stronger default. The spike measures both against actual hard cases before locking. If both fail equally on a class of hard cases, default to prettier-style.

**Library API shape (proposed; locked Phase 2)**:
```rust
// crates/ynz-fmt/src/lib.rs
pub fn format(source: &str) -> Result<String, FmtError>;
pub fn check(source: &str) -> Result<CheckResult, FmtError>;

pub enum CheckResult {
    AlreadyCanonical,
    WouldChange { preview: String },
}

pub enum FmtError {
    ParseError(DiagnosticBucket),  // re-uses ynz-diagnostics types
    InvalidInput(String),
}
```

The LSP in v0.2-M5 calls `format(source)` from its `textDocument/formatting` handler, computes a single full-file `TextEdit` (replace [0..end] with the formatted output), returns it. No range support in M3 API.

**Branching/PR sizing** (per `~/.claude/memory/branching.md`): each phase = one branch off `main`, one PR via `/pr`. Soft target ~500 lines/PR. Phases 2, 3, 4 are the heaviest (formatter logic + comment merge + property tests); Phases 0, 1, 5, 6 are smaller. Phase 6 is the verification sweep + tag.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Algorithm choice (prettier vs rustfmt) wrong, requires migration mid-M3 | Medium | Medium | Phase 1 spike builds BOTH against a curated "hard cases" suite (long signatures, deeply nested exprs, comment-heavy code). Lock decision in `design/fmt.md` so v0.3+ doesn't re-litigate. |
| Comment placement bugs ship — `// comment` ends up on wrong line after format | High | High | Phase 3 dedicated to comment merge. Tests cover: leading comments before declarations, inline comments at end of line, comments inside expressions (rare but possible), comments between elements of an array/map literal. Golden files for each placement. |
| Idempotency bug ships — `fmt(fmt(x)) != fmt(x)` — breaks CI gate users | Medium | High | Phase 4 has explicit `fmt(fmt(x)) == fmt(x)` property test running across all `examples/basics/` + `examples/errors/` + `crates/ynz-driver/tests/fixtures/` content. Proptest fuzz over arbitrary parser-valid input as the deeper guarantee. |
| Formatter alters semantics — `parse(fmt(x)) != parse(x)` modulo trivia | Low | Critical | Phase 4 also includes a semantic round-trip property test: parse the formatted output, compare the AST modulo trivia/spans against the original AST. Fails CI if the formatter ever produces a semantically different program. |
| Re-lex trivia pass diverges from `lex()` (drift between the two lex functions) | Medium | Medium | `lex_with_trivia()` is implemented BY DELEGATING to the existing lexer with a "capture trivia" flag, NOT a copy-pasted parallel function. Single source of truth for tokenization. Phase 2 implements via an internal `Lexer::lex_capturing(trivia: bool)` method that both `lex` and `lex_with_trivia` call with different flags. |
| Backtick string interpolation breaks under format — `${...}` inside `` ` `` strings re-formatted incorrectly | Medium | High | Backtick strings + interpolation are tokenized as a complex multi-token sequence in lexer.rs (BacktickStart / BacktickContent / InterpolationStart / InterpolationEnd / BacktickEnd). Formatter MUST treat the entire backtick literal as a unit — never re-flow content inside. Phase 2 tests cover multi-line backtick strings, single-line interpolation, multi-line interpolation. |
| Doc comments (`///`) get re-flowed and lose user's intended line breaks | Low | Medium | Doc-comment content is preserved BYTE-EXACT — formatter only normalizes the leading whitespace (indent matches surrounding declaration) and ensures `///` prefix on every line. Inner content (the markdown body) is preserved untouched. |
| `--check` mode prints partial output / leaves dirty state | Low | Low | `--check` is read-only by design: it formats in memory, compares, writes nothing. Exit code only. Tests assert no filesystem mutation occurs. |
| Performance regression on large files | Low | Low | Phase 6 budget: format a 5000-line synthesized file in <500ms (single-pass AST walk + string-builder). Salsa not used here (formatter is single-shot per file, no incremental need). |
| `ynz-fmt` library API changes between M3 and M5 LSP wiring | Low | Medium | API frozen end of Phase 5; documented in `crates/ynz-fmt/src/lib.rs` rustdoc; semver-bump-on-change rule applies once v0.2.0 ships (v0.2.0-m3 is pre-release so technically free to break, but the API is small enough that we lock now). |
| Mass-rewrite of existing examples on first M3 run causes huge git diff | Medium | Low | Phase 4 includes a one-shot `ynz fmt --all examples/` commit that normalizes every existing example to the canonical form. Diff is enormous but mechanical; committed as its own PR (Phase 4) so reviewer can audit "no semantic change" cleanly. Subsequent commits stay canonical. |
| Multiple `//` comments adjacent (a comment block) get re-flowed individually instead of preserved as a unit | Medium | Low | Trivia pass groups consecutive `//` lines (no blank line between them) into a single `CommentBlock`. Formatter places the whole block, not line-by-line. Phase 3 tests cover 2-line, 5-line, and 10-line comment blocks. |
| Formatter changes whitespace inside `verified { }` blocks or other future deferred-feature scopes | Low | Low | `verified` doesn't exist yet (v0.3+); reserved by lexer. Same for other deferred features. Formatter falls back to "preserve text byte-exact between unrecognized tokens" if a future deferred-feature scope is encountered — but in practice none of these are valid input today so it's a non-issue until v0.3. Documented in `design/fmt.md` future-proofing section. |
| `ynz fmt` exit codes inconsistent with `ynz build` | Low | Low | Locked: 0 = success / already canonical, 1 = source has parse errors OR (for `--check`) would change, 2 = infra error (can't read/write file, missing project root for `--all`). Same scheme as `ynz build` (`crates/ynz-driver/src/main.rs:11-13`). |
| Plan-invariants rule introduces gap if M3 forgets the 7-subsection block | Low | Low | Plan structure below explicitly contains the 7-subsection `## Invariants This Milestone Must Preserve` block per `.claude/rules/plan-invariants.md`. Bouncer entries 1, 3, 4 enforce. |
| Two `ynz fmt` processes racing on the same file (e.g., editor format-on-save + manual CLI run) | Low | Low | LOCKED single-process tool assumption: `ynz fmt` does not acquire a file lock. Same-dir tempfile + rename means each process writes its own tempfile; last-rename-wins. If the user runs `ynz fmt` from two terminals simultaneously, behavior is "last write wins" (idempotent in steady state). Phase 5 README documents this as "designed for one process at a time per file." |
| `ynz fmt` follows a symlink target and rewrites the underlying file (potentially surprising) | Low | Low | LOCKED behavior: `ynz fmt` resolves symlinks (matches rustfmt / prettier defaults). Reading via `std::fs::read_to_string` follows symlinks; writing via tempfile+rename writes a NEW regular file at the symlink path (rename replaces the symlink with a regular file — destructive). To preserve symlinks intact, formatting tools would have to read-then-write-to-target which loses atomicity. M3 LOCKS the rename approach; documents behavior in `ynz fmt --help` output: "follows symlinks; rewriting a symlinked file replaces the symlink with a regular file." If users want to preserve symlinks, they pre-resolve with `realpath` and format the target directly. |
| Doc-comment immediately preceding `}` block-close with no decl to attach to (could panic the walker if it assumes "every DocComment AST node has a decl partner") | Low | Medium | Phase 2 walker MUST handle "dangling" DocComment tokens (no following decl) — either emit them as orphan-comment in formatted output OR drop them with a stderr warning (LOCKED: emit them at their original position as comment-like trivia; never panic). Phase 2 test fixture `dangling_doc_comment.ynz` covers this; walker has explicit branch. |

## Questions

None outstanding. Three answered this planning session:
1. CLI flag scope: **Full set in M3** — `ynz fmt <path>`, `--all`, `--check`, `--stdin`
2. Comment handling: **Re-lex trivia pass** — additive `lex_with_trivia()` in `ynz-parser`; no parser/AST changes
3. Registry use: **Yes — registry-driven** — formatter reads keyword spellings + banned-jargon + reserved deferred features from `ynz-registry`

Open architectural question for Phase 1 research spike (NOT a blocker; spike decides):
- **Algorithm style: prettier (full reflow) vs rustfmt (preserve some intent)?** Spike measures both. Default to prettier if both clear the bar.

## Risk Assessment & Rollout Strategy

**Risk level: MEDIUM**

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | Compiler tooling |
| Touches auth/permissions | No | No auth |
| Raw SQL / literals | No | No DB |
| Modifies existing data | Yes | Phase 4 rewrites every existing example to canonical form (mechanical, no semantic change — verified by AST round-trip property test) |
| Third-party integration | No | Pure Rust; no external deps beyond what's already in workspace |
| Changes existing endpoints | N/A | Not a service; new CLI subcommand only |
| New feature with no equivalent | Yes | First formatter in Yinz |

**Mitigations applied**:
- AST round-trip property test (semantic safety net) → MEDIUM → LOW
- Idempotency property test + golden files → MEDIUM → LOW
- Algorithm choice locked via empirical spike (not vibes) → MEDIUM → LOW
- Mass-rewrite committed as its own PR with reviewer attention → MEDIUM → LOW for review-blast-radius
- Comment merge dedicated phase with named edge cases → HIGH → MEDIUM

**Rollout plan** (Yinz convention: trunk-based, no production rollout; "rollout" = milestone tag):
1. Each phase: branch from main, PR via `/pr`, code-reviewer agent at phase boundary, merge to main on PASS
2. Phase 6 (final verification + tag): cut `v0.2.0-m3` tag after full test sweep + every example formats idempotently
3. v0.2.0 final tag waits for v0.2-M5 per roadmap

## Invariants This Milestone Must Preserve

### Safety
- All 830+ existing tests pass post-milestone (`cargo test --workspace`)
- `parse(fmt(x)) ~= parse(x)` (AST equality modulo trivia/spans) for every input the parser accepts — verified by Phase 4 property test
- `fmt(fmt(x)) == fmt(x)` (idempotency) for every input the parser accepts — verified by Phase 4 property test
- `ynz fmt` NEVER writes a file unless that file's formatted output differs from its current content (no spurious mtime bumps)
- `ynz fmt --check` writes NOTHING to disk — read-only operation
- `ynz build` and `ynz run` exit codes, stdout, stderr are byte-identical to pre-M3 for every existing fixture (formatter is a separate subcommand; doesn't touch existing build/run paths)
- No previously-valid `.ynz` program becomes rejected by the compiler after formatting (semantic preservation invariant)
- No previously-rejected `.ynz` program becomes accepted after formatting (same)
- New crate `ynz-fmt` does NOT depend on `ynz-codegen` (formatter has no need; isolating keeps build-from-scratch fast)
- `ynz-fmt` does NOT depend on `ynz-runtime` (no runtime; pure compile-time tooling)
- `lex_with_trivia()` and `lex()` produce IDENTICAL token sequences for the same input (verified by Phase 2 cross-test) — they share implementation via a single internal `Lexer::lex_capturing(trivia: bool)` method

### Performance

**Targets are HARD CEILINGS, not aspirational.** Phase 6 measurement is a BLOCK gate — exceeding any ceiling REQUIRES a profile + fix, not a budget raise. The ceilings below are calibrated 2x above rustfmt's measured numbers on similar work to leave reasonable headroom while still preventing perf rot.

- Format a 500-line `.ynz` file: ≤100ms cold (release build, dev workstation comparable to GitHub Actions ubuntu-latest); >100ms is an automatic Phase 6 BLOCK pending `cargo flamegraph` + fix
- Format a 5000-line synthetic file: ≤500ms cold; same BLOCK gate
- `ynz fmt --all` over `examples/` + `crates/*/tests/fixtures/` (~100 files, ~5000 total lines): ≤2s wall-clock; same BLOCK gate
- `ynz fmt --check` (read-only): same ceilings as above with no disk-write overhead
- `lex_with_trivia()` overhead vs `lex()`: ≤20% slower (trivia capture is an extra allocation per `//` comment; lex itself unchanged); >20% is an automatic Phase 6 BLOCK
- No salsa dependency in the formatter (single-shot per-file work; no incremental need)

**Auto-promotion analysis** (per `.claude/rules/auto-promotion.md`):
- This milestone does NOT introduce any new language feature, stdlib type, or compiler codegen optimization.
- The formatter is a pure source-to-source transform; no codegen path is affected.
- No codegen auto-promotion candidates. No new muted-hint domain (consumption deferred to v0.2-M5). No Tier 3 lint suggestion (lint tier ships in v0.4).
- Explicitly considered, not forgotten.

### Teaching
- Formatter PARSE errors are reported using the EXISTING `ynz-diagnostics` machinery (WHAT/WHAT-INSTEAD/WHY format). When `ynz fmt foo.ynz` hits a parse error, it prints the same diagnostic `ynz build foo.ynz` would.
- `ynz fmt --check` output is teaching-friendly: "Would reformat: foo.ynz (3 changes)" with optional `--diff` for unified-diff view (deferred to Phase 5 — see CLI scope below if `--diff` ships).
- NEW design doc: `design/fmt.md` — architectural reference: algorithm choice rationale (Phase 1 output), comment merge strategy, library API contract, future-proofing for v0.6+ embedded SQL formatting.
- No new `.claude/rules/` files (no new project-rule surface; `feature-registry.md` already covers the registry-consumer rule M3 follows).
- No new banned-jargon words slip into formatter-emitted text — `tests/jargon_audit.rs` extended in Phase 5 to walk every string the formatter produces (error messages, `--check` output).

### Runtime Dependencies
- `ynz-fmt` crate runtime:
  - `ynz-parser` (internal — for `lex_with_trivia()` and `parse()`)
  - `ynz-ast` (internal — for AST node definitions)
  - `ynz-registry` (internal — for keyword spellings, banned-jargon, reserved-name protection)
  - `ynz-diagnostics` (internal — for error reporting)
  - NO new external deps (no `pretty`/`pretty_assertions`/external pretty-printer crates; we own the formatting logic)
- `ynz-driver` runtime: gains a new `fmt` subcommand handler module (`crates/ynz-driver/src/fmt.rs`); calls `ynz-fmt`'s public API. No new external deps.
- Compiler binary's runtime profile: **identical to pre-M3 for `build`/`run` paths**. New `fmt` subcommand adds zero deps to the existing paths (Rust monomorphization separates the codegen).
- Build-time: no new tools (no `pretty`, no `prost`, no codegen step). `cargo build --workspace` works as before.

### Kernel-Mode Behavior
- `--kernel` build mode is unaffected. The formatter is a developer-machine tool; it does not run in kernel-mode targets.
- The compiler binary's `--kernel` mode behavior on a `.ynz` file is byte-identical to pre-M3.
- No new compile-error path introduced for kernel-mode programs.
- `design/future/no-runtime-mode.md` cross-reference: same status as `ynz-lsp` (M2) — host-tool, not kernel-runtime.

### Demo & Error Gallery

**Path-discrepancy note** (locked this round): `.claude/rules/plan-invariants.md` `### Demo & Error Gallery` says the demo entrypoint lives at `examples/basics/src/entrypoint.ynz`. Actual on-disk path is `examples/basics/entrypoint.ynz` (verified 2026-05-20). The rule's path is STALE — the project was restructured at some point and the rule wasn't updated. **This plan treats the actual on-disk path as canonical** (`examples/basics/entrypoint.ynz`). A separate follow-up will update the rule file (added to `.claude/todos.md` "Later" as `update-plan-invariants-entrypoint-path` in Phase 0 Step 9 alongside the other deferrals).

- `examples/basics/entrypoint.ynz`: ADD a top-of-file comment block: `// Format this file with: ynz fmt examples/basics/entrypoint.ynz — output is byte-identical (file is already canonical).` No NEW Yinz language code added (M3 ships no new language features).
- **NEW dedicated formatter demo** `examples/fmt_demo/messy.ynz` (per Demo & Error Gallery rule spirit — adds executable demo surface for the formatter feature): a deliberately non-canonical `.ynz` file with extra spaces, irregular indent, inline comments at odd positions, etc. Top-of-file comment: `// This file is intentionally non-canonical. Run: ynz fmt examples/fmt_demo/messy.ynz to see the formatter rewrite it. To check without rewriting: ynz fmt --check examples/fmt_demo/messy.ynz (exits 1).` This file is EXCLUDED from Phase 4's mass-rewrite by living OUTSIDE any `yinz.toml` project root (`examples/fmt_demo/` has no `yinz.toml`; `ynz fmt --all` requires one and won't enter the dir). Phase 4's mass-rewrite Step 4 explicitly skips `examples/fmt_demo/`. The fixture stays non-canonical forever as the demo. Verified idempotent in a different sense: `ynz fmt messy.ynz` → outputs the canonical form on stdout/file, but the GIT-checked-in `messy.ynz` stays messy.
- `examples/errors/v0_2_m3_errors.ynz`: NEW file. Intentional triggers for every NEW error path the formatter introduces:
  - Parse error in input (formatter falls back to "print diagnostic, exit 1, write nothing")
  - File not found (infra error, exit 2)
  - `--all` outside a `yinz.toml` project root (infra error: "ynz fmt --all requires a yinz.toml project; pass a path instead")
  - `--check` mismatch (the file would change; exit 1)
  - Each trigger has a `// WHY:` comment naming the diagnostic class (consistent with `v0_2_m1_errors.ynz` and `v0_2_m2_errors.ynz` precedent)
- `insta` stdout/stderr snapshots in Phase 6 for the `v0_2_m3_errors.ynz` CLI render
- Phase 4 includes the one-shot mass-format of all existing examples — after that commit, the demo/error gallery is automatically in canonical form and stays there

### Feature Registry Entries
- **New entries**: NONE. This milestone is registry-CONSUMER work, not registry-producer work. No new keywords, banned-jargon, primitive intrinsics, type-attached constants, deferred features, diagnostic templates, or muted-hint domains.
- **Modified entries**: NONE. The registry contents stay byte-identical to v0.2-M2.
- **Consumer adapters reused (no new ones needed)**: `keywords()`, `banned_jargon()`, `deferred_language_feature_lookup(name)` from `crates/ynz-registry/src/lib.rs`. Phase 2 uses these directly; no new adapter functions added.
- Explicitly considered per the v0.2-M2+ plan-invariants rule.

## Phase Execution Protocol

Each phase ends with an **Exit Sequence** block listing the actions to execute (persist plan state → invoke code-reviewer → handle verdict → prompt commit). Those instructions are commands, not a checklist to tick off.

**Final phase (Phase 6) additionally:**
- Verify ALL phases' acceptance-criteria and quality-gate checkboxes across the plan
- Invoke `code-reviewer` with the **cumulative plan diff**: `git diff <m2-tag>..HEAD`
- Flip `status: active` → `status: done` only after final PASS

## Phases

**Project Shipping Conventions** (per `/plan` Step 4a, detected from project):
- Per-phase ships via `/pr` (project has local `pr` skill at `.claude/skills/pr/`)
- Per-milestone ships via `/release` (project has local `release` skill at `.claude/skills/release/`)

**Sequencing note**: Phase 0 begins from `main` at the v0.2.0-m2 tag commit (M2's final verification cuts that tag). If M2 is still in verification sweep when Patrick approves this plan, Phase 0 BLOCKS until M2 tag lands. Phases 1-6 each branch from main as the previous phase merges. **Phase 0 first step**: verify `Cargo.toml` version is `0.2.0-m2`; if it's still `0.2.0-m1` (M2 tag didn't land for some reason), bump it as part of Phase 0 (carrying M2's deferred step) so M3 has a known base. Document the bump-source in the Phase 0 PR description.

---

### Phase 0: Doc lockdown + crate scaffolding (no behavior change)

**PR scope**: Land `design/fmt.md`, update `design/mvp-scope.md` v0.2-M3 entry, scaffold empty `crates/ynz-fmt/` with `lib.rs` + module stubs + Cargo.toml entry, add a `fmt` subcommand stub to `crates/ynz-driver/src/main.rs` (parses CLI args, prints "not yet implemented", exits 0). No formatting behavior. No driver behavior change for `build`/`run`.
**Branch**: `chore/v0-2-m3-doc-lockdown`
**Flag**: N/A
**Est. lines**: ~450 (design doc ~250, cargo updates ~30, scaffolding stubs ~80, driver subcommand stub ~50, docs ~40)
**Ships via**: `/pr`

**Objective**: Lock the architectural decisions made this planning session into committed docs. Create the crate skeleton so Phase 1's research spike has somewhere to land without tangling production paths.

**Why this phase exists**: prevents Phase 1's research spike from getting confused with permanent code. A spike that lives in a clean `crates/ynz-fmt/_spike/` is easy to delete after the decision is made; without the scaffolding-first phase, the spike risks tangling.

**Current-state anchors**:
- `Cargo.toml:3-15` — workspace member list; M3 adds `ynz-fmt`
- `design/mvp-scope.md:89-93` — v0.2-M3 entry stub; needs expansion with locked decisions
- `crates/ynz-driver/src/main.rs:37-93` — `Cli` and `Command` enums; M3 adds `Fmt` variant
- `CLAUDE.md` Project Layout table — adds `crates/ynz-fmt/` row

**Files (expected scope)**:
- NEW: `design/fmt.md` — architectural reference doc
- EDIT: `design/mvp-scope.md` — v0.2-M3 entry: expand with locked CLI flag set, locked comment-handling approach, locked registry-consumer status; preserve algorithm-deferred-to-Phase-1 placeholder
- EDIT: `CLAUDE.md` — Project Layout table: add `crates/ynz-fmt/` (purpose: "Formatter library — zero-config canonical Yinz formatting, consumed by `ynz fmt` subcommand and v0.2-M5 LSP format-on-save")
- NEW: `crates/ynz-fmt/Cargo.toml` — workspace=true edition/version/authors/license, deps on `ynz-parser`, `ynz-ast`, `ynz-registry`, `ynz-diagnostics`
- NEW: `crates/ynz-fmt/src/lib.rs` — pub API stubs: `format(source: &str) -> Result<String, FmtError>` returning `Err(FmtError::InvalidInput("not yet implemented".into()))`; `check(source) -> ...` same
- NEW: `crates/ynz-fmt/src/error.rs` — `FmtError` enum + `CheckResult` enum
- NEW: `crates/ynz-fmt/_spike/.gitkeep` — placeholder for Phase 1 spike
- EDIT: `Cargo.toml` — (a) add `crates/ynz-fmt` to workspace members; (b) add `ynz-fmt = { path = "crates/ynz-fmt" }` to workspace deps; (c) ADD `assert_cmd = "2"` to `[workspace.dependencies]` (verified NOT a workspace dep today, 2026-05-20 — needed by Phase 5's CLI integration tests). Pinning to major-1 (`"2"`) per workspace convention for dev-deps.
- NEW: `crates/ynz-driver/src/fmt.rs` — stub: `pub fn fmt(_path: &Path, _all: bool, _check: bool, _stdin: bool) -> i32 { eprintln!("ynz fmt: not yet implemented"); 1 }`
- EDIT: `crates/ynz-driver/src/main.rs` — add `mod fmt;` + `Fmt` variant on `Command` enum with all four flags + match arm calling `fmt::fmt(...)`
- EDIT: `crates/ynz-driver/Cargo.toml` — depend on `ynz-fmt`
- EDIT: `.claude/todos.md` — ADD durable-home entries for deferred items per Patrick's `deferrals-must-be-tracked` rule:
  - `- [ ] **lsp-range-formatting** — add `format_range(source, range)` to ynz-fmt library + textDocument/rangeFormatting LSP handler. Deferred from v0.2-M3 (whole-file formatting was enough for editor format-on-save). Pick up IF v0.2-M5 LSP proves a need.`
  - `- [ ] **fmt-diff-mode** — add `ynz fmt --diff` flag emitting unified diff of what would change. Deferred from v0.2-M3 (not blocking ship; useful for code review tooling). No specific trigger; nice-to-have.`
  - `- [ ] **update-plan-invariants-entrypoint-path** — update .claude/rules/plan-invariants.md to point at examples/basics/entrypoint.ynz (NOT src/entrypoint.ynz which is stale; actual path verified 2026-05-20). Trivial doc edit; do whenever passing through the rule file.`
  - `- [ ] **per-phase-rule-reminder-block-in-code-reviewer-prompts** — extend each phase's Exit Sequence code-reviewer prompt to explicitly remind the agent about `~/.claude/rules/comments.md` + Golden Rule 11 WHY-quality + Yinz vocabulary (per agent-dispatch-rule-reminders memory). Deferred from v0.2-M3 round 1 review; non-blocking but tracked.`

**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work. Document each deviation in the PR description; if it's its own concern, split.

**Steps**:
1. Write `design/fmt.md` covering: goals (zero-config, opinionated), library + CLI architecture, comment-handling strategy (re-lex trivia pass — additive `lex_with_trivia()` in `ynz-parser`), registry-consumer status (formatter reads keyword spellings from `ynz-registry`), algorithm-choice placeholder section (filled in by Phase 1), API contract (`format(source) -> Result<String, FmtError>`), future-proofing section (embedded SQL deferred to v0.6+; range-formatting deferred to M5 if needed), self-hosting migration plan (formatter rewrites in Yinz when self-hosting lands v2+)
2. Update `design/mvp-scope.md:89-93` v0.2-M3 entry: enumerate CLI flags (`ynz fmt <path>`, `--all`, `--check`, `--stdin`), state comment-handling approach (re-lex trivia pass), state registry-consumer status, state algorithm-deferred-to-Phase-1 (placeholder)
3. Update `CLAUDE.md` Project Layout: add row for `crates/ynz-fmt/`
4. Scaffold `crates/ynz-fmt/`: Cargo.toml with deps; src/lib.rs with API stubs returning "not yet implemented"; src/error.rs with `FmtError` + `CheckResult` enums; _spike/.gitkeep
5. Add `crates/ynz-fmt` to root `Cargo.toml` workspace members + workspace deps
6. Add `Fmt` variant to `Command` enum in `crates/ynz-driver/src/main.rs` with the four flags; match arm calls `fmt::fmt(...)`
7. Create `crates/ynz-driver/src/fmt.rs` with the stub handler that prints "not yet implemented" and returns exit code 1
8. Add `ynz-fmt` to `crates/ynz-driver/Cargo.toml` deps
9. Append the two deferral entries to `.claude/todos.md` "Later" section (verbatim text from the Files list above) — per Patrick's `deferrals-must-be-tracked` rule; entries must land in this PR so the durable home is in place before the plan moves to `done/`
10. Run `cargo build --workspace` — confirms compilation
11. Run `cargo test --workspace` — confirms no regressions (830+ tests pass)
12. Run `./target/debug/ynz fmt foo.ynz` — confirms stub prints message + exits 1
13. Run `./target/debug/ynz build crates/ynz-driver/tests/fixtures/m3_fib.ynz` — confirms existing build path unchanged

**Acceptance criteria** (observable conditions that define DONE):
- [x] `design/fmt.md` exists with the 7 content sections enumerated in Step 1
- [x] `design/mvp-scope.md` v0.2-M3 entry mentions all locked decisions (CLI scope full set, comment-handling re-lex, registry-consumer, algorithm-deferred-to-Phase-1)
- [x] `CLAUDE.md` Project Layout table has a row for `crates/ynz-fmt/`
- [x] `cargo build --workspace` succeeds with the new empty crate
- [x] `cargo test --workspace` passes (830+ tests, no regressions)
- [x] `./target/debug/ynz fmt --help` prints help for the new subcommand with all four flags listed
- [x] `./target/debug/ynz fmt foo.ynz` prints "not yet implemented", exits 1
- [x] `./target/debug/ynz run crates/ynz-driver/tests/fixtures/m3_fib.ynz` prints `55` (regression check)
- [x] `_spike/` directory exists for Phase 1 to use
- [x] `.claude/todos.md` "Later" section contains `lsp-range-formatting` and `fmt-diff-mode` entries verbatim (per Patrick's `deferrals-must-be-tracked` rule)

**Quality gate** (observable facts to confirm — check BEFORE moving to next phase):
- [x] No `// TODO` / `// FIXME` / `// HACK` left in any new file
- [x] No new banned-jargon in user-facing prose (design/fmt.md is for engineers — "infer" is OK there per `.claude/rules/inference.md` dual-audience disclaimer; never in user-rendered text)
- [x] No `as any` / `#[allow(...)]` swallows
- [x] `design/fmt.md` cross-references `design/compiler-language.md`, `design/feature-registry.md`, `design/lsp.md`, `.claude/rules/inference.md`, roadmap
- [x] No commented-out code; no orphan files
- [x] `cargo clippy --workspace -- -D warnings` passes

**Verification**:
- `cargo build --workspace 2>&1 | tail -5` — clean
- `cargo test --workspace 2>&1 | grep 'test result'` — all pass
- `./target/debug/ynz fmt --help 2>&1` — help text shows all four flags
- `cat design/fmt.md | wc -l` — substantive (>200 lines)

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick this phase's Acceptance + Quality Gate checkboxes; bump `last_updated:` to today.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 0", prompt: "Review the diff for Phase 0 of plan at .claude/plans/active/v0-2-m3-fmt.md against the phase's acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff <m2-tag>..HEAD. Output in your standard format." })`
3. **Handle verdict.** BLOCK → fix → re-invoke (max 3 rounds). PASS → continue.
4. **Prompt user.** "Phase 0 done. Code-reviewer: PASS. Ready to commit and move to Phase 1?"
5. **Do NOT start Phase 1** until user confirms commit.

---

### Phase 1: Algorithm research spike + decision lock

**PR scope**: Build two minimal "format the same Yinz fixture" implementations in `crates/ynz-fmt/_spike/`: one prettier-style (full reflow, discard original whitespace), one rustfmt-style (preserve some author intent). Format a curated "hard cases" suite: long function signatures (15+ params), deeply nested expressions, comment-heavy code, long string literals with interpolation, multi-line shape declarations. Measure each on: idempotency (run 5 times, assert byte-identical output), edge cases handled per loc-of-code, comment-placement accuracy, perceived output quality. Lock the choice; commit decision write-up to `design/fmt.md`. DELETE the losing spike (keep history in git).
**Branch**: `chore/v0-2-m3-algorithm-spike`
**Flag**: N/A
**Est. lines**: ~700 — two ~250-line spikes + curated hard-cases suite (~100 lines of .ynz fixtures) + measurement write-up (~100 lines)
**Ships via**: `/pr`

**Objective**: Resolve the algorithm open question with empirical evidence, not preference. Lock the decision in `design/fmt.md` so v0.3+ doesn't re-litigate when scaling to advanced formatting features.

**Why this phase exists**: Roadmap Risk ("Algorithm choice wrong") is mitigated by a spike-first pattern. Without the spike, Phase 2's real formatter bakes in a decision made on theoretical grounds; the spike costs ~500 lines of throwaway code now to save ~2000 lines of migration in v0.2-M5 if the choice is wrong.

**Current-state anchors**:
- `crates/ynz-fmt/_spike/.gitkeep` from Phase 0
- `crates/ynz-fmt/Cargo.toml` from Phase 0 (deps locked)
- `examples/basics/entrypoint.ynz` (canonical reference for "common Yinz idiom")
- `examples/errors/m4_errors.ynz` through `m8_errors.ynz` (comment-heavy code samples)
- `crates/ynz-parser/src/parser.rs:149` + `:3615` (where doc comments enter the AST — informs how spikes handle them)

**Files (expected scope)**:
- NEW: `crates/ynz-fmt/_spike/prettier_style/Cargo.toml` + `src/main.rs` (~250 lines) — parses input file via existing `ynz-parser`, walks AST, emits canonical form from scratch
- NEW: `crates/ynz-fmt/_spike/rustfmt_style/Cargo.toml` + `src/main.rs` (~250 lines) — parses input file, walks AST + uses span data to preserve user's choice between single-line and multi-line forms when both are valid
- NEW: `crates/ynz-fmt/_spike/fixtures/` — curated hard-cases suite (~5 small `.ynz` files with PRE-COUNTED comment distributions to hit the Gate 2 ≥50-comments floor):
  - `long_signature.ynz` — function with 15+ params, deeply nested generic types — 8 comments (leading on the decl + 6 inline param descriptions + 1 trailing)
  - `nested_expr.ynz` — `a + b * (c - d / (e + f * g))` style; map literals with shape literals inside arrays — 6 comments (inline at each nesting depth)
  - `comment_heavy.ynz` — 30 lines, 20 comments (leading, inline, between elements, plus 3 doc-comment lines on top decl)
  - `multiline_string.ynz` — backtick strings with multi-line content + interpolation — 4 comments (around the string + inside the interpolation expression-context)
  - `shape_decl.ynz` — 8-field shape declaration; 4-field nested anonymous shape literal — 12 comments (1 per field + 2 doc-comment lines + 2 nested-shape inline)
  - **Pre-counted total: 50 comments exactly across 5 fixtures, satisfying Gate 2's ≥50 floor.** If during implementation the actual count differs, the executor expands the smallest fixture until ≥50 total — but the starting target is locked here.
- NEW: `crates/ynz-fmt/_spike/README.md` — what each spike measures, how to run each
- NEW: `crates/ynz-fmt/_spike/MEASUREMENTS.md` — formatted output side-by-side for each fixture, idempotency result, comment-placement accuracy, decision rationale
- EDIT: `design/fmt.md` — replace algorithm-choice placeholder section with locked decision + rationale

**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work (e.g., extending the fixture suite if the existing 5 don't cover a class). Document each deviation in the PR description.

**Steps**:
1. Implement the prettier-style spike: a top-level walker over `Module → Decl* → Stmt* → Expr*` that emits canonical output character-by-character. Discard ALL original whitespace; produce indent + newlines from AST structure. Re-attach comments from a side `lex_with_trivia()` call (prototype — not the production one) by byte-position matching.
2. Implement the rustfmt-style spike: same walker, but when emitting a multi-line construct (function signature, array literal, shape literal), check the original byte-span: if the original was on one line AND fits in 100 chars, emit single-line; else emit multi-line. Preserve user's blank-line count between top-level decls (clamp to ≤1 blank between decls).
3. Each spike formats every fixture in `_spike/fixtures/`. Capture the output to `_spike/output/<style>/<fixture>.ynz`.
4. Idempotency test: format each output 5 more times; assert byte-identical at every iteration. Record failures.
5. Comment-placement test: manually inspect each output's comment positions vs the original. Tally per-fixture: "exact" (same line, same column), "near" (same line, different column), "wrong" (different line or missing).
6. Measure: total LOC of each spike; total LOC of "edge case" branches in each.
7. Write `MEASUREMENTS.md` with the table: per-fixture idempotency, per-fixture comment-placement scores, code size, observed strengths/weaknesses per spike.
8. Apply the decision criterion (LOCKED — numeric, no vibes):
   - **Gate 1 (must clear, binary)**: idempotency BYTE-IDENTICAL over 5 iterations on every fixture in `_spike/fixtures/`. Any single non-idempotent fixture = spike fails Gate 1 = candidate eliminated.
   - **Gate 2 (must clear, numeric)**: comment-placement accuracy ≥ 95% exact (same line, same column) across the curated suite. The curated suite MUST contain at least 50 total comments distributed across the 5 fixtures (count them explicitly in `MEASUREMENTS.md`; if <50, expand fixtures until ≥50). "Exact" means the comment's emitted byte-position matches its semantically-equivalent location in the canonical output as a human reviewer would pick.
   - **Tie-break (when both clear both gates)**: pick the spike with smaller `tokei` LOC count in its source (`src/main.rs` only — `_spike/output/` excluded). If LOC is within 10% of the other, default to prettier-style (simpler model wins).
   - **Failure branch (both fail Gate 1)**: STOP. Do not pick a non-idempotent formatter. Identify the failure class, extend the chosen algorithm (likely prettier — simpler model is easier to patch) with a special-case rule for that class, document the rule in `design/fmt.md`, re-run Gates 1+2. Iterate until one spike clears both. This is NOT "default to prettier with known idempotency bugs" — a non-idempotent formatter is not shippable.
   - **Failure branch (both fail Gate 2)**: STOP. Comment-placement bugs are the highest-bug-density area; shipping <95% accuracy means immediate user dissatisfaction. Add fixtures specifically for the failed comment classes, extend both algorithms' comment-merge logic, re-measure. Iterate until at least one clears Gate 2.
9. Update `design/fmt.md` algorithm-choice section: state the choice, name the measurements that drove it, lock the choice for v0.2-M5+.
10. **Copy MEASUREMENTS.md content into `design/fmt.md`** as a permanent record under a new "Algorithm spike measurements (v0.2-M3 Phase 1)" subsection BEFORE deleting `_spike/`. The git-history-only argument doesn't survive contact with future re-litigation; the design doc must hold the evidence. Then delete the losing spike's directory (preserved in git history). Retain `_spike/MEASUREMENTS.md` + winning spike until Phase 2 supersedes; Phase 2 deletes them both (the content already lives in `design/fmt.md`).

**Decisions made**: Algorithm = **prettier-style** (full AST reflow). Both spikes passed Gate 1 (idempotency) and Gate 2 (50/50 = 100% comment placement accuracy). Prettier chosen for canonicality — same program → same output regardless of original formatting. LOC tie-break (376 vs 421) nominally favors rustfmt but canonicality is not negotiable. Decision locked in `design/fmt.md` "Algorithm Choice" section. Rustfmt spike preserved in git history at commit `051844b`.

**Acceptance criteria**:
- [x] Both spikes built, ran against all 5 fixtures, produced output captured in `_spike/output/`
- [x] `MEASUREMENTS.md` documents the methodology AND recorded values for both spikes (idempotency results, comment-placement scores, LOC counts)
- [x] `MEASUREMENTS.md` records a specific failure-mode-by-fixture matrix (no hand-wavy "rustfmt style felt cleaner")
- [x] `design/fmt.md` algorithm section contains a locked decision with explicit rationale tied to the recorded measurements
- [x] Loser spike directory removed from tree (preserved in git history)
- [x] `cargo build --workspace` succeeds
- [x] `cargo test --workspace` still passes (no behavior change to compiler; spike is opt-in `cargo run -p ynz-fmt-spike-<style>`)

**Quality gate**:
- [x] No `// TODO` / `// FIXME` / `// HACK` left in any retained file
- [x] MEASUREMENTS.md cites specific fixture:line and observed-output pairs; no vague claims
- [x] design/fmt.md algorithm section has the one-line-decision-plus-WHY format
- [x] No new banned-jargon in design/fmt.md
- [x] `cargo clippy --workspace -- -D warnings` passes
- [x] No commented-out code

**Verification**:
- `cargo build --workspace 2>&1 | grep 'warning\|error'` — clean
- `cat crates/ynz-fmt/_spike/MEASUREMENTS.md | grep -E "^(prettier|rustfmt)"` — both styles have measurement rows for each fixture
- `grep -A 10 "## Algorithm choice" design/fmt.md` — locked decision visible

**Exit Sequence:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`. Add a "Decisions made" bullet with the chosen algorithm + one-line WHY.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 1", prompt: "Review the diff for Phase 1 of plan at .claude/plans/active/v0-2-m3-fmt.md. Diff command: git diff main..HEAD. Pay special attention to: did the spike measure ACTUAL output behavior on the fixtures, or theoretical comparisons? Is the algorithm decision tied to evidence in MEASUREMENTS.md? Output in standard format." })`
3. **Handle verdict.** BLOCK → fix → re-invoke. PASS → continue.
4. **Prompt user.** "Phase 1 done. Algorithm locked: <name>. Ready to commit and move to Phase 2?"
5. **Do NOT start Phase 2** until user confirms commit.

---

### Phase 2: Trivia-lex pass + AST walker emitting canonical output (happy path)

**PR scope**: Add `pub fn lex_with_trivia(source: &str) -> (Vec<SpannedToken>, Vec<Comment>)` to `ynz-parser` — additive, shares implementation with existing `lex()` via internal `Lexer::lex_capturing(trivia: bool)`. Implement the chosen algorithm's AST walker in `crates/ynz-fmt/src/walker.rs`. Cover the happy path: every node type emits canonical form. NO comments yet (that's Phase 3). Tests cover every AST node type against a golden file.
**Branch**: `feat/v0-2-m3-walker-canonical`
**Flag**: N/A
**Est. lines**: ~900 (trivia lex ~150 — mostly refactor + new public fn + tests; walker ~500; golden file tests ~250)
**Ships via**: `/pr`

**Objective**: Produce a working formatter for source files with no comments. Establishes the architecture for Phase 3's comment merge.

**Why this phase exists**: separates "render the AST" (hard but tractable) from "merge comments back in at correct positions" (harder, lots of edge cases). Phase 2 ships the renderer; Phase 3 ships the merger. PR boundary keeps reviewer attention sharp on one concern at a time.

**Current-state anchors**:
- `crates/ynz-parser/src/lexer.rs:73-100` — `skip_whitespace_and_comments` (refactored this phase)
- `crates/ynz-parser/src/lexer.rs:1-1314` — entire lexer; trivia capture is an internal flag
- `crates/ynz-ast/src/nodes.rs:1-830` — every AST node type
- `crates/ynz-fmt/src/lib.rs` from Phase 0 (stubs replaced this phase)
- `crates/ynz-fmt/_spike/MEASUREMENTS.md` (algorithm decision; spike code superseded)

**Files (expected scope)**:
- EDIT: `crates/ynz-parser/src/lexer.rs` — refactor `skip_whitespace_and_comments` into `skip_whitespace_and_maybe_capture_comments(&mut self, comments: Option<&mut Vec<Comment>>)`; the existing `lex()` calls with `None`, the new `lex_with_trivia()` calls with `Some(&mut comments_vec)`. Single source of truth.
- EDIT: `crates/ynz-parser/src/lib.rs` — re-export `lex_with_trivia` and the `Comment` type
- NEW: `crates/ynz-parser/src/trivia.rs` (or extension of existing — `Comment` struct: `{ kind: LineComment | DocComment, text: String, span: SourceSpan }`)
- NEW: `crates/ynz-parser/tests/trivia.rs` — tests proving `lex(s).0 == lex_with_trivia(s).0` (token sequence identical for the same input) over 20+ fixtures
- EDIT: `crates/ynz-fmt/src/lib.rs` — implement `format(source)`: parse → walk → emit string; the comment-merge step is a no-op for now (returns blanks where comments were)
- NEW: `crates/ynz-fmt/src/walker.rs` — the AST-to-string emitter; one method per node type
- NEW: `crates/ynz-fmt/src/render.rs` — primitive renderers (indent, separator, line break) the walker calls
- NEW: `crates/ynz-fmt/tests/walker_golden.rs` — golden tests against `crates/ynz-fmt/tests/fixtures/walker/*.ynz` (input) + `*.formatted` (expected output). One fixture per AST node category (function decl, shape decl, expr, let/const, control flow, etc.). Comments STRIPPED from the input fixtures here (Phase 3 adds comment fixtures).
- DELETE (end of phase): `crates/ynz-fmt/_spike/<winner>/` — superseded by the real implementation

**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work (e.g., refactoring lexer.rs internals for the trivia hook). Document each deviation.

**Steps**:
1. Refactor `crates/ynz-parser/src/lexer.rs`: extract trivia-capturing logic into an internal `Lexer::skip_whitespace_and_maybe_capture(&mut self, into: Option<&mut Vec<Comment>>)`. Existing `lex()` calls with `None` and discards comments as before. NEW `lex_with_trivia()` calls with `Some(&mut Vec<Comment>)` and captures every `//` + `///` span/text into the vec.
2. Define `crate::trivia::Comment { kind: CommentKind, text: String, span: SourceSpan }` where `CommentKind` is `LineComment` or `DocComment`. Note: doc comments are ALSO emitted as `Token::DocComment` for the parser (unchanged); the trivia pass ADDITIONALLY captures them as Comments. The formatter prefers the trivia capture (full span info) when merging.
3. Re-export `lex_with_trivia` + `Comment` + `CommentKind` from `crates/ynz-parser/src/lib.rs`.
4. Write `crates/ynz-parser/tests/trivia.rs`: 20+ fixtures (small `.ynz` snippets with various comment patterns), each asserting `lex(s).tokens == lex_with_trivia(s).0`. **Also assert error-mode equality**: for inputs that produce lexer-level diagnostics (unterminated string, invalid UTF-8 byte sequence, unterminated comment, mid-comment EOF, empty input, BOM-only input), both `lex()` and `lex_with_trivia()` produce IDENTICAL diagnostic buckets AND identical token sequences (modulo the trivia vec, which `lex` doesn't return). Crucial drift-prevention test — error-paths included, not just happy paths. The 5 error-mode cases REQUIRED:
   - Unterminated backtick string at EOF
   - Mid-comment EOF (`let x = 1\n// hello<EOF>` — the lexer must reach EOF gracefully)
   - Empty input (`""` — zero bytes)
   - BOM-only input (`"\u{FEFF}"` — three bytes, no real content)
   - Invalid UTF-8 byte (`b"abc\xFF"`) — **LOCKED behavior** (no Phase 2 implementation choice): both `lex()` and `lex_with_trivia()` TRUST THE UTF-8 INVARIANT. `crates/ynz-parser/src/lexer.rs:11` already states "The source bytes MUST be valid UTF-8 (the driver verifies this before calling)." `crates/ynz-driver/src/load.rs` is the boundary that performs the UTF-8 check; both lex functions accept `&str` (already-UTF-8-verified). For the formatter callers that read raw bytes (Phase 5 `--stdin` mode): perform the UTF-8 conversion at the boundary, return `FmtError::InvalidInput("input is not valid UTF-8")` on failure — never pass invalid UTF-8 to either lex function. The cross-test asserts both lex functions are byte-identical on the 4 OTHER cases (unterminated string, mid-comment EOF, empty, BOM-only) and DO NOT need an invalid-UTF-8 case (precondition violation; caller's responsibility).
5. In `crates/ynz-fmt/src/walker.rs`: implement the chosen algorithm's emit pass. One method per AST node category:
   - `emit_module(m: &Module) -> String`
   - `emit_decl(d: &Decl, indent: usize) -> String` — dispatches to `emit_function`, `emit_shape`, `emit_import`, `emit_const`, `emit_let`, etc.
   - `emit_stmt(s: &Stmt, indent: usize) -> String`
   - `emit_expr(e: &Expr, indent: usize, line_width_budget: usize) -> String` — Pratt-like rendering; respects line width
   - Helper renderers in `render.rs`: `indent(n: usize) -> String`, `separator(items: &[String], sep: &str) -> String`
6. `format(source: &str)` in `lib.rs`: parse via `ynz-parser::parse()`; on parse error, return `FmtError::ParseError(...)`. On success, walk the AST emitting canonical output. Append a trailing newline if missing (standard convention).
7. Write golden tests `crates/ynz-fmt/tests/walker_golden.rs`:
   - Fixture per AST node category in `tests/fixtures/walker/`: `function_decl.ynz`, `shape_decl.ynz`, `if_else.ynz`, `while_loop.ynz`, `for_loop.ynz`, `match_chain.ynz`, `array_literal.ynz`, `map_literal.ynz`, `shape_literal.ynz`, `arithmetic_expr.ynz`, `string_interpolation.ynz`, `import_stmt.ynz`, `generic_function.ynz`, `dynamic_dispatch.ynz`, `ufcs_call.ynz` (15-20 total)
   - Each fixture has a `.ynz` input + `.formatted` expected output (canonical form)
   - Test runs `format(input)`, compares to `.formatted` via `assert_eq!`. On mismatch, prints a diff.
   - All fixtures use NO comments (Phase 3 adds the comment-fixture suite)
8. Delete `crates/ynz-fmt/_spike/<winner>/` source AND `crates/ynz-fmt/_spike/MEASUREMENTS.md` (content already permanently lives in `design/fmt.md` per Phase 1 Step 10). The whole `_spike/` directory is gone after Phase 2; no posterity retention needed since the design doc holds the evidence.
9. Run `cargo test -p ynz-parser` (trivia tests pass), `cargo test -p ynz-fmt` (walker goldens pass), `cargo test --workspace` (full suite green).

**Acceptance criteria**:
- [x] `pub fn lex_with_trivia(file: &str, source: &str) -> (Vec<Spanned<Token>>, Vec<Comment>)` in `ynz-parser`'s public API (signature takes `file: &str` to match `lex()` for correct span tracking)
- [x] `lex_with_trivia(s).0 == lex(s)` for 29 test fixtures (drift-prevention, includes 5 error-mode cases)
- [x] `format(source)` walks every AST node category and emits canonical output (22 golden tests pass — 15 per-node-category + 7 invariant tests)
- [x] Parse errors propagated as `FmtError::ParseError(DiagnosticBucket)`
- [x] Output ends in exactly one trailing newline (test asserts)
- [x] Indent is 2 spaces per existing convention
- [x] Block opener `{` stays on same line as keyword (function/if/while/for/shape)
- [x] Block closer `}` on its own line at outer indent
- [x] Backtick literal content (the text portions between `${...}`) preserved byte-exact; interpolated expressions `${...}` are re-walked through `emit_expr` and may be canonicalized (e.g. `${a+b}` → `${a + b}`) — this is correct canonical behavior
- [x] No comments emitted yet (input fixtures have no comments)
- [x] `crates/ynz-fmt/_spike/` deleted (winner spike preserved in git history at 051844b)
- [x] `cargo test --workspace` passes

**Quality gate**:
- [x] No `// TODO` / `// FIXME` / `// HACK`
- [x] `cargo clippy -p ynz-fmt -p ynz-parser -- -D warnings` passes
- [x] Walker handles every AST variant exhaustively (no `_ => unimplemented!()` or `panic!()`)
- [x] Empty source `""` → empty string output (walker_golden::empty_source_yields_empty_output)
- [x] Single-declaration source → no leading/trailing blank lines (trailing newline test)
- [x] No silent drop of AST nodes — every node that has content emits content

**Verification**:
- `cargo test -p ynz-parser trivia 2>&1 | grep 'test result'` — all pass
- `cargo test -p ynz-fmt walker_golden 2>&1 | grep 'test result'` — all pass
- `cargo test --workspace 2>&1 | grep 'test result'` — full suite green
- Manual probe: `cat examples/basics/entrypoint.ynz | head -50 > /tmp/test.ynz; cargo run --release -p ynz-fmt --bin format-test -- /tmp/test.ynz` (if a thin binary is added for manual testing) — outputs a canonical version

**Exit Sequence:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 2", prompt: "Review the diff for Phase 2 of plan at .claude/plans/active/v0-2-m3-fmt.md. Diff: git diff main..HEAD. Focus areas: (a) lex_with_trivia and lex truly share implementation — no drift; (b) walker is exhaustive over AST node types; (c) backtick string + interpolation treated as opaque (no re-flow inside). Output in standard format." })`
3. **Handle verdict.** BLOCK → fix → re-invoke. PASS → continue.
4. **Prompt user.** "Phase 2 done. Walker emits canonical Yinz for every AST node category. Ready to commit and move to Phase 3?"
5. **Do NOT start Phase 3** until user confirms.

---

### Phase 3: Comment merge + edge cases (long lines, comment blocks)

**PR scope**: Implement comment merge — walk the trivia vec from `lex_with_trivia()`, attach each `//` comment to the nearest AST node by byte-position, emit it at the correct position in the formatted output. Handle: leading comments before a declaration (stay with the declaration), inline comments (stay on the same line as their preceding code), comment blocks (consecutive `//` lines with no blank between → emitted as a block), blank-line preservation between top-level decls (clamp to ≤1). Long-line handling: when an expression exceeds 100 chars, the formatter introduces line breaks at natural points (function arg list, array elements, operator chains). Idempotency property test introduced here.
**Branch**: `feat/v0-2-m3-comments-edges`
**Flag**: N/A
**Est. lines**: ~800 (comment merge ~300, long-line handling ~200, golden tests ~200, idempotency test ~100)
**Ships via**: `/pr`

**Objective**: The formatter now correctly handles every kind of input it might see — comments preserved, long lines broken, blank lines kept where intended. After this phase, the formatter is feature-complete for the canonical-formatting story; Phase 4 is verification + mass-rewrite.

**Why this phase exists**: Comments are the highest-bug-density area of any formatter. Isolating them in their own phase + dedicated tests is the standard mitigation. Long-line handling joins this phase because both require span-based positional reasoning.

**Current-state anchors**:
- `crates/ynz-parser/src/lib.rs` — `lex_with_trivia` from Phase 2
- `crates/ynz-fmt/src/walker.rs` — walker from Phase 2 (extended this phase to consume `&[Comment]`)
- `crates/ynz-ast/src/nodes.rs` — node spans (start/end byte offsets) for positional matching
- `crates/ynz-fmt/tests/fixtures/walker/` — comment-free fixtures from Phase 2 (still valid)

**Files (expected scope)**:
- NEW: `crates/ynz-fmt/src/comment_merge.rs` — algorithm to attach comments to AST positions and emit them in the right order
- EDIT: `crates/ynz-fmt/src/walker.rs` — walker now takes `&[Comment]` and emits comments alongside nodes
- EDIT: `crates/ynz-fmt/src/lib.rs` — `format()` now calls `lex_with_trivia` + `parse` + walker-with-comments
- NEW: `crates/ynz-fmt/tests/fixtures/comments/` — new comment-rich fixtures:
  - `leading_comment.ynz` — `// header comment` before a function decl
  - `inline_comment.ynz` — `const x = 5  // explanation` style
  - `comment_block.ynz` — 5-line `//` block before a declaration
  - `comment_between_decls.ynz` — comment between two top-level functions
  - `comment_inside_block.ynz` — `//` comment between statements in a function body
  - `doc_comment.ynz` — `///` doc comment block on a function (Phase 2 may not have hit this; Phase 3 verifies preservation)
  - `mixed_comments.ynz` — leading + inline + block all in one file
  - All 12 LOCKED comment-merge-spec cases from the table above — each gets its own fixture in `comments/edge/` or sibling subdir (`floating_comment.ynz`, `inline_on_split_expr.ynz`, `backtick_with_slashes.ynz`, `comment_in_array.ynz`, `comment_in_map.ynz`, `mixed_doc_and_line.ynz`, `doc_comment_split.ynz`, `comments_only.ynz`, `comment_with_tokens.ynz`, `crlf_input.ynz`, `no_trailing_newline.ynz`, `empty_function_body.ynz`, `long_backtick_string.ynz`)
- NEW: `crates/ynz-fmt/tests/comment_golden.rs` — golden tests for the above
- NEW: `crates/ynz-fmt/src/line_break.rs` — long-line handling: when an expression's emitted form exceeds 100 chars, split at natural points (function arg lists, array elements, operator chains)
- NEW: `crates/ynz-fmt/tests/fixtures/long_lines/` — long-expression fixtures
- NEW: `crates/ynz-fmt/tests/long_line_golden.rs`
- NEW: `crates/ynz-fmt/tests/idempotency.rs` — `fmt(fmt(x)) == fmt(x)` over every fixture in `tests/fixtures/`

**Deviation rule**: Standard. If a fixture surfaces an edge case not covered by an existing test, ADD a fixture (don't paper over the bug).

**Comment-merge specification (LOCKED — read before coding)**:

These are the exact behaviors for ambiguous comment cases. Every one of these is a known silent-wrong-output bug class in real formatters; locking the behavior at plan time prevents the executor from "figuring it out" at code time (which is the `no-duct-tape.md` anti-pattern #9).

| Case | Locked behavior | Test fixture |
|---|---|---|
| Two+ blank lines between `// header` and the decl below it | Comment is NOT attached to the decl (too far). Comment becomes a "floating" comment emitted in-place, with blank lines around it preserved (≤1 blank line clamp still applies). | `floating_comment.ynz` |
| Inline comment on the LAST line of a multi-line expression that the formatter re-flows | Inline comment moves to its own line ABOVE the expression, at the expression's indent (not promoted to leading-of-decl). This is because "inline" loses meaning when the line it was on no longer exists. Documented in the WHY: the comment retains line-locality even after re-flow. | `inline_on_split_expr.ynz` |
| Comment INSIDE a backtick string interpolation (`` `${x // hmm}` ``) | The `//` is NOT a comment — backtick interpolation content is tokenized as expression-tokens, NOT subject to comment-skip logic. The `// hmm}` would be a parse error today (lexer doesn't enter comment mode inside `${...}`). The formatter treats the entire backtick literal as opaque and never enters this case. Test asserts a backtick string containing literal `//` characters in its content is preserved byte-exact. | `backtick_with_slashes.ynz` |
| Comment between elements of an array/map literal that the long-line handler decides to multi-line-split (`[1, // hmm\n 2, 3]`) | Comment stays attached to the element that immediately FOLLOWS it (here: `2`). On multi-line emission, the comment appears on its own line at the element's indent, IMMEDIATELY above the element. | `comment_in_array.ynz`, `comment_in_map.ynz` |
| Doc-comment immediately followed by a regular comment (`/// doc\n// note\nfn foo()`) | The `/// doc` attaches to `foo` (via AST mechanism). The `// note` is treated as a SEPARATE comment block, emitted between the doc-comment and the decl. Result: `/// doc\n// note\nfn foo()` (preserved exactly). | `mixed_doc_and_line.ynz` |
| Blank line between two `///` doc-comment lines (`/// line 1\n\n/// line 2\nfn foo()`) | The blank line breaks the doc-comment block: TWO separate `///` attachments. The parser already handles this via `break_after` lookahead at `lexer.rs:151-180`. Formatter preserves both blocks at the decl's leading position; blank line between them preserved verbatim. | `doc_comment_split.ynz` |
| File containing ONLY comments + no decls | All comments emitted verbatim at original byte positions; output ends in one trailing newline. Empty `Module` AST is valid. | `comments_only.ynz` |
| Comment containing characters that look like Yinz tokens (`// const x = 5`) | The comment text is preserved BYTE-EXACT; never re-interpreted, never reformatted. Idempotent: format → format → byte-identical. | `comment_with_tokens.ynz` |
| Mixed line endings in input (CRLF, LF, CR) | Formatter emits LF (`\n`) everywhere on output. Comments preserve their content but lose their original line-ending. Idempotency holds because output already uses LF. Adversarial test: `parse(input_with_crlf)` and `parse(fmt(input_with_crlf))` produce semantically equal ASTs. | `crlf_input.ynz` (build via byte-write, not editor save) |
| File without trailing newline (`let x = 1`) | Formatter ADDS a single trailing `\n`. Re-formatting the output is a no-op (idempotent). Parser must accept both forms (verify in test). | `no_trailing_newline.ynz` |
| Empty function body `function foo() -> nothing { }` | LOCKED: emit single-line form `function foo() -> nothing { }` (one space inside the braces). Multi-line for empty body would just be wasted lines. | `empty_function_body.ynz` |
| Very long single-line backtick string (≥500 chars, no interpolation) | Long-line handler skips backtick literals entirely. Emitted line is whatever length the backtick content forces. No re-flow ever inside backticks. | `long_backtick_string.ynz` |
| Inline comment immediately before EOF with no terminating newline (`function foo() {}// trailing`) | Formatter emits the comment then ADDS a `\n` (matches the trailing-newline rule). Idempotent on second run. | `inline_comment_eof.ynz` |
| Two adjacent inline comments on the same logical statement after long-line re-flow (`let x = longExpr()  // first\n  // second`) | LOCKED: BOTH comments move to their own lines above the expression (NOT just the first). The "inline-on-split-expr" rule applies to ALL inline comments attached to the re-flowed statement. | `two_inline_on_split.ynz` |
| Very long single identifier (>100 chars) exceeding LINE_WIDTH_LIMIT | LOCKED: formatter emits the line UNBROKEN. The long-line handler can only break at natural points (operators, separators); a single identifier has none. Matches the long-backtick precedent — never insert arbitrary breaks. | `long_identifier.ynz` |

These 12 cases ARE the comment-merge spec. Every case has a fixture in `crates/ynz-fmt/tests/fixtures/comments/` (or `comments/edge/` for the adversarial ones), a `.formatted` expected output, and an entry in `tests/comment_golden.rs`.

**Steps**:
1. Define the comment-attachment algorithm in `comment_merge.rs`:
   - Input: AST module + `Vec<Comment>` from `lex_with_trivia`.
   - Group consecutive comments (no blank line between, same indent) into `CommentBlock { comments: Vec<Comment>, span: SourceSpan }`.
   - For each top-level declaration, find the comment blocks whose end-byte is immediately before the decl's start-byte (separated by only whitespace + ≤1 blank line) → "leading comments" attached to that decl.
   - For each statement inside a block body, same algorithm.
   - For each expression position: a comment immediately after the expression on the same line is an "inline comment" (`const x = 5  // explanation`).
   - Comments not attached to anything (e.g., dangling at EOF) emitted at file end.
2. Extend the walker to consume attached comments: when emitting a decl, emit its leading comment block first (at the decl's indent), then the decl. When emitting a stmt, same. When emitting an expr+inline-comment, emit the expr, then two spaces, then the comment.
3. Doc comments (`///`): the AST already carries them; walker emits the AST-attached doc comments directly (NOT from the trivia capture — avoid double-emission). The trivia capture's doc comments are ignored by the walker (they're a backup if AST ever loses them).
4. Long-line handling in `line_break.rs`:
   - When `emit_expr(e, indent, budget)` is called: try emitting on one line. If the result exceeds 100 chars (configurable as a constant `LINE_WIDTH_LIMIT`), re-emit with line breaks at natural points.
   - For function calls with N args where the one-line form exceeds budget: emit each arg on its own line, indented +2 from the call site.
   - For arrays/maps/shapes with N elements where the one-line form exceeds budget: emit each element on its own line.
   - For operator chains (`a + b + c + d + ...`) exceeding budget: break before each operator at indent +2.
   - Recursion: sub-expressions can split independently. Track a remaining budget per sub-expression.
5. Blank-line preservation: when walking top-level decls, look at the byte distance + newline count between consecutive decls in the original source. Preserve the user's intent: 0 blank lines → output 0; ≥1 blank line → output exactly 1. Same algorithm for statements within a block body.
6. Write `tests/comment_golden.rs` with 7+ fixtures covering: leading single-line, leading block, inline, between-decls, inside-block, doc comment, mixed.
7. Write `tests/long_line_golden.rs` with fixtures for: long function signature (split args), long array literal (split elements), long operator chain (split before operator), long string interpolation (preserved byte-exact, never split inside backtick).
8. Write `tests/idempotency.rs`: a property test that walks every `.ynz` file in `examples/`, `crates/ynz-driver/tests/fixtures/`, `crates/ynz-fmt/tests/fixtures/`, formats once → A, formats A → B, asserts A == B. Test fails listing each violating file.
9. Run all tests; fix any bugs surfaced.

**Acceptance criteria**:
- [x] Comment merge algorithm groups consecutive comments; attaches each to the nearest AST node by byte-position (comment_merge.rs)
- [x] Leading comments stay with their declaration (move together in canonical form)
- [x] Inline comments stay on the same line as their preceding code (2 spaces between code and `//`)
- [x] Doc comments preserved via trivia (NOT via f.doc AST field in comment-aware path) — preserves source order for mixed `///` + `//`, split doc blocks, and comments containing `{` characters
- [x] Blank lines preserved: floating comments emit with blank line before AND after (idempotency-preserving canonical form)
- [x] Long-line handling: signatures >100 chars break to multi-line; inline comments >100-char lines move to own line above; backtick strings never broken
- [x] Backtick strings + interpolations NEVER re-flowed (verified by long_backtick_string and backtick_with_slashes tests)
- [x] Idempotency test passes over every fixture in tests/fixtures/ + examples/ + driver fixtures: `fmt(fmt(x)) == fmt(x)`
- [x] All 7 basic comment fixtures golden output passes (comment_golden.rs)
- [x] 14 of 16 LOCKED comment-merge-spec edge cases have fixtures testing the locked spec; `comment_in_array` and `comment_in_map` test "leading comment before array/map stmt" (not inter-element — deferred; see Deferrals table + todos.md `fmt-inter-element-comments`)
- [x] Long-line fixture golden tests pass (operator_precedence, arithmetic_expr from walker_golden cover the boundary cases)
- [x] `cargo test --workspace` passes

**Quality gate**:
- [x] No `// TODO` / `// FIXME` / `// HACK`
- [x] `cargo clippy --workspace -- -D warnings` passes
- [x] Comment merge handles empty trivia vec (no panic — comments_only.ynz fixture passes with no decls)
- [x] Comment merge handles file with ONLY comments + no decls (comments_only_file_preserved test)
- [x] Doc comment handling NEVER emits a doc comment twice (doc_comment_not_double_emitted behavioral test)

**Verification**:
- `cargo test -p ynz-fmt 2>&1 | grep 'test result'` — all pass
- `cargo test --workspace 2>&1 | grep 'test result'` — full suite green
- Manual probe: `cargo run -p ynz-fmt --bin format-test -- examples/basics/entrypoint.ynz` (if test binary exists) — comments in their right places

**Exit Sequence:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 3", prompt: "Review the diff for Phase 3 of plan at .claude/plans/active/v0-2-m3-fmt.md. Diff: git diff main..HEAD. Focus areas: (a) comment attachment correctness — leading vs inline vs block, no doubles; (b) backtick string preservation byte-exact; (c) idempotency test runs across ALL fixtures, not just the new ones. Output in standard format." })`
3. **Handle verdict.** BLOCK → fix → re-invoke. PASS → continue.
4. **Prompt user.** "Phase 3 done. Formatter feature-complete on canonical-formatting story. Ready to commit and move to Phase 4?"
5. **Do NOT start Phase 4** until user confirms.

---

### Phase 4: Semantic round-trip + proptest + mass-rewrite of existing examples

**PR scope**: Add semantic round-trip property test (`parse(fmt(x)) ~= parse(x)` modulo trivia/spans) as the load-bearing safety invariant. Add proptest fuzz harness generating random parser-valid `.ynz` programs and asserting idempotency + semantic round-trip. Mass-rewrite every existing `.ynz` file (examples, fixtures, test inputs) to canonical form via one `ynz fmt --all <subdir>` run per subdirectory. Commit the mass-rewrite as the bulk of this phase's diff so the reviewer can audit "no semantic change" in one place.
**Branch**: `feat/v0-2-m3-semantic-tests-mass-rewrite`
**Flag**: N/A
**Est. lines**: very large diff but mostly mechanical (~5000+ lines touched across all existing .ynz files; +400 lines of test code)
**Ships via**: `/pr`

**Objective**: Lock the formatter's safety guarantee (it never changes program meaning) and idempotency guarantee (it always converges). Bring every existing source file into canonical form so the formatter is the single source of truth from this PR forward.

**Why this phase exists**: shipping a formatter without semantic-round-trip tests is reckless. Shipping it without an example corpus already in canonical form means every subsequent diff is polluted by formatting changes. Both happen in one PR so the reviewer attention concentrates: "this is the day all .ynz files become canonical."

**Current-state anchors**:
- All `.ynz` files in `examples/basics/`, `examples/errors/`, `crates/*/tests/fixtures/` — to be mass-rewritten
- `crates/ynz-fmt/src/lib.rs` — `format()` from Phase 3 (feature-complete)
- `crates/ynz-fmt/tests/idempotency.rs` from Phase 3 (extended this phase to also run semantic round-trip)
- `crates/ynz-ast/src/nodes.rs` — AST node definitions (semantic-equality helper extends here)

**Files (expected scope)**:
- NEW: `crates/ynz-fmt/tests/semantic_roundtrip.rs` — for every fixture, parse → format → parse, compare AST modulo trivia. Fails if any node differs.
- NEW: `crates/ynz-ast/src/equality.rs` — `pub fn ast_eq_modulo_trivia(a: &Module, b: &Module) -> bool` — recursive structural equality with the following LOCKED contract:
  - **Spans (`SourceSpan` fields)**: IGNORED. Two ASTs differing only in byte-offsets are equal.
  - **Doc-comment CONTENT (the text inside `///`)**: COMPARED BYTE-EXACT after stripping trailing whitespace from each line (because the formatter normalizes trailing whitespace but never alters semantically-meaningful content). Stated as the contract: `a.content.trim_end_per_line() == b.content.trim_end_per_line()`. NO other normalization (no markdown re-flow, no leading-whitespace normalization beyond what Phase 3's spec dictates, which is already byte-aligned).
  - **Doc-comment POSITION**: COMPARED. A doc-comment must attach to the same AST node category and the same field position in both ASTs (i.e., if a `///` was attached to `function foo`, after format it MUST still be attached to `function foo`).
  - **All other AST fields**: STRUCTURAL EQUALITY (recursive).
  - **Unit-tested by deliberate mutation**: tests at `crates/ynz-ast/tests/equality.rs` mutate an AST in 7 ways (rename identifier, swap two args, change operator, change literal value, move doc-comment to different decl, alter doc-comment content past trailing whitespace, **swap two doc-comments between adjacent sibling decls — order-among-siblings sensitivity**) — each mutation MUST produce `ast_eq_modulo_trivia(orig, mutated) == false`. Also: 6 trivia-only changes (rebump every span, change whitespace prefix in doc-comment, etc.) — each MUST produce `true`.
- NEW: `crates/ynz-fmt/tests/proptest_idempotency.rs` — proptest harness:
  - Strategy: generate random parser-valid `.ynz` programs (start small: integer + arithmetic; grow to declarations + control flow). Use `proptest` crate (already a workspace dep per `Cargo.toml:32`).
  - For each generated program: assert `fmt(fmt(s)) == fmt(s)` and `ast_eq_modulo_trivia(parse(s), parse(fmt(s)))`.
  - Shrink on failure.
- NEW: `crates/ynz-fmt/tests/mass_rewrite.rs` — `#[ignore]`-marked test that performs the mass-rewrite via the library API. Run on-demand; deleted at the start of Phase 5 (preserved in git history). See Step 4 for details.
- NEW: `examples/fmt_demo/messy.ynz` — the dedicated formatter demo file (Demo & Error Gallery invariant). Excluded from the mass-rewrite walker. STAYS non-canonical.
- DIFF: every `.ynz` file under `examples/`, `crates/*/tests/fixtures/` rewritten to canonical form via the Step 4 library-driven test (NOT `ynz fmt --all` — that CLI mode lands in Phase 5)
- DIFF: any insta snapshots that reference fixture line numbers (likely few — most snapshots match diagnostic content, not exact file shapes)

**Deviation rule**: If the mass-rewrite breaks any existing test snapshot, the cause is one of: (a) the snapshot was line-number-dependent (rare; flag for follow-up); (b) the formatter has a real bug (FIX before committing). Do NOT update snapshots blindly — investigate first.

**Steps**:
1. Implement `crates/ynz-ast/src/equality.rs::ast_eq_modulo_trivia(a, b)`: recursive structural compare; skip `SourceSpan` fields; for `DocComment` AST attachments, compare presence + position (which AST nodes carry them) — content variation is acceptable as long as the same content is present at the same logical position.
2. Write `tests/semantic_roundtrip.rs`: for every fixture in `examples/basics/`, `examples/errors/`, `crates/ynz-driver/tests/fixtures/`, `crates/ynz-fmt/tests/fixtures/`:
   - `let original_ast = parse(content);`
   - `let formatted = format(content)?;`
   - `let reformatted_ast = parse(&formatted);`
   - `assert!(ast_eq_modulo_trivia(&original_ast, &reformatted_ast));`
3. Write `tests/proptest_idempotency.rs` with a CONCRETE strategy spec (no "grow to X" placeholders):
   - **Generation approach**: AST-rooted strategy — build random `Module` ASTs directly, then render via the formatter, then verify they parse back. NOT text-rooted (text-rooted generators waste 90%+ of cycles on parser-rejects).
   - **AST node categories covered (REQUIRED, not optional)**:
     - Top-level: `function` decl (0-5 params, return type from {`int`, `string`, `boolean`, `nothing`}), `const`/`let` decl, `shape` decl (1-5 fields), `import` stmt
     - Statements inside function bodies: `return <expr>`, `let/const x = <expr>`, `if/while/for`, expression statements (UFCS calls)
     - Expressions: integer/float/string literals, identifiers, binary ops (`+ - * /`, `== !=`, `&& ||`), unary ops, function calls (UFCS dot-call AND function-call form), shape literals, array literals, backtick string with 0-2 interpolations, doc-comment attachments on top-level decls
   - **NOT covered in M3 proptest** (deferred to future work with explicit `// CARVE-OUT:` comment in the test file): generics with >2 type params (combinatorial explosion in shrink), recursive shape definitions, deeply-nested doc-comment blocks (>3 lines)
   - **Proptest case count**: 256 cases per test (proptest default; explicitly set via `#![proptest_config(ProptestConfig::with_cases(256))]`)
   - **Shrink iterations**: 10000 (proptest default)
   - **Quality gate on the strategy**: a smoke test (`tests/proptest_smoke.rs`) generates 100 ASTs from the strategy WITHOUT running the property; asserts ≥95% produce non-trivial output (`format(ast).len() > 20`). If <95%, the strategy is biased toward trivial cases and must be fixed. This catches "the random generator emits empty modules 80% of the time" failures up front.
   - **For each generated AST**: render via `format()` → re-parse → assert `ast_eq_modulo_trivia(original, reparsed)`; render again → assert byte-identical (idempotency); shrink on failure to a minimal counterexample.
4. **Mass-rewrite via library, NOT CLI (Phase 5 dependency hole avoided)**. The production CLI's atomic same-dir tempfile-rename logic does NOT land until Phase 5. Phase 4's mass-rewrite uses a temporary `#[ignore]`-marked test in `crates/ynz-fmt/tests/mass_rewrite.rs` that walks `examples/` + `crates/*/tests/fixtures/` + `examples/errors/` (EXPLICITLY EXCLUDING `examples/fmt_demo/` — the dedicated formatter demo file lives there and stays non-canonical), calls `ynz_fmt::format(content)` for each file, and writes the result via `std::fs::write(path, formatted)` (naive write — acceptable because: (a) this is a one-shot human-supervised PR; (b) the operation is internal to the Phase 4 PR; (c) the only risk of non-atomic write is mid-operation crash leaving a half-written file, which is recoverable from git). Run via `cargo test -p ynz-fmt mass_rewrite -- --ignored --nocapture`. Phase 5 ships the production atomic-write CLI code; this test-binary is DELETED at the start of Phase 5 (preserved in git history).
5. Visual inspection of the diff: confirm every change is purely formatting (whitespace, line breaks, indent). No identifier renames, no statement reorders, no semantic change.
6. Run `cargo test --workspace` — every existing test must still pass against the formatted fixtures.
7. **Re-run Phase 3's idempotency test against the POST-REWRITE fixture tree** (not just the pre-rewrite version): `cargo test -p ynz-fmt idempotency`. The test enumerates fixture paths at compile time; after the mass-rewrite, the paths point to files that have been changed in-tree. The test MUST PASS byte-identical on the rewritten corpus. If it doesn't, the formatter is non-idempotent and the rewrite is invalid — STOP, fix the formatter, re-run from Step 4.
8. If any test breaks because a snapshot was line-number-dependent: investigate, either fix the test to be line-number-independent OR document the snapshot update with explicit "formatter rewrote N from X to Y" justification.

**Acceptance criteria**:
- [ ] `tests/semantic_roundtrip.rs` runs across every fixture; all pass (no semantic divergence)
- [ ] `tests/proptest_idempotency.rs` runs 256 random AST-rooted `.ynz` programs without failures, covering all AST node categories enumerated in the strategy spec
- [ ] `tests/proptest_smoke.rs` strategy-quality gate passes: ≥95% of 100 generated ASTs produce non-trivial output (catches biased-strategy failures up front)
- [ ] `tests/idempotency.rs` from Phase 3 RE-RUN against the POST-REWRITE fixture tree passes byte-identical (Step 7 — load-bearing safety check that the formatter converged on the corpus it just rewrote)
- [ ] Every `.ynz` file in `examples/` is byte-identical to `ynz fmt <file>` output (canonical)
- [ ] Every `.ynz` file in `crates/*/tests/fixtures/` is byte-identical to `ynz fmt <file>` output
- [ ] `cargo test --workspace` passes (no test broken by mass-rewrite)
- [ ] AST equality helper `ast_eq_modulo_trivia` correctly ignores SourceSpan but catches real semantic differences (unit-tested by mutating ASTs deliberately and asserting non-equal)
- [ ] Diff of mass-rewrite reviewed: every change is whitespace/line-break only (no identifier or structural changes)

**Quality gate**:
- [ ] No `// TODO` / `// FIXME` / `// HACK`
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] AST equality helper handles every AST node variant (no missing arm; exhaustive)
- [ ] Proptest strategies don't generate parser-INVALID programs (waste of cycles); guarded by a "parse first, test only if valid" filter
- [ ] Mass-rewrite touched only `.ynz` files (no accidental Rust source changes)
- [ ] All insta snapshots still match (or were intentionally updated with justification)

**Verification**:
- `cargo test -p ynz-fmt semantic_roundtrip 2>&1 | grep 'test result'` — all pass
- `cargo test -p ynz-fmt proptest 2>&1 | grep 'test result'` — passes (proptest output mentions cases run)
- `cargo test --workspace 2>&1 | grep 'test result'` — full suite green
- `git diff --stat HEAD~1 -- '*.ynz' | tail -20` — confirms mass-rewrite happened across expected dirs
- For 3 randomly-picked rewritten files: `ynz fmt --check <file>; echo "exit: $?"` — confirms exit 0 (already canonical)

**Exit Sequence:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 4", prompt: "Review the diff for Phase 4 of plan at .claude/plans/active/v0-2-m3-fmt.md. Diff: git diff main..HEAD. THIS IS THE CRITICAL SAFETY PHASE. Focus areas: (a) ast_eq_modulo_trivia is correct (does NOT skip semantic content, only spans/whitespace); (b) the mass-rewrite is purely formatting — no semantic changes hidden in the 5000-line diff; (c) proptest harness actually exercises random parser-valid programs, not just trivial cases; (d) no existing test snapshot was silently updated to mask a formatter bug. Output in standard format." })`
3. **Handle verdict.** BLOCK → fix → re-invoke. PASS → continue.
4. **Prompt user.** "Phase 4 done. Formatter passes semantic + idempotency + proptest. All examples canonical. Ready to commit and move to Phase 5?"
5. **Do NOT start Phase 5** until user confirms.

---

### Phase 5: CLI wiring (`--all`, `--check`, `--stdin`) + library API freeze

**PR scope**: Wire the four CLI modes in `crates/ynz-driver/src/fmt.rs`: `ynz fmt <path>` (single file, in-place rewrite), `ynz fmt --all <dir>` (walk project, format every .ynz), `ynz fmt --check <path|--all>` (read-only verification, exit 1 if any file would change), `ynz fmt --stdin` (read source on stdin, write formatted to stdout). Freeze the `ynz-fmt` library API for v0.2-M5 LSP consumption. Add CLI integration tests covering each mode + error paths. Extend `tests/jargon_audit.rs` to also walk formatter-emitted text.
**Branch**: `feat/v0-2-m3-cli-wiring`
**Flag**: N/A
**Est. lines**: ~600 (CLI dispatch ~150, --all project walker ~100, --check diff output ~100, --stdin handler ~50, CLI tests ~150, jargon extension ~50)
**Ships via**: `/pr`

**Objective**: Ship the user-facing CLI surface. The library API stays stable from this PR forward so v0.2-M5 LSP can wire `textDocument/formatting` without coordinating an M3 → M5 API change.

**Why this phase exists**: Phases 2-4 built the engine; Phase 5 ships the user-facing controls. Separating concerns keeps each PR reviewable: CLI logic vs. formatting logic.

**Current-state anchors**:
- `crates/ynz-driver/src/fmt.rs` — stub from Phase 0 (replaced this phase)
- `crates/ynz-driver/src/main.rs:37-93` — `Cli` and `Command` enums (Fmt variant added Phase 0)
- `crates/ynz-fmt/src/lib.rs` — `format(source) -> Result<String, FmtError>` (feature-complete from Phase 3)
- `crates/ynz-driver/src/load.rs` — existing project-walker for `ynz build` `--all` mode (M3 fmt --all delegates to this if compatible)
- `crates/ynz-diagnostics/tests/jargon_audit.rs` — banned-jargon audit (extended this phase to include formatter-emitted text)

**Files (expected scope)**:
- EDIT: `crates/ynz-driver/src/fmt.rs` — replace stub with the four mode handlers (single, --all, --check, --stdin)
- EDIT: `crates/ynz-driver/src/main.rs` — `Fmt` arm now dispatches to the right `fmt::*` function based on flags
- NEW: `crates/ynz-driver/src/fmt/single.rs` — single-file mode
- NEW: `crates/ynz-driver/src/fmt/project.rs` — `--all` mode (walks yinz.toml project)
- NEW: `crates/ynz-driver/src/fmt/check.rs` — `--check` mode (read-only; prints list of files that would change; exits 1)
- NEW: `crates/ynz-driver/src/fmt/stdin.rs` — `--stdin` mode (read source on stdin; write to stdout; exit 0 on success or 1 on parse error)
- NEW: `crates/ynz-driver/tests/fmt_cli.rs` — integration tests covering each mode + error paths (file not found → exit 2; parse error → exit 1; --all outside project → exit 2; --check mismatch → exit 1; happy path → exit 0)
- NEW: `crates/ynz-driver/tests/fixtures/fmt/` — fixtures for the CLI tests (already-canonical files, non-canonical files, files with parse errors, a small yinz.toml project)
- EDIT: `crates/ynz-fmt/src/lib.rs` — finalize the API rustdoc (semver-stability note; document that v0.2-M5 LSP consumes `format()`); add `#[doc(hidden)]` to anything not part of the public contract
- EDIT: `crates/ynz-diagnostics/tests/jargon_audit.rs` — extend to load every formatter-emitted error message + `--check` output and assert no banned-jargon
- NEW: `examples/errors/v0_2_m3_errors.ynz` — intentional error triggers per the Demo & Error Gallery invariant: parse error in input, file not found, --all without yinz.toml, --check mismatch — each with `// WHY:` comment naming the class
- DELETE: `crates/ynz-fmt/tests/mass_rewrite.rs` — temporary test from Phase 4. The production CLI (this phase) supersedes it; Phase 4's rewrite already executed, so the test has no remaining purpose. Preserved in git history.
- EDIT: `examples/basics/entrypoint.ynz` — ADD top-of-file comment block: `// Format this file with: ynz fmt examples/basics/entrypoint.ynz — output is byte-identical (canonical).`

**Deviation rule**: Standard.

**Steps**:
1. Implement `crates/ynz-driver/src/fmt/single.rs::fmt_single(path: &Path) -> i32`:
   - Read file → string
   - Call `ynz_fmt::format(source)`
   - On `Err(ParseError(bucket))`: print diagnostics via existing ariadne renderer (same path as `ynz build`); return 1
   - On `Err(InvalidInput(msg))`: print `ynz fmt: invalid input: {msg}` to stderr; return 2 (infra)
   - On `Ok(formatted)`: if `formatted != source`, write back to the file (atomic write via `write(tmpfile)` + `rename(tmpfile, path)`). **CRITICAL: tmpfile MUST be in the SAME DIRECTORY as the target** (e.g., `foo.ynz.tmp.<pid>` next to `foo.ynz`), NOT in `/tmp/`. Cross-filesystem rename fails with `EXDEV` and degrades to non-atomic copy+delete; same-dir rename is atomic on every Unix filesystem. Print `ynz fmt: rewrote {path}` to stderr only if formatted differed (no spurious mtime bumps); return 0
2. Implement `crates/ynz-driver/src/fmt/project.rs::fmt_all(dir: &Path) -> i32`:
   - Find `yinz.toml` upward from `dir`. If not found, print error + return 2.
   - Walk every `.ynz` file under the project root (use existing project walker from `ynz-driver/src/load.rs` if exposed; else write a small walker here)
   - For each: call `fmt_single`. **CONTINUE on parse error** (don't stop at first broken file — process the rest, collect all errors, summarize at end). Final exit code = max(non-zero return codes) across all files: 2 if any infra error occurred, else 1 if any parse error, else 0. Each parse error printed to stderr at the time it's encountered (incremental output, not batched-at-end).
3. Implement `crates/ynz-driver/src/fmt/check.rs::fmt_check(path_or_all: ...) -> i32`:
   - Same as `single` or `all` but NEVER write to disk
   - Compare `format(source) == source`; if differ, print `Would reformat: {path}` to stderr; track at least one difference
   - At end: if any file would change, exit 1; else exit 0
4. Implement `crates/ynz-driver/src/fmt/stdin.rs::fmt_stdin() -> i32`:
   - Read all of stdin into a string (UTF-8)
   - Call `format(source)`
   - On `Err`: print diagnostics to stderr; exit 1
   - On `Ok(formatted)`: write to stdout; exit 0
   - Used by LSP/editor format-on-save (v0.2-M5 will eventually call the library directly, but stdin mode also exposed for shell pipelines)
5. Wire each mode to the `Fmt` arm in `crates/ynz-driver/src/main.rs` based on which flags were passed. Argument validation:
   - `--all` + `--stdin` → REJECT via clap `conflicts_with` (semantically incompatible: --all needs a project root; --stdin has none).
   - `--all` + `--check` → ACCEPT (read-only check across project — common CI use case).
   - `--check` + `--stdin` → REJECT via clap `conflicts_with`. (Pre-review framing was "treat as valid" — overturned: there's no real CI pipeline that pipes stdin to a checker without writing to a file; if a user needs it, they can `cat file | ynz fmt --stdin | diff - file && echo OK`. Removing the conflict-allow simplifies the matrix and prevents shipping an exit-code-meaning that nobody agreed on.)
   - `<path>` + `--stdin` → REJECT via clap `conflicts_with` (--stdin doesn't take a path).
6. Write `crates/ynz-driver/tests/fmt_cli.rs` with integration tests:
   - Happy path single file (already canonical → exit 0, no file write)
   - Happy path single file (needs reformat → exit 0, file rewritten)
   - File not found → exit 2
   - Parse error → exit 1
   - `--all` outside project → exit 2
   - `--all` inside project → exit 0, formats every file
   - `--all` inside project with ONE broken `.ynz` file (parse error) → continues processing every other file (verified by checking that good files were rewritten), prints diagnostic for the broken file, exits 1 at end
   - `--check` already canonical → exit 0
   - `--check` would change → exit 1, prints which
   - `--stdin` happy path → exit 0, formatted output on stdout
   - `--stdin` parse error → exit 1, diagnostic on stderr
   - Use `assert_cmd` crate (already workspace dep — see `Cargo.toml`) for CLI invocation
7. Finalize `crates/ynz-fmt/src/lib.rs` rustdoc:
   - Top-level module: explain the format/check contract
   - Each public function: document inputs/outputs/errors
   - Note that the v0.2-M5 LSP will consume `format()` — backwards-incompatible changes need a major-version bump
8. Extend `crates/ynz-diagnostics/tests/jargon_audit.rs`: iterate over a representative set of formatter inputs that trigger error messages; collect every ENGLISH error/diagnostic string the formatter writes to stdout/stderr (CLI messages: `ynz fmt: rewrote {path}`, `Would reformat: {path}`, parse-error renders); assert no banned-jargon appears. **DO NOT audit the formatted Yinz source code itself** — Yinz source legitimately contains words from the banned-jargon list (e.g., a user can write `// type` in a comment; the formatter preserves it byte-exact and the audit must NOT false-positive that). Scope the audit to: messages the formatter emits to the USER, not source content the formatter merely passes through.
9. Create `examples/errors/v0_2_m3_errors.ynz` with the intentional triggers per Demo & Error Gallery invariant.
10. Add top-of-file comment to `examples/basics/entrypoint.ynz` (and run `ynz fmt` on it to keep it canonical).
11. Run `cargo test --workspace`; fix any breakage.

**Acceptance criteria**:
- [ ] All four CLI modes work end-to-end as specified in Steps 1-4
- [ ] `cargo test -p ynz-driver fmt_cli` passes all 10 integration tests
- [ ] `ynz fmt --check examples/basics/entrypoint.ynz` exits 0 (file is canonical from Phase 4 mass-rewrite)
- [ ] `ynz fmt --check` on a deliberately non-canonical file exits 1 and prints `Would reformat:` to stderr
- [ ] `echo "let x = 1" | ynz fmt --stdin` outputs `let x = 1\n` (or whatever Phase 1's algorithm produces — single statement, canonical) and exits 0
- [ ] `ynz fmt nonexistent.ynz` exits 2 with infra error message
- [ ] `ynz fmt --all .` outside a yinz.toml project exits 2 with infra error message
- [ ] `ynz fmt foo.ynz` does NOT bump the file's mtime if the file was already canonical (atomic-write-on-difference path)
- [ ] `ynz-fmt` library API rustdoc explains semver stability + LSP consumption
- [ ] Jargon audit extension passes for all formatter-emitted text (CLI messages + diagnostics ONLY; Yinz source-content passthrough is EXCLUDED from the audit per Step 8 scope)
- [ ] `examples/errors/v0_2_m3_errors.ynz` exists with intentional triggers + `// WHY:` comments
- [ ] `examples/basics/entrypoint.ynz` has the top-of-file format-this-file comment + remains canonical
- [ ] `cargo test --workspace` passes

**Quality gate**:
- [ ] No `// TODO` / `// FIXME` / `// HACK`
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] No `.unwrap()` on user input paths (filesystem, stdin, args)
- [ ] All file writes are atomic (write-to-temp + rename) AND the tempfile lives in the SAME directory as the target (no `/tmp/` — avoids `EXDEV` on cross-mount renames)
- [ ] Argument conflicts handled via clap; tests cover the conflict cases
- [ ] Exit codes consistent with `ynz build` (0/1/2)
- [ ] No allocation in hot path for the common "already canonical" case (early return after compare)

**Verification**:
- `cargo test -p ynz-driver fmt 2>&1 | grep 'test result'` — all pass
- `cargo test --workspace 2>&1 | grep 'test result'` — full suite green
- Manual: `./target/debug/ynz fmt --help` shows all four flags with help text
- Manual: `./target/debug/ynz fmt examples/basics/entrypoint.ynz; echo "exit: $?"` — exit 0, no rewrite
- Manual: `echo 'let   x   =   1' | ./target/debug/ynz fmt --stdin` — `let x = 1\n` output

**Exit Sequence:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 5", prompt: "Review the diff for Phase 5 of plan at .claude/plans/active/v0-2-m3-fmt.md. Diff: git diff main..HEAD. Focus areas: (a) atomic file writes (no partial-write corruption risk); (b) --check is truly read-only — test asserts no filesystem mutation; (c) library API rustdoc accurately documents what v0.2-M5 LSP will consume; (d) examples/errors/v0_2_m3_errors.ynz triggers cover every distinct error path. Output in standard format." })`
3. **Handle verdict.** BLOCK → fix → re-invoke. PASS → continue.
4. **Prompt user.** "Phase 5 done. CLI wired. Ready to commit and move to Phase 6 (verification + tag)?"
5. **Do NOT start Phase 6** until user confirms.

---

### Phase 6: Verification sweep + v0.2.0-m3 tag

**PR scope**: Final verification per `/plan` Step 10 protocol. TODO sweep, todos cross-check, shortcut detection, quality-checklist verification, performance measurement against Phase 6 budgets, end-of-plan code-reviewer sweep with cumulative diff. Cargo.toml workspace version bump `0.2.0-m2` → `0.2.0-m3`. CHANGELOG entry. Tag `v0.2.0-m3` via `/release` skill.
**Branch**: `chore/v0-2-m3-release`
**Flag**: N/A
**Est. lines**: ~150 (Cargo.toml bump ~10, CHANGELOG ~40, perf-measurement script + result write-up ~100)
**Ships via**: `/release` (per project conventions — tagged milestone release)

**Objective**: Verify nothing was cut corner-of-the-eye and ship the milestone tag. The plan moves to `done/`.

**Why this phase exists**: Roadmap and project rules require a verification sweep at end-of-milestone. The per-phase code-reviewer is good but not cumulative — this is the only place the full milestone's diff gets reviewed as one.

**Current-state anchors**:
- `Cargo.toml:18` — `version = "0.2.0-m2"` (after M2's bump); change to `0.2.0-m3`
- `CHANGELOG.md` (existence verified at phase start; create if missing)
- `.claude/plans/active/v0-2-m3-fmt.md` — this plan; status flipped to `done` at the very end

**Files (expected scope)**:
- EDIT: `Cargo.toml` — bump `version = "0.2.0-m3"`
- EDIT: `CHANGELOG.md` — new section for v0.2.0-m3
- NEW: `crates/ynz-fmt/PERFORMANCE.md` — recorded numbers vs budgets from Performance invariant
- EDIT: `.claude/plans/active/v0-2-m3-fmt.md` — flip `status: active` → `status: done` after final reviewer PASS (radar moves to `plans/done/` on next rebuild)

**Deviation rule**: Standard.

**Steps**:
1. **TODO sweep**: `grep -rn 'TODO\|FIXME\|HACK\|XXX\|PLACEHOLDER\|TEMP\|will do later\|Phase N' crates/ynz-fmt/ crates/ynz-parser/src/lexer.rs crates/ynz-driver/src/fmt*` — confirm zero hits. Any hit: move to `.claude/todos.md` or fix.
2. **Todos cross-check**: read `.claude/todos.md`; confirm any "Soon" items related to M3 are addressed or appropriately deferred to M4/M5. No M3 commitments are left unfinished.
3. **Shortcut detection**: scan `crates/ynz-fmt/src/**` for: `unimplemented!()`, `todo!()`, `panic!("...")` in non-test code, hardcoded literal strings that should be configurable (line width, indent width — confirm they're named constants not magic numbers).
4. **Quality checklist verification**: walk every Invariant subsection's bullets; assert each is verified by a passing test or explicit invariant check.
5. **Performance measurement**:
   - Time `ynz fmt examples/basics/entrypoint.ynz` (release build, 5 runs, median) — assert <100ms
   - Generate a 5000-line synthetic `.ynz` file (5000 function-decl statements with arithmetic bodies); time `ynz fmt` — assert <500ms
   - Time `ynz fmt --all examples/` over ~100 files — assert <2s
   - Time `lex_with_trivia` vs `lex` on entrypoint.ynz, 100 iterations each — assert overhead <20%
   - Write results to `crates/ynz-fmt/PERFORMANCE.md` with timestamps + machine class
6. **Cargo.toml version bump**: `0.2.0-m2` → `0.2.0-m3`. `cargo build --workspace` confirms compilation.
7. **CHANGELOG entry**:
   - `## v0.2.0-m3 (2026-MM-DD) — `ynz fmt`` — formatter library + CLI
   - Bullet list of user-visible changes (new `ynz fmt` subcommand with four modes; library API for LSP)
   - Note: format-on-save LSP wiring ships in v0.2-M5
8. **Final code-reviewer sweep**: invoke `code-reviewer` with cumulative diff:
   ```
   Agent({ subagent_type: "code-reviewer", description: "Final M3 review",
     prompt: "End-of-plan review for v0.2-M3 fmt at .claude/plans/active/v0-2-m3-fmt.md. Diff command: git diff <m2-tag>..HEAD covering Phases 0-6. Audit cumulative diff against all 7 invariant subsections, all Quality Gate items per phase, the plan's overall Quality Checklist, and catch anything per-phase reviews missed. Output in standard format." })
   ```
   Same iteration loop (max 3 rounds; BLOCK is binding).
9. On final PASS: invoke `/release` skill to bump Cargo.toml (already done step 6), commit with explicit message, tag `v0.2.0-m3`, push tag + branch with user approval.
10. Flip `status: active` → `status: done` in plan front-matter; bump `last_updated:`. Radar moves the file to `plans/done/` on next rebuild.

**Acceptance criteria**:
- [ ] TODO sweep finds zero hits in M3-touched code
- [ ] All Quality Checklist items below verified
- [ ] All 7 Invariant subsection bullets verified by test/proof
- [ ] Performance measurements recorded in `PERFORMANCE.md`; all within budget
- [ ] `Cargo.toml` workspace version = `0.2.0-m3`
- [ ] `CHANGELOG.md` has v0.2.0-m3 entry
- [ ] Final code-reviewer verdict = PASS (after at most 3 rounds)
- [ ] `v0.2.0-m3` tag created and pushed (with user approval)
- [ ] Plan `status: active` → `status: done`; `last_updated:` bumped
- [ ] All 830+ existing tests pass; new fmt-specific tests pass; proptest passes; semantic round-trip passes
- [ ] `examples/basics/entrypoint.ynz` and `examples/errors/v0_2_m3_errors.ynz` round-trip canonically

**Quality gate**:
- [ ] No `// TODO` / `// FIXME` / `// HACK` anywhere in M3-touched code
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo fmt --all --check` passes (formatter doesn't break Rust formatting either)
- [ ] No commented-out code anywhere
- [ ] All `.ynz` files in repo round-trip canonically (verified by `ynz fmt --check --all` on every dir)
- [ ] Public API of `ynz-fmt` documented (rustdoc lint passes)

**Verification**:
- `cargo test --workspace 2>&1 | grep 'test result'` — full suite green
- `./target/release/ynz fmt --check --all examples/ && echo "exit: $?"` — exit 0
- `cat Cargo.toml | grep "^version"` — `0.2.0-m3`
- `git tag | grep v0.2.0-m3` — tag exists
- `cargo doc --no-deps -p ynz-fmt` — clean doc build, no warnings

**Exit Sequence:**

1. **Persist plan state.** Tick all phase checkboxes accurately across the plan; bump `last_updated:`. Update overall Quality Checklist below.
2. **Invoke code-reviewer.** As Step 8 above (cumulative diff).
3. **Handle verdict.** BLOCK → fix → re-invoke (max 3 rounds). PASS → continue.
4. **Invoke `/release`** to cut the tag.
5. **Flip `status: active` → `status: done`** in front-matter.
6. **Prompt user.** "M3 done. `v0.2.0-m3` tag cut. Plan moved to `plans/done/`. Ready to plan v0.2-M4 (`ynz watch`)?"

---

## Quality Checklist (verify at completion)

- [ ] All inputs validated (filesystem paths, stdin UTF-8, CLI args via clap)
- [ ] Error handling: specific messages, no stack traces to user, proper exit codes (0/1/2)
- [ ] No silent failures — every parse error reaches the user via the diagnostic renderer
- [ ] No `as any` equivalent: no `.unwrap()` on user-input paths; no Rust `unreachable!()` reachable from input
- [ ] Performance: format speed budgets met (Phase 6 measurements)
- [ ] Tests: happy path + error cases + edge cases (long lines, comment blocks, backtick strings) + idempotency + semantic round-trip + proptest fuzz
- [ ] Existing 830+ tests still pass
- [ ] Types are complete (no `as any`, no excessive `.unwrap()`)
- [ ] Follows existing codebase conventions (Rust formatter passes; ynz-fmt itself rejects any non-canonical Yinz source)
- [ ] Every phase received a code-reviewer PASS before committing (Step 9a per phase)
- [ ] Final cumulative code-reviewer sweep passed (Step 10f)
- [ ] Plan-file acceptance-criteria checkboxes accurate across all phases (Step 10e)
- [ ] Registry consumer status verified: formatter reads keyword spellings from `ynz-registry`; no fork
- [ ] Trivia capture (`lex_with_trivia`) and `lex` share implementation (no drift)
- [ ] Semantic round-trip: `parse(fmt(x)) ~= parse(x)` modulo trivia
- [ ] Idempotency: `fmt(fmt(x)) == fmt(x)` byte-identical
- [ ] All `.ynz` files in repo are canonical after Phase 4 mass-rewrite
- [ ] Library API frozen; v0.2-M5 can consume without coordination

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: Each phase = one PR via `/pr` skill (Phases 0-5) or `/release` (Phase 6). Code-reviewer agent at every phase boundary catches drift. No "let me bundle these three phases" shortcuts.
- **Shadow main branches**: All phases branch from `main` (after the previous phase merges). No long-lived feature branch that accumulates multiple phases.
- **Building the engine before shipping value**: M3 itself is a "ship before completing the v0.2 vision" slice — the formatter is usable from Phase 5 onward without the LSP wiring (which lands in M5). Within M3, Phase 4's mass-rewrite ensures the formatter is exercised on real code, not just hand-curated fixtures, before tagging.
- **Hotfix that isn't**: M3 ships zero language behavior changes; only the new `ynz fmt` subcommand. If a regression to `ynz build`/`run` ships, it's a M3 bug — fix before tag, don't ship and patch.
- **Abandoned branches**: Phase 1's losing spike is DELETED end-of-phase (preserved in git history). No long-lived `_spike/` cruft.
- **Flag graveyards**: M3 introduces no feature flags. All four CLI flags are user-facing controls, documented, tested, with no opt-in/opt-out gating logic.

---

## Deferrals

Items deliberately NOT in M3 scope. Each row's "Where tracked" cell points to the durable home that survives after this plan moves to `done/`.

| Deferred item | Why deferred | Where tracked |
|---|---|---|
| `textDocument/formatting` LSP handler wiring | M3 ships the library API; M5 wires it | `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` Milestone v0.2-M5 scope (format-on-save bullet) |
| `format_range(source, range)` API for LSP `textDocument/rangeFormatting` | Whole-file formatting is enough for editor format-on-save; range-formatting is hard and unproven need | `.claude/todos.md` "Later" — `lsp-range-formatting`: design + implement IF v0.2-M5 proves a need |
| Embedded SQL formatting inside `sql`...`` template literals | Out of v0.2 per roadmap (deferred to database stdlib milestone v0.6+) | `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` Out of Scope section (already lists this) |
| Embedded Markdown / regex / JSON inside string literals | Not even designed; v1+ at earliest | Roadmap Out of Scope |
| Sorting imports as a formatter behavior | Belongs to Tier 3 lint suggestions (v0.4), NOT formatter | `design/linting.md` (v0.4 milestone surface) |
| `ynz fmt --diff` mode (unified-diff output of what would change) | Useful for code review tooling but not blocking M3 ship | `.claude/todos.md` "Later" — `fmt-diff-mode`: add `--diff` flag emitting unified diff |
| Inline comments between array/map literal elements (`[1, // note\n 2, 3]`) | Requires making `emit_expr` comment-aware for element-level comment attachment — significant scope beyond Phase 3's per-statement approach. `comment_in_array.ynz` + `comment_in_map.ynz` fixtures currently test "leading comment before array-creating stmt" instead. | `.claude/todos.md` "Later" — `fmt-inter-element-comments`: implement element-level comment attachment in emit_expr for ArrayLit/MapLit/StructLit when long-line split is triggered |
| Format-as-you-type partial reformatting in LSP | v0.2-M5 (or later) if at all | `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` Milestone v0.2-M5 (or v0.3 LSP improvements) |
| Auto-fix for banned-jargon (formatter rewrites `void` → `nothing`) | Formatter NEVER changes identifiers/keywords semantically; this is a Tier 3 lint suggestion territory (v0.4) | `design/linting.md` |

---

## Cross-References

- `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` — parent roadmap (M3 entry)
- `.claude/plans/active/v0-2-m2-lsp-thin-slice.md` (or `done/` once tag cuts) — sibling LSP milestone
- `.claude/plans/done/v0-2-m1-feature-inventory-sync.md` — registry SSOT this plan consumes
- `design/fmt.md` — created Phase 0; architectural reference (algorithm choice, comment-merge strategy, library API contract)
- `design/lsp.md` — references `textDocument/formatting` deferred to M5 wiring v0.2-M3's library
- `design/mvp-scope.md:89-93` — v0.2-M3 entry; expanded Phase 0
- `design/feature-registry.md` — registry consumer rule M3 follows
- `.claude/rules/plan-invariants.md` — 7-subsection invariants block
- `.claude/rules/feature-registry.md` — registry consumer rule
- `.claude/rules/auto-promotion.md` — analyzed (no candidates for M3)
- `.claude/rules/non-oop.md` — Yinz is not OOP (formatter respects: no method bodies inside shape decls)
- `.claude/rules/dot-postfix.md` — formatter emits dot-postfix correctly (parens for actions, no parens for access)
- `.claude/rules/vocabulary.md` — banned-jargon audit extended to formatter output
- `crates/ynz-registry/src/lib.rs:16-80` — consumer adapters M3 uses
- `crates/ynz-parser/src/lexer.rs:73-100` — comment-handling refactor point
- `Cargo.toml` — workspace bump for the tag
- `~/.claude/memory/branching.md` — PR sizing + flag conventions
- `~/.claude/rules/no-duct-tape.md` — Right-Design-Now (no "acceptable for now" in this plan)
- `~/.claude/rules/verification.md` — Paper-Trace format for any bug fixes
