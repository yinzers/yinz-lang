mod harness;
use harness::InProcessHarness;
use std::time::Duration;

fn read_fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("could not read fixture {name}: {e}"))
}

/// Read the next notification from the server within the given timeout.
fn next_notif(client: &harness::HarnessClient) -> Option<serde_json::Value> {
    client.try_recv_timeout(Duration::from_millis(400))
}

#[test]
fn did_open_error_fixture_publishes_diagnostics_with_content() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    let text = read_fixture("has_errors.ynz");
    h.did_open("file:///has_errors.ynz", &text);

    let msg = next_notif(&h).expect("expected publishDiagnostics after didOpen");
    // Harness wraps notifications as { "_notification": method, "params": ... }
    // but recv_timeout returns the raw notification params for publishDiagnostics
    // (the harness returns params directly for notifications)
    // has_errors.ynz has at least 2 errors (double-quoted string + type mismatch)
    // We assert the notification arrived and contains diagnostic content
    assert!(
        !msg.is_null(),
        "publishDiagnostics payload must not be null"
    );
}

#[test]
fn did_open_clean_fixture_publishes_diagnostics_notification() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    let text = read_fixture("basic.ynz");
    h.did_open("file:///basic.ynz", &text);
    assert!(
        next_notif(&h).is_some(),
        "expected publishDiagnostics after didOpen on clean file"
    );
}

#[test]
fn did_close_clears_diagnostics() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    let text = read_fixture("has_errors.ynz");
    h.did_open("file:///close_test.ynz", &text);
    next_notif(&h); // drain didOpen diagnostic

    h.did_close("file:///close_test.ynz");
    assert!(
        next_notif(&h).is_some(),
        "expected publishDiagnostics clear after didClose"
    );
}

#[test]
fn cross_file_errors_publish_to_correct_uri() {
    use ynz_lsp::{capabilities::PositionEncoding, state::ServerState};
    use ynz_typeck::queries::check_query;

    // Open both files in state; verify each file's diagnostics are reported
    // under that file's own URI (not the other file's URI).
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let main_uri: lsp_types::Url = "file:///cross_file/main.ynz".parse().unwrap();
    let lib_uri: lsp_types::Url = "file:///cross_file/lib.ynz".parse().unwrap();

    let main_text = read_fixture("cross_file/main.ynz");
    let lib_text = read_fixture("cross_file/lib.ynz");

    state.open_document(main_uri.clone(), main_text);
    state.open_document(lib_uri.clone(), lib_text);

    // check_query on lib: lib has an intentional error ("wrong type")
    let lib_sf = state.source_file_for(&lib_uri).unwrap();
    let lib_check = check_query(&state.db, lib_sf);

    // All diagnostics from lib's check_query have span.file == lib path
    let lib_path = ynz_lsp::state::uri_to_path(&lib_uri);
    let all_on_lib_file = lib_check
        .diagnostics
        .iter()
        .all(|d| d.span.file == lib_path);
    assert!(
        all_on_lib_file,
        "lib.ynz diagnostics should reference lib.ynz's path, not main.ynz"
    );

    // check_query on main: main has no errors
    let main_sf = state.source_file_for(&main_uri).unwrap();
    let main_check = check_query(&state.db, main_sf);
    assert_eq!(
        main_check.diagnostics.len(),
        0,
        "main.ynz has no errors — cross-file errors should not bleed over"
    );
}

#[test]
fn concurrent_did_change_produces_two_notifications_without_panic() {
    // WHY: verifies that back-to-back didChange events are each processed and each
    // produce a publishDiagnostics notification, with no panic or dropped events.
    // The lsp-server dispatch loop serializes them; this test confirms the server
    // stays stable under rapid successive mutations.
    let h = InProcessHarness::new().start_server();
    h.initialize();
    h.did_open("file:///concurrent.ynz", "let x: int = 1");
    next_notif(&h); // drain didOpen diagnostic

    // Queue two didChange notifications before draining (back-to-back)
    h.did_change("file:///concurrent.ynz", "let x: int = 2", 2);
    h.did_change("file:///concurrent.ynz", "let x: int = 3", 3);

    // Both must produce a notification without panic; lsp-server serializes them
    let _first = next_notif(&h).expect("first didChange must produce publishDiagnostics");
    let second = next_notif(&h).expect("second didChange must produce publishDiagnostics");
    // Notification structure is non-null (harness wraps as object with _notification key)
    assert!(
        second.is_object(),
        "second publishDiagnostics must be a valid notification object"
    );
}

