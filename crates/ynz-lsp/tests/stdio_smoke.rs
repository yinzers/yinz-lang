mod harness;
use harness::SubprocessHarness;
use serde_json::json;

// Basic fixture content inlined so this test has no filesystem dependency
// beyond the ynz-lsp binary itself.
const BASIC_YNZ: &str = "\
// Basic fixture for LSP lifecycle tests.\n\
function entrypoint() -> nothing {\n\
    let x: int = 42\n\
    print(x.toString())\n\
}\n";

#[test]
fn stdio_smoke_full_sequence() {
    // WHY: Exercises the complete LSP wire-format sequence through a real
    // subprocess: initialize → didOpen → completion → hover → didClose → shutdown.
    // Verifies that each response is a valid JSON object (not an error) and that
    // the process exits cleanly. Catches regressions in the JSON-RPC framing or
    // in the message dispatch loop that only surface over the stdio wire.
    let mut h = SubprocessHarness::spawn();

    // initialize
    h.send(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "processId": null,
            "rootUri": null,
            "capabilities": {
                "general": { "positionEncodings": ["utf-8"] }
            }
        }
    }));
    let init_msg = h.recv();
    assert!(
        init_msg["result"].is_object(),
        "initialize must return an object result: {init_msg}"
    );
    assert!(
        init_msg["error"].is_null(),
        "initialize must not return an error: {init_msg}"
    );

    // initialized notification (no response expected)
    h.send(&json!({"jsonrpc":"2.0","method":"initialized","params":{}}));

    // didOpen — server will send a publishDiagnostics notification; consume it
    h.send(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///smoke_full.ynz",
                "languageId": "ynz",
                "version": 1,
                "text": BASIC_YNZ
            }
        }
    }));
    // Drain the publishDiagnostics notification (one message).
    let _diag_notif = h.recv();

    // completion request at (0,0)
    h.send(&json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": "file:///smoke_full.ynz" },
            "position": { "line": 0, "character": 0 }
        }
    }));
    let completion_msg = h.recv();
    assert!(
        completion_msg.is_object(),
        "completion response must be a JSON object: {completion_msg}"
    );
    assert!(
        completion_msg["error"].is_null(),
        "completion response must not contain an error: {completion_msg}"
    );

    // hover request at (0,0)
    h.send(&json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": "file:///smoke_full.ynz" },
            "position": { "line": 0, "character": 0 }
        }
    }));
    let hover_msg = h.recv();
    assert!(
        hover_msg.is_object(),
        "hover response must be a JSON object: {hover_msg}"
    );
    assert!(
        hover_msg["error"].is_null(),
        "hover response must not contain an error: {hover_msg}"
    );

    // didClose
    h.send(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didClose",
        "params": { "textDocument": { "uri": "file:///smoke_full.ynz" } }
    }));
    // Drain the publishDiagnostics clear notification.
    let _close_notif = h.recv();

    // shutdown + exit
    h.send(&json!({"jsonrpc":"2.0","id":4,"method":"shutdown"}));
    let shutdown_msg = h.recv();
    assert!(
        shutdown_msg["result"].is_null(),
        "shutdown must return null result: {shutdown_msg}"
    );

    h.send(&json!({"jsonrpc":"2.0","method":"exit","params":{}}));
    h.kill();
}

#[test]
fn stdio_initialize_shutdown() {
    let mut h = SubprocessHarness::spawn();

    h.send(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "processId": null,
            "rootUri": null,
            "capabilities": {}
        }
    }));

    // recv() returns the full JSON-RPC message; result field is InitializeResult
    let msg = h.recv();
    let result = &msg["result"];
    assert_eq!(result["capabilities"]["hoverProvider"], json!(true));
    assert!(result["capabilities"]["completionProvider"].is_object());
    // positionEncoding belongs inside ServerCapabilities (LSP 3.17 spec §3.15)
    assert_eq!(result["capabilities"]["positionEncoding"], json!("utf-16")); // client sent no encodings
    assert!(result["serverInfo"]["name"] == "ynz-lsp");

    h.send(&json!({"jsonrpc":"2.0","method":"initialized","params":{}}));

    h.send(&json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "shutdown"
    }));
    let shutdown_msg = h.recv();
    assert!(shutdown_msg["result"].is_null());

    h.send(&json!({"jsonrpc":"2.0","method":"exit","params":{}}));
    h.kill();
}
