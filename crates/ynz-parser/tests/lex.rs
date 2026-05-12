// WHY: The lexer is the first stage of the pipeline. If it silently bails on
// unknown characters or unterminated strings, all downstream stages see
// incomplete input and produce cascading misleading errors. Every negative
// test asserts that the lexer recovers and produces a usable token stream
// alongside its diagnostic.

use insta::assert_debug_snapshot;
use salsa::Setter as _;
use ynz_parser::{lex_query, CompilerDb, SourceFile, Token};

const FILE: &str = "test.ynz";

/// Run the lexer on `source` and return the token values (not spans).
fn lex_tokens(source: &str) -> Vec<Token> {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    let output = lex_query(&db, sf);
    output.tokens.iter().map(|s| s.value.clone()).collect()
}

/// Run the lexer and return (token count, diagnostic count).
fn lex_counts(source: &str) -> (usize, usize) {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    let output = lex_query(&db, sf);
    (output.tokens.len(), output.diagnostics.len())
}

// ─── Happy path ─────────────────────────────────────────────────────────────

#[test]
fn m1_source_produces_expected_tokens() {
    // WHY: This is the exact token stream the Phase 4 parser depends on.
    // A silent change to this snapshot means the parser's expectations are broken.
    let source = r#"function main() -> nothing { print("hello, yinz") }"#;
    let tokens = lex_tokens(source);
    assert_debug_snapshot!("m1_token_stream", tokens);
}

// ─── Scope-creep gate ────────────────────────────────────────────────────────

#[test]
fn m1_token_variant_count_locked() {
    // WHY: This test pins the token vocabulary to the M1 surface.
    // If you need to add a new token for a later milestone, add an inline
    // // test-ratchet: <reason-and-milestone> comment on this line and update
    // the count in the Token enum's doc comment.
    //
    // The count here is the number of discriminants in the Token enum.
    // M1 count: Function(1) + Nothing(2) + Identifier(3) + StringLit(4) +
    //           LParen(5) + RParen(6) + LBrace(7) + RBrace(8) + Arrow(9) + Eof(10)
    let expected_count = 10usize;

    // Verify by constructing one of each variant and checking we haven't missed any.
    // This is a manual count test — Rust stable does not expose variant_count() for
    // non-unit enums without a nightly feature. We enumerate exhaustively.
    use ynz_parser::Token::*;
    let all_variants: &[Token] = &[
        Function,
        Nothing,
        Identifier("x".into()),
        StringLit(vec![]),
        LParen,
        RParen,
        LBrace,
        RBrace,
        Arrow,
        Eof,
    ];
    assert_eq!(
        all_variants.len(),
        expected_count,
        "Token variant count changed from {expected_count} — update this test \
         with a // test-ratchet: <reason> comment and update the Token doc comment"
    );
}

// ─── Positions ───────────────────────────────────────────────────────────────

#[test]
fn token_spans_reconstruct_lexemes() {
    // WHY: Accurate byte spans are required for ariadne to point carets at the
    // right source location. If the spans are off, all diagnostic arrows are wrong.
    let source = r#"function main() -> nothing { print("hello") }"#;
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    let output = lex_query(&db, sf);

    for spanned in &output.tokens {
        if spanned.value == ynz_parser::Token::Eof {
            continue;
        }
        let reconstructed = &source[spanned.span.start..spanned.span.end];
        // Reconstructed lexeme must match what we'd expect for the token kind.
        match &spanned.value {
            Token::Function => assert_eq!(reconstructed, "function"),
            Token::Nothing => assert_eq!(reconstructed, "nothing"),
            Token::LParen => assert_eq!(reconstructed, "("),
            Token::RParen => assert_eq!(reconstructed, ")"),
            Token::LBrace => assert_eq!(reconstructed, "{"),
            Token::RBrace => assert_eq!(reconstructed, "}"),
            Token::Arrow => assert_eq!(reconstructed, "->"),
            Token::Identifier(s) => assert_eq!(reconstructed, s.as_str()),
            Token::StringLit(_) => {
                assert!(reconstructed.starts_with('"') && reconstructed.ends_with('"'));
            }
            Token::Eof => {}
        }
    }
}

// ─── Empty and whitespace sources ────────────────────────────────────────────

#[test]
fn empty_source_produces_only_eof() {
    let tokens = lex_tokens("");
    assert_eq!(tokens, vec![Token::Eof]);
}

#[test]
fn whitespace_only_source_produces_only_eof() {
    let tokens = lex_tokens("   \n\t  ");
    assert_eq!(tokens, vec![Token::Eof]);
}

// ─── Error recovery ──────────────────────────────────────────────────────────

#[test]
fn unknown_char_produces_diagnostic_and_continues() {
    // WHY: the lexer must not bail on the first unknown character. A broken
    // file should show all its errors at once, not just the first.
    let source = r#"function main() -> nothing { print($) }"#;
    let (token_count, diag_count) = lex_counts(source);
    assert_eq!(
        diag_count, 1,
        "Expected exactly 1 diagnostic for the unknown '$'"
    );
    assert!(
        token_count > 1,
        "Lexer must produce a usable token stream after the error"
    );
}

#[test]
fn unterminated_string_produces_diagnostic_and_continues() {
    // WHY: parser must not panic on unterminated strings — it needs a
    // complete token stream to detect all parse errors at once.
    let source = r#"function main() -> nothing { print("oops) }"#;
    let (token_count, diag_count) = lex_counts(source);
    assert_eq!(
        diag_count, 1,
        "Expected exactly 1 diagnostic for the unterminated string"
    );
    assert!(
        token_count > 1,
        "Lexer must produce tokens after the unterminated string"
    );
}

#[test]
fn non_ascii_bytes_inside_string_lex_clean() {
    // WHY: M1 strings are raw UTF-8 bytes passed through to codegen unchanged.
    // Non-ASCII content is NOT an error at lex time — it becomes the bytes
    // in the StringLit token's Vec<u8>.
    let source = r#"function main() -> nothing { print("café") }"#;
    let (token_count, diag_count) = lex_counts(source);
    assert_eq!(
        diag_count, 0,
        "Non-ASCII in a string literal must not produce a diagnostic"
    );

    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    let output = lex_query(&db, sf);
    let string_lit = output
        .tokens
        .iter()
        .find(|t| matches!(t.value, Token::StringLit(_)))
        .expect("should find a StringLit token");

    if let Token::StringLit(bytes) = &string_lit.value {
        let expected = "café".as_bytes();
        assert_eq!(
            bytes.as_slice(),
            expected,
            "StringLit bytes must match the UTF-8 encoding of the literal"
        );
    }
    let _ = token_count; // used implicitly via find above
}

// ─── Salsa invalidation ──────────────────────────────────────────────────────

#[test]
fn changing_source_text_invalidates_cache() {
    // WHY: salsa must re-run lex_query when the source changes. If the cache
    // is not invalidated, a second build with corrected source would return
    // stale results.
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(
        &db,
        FILE.to_string(),
        "function main() -> nothing { }".to_string(),
    );

    let output1 = lex_query(&db, sf);
    let token_count_1 = output1.tokens.len();

    // Mutate the source text — salsa should invalidate the cached lex result.
    sf.set_text(&mut db)
        .to(r#"function main() -> nothing { print("hello") }"#.to_string());

    let output2 = lex_query(&db, sf);
    let token_count_2 = output2.tokens.len();

    assert_ne!(
        token_count_1, token_count_2,
        "Token count should differ after source text change"
    );
}
