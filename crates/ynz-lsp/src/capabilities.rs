use crate::semantic_tokens::semantic_tokens_options;
use lsp_types::{
    CodeActionOptions, CodeActionProviderCapability, CompletionOptions, DocumentLinkOptions,
    FileOperationFilter, FileOperationPattern, FileOperationPatternKind,
    FileOperationRegistrationOptions, FoldingRangeProviderCapability, HoverProviderCapability,
    InlayHintOptions, InlayHintServerCapabilities, OneOf, PositionEncodingKind, RenameOptions,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    WorkDoneProgressOptions, WorkspaceFileOperationsServerCapabilities,
    WorkspaceServerCapabilities,
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
            trigger_characters: Some(vec![".".to_string()]),
            resolve_provider: Some(false),
            ..Default::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_range_formatting_provider: Some(OneOf::Left(true)),
        inlay_hint_provider: Some(OneOf::Right(InlayHintServerCapabilities::Options(
            InlayHintOptions {
                work_done_progress_options: WorkDoneProgressOptions {
                    work_done_progress: None,
                },
                resolve_provider: Some(false),
            },
        ))),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: WorkDoneProgressOptions {
                work_done_progress: None,
            },
        })),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![
                lsp_types::CodeActionKind::QUICKFIX,
                lsp_types::CodeActionKind::REFACTOR_REWRITE,
            ]),
            work_done_progress_options: WorkDoneProgressOptions {
                work_done_progress: None,
            },
            resolve_provider: Some(false),
        })),
        semantic_tokens_provider: Some(semantic_tokens_options()),
        document_link_provider: Some(DocumentLinkOptions {
            resolve_provider: Some(false),
            work_done_progress_options: WorkDoneProgressOptions::default(),
        }),
        folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        workspace: Some(WorkspaceServerCapabilities {
            file_operations: Some(WorkspaceFileOperationsServerCapabilities {
                did_rename: Some(ynz_file_op_filter()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn ynz_file_op_filter() -> FileOperationRegistrationOptions {
    FileOperationRegistrationOptions {
        filters: vec![FileOperationFilter {
            scheme: Some("file".to_string()),
            pattern: FileOperationPattern {
                glob: "**/*.ynz".to_string(),
                matches: Some(FileOperationPatternKind::File),
                options: None,
            },
        }],
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

    macro_rules! negotiation_case {
        ($name:ident, $offered:expr, $expected:expr) => {
            // bug: a client offering no UTF-8 gets UTF-8 positions (or the reverse), so every range lands on the wrong column | tier: Medium
            #[test]
            fn $name() {
                let offered: Option<Vec<PositionEncodingKind>> = $offered;
                assert_eq!(negotiate_encoding(offered.as_deref()), $expected);
            }
        };
    }

    negotiation_case!(
        utf8_is_picked_when_offered,
        Some(vec![
            PositionEncodingKind::UTF8,
            PositionEncodingKind::UTF16
        ]),
        PositionEncoding::Utf8
    );
    negotiation_case!(
        utf16_is_the_fallback_when_utf8_is_not_offered,
        Some(vec![PositionEncodingKind::UTF16]),
        PositionEncoding::Utf16
    );
    negotiation_case!(
        utf16_is_the_default_when_the_client_offers_nothing,
        None,
        PositionEncoding::Utf16
    );
}
