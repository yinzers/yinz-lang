use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemTag, CompletionList,
    Documentation, MarkupContent, MarkupKind, Position,
};
use ynz_registry::{CompletionContext, CompletionKind, RegistryCompletionItem};
use ynz_typeck::signatures::SignatureTable;
use ynz_typeck::shapes::ShapeTable;
use ynz_typeck::types::type_name;

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

        // Receiver type narrowing via typeck is deferred: requires
        // `type_of_expression_at_offset` in ynz-typeck (not yet exposed).
        // Until then, all primitive methods appear as best-effort candidates.
        // Tracked: .claude/todos.md "lsp-completion-typeck-receiver-narrowing".
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

/// Build user-defined function + shape completion items from typeck output.
/// These are inserted at sort_priority 0 (before all registry items).
pub fn user_symbol_items(sig_table: &SignatureTable, shape_table: &ShapeTable) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = Vec::new();

    for (name, sig) in &sig_table.fns {
        let param_str = sig.params.iter()
            .map(|(pname, ptype)| format!("{pname}: {}", type_name(ptype)))
            .collect::<Vec<_>>()
            .join(", ");
        let detail = format!("function {name}({param_str}) -> {}", type_name(&sig.ret));
        items.push(CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(detail),
            sort_text: Some(format!("{:04}_{name}", 0u16)),
            ..Default::default()
        });
    }

    for name in shape_table.shapes.keys() {
        items.push(CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some(format!("shape {name}")),
            sort_text: Some(format!("{:04}_{name}", 10u16)),
            ..Default::default()
        });
    }

    items
}

/// Build the LSP `CompletionList` for the given cursor position in `text`.
/// Merges user-defined symbols (from typeck) before registry items.
pub fn completion_list(
    text: &str,
    table: &LineTable,
    position: Position,
    encoding: PositionEncoding,
    sig_table: Option<&SignatureTable>,
    shape_table: Option<&ShapeTable>,
) -> Option<CompletionList> {
    let cursor_offset = table.position_to_byte_offset(text, position, encoding)?;
    let context = detect_context(text, cursor_offset);

    let mut items: Vec<CompletionItem> = Vec::new();

    // User-defined symbols come first (sort_priority 0-10) for BareIdentifier context
    if matches!(context, CompletionContext::BareIdentifier) {
        if let (Some(sig), Some(shapes)) = (sig_table, shape_table) {
            items.extend(user_symbol_items(sig, shapes));
        }
    }

    // Registry items (keywords, intrinsics, deferred features, etc.)
    let registry_items = ynz_registry::lsp_completion_items(&context);
    items.extend(registry_items.into_iter().map(to_lsp_completion_item));

    Some(CompletionList { is_incomplete: false, items })
}
