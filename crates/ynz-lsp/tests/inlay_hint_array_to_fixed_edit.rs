// WHY: The `array→fixed` inlay hint is a Replacement-category auto-promotion.
// Per `.claude/rules/inference.md` "Two Surfaces for the Same Decision", both
// Replacement-category auto-promotions (`array→fixed` and `let→const`) must
// carry a TextEdit so the user can click the decoration to rewrite the source.
// Without the edit, the decoration is a dead teaching surface — the user sees
// the hint but clicking it does nothing, breaking the "compiler is a teacher"
// contract (Golden Rule 11). This test catches regressions where the TextEdit
// is dropped or never attached.

use lsp_types::{Position, Range};
use ynz_lsp::{
    capabilities::PositionEncoding,
    inlay_hint::inlay_hint_response,
    state::ServerState,
};

fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path(path).expect("valid path");
    state.open_document(uri.clone(), src.to_string());
    (state, uri)
}

fn full_range() -> Range {
    Range {
        start: Position { line: 0, character: 0 },
        end: Position { line: 9999, character: 0 },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test: array→fixed hint carries a TextEdit
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn array_to_fixed_hint_carries_text_edit() {
    // WHY: the array→fixed Replacement-category hint must have a TextEdit so
    // clicking it rewrites `array` to `fixed` in the annotation. A missing
    // TextEdit means the decoration is dead — the user can never act on the hint.
    // This test MUST fail on pre-fix code (no edit attached) and pass post-fix.
    let src = concat!(
        "function entrypoint() -> nothing {\n",
        "  let nums: array<int> = [1, 2, 3]\n",
        "}\n",
    );
    let (state, uri) = state_single("/tmp/ynz_atf_edit.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());

    // Find the array→fixed promotion hint (label contains "fixed").
    let atf_hints: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                s.contains("fixed")
            } else {
                false
            }
        })
        .collect();

    assert!(
        !atf_hints.is_empty(),
        "expected an array→fixed promotion hint; got no 'fixed' hints. All hint labels: {:?}",
        hints.iter().filter_map(|h| if let lsp_types::InlayHintLabel::String(s) = &h.label { Some(s) } else { None }).collect::<Vec<_>>()
    );

    // Every array→fixed hint must carry at least one TextEdit.
    for hint in &atf_hints {
        let edits = hint.text_edits.as_deref().unwrap_or(&[]);
        assert!(
            !edits.is_empty(),
            "array→fixed hint carries no TextEdit — click-to-make-explicit is broken. \
             hint label: {:?}, position: {:?}",
            hint.label,
            hint.position,
        );
    }
}

#[test]
fn array_to_fixed_edit_replaces_array_keyword_with_fixed() {
    // WHY: the TextEdit must replace exactly the `array` keyword, not the whole
    // type annotation. After applying the edit, `array<int>` becomes `fixed<int>`,
    // which is syntactically valid Yinz (verified textually here: `fixed<int>`
    // present, `array<int>` absent, edit width == "array".len()). Replacing too much
    // (e.g. including `<int>`) would corrupt the type annotation.
    let src = concat!(
        "function entrypoint() -> nothing {\n",
        "  let nums: array<int> = [1, 2, 3]\n",
        "}\n",
    );
    let (state, uri) = state_single("/tmp/ynz_atf_edit2.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());

    let atf_hints: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                s.contains("fixed")
            } else {
                false
            }
        })
        .collect();

    assert!(!atf_hints.is_empty(), "no array→fixed hint found");

    // Apply the first edit to the source text and verify the result.
    let edit = atf_hints[0]
        .text_edits
        .as_deref()
        .and_then(|edits| edits.first())
        .expect("array→fixed hint must carry at least one TextEdit");

    // Convert LSP position → byte offset to apply the edit in-place.
    let edit_start_line = edit.range.start.line as usize;
    let edit_start_char = edit.range.start.character as usize;
    let edit_end_line   = edit.range.end.line as usize;
    let edit_end_char   = edit.range.end.character as usize;

    // Compute byte offsets (UTF-8 assumption; PositionEncoding is Utf8 above).
    let src_bytes = src.as_bytes();
    let start_byte = line_char_to_byte(src_bytes, edit_start_line, edit_start_char);
    let end_byte   = line_char_to_byte(src_bytes, edit_end_line,   edit_end_char);

    let replaced = format!(
        "{}{}{}",
        &src[..start_byte],
        edit.new_text,
        &src[end_byte..],
    );

    // The replaced fragment at the edit site must be "fixed", not "array".
    assert!(
        replaced.contains("fixed<int>"),
        "after applying the edit, source must contain 'fixed<int>'; got:\n{replaced}"
    );
    assert!(
        !replaced.contains("array<int>"),
        "after applying the edit, source must NOT contain 'array<int>'; got:\n{replaced}"
    );

    // The edit must cover exactly "array" (5 bytes) and replace it with "fixed" (5 bytes).
    let edit_width_bytes = end_byte - start_byte;
    assert_eq!(
        edit_width_bytes,
        "array".len(),
        "edit must cover exactly the 'array' keyword ({} bytes), not {} bytes",
        "array".len(),
        edit_width_bytes,
    );
    assert_eq!(
        edit.new_text, "fixed",
        "edit new_text must be 'fixed'; got {:?}",
        edit.new_text,
    );
}

