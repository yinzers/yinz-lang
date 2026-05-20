mod harness;
use harness::SubprocessHarness;
use serde_json::json;

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
