use lsp_types::{
    CompletionOptions, HoverProviderCapability, OneOf, PositionEncodingKind,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
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

pub fn server_capabilities(encoding: PositionEncoding) -> ServerCapabilities {
    let position_encoding = match encoding {
        PositionEncoding::Utf8 => PositionEncodingKind::UTF8,
        PositionEncoding::Utf16 => PositionEncodingKind::UTF16,
    };
    ServerCapabilities {
        position_encoding: Some(position_encoding),
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
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        ..Default::default()
    }
}

/// Negotiate position encoding from the client's advertised list.
/// Prefers UTF-8; falls back to UTF-16 (the LSP default).
pub fn negotiate_encoding(client_encodings: Option<&[PositionEncodingKind]>) -> PositionEncoding {
    if let Some(encodings) = client_encodings {
        if encodings.iter().any(|e| e.as_str() == "utf-8") {
            return PositionEncoding::Utf8;
        }
    }
    PositionEncoding::Utf16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiation_picks_utf8_when_offered() {
        let encodings = vec![PositionEncodingKind::UTF8, PositionEncodingKind::UTF16];
        assert_eq!(negotiate_encoding(Some(&encodings)), PositionEncoding::Utf8);
    }

    #[test]
    fn negotiation_falls_back_to_utf16_when_no_utf8() {
        let encodings = vec![PositionEncodingKind::UTF16];
        assert_eq!(
            negotiate_encoding(Some(&encodings)),
            PositionEncoding::Utf16
        );
    }

    #[test]
    fn negotiation_falls_back_to_utf16_when_none() {
        assert_eq!(negotiate_encoding(None), PositionEncoding::Utf16);
    }
}