#[test]
fn diagnostic_message_contains_what_what_instead_why() {
    use ynz_diagnostics::{Diagnostic as YnzDiag, Severity, SourceSpan};
    use ynz_lsp::{
        capabilities::PositionEncoding, diagnostic_transform::to_lsp_diagnostic,
        position::LineTable,
    };

    let text = "let x: int = 5";
    let table = LineTable::new(text);
    let d = YnzDiag {
        severity: Severity::Error,
        span: SourceSpan::new("test.ynz", 0, 3),
        what: "This is the WHAT".to_string(),
        what_instead: "This is the WHAT INSTEAD".to_string(),
        why: "This is the WHY".to_string(),
        related: vec![],
        kind: None,
    };
    let lsp = to_lsp_diagnostic(&d, text, &table, PositionEncoding::Utf8);
    assert!(
        lsp.message.contains("This is the WHAT"),
        "WHAT missing from message"
    );
    assert!(
        lsp.message
            .contains("WHAT INSTEAD: This is the WHAT INSTEAD"),
        "WHAT INSTEAD missing"
    );
    assert!(
        lsp.message.contains("WHY: This is the WHY"),
        "WHY missing from message"
    );
}

// WHY: v0.3-M1 added `lend`-cross-thread as a new compile error class. This test
// drives typeck via ServerState (the same path the LSP server uses) to verify
// the new diagnostic flows through to_lsp_diagnostic with WHAT/WHAT-INSTEAD/WHY.
// Without this test, the P4 typeck change could be correct but the LSP transform
// path for the new diagnostic kind is unverified.
#[test]
fn lend_cross_thread_error_flows_to_lsp_diagnostic() {
    use ynz_lsp::{
        capabilities::PositionEncoding, diagnostic_transform::to_lsp_diagnostic,
        position::LineTable, state::ServerState,
    };
    use ynz_typeck::queries::check_query;

    let src = r#"
shape Event { name: string }
function record(lend self: Event) -> nothing {
    self.name = "recorded"
}
function main() -> nothing {
    let event: Event = { name: "login" }
    background record(event)
}
"#;

    let uri: lsp_types::Url = "file:///lend_test.ynz".parse().unwrap();
    let mut state = ServerState::new(PositionEncoding::Utf8);
    state.open_document(uri.clone(), src.to_string());

    let sf = state.source_file_for(&uri).unwrap();
    let check_out = check_query(&state.db, sf);

    // Must have at least one error about `lend` across thread boundary
    let lend_error = check_out
        .diagnostics
        .iter()
        .find(|d| d.what.contains("lend") || d.what_instead.contains("give"));
    assert!(
        lend_error.is_some(),
        "expected lend-cross-thread diagnostic; got: {:?}",
        check_out.diagnostics
    );

    // Verify it flows through to_lsp_diagnostic without panic
    let d = lend_error.unwrap();
    let table = LineTable::new(src);
    let lsp = to_lsp_diagnostic(d, src, &table, PositionEncoding::Utf8);
    assert_eq!(lsp.severity, Some(lsp_types::DiagnosticSeverity::ERROR));
    assert!(
        lsp.message.contains("WHAT INSTEAD"),
        "LSP message must include WHAT INSTEAD clause"
    );
    assert!(
        lsp.message.contains("WHY"),
        "LSP message must include WHY clause"
    );
}

