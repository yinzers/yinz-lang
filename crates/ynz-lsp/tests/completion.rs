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

// Snippet template tests

#[test]
// WHY: Snippet format on the `function` keyword guarantees the editor's tab-stop
//      machinery fires — without SNIPPET format the `${1:name}` literal text is
//      inserted verbatim instead of becoming an interactive placeholder. This test
//      catches any regression where format is accidentally dropped or set to PlainText.
fn function_keyword_has_snippet_format() {
    use lsp_types::Position;
    use ynz_lsp::{completion::completion_list, position::LineTable};

    let text = "fun";
    let table = LineTable::new(text);
    let position = Position { line: 0, character: 3 };
    let list = completion_list(text, &table, position, PositionEncoding::Utf8, None, None, None)
        .expect("completion must be Some");

    let fn_kw = list
        .items
        .iter()
        .find(|i| i.label == "function" && i.kind == Some(lsp_types::CompletionItemKind::KEYWORD))
        .expect("'function' keyword must appear in completion list");

    assert_eq!(
        fn_kw.insert_text_format,
        Some(lsp_types::InsertTextFormat::SNIPPET),
        "'function' keyword item must use SNIPPET insert_text_format"
    );
    let insert_text = fn_kw.insert_text.as_deref().expect("'function' must have insert_text");
    assert!(
        insert_text.contains("${1:name}"),
        "'function' snippet must contain a name tab stop; got: {insert_text:?}"
    );
}

