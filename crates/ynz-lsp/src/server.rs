use std::collections::HashMap;

use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidRenameFiles,
        Initialized, Notification as _,
    },
    request::{
        CodeActionRequest, Completion, DocumentLinkRequest, DocumentSymbolRequest,
        FoldingRangeRequest, Formatting, GotoDefinition, HoverRequest, Initialize,
        InlayHintRequest, PrepareRenameRequest, RangeFormatting, References, Rename, Request as _,
        SemanticTokensFullRequest, SemanticTokensRangeRequest, Shutdown, WorkspaceSymbolRequest,
    },
    CodeActionParams, CompletionParams, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentFormattingParams, DocumentLinkParams,
    DocumentRangeFormattingParams, DocumentSymbolParams, DocumentSymbolResponse,
    FoldingRangeParams, GotoDefinitionParams, HoverParams, InitializeParams, InitializeResult,
    InlayHintParams, PrepareRenameResponse, ReferenceParams, RenameFilesParams, RenameParams,
    SemanticTokensParams, SemanticTokensRangeParams, ServerInfo, Url, WorkspaceSymbolParams,
};

use crate::{
    capabilities::{negotiate_encoding, server_capabilities},
    code_action::code_action_response,
    completion::{
        completion_list, cross_file_completion_items, detect_context, receiver_end_offset,
    },
    diagnostic_transform::{path_to_uri, to_lsp_diagnostic},
    document_link::document_link_response,
    document_symbol::{document_symbol_response, workspace_symbol_response},
    folding_range::folding_range_response,
    formatting::{formatting_response, range_formatting_response},
    goto_definition::definition_response,
    hover::hover_response,
    inlay_hint::inlay_hint_response,
    position::LineTable,
    references::references_response,
    rename::{prepare_rename_response, rename_response},
    rename_file::did_rename_files,
    semantic_tokens::{semantic_tokens_full_response, semantic_tokens_range_response},
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
    let (encoding, project_root) = handshake(&connection);
    let mut state = ServerState::new(encoding);
    if let Some(root) = project_root {
        state.project_root = Some(root.clone());
        state.index_project_files_pub(&root);
    }
    main_loop(&connection, &mut state);
}

