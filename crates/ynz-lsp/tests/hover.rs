// WHY: Hover integration cases verify that the token-at-offset → registry /
// signature / AST pipeline produces the right hover body for each kind of
// symbol under the cursor, and none where there is nothing to explain.
// Bugs here show the wrong type, a keyword hover on a user variable, or
// nothing at all.
//
// Each case's source holds one `§` marker: the cursor sits right after it.
mod harness;
use harness::InProcessHarness;
use lsp_types::HoverContents;
use ynz_lsp::{capabilities::PositionEncoding, hover::hover_response, position::LineTable};
use ynz_parser::lexer::lex;
use ynz_parser::queries::parse_query;
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::queries::module_signatures_query;

const CURSOR: char = '§';

/// What a hover at the cursor must be.
enum Expect {
    /// No hover at all.
    Nothing,
    /// A registry keyword hover (its body opens with `## Keyword`).
    Keyword,
    /// A hover that is not a keyword hover and contains every listed text.
    Contains(&'static [&'static str]),
}

/// Hover body at the cursor of `marked_source`, through the real lex, parse and
/// signature queries; `None` when `hover_response` returns no hover.
fn hover_body(marked_source: &str) -> Option<String> {
    let cursor_offset = marked_source
        .find(CURSOR)
        .expect("source has a cursor marker");
    let source = marked_source.replacen(CURSOR, "", 1);
    let (tokens, _) = lex("test.ynz", &source);
    let table = LineTable::new(&source);
    let mut db = CompilerDb::default();
    let source_file = SourceFile::new(&db, "test.ynz".to_string(), source.clone());
    db.register_source(source_file);
    let parse = parse_query(&db, source_file);
    let signatures = module_signatures_query(&db, source_file);
    let hover = hover_response(
        &tokens,
        &signatures.sig_table,
        Some(&parse.module),
        &source,
        &table,
        cursor_offset,
        PositionEncoding::Utf8,
        Some(&signatures.shape_table),
        Some(&signatures.imported_options),
    )?;
    match hover.contents {
        HoverContents::Markup(markup) => Some(markup.value),
        other => panic!("expected Markup hover contents; got {other:?}"),
    }
}

macro_rules! hover_case {
    ($name:ident, $marked_source:expr, $expect:expr) => {
        // bug: hover shows the wrong type, a keyword hover or nothing for the symbol under the cursor, or a hover where there is nothing to explain | tier: Medium
        #[test]
        fn $name() {
            let body = hover_body($marked_source);
            match ($expect, body) {
                (Expect::Nothing, None) => {}
                (Expect::Nothing, Some(body)) => panic!("expected no hover; got {body:?}"),
                (_, None) => panic!("expected a hover; got none"),
                (Expect::Keyword, Some(body)) => {
                    assert!(
                        body.contains("## Keyword"),
                        "expected a keyword hover; got {body:?}"
                    )
                }
                (Expect::Contains(expected_texts), Some(body)) => {
                    assert!(
                        !body.contains("## Keyword"),
                        "unexpected keyword hover: {body:?}"
                    );
                    for expected_text in expected_texts {
                        assert!(
                            body.contains(expected_text),
                            "{expected_text:?} missing from {body:?}"
                        );
                    }
                }
            }
        }
    };
}

hover_case!(
    keyword_shows_the_registry_hover,
    "§function entrypoint() -> Nothing { }",
    Expect::Keyword
);
// `errors` outside any function body is a hard token, not an identifier use-site.
hover_case!(
    keyword_outside_a_function_body_still_shows_the_registry_hover,
    "§errors",
    Expect::Keyword
);
// Editors put the cursor on the byte after a token's last character.
hover_case!(
    cursor_at_the_end_of_a_token_resolves_that_token,
    "function§ entrypoint() -> Nothing { }",
    Expect::Keyword
);
hover_case!(
    cursor_in_the_whitespace_after_a_token_shows_nothing,
    "function entrypoint() ->§ Nothing { }",
    Expect::Nothing
);
hover_case!(
    cursor_in_whitespace_between_tokens_shows_nothing,
    "let  § x = 5",
    Expect::Nothing
);
hover_case!(
    cursor_in_a_comment_shows_nothing,
    "// this is §a comment",
    Expect::Nothing
);
hover_case!(cursor_in_an_empty_file_shows_nothing, "§", Expect::Nothing);
hover_case!(
    user_function_shows_its_parameter_and_return_types,
    "function §myFunc(n: Int) -> String { return `x` }",
    Expect::Contains(&["myFunc", "Int", "String"])
);
hover_case!(
    doc_comment_is_shown_above_the_signature,
    "/// Heals the player by amount.\nfunction §heal(amount: Int) -> Nothing { }",
    Expect::Contains(&["Heals the player by amount", "heal"])
);
hover_case!(
    function_without_a_doc_comment_still_shows_its_signature,
    "function §greet() -> Nothing { }",
    Expect::Contains(&["greet"])
);
// Registry intrinsics such as `range` carry `<>` return types inside markdown.
hover_case!(
    intrinsic_free_function_shows_a_hover,
    "§range",
    Expect::Contains(&["range"])
);
hover_case!(
    shape_name_at_a_use_site_shows_its_fields,
    "shape Bar {\n  symbol: String\n  open: Number\n}\nfunction insertBars(bars: §Bar) -> Nothing { }",
    Expect::Contains(&["shape Bar", "symbol", "open"])
);
hover_case!(
    options_name_at_a_use_site_shows_its_variants,
    "options Status {\n  active\n  inactive\n  banned: `Banned User`\n}\nfunction check(s: §Status) -> Nothing { }",
    Expect::Contains(&["options Status", "active", "inactive", "Banned User"])
);
// `share` is lexed as an identifier: a local named `share` must show its type,
// not the keyword hover and not nothing.
hover_case!(
    local_named_like_a_contextual_keyword_shows_its_type,
    "function entrypoint() -> Nothing {\n  let share: Int = 5\n  print(§share)\n}",
    Expect::Contains(&["Int"])
);
hover_case!(
    shadowing_inner_local_shows_a_binding_hover,
    "function entrypoint() -> Nothing {\n  let share: Int = 5\n  if (true) {\n    let share: Int = 99\n    print(§share)\n  }\n}",
    Expect::Contains(&["share"])
);
// An ownership modifier has no registry entry and is no variable use-site.
hover_case!(
    ownership_modifier_in_a_signature_shows_nothing,
    "function f(§share self: Int) -> Nothing { }",
    Expect::Nothing
);

// bug: the textDocument/hover request errors or returns null over a keyword instead of the hover from hover_response | tier: Medium
#[test]
fn hover_request_over_a_keyword_returns_a_hover() {
    let client = InProcessHarness::new().start_server();
    client.initialize();
    client.did_open(
        "file:///hover_test.ynz",
        "function entrypoint() -> Nothing { }",
    );
    let result = client.request(
        20,
        "textDocument/hover",
        serde_json::json!({
            "textDocument": { "uri": "file:///hover_test.ynz" },
            "position": { "line": 0, "character": 0 }
        }),
    );
    assert!(!result.is_null(), "hover over `function` must not be null");
}
