//! Atomic workspace-edit builder for the `textDocument/rename` handler.
//!
//! Accumulates `TextEdit`s per URI and builds a single `WorkspaceEdit` at the end.
//! The edit is atomic from the LSP client's perspective: either all locations are
//! included (returned) or the caller returns an error before calling `build()` —
//! the client never receives a partial rename.

use std::collections::HashMap;

use lsp_types::{TextEdit, Url, WorkspaceEdit};

/// Accumulates per-file text edits and builds an atomic `WorkspaceEdit`.
///
/// Build the complete edit map before calling `build()` — if any location fails
/// conversion, return an error to the LSP client instead of calling `build()`.
/// This guarantees the client never receives a partial rename.
///
/// Time: O(locations) to add all edits; O(1) to build.
pub struct RenameEditBuilder {
    changes: HashMap<Url, Vec<TextEdit>>,
}

impl RenameEditBuilder {
    pub fn new() -> Self {
        Self {
            changes: HashMap::new(),
        }
    }

    /// Record a replacement of `range` in `url` with `new_text`.
    ///
    /// Calling `add` never fails — conversion errors must be detected BEFORE
    /// calling `add` so no partial state is accumulated.
    pub fn add(&mut self, url: Url, range: lsp_types::Range, new_text: String) {
        self.changes
            .entry(url)
            .or_default()
            .push(TextEdit { range, new_text });
    }

    /// Consume the builder and produce the final `WorkspaceEdit`.
    pub fn build(self) -> WorkspaceEdit {
        WorkspaceEdit {
            changes: Some(self.changes),
            document_changes: None,
            change_annotations: None,
        }
    }

    /// Total number of individual edits accumulated (for progress reporting).
    pub fn edit_count(&self) -> usize {
        self.changes.values().map(Vec::len).sum()
    }

    /// Number of distinct files with at least one edit.
    pub fn file_count(&self) -> usize {
        self.changes.len()
    }
}

impl Default for RenameEditBuilder {
    fn default() -> Self {
        Self::new()
    }
}
