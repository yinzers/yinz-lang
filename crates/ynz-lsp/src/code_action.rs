use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Range, TextEdit, Url, WorkspaceEdit,
};
use std::collections::HashMap;
use ynz_ast::nodes::Item;
use ynz_diagnostics::DiagnosticKind;
use ynz_parser::SourceFileRegistry as _;

use crate::{position::LineTable, state::{import_path_for_file, ServerState}};

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
                build_banned_keyword_action(
                    uri, text, &table, diag, keyword, kind.kind_name(), state.encoding,
                )
            }
            DiagnosticKind::BannedJargon { term } => {
                build_banned_jargon_action(uri, text, &table, diag, term, state.encoding)
            }
            DiagnosticKind::NotDefined => {
                let name =
                    text.get(diag.span.start..diag.span.end.min(text.len())).unwrap_or("").trim();
                if name.is_empty() {
                    None
                } else {
                    build_auto_import_action(state, uri, text, &table, name)
                }
            }
            DiagnosticKind::UnusedImport { name } => {
                build_remove_import_action(state, uri, text, &table, diag.span.start, name)
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
    kind_name: &str,
    encoding: crate::capabilities::PositionEncoding,
) -> Option<CodeAction> {
    let replacement = ynz_registry::lsp_code_action_replacement_for(kind_name, keyword)?;
    let label = ynz_registry::lsp_code_action_label_for(kind_name, keyword)?;

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

/// Build a quick-fix for a `BannedJargon` diagnostic.
///
/// The action title includes the registry `reason` field so users learn WHY
/// the word is banned, not just that it needs to change.  The replacement text
/// comes from the registry `replacement` field — no duplicate hardcoding here.
///
/// Returns `None` when the term has no registry entry (unknown jargon).
fn build_banned_jargon_action(
    uri: &Url,
    text: &str,
    table: &LineTable,
    diag: &ynz_diagnostics::Diagnostic,
    term: &str,
    encoding: crate::capabilities::PositionEncoding,
) -> Option<CodeAction> {
    let replacement = ynz_registry::lsp_code_action_replacement_for("BannedJargon", term)?;
    let label = ynz_registry::lsp_code_action_label_for_jargon(term)?;

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

/// Search all registered files (excluding `current_uri`) for an exported symbol named `name`.
/// Returns the first match as `(absolute_file_path, import_statement)` or `None`.
///
/// If multiple files export the same name, the first hit in path-sorted order wins.
/// A future iteration can surface all candidates as separate actions.
fn build_auto_import_action(
    state: &ServerState,
    current_uri: &Url,
    text: &str,
    table: &LineTable,
    name: &str,
) -> Option<CodeAction> {
    let project_root = state.project_root.as_deref()?;

    let current_path = crate::state::uri_to_path(current_uri);

    // Collect all other registered files and sort for deterministic ordering.
    let mut all_paths = state.db.all_source_paths();
    all_paths.sort();

    let mut import_path: Option<String> = None;
    for path in &all_paths {
        if path.as_str() == current_path {
            continue;
        }
        let Some(sf) = state.db.source_by_path(path) else {
            continue;
        };
        let exports = ynz_typeck::exports_query(&state.db, sf);
        let found = exports.shapes.contains_key(name)
            || exports.options.contains_key(name)
            || exports.functions.contains_key(name);
        if found {
            import_path = import_path_for_file(path, project_root);
            break;
        }
    }

    let import_path = import_path?;
    let import_text = format!("import {{ {name} }} from `{import_path}`\n");

    // Find insertion byte: after the last existing import declaration, or at file start.
    let insert_byte = import_insert_byte(state, current_uri);
    let insert_pos = table.byte_offset_to_position(text, insert_byte, state.encoding);

    let edit = TextEdit {
        range: Range { start: insert_pos, end: insert_pos },
        new_text: import_text,
    };

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    changes.insert(current_uri.clone(), vec![edit]);

    Some(CodeAction {
        title: format!("Import `{name}` from `{import_path}`"),
        kind: Some(CodeActionKind::QUICKFIX),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    })
}

/// Build a quick-fix that removes an unused import line.
///
/// Strategy: find the `ImportDecl` whose span contains the diagnostic offset, then
/// delete from the start of that declaration's line to the start of the next line.
/// This removes the entire import statement (including its trailing newline) with a
/// single TextEdit, which is what editors expect for a "remove import" action.
///
/// When a named import has multiple items (`import { A, B } from "..."`) and only one
/// is unused, this still deletes the whole line. That is intentional for v0.2 — the
/// user can re-add the symbol they still need. Granular comma-aware removal ships
/// when the formatter can reconstruct the import from individual edits.
fn build_remove_import_action(
    state: &ServerState,
    uri: &Url,
    text: &str,
    table: &LineTable,
    diag_start: usize,
    name: &str,
) -> Option<CodeAction> {
    let Some(sf) = state.source_file_for(uri) else {
        return None;
    };
    let parse = ynz_parser::queries::parse_query(&state.db, sf);

    // Find the ImportDecl whose byte span contains the diagnostic start offset.
    let import_decl = parse.module.items.iter().find_map(|item| {
        if let Item::ImportDecl(d) = item {
            if d.span.start <= diag_start && diag_start <= d.span.end {
                Some(d)
            } else {
                None
            }
        } else {
            None
        }
    })?;

    // Walk back from the declaration start to the beginning of its line.
    let line_start = text[..import_decl.span.start]
        .rfind('\n')
        .map(|n| n + 1)
        .unwrap_or(0);

    // Walk forward from declaration end to include the trailing newline so the
    // blank line left behind is also removed.
    let line_end = text[import_decl.span.end.min(text.len())..]
        .find('\n')
        .map(|n| import_decl.span.end + n + 1)
        .unwrap_or_else(|| import_decl.span.end.min(text.len()));

    let start_pos = table.byte_offset_to_position(text, line_start, state.encoding);
    let end_pos = table.byte_offset_to_position(text, line_end, state.encoding);

    let edit = TextEdit {
        range: Range { start: start_pos, end: end_pos },
        new_text: String::new(),
    };

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    changes.insert(uri.clone(), vec![edit]);

    Some(CodeAction {
        title: format!("Remove unused import `{name}`"),
        kind: Some(CodeActionKind::QUICKFIX),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    })
}

/// Find the byte offset at which to insert a new import statement.
/// Exposed for reuse by the completion cross-file items builder.
///
/// Returns the start of the line immediately after the last existing import declaration.
/// If there are no imports in the file, returns 0 (insert at the very top).
pub fn import_insert_byte(state: &ServerState, uri: &Url) -> usize {
    let Some(sf) = state.source_file_for(uri) else {
        return 0;
    };
    let parse = ynz_parser::queries::parse_query(&state.db, sf);
    let last_import_end = parse.module.items.iter().filter_map(|item| {
        if let Item::ImportDecl(d) = item {
            Some(d.span.end)
        } else {
            None
        }
    }).max();

    let Some(end_byte) = last_import_end else {
        return 0;
    };

    // Advance past any trailing newline so insertion lands at the start of the next line.
    let text = state.text_for(uri).unwrap_or("");
    let after = end_byte.min(text.len());
    if text.as_bytes().get(after) == Some(&b'\n') {
        after + 1
    } else {
        after
    }
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
