//! `textDocument/inlayHint` LSP handler — teaching annotations for the editor.
//!
//! Fires 5 of the 9 registry-defined muted-hint domains; the other 4 return
//! empty lists (protocol-only, awaiting v0.3+ analysis data).
//!
//! # Firing domains
//!
//! | Domain | Placement | What it emits |
//! |---|---|---|
//! | `variable_type` | Addition | `: TypeName` after un-annotated `let` |
//! | `ownership_call_site` | Informational | `share`/`lend`/`give` after call args |
//! | `copy_points` | Informational | `.copy (N bytes)` for trivially-copyable args |
//! | `array_to_fixed_promotion` | Replacement | decoration on never-grown arrays |
//! | `let_to_const_promotion` | Replacement | decoration on never-mutated lets |
//!
//! # Protocol-only domains (empty list, no error)
//!
//! `function_param_type`, `wait_points`, `lifetimes`, `allocators` — each
//! handled by a registered branch in `inlay_hint_response` that returns `[]`.
//! When v0.3+ adds the underlying analysis, those branches emit real hints
//! with no further LSP code change.
//!
//! # Viewport filtering
//!
//! The LSP `InlayHintRequest` includes a `range` parameter (the visible viewport).
//! A hint is included if its `position` byte offset falls within the requested
//! range's byte span — even if the anchor expression starts before the range.
//! This matches rust-analyzer's behaviour (position-only, not anchor-span).

use lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, Position, Range};
use ynz_typeck::{
    array_to_fixed_promotion_hints, copy_point_hints, let_to_const_promotion_hints,
    ownership_call_site_hints, variable_type_hints, PromotionKind,
};

use crate::{
    capabilities::PositionEncoding,
    position::LineTable,
    state::ServerState,
};

/// Compute the `textDocument/inlayHint` response for the viewport `range`.
///
/// Returns hints for all firing domains filtered to hints whose `position`
/// falls within `range`.  Protocol-only domains return no hints.
///
/// Time: O(AST × 5 passes) on cache miss; O(1) salsa hit on re-render.
pub fn inlay_hint_response(
    state: &ServerState,
    uri: &lsp_types::Url,
    range: Range,
) -> Vec<InlayHint> {
    let text = match state.text_for(uri) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let table = match state.line_table_for(uri) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let sf = match state.source_file_for(uri) {
        Some(sf) => sf,
        None => return Vec::new(),
    };

    // Convert the viewport Range to byte offsets for filtering.
    let range_start_byte = table
        .position_to_byte_offset(text, range.start, state.encoding)
        .unwrap_or(0);
    let range_end_byte = table
        .position_to_byte_offset(text, range.end, state.encoding)
        .unwrap_or(text.len());

    let mut hints: Vec<InlayHint> = Vec::new();

    // ── Domain 1: variable_type (Addition) ───────────────────────────────────

    for h in variable_type_hints(&state.db, sf) {
        if h.position < range_start_byte || h.position > range_end_byte {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            hints.push(InlayHint {
                position: pos,
                label: InlayHintLabel::String(format!(": {}", h.type_text)),
                kind: Some(InlayHintKind::TYPE),
                text_edits: None,
                tooltip: None,
                padding_left: None,
                padding_right: None,
                data: None,
            });
        }
    }

    // ── Domain 2: ownership_call_site (Informational) ────────────────────────

    for h in ownership_call_site_hints(&state.db, sf) {
        if h.position < range_start_byte || h.position > range_end_byte {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            hints.push(InlayHint {
                position: pos,
                label: InlayHintLabel::String(format!(" {}", h.modifier)),
                kind: Some(InlayHintKind::PARAMETER),
                text_edits: None,
                tooltip: None,
                padding_left: None,
                padding_right: None,
                data: None,
            });
        }
    }

    // ── Domain 3: copy_points (Informational) ────────────────────────────────

    for h in copy_point_hints(&state.db, sf) {
        if h.position < range_start_byte || h.position > range_end_byte {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            hints.push(InlayHint {
                position: pos,
                label: InlayHintLabel::String(format!(".copy ({}, trivially copyable)", h.size_text)),
                kind: Some(InlayHintKind::PARAMETER),
                text_edits: None,
                tooltip: None,
                padding_left: None,
                padding_right: None,
                data: None,
            });
        }
    }

    // ── Domains 4+5: array_to_fixed_promotion + let_to_const (Replacement) ──

    for h in array_to_fixed_promotion_hints(&state.db, sf) {
        if h.position < range_start_byte || h.position > range_end_byte {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            hints.push(InlayHint {
                position: pos,
                label: InlayHintLabel::String(h.label.clone()),
                kind: Some(InlayHintKind::TYPE),
                text_edits: None,
                tooltip: None,
                padding_left: None,
                padding_right: None,
                data: None,
            });
        }
    }

    for h in let_to_const_promotion_hints(&state.db, sf) {
        if h.position < range_start_byte || h.position > range_end_byte {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            let label = match h.kind {
                PromotionKind::LetToConst => h.label.clone(),
                PromotionKind::ArrayToFixed => h.label.clone(),
            };
            hints.push(InlayHint {
                position: pos,
                label: InlayHintLabel::String(label),
                kind: Some(InlayHintKind::TYPE),
                text_edits: None,
                tooltip: None,
                padding_left: None,
                padding_right: None,
                data: None,
            });
        }
    }

    // ── Protocol-only domains (empty) ────────────────────────────────────────
    // function_param_type, wait_points, lifetimes, allocators:
    // Each is registered here explicitly so the LSP never returns an error for
    // these domains — they simply emit nothing until v0.3+ data exists.
    // No code needed: no data → no hints → Vec unchanged.

    hints
}

fn byte_to_position(text: &str, byte: usize, table: &LineTable, encoding: PositionEncoding) -> Option<Position> {
    Some(table.byte_offset_to_position(text, byte.min(text.len()), encoding))
}
