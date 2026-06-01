//! `textDocument/documentSymbol` and `workspace/symbol` handlers.
//!
//! `documentSymbol` walks the parsed module's top-level items and returns a
//! hierarchical list the client uses to populate the Outline and breadcrumb views.
//!
//! `workspaceSymbol` iterates every registered source file, collects top-level
//! items, filters by a case-insensitive substring query, and returns a flat list.

use lsp_types::{DocumentSymbol, Location, Range, SymbolInformation, SymbolKind};
use ynz_ast::nodes::Item;
use ynz_parser::SourceFileRegistry as _;

use crate::{
    position::LineTable,
    state::{uri_for_source_file, ServerState},
};

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a `DocumentSymbol` for a single name/kind/span triple extracted from an AST item.
///
/// `detail` is the optional secondary line shown in the Outline panel (e.g. `export`,
/// param count). Returns `None` only if the text has no bytes at the span boundaries
/// (degenerate empty file with a non-zero span — should not occur in practice).
// Each arg is a distinct DocumentSymbol field (name, kind, detail, spans, text,
// table, encoding); they're the LSP shape's own fields, not bundleable context.
#[allow(clippy::too_many_arguments)]
fn make_document_symbol(
    name: String,
    kind: SymbolKind,
    detail: Option<String>,
    name_start: usize,
    name_end: usize,
    outer_start: usize,
    outer_end: usize,
    text: &str,
    table: &LineTable,
    encoding: crate::capabilities::PositionEncoding,
) -> DocumentSymbol {
    let clamp = |b: usize| b.min(text.len());

    let selection_start = table.byte_offset_to_position(text, clamp(name_start), encoding);
    let selection_end = table.byte_offset_to_position(text, clamp(name_end), encoding);
    let outer_start_pos = table.byte_offset_to_position(text, clamp(outer_start), encoding);
    let outer_end_pos = table.byte_offset_to_position(text, clamp(outer_end), encoding);

    #[allow(deprecated)]
    DocumentSymbol {
        name,
        detail,
        kind,
        tags: None,
        deprecated: None,
        range: Range {
            start: outer_start_pos,
            end: outer_end_pos,
        },
        selection_range: Range {
            start: selection_start,
            end: selection_end,
        },
        children: None,
    }
}

