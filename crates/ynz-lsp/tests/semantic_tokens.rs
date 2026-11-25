// WHY: Semantic token tests keep the keyword classification in agreement with
// the TextMate grammar, and hold the request latency budgets (nightly only).

use ynz_lsp::{
    capabilities::PositionEncoding,
    position::LineTable,
    semantic_tokens::{semantic_tokens_full_response, semantic_tokens_range_response},
    state::ServerState,
};

fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path(path).expect("valid path");
    state.open_document(uri.clone(), src.to_string());
    (state, uri)
}

fn range_at(src: &str, needle: &str) -> lsp_types::Range {
    let offset = src.find(needle).expect("needle in src");
    let table = LineTable::new(src);
    let start = table.byte_offset_to_position(src, offset, PositionEncoding::Utf8);
    let end = table.byte_offset_to_position(src, offset + needle.len(), PositionEncoding::Utf8);
    lsp_types::Range { start, end }
}

// bug: semantic tokens disagree with the TextMate grammar on keywords, so a keyword flickers between keyword and variable colour | tier: Low
#[test]
fn keyword_agreement_with_tm_grammar() {
    // Semantic tokens refine the TextMate grammar's keyword colouring; they must not
    // disagree on keyword tokens. Identifier-type disagreement is expected (TM cannot
    // tell variables from functions).
    let src = "function greet() -> Nothing { return }\n";
    let (state, uri) = state_single("/tmp/ynz_st_tm.ynz", src);
    let tokens = semantic_tokens_full_response(&state, &uri).expect("tokens");

    // The Yinz keyword set: each entry matches the TM grammar's keyword.ynz scope.
    let yinz_keywords = [
        "function",
        "let",
        "const",
        "return",
        "if",
        "else",
        "while",
        "for",
        "in",
        "shape",
        "follows",
        "extends",
        "base",
        "hidden",
        "dynamic",
        "options",
        "is",
        "import",
        "export",
        "wait",
        "background",
        "sensitive",
        "errors",
        "true",
        "false",
        "nothing",
        "none",
        "self",
    ];

    let keyword_token_count = tokens
        .data
        .iter()
        .filter(|token| token.token_type == 0)
        .count();
    let keywords_in_source: Vec<_> = yinz_keywords
        .iter()
        .filter(|keyword| {
            src.split_whitespace().any(|word| {
                word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_') == **keyword
            })
        })
        .collect();

    assert!(
        keyword_token_count >= keywords_in_source.len(),
        "expected at least {} KEYWORD tokens; got {keyword_token_count}; keywords found: {keywords_in_source:?}",
        keywords_in_source.len(),
    );
}

// bug: full-file semantic tokens take over 100 ms on a 50-line file and make the editor lag on every keystroke | tier: Medium
#[test]
fn nightly_semantic_tokens_full_under_100ms() {
    let src = "let x = 42\n".repeat(50);
    let (state, uri) = state_single("/tmp/ynz_st_perf_full.ynz", &src);
    let start = std::time::Instant::now();
    let _ = semantic_tokens_full_response(&state, &uri);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 100,
        "semantic_tokens_full took {}ms; budget is 100ms",
        elapsed.as_millis()
    );
}

// bug: range semantic tokens take over 30 ms for a two-line viewport and make the editor lag while scrolling | tier: Medium
#[test]
fn nightly_semantic_tokens_range_under_30ms() {
    let src = "let x = 42\n".repeat(50);
    let (state, uri) = state_single("/tmp/ynz_st_perf_range.ynz", &src);
    let range = range_at(&src, "let x = 42\nlet x = 42");
    let start = std::time::Instant::now();
    let _ = semantic_tokens_range_response(&state, &uri, range);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 30,
        "semantic_tokens_range took {}ms; budget is 30ms",
        elapsed.as_millis()
    );
}
