use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Range};
use ynz_parser::token::{Spanned, Token};
use ynz_registry::lsp_hover_for_token;
use ynz_typeck::types::type_name;

use crate::{capabilities::PositionEncoding, position::LineTable};

/// Find the token at `byte_offset` in the token stream from a lex result.
/// Returns `(token_text, span_start, span_end)` or `None` if the offset
/// falls inside whitespace, a comment, or outside any token.
pub fn token_at_offset(
    tokens: &[Spanned<Token>],
    byte_offset: usize,
) -> Option<(String, usize, usize)> {
    // Binary search: find the last token whose start ≤ byte_offset
    let idx = tokens.partition_point(|t| t.span.start <= byte_offset);
    if idx == 0 {
        return None;
    }
    let tok = &tokens[idx - 1];
    // The offset must be inside [start, end)
    if byte_offset >= tok.span.start && byte_offset < tok.span.end {
        let text = token_text(&tok.value);
        if text.is_empty() {
            return None;
        }
        return Some((text, tok.span.start, tok.span.end));
    }
    None
}

/// Extract the user-visible text from a token.
fn token_text(tok: &Token) -> String {
    match tok {
        Token::Identifier(name) => name.clone(),
        Token::Function => "function".to_string(),
        Token::Nothing => "nothing".to_string(),
        Token::Let => "let".to_string(),
        Token::Const => "const".to_string(),
        Token::True => "true".to_string(),
        Token::False => "false".to_string(),
        Token::If => "if".to_string(),
        Token::Else => "else".to_string(),
        Token::While => "while".to_string(),
        Token::For => "for".to_string(),
        Token::In => "in".to_string(),
        Token::Return => "return".to_string(),
        Token::Shape => "shape".to_string(),
        Token::Follows => "follows".to_string(),
        Token::Extends => "extends".to_string(),
        Token::Base => "base".to_string(),
        Token::Hidden => "hidden".to_string(),
        Token::Import => "import".to_string(),
        Token::Export => "export".to_string(),
        Token::Options => "options".to_string(),
        Token::Dynamic => "dynamic".to_string(),
        Token::Wait => "wait".to_string(),
        Token::Background => "background".to_string(),
        Token::Errors => "errors".to_string(),
        Token::None => "none".to_string(),
        Token::Is => "is".to_string(),
        Token::Sensitive => "sensitive".to_string(),
        Token::SelfValue => "self".to_string(),
        Token::SelfType => "Self".to_string(),
        _ => String::new(), // operators, punctuation, literals — no hover text
    }
}

/// Build the LSP `Hover` response for the given cursor position.
///
/// Priority:
/// 1. Registry lookup by token name (covers keywords, intrinsics, deferred features, banned terms)
/// 2. Typeck signature lookup for user-defined functions
/// 3. `None` if neither resolves (e.g., cursor on punctuation, inside whitespace)
pub fn hover_response(
    tokens: &[Spanned<Token>],
    sig_table: &ynz_typeck::signatures::SignatureTable,
    text: &str,
    table: &LineTable,
    byte_offset: usize,
    encoding: PositionEncoding,
) -> Option<Hover> {
    let (token_name, span_start, span_end) = token_at_offset(tokens, byte_offset)?;

    let range = Some(Range {
        start: table.byte_offset_to_position(text, span_start, encoding),
        end: table.byte_offset_to_position(text, span_end, encoding),
    });

    // Registry lookup first (keywords, intrinsics, deferred features)
    if let Some(content) = lsp_hover_for_token(&token_name) {
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content.markdown_body,
            }),
            range,
        });
    }

    // Typeck fallback: user-defined function signature
    if let Some(sig) = sig_table.fns.get(&token_name) {
        let param_str = sig.params.iter()
            .map(|(pname, ptype)| format!("{pname}: {}", type_name(ptype)))
            .collect::<Vec<_>>()
            .join(", ");
        let body = format!(
            "## `function {}({})`\n\nReturns: `{}`",
            token_name,
            param_str,
            type_name(&sig.ret)
        );
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: body,
            }),
            range,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ynz_parser::lexer::lex;
    use std::collections::HashMap;
    use ynz_typeck::signatures::SignatureTable;

    fn tokenize(src: &str) -> Vec<Spanned<Token>> {
        let (tokens, _) = lex("test.ynz", src);
        tokens
    }

    #[test]
    fn token_at_offset_on_keyword() {
        let src = "function entrypoint";
        let tokens = tokenize(src);
        let result = token_at_offset(&tokens, 3); // inside "function"
        assert!(result.is_some(), "should find token at offset 3");
        let (text, _, _) = result.unwrap();
        assert_eq!(text, "function");
    }

    #[test]
    fn token_at_offset_on_identifier() {
        let src = "let myVar = 5";
        let tokens = tokenize(src);
        let result = token_at_offset(&tokens, 4); // 'myVar' starts at 4
        assert!(result.is_some(), "should find identifier token");
        let (text, _, _) = result.unwrap();
        assert_eq!(text, "myVar");
    }

    #[test]
    fn token_at_offset_in_whitespace_returns_none() {
        let src = "let   x = 5"; // 3 spaces between let and x
        let tokens = tokenize(src);
        // Offset 4 is inside the whitespace
        let result = token_at_offset(&tokens, 4);
        assert!(result.is_none(), "whitespace should return None");
    }

    #[test]
    fn token_at_byte_zero_in_empty_returns_none() {
        let tokens = tokenize("");
        assert!(token_at_offset(&tokens, 0).is_none());
    }

    fn make_sig() -> SignatureTable {
        SignatureTable { fns: HashMap::new() }
    }

    #[test]
    fn hover_keyword_returns_some() {
        let src = "function entrypoint() -> nothing { }";
        let tokens = tokenize(src);
        let table = LineTable::new(src);
        let result = hover_response(&tokens, &make_sig(), src, &table, 3, PositionEncoding::Utf8);
        assert!(result.is_some(), "hovering over 'function' keyword should return Some");
        if let Some(Hover { contents: HoverContents::Markup(mc), .. }) = result {
            assert!(mc.value.contains("function"), "hover body must mention the keyword");
        }
    }

    #[test]
    fn hover_in_whitespace_returns_none() {
        let src = "let   x = 5";
        let tokens = tokenize(src);
        let table = LineTable::new(src);
        let result = hover_response(&tokens, &make_sig(), src, &table, 4, PositionEncoding::Utf8);
        assert!(result.is_none(), "whitespace offset should return None");
    }

    #[test]
    fn hover_empty_file_returns_none() {
        let src = "";
        let tokens = tokenize(src);
        let table = LineTable::new(src);
        let result = hover_response(&tokens, &make_sig(), src, &table, 0, PositionEncoding::Utf8);
        assert!(result.is_none());
    }
}
