//! `textDocument/inlayHint` LSP handler — teaching annotations for the editor.
//!
//! Fires 9 of the registry-defined muted-hint domains; the rest return
//! empty lists (protocol-only or awaiting their underlying analysis).
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
//! | `wait_points` | Addition | muted `wait` before suspending call sites |
//! | `background_routing` | Informational | I/O-pool vs CPU-pool routing comment |
//! | `parallel_groups` | Informational | concurrent-statement overlap comment |
//! | `channel_capacity` | Addition | muted default capacity inside `channel<T>()` empty parens |
//!
//! Every firing hint carries a WHAT / WHAT-INSTEAD / WHY hover tooltip sourced
//! from the registry via `ynz_registry::lsp_inlay_hint_hover_for`.  Per Golden
//! Rule 11 (compiler is a teacher), muted annotations without a hover tooltip
//! are teaching surfaces the user cannot learn from.
//!
//! # Registered-but-not-firing domains (empty list, no error)
//!
//! `function_param_type`, `lifetimes` — handled here but returning `[]` until the
//! underlying analysis lands.  `allocators` — registered but fires when arena
//! allocation lands (v0.2+).  `auto_arc` — registered with its full cautionary
//! WHAT/WHY hover text, but firing requires the auto-Arc codegen emission
//! (`[[deferred_language_feature]]` `auto-arc-codegen-emission`, v0.4+): with no
//! emission there is no compiler decision to annotate.  When future milestones add
//! the analysis, those branches emit real hints with no further LSP change.
//!
//! # Viewport filtering
//!
//! A hint is included if its `position` byte offset falls within the requested
//! `range` — even if the anchor expression starts before the range.  This is
//! position-only filtering, matching rust-analyzer's behaviour.

use lsp_types::{
    InlayHint, InlayHintKind, InlayHintLabel, MarkupContent, MarkupKind, Position, Range, TextEdit,
};
use ynz_diagnostics::{Severity, SourceSpan};
use ynz_typeck::queries::check_query;
use ynz_typeck::{
    array_to_fixed_promotion_hints, background_routing_hints, channel_capacity_hints,
    copy_point_hints, let_to_const_promotion_hints, ownership_call_site_hints,
    parallel_group_hints, variable_type_hints, wait_points_hints, DEFAULT_CHANNEL_CAPACITY,
};

use crate::{capabilities::PositionEncoding, position::LineTable, state::ServerState};

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

fn byte_to_position(
    text: &str,
    byte: usize,
    table: &LineTable,
    encoding: PositionEncoding,
) -> Option<Position> {
    Some(table.byte_offset_to_position(text, byte.min(text.len()), encoding))
}

fn in_viewport(pos: usize, start: usize, end: usize) -> bool {
    pos >= start && pos <= end
}

fn tooltip(domain: &str) -> Option<lsp_types::InlayHintTooltip> {
    ynz_registry::lsp_inlay_hint_hover_for(domain).map(|md| {
        lsp_types::InlayHintTooltip::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: md,
        })
    })
}

fn make_hint(pos: Position, label: String, kind: InlayHintKind, domain: &str) -> InlayHint {
    make_hint_with_edit(pos, label, kind, domain, None)
}

fn make_hint_with_edit(
    pos: Position,
    label: String,
    kind: InlayHintKind,
    domain: &str,
    text_edits: Option<Vec<TextEdit>>,
) -> InlayHint {
    InlayHint {
        position: pos,
        label: InlayHintLabel::String(label),
        kind: Some(kind),
        text_edits,
        tooltip: tooltip(domain),
        padding_left: None,
        padding_right: None,
        data: None,
    }
}

/// Build the TextEdit that inserts `: TypeName` at byte `insert_at` when clicked.
fn type_insertion_edit(
    text: &str,
    insert_at: usize,
    type_text: &str,
    table: &crate::position::LineTable,
    encoding: crate::capabilities::PositionEncoding,
) -> TextEdit {
    let pos = table.byte_offset_to_position(text, insert_at.min(text.len()), encoding);
    TextEdit {
        range: lsp_types::Range {
            start: pos,
            end: pos,
        },
        new_text: format!(": {type_text}"),
    }
}

