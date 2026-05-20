// tower-lsp spike: minimal "hello LSP" measuring plumbing overhead + &mut DB ergonomics.
//
// Key measurement target: salsa CompilerDb requires &mut for input mutations (didChange).
// tower-lsp gives handlers &self (not &mut self) — requires Arc<Mutex<Db>> or channel.
// This spike uses a DbSimulation struct to expose the ownership friction concretely.

use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

// Simulates CompilerDb: update() needs &mut self (like salsa input mutations),
// query() needs &self (like salsa tracked queries). This is the exact ownership
// pattern that tower-lsp's async handlers must navigate.
struct DbSimulation {
    text: String,
}

impl DbSimulation {
    fn update(&mut self, new_text: String) {
        self.text = new_text;
    }

    fn query(&self) -> String {
        format!("ynz-lsp spike: hello from tower-lsp (doc len={})", self.text.len())
    }
}

struct Backend {
    client: Client,
    // Arc<Mutex<...>> required because tower-lsp's LanguageServer trait gives &self
    // to handlers that run in separate tokio tasks. Without this, &mut db access
    // across concurrent handlers is unsound. For single-threaded dispatch we'd use
    // a tokio::sync::Mutex; for true async multi-handler we'd need snapshots.
    db: Arc<Mutex<DbSimulation>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {}

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        // Acquire the Mutex lock to mutate state — this is the &mut friction point.
        // In real ynz-lsp this lock is held while running salsa queries (which can
        // be expensive), meaning all concurrent requests block while one query runs.
        let mut db = self.db.lock().await;
        db.update(params.text_document.text);
        let message = db.query();
        drop(db);

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
        self.client
            .publish_diagnostics(params.text_document.uri, vec![diagnostic], None)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            let mut db = self.db.lock().await;
            db.update(change.text);
        }
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        db: Arc::new(Mutex::new(DbSimulation { text: String::new() })),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}
