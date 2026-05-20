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
fn did_open_triggers_publish_diagnostics() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    h.did_open("file:///test.ynz", "function entrypoint() -> nothing { }");
    // Phase 3: didOpen now triggers publishDiagnostics
    let msg = h.try_recv_timeout(Duration::from_millis(300));
    assert!(msg.is_some(), "expected publishDiagnostics notification after didOpen");
}

#[test]
fn did_change_triggers_publish_diagnostics() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    h.did_open("file:///test.ynz", "let x: int = 1");
    h.try_recv_timeout(Duration::from_millis(200)); // drain didOpen diagnostic
    h.did_change("file:///test.ynz", "let x: int = 2", 2);
    let msg = h.try_recv_timeout(Duration::from_millis(300));
    assert!(msg.is_some(), "expected publishDiagnostics notification after didChange");
}

#[test]
fn did_close_sends_empty_publish_diagnostics() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    h.did_open("file:///test.ynz", "let x: int = 1");
    h.try_recv_timeout(Duration::from_millis(200)); // drain didOpen diagnostic
    h.did_close("file:///test.ynz");
    let msg = h.try_recv_timeout(Duration::from_millis(300));
    assert!(msg.is_some(), "expected publishDiagnostics clear after didClose");
}

#[test]
fn shutdown_exit_sequence() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    h.shutdown(); // should complete cleanly; no panic
}

#[test]
fn salsa_cache_invalidated_on_did_change() {
    use std::sync::Arc;
    use ynz_lsp::{capabilities::PositionEncoding, state::ServerState};
    use ynz_parser::db::SourceFile;
    use ynz_typeck::queries::check_query;

    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri: lsp_types::Url = "file:///test.ynz".parse().unwrap();

    // Open with a known-good program.
    state.open_document(uri.clone(), "function entrypoint() -> nothing { }".to_string());
    let sf: SourceFile = state.source_file_for(&uri).unwrap();

    // Run check_query once — result gets memoized by salsa.
    let result_before: Arc<_> = check_query(&state.db, sf);
    let ptr_before = Arc::as_ptr(&result_before);

    // Update the document — salsa should invalidate the cached query.
    state.update_document(&uri, "function entrypoint() -> nothing { let x: int = 99 }".to_string());
    let sf2: SourceFile = state.source_file_for(&uri).unwrap();

    // Re-run check_query — must produce a NEW Arc (cache miss after input change).
    let result_after: Arc<_> = check_query(&state.db, sf2);
    let ptr_after = Arc::as_ptr(&result_after);

    assert_ne!(
        ptr_before, ptr_after,
        "salsa did not invalidate check_query after didChange — cache hit when miss expected"
    );

    // Run a THIRD time without any mutation — must be a cache HIT (same Arc).
    let result_again: Arc<_> = check_query(&state.db, sf2);
    let ptr_again = Arc::as_ptr(&result_again);
    assert_eq!(
        ptr_after, ptr_again,
        "salsa should reuse cached result on second identical query"
    );
}
