//! `textDocument/formatting` and `textDocument/rangeFormatting` LSP handlers.
//!
//! Both delegates to `ynz_fmt::format` (and `format_range` for the range variant).
//! Format-on-save is idempotent: if the file is already in canonical form,
//! `formatting_response` returns an empty `Vec<TextEdit>` — the client applies
//! nothing and fires no further `didChange` event, breaking any potential loop.
//!
//! # Parse-error behaviour
//!
//! If `ynz-fmt` encounters a parse error, the handler returns:
//!   - An empty `Vec<TextEdit>` (no changes applied), AND
//!   - A `window/showMessage` Info notification so the user knows WHY the file
//!     was not formatted.  Silent-empty is a violation of Golden Rule 11 (teaching
//!     mission); the visible signal is required, not optional.
//!
//! # Line-ending policy
//!
//! Yinz files are LF-only by spec.  `ynz-fmt::format` normalises CRLF → LF on
//! input and produces LF-only output.  Format-on-save on a CRLF file will
//! normalise the entire file to LF — this is intentional, not a side effect.

use lsp_server::{Message, Notification};
use lsp_types::{
    MessageType, Range, ShowMessageParams, TextEdit,
    notification::{Notification as _, ShowMessage},
};

use crate::{
    capabilities::PositionEncoding,
    position::LineTable,
    state::ServerState,
};

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Emit a `window/showMessage` notification to the client.
fn show_message(sender: &crossbeam_channel::Sender<Message>, kind: MessageType, text: &str) {
    let notif = Notification {
        method: ShowMessage::METHOD.to_string(),
        params: serde_json::to_value(ShowMessageParams {
            typ: kind,
            message: text.to_string(),
        })
        .unwrap_or_default(),
    };
    sender.send(Message::Notification(notif)).ok();
}

/// Convert a byte range (start_byte, end_byte) in `text` to an LSP `Range`.
fn byte_range_to_lsp_range(text: &str, start_byte: usize, end_byte: usize, encoding: PositionEncoding) -> Range {
    let table = LineTable::new(text);
    let start = table.byte_offset_to_position(text, start_byte, encoding);
    let end = table.byte_offset_to_position(text, end_byte.min(text.len()), encoding);
    Range { start, end }
}

/// Build the TextEdit list for a whole-file format.
///
/// Returns `vec![TextEdit { range: 0..EOF, new_text: formatted }]` when the
/// file changes, or `vec![]` when the file is already canonical.
fn whole_file_edits(source: &str, formatted: String, encoding: PositionEncoding) -> Vec<TextEdit> {
    if formatted == source {
        return Vec::new();
    }
    let eof_range = byte_range_to_lsp_range(source, 0, source.len(), encoding);
    vec![TextEdit {
        range: eof_range,
        new_text: formatted,
    }]
}

// ─────────────────────────────────────────────────────────────────────────────
// Public LSP handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the `textDocument/formatting` response.
///
/// Returns a `Vec<TextEdit>` that rewrites the entire file with the canonical
/// Yinz formatting.  Returns an empty vec when the file is already canonical
/// or when formatting fails (with a `window/showMessage` diagnostic in the
/// failure case so the user knows why nothing happened).
///
/// Time: O(AST) dominated by the formatter; typically <50ms.  Space: O(source).
pub fn formatting_response(
    state: &ServerState,
    uri: &lsp_types::Url,
    sender: &crossbeam_channel::Sender<Message>,
) -> Vec<TextEdit> {
    let source = match state.text_for(uri) {
        Some(s) => s,
        None => return Vec::new(),
    };

    match ynz_fmt::format(source) {
        Ok(formatted) => whole_file_edits(source, formatted, state.encoding),
        Err(ynz_fmt::FmtError::ParseError(_)) => {
            // Visible signal per Golden Rule 11: user must know WHY no formatting happened.
            show_message(
                sender,
                MessageType::INFO,
                "ynz-fmt: cannot format file with parse errors — fix syntax errors first.",
            );
            Vec::new()
        }
        Err(ynz_fmt::FmtError::InvalidInput(msg)) => {
            show_message(
                sender,
                MessageType::WARNING,
                &format!("ynz-fmt: formatting failed — {msg}"),
            );
            Vec::new()
        }
    }
}

/// Compute the `textDocument/rangeFormatting` response.
///
/// Formats the sub-range of the source identified by `range`.  The formatter
/// rewrites the full file and returns the canonical form for the requested line
/// range (the Yinz grammar is file-scoped; partial-AST formatting would be
/// ambiguous at expression boundaries).
///
/// Returns an empty vec when the range is already canonical or when formatting
/// fails.  In the failure case, emits a `window/showMessage` notification.
///
/// Time: same as [`formatting_response`] (full-file format pass).
pub fn range_formatting_response(
    state: &ServerState,
    uri: &lsp_types::Url,
    range: Range,
    sender: &crossbeam_channel::Sender<Message>,
) -> Vec<TextEdit> {
    let source = match state.text_for(uri) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let table = match state.line_table_for(uri) {
        Some(t) => t,
        None => return Vec::new(),
    };

    // Convert the LSP range to byte offsets.
    let start_byte = table
        .position_to_byte_offset(source, range.start, state.encoding)
        .unwrap_or(0);
    let end_byte = table
        .position_to_byte_offset(source, range.end, state.encoding)
        .unwrap_or(source.len());

    // Format the whole file — the Yinz grammar is file-scoped, so partial
    // formatting is always a full-file pass.
    let formatted_source = match ynz_fmt::format(source) {
        Ok(f) => f,
        Err(ynz_fmt::FmtError::ParseError(_)) => {
            show_message(
                sender,
                MessageType::INFO,
                "ynz-fmt: cannot format selection with parse errors — fix syntax errors first.",
            );
            return Vec::new();
        }
        Err(ynz_fmt::FmtError::InvalidInput(msg)) => {
            show_message(
                sender,
                MessageType::WARNING,
                &format!("ynz-fmt: range formatting failed — {msg}"),
            );
            return Vec::new();
        }
    };

    // If the whole file is already canonical, nothing changed in the range either.
    if formatted_source == source {
        return Vec::new();
    }

    // The formatted content for the exact byte range.
    let formatted_range =
        ynz_fmt::format_range(source, start_byte, end_byte).unwrap_or_default();
    let original_range_text = &source[start_byte..end_byte.min(source.len())];

    if formatted_range.trim_end_matches('\n') == original_range_text.trim_end_matches('\n') {
        return Vec::new();
    }

    // Build a single TextEdit covering the requested range.
    let lsp_range = byte_range_to_lsp_range(source, start_byte, end_byte, state.encoding);
    vec![TextEdit {
        range: lsp_range,
        new_text: formatted_range,
    }]
}
