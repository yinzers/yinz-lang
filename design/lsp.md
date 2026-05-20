# LSP Architecture — ynz-lsp Design Reference

This doc is for **compiler contributors and milestone implementers**, not Yinz language users. It covers how `ynz-lsp` is structured, why it works the way it does, and what is deferred to v0.2-M5.

User spec reference: N/A (tooling internals)
Plan reference: `.claude/plans/active/v0-2-m2-lsp-thin-slice.md`

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

## Deferred to v0.2-M5

The following LSP capabilities are explicitly out of scope for v0.2-M2. Each will be added in v0.2-M5's execution plan:

- `textDocument/definition` — go-to-def (requires cross-file span lookup)
- `textDocument/references` — find-refs (requires cross-file use-site tracking)
- `textDocument/rename` — requires careful salsa invalidation across files
- `textDocument/formatting` — format-on-save; delegates to v0.2-M3's `ynz-fmt` library
- `textDocument/inlayHint` — muted-hint surfaces per `.claude/rules/inference.md` three placement categories (Addition / Replacement / Informational)
- `textDocument/codeAction` — code actions and quick-fixes
- `textDocument/semanticTokens` — semantic highlighting richer than TextMate
- Doc-comment integration in hover body (rich `///` comment content in `textDocument/hover`)
- Pull-diagnostics model (LSP 3.17 `textDocument/diagnostic` pull model as alternative to push)
- `Diagnostic.code` + `Diagnostic.codeDescription` fields (for DiagnosticKind name linking)
- Structured `Diagnostic.data` field (for client-side WHAT/WHAT-INSTEAD/WHY rendering)
