use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Range};
use ynz_ast::nodes::{Item, Module};
use ynz_parser::token::{Spanned, Token};
use ynz_registry::lsp_hover_for_token;
use ynz_typeck::ast_offset::identifier_use_site_at_offset;
use ynz_typeck::type_at_offset::type_of_name_at_offset;
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
    // Accept [start, end] so editors that place the cursor at the byte
    // immediately after the last character (tok.span.end) still resolve the
    // token. Binary-search guarantees idx-1 is the rightmost candidate whose
    // start ≤ offset, so adjacent tokens never double-match: if offset ==
    // span.end and a next token starts at the same byte, partition_point
    // would have already selected that next token as idx-1 instead.
    if byte_offset >= tok.span.start && byte_offset <= tok.span.end {
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

/// Look up the `///` doc comment attached to a top-level declaration by name.
///
/// Returns the doc string from the first matching declaration, or `None` if the
/// name resolves to a keyword, primitive, or a declaration without doc comments.
fn doc_for_name<'a>(module: &'a Module, name: &str) -> Option<&'a str> {
    for item in &module.items {
        match item {
            Item::Function(f) if f.name == name => return f.doc.as_deref(),
            Item::ShapeDecl(s) if s.name == name => return s.doc.as_deref(),
            Item::OptionsDecl(o) if o.name == name => return o.doc.as_deref(),
            Item::ConstDecl(c) if c.name == name => return c.doc.as_deref(),
            _ => {}
        }
    }
    None
}

/// Build the LSP `Hover` response for the given cursor position.
///
/// Priority (context-aware):
/// 1. When the parsed module is available, check whether the cursor is on a
///    value-position expression (identifier use-site in the AST). If so, show
///    the user-symbol hover — either the typeck function signature or a minimal
///    binding hover. This prevents contextual keywords (`errors`, `wait`, `is`,
///    `background`, `share`, …) from triggering their registry keyword hover when
///    a user has defined a same-named local binding.
/// 2. Registry lookup by token name — covers keywords, intrinsics, deferred
///    features, and banned terms that are NOT shadowed by a user binding at this
///    cursor position (e.g. `errors` in a return-type annotation, `share` in a
///    parameter modifier position).
/// 3. Typeck signature lookup for user-defined functions, prepended with `///`
///    doc comments when available.
/// 4. `None` if nothing resolves (cursor on punctuation, whitespace, or an
///    unannotated binding with no sig entry).
///
/// `module` is `Some` when the parsed AST is available; passing `None` skips
/// step 1 and falls back to the signature-only path.
///
/// Time: O(log t) token bsearch + O(m) AST use-site walk (step 1) + O(1) registry
/// + O(f) sig_table scan, where t = tokens, m = AST nodes, f = top-level fns. Runs
/// per hover request (not per keystroke). Space: O(1).
pub fn hover_response(
    tokens: &[Spanned<Token>],
    sig_table: &ynz_typeck::signatures::SignatureTable,
    module: Option<&Module>,
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

    // Step 1 (context-aware): if the AST is available and the cursor is on a
    // value-position identifier use-site, show the user-symbol hover immediately
    // — bypassing the registry so a variable named `errors` does not receive
    // the `errors`-keyword hover. Ownership-modifier positions (`share self`,
    // `lend x`, `give x`) are NOT recorded as identifier use-sites in the AST
    // walk (`ident_in_function` tracks param name spans, not modifier spans), so
    // they fall through to the registry lookup as expected.
    if let Some(module) = module {
        if let Some((use_name, _)) = identifier_use_site_at_offset(module, byte_offset) {
            // Prefer full function-signature hover (for named functions). For local
            // bindings not in sig_table, look up the binding's declared type by name
            // so annotated locals like `let share: int = 5` show the type at any
            // use-site, not just at the declaration offset. Only annotated bindings and
            // parameters are covered; unannotated lets return None → typeless binding
            // hover — never wrong, sometimes incomplete.
            return Some(
                user_symbol_hover(&token_name, sig_table, Some(module), range)
                    .unwrap_or_else(|| {
                        let ty = type_of_name_at_offset(module, &use_name, byte_offset);
                        binding_hover(&token_name, ty.as_ref(), range)
                    }),
            );
        }
    }

    // Step 2: registry lookup — keywords, intrinsics, deferred features.
    if let Some(content) = lsp_hover_for_token(&token_name) {
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content.markdown_body,
            }),
            range,
        });
    }

    // Step 3: typeck signature lookup (module=None path, or name not a use-site).
    user_symbol_hover(&token_name, sig_table, module, range)
}

