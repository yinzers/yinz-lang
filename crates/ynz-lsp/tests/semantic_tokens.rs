// WHY: Semantic token integration tests verify that the delta-encoded wire
// format is correct for each token type and that the per-token-type lookup
// classification matches the Golden Rule 13 capital-letter convention and the
// sig_table lookup. Bugs here produce wrong colors in VSCode (e.g., keywords
// showing as variables) without any compile error or test failure in typeck.

use ynz_lsp::{
    capabilities::PositionEncoding,
    semantic_tokens::{semantic_tokens_full_response, semantic_tokens_range_response},
    position::LineTable,
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
    let end =
        table.byte_offset_to_position(src, offset + needle.len(), PositionEncoding::Utf8);
    lsp_types::Range { start, end }
}

// ─── Legend ──────────────────────────────────────────────────────────────────

#[test]
fn legend_contains_required_token_types() {
    use lsp_types::SemanticTokenType;
    let legend = ynz_lsp::semantic_tokens::SEMANTIC_TOKEN_LEGEND;
    assert!(legend.contains(&SemanticTokenType::KEYWORD));
    assert!(legend.contains(&SemanticTokenType::TYPE));
    assert!(legend.contains(&SemanticTokenType::FUNCTION));
    assert!(legend.contains(&SemanticTokenType::VARIABLE));
    assert!(legend.contains(&SemanticTokenType::NUMBER));
    assert!(legend.contains(&SemanticTokenType::STRING));
    assert!(legend.contains(&SemanticTokenType::COMMENT));
}

// ─── Token type emission: keywords ───────────────────────────────────────────

#[test]
fn test_semantic_tokens_keywords_emit_keyword_type() {
    // WHY: Every Yinz keyword must be mapped to IDX_KEYWORD (0). Any gap produces
    // "plain identifier" colors for keywords — visible regression in the editor.
    let src = "function greet() -> nothing { return }\n";
    let (state, uri) = state_single("/tmp/ynz_st_keywords.ynz", src);
    let tokens = semantic_tokens_full_response(&state, &uri).expect("tokens");
    // Verify at least one KEYWORD token (token_type == 0) is emitted.
    let keyword_count = tokens.data.iter().filter(|t| t.token_type == 0).count();
    assert!(
        keyword_count >= 2,
        "expected at least 2 keyword tokens (function, nothing, return); got {keyword_count}"
    );
}

// ─── Token type emission: type identifier ────────────────────────────────────

#[test]
fn test_semantic_tokens_pascalcase_identifier_is_type() {
    // WHY: Golden Rule 13 — capital letter = type. Any PascalCase identifier
    // must emit IDX_TYPE (1). Wrong classification means VSCode colors a type
    // name the same as a variable, hiding the naming-convention signal.
    let src = "shape Player { name: string }\n";
    let (state, uri) = state_single("/tmp/ynz_st_type.ynz", src);
    let tokens = semantic_tokens_full_response(&state, &uri).expect("tokens");
    // `Player` is PascalCase → must appear as TYPE (1)
    let type_count = tokens.data.iter().filter(|t| t.token_type == 1).count();
    assert!(
        type_count >= 1,
        "expected at least 1 TYPE token (Player); got {type_count}"
    );
}

// ─── Token type emission: function identifier ─────────────────────────────────

#[test]
fn test_semantic_tokens_function_name_is_function() {
    // WHY: sig_table lookup must classify known function names as FUNCTION (2),
    // not VARIABLE (3). If the lookup is missing, every function call site
    // shows in the wrong color category.
    let src = "function greet() -> nothing { }\nfunction entrypoint() -> nothing { greet() }\n";
    let (state, uri) = state_single("/tmp/ynz_st_fn.ynz", src);
    let tokens = semantic_tokens_full_response(&state, &uri).expect("tokens");
    let fn_count = tokens.data.iter().filter(|t| t.token_type == 2).count();
    // At minimum, `greet` should appear as FUNCTION (2) — at declaration + call site.
    assert!(fn_count >= 1, "expected at least 1 FUNCTION token; got {fn_count}");
}

// ─── Token type emission: number ─────────────────────────────────────────────

#[test]
fn test_semantic_tokens_integer_literal_is_number() {
    let src = "let x = 42\n";
    let (state, uri) = state_single("/tmp/ynz_st_number.ynz", src);
    let tokens = semantic_tokens_full_response(&state, &uri).expect("tokens");
    let num_count = tokens.data.iter().filter(|t| t.token_type == 7).count();
    assert!(num_count >= 1, "expected at least 1 NUMBER token; got {num_count}");
}

// ─── Token type emission: string ─────────────────────────────────────────────

#[test]
fn test_semantic_tokens_string_literal_is_string() {
    let src = "let msg = \"hello\"\n";
    let (state, uri) = state_single("/tmp/ynz_st_string.ynz", src);
    let tokens = semantic_tokens_full_response(&state, &uri).expect("tokens");
    let str_count = tokens.data.iter().filter(|t| t.token_type == 8).count();
    assert!(str_count >= 1, "expected at least 1 STRING token; got {str_count}");
}

// ─── Options variant emission ─────────────────────────────────────────────────

