---
slug: v0-2-m2-lsp-thin-slice
type: execution
owner: Patrick Rizzardi
status: active
roadmap: v0-2-dev-loop-tooling
created: 2026-05-20
last_updated: 2026-05-20
files:
  - crates/ynz-lsp/**
  - crates/ynz-registry/src/lib.rs
  - tooling/vscode-ynz/**
  - design/lsp.md
  - design/mvp-scope.md
  - design/compiler-language.md
  - CLAUDE.md
  - examples/basics/src/entrypoint.ynz
  - examples/errors/v0_2_m2_errors.ynz
  - Cargo.toml
---

# Plan: v0.2-M2 — LSP Thin Slice + VSCode Plugin

Created: 2026-05-20
Status: in_progress — Phase 3 COMPLETE (code-reviewer PASS, 3 rounds). Phase 4 next.

## Context & Why

**Goal**: Ship a JSON-RPC-over-stdio Language Server (`ynz-lsp`) backed by the existing salsa-tracked compiler queries, plus a VSCode extension that spawns it. Editor users get autocomplete, inline diagnostics, and basic hover for `.ynz` files. Every editor feature reads from the SSOT registry built in v0.2-M1, so adding a keyword/intrinsic/deferred-feature in any future version updates the editor automatically.

**Why now**:
- v0.1.0 shipped with the compiler structured around `salsa` queries from day 1 (per `design/compiler-language.md` "Why Salsa") specifically to make this milestone a wrapper job rather than a rewrite. Verified: `crates/ynz-parser/src/db.rs` exposes `CompilerDb`, `SourceFile (#[salsa::input])`, and `SourceFileRegistry` (db supertrait). Tracked queries already exist: `parse_query`, `module_signatures_query`, `check_query`, `codegen_query`.
- v0.2-M1 shipped the SSOT registry (`crates/ynz-registry/src/lib.rs`) with 189 entries across 9 entry kinds plus typed adapter functions. Every editor feature this milestone ships is a direct consumer of that registry — no hardcoded keyword/intrinsic/deferred-feature lists land in this milestone.
- The roadmap (`v0-2-dev-loop-tooling`) puts the thin LSP early specifically so Patrick can eyes-on test it while v0.2-M3 (fmt) and v0.2-M4 (watch) are being built in parallel. Without M2 shipping early, all of v0.2 is invisible until the very end.

**Background**:
- 9 workspace crates, ~45k Rust lines, 830+ tests on `main`. Cargo.toml currently at `0.2.0-m1`.
- `SourceSpan` is byte-offset based (`pub start: usize, pub end: usize`). LSP `Position` is line+character. A byte-offset → LSP-Position converter is needed; UTF-8 negotiation via `general.positionEncodings` capability sidesteps the LSP-default UTF-16 conversion mess and matches the compiler's existing byte indexing exactly.
- Diagnostics use a three-part WHAT/WHAT-INSTEAD/WHY format (enforced by the `Diagnostic` constructor — panics on empty fields per Golden Rule 11). Severity = Error / Warning / Suggestion. Ariadne renders the CLI form.
- No external editor distribution exists today. No VSCode publisher account exists today.

**Constraints (locked from roadmap + this session's research)**:
- **Yinz is NOT object-oriented** (`.claude/rules/non-oop.md`). LSP autocomplete after `.` lists STANDALONE functions whose first parameter type matches the receiver (UFCS), not "methods bound to the type." Primitive intrinsics are the special case (they ARE keyed by `receiver_type` in the registry); user-defined methods come from typeck's existing signature lookup.
- **LSP framework: research phase locks the choice (Phase 1)**. Patrick chose research-first over pre-decide. Locked choice carries through to v0.2-M5 (no re-decision).
- **VSCode extension home: in-repo subdir `tooling/vscode-ynz/`** (per Patrick decision this session). Atomic version bumps; single source of truth.
- **Marketplace publish: in-M2 as "preview", with .vsix fallback if publisher verification stalls**. Patrick's caveat: "as long as it's not a huge headache" — Phase 7 has an explicit fallback branch.
- **TextMate grammar is registry-derived** (per Patrick decision this session). A `crates/ynz-tmgrammar` binary reads `ynz-registry` and emits `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json`; the file is committed; a consistency test re-runs the generator and fails if the checked-in artifact is stale.
- **`positionEncodings: ["utf-8", "utf-16"]`** advertised at `initialize` time. Server prefers UTF-8 (byte-accurate to compiler internals); falls back to UTF-16 if client lacks UTF-8 support (per LSP 3.17 spec).
- **No new language features** — tooling only. Auto-promotion analyses, lint rules, stdlib all stay deferred. Verified: this milestone introduces zero new tokens, zero new typeck/codegen behavior.
- **Existing 830+ tests must still pass.** New tests added; no existing tests weakened.
- **All compile errors continue WHAT/WHAT-INSTEAD/WHY format** — the LSP renders the same diagnostics the CLI does, transformed to LSP `Diagnostic` shape.
- **Compiler binary's behavior on a `.ynz` file is byte-identical** — `ynz build` and `ynz run` produce the same output, same exit codes, same error text as before M2.

**Out of M2 scope (deferred to v0.2-M5)**:
- `textDocument/definition` (go-to-def)
- `textDocument/references` (find-refs)
- `textDocument/rename`
- `textDocument/formatting` / `textDocument/rangeFormatting` (waits on v0.2-M3 `ynz-fmt` library)
- Muted-hint surfaces (`textDocument/inlayHint` for the three placement categories from `design/inference.md`)
- Code actions / code lenses
- Semantic tokens (richer-than-TextMate highlighting)
- The "deferred-to-v0.2" sweep of `todos.md` items (hidden-field default eval, dynamic dispatch call-site coercion, UFCS const-lend check)

**Success criteria**:
- `ynz-lsp` binary builds and runs as a stdio JSON-RPC server.
- A VSCode user installs the Yinz extension (marketplace preview or `.vsix`), opens a `.ynz` file, and sees: inline red squiggles with WHAT/WHAT-INSTEAD/WHY content; autocomplete of keywords, types, primitive methods (filtered by receiver), and deferred features marked deprecated; hover popups showing the registry's WHY content for any registered item.
- Adding a new keyword/intrinsic/deferred-feature entry to `registry/features.toml` and rebuilding makes it available in the IDE on next server restart — no manual code edit anywhere in `ynz-lsp` or `tooling/vscode-ynz/`.
- TextMate grammar regenerates automatically; CI fails if the committed grammar drifts from registry.
- All 830+ existing tests pass. New `ynz-lsp/tests/*` integration tests cover lifecycle + each LSP request type.
- Tag cut: `v0.2.0-m2` (intermediate; v0.2.0 final ships at v0.2-M5).

## Research Findings

**Salsa wiring (verified 2026-05-20 against `crates/ynz-parser/src/db.rs:1-67`)**:
- `CompilerDb` is the concrete salsa database with `Storage<Self>` + a `source_registry: HashMap<String, SourceFile>` field for cross-file lookup. `Default` derive enables fresh-DB construction. The LSP creates exactly one DB at startup and mutates `SourceFile.text` inputs on `didChange` — salsa invalidates downstream queries automatically.
- `#[salsa::input] SourceFile { path: String, text: String }` — this is the LSP's write surface. `text.set(&mut db, new_text).to(new_text)` triggers re-computation.
- Tracked queries: `parse_query(db, source) -> Arc<ParseResult>` (`ynz-parser/src/queries.rs:25`), `module_signatures_query(db, source) -> Arc<SignatureOutput>` (`ynz-typeck/src/queries.rs:49`), `check_query(db, source) -> Arc<CheckOutput>` (`ynz-typeck/src/queries.rs:100`), `codegen_query(db, source) -> ...` (`ynz-codegen/src/queries.rs:20`). The LSP runs `check_query` (deepest typeck pass) and pulls the `DiagnosticBucket` from its output. Codegen is not run from the LSP (no IR/binary output needed).

**Registry consumer surface (verified against `crates/ynz-registry/src/lib.rs`)**:
- 9 entry kinds (KeywordEntry, BannedDeclarationKeywordEntry, BannedJargonEntry, PrimitiveIntrinsicEntry, TypeAttachedConstantEntry, DeferredLanguageFeatureEntry, DeferredToolingFeatureEntry, DiagnosticTemplateEntry, MutedHintDomainEntry).
- Adapter functions already exposed: `keywords()`, `keyword_lookup(name)`, `banned_jargon()`, `banned_jargon_lookup(word)`, `primitive_intrinsics()`, `primitive_intrinsic_methods(receiver_type, name)`, `primitive_free_fns(name)`, `type_attached_constants()`, `type_attached_constant_lookup(type_name, const_name)`, `deferred_language_features()`, `deferred_language_feature_lookup(name)`, `deferred_tooling_features()`, `diagnostic_templates()`, `diagnostic_template_lookup(kind_name)`, `muted_hint_domains()`, `muted_hint_domain_lookup(domain)`. New M2 surface: `lsp_completion_items(context)` and `lsp_hover_for_token(name)` (Phases 4-5 add these).
- Entry counts: 29 keywords, 17 banned-declaration-keywords, 55 banned-jargon, 43 primitive-intrinsics, 8 type-attached-constants, 15 deferred-language-features, 3 deferred-tooling-features, 10 diagnostic-templates, 9 muted-hint-domains.

**LSP framework comparison (Context7 + WebSearch, 2026-05-20)** — DEFERRED to Phase 1 spike per Patrick's choice. Pre-research observations only:
- `tower-lsp` (lebensterben/tower-lsp-server, the maintained fork as of mid-2026 after the original repo went unmaintained late 2025): async via tokio, Trait `LanguageServer` with `async fn` request handlers, JSON-RPC routing via `jsonrpsee`. Used by `wasm-language-tools`, `terraform-ls`, many newer Rust LSPs. ~3k LOC dependency surface.
- `lsp-server` (rust-analyzer crate, vendored from rust-analyzer's repo): synchronous, lower-level — you own the `Connection` and dispatch loop. rust-analyzer itself uses it. Smaller dep tree, fewer abstractions to fight when you need control.
- Both consume `lsp-types` (the official Microsoft-blessed type definitions). The decision is about plumbing, not data shapes.
- The Phase 1 spike builds a minimal "hello LSP" against each (responds to `initialize`/`shutdown`, publishes one hardcoded diagnostic on `didOpen`) and measures: lines of plumbing required, async/sync ergonomics with salsa's `&mut db` requirement on inputs, integration-test setup complexity.

**Spike-decision criterion (locked here, executed in Phase 1)**: choose the framework that gives the smaller plumbing+test footprint while not forcing async semantics over the salsa DB (which is `Send` but not `Sync` by default; cross-thread access requires per-request snapshots or a single dispatch thread). If both spikes pass the bar, default to `tower-lsp` (more active, more sample code online, simpler request-handler shape).

**LSP capability negotiation strategy**:
- Server response to `initialize.params.capabilities.general.positionEncodings`: advertise `["utf-8", "utf-16"]`. Pick `utf-8` if client supports it; fall back to `utf-16` otherwise. The byte-offset → Position converter handles both; the `utf-16` path uses `unicode-segmentation` (already a workspace dep) to count code units, the `utf-8` path is a direct line-table scan.
- Server capabilities advertised: `textDocumentSync: Incremental` (per `didChange`), `completionProvider: { triggerCharacters: [".", " "] }`, `hoverProvider: true`, `diagnosticProvider` NOT used — we push via `publishDiagnostics` (push model is sufficient for thin slice; pull model is a v0.2-M5 add).

**Branching/PR sizing** (per `~/.claude/memory/branching.md`): each phase = one branch off `main`, one PR via `/pr`. Soft target ~500 lines/PR. Phase 5 (TM grammar) and Phase 6 (VSCode extension) are intentionally separate because they're independent artifacts — grammar regenerates from registry, extension is a TypeScript module that consumes the grammar. The marketplace-publish phase (Phase 7) is ~50 lines of config + the marketplace dance itself (network operation, not a PR).

**VSCode extension prior art**:
- Standard pattern (rust-analyzer, deno, tinymist): `package.json` + `extension.ts` that spawns the LSP binary via `child_process.spawn(...)` and pipes stdio through a `vscode-languageclient`. ~200 lines of TS.
- Marketplace publish: `vsce` CLI (Visual Studio Code Extensions). Requires Azure DevOps publisher account + Personal Access Token. Typical setup time: 15-30 min; verification delays rare for non-enterprise publishers.
- `.vsix` fallback: `vsce package` produces a single `.vsix` file installable via `code --install-extension foo.vsix`. Patrick can ship that to himself even if marketplace stalls.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| LSP framework choice (tower-lsp vs lsp-server) turns out to be wrong mid-M2, requires migration | Medium | Medium | Phase 1 spike builds both, measures plumbing footprint + test ergonomics, locks BEFORE the real scaffolding starts. Lock decision committed to `design/lsp.md` so v0.2-M5 doesn't re-litigate. |
| Salsa DB threading model fights LSP's async request model (DB inputs require `&mut`; salsa is `Send` but not `Sync`) | Medium | High | Single-threaded dispatch in Phase 2 — JSON-RPC requests serialized through one task that owns the DB. Concurrent requests queue. For thin slice this matches throughput needs (one Patrick, one editor); v0.2-M5 may revisit with salsa snapshots when go-to-def scales horizontally. |
| Byte-offset → LSP Position conversion off-by-one bugs (LF vs CRLF, multi-byte characters) | Medium | Medium | Position conversion has unit tests covering: LF-only files, CRLF files, mixed line endings, BOM, 4-byte UTF-8 (emoji), surrogate-pair UTF-16. Property test: round-trip `byte → Position → byte == original`. UTF-8 path is the default; UTF-16 path tested independently. |
| Diagnostic transform loses WHAT/WHAT-INSTEAD/WHY structure when packed into LSP `Diagnostic.message` (plaintext field) | Medium | Low | Concatenate as `"<WHAT>\n\nWHAT-INSTEAD: <wi>\n\nWHY: <why>"`. Verified against VSCode renderer behavior: plaintext newlines render as soft breaks in hover squiggles. Test asserts the exact format; UI verification is part of Phase 8. Future enhancement: structured `data` field for custom client rendering — out of M2 scope. |
| Autocomplete context-detection is too naive — fires the wrong suggestions in real code | High | Low | Thin-slice context detection: look at the previous non-whitespace char. `.` → primitive methods on inferred receiver type + UFCS candidates from typeck signatures. Whitespace/start-of-line → keywords + visible identifiers (functions, shapes, top-level consts/lets). Disambiguate `x.` from numeric literal `5.0`: walk left from the `.` skipping whitespace; if the previous non-whitespace char is an ASCII digit (`0`-`9`) AND not preceded by an identifier-character, treat as numeric-literal-with-decimal-point and return NO completion (LSP `null` response) rather than incorrect method suggestions. Test covers `let x = 5.<cursor>` → empty completion vs `let x = score.<cursor>` → int method completion. |
| Hover token detection requires re-lexing around byte offset; performance hit at every cursor move | Medium | Low | Lex-on-demand for hover is bounded by file size; for typical .ynz files (<50KB) the relex is <1ms. Salsa caches the parse so re-lexing the WHOLE file isn't actually needed — we just locate the AST node containing the offset, then read its span. Falls back to local re-lex if AST resolution misses. |
| Marketplace publisher verification delays block Phase 7 indefinitely | Low | Medium | Phase 7's first sub-step is "register publisher + run `vsce publish --pre-release` dry-run." If verification takes >24h, fork to the `.vsix` fallback: `vsce package` produces a `.vsix`, attach to a GitHub release, Phase 9 README points users at it. Patrick's "as long as it's not a huge headache" caveat is the explicit trigger for this branch. |
| New `vsce` / `node` / `npm` toolchain in `tooling/vscode-ynz/` causes CI breakage if CI doesn't have Node | Medium | Low | `tooling/vscode-ynz/` build is OPT-IN — not wired to `cargo build --workspace`. CI runs Rust workspace tests only; extension build is a separate `npm run build` invoked manually OR by a Phase 7 GitHub Actions job that runs only on `tooling/**` PRs. Documented in `tooling/vscode-ynz/README.md`. |
| TextMate grammar generator falls out of sync with VSCode-flavored TM grammar dialect (Oniguruma regex quirks) | Low | Low | Phase 5's generator emits a minimal grammar (keyword tokenization only — no semantic highlighting). Oniguruma regex used = simple word-boundary `\b(keyword)\b` patterns; no lookbehind / nested captures / other Oniguruma-specific quirks. Snapshot test against a hand-validated reference grammar locks the format. |
| LSP exposes existing compiler bugs that CLI users hadn't hit (LSP runs check on every keystroke; bugs surface that batch builds skipped) | High | Low | Acknowledged risk per roadmap. Bugs surfaced get triaged: if M2 phase work can fix in <50 lines, fix in M2; otherwise add to `.claude/todos.md` "Soon" section. Roadmap budgets M5 for this. |
| Integration tests for LSP require spawning subprocesses; flaky on CI under load | Medium | Low | Two-tier test harness: pure-Rust in-process tests against the LSP service struct directly (fast, no fork) AND a smaller end-to-end stdio test that proves wire format works (one test per major request type, marked `#[ignore]`-able if CI gets flaky). Phase 8 covers the harness design. |
| Cargo.toml workspace bump to `0.2.0-m2` collides with mid-flight uncommitted work in other branches | Low | Low | Phase 9 (verification + tag) is intentionally the LAST phase; the bump happens after all other phases merge to main. No parallel work expected during M2 — Patrick is solo dev. |
| Self-hosting transition (v2+) — `ynz-lsp` and `tooling/vscode-ynz` would need re-implementation in Yinz | Low (timing) | Low | Same status as `ynz-registry` build.rs: documented in `design/lsp.md` "Self-hosting migration plan" subsection. TOML parsing already required for `yinz.toml` (per `design/packages.md`); JSON-RPC parsing is a stdlib `v0.9 json` module concern. No M2 work needed. |
| Patrick burns out on marketplace publishing dance and stalls the milestone | Low | Medium | Explicit Phase 7 fork: if marketplace setup blocks for >30min of cumulative friction, abort to `.vsix` fallback. Marketplace publish becomes a follow-up issue, NOT a v0.2-M2 gate. Acceptance criterion for Phase 7 PASSES on `.vsix` ship; marketplace publish is a stretch goal. |

## Questions

None outstanding. Four answered this session before plan draft:
1. LSP framework decision: **Phase 1 research spike, lock then proceed**.
2. VSCode extension home: **`tooling/vscode-ynz/` in-repo subdir**.
3. Marketplace publish scope: **In M2 as preview, with .vsix fallback if verification stalls**.
4. TextMate grammar source: **Registry-derived; committed artifact + consistency test**.

## Risk Assessment & Rollout Strategy

**Risk level: MEDIUM**

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | Compiler tooling only |
| Touches auth/permissions | No | No auth |
| Raw SQL / literals | No | No DB |
| Modifies existing data | No | New crate + new subdir; compiler binary's behavior unchanged on existing .ynz inputs |
| Third-party integration | Yes | LSP framework crate (tower-lsp OR lsp-server), `lsp-types`, VSCode marketplace |
| Changes existing endpoints | N/A | Not a service; CLI behavior unchanged |
| New feature with no equivalent | Yes | First IDE story for Yinz |

**Mitigations applied**:
- Two-tier test harness (in-process + subprocess) from Phase 2 onward → MEDIUM-HIGH → MEDIUM
- Marketplace .vsix fallback documented + scope-gated → MEDIUM → LOW for publish risk
- Salsa already does the heavy lifting (no new incremental-build infra) → MEDIUM → LOW for incremental-correctness risk
- Compiler binary behavior byte-identical (no production-path changes) → MEDIUM → LOW for regression risk
- Registry-derived TM grammar (no drift class) → LOW (preemptive mitigation)

**Rollout plan** (Yinz convention: trunk-based, no production rollout; "rollout" = milestone tag):
1. Each phase: branch from main, PR via `/pr`, code-reviewer agent at phase boundary, merge to main on PASS
2. Phase 9 (final verification + tag): cut `v0.2.0-m2` tag after full test sweep + fixture run + extension install verification
3. v0.2.0 final tag waits for v0.2-M5 per roadmap

## Invariants This Milestone Must Preserve

### Safety
- All 830+ existing tests pass post-milestone (`cargo test --workspace`)
- No previously-valid `.ynz` program becomes rejected by the compiler — verified by full test suite + every `examples/basics` and `examples/errors` fixture compile-runs identically
- No previously-rejected `.ynz` program becomes accepted — verified by `examples/errors/*.ynz` snapshot stability
- `ynz build` and `ynz run` exit codes, stdout, stderr are byte-identical to pre-M2 for every existing fixture
- The `ynz-lsp` binary, when not running, has ZERO effect on compilation
- New crate `ynz-lsp` does NOT depend on `ynz-codegen` (LSP has no need for codegen; isolating it keeps the LSP startup fast and avoids inkwell linkage in the LSP binary)
- LSP cannot modify source files — `didChange` updates the in-memory salsa input only; never writes to disk

### Performance

**Targets are INITIAL BUDGETS, not spec-derived hard requirements.** They are calibrated against (a) typical rust-analyzer numbers for similar workloads (rust-analyzer publishes <1s cold init on small projects, <100ms keystroke-to-diagnostic-update via salsa) and (b) Yinz compiler measured numbers: `cargo run -p ynz-driver -- run examples/basics/src/entrypoint.ynz` runs the full pipeline (parse → typeck → codegen → link) in ~3s today, of which check_query (the LSP's hot path) is a small fraction. If the budget is exceeded during Phase 8 measurement, EITHER fix the slow path OR raise the budget with a documented rationale — but DO NOT silently let the budget rot. The numbers below are the bar Phase 8 enforces.

- LSP startup: server initialization + first `didOpen` response (initial diagnostics) on a 100-line `.ynz` file: <500ms cold (machine class: dev workstation comparable to GitHub Actions ubuntu-latest; release build)
- LSP incremental: keystroke → updated diagnostics for a single-file edit: <100ms p95 on a 500-line `.ynz` file (salsa memoization is doing the work; verified by integration-test timing assertions)
- Autocomplete response (`completion` request): <50ms p95 — registry lookup is O(N) over <300 entries; visible-symbol scan is O(symbols in module) capped by salsa cache
- Hover response: <50ms p95 — single registry lookup or single salsa-cached signature read
- Compiler binary cold-build time: within ±10% of pre-M2 baseline (new `ynz-lsp` crate adds a dep but is independent; `cargo build -p ynz-driver` should be unchanged because driver doesn't depend on ynz-lsp)
- TextMate grammar generation: <100ms (one-shot binary; offline build step)

**Auto-promotion analysis** (per `.claude/rules/auto-promotion.md`):
- This milestone does NOT introduce any new language feature, stdlib type, or compiler codegen optimization. There is no stricter/faster form the compiler could prove fits.
- LSP responses are reads against registry data + salsa-cached queries — no codegen, no allocator decisions.
- No codegen auto-promotion. No muted-hint surface added (muted-hint *consumption* is v0.2-M5; this milestone populates zero new muted-hint domains beyond what M1 already shipped).
- No Tier 3 lint suggestion (lint tier ships in v0.4).
- Explicitly considered, not forgotten.

### Teaching
- LSP renders diagnostics with the SAME WHAT/WHAT-INSTEAD/WHY content the CLI produces — verified by Phase 3 integration tests that compare LSP `Diagnostic.message` strings to CLI-rendered output (modulo ariadne ASCII art that doesn't transfer to LSP)
- Hover popups for every registered keyword, primitive method, type-attached constant, deferred feature display the registry's `why` content
- Autocomplete `documentation` field carries the WHY for any item that has one
- Deferred features (sized ints, `gpu`, `foreign`, `test`, etc.) show as `CompletionItemTag::Deprecated` in autocomplete with `documentation` = "ships in vX.Y; use <substitute>; <why>"
- No new banned-jargon words slip into LSP-rendered text (re-uses `crates/ynz-diagnostics` rendering layer; `tests/jargon_audit.rs` already audits user-facing diagnostic output and will be extended in Phase 3 to also audit LSP-transformed messages)
- NEW design doc: `design/lsp.md` — architectural reference: salsa wiring, JSON-RPC dispatch model, capability negotiation, position-encoding strategy, framework choice rationale (the Phase 1 spike output), self-hosting migration plan
- NO new `.claude/rules/` files in this milestone (no new project-rule surface needed beyond what M1's `feature-registry.md` already established)

### Runtime Dependencies
- `ynz-lsp` crate runtime:
  - Chosen LSP framework crate (tower-lsp OR lsp-server — decided in Phase 1)
  - `lsp-types` crate (official Microsoft LSP type definitions)
  - `tokio` (only if `tower-lsp` is chosen — async runtime)
  - `serde` + `serde_json` (already workspace deps for diagnostics)
  - Existing internal deps: `ynz-parser`, `ynz-typeck`, `ynz-diagnostics`, `ynz-registry`
  - NOT `ynz-codegen` (deliberately excluded — LSP doesn't compile to native)
  - NOT `inkwell` (transitively excluded by the above)
  - NOT `ynz-runtime` (no runtime needs)
- `tooling/vscode-ynz/` build-time:
  - Node.js (developer-machine, not CI gate; users install the published extension, they don't build it)
  - `vsce` (Visual Studio Code Extensions CLI) — for packaging + publishing
  - `@types/vscode`, `vscode-languageclient` (npm deps; produces a small extension bundle)
- `crates/ynz-tmgrammar/` build-time:
  - `serde` + `serde_json` (already workspace deps)
  - `ynz-registry` (internal)
- Compiler binary (`ynz-driver`) runtime profile: **byte-identical to pre-M2**. No new deps to `ynz-driver`.

### Kernel-Mode Behavior
- `--kernel` build mode is unaffected. The LSP and VSCode extension are developer-machine tools; they don't run in kernel-mode targets.
- The compiler binary's `--kernel` mode behavior on a `.ynz` file is byte-identical to pre-M2.
- No new compile-error path introduced for kernel-mode programs.
- `design/future/no-runtime-mode.md` cross-reference: the LSP is a host-tool, not a kernel-runtime; same status as `ynz fmt` (when it ships in v0.2-M3).

### Demo & Error Gallery
- `examples/basics/src/entrypoint.ynz`: ADD a top-of-file comment block: `// Open this file in VSCode with the Yinz extension installed to see: hover docs on every keyword, autocomplete after typing 'int.', inline diagnostics for intentional errors (see examples/errors/v0_2_m2_errors.ynz).` No NEW Yinz language code added (M2 ships no new language features per the no-new-language-features constraint).
- `examples/errors/v0_2_m2_errors.ynz`: NEW file. Intentional triggers for every NEW error path the LSP introduces:
  - LSP-specific: any error class introduced by LSP request handlers (likely none — the LSP renders existing diagnostics, doesn't generate new ones)
  - LSP autocomplete behavior demonstration triggers: place a `.` after a known primitive type, after a user-defined shape value, at the start of a line — show what each context surfaces (commentary in the file documents the expected autocomplete; not a compile-error file per se, but a UX-demonstration file)
- A `tooling/vscode-ynz/screenshots/` directory with 3-5 PNG screenshots of the extension in action: hover, autocomplete, inline diagnostic. Linked from the extension README.
- Each error trigger has a `// WHY:` comment naming the diagnostic class (consistent with `examples/errors/v0_2_m1_errors.ynz` precedent)
- `insta` stdout/stderr snapshots in Phase 9 for the `v0_2_m2_errors.ynz` CLI render (LSP-side rendering tested via integration tests, not insta snapshots)

### Feature Registry Entries
- **New entries**: NONE. This milestone is registry-CONSUMER work, not registry-producer work. No new keywords, banned-jargon, primitive intrinsics, type-attached constants, deferred features, diagnostic templates, or muted-hint domains.
- **Modified entries**: NONE. The registry contents stay byte-identical to v0.2-M1.
- **New consumer adapters in `ynz-registry/src/lib.rs`**:
  - `lsp_completion_items(context: CompletionContext) -> Vec<CompletionItem>` — returns LSP CompletionItem-shaped data sourced from KEYWORDS, PRIMITIVE_INTRINSICS, TYPE_ATTACHED_CONSTANTS, DEFERRED_LANGUAGE_FEATURES (the latter marked `CompletionItemTag::Deprecated`). The `CompletionContext` enum is defined in `ynz-registry` and captures: `BareIdentifier` (top-level — show keywords + types + visible-symbols-from-caller), `AfterDot { receiver_type: Option<&str> }` (show primitive methods + UFCS-eligible functions, where the LSP supplies UFCS candidates). The `ynz-registry` crate KNOWS NOTHING about user-defined symbols — those come from typeck via the LSP.
  - `lsp_hover_for_token(name: &str) -> Option<HoverContent>` — returns a `HoverContent { markdown_body: String, kind: Keyword|PrimitiveMethod|TypeAttachedConstant|DeferredFeature|BannedKeyword|...}` for any registered token name. Returns `None` for unrecognized names (LSP then falls back to typeck symbol lookup).
  - These adapters DO NOT add new registry data; they project existing data into LSP-shaped output. The single-source-of-truth rule is preserved.

## Phase Execution Protocol

Each phase ends with an **Exit Sequence** block listing the actions to execute (persist plan state → invoke code-reviewer → handle verdict → prompt commit). Those instructions are commands, not a checklist to tick off.

**Final phase (Phase 9) additionally:**
- Verify ALL phases' acceptance-criteria and quality-gate checkboxes are accurate across the plan
- Invoke `code-reviewer` with the **cumulative plan diff**: `git diff <plan-base-commit>..HEAD`
- Flip `status: active` → `status: done` only after final PASS; the radar moves the file to `plans/done/` on next rebuild

## Phases

**Project Shipping Conventions** (per `/plan` Step 4a, detected from project):
- Per-phase ships via `/pr` (project has local `pr` skill at `.claude/skills/pr/`)
- Per-milestone ships via `/release` (project has local `release` skill at `.claude/skills/release/`)

---

### Phase 0: Doc lockdown + crate scaffolding (no behavior change)

**PR scope**: Land the design doc, update `design/mvp-scope.md` v0.2-M2 entry to reflect the locked decisions, scaffold empty `crates/ynz-lsp/` with `lib.rs` + `main.rs` stubs + Cargo.toml entry, and add `tooling/` as a top-level directory with a `tooling/README.md` describing what lives there. No LSP behavior. No VSCode extension yet.
**Branch**: `chore/v0-2-m2-doc-lockdown` (per branching.md §Branch Prefixes — chore for docs/scaffolding)
**Flag**: N/A
**Est. lines**: ~400 (design doc ~250, cargo updates ~30, scaffolding stubs ~50, docs ~70)
**Ships via**: `/pr`

**Objective**: Lock the architectural decisions made in this planning session into committed docs so subsequent phases can reference one source of truth. Create the crate skeleton so Phase 1's spike work has somewhere to land.

**Why this phase exists**: prevents Phase 1's research spike from getting confused with permanent code. A spike that lives in a clean `crates/ynz-lsp/_spike/tower_lsp/` and `crates/ynz-lsp/_spike/lsp_server/` directory is easy to delete after the decision is made; without the scaffolding-first phase, the spike risks tangling with real code.

**Current-state anchors**:
- `design/mvp-scope.md` — has a v0.2 entry; needs M2-specific lockdown additions per the Architectural Decisions Made in the roadmap
- `crates/` — currently 9 workspace members; M2 adds `ynz-lsp` (and Phase 5 adds `ynz-tmgrammar`)
- `Cargo.toml` workspace member list (`/workspaces/ynz/Cargo.toml:3-13`)
- No `tooling/` directory exists yet

**Files (expected scope)**:
- NEW: `design/lsp.md` — architectural reference doc
- EDIT: `design/mvp-scope.md` — v0.2-M2 entry: name "LSP Thin Slice + VSCode Plugin", deliverables, locked decisions
- EDIT: `design/compiler-language.md` — paragraph on "Both LSP and CLI also share the SSOT registry" (per roadmap "Architectural Decisions Made")
- EDIT: `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` — UPDATE the v0.2-M5 milestone entry to explicitly enumerate every M2-deferral listed in this plan's Deferrals table (see Step 9). Without this, deferrals claim to be "tracked in M5 roadmap" but aren't actually there — Patrick's deferrals-must-be-tracked feedback rule.
- EDIT: `.claude/todos.md` — ADD "Later" bin entry for `vscode-extension-ci-workflow` (the GitHub Actions for `tooling/vscode-ynz/` deferral that has no other durable home).
- EDIT: `CLAUDE.md` — Project Layout section: add `crates/ynz-lsp/` and `tooling/vscode-ynz/` row entries
- NEW: `crates/ynz-lsp/Cargo.toml` (empty deps; framework decision happens Phase 1)
- NEW: `crates/ynz-lsp/src/main.rs` (stub: `fn main() { println!("ynz-lsp v0.2.0-m2 (not yet implemented)"); std::process::exit(0); }`)
- NEW: `crates/ynz-lsp/src/lib.rs` (empty)
- NEW: `tooling/README.md` — describes the tooling/ subtree, naming convention, what's in scope (vscode-ynz so far), build-vs-Cargo separation
- EDIT: `Cargo.toml` — add `crates/ynz-lsp` to workspace members
- NEW: `crates/ynz-lsp/_spike/.gitkeep` — placeholder so the Phase 1 spike has a home

**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work (lint fix in adjacent code, blocking bug, missing dependency). Document each deviation in the PR description with a one-line reason. If a deviation is its own concern, STOP — split into a separate PR.

**Steps**:
1. Write `design/lsp.md` covering: salsa wiring overview (single CompilerDb owned by server, didChange→input.set→salsa invalidates), JSON-RPC dispatch model (single-threaded for thin slice), capability negotiation (positionEncodings utf-8/utf-16, completionProvider triggerChars `[".", " "]`, hoverProvider true, NO definitionProvider/referencesProvider/renameProvider/formattingProvider yet), framework choice rationale (deferred to Phase 1 spike — explicit placeholder section), the byte-offset → Position conversion strategy, self-hosting migration plan (mirrors `design/feature-registry.md` self-hosting section)
2. Update `design/mvp-scope.md` v0.2 entry: replace the existing single-M2 line with a v0.2-M2 expanded entry listing: in-scope deliverables (LSP thin slice + VSCode extension), out-of-scope deferred-to-M5 features (definition / references / rename / formatting / inlay hints / code lenses), the in-repo `tooling/vscode-ynz/` location, the marketplace-preview-with-vsix-fallback decision, the registry-derived-TM-grammar decision
3. Update `design/compiler-language.md`: insert a paragraph at the end of the "Why Salsa" section noting "The LSP shares the salsa DB instance with no parallel pipelines, and consults `ynz-registry` for all keyword/type/intrinsic/deferred-feature metadata — same SSOT as the CLI."
4. Update `CLAUDE.md` Project Layout table: add `crates/ynz-lsp/` (purpose: "LSP server — wraps existing salsa queries in JSON-RPC, consumes `ynz-registry` for IDE features"), add `tooling/vscode-ynz/` (purpose: "VSCode extension — spawns `ynz-lsp`, ships syntax highlighting and language association")
5. Scaffold `crates/ynz-lsp/`: Cargo.toml with workspace=true edition/version/authors/license, src/lib.rs (empty), src/main.rs (the stub above), _spike/.gitkeep
6. Add `crates/ynz-lsp` to workspace members in root `Cargo.toml`
7. Create `tooling/README.md`: explains tooling/ is for build outputs and editor distributions, NOT part of `cargo build --workspace`, opt-in builds only; current subdir = vscode-ynz (Phase 6+)
8. **Enumerate M2 deferrals in the roadmap's M5 milestone entry** (the durable home — this plan file moves to `done/` after the tag and disappears from the radar). Open `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md`, find the `### Milestone v0.2-M5` block (currently lines ~163-167), and EXPAND its scope list so every deferral from this plan's Deferrals table is named explicitly. The Round-1 reviewer was satisfied with the Deferrals table, but the table's "Where tracked" cells need to be TRUE — most currently point to M5 roadmap entries that don't yet enumerate the item. Specifically, the M5 milestone entry must explicitly list:
   - `textDocument/inlayHint` (the LSP method specifically; current text says "muted-hint surfaces" without naming the method)
   - `textDocument/codeAction` (currently only `codeLens` is mentioned)
   - `textDocument/semanticTokens` (NOT in roadmap M5 today)
   - Doc-comment integration in `textDocument/hover` body (NOT in roadmap M5 today)
   - Pull-diagnostics model (LSP 3.17 `textDocument/diagnostic` pull-mode) as alternative to current push-via-publishDiagnostics (NOT in roadmap M5 today)
   - `Diagnostic.code` + `Diagnostic.codeDescription` fields (NOT in roadmap M5 today; might become DiagnosticKind name)
   - Structured `Diagnostic.data` field for client-side rendering of WHAT/WHAT-INSTEAD/WHY (NOT in roadmap M5 today)
   - Edit the roadmap's `last_updated:` to today
9. **Add untracked M2 deferral to `.claude/todos.md`** (the GitHub Actions for `tooling/vscode-ynz/` build/publish — this is infrastructure work that doesn't belong in M5 scope but has no other durable home, and the plan file moves to `done/` after the tag). Add to the "Later" bin: `- [ ] **vscode-extension-ci-workflow** — GitHub Actions to build + publish tooling/vscode-ynz/ on release tags (currently manual). Deferred from v0.2-M2 Phase 7; M2 ships extension via local cargo+npm or marketplace publish, no CI yet. Pick up whenever marketplace publishing automation is wanted OR when a non-Patrick contributor needs to repro the build.`
10. Run `cargo build --workspace` — confirms the empty crate compiles and doesn't break the workspace

**Acceptance criteria** (observable conditions that define DONE):
- [x] `design/lsp.md` exists and includes the seven content sections enumerated in Step 1
- [x] `design/mvp-scope.md` v0.2-M2 entry mentions all four locked decisions (framework=research-phase, extension=in-repo, marketplace=preview-with-fallback, grammar=registry-derived)
- [x] `design/compiler-language.md` mentions LSP shares salsa + registry
- [x] `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` v0.2-M5 milestone entry now explicitly lists ALL 7 M2-deferrals enumerated in Step 8 (inlay hints, code actions, semantic tokens, doc-comment hover integration, pull diagnostics, diagnostic code/codeDescription fields, structured data field). Verified by `grep` of the M5 section for each item name.
- [x] `.claude/todos.md` "Later" bin has the `vscode-extension-ci-workflow` entry from Step 9
- [x] `CLAUDE.md` Project Layout table has rows for `crates/ynz-lsp/` and `tooling/vscode-ynz/`
- [x] `cargo build --workspace` succeeds with the new empty crate
- [x] `cargo test --workspace` still passes (830+ tests)
- [x] `./target/debug/ynz-lsp` prints the placeholder string and exits 0
- [x] No existing test fixture's output changes (compiler behavior unchanged)
- [x] `tooling/` exists as a top-level directory with `tooling/README.md`

**Quality gate** (observable facts to confirm — check BEFORE moving to next phase):
- [x] No `// TODO` / `// FIXME` / `// HACK` left in any new file
- [x] No new banned-jargon in user-facing prose (design/lsp.md is for engineers — "infer" is OK there per `.claude/rules/inference.md` dual-audience disclaimer; never in user-rendered text)
- [x] No `as any` / `#[allow(...)]` swallows
- [x] design/lsp.md cross-references `design/compiler-language.md`, `design/feature-registry.md`, `design/teaching-mission.md`, `.claude/rules/inference.md`, `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md`
- [x] No commented-out code; no orphan files

**Verification**:
- `cargo build --workspace 2>&1 | tail -5` — clean build, no warnings on `ynz-lsp`
- `cargo test --workspace 2>&1 | grep 'test result'` — all 830+ tests pass
- `./target/debug/ynz run crates/ynz-driver/tests/fixtures/m3_fib.ynz` — outputs `55` (regression check)
- `cat design/lsp.md | wc -l` — design doc is substantive (>200 lines)

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick this phase's Acceptance and Quality Gate checkboxes; bump `last_updated:` to today.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 0", prompt: "Review the diff for Phase 0 of plan at .claude/plans/active/v0-2-m2-lsp-thin-slice.md against the phase's acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff main..HEAD. Output in your standard format." })`
3. **Handle verdict.** BLOCK → fix → re-invoke (max 3 rounds). PASS → continue.
4. **Prompt user.** "Phase 0 done. Code-reviewer: PASS. Ready to commit and move to Phase 1?"
5. **Do NOT start Phase 1** until user confirms commit.

---

### Phase 1: LSP framework research spike + decision lock

**PR scope**: Build minimal "hello LSP" against BOTH `tower-lsp` and `lsp-server` in `crates/ynz-lsp/_spike/<framework>/`. Each spike implements: `initialize` lifecycle, single hardcoded diagnostic on `didOpen`, `shutdown`. Measure plumbing footprint, ergonomics with salsa's `&mut db` requirement, and integration-test setup. Lock the choice; commit decision write-up to `design/lsp.md`. DELETE the losing spike (keep history in git; remove from tree for clarity).
**Branch**: `chore/v0-2-m2-lsp-framework-spike` (chore — research, doc lockdown)
**Flag**: N/A
**Est. lines**: ~600 — two ~150-line spikes + ~150 line shared scaffold + decision write-up + deletion of the loser nets to ~300 retained. Cargo.toml deps additions.
**Ships via**: `/pr`

**Objective**: Resolve the LSP framework open question with empirical evidence, not preference. Lock the decision in `design/lsp.md` so v0.2-M5 doesn't re-litigate when scaling to the full LSP.

**Why this phase exists**: Roadmap Risk #2 ("LSP framework choice turns out to be wrong mid-M2") is mitigated by a spike-first pattern. Without the spike, Phase 2's scaffolding bakes in a framework decision made on theoretical grounds; the spike costs ~300 lines of throwaway code now to save ~1500 lines of migration in v0.2-M3+ if the choice is wrong.

**Current-state anchors**:
- `crates/ynz-lsp/_spike/.gitkeep` from Phase 0
- `crates/ynz-lsp/Cargo.toml` (currently empty deps from Phase 0)
- `crates/ynz-parser/src/db.rs:1-67` — `CompilerDb` definition; spike code instantiates one of these
- `crates/ynz-typeck/src/queries.rs:49-100` — `module_signatures_query` + `check_query`; spike's "hello LSP" publishes a diagnostic from a real query

**Files (expected scope)**:
- NEW: `crates/ynz-lsp/_spike/tower_lsp/Cargo.toml` + `src/main.rs` (~150 lines)
- NEW: `crates/ynz-lsp/_spike/lsp_server/Cargo.toml` + `src/main.rs` (~150 lines)
- NEW: `crates/ynz-lsp/_spike/README.md` — what each spike measures, how to run each
- NEW: `crates/ynz-lsp/_spike/MEASUREMENTS.md` — lines of plumbing, async/sync ergonomics, integration-test boilerplate, decision rationale
- EDIT: `design/lsp.md` — replace the "framework choice deferred to Phase 1 spike" placeholder section with the locked decision + rationale
- EDIT: `crates/ynz-lsp/Cargo.toml` — add the chosen framework's deps under the main `[dependencies]` (NOT a spike anymore)
- DELETE (at phase end): `crates/ynz-lsp/_spike/<loser>/` — keep only the winning spike's structure or fold lessons into the real crate; the loser is preserved in git history
- KEPT (after phase): `crates/ynz-lsp/_spike/MEASUREMENTS.md` + winning spike for reference until Phase 2 supersedes

**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work. Document each deviation in the PR description.

**Steps**:
1. Implement the `tower-lsp` spike: `crates/ynz-lsp/_spike/tower_lsp/Cargo.toml` adds `tower-lsp = "...latest..."`, `lsp-types`, `tokio = { version = "1", features = ["full"] }`, `serde_json`. `src/main.rs` implements `LanguageServer` trait with `async fn initialize/initialized/shutdown` and `async fn did_open` that creates a `CompilerDb`, registers the file as a `SourceFile`, runs `check_query`, transforms one diagnostic to LSP `Diagnostic`, calls `client.publish_diagnostics(...)`. Connect via `Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)` for stdio.
2. Implement the `lsp-server` spike: `crates/ynz-lsp/_spike/lsp_server/Cargo.toml` adds `lsp-server = "0.7"`, `lsp-types`, `crossbeam-channel`, `serde_json`. `src/main.rs` opens stdio Connection, drives the request loop manually, handles `initialize`/`shutdown`/`didOpen` with the same CompilerDb-query-publish flow.
3. Both spikes use the same `examples/basics/src/entrypoint.ynz` as the test input.
4. Run both: `cd crates/ynz-lsp/_spike/tower_lsp && cargo run -- <stdio harness>` and equivalent. Use a simple test harness (could be a hand-written shell script that pipes JSON-RPC messages and asserts the diagnostic shows up).
5. Measure each across: total lines of plumbing (excluding query/registry logic which is shared), required Cargo deps, async/sync handler ergonomics with salsa's DB ownership, integration-test setup cost, observed memory at idle, observed time-to-first-diagnostic.
6. Write `MEASUREMENTS.md` with the table. Apply the locked decision criterion from the Research Findings section: smaller plumbing+test footprint without forcing async semantics over salsa DB. Default to `tower-lsp` if both pass.
7. Update `design/lsp.md` framework section: state the choice, name the measurements that drove it, lock the choice for v0.2-M5.
8. Move the winning spike's deps into `crates/ynz-lsp/Cargo.toml` `[dependencies]` (the real crate, not the spike subdir). Delete the loser's `_spike/<loser>/` directory.

**Acceptance criteria** (observable conditions that define DONE):
- [x] Both spikes built, ran, and published at least one LSP diagnostic visible via the test harness
- [x] `MEASUREMENTS.md` documents the measurement methodology AND the recorded values for both frameworks (lines of plumbing, deps, async/sync notes)
- [x] `MEASUREMENTS.md` records each candidate's **last-commit date and open-issue count as of spike day** (tower-lsp's original repo reportedly went unmaintained late 2025; the maintained fork story must be locked here, not assumed). If the winning candidate's last meaningful commit is >6 months stale OR open-issue count >50 OR a critical-bug issue is open and unaddressed, document the migration cost-estimate inline (estimated effort to swap to the alternative if needed mid-M2 or in M5).
- [x] `design/lsp.md` framework section contains a locked decision with explicit rationale tied to the recorded measurements (not vibes)
- [x] `crates/ynz-lsp/Cargo.toml` `[dependencies]` lists ONLY the winning framework's deps, with EXACT version pinned (no `*`, no caret range alone — pin minor for risk control given the maintenance-status concern)
- [x] Loser spike directory removed from tree (preserved in git history)
- [x] `cargo build --workspace` succeeds
- [x] `cargo test --workspace` still passes (no behavior change to compiler)

**Quality gate**:
- [x] No `// TODO` / `// FIXME` / `// HACK` left in any retained file
- [x] MEASUREMENTS.md cites specific file:line counts and benchmark values; no hand-wavy "tower-lsp felt cleaner"
- [x] design/lsp.md framework section has the same one-line-decision-plus-WHY format as decisions in `state.md`
- [x] No new banned-jargon in design/lsp.md
- [x] `cargo clippy --workspace -- -D warnings` passes
- [x] No commented-out code

**Verification**:
- `cargo build --workspace 2>&1 | grep 'warning\|error'` — clean
- `cat crates/ynz-lsp/_spike/MEASUREMENTS.md | grep -E "^(tower-lsp|lsp-server)"` — both frameworks have measurement rows
- `grep -A 10 "## Framework choice" design/lsp.md` — locked decision visible

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`. Add a "Decisions made" bullet to this phase section noting the chosen framework + one-line WHY.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 1", prompt: "Review the diff for Phase 1 of plan at .claude/plans/active/v0-2-m2-lsp-thin-slice.md. Diff command: git diff main..HEAD. Pay special attention to: did the spike measure ACTUAL behavior or theoretical comparisons? Is the framework decision tied to evidence in MEASUREMENTS.md? Output in your standard format." })`
3. **Handle verdict.** BLOCK → fix → re-invoke. PASS → continue.
4. **Prompt user.** "Phase 1 done. Framework locked: <name>. Ready to commit and move to Phase 2?"
5. **Do NOT start Phase 2** until user confirms commit.

---

### Phase 2: Server lifecycle + didOpen/didChange/didClose wired to salsa

**PR scope**: Build the real `ynz-lsp` binary on the locked framework. Implement `initialize` (advertising capabilities including `positionEncodings`), `initialized`, `shutdown`, `exit`. Wire `textDocument/didOpen`/`didChange`/`didClose` to update a long-lived `CompilerDb`'s `SourceFile` inputs. Implement the byte-offset ↔ LSP Position converter (UTF-8 + UTF-16 paths). Add the two-tier integration-test harness scaffolding (in-process tests + one subprocess smoke test). NO diagnostics published yet (that's Phase 3); request handlers should run the salsa queries to confirm wiring but not yet transmit results.
**Branch**: `feat/v0-2-m2-lsp-lifecycle`
**Flag**: N/A
**Est. lines**: ~700 (lifecycle handlers ~150, position converter + tests ~200, test harness ~250, salsa wiring ~100)
**Ships via**: `/pr`

**Objective**: Get the LSP up and running as a salsa-backed server that knows about open files, with the test harness ready to grow into Phases 3-5.

**Why this phase exists**: Establishes the architecture for every subsequent feature phase. Without lifecycle + DB wiring + tests, Phases 3-5 each have to re-litigate "how do I open a file in the test." Front-loading the harness is cheaper.

**Current-state anchors**:
- `crates/ynz-parser/src/db.rs:1-67` — `CompilerDb` + `SourceFile` salsa input
- `crates/ynz-parser/src/queries.rs:25-52` — `parse_query`
- `crates/ynz-typeck/src/queries.rs:49-100` — `module_signatures_query` + `check_query`
- `crates/ynz-diagnostics/src/span.rs:5-30` — `SourceSpan` byte-offset definition
- `crates/ynz-lsp/Cargo.toml` from Phase 1 (framework deps locked)
- `crates/ynz-lsp/src/main.rs` from Phase 0 (stub) — replaced this phase

**Files (expected scope)**:
- EDIT: `crates/ynz-lsp/src/lib.rs` — public API: `LspServer` struct, `run_stdio()` entry, capability-builder helpers
- EDIT: `crates/ynz-lsp/src/main.rs` — invokes `ynz_lsp::run_stdio()`, exits with appropriate codes
- NEW: `crates/ynz-lsp/src/server.rs` — the framework-specific request-handler impls
- NEW: `crates/ynz-lsp/src/state.rs` — `ServerState { db: CompilerDb, open_documents: HashMap<Url, String> }` (Url = file URI; client_text mirrored for incremental edit application)
- NEW: `crates/ynz-lsp/src/position.rs` — byte-offset ↔ LSP Position converter (UTF-8 + UTF-16); pure functions; heavy unit-test coverage
- NEW: `crates/ynz-lsp/src/capabilities.rs` — `server_capabilities()` builder; reports `positionEncodings: ["utf-8", "utf-16"]`, `textDocumentSync: Incremental`, `completionProvider: { triggerCharacters: [".", " "] }` (registered but not implemented yet), `hoverProvider: true` (same), no other providers
- NEW: `crates/ynz-lsp/tests/lifecycle.rs` — in-process integration tests: initialize → didOpen → didChange → didClose → shutdown
- NEW: `crates/ynz-lsp/tests/stdio_smoke.rs` — one subprocess smoke test that spawns `target/debug/ynz-lsp` and exchanges a minimal initialize handshake
- NEW: `crates/ynz-lsp/tests/fixtures/basic.ynz` — small test input
- NEW: `crates/ynz-lsp/tests/harness/mod.rs` — shared test helpers (LSP client mock that drives request/response over channels)

**Deviation rule**: Executor MAY touch files not listed (e.g. add a small helper to `ynz-parser` or `ynz-diagnostics` if the LSP exposes a need not currently in those crates). Document each deviation; if it's its own concern, split into a separate PR.

**Steps**:
1. Define `ServerState { db: CompilerDb, open_documents: HashMap<Url, String>, pending_request_count: ... }`. The DB is mutated in place when documents change; the `open_documents` shadow map exists because LSP `didChange` events can be incremental (range edits, not full-text); the shadow map applies the diff then writes the result to the salsa input.

   **Concurrent-request semantics (LOCKED to avoid duct-tape "executor will figure it out" anti-pattern)**:
   - ALL `textDocument/*` requests AND notifications serialize through a single mpsc channel handled by ONE worker task that owns the `&mut CompilerDb`. No `Arc<Mutex<...>>` over the DB.
   - In-flight queries from a prior notification COMPLETE before the next mutation. salsa's tracked queries are not cancellable mid-execution in the thin slice.
   - If a `didChange` arrives while a previous request's response is still being computed: the worker drains pending mutations BEFORE replying. The in-flight response will reflect SOME state between (n-1) and (n) but is sent to the client unmodified; the client's NEXT request will see post-mutation state.
   - For request types that the LSP spec marks as cancellable (`completion`, `hover`, `definition`): the client may send `$/cancelRequest`. In the thin slice, cancellation is BEST-EFFORT — if the query hasn't started yet (still in the channel), it's dropped and the response is `lsp_types::error::RESPONSE_ERROR_REQUEST_CANCELLED`. If the query is already running, it completes; the client receives the late response (LSP spec permits this).
   - This model is sufficient for one Patrick + one editor; v0.2-M5 may revisit if multi-window editing or background analysis raises throughput needs (documented in `design/lsp.md` under "Concurrency model — thin slice").
2. Implement `initialize`: validate client `general.positionEncodings` capability, pick `utf-8` if present else `utf-16`, store the negotiated encoding in `ServerState`. Reply with `server_capabilities()` + the chosen encoding.
3. Implement `initialized`, `shutdown`, `exit` (boilerplate per LSP spec). `shutdown` flips a flag; `exit` calls `std::process::exit(0)` if shutdown was first; else exits with code 1.
4. Implement `textDocument/didOpen`: create a salsa `SourceFile` input with the document's URI-as-path and full text; register it in `CompilerDb`; store text in `open_documents`. Run `module_signatures_query` once to warm the cache (the result is dropped; diagnostics in Phase 3).
5. Implement `textDocument/didChange`: apply LSP `TextDocumentContentChangeEvent`s to the shadow text in `open_documents`, then write the result back to the salsa input via `source_file.text(&mut db).set(new_text)`. Salsa invalidates downstream queries.
6. Implement `textDocument/didClose`: remove from `open_documents`; the salsa input stays in `source_registry` (no harm) so re-opens are fast.
7. Implement `crates/ynz-lsp/src/position.rs`:
   - `byte_offset_to_position(text: &str, byte_offset: usize, encoding: PositionEncoding) -> Position`
   - `position_to_byte_offset(text: &str, position: Position, encoding: PositionEncoding) -> Option<usize>` (`None` if out of bounds)
   - Internal line-table cache (precomputed at didOpen/didChange) so converting N spans in the same document is O(log lines) per conversion
   - **Given/When/Then unit tests with explicit values** (no enumerated case lists without expected outputs — silent-wrong-output domain):
     - GIVEN `text="abc\nde"` with utf-8, WHEN `byte_offset_to_position(4)`, THEN `Position{line:1, character:0}` (newline at byte 3 ends line 0; `d` starts line 1 at character 0)
     - GIVEN `text="ab\r\ncd"` with utf-8, WHEN `byte_offset_to_position(4)`, THEN `Position{line:1, character:0}` (`\r\n` is byte 2-3; `c` is byte 4 = line 1 char 0)
     - GIVEN `text="a\r\nb"` with utf-8, WHEN `byte_offset_to_position(2)`, THEN `Position{line:0, character:2}` (offset 2 is `\n` — at the boundary; convention: stays on line 0 character 2, since `\n` ends line 0)
     - GIVEN `text="\u{FEFF}ab"` (BOM-prefixed) with utf-8, WHEN `byte_offset_to_position(3)`, THEN `Position{line:0, character:1}` for UTF-8 character count (after BOM); BOM is byte 0-2 (3 bytes) but counts as ONE character per LSP spec; documented invariant
     - GIVEN `text="✓✓"` (each `✓` is 3 UTF-8 bytes / 1 UTF-16 code unit / 1 Unicode scalar) with utf-8, WHEN `byte_offset_to_position(3)`, THEN `Position{line:0, character:3}` (utf-8 path counts bytes); same input with utf-16, THEN `Position{line:0, character:1}` (utf-16 path counts code units). **The divergence is the whole point of the encoding param** — both paths tested with explicit expectations.
     - GIVEN `text="a\u{1F600}b"` (emoji is 4 UTF-8 bytes / surrogate-pair = 2 UTF-16 code units / 1 Unicode scalar) with utf-8, WHEN `byte_offset_to_position(5)`, THEN `Position{line:0, character:5}`; same with utf-16, THEN `Position{line:0, character:3}` (1 char for `a` + 2 code units for surrogate-pair + cursor before `b`)
     - GIVEN any of the above texts, WHEN `position_to_byte_offset(byte_offset_to_position(N))`, THEN `Some(N)` (round-trip property — assert across 50+ random offsets via proptest IF the proptest crate is already a workspace dep; otherwise enumerated)
     - GIVEN empty text `""`, WHEN `byte_offset_to_position(0)`, THEN `Position{line:0, character:0}`; offset 1 → `None` from `position_to_byte_offset` reverse (out of bounds)
8. Test harness `crates/ynz-lsp/tests/harness/mod.rs`: in-process LSP service struct callable from tests without spawning a subprocess; one method per LSP request (`initialize(...)`, `did_open(...)`, `did_change(...)`, `expect_diagnostics(...)`, etc.). Uses `tokio::test` if framework is tower-lsp; sync calls if lsp-server.
9. Smoke test `tests/stdio_smoke.rs`: spawn `target/debug/ynz-lsp`, send minimal initialize JSON-RPC, parse the response, assert capabilities contain the expected fields, send shutdown+exit, assert clean exit.

**Acceptance criteria**:
- [x] `cargo run -p ynz-lsp` starts the server, doesn't crash on minimal `initialize`/`shutdown`/`exit` sequence
- [x] `tests/lifecycle.rs` passes: initialize → didOpen → didChange → didClose → shutdown with the test fixture
- [x] `tests/stdio_smoke.rs` passes: subprocess test exits cleanly
- [x] Position converter unit tests cover all 7 cases listed in Step 7 (LF/CRLF/mixed/BOM/emoji/surrogate/round-trip)
- [x] Salsa DB invalidation verified: didChange triggers re-parse on next query (test asserts cache miss after didChange, cache hit on second query)
- [x] `cargo test --workspace` passes (existing 830+ plus new ones)
- [x] Server advertises `positionEncodings: ["utf-8", "utf-16"]` in initialize response
- [x] No diagnostics published yet (Phase 3) — observable: `publishDiagnostics` never called

**Quality gate**:
- [x] No `// TODO` / `// FIXME` / `// HACK`
- [x] `cargo clippy -p ynz-lsp -- -D warnings` passes
- [x] All public functions in `position.rs` are tested
- [x] No `as any` equivalent: no `.unwrap()` on user input paths; user errors return LSP-protocol errors via the framework's normal mechanism
- [x] Documents the empty-bucket / empty-text edge case explicitly
- [x] No DB shared across threads incorrectly: ServerState owned by a single task

**Verification**:
- `cargo test -p ynz-lsp 2>&1 | grep 'test result'` — all tests pass
- `cargo run -p ynz-lsp --release -- --version 2>&1` (assuming we add a --version flag) — prints version, exits 0
- Manual test: `echo '{"jsonrpc":"2.0","id":1,"method":"initialize",...}' | ./target/debug/ynz-lsp` — produces a valid initialize response (smoke verifies this automatically)

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 2", prompt: "Review the diff for Phase 2. Diff: git diff main..HEAD. Focus areas: (a) DB threading model — is it sound? (b) position converter — does it correctly handle CRLF and multi-byte? (c) test harness — does it support extension to Phases 3-5? Output in standard format." })`
3. **Handle verdict.** BLOCK → fix → re-invoke. PASS → continue.
4. **Prompt user.** "Phase 2 done. LSP lifecycle wired to salsa. Ready to commit and move to Phase 3?"
5. **Do NOT start Phase 3** until user confirms.

---

### Phase 3: Inline diagnostics via publishDiagnostics

**PR scope**: Implement `textDocument/publishDiagnostics`. On `didOpen` and after every `didChange`, run `check_query`, collect the `DiagnosticBucket`, transform each Diagnostic to LSP `Diagnostic` shape (severity mapping, byte-span → Range conversion, WHAT/WHAT-INSTEAD/WHY packed into message), and push to the client. Clear diagnostics for fixed errors (push empty list).
**Branch**: `feat/v0-2-m2-lsp-diagnostics`
**Flag**: N/A
**Est. lines**: ~400 (diagnostic transform ~150, related-info handling ~50, integration tests ~150, jargon-audit extension ~50)
**Ships via**: `/pr`

**Objective**: First end-user-visible LSP feature. When Patrick opens a `.ynz` file with errors in VSCode (after Phase 6 ships the extension), red squiggles appear with the full teaching content.

**Why this phase exists**: Inline diagnostics are the highest-bang-for-buck LSP feature; they reuse the existing `ynz-diagnostics` rendering content with minimal new logic. Ships before autocomplete/hover because those are more complex and benefit from having the diagnostic-transform pattern established first.

**Current-state anchors**:
- `crates/ynz-diagnostics/src/diagnostic.rs:83-200` — `Diagnostic` struct + constructor (enforces WHAT/WHAT-INSTEAD/WHY)
- `crates/ynz-diagnostics/src/bucket.rs:15-90` — `DiagnosticBucket`
- `crates/ynz-diagnostics/src/render.rs:6-80` — Severity → ariadne mapping (LSP gets its own parallel mapping)
- `crates/ynz-typeck/src/queries.rs:100+` — `check_query` returns `Arc<CheckOutput>` with `.diagnostics: DiagnosticBucket`
- `crates/ynz-diagnostics/tests/jargon_audit.rs` — audits user-facing prose

**Files (expected scope)**:
- NEW: `crates/ynz-lsp/src/diagnostic_transform.rs` — `Diagnostic → lsp_types::Diagnostic` mapping
- EDIT: `crates/ynz-lsp/src/server.rs` — `did_open`/`did_change` handlers now run `check_query` and publish
- EDIT: `crates/ynz-lsp/src/state.rs` — track `last_published_diagnostics: HashMap<Url, Vec<lsp_types::Diagnostic>>` for clear-on-fix detection
- NEW: `crates/ynz-lsp/tests/diagnostics.rs` — integration tests using fixtures with intentional errors
- NEW: `crates/ynz-lsp/tests/fixtures/has_errors.ynz` — fixture with at least one of each Severity
- EDIT: `crates/ynz-diagnostics/tests/jargon_audit.rs` — extend audit to also walk `Diagnostic` messages as they would render through the LSP transform (not just CLI)

**Deviation rule**: Standard.

**Steps**:
1. Implement `diagnostic_transform::to_lsp_diagnostic(d: &ynz_diagnostics::Diagnostic, text: &str, encoding: PositionEncoding) -> lsp_types::Diagnostic`:
   - `range`: byte-span → LSP `Range` using `position::byte_offset_to_position` from Phase 2
   - `severity`: Error → DiagnosticSeverity::ERROR; Warning → WARNING; Suggestion → HINT
   - `message`: `format!("{}\n\nWHAT INSTEAD: {}\n\nWHY: {}", d.what, d.what_instead, d.why)` — verified against VSCode behavior; newlines render as soft breaks in squiggle tooltips
   - `source`: `Some("ynz".into())` — identifies Yinz as the source
   - `related_information`: if `d.related_spans` non-empty, populate with each (span, label) pair, converted to LSP `DiagnosticRelatedInformation`
   - `code` / `code_description`: NONE in thin slice (could become DiagnosticKind name in v0.2-M5)
   - `data`: NONE in thin slice. **Future-proofing decision**: if v0.2-M5 wants structured client-side rendering (separate UI for WHY vs WHAT-INSTEAD vs related spans), it adds a `data` field carrying the original three components as a JSON object. The plaintext `message` stays as the canonical fallback for clients that don't know about the `data` extension. This deferral is RECORDED in the Deferrals table.

   **Silent-wrong-output mitigation** (delimiter collision): the format above uses literal substrings `"\n\nWHAT INSTEAD: "` and `"\n\nWHY: "` as separators. If a registry diagnostic template body (`diagnostic_template_lookup(...).what` / `.what_instead_template` / `.why_template`) contains either substring, downstream parsing would mis-split. Mitigation has TWO layers:
   - **Audit assertion at registry-build time**: extend `crates/ynz-registry/tests/consistency.rs` with a test that walks every `DiagnosticTemplateEntry`'s three template fields AND every `BannedJargonEntry.reason`, `DeferredLanguageFeatureEntry.why` / `.substitute`, `BannedDeclarationKeywordEntry.what_instead` / `.why` (i.e., every field that can render into `d.what` / `d.what_instead` / `d.why`) and asserts NONE contains the literal substring `"WHAT INSTEAD:"` or the substring `"\n\nWHY:"` (case-insensitive). Fails CI if any does. (Today, none do — verified by inspection of `registry/features.toml`.)
   - **Runtime check in `to_lsp_diagnostic`**: `debug_assert!(!d.what.contains("\n\nWHAT INSTEAD:") && !d.what_instead.contains("\n\nWHY:"))` — catches dynamic-construction collisions that the static audit can't see. Debug-mode only; release builds skip the check for perf.
2. On `didOpen`/`didChange`, after the salsa input is updated, run `check_query` for the file's source; collect diagnostics from `CheckOutput.diagnostics`; transform each; call `publish_diagnostics(uri, transformed, version)`. Store the published list in `ServerState.last_published_diagnostics` to support empty-push clear-on-fix.
3. Cross-file diagnostics: `check_query` may produce diagnostics whose `SourceSpan.file` differs from the file that triggered the query (imported modules). Publish diagnostics PER FILE: group transformed diagnostics by their span's file, push to each file's URL.
4. `didClose` clears diagnostics for the closed URI (push empty list).
5. Extend `tests/jargon_audit.rs`: in addition to scanning user-facing diagnostic text fields, also build the LSP-rendered message (`format!(...)` above) and assert no banned-jargon appears. This is the LSP equivalent of the existing audit.
6. Integration test `tests/diagnostics.rs`: open `has_errors.ynz`, assert the published `Diagnostic` for each expected error has correct severity, correct range, message containing each of WHAT/WHAT-INSTEAD/WHY substrings, source="ynz". Then edit the file to fix one error, assert the corresponding diagnostic is removed from the next publish.
7. Position-encoding test: same `has_errors.ynz` opened twice — once with `utf-8` negotiated, once with `utf-16` negotiated — assert ranges are correctly computed for both (no off-by-one on multi-byte content).
8. **Cross-file clear-on-fix test** (adversarial, addresses stale-squiggle bug class): create fixture `tests/fixtures/cross_file/main.ynz` that imports a function from `tests/fixtures/cross_file/lib.ynz`. Open both. Introduce a type mismatch by editing `lib.ynz` so that `main.ynz`'s call-site type-checks but `lib.ynz` is internally broken. didOpen on both → assert correct diagnostics appear on both. Fix `lib.ynz`. Assert: `main.ynz`'s diagnostics list is unchanged in its NEXT publish (no spurious clear); `lib.ynz`'s diagnostics CLEAR in its publish. Then introduce an error in `lib.ynz` whose effect surfaces in `main.ynz` at the import site. Assert: `main.ynz` gets diagnostics published even though only `lib.ynz` was edited. Then fix `lib.ynz`. Assert: `main.ynz`'s stale diagnostic CLEARS in the next publish. This locks the cross-file invalidation behavior end-to-end.
9. **Concurrent didChange test** (adversarial, addresses race-during-query bug class): send two `didChange` notifications back-to-back with no intervening response opportunity (the test harness queues both before the worker drains). Assert final `publishDiagnostics` reflects the LAST didChange's text, not interleaved or stale state. Use the channel-serialized worker pattern from Phase 2 step 1.

**Acceptance criteria**:
- [x] `did_open` of a fixture with N errors publishes exactly N LSP diagnostics with correct severity, range, and full WHAT/WHAT-INSTEAD/WHY content in message
- [x] `did_change` that fixes one error publishes (N-1) diagnostics on the next push (empty entry removes the old one)
- [x] Cross-file errors publish to the correct URI (not the editing URI, unless the same)
- [x] Cross-file clear-on-fix: editing the source file clears stale diagnostics in the dependent file in the next publish (Step 8 test)
- [x] Concurrent didChange test passes: two back-to-back notifications yield diagnostics reflecting the LAST change, not interleaved state (Step 9 test)
- [x] `tests/jargon_audit.rs` extension passes: no banned-jargon appears in LSP-rendered messages
- [x] `crates/ynz-registry/tests/consistency.rs` extension passes: NO diagnostic-template field contains the substring `"WHAT INSTEAD:"` or `"\n\nWHY:"` (delimiter audit)
- [x] UTF-8 and UTF-16 position-encoding tests both pass for the same multi-byte fixture (including the 4-byte UTF-8 emoji / 2-code-unit UTF-16 surrogate case from Phase 2)
- [x] `did_close` clears diagnostics for the URI
- [x] `debug_assert!` runtime delimiter check exists in `to_lsp_diagnostic` (release builds skip; debug builds catch)
- [x] All existing 830+ tests pass; new tests added (≥8 LSP-specific including the cross-file and concurrent tests)

**Quality gate**:
- [x] No `// TODO` / `// FIXME` / `// HACK`
- [x] `cargo clippy -p ynz-lsp -- -D warnings` passes
- [x] Diagnostic transform handles empty bucket (no panic on zero-diagnostic publish)
- [x] Diagnostic transform handles diagnostics whose span is at byte-offset 0 (start of file) — common edge case
- [x] No allocation in hot path for the common-case "no diagnostics" return
- [x] Diagnostic.severity mapping covers ALL three ynz-diagnostics Severity variants (no panic on Suggestion)

**Verification**:
- `cargo test -p ynz-lsp diagnostics 2>&1 | grep 'test result'` — all pass
- `cargo test --workspace 2>&1 | grep 'test result'` — full suite green
- Manual LSP-client probe (using a simple client harness from Phase 2): open `has_errors.ynz`, observe published diagnostics in the test trace

**Exit Sequence:** Standard — persist, invoke code-reviewer with diff `git diff main..HEAD`, focus prompt on "are WHAT/WHAT-INSTEAD/WHY preserved end-to-end? does the transform round-trip the registry-driven deferred-feature messages from M1?", handle verdict, prompt user.

---

### Phase 4: Autocomplete via textDocument/completion (registry-driven)

**PR scope**: Implement `textDocument/completion`. Add `ynz_registry::lsp_completion_items(context)` adapter producing `lsp_types::CompletionItem`s from KEYWORDS, PRIMITIVE_INTRINSICS (filtered by receiver_type when `AfterDot { receiver_type: Some(...) }`), TYPE_ATTACHED_CONSTANTS, DEFERRED_LANGUAGE_FEATURES (marked Deprecated). LSP server computes context from cursor position + nearby text, calls the registry adapter, merges in user-defined symbols from `module_signatures_query`'s SignatureTable + ShapeTable, returns the union.
**Branch**: `feat/v0-2-m2-lsp-completion`
**Flag**: N/A
**Est. lines**: ~600 (registry adapter ~150, completion handler ~150, context detection ~100, user-symbol merge ~80, integration tests ~120)
**Ships via**: `/pr`

**Objective**: Second user-visible LSP feature. After typing `.` or whitespace, Patrick sees a popup with keywords, methods, deferred features marked deprecated — all sourced from the SSOT registry plus user-defined symbols from the open project.

**Why this phase exists**: Autocomplete is the highest-throughput "registry working as designed" demonstration. Every new keyword or deferred feature added in any future version appears here automatically — no `ynz-lsp` code change needed. This is the "drift class goes away by construction" promise from the roadmap, made concrete.

**Current-state anchors**:
- `crates/ynz-registry/src/lib.rs:11-93` — existing adapter functions
- `crates/ynz-typeck/src/signatures.rs` (SignatureTable), `shapes.rs` (ShapeTable) — user-defined symbols
- `crates/ynz-typeck/src/queries.rs:49-100` — `module_signatures_query`'s `SignatureOutput` carries both
- Yinz is non-OOP: `value.method()` is UFCS sugar for `method(value)`. After-dot completion needs to find STANDALONE functions whose first parameter matches the receiver type (typeck's UFCS resolution already does this; we expose it as a helper)

**Files (expected scope)**:
- EDIT: `crates/ynz-registry/src/lib.rs` — add `CompletionContext` enum, `lsp_completion_items(context)` adapter; expose `CompletionItemKind`/`CompletionItemTag` as re-exports OR mirror types if the registry shouldn't depend on `lsp-types` (decision below)
- EDIT: `crates/ynz-registry/Cargo.toml` — IF the registry adapter returns `lsp_types::CompletionItem` directly, add `lsp-types` as a dep; OTHERWISE (preferred) the registry returns a small registry-owned struct (`RegistryCompletionItem`) and the LSP translates to `lsp_types::CompletionItem`. Decision: keep `ynz-registry` lsp-types-free; LSP translates. Justification: the registry is a foundational crate; depending on lsp-types ties ALL consumers (CLI, future watch, future fmt) to LSP types they don't need.
- NEW: `crates/ynz-registry/src/lsp_adapter.rs` — `RegistryCompletionItem` + `CompletionContext` + `lsp_completion_items`
- NEW: `crates/ynz-lsp/src/completion.rs` — context detection + LSP translation + user-symbol merge
- EDIT: `crates/ynz-lsp/src/server.rs` — wire `textDocument/completion` handler
- NEW: `crates/ynz-lsp/tests/completion.rs` — integration tests
- NEW: `crates/ynz-lsp/tests/fixtures/completion_after_dot.ynz` — fixture exercising after-dot context
- NEW: `crates/ynz-lsp/tests/fixtures/completion_bare.ynz` — fixture exercising bare-identifier context
- NEW: `crates/ynz-registry/tests/lsp_adapter.rs` — registry-level unit tests for the adapter

**Deviation rule**: Standard.

**Steps**:
1. Define `crates/ynz-registry/src/lsp_adapter.rs`:
   - `enum CompletionContext { BareIdentifier, AfterDot { receiver_type: Option<&'static str> } }` — receiver_type is `None` when receiver is unresolved (still suggest UFCS candidates by best-effort)
   - `struct RegistryCompletionItem { label: String, kind: CompletionKind, detail: Option<String>, documentation: Option<String>, deprecated: bool, sort_priority: u8 }` — registry-owned mirror types
   - `enum CompletionKind { Keyword, PrimitiveMethod, FreeFn, TypeAttachedConstant, DeferredFeature, BannedKeyword }` — maps to LSP `CompletionItemKind` in the LSP-side translation
   - `pub fn lsp_completion_items(context: CompletionContext) -> Vec<RegistryCompletionItem>` — walks the appropriate registry arrays based on context
2. Context-detection in `crates/ynz-lsp/src/completion.rs`:
   - On `textDocument/completion`, get the cursor position from the request, convert to byte offset using `position::position_to_byte_offset`, look at the byte just before the offset.
   - If `b'.'`: walk left skipping whitespace from the position BEFORE the `.`. Inspect the previous non-whitespace char:
     - If ASCII digit (`b'0'..=b'9'`) AND the char before that digit is NOT an identifier-continue char (letter, underscore, digit) → this is a numeric literal's decimal point (e.g. `5.<cursor>`). Return NO completions (empty list / `null`).
     - Else → walk left through identifier chars (letter/underscore/digit) to capture the receiver token's name; ask typeck for the inferred type of that token (via `module_signatures_query` + a new public `type_of_expression_at_offset(db, source, offset) -> Option<Type>` helper added to `ynz-typeck` IF not already exposed; else use the most accessible adjacent API). If type resolution fails, return `AfterDot { receiver_type: None }`.
   - Otherwise → `BareIdentifier`.
3. User-symbol merge:
   - For `BareIdentifier`: walk `SignatureOutput.sig_table` for functions in scope, `shape_table` for shapes, top-level `const`/`let` bindings; create `lsp_types::CompletionItem`s with `kind = Function`/`Class`/`Variable`.
   - For `AfterDot { receiver_type: Some(t) }`: also include standalone functions whose first parameter type matches `t` (UFCS candidates).
4. LSP-side translation: `to_lsp_completion_item(rci: RegistryCompletionItem) -> lsp_types::CompletionItem`:
   - `label = rci.label`
   - `kind = match rci.kind { Keyword → Keyword, PrimitiveMethod → Method, ... }`
   - `detail = rci.detail` (e.g. "int.max — i64 maximum value")
   - `documentation = rci.documentation.map(|d| Documentation::MarkupContent(MarkupContent { kind: Markdown, value: d }))` — markdown WHY content
   - `tags = if rci.deprecated { vec![CompletionItemTag::DEPRECATED] } else { vec![] }`
   - `sort_text = format!("{:03}_{}", rci.sort_priority, rci.label)` — priority-ordered: user symbols (0xx) < keywords (1xx) < intrinsics (2xx) < deferred-features (9xx) (deprecated last)
5. Wire `completionProvider: { triggerCharacters: [".", " "], resolveProvider: false }` in capabilities (already advertised in Phase 2; now actually implemented).
6. Registry-side tests in `crates/ynz-registry/tests/lsp_adapter.rs`:
   - `BareIdentifier` returns at least all 29 keywords
   - `AfterDot { receiver_type: Some("int") }` returns all `int`-receiver intrinsics
   - `AfterDot { receiver_type: Some("nonexistent") }` returns only UFCS-candidate-empty result (registry adapter returns empty for unknown receiver; LSP merges any user-defined functions)
   - Deferred features appear in `BareIdentifier` results with `deprecated = true`
7. LSP-side integration tests in `crates/ynz-lsp/tests/completion.rs`:
   - `did_open` `completion_after_dot.ynz` with cursor positioned after `score.`, request completion: assert primitive int methods appear (`toString`, `toFloat`, `toNumber`, `wrappingAdd`, etc.) and deferred-feature entries DO NOT appear (they're not int methods)
   - `did_open` `completion_bare.ynz` with cursor at bare position: assert keywords (`function`, `shape`, `if`, `let`, etc.) appear, deferred-features (`gpu`, `test`, etc.) appear with deprecated tag
   - Numeric-literal disambiguation: fixture with `let x = 5.<cursor>` → assert completion is empty (NOT primitive method list). Fixture with `let x = score.<cursor>` where `score: int` → assert primitive int methods appear. Fixture with `let x = a5.<cursor>` (identifier `a5`) → assert UFCS/method completions appear (the previous char is digit but the char BEFORE THE DIGIT is `a` which is identifier-continue — so this IS a receiver, not a numeric literal).
   - Position-encoding parity: same fixture, UTF-8 and UTF-16 encodings, same completion results

**Acceptance criteria**:
- [ ] `BareIdentifier` completion returns all 29 keywords + 17 banned-declaration-keywords (as deprecated/won't-fix) + 15 deferred-features (as deprecated) + user-defined symbols
- [ ] `AfterDot { receiver_type: Some("int") }` returns all `int`-receiver intrinsics from registry + user-defined functions taking `int` as first param (UFCS)
- [ ] Deferred features show `CompletionItemTag::Deprecated`
- [ ] `documentation` field renders markdown WHY content for items that have it
- [ ] Sort order: user-defined symbols first, keywords next, intrinsics, deferred last
- [ ] LSP `completionProvider.triggerCharacters` is `[".", " "]` in capabilities
- [ ] Registry adapter does NOT depend on `lsp-types` (verified by `cargo tree -p ynz-registry`)
- [ ] All existing tests pass; new tests added (≥4 registry-level, ≥3 LSP-level)
- [ ] Patrick can add a new entry to `registry/features.toml`, rebuild ynz-lsp, restart server, and the new entry appears in completion — manually verified or covered by a registry-rebuild integration test

**Quality gate**:
- [ ] No `// TODO` / `// FIXME` / `// HACK`
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] No hardcoded keyword/intrinsic/deferred-feature lists in `ynz-lsp` (grep-check enforced; documented in Quality gate)
- [ ] No `.unwrap()` on user-input paths
- [ ] Context detection handles edge cases: cursor at byte 0, cursor right after a unicode char, cursor inside a string literal (returns empty completion or BareIdentifier — pick one and test)

**Verification**:
- `cargo test -p ynz-registry lsp_adapter` — registry tests pass
- `cargo test -p ynz-lsp completion` — LSP tests pass
- `cargo test --workspace` — full suite green
- `grep -rn "Tok::\|primitive_intrinsics\b\|deferred" crates/ynz-lsp/src/completion.rs` — should show registry adapter calls, NOT hardcoded lists

**Exit Sequence:** Standard. Code-reviewer focus prompt: "verify no hardcoded keyword/intrinsic/deferred-feature lists in `ynz-lsp`. Confirm registry adapter is lsp-types-free. Confirm UFCS candidates appear in after-dot completion per Yinz's non-OOP model."

---

### Phase 5: Hover via textDocument/hover (registry-driven)

**PR scope**: Implement `textDocument/hover`. Find the token at the cursor's byte offset; ask registry `lsp_hover_for_token(name)` first (covers keywords, primitive methods, type-attached constants, deferred features, banned-keywords); fall back to typeck symbol resolution for user-defined functions/shapes/variables. Render Markdown with signature + WHY content.
**Branch**: `feat/v0-2-m2-lsp-hover`
**Flag**: N/A
**Est. lines**: ~400 (registry adapter ~100, token resolver ~100, LSP handler ~80, tests ~120)
**Ships via**: `/pr`

**Objective**: Third user-visible LSP feature. Hovering over any keyword, intrinsic, type, or deferred feature shows the registry's `why` content in a markdown popup.

**Why this phase exists**: Hover is the surface where Patrick can validate that EVERY registry entry has good teaching content. If a hover is empty or unhelpful, the registry's `why` field is the problem — the LSP reads what the registry has. This is the surface that exposes "Yinz teaches users" as a measurable claim.

**Current-state anchors**:
- `crates/ynz-registry/src/lib.rs` adapter API
- `crates/ynz-parser/src/queries.rs` `parse_query` — provides AST with spans; use to find the node at a byte offset
- `crates/ynz-typeck/src/queries.rs` `module_signatures_query` + `check_query` — provide typed symbols and their declarations

**Files (expected scope)**:
- EDIT: `crates/ynz-registry/src/lsp_adapter.rs` — add `HoverContent { markdown_body: String, kind: HoverKind }` and `lsp_hover_for_token(name: &str) -> Option<HoverContent>` covering all 9 entry kinds (with appropriate fallthroughs)
- NEW: `crates/ynz-lsp/src/hover.rs` — token resolution at byte offset (AST-first, lex-fallback), LSP request handler, markdown rendering
- EDIT: `crates/ynz-lsp/src/server.rs` — wire `textDocument/hover` handler
- NEW: `crates/ynz-lsp/tests/hover.rs` — integration tests
- NEW: `crates/ynz-lsp/tests/fixtures/hover_targets.ynz` — fixture with positions for each entry kind

**Deviation rule**: Standard.

**Steps**:
1. Extend `ynz-registry/src/lsp_adapter.rs`:
   - `HoverContent { markdown_body: String, kind: HoverKind }`
   - `enum HoverKind { Keyword, PrimitiveMethod, FreeFn, TypeAttachedConstant, DeferredFeature, BannedDeclarationKeyword, BannedJargon, DiagnosticTemplate, MutedHintDomain }`
   - `pub fn lsp_hover_for_token(name: &str) -> Option<HoverContent>`:
     - First lookup KEYWORDS by name → produce `## Keyword: <name>\n\nIntroduced in <since>.` (keywords have no WHY field today; future M2 follow-up could enrich)
     - Then PRIMITIVE_INTRINSICS by name (any overload) → produce `## Primitive intrinsic: <name>\n\nReceiver: <type>\nReturns: <return>\nIntroduced in <since>.`
     - Then TYPE_ATTACHED_CONSTANTS by name (extract `type.name` from token if it has a dot) → `## <type>.<name>\n\nValue: <value_literal>\n\nIntroduced in <since>.`
     - Then DEFERRED_LANGUAGE_FEATURES by name → `## Deferred feature: <name>\n\n**Ships in:** <ships_in>\n\n**Substitute:** <substitute>\n\n**Why deferred:** <why>\n\n[Design doc](<design_doc>)`
     - Then BANNED_DECLARATION_KEYWORDS / BANNED_JARGON → `## Not a Yinz term: <name>\n\n**What to use instead:** <what_instead>/<replacement>\n\n**Why:** <why>/<reason>`
     - Returns `None` if not registered
2. Token resolution `crates/ynz-lsp/src/hover.rs::token_at_offset(...)`:
   - Use `parse_query` to get the AST; walk top-down finding the smallest node whose span contains the offset
   - Extract the token text from the source string using the node's span
   - If AST has no node at offset (e.g., inside whitespace or comment) → return `None`
   - If the node IS a keyword/identifier token → return the text and its span
3. LSP `textDocument/hover` handler:
   - Get position from request → convert to byte offset → call `token_at_offset(...)`
   - If token resolved → call `lsp_hover_for_token(token_name)` → if `Some(content)` return LSP `Hover { contents: HoverContents::Markup(MarkupContent { kind: Markdown, value: content.markdown_body }), range: Some(token_span_as_lsp_range) }`
   - If registry lookup misses → ask typeck for user-defined symbol info (function signature with WHY from doc comment, shape definition, variable type) — render as markdown
   - If both miss → return `None` (LSP spec: no hover for this position)
4. Doc-comment integration (light): if the user-defined symbol has a leading doc comment (M8 P3 work — verify it's accessible via the AST/typeck output), include it in the markdown body. If doc comments are NOT accessible yet from the AST/typeck output, skip the integration AND add an entry to `.claude/todos.md` "Soon" section BEFORE the Phase 5 PR merges (entry: `lsp-hover-doc-comments: wire doc-comment body from AST/typeck into ynz-lsp hover markdown body; deferred from v0.2-M2 Phase 5 because <name the accessibility gap that blocked>`). This guarantees the deferral is tracked at the moment it's made, not later — per the Patrick-universal "deferrals-must-be-tracked" feedback rule.
5. Integration tests `tests/hover.rs`:
   - Position over `function` keyword → hover shows "Keyword: function"
   - Position over `int.max` → hover shows "int.max ... Value: 9223372036854775807"
   - Position over `gpu` (deferred feature) → hover shows "Deferred feature: gpu ... Ships in: v2+ ... Substitute: ..."
   - Position over `type` (banned-declaration-keyword) → hover shows "Not a Yinz term: type ... Use shape instead"
   - Position over a user-defined function name → hover shows the function signature (from typeck)
   - Position inside a comment → hover returns None
   - Position at byte 0 of an empty file → hover returns None (no crash)

**Acceptance criteria**:
- [ ] Hover over any KEYWORDS entry returns a populated hover
- [ ] Hover over any PRIMITIVE_INTRINSICS entry returns hover with receiver type + return type
- [ ] Hover over any TYPE_ATTACHED_CONSTANTS entry returns hover with the value literal
- [ ] Hover over any DEFERRED_LANGUAGE_FEATURES entry returns hover with ships_in/substitute/why and design_doc link
- [ ] Hover over BANNED_DECLARATION_KEYWORDS / BANNED_JARGON returns hover with what_instead + why
- [ ] Hover over a user-defined function name returns hover with the function signature
- [ ] Hover at no-token position returns None (no crash)
- [ ] All existing tests pass; new hover tests ≥7

**Quality gate**:
- [ ] No `// TODO` / `// FIXME` / `// HACK`
- [ ] No hardcoded entry-text strings in `ynz-lsp/src/hover.rs` — all content comes via `lsp_hover_for_token`
- [ ] Markdown rendering escapes `<`, `>`, `&` correctly if registry data contains them (audit + test with a deliberately-tricky registry entry)
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] No `.unwrap()` on user-input paths

**Verification**:
- `cargo test -p ynz-lsp hover` — all hover tests pass
- `cargo test --workspace` — full suite green
- Manual: a small test script does an LSP hover at byte offset of `gpu` in a test file, asserts the response body contains "v2+" and the design-doc URL

**Exit Sequence:** Standard. Code-reviewer focus prompt: "confirm hover content is registry-sourced for every entry kind; confirm no hardcoded teaching text in ynz-lsp source. Audit for markdown injection risk if a registry entry contains backticks or angle brackets."

---

### Phase 6: TextMate grammar generator + VSCode extension scaffold

**PR scope**: Build `crates/ynz-tmgrammar` binary that reads `ynz-registry` and emits `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` (committed to repo). Scaffold the VSCode extension under `tooling/vscode-ynz/`: `package.json`, `extension.ts`, `tsconfig.json`, `language-configuration.json`, README, build/install instructions. Extension spawns local `ynz-lsp` binary and registers `.ynz` language. NO marketplace publish yet (Phase 7).
**Branch**: `feat/v0-2-m2-vscode-extension-scaffold`
**Flag**: N/A
**Est. lines**: ~700 (tmgrammar generator + tests ~250, extension.ts + package.json + configs ~300, README + screenshots dir ~50, generated grammar checked in ~100)
**Ships via**: `/pr`

**Objective**: Patrick can clone the repo, build the LSP, install the extension locally via `code --install-extension`, and edit `.ynz` files in VSCode with syntax highlighting + LSP features wired in.

**Why this phase exists**: Until an extension exists, the LSP has no client; the M2 deliverables are invisible. Without registry-derived grammar, adding a keyword in v0.5+ would require manual edits to the grammar file. The two are bundled because they're tightly coupled — the extension references the grammar artifact, and an isolated grammar PR would have nothing using it.

**Current-state anchors**:
- `crates/ynz-registry/src/lib.rs` `keywords()` adapter
- No `tooling/vscode-ynz/` yet (Phase 0 created `tooling/` and `tooling/README.md`)
- VSCode extension standard files: `package.json` (extension manifest), `extension.ts` (activate hook), `language-configuration.json` (brackets, comments, indentation), `syntaxes/*.tmLanguage.json` (TextMate grammar)

**Files (expected scope)**:
- NEW: `crates/ynz-tmgrammar/Cargo.toml` — adds `ynz-registry`, `serde`, `serde_json` deps
- NEW: `crates/ynz-tmgrammar/src/main.rs` — binary: reads registry, writes `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json`
- NEW: `crates/ynz-tmgrammar/src/grammar.rs` — `build_grammar() -> serde_json::Value` (returns the grammar; main binary writes to file)
- NEW: `crates/ynz-tmgrammar/tests/grammar_snapshot.rs` — re-runs `build_grammar()`, compares to checked-in `ynz.tmLanguage.json` byte-for-byte, fails if drift
- NEW: `tooling/vscode-ynz/package.json` — extension manifest (name, version, publisher placeholder, activation events, contributes section)
- NEW: `tooling/vscode-ynz/extension.ts` — activate function spawns `ynz-lsp` binary, sets up `vscode-languageclient`
- NEW: `tooling/vscode-ynz/tsconfig.json`
- NEW: `tooling/vscode-ynz/language-configuration.json` — brackets, comments (`//`, `/* */`), auto-closing pairs
- NEW: `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` — generated, committed
- NEW: `tooling/vscode-ynz/README.md` — install instructions (build LSP, package extension, install via `code --install-extension`)
- NEW: `tooling/vscode-ynz/.vscodeignore` — exclude tsconfig, src/, etc. from packaged extension
- NEW: `tooling/vscode-ynz/screenshots/.gitkeep` (placeholder; Phase 7 adds real screenshots)
- EDIT: root `Cargo.toml` — add `crates/ynz-tmgrammar` to workspace
- EDIT: `tooling/README.md` (from Phase 0) — describe the now-populated vscode-ynz subdir + the build flow
- EDIT: `.gitignore` — `tooling/vscode-ynz/node_modules/`, `tooling/vscode-ynz/out/`, `tooling/vscode-ynz/*.vsix`

**Deviation rule**: Standard.

**Steps**:
1. Build `crates/ynz-tmgrammar`:
   - `grammar.rs::build_grammar()` returns a serde_json `Object` with TextMate grammar shape: `{ name: "ynz", scopeName: "source.ynz", patterns: [...] }`
   - Patterns: one `match` rule per category — keywords (regular), banned-declaration-keywords (highlighted as `invalid.deprecated`), deferred-features (`invalid.illegal`), literals (booleans, numbers, strings), comments (`//`, `/* */`)
   - Keyword regex: `\\b(function|let|const|...)\\b` — list assembled from `ynz_registry::keywords().map(|e| e.name)`
   - Banned-declaration-keywords: `\\b(type|struct|class|...)\\b` — from `ynz_registry::banned_declaration_keywords()`
   - Deferred-language-features: `\\b(gpu|foreign|test|f32|...)\\b` — from `ynz_registry::deferred_language_features()`
   - `main.rs` writes the result to `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` with `serde_json::to_string_pretty` (stable formatting for diffability)
2. Snapshot test in `tests/grammar_snapshot.rs`:
   - Re-runs `build_grammar()` → produces a `serde_json::Value`
   - Reads `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` from disk → parses to `serde_json::Value`
   - **Canonicalizes both** before compare: sort object keys recursively, use the SAME serializer in both paths. This avoids flake from `serde_json::to_string_pretty` formatting differences across versions. Compare `Value` equality, NOT byte-for-byte string equality.
   - Pin `serde_json` to an exact version in `crates/ynz-tmgrammar/Cargo.toml` (or use workspace `serde_json = { workspace = true }` with the workspace version pinned) — defense in depth against version drift.
   - Fails with: "ynz.tmLanguage.json drifted from registry. Re-run `cargo run -p ynz-tmgrammar` and commit the updated file."
3. Scaffold `tooling/vscode-ynz/`:
   - `package.json`:
     - `name: "yinz"`, `displayName: "Yinz Language"`, `version: "0.2.0-m2"`, `publisher: "<placeholder-set-in-phase-7>"`
     - `activationEvents: ["onLanguage:ynz"]`
     - `contributes`: `languages` (id=ynz, extensions=[".ynz"], configuration="./language-configuration.json"), `grammars` (scopeName=source.ynz, path="./syntaxes/ynz.tmLanguage.json")
     - `configuration`: `yinz.server.path` (string, default `ynz-lsp`)
     - `main: "./out/extension.js"`
     - `engines.vscode: "^1.85.0"` (reasonable floor; VSCode 1.85 = Nov 2023)
     - `categories: ["Programming Languages"]`
     - `preview: true`
   - `extension.ts`:
     - `activate(context)`: spawn `ynz-lsp` binary using `vscode-languageclient.LanguageClient`, configure with stdio transport, register for `ynz` language
     - `deactivate()`: stop the language client
   - `language-configuration.json`: brackets `()`, `[]`, `{}`; line-comment `//`; block-comment `/* */`; auto-closing brackets and quotes
   - `tsconfig.json`: target ES2020, module Node16, strict, declaration: false, outDir: "./out"
   - `README.md`: explains install flow (build `ynz-lsp` via `cargo build -p ynz-lsp`, copy/symlink to PATH or set `yinz.server.path`, package extension via `npm install && npx vsce package`, install via `code --install-extension yinz-0.2.0-m2.vsix`)
