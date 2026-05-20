// WHY: Hover integration tests verify the token-at-offset → registry/typeck
// pipeline produces correct markdown content. These tests complement the
// unit tests in ynz_lsp::hover::tests.

mod harness;
use harness::InProcessHarness;
use ynz_lsp::{
    capabilities::PositionEncoding,
    hover::{hover_response, token_at_offset},
    position::LineTable,
};
use ynz_parser::lexer::lex;

fn tokenize(src: &str) -> Vec<ynz_parser::token::Spanned<ynz_parser::token::Token>> {
    let (tokens, _) = lex("test.ynz", src);
    tokens
}

fn make_sig() -> ynz_typeck::signatures::SignatureTable {
    ynz_typeck::signatures::SignatureTable {
        fns: std::collections::HashMap::new(),
    }
}

#[test]
fn hover_function_keyword() {
    let src = "function entrypoint";
    let tokens = tokenize(src);
    let table = LineTable::new(src);
    let h = hover_response(&tokens, &make_sig(), src, &table, 0, PositionEncoding::Utf8);
    assert!(
        h.is_some(),
        "hover over 'function' keyword should return Some"
    );
    if let Some(h) = h {
        use lsp_types::HoverContents;
        if let HoverContents::Markup(mc) = h.contents {
            assert!(
                mc.value.contains("function"),
                "hover must mention the keyword"
            );
        }
    }
}

#[test]
fn hover_user_defined_function() {
    use std::collections::HashMap;
    use ynz_diagnostics::SourceSpan;
    use ynz_typeck::{
        signatures::{FunctionSig, SignatureTable},
        types::Type,
    };

    let src = "myFunc";
    let tokens = tokenize(src);
    let table = LineTable::new(src);

    let mut fns = HashMap::new();
    fns.insert(
        "myFunc".to_string(),
        FunctionSig {
            params: vec![("n".to_string(), Type::Int)],
            param_ownerships: vec![None],
            ret: Type::String,
            decl_span: SourceSpan::new("test.ynz", 0, 0),
        },
    );
    let sig = SignatureTable { fns };

    let h = hover_response(&tokens, &sig, src, &table, 2, PositionEncoding::Utf8);
    assert!(
        h.is_some(),
        "hover over user-defined function name should return Some"
    );
    if let Some(h) = h {
        use lsp_types::HoverContents;
        if let HoverContents::Markup(mc) = h.contents {
            assert!(
                mc.value.contains("myFunc"),
                "hover must include function name"
            );
            assert!(mc.value.contains("int"), "hover must include param type");
            assert!(
                mc.value.contains("string"),
                "hover must include return type"
            );
        }
    }
}

#[test]
fn hover_inside_comment_returns_none() {
    // token_at_offset only returns tokens — comments are not tokens, so hover is None
    let src = "// this is a comment";
    let tokens = tokenize(src);
    let table = LineTable::new(src);
    // Offset 5 is inside the comment text
    let h = hover_response(&tokens, &make_sig(), src, &table, 5, PositionEncoding::Utf8);
    assert!(h.is_none(), "hover inside a comment should return None");
}

#[test]
fn hover_at_byte_zero_of_empty_file_returns_none() {
    let src = "";
    let tokens = tokenize(src);
    let table = LineTable::new(src);
    let h = hover_response(&tokens, &make_sig(), src, &table, 0, PositionEncoding::Utf8);
    assert!(
        h.is_none(),
        "hover at byte 0 of empty file should return None"
    );
}

#[test]
fn token_at_offset_returns_range() {
    let src = "function entrypoint";
    let tokens = tokenize(src);
    let result = token_at_offset(&tokens, 0);
    assert!(result.is_some());
    let (text, start, end) = result.unwrap();
    assert_eq!(text, "function");
    assert_eq!(start, 0);
    assert!(end > start, "end must be after start");
}

#[test]
fn hover_registry_content_with_angle_brackets_does_not_crash() {
    // WHY: registry entries like intrinsic return types contain `<>` (e.g. "range<int>",
    // "maybe<int>"). In LSP markdown, backtick code spans make `<>` HTML-inert.
    // This test verifies that the hover pipeline doesn't panic on such content
    // and that the body is a valid non-empty string.
    let src = "range";
    let tokens = tokenize(src);
    let table = LineTable::new(src);
    // "range" is a free function intrinsic — lsp_hover_for_token should return Some
    let h = hover_response(&tokens, &make_sig(), src, &table, 0, PositionEncoding::Utf8);
    assert!(
        h.is_some(),
        "hover over 'range' intrinsic should return Some"
    );
    if let Some(h) = h {
        use lsp_types::HoverContents;
        if let HoverContents::Markup(mc) = h.contents {
            assert!(!mc.value.is_empty(), "hover body must not be empty");
            assert!(mc.value.contains("range"), "body must mention 'range'");
        }
    }
}

#[test]
fn hover_request_via_lsp_returns_response() {
    use serde_json::json;

    let h = InProcessHarness::new().start_server();
    h.initialize();
    h.did_open(
        "file:///hover_test.ynz",
        "function entrypoint() -> nothing { }",
    );
    h.try_recv_timeout(std::time::Duration::from_millis(100)); // drain diagnostic

    // Send hover request at position 0 (over "function" keyword)
    h.conn
        .sender
        .send(lsp_server::Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(20i32),
            method: "textDocument/hover".to_string(),
            params: json!({
                "textDocument": { "uri": "file:///hover_test.ynz" },
                "position": { "line": 0, "character": 0 }
            }),
        }))
        .unwrap();

    let response = h.recv_response();
    // Hover at position 0 over "function" should return a populated result
    assert!(
        response.get("error").is_none(),
        "hover response must not contain an error: {response}"
    );
}
