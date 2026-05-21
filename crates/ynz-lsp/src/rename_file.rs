//! `workspace/didRenameFiles` handler — file-move import fixup + salsa db update.
//!
//! Strategy: write import patches directly to disk (not via WorkspaceEdit).
//! This avoids creating unsaved dirty buffers in the editor — VSCode's file
//! watcher detects the disk change and refreshes the buffer automatically,
//! so the user never has to manually save touched files.
//!
//! `willRenameFiles` is NOT used. It returns a WorkspaceEdit that marks files
//! dirty and requires a manual save per file — exactly the friction we're
//! eliminating.
//!
//! Flow for a move `services/users.ynz` → `models/users.ynz`:
//! 1. Compute old_import = `services/users`, new_import = `models/users`
//! 2. Scan all registered files; for each that imports old_import, patch the
//!    source text in memory and write to disk.
//! 3. Update salsa db for each patched file (so queries see fresh content).
//! 4. Swap salsa db entry for the moved file itself (old path → new path).
//! 5. Update open_documents if the moved file was open.

use lsp_types::{RenameFilesParams, Url};
use salsa::Setter as _;
use ynz_ast::nodes::Item;
use ynz_parser::SourceFileRegistry as _;

use crate::state::{import_path_for_file, uri_to_path, ServerState};

/// Handle `workspace/didRenameFiles`.
///
/// Patches import paths in all affected files by writing directly to disk,
/// then updates the salsa db so subsequent queries reflect the new state.
pub fn did_rename_files(state: &mut ServerState, params: &RenameFilesParams) {
    let Some(project_root) = state.project_root.clone() else { return };

    for rename in &params.files {
        let Ok(old_uri) = Url::parse(&rename.old_uri) else { continue };
        let Ok(new_uri) = Url::parse(&rename.new_uri) else { continue };
        let old_path = uri_to_path(&old_uri);
        let new_path = uri_to_path(&new_uri);

        let Some(old_import) = import_path_for_file(&old_path, &project_root) else { continue };
        let Some(new_import) = import_path_for_file(&new_path, &project_root) else { continue };

        if old_import == new_import {
            continue;
        }

        // ── Step 1: patch all files that import from old_import ──────────────
        let paths = state.db.all_source_paths();
        for path in &paths {
            // Don't touch the moved file itself here — handled in step 2.
            if path.as_str() == old_path || path.as_str() == new_path {
                continue;
            }
            let Some(sf) = state.db.source_by_path(path) else { continue };
            let parse = ynz_parser::queries::parse_query(&state.db, sf);

            // Check cheaply whether this file even has the import before cloning text.
            let has_import = parse.module.items.iter().any(|item| {
                matches!(item, Item::ImportDecl(d) if d.source == old_import)
            });
            if !has_import {
                continue;
            }

            let owned;
            let original: &str = if let Some(t) = state.open_documents.get(&uri_for_path(path)) {
                t.as_str()
            } else {
                owned = sf.text(&state.db).to_string();
                &owned
            };

            let patched = patch_import_path(original, &parse.module, &old_import, &new_import);
            if patched == original {
                continue;
            }

            // Write to disk — VSCode file watcher refreshes the buffer, no dirty state.
            if std::fs::write(path, &patched).is_ok() {
                // Update salsa so queries see the fresh content immediately.
                sf.set_text(&mut state.db).to(patched.clone());
                // Update open document buffer if the file is open.
                if let Some(buf) = state.open_documents.get_mut(&uri_for_path(path)) {
                    *buf = patched;
                }
            }
        }

        // ── Step 2: swap salsa db entry for the moved file itself ─────────────
        let moved_text = if let Some(t) = state.open_documents.remove(&old_uri) {
            state.line_tables.remove(&old_uri);
            t
        } else {
            match std::fs::read_to_string(&new_path) {
                Ok(t) => t,
                Err(_) => continue,
            }
        };

        state.db.deregister_source(&old_path);
        let new_sf = ynz_parser::SourceFile::new(&state.db, new_path.clone(), moved_text.clone());
        state.db.register_source(new_sf);

        // Re-register open document under new URI if it was open.
        let table = crate::position::LineTable::new(&moved_text);
        state.open_documents.insert(new_uri.clone(), moved_text);
        state.line_tables.insert(new_uri, table);
    }
}

/// Apply all `old_import` → `new_import` substitutions to `text` and return
/// the patched string. Operates on raw bytes (all Yinz source is UTF-8).
fn patch_import_path(
    text: &str,
    module: &ynz_ast::nodes::Module,
    old_import: &str,
    new_import: &str,
) -> String {
    // Collect byte ranges to replace, in reverse order so offsets stay valid.
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for item in &module.items {
        let Item::ImportDecl(decl) = item else { continue };
        if decl.source != old_import {
            continue;
        }
        let span_start = decl.source_span.start;
        let span_end = decl.source_span.end.min(text.len());
        let span_text = text.get(span_start..span_end).unwrap_or("");
        if let Some(rel) = span_text.find(old_import) {
            ranges.push((span_start + rel, span_start + rel + old_import.len()));
        }
    }

    if ranges.is_empty() {
        return text.to_string();
    }

    ranges.sort_by(|a, b| b.0.cmp(&a.0)); // reverse order
    let mut out = text.to_string();
    for (start, end) in ranges {
        out.replace_range(start..end, new_import);
    }
    out
}

fn uri_for_path(path: &str) -> Url {
    Url::from_file_path(path).unwrap_or_else(|_| {
        Url::parse(&format!("file://{path}")).unwrap_or_else(|_| {
            Url::parse("file:///unknown").expect("static fallback")
        })
    })
}