/// Build the TextEdit that replaces `let` (3 bytes at `let_start`) with `const`.
fn let_to_const_edit(
    text: &str,
    let_start: usize,
    table: &crate::position::LineTable,
    encoding: crate::capabilities::PositionEncoding,
) -> TextEdit {
    let start = table.byte_offset_to_position(text, let_start.min(text.len()), encoding);
    let end = table.byte_offset_to_position(text, (let_start + 3).min(text.len()), encoding);
    TextEdit {
        range: lsp_types::Range { start, end },
        new_text: "const".to_string(),
    }
}

/// Build the TextEdit that replaces the `array` keyword with `fixed` in a type
/// annotation, using the byte range carried in the `PromotionHint`.
///
/// The `array` token's span comes from `Type::Generic { name_span, .. }` in the
/// AST — it covers exactly the `array` keyword, never the `<int>` type arguments.
/// Applying this edit converts `array<int>` → `fixed<int>` while leaving the
/// element type and initializer untouched, producing valid type-checking source.
///
/// Time: O(1).  Space: O(1).
fn array_to_fixed_edit(
    text: &str,
    array_span: SourceSpan,
    table: &crate::position::LineTable,
    encoding: crate::capabilities::PositionEncoding,
) -> TextEdit {
    let start = table.byte_offset_to_position(text, array_span.start.min(text.len()), encoding);
    let end = table.byte_offset_to_position(text, array_span.end.min(text.len()), encoding);
    TextEdit {
        range: lsp_types::Range { start, end },
        new_text: "fixed".to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public handler
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the `textDocument/inlayHint` response for the viewport `range`.
///
/// Returns hints for all firing domains filtered to those whose `position`
/// falls within `range`.  Protocol-only domains return no hints (not an error).
///
/// Time: O(AST × 5 passes) on cache miss; O(1) on salsa cache hit.
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

    // Convert the viewport Range to byte offsets.
    let vp_start = table
        .position_to_byte_offset(text, range.start, state.encoding)
        .unwrap_or(0);
    let vp_end = table
        .position_to_byte_offset(text, range.end, state.encoding)
        .unwrap_or(text.len());

    let mut hints: Vec<InlayHint> = Vec::new();

    // ── Domain 1: variable_type (Addition) ───────────────────────────────────

    for h in variable_type_hints(&state.db, sf) {
        if !in_viewport(h.position, vp_start, vp_end) {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            let edit = type_insertion_edit(text, h.position, &h.type_text, table, state.encoding);
            hints.push(make_hint_with_edit(
                pos,
                format!(": {}", h.type_text),
                InlayHintKind::TYPE,
                "variable_type",
                Some(vec![edit]),
            ));
        }
    }

    // ── Domain 2: ownership_call_site (Informational) ────────────────────────

    for h in ownership_call_site_hints(&state.db, sf) {
        if !in_viewport(h.position, vp_start, vp_end) {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            hints.push(make_hint(
                pos,
                format!(" {}", h.modifier),
                InlayHintKind::PARAMETER,
                "ownership_call_site",
            ));
        }
    }

    // ── Domain 3: copy_points (Informational) ────────────────────────────────

    for h in copy_point_hints(&state.db, sf) {
        if !in_viewport(h.position, vp_start, vp_end) {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            hints.push(make_hint(
                pos,
                format!(".copy ({}, trivially copyable)", h.size_text),
                InlayHintKind::PARAMETER,
                "copy_points",
            ));
        }
    }

    // ── Domain 4: array_to_fixed_promotion (Replacement) ─────────────────────

    for h in array_to_fixed_promotion_hints(&state.db, sf) {
        if !in_viewport(h.position, vp_start, vp_end) {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            // Build click-to-make-explicit edit when the `array` keyword span is
            // available.  `type_keyword_span` is None only when the type annotation
            // can't be located (graceful no-edit path — no panic, no missing hint).
            let text_edits = h
                .type_keyword_span
                .map(|span| vec![array_to_fixed_edit(text, span, table, state.encoding)]);
            hints.push(make_hint_with_edit(
                pos,
                h.label.clone(),
                InlayHintKind::TYPE,
                "array_to_fixed_promotion",
                text_edits,
            ));
        }
    }

    // ── Domain 5: let_to_const_promotion (Replacement) ───────────────────────

    for h in let_to_const_promotion_hints(&state.db, sf) {
        if !in_viewport(h.position, vp_start, vp_end) {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            let edit = let_to_const_edit(text, h.position, table, state.encoding);
            hints.push(make_hint_with_edit(
                pos,
                h.label.clone(),
                InlayHintKind::TYPE,
                "let_to_const_promotion",
                Some(vec![edit]),
            ));
        }
    }

    // ── Domain 6: ownership_call_site — background large-copy (.give hint) ─────
    // Fires when a BackgroundLargeStructCopy warning is present at the call site.
    // The Tier 3 warning already teaches via the diagnostic; this hint reinforces
    // with an inline annotation at the exact arg position.
    //
    // Detection: `d.what.starts_with("Copying ")` uniquely identifies this warning
    // class. The previous `d.what_instead.contains(".give")` secondary gate has been
    // removed — `.give` no longer appears in the diagnostic text (`.give` is not valid
    // Yinz body syntax; the WHAT-INSTEAD was reworded per dot-postfix.md).

    let check_out = check_query(&state.db, sf);
    for d in check_out.diagnostics.iter() {
        if d.severity == Severity::Warning && d.what.starts_with("Copying ") {
            let hint_pos = d.span.end;
            if !in_viewport(hint_pos, vp_start, vp_end) {
                continue;
            }
            if let Some(pos) = byte_to_position(text, hint_pos, table, state.encoding) {
                hints.push(make_hint(
                    pos,
                    " .give (transfers ownership; no copy)".to_string(),
                    InlayHintKind::PARAMETER,
                    "ownership_call_site",
                ));
            }
        }
    }

    // ── Domain 7: wait_points (Addition) — muted `wait` before suspending calls ─

    for h in wait_points_hints(&state.db, sf) {
        if !in_viewport(h.position, vp_start, vp_end) {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            hints.push(make_hint(
                pos,
                "wait  ".to_string(),
                InlayHintKind::PARAMETER,
                "wait_points",
            ));
        }
    }

    // ── Domain 8: background_routing (Informational) — I/O vs CPU pool routing ─

    for h in background_routing_hints(&state.db, sf) {
        if !in_viewport(h.position, vp_start, vp_end) {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            hints.push(make_hint(
                pos,
                format!("  {}", h.label),
                InlayHintKind::PARAMETER,
                "background_routing",
            ));
        }
    }

    // Domain 9: parallel_groups (Informational) — concurrent-statement overlap.
    //    Fires for I/O-overlap groups and CPU-parallel-group groups.
    //    Reads the same admission decision codegen spawns from — hint and binary always agree.

    for h in parallel_group_hints(&state.db, sf) {
        if !in_viewport(h.position, vp_start, vp_end) {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            hints.push(make_hint(
                pos,
                format!("  {}", h.label),
                InlayHintKind::PARAMETER,
                "parallel_groups",
            ));
        }
    }

    // ── Domain 10: channel_capacity (Addition) — muted default capacity inside the
    //    empty parens. Fires on `channel<T>()` constructions that rely on the default.
    //    Label and click-edit both thread from the ONE authoritative constant
    //    (`ynz_typeck::DEFAULT_CHANNEL_CAPACITY` — authoritative-derivation.md), and the
    //    label is the plain insertable text, matching every other Addition-category
    //    hint (`": int"`, `"wait  "`) per inference.md's one-renderer-per-category goal.
    //    Click-to-make-explicit inserts the number at the hint position, producing
    //    `channel<T>(64)`-style source — identical behavior, now visible.

    for h in channel_capacity_hints(&state.db, sf) {
        if !in_viewport(h.position, vp_start, vp_end) {
            continue;
        }
        if let Some(pos) = byte_to_position(text, h.position, table, state.encoding) {
            let capacity_text = DEFAULT_CHANNEL_CAPACITY.to_string();
            let edit = TextEdit {
                range: lsp_types::Range {
                    start: pos,
                    end: pos,
                },
                new_text: capacity_text.clone(),
            };
            hints.push(make_hint_with_edit(
                pos,
                capacity_text,
                InlayHintKind::PARAMETER,
                "channel_capacity",
                Some(vec![edit]),
            ));
        }
    }

    // ── Registered-but-not-firing domains (function_param_type, lifetimes,
    //    auto_arc) — return empty until a future milestone adds the underlying
    //    analysis (auto_arc: the auto-Arc codegen emission, v0.4+).
    //    No code needed: no data → no hints appended → Vec unchanged.

    hints
}
