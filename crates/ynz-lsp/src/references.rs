//! `textDocument/references` handler — find all use-sites of a symbol.
//!
//! Delegates to `ynz_typeck::references_for_offset` which walks all registered
//! source files and collects spans where the symbol resolves to the same
//! canonical declaration.  Uses per-site semantic resolution so shadowing is
//! handled correctly — inner-scope bindings with the same name are excluded.

use lsp_server::Message;
use lsp_types::Location;

use crate::{
    goto_definition::span_to_range,
    progress::ProgressTracker,
    state::{uri_for_source_file, ServerState},
};

/// Return all reference locations for the symbol at the given position.
///
/// `include_declaration` mirrors `ReferenceParams.context.include_declaration`:
/// when `true`, the declaration site is included in the result list alongside
/// the use-sites; when `false`, only use-sites are returned.
///
/// Emits `$/progress` via `sender` when the scan is expected to take >1s:
/// - `state.open_documents.len() > 10`, OR
/// - `cross_file_reference_count_estimate` returns >5 candidate files.
///
/// Returns `Some(Vec<Location>)` on success (including the empty-vec case —
/// "no references found" is NOT `None`; `None` means "offset is not a symbol").
///
/// Time: O(files × AST) on cache miss; O(1) salsa hit on repeated requests.
pub fn references_response(
    state: &ServerState,
    uri: &lsp_types::Url,
    position: lsp_types::Position,
    include_declaration: bool,
    sender: &crossbeam_channel::Sender<Message>,
) -> Option<Vec<Location>> {
    let text = state.text_for(uri)?;
    let table = state.line_table_for(uri)?;
    let byte_offset = table.position_to_byte_offset(text, position, state.encoding)?;
    let sf = state.source_file_for(uri)?;

    // Emit progress if the scan is likely to be slow.
    let open_count = state.open_documents.len();
    let estimate = ynz_typeck::cross_file_reference_count_estimate(&state.db, sf, byte_offset);
    let progress = if open_count > 10 || estimate > 5 {
        Some(ProgressTracker::begin("Finding references", sender.clone()))
    } else {
        None
    };

    let raw = ynz_typeck::references_for_offset(&state.db, sf, byte_offset, include_declaration);

    // Always end the progress indicator (paired with begin).
    if let Some(p) = progress {
        p.end(None);
    }

    // Map (SourceFile, SourceSpan) → LSP Location.
    let locations: Vec<Location> = raw
        .into_iter()
        .filter_map(|(origin_sf, span)| {
            let origin_uri = uri_for_source_file(&state.db, origin_sf);
            let origin_text = origin_sf.text(&state.db);
            let range = span_to_range(&origin_text, &span, state.encoding);
            range.map(|r| Location {
                uri: origin_uri,
                range: r,
            })
        })
        .collect();

    Some(locations)
}
