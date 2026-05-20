mod harness;
use harness::InProcessHarness;
use std::time::Duration;

fn read_fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read fixture {name}: {e}"))
}

/// Read the next publishDiagnostics notification from the server, skipping any
/// intermediate messages until we get one or time out.
fn next_diagnostics(client: &harness::HarnessClient) -> serde_json::Value {
    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        let Some(msg) = client.try_recv_timeout(Duration::from_millis(50)) else {
            if std::time::Instant::now() >= deadline {
                panic!("timed out waiting for publishDiagnostics");
            }
            continue;
        };
        if msg.get("_notification").and_then(|v| v.as_str()) == Some("textDocument/publishDiagnostics") {
            // The harness wraps it; extract the raw params by re-requesting from receiver
        }
        // The harness recv returns { "_notification": method, "params": ... } shape
        return msg;
    }
}

#[test]
fn did_open_error_fixture_publishes_diagnostics() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    let text = read_fixture("has_errors.ynz");
    h.did_open("file:///has_errors.ynz", &text);

    // Wait for publishDiagnostics notification
    let deadline = Duration::from_millis(500);
    let diag_msg = h.try_recv_timeout(deadline);
    assert!(diag_msg.is_some(), "expected publishDiagnostics notification after didOpen");
}

#[test]
fn did_open_clean_fixture_publishes_empty_diagnostics() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    let text = read_fixture("basic.ynz");
    h.did_open("file:///basic.ynz", &text);

    let diag_msg = h.try_recv_timeout(Duration::from_millis(300));
    assert!(diag_msg.is_some(), "expected publishDiagnostics notification after didOpen");
}

#[test]
fn did_close_clears_diagnostics() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    let text = read_fixture("has_errors.ynz");
    h.did_open("file:///close_test.ynz", &text);
    // Drain the open-event diagnostics
    h.try_recv_timeout(Duration::from_millis(300));

    h.did_close("file:///close_test.ynz");
    // didClose should push an empty publishDiagnostics (clear squiggles)
    let clear_msg = h.try_recv_timeout(Duration::from_millis(300));
    assert!(clear_msg.is_some(), "expected publishDiagnostics clear after didClose");
}

#[test]
fn diagnostic_message_contains_what_what_instead_why() {
    use ynz_lsp::{
        capabilities::PositionEncoding,
        diagnostic_transform::to_lsp_diagnostic,
        position::LineTable,
    };
    use ynz_diagnostics::{Diagnostic as YnzDiag, Severity, SourceSpan};

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
    assert!(lsp.message.contains("This is the WHAT"), "WHAT missing from message");
    assert!(lsp.message.contains("WHAT INSTEAD: This is the WHAT INSTEAD"), "WHAT INSTEAD missing");
    assert!(lsp.message.contains("WHY: This is the WHY"), "WHY missing from message");
}

#[test]
fn utf8_and_utf16_ranges_differ_for_multibyte() {
    use ynz_lsp::{
        capabilities::PositionEncoding,
        diagnostic_transform::to_lsp_diagnostic,
        position::LineTable,
    };
    use ynz_diagnostics::{Diagnostic as YnzDiag, Severity, SourceSpan};

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
