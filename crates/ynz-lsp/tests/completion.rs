// WHY: Completion tests verify the registry → LSP pipeline produces items with
// correct content, and that the cursor context decides which items are offered.
// They complement the registry-level tests in crates/ynz-registry/tests/lsp_adapter.rs.

mod harness;
use harness::InProcessHarness;
use lsp_types::{CompletionItemKind, CompletionList, InsertTextFormat, Position};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use ynz_diagnostics::SourceSpan;
use ynz_lsp::{
    capabilities::PositionEncoding,
    completion::{completion_list, cross_file_completion_items, detect_context},
    position::LineTable,
    state::ServerState,
};
use ynz_registry::CompletionContext;
use ynz_typeck::shapes::{ShapeDef, ShapeTable};
use ynz_typeck::{
    signatures::{FunctionSig, SignatureTable},
    types::Type,
};

// ---------------------------------------------------------------------------
// detect_context
// ---------------------------------------------------------------------------

macro_rules! detect_context_case {
    ($name:ident, $text:expr, $cursor_offset:expr, $expects_after_dot:expr) => {
        // bug: the cursor context is misclassified, so completion offers keywords after a dot, receiver methods after a decimal point, or panics on a multibyte receiver | tier: Medium
        #[test]
        fn $name() {
            let text: &str = $text;
            let context = detect_context(text, $cursor_offset);
            let is_after_dot = matches!(context, CompletionContext::AfterDot { .. });
            assert_eq!(
                is_after_dot, $expects_after_dot,
                "context for {text:?} was {context:?}"
            );
        }
    };
}

detect_context_case!(empty_text_is_a_bare_identifier, "", 0, false);
detect_context_case!(
    whitespace_before_cursor_is_a_bare_identifier,
    "let x = ",
    8,
    false
);
detect_context_case!(dot_after_an_identifier_is_after_dot, "score.", 6, true);
detect_context_case!(
    dot_after_an_identifier_ending_in_a_digit_is_after_dot,
    "let a5 = 0\na5.",
    14,
    true
);
detect_context_case!(decimal_point_is_a_bare_identifier, "let x = 5.", 10, false);
// '✓' is 3 bytes, so the cursor after the dot sits at byte 4.
detect_context_case!(dot_after_a_multibyte_char_is_after_dot, "✓.", 4, true);

// ---------------------------------------------------------------------------
// completion_list
// ---------------------------------------------------------------------------

fn function_sig(params: Vec<(&str, Type)>) -> FunctionSig {
    FunctionSig {
        param_ownerships: vec![None; params.len()],
        params: params
            .into_iter()
            .map(|(name, ty)| (name.to_string(), ty))
            .collect(),
        ret: Type::Nothing,
        decl_span: SourceSpan::new("test.ynz", 0, 0),
        contains_wait: false,
        suspends: false,
        does_real_work: false,
        written_param_places: Vec::new(),
        original_name: None,
    }
}

fn signature_table(functions: Vec<(&str, FunctionSig)>) -> SignatureTable {
    SignatureTable {
        fns: functions
            .into_iter()
            .map(|(name, sig)| (name.to_string(), sig))
            .collect(),
    }
}

fn shape_table(shape_names: &[&str]) -> ShapeTable {
    let shapes = shape_names
        .iter()
        .map(|name| {
            (
                name.to_string(),
                ShapeDef {
                    name: name.to_string(),
                    is_base: false,
                    extends: None,
                    follows: vec![],
                    fields: vec![],
                    contract_sigs: vec![],
                    defined_at: SourceSpan::new("test.ynz", 0, 0),
                },
            )
        })
        .collect();
    ShapeTable {
        shapes,
        union_aliases: HashMap::new(),
        options_names: HashSet::new(),
        layout_only: HashMap::new(),
    }
}

/// Completion with the cursor at the very end of `text` (ASCII only).
fn completion_at_end_of(
    text: &str,
    signatures: Option<&SignatureTable>,
    shapes: Option<&ShapeTable>,
    receiver_type_name: Option<&str>,
) -> CompletionList {
    let line = text.matches('\n').count() as u32;
    let character = text.rsplit('\n').next().unwrap_or(text).len() as u32;
    completion_list(
        text,
        &LineTable::new(text),
        Position { line, character },
        PositionEncoding::Utf8,
        signatures,
        shapes,
        receiver_type_name,
    )
    .expect("completion must be Some")
}

fn labels_of(list: &CompletionList) -> Vec<&str> {
    list.items.iter().map(|item| item.label.as_str()).collect()
}

fn item_named<'a>(list: &'a CompletionList, label: &str) -> &'a lsp_types::CompletionItem {
    list.items
        .iter()
        .find(|item| item.label == label)
        .unwrap_or_else(|| panic!("{label:?} must be offered; got {:?}", labels_of(list)))
}