4. Regenerate grammar: `cargo run -p ynz-tmgrammar`. Commit the output.
5. End-to-end manual verification: install the extension locally, open `examples/basics/src/entrypoint.ynz` in VSCode, see syntax highlighting + LSP-driven diagnostics + autocomplete + hover.
6. Document the manual verification in PR description (since CI can't run VSCode UI tests).

**Acceptance criteria**:
- [ ] `cargo run -p ynz-tmgrammar` regenerates `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` from registry
- [ ] `cargo test -p ynz-tmgrammar` snapshot test passes (committed file matches generator output)
- [ ] CI fails if a keyword is added to registry and the grammar isn't regenerated
- [ ] `npm install && npx vsce package` in `tooling/vscode-ynz/` produces a valid `.vsix`
- [ ] Manually installed extension: opens `.ynz` files, highlights keywords, deprecated visual on banned-declaration-keywords, illegal visual on deferred-features
- [ ] Extension launches LSP and shows diagnostics/autocomplete/hover from Phases 3-5
- [ ] `tooling/vscode-ynz/README.md` install instructions work end-to-end (followed manually in PR review)
- [ ] No grammar drift introduced manually — file is generator output only

**Quality gate**:
- [ ] No `// TODO` / `// FIXME` / `// HACK`
- [ ] No hardcoded keyword/banned/deferred lists in `crates/ynz-tmgrammar/src/` — all sourced from `ynz-registry`
- [ ] Grammar is valid TextMate JSON (parseable; passes a small structural validation in the snapshot test)
- [ ] Generated grammar uses simple Oniguruma-compatible regex only (no lookbehind / nested captures — verified by the snapshot test or a small parse-check)
- [ ] `package.json` has no obvious typos in extension manifest fields (validated by `npx vsce ls` or similar in the test pipeline if installed; otherwise reviewer-verified)
- [ ] No `node_modules/` committed (`.gitignore` checked)
- [ ] No `.vsix` checked in (`.gitignore` checked)

**Verification**:
- `cargo test -p ynz-tmgrammar` — snapshot passes
- `cargo run -p ynz-tmgrammar && git diff --exit-code tooling/vscode-ynz/syntaxes/` — empty diff (grammar in sync)
- `cd tooling/vscode-ynz && npm install && npx vsce package --no-yarn` — produces a .vsix
- Manual: install extension, open `examples/basics/src/entrypoint.ynz`, observe LSP features working

**Exit Sequence:** Standard. Code-reviewer focus prompt: "is the grammar generator the ONLY source of `ynz.tmLanguage.json`? Is the snapshot test sufficient to catch drift? Does the VSCode extension's `extension.ts` correctly handle the case where the `ynz-lsp` binary is missing from PATH (does it surface a useful error to the user)?"

---

### Phase 7: VSCode marketplace publish (preview) OR .vsix fallback

**PR scope**: Patrick registers the VSCode publisher account, configures `package.json` publisher field, runs `vsce publish --pre-release`. If marketplace verification stalls or proves friction-heavy (>30 min cumulative), abort to `.vsix` fallback: tag a GitHub Release named `ynz-vscode-v0.2.0-m2` with the `.vsix` attached, document install in `tooling/vscode-ynz/README.md`. Marketplace publish becomes a follow-up; .vsix ship is the M2 commit. Either path produces user-visible distribution.
**Branch**: `chore/v0-2-m2-vscode-publish`
**Flag**: N/A
**Est. lines**: ~150 (publisher manifest updates, README install-from-marketplace section, screenshots dir population)
**Ships via**: `/pr`

**Objective**: User-visible distribution channel. Patrick (and any tester) can install with one click (marketplace) or one command (.vsix download + install).

**Why this phase exists**: Without a distribution channel, the extension is internal-only. Roadmap explicitly lists marketplace-as-preview as in-scope; risk row captures the friction-fallback path.

**Current-state anchors**:
- `tooling/vscode-ynz/package.json` from Phase 6 (publisher = placeholder)
- `tooling/vscode-ynz/README.md` from Phase 6 (local-install only)

**Files (expected scope)**:
- EDIT: `tooling/vscode-ynz/package.json` — set publisher = "<patrick-or-yinz-publisher-id>", confirm version/preview flags
- EDIT: `tooling/vscode-ynz/README.md` — add marketplace-install section (or `.vsix`-install section if fallback path)
- NEW: `tooling/vscode-ynz/screenshots/hover.png`, `autocomplete.png`, `diagnostic.png` (3 minimal screenshots)
- EDIT: `tooling/vscode-ynz/screenshots/.gitkeep` (delete; replaced by real screenshots)
- NEW (if marketplace path): `tooling/vscode-ynz/CHANGELOG.md` — marketplace requires; starts at v0.2.0-m2
- NEW (if fallback path): `.github/workflows/vscode-vsix-release.yml` — optional GitHub Actions job that builds and uploads .vsix on release tag (defer if friction)

**Deviation rule**: Standard.

**Steps**:
1. Patrick registers a VSCode marketplace publisher (Azure DevOps Personal Access Token + publisher creation via https://aka.ms/vscodepublish).
2. Set `package.json.publisher = "<id>"`. Add CHANGELOG.md.
3. Capture screenshots (3 small PNGs of the extension in action) and place in `tooling/vscode-ynz/screenshots/`. Reference from README and `package.json.icon` (or `galleryBanner`).
4. Run `vsce publish --pre-release` from `tooling/vscode-ynz/`. Marketplace propagation typically takes <5 min. Verify by opening VSCode → Extensions → search "yinz" → preview-tagged result appears.

**Fallback-trigger criteria (OBJECTIVE — switch to step 5 if ANY apply, not subjective "felt like a hassle")**:
- Azure DevOps publisher-account verification email NOT received within 24 hours after Patrick submits the form, OR
- `vsce publish --pre-release` returns an authentication error after 3 distinct PAT generations (suggests Azure-side config issue, not a typo), OR
- Marketplace requires SSO / enterprise Azure tenant / publisher-level review that Patrick can't single-handedly resolve in one session, OR
- Marketplace propagation has NOT completed within 24 hours after a successful `vsce publish` exits 0 (suggests stuck review queue).

If none of the above triggered, Phase 7 ships marketplace publish (Step 4 succeeded). If any triggered, fork to Step 5.

5. **Fallback path** (triggered per the criteria above):
   - Run `vsce package` to build the `.vsix` locally
   - Create a GitHub Release on the ynz repo named `ynz-vscode-v0.2.0-m2`
   - Attach the `.vsix` as a release asset
   - Update `tooling/vscode-ynz/README.md` install section: "Download the latest .vsix from <release URL>, then run `code --install-extension yinz-0.2.0-m2.vsix`"
   - Add an entry to `.claude/todos.md` "Soon" section: `marketplace-publish-followup: register VSCode publisher and run vsce publish --pre-release; objectively-triggered fallback fired during v0.2-M2 Phase 7; original blocker: <name the trigger>`
6. Either path: update `tooling/vscode-ynz/README.md` so install instructions match the chosen path. Remove the local-install-only language from Phase 6; the marketplace-or-vsix is the primary path.
7. Update root `README.md` (if it exists, else `CLAUDE.md` Project Layout) with a one-liner pointing at "Yinz VSCode extension: <marketplace URL or release URL>".
8. **Token-leak audit (mandatory regardless of path)**: run `git log -p <plan-base-commit>..HEAD | grep -E '([a-zA-Z0-9]{52}|[a-zA-Z0-9]{84}|ghp_[a-zA-Z0-9]{36}|pat_[a-zA-Z0-9]+)'` — Azure DevOps PATs are 52 chars, GitHub classic tokens are 40 chars (`ghp_` prefix), GitHub fine-grained tokens are 84 chars (`pat_` prefix). Empty result = no token leaked across the milestone. Document this audit run in the PR description with the exact command + the empty result. If non-empty result, STOP — rotate the leaked token, force-rewrite history (consult Patrick), do NOT merge.

**Acceptance criteria**:
- [ ] Either: extension installable via `code --install-extension yinz` (marketplace path), OR extension installable via downloaded `.vsix` from a GitHub release
- [ ] Install instructions in `tooling/vscode-ynz/README.md` work end-to-end (verified manually)
- [ ] Three screenshots committed in `tooling/vscode-ynz/screenshots/`
- [ ] `package.json` has a real publisher value (not placeholder)
- [ ] CHANGELOG.md exists with v0.2.0-m2 entry (if marketplace path)
- [ ] Patrick can install on a fresh VSCode and open `examples/basics/src/entrypoint.ynz`, see LSP features working
- [ ] If fallback path: `.claude/todos.md` updated with marketplace-publish-followup item; PR description names the OBJECTIVE trigger that fired (from the four enumerated in Step 4)
- [ ] **Token-leak audit (Step 8)**: PR description includes the exact `git log -p | grep` command run + the empty result confirming no PAT/token leaked

**Quality gate**:
- [ ] No `// TODO` / `// FIXME` / `// HACK`
- [ ] No publisher tokens or PATs committed — verified by Step 8 audit, NOT by inspection
- [ ] Screenshots are < 500KB each (no bloated PNGs)
- [ ] Install instructions read sensibly to a first-time user
- [ ] If fallback path is taken, the path-taken decision is documented in the PR description with the OBJECTIVE trigger name (not "felt like a hassle")

**Verification**:
- Manual install verification on a fresh VSCode profile
- `gh release list` (if fallback path) — release exists with .vsix attached
- `code --list-extensions | grep -i yinz` (after install) — extension listed

**Exit Sequence:** Standard. Code-reviewer focus prompt: "verify no secrets committed. Verify README install path matches the actual distribution channel chosen. If fallback path: confirm todos.md has the marketplace-follow-up entry."

---

### Phase 8: Integration-test sweep + LSP smoke fixture

**PR scope**: Comprehensive end-to-end integration tests using the in-process harness from Phase 2: open every `examples/basics` and `examples/errors` fixture, send each LSP request type at multiple positions, assert responses. One additional subprocess-spawn smoke test covers the full stdio wire format from a fresh process. Verify no regressions, no flake.
**Branch**: `test/v0-2-m2-lsp-integration-sweep`
**Flag**: N/A
**Est. lines**: ~500 (integration test cases ~350, fixtures ~80, regression-check helpers ~70)
**Ships via**: `/pr`

**Objective**: Lock the LSP's behavior against a known set of inputs so v0.2-M3+ can refactor with confidence.

**Why this phase exists**: Phases 3-5 each added scoped tests; Phase 8 stitches them into a comprehensive sweep that runs every LSP feature against every example. The sweep is the contract subsequent milestones (M3 fmt, M4 watch, M5 LSP-full) refactor against.

**Current-state anchors**:
- `examples/basics/src/entrypoint.ynz` — Yinz language demo, all v0.1 features
- `examples/errors/*.ynz` — error gallery per milestone (m1 through m8, plus v0_2_m1, plus v0_2_m2 added later in Phase 9)
- `crates/ynz-lsp/tests/harness/mod.rs` from Phase 2 — in-process LSP client

**Files (expected scope)**:
- NEW: `crates/ynz-lsp/tests/integration_sweep.rs` — opens each example fixture, exercises each LSP request type
- NEW: `crates/ynz-lsp/tests/regression.rs` — opens a fixture-with-no-errors, asserts no diagnostics; opens a fixture-with-errors, asserts exact error count and content
- NEW: `crates/ynz-lsp/tests/fixtures/no_errors.ynz`
- EDIT: `crates/ynz-lsp/tests/stdio_smoke.rs` (from Phase 2) — extend with one full sequence: initialize → didOpen → completion → hover → didClose → shutdown
- NEW: `crates/ynz-lsp/tests/performance.rs` — timing assertions per the Performance invariants (cold initialize <500ms, keystroke <100ms p95, completion <50ms p95, hover <50ms p95). Marked `#[ignore]`-able if CI is too noisy; runs manually `cargo test --release -p ynz-lsp -- --ignored performance`

**Deviation rule**: Standard.

**Steps**:
1. Integration sweep iterates over a hardcoded list of fixtures (every file under `examples/basics/src/` and `examples/errors/`):
   - For each: didOpen → assert diagnostics count matches expectation (zero or "many")
   - For each: cursor at start, mid, end → request completion → assert at least keywords appear
   - For each: cursor at known token positions → request hover → assert markdown body is non-empty for registry-known tokens
2. Regression test: maintain a hand-curated `no_errors.ynz` and `m4_classic_errors.ynz` (the latter from an existing example); assert exact diagnostic counts and that one well-known error message renders correctly via the LSP (verifies end-to-end teaching content)
3. Stdio smoke extension: full lifecycle PLUS at least one completion + hover request over the wire, asserting the response JSON is well-formed
4. Performance tests: open a 100-line fixture, time the initial diagnostic response (assert <500ms); apply a single-char didChange, time the next diagnostic (assert <100ms); request completion (assert <50ms); request hover (assert <50ms). Use `std::time::Instant`. Run in release mode for tight timings.
5. Run the full workspace test suite to ensure no regressions: `cargo test --workspace`
6. Run all `examples/basics/src/entrypoint.ynz` and `examples/errors/*.ynz` through `./target/debug/ynz run` (where applicable) and `./target/debug/ynz build` — assert CLI output is byte-identical to a pre-M2 baseline captured at the start of this milestone
7. Audit pass: `grep -rn "as any\|unwrap()\|TODO\|FIXME" crates/ynz-lsp/` (manual check; sanity)
8. **LSP-vs-CLI divergence check**: for every fixture in `examples/errors/*.ynz`, run BOTH the CLI (`./target/debug/ynz build <fixture>`) and the LSP (in-process harness — open the fixture, capture published diagnostics). Assert the count of distinct (file, span, what) tuples MATCHES between the two paths. Log any divergence with the fixture name + diff. Acceptance: zero divergence across the gallery. This catches the "LSP renders different diagnostics than CLI" failure mode mentioned in Risk Row #9 ("LSP exposes existing compiler bugs") — making bug surfacing visible rather than hidden.

**Acceptance criteria**:
- [ ] Sweep covers every fixture file in `examples/basics/src/` and `examples/errors/` — verified by an assertion that the test iterates a non-empty fixture list and visits each
- [ ] Performance tests pass in release mode (assertions in the test file are the bar)
- [ ] Stdio smoke extension passes
- [ ] No new compiler-binary output regressions (CLI byte-identical for existing fixtures)
- [ ] LSP-vs-CLI divergence check (Step 8): zero divergence across the error gallery
- [ ] No flaky tests on 10 consecutive `cargo test -p ynz-lsp` runs (verified manually before PR)
- [ ] All existing 830+ tests pass

**Quality gate**:
- [ ] No `// TODO` / `// FIXME` / `// HACK`
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] No hardcoded keyword/intrinsic strings in integration tests (use registry); fixtures are the only place specific tokens appear
- [ ] Tests document what's being verified in the test name (no `test_thing1`, `test_thing2`)
- [ ] No subprocess tests leak processes (use `Drop`-based cleanup or `tokio::process::Child::kill_on_drop`)

**Verification**:
- `cargo test -p ynz-lsp 2>&1 | grep 'test result'` — all pass
- `cargo test --release -p ynz-lsp -- --ignored performance` — perf tests pass
- `cargo test --workspace 2>&1 | grep 'test result'` — full green
- `for f in examples/basics/src/*.ynz examples/errors/*.ynz; do echo "$f"; ./target/debug/ynz build "$f" > /dev/null && echo OK || echo FAIL; done` — no surprises

**Exit Sequence:** Standard.

---

### Phase 9: Verification, demo, error gallery, tag v0.2.0-m2

**PR scope**: Final sweep. Run TODO scan across the milestone diff (lift any orphaned items into `.claude/todos.md`). Run jargon audit + clippy. Update `examples/basics/src/entrypoint.ynz` with the "open in VSCode to see X" comment block. Write `examples/errors/v0_2_m2_errors.ynz`. Final cumulative code-reviewer sweep. Bump `Cargo.toml` workspace version to `0.2.0-m2`. Cut tag. Update root README + state.md.
**Branch**: `chore/v0-2-m2-release`
**Flag**: N/A
**Est. lines**: ~250 (entrypoint.ynz comment + v0_2_m2_errors.ynz + Cargo.toml bump + state.md + README ~250)
**Ships via**: `/release`

**Objective**: Close the milestone. Tag `v0.2.0-m2`. Flip plan to `done`. Patrick has a working LSP+VSCode setup committed and tagged.

**Why this phase exists**: The roadmap's release ritual (Step 10 cumulative review + Step 11 tag) closes the milestone cleanly and produces a checkpoint that v0.2-M3 can depend on.

**Current-state anchors**:
- `examples/basics/src/entrypoint.ynz` — v0.1 demo with comments per milestone
- `examples/errors/` — per-milestone gallery; `v0_2_m1_errors.ynz` from previous milestone
- `Cargo.toml` workspace.package.version = `0.2.0-m1`
- `.claude/state.md` Active Decisions section

**Files (expected scope)**:
- EDIT: `examples/basics/src/entrypoint.ynz` — add top-of-file comment block documenting the v0.2-M2 LSP UX (see Demo & Error Gallery invariant)
- NEW: `examples/errors/v0_2_m2_errors.ynz` — error-gallery file for M2-introduced error/UX surfaces (commentary-driven UX-demo per the invariant)
- EDIT: `Cargo.toml` — `workspace.package.version = "0.2.0-m2"`
- EDIT: `.claude/state.md` — append v0.2-M2 SHIPPED entry to Active Decisions
- EDIT: `CLAUDE.md` (if has a "What's new" section) — note the LSP + VSCode extension
- EDIT: root `README.md` (if exists) — add a "Editors" section pointing at the VSCode extension
- EDIT: this plan file front-matter — flip `status: active` → `status: done` after final reviewer PASS

**Deviation rule**: Standard.

**Steps**:
1. TODO sweep: `grep -rn "TODO\|FIXME\|HACK\|XXX\|PLACEHOLDER" crates/ynz-lsp crates/ynz-tmgrammar tooling/ design/lsp.md` — for any hits, move to `.claude/todos.md` and remove the inline comment
2. Quality Checklist verification (the "Quality Checklist" block below): tick each box with evidence
3. Update `examples/basics/src/entrypoint.ynz`: top-of-file comment block per the Demo & Error Gallery invariant ("open this file in VSCode with the Yinz extension installed to see hover docs on every keyword, autocomplete after typing `int.`, inline diagnostics for intentional errors at `examples/errors/v0_2_m2_errors.ynz`"). No new Yinz CODE — only the comment.
4. Write `examples/errors/v0_2_m2_errors.ynz`: a UX-demo file with intentional errors plus commentary documenting expected LSP behavior. Each section has a `// WHY:` heading. Sections: hover-over-deferred-feature (e.g. `let x: f32 = 1.0` triggers the f32-deferred-feature error AND demonstrates hover content); autocomplete-after-dot demo (cursor positions noted in comments); banned-declaration-keyword demo (`type Foo = ...` triggers ban + the LSP hover shows what-instead).
5. Run the FULL test suite: `cargo test --workspace --release` — must be green
6. Run `cargo clippy --workspace -- -D warnings`
7. Run `cargo fmt --all --check` — must be clean
8. Bump `Cargo.toml` workspace version to `0.2.0-m2`
9. Run cumulative code-reviewer (Step 10f) on `git diff <plan-base-commit>..HEAD`
10. Update `.claude/state.md` Active Decisions: append `- [<date>] **v0.2-M2 SHIPPED (tag v0.2.0-m2, NNN tests)**: ynz-lsp crate, VSCode extension (in-repo tooling/vscode-ynz, marketplace preview OR vsix), registry-derived TM grammar. LSP wraps salsa queries for diagnostics/autocomplete/hover. Plan: ` (link to done/ path)
11. Run `/release` skill — bumps Cargo.toml, generates CHANGELOG section, commits, tags `v0.2.0-m2`, pushes (with Patrick's approval per the skill's confirmation step)
12. Flip plan front-matter: `status: active` → `status: done`. Radar moves the file to `plans/done/` on next rebuild.

**Acceptance criteria**:
- [ ] All milestone phases (Phases 0-8) have all acceptance/quality boxes ticked
- [ ] No orphaned TODOs/FIXMEs/etc. in the milestone diff
- [ ] `examples/basics/src/entrypoint.ynz` has the v0.2-M2 LSP comment block
- [ ] `examples/errors/v0_2_m2_errors.ynz` exists with intentional triggers + `// WHY:` commentary
- [ ] `cargo test --workspace --release` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo fmt --all --check` clean
- [ ] `Cargo.toml` workspace version = `0.2.0-m2`
- [ ] Cumulative code-reviewer verdict: PASS
- [ ] `.claude/state.md` updated with shipped entry
- [ ] `v0.2.0-m2` git tag exists locally and remotely
- [ ] This plan file has `status: done`

**Quality gate**: (this is the milestone-wide Quality Checklist; see the consolidated checklist below)

**Verification**:
- `git tag -l 'v0.2.0-m2'` — tag exists
- `cargo test --workspace 2>&1 | grep 'test result'` — green
- `grep -rn "TODO\|FIXME\|HACK" crates/ynz-lsp crates/ynz-tmgrammar tooling/ design/lsp.md` — empty
- `cat .claude/state.md | grep 'v0.2-M2 SHIPPED'` — entry present

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick all milestone-level Quality Checklist boxes; bump `last_updated:`. Ensure every phase's checkboxes are accurate.
2. **Invoke code-reviewer (CUMULATIVE).** `Agent({ subagent_type: "code-reviewer", description: "Review cumulative v0.2-M2 diff", prompt: "End-of-milestone review for v0.2-M2 LSP Thin Slice. Cumulative diff: git diff <plan-base-commit>..HEAD. Audit against ALL phases' acceptance criteria, the invariants block (Safety/Performance/Teaching/Runtime Dependencies/Kernel-Mode/Demo & Error Gallery/Feature Registry Entries), the project rules, and laziness patterns. Output in standard format." })`
3. **Handle verdict.** BLOCK → fix → re-invoke (max 3 rounds). PASS → continue.
4. **Run `/release`** — bumps Cargo.toml, generates CHANGELOG, commits, tags, pushes (with Patrick approval).
5. **Flip status.** `status: active` → `status: done` in front-matter. Radar moves file on next rebuild.
6. **Prompt user.** "v0.2-M2 shipped. Tag v0.2.0-m2 cut. Plan archived to done/. Ready to plan v0.2-M3 (`ynz fmt`)?"

---

## Quality Checklist (verify at completion)

- [ ] All inputs validated (LSP requests parsed via `lsp-types` deserialization; malformed requests return LSP-protocol errors via framework)
- [ ] Auth/authz: N/A (LSP runs as the user, no auth layer)
- [ ] Error handling: LSP-protocol errors for malformed requests; salsa query errors surface as inline diagnostics (not LSP errors); no panic-on-user-input
- [ ] No SQL injection / XSS / path traversal: LSP doesn't open arbitrary files outside what client sends as URIs; URIs are validated as well-formed before salsa-input creation
- [ ] No secrets exposed: marketplace publisher token is NEVER in any committed file (Phase 7 quality gate enforces)
- [ ] Performance: salsa memoization is the only incremental layer; cold/incremental/completion/hover all within stated p95 targets; perf tests in Phase 8 lock the bar
- [ ] Tests: happy path + error cases + edge cases (LF/CRLF, multi-byte chars, empty files, cursor at byte 0, cross-file diagnostics)
- [ ] Existing tests still pass (830+ initial baseline; new count after M2 ≈ 880+)
- [ ] Types are complete (no `any`-equivalent, no non-null assertions on user input paths)
- [ ] Follows existing codebase conventions (*Table pattern, builder-style, single-source-of-truth via registry)
- [ ] Every phase received a code-reviewer PASS before committing (per phase Exit Sequence)
- [ ] Final cumulative code-reviewer sweep passed (Phase 9 Step 2)
- [ ] Plan-file acceptance-criteria checkboxes accurate across all phases (Phase 9 Step 1)
- [ ] Cargo.toml workspace version bumped to `0.2.0-m2`
- [ ] `v0.2.0-m2` git tag cut

---

## Deferrals in This Plan (all tracked per global graveyard "Untracked Deferrals" entry)

**Tracking discipline**: every row's "Where tracked" must point to a durable doc that survives this plan moving to `done/`. The plan file itself is NOT a durable home (it disappears from the active radar after the milestone tag). Phase 0 Steps 8-9 PROACTIVELY edit each home to enumerate the deferral by name, so a future chat reading the M5 plan doesn't miss anything M2 dropped on the floor.

| Deferral | Why deferred | Durable home (edited in Phase 0 Step 8/9) |
|---|---|---|
| `textDocument/definition` (go-to-def) | M5 scope per roadmap | Roadmap M5 milestone entry (already lists it) |
| `textDocument/references` (find-refs) | M5 scope per roadmap | Roadmap M5 milestone entry (already lists it) |
| `textDocument/rename` | M5 scope per roadmap | Roadmap M5 milestone entry (already lists it) |
| `textDocument/formatting` (format-on-save) | M3 (fmt library) + M5 (LSP wiring) | Roadmap M3 + M5 entries (already list it) |
| `textDocument/inlayHint` (the LSP method specifically) | M5 scope (muted-hint surfaces) | Roadmap M5 entry — **enumerated by name** in Phase 0 Step 8 |
| `textDocument/codeAction` | M5 scope | Roadmap M5 entry — **enumerated by name** in Phase 0 Step 8 |
| `textDocument/semanticTokens` (richer highlighting beyond TextMate) | M5 scope | Roadmap M5 entry — **enumerated by name** in Phase 0 Step 8 |
| Doc-comment integration in hover (rich body from `///` comments) | Touched in Phase 5 best-effort; full support in M5 | Roadmap M5 entry **enumerated** + (if Phase 5 skips it) `.claude/todos.md` "Soon" entry added by Phase 5 Step 4 |
| Pull diagnostics (LSP 3.17 `textDocument/diagnostic` pull model) | Push-via-publishDiagnostics sufficient for thin slice | Roadmap M5 entry — **enumerated by name** in Phase 0 Step 8 |
| `Diagnostic.code` / `Diagnostic.codeDescription` fields | Could become DiagnosticKind name in M5 | Roadmap M5 entry — **enumerated by name** in Phase 0 Step 8 |
| Structured `Diagnostic.data` field (for client-side WHAT/WHAT-INSTEAD/WHY rendering) | Plaintext message sufficient for thin slice; structured data is an M5 enhancement | Roadmap M5 entry — **enumerated by name** in Phase 0 Step 8 |
| GitHub Actions CI for `tooling/vscode-ynz/` build/publish | Infrastructure work, not M5 scope; manual build OK for now | `.claude/todos.md` "Later" bin entry `vscode-extension-ci-workflow` added by Phase 0 Step 9 |
| Marketplace publish IF Phase 7 fallback triggers | Friction-driven fallback to .vsix release | Conditional — Phase 7 Step 5 adds the `.claude/todos.md` "Soon" entry IFF an objective trigger fires |
| Self-hosting (rewrite `ynz-lsp` in Yinz) | v2+ per `design/mvp-scope.md` | `design/lsp.md` "Self-hosting migration plan" section (created in Phase 0 Step 1) |

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: Each phase is ONE branch off main + ONE PR. Marketplace publish is a separate phase from extension scaffolding. The grammar generator + extension scaffolding ship in ONE PR because they're tightly coupled (extension references the generated artifact) but each is reviewable independently in the diff.
- **Shadow main branches**: Every phase merges to `main` via `/pr` before the next phase starts. No long-lived feature branches.
- **Building the engine before shipping value**: Phase 2 (lifecycle) is the only "engine" phase that doesn't ship user-visible value. Phases 3 (diagnostics), 4 (completion), 5 (hover), 6 (grammar + extension scaffold), 7 (distribute) each ship something Patrick can SEE in VSCode. Three of those land before Phase 7's marketplace decision so even if Phase 7 falls to .vsix, the LSP value still shipped.
- **Hotfix that isn't**: No "hotfix" pattern in this plan — it's a feature milestone. If a Phase N CI break needs a fix, it's a fix commit on the Phase N branch, not a parallel hotfix branch.
- **Abandoned branches**: Each phase's branch closes via merge OR explicit abandonment with a `todos.md` entry. The Phase 1 spike's loser branch is folded back into the winning branch and the loser's spike dir deleted in the same PR.
- **Flag graveyards**: No feature flags in this milestone. The LSP/extension is either installed (then it works) or not (then it doesn't affect anything). No `Cargo.toml` cfg gates either — the `ynz-lsp` crate is a separate binary that doesn't link into `ynz-driver`.

---

## Reviewer Disputes

(none yet — populated by Step 7 plan-review iteration if applicable)