/// Produce a user-symbol hover: function signature with optional doc comment,
/// or `None` when the name is not a known function (caller falls through to
/// `binding_hover`).
///
/// Time: O(1) sig_table lookup + O(m) doc-comment scan where m = top-level items.
/// Space: O(1).
fn user_symbol_hover(
    token_name: &str,
    sig_table: &ynz_typeck::signatures::SignatureTable,
    module: Option<&Module>,
    range: Option<Range>,
) -> Option<Hover> {
    if let Some(sig) = sig_table.fns.get(token_name) {
        let param_str = sig
            .params
            .iter()
            .map(|(pname, ptype)| format!("{pname}: {}", type_name(ptype)))
            .collect::<Vec<_>>()
            .join(", ");
        let sig_body = format!(
            "## `function {}({})`\n\nReturns: `{}`",
            token_name,
            param_str,
            type_name(&sig.ret)
        );

        // Prepend doc comment if the parsed module is available.
        let body = if let Some(doc) = module.and_then(|m| doc_for_name(m, token_name)) {
            format!("{doc}\n\n---\n\n{sig_body}")
        } else {
            sig_body
        };

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

/// Produce a minimal binding hover for a user-defined local that is not a
/// function in `sig_table`. Called only from the context-aware path when
/// `identifier_use_site_at_offset` confirms the cursor is on an expression
/// use-site (not a keyword modifier position).
///
/// When `ty` is `Some`, the type name is shown (e.g. "`share`: `int`") using
/// the cheap AST annotation walk from `type_of_name_at_offset`. When
/// `ty` is `None` (unannotated binding), only the name is shown — the full
/// type requires `check_query` (too expensive per-keystroke).
///
/// Time: O(1).  Space: O(1).
fn binding_hover(token_name: &str, ty: Option<&ynz_typeck::types::Type>, range: Option<Range>) -> Hover {
    let value = match ty {
        Some(t) => format!("`{}`: `{}`", token_name, type_name(t)),
        None => format!("**Binding**: `{token_name}`"),
    };
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use ynz_parser::lexer::lex;
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
        SignatureTable {
            fns: HashMap::new(),
        }
    }

    #[test]
    fn hover_keyword_returns_some() {
        let src = "function entrypoint() -> nothing { }";
        let tokens = tokenize(src);
        let table = LineTable::new(src);
        let result = hover_response(&tokens, &make_sig(), None, src, &table, 3, PositionEncoding::Utf8);
        assert!(
            result.is_some(),
            "hovering over 'function' keyword should return Some"
        );
        if let Some(Hover {
            contents: HoverContents::Markup(mc),
            ..
        }) = result
        {
            assert!(
                mc.value.contains("function"),
                "hover body must mention the keyword"
            );
        }
    }

    #[test]
    fn hover_in_whitespace_returns_none() {
        let src = "let   x = 5";
        let tokens = tokenize(src);
        let table = LineTable::new(src);
        let result = hover_response(&tokens, &make_sig(), None, src, &table, 4, PositionEncoding::Utf8);
        assert!(result.is_none(), "whitespace offset should return None");
    }

    #[test]
    fn hover_empty_file_returns_none() {
        let src = "";
        let tokens = tokenize(src);
        let table = LineTable::new(src);
        let result = hover_response(&tokens, &make_sig(), None, src, &table, 0, PositionEncoding::Utf8);
        assert!(result.is_none());
    }
}
