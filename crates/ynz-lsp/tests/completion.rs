// WHY: Completion integration tests verify the registry → LSP pipeline
// produces items with correct content and sort order. Tests here complement
// the registry-level unit tests in crates/ynz-registry/tests/lsp_adapter.rs.

mod harness;
use harness::InProcessHarness;
use ynz_lsp::{capabilities::PositionEncoding, completion::detect_context};
use ynz_registry::CompletionContext;

fn read_fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("could not read fixture {name}: {e}"))
}

// Context detection unit tests

#[test]
fn context_bare_at_start() {
    assert_eq!(detect_context("", 0), CompletionContext::BareIdentifier);
}

#[test]
fn context_bare_after_whitespace() {
    let text = "let x = ";
    let ctx = detect_context(text, text.len());
    assert_eq!(ctx, CompletionContext::BareIdentifier);
}

#[test]
fn context_after_dot_is_after_dot() {
    let text = "score.";
    let ctx = detect_context(text, text.len());
    assert!(matches!(ctx, CompletionContext::AfterDot { .. }));
}

#[test]
fn context_cursor_inside_string_no_panic_and_returns_bare() {
    // WHY: thin-slice detect_context does not distinguish string-interior from
    // normal code; it returns BareIdentifier. This test verifies no panic AND
    // that the fallback produces the expected BareIdentifier context.
    let text = "`hello wor";
    let ctx = detect_context(text, text.len());
    assert_eq!(ctx, CompletionContext::BareIdentifier);
}

#[test]
fn context_multibyte_char_boundary_no_panic() {
    // Cursor offset at a valid UTF-8 boundary within multi-byte characters.
    let text = "✓.";
    // '✓' is 3 bytes; cursor after the dot (byte 4)
    let ctx = detect_context(text, 4);
    assert!(matches!(ctx, CompletionContext::AfterDot { .. }));
}

#[test]
fn context_numeric_literal_dot_is_bare() {
    // "5." should NOT trigger after-dot completion (it's a decimal literal)
    let text = "let x = 5.";
    let ctx = detect_context(text, text.len());
    assert_eq!(ctx, CompletionContext::BareIdentifier);
}

#[test]
fn context_identifier_dot_is_after_dot() {
    // "a5." — last char before '.' is digit but preceded by 'a' (ident char), so it's a receiver
    let text = "let a5 = 0\na5.";
    let ctx = detect_context(text, text.len());
    assert!(matches!(ctx, CompletionContext::AfterDot { .. }));
}

// Completion list tests (using the completion module directly)

#[test]
fn completion_list_with_user_fns_includes_them() {
    use lsp_types::Position;
    use std::collections::HashMap;
    use ynz_diagnostics::SourceSpan;
    use ynz_lsp::{completion::completion_list, position::LineTable};
    use ynz_typeck::shapes::ShapeTable;
    use ynz_typeck::{
        signatures::{FunctionSig, SignatureTable},
        types::Type,
    };

    let text = "let ";
    let table = LineTable::new(text);
    let position = Position {
        line: 0,
        character: 4,
    };

    let mut fns = HashMap::new();
    fns.insert(
        "myUserFunction".to_string(),
        FunctionSig {
            params: vec![("x".to_string(), Type::Int)],
            param_ownerships: vec![None],
            ret: Type::Nothing,
            decl_span: SourceSpan::new("test.ynz", 0, 0),
        },
    );
    let sig_table = SignatureTable { fns };
    let shape_table = ShapeTable {
        shapes: HashMap::new(),
        union_aliases: HashMap::new(),
        options_names: std::collections::HashSet::new(),
    };

    let list = completion_list(
        text,
        &table,
        position,
        PositionEncoding::Utf8,
        Some(&sig_table),
        Some(&shape_table),
        None,
    );
    let list = list.expect("completion must be Some");
    let labels: Vec<_> = list.items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.contains(&"myUserFunction"),
        "user-defined function must appear in completion"
    );
    let user_item = list
        .items
        .iter()
        .find(|i| i.label == "myUserFunction")
        .unwrap();
    assert_eq!(
        user_item.kind,
        Some(lsp_types::CompletionItemKind::FUNCTION)
    );
}