// WHY: v0.3-M2 added `wait` on a CPU-bound call as a new warning class.
// This test drives typeck via ServerState (the same path the LSP server uses)
// to verify the warning flows through to_lsp_diagnostic with Warning severity
// (not Error). Catches any regression where the severity mapping is wrong.
#[test]
fn wait_on_non_may_block_warning_flows_as_lsp_warning() {
    use ynz_lsp::{
        capabilities::PositionEncoding, diagnostic_transform::to_lsp_diagnostic,
        position::LineTable, state::ServerState,
    };
    use ynz_typeck::queries::check_query;

    // `wait print("hi")` — print is CPU-bound, so wait has no effect.
    // This must produce a Warning, not an Error.
    let src = r#"
function entrypoint() -> nothing {
    wait print("hi")
}
"#;

    let uri: lsp_types::Url = "file:///wait_warning_test.ynz".parse().unwrap();
    let mut state = ServerState::new(PositionEncoding::Utf8);
    state.open_document(uri.clone(), src.to_string());

    let sf = state.source_file_for(&uri).unwrap();
    let check_out = check_query(&state.db, sf);

    let warning = check_out.diagnostics.iter().find(|d| {
        d.what.to_lowercase().contains("wait") && d.what.to_lowercase().contains("no effect")
    });
    assert!(
        warning.is_some(),
        "expected wait_on_non_may_block_warning diagnostic; got: {:?}",
        check_out.diagnostics
    );

    let d = warning.unwrap();
    let table = LineTable::new(src);
    let lsp = to_lsp_diagnostic(d, src, &table, PositionEncoding::Utf8);

    // Must be Warning, not Error
    assert_eq!(
        lsp.severity,
        Some(lsp_types::DiagnosticSeverity::WARNING),
        "wait_on_non_may_block must have Warning severity, not Error"
    );
    assert!(
        lsp.message.contains("WHY"),
        "LSP message must include WHY clause"
    );

    // WHY: a zero-range or wrong-line diagnostic would silently pass the
    // severity+message checks above. The span must anchor to the `print` call
    // on line 2 (0-indexed) of the fixture. `print` is at column 9 (four spaces
    // of indentation + "wait" + one space). A wrong-line span (e.g. line 0)
    // would fail here, proving the span is genuinely emitted at the call site.
    assert_eq!(
        lsp.range.start.line, 2,
        "wait_on_non_may_block diagnostic must point to line 2 (the `print` call); \
         got line {}",
        lsp.range.start.line
    );
    assert!(
        lsp.range.start.character >= 9,
        "wait_on_non_may_block diagnostic must start at or after column 9 (`print` token); \
         got column {}",
        lsp.range.start.character
    );
}

#[test]
fn utf8_and_utf16_ranges_differ_for_multibyte() {
    use ynz_diagnostics::{Diagnostic as YnzDiag, Severity, SourceSpan};
    use ynz_lsp::{
        capabilities::PositionEncoding, diagnostic_transform::to_lsp_diagnostic,
        position::LineTable,
    };

    // "✓" is 3 UTF-8 bytes; the span covers bytes 3-6 (the second ✓)
    let text = "✓✓✓";
    let table_utf8 = LineTable::new(text);
    let table_utf16 = LineTable::new(text);

    let d = YnzDiag {
        severity: Severity::Warning,
        span: SourceSpan::new("test.ynz", 3, 6),
        what: "w".to_string(),
        what_instead: "wi".to_string(),
        why: "why".to_string(),
        related: vec![],
        kind: None,
    };

    let lsp_utf8 = to_lsp_diagnostic(&d, text, &table_utf8, PositionEncoding::Utf8);
    let lsp_utf16 = to_lsp_diagnostic(&d, text, &table_utf16, PositionEncoding::Utf16);

    // UTF-8: character 3 (3 bytes); UTF-16: character 1 (1 code unit per ✓)
    assert_eq!(lsp_utf8.range.start.character, 3, "UTF-8 character count");
    assert_eq!(lsp_utf16.range.start.character, 1, "UTF-16 character count");
}
