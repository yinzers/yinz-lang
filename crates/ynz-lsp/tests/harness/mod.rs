// Each test file includes only the subset it needs; suppress dead-code for the remainder.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{json, Value};

/// In-process harness that drives ynz-lsp through its public `serve()` API.
/// Uses an lsp-server in-process Connection pair so no subprocess is needed.
pub struct InProcessHarness {
    pub client: lsp_server::Connection,
    pub server: lsp_server::Connection,
}

impl InProcessHarness {
    pub fn new() -> Self {
        let (client, server) = lsp_server::Connection::memory();
        Self { client, server }
    }

    /// Drive `serve()` on the server-side connection in a background thread.
    pub fn start_server(self) -> HarnessClient {
        let server_conn = self.server;
        std::thread::spawn(move || {
            ynz_lsp::serve(server_conn);
        });
        HarnessClient { conn: self.client }
    }
}

/// Client side of the in-process harness.
pub struct HarnessClient {
    pub conn: lsp_server::Connection,
}

impl HarnessClient {
    pub fn initialize(&self) -> Value {
        self.conn
            .sender
            .send(lsp_server::Message::Request(lsp_server::Request {
                id: lsp_server::RequestId::from(1i32),
                method: "initialize".to_string(),
                params: json!({
                    "processId": null,
                    "rootUri": null,
                    "capabilities": {
                        "general": { "positionEncodings": ["utf-8"] }
                    }
                }),
            }))
            .unwrap();
        // Send initialized notification
        self.conn
            .sender
            .send(lsp_server::Message::Notification(
                lsp_server::Notification {
                    method: "initialized".to_string(),
                    params: json!({}),
                },
            ))
            .unwrap();
        // Read the initialize response
        self.recv_response()
    }

    pub fn did_open(&self, uri: &str, text: &str) {
        self.conn
            .sender
            .send(lsp_server::Message::Notification(
                lsp_server::Notification {
                    method: "textDocument/didOpen".to_string(),
                    params: json!({
                        "textDocument": {
                            "uri": uri,
                            "languageId": "ynz",
                            "version": 1,
                            "text": text
                        }
                    }),
                },
            ))
            .unwrap();
    }

    pub fn did_change(&self, uri: &str, new_text: &str, version: i32) {
        self.conn
            .sender
            .send(lsp_server::Message::Notification(
                lsp_server::Notification {
                    method: "textDocument/didChange".to_string(),
                    params: json!({
                        "textDocument": { "uri": uri, "version": version },
                        "contentChanges": [{ "text": new_text }]
                    }),
                },
            ))
            .unwrap();
    }

    pub fn did_close(&self, uri: &str) {
        self.conn
            .sender
            .send(lsp_server::Message::Notification(
                lsp_server::Notification {
                    method: "textDocument/didClose".to_string(),
                    params: json!({ "textDocument": { "uri": uri } }),
                },
            ))
            .unwrap();
    }

    pub fn shutdown(&self) {
        self.conn
            .sender
            .send(lsp_server::Message::Request(lsp_server::Request {
                id: lsp_server::RequestId::from(99i32),
                method: "shutdown".to_string(),
                params: serde_json::Value::Null,
            }))
            .unwrap();
        self.recv_response(); // consume the Ok response
                              // exit is fire-and-forget — server may have already closed its channel after shutdown
        self.conn
            .sender
            .send(lsp_server::Message::Notification(
                lsp_server::Notification {
                    method: "exit".to_string(),
                    params: json!({}),
                },
            ))
            .ok();
    }

    /// Read the next response from the server (blocks).
    pub fn recv_response(&self) -> Value {
        match self.conn.receiver.recv().unwrap() {
            lsp_server::Message::Response(r) => r.result.unwrap_or(serde_json::Value::Null),
            lsp_server::Message::Notification(n) => {
                json!({ "_notification": n.method, "params": n.params })
            }
            lsp_server::Message::Request(_) => {
                json!({ "_unexpected": "server_sent_request" })
            }
        }
    }

    /// Try to read a message with a timeout. Returns None if nothing arrives.
    pub fn try_recv_timeout(&self, dur: std::time::Duration) -> Option<Value> {
        self.conn
            .receiver
            .recv_timeout(dur)
            .ok()
            .map(|msg| match msg {
                lsp_server::Message::Response(r) => r.result.unwrap_or_default(),
                lsp_server::Message::Notification(n) => json!({ "_notification": n.method }),
                lsp_server::Message::Request(_) => json!({ "_unexpected": "server_request" }),
            })
    }

    /// Like recv_response() but with a timeout; returns None if nothing arrives.
    /// Unlike try_recv_timeout(), notifications include their `params` field.
    pub fn try_recv_with_params(&self, dur: std::time::Duration) -> Option<Value> {
        self.conn
            .receiver
            .recv_timeout(dur)
            .ok()
            .map(|msg| match msg {
                lsp_server::Message::Response(r) => r.result.unwrap_or(serde_json::Value::Null),
                lsp_server::Message::Notification(n) => {
                    json!({ "_notification": n.method, "params": n.params })
                }
                lsp_server::Message::Request(_) => json!({ "_unexpected": "server_request" }),
            })
    }
}

// ─── Subprocess harness ───────────────────────────────────────────────────────

/// Subprocess harness for stdio wire-format smoke tests.
pub struct SubprocessHarness {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl SubprocessHarness {
    pub fn spawn() -> Self {
        let binary = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("ynz-lsp");
        let mut child = Command::new(&binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|_| panic!("failed to spawn {:?}", binary));
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
        }
    }

    pub fn send(&mut self, msg: &Value) {
        let body = serde_json::to_string(msg).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
        self.stdin.flush().unwrap();
    }

    /// Read one LSP message from stdout.
    pub fn recv(&mut self) -> Value {
        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line).unwrap();
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(rest) = trimmed.strip_prefix("Content-Length: ") {
                content_length = rest.trim().parse().unwrap();
            }
        }
        let mut body = vec![0u8; content_length];
        self.stdout.read_exact(&mut body).unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    pub fn kill(mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

impl Drop for SubprocessHarness {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}
