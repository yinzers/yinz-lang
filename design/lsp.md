# LSP Architecture — ynz-lsp Design Reference

This doc is for **compiler contributors and milestone implementers**, not Yinz language users. It covers how `ynz-lsp` is structured, why it works the way it does, and what is deferred to v0.2-M5.

User spec reference: N/A (tooling internals)
Plan reference: `.claude/plans/done/v0-2-m2-lsp-thin-slice.md` (M2); `.claude/plans/active/v0-2-m5-lsp-full-and-release.md` (M5)

Cross-references: `design/compiler-language.md`, `design/feature-registry.md`, `design/teaching-mission.md`, `.claude/rules/inference.md`, `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md`

---

## Salsa Wiring

The LSP and CLI share a single compiler pipeline. There is no separate LSP pipeline — the LSP is `ynz-lsp`, a binary that wraps existing salsa queries over JSON-RPC.

**Single CompilerDb owned by the server, not reconstructed per request.**

`CompilerDb` (defined in `crates/ynz-parser/src/db.rs`) is constructed once at server startup and lives for the lifetime of the process. When the client sends a `textDocument/didChange`, the server calls:

```rust
source_file.text().set(&mut db).to(new_text);
```

Salsa immediately invalidates all queries that depend on that source file's text. The NEXT query against the DB for that file re-runs from the parse step forward. Queries against OTHER files hit the memoized cache.

**Tracked queries the LSP uses** (all defined before M2; LSP is a consumer, not a producer):

| Query | Crate | Used for |
|---|---|---|
| `parse_query(db, source) -> Arc<ParseResult>` | `ynz-parser/src/queries.rs:25` | Token-at-offset for hover (Phase 5) |
| `module_signatures_query(db, source) -> Arc<SignatureOutput>` | `ynz-typeck/src/queries.rs:49` | User-defined symbol completion (Phase 4), hover fallback (Phase 5) |
| `check_query(db, source) -> Arc<CheckOutput>` | `ynz-typeck/src/queries.rs:100` | Diagnostics (Phase 3) — deepest typeck pass |

The LSP does NOT run `codegen_query`. The LSP server never emits machine code. This is intentional and permanent — it keeps startup fast and avoids `inkwell` (LLVM bindings) in the LSP binary.

**SourceFile input registration:**

```rust
// On didOpen:
let sf = SourceFile::new(&mut db, uri_to_path(&uri), text);
db.source_registry_mut().insert(path.clone(), sf);

// On didChange (apply incremental edits to shadow text, then write):
source_file.text().set(&mut db).to(new_full_text);
// salsa invalidates downstream queries automatically
```

---

## JSON-RPC Dispatch Model

**Thin slice (v0.2-M2): single-threaded dispatch.**

All `textDocument/*` requests AND notifications serialize through one worker task that owns `&mut CompilerDb`. No `Arc<Mutex<...>>` over the DB.

Rationale: salsa inputs require `&mut db` access. Salsa is `Send` but not `Sync` by default. The simplest sound model is one owner. For one Patrick + one editor, this matches the throughput needs perfectly.

**Concurrency semantics (locked):**

- In-flight queries complete before the next mutation. Salsa's tracked queries are not cancellable mid-execution in the thin slice.
- If `didChange` arrives while a previous response is being computed: the worker drains pending mutations BEFORE replying. The in-flight response reflects state N-1 but is sent unmodified; the next request sees post-mutation state.
- For requests the LSP spec marks cancellable (`completion`, `hover`): `$/cancelRequest` is handled BEST-EFFORT. If the query hasn't started (still queued), it's dropped and a `RESPONSE_ERROR_REQUEST_CANCELLED` is returned. If already running, it completes; the client receives the late response (LSP spec permits this).

**v0.2-M5 note:** When go-to-def / rename / find-refs scale horizontally (multi-window, background analysis), this model is revisited with salsa snapshots. See `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` M5 entry.

---

## Capability Negotiation

The server's `initialize` response advertises exactly these capabilities:

```json
{
  "capabilities": {
    "positionEncoding": "utf-8",
    "textDocumentSync": {
      "openClose": true,
      "change": 2
    },
    "completionProvider": {
      "triggerCharacters": [".", " "],
      "resolveProvider": false
    },
    "hoverProvider": true
  }
}
```

