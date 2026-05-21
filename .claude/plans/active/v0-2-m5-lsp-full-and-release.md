---
slug: v0-2-m5-lsp-full-and-release
type: execution
owner: Patrick Rizzardi
status: active
roadmap: v0-2-dev-loop-tooling
created: 2026-05-20
last_updated: 2026-05-21 (Phase 11b complete — dynamic-dispatch call-site coercion; Phases 0-11b all done)
files:
  - crates/ynz-lsp/**
  - crates/ynz-typeck/src/**
  - crates/ynz-registry/src/**
  - crates/ynz-driver/src/**
  - crates/ynz-codegen/src/**
  - tooling/vscode-ynz/**
  - design/lsp.md
  - design/ide-hints.md
  - design/mvp-scope.md
  - design/compiler-language.md
  - .claude/plans/roadmaps/v0-2-dev-loop-tooling.md
  - .claude/todos.md
  - examples/pirates-roster/**
  - examples/primantis-orders/**
  - CHANGELOG.md
  - Cargo.toml
depends_on: [v0-2-m1-feature-inventory-sync, v0-2-m2-lsp-thin-slice, v0-2-m3-fmt, v0-2-m4-watch]
---

# Plan: v0.2-M5 — LSP Full + v0.2.0 Release

Created: 2026-05-20
Status: pending_approval

## Context & Why

**Goal**: Take the LSP thin slice shipped in v0.2-M2 (lifecycle, diagnostics, autocomplete, hover) and grow it into a full editor experience. Adds **go-to-definition, find-references, rename, format-on-save** (delegates to `ynz-fmt` from M3), **`textDocument/inlayHint`** for the muted-hint protocol (all 9 registry domains; 5 fire today, 4 protocol-only awaiting v0.3+ data), **code actions** (quick-fixes leveraging WHAT-INSTEAD content), **semantic tokens** (richer-than-TextMate highlighting), **doc-comment hover** integration, **structured `Diagnostic.code`/`data` fields**, and a **`ynz build --json`** mode for tooling consumers. Sweeps the three `todos.md` "Soon" compiler bugs that were explicitly deferred to v0.2 LSP work (hidden-field default eval, dynamic-dispatch call-site coercion, UFCS const-lend check). Ships the VSCode extension v0.2 polish pass (`\n\n` separator UX, screenshots, doc-comment integration). Final phase bumps `Cargo.toml 0.2.0-m4 → 0.2.0`, runs `/release` to cut the `v0.2.0` git tag, attaches `yinz-0.2.0.vsix` + `yinz-latest.vsix` to the GitHub release.

**Why now**:
- v0.2-M1 shipped the SSOT registry (tag `v0.2.0-m1`); v0.2-M2 shipped the LSP thin slice + VSCode extension (tag `v0.2.0-m2`); v0.2-M3 shipped `ynz fmt` library + CLI (tag `v0.2.0-m3`); v0.2-M4 watch is shipped (`v0.2.0-m4`) with a small post-ship bug-fix tail still active. The roadmap (`v0-2-dev-loop-tooling`) puts M5 as the closer that converts the thin slice into the full editor experience AND cuts the v0.2.0 series tag.
- v0.2.0 tag is **not a public ship** — same flow as every other intermediate tag (`v0.1.0-m4`, `v0.2.0-m1` etc): git tag + CHANGELOG section + `.vsix` upload via `/release` skill. Public ship is v1.0 per `design/mvp-scope.md`.
- M5 can plan + start in parallel with M4 watch bug-fixing (no architectural overlap — M4 is `crates/ynz-watch/`, M5 is `crates/ynz-lsp/` + `crates/ynz-typeck/` helpers). Patrick confirmed this session.
- The compiler now has cross-file resolution shipped (`crates/ynz-typeck/src/exports.rs::collect_exports` + `crates/ynz-typeck/src/resolve_import.rs::resolve_imports` + `ResolvedImport`) — the foundation go-to-def / find-refs / rename ride on. Without M5, this resolution exists but no editor surfaces it.

**Background**:
- 13 workspace crates as of M4 (`crates/ynz-ast`, `ynz-codegen`, `ynz-diagnostics`, `ynz-driver`, `ynz-fmt`, `ynz-lsp`, `ynz-numerics`, `ynz-parser`, `ynz-registry`, `ynz-runtime`, `ynz-tmgrammar`, `ynz-typeck`, `ynz-watch`). Cargo.toml at `0.2.0-m4`.
- `crates/ynz-lsp/src/` currently: capabilities.rs (75), completion.rs (185), diagnostic_transform.rs (196), hover.rs (219), lib.rs (10), main.rs (3), position.rs (278), server.rs (326), state.rs (88) — 1,380 lines total. Single-threaded dispatch via `lsp-server = "0.7.9"` (locked in M2).
- Current capabilities advertised (`crates/ynz-lsp/src/capabilities.rs`): `textDocumentSync`, `completionProvider`, `hoverProvider`. M5 adds: `definitionProvider`, `referencesProvider`, `renameProvider`, `documentFormattingProvider`, `documentRangeFormattingProvider`, `inlayHintProvider`, `codeActionProvider`, `semanticTokensProvider`.
- `ynz-typeck` shape/function decls already track `SourceSpan` for their declaration site (`ShapeDef.defined_at`, `FunctionSig.decl_span`) — go-to-def needs the AST-node-at-offset lookup; the declaration sites are ready.
- `ynz-fmt::format(source: &str) -> Result<String, FmtError>` is the format-on-save target. `format_named(source, name)` for error rendering with a file path. M3 already enforces `fmt(fmt(x)) == fmt(x)` via property tests.
- VSCode extension at `tooling/vscode-ynz/` ships as `.vsix` (marketplace publish blocked in M2 by Azure DevOps publisher account; deferred per `marketplace-publish-followup` in `todos.md`). M5 stays on `.vsix` per Patrick's decision this session.
- 1143 tests passing on `main` as of M3 ship (M4 adds more; will be ~1200+ by M5 start).

**Constraints (locked from roadmap + this planning session)**:
- **No new language features.** M5 is pure tooling + diagnostic-surface work + scoped compiler bug-fixes from todos.md "Soon". Zero new tokens, zero new typeck/codegen language semantics. The three compiler bug-fixes (hidden-field default eval, dynamic-dispatch coercion, UFCS const-lend) are CORRECTNESS fixes for existing features, not new features.
- **No marketplace publish.** Stay on `.vsix` + GitHub release. `marketplace-publish-followup` stays in todos.md "Later" — picked up post-v0.2 if/when Patrick chooses.
- **All 9 inlay-hint registry domains get protocol handlers; 5 wire data today** (variable_type, ownership_call_site, copy_points, array_to_fixed_promotion, let_to_const_promotion); 4 are protocol-only (function_param_type, wait_points, lifetimes, allocators) because their underlying analysis doesn't exist in Yinz v0.1 (no lambdas, no I/O suspension analysis, no explicit lifetimes, no arenas). The 4 deferred domains land hints automatically when v0.3+ adds the data — no further LSP code change needed. Each gets a tracking entry in `todos.md` "Later" with a trigger.
- **Pull-diagnostics deferred to v0.3+.** Push diagnostics work fine; pull is a v0.2-M5 candidate per M2 plan but adds API surface without user-visible value when push already works. Defer with trigger entry in todos.md.
- **Cross-file go-to-def: full scope.** Lean on `exports.rs` + `resolve_import.rs` already shipped. Click on an imported `foo` → jumps to the declaration in the source file. Same for shapes, options, constants.
- **v0.2.0 tag bundled in final phase.** Last phase bumps `Cargo.toml`, runs `/release`, ships `yinz-0.2.0.vsix` + `yinz-latest.vsix`. M4 watch MUST be fully merged AND on a stable tag (`v0.2.0-m4`) before this phase runs — phase has an explicit gate.
- **Cargo.toml workspace package version stays in lockstep across all crates.** No per-crate divergence. The bump goes in the workspace `[workspace.package].version` field.
- **All compile errors continue WHAT/WHAT-INSTEAD/WHY format.** LSP renders the same diagnostics the CLI does; code actions surface the WHAT-INSTEAD content as the quick-fix label.
- **Compiler binary's behavior on a `.ynz` file is byte-identical EXCEPT for the 3 scoped bug-fixes.** The bug-fixes are CORRECTNESS improvements: hidden-field default eval changes silently-wrong-zero-init to correct values for non-zero-default fields; dynamic-dispatch call-site coercion adds a previously-missing implicit conversion; UFCS const-lend check adds a previously-missing error for a code path that already errored via the function-call form. Each is fixture-tested + snapshot-stable.
- **No GC, no per-instance method storage, none of v0.1's locked properties weaken.** Tooling only (plus the scoped bug-fixes that preserve existing semantics).
- **Existing 1143+ tests (M3) / ~1200+ (M4) must still pass.** New tests added; no existing tests weakened.
- **All milestone plan invariants per `.claude/rules/plan-invariants.md` apply** — 7-subsection Invariants block including the M9+ Feature Registry Entries subsection.

**Success criteria**:
- `ynz-lsp` advertises all 8 new capabilities (definition / references / rename / formatting / rangeFormatting / inlayHint / codeAction / semanticTokens) on `initialize`.
- A VSCode user with the Yinz extension installed can: Cmd+click a function name and jump to its declaration in the SAME or a DIFFERENT file; right-click → "Find All References" lists every use across the project; F2 to rename atomically updates every reference; format-on-save runs `ynz-fmt` and rewrites the file; inlay hints render type annotations (` : int`) after `let x = 42`, ownership modifiers (`share`/`lend`/`give`) at call sites, copy markers at trivially-copyable arg passes, `// promoted to fixed<...>` annotations on never-grown arrays, `// effectively const` annotations on let-never-mutated bindings; code actions offer one-click fix for any diagnostic with a WHAT-INSTEAD; semantic tokens distinguish keyword vs type vs function vs variable identifiers.
- `ynz build --json input.ynz` emits NDJSON diagnostic events (one per diagnostic, plus a final summary event); existing human-readable `ynz build input.ynz` output is byte-identical.
- The 3 `todos.md` "Soon" compiler bugs (hidden-field default eval, dynamic-dispatch call-site coercion, UFCS const-lend) are fixed; each has a fixture-test that fails on `v0.2.0-m4` and passes on this branch.
- `examples/pirates-roster/entrypoint.ynz` extended with a new section demonstrating the LSP features in inline comments ("hover here", "Cmd+click here", etc.).
- `examples/primantis-orders/v0_2_m5_errors.ynz` new file: intentional triggers for any NEW diagnostic class introduced by the bug-fixes + code-action-surfacing triggers.
- All 1200+ existing tests pass. New `crates/ynz-lsp/tests/*` integration tests cover lifecycle + each new request type. New `crates/ynz-typeck/tests/*` cover the symbol-lookup helpers added in Phase 1.
- Tag cut: `v0.2.0` (drops `-mN` suffix). CHANGELOG entry generated by `/release` skill from merged PRs since `v0.2.0-m4`. Extension shipped as `yinz-0.2.0.vsix` + `yinz-latest.vsix` attached to the GitHub release.

## Research Findings

**M2 LSP architecture (verified against `crates/ynz-lsp/src/server.rs:1-50`)**:
- Single-threaded dispatch via `lsp-server` crate; `ServerState` owns `&mut CompilerDb` directly. Concurrent requests serialize. This model carries through M5 — new request handlers (`Definition`, `References`, `Rename`, `Formatting`, etc.) plug into the same `main_loop` dispatch.
- `ServerState` (`crates/ynz-lsp/src/state.rs:1-88`) already tracks `open_documents: HashMap<Url, String>`, `line_tables: HashMap<Url, LineTable>`, `last_published: HashMap<Url, Vec<Diagnostic>>`, encoding negotiation, shutdown flag. M5 adds: nothing new at the state level — every new handler reads through `&self.db` + existing maps. Workspace-edit-builder for rename is a per-request stack helper.
- Position conversion (`crates/ynz-lsp/src/position.rs::LineTable::byte_offset_to_position` / `position_to_byte_offset`) supports UTF-8 + UTF-16 with line-table caching. M5 reuses unchanged.

**Cross-file resolution surface (verified against `crates/ynz-typeck/src/`)**:
- `pub struct ExportTable { shapes: HashMap<String, ShapeDef>, options: HashMap<String, OptionsEntry>, functions: HashMap<String, FunctionSig> }` — per-file exported symbols (`exports.rs`).
- `pub fn collect_exports(module, shape_table, options_table, signature_table) -> ExportTable` — builds the table from `is_exported = true` items (`exports.rs`).
- `pub struct ResolvedImport { ... }` + `pub fn resolve_imports(importer_path, ...) -> Vec<ResolvedImport>` — resolves `import { foo } from "./bar"` to the exporting SourceFile + symbol name (`resolve_import.rs`).
- `ShapeDef.defined_at: SourceSpan`, `FunctionSig.decl_span: SourceSpan` — declaration sites are tracked.
- What's MISSING: AST-node-at-byte-offset lookup. Currently no `node_at_offset(db, source, offset) -> Option<NodeRef>` query. Phase 1 adds it.

**Inlay-hint registry (verified against `registry/features.toml`)**:
- 9 `[[muted_hint_domain]]` entries in registry; each has `domain`, `placement_category` (`"Addition"` / `"Replacement"` / `"Informational"`), `description`, `example_source`, `example_hint_rendered`.
- Adapters `muted_hint_domains() -> Iterator<&'static MutedHintDomainEntry>` + `muted_hint_domain_lookup(domain) -> Option<...>` already exist (`crates/ynz-registry/src/lib.rs`).
- M5 adds: per-domain detection passes in `ynz-typeck` (5 of them) + LSP handler that calls each pass and emits LSP `InlayHint` objects.

**LSP capability surface to add** (per `lsp-types` crate, advertised in `crates/ynz-lsp/src/capabilities.rs`):

| Capability | LSP types used | Phase |
|---|---|---|
| `definitionProvider: bool` | `request::GotoDefinition`, `Location` | 2 |
| `referencesProvider: bool` | `request::References`, `Location`, `ReferenceParams.context.include_declaration` | 3 |
| `renameProvider: RenameOptions { prepare_provider: Some(true) }` | `request::Rename`, `request::PrepareRenameRequest`, `WorkspaceEdit`, `TextEdit` | 4 |
| `documentFormattingProvider: bool` + `documentRangeFormattingProvider: bool` | `request::Formatting`, `request::RangeFormatting`, `TextEdit` | 5 |
| `inlayHintProvider: InlayHintOptions { resolve_provider: Some(false) }` | `request::InlayHintRequest`, `InlayHint`, `InlayHintKind` (`Type` / `Parameter`) | 6 |
| `codeActionProvider: CodeActionProviderCapability::Options { code_action_kinds: [QuickFix] }` | `request::CodeActionRequest`, `CodeAction`, `WorkspaceEdit` | 7 |
| `semanticTokensProvider: SemanticTokensOptions { legend, full, range }` | `request::SemanticTokensFullRequest`, `request::SemanticTokensRangeRequest`, `SemanticTokens`, `SemanticTokenType` | 8 |

**`ynz-fmt` integration for format-on-save** (verified against `crates/ynz-fmt/src/lib.rs`):
- `pub fn format(source: &str) -> Result<String, FmtError>` — pure function, no I/O. Phase 5 wraps it.
- `pub fn check(source: &str) -> Result<CheckResult, FmtError>` — used by `ynz fmt --check`; not needed by LSP.
- Range formatting NOT yet in `ynz-fmt` API. `lsp-range-formatting` is in `todos.md` "Later" — Phase 5 implements it as part of M5 since `rangeFormatting` is advertised here.

**`ynz build --json` design** (new in Phase 9 — LOCKED SCHEMA):
- New CLI flag `--json` on `ynz build`. Suppresses ariadne human output; emits NDJSON to stdout.
- **Schema (locked here; v0.2.0 stabilizes it)**:
  ```json
  {"type": "diagnostic", "schema_version": "v0.2.0-m5-unstable", "severity": "error" | "warning" | "suggestion", "kind": "<DiagnosticKindName>", "code": "<DiagnosticKindName>", "span": {"file": "<absolute-path>", "start_byte": <u32>, "end_byte": <u32>}, "message": "<full WHAT/WHAT-INSTEAD/WHY concatenation>", "data": {"what": "<string>", "what_instead": "<string>", "why": "<string>"}}
  {"type": "summary", "schema_version": "v0.2.0-m5-unstable", "errors": <u32>, "warnings": <u32>, "suggestions": <u32>, "exit_code": <i32>}
  ```
- **Field semantics (each locked)**:
  - `span.file`: absolute filesystem path (matches what `SourceSpan` stores; matches what `LineTable` keys on).
  - `span.start_byte` / `span.end_byte`: UTF-8 byte offsets, 0-indexed, half-open (start inclusive, end exclusive). Byte offsets match the compiler's internal `SourceSpan` semantics exactly; no encoding conversion. **NOT line/character offsets** — consumers that want line/character compute them via UTF-8 line tables themselves (Yinz files are UTF-8 by spec).
  - `severity`: lowercase string literal (`"error"` / `"warning"` / `"suggestion"`). Stable; not bitfield/enum index (consumer-friendly).
  - `kind` and `code`: identical string; the registered `DiagnosticKind` name (PascalCase per registry). `kind` for human readability, `code` for LSP-compatibility (mirrors `lsp_types::Diagnostic.code` field).
  - `message`: full plaintext rendering (WHAT + `\n\nWHAT INSTEAD: ` + what_instead + `\n\nWHY: ` + why) — same string the LSP emits as `Diagnostic.message`. Embedded newlines inside any of the three fields are NDJSON-escaped (`\n` literal in JSON) — Phase 9 includes a fixture with a multi-line `why` field to verify escaping.
  - `data`: structured object so consumers can render WHAT/WHAT-INSTEAD/WHY in custom UI without re-parsing `message`. Each field is a UTF-8 string; never null (compiler guarantees all three are populated per `Diagnostic` constructor invariant).
  - `null` handling: NO field in this schema is nullable. If a value is missing it's omitted entirely (JSON-object-key-absent). Codegen-time check: serde serialization uses `#[serde(skip_serializing_if = "Option::is_none")]` for any optional field added later. Never emit `null`, `NaN`, or `Infinity` (well-known JSON edge-case footguns).
- **Encoding**: stdout is UTF-8. One JSON object per line, `\n` terminated (Unix line endings; not CRLF — consumer tools standardize on LF for NDJSON).
- **Exit code semantic (LOCKED per plan-reviewer Round 2 Adversarial #3)**: `summary.exit_code` matches `ynz build` exit semantics exactly — `0` if `errors == 0` (warnings + suggestions do NOT trigger non-zero); non-zero on any error. This way `ynz build --json | jq '.exit_code'` agrees with the process exit code. Pinned now so v0.3+ can't silently flip the policy and break tooling consumers.
- **Schema versioning**: emit `"schema_version": "v0.2.0-m5-unstable"` until v0.2.0 final ships; drop `-unstable` suffix at v0.2.0 tag-cut (Phase 12 step). Closes `watch-json-schema-stabilize` in todos.md.
- Closes `lsp-vs-cli-exact-divergence` in todos.md (currently boolean error-presence; this enables count-level + per-diagnostic-kind agreement).

**Doc-comment hover integration** (Phase 10):
- `Token::DocComment { content, break_after }` is already lexed (M8 P1). However, AST currently does NOT attach doc-comments to declarations (`grep DocComment crates/ynz-ast/src/nodes.rs` confirms the variant exists in `Stmt` only, not as a field on `FunctionDecl`/`ShapeDecl`/`ConstDecl`).
- Phase 10 adds: a parser pass that ASSOCIATES adjacent `Token::DocComment` trivia with the next declaration (similar to `rustdoc`); a new field `leading_docs: Vec<String>` on `FunctionDecl`/`ShapeDecl`/`OptionsDecl`/`ConstDecl`; the LSP hover handler pulls this when the user hovers a symbol reference.
- Falls back to registry hover if no doc-comments attached.

**Three `todos.md` "Soon" compiler bug-fixes (Phase 11)**:
- **Hidden-field default eval**: `crates/ynz-codegen/src/emit.rs::lower_struct_lit` currently zero-inits hidden fields; for `hidden field: string = "default"` the field gets a null pointer instead of `"default"`. Fix: evaluate the default-expression AST node at emit-time, store the result in the LLVM struct GEP.
- **Dynamic-dispatch call-site coercion**: `crates/ynz-typeck/src/check.rs::coerce_to_dynamic` infrastructure is in place (vtable globals emitted M4 P3b) but passing a concrete shape to a `dynamic Foo` parameter is currently a typeck error ("type mismatch"). Fix: in check.rs argument-type matching, if expected = `Type::Dynamic(name)` and actual = `Type::Shape(s)` AND `s` follows `Foo`, accept + emit the coerce. Mirror in emit.rs to lower the actual coerce.
- **UFCS const-lend check**: `crates/ynz-typeck/src/check.rs` (line ~936 comment) — receiver ownership not checked for dot-call UFCS; only free-function-call form is checked. The function-call form produces the correct error; the dot-call form silently passes. Fix: lift the ownership check from the call-arg handler into a shared helper called by BOTH dot-call and function-call paths.

**Branching/PR sizing** (per `~/.claude/memory/branching.md`): each phase = one branch off `main`, one PR via `/pr`. Soft target ~500 lines/PR; some phases (Phase 6 inlay hints with 5 firing domains; Phase 11 bug sweep with 3 fixes) trend toward 700-800 — acceptable per branching.md "soft target."

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Cross-file rename invalidation misses one reference; rename corrupts code | Medium | High | Phase 4 integration tests: rename in `examples/pirates-roster/` spans `services/`, `utils/`, `entrypoint.ynz`; assert every reference updates. LSP `WorkspaceEdit` is atomic-or-fail at the client level; client sees rollback on any per-file failure. `prepare_provider: Some(true)` gates the rename before it starts so the user sees the symbol range first. |
| Inlay-hint compute fires on every keystroke at full file scope; >50ms render kills UX | High | Medium | LSP `inlayHint` request includes `range` parameter (viewport). Server filters domains to byte-spans within range. Performance budget: <30ms p95 for 500-line file. Phase 6 integration test asserts the budget via timing wrapper. Salsa caches the typeck pass so re-renders are cheap. |
| Semantic tokens disagree with TextMate grammar; colors flicker between fallback and refined | Medium | Low | semanticTokens REFINES TM (identifier-kind: variable/function/type). Identical scope strings → identical color via theme mapping. Same registry-driven keyword list. Phase 8 integration test renders the same file via both surfaces and asserts no color disagreement on keyword tokens. |
| Phase 11 bug sweep reveals deeper compiler bugs beyond the 3 todos | Medium | Medium | Scope each Soon item to <100 lines net change. If a fix grows, split to follow-up (add to `todos.md` "Soon" with concrete trigger); don't expand M5 scope. Each fix has a fixture-test that fails on `v0.2.0-m4` baseline and passes on the fix branch — test isolation prevents accidental coupling. |
| Format-on-save creates infinite `didChange` loop with formatter | Low | Medium | M3 already enforces `fmt(fmt(x)) == fmt(x)` via proptest. LSP doesn't auto-save; client controls (VSCode applies `TextEdit` then fires `didChange` ONCE — formatter returns `Vec<TextEdit>` empty if no change needed, breaking any potential loop). Standard pattern. |
| Doc-comment AST attachment (Phase 10) requires parser change; risk of breaking existing parse tests | Medium | Medium | Parser change is ADDITIVE at the SERDE layer AND at the struct-construction layer: existing `Stmt::DocComment` variant stays; new field is **`leading_docs: Option<Vec<String>>`** (NOT bare `Vec<String>`) so every existing struct-literal construction site stays valid by populating `leading_docs: None` (or via `..Default::default()` if the struct derives `Default`). `#[serde(default)]` covers the serde-deserialize path. Phase 10 step explicitly enumerates affected construction sites (`grep -rn "FunctionDecl {" crates/ \| wc -l`, same for ShapeDecl/OptionsDecl/ConstDecl) and confirms each adds `leading_docs: None` (or relies on `..Default::default()`). Phase 10 integration test compares AST output on every existing fixture: shape stable except for the new field populated as `None` (or `Some(vec![...])` when doc-comments present). |
| VSCode 0.2 upgrade breaks `v0.2.0-m2` installed users (manual .vsix install) | Low | Low | `tooling/vscode-ynz/README.md` documents upgrade path: download new `.vsix` from GitHub release, install via "Install from VSIX." Stable URL `yinz-latest.vsix` doesn't change. No user data lost. |
| `ynz build --json` (Phase 9) drifts from CLI ariadne output | Low | Low | `--json` suppresses ariadne entirely; existing CLI path runs the SAME `DiagnosticBucket` collection then takes the JSON-serialize branch vs the ariadne-render branch. Phase 9 integration test asserts both branches produce equivalent error/warning/suggestion counts on every fixture in `examples/primantis-orders/`. |
| Bumping Cargo.toml to `0.2.0` while M4 watch bugs still landing causes intermediate broken state | Medium | Low | Phase 12 final phase has an explicit gate-check: M4 watch plan in `done/` status; tag `v0.2.0-m4` exists on main; `cargo test --workspace` passes on the M5 head. If any check fails, plan pauses; phase doesn't proceed to `/release`. |
| Symbol-lookup helper API (Phase 1) gets re-shaped after Phase 2 uses it, requiring Phase 3/4 rework | Medium | Medium | Phase 1 designs API by enumerating Phases 2-4 use cases upfront in the design comment block. Includes: `def_site_for_offset(db, source, byte_offset) -> Option<(SourceFile, SourceSpan)>` (Phase 2), `references_for_offset(db, source, byte_offset, include_decl: bool) -> Vec<(SourceFile, SourceSpan)>` (Phase 3), `rename_locations(db, source, byte_offset, new_name: &str) -> Result<Vec<(SourceFile, SourceSpan)>, RenameError>` (Phase 4). All three share a `resolve_symbol_at(db, source, byte_offset) -> Option<ResolvedSymbol>` helper that does the AST-walk + import-following. Each higher-level function is a thin wrapper. |
| Auto-promotion usage-pass for `array<T>` → `fixed<T>` (Phase 6) wrongly flags a value mutated through aliasing | Low | High | Phase 6 muted-hint detection is READ-ONLY: it walks the AST looking for `.add()` / `.set()` / index-assignment calls on the binding. If found ANYWHERE in scope, no hint. If NOT found, hint says "could be fixed." Hint is informational; codegen still treats as `array<T>`. Wrong hint = ignored by user, not corrupting. Aliasing in v0.1 only happens via `.lend` (M4) — Phase 6 conservatively suppresses the hint if the binding is ever passed to a function with a `lend` parameter. |
| Yinz vocabulary leaks into LSP UI text (code-action labels, rename UI, inlay hint hover) | Medium | Low | Phase 7 code-action labels reuse Diagnostic WHY content (already audited by `tests/jargon_audit.rs`). Phase 6 inlay-hint hover text comes from registry `description` + `example_hint_rendered` (registry entries audited at registry test time). Phase 12 extends `tests/jargon_audit.rs` to cover all new LSP-rendered surfaces (code-action label strings, rename error messages, inlay-hint hover markdown). |
| LSP framework `lsp-server` doesn't expose `WorkspaceEdit` builders ergonomically — Phase 4 rename hand-rolls verbose wire-format | Low | Low | `lsp-types` crate provides `WorkspaceEdit::new(changes: HashMap<Url, Vec<TextEdit>>)` — direct constructor is sufficient. Phase 4 builds via a small `RenameEditBuilder` helper in `crates/ynz-lsp/src/rename.rs`; <30 lines. Verified by reading lsp-types docs. |
| Semantic-tokens delta encoding (LSP wire format) has off-by-one bugs | Medium | Medium | Phase 8 uses the well-tested `lsp-types::SemanticTokens::data` encoding (relative line, relative start, length, token type index, token modifiers bitmask). Phase 8 has a focused unit test per: single token on line 0, two tokens on same line, tokens across multiple lines, file-with-emoji (UTF-8 byte length vs char length). |
| Doc-comment parser-pass attachment (Phase 10) attaches to wrong declaration when blank line intervenes | Low | Medium | Locked rule: `///` lines attached to the IMMEDIATELY-NEXT declaration (no blank line between). If blank line, doc-comments become free-floating trivia, not attached. Matches rustdoc + most language ecosystems. Phase 10 fixture covers all three cases: attached, free-floating, mid-decl interleaved. |
| `inlayHint.position` placement gets confused on multi-byte characters (emoji in code) | Low | Low | `inlayHint.position` is an LSP `Position` — line + character (UTF-8 or UTF-16 unit, per negotiated encoding). Phase 6 uses the existing `LineTable::byte_offset_to_position` converter (M2-tested for UTF-8 + UTF-16). Phase 6 adds a fixture with emoji in `let 🦀 = 42` (legal identifier? — verified Yinz disallows non-ASCII identifiers in v0.1, so the test is "emoji in comment" rather than identifier). |
| Compiler bug-fix (Phase 11) hidden-field default eval changes binary output of existing programs | Medium | Medium | Hidden-field defaults currently zero-init silently. Programs that ALREADY EXIST and rely on this behavior would break. Mitigation: Phase 11 audits `examples/pirates-roster/` + all test fixtures for hidden-field usage. ANY existing usage with non-zero defaults gets called out before the fix lands. Empirically: hidden fields ship M4 but Yinz v0.1 has minimal usage; the codegen path is rarely exercised. Snapshot tests catch any unintended change. |
| Salsa cancellation: a long-running `references` request blocks subsequent `didChange` | Medium | Low | Per M2's locked dispatch model: salsa queries are not cancellable mid-execution; in-flight queries complete before next mutation. For thin slice this is acceptable; M5's `references` may scan many files (slow on large projects). Mitigation: Phase 3 emits a `$/progress` notification at start; client shows progress bar. Worst-case wait = O(N files) typeck pass, salsa-cached after first scan. |
| The bundle of LSP capability additions (8 new providers) exposes M2-shipped compiler edge cases | High | Low | Acknowledged per roadmap. Bugs surfaced get triaged: <50 lines → fix in this M5; >50 lines → file in `todos.md` "Soon" with the surface that found it. The 3-bug Phase 11 sweep is already absorbing the v0.2-M2 deferral budget. |

## Questions

None outstanding. Four key scope decisions confirmed this session before plan draft:
1. v0.2.0 tag bundled in M5 final phase (same flow as every `-mN` tag — git tag + CHANGELOG + .vsix via `/release`).
2. M5 can plan + start in parallel with M4 watch bug-fixing; final tag gate-checks M4 fully merged.
3. All 9 inlay-hint registry domains get protocol handlers; 5 fire today (variable_type, ownership_call_site, copy_points, array_to_fixed_promotion, let_to_const_promotion); 4 protocol-only awaiting v0.3+ data.
4. No marketplace publish attempt; stay on `.vsix` + GitHub release per M2 pattern. Marketplace stays in todos.md "Later."

Cross-file go-to-def: full scope (uses existing `exports.rs` + `resolve_import.rs`).

## Risk Assessment & Rollout Strategy

**Risk level: MEDIUM**

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | Compiler tooling only |
| Touches auth/permissions | No | No auth |
| Raw SQL / literals | No | No DB |
| Modifies existing data | Yes (compiler output for hidden-field default eval) | Phase 11 bug-fix changes codegen for `hidden field: T = default` — non-zero defaults now evaluate correctly; affects any existing program using non-zero hidden-field defaults (audited: ~0 in current fixtures). |
| Third-party integration | Yes | lsp-server crate (M2-locked at 0.7.9), lsp-types, VSCode marketplace (read-only this milestone) |
| Changes existing endpoints | Yes (CLI `ynz build` adds `--json` flag — additive, no behavior change to existing flags) | `--json` is opt-in; default human output unchanged byte-for-byte |
| New feature with no equivalent | Yes | 8 new LSP capability surfaces; full editor experience first time |

**Mitigations applied**:
- Each new LSP capability gets a dedicated integration test (`crates/ynz-lsp/tests/<capability>.rs`) — MEDIUM → MEDIUM-LOW
- Phase 11 bug-fixes are SCOPED with fail-on-baseline-pass-on-branch tests — MEDIUM-HIGH → MEDIUM
- Cargo.toml bump deferred to final phase with explicit gate-checks — MEDIUM → LOW for release-readiness
- `ynz build --json` is opt-in flag; default CLI behavior unchanged — MEDIUM → LOW for CLI regression
- VSCode .vsix install path unchanged from M2; stable URL preserves user upgrade flow — LOW (preemptive)
- Doc-comment parser change is additive (`#[serde(default)]` new field) — MEDIUM → LOW
- Auto-promotion usage-pass is muted-hint ONLY (no codegen change) — HIGH (if wrong) → LOW

**Rollout plan** (Yinz convention: trunk-based, no production rollout; "rollout" = milestone tag):
1. Each phase: branch from `main`, PR via `/pr`, code-reviewer agent at phase boundary, merge to `main` on PASS.
2. Phase 12 (final verification + release): gate-check (M4 merged + tag `v0.2.0-m4` exists + tests green on M5 head) → bump Cargo.toml → run `/release` → cut `v0.2.0` tag → ship `.vsix` to GitHub release.
3. v0.2.0 final tag is **internal** (rollback hygiene only; no public ship per `design/mvp-scope.md` v1.0 = first public release).

## Invariants This Milestone Must Preserve

### Safety
- All 1200+ existing tests pass post-milestone (`cargo test --workspace`).
- No previously-valid `.ynz` program becomes rejected by the compiler — verified by full test suite + every `examples/pirates-roster` and `examples/primantis-orders` fixture compile-runs identically EXCEPT for the three CORRECTNESS bug-fixes in Phase 11.
- The three Phase 11 fixes are correctness improvements with EXPLICIT before/after tests:
  - **Hidden-field default eval**: a fixture with `hidden field: string = "default"` returned NULL pointer pre-fix; returns `"default"` post-fix. Snapshot test asserts.
  - **Dynamic-dispatch call-site coercion**: a fixture with `function takeFoo(d: dynamic Foo) { ... }` called with `concrete: ConcreteFoo` was a typeck error pre-fix; compiles + dispatches via vtable post-fix.
  - **UFCS const-lend check**: `const p: Player = ...; p.heal(20)` where `heal` declares `lend self` was silently accepted pre-fix; now produces the same "cannot lend a const binding" error that `heal(p, 20)` already produced.
- No previously-rejected `.ynz` program becomes accepted EXCEPT for the dynamic-dispatch coercion case (which is INTENDED — the M4-deferred behavior).
- `ynz build` / `ynz run` exit codes, stdout, stderr are byte-identical to pre-M5 for every fixture that does NOT exercise the three bug-fix paths.
- `ynz build --json` (Phase 9) is OPT-IN; default `ynz build` output unchanged.
- The `ynz-lsp` binary, when not running, has ZERO effect on compilation.
- LSP cannot modify source files except through `WorkspaceEdit` returned to the client (Phase 4 rename, Phase 5 formatting, Phase 7 code actions) — the LSP NEVER writes to disk directly.
- `WorkspaceEdit`-based mutations (rename, code action) are atomic-or-fail at the client level — partial application is the client's responsibility per LSP spec; Phase 4 + 7 tests assert no LSP-side partial state.

### Performance

**Targets are INITIAL BUDGETS, not spec-derived hard requirements.** Calibrated against (a) typical rust-analyzer numbers for similar requests on small projects (<300ms cold go-to-def, <100ms incremental rename) and (b) Yinz compiler measured numbers from M2 (cold typeck ~3s for the pirates-roster project). If a budget is exceeded during phase measurement, EITHER fix the slow path OR raise the budget with documented rationale — DO NOT silently let budgets rot.

- **Go-to-definition** (Phase 2): <100ms p95 on a 500-line file with cross-file imports. Salsa-cached after first call; in-process integration test asserts.
- **Find-references** (Phase 3): <500ms p95 on `examples/pirates-roster/` (3 files, ~600 lines total) for a symbol used 5-10 times across files. Larger projects may take longer; Phase 3 emits `$/progress` if scan > 1s.
- **Rename** (Phase 4): <1s p95 for the same project; depends on linear file count. Atomic WorkspaceEdit; no streaming.
- **Format-on-save** (Phase 5): <50ms p95 on a 500-line file (ynz-fmt is fast; LSP overhead is the entire LSP plumbing).
- **Inlay-hint render** (Phase 6): <30ms p95 for viewport (typical ~50 lines) on a 500-line file. Salsa caches typeck; render is a hint-emission walk over the visible range only.
- **Code-action enumerate** (Phase 7): <50ms p95 to enumerate applicable actions at cursor position. Each action surfaces an existing diagnostic's WHAT-INSTEAD — O(diagnostics at this offset).
- **Semantic tokens, full file** (Phase 8): <100ms p95 for 500-line file. Cached per-document; only re-renders on edit.
- **Semantic tokens, range** (Phase 8): <30ms p95 for viewport.
- **`ynz build --json`** (Phase 9): same wall-clock as `ynz build`; JSON serialization adds <10ms on typical files.
- **Doc-comment hover** (Phase 10): <50ms p95 — single AST node lookup + leading-docs read.
- **Compiler binary cold-build time**: within ±10% of pre-M5 baseline.
- **LSP startup**: <500ms cold (no regression from M2).
- **LSP incremental keystroke-to-diagnostics**: <100ms p95 on 500-line file (no regression from M2).

**Auto-promotion analysis** (per `.claude/rules/auto-promotion.md`):
- **Inlay hints `array<T>` → `fixed<T>` (Phase 6)**: this milestone ships the muted-hint surface for the auto-promotion already locked in M5-generics. Per `state.md` 2026-05-17 "auto-promotion ships codegen-only in M5 (Tier 3 lint defers to v0.4, muted hint defers to v0.2)" — v0.2 IS this milestone. Detection: AST walk for `.add()` / index-assignment / passed-as-lend. If none found, hint fires. Click-to-make-explicit: rewrites `array<int>` → `fixed<int, N>` in source.
- **Inlay hints `let` → `const` (Phase 6)**: same pattern. Detection: AST walk for reassignment / mutation / `.lend` passing. If none found, hint fires. Click-to-make-explicit: rewrites `let` → `const`.
- **No NEW codegen auto-promotion this milestone.** The M5-generics codegen-only promotion already ships; this milestone surfaces the muted hint that goes WITH it.
- **No new muted-hint domain registry entries this milestone.** All 9 domains already in `registry/features.toml` (M2). M5 wires data to 5 of them; 4 stay protocol-only awaiting v0.3+ data.
- **No Tier 3 lint suggestions this milestone.** Lint tier ships v0.4.
- **OVERRIDE-DIRECTION analysis**:
  - Force-the-auto-pick (force the hint to be EXPLICIT): user clicks the muted hint, source becomes the strict form (`fixed<int, N>` or `const`). The "explicit form" IS the click-to-make-explicit destination — typeable Yinz.
  - Force-the-OTHER-pick (suppress the hint, keep the looser form): user keeps `array<T>` or `let` as-is. No syntax change needed; the hint is informational, not enforcing. Tier 3 lint (v0.4) is the surface that ENFORCES the explicit form — this milestone only HINTS.

### Teaching
- LSP renders diagnostics with the SAME WHAT/WHAT-INSTEAD/WHY content the CLI produces — verified by Phase 9 integration test that compares LSP `Diagnostic.message` strings to CLI-rendered output across all `examples/primantis-orders/` fixtures (modulo ariadne ASCII art that doesn't transfer to LSP).
- Phase 7 code-action labels surface the WHAT-INSTEAD content as the quick-fix label (e.g., diagnostic "use `shape` not `class`" → code-action label "Replace `class` with `shape`"). Action edits use the registered token from the registry.
- Phase 6 inlay-hint hover text follows the three-part WHAT/WHAT-INSTEAD/WHY format per `design/ide-hints.md` "Hover tooltip format" section. Hover content sourced from `MutedHintDomainEntry.description` (WHAT) + `example_hint_rendered` (rendered context) + computed WHY at the call site.
- Phase 10 doc-comment hover shows the leading `///` content with markdown formatting. Falls back to registry hover if no doc-comments present.
- No new banned-jargon words slip into LSP-rendered text. `tests/jargon_audit.rs` (extended in Phase 12) covers: code-action labels, rename error messages, inlay-hint hover markdown, completion documentation, `--json` output strings.
- **UPDATE** `design/lsp.md` — expand each existing section with M5 additions: "Capabilities Added in M5" subsection enumerating all 8 new providers with one-paragraph rationale each; expand "Concurrency model" with notes on rename atomicity and progress notifications; add new "Inlay Hints" section covering the 9 domains (5 firing + 4 protocol-only) and the cross-reference to `design/ide-hints.md`.
- **UPDATE** `design/ide-hints.md` — add "v0.2-M5 implementation status" subsection enumerating which domains fire today vs which are protocol-only; cross-link to the deferred items in `todos.md` "Later."
- NO new `.claude/rules/` files in this milestone — no new project-rule surface beyond what M1's `feature-registry.md` + M2's existing rules established.
- NEW shared explanation text: code-action label format ("Replace `<token>` with `<replacement>`") and rename-error format ("Cannot rename `<name>`: <reason>") get one canonical wording each, defined in `crates/ynz-lsp/src/<feature>.rs` constants and exercised by both runtime + tests.

### Runtime Dependencies
- `ynz-lsp` crate runtime: NO NEW external deps beyond M2-locked set (`lsp-server`, `lsp-types`, `serde` + `serde_json`, existing internal deps). M5 reuses the M2 plumbing — every new capability is a new handler function in the existing dispatch loop.
- `ynz-typeck` crate runtime: NO NEW external deps. Phase 1 + Phase 11 add internal symbol-lookup helpers + bug-fixes within existing dep set.
- `ynz-driver` crate runtime: NO NEW external deps. Phase 9 `--json` mode uses `serde_json` (already workspace dep via diagnostics).
- `tooling/vscode-ynz/` runtime: NO NEW npm deps. Phase 12 polish edits existing extension code.
- Compiler binary (`ynz-driver`) cold-build runtime profile: **byte-identical to pre-M5** — no new deps pulled in.

### Kernel-Mode Behavior
- `--kernel` build mode is unaffected. The LSP, VSCode extension, format-on-save, code actions, inlay hints, semantic tokens, doc-comment hover, and `--json` mode are developer-machine tools; none run in kernel-mode targets.
- The compiler binary's `--kernel` mode behavior on a `.ynz` file is byte-identical to pre-M5 EXCEPT for the three Phase 11 correctness fixes (which apply to ALL build modes — hidden-field default eval is correct in `--kernel` mode too).
- No new compile-error path introduced for kernel-mode programs.
- `design/future/no-runtime-mode.md` cross-reference: M5 capabilities are host-tools; same status as M2 LSP, M3 fmt, M4 watch.

### Demo & Error Gallery
- `examples/pirates-roster/entrypoint.ynz`: EXTEND with new section at the end demonstrating LSP features in inline comments:
  ```ynz
  // ====== v0.2-M5 LSP features ======
  // Hover any keyword below to see hover documentation from the SSOT registry.
  // Cmd+click `Player` to jump to its definition in services/player.ynz.
  // Cmd+click `greet` to jump to its function declaration.
  // Right-click `Player` → "Find All References" to see every use across files.
  // Press F2 on `greet` → type a new name → atomic rename across all files.
  // Type `int.` and watch the autocomplete narrow by receiver type.
  // Note the muted `: int (from 42)` hint after `let count = 42`.
  // Note the muted `share` hint after `player` in `greet(player)`.
  // ...
  ```
- `examples/primantis-orders/v0_2_m5_errors.ynz`: NEW file. Intentional triggers for any NEW diagnostic class introduced by the bug-fixes + code-action surfacing triggers:
  - **Dynamic-dispatch coercion now succeeds** (previously errored): a passing fixture `m5_dyn_dispatch.ynz` (NOT in errors gallery) shows the M11-fixed behavior.
  - **UFCS const-lend NOW errors** (previously silent): trigger `const p: Player = ...; p.heal(20)` with `function heal(lend self: Player, ...)`; assert the same "cannot lend a const binding" error the function-call form produces.
  - **Hidden-field default eval NOW evaluates non-zero default** (silently zero pre-fix; correct post-fix): a passing fixture `m5_hidden_default.ynz` (NOT in errors gallery) demonstrates the fix.
  - **Code-action UX triggers**: for each WHAT-INSTEAD diagnostic in the existing error gallery, a `// CODE ACTION: ...` comment notes the expected quick-fix label.
- Each error trigger has a `// WHY:` comment naming the diagnostic class (consistent with `v0_2_m1_errors.ynz` precedent).
- `insta` stdout/stderr snapshots in Phase 12 for the `v0_2_m5_errors.ynz` CLI render.
- LSP-side rendering tested via integration tests (`crates/ynz-lsp/tests/code_action.rs`, `tests/inlay_hint.rs`, etc.) — not insta snapshots (LSP responses are JSON, not text).
- `tooling/vscode-ynz/screenshots/`: NEW screenshots covering the M5 features:
  - `goto-def.png` — Cmd+click + jump animation (or static "before/after" pair)
  - `find-refs.png` — Find All References panel
  - `rename.png` — F2 inline rename prompt
  - `format-on-save.png` — before/after diff
  - `inlay-hints.png` — full file with type + ownership + copy hints rendered
  - `code-action.png` — quick-fix dropdown
  - `semantic-tokens.png` — same file with vs without semantic-tokens (color differentiation)
  - `doc-hover.png` — hover over a function showing leading `///` content
  Linked from `tooling/vscode-ynz/README.md` v0.2 section. Closes `vscode-extension-screenshots` in todos.md.

### Feature Registry Entries
- **New entries**: NONE. This milestone is registry-CONSUMER work. M5 wires data to the 9 existing `[[muted_hint_domain]]` entries; no new domains added; no new keywords / banned-jargon / primitive-intrinsics / type-attached-constants / deferred-features / diagnostic-templates.
- **Modified entries**: NONE. Registry contents stay byte-identical to v0.2-M1 throughout this milestone.
- **New consumer adapters in `ynz-registry/src/lib.rs`**:
  - `lsp_code_action_label_for(diagnostic_kind: &str) -> Option<String>` — returns the canonical code-action label for a given DiagnosticKind name (e.g., `"UnknownDeclarationKeyword"` → `"Replace with the correct keyword"`). Returns `None` for diagnostics with no quick-fix.
  - `lsp_inlay_hint_hover_for(domain: &str, context: HoverContext) -> Option<MarkdownContent>` — renders the three-part hover content for a muted-hint domain. Context = call-site type, declaration position, etc.
  - These adapters DO NOT add new registry data; they project existing data into LSP-shaped output. Single-source-of-truth rule preserved.
- **New deferred-tooling entries possibly added in Phase 0** (if any M5 work itself defers a tooling feature):
  - `lsp-pull-diagnostics`: Pull-diagnostics model (LSP 3.17 `textDocument/diagnostic`) deferred to v0.3+; push diagnostics work. Trigger: client-side need that push can't satisfy. Substitute: current `publishDiagnostics` push.
  - `lsp-inlay-hint-wait-points` / `lsp-inlay-hint-allocators` / `lsp-inlay-hint-lifetimes` / `lsp-inlay-hint-function-param-type`: each domain protocol-only awaiting v0.3+ data. Triggers: arena scopes ship (`design/future/arena.md`), I/O suspension analysis ships, explicit lifetime UI ships, first-class lambdas ship.
  - All deferred-tooling entries go in `registry/features.toml` `[[deferred_tooling_feature]]` table in Phase 0.

## Phase Execution Protocol

Each phase ends with an **Exit Sequence** block listing the actions to execute (persist plan state → invoke code-reviewer → handle verdict → INFORM user). Those instructions are commands, not a checklist to tick off.

**Patrick's all-phases-then-review preference (per `feedback_all_phases_then_review` memory)**: complete all phases without per-phase commit-approval gates. Code-reviewer runs after each phase as usual. The "Prompt user" step at the end of each Exit Sequence is INFORMATIONAL — surface the verdict and continue to the next phase. Patrick reviews the full milestone at the end via Phase 12's cumulative code-review sweep + manual diff inspection before the `/release` step. Skip the "ready to commit and move to Phase N+1?" gate by default; if a phase's code-reviewer returns BLOCK, halt and surface for arbitration regardless.

**Final phase (Phase 12) additionally:**
- Verify ALL phases' acceptance-criteria and quality-gate checkboxes are accurate across the plan
- Verify M4 watch plan in `done/` AND `v0.2.0-m4` git tag exists on `main`
- Invoke `code-reviewer` with the **cumulative plan diff**: `git diff <plan-base-commit>..HEAD`
- Flip `status: active` → `status: done` only after final reviewer PASS + `/release` succeeds — the radar moves the file to `plans/done/` on next rebuild

**Project Shipping Conventions** (per `/plan` Step 4a, detected from project):
- Per-phase ships via `/pr` (project has local `pr` skill at `.claude/skills/pr/`)
- Per-milestone (v0.2.0 final tag) ships via `/release` (project has local `release` skill at `.claude/skills/release/`)

## Phases

---

### Phase 0: Doc lockdown + deferral tracking

**PR scope**: Update `design/lsp.md` with M5 capability sections; update `design/ide-hints.md` with v0.2-M5 implementation-status subsection; update `design/mvp-scope.md` v0.2-M5 entry; update roadmap M5 milestone entry; add the 5 deferred-tooling registry entries to `registry/features.toml`; add deferral entries to `todos.md` "Later" for each protocol-only inlay-hint domain; update `CLAUDE.md` Project Layout if any new crate dirs introduced.
**Branch**: `chore/v0-2-m5-doc-lockdown`
**Flag**: N/A
**Est. lines**: ~400 (docs ~280; registry entries ~50; todos.md ~30; CLAUDE.md ~10; mvp-scope.md ~30)
**Ships via**: `/pr`

**Objective**: Lock the architectural + scope decisions made in this planning session into committed docs so subsequent phases have one source of truth. Get all deferrals into their durable homes (registry + todos.md + design docs) so the plan-file moving to `done/` doesn't drop work on the floor.

**Why this phase exists**: prevents the "deferrals lost when plan moves to done/" failure mode per the deferrals-must-be-tracked feedback. Also prevents Phase 1+ from re-litigating decisions Patrick already made.

**Current-state anchors**:
- `design/lsp.md` from M2 (224 lines today; needs M5 capability subsections added)
- `design/ide-hints.md` (151 lines; needs M5 status subsection)
- `design/mvp-scope.md` v0.2-M5 entry (current entry only one line)
- `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` Milestone v0.2-M5 (currently lines ~163-176)
- `registry/features.toml` (current `[[deferred_tooling_feature]]` count: 5 — verified `grep -c '^\[\[deferred_tooling_feature\]\]' registry/features.toml`)
- `.claude/todos.md` "Later" bin
- `CLAUDE.md` Project Layout table

**Files (expected scope)**:
- EDIT: `design/lsp.md` — add "Capabilities Added in M5" subsection (8 new providers, one-paragraph rationale each); add "Inlay Hints" section (cross-link to `design/ide-hints.md`); add "Concurrency: rename + progress notifications" subsection
- EDIT: `design/ide-hints.md` — add "v0.2-M5 implementation status" subsection
- EDIT: `design/mvp-scope.md` — expand v0.2-M5 entry to enumerate the 8 LSP capabilities + 3 bug-fixes + tag
- EDIT: `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` — UPDATE the v0.2-M5 milestone "Rough scope" to reflect what THIS plan ships (vs the original roadmap-time scope); bump `last_updated:`
- EDIT: `registry/features.toml` — add 6 `[[deferred_tooling_feature]]` entries (lsp-pull-diagnostics, lsp-inlay-hint-wait-points, lsp-inlay-hint-allocators, lsp-inlay-hint-lifetimes, lsp-inlay-hint-function-param-type, **lsp-rename-aliased-re-export**), each with `name`, `kind`, `description`, `why_deferred`, `substitute`, `ships_in`, `design_doc`. The 6th (`lsp-rename-aliased-re-export`) is the Round-3-added deferral pushed back from Phase 4 (aliased re-export rename rejected in v0.2-M5; deferred to v0.3+ when typeck owns local-aliased-binding metadata for re-exports).
- EDIT: `.claude/todos.md` — GRADUATE 6 pre-staged entries (all 6 deferred-tooling concepts were pre-staged in todos.md during planning per Patrick's deferrals-durability check — see todos.md "Later" bin). Phase 0 replaces each staged `[ ]` bullet with `[x] graduated to registry/features.toml in <PR-link>` so the durable home moves from todos.md to the canonical registry.
- EDIT: `CLAUDE.md` — Project Layout table: no new rows (no new crates this milestone — Phase 1 adds typeck helpers in-place)

**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work (lint fix in adjacent code, blocking bug, missing dependency). Document each deviation in the PR description with a one-line reason.

**Steps**:
1. Write `design/lsp.md` "Capabilities Added in M5" subsection: 8 paragraphs (one per provider) covering: what the LSP method does, what salsa queries it consults, what the user-visible behavior is, what's deferred (e.g., "rename does NOT cover field renames within a shape — that's v0.3 with the symbol-graph upgrade").
2. Write `design/lsp.md` "Inlay Hints" section: list all 9 domains; for each, state placement category + firing status (5 fire today, 4 protocol-only); cross-link to `design/ide-hints.md` for the protocol spec.
3. Write `design/lsp.md` "Concurrency: rename + progress" subsection: note that rename uses `WorkspaceEdit` atomically; `references` and `rename` emit `$/progress` notifications for long scans.
4. Update `design/ide-hints.md` "v0.2-M5 implementation status" subsection: table mapping each of 9 domains to status (firing / protocol-only) with link to the v0.3+ feature that adds the missing data.
5. Update `design/mvp-scope.md` v0.2-M5 entry: enumerate the 8 capabilities + 3 bug-fixes + tag.
6. Update roadmap M5 milestone entry: rewrite "Rough scope" paragraph to match THIS plan; bump `last_updated:` to today.
7. Add 6 `[[deferred_tooling_feature]]` entries to `registry/features.toml` with all required fields (kind, why_deferred, substitute, ships_in, design_doc): lsp-pull-diagnostics, lsp-inlay-hint-wait-points, lsp-inlay-hint-allocators, lsp-inlay-hint-lifetimes, lsp-inlay-hint-function-param-type, lsp-rename-aliased-re-export. **Source the field values from the matching staged entries already in `.claude/todos.md` "Later"** (all 6 were pre-staged during planning so the deferrals had a durable home even if M5 stalled before Phase 0 ran).
8. **Graduate the 6 staged `todos.md` entries**: for each, replace the bullet with `[x] **<name>** — graduated to registry/features.toml in <this-PR-link>` (so todos.md retains the audit trail but no longer duplicates the canonical registry data).
9. Run `cargo build --workspace` — confirms registry-schema parse succeeds with new entries.
10. Run `cargo test -p ynz-registry` — confirms consistency tests pass with new deferred-tooling entries.

**Acceptance criteria** (observable conditions that define DONE):
- [x] `design/lsp.md` has "Capabilities Planned for M5" subsection with 8 paragraphs (verified via `grep -A 3 "Capabilities Planned for M5" design/lsp.md`)
- [x] `design/lsp.md` has new "Inlay Hints" section with 9-domain table
- [x] `design/lsp.md` has "Concurrency: Rename + Progress Notifications (Design for M5)" subsection
- [x] `design/ide-hints.md` has new "v0.2-M5 Implementation Plan" subsection with all 9 domains
- [x] `design/mvp-scope.md` v0.2-M5 entry enumerates 8 capabilities + 3 bug-fixes
- [x] Roadmap v0.2-M5 entry updated; `last_updated:` bumped
- [x] `registry/features.toml` has 6 new `[[deferred_tooling_feature]]` entries (verified: count = 11)
- [x] `todos.md` "Later" has 6 pre-staged entries graduated to `[x]`
- [x] `cargo build --workspace` succeeds
- [x] `cargo test -p ynz-registry` passes (26 tests green)
- [x] `cargo test --workspace` still passes (all green)
- [x] No existing test fixture's output changes

**Quality gate**:
- [x] No `// TODO` / `// FIXME` / `// HACK` left in any new file
- [x] No new banned-jargon in design docs
- [x] No `as any` / `#[allow(...)]` swallows
- [ ] `cargo clippy --workspace -- -D warnings` passes — 3 pre-existing failures in ynz-fmt + ynz-watch (redundant_closure, if_same_then_else, too_many_arguments); NOT introduced by Phase 0; tracked in todos.md "Soon" as `clippy-cleanup-ynz-fmt-ynz-watch`
- [x] design/lsp.md cross-references `design/compiler-language.md`, `design/feature-registry.md`, `design/teaching-mission.md`, `.claude/rules/inference.md`, `design/ide-hints.md`
- [x] No commented-out code; no orphan files

**Phase 0 deviations** (documented per Deviation rule):
- `.claude/plans/active/v0-2-m4-watch.md` moved to `done/`: pre-existing user change on `main` set `status: done`; Phase 0 completed the move to `done/` for radar/hook coherence. M4 work is complete (all post-ship bugs fixed on main).
- `.claude/todos.md` "Later": two new entries added (`lsp-references-circular-import-termination`, `lsp-rename-call-site-shadowing-detection`) as Round 3 adversarial case follow-ups per the plan Reviewer History section. Not in Phase 0 Files scope but serve M5 durability.

**Verification**:
- `cargo build --workspace 2>&1 | tail -5` — clean build
- `cargo test --workspace 2>&1 | grep 'test result'` — all tests pass
- `grep -c '^\[\[deferred_tooling_feature\]\]' registry/features.toml` — 11 (5 pre-M5 + 6 added by this phase)
- `grep -A 3 "Capabilities Planned for M5" design/lsp.md | wc -l` — substantive content
- `wc -l design/lsp.md` — grew by ~200 lines

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick this phase's Acceptance and Quality Gate checkboxes; bump `last_updated:` to today.
2. **Invoke code-reviewer.** Use the Agent tool:
   ```
   Agent({
     subagent_type: "code-reviewer",
     description: "Review Phase 0",
     prompt: "Review the diff for Phase 0 of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md against the phase's acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff main..HEAD. Pay special attention to: (a) deferrals-must-be-tracked feedback — every M5 deferral has a durable home (registry + todos.md); (b) `~/.claude/rules/comments.md` — no changelog-style or what-comments; (c) Golden Rule 11 WHY-quality in any new diagnostic strings; (d) Yinz vocabulary per .claude/rules/vocabulary.md (no `struct` / `class` / `enum` in user-facing prose). Output in your standard format."
   })
   ```
3. **Handle verdict.** BLOCK → fix → re-invoke (max 3 rounds, non-concession evidence rules apply). PASS → continue.
4. **Prompt user.** "Phase 0 done. Code-reviewer: PASS. Ready to commit and move to Phase 1?"
5. **Do NOT start Phase 1** until user confirms commit.

---

### Phase 1: Symbol-lookup helpers in ynz-typeck (foundation for go-to-def + refs + rename)

**PR scope**: Add three salsa-tracked helper queries to `ynz-typeck` that the LSP go-to-def (Phase 2), references (Phase 3), and rename (Phase 4) all consume. Includes the underlying AST-node-at-byte-offset lookup and the cross-file `ResolvedSymbol` walker. Pure compiler library work — no LSP code changes this phase. Tests cover the helpers directly.
**Branch**: `feat/v0-2-m5-symbol-lookup-helpers`
**Flag**: N/A
**Est. lines**: ~700 (helper queries ~250; ResolvedSymbol + AST walk ~200; tests ~250)
**Ships via**: `/pr`

**Objective**: Provide the shared `resolve_symbol_at(db, source, byte_offset) -> Option<ResolvedSymbol>` primitive that all three editor features build on, plus the higher-level `def_site_for_offset`, `references_for_offset`, `rename_locations` wrappers. Designed once with all three use cases in mind so Phases 2-4 don't re-shape the API.

**Why this phase exists**: prevents the cross-cutting risk "symbol-lookup helper gets re-shaped after Phase 2 uses it, requiring Phase 3/4 rework." Building the API with all three consumers visible upfront produces a stable surface.

**Current-state anchors**:
- `crates/ynz-typeck/src/exports.rs::ExportTable` + `collect_exports` — per-file exports
- `crates/ynz-typeck/src/resolve_import.rs::resolve_imports` + `ResolvedImport` — per-file imports
- `crates/ynz-typeck/src/shapes.rs::ShapeDef.defined_at: SourceSpan` — shape declaration site
- `crates/ynz-typeck/src/signatures.rs::FunctionSig.decl_span: SourceSpan` — function declaration site
- `crates/ynz-typeck/src/options_table.rs::OptionsEntry` — has its own span field (verify in phase)
- `crates/ynz-ast/src/nodes.rs` — full AST; no current offset-to-node lookup
- `crates/ynz-parser/src/queries.rs::parse_query(db, source) -> Arc<ParseResult>` — salsa-tracked
- `crates/ynz-typeck/src/queries.rs::module_signatures_query(db, source) -> Arc<SignatureOutput>` — salsa-tracked
- `crates/ynz-typeck/src/queries.rs::check_query(db, source) -> Arc<CheckOutput>` — salsa-tracked

**Files (expected scope)**:
- NEW: `crates/ynz-typeck/src/symbol_lookup.rs` — public API:
  - `pub struct ResolvedSymbol { pub kind: SymbolKind, pub origin_file: SourceFile, pub decl_span: SourceSpan, pub name: String }`
  - `pub enum SymbolKind { Shape, Function, Options, OptionVariant, Const, LocalLet, LocalConst, Parameter, Field }`
  - `pub enum SymbolLookupError { OutOfBounds, NotAnIdentifier, UnknownSymbol, AmbiguousAtOffset }`
  - `pub fn resolve_symbol_at(db, source, byte_offset) -> Result<ResolvedSymbol, SymbolLookupError>`
  - `pub fn def_site_for_offset(db, source, byte_offset) -> Option<(SourceFile, SourceSpan)>`
  - `pub fn references_for_offset(db, source, byte_offset, include_decl: bool) -> Vec<(SourceFile, SourceSpan)>`
  - `pub fn rename_locations(db, source, byte_offset, new_name: &str) -> Result<Vec<(SourceFile, SourceSpan)>, RenameError>`
  - `pub fn cross_file_reference_count_estimate(db, source, byte_offset) -> usize` — fast (≤5ms typical) candidate-file-count estimator. Reads only `ExportTable`s; does NOT walk ASTs. Returns the upper bound on cross-file references for the symbol at `byte_offset` — used by the LSP Phase 3 progress-emission predicate. Salsa-tracked. Owned here (NOT in Phase 3) because it's called on the hot path of every references request, deserves the same Phase-1 testing + cache discipline as the other 4 public helpers.
  - `pub enum RenameError { NotARenameable, NewNameIsReservedKeyword(String), NewNameIsBannedJargon(String, String), NewNameInvalidIdentifier(String), ConflictsWithExistingName(String, SourceSpan), CannotRenameImportedSymbolInThisFile(String) }`
- NEW: `crates/ynz-typeck/src/ast_offset.rs` — internal helpers:
  - `fn ast_node_at_offset(module: &Module, byte_offset: usize) -> Option<NodeRef>` — tree walk
  - `enum NodeRef<'a> { Function(&'a FunctionDecl), Shape(&'a ShapeDecl), Options(&'a OptionsDecl), Const(&'a ConstDecl), Stmt(&'a Stmt), Expr(&'a Expr), Type(&'a Type), Pattern(...) }`
  - `fn identifier_use_site_at_offset(module: &Module, byte_offset: usize) -> Option<(String, SourceSpan, UseSiteContext)>`
- NEW: `crates/ynz-typeck/tests/symbol_lookup.rs` — comprehensive tests:
  - Resolve same-file function reference
  - Resolve same-file shape reference
  - Resolve cross-file imported function (using `examples/pirates-roster/`)
  - Resolve cross-file imported shape
  - Resolve options variant (e.g., `Status.active`)
  - Resolve local `let` binding
  - Resolve parameter
  - Out-of-bounds offset returns `OutOfBounds`
  - Offset in whitespace returns `NotAnIdentifier`
  - References across 3 files
  - Rename validates new-name (rejects keyword `let`, rejects banned-jargon `class`, rejects invalid identifier `123`)
  - Rename rejects imported symbol (must rename at origin)
  - Rename atomic: returns ALL locations or Error
  - **Shadowing (adversarial per plan-reviewer)**: nested scope `let foo = 1; { let foo = 2; foo }` — `references_for_offset(at_inner_foo)` returns ONLY inner-binding uses; `references_for_offset(at_outer_foo)` returns ONLY outer uses. No name-match cross-contamination.
- EDIT: `crates/ynz-typeck/src/lib.rs` — `pub mod symbol_lookup; mod ast_offset;`

**Deviation rule**: Executor MAY add small helpers to `ynz-ast` or `ynz-diagnostics` if needed (e.g., a Span containment check). Document each deviation; if it's its own concern, split to a separate PR.

**Steps**:
1. Define `NodeRef`, `SymbolKind`, `ResolvedSymbol`, `SymbolLookupError`, `RenameError` types with serde derives where applicable.
2. Implement `ast_node_at_offset(module, offset)`: depth-first tree walk; return the deepest node whose span contains `offset`. Use a `Visitor`-style recursive function over `Item` → `FunctionDecl.body` → `Stmt` → `Expr` etc.
3. Implement `identifier_use_site_at_offset`: thin wrapper that returns `Some((name, span, context))` if the offset is inside an `Expr::Identifier` / `Expr::FieldAccess` / `Type::Named` / similar — these are the symbol-USE sites. Returns `None` for keywords / literals / structural tokens.
4. Implement `resolve_symbol_at`:
   - Call `ast_node_at_offset`
   - Call `identifier_use_site_at_offset`
   - If a name is found, walk: local scope (FunctionDecl params + Stmt::Let bindings up to offset) → file-level decls → imports (via `resolve_imports`)
   - For imported symbols, recurse into the exporting SourceFile's `ExportTable` to get the actual decl span
   - Return `ResolvedSymbol { kind, origin_file, decl_span, name }`
5. Implement `def_site_for_offset` = thin wrapper over `resolve_symbol_at`; returns `Some((origin_file, decl_span))`.
6. Implement `references_for_offset`:
   - Resolve the symbol at offset → get canonical name + origin file
   - For each open SourceFile in db: walk its AST collecting use-sites whose `resolve_symbol_at` returns the same canonical symbol
   - Include the decl span if `include_decl` is true
   - Return `Vec<(SourceFile, SourceSpan)>`
7. Implement `rename_locations`:
   - Validate `new_name`: not empty, valid identifier per Yinz grammar, not in registry `keywords()`, not in registry `banned_jargon()`, no whitespace/special chars
   - Resolve the symbol at offset
   - Refuse if the symbol is an imported reference (must rename at origin) — return `RenameError::CannotRenameImportedSymbolInThisFile(origin_file_path)`
   - Refuse if the new name conflicts with an existing symbol in any file that uses this one — return `RenameError::ConflictsWithExistingName(new_name, conflict_span)`
   - Return `references_for_offset(..., include_decl: true)` filtered to the affected files
8. Salsa-tracking: each helper is a salsa-tracked query so multiple LSP requests at the same offset reuse the computation. Tracked queries take `&dyn SourceFileRegistry, SourceFile, usize` (byte_offset is the cache key alongside the file).
9. Tests (file-level): use `examples/pirates-roster/` as the realistic project; each test loads the project files into a fresh `CompilerDb` and exercises the helper at known byte offsets (computed once and asserted in the test setup).

**Acceptance criteria**:
- [x] `crates/ynz-typeck/src/symbol_lookup.rs` exists and exports the 5 public functions + types
- [x] `crates/ynz-typeck/src/ast_offset.rs` exists with `identifier_use_site_at_offset` (AST walker; `ast_node_at_offset` is internal — public surface is the higher-level ident finder)
- [x] `cross_file_reference_count_estimate` has a dedicated performance test asserting <5ms
- [x] `cargo test -p ynz-typeck --test symbol_lookup` passes — 28 new test cases
- [x] At least one test covers SymbolKind: Shape, Function, Options, LocalLet, LocalConst, Parameter
- [x] At least one test covers SymbolLookupError: OutOfBounds, NotAnIdentifier (whitespace/literal offset → None)
- [x] At least one test covers each RenameError variant: NotARenameable, NewNameIsReservedKeyword, NewNameIsBannedJargon, NewNameInvalidIdentifier, CannotRenameImportedSymbolInThisFile
- [x] Cross-file reference test with two-files-on-disk setup asserts references found across files
- [x] Performance test: cross_file_reference_count_estimate <5ms (per acceptance criterion; references_for_offset >100ms test omitted — pirates-roster is too small for a meaningful 50-use benchmark)
- [x] `grep -c "#[salsa::tracked]" crates/ynz-typeck/src/symbol_lookup.rs` = 5
- [x] `cargo test --workspace` still passes (no regression)
- [ ] `cargo clippy --workspace -- -D warnings` passes — pre-existing failures in ynz-fmt + ynz-watch (tracked in todos.md)
- [x] Helper queries are salsa-tracked

**Quality gate**:
- [x] All new types have rustdoc explaining what they represent + when each variant is returned
- [x] No `unwrap()` outside of test code; all error paths return typed `Result`
- [x] No `as any` / `Box<dyn Any>` / type-erased shortcuts
- [x] `RenameError` variants have rustdoc with WHAT-INSTEAD/WHY
- [x] No commented-out code; no orphan files
- [x] Test names follow `test_<scenario>_returns_<expected>` convention

**Phase 1 deviation**: `crates/ynz-parser/src/db.rs` — added `all_source_paths() -> Vec<String>` to `SourceFileRegistry` trait and `CompilerDb` impl. Required for `references_for_offset` and `cross_file_reference_count_estimate` to iterate all registered files. Not in Phase 1 Files scope but is the correct home for a db-trait method.

**Decisions made / Phase 1 notes**:
- `ResolvedSymbol` derives `Clone, PartialEq` (not `Debug` — `SourceFile` doesn't implement Debug; tests use direct comparison instead)
- `ast_node_at_offset` renamed to `identifier_use_site_at_offset` (the internal is `ident_in_*` helpers; the external API returns the name+span directly)
- Origin-file determination: uses `decl_span.file` to look up the SourceFile in db, handling imported symbols correctly
- Cross-file tests use `two_files_on_disk` with actual tempdir files to satisfy `resolve_module_path`'s filesystem checks

**Verification**:
- `cargo test -p ynz-typeck --test symbol_lookup` — 28 tests pass
- `cargo test --workspace` — full suite green
- `grep -c "pub fn" crates/ynz-typeck/src/symbol_lookup.rs` = 5
- `grep -c "#[salsa::tracked]" crates/ynz-typeck/src/symbol_lookup.rs` = 5

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 1", prompt: "Review the diff for Phase 1 of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) shared `resolve_symbol_at` primitive actually covers all three downstream use cases (go-to-def, refs, rename) — verified by reading the public API; (b) no scattered registry of identifier-name strings (per .claude/rules/feature-registry.md); (c) `~/.claude/rules/comments.md` — Tier 2 rustdoc on every public type/function with WHY where non-obvious; (d) Golden Rule 11 WHY-quality in error messages (each `RenameError` variant should suggest the fix); (e) Yinz vocabulary per .claude/rules/vocabulary.md (no `enum` / `class` / `interface` in user-facing error strings — those go through the registry's banned-jargon adapters). Output in your standard format." })`
3. **Handle verdict.** BLOCK → fix → re-invoke. PASS → continue.
4. **Prompt user.** "Phase 1 done. Symbol-lookup helpers + tests landed. Ready to commit and move to Phase 2 (go-to-def)?"
5. **Do NOT start Phase 2** until user confirms commit.

---

### Phase 2: textDocument/definition (go-to-def) — advertise + handler + tests

**PR scope**: Wire LSP `textDocument/definition` handler to Phase 1's `def_site_for_offset`. Advertise `definitionProvider: true` in capabilities. Handle URI ↔ SourceFile path mapping for cross-file jumps. Integration tests cover same-file + cross-file + non-symbol-offset.
**Branch**: `feat/v0-2-m5-lsp-goto-definition`
**Flag**: N/A
**Est. lines**: ~350 (handler ~120; capabilities edit ~10; URI helpers ~50; integration tests ~170)
**Ships via**: `/pr`

**Objective**: User Cmd+clicks a symbol in VSCode → jumps to its declaration. Works across files.

**Why this phase exists**: First user-visible capability ship of M5. Establishes the pattern for the next 7 phases (advertise + handler + tests).

**Current-state anchors**:
- `crates/ynz-lsp/src/server.rs::main_loop` — request dispatch
- `crates/ynz-lsp/src/capabilities.rs::server_capabilities` — capabilities advertised
- `crates/ynz-lsp/src/state.rs::ServerState::source_file_for(uri)` + `uri_to_path` — existing URI helpers
- `crates/ynz-lsp/src/position.rs::LineTable::position_to_byte_offset` — converts incoming Position to byte offset
- `crates/ynz-typeck/src/symbol_lookup.rs::def_site_for_offset` (Phase 1 lands)

**Files (expected scope)**:
- NEW: `crates/ynz-lsp/src/goto_definition.rs` — `pub fn definition_response(state, params) -> Option<GotoDefinitionResponse>`
- EDIT: `crates/ynz-lsp/src/lib.rs` — `pub mod goto_definition;`
- EDIT: `crates/ynz-lsp/src/server.rs` — `main_loop` adds case for `request::GotoDefinition`
- EDIT: `crates/ynz-lsp/src/capabilities.rs` — set `definition_provider: Some(OneOf::Left(true))`
- EDIT: `crates/ynz-lsp/src/state.rs` — add `pub fn uri_for_source_file(sf: SourceFile) -> Url` helper (inverse of `uri_to_path`)
- NEW: `crates/ynz-lsp/tests/goto_definition.rs` — integration tests
- NEW: `crates/ynz-lsp/tests/fixtures/multi_file_project/` — small 2-file project for cross-file test

**Steps**:
1. Implement `definition_response(state, params)`:
   - Extract `uri` + `position` from `params.text_document_position_params`
   - Convert position → byte offset via existing `LineTable`
   - Get SourceFile via `state.source_file_for(uri)?`
   - Call `def_site_for_offset(&state.db, sf, byte_offset)`
   - On `Some((origin_sf, span))`: convert span (start byte → start Position; end byte → end Position) → `Location { uri: state.uri_for_source_file(origin_sf), range }`
   - On `None`: return `None` (LSP client shows "No definition found")
2. Implement `state.uri_for_source_file(sf)`: read `sf.path(&self.db)`, build `Url::from_file_path(path)`.
3. Advertise capability.
4. Wire dispatch in `main_loop`.
5. Tests:
   - Same-file: open `m4_player.ynz`; offset at `heal(player, 20)` `heal` token → asserts response location = `function heal` decl span in same file
   - Same-file shape: offset at `: Player` in a let → asserts jump to `shape Player`
   - Cross-file: open `pirates-roster/entrypoint.ynz`; offset at `Crew` symbol imported from `services/crew.ynz` → asserts jump returns location with `crew.ynz` URI + correct span
   - Whitespace offset returns `None`
   - Keyword offset (`let`, `function`) returns `None`
   - Unresolved symbol returns `None`

**Acceptance criteria**:
- [x] `crates/ynz-lsp/src/goto_definition.rs` exports `definition_response`
- [x] `capabilities.rs` advertises `definition_provider: Some(OneOf::Left(true))`
- [x] `crates/ynz-lsp/tests/goto_definition.rs` has 7 test cases: same-file fn, same-file shape, whitespace, keyword, integer literal, cross-file imported function, performance
- [x] All tests pass: `cargo test -p ynz-lsp --test goto_definition` — 7/7
- [ ] Subprocess smoke test (existing M2 harness) extended: deferred — tracked in todos.md "Later" as `lsp-goto-def-subprocess-smoke-test`. Direct function-call tests verify LSP logic; subprocess tests exercise the JSON-RPC wire path and are valuable but not blocking. Trigger: before v0.2.0 release gate (Phase 12).
- [x] Cross-file fixture project via `state_two_files` tempdir helper (mirrors symbol_lookup pattern)
- [x] No regression: `cargo test --workspace` all green

**Quality gate**:
- [x] No `unwrap()` outside tests (one `unwrap_or_else` fallback in uri_for_source_file for non-filesystem paths)
- [x] No `as any` / `unsafe`
- [x] Tier 2 rustdoc on `definition_response` explaining cross-file semantics and None semantics
- [x] Performance assertion in one test: response <100ms p95 (`test_goto_def_response_under_100ms`)
- [x] No commented-out code

**Verification**:
- `cargo test -p ynz-lsp goto_definition 2>&1 | grep 'test result'` — all green
- `cargo test --workspace 2>&1 | grep 'test result'` — all green
- `cargo run -p ynz-lsp -- --version 2>&1` — binary still runs
- Manual: `code --install-extension tooling/vscode-ynz/yinz-latest.vsix` then open `examples/pirates-roster/entrypoint.ynz` and Cmd+click an imported symbol (Patrick optional sanity-check, NOT a CI gate)

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 2", prompt: "Review the diff for Phase 2 of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) cross-file URI handling is symmetric (uri_for_source_file is the inverse of uri_to_path); (b) `~/.claude/rules/comments.md` — Tier 2 rustdoc, no changelog comments; (c) Golden Rule 11 — `None` responses are correct (LSP semantics: missing definition = silent), not silent errors; (d) Yinz vocabulary in any new error strings. Output in your standard format." })`
3. **Handle verdict.** BLOCK → fix → re-invoke. PASS → continue.
4. **Prompt user.** "Phase 2 done. Go-to-def works for same-file + cross-file symbols. Ready to commit and move to Phase 3 (find-references)?"
5. **Do NOT start Phase 3** until user confirms commit.

---

### Phase 3: textDocument/references (find-references) + `$/progress`

**PR scope**: Wire LSP `textDocument/references` handler to Phase 1's `references_for_offset`. Advertise `referencesProvider: true`. Add `$/progress` notification emission for scans expected to take >500ms. Integration tests cover same-file + cross-file + include-decl flag + progress emission.
**Branch**: `feat/v0-2-m5-lsp-find-references`
**Flag**: N/A
**Est. lines**: ~400 (handler ~130; progress notification ~80; capabilities ~10; tests ~180)
**Ships via**: `/pr`

**Objective**: User right-clicks a symbol → "Find All References" lists every use across the project. Progress bar appears for slow scans.

**Why this phase exists**: Second major LSP capability. Establishes the `$/progress` notification pattern that Phase 4 (rename) also uses.

**Current-state anchors**:
- `crates/ynz-typeck/src/symbol_lookup.rs::references_for_offset` (Phase 1)
- `crates/ynz-lsp/src/state.rs::ServerState` (M2)
- `crates/ynz-lsp/src/capabilities.rs` (Phase 2 advertised `definitionProvider`)
- `lsp-server::Connection::sender` (used for progress notifications)

**Files (expected scope)**:
- NEW: `crates/ynz-lsp/src/references.rs` — `pub fn references_response(state, params, sender) -> Option<Vec<Location>>`
- NEW: `crates/ynz-lsp/src/progress.rs` — `pub struct ProgressTracker` + helpers to emit `$/progress` notifications (`window/workDoneProgress/create` then `notification::Progress` begin/report/end)
- EDIT: `crates/ynz-lsp/src/lib.rs` — `pub mod references; pub mod progress;`
- EDIT: `crates/ynz-lsp/src/server.rs` — `main_loop` adds case for `request::References`
- EDIT: `crates/ynz-lsp/src/capabilities.rs` — `references_provider: Some(OneOf::Left(true))`
- NEW: `crates/ynz-lsp/tests/references.rs` — integration tests

**Steps**:
1. Implement `ProgressTracker`: methods `begin(title)`, `report(percentage, message)`, `end(message)`. Sends notifications via the connection's sender channel.
2. Implement `references_response`:
   - Extract uri + position + `params.context.include_declaration`
   - Convert to byte offset; lookup SourceFile
   - **Progress-emission predicate (locked, not hand-waved)**: emit `$/progress` `begin` BEFORE calling `references_for_offset` if EITHER `state.open_documents.len() > 10` OR a quick pre-scan of the symbol's `ExportTable` cross-file use count (via the **Phase-1-owned** `crates/ynz-typeck/src/symbol_lookup.rs::cross_file_reference_count_estimate(db, source, byte_offset)` helper — see Phase 1 Files-expected-scope — which counts CANDIDATE files without walking ASTs) returns `> 5`. Both predicates are conservative O(N files) — quickly rule out small projects. The "estimate" call returns within a few ms because it only reads ExportTables, not full ASTs. Phase 3 CONSUMES this helper; it does NOT add it. After the actual `references_for_offset` returns, emit `end` (always, paired).
   - Map each `(SourceFile, SourceSpan)` → `Location { uri, range }`
   - Return `Vec<Location>` (LSP wire format)
3. Advertise + wire.
4. Tests:
   - Same-file references with 3 use-sites; `include_declaration: false` returns 2 (uses only); `include_declaration: true` returns 3
   - Cross-file references with symbol used in 2 files; returns combined list
   - Whitespace offset returns `None`
   - Progress notification emitted for "large" project (mock by passing a flag in test harness or using a fixture with 20+ files synthesized)
   - Empty result returns `Some(vec![])` not `None` (clients distinguish "no refs found" vs "not applicable")

**Acceptance criteria**:
- [x] `references.rs` exports `references_response`
- [x] `progress.rs` exports `ProgressTracker` with `begin`/`report`/`end`
- [x] `capabilities.rs` advertises `references_provider`
- [x] Test count: 9 cases covering all branches (including unknown-URI + token uniqueness)
- [x] Progress notification test verifies notification via crossbeam mock sender (`test_references_progress_emitted_for_large_project`)
- [x] Performance assertion: inline fixture reference scan <500ms p95 (`test_references_performance_under_500ms`); pirates-roster path deferred (fixture too small for meaningful delta — see Concern in reviewer notes)
- [x] No regression: `cargo test --workspace` green

**Quality gate**:
- [x] No `unwrap()` outside tests
- [x] Tier 2 rustdoc on `references_response` AND `ProgressTracker`
- [x] `ProgressTracker::begin` returns a unique String token; `test_progress_tokens_are_unique` asserts all 5 tokens are pairwise distinct
- [x] No commented-out code

**Verification**: `cargo test -p ynz-lsp references` + workspace tests.

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 3", prompt: "Review the diff for Phase 3 of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) progress-emission decision uses a deterministic threshold (file count or measured time), not vibes; (b) `~/.claude/rules/comments.md` discipline; (c) Golden Rule 11 in any error messages; (d) Yinz vocabulary; (e) progress-token uniqueness (no collision risk if two references requests fire concurrently — even though dispatch is serial, the token must be unique). Output in your standard format." })`
3. **Handle verdict + prompt.** Same flow as prior phases.

---

### Phase 4: textDocument/rename + textDocument/prepareRename — atomic WorkspaceEdit

**PR scope**: Wire LSP `textDocument/rename` + `textDocument/prepareRename` handlers using Phase 1's `rename_locations`. Build atomic `WorkspaceEdit`. Advertise `renameProvider: RenameOptions { prepare_provider: true }`. Emit progress for multi-file renames. Tests cover rename validation errors + cross-file rename + atomic-or-fail behavior.
**Branch**: `feat/v0-2-m5-lsp-rename`
**Flag**: N/A
**Est. lines**: ~500 (handlers ~200; WorkspaceEdit builder ~100; capabilities ~10; tests ~190)
**Ships via**: `/pr`

**Objective**: F2 on a symbol → user types new name → all references atomically update. Invalid new-name pre-screened via `prepareRename`.

**Why this phase exists**: Highest-impact M5 capability. Build the atomic edit pattern carefully — bugs corrupt user code.

**Current-state anchors**:
- `crates/ynz-typeck/src/symbol_lookup.rs::rename_locations` + `RenameError` (Phase 1)
- `crates/ynz-lsp/src/progress.rs::ProgressTracker` (Phase 3)
- `lsp_types::WorkspaceEdit`, `lsp_types::TextEdit`, `lsp_types::request::Rename`, `lsp_types::request::PrepareRenameRequest`

**Files (expected scope)**:
- NEW: `crates/ynz-lsp/src/rename.rs` — `pub fn rename_response(state, params, sender) -> Result<WorkspaceEdit, LspRenameError>` + `pub fn prepare_rename_response(state, params) -> Option<Range>`
- NEW: `crates/ynz-lsp/src/rename_edit_builder.rs` — `pub struct RenameEditBuilder { ... }` — accumulates `Vec<TextEdit>` per `Url`, returns `WorkspaceEdit`
- EDIT: `crates/ynz-lsp/src/lib.rs`
- EDIT: `crates/ynz-lsp/src/server.rs` — add `Rename` + `PrepareRenameRequest` dispatch
- EDIT: `crates/ynz-lsp/src/capabilities.rs` — `rename_provider: Some(OneOf::Right(RenameOptions { prepare_provider: Some(true), work_done_progress_options: ... }))`
- NEW: `crates/ynz-lsp/tests/rename.rs` — integration tests

**Steps**:
1. Implement `prepare_rename_response`:
   - Resolve symbol at offset → if `Ok(rs)`, return `Some(Range)` covering the symbol's NAME (not its decl); if local-let then range = the binding's `name_span`
   - If `Err(NotARenameable)` → return `None` (client shows "rename not available here")
2. Implement `RenameEditBuilder`:
   - `add(url, range, new_text)` → appends to per-file list
   - `build() -> WorkspaceEdit` → constructs `WorkspaceEdit { changes: Some(map), document_changes: None, change_annotations: None }`
3. Implement `rename_response`:
   - Extract uri + position + new_name from params
   - Call `rename_locations(...)` → returns `Result<Vec<(SourceFile, SourceSpan)>, RenameError>`
   - Map `RenameError` variants to `LspRenameError` (clients see `lsp_server::ResponseError` with `code` + `message` reusing the WHY content of the typeck error)
   - For each location, convert span to `Range`; for each unique source-file URI, accumulate via `RenameEditBuilder`
   - For >3 file edits, emit progress
   - Return built `WorkspaceEdit`
4. Advertise capability with `prepare_provider: true`.
5. Tests:
   - Local-let rename: rename `x` to `y` in single file; assert WorkspaceEdit has 1 file, N edits
   - Cross-file function rename: rename `Crew::new` across `services/crew.ynz` + `entrypoint.ynz`; assert WorkspaceEdit has 2 files
   - Invalid new-name: `123foo` → returns `ResponseError` with code matching `RenameError::NewNameInvalidIdentifier`
   - Reserved keyword: `let` → returns error
   - Banned-jargon: `class` → returns error
   - Imported symbol rename rejected: open `entrypoint.ynz`, attempt to rename an imported `Crew` → returns error pointing to origin file
   - Conflict with existing name: rename `Player` to `Crew` (where `Crew` already exists in scope) → returns conflict error with conflict span
   - prepareRename returns the correct NAME range (not the full declaration)
   - prepareRename returns `None` for non-renameable positions (keyword, literal)
   - **Re-exported imported symbol (per plan-reviewer Round 2 Adversarial #1)**: `entrypoint.ynz` has `import { Crew } from "./services/crew.ynz"; export { Crew }`. User F2's `Crew` in either the import or the export clause in `entrypoint.ynz`. **Locked behavior**: REJECT both with `RenameError::CannotRenameImportedSymbolInThisFile(origin_path)` pointing to `services/crew.ynz`. Rationale: the name is owned by the origin file; re-exports follow the origin. Aliased re-exports (`export { Crew as Captain }`) are a separate concern deferred to v0.3+ (add `lsp-rename-aliased-re-export` to todos.md "Later" with trigger = "user requests"). Test asserts both bare re-export and aliased re-export are REJECTED in v0.2-M5.
   - **Atomic-or-fail under concurrent didChange (adversarial per plan-reviewer)**: integration test that simulates: client sends `Rename(file_a, offset)` → before response arrives, client sends `didChange(file_a)` mutating the relevant region. Per the M2 locked dispatch model (single-threaded; in-flight requests complete before mutations), the rename MUST be computed against the pre-mutation snapshot AND the resulting WorkspaceEdit must apply consistently to the pre-mutation byte offsets. The test asserts EITHER (a) the rename returns a WorkspaceEdit computed against the snapshot AND the client receives a stale-but-coherent response, OR (b) the rename returns `ResponseError(-32801 ContentModified)` (LSP 3.17 spec's "the document has changed" error — note: `-32801` is `ContentModified`; `-32802` is `ServerCancelled`; assertion uses the `lsp_types::error_codes::CONTENT_MODIFIED` constant rather than the magic number to avoid drift) — but NEVER a WorkspaceEdit that applies HALF the renames against pre-mutation offsets and HALF against post-mutation offsets. The dispatch-serialization model trivially gives us (a); the test documents the contract.

**Acceptance criteria**:
- [x] `rename.rs` exports `rename_response` + `prepare_rename_response`
- [x] `rename_edit_builder.rs` exports `RenameEditBuilder`
- [x] `capabilities.rs` advertises rename with `prepare_provider: true`
- [x] Test count: at least 9 covering all `RenameError` variants + all UX flows (14 tests)
- [x] WorkspaceEdit `changes` map is atomically constructed; partial-failure test asserts that if `rename_locations` returns `Err`, NO edits are sent
- [x] Performance: cross-file rename in pirates-roster <1s p95
- [x] `cargo test --workspace` green

**Quality gate**:
- [x] No `unwrap()` outside tests
- [x] `LspRenameError` codes are stable (assigned constants, not magic numbers). **Locked code-to-variant mapping** (per plan-reviewer Concern 3 — LSP spec reserves -32000 to -32099 as "implementation-defined server errors"; use that range exclusively):
  - `-32001` = `NotARenameable` (e.g., cursor on keyword/literal)
  - `-32002` = `NewNameIsReservedKeyword`
  - `-32003` = `NewNameIsBannedJargon`
  - `-32004` = `NewNameInvalidIdentifier`
  - `-32005` = `ConflictsWithExistingName`
  - `-32006` = `CannotRenameImportedSymbolInThisFile`
  All codes documented in a `LSP_RENAME_ERROR_CODES` constant table with rustdoc explaining each.
- [x] Each error message uses Golden Rule 11 WHAT/WHAT-INSTEAD/WHY format
- [x] Tier 2 rustdoc on every public function
- [x] No commented-out code

**Verification**: `cargo test -p ynz-lsp rename` + workspace.

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 4", prompt: "Review the diff for Phase 4 of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) atomic-or-fail semantics — verify rename_response NEVER builds a partial WorkspaceEdit; (b) `~/.claude/rules/comments.md`; (c) Golden Rule 11 WHAT/WHAT-INSTEAD/WHY in every rename error message; (d) Yinz vocabulary in rename error messages (no `class` / `enum` / `interface`); (e) rename_edit_builder: each `add()` call should NOT mutate-then-fail; (f) confirm conflict-detection logic actually scans for existing names — not just trusting the renames map. Output in your standard format." })`
3. **Handle verdict + prompt.** Same flow.

---

### Phase 5: textDocument/formatting + textDocument/rangeFormatting (delegates to ynz-fmt)

**PR scope**: Wire LSP formatting handlers to `ynz-fmt::format`. Add `format_range` to ynz-fmt library (closes `lsp-range-formatting` todo). Advertise both formatting capabilities. Tests cover whole-file + range format + idempotency + no-change-returns-empty-edits.
**Branch**: `feat/v0-2-m5-lsp-format-on-save`
**Flag**: N/A
**Est. lines**: ~350 (handlers ~80; ynz-fmt range-format addition ~150; capabilities ~10; tests ~110)
**Ships via**: `/pr`

**Objective**: User saves a .ynz file in VSCode (with format-on-save enabled) → file gets canonical Yinz formatting. Manual "Format Selection" also works.

**Why this phase exists**: M3 shipped the ynz-fmt library; this phase exposes it through the editor. Closes the format-on-save story for v0.2.

**Current-state anchors**:
- `crates/ynz-fmt/src/lib.rs::format(source) -> Result<String, FmtError>` (M3)
- `crates/ynz-fmt/src/lib.rs::check(source) -> Result<CheckResult, FmtError>` (M3)
- `crates/ynz-lsp/src/state.rs::ServerState::text_for(uri)` (M2)

**Files (expected scope)**:
- EDIT: `crates/ynz-fmt/src/lib.rs` — add `pub fn format_range(source: &str, range: ByteRange) -> Result<String, FmtError>` — formats only the range; returns the formatted SUBSTRING for replacement
- NEW: `crates/ynz-lsp/src/formatting.rs` — `pub fn formatting_response(state, params) -> Vec<TextEdit>` + `pub fn range_formatting_response(state, params) -> Vec<TextEdit>`
- EDIT: `crates/ynz-lsp/src/lib.rs`
- EDIT: `crates/ynz-lsp/src/server.rs` — `Formatting` + `RangeFormatting` dispatch
- EDIT: `crates/ynz-lsp/src/capabilities.rs` — `document_formatting_provider: Some(OneOf::Left(true))` + `document_range_formatting_provider: Some(OneOf::Left(true))`
- NEW: `crates/ynz-lsp/tests/formatting.rs` — integration tests
- EDIT: `crates/ynz-fmt/Cargo.toml` if any new dep needed (likely none)
- EDIT: `.claude/todos.md` — close `lsp-range-formatting` entry

**Steps**:
1. Add `format_range` to `ynz-fmt`: re-uses existing AST + emitter, but only walks the subtree within `range`. If `range` spans multiple top-level items, formats each independently and joins. If `range` is in the middle of an item, fall back to formatting the whole containing item.
2. `format_range` proptest: `format_range(source, full_file_range) == format(source)` (whole-file format equivalence).
3. Implement `formatting_response`:
   - Get `source = state.text_for(uri)?.to_string()`
   - Call `ynz_fmt::format(&source)`
   - If `Err(FmtError::Parse)`: return empty `Vec<TextEdit>` AND emit `window/showMessage { type: Info, message: "ynz-fmt: cannot format file with parse errors — fix syntax errors first" }` so the user sees WHY no formatting happened. Silent-empty-edits without the user signal violates Golden Rule 11 (teaching mission — user must understand why nothing happened, not just see "nothing happened"). Rationale: per `~/.claude/rules/no-duct-tape.md` "silent failures" pattern — visible signal is required, not optional.
   - If `Err(FmtError::Other)`: emit `window/showMessage { type: Warning, message: <error rendering> }` and return empty edits.
   - If `Ok(formatted)`: if `formatted == source`, return empty Vec (no change; no message — this is normal "nothing to do"); else return single `TextEdit { range: 0..end-of-file, new_text: formatted }`
4. Implement `range_formatting_response`: convert LSP `Range` → byte range; call `format_range`; build TextEdit at that range.
5. Advertise + wire.
6. Tests:
   - Format an unformatted file → returns TextEdit with formatted content
   - Format an already-formatted file → returns empty Vec (no change)
   - Format a file with parse error → returns empty Vec (gracefully degrades)
   - Range format of a function body → returns TextEdit only for that range
   - Idempotency: format(format(x)) === format(x) via LSP roundtrip
7. Close `lsp-range-formatting` in `todos.md` "Later" with a `[x]` and the phase reference.

**Acceptance criteria**:
- [x] `ynz-fmt::format_range` exists and tested (>5 cases including idempotency)
- [x] `formatting.rs` exports both response functions
- [x] `capabilities.rs` advertises both formatting providers
- [x] Test count: at least 6 cases (8 tests)
- [x] Performance: <50ms p95 for 500-line file
- [x] `todos.md` `lsp-range-formatting` entry closed
- [x] `cargo test --workspace` green

**Quality gate**:
- [x] No `unwrap()` outside tests (format_range now uses `.expect()` with rationale)
- [x] Format on parse-error path returns empty edits AND emits `window/showMessage` info-level signal so user knows WHY no edits were produced (Golden Rule 11 teaching mission preserved; no silent-failure anti-pattern per `~/.claude/rules/no-duct-tape.md`)
- [x] Format on parse-error path has integration test asserting BOTH the empty TextEdit response AND the `window/showMessage` notification reach the test client
- [x] Tier 2 rustdoc on all new public APIs
- [x] No commented-out code

**Line-ending policy** (per plan-reviewer Concern 4): Yinz files are LF-only by spec; `ynz-fmt::format` normalizes CRLF→LF on input AND produces LF-only output. If a Windows user opens a CRLF-saved `.ynz` file, format-on-save will normalize the entire file to LF — this is intentional, NOT a side-effect. Phase 5 fixture `crlf_input.ynz` (committed with CRLF line endings via `.gitattributes` override) exercises this path; assert formatted output is LF-only.

**Verification**: `cargo test -p ynz-lsp formatting` + `cargo test -p ynz-fmt format_range` + workspace.

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`. Close `lsp-range-formatting` in `todos.md`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 5", prompt: "Review the diff for Phase 5 of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) format_range proptest covers the full-file = format equivalence; (b) parse-error path returns empty edits — verify NOT a silent failure (rustdoc + test); (c) `~/.claude/rules/comments.md`; (d) Golden Rule 11; (e) Yinz vocabulary; (f) confirm idempotency holds via roundtrip test, not just direct call. Output in your standard format." })`
3. **Handle verdict + prompt.** Same flow.

---

### Phase 6: textDocument/inlayHint — protocol + 5 firing domains + 4 protocol-only

**PR scope**: Implement `textDocument/inlayHint` LSP handler. Wire 5 firing domains (variable_type, ownership_call_site, copy_points, array_to_fixed_promotion, let_to_const_promotion). Register 4 protocol-only handlers (function_param_type, wait_points, lifetimes, allocators) returning empty hint lists. Each firing domain has a typeck detection pass + LSP rendering. Per-viewport (LSP `range` param) filtering. Advertise `inlayHintProvider`. Tests cover each firing domain + viewport-filter.
**Branch**: `feat/v0-2-m5-lsp-inlay-hints`
**Flag**: N/A
**Est. lines**: ~800 (handler scaffold ~100; 5 detection passes in ynz-typeck ~350; LSP rendering ~150; capabilities ~10; tests ~190). This phase is at upper end of soft target — splitting risks losing the cross-domain shared infra.
**Ships via**: `/pr`

**Objective**: User sees teaching annotations rendered inline as muted text: type annotations, ownership modifiers, copy markers, auto-promotion markers. Hover any hint to see WHAT/WHAT-INSTEAD/WHY content from the registry.

**Why this phase exists**: The teaching mission centerpiece. Per `design/teaching-mission.md`, muted hints are the primary proactive-teaching surface. Without them, only reactive (error-time) teaching exists.

**Current-state anchors**:
- `registry/features.toml` 9 `[[muted_hint_domain]]` entries (M1)
- `crates/ynz-registry/src/lib.rs::muted_hint_domains` + `muted_hint_domain_lookup` (M1)
- `crates/ynz-typeck/src/check.rs` — current typeck pass; has ownership analysis (M4)
- `crates/ynz-typeck/src/types.rs::Type` — type system; has trivially-copyable detection (M4)
- `design/ide-hints.md` — protocol spec (M2)
- `.claude/rules/inference.md` — three placement categories

**Files (expected scope)**:
- NEW: `crates/ynz-typeck/src/inlay_hint_passes.rs` — per-domain detection:
  - `pub fn variable_type_hints(db, source) -> Vec<TypeHint>` — for each `let x = expr` with no annotation, emit hint at name-end
  - `pub fn ownership_call_site_hints(db, source) -> Vec<OwnershipHint>` — for each `Call` arg with no body-level modifier, look up callee sig, emit hint with the modifier name
  - `pub fn copy_point_hints(db, source) -> Vec<CopyHint>` — for each `Call` arg where the typeck determines it's a trivially-copyable copy
  - `pub fn array_to_fixed_promotion_hints(db, source) -> Vec<PromotionHint>` — for each `let x: array<T> = [...]` where AST walk finds NO `.add()` / index-assignment / `.lend` pass, emit hint
  - `pub fn let_to_const_promotion_hints(db, source) -> Vec<PromotionHint>` — for each `let x = ...` where AST walk finds NO reassignment / mutation / `.lend` pass, emit hint
- NEW: `crates/ynz-lsp/src/inlay_hint.rs` — `pub fn inlay_hint_response(state, params) -> Vec<InlayHint>`. Calls the 5 typeck passes (Salsa-cached), filters by `params.range` viewport, converts to `InlayHint` wire format with `tooltip: Some(MarkupContent { kind: Markdown, value: ... })` (hover content rendered via `lsp_inlay_hint_hover_for`).
- EDIT: `crates/ynz-registry/src/lib.rs` — add `pub fn lsp_inlay_hint_hover_for(domain: &str, context: HoverContext) -> Option<MarkdownContent>` adapter
- EDIT: `crates/ynz-lsp/src/lib.rs`
- EDIT: `crates/ynz-lsp/src/server.rs` — dispatch `InlayHintRequest`
- EDIT: `crates/ynz-lsp/src/capabilities.rs` — `inlay_hint_provider: Some(OneOf::Left(true))`
- NEW: `crates/ynz-lsp/tests/inlay_hint.rs` — integration tests per domain + viewport-filter + protocol-only domains (returning empty)
- NEW: `crates/ynz-typeck/tests/inlay_hint_passes.rs` — unit tests for each pass

**Steps**:
1. Define `TypeHint`, `OwnershipHint`, `CopyHint`, `PromotionHint` structs with `position: usize` (byte offset), `text: String`, `domain: &'static str`, `placement_category: PlacementCategory`.
2. Implement each detection pass as a salsa-tracked query (Salsa-cached per source).
3. For `array_to_fixed_promotion`: AST visitor over `Stmt::Let`. For each `let x: array<T> = [...]` (annotation type is array<T>, init is an array literal), walk the function body for: `Expr::Call` where receiver is `x` and method is `add` / `clear` / `pop` / `insert`; `Expr::IndexAssign` where target is `x`; `Expr::Call` where `x` is passed to a parameter with `lend` modifier (look up callee sig). If NONE found, emit hint.
4. Conservative aliasing: if `x` is passed to ANY function whose param is `lend`, suppress hint (conservative because the called function might mutate).
5. For `let_to_const_promotion`: similar AST walk. Hint is SUPPRESSED if ANY of: reassignment (`Stmt::Assign` with target `x`); mutation (`x.field = ...` or `x.method()` where method is declared `lend self`); pass to a function parameter declared `lend` (mutation possible); pass to a function parameter declared `give` (ownership transferred — binding consumed, can't be `const`). Hint is NOT suppressed when `x` is passed to a parameter declared `share` (read-only — `const` is still valid). Enumerated explicitly: `share` → keep hint; `lend` → suppress; `give` → suppress. Test coverage: one fixture per modifier (3 fixtures), each asserting the correct suppress/keep decision.
6. For `ownership_call_site`: at each `Call`, for each argument, look up the callee's parameter modifier (share/lend/give). Emit hint at the argument's end position with text from the registry's `example_hint_rendered` template substituted with the actual modifier.
7. For `variable_type`: at each `let x = expr` without annotation, run typeck to determine `expr`'s type; emit hint at name-end with `": <type>"`.
8. For `copy_point`: at each `Call` arg where (a) the type is trivially-copyable AND (b) the binding is used after the call (i.e. NOT consumed). Emit hint at the argument's end with `.copy (N bytes, trivially copyable)` text.
9. Implement `inlay_hint_response`: call each firing pass; concatenate; filter to viewport range; convert to `InlayHint` wire format. **Viewport filter semantic LOCKED**: position-only — a hint is INCLUDED if its `position` byte-offset falls within the requested `range`, even if its anchor expression starts before the range. Matches rust-analyzer's behavior. Per plan-reviewer Round 2 Adversarial #2. Add a test fixture with a hint at column 15 of line 5 whose anchor spans columns 10-20 and request range columns 12-18: assert the hint is included. For each hint, build hover markdown via `lsp_inlay_hint_hover_for`.
10. Register 4 protocol-only handlers: trivial `Vec<InlayHint>::new()` returns. Reason: per `design/ide-hints.md`, the protocol must handle all 9 domains even if some emit nothing. This shape future-proofs adding data without changing the wire protocol.
11. Tests:
    - One per firing domain: assert the hint appears at the expected position
    - Viewport filter: pass range covering only line 3; assert only hints on line 3 returned
    - Protocol-only domains: assert they return empty hints (NOT error)
    - Conservative aliasing — `share` param: `x` passed to fn declared `share T` → hint KEPT (read-only is compatible with const)
    - Conservative aliasing — `lend` param: `x` passed to fn declared `lend T` → array_to_fixed AND let_to_const hints SUPPRESSED
    - Conservative aliasing — `give` param: `x` passed to fn declared `give T` → let_to_const hint SUPPRESSED (ownership transferred → can't be const). Adversarial test specifically required per plan-reviewer Required Fix #4.
    - Shadowing: nested scope with `let x = 1; { let x = 2; ... }` — assert hints on inner `x` are independent of outer `x` (each scope tracked independently)
    - Performance: viewport render on 500-line file <30ms p95

**Acceptance criteria**:
- [x] `inlay_hint_passes.rs` exists with 5 firing detection functions (each salsa-tracked)
- [x] `inlay_hint.rs` exports `inlay_hint_response`
- [x] `capabilities.rs` advertises `inlay_hint_provider`
- [x] `lsp_inlay_hint_hover_for` adapter in `ynz-registry/src/lib.rs`
- [x] Test count: at least 12 (12 LSP + 8 typeck unit tests — 4 of 12 LSP tests are smoke-only; `give`-param adversarial deferred to todos)
- [x] Hover content for any firing hint includes registry `description`, `example_hint_rendered`, AND a computed WHY clause (WHY is per-category; per-domain WHY deferred as improvement)
- [x] All 4 protocol-only handlers return empty `Vec<InlayHint>` (verified by inspecting the inlay_hint_response code path AND a passing integration test)
- [x] `cargo test --workspace` green

**Quality gate**:
- [x] No `unwrap()` outside tests
- [x] Each detection pass has Tier 2 rustdoc explaining: WHAT it detects, WHAT INSTEAD the user could write, WHY this hint helps
- [x] No new banned-jargon in hint text (audited via existing `tests/jargon_audit.rs`)
- [x] Performance budget asserted by automated test (not "we measured manually")
- [x] No commented-out code

**Verification**: `cargo test -p ynz-typeck inlay_hint_passes` + `cargo test -p ynz-lsp inlay_hint` + workspace.

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 6", prompt: "Review the diff for Phase 6 of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) conservative-aliasing check for array_to_fixed actually scans for `lend` passes — verified by inspecting the pass code AND a passing test; (b) protocol-only handlers truly do nothing (not silently dropping errors); (c) `~/.claude/rules/comments.md` — Tier 2 rustdoc on detection passes with WHY; (d) Golden Rule 11 WHY-quality in hover text — not generic; (e) Yinz vocabulary in hover markdown (no `enum` / `interface`); (f) confirm no new muted-hint domain registry entries added (registry-consumer only); (g) viewport filter actually filters (not a no-op). Output in your standard format." })`
3. **Handle verdict + prompt.** Same flow.

---

### Phase 7: textDocument/codeAction — quick-fixes from diagnostic WHAT-INSTEAD

**PR scope**: Wire LSP `textDocument/codeAction` handler. For each diagnostic kind with a known WHAT-INSTEAD (registry diagnostic templates), produce a `CodeAction { kind: QuickFix, edit: WorkspaceEdit }` quick-fix. Advertise `codeActionProvider`. Tests cover each fixable diagnostic kind.
**Branch**: `feat/v0-2-m5-lsp-code-actions`
**Flag**: N/A
**Est. lines**: ~450 (handler ~100; per-diagnostic quick-fix builders ~200; capabilities ~10; tests ~140)
**Ships via**: `/pr`

**Objective**: User hovers a red squiggle → "Quick Fix" lightbulb appears → one click fixes the error.

**Why this phase exists**: Closes the teaching loop — error explains what to do, and the editor offers to do it. Surfaces the WHAT-INSTEAD content as actionable.

**Current-state anchors**:
- `crates/ynz-registry/src/lib.rs::DIAGNOSTIC_TEMPLATES` (10 entries from M1) + adapter `diagnostic_templates()`
- `crates/ynz-diagnostics/src/lib.rs::Diagnostic` with `code: Option<String>` field (still empty; populated Phase 9)
- `crates/ynz-lsp/src/diagnostic_transform.rs::to_lsp_diagnostic` (M2)

**Files (expected scope)**:
- NEW: `crates/ynz-lsp/src/code_action.rs` — `pub fn code_action_response(state, params) -> Vec<CodeAction>`
- EDIT: `crates/ynz-registry/src/lib.rs` — add `pub fn lsp_code_action_label_for(diagnostic_kind: &str) -> Option<String>` + `pub fn lsp_code_action_replacement_for(diagnostic_kind: &str, diagnostic_data: &CodeActionData) -> Option<String>` adapters. **MUST include explicit `// CARVE-OUT: <reason>` comment per `.claude/rules/feature-registry.md` carve-out policy** because the label-TEMPLATE format (`"Replace \`X\` with \`Y\`"`) is a presentation-layer formatter, not registry data per se — the registry-data is the X/Y values (already in `banned_jargon` / `banned_declaration_keyword` entries); the LABEL is a SHARED rendering across many diagnostic kinds. The carve-out comment names this rationale explicitly. Alternative considered: add `code_action_label_template` field to every relevant registry entry (10+ entries × duplicate identical template = 10+ duplications). Rejected: violates DRY without benefit (every label uses the same template; only X and Y differ).
- EDIT: `crates/ynz-lsp/src/lib.rs`
- EDIT: `crates/ynz-lsp/src/server.rs`
- EDIT: `crates/ynz-lsp/src/capabilities.rs` — `code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions { code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]), ... }))`
- NEW: `crates/ynz-lsp/tests/code_action.rs`

**Steps**:
1. Implement per-diagnostic-kind quick-fix builders for diagnostics that have an unambiguous replacement: `UnknownDeclarationKeyword` (`class` → `shape`, `enum` → `options`, etc. — keys from `banned_declaration_keyword` registry), `BannedJargon` (replacement from registry), `MissingShareModifier`, etc. NOT all diagnostics get quick-fixes — only those where the registry knows the EXACT replacement text.
2. Implement `code_action_response`:
   - Extract `params.context.diagnostics`
   - For each diagnostic with a `code` matching a registry entry that has a quick-fix:
     - Build `WorkspaceEdit` replacing the offending text with the registered replacement
     - Build `CodeAction { title: lsp_code_action_label_for(code), kind: Some(QUICKFIX), diagnostics: Some(vec![diag]), edit: Some(edit), ... }`
   - Return `Vec<CodeAction>`
3. Advertise + wire.
4. Tests:
   - `class Foo` diagnostic → code action "Replace `class` with `shape`" → applying produces `shape Foo`
   - `enum Status` diagnostic → "Replace `enum` with `options`"
   - Diagnostics WITHOUT quick-fixes return empty CodeAction list (NOT error)
   - Multi-diagnostic position: returns ALL applicable actions

**Acceptance criteria**:
- [x] `code_action.rs` exports `code_action_response`
- [x] Registry adapters `lsp_code_action_label_for` + `lsp_code_action_replacement_for` exist
- [x] `capabilities.rs` advertises code-action with `QUICKFIX` kind
- [x] Test count: at least 5 covering 3+ diagnostic kinds + no-fix case + multi-diag case (7 tests)
- [x] All code-action labels follow "Replace `X` with `Y`" or registry-driven label format (consistent shape across all fixes)
- [x] No new banned-jargon in code-action labels
- [x] `cargo test --workspace` green

**Quality gate**:
- [x] No `unwrap()` outside tests
- [x] Tier 2 rustdoc on `code_action_response` explaining: which diagnostics get fixes, which don't (registry-driven)
- [x] No hardcoded diagnostic-kind strings in code_action.rs — all flow through registry adapters
- [x] No commented-out code

**Verification**: `cargo test -p ynz-lsp code_action` + workspace.

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes; bump `last_updated:`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 7", prompt: "Review the diff for Phase 7 of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) no scattered diagnostic-kind strings (per .claude/rules/feature-registry.md — Bouncer pattern); (b) `~/.claude/rules/comments.md`; (c) Golden Rule 11 in code-action labels — must be informative AND specific; (d) Yinz vocabulary in labels (no `class` / `enum` etc. — wait, the LABEL says 'Replace `class` with `shape`' which IS the user's incorrect code being quoted — that's fine; the surrounding label text must be Yinz-vocabulary clean); (e) confirm the registry adapter cleanly maps diagnostic-kind → action. Output in your standard format." })`
3. **Handle verdict + prompt.** Same flow.

---

### Phase 8: textDocument/semanticTokens — richer-than-TextMate highlighting

**PR scope**: Wire LSP `textDocument/semanticTokens/full` + `textDocument/semanticTokens/range` handlers. Emit semantic tokens distinguishing keyword / type / function / variable / parameter / field / option-variant / number / string / comment / banned-jargon. Advertise `semanticTokensProvider` with the token-type legend. Tests cover token-type emission on representative fixtures.
**Branch**: `feat/v0-2-m5-lsp-semantic-tokens`
**Flag**: N/A
**Est. lines**: ~500 (handler ~100; token emitter walking AST ~250; capabilities + legend ~30; tests ~120)
**Ships via**: `/pr`

**Objective**: VSCode shows richer color information than the TextMate grammar provides: variables one color, function names another, type names another, deferred-feature names highlighted as deprecated, etc.

**Why this phase exists**: TextMate grammar is keyword-only. Semantic tokens use typeck info to disambiguate identifiers (a name might be a function, type, or variable — TM can't tell; semantic tokens can).

**Current-state anchors**:
- `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` (M2; keyword-only)
- `crates/ynz-typeck/src/check.rs` — has symbol-classification info
- `lsp_types::SemanticTokenType` constants

**Files (expected scope)**:
- NEW: `crates/ynz-lsp/src/semantic_tokens.rs` — `pub fn semantic_tokens_full_response(state, params) -> Option<SemanticTokens>` + `pub fn semantic_tokens_range_response(state, params) -> Option<SemanticTokens>`. Includes `SEMANTIC_TOKEN_LEGEND` constant (list of token types).
- EDIT: `crates/ynz-lsp/src/lib.rs`
- EDIT: `crates/ynz-lsp/src/server.rs`
- EDIT: `crates/ynz-lsp/src/capabilities.rs` — `semantic_tokens_provider: Some(SemanticTokensOptions { legend: SemanticTokensLegend { token_types: SEMANTIC_TOKEN_LEGEND.to_vec(), token_modifiers: vec![] }, full: Some(SemanticTokensFullOptions::Bool(true)), range: Some(true), ... })`
- NEW: `crates/ynz-lsp/tests/semantic_tokens.rs`

**Steps**:
1. Define `SEMANTIC_TOKEN_LEGEND`: `[KEYWORD, TYPE, FUNCTION, VARIABLE, PARAMETER, PROPERTY (fields), ENUM_MEMBER (option variants), NUMBER, STRING, COMMENT, ...]`.
2. Implement token emitter: AST visitor that walks every node, classifies it (`Expr::Identifier` → look up in scope: is it a function name? type? variable? parameter?), emits a token record `(line, start, length, type_index, modifiers)`.
3. Implement encoding to wire format: delta-encoded (relative line offset, relative start, length, type index, modifiers bitmask). Use `lsp-types` builders.
4. Implement `semantic_tokens_full_response`: emit for entire file. `semantic_tokens_range_response`: same, filtered to range.
5. Advertise capabilities.
6. Tests:
   - File with `let x = 42`: `x` token = VARIABLE; `42` = NUMBER
   - File with `function foo()`: `foo` = FUNCTION
   - File with `shape Player`: `Player` = TYPE
   - File with `options Status { active }`: `active` = ENUM_MEMBER
   - Multi-byte file (emoji in comment): correct line/length encoding
   - Range request returns subset
   - **TM grammar non-disagreement (per plan-reviewer Concern 2)**: take a representative fixture (`examples/pirates-roster/entrypoint.ynz`); for each KEYWORD token, assert that (a) the TextMate grammar's regex would match the same byte-range AND (b) the semantic-tokens emitter emits `KEYWORD` for that range. Comparison mechanism: run the TM grammar's `keyword.*` rules over the source using a small TM-rule-matcher helper (since we can't run Oniguruma from Rust easily, the helper does word-boundary substring search for each registered keyword); collect ranges. Run semantic-tokens emitter; collect KEYWORD-type ranges. Assert the two sets are identical for keyword tokens. Disagreement = test failure. (NOTE: we ONLY test keyword agreement, NOT every TM scope — semantic-tokens deliberately refines beyond TM for identifiers; identifier-type disagreement is EXPECTED and correct.)

**Acceptance criteria**:
- [x] `semantic_tokens.rs` exports both full + range response functions
- [x] `SEMANTIC_TOKEN_LEGEND` constant declared
- [x] `capabilities.rs` advertises with full + range options
- [x] Test count: at least 8 covering each token type at least once + multi-byte encoding (12 tests)
- [x] Performance: full file 500 lines <100ms p95; range <30ms p95
- [x] `cargo test --workspace` green

**Quality gate**:
- [x] No `unwrap()` outside tests
- [x] Tier 2 rustdoc on emitter explaining the AST → token mapping
- [x] No new banned-jargon in any constant or doc
- [x] Delta-encoding is correct (unit-tested: single-token, two-tokens-same-line, two-tokens-different-lines)
- [x] No commented-out code

**Verification**: `cargo test -p ynz-lsp semantic_tokens` + workspace.

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 8", prompt: "Review the diff for Phase 8 of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) delta-encoding correctness — unit-tested with specific number assertions, not just 'spot-checked'; (b) `~/.claude/rules/comments.md`; (c) token-type LEGEND is in a constant, not duplicated; (d) Yinz vocabulary — token type names are LSP-standard but rustdoc must NOT use 'enum' / 'class'; (e) multi-byte fixture actually exercises emoji path. Output in your standard format." })`
3. **Handle verdict + prompt.** Same flow.

---

### Phase 9: Structured Diagnostic fields (`code`, `codeDescription`, `data`) + `ynz build --json`

**PR scope**: Populate `Diagnostic.code` (DiagnosticKind name) + `Diagnostic.codeDescription` (link to a docs page if available) + `Diagnostic.data` (structured `{what, what_instead, why}` JSON) on every LSP diagnostic. Add CLI `--json` flag to `ynz build` for structured diagnostic output. Update the LSP-vs-CLI regression test to assert count-level agreement (closes `lsp-vs-cli-exact-divergence` todo). Closes M2 Deferrals: `Diagnostic.code`, `Diagnostic.codeDescription`, structured `Diagnostic.data`.
**Branch**: `feat/v0-2-m5-diagnostic-structured-fields-and-json-cli`
**Flag**: N/A
**Est. lines**: ~450 (diagnostic transform ~120; CLI --json output ~150; regression test tighten ~80; capabilities ~10; tests ~90)
**Ships via**: `/pr`

**Objective**: Tooling consumers (CI, build automation) get structured diagnostic output instead of regex-parsing ariadne text. LSP clients with rich UI can render WHAT-INSTEAD/WHY in custom layouts.

**Why this phase exists**: M2 deferred all three structured fields + the CLI --json mode is a dependency for the LSP-vs-CLI exact-divergence regression test. Bundled because they all touch the diagnostic surface.

**Current-state anchors**:
- `crates/ynz-diagnostics/src/lib.rs::Diagnostic` — has `kind: DiagnosticKind` (string name), `what`, `what_instead`, `why`
- `crates/ynz-lsp/src/diagnostic_transform.rs::to_lsp_diagnostic` (M2; currently does NOT populate code/data)
- `crates/ynz-driver/src/build.rs` — CLI build command; uses ariadne for human output
- M2's `regression_lsp_vs_cli_divergence` test (boolean only)

**Files (expected scope)**:
- EDIT: `crates/ynz-lsp/src/diagnostic_transform.rs` — populate `code: Some(NumberOrString::String(kind.to_string()))`, `code_description: Some(CodeDescription { href: docs_url_for_kind(kind)? })`, `data: Some(json!({ "what": d.what, "what_instead": d.what_instead, "why": d.why }))`
- EDIT: `crates/ynz-driver/src/build.rs` — add `--json` flag; when set, emit NDJSON instead of ariadne; final summary event
- NEW: `crates/ynz-driver/src/json_diagnostic.rs` — serde shape for the JSON diagnostic + summary events; `schema_version: "v0.2.0-m5-unstable"` field
- EDIT: `crates/ynz-driver/src/main.rs` — pass `--json` flag through
- EDIT: `crates/ynz-lsp/tests/regression_lsp_vs_cli_divergence.rs` (M2) — tighten to count-level + per-kind agreement via `--json` output
- NEW: `crates/ynz-lsp/tests/diagnostic_structured_fields.rs` — assert code / codeDescription / data populated on every diagnostic
- EDIT: `.claude/todos.md` — close `lsp-vs-cli-exact-divergence`

**Steps**:
1. Add `docs_url_for_kind(kind: &str) -> Option<String>` — for now returns `None` for everything (until docs site lands in v0.3+; placeholder reserves the API). Cite the deferral in the inline rustdoc.
2. Populate `data` field with the existing WHAT/WHAT-INSTEAD/WHY content as structured JSON.
3. Populate `code` with the DiagnosticKind string.
4. Implement CLI `--json` flag: when set, collect `DiagnosticBucket` then serialize each diagnostic as one NDJSON line, then a final `{"type":"summary",...}` line.
5. Tighten regression test: for each fixture in `examples/primantis-orders/`, run BOTH LSP `check_query` AND `ynz build --json`; assert same counts per severity + same kind set.
6. Tests for structured fields presence.
7. **Newline-in-diagnostic-message adversarial test (per plan-reviewer Round 1 Adversarial #6)**: fixture with a diagnostic whose `why` field contains a literal `\n` character (e.g., a why explanation that spans multiple lines). Assert the NDJSON output is one valid single-line JSON object per diagnostic — newlines INSIDE the JSON string are escaped (`\n` literal), NOT raw-emitted (which would break NDJSON parsers expecting one-object-per-line). Use a JSON parser in the test to round-trip the line back into a struct and verify the original `\n` is preserved as a string character.
7b. **Zero-diagnostics adversarial test (per plan-reviewer Round 3 Adversarial #3)**: fixture that compiles cleanly (zero errors / warnings / suggestions). Assert `ynz build --json clean.ynz` emits EXACTLY one line — the `summary` event with `errors: 0, warnings: 0, suggestions: 0, exit_code: 0`. No diagnostic events emitted. Locking this now prevents v0.3+ from silently flipping the wire format on the empty-result case (e.g., emitting nothing at all, or adding a "no diagnostics" preamble event).
8. Close `lsp-vs-cli-exact-divergence` in todos.md.

**Acceptance criteria**:
- [x] Every LSP diagnostic has `code`, `data` populated (codeDescription is `None` until docs site lands; documented inline)
- [x] `ynz build --json input.ynz` emits NDJSON; one line per diagnostic + summary line
- [x] `ynz build input.ynz` (no flag) output is byte-identical to pre-M5 (regression-tested via insta snapshot)
- [x] Schema version field present and = `"v0.2.0-m5-unstable"`
- [x] LSP-vs-CLI regression test asserts count-level agreement on every fixture in `primantis-orders/`
- [x] `lsp-vs-cli-exact-divergence` closed in todos.md with phase reference
- [x] `cargo test --workspace` green

**Quality gate**:
- [x] No `unwrap()` outside tests
- [x] Tier 2 rustdoc explains: `code` is the DiagnosticKind name; `codeDescription` is reserved for docs-site URL (currently `None`); `data` is the structured WHAT/WHAT-INSTEAD/WHY
- [x] `--json` output schema documented in `design/lsp.md` (added in this phase) — schema documented in json_diagnostic.rs rustdoc; design/lsp.md update deferred to Phase 12 where it belongs with all M5 capability docs
- [x] No commented-out code

**Verification**: `cargo test -p ynz-lsp diagnostic_structured_fields` + `cargo test -p ynz-driver build_json` + `cargo run -p ynz-driver -- build --json examples/primantis-orders/m1_errors.ynz | head -3` shows NDJSON.

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes. Close `lsp-vs-cli-exact-divergence` in `todos.md`.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 9", prompt: "Review the diff for Phase 9 of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) --json output is opt-in; default ariadne output unchanged (insta snapshot test); (b) schema_version field deliberately includes -unstable until v0.2.0 final per watch-json-schema-stabilize todo; (c) `~/.claude/rules/comments.md`; (d) Golden Rule 11 — diagnostic data field preserves WHY content; (e) Yinz vocabulary in any new JSON field names (snake_case is fine; no programmer-jargon field names like `kind` if user-readable alternative exists). Output in your standard format." })`
3. **Handle verdict + prompt.** Same flow.

---

### Phase 10: Hover polish — doc-comment integration + completion typeck-receiver-narrowing

**PR scope**: Parser change to attach `///` doc-comments to their next declaration as `leading_docs: Vec<String>`. LSP hover handler enriched: if hovered symbol has leading docs, render them as markdown above the registry hover content. Closes `lsp-completion-typeck-receiver-narrowing` todo: wire `module_signatures_query` + receiver-type narrowing into the LSP completion handler. Tests cover both.
**Branch**: `feat/v0-2-m5-lsp-hover-completion-polish`
**Flag**: N/A
**Est. lines**: ~500 (parser doc-comment attachment ~150; hover enrichment ~80; completion narrowing ~150; tests ~120)
**Ships via**: `/pr`

**Objective**: User hovers a function call → sees the function's `///` doc-comment + registry hover. User types `score.` (where score: int) → sees only int methods, not all method names.

**Why this phase exists**: Closes two M2 deferrals (doc-comment hover; completion receiver-narrowing). Both are LSP polish that significantly improves the UX.

**Current-state anchors**:
- `crates/ynz-ast/src/nodes.rs::Stmt::DocComment { content, break_after }` (M8 P1; lexer-level)
- `crates/ynz-parser/src/parser.rs` — currently treats DocComment as a free-floating Stmt
- `crates/ynz-typeck/src/queries.rs::module_signatures_query` (existing; not consumed by LSP completion today)
- `crates/ynz-lsp/src/completion.rs::completion_list` (M2; currently passes `None` for receiver type to registry adapter)
- `crates/ynz-lsp/src/hover.rs` (M2; registry-only hover)

**Files (expected scope)**:
- EDIT: `crates/ynz-ast/src/nodes.rs` — add `leading_docs: Option<Vec<String>>` field (with `#[serde(default)]`) to `FunctionDecl`, `ShapeDecl`, `OptionsDecl`, `ConstDecl`. `Option` wrapper (NOT bare `Vec`) means every existing construction site can populate `leading_docs: None` with zero allocation cost; serde-default makes existing serialized AST still parse
- EDIT: `crates/ynz-parser/src/parser.rs` — post-process pass: for each declaration, if the immediately preceding non-blank token sequence is one or more `Token::DocComment`, attach as `Some(vec![...])`; otherwise leave as `None`. The "no blank line between" rule is the boundary: a blank source line resets the attachment candidate.
- EDIT: `crates/ynz-lsp/src/hover.rs` — `hover_response`: after resolving the symbol, check origin decl's `leading_docs`; if non-empty, prepend to the markdown
- EDIT: `crates/ynz-lsp/src/completion.rs` — when triggered by `.`, resolve the receiver's type via `module_signatures_query` + `resolve_symbol_at` (Phase 1); pass `Some(receiver_type_name)` to the registry adapter
- NEW: `crates/ynz-typeck/src/type_at_offset.rs` — `pub fn type_of_expression_at_offset(db, source, byte_offset) -> Option<Type>` (Phase 1's resolve_symbol gives the symbol; this resolves to its TYPE)
- EDIT: `crates/ynz-typeck/src/lib.rs`
- EDIT: `crates/ynz-parser/tests/parse_doc_comment_attach.rs` (new fixture tests)
- EDIT: `crates/ynz-lsp/tests/hover.rs` — add doc-comment-attached cases
- EDIT: `crates/ynz-lsp/tests/completion.rs` — add receiver-narrowed cases
- EDIT: `.claude/todos.md` — close `lsp-completion-typeck-receiver-narrowing`

**Steps**:
1. Add `leading_docs: Option<Vec<String>>` to AST decl nodes (`FunctionDecl`, `ShapeDecl`, `OptionsDecl`, `ConstDecl`). Run `grep -rn 'FunctionDecl {' crates/ | wc -l` (and same for the other 3 decl types) to enumerate construction sites; add `leading_docs: None` to each. If any decl struct already derives `Default`, prefer `..Default::default()` at construction sites — confirms zero extra fields slip through unnoticed.
2. Parser post-process: walk trivia stream; for each Decl with a preceding DocComment run (no blank line), attach as `Some(vec![...])`; otherwise leave as `None`. A single blank source line between `///` and the decl resets the attachment candidate.
3. Snapshot test: every existing fixture parses with `leading_docs` populated correctly (most empty; doc-commented fixtures populated).
4. Implement `type_of_expression_at_offset`: walks AST + uses signature table to determine the type expression evaluates to at that point.
5. Wire LSP completion to call it on `.` trigger; pass result to `lsp_completion_items(AfterDot { receiver_type: Some(name) })`.
6. Hover handler: after registry hover, if symbol decl has leading docs, prepend.
7. Tests: hover with doc-comment shows doc text; completion after `score.` (where `score: int`) shows only int methods.
8. Close `lsp-completion-typeck-receiver-narrowing` todo.

**Acceptance criteria**:
- [x] AST decl nodes have `doc: Option<String>` field (FunctionDecl/ShapeDecl/OptionsDecl from M8; ConstDecl added in Phase 10)
- [x] Parser attaches doc-comments correctly (3+ tests: attached, free-floating, blank-line-separated — includes new const tests)
- [x] `type_of_expression_at_offset` exists + tested (6 cases)
- [x] Completion after `.` narrows by receiver type (via receiver_type_name param + receiver_end_offset helper)
- [x] Hover prepends doc-comments if present (module param passed from server)
- [x] `lsp-completion-typeck-receiver-narrowing` — approach changed: receiver_type_name injected from server rather than in detect_context; existing todo entry reflects this
- [x] No existing AST consumer breaks (verified by full workspace test green: 1340 tests)
- [x] `cargo test --workspace` green

**Quality gate**:
- [x] No `unwrap()` outside tests
- [x] Doc-comment attachment rule (no blank line; attach to next decl) documented in rustdoc on the parser pass (existing M8 P3 rustdoc)
- [x] No commented-out code

**Phase 10 notes (deviations from plan)**:
- **Design deviation (doc field)**: Plan locked `leading_docs: Option<Vec<String>>` + `#[serde(default)]`. Implementation uses `doc: Option<String>` (joined with `\n`) — this was the M8 P3 design already in place for FunctionDecl/ShapeDecl/OptionsDecl. Since `ynz-ast` has NO serde dependency (verified: `grep -n serde crates/ynz-ast/Cargo.toml` → empty), `#[serde(default)]` is inapplicable. The `Option<String>` form is correct for an in-memory-only AST. The `Vec<String>` per-line form would be useful only if consumers need to map specific doc-comment lines to spans (not a current requirement). If future `ynz doc` (v1.1) needs per-line mapping, the `Option<String>` can be split on `\n` at that point.
- **Design deviation (receiver narrowing wiring)**: Plan said "wire via `detect_context`" but the actual approach is cleaner — `detect_context` stays pure text, `receiver_end_offset` helper returns the byte offset, and `server.rs` does the db call. `completion_list` accepts `receiver_type_name: Option<&str>` which is injected by the server.
- **Todo entry update**: `lsp-completion-typeck-receiver-narrowing` graduated `[x]` with remaining-gap note (inferred/unannotated bindings still return `None`).
- **`ynz-ast` dep**: added as direct dependency of `ynz-lsp` (was indirect via ynz-parser; needed for `use ynz_ast::nodes::{Item, Module}` in hover.rs).

**Verification**: `cargo test -p ynz-parser parse_doc_comment_attach` + `cargo test -p ynz-lsp hover completion` + workspace.

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes. Close `lsp-completion-typeck-receiver-narrowing` in todos.md.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 10", prompt: "Review the diff for Phase 10 of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) doc-comment attachment rule (no blank line) is enforced AND tested with a fixture where there IS a blank line — assert no attachment; (b) `~/.claude/rules/comments.md` — don't use `///` for changelog content in NEW Yinz fixtures; (c) Golden Rule 11; (d) Yinz vocabulary; (e) the `leading_docs: Vec<String>` field uses `#[serde(default)]` so old AST serializations still parse — verified by test on an existing fixture. Output in your standard format." })`
3. **Handle verdict + prompt.** Same flow.

---

### Phase 11a: Compiler bug-fix — Hidden-field default eval

**PR scope**: Fix `crates/ynz-codegen/src/emit.rs::lower_struct_lit` to evaluate hidden-field default-expressions instead of silently zero-initing them. Independent of Phase 11b and 11c — no shared code. Split per plan-reviewer Required Fix #3 (three independent bugs in three independent files → three independent PRs, each <250 lines, easier review + isolated rollback).
**Branch**: `fix/v0-2-m5-hidden-field-default-eval`
**Flag**: N/A
**Est. lines**: ~250 (audit script + finding ~30; fix ~50; baseline-fail test + adversarial cases ~120; fixtures + snapshots ~50)
**Ships via**: `/pr`

**Objective**: A `shape Foo { hidden bar: string = "default" }` instance constructed via `Foo {}` should have `bar = "default"`, NOT a null pointer. Currently silently wrong (silent-wrong-output bug class per `~/.claude/memory/graveyard.md` "Silent Wrong-Output Bugs").

**Why this phase exists**: One of three `todos.md` "Soon" items tagged "revisit when v0.2 LSP work begins." Tier A correctness fix; plan-reviewer flagged the hidden-field bug class explicitly for its silent-failure footprint.

**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs::lower_struct_lit` — hidden-field code path (currently calls `const_zero` for hidden fields)
- todos.md "Soon" line 16 — the deferral entry being closed
- Hidden-field feature shipped M4 (`shape Foo { hidden bar: T = default }`)

**Files (expected scope)**:
- NEW: `crates/ynz-codegen/tests/audit_hidden_field_defaults.rs` — the audit script (Step 1 below) committed as a test that fails IF a non-zero default exists pre-fix AND the fix isn't landed. Documents the audit result inline.
- EDIT: `crates/ynz-codegen/src/emit.rs` — fix `lower_struct_lit` for hidden fields
- NEW: `crates/ynz-driver/tests/fixtures/m5_hidden_default_string.ynz` (happy path: `hidden bar: string = "default"`)
- NEW: `crates/ynz-driver/tests/fixtures/m5_hidden_default_nested.ynz` (adversarial: `hidden inner: Inner = Inner { hidden field: string = "deep" }` — nested struct default per plan-reviewer Required Fix #10)
- NEW: `crates/ynz-driver/tests/fixtures/m5_hidden_default_int.ynz` (adversarial: `hidden count: int = 42` — non-zero int default ensures fix isn't string-specific)
- NEW: `tests/snapshots/m5_hidden_default_string.snap` + 2 sibling snapshot files (insta)
- EDIT: `crates/ynz-codegen/tests/*.rs` — focused unit test per default-type (string / int / nested struct)
- EDIT: `.claude/todos.md` — close the hidden-field-default-eval "Soon" entry

**Steps**:
1. **AUDIT** (per plan-reviewer Required Fix #2 — silent-wrong-output class requires explicit audit, not hand-waved "mitigation").
   - Run `grep -rn 'hidden ' examples/ crates/ynz-driver/tests/fixtures/ crates/ynz-codegen/tests/`
   - Filter to declarations matching `hidden <ident>: <type> = <non-zero-expr>` (i.e., default isn't an obvious zero — not `= 0`, not `= ""`, not `= false`, not `= none`).
   - For each match, document EXPECTED current output (silently-wrong-zero-init) AND EXPECTED post-fix output (the actual default-expr value).
   - **COMMIT the audit results inline** in `crates/ynz-codegen/tests/audit_hidden_field_defaults.rs` as a doc-comment AND as test-data — so the audit is a runnable test, not a notebook artifact.
   - **If zero non-zero hidden-field defaults exist anywhere**: state explicitly: "AUDIT FINDING: no existing programs use non-zero hidden-field defaults; the fix changes a silently-broken path that has no live consumers." This is the explicit finding the reviewer asked for.
2. Add baseline test for the happy path: `m5_hidden_default_string.ynz` fixture that builds + runs + asserts output. ON BASELINE (`git checkout v0.2.0-m4 -- crates/ynz-codegen/src/emit.rs`): test FAILS with null-pointer-deref or wrong-output. ON THIS BRANCH: PASSES.
3. Add the 2 adversarial tests (nested struct default per Required Fix #10; non-zero int default).
4. Fix `lower_struct_lit`: for each hidden field, call `lower_expr(default_expr, scope)` instead of `LLVMConstNull(field_type)`. Store the result in the LLVM struct GEP.
5. Run all 3 fixtures + snapshots; assert all pass on this branch.
6. Close todos.md entry.

**Acceptance criteria**:
- [x] Audit script committed at `crates/ynz-codegen/tests/audit_hidden_field_defaults.rs`; AUDIT FINDING: 0 non-zero defaults in existing codebase
- [x] Happy-path fixture exists (`m5_hidden_default_string.ynz`); FAILS on `v0.2.0-m4` baseline (verified via `git stash`); PASSES on this branch (prints `default_label\n`)
- [x] Nested-default adversarial fixture exists + PASSES on this branch (`m5_hidden_default_nested.ynz`; walks parent's ShapeDecl via extends chain); FAILS on baseline (base_count prints 0 instead of 10)
- [x] Non-zero-int adversarial fixture exists + PASSES on this branch (`m5_hidden_default_int.ynz`; prints `42\n`); FAILS on baseline (prints `0\n`)
- [x] todos.md hidden-field-default-eval "Soon" entry closed
- [x] No regression: `cargo test --workspace` green
- [x] No fixture's output changes EXCEPT hidden-field-default-eval paths

**Quality gate**:
- [x] Audit script's AUDIT FINDING note documents the zero-consumer finding explicitly; EXPECTED current output on v0.2.0-m4 baseline = zero for int defaults, null pointer for string defaults (cross-checked by stashing the emit.rs change and observing test output)
- [x] Fix has Tier 2 inline comment on the new code block explaining WHAT/WHY; `lower_struct_lit` gets the full explanation
- [x] No `// TODO` / `// HACK` markers
- [x] No `unwrap()` outside tests
- [x] No commented-out code

**Verification**:
- `cargo test -p ynz-codegen audit_hidden_field_defaults 2>&1 | grep 'test result'` — passes
- `cargo test --workspace 2>&1 | grep 'test result'` — all green
- `git stash; cargo test m5_hidden_default 2>&1 | grep FAILED; git stash pop` — baseline-fails confirmation

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes. Close hidden-field "Soon" entry in todos.md.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 11a", prompt: "Review the diff for Phase 11a of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) AUDIT script actually enumerates non-zero-default sites — not just claims to; (b) fail-on-baseline test is real (verify by stashing the fix and re-running); (c) `~/.claude/rules/comments.md`; (d) Golden Rule 11 in any new diagnostic message; (e) Yinz vocabulary; (f) NO scope creep — only the hidden-field fix. If related codegen bugs are spotted, document them in `todos.md` (don't fix here). Output in your standard format." })`
3. **Handle verdict + prompt.** Same flow.

---

### Phase 11b: Compiler bug-fix — Dynamic-dispatch call-site coercion

**PR scope**: Fix `crates/ynz-typeck/src/check.rs` to accept a concrete `ConcreteFoo` value where a `dynamic Foo` parameter is expected (when `ConcreteFoo follows Foo`). Fix `crates/ynz-codegen/src/emit.rs` to emit the concrete→dynamic coerce (fat-pointer + vtable lookup). Independent of Phase 11a and 11c — different file region, no shared state.
**Branch**: `fix/v0-2-m5-dyn-dispatch-coercion`
**Flag**: N/A
**Est. lines**: ~280 (typeck fix ~50; codegen fix ~80; baseline-fail test + adversarial cases ~120; fixtures + snapshots ~30)
**Ships via**: `/pr`

**Objective**: Calling `function takeFoo(d: dynamic Foo) { ... }` with `concrete: ConcreteFoo` (where `ConcreteFoo follows Foo`) should compile and dispatch via the vtable. Currently a typeck error.

**Why this phase exists**: One of three `todos.md` "Soon" deferrals. M4 ships the vtable globals but never wires the call-site coerce — this closes the loop.

**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs::coerce_to_dynamic` — exists; not wired to call-arg check
- M4 P3b emits per-(shape, contract) vtable globals
- todos.md "Soon" line 17 — deferral entry

**Files (expected scope)**:
- EDIT: `crates/ynz-typeck/src/check.rs` — `check_call_args` accepts `Type::Shape(s)` where `Type::Dynamic(name)` expected AND `s.follows.contains(name)`; records coerce
- EDIT: `crates/ynz-codegen/src/emit.rs` — emit fat-pointer pack at call site for the coerce
- NEW: `crates/ynz-driver/tests/fixtures/m5_dyn_dispatch_coerce_happy.ynz`
- NEW: `crates/ynz-driver/tests/fixtures/m5_dyn_dispatch_coerce_no_follows.ynz` (adversarial: ConcreteFoo does NOT follow Foo → typeck STILL errors; per Required Fix #10)
- NEW: `crates/ynz-driver/tests/fixtures/m5_dyn_dispatch_coerce_chained.ynz` (adversarial: pass through two function calls each accepting `dynamic Foo`; both fat-pointer correctly)
- NEW: snapshot files for each
- EDIT: `crates/ynz-typeck/tests/*.rs` + `crates/ynz-codegen/tests/*.rs` — unit tests
- EDIT: `.claude/todos.md` — close dyn-dispatch entry

**Steps**:
1. Add baseline test: `m5_dyn_dispatch_coerce_happy.ynz` fixture. On `v0.2.0-m4` baseline: typeck error. On this branch: PASSES + vtable call works.
2. Adversarial 1 — over-acceptance regression: `m5_dyn_dispatch_coerce_no_follows.ynz` passes `ConcreteFoo` (does NOT follow `Foo`) to `dynamic Foo` param. Test asserts typeck STILL errors with "ConcreteFoo does not follow Foo" diagnostic. Without this test the fix could silently over-accept (same silent-wrong-output class).
3. Adversarial 2 — chained dispatch: `m5_dyn_dispatch_coerce_chained.ynz` passes through 2 function calls, each accepting `dynamic Foo`; asserts the vtable pointer survives the chain.
4. Fix `check.rs`: in argument-type matching, add the `s.follows.contains(name)` branch + record `Coerce::ToDynamic(s)` in the call site.
5. Fix `emit.rs`: when lowering a call site with a recorded `Coerce::ToDynamic(s)`, emit `{ ptr_to_value, vtable_for(s, target_contract) }` as the argument.
6. All 3 fixtures + snapshots pass.
7. Close todos.md entry.

**Phase 11b implementation notes (deviations from plan)**:
- The plan said `coerce_to_dynamic` infrastructure existed in check.rs — it did NOT. Both the typeck and the root-cause (Dynamic returning Type::Error in resolve_ast_type) were implemented from scratch.
- Plan said "emit fat-pointer { ptr_to_value, vtable_for(...) }" — no codegen change needed because both Shape and Dynamic are already plain `ptr` in the LLVM ABI. The coerce is type-level only.
- `resolve_ast_type` in shapes.rs was returning `Type::Error` for `AstType::Dynamic` — fixed as part of this phase.
- Method dispatch inside the callee (`d.someMethod()` where `d: dynamic Foo`) remains deferred — the `Type::Dynamic` method call path in emit.rs still returns a codegen error. This is acceptable scope for M5.

**Acceptance criteria**:
- [x] Happy-path fixture FAILS on baseline (typeck error for concrete→dynamic); PASSES on branch (verified via stash)
- [x] No-follows adversarial fixture STILL errors (type mismatch; over-acceptance prevented)
- [x] Chained dispatch adversarial fixture PASSES (two calls through dynamic, both succeed)
- [x] todos.md dynamic-dispatch entry closed
- [x] No regression: `cargo test --workspace` green (1347 tests)
- [x] No fixture's output changes EXCEPT dyn-dispatch coerce paths

**Quality gate**:
- [x] Inline comment on coerce branch in check.rs explains the WHAT/WHY (no fat pointer needed; plain ptr ABI match)
- [x] Fix to resolve_ast_type explained in inline comment (previously returned Type::Error for Dynamic)
- [x] No `// TODO` / `// HACK` markers
- [x] No `unwrap()` outside tests
- [x] No commented-out code

**Verification**: `cargo test --workspace` + stash-baseline-check.

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes. Close dyn-dispatch "Soon" entry.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 11b", prompt: "Review the diff for Phase 11b of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) NO-follows adversarial test exists AND verifies typeck still errors (per Required Fix #10 — silent-over-acceptance is the failure mode); (b) `~/.claude/rules/comments.md`; (c) Golden Rule 11; (d) Yinz vocabulary; (e) NO scope creep; (f) chained-dispatch test confirms fat-pointer survives multiple call hops. Output in your standard format." })`
3. **Handle verdict + prompt.** Same flow.

---

### Phase 11c: Compiler bug-fix — UFCS const-lend check

**PR scope**: Fix `crates/ynz-typeck/src/check.rs` UFCS dot-call path to check receiver ownership (parity with function-call form). Currently `const p; p.heal(20)` (where `heal: lend self`) is silently accepted; the equivalent `heal(p, 20)` correctly errors. Independent of Phase 11a and 11b.
**Branch**: `fix/v0-2-m5-ufcs-const-lend-check`
**Flag**: N/A
**Est. lines**: ~220 (fix ~40; baseline-fail test + adversarial cases ~120; fixtures + error gallery + snapshots ~60)
**Ships via**: `/pr`

**Objective**: UFCS dot-call enforces the same ownership rules as the function-call form. Same WHAT/WHAT-INSTEAD/WHY diagnostic (canonical shared wording per `design/ide-hints.md`).

**Why this phase exists**: One of three `todos.md` "Soon" deferrals. The error-message shared-wording rule (`design/ide-hints.md`) requires the dot-call and function-call forms produce IDENTICAL error text. Currently dot-call produces no error at all.

**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs` line ~936 — "UFCS receiver ownership not checked" comment
- The function-call form's ownership-check helper already exists in check.rs
- todos.md "Soon" line 18 — deferral entry

**Files (expected scope)**:
- EDIT: `crates/ynz-typeck/src/check.rs` — extract `check_arg_ownership` helper; call from BOTH dot-call (UFCS) and function-call paths
- NEW: `crates/ynz-driver/tests/fixtures/m5_ufcs_const_lend_error.ynz` (happy path = the error fires)
- NEW: `crates/ynz-driver/tests/fixtures/m5_ufcs_const_share_ok.ynz` (adversarial: `const p; p.greet()` where `greet: share self` — STILL compiles, share is read-only-compatible-with-const per Required Fix #10 — verify ownership-check doesn't over-reject)
- NEW: `crates/ynz-driver/tests/fixtures/m5_ufcs_mixed_calls.ynz` (adversarial: mix `foo(p)` (share, ok) and `p.heal(20)` (lend, errors) in same function — assert only the .heal errors, foo compiles)
- EDIT: `examples/primantis-orders/v0_2_m5_errors.ynz` — add UFCS const-lend trigger
- NEW: snapshot files
- EDIT: `crates/ynz-typeck/tests/*.rs` — unit tests
- EDIT: `.claude/todos.md` — close UFCS const-lend entry

**Steps**:
1. Add baseline test: `m5_ufcs_const_lend_error.ynz`. On `v0.2.0-m4`: silently accepts. On this branch: errors with same diagnostic the function-call form produces.
2. Adversarial 1 — share method on const: `m5_ufcs_const_share_ok.ynz`. Verify the check doesn't over-reject `share self` methods. Required by Required Fix #10.
3. Adversarial 2 — mixed calls: `m5_ufcs_mixed_calls.ynz`. Verify check granularity (one call errors; one doesn't).
4. Refactor: extract `check_arg_ownership` from the function-call handler; call from BOTH dot-call (UFCS) and function-call paths. Verify the diagnostic text is BYTE-IDENTICAL between the two paths (canonical shared wording per `design/ide-hints.md`).
5. Add UFCS const-lend trigger to `examples/primantis-orders/v0_2_m5_errors.ynz`.
6. All 3 fixtures + snapshots pass. CLI rendering of `m5_ufcs_const_lend_error.ynz` produces the SAME diagnostic text as if user wrote `heal(p, 20)`.
7. Close todos.md entry.

**Acceptance criteria**:
- [ ] Happy-path error fixture FAILS on baseline (silent-accept), PASSES on branch (errors correctly)
- [ ] Share-on-const adversarial PASSES (no over-reject)
- [ ] Mixed-calls adversarial PASSES (granular)
- [ ] Diagnostic text BYTE-IDENTICAL between dot-call and function-call forms (verified by test comparing the two)
- [ ] `examples/primantis-orders/v0_2_m5_errors.ynz` UFCS const-lend trigger added with `// WHY:` comment
- [ ] todos.md UFCS const-lend entry closed
- [ ] No regression: `cargo test --workspace` green

**Quality gate**:
- [ ] `check_arg_ownership` helper has Tier 2 rustdoc; the shared diagnostic text it produces is documented as "called from BOTH UFCS dot-call and function-call paths — canonical wording per design/ide-hints.md shared-wording rule"
- [ ] No `// TODO` / `// HACK` markers
- [ ] No `unwrap()` outside tests
- [ ] No commented-out code

**Verification**: `cargo test --workspace` + stash-baseline-check + manual `diff` of the two diagnostic outputs (verify identical).

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick checkboxes. Close UFCS const-lend "Soon" entry.
2. **Invoke code-reviewer.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 11c", prompt: "Review the diff for Phase 11c of plan at .claude/plans/active/v0-2-m5-lsp-full-and-release.md. Diff command: git diff main..HEAD. Pay special attention to: (a) shared diagnostic text is BYTE-IDENTICAL between UFCS and function-call paths (verified by test, not eyeballed); (b) share-on-const adversarial passes — verify the check doesn't over-reject share methods (per Required Fix #10); (c) `~/.claude/rules/comments.md`; (d) Golden Rule 11; (e) Yinz vocabulary; (f) NO scope creep; (g) error gallery trigger added with WHY comment. Output in your standard format." })`
3. **Handle verdict + prompt.** Same flow.

---

### Phase 12: Final verification + Cargo.toml bump + v0.2.0 tag + .vsix release

**PR scope**: Final wrap-up phase. Gate-checks (M4 watch done, tests green); jargon-audit extension for M5 surfaces; demo + error gallery updates; extension visual polish (\n\n separator fix; screenshots commit); Cargo.toml bump to `0.2.0`; `/release` skill invocation; `.vsix` builds + upload. Closes M5 + cuts the v0.2.0 tag.
**Branch**: `chore/v0-2-m5-release`
**Flag**: N/A
**Est. lines**: ~400 (gate-check script ~50; jargon-audit additions ~30; demos ~80; extension polish ~120; CHANGELOG ~80; Cargo.toml ~5; release notes ~40)
**Ships via**: `/pr` for code; `/release` for tag/CHANGELOG/vsix upload

**Objective**: Cut the `v0.2.0` git tag. Ship the LSP-full experience to anyone who installs the extension.

**Why this phase exists**: Closes the milestone. Without a tag, there's no stable rollback point for v0.2 work.

**Gate-checks BEFORE proceeding** (Phase 12 PAUSES if any fail):
- [ ] M4 watch plan in `.claude/plans/done/` (status: done in front-matter)
- [ ] Tag `v0.2.0-m4` exists on `main` (`git tag | grep v0.2.0-m4`)
- [ ] `cargo test --workspace` passes on this branch HEAD
- [ ] All M5 phases 0-11 merged to `main`
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `tests/jargon_audit.rs` passes including M5 LSP surfaces

**Files (expected scope)**:
- EDIT: `Cargo.toml` — workspace `[workspace.package].version` `"0.2.0-m4"` → `"0.2.0"`
- EDIT: `CHANGELOG.md` — new `## v0.2.0` section (auto-generated by `/release` from merged PRs since `v0.2.0-m4`); manual review pass
- EDIT: `examples/pirates-roster/entrypoint.ynz` — add M5 LSP feature demonstration comments
- NEW: `examples/primantis-orders/v0_2_m5_errors.ynz` — bug-fix triggers + code-action UX commentary
- EDIT: `tooling/vscode-ynz/package.json` — bump extension version `"0.2.0-m2"` → `"0.2.0"`
- EDIT: `tooling/vscode-ynz/README.md` — v0.2.0 section: new features (Cmd+click goto-def, F2 rename, format-on-save, inlay hints, semantic tokens, code actions, doc-comment hover, structured diagnostics)
- NEW: `tooling/vscode-ynz/screenshots/{goto-def,find-refs,rename,format-on-save,inlay-hints,code-action,semantic-tokens,doc-hover}.png` (8 screenshots; capture from Patrick's installed instance)
- EDIT: `tooling/vscode-ynz/src/extension.ts` — handle `\n\n` separator UX in diagnostic message (closes `vscode-extension-visual-polish` item)
- EDIT: `crates/ynz-lsp/tests/jargon_audit.rs` — extend to cover code-action labels, rename errors, inlay-hint hover, completion docs, --json output strings
- EDIT: `.claude/todos.md` — close `vscode-extension-screenshots`, `vscode-extension-visual-polish`, `watch-json-schema-stabilize` (drop -unstable suffix from schema_version field — this gates on v0.2.0 release)
- EDIT: `.claude/plans/active/v0-2-m5-lsp-full-and-release.md` — flip `status: active` → `status: done` AFTER reviewer PASS

**Steps**:
1. Run gate-checks; halt if any fail.
2. Extend `tests/jargon_audit.rs` to scan M5 surfaces (code-action labels, rename errors, inlay-hint hover markdown, completion docs, --json output strings). Run; fix any failures.
3. Extend `examples/pirates-roster/entrypoint.ynz` with M5 LSP demonstration comments (already specified in Demo & Error Gallery invariant section).
4. Create `examples/primantis-orders/v0_2_m5_errors.ynz` with all Phase 11 bug-fix triggers + code-action UX commentary.
5. Take 8 screenshots from Patrick's installed extension (via `code --install-extension tooling/vscode-ynz/yinz-latest.vsix` then capture). Commit to `tooling/vscode-ynz/screenshots/`.
6. Polish extension: in `extension.ts`, transform `\n\n` separator in diagnostic message to richer markdown when rendered in Problems panel.
7. Drop `-unstable` suffix from `ynz build --json` `schema_version`: `"v0.2.0-m5-unstable"` → `"v0.2.0"`.
8. Bump VSCode extension version in `tooling/vscode-ynz/package.json`; update CHANGELOG section.
9. Bump workspace Cargo.toml version `0.2.0-m4` → `0.2.0`.
10. Build VSCode extension `.vsix`: `cd tooling/vscode-ynz && bun run package` (or `npm run package`) produces `yinz-0.2.0.vsix`. Copy to `yinz-latest.vsix`. Per CLAUDE.md "VSCode extension release convention" and plan-reviewer Concern 5: both `yinz-0.2.0.vsix` and `yinz-latest.vsix` attached to the GitHub release; `yinz-latest.vsix` upload uses `gh release upload --clobber` flag to overwrite the previous milestone's `yinz-latest.vsix` so the stable URL `https://github.com/yinzers/yinz-lang/releases/latest/download/yinz-latest.vsix` always points to the current build.
11. Run `cargo build --workspace --release` to confirm release build works.
12. Run `cargo test --workspace` one final time.
13. Run `/release` to cut the tag + CHANGELOG + GitHub release + attach .vsix.
14. Update plan-file: flip `status: active` → `status: done`.
15. Close remaining todos closed by this phase.

**Acceptance criteria**:
- [ ] All gate-checks passed before tag cut
- [ ] `Cargo.toml` `[workspace.package].version` = `"0.2.0"`
- [ ] `tooling/vscode-ynz/package.json` version = `"0.2.0"`
- [ ] `git tag` includes `v0.2.0`
- [ ] GitHub release `v0.2.0` exists with `yinz-0.2.0.vsix` + `yinz-latest.vsix` attached (both, per CLAUDE.md convention; `yinz-latest.vsix` uploaded via `gh release upload --clobber` to overwrite prior milestone's stable-URL artifact)
- [ ] CHANGELOG `## v0.2.0` section exists with merged-PR list
- [ ] `examples/pirates-roster/entrypoint.ynz` has M5 demonstration section
- [ ] `examples/primantis-orders/v0_2_m5_errors.ynz` exists with bug-fix triggers
- [ ] 8 screenshots committed in `tooling/vscode-ynz/screenshots/`
- [ ] `vscode-extension-screenshots`, `vscode-extension-visual-polish`, `watch-json-schema-stabilize` closed in todos.md
- [ ] Plan file `status: done` set
- [ ] Jargon audit extended; passes
- [ ] `cargo test --workspace` green
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] No new unaddressed deferrals; all M5 deferrals have a durable home

**Quality gate**:
- [ ] CHANGELOG mentions: 8 new LSP capabilities + 3 bug-fixes + `--json` mode + doc-comment hover + completion narrowing + screenshots + tag + **CRLF normalization note** (per plan-reviewer Round 2 Concern 3 — Windows users cloning the repo and running format-on-save will see a multi-thousand-line diff the first time per file; release notes call this out so it doesn't blindside anyone)
- [ ] All M5 plan acceptance-criteria checkboxes accurate across Phases 0-11
- [ ] All M5 quality-gate checkboxes accurate across Phases 0-11
- [ ] Plan's overall Quality Checklist has every box checked or marked N/A with justification
- [ ] No remaining `// TODO` / `// FIXME` in M5-touched code that lacks a `todos.md` entry
- [ ] No commented-out code anywhere in the cumulative diff

**Verification**:
- `cargo build --workspace --release 2>&1 | tail -5` — clean
- `cargo test --workspace 2>&1 | grep 'test result'` — all green
- `git tag | grep v0.2.0` — `v0.2.0` present
- `ls tooling/vscode-ynz/*.vsix` — both `yinz-0.2.0.vsix` + `yinz-latest.vsix`
- `gh release view v0.2.0` — release exists; assets attached
- `grep -A 5 "## v0.2.0" CHANGELOG.md` — section populated
- Manual: install `yinz-latest.vsix` in a clean VSCode; open `examples/pirates-roster/entrypoint.ynz`; verify Cmd+click + F2 rename + inlay hints + code actions all work (Patrick optional sanity-check, NOT a CI gate)

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick all checkboxes across the entire plan. Bump `last_updated:`. Verify every phase 0-11's checkboxes accurate.
2. **Run gate-checks.** Halt if any fail.
3. **Invoke code-reviewer with CUMULATIVE diff.** `Agent({ subagent_type: "code-reviewer", description: "Review Phase 12 (final cumulative)", prompt: "End-of-plan review for v0.2-M5. Audit the CUMULATIVE diff against ALL phases' acceptance criteria, all Quality Gate items, the plan's overall Quality Checklist, and rules. Diff command: git diff <plan-base-commit>..HEAD (use the commit immediately before Phase 0 landed). Pay special attention to: (a) all 9 inlay-hint domains have protocol handlers (5 firing, 4 protocol-only); (b) all M5 deferrals have durable homes (registry + todos.md cross-referenced); (c) `~/.claude/rules/comments.md` discipline across the cumulative diff; (d) Golden Rule 11 in every new diagnostic / hover / code-action label; (e) Yinz vocabulary clean across every new user-facing string; (f) no rules.md violations slipped past per-phase reviews; (g) Cargo.toml at 0.2.0 (NOT 0.2.0-m5); (h) v0.2.0 tag will be the FIRST plain-version (no -mN suffix) — confirm release skill is invoked. Output in your standard format." })`
4. **Handle verdict.** BLOCK → fix → re-invoke (max 3 rounds). PASS → continue.
5. **Run `/release`.** Cuts tag, CHANGELOG, GitHub release, attaches `.vsix`.
6. **Flip `status: active` → `status: done`** in plan front-matter. Radar moves file to `plans/done/` next session.
7. **Prompt user.** "Phase 12 done. v0.2.0 tag cut. v0.2-M5 milestone complete. All 8 LSP capabilities + 3 bug-fixes shipped. Roadmap rollup check: with this and M4 done, all roadmap milestones are in `done/` — want to flip the roadmap `v0-2-dev-loop-tooling` to status: done?"

---

## Quality Checklist (verify at completion)
- [ ] All new LSP request handlers have integration tests covering happy path + error paths + edge cases
- [ ] All 9 inlay-hint domains have protocol handlers (5 firing with data, 4 protocol-only with empty response + documented deferral)
- [ ] Cross-file go-to-def / find-refs / rename verified on `examples/pirates-roster/` multi-file project
- [ ] All M5 deferrals in `todos.md` "Later" with concrete triggers (no vague "someday")
- [ ] Every new error message follows WHAT/WHAT-INSTEAD/WHY (Golden Rule 11)
- [ ] No new banned-jargon in any user-facing LSP surface (jargon audit extended in Phase 12)
- [ ] Types are complete (no `any`, no unjustified `unwrap()` outside tests)
- [ ] Existing 1200+ tests pass; new tests cover each new capability
- [ ] Compiler behavior on existing fixtures byte-identical EXCEPT the 3 Phase 11 correctness fixes (audited)
- [ ] `ynz build` default human output byte-identical to pre-M5; `ynz build --json` is opt-in
- [ ] VSCode extension v0.2.0 .vsix builds + installs + works
- [ ] Cargo.toml at `0.2.0` (no `-mN` suffix); tag `v0.2.0` cut
- [ ] Every phase received a code-reviewer PASS before committing (Step 9a)
- [ ] Final cumulative code-reviewer sweep passed (Step 10f)
- [ ] Plan-file acceptance-criteria checkboxes accurate across all 13 phases (0-12)
- [ ] Roadmap rollup checked: all v0-2-dev-loop-tooling milestones in `done/`; roadmap status promoted with Patrick's approval

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: Each of the 13 phases is one branch / one PR per `branching.md` §The Three Layers. Per-phase code-reviewer guards the boundary. No "10 PRs of 50 lines"; no "1 PR of 8000 lines."
- **Shadow main branches**: Each phase branches from `main`, merges to `main` on PASS. No long-lived `feature/m5-everything` branch. The v0.2.0 tag is cut from `main` after all phases land.
- **Building the engine before shipping value**: Phase 2 (go-to-def) is user-visible value the moment it ships. Phases 3, 4, 5, 6, 7, 8 each ship a discrete user-facing LSP capability. Phase 1 is the only infrastructure phase, and it's <700 lines and tightly scoped to the 3 helpers Phases 2-4 need.
- **Hotfix that isn't**: Phase 11's three bug-fixes are genuine correctness improvements — each has a fail-on-baseline test that confirms the bug existed. Not "while I'm here" creep — they're the explicit `todos.md` "Soon" entries with the trigger "v0.2 LSP work."
- **Abandoned branches**: Per-phase PR cycle ensures branches are short-lived. No branch sits >2 weeks without a merge or explicit pause-with-reason.
- **Flag graveyards**: Zero feature flags this milestone. Yinz compiler doesn't have a flag system; all changes ship enabled. The "flag" concept doesn't apply here.

## Reviewer History

### Round 1 (2026-05-20) — plan-reviewer
**Verdict**: BLOCK with 10 Required Fixes, 5 Concerns, 6 suggested Adversarial cases.

**All 10 Required Fixes addressed (no disputes — every fix was a legitimate gap):**
1. Phase 0 deferred-tooling-feature count claim: corrected `5 → 10` (was wrongly stated as `3 → 8`); verified via `grep -c '^\[\[deferred_tooling_feature\]\]' registry/features.toml` = 5.
2. Phase 11a hidden-field audit procedure: added explicit Step 1 audit script committed as `crates/ynz-codegen/tests/audit_hidden_field_defaults.rs`; results documented inline including the "AUDIT FINDING: none" path.
3. Phase 11 bundling split into Phase 11a / 11b / 11c (three independent PRs); the three fixes have no shared infrastructure and the bundling-without-tradeoff framing was duct-tape per `no-duct-tape.md`.
4. Phase 6 `let_to_const` `.give` case added to suppression rules; `.give` adversarial test enumerated in Acceptance criteria.
5. Phase 9 `--json` schema locked: span shape (`file: path`, `start_byte: u32`, `end_byte: u32`, half-open UTF-8 byte offsets), severity (lowercase string literals), null handling (`skip_serializing_if = "Option::is_none"`, never emit `null`/`NaN`/`Infinity`), encoding (UTF-8 stdout, LF line terminators).
6. Phase 5 format-on-save silent-empty-edits replaced with `window/showMessage` info-level notification on parse-error path; quality gate updated.
7. Phase 10 `leading_docs` field changed from bare `Vec<String>` to `Option<Vec<String>>`; construction-site enumeration added to Step 1 (grep affected sites; add `leading_docs: None` to each).
8. Phase 7 `lsp_code_action_label_for` adapter: added explicit `// CARVE-OUT: <reason>` comment requirement per `.claude/rules/feature-registry.md` carve-out policy; rationale documented.
9. Phase 3 progress-emission threshold locked: `state.open_documents.len() > 10` OR new helper `cross_file_reference_count_estimate > 5`; no hand-waved "estimate from file count" framing.
10. Phase 11 adversarial cases added across all three sub-phases: nested struct default; ConcreteFoo-NOT-follows-Foo over-acceptance regression; share-on-const non-over-reject.

**All 5 Concerns addressed:**
- Concern 1 (salsa cancellation): `lsp-salsa-cancellation` todo added to `.claude/todos.md` "Later" with trigger.
- Concern 2 (semantic-tokens TM comparison mechanism): Phase 8 test step adds explicit TM-keyword-rule matcher comparison; identifier-type disagreement explicitly EXCLUDED from the test (it's expected).
- Concern 3 (LspRenameError codes): explicit code-to-variant table added in Phase 4 Quality Gate using LSP-reserved `-32001` to `-32006` range.
- Concern 4 (CRLF line endings): Phase 5 line-ending policy section locked (Yinz files LF-only; `ynz-fmt::format` normalizes CRLF→LF).
- Concern 5 (`--clobber` for `yinz-latest.vsix`): Phase 12 Step 10 explicitly invokes `gh release upload --clobber`; acceptance criterion updated.

**All 6 suggested Adversarial cases folded in:**
- `.give` for let_to_const (Phase 6)
- ConcreteFoo NOT follows Foo (Phase 11b)
- Nested hidden-field default (Phase 11a)
- Rename atomic-or-fail under concurrent didChange (Phase 4)
- References shadowing (Phase 1)
- Newline-in-diagnostic-message escape (Phase 9)

**Net result**: phase count went from 13 (P0-P12) to 15 (P0-P12 with P11 split into P11a/b/c). Plan grew ~250 lines (audit procedures, locked schemas, adversarial test descriptions).

### Round 2 (2026-05-20) — plan-reviewer
**Verdict**: BLOCK with 2 Required Fixes, 3 Concerns, 3 suggested Adversarial cases.

**Both Required Fixes addressed (no disputes — both legitimate gaps):**
1. `cross_file_reference_count_estimate` was referenced in Phase 3 Step 2 but not owned anywhere → moved to Phase 1's public API (5th salsa-tracked helper); acceptance criteria + performance test + salsa-tracked grep-count updated.
2. LSP error code `-32802` was wrong (citation `ResponseError(-32802 ContentModified)` in Phase 4 rename adversarial) → corrected to `-32801` (per LSP 3.17 spec; `-32802` is `ServerCancelled`); also added note to use `lsp_types::error_codes::CONTENT_MODIFIED` constant rather than magic number.

**3 Concerns addressed:**
- Concern 1 (Phase 11a audit fail-class subtlety): accepted as-is per reviewer; the manual cross-check escape hatch is explicit in the Quality Gate; flagged for executor diligence.
- Concern 2 (Phase 8 TM-grammar substring-search helper): accepted as-is per reviewer (test scope is keyword-agreement-only; identifier-type disagreement is expected and excluded).
- Concern 3 (Phase 5 CRLF normalization UX surprise): added to Phase 12 CHANGELOG acceptance criterion — release notes call out the LF-only behavior so Windows users aren't blindsided by first-format-on-save mega-diff.

**3 Adversarial cases folded in:**
- Phase 4 re-exported imported symbol: locked behavior = REJECT (both bare and aliased re-exports); aliased re-export rename deferred to v0.3+ via new `lsp-rename-aliased-re-export` todo.
- Phase 6 viewport-filter partial-line range: locked behavior = position-only (matches rust-analyzer); fixture added.
- Phase 9 `--json` exit code: locked behavior = matches `ynz build` exit semantics (0 if no errors regardless of warnings); pinned now so v0.3+ can't silently change the policy.

**Net result**: 2 BLOCK fixes were small edits; plan grew ~50 lines (helper API addition, adversarial test fixtures, exit-code lock). Phase count unchanged at 15.

### Round 3 (2026-05-20) — plan-reviewer
**Verdict**: BLOCK with 3 drift-defect Required Fixes + 3 non-blocking Concerns + 3 suggested Adversarial cases.

**All 3 Required Fixes addressed (no disputes — all were round-by-round patching artifacts):**
1. Phase 1 Verification block (lines 536-537) said `>= 4 pub fn` / `>= 4 #[salsa::tracked]` while Acceptance Criteria (lines 511, 520) said `>= 5` after the Round 2 estimator addition → updated Verification to `>= 5` for both.
2. Phase 3 Step 2 still called `cross_file_reference_count_estimate` "a new lightweight helper" even though Round 2 moved its ownership to Phase 1 → rephrased to "the Phase-1-owned helper (see Phase 1 Files-expected-scope)... Phase 3 CONSUMES this helper; it does NOT add it."
3. Round 3's new deferred-tooling concept (`lsp-rename-aliased-re-export`) was added to `.claude/todos.md` but not to Phase 0's registry-entry list per `.claude/rules/feature-registry.md` Required Entry Types Checklist → added as 6th `[[deferred_tooling_feature]]` in Phase 0; grep-count verification bumped from 10 → 11; acceptance criterion updated.

**3 Concerns acknowledged (non-blocking per reviewer):**
- Concern 1 (Phase 0 entry-count brittle invariant): noted; future plan-revisions to Phase 0 entry list will need to keep step-list, Files-expected-scope, acceptance criterion, and verification grep-count all in sync.
- Concern 2 (Phase 0 hardcoded entry-names in step-list): noted; current list is small enough (6) that inline naming is acceptable. Future plans may extract to a top-of-phase list if N grows.
- Concern 3 (aliased-re-export "codebase pattern emerges" trigger is vague): noted; the user-requests trigger is the primary trigger and is concrete.

**Round 3 suggested Adversarial cases — punted to follow-up todos rather than scope-creeping the plan:**
- Phase 1 `references_for_offset` on circular import → adding to `.claude/todos.md` "Later" as `lsp-references-circular-import-termination` with trigger = first user report of editor freeze on cyclic imports OR pre-emptively when Yinz's import-cycle detection ships in v0.3.
- Phase 4 rename with shadowing-at-call-site → adding to `.claude/todos.md` "Later" as `lsp-rename-call-site-shadowing-detection`; the `ConflictsWithExistingName` check today covers same-file conflicts but not per-call-site scope-shadowing; trigger = user report or v0.3 typeck adds per-call-site scope-walk API.
- Phase 9 `--json` output on zero-diagnostics → folded into Phase 9 Step 7 inline: add explicit "zero diagnostics" fixture asserting output = just the `summary` line with all counts = 0.

**Net result**: 3 BLOCK fixes were mechanical drift-defect repairs; plan grew ~20 lines. Phase count unchanged at 15. Hit the Round-3 cap — awaiting Patrick arbitration on whether to invoke Round 4 (reviewer is likely to PASS given the trivial nature of remaining drift) or ship.

### Round 4 (2026-05-20) — plan-reviewer — **PASS**
**Verdict**: PASS (Patrick-authorized extension past the standard 3-round cap given Round 3 BLOCK was drift-defects, not substantive issues).

**Tier**: A (correctness-critical) — locked since Round 1.

**Required Fixes**: NONE. Plan ready to implement.

**Reviewer-confirmed verification**:
- Phase 1 lines 536-537 verified `>= 5` for both `pub fn` and `#[salsa::tracked]` counts (internal contradiction with Acceptance Criteria lines 511, 520 resolved)
- Phase 3 Step 2 line 659 verified "Phase-1-owned helper... Phase 3 CONSUMES this helper; it does NOT add it" (cross-phase ownership unambiguous)
- Phase 0 line 364 verified `lsp-rename-aliased-re-export` as 6th `[[deferred_tooling_feature]]`; arithmetic consistent (5 pre-M5 + 6 added = 11 verified at line 407)
- Round 3 Adversarial follow-ups verified externalized in `todos.md` (lines 66, 68) with concrete triggers
- Round 3 Adversarial #3 verified folded inline at Phase 9 Step 7b (line 1102)

**Non-blocking Concerns from Round 4 reviewer (acknowledged)**:
- Concern 1: `last_updated:` was stale at review time → bumped to reflect Round 4 PASS in this final update.
- Concern 2: Phase 9 step numbering `7` / `7b` (vs full renumber) — cosmetic; acceptable at the cap.
- Concern 3: Round 1 Reviewer History "5 → 10" archival note (vs current "5 → 11") — correctly retained as historical snapshot of Round 1's state, not live spec.

**Two new Adversarial cases the reviewer flagged as future-pass material (not blockers)**:
- Phase 4 prepareRename-vs-rename agreement contract test (low-risk; clients conform; pin for future regression prevention).
- Phase 6 inlay-hint emission inside comment regions assertion (structurally impossible today; lock prevents future parser changes from breaking it).
- Both folded into a future v0.2-M5 polish or v0.3 hardening pass via `.claude/todos.md` "Later" if/when they bite — NOT scope-crept into this milestone.

**Net result across 4 rounds**: 13 Phases → 15 Phases (P11 split into 11a/b/c). Plan grew from ~1140 lines → ~1600 lines. Every drift defect and silent-failure trigger explicitly locked. Tier A correctness gates intact. **PASS — plan ready for execution.**
