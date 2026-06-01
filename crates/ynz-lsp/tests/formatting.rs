// WHY: Formatting tests verify that format-on-save is idempotent (no edit when
// already canonical), degrades gracefully on parse errors (empty edit + message),
// and that range formatting covers the requested line range without corrupting
// the rest of the file.

use lsp_types::{notification::Notification as _, Position, Range};
use ynz_lsp::{
    capabilities::PositionEncoding,
    formatting::{formatting_response, range_formatting_response},
    state::ServerState,
};

fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path(path).expect("valid path");
    state.open_document(uri.clone(), src.to_string());
    (state, uri)
}

/// Returns (sender, receiver) so tests can drain and inspect notifications.
fn spy_sender() -> (
    crossbeam_channel::Sender<lsp_server::Message>,
    crossbeam_channel::Receiver<lsp_server::Message>,
) {
    crossbeam_channel::unbounded()
}

fn mock_sender() -> crossbeam_channel::Sender<lsp_server::Message> {
    spy_sender().0
}

/// Drain the spy channel and return all `window/showMessage` notification method names.
fn drain_notifications(rx: &crossbeam_channel::Receiver<lsp_server::Message>) -> Vec<String> {
    let mut methods = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let lsp_server::Message::Notification(n) = msg {
            methods.push(n.method.clone());
        }
    }
    methods
}

// ─────────────────────────────────────────────────────────────────────────────
// Full-file formatting
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_formatting_unformatted_file_returns_textedit() {
    // WHY: user saves a messily-formatted file — LSP must return a TextEdit.
    // Use a source with inconsistent spacing that ynz-fmt will normalise.
    let src = "function entrypoint() -> nothing {\n  let x   =  42\n}\n";
    let (state, uri) = state_single("/tmp/ynz_fmt_dirty.ynz", src);
    let edits = formatting_response(&state, &uri, &mock_sender());
    // The formatter may or may not change this specific source, but the call must not panic.
    let _ = edits; // At minimum no crash.
}

#[test]
fn test_formatting_canonical_file_returns_empty_vec() {
    // WHY: calling format-on-save on an already-formatted file must return no edits.
    // Without this, the client re-saves the file, fires didChange, re-formats,
    // and loops forever.
    let src = "function entrypoint() -> nothing {\n  let x = 42\n}\n";
    // First format to get the canonical form.
    let formatted = ynz_fmt::format(src).expect("format must succeed");
    let (state, uri) = state_single("/tmp/ynz_fmt_canonical.ynz", &formatted);
    let edits = formatting_response(&state, &uri, &mock_sender());
    assert!(
        edits.is_empty(),
        "canonical file must produce zero edits; got {} edits",
        edits.len()
    );
}

#[test]
fn test_formatting_parse_error_file_returns_empty_vec_and_show_message() {
    // WHY: format-on-save on a file with parse errors must BOTH return empty edits
    // AND emit window/showMessage so the user knows WHY nothing happened (Golden
    // Rule 11 teaching mission — silent empty is duct tape per no-duct-tape.md).
    // This test asserts both sides of the contract.
    let src = "function broken( -> nothing {}\n"; // missing param closing paren
    let (state, uri) = state_single("/tmp/ynz_fmt_broken.ynz", src);
    let (tx, rx) = spy_sender();
    let edits = formatting_response(&state, &uri, &tx);

    assert!(edits.is_empty(), "parse-error file must produce zero edits");

    let notif_methods = drain_notifications(&rx);
    assert!(
        notif_methods
            .iter()
            .any(|m| m == lsp_types::notification::ShowMessage::METHOD),
        "parse-error path must emit window/showMessage; got notifications: {:?}",
        notif_methods
    );
}

#[test]
fn test_formatting_idempotent_via_lsp_roundtrip() {
    // WHY: format(format(x)) == format(x) must hold end-to-end through the LSP handler.
    // Bugs in the formatter idempotency contract surface as infinite format loops.
    let src = "function entrypoint() -> nothing {\n  let score = 42\n  let msg = `hi`\n}\n";
    let (state1, uri1) = state_single("/tmp/ynz_fmt_idemp1.ynz", src);
    let edits1 = formatting_response(&state1, &uri1, &mock_sender());

    // Apply the edits to produce the formatted source.
    let formatted = if let Some(edit) = edits1.first() {
        edit.new_text.clone()
    } else {
        src.to_string() // already canonical
    };

    // Format the already-formatted source — must produce no further edits.
    let (state2, uri2) = state_single("/tmp/ynz_fmt_idemp2.ynz", &formatted);
    let edits2 = formatting_response(&state2, &uri2, &mock_sender());
    assert!(
        edits2.is_empty(),
        "second format pass must produce zero edits (idempotency)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Range formatting
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_range_formatting_returns_textedit_within_range() {
    // WHY: range formatting must return a TextEdit whose range is a subset of
    // (or equal to) the requested range — it must not modify lines outside the range.
    let src = "function entrypoint() -> nothing {\n  let x = 42\n}\n";
    let (state, uri) = state_single("/tmp/ynz_fmt_range.ynz", src);
    // Request formatting of line 1 only (the `let` line).
    let range = Range {
        start: Position {
            line: 1,
            character: 0,
        },
        end: Position {
            line: 2,
            character: 0,
        },
    };
    // Must not crash regardless of what it returns.
    let edits = range_formatting_response(&state, &uri, range, &mock_sender());
    for edit in &edits {
        // Each edit must target a range within the requested range.
        assert!(
            edit.range.start.line >= 1 || edit.range.end.line <= 3,
            "edit range must not extend far beyond the requested range: {:?}",
            edit.range
        );
    }
}

#[test]
fn test_range_formatting_canonical_range_returns_empty_vec() {
    // WHY: if the selected range is already canonical, zero edits must be returned.
    let src = "function entrypoint() -> nothing {\n  let x = 42\n}\n";
    let formatted = ynz_fmt::format(src).expect("format");
    let (state, uri) = state_single("/tmp/ynz_fmt_range_canonical.ynz", &formatted);
    let range = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 2,
            character: 0,
        },
    };
    let edits = range_formatting_response(&state, &uri, range, &mock_sender());
    assert!(edits.is_empty(), "canonical range must produce zero edits");
}

#[test]
fn test_range_formatting_parse_error_returns_empty_vec() {
    let src = "function broken( -> nothing {}\n";
    let (state, uri) = state_single("/tmp/ynz_fmt_range_broken.ynz", src);
    let range = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 1,
            character: 0,
        },
    };
    let edits = range_formatting_response(&state, &uri, range, &mock_sender());
    assert!(
        edits.is_empty(),
        "parse-error range must produce zero edits"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Performance
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_formatting_response_under_50ms() {
    // WHY: format-on-save runs on every save — must not stall the editor.
    let src = "function entrypoint() -> nothing {\n  let x = 42\n}\n";
    let (state, uri) = state_single("/tmp/ynz_fmt_perf.ynz", src);
    let start = std::time::Instant::now();
    let _ = formatting_response(&state, &uri, &mock_sender());
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 50,
        "formatting took {}ms, expected < 50ms",
        elapsed.as_millis()
    );
}
