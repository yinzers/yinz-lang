//! Comment attachment logic for the Yinz formatter.
//!
//! Given a `Vec<Comment>` from `lex_with_trivia` and the original source text, this module
//! answers two questions for every emitted node:
//!
//! 1. **Leading comments**: which `//` blocks appear DIRECTLY above this node (no blank line
//!    separating them)?
//! 2. **Inline comment**: is there a `//` comment on the same source line as the node?
//!
//! Comments not claimed by either query at any node become "floating" and are emitted
//! between the items they appear between (preserving their original position relative to
//! surrounding code).
//!
//! # Blank-line semantics
//!
//! A comment is "leading" for a node when the last byte of the comment and the first byte of
//! the node are separated by at most one intervening newline (i.e., no blank line between
//! them). Comments separated from the next node by a blank line are "floating" — they are
//! emitted in the gap between the previous item and the current item, but not attached to
//! either.

use ynz_parser::{Comment, CommentKind};

pub struct CommentContext<'a> {
    pub comments: &'a [Comment],
    pub source: &'a str,
    /// Byte offset of every `\n` in `source`, ascending. Built once in [`Self::new`] so
    /// [`Self::line_of`] is a binary search rather than a rescan — see that method.
    newlines: Vec<usize>,
}

impl<'a> CommentContext<'a> {
    pub fn new(comments: &'a [Comment], source: &'a str) -> Self {
        // One linear pass over the source, once per format() call.
        let newlines = source
            .bytes()
            .enumerate()
            .filter(|(_, b)| *b == b'\n')
            .map(|(i, _)| i)
            .collect();
        Self {
            comments,
            source,
            newlines,
        }
    }

    /// Line number (0-indexed) of the given byte offset in the source.
    ///
    /// PERF (2026-09-02): this was `source[..byte].bytes().filter(|b| *b == b'\n').count()` —
    /// a full rescan from byte 0 on EVERY call, O(file length) each time. `between()` calls it
    /// once per comment inside a filter and again in its backwards scan, and
    /// `inline_comment_after()` calls it once per comment inside a `find()`, with both invoked
    /// per emitted AST node. That compounded to O(nodes × comments × filesize) — measured
    /// cubic: 246 lines formatted in 0.18s while 1,352 lines took 91.18s (5.5x the input,
    /// 506x the time), making `ynz fmt` and LSP format-on-save unusable on any real file and
    /// accounting for 100% of two test suites' runtime (`idempotency.rs` calls format() twice,
    /// `semantic_roundtrip.rs` once).
    ///
    /// Counting newlines strictly before `byte` is exactly `partition_point` over the
    /// precomputed offsets, so the result is identical to the old scan — including the
    /// `min(len)` clamp, preserved below. It also removes a latent panic: the old version
    /// sliced `source[..byte]`, which aborts if `byte` is not a UTF-8 char boundary; indexing
    /// a `Vec<usize>` of newline offsets cannot.
    pub fn line_of(&self, byte: usize) -> usize {
        let byte = byte.min(self.source.len());
        self.newlines.partition_point(|&nl| nl < byte)
    }

    /// Collect leading + floating comments in the range `[prev_end, node_start)`.
    ///
    /// Returns `(leading, floating)`:
    /// - `leading`: comments immediately above `node_start` (no blank line between last comment
    ///   and node).
    /// - `floating`: comments separated from the node by a blank line. These should be emitted
    ///   AFTER the inter-item blank line, BEFORE the leading comments.
    ///
    /// The caller merges them in the correct order: blank-line separator, then floating
    /// comments, then leading comments, then the node itself.
    pub fn between(&self, prev_end: usize, node_start: usize) -> (Vec<&Comment>, Vec<&Comment>) {
        let prev_line = self.line_of(prev_end);
        let node_line = self.line_of(node_start);

        let in_range: Vec<&Comment> = self
            .comments
            .iter()
            .filter(|c| c.span.start >= prev_end && c.span.start < node_start)
            .filter(|c| {
                let cline = self.line_of(c.span.start);
                // At file start (prev_end == 0), prev_line == 0 and there is no
                // "previous item's inline comment" to exclude. Comments on line 0 of the
                // file would be incorrectly excluded by `cline > prev_line` (0 > 0 = false),
                // so we use `>=` here. For all subsequent items, `>` correctly excludes
                // inline comments that appear on the SAME line as the previous item's last
                // token (those belong to the previous item, not the upcoming one).
                let after_prev = if prev_end == 0 {
                    cline >= prev_line
                } else {
                    cline > prev_line
                };
                after_prev && cline < node_line
            })
            .collect();

        if in_range.is_empty() {
            return (vec![], vec![]);
        }

        // Comments that are "leading": directly above node with no blank line between them
        // and the node. We scan BACKWARDS from node_line.
        let mut leading: Vec<&Comment> = Vec::new();
        let mut current_line = node_line;

        // Build the leading block from the comments closest to the node backwards
        let mut sorted = in_range.clone();
        sorted.sort_by_key(|c| c.span.start);

        // Find the leading block: contiguous comments immediately before node_line (no gap)
        for comment in sorted.iter().rev() {
            let cline = self.line_of(comment.span.start);
            if cline + 1 < current_line {
                // Gap between this comment and the block above — stop the leading block
                break;
            }
            leading.push(comment);
            current_line = cline;
        }
        leading.reverse();

        // Anything in `in_range` NOT in `leading` is floating
        let leading_starts: std::collections::HashSet<usize> =
            leading.iter().map(|c| c.span.start).collect();
        let floating: Vec<&Comment> = sorted
            .iter()
            .filter(|c| !leading_starts.contains(&c.span.start))
            .copied()
            .collect();

        (leading, floating)
    }

    /// Find an inline comment on the same source line as `node_start`, after `node_start`.
    ///
    /// Returns the comment text (the `//` portion) with exactly one space between the code
    /// and the comment marker.
    pub fn inline_comment_after(&self, node_start: usize) -> Option<String> {
        let node_line = self.line_of(node_start);
        self.comments
            .iter()
            .find(|c| self.line_of(c.span.start) == node_line && c.span.start > node_start)
            .map(|c| {
                // Strip trailing \r from the comment text (CRLF input normalization)
                let text = c.text.trim_end_matches('\r');
                format!("  {text}")
            })
    }

    /// Doc comments attached to a function or shape come from the AST (via `FunctionDecl.doc`
    /// and `ShapeDecl.doc`), not from the trivia vec. This method strips any doc comments from
    /// the `between()` result so they're not double-emitted.
    pub fn is_doc_comment(c: &Comment) -> bool {
        matches!(c.kind, CommentKind::DocComment)
    }
}
