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

    #[test]
    fn single_function_produces_one_fold() {
        // WHY: A multi-line function body must produce exactly one FoldingRange
        // covering the body block. If zero ranges are returned the editor cannot
        // collapse the function; if more than one is returned there are phantom
        // fold regions that confuse the UI.
        //
        // The source has the opening `{` on line 0 and the body statements on line 1.
        // The parser sets body.span.start to the byte after `{\n`, which is the first
        // character of line 1. So start_line == 1 and end_line == 2 (the `}` line).
        let src = "function entrypoint() -> nothing {\n  let x = 42\n}\n";
        let (state, uri) = state_single("/tmp/ynz_fold_fn.ynz", src);
        let ranges = folding_range_response(&state, &uri);
        assert_eq!(ranges.len(), 1, "one function → one fold, got: {ranges:?}");
        let fr = &ranges[0];
        assert_eq!(fr.kind, Some(FoldingRangeKind::Region));
        // start_line < end_line: body block spans at least two lines.
        assert!(
            fr.start_line < fr.end_line,
            "body fold must span at least two lines; got start={} end={}",
            fr.start_line,
            fr.end_line,
        );
        // end_line must be 2 — the `}` is the third line (0-indexed).
        assert_eq!(fr.end_line, 2, "closing brace is on line 2 (0-indexed)");
    }

    #[test]
    fn single_line_function_produces_no_fold() {
        // WHY: A function whose entire declaration fits on one line has nothing to
        // fold. Returning a FoldingRange with start_line == end_line would produce
        // a zero-height fold region that clients handle inconsistently.
        let src = "function id(x: int) -> int { return x }\n";
        let (state, uri) = state_single("/tmp/ynz_fold_oneline.ynz", src);
        let ranges = folding_range_response(&state, &uri);
        assert!(
            ranges.is_empty(),
            "single-line function must not produce a fold, got: {ranges:?}"
        );
    }

    #[test]
    fn shape_decl_produces_fold() {
        // WHY: A multi-line shape declaration is a common fold candidate — users
        // want to collapse the field list when reading calling code. Missing this
        // fold is a noticeable UX gap.
        let src = "shape Player {\n  name: string\n  health: int\n}\n";
        let (state, uri) = state_single("/tmp/ynz_fold_shape.ynz", src);
        let ranges = folding_range_response(&state, &uri);
        assert_eq!(ranges.len(), 1, "one shape → one fold, got: {ranges:?}");
        assert_eq!(ranges[0].start_line, 0);
        assert_eq!(ranges[0].end_line, 3);
    }

    #[test]
    fn options_decl_produces_fold() {
        // WHY: options declarations follow the same multi-line fold pattern as
        // shapes. If the handler skips OptionsDecl items, options types lose
        // editor collapsibility without any diagnostic or error — a silent UX bug.
        let src = "options Status {\n  active\n  inactive\n  banned\n}\n";
        let (state, uri) = state_single("/tmp/ynz_fold_options.ynz", src);
        let ranges = folding_range_response(&state, &uri);
        assert_eq!(ranges.len(), 1, "one options → one fold, got: {ranges:?}");
        assert_eq!(ranges[0].start_line, 0);
        assert_eq!(ranges[0].end_line, 4);
    }

    #[test]
    fn empty_file_returns_no_folds() {
        // WHY: The empty-file path must not panic or return phantom folds.
        // This is the degenerate input that frequently exposes off-by-one bugs
        // in range conversion code.
        let src = "";
        let (state, uri) = state_single("/tmp/ynz_fold_empty.ynz", src);
        let ranges = folding_range_response(&state, &uri);
        assert!(ranges.is_empty(), "empty file → no folds, got: {ranges:?}");
    }
}
