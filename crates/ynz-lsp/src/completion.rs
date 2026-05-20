use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemTag, CompletionList,
    Documentation, MarkupContent, MarkupKind, Position,
};
use ynz_registry::{CompletionContext, CompletionKind, RegistryCompletionItem};

use crate::{capabilities::PositionEncoding, position::LineTable};

/// Detect the completion context from the source text and cursor byte offset.
pub fn detect_context<'a>(text: &str, cursor_offset: usize) -> CompletionContext<'a> {
    if cursor_offset == 0 || text.is_empty() {
        return CompletionContext::BareIdentifier;
    }

    let before = &text[..cursor_offset.min(text.len())];

    // Walk backwards past the cursor to find the character just before it
    let bytes = before.as_bytes();
    let last_char = bytes.iter().rev().find(|&&b| !b.is_ascii_whitespace()).copied();

    if last_char == Some(b'.') {
        // After-dot context: walk left of the '.' to find the receiver token
        // Numeric literal disambiguation: if the previous non-whitespace byte before
        // the '.' is an ASCII digit AND the char before THAT digit is NOT an
        // identifier-continue char, this is a numeric literal — return no context.
        let pos_of_dot = before.trim_end_matches(|c: char| c.is_ascii_whitespace()).len();
        if pos_of_dot == 0 {
            return CompletionContext::BareIdentifier;
        }
        let before_dot = &before[..pos_of_dot - 1]; // exclude the '.'
        let prev_non_ws = before_dot.trim_end_matches(|c: char| c.is_ascii_whitespace());

        if prev_non_ws.is_empty() {
            return CompletionContext::BareIdentifier;
        }

        // Check if this looks like a numeric literal (e.g. `5.`)
        let last_byte = prev_non_ws.as_bytes().last().copied().unwrap_or(0);
        if last_byte.is_ascii_digit() {
            let second_last = prev_non_ws.as_bytes().get(prev_non_ws.len().wrapping_sub(2)).copied();
            let is_identifier_continue = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
            let preceded_by_ident = second_last.map(is_identifier_continue).unwrap_or(false);
            if !preceded_by_ident {
                // e.g. `5.` — numeric literal decimal point, not a method call
                return CompletionContext::BareIdentifier;
            }
        }

        // Thin slice: receiver type narrowing via typeck is deferred — we return all
        // methods as best-effort candidates. Typeck integration (module_signatures_query)
        // is deferred because it requires per-offset AST lookup not yet exposed.
        CompletionContext::AfterDot { receiver_type: None }
    } else {
        CompletionContext::BareIdentifier
    }
}

/// Convert a `RegistryCompletionItem` to an LSP `CompletionItem`.
pub fn to_lsp_completion_item(rci: RegistryCompletionItem) -> CompletionItem {
    let kind = match rci.kind {
        CompletionKind::Keyword => CompletionItemKind::KEYWORD,
        CompletionKind::PrimitiveMethod => CompletionItemKind::METHOD,
        CompletionKind::FreeFn => CompletionItemKind::FUNCTION,
        CompletionKind::TypeAttachedConstant => CompletionItemKind::CONSTANT,
        CompletionKind::DeferredFeature => CompletionItemKind::KEYWORD,
        CompletionKind::BannedKeyword => CompletionItemKind::KEYWORD,
    };

    let tags = if rci.deprecated {
        Some(vec![CompletionItemTag::DEPRECATED])
    } else {
        None
    };

    let documentation = rci.documentation.map(|d| {
        Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: d,
        })
    });

    // sort_text pads priority to 3 digits so lexicographic sort == numeric sort
    let sort_text = Some(format!("{:04}_{}", rci.sort_priority, rci.label));

    CompletionItem {
        label: rci.label,
        kind: Some(kind),
        detail: rci.detail,
        documentation,
        deprecated: Some(rci.deprecated),
        tags,
        sort_text,
        ..Default::default()
    }
}

/// Build the LSP `CompletionList` for the given cursor position in `text`.
pub fn completion_list(
    text: &str,
    table: &LineTable,
    position: Position,
    encoding: PositionEncoding,
) -> Option<CompletionList> {
    let cursor_offset = table.position_to_byte_offset(text, position, encoding)?;
    let context = detect_context(text, cursor_offset);
    let registry_items = ynz_registry::lsp_completion_items(&context);
    let items: Vec<CompletionItem> = registry_items.into_iter().map(to_lsp_completion_item).collect();

    Some(CompletionList { is_incomplete: false, items })
}
