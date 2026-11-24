//! `textDocument/foldingRange` LSP handler — collapsible regions in the editor.
//!
//! Emits one `FoldingRange` per top-level declaration that has a body or a span
//! large enough to be worth folding:
//!
//! | Item | Fold covers |
//! |---|---|
//! | `Item::Function(f)` | `f.body.span` (the `{...}` block) |
//! | `Item::ShapeDecl(s)` | `s.span` (the whole declaration) |
//! | `Item::OptionsDecl(o)` | `o.span` (the whole declaration) |
//!
//! Import, const, and re-export items are not folded — they are always
//! single-line constructs in practice and folding them adds no value.

use lsp_types::{FoldingRange, FoldingRangeKind};
use ynz_ast::nodes::Item;

use crate::{position::LineTable, state::ServerState};

/// Build the `textDocument/foldingRange` response for `uri`.
///
/// Returns a `FoldingRange` per foldable top-level item.  All ranges use
/// `FoldingRangeKind::Region`.  `start_line` and `end_line` are zero-indexed
/// line numbers derived from byte offsets via `LineTable`.
///
/// Time: O(top-level items) — one pass.
pub fn folding_range_response(state: &ServerState, uri: &lsp_types::Url) -> Vec<FoldingRange> {
    let text = match state.text_for(uri) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let sf = match state.source_file_for(uri) {
        Some(sf) => sf,
        None => return Vec::new(),
    };

    let parse_output = ynz_parser::queries::parse_query(&state.db, sf);
    let module = &parse_output.module;
    let table = LineTable::new(text);

    let mut ranges = Vec::new();

    for item in &module.items {
        match item {
            Item::Function(f) => {
                // Fold the body block (the `{...}`) so the signature stays visible
                // when the function is collapsed.
                let fold = span_to_fold(&f.body.span, text, &table);
                if let Some(fr) = fold {
                    ranges.push(fr);
                }
            }
            Item::ShapeDecl(s) => {
                let fold = span_to_fold(&s.span, text, &table);
                if let Some(fr) = fold {
                    ranges.push(fr);
                }
            }
            Item::OptionsDecl(o) => {
                let fold = span_to_fold(&o.span, text, &table);
                if let Some(fr) = fold {
                    ranges.push(fr);
                }
            }
            // ImportDecl, ConstDecl, ReExport — single-line; not worth folding.
            _ => {}
        }
    }

    ranges
}

/// Convert a `SourceSpan` to a `FoldingRange`.
///
/// Returns `None` when the span is entirely on one line (nothing to fold).
fn span_to_fold(
    span: &ynz_diagnostics::SourceSpan,
    text: &str,
    table: &LineTable,
) -> Option<FoldingRange> {
    let start_pos = table.byte_offset_to_position(
        text,
        span.start,
        crate::capabilities::PositionEncoding::Utf8,
    );
    let end_pos = table.byte_offset_to_position(
        text,
        span.end.min(text.len()),
        crate::capabilities::PositionEncoding::Utf8,
    );

    let start_line = start_pos.line;
    let end_line = end_pos.line;

    // Single-line spans are not foldable.
    if start_line >= end_line {
        return None;
    }

    Some(FoldingRange {
        start_line,
        end_line,
        start_character: None,
        end_character: None,
        kind: Some(FoldingRangeKind::Region),
        collapsed_text: None,
    })
}
