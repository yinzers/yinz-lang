//! `textDocument/rename` and `textDocument/prepareRename` handlers.
//!
//! `prepareRename` validates that the cursor is on a renameable identifier and
//! returns the range of that identifier (shown as a pre-selected edit box in
//! the client).  `rename` performs the actual rename by collecting all
//! reference locations and building an atomic `WorkspaceEdit`.
//!
//! The `WorkspaceEdit` is built atomically: if any step fails (e.g. an invalid
//! new name), the caller returns `Response::new_err` — a partial edit is never
//! sent to the client.  See `RenameEditBuilder` docs for the full guarantee.

use std::collections::HashSet;

use lsp_server::Message;
use lsp_types::{Range, Url, WorkspaceEdit};
use ynz_typeck::RenameError;

use crate::{
    goto_definition::span_to_range,
    progress::ProgressTracker,
    rename_edit_builder::RenameEditBuilder,
    state::{uri_for_source_file, ServerState},
};

// ─────────────────────────────────────────────────────────────────────────────
// LSP rename error codes — server-defined, in the -32000..-32099 range reserved
// by the LSP spec.  These are stable; changing a code breaks clients that
// pattern-match on the error code to display custom messages.
// ─────────────────────────────────────────────────────────────────────────────

/// Stable LSP error codes for rename failures.  Index into this table by variant
/// name to get `(code, human-readable label)`.
/// Stable LSP error codes for rename failures (server-defined, -32000..-32099).
///
/// | Code   | Variant                             |
/// |--------|-------------------------------------|
/// | -32001 | `NotARenameable`                    |
/// | -32002 | `NewNameIsReservedKeyword`          |
/// | -32003 | `NewNameIsBannedJargon`             |
/// | -32004 | `NewNameInvalidIdentifier`          |
/// | -32005 | `ConflictsWithExistingName`         |
/// | -32006 | `CannotRenameImportedSymbolInThisFile` |
///
/// These codes are stable — changing them breaks clients that pattern-match on the
/// code to display custom error UI.  Do not renumber without a major-version bump.
pub const LSP_RENAME_ERROR_CODES: &[(&str, i32)] = &[
    ("NotARenameable", -32001),
    ("NewNameIsReservedKeyword", -32002),
    ("NewNameIsBannedJargon", -32003),
    ("NewNameInvalidIdentifier", -32004),
    ("ConflictsWithExistingName", -32005),
    ("CannotRenameImportedSymbolInThisFile", -32006),
];

fn rename_error_to_lsp(err: &RenameError) -> (i32, String) {
    match err {
        RenameError::NotARenameable => (
            -32001,
            "Not a renameable symbol — cursor must be on an identifier (variable, \
             function name, shape name, etc.).\n\n\
             WHAT INSTEAD: place the cursor on an identifier and press F2.\n\n\
             WHY: keywords, literals, and punctuation have no declaration site \
             and cannot be renamed."
                .to_string(),
        ),
        RenameError::NewNameIsReservedKeyword(name) => (
            -32002,
            format!(
                "Cannot rename to `{name}` — that is a Yinz reserved keyword.\n\n\
                 WHAT INSTEAD: choose a name that is not a Yinz keyword \
                 (e.g. `{name}Value`, `my{name}`).\n\n\
                 WHY: keywords have special meaning in the grammar; using one as \
                 an identifier would make the file unparseable."
            ),
        ),
        RenameError::NewNameIsBannedJargon(name, replacement) => (
            -32003,
            format!(
                "Cannot rename to `{name}` — that term is reserved in Yinz diagnostics.\n\n\
                 WHAT INSTEAD: use `{replacement}` or another name that does not conflict \
                 with Yinz's teaching vocabulary.\n\n\
                 WHY: banned words trigger special lexer behavior or are reserved for \
                 compiler messages."
            ),
        ),
        RenameError::NewNameInvalidIdentifier(name) => (
            -32004,
            format!(
                "Cannot rename to `{name}` — that is not a valid Yinz identifier.\n\n\
                 WHAT INSTEAD: identifiers must start with a letter and contain only \
                 letters, digits, and underscores.\n\n\
                 WHY: the parser would reject the file if a non-identifier were used \
                 as a name."
            ),
        ),
        RenameError::ConflictsWithExistingName(file, _span) => (
            -32005,
            format!(
                "Cannot rename — a symbol with that name already exists in `{file}`.\n\n\
                 WHAT INSTEAD: choose a name not already used in any file affected by this rename.\n\n\
                 WHY: two top-level symbols with the same name in the same scope produce a \
                 compile error immediately after the rename.",
            ),
        ),
        RenameError::CannotRenameImportedSymbolInThisFile(origin) => (
            -32006,
            format!(
                "Cannot rename here — this symbol is declared in `{origin}`.\n\n\
                 WHAT INSTEAD: open `{origin}` and trigger rename from the \
                 declaration site.\n\n\
                 WHY: renaming from an import site would only rename the local \
                 binding, not the canonical declaration that all importers share."
            ),
        ),
    }
}