/// Perform the LSP initialize handshake and return the negotiated position encoding
/// plus any project root extracted from the client's workspace params.
fn handshake(
    connection: &Connection,
) -> (
    crate::capabilities::PositionEncoding,
    Option<std::path::PathBuf>,
) {
    // I/O-init: framework guarantees deserialization; panic surfaces unrecoverable stdio breakage
    let (id, params_value) = connection
        .initialize_start()
        .expect("initialize_start failed");

    let params: InitializeParams = match serde_json::from_value(params_value) {
        Ok(p) => p,
        Err(e) => {
            // Malformed initialize params — log and use defaults (safe; we can still serve)
            eprintln!("ynz-lsp: malformed InitializeParams, using defaults: {e}");
            InitializeParams::default()
        }
    };

    let client_encodings = params
        .capabilities
        .general
        .as_ref()
        .and_then(|g| g.position_encodings.as_deref());

    let encoding = negotiate_encoding(client_encodings);

    // Extract project root from Initialize params (rootUri preferred, rootPath fallback).
    // Both fields are deprecated in newer LSP in favor of workspaceFolders, but editors
    // still send them on Initialize, so we intentionally read both for compatibility.
    #[allow(deprecated)]
    let project_root = params
        .root_uri
        .as_ref()
        .and_then(|uri| crate::state::find_project_root(&uri.to_file_path().ok()?))
        .or_else(|| {
            params
                .root_path
                .as_deref()
                .and_then(|p| crate::state::find_project_root(std::path::Path::new(p)))
        });

    let result = InitializeResult {
        // positionEncoding lives inside ServerCapabilities per LSP 3.17 spec §3.15
        capabilities: server_capabilities(encoding),
        server_info: Some(ServerInfo {
            name: "ynz-lsp".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    };

    // I/O-init: framework guarantees serialization; panic surfaces unrecoverable stdio breakage
    let result_value = serde_json::to_value(result).expect("serialize InitializeResult");
    connection
        .initialize_finish(id, result_value)
        .expect("initialize_finish failed");
    (encoding, project_root)
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
    if req.method == HoverRequest::METHOD {
        let params: HoverParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid hover params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let lex_output = state
            .source_file_for(uri)
            .map(|sf| ynz_parser::queries::lex_query(&state.db, sf));
        let sig_output = state
            .source_file_for(uri)
            .map(|sf| ynz_typeck::queries::module_signatures_query(&state.db, sf));
        let parse_output = state
            .source_file_for(uri)
            .map(|sf| ynz_parser::queries::parse_query(&state.db, sf));

        let empty_sig = ynz_typeck::signatures::SignatureTable {
            fns: std::collections::HashMap::new(),
        };
        let hover = lex_output.as_ref().and_then(|lex| {
            let text = state.text_for(uri)?;
            let table = state.line_table_for(uri)?;
            let byte_offset = table.position_to_byte_offset(text, position, state.encoding)?;
            let sig = sig_output
                .as_ref()
                .map(|o| &o.sig_table)
                .unwrap_or(&empty_sig);
            let shape_table = sig_output.as_ref().map(|o| &o.shape_table);
            let imported_options = sig_output.as_ref().map(|o| &o.imported_options);
            let module = parse_output.as_ref().map(|p| &p.module);
            hover_response(
                &lex.tokens,
                sig,
                module,
                text,
                table,
                byte_offset,
                state.encoding,
                shape_table,
                imported_options,
            )
        });

        let result = match hover {
            Some(h) => serde_json::to_value(h).unwrap_or(serde_json::Value::Null),
            None => serde_json::Value::Null,
        };
        let response = Response::new_ok(req.id, result);
        connection.sender.send(Message::Response(response)).ok();
        return;
    }

    if req.method == Completion::METHOD {
        let params: CompletionParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid completion params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let sig_output = state
            .source_file_for(uri)
            .map(|sf| ynz_typeck::queries::module_signatures_query(&state.db, sf));
        let completion_sf = state.source_file_for(uri);

        let list = state.text_for(uri).and_then(|text| {
            let table = state.line_table_for(uri)?;
            let sig_table = sig_output.as_ref().map(|o| &o.sig_table);
            let shape_table = sig_output.as_ref().map(|o| &o.shape_table);

            // Resolve receiver type for after-dot completion narrowing.
            let cursor_offset = table.position_to_byte_offset(text, position, state.encoding)?;
            let receiver_type: Option<String> = completion_sf.and_then(|sf| {
                let recv_offset = receiver_end_offset(text, cursor_offset)?;
                let ty = ynz_typeck::type_of_expression_at_offset(&state.db, sf, recv_offset)?;
                Some(ynz_typeck::types::type_name(&ty))
            });

            let mut list = completion_list(
                text,
                table,
                position,
                state.encoding,
                sig_table,
                shape_table,
                receiver_type.as_deref(),
            )?;

            // In BareIdentifier context only: extend with cross-file exportable symbols.
            // Each item carries additionalTextEdits that inserts the import on selection.
            let ctx = detect_context(text, cursor_offset);
            if matches!(ctx, ynz_registry::CompletionContext::BareIdentifier) {
                let cross =
                    cross_file_completion_items(state, uri, text, table, sig_table, shape_table);
                list.items.extend(cross);
            }

            Some(list)
        });

        let result = match list {
            Some(l) => serde_json::to_value(l).unwrap_or(serde_json::Value::Null),
            None => serde_json::Value::Null,
        };
        let response = Response::new_ok(req.id, result);
        connection.sender.send(Message::Response(response)).ok();
        return;
    }

    if req.method == GotoDefinition::METHOD {
        let params: GotoDefinitionParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid definition params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let result = definition_response(state, uri, position);
        let value = match result {
            Some(r) => serde_json::to_value(r).unwrap_or(serde_json::Value::Null),
            None => serde_json::Value::Null,
        };
        let response = Response::new_ok(req.id, value);
        connection.sender.send(Message::Response(response)).ok();
        return;
    }

    if req.method == References::METHOD {
        let params: ReferenceParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid references params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let include_decl = params.context.include_declaration;
        let result = references_response(state, uri, position, include_decl, &connection.sender);
        let value = match result {
            Some(locs) => serde_json::to_value(locs).unwrap_or(serde_json::Value::Null),
            // None means lookup failed (unknown URI or invalid position).
            // Some(vec![]) = valid position with zero references — serializes to [].
            None => serde_json::Value::Null,
        };
        let response = Response::new_ok(req.id, value);
        connection.sender.send(Message::Response(response)).ok();
        return;
    }

    if req.method == Formatting::METHOD {
        let params: DocumentFormattingParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid formatting params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let edits = formatting_response(state, &params.text_document.uri, &connection.sender);
        let value = serde_json::to_value(edits).unwrap_or(serde_json::Value::Null);
        connection
            .sender
            .send(Message::Response(Response::new_ok(req.id, value)))
            .ok();
        return;
    }

    if req.method == RangeFormatting::METHOD {
        let params: DocumentRangeFormattingParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid rangeFormatting params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let edits = range_formatting_response(
            state,
            &params.text_document.uri,
            params.range,
            &connection.sender,
        );
        let value = serde_json::to_value(edits).unwrap_or(serde_json::Value::Null);
        connection
            .sender
            .send(Message::Response(Response::new_ok(req.id, value)))
            .ok();
        return;
    }

    if req.method == InlayHintRequest::METHOD {
        let params: InlayHintParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid inlayHint params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document.uri;
        let hints = inlay_hint_response(state, uri, params.range);
        let value = serde_json::to_value(hints).unwrap_or(serde_json::Value::Null);
        connection
            .sender
            .send(Message::Response(Response::new_ok(req.id, value)))
            .ok();
        return;
    }

    if req.method == PrepareRenameRequest::METHOD {
        let params: lsp_types::TextDocumentPositionParams = match serde_json::from_value(req.params)
        {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid prepareRename params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document.uri;
        let position = params.position;
        let result =
            prepare_rename_response(state, uri, position).map(PrepareRenameResponse::Range);
        let value = match result {
            Some(r) => serde_json::to_value(r).unwrap_or(serde_json::Value::Null),
            None => serde_json::Value::Null,
        };
        let response = Response::new_ok(req.id, value);
        connection.sender.send(Message::Response(response)).ok();
        return;
    }

    if req.method == Rename::METHOD {
        let params: RenameParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid rename params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = &params.new_name;
        match rename_response(state, uri, position, new_name, &connection.sender) {
            Ok(edit) => {
                let value = serde_json::to_value(edit).unwrap_or(serde_json::Value::Null);
                connection
                    .sender
                    .send(Message::Response(Response::new_ok(req.id, value)))
                    .ok();
            }
            Err((code, message)) => {
                let response = Response::new_err(req.id, code, message);
                connection.sender.send(Message::Response(response)).ok();
            }
        }
        return;
    }

    if req.method == SemanticTokensFullRequest::METHOD {
        let params: SemanticTokensParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid semanticTokens/full params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document.uri;
        let result = semantic_tokens_full_response(state, uri);
        let value = match result {
            Some(r) => serde_json::to_value(r).unwrap_or(serde_json::Value::Null),
            None => serde_json::Value::Null,
        };
        connection
            .sender
            .send(Message::Response(Response::new_ok(req.id, value)))
            .ok();
        return;
    }

    if req.method == SemanticTokensRangeRequest::METHOD {
        let params: SemanticTokensRangeParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid semanticTokens/range params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document.uri;
        let result = semantic_tokens_range_response(state, uri, params.range);
        let value = match result {
            Some(r) => serde_json::to_value(r).unwrap_or(serde_json::Value::Null),
            None => serde_json::Value::Null,
        };
        connection
            .sender
            .send(Message::Response(Response::new_ok(req.id, value)))
            .ok();
        return;
    }

    if req.method == CodeActionRequest::METHOD {
        let params: CodeActionParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid codeAction params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document.uri;
        let actions = code_action_response(state, uri, params.range);
        let value = serde_json::to_value(actions).unwrap_or(serde_json::Value::Null);
        connection
            .sender
            .send(Message::Response(Response::new_ok(req.id, value)))
            .ok();
        return;
    }

    if req.method == DocumentLinkRequest::METHOD {
        let params: DocumentLinkParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid documentLink params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document.uri;
        let links = document_link_response(state, uri);
        let value = serde_json::to_value(links).unwrap_or(serde_json::Value::Null);
        connection
            .sender
            .send(Message::Response(Response::new_ok(req.id, value)))
            .ok();
        return;
    }

    if req.method == FoldingRangeRequest::METHOD {
        let params: FoldingRangeParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid foldingRange params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document.uri;
        let ranges = folding_range_response(state, uri);
        let value = serde_json::to_value(ranges).unwrap_or(serde_json::Value::Null);
        connection
            .sender
            .send(Message::Response(Response::new_ok(req.id, value)))
            .ok();
        return;
    }

    if req.method == DocumentSymbolRequest::METHOD {
        let params: DocumentSymbolParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid documentSymbol params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let uri = &params.text_document.uri;
        let symbols = document_symbol_response(state, uri);
        let value = if symbols.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::to_value(DocumentSymbolResponse::Nested(symbols))
                .unwrap_or(serde_json::Value::Null)
        };
        connection
            .sender
            .send(Message::Response(Response::new_ok(req.id, value)))
            .ok();
        return;
    }

    if req.method == WorkspaceSymbolRequest::METHOD {
        let params: WorkspaceSymbolParams = match serde_json::from_value(req.params) {
            Ok(p) => p,
            Err(e) => {
                let response = Response::new_err(
                    req.id,
                    lsp_server::ErrorCode::InvalidParams as i32,
                    format!("invalid workspace/symbol params: {e}"),
                );
                connection.sender.send(Message::Response(response)).ok();
                return;
            }
        };
        let symbols = workspace_symbol_response(state, &params.query);
        let value = serde_json::to_value(symbols).unwrap_or(serde_json::Value::Null);
        connection
            .sender
            .send(Message::Response(Response::new_ok(req.id, value)))
            .ok();
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

fn handle_notification(connection: &Connection, state: &mut ServerState, notif: Notification) {
    match notif.method.as_str() {
        Initialized::METHOD => {}

        DidOpenTextDocument::METHOD => {
            if let Ok(params) = serde_json::from_value::<DidOpenTextDocumentParams>(notif.params) {
                let uri = params.text_document.uri;
                let text = params.text_document.text;
                // Infer project root lazily on first open if Initialize didn't provide one.
                state.set_project_root_from_uri(&uri);
                state.open_document(uri.clone(), text);
                run_and_publish_diagnostics(connection, state, &uri, None);
            }
        }

        DidChangeTextDocument::METHOD => {
            if let Ok(params) = serde_json::from_value::<DidChangeTextDocumentParams>(notif.params)
            {
                let uri = params.text_document.uri;
                let version = params.text_document.version;
                if let Some(change) = params.content_changes.into_iter().last() {
                    state.update_document(&uri, change.text);
                }
                run_and_publish_diagnostics(connection, state, &uri, Some(version));
            }
        }

        DidCloseTextDocument::METHOD => {
            if let Ok(params) = serde_json::from_value::<DidCloseTextDocumentParams>(notif.params) {
                let uri = params.text_document.uri;
                // Clear squiggles for the closed document.
                publish_diagnostics(connection, uri.clone(), vec![], None);
                state.last_published.remove(&uri);
                state.close_document(&uri);
            }
        }

        DidRenameFiles::METHOD => {
            if let Ok(params) = serde_json::from_value::<RenameFilesParams>(notif.params) {
                did_rename_files(state, &params);
            }
        }

        _ => {} // unknown notification — ignore per LSP spec
    }
}

/// Run `check_query` for the given URI and publish diagnostics to the client.
///
/// Diagnostics are routed per `span.file` — a cross-file diagnostic publishes to
/// its origin file's URI, not necessarily the editing file's URI. This matches the
/// LSP expectation: squiggles appear in the file where the error physically lives.
///
/// Stale squiggles are cleared for any URI that had diagnostics in the previous round
/// but has none in the current round.
fn run_and_publish_diagnostics(
    connection: &Connection,
    state: &mut ServerState,
    uri: &Url,
    version: Option<i32>,
) {
    let Some(sf) = state.source_file_for(uri) else {
        return;
    };

    let check_output = ynz_typeck::queries::check_query(&state.db, sf);
    // The `array-using-soa-layout` Tier 3 lint lives OUTSIDE check_query (its inputs
    // depend on check_query — salsa cycle), so the LSP merges it here to reach the
    // editor squiggle/hover surface. Empty on type errors and under the analysis gates.
    let soa_lints = ynz_typeck::queries::soa_layout_lints(&state.db, sf);
    let encoding = state.encoding;

    // Group diagnostics by their span.file (the file where the error physically lives).
    let mut by_file: HashMap<String, Vec<lsp_types::Diagnostic>> = HashMap::new();
    for d in check_output.diagnostics.iter().chain(soa_lints.iter()) {
        let file_uri = path_to_uri(&d.span.file);
        let file_uri = file_uri.as_ref().unwrap_or(uri); // fall back to editing file

        let file_text = state.open_documents.get(file_uri).map(String::as_str);
        let text = file_text.unwrap_or_default();
        let lsp_d = if let Some(table) = state.line_tables.get(file_uri) {
            to_lsp_diagnostic(d, text, table, encoding)
        } else {
            let tmp_table = LineTable::new(text);
            to_lsp_diagnostic(d, text, &tmp_table, encoding)
        };
        by_file.entry(d.span.file.clone()).or_default().push(lsp_d);
    }

    // Build the this-round publication map (URI → diagnostics).
    let mut this_round: HashMap<Url, Vec<lsp_types::Diagnostic>> = HashMap::new();
    for (file_path, diags) in by_file {
        let file_uri = path_to_uri(&file_path).unwrap_or_else(|| uri.clone());
        this_round.entry(file_uri).or_default().extend(diags);
    }

    // The editing file always gets a publish (possibly empty) so clients
    // clear stale squiggles when errors are fixed.
    this_round.entry(uri.clone()).or_default();

    // Publish to each URI affected this round.
    for (file_uri, diags) in &this_round {
        let ver = if file_uri == uri { version } else { None };
        publish_diagnostics(connection, file_uri.clone(), diags.clone(), ver);
    }

    // Clear stale URIs from the previous round that have no diagnostics now.
    let prev_uris: Vec<Url> = state.last_published.keys().cloned().collect();
    for prev_uri in prev_uris {
        if !this_round.contains_key(&prev_uri) {
            publish_diagnostics(connection, prev_uri, vec![], None);
        }
    }

    state.last_published = this_round;
}

/// Send a `textDocument/publishDiagnostics` notification.
pub fn publish_diagnostics(
    connection: &Connection,
    uri: lsp_types::Url,
    diagnostics: Vec<lsp_types::Diagnostic>,
    version: Option<i32>,
) {
    let params = lsp_types::PublishDiagnosticsParams {
        uri,
        diagnostics,
        version,
    };
    let notif = Notification::new("textDocument/publishDiagnostics".to_string(), params);
    connection.sender.send(Message::Notification(notif)).ok();
}