/// Map a top-level AST `Item` to a `DocumentSymbol`.
///
/// Returns `None` for `ReExport` items — those have no single-name symbol to show
/// in the outline (they represent re-exported batches, not named local declarations).
fn item_to_document_symbol(
    item: &Item,
    text: &str,
    table: &LineTable,
    encoding: crate::capabilities::PositionEncoding,
) -> Option<DocumentSymbol> {
    match item {
        Item::Function(f) => {
            let mut parts = Vec::new();
            if f.is_exported {
                parts.push("export".to_string());
            }
            // Show param count only when non-zero — zero-param functions have no useful detail.
            let param_count = f.params.len();
            if param_count > 0 {
                parts.push(format!("{param_count} param(s)"));
            }
            let detail = if parts.is_empty() {
                None
            } else {
                Some(parts.join(", "))
            };
            Some(make_document_symbol(
                f.name.clone(),
                SymbolKind::FUNCTION,
                detail,
                f.name_span.start,
                f.name_span.end,
                f.span.start,
                f.span.end,
                text,
                table,
                encoding,
            ))
        }

        Item::ShapeDecl(s) => {
            let detail = if s.is_exported {
                Some("export".to_string())
            } else {
                None
            };
            Some(make_document_symbol(
                s.name.clone(),
                SymbolKind::CLASS,
                detail,
                s.name_span.start,
                s.name_span.end,
                s.span.start,
                s.span.end,
                text,
                table,
                encoding,
            ))
        }

        Item::OptionsDecl(o) => {
            let detail = if o.is_exported {
                Some("export".to_string())
            } else {
                None
            };
            Some(make_document_symbol(
                o.name.clone(),
                SymbolKind::ENUM,
                detail,
                o.name_span.start,
                o.name_span.end,
                o.span.start,
                o.span.end,
                text,
                table,
                encoding,
            ))
        }

        Item::ConstDecl(c) => {
            let detail = if c.is_exported {
                Some("export".to_string())
            } else {
                None
            };
            Some(make_document_symbol(
                c.name.clone(),
                SymbolKind::CONSTANT,
                detail,
                c.name_span.start,
                c.name_span.end,
                c.span.start,
                c.span.end,
                text,
                table,
                encoding,
            ))
        }

        Item::ImportDecl(i) => {
            // Use the import source path as the symbol name so the Outline shows what is
            // being imported (e.g. `services/users`) rather than an anonymous placeholder.
            Some(make_document_symbol(
                i.source.clone(),
                SymbolKind::MODULE,
                None,
                i.source_span.start,
                i.source_span.end,
                i.span.start,
                i.span.end,
                text,
                table,
                encoding,
            ))
        }

        // Re-exports have no canonical local name — skip them in the outline.
        Item::ReExport(_) => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Build the `textDocument/documentSymbol` response for one file.
///
/// Returns a flat list of top-level `DocumentSymbol` entries. Clients that
/// support hierarchical symbols receive the full `DocumentSymbol` tree; clients
/// that only support the flat `SymbolInformation` list receive a degraded view
/// (handled by the client, not the server).
///
/// Time: O(items) — one pass over the module's top-level item list.
pub fn document_symbol_response(state: &ServerState, uri: &lsp_types::Url) -> Vec<DocumentSymbol> {
    let Some(sf) = state.source_file_for(uri) else {
        return Vec::new();
    };
    let parse = ynz_parser::queries::parse_query(&state.db, sf);

    // Build a line table for span conversion. The open-document text is preferred;
    // fall back to reading straight from salsa so indexed-but-not-open files work.
    let owned_text: String;
    let text: &str = if let Some(t) = state.text_for(uri) {
        t
    } else {
        owned_text = sf.text(&state.db);
        &owned_text
    };
    let table = LineTable::new(text);

    parse
        .module
        .items
        .iter()
        .filter_map(|item| item_to_document_symbol(item, text, &table, state.encoding))
        .collect()
}

/// Build the `workspace/symbol` response for a search query.
///
/// Iterates every registered source file, parses it (salsa-cached after the
/// first parse), and collects top-level items whose names contain `query` as a
/// case-insensitive substring. An empty `query` returns all symbols.
///
/// Time: O(files × items) — one parse query per file (O(1) after warm-up).
pub fn workspace_symbol_response(state: &ServerState, query: &str) -> Vec<SymbolInformation> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for path in state.db.all_source_paths() {
        let Some(sf) = state.db.source_by_path(&path) else {
            continue;
        };
        let parse = ynz_parser::queries::parse_query(&state.db, sf);
        let uri = uri_for_source_file(&state.db, sf);
        let file_text = sf.text(&state.db);
        let table = LineTable::new(&file_text);

        for item in &parse.module.items {
            let (name, kind, name_start, name_end) = match item {
                Item::Function(f) => (
                    f.name.as_str(),
                    SymbolKind::FUNCTION,
                    f.name_span.start,
                    f.name_span.end,
                ),
                Item::ShapeDecl(s) => (
                    s.name.as_str(),
                    SymbolKind::CLASS,
                    s.name_span.start,
                    s.name_span.end,
                ),
                Item::OptionsDecl(o) => (
                    o.name.as_str(),
                    SymbolKind::ENUM,
                    o.name_span.start,
                    o.name_span.end,
                ),
                Item::ConstDecl(c) => (
                    c.name.as_str(),
                    SymbolKind::CONSTANT,
                    c.name_span.start,
                    c.name_span.end,
                ),
                Item::ImportDecl(i) => (
                    i.source.as_str(),
                    SymbolKind::MODULE,
                    i.source_span.start,
                    i.source_span.end,
                ),
                // Re-exports have no local name — skip.
                Item::ReExport(_) => continue,
            };

            // Empty query matches everything; non-empty query is case-insensitive substring.
            if !query_lower.is_empty() && !name.to_lowercase().contains(&query_lower) {
                continue;
            }

            let clamp = |b: usize| b.min(file_text.len());
            let start =
                table.byte_offset_to_position(&file_text, clamp(name_start), state.encoding);
            let end = table.byte_offset_to_position(&file_text, clamp(name_end), state.encoding);

            #[allow(deprecated)]
            results.push(SymbolInformation {
                name: name.to_string(),
                kind,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range: Range { start, end },
                },
                container_name: None,
            });
        }
    }

    results
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{capabilities::PositionEncoding, state::ServerState};

    fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
        let mut state = ServerState::new(PositionEncoding::Utf8);
        let uri = lsp_types::Url::from_file_path(path).expect("valid path");
        state.open_document(uri.clone(), src.to_string());
        (state, uri)
    }

    // WHY: document_symbol_response must return an empty vec for an unknown URI
    // without panicking — prevents crashes when the client sends a request for
    // a file that has not been opened yet.
    #[test]
    fn unknown_uri_returns_empty() {
        let state = ServerState::new(PositionEncoding::Utf8);
        let uri = lsp_types::Url::parse("file:///does_not_exist.ynz").unwrap();
        let symbols = document_symbol_response(&state, &uri);
        assert!(symbols.is_empty(), "unknown URI must produce no symbols");
    }

    // WHY: a single function declaration must map to exactly one FUNCTION symbol
    // with the correct name. Verifies the basic happy-path of the outline pipeline.
    #[test]
    fn function_decl_maps_to_function_symbol() {
        let src = "function greet() -> nothing { }";
        let (state, uri) = state_single("/tmp/ds_fn.ynz", src);
        let symbols = document_symbol_response(&state, &uri);
        assert_eq!(symbols.len(), 1, "one function = one symbol");
        assert_eq!(symbols[0].name, "greet");
        assert_eq!(symbols[0].kind, SymbolKind::FUNCTION);
    }

    // WHY: a shape declaration must map to a CLASS symbol. Verifies that the
    // SymbolKind mapping table is correct for shapes.
    #[test]
    fn shape_decl_maps_to_class_symbol() {
        let src = "shape Player { name: string\n health: int }";
        let (state, uri) = state_single("/tmp/ds_shape.ynz", src);
        let symbols = document_symbol_response(&state, &uri);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Player");
        assert_eq!(symbols[0].kind, SymbolKind::CLASS);
    }

    // WHY: an options declaration must map to an ENUM symbol. Verifies the
    // SymbolKind mapping for the Yinz `options` type.
    #[test]
    fn options_decl_maps_to_enum_symbol() {
        let src = "options Status { active\n inactive }";
        let (state, uri) = state_single("/tmp/ds_opts.ynz", src);
        let symbols = document_symbol_response(&state, &uri);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Status");
        assert_eq!(symbols[0].kind, SymbolKind::ENUM);
    }

    // WHY: an empty query to workspace_symbol_response must return all symbols
    // across all registered files. Verifies that the empty-query fast-path works.
    #[test]
    fn workspace_symbol_empty_query_returns_all() {
        let src = "function entrypoint() -> nothing { }";
        let (state, _uri) = state_single("/tmp/ws_all.ynz", src);
        let results = workspace_symbol_response(&state, "");
        assert!(
            !results.is_empty(),
            "empty query must match at least one symbol"
        );
    }

    // WHY: workspace_symbol_response must filter by case-insensitive substring.
    // A query of "ENTRY" must match a function named "entrypoint".
    #[test]
    fn workspace_symbol_case_insensitive_filter() {
        let src = "function entrypoint() -> nothing { }";
        let (state, _uri) = state_single("/tmp/ws_filter.ynz", src);
        let matched = workspace_symbol_response(&state, "ENTRY");
        assert!(
            matched.iter().any(|s| s.name == "entrypoint"),
            "case-insensitive filter must match 'entrypoint' for query 'ENTRY'"
        );
        let no_match = workspace_symbol_response(&state, "zzznomatch");
        assert!(
            no_match.is_empty(),
            "non-matching query must produce empty results"
        );
    }
}