**What is NOT advertised (deferred to v0.2-M5):**

- `definitionProvider` — go-to-def
- `referencesProvider` — find-refs
- `renameProvider` — rename
- `documentFormattingProvider` — format-on-save (needs v0.2-M3 `ynz-fmt`)
- `inlayHintProvider` — muted-hint surfaces per `.claude/rules/inference.md`
- `codeActionProvider` — code actions
- `semanticTokensProvider` — semantic highlighting beyond TextMate

**Diagnostics model: PUSH.** The server publishes diagnostics via `textDocument/publishDiagnostics` on `didOpen` and `didChange`. It does NOT wait for the client to request them (pull model via `textDocument/diagnostic` is v0.2-M5).

---

## Position Encoding Strategy

LSP 3.17 introduced `general.positionEncodings` capability negotiation. The server advertises `["utf-8", "utf-16"]` and picks UTF-8 if the client supports it.

**Why UTF-8 is preferred:** The compiler's internal `SourceSpan` is byte-offset-based (`start: usize, end: usize`). UTF-8 means LSP Position characters == bytes. UTF-16 requires an extra pass over the line to count code units, and surrogate pairs (4-byte UTF-8 codepoints like emoji) count as TWO code units in UTF-16 but ONE character in UTF-8.

**UTF-16 fallback:** Required for clients that don't support UTF-8 position encoding (VSCode supports it since v1.80+; LSP clients before that era are UTF-16 only). The fallback path counts code units via `unicode-segmentation` (already a workspace dep).

**Byte-offset → LSP Position conversion** is handled by `crates/ynz-lsp/src/position.rs`. The converter pre-builds a line-offset table at `didOpen`/`didChange` so N spans from the same document cost O(log lines) each, not O(file size) each.

---

## SSOT Registry as the Autocomplete + Hover Source

Both the CLI compiler and `ynz-lsp` read from the same SSOT registry (`registry/features.toml`, parsed by `crates/ynz-registry/build.rs`). The registry is the single source of truth for keyword lists, banned-jargon words, primitive intrinsics, type-attached constants, deferred-feature catalog, and IDE muted-hint domains.

**This means:**

- Adding a new keyword to `registry/features.toml` makes it appear in autocomplete AND in hover AND as a syntax-highlighted token (via the TextMate grammar generator) — no `ynz-lsp` code changes needed.
- Renaming an intrinsic updates all three surfaces in one edit.
- The drift class ("LSP knows keywords the compiler doesn't, or vice versa") is architecturally impossible once the registry is the source.

**LSP-specific registry adapters (added in v0.2-M2, defined in `crates/ynz-registry/src/lsp_adapter.rs`):**

- `lsp_completion_items(context: CompletionContext) -> Vec<RegistryCompletionItem>` — returns completion items for the given context, sourced from KEYWORDS, PRIMITIVE_INTRINSICS, TYPE_ATTACHED_CONSTANTS, DEFERRED_LANGUAGE_FEATURES (marked deprecated).
- `lsp_hover_for_token(name: &str) -> Option<HoverContent>` — returns hover content for any registered token name. Falls back to typeck symbol lookup if the registry returns `None`.

The `ynz-registry` crate does NOT depend on `lsp-types`. It returns registry-owned mirror types (`RegistryCompletionItem`, `HoverContent`). The LSP crate translates these to `lsp_types::CompletionItem` etc. This isolation keeps `ynz-registry` as a foundational crate with no editor-tool deps.

---

## Framework Choice (Decided in Phase 1 Spike)

*This section is populated by Phase 1. The placeholder below documents what the spike measures and the decision criterion.*

**Candidates:**
- `tower-lsp` (lebensterben/tower-lsp-server, the maintained fork as of mid-2026): async via tokio, `LanguageServer` trait with `async fn` request handlers, JSON-RPC routing via `jsonrpsee`.
- `lsp-server` (rust-analyzer crate): synchronous, lower-level — you own the `Connection` and dispatch loop. Smaller dep tree.

Both consume `lsp-types` (official Microsoft-blessed type definitions).

**Decision criterion:** Choose the framework with the smaller plumbing+test footprint while not forcing async semantics over the salsa DB. If both pass, default to `tower-lsp` (more active, more sample code, simpler request-handler shape).

