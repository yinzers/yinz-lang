// WHY: documentSymbol and workspace/symbol integration tests verify the full
// LSP round-trip: params → handler → serialised DocumentSymbol/SymbolInformation
// response. Unit tests in ynz_lsp::document_symbol::tests cover the mapping
// logic; these tests confirm the server dispatch, param parsing, and
// serialisation path are all wired correctly.

mod harness;
use harness::InProcessHarness;
use serde_json::json;

// ─────────────────────────────────────────────────────────────────────────────
// textDocument/documentSymbol
// ─────────────────────────────────────────────────────────────────────────────

// WHY: a function declaration must appear in the documentSymbol response as a
// FUNCTION-kind symbol. Catching regressions where the handler returns Null or
// an empty array for non-empty files.
#[test]
fn document_symbol_returns_function_entry() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    let uri = "file:///ds_test.ynz";
    h.did_open(uri, "function greet() -> nothing { }");
    // Drain any diagnostic notification the server pushes.
    h.try_recv_timeout(std::time::Duration::from_millis(100));

    h.conn
        .sender
        .send(lsp_server::Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(10i32),
            method: "textDocument/documentSymbol".to_string(),
            params: json!({ "textDocument": { "uri": uri } }),
        }))
        .unwrap();

    let response = h.recv_response();
    assert!(
        response.get("error").is_none(),
        "documentSymbol response must not contain an error: {response}"
    );
    // Response is an array of DocumentSymbol objects.
    let symbols = response.as_array().expect("response must be a JSON array");
    assert!(
        !symbols.is_empty(),
        "open file with one function must have at least one symbol"
    );
    let names: Vec<&str> = symbols
        .iter()
        .filter_map(|s| s.get("name")?.as_str())
        .collect();
    assert!(
        names.contains(&"greet"),
        "symbol list must contain 'greet'; got: {names:?}"
    );
}

// WHY: the server must return a valid (possibly empty) response — not an error —
// when documentSymbol is requested for a URI the server hasn't seen.
// LSP spec says the result SHOULD be null or an empty list for unknown files.
#[test]
fn document_symbol_unknown_uri_returns_null_or_empty() {
    let h = InProcessHarness::new().start_server();
    h.initialize();

    h.conn
        .sender
        .send(lsp_server::Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(11i32),
            method: "textDocument/documentSymbol".to_string(),
            params: json!({ "textDocument": { "uri": "file:///not_opened.ynz" } }),
        }))
        .unwrap();

    let response = h.recv_response();
    assert!(
        response.get("error").is_none(),
        "documentSymbol for unknown URI must not error: {response}"
    );
    // Null OR an empty array are both acceptable.
    let ok = response.is_null() || response.as_array().map(|a| a.is_empty()).unwrap_or(false);
    assert!(
        ok,
        "unknown URI must yield null or empty array; got: {response}"
    );
}

// WHY: a shape declaration must appear as a CLASS-kind symbol in the response.
// Verifies that the SymbolKind mapping table is wired through the full LSP path,
// not just in the unit tests.
#[test]
fn document_symbol_shape_decl_returns_class_kind() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    let uri = "file:///ds_shape.ynz";
    h.did_open(uri, "shape Player { name: string\n health: int }");
    h.try_recv_timeout(std::time::Duration::from_millis(100));

    h.conn
        .sender
        .send(lsp_server::Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(12i32),
            method: "textDocument/documentSymbol".to_string(),
            params: json!({ "textDocument": { "uri": uri } }),
        }))
        .unwrap();

    let response = h.recv_response();
    let symbols = response.as_array().expect("response must be JSON array");
    let player = symbols
        .iter()
        .find(|s| s.get("name").and_then(|n| n.as_str()) == Some("Player"));
    assert!(player.is_some(), "Player shape must appear in symbol list");
    // LSP SymbolKind::Class == 5
    let kind = player.unwrap().get("kind").and_then(|k| k.as_u64());
    assert_eq!(
        kind,
        Some(5),
        "Player must have SymbolKind::Class (5); got: {kind:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// workspace/symbol
// ─────────────────────────────────────────────────────────────────────────────

// WHY: workspace/symbol with an empty query must return at least the symbols
// from files the server has registered. Verifies the handler round-trips through
// the LSP path and correctly serialises SymbolInformation.
#[test]
fn workspace_symbol_empty_query_returns_symbols() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    let uri = "file:///ws_all.ynz";
    h.did_open(uri, "function entrypoint() -> nothing { }");
    h.try_recv_timeout(std::time::Duration::from_millis(100));

    h.conn
        .sender
        .send(lsp_server::Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(20i32),
            method: "workspace/symbol".to_string(),
            params: json!({ "query": "" }),
        }))
        .unwrap();

    let response = h.recv_response();
    assert!(
        response.get("error").is_none(),
        "workspace/symbol must not error: {response}"
    );
    let symbols = response.as_array().expect("response must be a JSON array");
    assert!(
        !symbols.is_empty(),
        "empty query with at least one open file must return at least one symbol"
    );
}

// WHY: workspace/symbol with a non-matching query must return an empty array,
// not null or an error. Clients use the empty array to clear previous results.
#[test]
fn workspace_symbol_no_match_returns_empty_array() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    let uri = "file:///ws_nomatch.ynz";
    h.did_open(uri, "function entrypoint() -> nothing { }");
    h.try_recv_timeout(std::time::Duration::from_millis(100));

    h.conn
        .sender
        .send(lsp_server::Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(21i32),
            method: "workspace/symbol".to_string(),
            params: json!({ "query": "zzznomatch" }),
        }))
        .unwrap();

    let response = h.recv_response();
    assert!(
        response.get("error").is_none(),
        "workspace/symbol must not error on no-match: {response}"
    );
    let symbols = response
        .as_array()
        .expect("no-match must return a JSON array");
    assert!(
        symbols.is_empty(),
        "non-matching query must produce an empty array"
    );
}

// WHY: workspace/symbol must support case-insensitive substring matching.
// A query of "ENTRY" must hit a function named "entrypoint". Without this,
// the IDE's symbol search is frustratingly case-sensitive.
#[test]
fn workspace_symbol_case_insensitive_query() {
    let h = InProcessHarness::new().start_server();
    h.initialize();
    let uri = "file:///ws_case.ynz";
    h.did_open(uri, "function entrypoint() -> nothing { }");
    h.try_recv_timeout(std::time::Duration::from_millis(100));

    h.conn
        .sender
        .send(lsp_server::Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(22i32),
            method: "workspace/symbol".to_string(),
            params: json!({ "query": "ENTRY" }),
        }))
        .unwrap();

    let response = h.recv_response();
    let symbols = response.as_array().expect("response must be JSON array");
    let names: Vec<&str> = symbols
        .iter()
        .filter_map(|s| s.get("name")?.as_str())
        .collect();
    assert!(
        names.contains(&"entrypoint"),
        "case-insensitive query 'ENTRY' must match 'entrypoint'; got: {names:?}"
    );
}