#[test]
fn user_symbols_sort_before_keywords() {
    use lsp_types::Position;
    use std::collections::HashMap;
    use ynz_diagnostics::SourceSpan;
    use ynz_lsp::{completion::completion_list, position::LineTable};
    use ynz_typeck::shapes::ShapeTable;
    use ynz_typeck::{
        signatures::{FunctionSig, SignatureTable},
        types::Type,
    };

    let text = "let ";
    let table = LineTable::new(text);
    let position = Position {
        line: 0,
        character: 4,
    };

    let mut fns = HashMap::new();
    fns.insert(
        "myFn".to_string(),
        FunctionSig {
            params: vec![],
            param_ownerships: vec![],
            ret: Type::Nothing,
            decl_span: SourceSpan::new("test.ynz", 0, 0),
        },
    );
    let sig_table = SignatureTable { fns };
    let shape_table = ShapeTable {
        shapes: HashMap::new(),
        union_aliases: HashMap::new(),
        options_names: std::collections::HashSet::new(),
    };

    let list = completion_list(
        text,
        &table,
        position,
        PositionEncoding::Utf8,
        Some(&sig_table),
        Some(&shape_table),
        None,
    );
    let list = list.expect("completion must be Some");

    let user_item = list.items.iter().find(|i| i.label == "myFn").unwrap();
    let kw_item = list
        .items
        .iter()
        .find(|i| i.label == "function" && i.kind == Some(lsp_types::CompletionItemKind::KEYWORD))
        .unwrap();

    let user_sort = user_item.sort_text.as_deref().unwrap_or("");
    let kw_sort = kw_item.sort_text.as_deref().unwrap_or("");
    assert!(
        user_sort < kw_sort,
        "user symbol ({user_sort}) must sort before keyword ({kw_sort})"
    );
}

#[test]
fn bare_completion_contains_keywords() {
    use lsp_types::Position;
    use ynz_lsp::{completion::completion_list, position::LineTable};

    let text = "function entrypoint() -> nothing {\n    ";
    let table = LineTable::new(text);
    let position = Position {
        line: 1,
        character: 4,
    };
    let list = completion_list(text, &table, position, PositionEncoding::Utf8, None, None, None);
    let list = list.expect("completion list must be Some");
    let kw_labels: Vec<_> = list
        .items
        .iter()
        .filter(|i| {
            i.kind == Some(lsp_types::CompletionItemKind::KEYWORD) && !i.deprecated.unwrap_or(false)
        })
        .map(|i| i.label.as_str())
        .collect();
    assert!(
        !kw_labels.is_empty(),
        "bare completion must include keywords"
    );
    assert!(
        kw_labels.contains(&"let"),
        "bare completion must include 'let'"
    );
    assert!(
        kw_labels.contains(&"const"),
        "bare completion must include 'const'"
    );
}

#[test]
fn deferred_features_in_bare_are_deprecated() {
    use lsp_types::Position;
    use ynz_lsp::{completion::completion_list, position::LineTable};

    let text = "let ";
    let table = LineTable::new(text);
    let list = completion_list(
        text,
        &table,
        Position {
            line: 0,
            character: 4,
        },
        PositionEncoding::Utf8,
        None,
        None,
        None,
    );
    let list = list.expect("completion must be Some");

    let deferred: Vec<_> = list
        .items
        .iter()
        .filter(|i| {
            i.tags
                .as_deref()
                .map(|t| t.contains(&lsp_types::CompletionItemTag::DEPRECATED))
                .unwrap_or(false)
        })
        .collect();
    assert!(
        !deferred.is_empty(),
        "deferred features must appear deprecated in bare completion"
    );
}

#[test]
fn after_dot_completion_returns_items() {
    use lsp_types::Position;
    use ynz_lsp::{completion::completion_list, position::LineTable};

    // Cursor after "score." where score is int
    let text = "function entrypoint() -> nothing {\n    let score: int = 42\n    score.";
    let table = LineTable::new(text);
    let lines: Vec<&str> = text.lines().collect();
    let last_line = lines.last().unwrap();
    let position = Position {
        line: (lines.len() - 1) as u32,
        character: last_line.len() as u32,
    };
    let list = completion_list(text, &table, position, PositionEncoding::Utf8, None, None, None);
    let list = list.expect("completion after dot must return Some");
    // In this thin slice, receiver type is not narrowed (None → all methods returned)
    assert!(
        !list.items.is_empty(),
        "after-dot completion should return items"
    );
}

