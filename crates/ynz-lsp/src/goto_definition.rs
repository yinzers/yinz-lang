//! `textDocument/definition` handler — jump to the declaration site of a symbol.
//!
//! Delegates to `ynz_typeck::def_site_for_offset` which resolves the symbol at the
//! cursor position through local scope → file-level declarations → imports.
//! Cross-file jumps return a `Location` pointing to the origin file's URI.

use lsp_types::{GotoDefinitionResponse, Location, Range};
use ynz_diagnostics::SourceSpan;

use crate::{
    capabilities::PositionEncoding,
    position::LineTable,
    state::{uri_for_source_file, ServerState},
};

/// Convert a `SourceSpan` byte range to an LSP `Range` using the given encoding.
///
/// Returns `None` only when `text` is empty AND the span starts at 0 AND ends at 0
/// (degenerate zero-length span with no text to index into — extremely rare).
pub fn span_to_range(text: &str, span: &SourceSpan, encoding: PositionEncoding) -> Option<Range> {
    let table = LineTable::new(text);
    let start = table.byte_offset_to_position(text, span.start, encoding);
    let end = table.byte_offset_to_position(text, span.end.min(text.len()), encoding);
    Some(Range { start, end })
}

/// Compute the go-to-definition response for a cursor position.
///
/// Returns `Some(GotoDefinitionResponse)` when the offset resolves to a known
/// declaration; `None` when the offset is on whitespace, a keyword, or an
/// unresolved identifier (LSP clients show "No definition found").
///
/// Time: O(1) salsa cache hit after the first call at this offset; O(AST) on
/// cache miss (same cost as hover).
pub fn definition_response(
    state: &ServerState,
    uri: &lsp_types::Url,
    position: lsp_types::Position,
) -> Option<GotoDefinitionResponse> {
    let text = state.text_for(uri)?;
    let table = state.line_table_for(uri)?;
    let byte_offset = table.position_to_byte_offset(text, position, state.encoding)?;
    let sf = state.source_file_for(uri)?;

    let (origin_sf, decl_span) = ynz_typeck::def_site_for_offset(&state.db, sf, byte_offset)?;

    // Convert decl_span byte offsets to LSP Positions.  The origin file may differ
    // from the active file (cross-file jump); we need the origin's line table.
    let origin_uri = uri_for_source_file(&state.db, origin_sf);
    let origin_text = origin_sf.text(&state.db);
    let origin_table = LineTable::new(&origin_text);

    // Use the session-negotiated encoding so UTF-16 clients get correct character offsets.
    let start = origin_table.byte_offset_to_position(&origin_text, decl_span.start, state.encoding);
    let end = origin_table.byte_offset_to_position(
        &origin_text,
        decl_span.end.min(origin_text.len()),
        state.encoding,
    );

    let location = Location {
        uri: origin_uri,
        range: Range { start, end },
    };
    Some(GotoDefinitionResponse::Scalar(location))
}