**Locked decision: `lsp-server = "0.7.9"` (rust-analyzer crate)**

Rationale (from `crates/ynz-lsp/_spike/MEASUREMENTS.md`):
- **Natural `&mut CompilerDb` ownership**: lsp-server's synchronous dispatch loop owns the DB directly. Every `didChange` mutation and every `check_query` read is a plain function call — no `Arc<Mutex<...>>` wrapper needed.
- **Smaller footprint**: 102 transitive deps, 17 MB debug binary vs 189 deps / 53 MB for tower-lsp. Less to audit, less to compile.
- **Simpler test harness**: in-process sync tests call handler functions directly; no `tokio::test` runtime setup.
- **Better maintenance**: `lsp-server 0.7.9` published 2024-07-12 by the rust-analyzer team (actively maintained). `tower-lsp 0.20.0` published 2023-09-11; original repo declared unmaintained; maintained fork `tower-lsp-f 0.25.0-beta3` is beta.
- **Architecture fit**: single-threaded dispatch (which our model requires) maps naturally to lsp-server's sync model. tower-lsp's async model adds boilerplate that adds no value for this architecture.

**This choice carries through to v0.2-M5.** When M5 adds go-to-def/rename/find-refs, the sync model scales via salsa snapshots (`Snapshot<CompilerDb>` for concurrent reads; main loop retains `&mut CompilerDb` for mutations). No framework migration needed.

---

## Diagnostic Rendering: WHAT/WHAT-INSTEAD/WHY Preserved End-to-End

The Yinz compiler enforces WHAT/WHAT-INSTEAD/WHY structure in every diagnostic (see `design/teaching-mission.md` and `design/compiler-errors.md`). The LSP must NOT lose this structure when transforming `ynz_diagnostics::Diagnostic` to `lsp_types::Diagnostic`.

**Plaintext format** (v0.2-M2):

```
{what}\n\nWHAT INSTEAD: {what_instead}\n\nWHY: {why}
```

VSCode renders plaintext newlines as soft breaks in squiggle tooltips. The format is verified by Phase 3's integration tests, which assert the LSP message contains each substring.

**Structured `Diagnostic.data` field** (v0.2-M5): if the client supports it, the data field will carry the three components as a JSON object, enabling per-client custom rendering of WHAT/WHAT-INSTEAD/WHY in a richer UI. The plaintext `message` stays as the canonical fallback. See the Deferrals table in the M2 plan.

**Delimiter collision mitigation** (two layers):
1. Registry consistency test asserts no diagnostic template field contains `"WHAT INSTEAD:"` or `"\n\nWHY:"` as a substring.
2. `debug_assert!` in the transform function catches dynamic-construction collisions at debug-build time.

---

## TextMate Grammar Generator

The file `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` is generated by `crates/ynz-tmgrammar`, a standalone binary. It reads `ynz-registry` and emits grammar rules:

- Keywords → `keyword.*` scope (normal highlighting)
- Banned-declaration-keywords → `invalid.deprecated` scope (deprecated visual)
- Deferred-language-features → `invalid.illegal` scope (illegal visual — not a Yinz term yet)
- Literals, strings, comments → standard TM scopes

The grammar is committed to the repo. A consistency test in `crates/ynz-tmgrammar/tests/grammar_snapshot.rs` re-runs the generator and fails if the committed file drifts from the registry state.

CI fails if a keyword is added to the registry without regenerating the grammar. This is the "drift class goes away by construction" pattern applied to syntax highlighting.

---

## Self-Hosting Migration Plan