#[test]
fn numeric_dot_returns_bare_completion() {
    use lsp_types::Position;
    use ynz_lsp::{completion::completion_list, position::LineTable};

    let text = "let x = 5.";
    let table = LineTable::new(text);
    let position = Position {
        line: 0,
        character: text.len() as u32,
    };
    let list = completion_list(text, &table, position, PositionEncoding::Utf8, None, None, None);
    // "5." treated as decimal literal → BareIdentifier context → keywords appear
    let list = list.expect("completion must be Some even for numeric dot");
    let kw_count = list
        .items
        .iter()
        .filter(|i| i.kind == Some(lsp_types::CompletionItemKind::KEYWORD))
        .count();
    assert!(
        kw_count > 0,
        "numeric dot should produce bare-identifier (keyword) completion"
    );
}

#[test]
fn after_dot_with_receiver_type_narrows_to_int_methods() {
    // WHY: receiver-type narrowing ensures `score.` (where score: int) shows ONLY int methods,
    //      not all primitive methods (string, float, etc.).  Without narrowing users see noisy
    //      false completions (e.g. `.count()` which is a string method).
    use lsp_types::Position;
    use ynz_lsp::{completion::completion_list, position::LineTable};

    let text = "score.";
    let table = LineTable::new(text);
    let position = Position {
        line: 0,
        character: text.len() as u32,
    };
    // Pass receiver_type_name = Some("int") as if typeck resolved `score: int`
    let list = completion_list(
        text,
        &table,
        position,
        PositionEncoding::Utf8,
        None,
        None,
        Some("int"),
    );
    let list = list.expect("completion must be Some");
    let labels: Vec<&str> = list.items.iter().map(|i| i.label.as_str()).collect();

    // int has methods like toString(), toFloat(), toNumber() etc.
    assert!(
        labels.iter().any(|l| l.starts_with("toString")),
        "int after-dot should include toString(): {labels:?}"
    );
    // string-only method should NOT appear when receiver is int
    assert!(
        !labels.contains(&"count()"),
        "string method count() must not appear for int receiver: {labels:?}"
    );
}

#[test]
fn after_dot_with_no_receiver_type_shows_all_methods() {
    // WHY: when receiver type is unknown (None), all primitive methods appear as best-effort
    //      candidates so the user still gets helpful completions rather than nothing.
    use lsp_types::Position;
    use ynz_lsp::{completion::completion_list, position::LineTable};

    let text = "x.";
    let table = LineTable::new(text);
    let position = Position {
        line: 0,
        character: text.len() as u32,
    };
    let list = completion_list(text, &table, position, PositionEncoding::Utf8, None, None, None);
    let list = list.expect("completion must be Some");
    // Without narrowing, should include methods from multiple primitive types
    let method_count = list
        .items
        .iter()
        .filter(|i| i.kind == Some(lsp_types::CompletionItemKind::METHOD))
        .count();
    assert!(
        method_count > 0,
        "after-dot with no receiver type should still return primitive method candidates"
    );
}

// LSP wire-level test (using in-process harness)

#[test]
fn completion_request_returns_list_via_lsp() {
    use serde_json::json;

    let h = InProcessHarness::new().start_server();
    h.initialize();
    let text = read_fixture("completion_bare.ynz");
    h.did_open("file:///completion_bare.ynz", &text);
    h.try_recv_timeout(std::time::Duration::from_millis(100)); // drain diagnostic

    // Send completion request at line 0 char 0 (start of file)
    h.conn
        .sender
        .send(lsp_server::Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(10i32),
            method: "textDocument/completion".to_string(),
            params: json!({
                "textDocument": { "uri": "file:///completion_bare.ynz" },
                "position": { "line": 0, "character": 0 }
            }),
        }))
        .unwrap();

    let response = h.recv_response();
    // Response may be Null (no items at position 0 = inside a comment) or a CompletionList
    // Either way it must not be an error object
    assert!(
        response.get("error").is_none(),
        "completion response must not contain an error field: {response}"
    );
}