fn check_user_function_is_listed_as_a_function_item(list: &CompletionList) {
    let item = item_named(list, "myUserFunction");
    assert_eq!(item.kind, Some(CompletionItemKind::FUNCTION));
    // User symbols outrank keywords in the editor's list.
    let keyword = list
        .items
        .iter()
        .find(|item| item.label == "function" && item.kind == Some(CompletionItemKind::KEYWORD))
        .expect("'function' keyword must be offered");
    assert!(
        item.sort_text < keyword.sort_text,
        "user symbol ({:?}) must sort before keyword ({:?})",
        item.sort_text,
        keyword.sort_text
    );
}

fn check_bare_position_offers_let_and_const(list: &CompletionList) {
    let labels = labels_of(list);
    assert!(labels.contains(&"let"), "got {labels:?}");
    assert!(labels.contains(&"const"), "got {labels:?}");
}

fn check_decimal_point_offers_keywords(list: &CompletionList) {
    let keyword_count = list
        .items
        .iter()
        .filter(|item| item.kind == Some(CompletionItemKind::KEYWORD))
        .count();
    assert!(
        keyword_count > 0,
        "a decimal point must fall back to bare completion"
    );
}

fn check_unknown_receiver_offers_methods(list: &CompletionList) {
    let method_count = list
        .items
        .iter()
        .filter(|item| item.kind == Some(CompletionItemKind::METHOD))
        .count();
    assert!(
        method_count > 0,
        "an unresolved receiver must still offer method candidates"
    );
}

fn check_int_receiver_offers_only_int_methods(list: &CompletionList) {
    let labels = labels_of(list);
    assert!(
        labels.iter().any(|label| label.starts_with("toString")),
        "Int receiver must offer toString(): {labels:?}"
    );
    assert!(
        !labels.contains(&"count()"),
        "String-only count() must not be offered for an Int receiver: {labels:?}"
    );
}

fn check_function_keyword_is_a_snippet_with_a_name_stop(list: &CompletionList) {
    let item = list
        .items
        .iter()
        .find(|item| item.label == "function" && item.kind == Some(CompletionItemKind::KEYWORD))
        .expect("'function' keyword must be offered");
    assert_eq!(item.insert_text_format, Some(InsertTextFormat::SNIPPET));
    let insert_text = item
        .insert_text
        .as_deref()
        .expect("snippet needs insert_text");
    assert!(insert_text.contains("${1:name}"), "got {insert_text:?}");
}

fn check_heal_snippet_skips_self(list: &CompletionList) {
    let item = item_named(list, "heal");
    assert_eq!(item.insert_text_format, Some(InsertTextFormat::SNIPPET));
    let snippet = item
        .insert_text
        .as_deref()
        .expect("heal must have insert_text");
    assert!(snippet.contains("${1:amount}"), "got {snippet:?}");
    assert!(!snippet.contains("self"), "got {snippet:?}");
}

fn check_self_only_snippet_places_the_cursor_inside_the_parens(list: &CompletionList) {
    let item = item_named(list, "greet");
    assert_eq!(item.insert_text_format, Some(InsertTextFormat::SNIPPET));
    assert_eq!(item.insert_text.as_deref(), Some("greet($0)"));
}

fn check_anonymous_shapes_are_hidden(list: &CompletionList) {
    let labels = labels_of(list);
    assert!(
        labels.contains(&"Player"),
        "named shape must be offered; got {labels:?}"
    );
    assert!(
        !labels.iter().any(|label| label.starts_with("__anon__")),
        "synthetic __anon__ shapes must not be offered; got {labels:?}"
    );
}

fn check_public_intrinsics_listed_internal_one_hidden(list: &CompletionList) {
    let labels = labels_of(list);
    assert!(labels.contains(&"sleep"), "got {labels:?}");
    assert!(labels.contains(&"sleepBlocking"), "got {labels:?}");
    assert!(
        !labels
            .iter()
            .any(|label| label.contains("__testFallibleAsync")),
        "internal intrinsic must stay invisible; got {labels:?}"
    );
}

macro_rules! completion_list_case {
    ($name:ident, $text:expr, $signatures:expr, $shapes:expr, $receiver:expr, $check:expr) => {
        // bug: the completion list is wrong for its cursor position: a user symbol is missing, mis-kinded or ranked below keywords, a keyword or method is missing, a snippet is plain text or wrong, or an internal name leaks | tier: Medium
        #[test]
        fn $name() {
            let signatures: Option<SignatureTable> = $signatures;
            let shapes: Option<ShapeTable> = $shapes;
            let list = completion_at_end_of($text, signatures.as_ref(), shapes.as_ref(), $receiver);
            let check: fn(&CompletionList) = $check;
            check(&list);
        }
    };
}

