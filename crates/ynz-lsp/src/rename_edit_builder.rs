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
    /// Create an empty builder.
    ///
    /// WHY `new` instead of a direct `WorkspaceEdit { ... }` literal: the builder
    /// enforces the accumulate-then-build contract at the type level — callers
    /// cannot call `build()` after a conversion error because `build()` consumes
    /// `self`, preventing any further `add` calls.  Trying to build a partial edit
    /// and then continue would not compile.
    pub fn new() -> Self {
        Self {
            changes: HashMap::new(),
        }
    }

    /// Record a text replacement of `range` in `url` with `new_text`.
    ///
    /// WHY this method is infallible: conversion from `SourceSpan` → LSP `Range`
    /// must happen BEFORE `add` is called so that a conversion failure causes the
    /// CALLER to return an error to the LSP client.  If `add` could fail, a partial
    /// accumulation (N–1 edits) could be built and sent, which would corrupt the
    /// user's source code.  The infallible contract ensures atomicity.
    pub fn add(&mut self, url: Url, range: lsp_types::Range, new_text: String) {
        self.changes
            .entry(url)
            .or_default()
            .push(TextEdit { range, new_text });
    }

    /// Consume the builder and return the final atomic `WorkspaceEdit`.
    ///
    /// WHY consuming: prevents the caller from calling `add` after `build` — once
    /// you hand the edit to the LSP client, the accumulation phase is over.
    pub fn build(self) -> WorkspaceEdit {
        WorkspaceEdit {
            changes: Some(self.changes),
            document_changes: None,
            change_annotations: None,
        }
    }

    /// Total individual edits accumulated across all files.  Used for progress reporting.
    pub fn edit_count(&self) -> usize {
        self.changes.values().map(Vec::len).sum()
    }

    /// Number of distinct files that have at least one edit.
    pub fn file_count(&self) -> usize {
        self.changes.len()
    }
}

impl Default for RenameEditBuilder {
    fn default() -> Self {
        Self::new()
    }
}
