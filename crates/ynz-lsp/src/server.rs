use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Initialized,
        Notification as _,
    },
    request::{Initialize, Request as _, Shutdown},
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, ServerInfo,
};

use crate::{
    capabilities::{negotiate_encoding, server_capabilities, FALLBACK_ENCODING, PREFERRED_ENCODING},
    state::ServerState,
};

/// Entry point for stdio JSON-RPC mode. Runs until `exit` is received.
pub fn run_stdio() {
    let (connection, io_threads) = Connection::stdio();
    serve(connection);
    io_threads.join().expect("LSP IO threads panicked");
}

/// Drive the request loop over `connection`. Separated from `run_stdio` so tests
/// can inject an in-process `Connection`.
pub fn serve(connection: Connection) {
    let encoding = handshake(&connection);
    let mut state = ServerState::new(encoding);
    main_loop(&connection, &mut state);
}

/// Perform the LSP initialize handshake and return the negotiated position encoding.
fn handshake(connection: &Connection) -> crate::capabilities::PositionEncoding {
    let (id, params_value) = connection.initialize_start().expect("initialize_start failed");
    let params: InitializeParams = serde_json::from_value(params_value).unwrap_or_default();

    let client_encodings: Option<Vec<String>> = params
        .capabilities
        .general
        .as_ref()
        .and_then(|g| g.position_encodings.as_ref())
        .map(|encs| encs.iter().map(|e| format!("{:?}", e)).collect());

    let encoding = negotiate_encoding(client_encodings.as_deref());
    let chosen_str = match encoding {
        crate::capabilities::PositionEncoding::Utf8 => PREFERRED_ENCODING,
        crate::capabilities::PositionEncoding::Utf16 => FALLBACK_ENCODING,
    };

    let result = InitializeResult {
        capabilities: server_capabilities(),
        server_info: Some(ServerInfo {
            name: "ynz-lsp".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    };

    let mut result_value = serde_json::to_value(result).expect("serialize InitializeResult");
    // Inject positionEncoding into the result (lsp-types doesn't expose this field directly)
    if let Some(obj) = result_value.as_object_mut() {
        obj.insert("positionEncoding".to_string(), serde_json::json!(chosen_str));
    }

    connection.initialize_finish(id, result_value).expect("initialize_finish failed");
    encoding
}

fn main_loop(connection: &Connection, state: &mut ServerState) {
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => handle_request(connection, state, req),
            Message::Notification(notif) => handle_notification(connection, state, notif),
            Message::Response(_) => {}
        }
        if state.shutdown_requested {
            break;
        }
    }
}

fn handle_request(connection: &Connection, state: &mut ServerState, req: Request) {
    if req.method == Shutdown::METHOD {
        state.shutdown_requested = true;
        let response = Response::new_ok(req.id, serde_json::Value::Null);
        connection.sender.send(Message::Response(response)).ok();
        return;
    }
    if req.method == Initialize::METHOD {
        // Re-initialize not supported; send an error.
        let response = Response::new_err(
            req.id,
            lsp_server::ErrorCode::InvalidRequest as i32,
            "Server already initialized".to_string(),
        );
        connection.sender.send(Message::Response(response)).ok();
        return;
    }
    // Unhandled request — send method-not-found error.
    let response = Response::new_err(
        req.id,
        lsp_server::ErrorCode::MethodNotFound as i32,
        format!("method not implemented: {}", req.method),
    );
    connection.sender.send(Message::Response(response)).ok();
}

fn handle_notification(
    connection: &Connection,
    state: &mut ServerState,
    notif: Notification,
) {
    match notif.method.as_str() {
        Initialized::METHOD => {}

        DidOpenTextDocument::METHOD => {
            if let Ok(params) =
                serde_json::from_value::<DidOpenTextDocumentParams>(notif.params)
            {
                let uri = params.text_document.uri;
                let text = params.text_document.text;
                state.open_document(uri.clone(), text);
                // Warm salsa cache; diagnostics published in Phase 3.
                if let Some(sf) = state.source_file_for(&uri) {
                    let _ = ynz_typeck::queries::module_signatures_query(&state.db, sf);
                }
                let _ = connection; // will be used in Phase 3 to publish diagnostics
            }
        }

        DidChangeTextDocument::METHOD => {
            if let Ok(params) =
                serde_json::from_value::<DidChangeTextDocumentParams>(notif.params)
            {
                let uri = params.text_document.uri;
                if let Some(change) = params.content_changes.into_iter().last() {
                    state.update_document(&uri, change.text);
                }
            }
        }

        DidCloseTextDocument::METHOD => {
            if let Ok(params) =
                serde_json::from_value::<DidCloseTextDocumentParams>(notif.params)
            {
                state.close_document(&params.text_document.uri);
            }
        }

        _ => {} // unknown notification — ignore per LSP spec
    }
}

/// Send a `textDocument/publishDiagnostics` notification.
/// Used by Phase 3+; defined here so the connection reference flows cleanly.
pub fn publish_diagnostics(
    connection: &Connection,
    uri: lsp_types::Url,
    diagnostics: Vec<lsp_types::Diagnostic>,
    version: Option<i32>,
) {
    let params = lsp_types::PublishDiagnosticsParams { uri, diagnostics, version };
    let notif = Notification::new(
        "textDocument/publishDiagnostics".to_string(),
        params,
    );
    connection.sender.send(Message::Notification(notif)).ok();
}