`ynz-lsp` is currently written in Rust (the bootstrap compiler's language). It will eventually be rewritten in Yinz when the language reaches self-hosting (v2+).

**Migration prerequisites:**
- `json` stdlib module (v0.8) — needed for JSON-RPC message parsing
- Network/stdio I/O primitives — needed for the stdio transport layer
- Self-hosted `ynz-registry` — the TOML parser and codegen loop, currently in `build.rs`

This mirrors the self-hosting plan in `design/feature-registry.md`. The Rust `ynz-lsp` runs as the production implementation until the Yinz-hosted version reaches feature parity. At that point, `crates/ynz-lsp/` is deprecated and eventually deleted.

**No M2 work needed.** The Rust implementation is the permanent v0.x LSP. Self-hosting is a v2+ concern.

---

## Targeted for v0.2-M5

The following LSP capabilities are out of scope for v0.2-M2. All are planned for v0.2-M5:

- `textDocument/definition` — go-to-def (cross-file span lookup via planned `symbol_lookup.rs`)
- `textDocument/references` — find-refs (cross-file use-site tracking)
- `textDocument/rename` — atomic `WorkspaceEdit` with pre-validate via `prepareRename`
- `textDocument/formatting` — format-on-save delegating to `ynz-fmt::format`
- `textDocument/inlayHint` — muted-hint surfaces (9 domains; 5 firing, 4 protocol-only)
- `textDocument/codeAction` — quick-fixes from registry WHAT-INSTEAD content
- `textDocument/semanticTokens` — semantic highlighting richer than TextMate
- Doc-comment integration in hover body (`///` leading-docs attached to declarations)
- `Diagnostic.code` + structured `Diagnostic.data` fields for client-side rendering
- `ynz build --json` — structured NDJSON diagnostic output for tooling consumers

---

## Capabilities Planned for M5

### `textDocument/definition` — Go-to-Definition

**What it will do**: User Cmd+clicks a symbol → LSP returns a `Location` pointing to the declaration site.

**Salsa queries**: `parse_query` (AST-node-at-offset) + `module_signatures_query` (function/shape declarations) + planned `symbol_lookup::def_site_for_offset` query (Phase 1).

**Intended behavior**: Jump for user-defined shapes, functions, options types, constants, and local `let` bindings. Works across files when the symbol is imported — click an imported name, jump to the source file where it's declared.

**What is NOT in M5**: jump into stdlib intrinsics (they are compiler-internal, no source file to jump to); jump into `dynamic` dispatch target (runtime-resolved, can't statically determine in M5).

### `textDocument/references` — Find All References

**What it will do**: User right-clicks a symbol → "Find All References" returns every use-site across all open project files as a list of `Location` values.

**Salsa queries**: planned `symbol_lookup::references_for_offset` (Phase 1) — walks every open `SourceFile`'s AST to find use-sites that resolve to the same canonical symbol.

**Intended behavior**: References panel shows all call sites, type annotations, import clauses that reference the symbol. `include_declaration: true/false` flag honored (LSP spec).

**Performance target**: emits `$/progress` notification for scans expected to exceed 1s (when `open_documents.len() > 10` or `cross_file_reference_count_estimate > 5`).

**What is NOT in M5**: references inside string literals; references across files not open in the editor (workspace-wide index not yet built).

### `textDocument/rename` + `textDocument/prepareRename` — Atomic Rename

**What it will do**: F2 on a symbol → user types a new name → every reference in every open file is updated atomically via a `WorkspaceEdit`.

**Salsa queries**: planned `symbol_lookup::rename_locations` (Phase 1) — validates the new name, finds all locations, rejects if origin is an imported symbol or if the name conflicts.

**Intended behavior**: `prepareRename` response shows the symbol's name range (not full declaration), so the editor highlights exactly what will be renamed. Rename is rejected with a specific error for: invalid identifier, reserved keyword, banned jargon, imported symbol (must rename at origin file), name conflict.

**Concurrency**: rename will use the single-threaded dispatch model (M2-locked). The `WorkspaceEdit` is built against the snapshot at the moment the request was received — no partial-mutation risk.

**Progress target**: emits `$/progress` for renames spanning >3 files.

**What is NOT in M5**: renaming a field inside a shape declaration; renaming an aliased re-export (`export { Crew as Captain }` — see `registry/features.toml` `lsp-rename-aliased-re-export` deferred entry).

### `textDocument/formatting` — Format on Save

**What it will do**: User saves a `.ynz` file with format-on-save enabled → LSP returns a `TextEdit` replacing the entire file with the `ynz-fmt`-formatted version.

**API**: delegates to `ynz_fmt::format(source) -> Result<String, FmtError>` (already shipped in M3). If `format(source) == source`, returns an empty `Vec<TextEdit>` (no change event; no noise).

**Error handling design**: if the file has parse errors, return empty edits AND emit a `window/showMessage` Info notification explaining that format-on-save is skipped until syntax errors are fixed.

**Line endings**: Yinz files are LF-only by spec. `ynz-fmt::format` normalizes CRLF→LF on input. First format-on-save on a CRLF file produces a large diff (normalization); subsequent saves are no-ops.

**Range formatting**: `format_range(source, range)` will be added to `ynz-fmt` in M5 (Phase 5); LSP `textDocument/rangeFormatting` delegates to it. Formats the tightest containing top-level item if the range is mid-item.

**What is NOT in M5**: formatting when LSP is not running (use `ynz fmt` CLI). Streaming incremental formatting. Format timeout (format is fast enough; no timeout needed at M5 scale).

### `textDocument/inlayHint` — Muted Teaching Hints

**What it will do**: LSP returns `InlayHint` objects for the viewport range, showing what the compiler figured out automatically. See **Inlay Hints** section below for the full 9-domain breakdown.

**Performance design**: request includes `range` parameter (viewport). Server filters to hints whose byte-offset falls within the viewport. Salsa-cached per-file typeck pass; re-render is cheap.

**What is NOT in M5**: inlay hints inside macro expansions (no macros in v0.1); configurable per-domain toggle (v0.3+ IDE preferences).

### `textDocument/codeAction` — Quick-Fixes from WHAT-INSTEAD

**What it will do**: when the cursor is on a diagnostic, the editor shows a "Quick Fix" lightbulb. LSP returns `CodeAction` objects with the fix pre-built as a `WorkspaceEdit`.

**Design**: code-action labels and replacement text come from the registry's `[[banned_declaration_keyword]]` and `[[banned_jargon]]` entries' `what_instead` field. Only diagnostics with an unambiguous single-token replacement get a quick-fix. Multi-step fixes (e.g., "move this to a separate file") are not quick-fixable in M5.

**Label format**: `"Replace \`<original>\` with \`<replacement>\`"` — consistent across all fixable diagnostic kinds.

**What is NOT in M5**: code actions that add imports, create new functions, or do multi-file structural edits.

### `textDocument/semanticTokens` — Richer-than-TextMate Highlighting

**What it will do**: emit token-type classifications (KEYWORD, TYPE, FUNCTION, VARIABLE, PARAMETER, PROPERTY, ENUM_MEMBER, NUMBER, STRING, COMMENT) for every token in the file. VSCode applies theme colors per type.

**Relationship to TextMate grammar**: semantic tokens REFINE TextMate. TM grammar handles syntax structure; semantic tokens handle identifier classification (is `foo` a function or a variable? TM can't tell; semantic tokens can, using typeck data). Keyword token ranges will be kept in sync with the TM grammar's keyword rules (tested via a keyword-agreement assertion in Phase 8).

**Wire format**: delta-encoded per LSP spec (`SemanticTokens.data` = array of `[deltaLine, deltaStart, length, tokenTypeIndex, tokenModifiers]` 5-tuples).

**What is NOT in M5**: modifiers (readonly, deprecated, async); per-file semantic-token caching invalidation (M5 re-emits on every `didChange`).

---

## Inlay Hints

All 9 registry `[[muted_hint_domain]]` entries will have LSP protocol handlers in M5. 5 will fire data; 4 will return empty hint lists (protocol-only, awaiting v0.3+ analysis data).

See `.claude/rules/inference.md` for the three placement categories (Addition / Replacement / Informational) and the full protocol spec.

| Domain | Status | Placement | What it shows | Firing condition | When it gets data |
|---|---|---|---|---|---|
| `variable_type` | Firing | Addition | `: int (from 42)` after `x` in `let x = 42` | Always — every unannotated `let` | Today |
| `ownership_call_site` | Firing | Informational | `share (matches foo's signature)` after arg | Every call site where callee signature has `share`/`lend`/`give` | Today |
| `copy_points` | Firing | Informational | `.copy (8 bytes, trivially copyable)` | Trivially-copyable arg passed while binding still live afterward | Today |
| `array_to_fixed_promotion` | Firing | Replacement | `// promoted to fixed<int, 3> — never grown` | `let x: array<T> = [...]` where no `.add()` / `.lend` found in scope | Today |
| `let_to_const_promotion` | Firing | Replacement | `// effectively const — never reassigned` | `let x = ...` where no reassignment / `.lend` / `.give` found in scope | Today |
| `function_param_type` | Protocol-only | Addition | (empty — awaiting lambdas) | N/A until lambdas ship | v0.3+ (lambdas milestone) |
| `wait_points` | Protocol-only | Addition | (empty — awaiting I/O suspension analysis) | N/A until `wait`-auto-insertion analysis ships | v0.3 (concurrency milestone) |
| `lifetimes` | Protocol-only | Informational | (empty — awaiting explicit lifetime UI) | N/A until explicit lifetimes have a user-facing surface | v0.3+ (may stay fully implicit) |
| `allocators` | Protocol-only | Addition | (empty — awaiting arena keyword) | N/A until `arena scratch { }` ships | v0.3 (per `design/future/arena.md`) |

Cross-reference: `design/ide-hints.md` for the protocol spec; `registry/features.toml` `[[muted_hint_domain]]` entries for the canonical domain definitions.

---

## Concurrency: Rename + Progress Notifications (Design for M5)

**Single-threaded dispatch model (unchanged from M2)**: all requests serialize through one worker that owns `&mut CompilerDb`. In-flight requests complete before the next mutation.

**Rename atomicity** (Phase 4 design): `rename_response` will build the entire `WorkspaceEdit` before returning. If `rename_locations` returns `Err`, NO edits will be sent — the response is a `ResponseError`. Partial-application is impossible server-side; if the client applies partial edits and crashes, that is the client's responsibility per LSP spec.

**Progress notifications** (`$/progress`) (Phase 3 design): will be emitted by `ProgressTracker` for operations expected to take >1s:
- `textDocument/references` when `open_documents.len() > 10` OR `cross_file_reference_count_estimate > 5`
- `textDocument/rename` when the rename spans >3 files

Progress tokens will be unique per request (monotonic counter). `ProgressTracker::begin` will return a token; `end` is always called on the same token, paired.

**Future concurrency** (v0.3+): salsa snapshots (`Snapshot<CompilerDb>`) enable concurrent reads while the main loop retains `&mut` for mutations. References and rename are the first candidates. See `todos.md` `lsp-salsa-cancellation` entry for the trigger condition.

---

## `ynz build --json` — Structured Diagnostic Output (Planned for M5 Phase 9)

Will be added as an opt-in flag on `ynz build` in Phase 9. Will suppress ariadne human output; emits NDJSON to stdout. Default `ynz build` output is byte-identical to pre-M5 (flag is strictly opt-in).

**Schema** (locked in plan; emits `schema_version: "v0.2.0-m5-unstable"` until Phase 12 drops the `-unstable` suffix at the v0.2.0 tag-cut):

```json
{"type": "diagnostic", "schema_version": "v0.2.0-m5-unstable", "severity": "error" | "warning" | "suggestion", "kind": "<DiagnosticKindName>", "code": "<DiagnosticKindName>", "span": {"file": "<absolute-path>", "start_byte": 0, "end_byte": 10}, "message": "<WHAT>\n\nWHAT INSTEAD: ...\n\nWHY: ...", "data": {"what": "...", "what_instead": "...", "why": "..."}}
{"type": "summary", "schema_version": "v0.2.0-m5-unstable", "errors": 0, "warnings": 0, "suggestions": 0, "exit_code": 0}
```

**Field semantics** (locked in plan; implemented in Phase 9):
- `span.file`: absolute filesystem path (UTF-8)
- `span.start_byte` / `span.end_byte`: UTF-8 byte offsets, 0-indexed, half-open (start inclusive, end exclusive)
- `severity`: lowercase string literal (`"error"` / `"warning"` / `"suggestion"`)
- `kind` and `code`: identical string — the registered `DiagnosticKind` name (PascalCase)
- `message`: full plaintext rendering with embedded `\n` escaped in JSON
- `data`: structured object; all three fields always present (never null)
- `summary.exit_code`: `0` if `errors == 0`; non-zero otherwise. Warnings and suggestions do NOT trigger non-zero exit

**Encoding**: UTF-8 stdout, LF line terminators, one JSON object per line.

Cross-reference: this section is the canonical schema reference; `crates/ynz-driver/src/json_diagnostic.rs` will be the implementation (created Phase 9).
