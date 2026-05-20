use lsp_types::{
    CompletionOptions, HoverProviderCapability, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions,
};

/// The position encoding this server prefers. UTF-8 means LSP Position.character == byte offset,
/// which matches SourceSpan exactly — no conversion needed when the client supports it.
pub const PREFERRED_ENCODING: &str = "utf-8";
pub const FALLBACK_ENCODING: &str = "utf-16";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionEncoding {
    Utf8,
    Utf16,
}

pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                ..Default::default()
            },
        )),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![".".to_string(), " ".to_string()]),
            resolve_provider: Some(false),
            ..Default::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        ..Default::default()
    }
}

/// Negotiate position encoding from the client's advertised list.
/// Prefers UTF-8; falls back to UTF-16 (the LSP default).
pub fn negotiate_encoding(client_encodings: Option<&[String]>) -> PositionEncoding {
    if let Some(encodings) = client_encodings {
        if encodings.iter().any(|e| e == "utf-8") {
            return PositionEncoding::Utf8;
        }
    }
    PositionEncoding::Utf16
}