/// Compute the `textDocument/prepareRename` response.
///
/// Returns `Some(Range)` covering the identifier token at the cursor position,
/// or `None` when the cursor is on a non-renameable position (keyword, literal,
/// whitespace, end-of-file).
///
/// The client uses this range as the pre-populated selection in the rename
/// input box.  `None` tells the client "rename not available here."
///
/// Time: O(AST) on cache miss; O(1) on salsa cache hit.  Space: O(1).
pub fn prepare_rename_response(
    state: &ServerState,
    uri: &Url,
    position: lsp_types::Position,
) -> Option<Range> {
    let text = state.text_for(uri)?;
    let table = state.line_table_for(uri)?;
    let byte_offset = table.position_to_byte_offset(text, position, state.encoding)?;
    let sf = state.source_file_for(uri)?;

    // First gate: is there a renameable symbol at this offset at all?
    ynz_typeck::resolve_symbol_at(&state.db, sf, byte_offset)?;

    // Get the USE-SITE span (not the decl span) — this is what the client
    // highlights in the inline rename box.
    let parse = ynz_parser::parse_query(&state.db, sf);
    let (_, use_site_span) =
        ynz_typeck::ast_offset::identifier_use_site_at_offset(&parse.module, byte_offset)?;

    span_to_range(text, &use_site_span, state.encoding)
}

/// Compute the `textDocument/rename` response.
///
/// On success, returns a `WorkspaceEdit` that updates every reference to the
/// symbol across all registered source files.  On failure, returns an LSP
/// `(error_code, message)` pair that the caller wraps in `Response::new_err`.
///
/// The `WorkspaceEdit` is built atomically — either ALL locations are included
/// or the function returns an error.  A partial edit is never returned.
///
/// Emits `$/progress` notifications via `sender` when the rename touches more
/// than 3 files (heuristic for "noticeably long" operations at project scale).
///
/// Time: O(files × AST walk) on cache miss; proportional to reference count.
pub fn rename_response(
    state: &ServerState,
    uri: &Url,
    position: lsp_types::Position,
    new_name: &str,
    sender: &crossbeam_channel::Sender<Message>,
) -> Result<WorkspaceEdit, (i32, String)> {
    let text = state.text_for(uri).ok_or_else(|| {
        (
            lsp_server::ErrorCode::InvalidParams as i32,
            format!("unknown document: {uri}"),
        )
    })?;
    let table = state.line_table_for(uri).ok_or_else(|| {
        (
            lsp_server::ErrorCode::InvalidParams as i32,
            "no line table for document".to_string(),
        )
    })?;
    let byte_offset = table
        .position_to_byte_offset(text, position, state.encoding)
        .ok_or_else(|| {
            (
                lsp_server::ErrorCode::InvalidParams as i32,
                "position out of range".to_string(),
            )
        })?;
    let sf = state.source_file_for(uri).ok_or_else(|| {
        (
            lsp_server::ErrorCode::InvalidParams as i32,
            format!("unknown document: {uri}"),
        )
    })?;

    // Collect all rename locations.  Any validation failure is returned as an
    // LSP ResponseError here — no partial WorkspaceEdit is ever built.
    let locations = ynz_typeck::rename_locations(&state.db, sf, byte_offset, new_name.to_string())
        .map_err(|e| rename_error_to_lsp(&e))?;

    if locations.is_empty() {
        // No references found — return an empty (but valid) WorkspaceEdit.
        return Ok(RenameEditBuilder::new().build());
    }

    // Emit progress if the rename spans more than 3 distinct files.
    let unique_files: HashSet<String> = locations
        .iter()
        .map(|(loc_sf, _)| loc_sf.path(&state.db).to_string())
        .collect();
    let progress = if unique_files.len() > 3 {
        Some(ProgressTracker::begin("Renaming…", sender.clone()))
    } else {
        None
    };

    // Build the WorkspaceEdit atomically.  All span conversions happen inside
    // this loop; the builder is consumed at the end.
    let mut builder = RenameEditBuilder::new();
    for (loc_sf, span) in &locations {
        let loc_text = loc_sf.text(&state.db);
        // skip locations whose span conversion fails (e.g. stale byte offsets
        // from a concurrent edit — the client's ContentModified check handles this)
        if let Some(range) = span_to_range(&loc_text, span, state.encoding) {
            let file_uri = uri_for_source_file(&state.db, *loc_sf);
            builder.add(file_uri, range, new_name.to_string());
        }
    }

    if let Some(p) = progress {
        p.end(Some("Rename complete."));
    }

    Ok(builder.build())
}