#[test]
// WHY: User-defined functions must use SNIPPET format so param tab stops are active.
//      Without it the inserted text `heal(${1:amount})` is literal, not interactive.
//      This test also verifies the `self` param is skipped — including it as a tab
//      stop forces the user to fill in the receiver, breaking UFCS dot-call ergonomics.
fn user_defined_fn_has_snippet_with_param_tab_stops() {
    use lsp_types::Position;
    use std::collections::HashMap;
    use ynz_diagnostics::SourceSpan;
    use ynz_lsp::{completion::completion_list, position::LineTable};
    use ynz_typeck::shapes::ShapeTable;
    use ynz_typeck::{
        signatures::{FunctionSig, SignatureTable},
        types::Type,
    };

    let text = "he";
    let table = LineTable::new(text);
    let position = Position { line: 0, character: 2 };

    let mut fns = HashMap::new();
    fns.insert(
        "heal".to_string(),
        FunctionSig {
            params: vec![
                ("self".to_string(), Type::Shape { name: "Player".to_string() }),
                ("amount".to_string(), Type::Int),
            ],
            param_ownerships: vec![None, None],
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
        text, &table, position, PositionEncoding::Utf8,
        Some(&sig_table), Some(&shape_table), None,
    )
    .expect("completion must be Some");

    let heal_item = list
        .items
        .iter()
        .find(|i| i.label == "heal")
        .expect("user-defined 'heal' function must appear");

    assert_eq!(
        heal_item.insert_text_format,
        Some(lsp_types::InsertTextFormat::SNIPPET),
        "user fn item must use SNIPPET format"
    );
    let snippet = heal_item.insert_text.as_deref().expect("heal must have insert_text");
    assert!(
        snippet.contains("${1:amount}"),
        "snippet must have tab stop for 'amount'; got: {snippet:?}"
    );
    assert!(
        !snippet.contains("self"),
        "snippet must not include 'self' parameter; got: {snippet:?}"
    );
}

#[test]
// WHY: A function with only a `self` parameter should still produce a valid snippet
//      (cursor inside the empty parens via $0). Without this, `greet($0)` regresses
//      to either an empty string or a plain `greet()` that loses the cursor placement.
fn user_defined_fn_self_only_snippet_has_cursor_stop() {
    use lsp_types::Position;
    use std::collections::HashMap;
    use ynz_diagnostics::SourceSpan;
    use ynz_lsp::{completion::completion_list, position::LineTable};
    use ynz_typeck::shapes::ShapeTable;
    use ynz_typeck::{
        signatures::{FunctionSig, SignatureTable},
        types::Type,
    };

    let text = "gr";
    let table = LineTable::new(text);
    let position = Position { line: 0, character: 2 };

    let mut fns = HashMap::new();
    fns.insert(
        "greet".to_string(),
        FunctionSig {
            params: vec![("self".to_string(), Type::Shape { name: "Player".to_string() })],
            param_ownerships: vec![None],
            ret: Type::String,
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
        text, &table, position, PositionEncoding::Utf8,
        Some(&sig_table), Some(&shape_table), None,
    )
    .expect("completion must be Some");

    let greet_item = list
        .items
        .iter()
        .find(|i| i.label == "greet")
        .expect("user-defined 'greet' function must appear");

    assert_eq!(
        greet_item.insert_text_format,
        Some(lsp_types::InsertTextFormat::SNIPPET),
        "self-only fn item must still use SNIPPET format"
    );
    let snippet = greet_item.insert_text.as_deref().expect("greet must have insert_text");
    assert_eq!(
        snippet, "greet($0)",
        "self-only fn snippet must be 'greet($0)'; got: {snippet:?}"
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

#[test]
// WHY: __anon__ names are internal synthetic identifiers for anonymous inline shapes.
// Leaking them into completion makes the list actively misleading — users see
// `__anon__a_int__b_string` and have no idea what to do with it. This test
// catches any regression where internal shape names slip through the filter.
fn anon_shapes_excluded_from_completion() {
    use lsp_types::Position;
    use std::collections::HashMap;
    use ynz_lsp::{capabilities::PositionEncoding, completion::completion_list, position::LineTable};
    use ynz_typeck::shapes::{ShapeDef, ShapeTable};
    use ynz_typeck::signatures::SignatureTable;

    let text = "let ";
    let table = LineTable::new(text);

    let mut shapes = HashMap::new();
    let dummy_span = ynz_diagnostics::SourceSpan::new("test.ynz", 0, 0);
    // A normal shape — must appear.
    shapes.insert(
        "Player".to_string(),
        ShapeDef {
            name: "Player".to_string(),
            is_base: false,
            extends: None,
            follows: vec![],
            fields: vec![],
            contract_sigs: vec![],
            defined_at: dummy_span.clone(),
        },
    );
    // A synthetic anon shape — must NOT appear.
    shapes.insert(
        "__anon__health_int__name_string".to_string(),
        ShapeDef {
            name: "__anon__health_int__name_string".to_string(),
            is_base: false,
            extends: None,
            follows: vec![],
            fields: vec![],
            contract_sigs: vec![],
            defined_at: dummy_span,
        },
    );

    let shape_table = ShapeTable {
        shapes,
        union_aliases: HashMap::new(),
        options_names: std::collections::HashSet::new(),
    };
    let sig_table = SignatureTable { fns: HashMap::new() };

    let list = completion_list(
        text,
        &table,
        Position { line: 0, character: 4 },
        PositionEncoding::Utf8,
        Some(&sig_table),
        Some(&shape_table),
        None,
    )
    .expect("completion must be Some");

    let labels: Vec<&str> = list.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"Player"), "named shape must appear in completion");
    assert!(
        !labels.iter().any(|l| l.starts_with("__anon__")),
        "synthetic __anon__ shapes must not appear in completion; got: {labels:?}"
    );
}

#[test]
// WHY: cross_file_completion_items must appear in Ctrl+Space suggestions with
// additionalTextEdits that insert the import. Without this, users type an exported
// symbol, get "No suggestions", and have to manually write the import — exactly
// the friction the feature exists to eliminate.
fn cross_file_items_appear_in_completion_with_import_edit() {
    use lsp_types::Position;
    use ynz_lsp::{capabilities::PositionEncoding, completion::cross_file_completion_items, position::LineTable, state::ServerState};

    let root = std::path::Path::new("/tmp/ynz_cross_completion");
    let exporter_src = "export shape Order { id: string }\nexport function processOrder(o: Order) -> nothing { }\n";
    let importer_src = "function main() -> nothing { }\n";

    let mut state = ServerState::new(PositionEncoding::Utf8);
    state.project_root = Some(root.to_path_buf());

    let exp_path = root.join("services/orders.ynz");
    let exp_uri = lsp_types::Url::from_file_path(&exp_path).unwrap();
    state.open_document(exp_uri, exporter_src.to_string());

    let imp_path = root.join("entrypoint.ynz");
    let imp_uri = lsp_types::Url::from_file_path(&imp_path).unwrap();
    state.open_document(imp_uri.clone(), importer_src.to_string());

    let table = LineTable::new(importer_src);
    let items = cross_file_completion_items(&state, &imp_uri, importer_src, &table, None, None);

    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"Order"), "exported shape must appear in cross-file completion; got: {labels:?}");
    assert!(labels.contains(&"processOrder"), "exported function must appear; got: {labels:?}");

    // Every cross-file item must carry an additionalTextEdits with the import line.
    for item in items.iter().filter(|i| i.label == "Order" || i.label == "processOrder") {
        let edits = item.additional_text_edits.as_ref()
            .unwrap_or_else(|| panic!("{} must have additionalTextEdits", item.label));
        assert_eq!(edits.len(), 1);
        assert!(
            edits[0].new_text.contains("services/orders"),
            "{} import text must contain the path; got: {:?}", item.label, edits[0].new_text
        );
    }
}