#[test]
fn let_to_const_edit_unchanged_no_regression() {
    // WHY: Phase 5 only adds an edit to the array→fixed path. The let→const edit
    // was implemented earlier and must remain intact — any accidental removal or
    // corruption of that path would silently break the let→const teaching surface.
    let src = concat!(
        "function entrypoint() -> nothing {\n",
        "  let count = 5\n",
        "}\n",
    );
    let (state, uri) = state_single("/tmp/ynz_atf_ltc_reg.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());

    let const_hints: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                s.contains("const")
            } else {
                false
            }
        })
        .collect();

    assert!(
        !const_hints.is_empty(),
        "let→const hint must still fire for a never-reassigned let"
    );

    let edit = const_hints[0]
        .text_edits
        .as_deref()
        .and_then(|edits| edits.first())
        .expect("let→const hint must carry a TextEdit (regression from Phase 4)");

    assert_eq!(
        edit.new_text, "const",
        "let→const edit new_text must be 'const'; got {:?}",
        edit.new_text,
    );
}

#[test]
fn every_array_to_fixed_hint_in_file_carries_an_edit() {
    // WHY: across multiple annotated `array<int>` bindings in one file, EVERY
    // array→fixed hint must carry a click-to-make-explicit TextEdit — no hint may
    // be left edit-less. Guards against a partial regression where the edit is wired
    // for one code path but a sibling binding still emits the old edit-less hint.
    // (The no-annotation/None-span path is not reachable today: the array→fixed hint
    // only fires on `array<T>`-annotated bindings, whose `Type::Generic` always carries
    // a `name_span`; the Option on `type_keyword_span` is defensive for a future
    // inferred-array promotion. See the field doc in inlay_hint_passes.rs.)
    let src = concat!(
        "function entrypoint() -> nothing {\n",
        "  let nums: array<int> = [1, 2, 3]\n",
        "  let grown: array<int> = []\n",
        "}\n",
    );
    let (state, uri) = state_single("/tmp/ynz_atf_all_edits.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    for hint in &hints {
        if let lsp_types::InlayHintLabel::String(s) = &hint.label {
            if s.contains("fixed") {
                assert!(
                    hint.text_edits.as_deref().map_or(false, |e| !e.is_empty()),
                    "every array→fixed hint must carry a TextEdit; hint: {:?}", hint.label
                );
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Byte-offset helper (test-only)
// ─────────────────────────────────────────────────────────────────────────────

/// Convert (line, UTF-8 character offset) to a byte offset in `text`.
///
/// Line and character are both 0-indexed, matching LSP UTF-8 position encoding.
///
/// Time: O(n) where n = byte offset into the file.  Space: O(1).
fn line_char_to_byte(text: &[u8], line: usize, character: usize) -> usize {
    let mut current_line = 0usize;
    let mut pos = 0usize;
    while pos < text.len() && current_line < line {
        if text[pos] == b'\n' {
            current_line += 1;
        }
        pos += 1;
    }
    // `pos` is now at the start of the target line.
    pos + character
}
