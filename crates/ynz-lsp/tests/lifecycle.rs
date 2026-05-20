mod harness;
use harness::InProcessHarness;
use std::time::Duration;

#[test]
fn initialize_returns_capabilities() {
    let h = InProcessHarness::new().start_server();
    let result = h.initialize();
    let caps = &result["capabilities"];
    assert!(caps["textDocumentSync"].is_object() || caps["textDocumentSync"].is_number());
    assert!(caps["hoverProvider"].as_bool().unwrap_or(false));
    assert!(caps["completionProvider"].is_object());
}

#[test]
fn did_open_accepted() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    // didOpen should not crash and server should remain responsive.
    h.did_open("file:///test.ynz", "function entrypoint() -> nothing { }");
    // No publishDiagnostics in Phase 2 (that's Phase 3).
    // Verify no unexpected message arrives within a short window.
    let unexpected = h.try_recv_timeout(Duration::from_millis(50));
    assert!(
        unexpected.is_none(),
        "unexpected message from server after didOpen: {unexpected:?}"
    );
}

#[test]
fn did_change_accepted() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    h.did_open("file:///test.ynz", "let x: int = 1");
    h.did_change("file:///test.ynz", "let x: int = 2", 2);
    let unexpected = h.try_recv_timeout(Duration::from_millis(50));
    assert!(unexpected.is_none(), "unexpected message after didChange: {unexpected:?}");
}

#[test]
fn did_close_accepted() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    h.did_open("file:///test.ynz", "let x: int = 1");
    h.did_close("file:///test.ynz");
    let unexpected = h.try_recv_timeout(Duration::from_millis(50));
    assert!(unexpected.is_none(), "unexpected message after didClose: {unexpected:?}");
}

#[test]
fn shutdown_exit_sequence() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    h.shutdown(); // should complete cleanly; no panic
}

#[test]
fn salsa_cache_invalidated_on_did_change() {
    use ynz_lsp::{capabilities::PositionEncoding, state::ServerState};
    use ynz_parser::db::SourceFile;

    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri: lsp_types::Url = "file:///test.ynz".parse().unwrap();

    state.open_document(uri.clone(), "let x: int = 1".to_string());
    let sf_before: SourceFile = state.source_file_for(&uri).unwrap();
    let text_before = sf_before.text(&state.db).clone();

    state.update_document(&uri, "let x: int = 2".to_string());
    let sf_after: SourceFile = state.source_file_for(&uri).unwrap();
    let text_after = sf_after.text(&state.db).clone();

    assert_eq!(text_before, "let x: int = 1");
    assert_eq!(text_after, "let x: int = 2");
    assert_ne!(text_before, text_after, "salsa input should have been updated");
}