completion_list_case!(
    user_function_is_listed_as_a_function_item_ranked_before_keywords,
    "let ",
    Some(signature_table(vec![(
        "myUserFunction",
        function_sig(vec![("x", Type::Int)])
    )])),
    Some(shape_table(&[])),
    None,
    check_user_function_is_listed_as_a_function_item
);
completion_list_case!(
    bare_position_offers_let_and_const,
    "function entrypoint() -> Nothing {\n    ",
    None,
    None,
    None,
    check_bare_position_offers_let_and_const
);
completion_list_case!(
    decimal_point_falls_back_to_keyword_completion,
    "let x = 5.",
    None,
    None,
    None,
    check_decimal_point_offers_keywords
);
completion_list_case!(
    unresolved_receiver_still_offers_method_candidates,
    "x.",
    None,
    None,
    None,
    check_unknown_receiver_offers_methods
);
completion_list_case!(
    int_receiver_offers_only_int_methods,
    "score.",
    None,
    None,
    Some("Int"),
    check_int_receiver_offers_only_int_methods
);
completion_list_case!(
    function_keyword_is_a_snippet_with_a_name_tab_stop,
    "fun",
    None,
    None,
    None,
    check_function_keyword_is_a_snippet_with_a_name_stop
);
completion_list_case!(
    user_function_snippet_has_a_tab_stop_per_parameter_except_self,
    "he",
    Some(signature_table(vec![(
        "heal",
        function_sig(vec![
            (
                "self",
                Type::Shape {
                    name: "Player".to_string()
                }
            ),
            ("amount", Type::Int)
        ])
    )])),
    Some(shape_table(&[])),
    None,
    check_heal_snippet_skips_self
);
completion_list_case!(
    self_only_function_snippet_places_the_cursor_inside_the_parens,
    "gr",
    Some(signature_table(vec![(
        "greet",
        function_sig(vec![(
            "self",
            Type::Shape {
                name: "Player".to_string()
            }
        )])
    )])),
    Some(shape_table(&[])),
    None,
    check_self_only_snippet_places_the_cursor_inside_the_parens
);
completion_list_case!(
    anonymous_shapes_are_not_offered,
    "let ",
    Some(signature_table(vec![])),
    Some(shape_table(&["Player", "__anon__health_int__name_string"])),
    None,
    check_anonymous_shapes_are_hidden
);
completion_list_case!(
    public_may_block_intrinsics_are_offered_and_the_internal_one_is_not,
    "sleep",
    None,
    None,
    None,
    check_public_intrinsics_listed_internal_one_hidden
);

// ---------------------------------------------------------------------------
// cross_file_completion_items
// ---------------------------------------------------------------------------

macro_rules! cross_file_case {
    ($name:ident, $exported_label:expr) => {
        // bug: an exported symbol of another file is missing from completion, or its item lacks the one edit that inserts the import | tier: Medium
        #[test]
        fn $name() {
            let root = std::path::Path::new("/tmp/ynz_cross_completion");
            let exporter_source = "export shape Order { id: String }\nexport function processOrder(o: Order) -> Nothing { }\n";
            let importer_source = "function main() -> Nothing { }\n";

            let mut state = ServerState::new(PositionEncoding::Utf8);
            state.project_root = Some(root.to_path_buf());
            let exporter_uri =
                lsp_types::Url::from_file_path(root.join("services/orders.ynz")).unwrap();
            state.open_document(exporter_uri, exporter_source.to_string());
            let importer_uri = lsp_types::Url::from_file_path(root.join("entrypoint.ynz")).unwrap();
            state.open_document(importer_uri.clone(), importer_source.to_string());

            let items = cross_file_completion_items(
                &state,
                &importer_uri,
                importer_source,
                &LineTable::new(importer_source),
                None,
                None,
            );
            let item = items
                .iter()
                .find(|item| item.label == $exported_label)
                .unwrap_or_else(|| panic!("{} must be offered; got {} items", $exported_label, items.len()));
            let edits = item
                .additional_text_edits
                .as_ref()
                .unwrap_or_else(|| panic!("{} must carry an import edit", $exported_label));
            assert_eq!(edits.len(), 1);
            assert!(
                edits[0].new_text.contains("services/orders"),
                "import text must name the path; got {:?}",
                edits[0].new_text
            );
        }
    };
}

cross_file_case!(exported_shape_is_offered_with_its_import_edit, "Order");
cross_file_case!(
    exported_function_is_offered_with_its_import_edit,
    "processOrder"
);

// ---------------------------------------------------------------------------
// textDocument/completion over the wire
// ---------------------------------------------------------------------------

// bug: the completion request answers with an error or an empty result instead of the keyword list for the typed prefix | tier: Medium
#[test]
fn completion_request_returns_the_items_for_the_typed_prefix() {
    let harness = InProcessHarness::new().start_server();
    harness.initialize();
    harness.did_open("file:///completion_prefix.ynz", "fun");

    let result = harness.request(
        10,
        "textDocument/completion",
        json!({
            "textDocument": { "uri": "file:///completion_prefix.ynz" },
            "position": { "line": 0, "character": 3 }
        }),
    );

    let labels: Vec<&str> = result["items"]
        .as_array()
        .unwrap_or_else(|| panic!("completion result must be a list; got {result}"))
        .iter()
        .filter_map(|item| item["label"].as_str())
        .collect();
    assert!(labels.contains(&"function"), "got {labels:?}");
}