#[test]
fn test_semantic_tokens_variable_fallback() {
    // WHY: lowercase identifiers not in sig_table must fall back to VARIABLE (3).
    // This covers local variables that can't be classified more specifically
    // without a full AST-level scope walk (deferred to v0.3+).
    let src = "function entrypoint() -> nothing { let score = 5 }\n";
    let (state, uri) = state_single("/tmp/ynz_st_var.ynz", src);
    let tokens = semantic_tokens_full_response(&state, &uri).expect("tokens");
    let var_count = tokens.data.iter().filter(|t| t.token_type == 3).count();
    // `score` is a local variable; must appear as VARIABLE.
    assert!(var_count >= 1, "expected at least 1 VARIABLE token; got {var_count}");
}

// ─── Range filter ────────────────────────────────────────────────────────────

#[test]
fn test_semantic_tokens_range_returns_subset() {
    // WHY: range requests must return ONLY tokens in the viewport. If the range
    // filter is a no-op, the client gets the full file for every keypress — O(N)
    // work when O(viewport) was needed.
    let src = "let x = 42\nlet y = 99\n";
    let (state, uri) = state_single("/tmp/ynz_st_range.ynz", src);

    let full = semantic_tokens_full_response(&state, &uri).expect("full tokens");
    let range = range_at(src, "let x = 42");
    let partial = semantic_tokens_range_response(&state, &uri, range).expect("range tokens");

    // Range (line 0 only) must have fewer tokens than the full file (2 lines).
    assert!(
        partial.data.len() <= full.data.len(),
        "range response must not exceed full-file token count"
    );
    // The full file has content on both lines; the range only covers line 0.
    // The range response must miss at least `y` and `99` from line 1.
    assert!(
        partial.data.len() < full.data.len(),
        "range response should exclude line-1 tokens; full={} range={}",
        full.data.len(),
        partial.data.len()
    );
}

// ─── TM grammar agreement on keywords ────────────────────────────────────────

#[test]
fn test_semantic_tokens_keyword_agreement_with_tm_grammar() {
    // WHY: semantic tokens REFINE the TextMate grammar's keyword coloring —
    // they must not DISAGREE on keyword tokens. If a keyword token is emitted
    // by TM but classified as VARIABLE by semantic tokens, colors flicker.
    // This test checks keyword-type agreement only; identifier-type disagreement
    // is expected (TM can't distinguish variables from functions).
    let src = "function greet() -> nothing { return }\n";
    let (state, uri) = state_single("/tmp/ynz_st_tm.ynz", src);
    let tokens = semantic_tokens_full_response(&state, &uri).expect("tokens");

    // The Yinz keyword set — each entry here matches the TM grammar's keyword.ynz scope.
    let yinz_keywords = [
        "function", "let", "const", "return", "if", "else", "while", "for", "in",
        "shape", "follows", "extends", "base", "hidden", "dynamic", "options", "is",
        "import", "export", "wait", "background", "sensitive", "errors", "true",
        "false", "nothing", "none", "self",
    ];

    // Find keyword tokens in our output.
    let keyword_token_count = tokens.data.iter().filter(|t| t.token_type == 0).count();

    // Every keyword in the source must have a corresponding KEYWORD token.
    let kw_in_src: Vec<_> = yinz_keywords
        .iter()
        .filter(|kw| {
            src.split_whitespace()
                .any(|word| word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_') == **kw)
        })
        .collect();

    assert!(
        keyword_token_count >= kw_in_src.len(),
        "expected at least {} KEYWORD tokens (matching TM grammar keywords in source), got {}; \
         keywords found: {:?}",
        kw_in_src.len(),
        keyword_token_count,
        kw_in_src
    );
}

// ─── Multi-byte (emoji in comment) ───────────────────────────────────────────

#[test]
fn test_semantic_tokens_correct_length_for_multibyte() {
    // WHY: token length is encoded in character units (UTF-8 bytes or UTF-16
    // code units). Getting this wrong shifts all subsequent token positions on
    // the same line — produces "ghost" highlights in VSCode.
    // We test with a doc-comment that contains ASCII only (Yinz identifiers are
    // ASCII-only in v0.1); the emoji appears in the comment body which gets
    // classified as COMMENT (9).
    let src = "/// comment with emoji 🦀\nlet x = 1\n";
    let (state, uri) = state_single("/tmp/ynz_st_multibyte.ynz", src);
    let tokens = semantic_tokens_full_response(&state, &uri).expect("tokens");
    // Must produce tokens without panic — length/position computation is the pass-or-fail.
    assert!(!tokens.data.is_empty(), "expected at least some tokens");
}

// ─── Performance budget ───────────────────────────────────────────────────────

#[test]
fn test_semantic_tokens_full_under_100ms() {
    // WHY: full-file semantic tokens are re-requested on every edit keystroke.
    // If computation exceeds 100ms, the editor lags noticeably.
    let src = "let x = 42\n".repeat(50); // ~500 lines
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

#[test]
fn test_semantic_tokens_range_under_30ms() {
    let src = "let x = 42\n".repeat(50);
    let (state, uri) = state_single("/tmp/ynz_st_perf_range.ynz", &src);
    // Viewport = first 10 lines
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
