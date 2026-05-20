// lsp-server spike: minimal "hello LSP" measuring plumbing overhead + &mut DB ergonomics.
//
// Key measurement target: salsa CompilerDb requires &mut for input mutations (didChange).
// lsp-server is synchronous and single-threaded — the main loop owns the DB directly.
// No Arc<Mutex<...>> needed. The &mut db is available naturally in the request loop.

use lsp_server::{Connection, Message, Notification};
use lsp_types::notification::{DidChangeTextDocument, DidOpenTextDocument, Notification as _};
use lsp_types::*;

// Simulates CompilerDb: update() needs &mut self (like salsa input mutations),
// query() needs &self (like salsa tracked queries).
struct DbSimulation {
    text: String,
}

impl DbSimulation {
    fn update(&mut self, new_text: String) {
        self.text = new_text;
    }

    fn query(&self) -> String {
        format!("ynz-lsp spike: hello from lsp-server (doc len={})", self.text.len())
    }
}

fn publish_diagnostics(
    conn: &Connection,
    uri: Url,
    diagnostics: Vec<Diagnostic>,
) -> Result<(), Box<dyn std::error::Error>> {
    let params = PublishDiagnosticsParams { uri, diagnostics, version: None };
    let notif = Notification::new(
        "textDocument/publishDiagnostics".to_string(),
        params,
    );
    conn.sender.send(Message::Notification(notif))?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (connection, io_threads) = Connection::stdio();

    // initialize handshake
    let server_capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::FULL,
        )),
        ..Default::default()
    };
    let (id, _params) = connection.initialize_start()?;
    let init_result = InitializeResult {
        capabilities: server_capabilities,
        ..Default::default()
    };
    connection.initialize_finish(id, serde_json::to_value(init_result)?)?;

    // Owned directly — no Arc<Mutex<...>> needed. &mut db is available anywhere
    // in the request loop because only one task runs at a time.
    let mut db = DbSimulation { text: String::new() };

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    break;
                }
            }

            Message::Notification(notif) => {
                if notif.method == DidOpenTextDocument::METHOD {
                    let params: DidOpenTextDocumentParams =
                        serde_json::from_value(notif.params)?;
                    // Natural &mut — no lock acquisition, no await point.
                    db.update(params.text_document.text);
                    let message = db.query();
                    let diagnostic = Diagnostic {
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: 0, character: 5 },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
                        source: Some("ynz".to_string()),
                        message,
                        ..Default::default()
                    };
                    publish_diagnostics(
                        &connection,
                        params.text_document.uri,
                        vec![diagnostic],
                    )?;
                } else if notif.method == DidChangeTextDocument::METHOD {
                    let params: DidChangeTextDocumentParams =
                        serde_json::from_value(notif.params)?;
                    if let Some(change) = params.content_changes.into_iter().last() {
                        db.update(change.text);
                    }
                }
            }

            Message::Response(_) => {}
        }
    }

    io_threads.join()?;
    Ok(())
}
