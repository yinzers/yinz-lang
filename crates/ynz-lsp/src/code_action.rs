use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, TextEdit, Url, WorkspaceEdit,
};
use std::collections::HashMap;
use ynz_diagnostics::DiagnosticKind;

use crate::{position::LineTable, state::ServerState};

/// Enumerate quick-fix code actions for diagnostics overlapping `range` in `uri`.
///
/// Each `BannedKeyword` diagnostic with a known single-token replacement produces
/// one `CodeAction { kind: QuickFix, edit }`.  Diagnostics without an unambiguous
/// replacement are silently omitted — `None` from the registry adapter means
/// "no quick-fix for this case."
///
/// Flow:
///   1. Get current check output for the file.
///   2. Convert LSP range to byte offsets.
///   3. Scan diagnostics that overlap the requested range.
///   4. For each `BannedKeyword { keyword }`, call the registry adapter.
///   5. Return the collected `CodeAction` list (empty = no fixes available).
///
/// Time: O(D) where D = number of diagnostics in the check output.
pub fn code_action_response(
    state: &ServerState,
    uri: &Url,
    range: lsp_types::Range,
) -> Vec<CodeActionOrCommand> {
    let Some(sf) = state.source_file_for(uri) else {
        return vec![];
    };
    let Some(text) = state.text_for(uri) else {
        return vec![];
    };
    let table = LineTable::new(text);

    let start_byte = table
        .position_to_byte_offset(text, range.start, state.encoding)
        .unwrap_or(0);
    let end_byte = table
        .position_to_byte_offset(text, range.end, state.encoding)
        .unwrap_or(text.len());

    let check_output = ynz_typeck::queries::check_query(&state.db, sf);
    let mut actions: Vec<CodeActionOrCommand> = vec![];

    for diag in check_output.diagnostics.iter() {
        // Only consider diagnostics that overlap the requested range.
        if diag.span.end <= start_byte || diag.span.start >= end_byte {
            continue;
        }

        let Some(kind) = &diag.kind else { continue };

        let action = match kind {
            DiagnosticKind::BannedKeyword { keyword } => {
                build_banned_keyword_action(uri, text, &table, diag, keyword, state.encoding)
            }
            _ => None,
        };

        if let Some(a) = action {
            actions.push(CodeActionOrCommand::CodeAction(a));
        }
    }

    actions
}

/// Build a quick-fix for a `BannedKeyword` diagnostic.
///
/// Returns `None` when the keyword has no unambiguous single-token replacement
/// (complex replacements like `async` are deferred to v0.3+).
fn build_banned_keyword_action(
    uri: &Url,
    text: &str,
    table: &LineTable,
    diag: &ynz_diagnostics::Diagnostic,
    keyword: &str,
    encoding: crate::capabilities::PositionEncoding,
) -> Option<CodeAction> {
    let replacement = ynz_registry::lsp_code_action_replacement_for("BannedKeyword", keyword)?;
    let label = ynz_registry::lsp_code_action_label_for("BannedKeyword", keyword)?;

    let diag_range = {
        let start = table.byte_offset_to_position(text, diag.span.start, encoding);
        let end = table.byte_offset_to_position(
            text,
            diag.span.end.min(text.len()),
            encoding,
        );
        lsp_types::Range { start, end }
    };

    let edit = TextEdit {
        range: diag_range,
        new_text: replacement.to_string(),
    };

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    changes.insert(uri.clone(), vec![edit]);

    Some(CodeAction {
        title: label,
        kind: Some(CodeActionKind::QUICKFIX),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_label_class_to_shape() {
        let label = ynz_registry::lsp_code_action_label_for("BannedKeyword", "class");
        assert_eq!(label.as_deref(), Some("Replace `class` with `shape`"));
    }

    #[test]
    fn registry_label_enum_to_options() {
        let label = ynz_registry::lsp_code_action_label_for("BannedKeyword", "enum");
        assert_eq!(label.as_deref(), Some("Replace `enum` with `options`"));
    }

    #[test]
    fn registry_label_struct_to_shape() {
        let label = ynz_registry::lsp_code_action_label_for("BannedKeyword", "struct");
        assert_eq!(label.as_deref(), Some("Replace `struct` with `shape`"));
    }

    #[test]
    fn registry_no_label_for_unknown_kind() {
        let label = ynz_registry::lsp_code_action_label_for("TypeMismatch", "class");
        assert!(label.is_none());
    }

    #[test]
    fn registry_replacement_class_is_shape() {
        let r = ynz_registry::lsp_code_action_replacement_for("BannedKeyword", "class");
        assert_eq!(r, Some("shape"));
    }

    #[test]
    fn registry_replacement_enum_is_options() {
        let r = ynz_registry::lsp_code_action_replacement_for("BannedKeyword", "enum");
        assert_eq!(r, Some("options"));
    }

    #[test]
    fn registry_replacement_complex_keyword_is_none() {
        // `async` has no single-token replacement — deferred per todos.md
        let r = ynz_registry::lsp_code_action_replacement_for("BannedKeyword", "async");
        assert!(r.is_none());
    }

    #[test]
    fn registry_replacement_await_is_wait() {
        let r = ynz_registry::lsp_code_action_replacement_for("BannedKeyword", "await");
        assert_eq!(r, Some("wait"));
    }
}
