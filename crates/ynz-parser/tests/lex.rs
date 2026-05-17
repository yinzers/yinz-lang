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

/// Run the lexer and return just the diagnostics.
fn lex_diags(source: &str) -> Vec<ynz_diagnostics::Diagnostic> {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    let output = lex_query(&db, sf);
    output.diagnostics.clone()
}


#[test]
fn m4_token_variant_count_locked() {
    // WHY: This test pins the token vocabulary to the M4 surface.
    // If you need to add a new token for a later milestone, add an inline
    // `// test-ratchet: <reason-and-milestone>` comment on this test and update
    // the count in the Token enum's doc comment.
    //
    // test-ratchet: M2 adds 32 variants over M1's 10:
    //   Keywords (4):   Let, Const, True, False
    //   Literals (2):   IntLit, NumberLit  (FloatLit removed — per pre-phase decision)
    //   Arithmetic (5): Plus, Minus, Star, Slash, Percent
    //   Comparison (6): EqEq, NotEq, Lt, LtEq, Gt, GtEq
    //   Boolean (3):    AmpAmp, PipePipe, Bang
    //   Bitwise (6):    Amp, Pipe, Caret, Tilde, LtLt, GtGt
    //   Punctuation (2): Eq, Colon
    //   Plumbing (4):   Dot, LBracket, RBracket, Comma  (needed by P3 within M2 scope)
    //   Total M1+M2: 10 + 32 = 42
    //
    // test-ratchet: M3 adds 7 variants over M2's 42:
    //   Keywords (6):    If, Else, While, For, In, Return
    //   Punctuation (1): FatArrow (`=>`)
    //   Total M1+M2+M3: 42 + 7 = 49
    //
    // test-ratchet: M4 adds 8 variants over M3's 49:
    //   Shape-system keywords (8): Shape, Follows, Extends, Base, Hidden, Dynamic,
    //                              SelfType (capital Self), SelfValue (lowercase self)
    //   Note: Override was NOT added — the non-OOP model removed it;
    //         function overloading by argument type replaces it.
    //   Total M1+M2+M3+M4: 49 + 8 = 57
    let expected_count = 57usize;

    use ynz_parser::Token::*;
    let all_variants: &[Token] = &[
        // M1
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
        // M2
        Let,
        Const,
        True,
        False,
        IntLit(0),
        NumberLit("0.0".into()),
        Plus,
        Minus,
        Star,
        Slash,
        Percent,
        EqEq,
        NotEq,
        Lt,
        LtEq,
        Gt,
        GtEq,
        AmpAmp,
        PipePipe,
        Bang,
        Amp,
        Pipe,
        Caret,
        Tilde,
        LtLt,
        GtGt,
        Eq,
        Colon,
        Dot,
        LBracket,
        RBracket,
        Comma,
        // M3
        If,
        Else,
        While,
        For,
        In,
        Return,
        FatArrow,
        // M4
        Shape,
        Follows,
        Extends,
        Base,
        Hidden,
        Dynamic,
        SelfType,
        SelfValue,
    ];
    assert_eq!(
        all_variants.len(),
        expected_count,
        "Token variant count changed from {expected_count} — update this test \
         with a // test-ratchet: <reason> comment and update the Token doc comment"
    );
}


#[test]
fn m5_token_variant_count_locked() {
    // WHY: pins the token vocabulary at the M5 P1 surface.
    //
    // test-ratchet: M5 P1 adds 1 variant over M4's 57:
    //   Keyword (1): None (for the `none` literal value of `maybe<T>`)
    //   Total M1+M2+M3+M4+M5: 57 + 1 = 58
    let expected_count = 58usize;

    use ynz_parser::Token::*;
    let all_variants: &[Token] = &[
        // M1
        Function, Nothing, Identifier("x".into()), StringLit(vec![]),
        LParen, RParen, LBrace, RBrace, Arrow, Eof,
        // M2
        Let, Const, True, False, IntLit(0), NumberLit("0.0".into()),
        Plus, Minus, Star, Slash, Percent,
        EqEq, NotEq, Lt, LtEq, Gt, GtEq,
        AmpAmp, PipePipe, Bang,
        Amp, Pipe, Caret, Tilde, LtLt, GtGt,
        Eq, Colon, Dot, LBracket, RBracket, Comma,
        // M3
        If, Else, While, For, In, Return, FatArrow,
        // M4
        Shape, Follows, Extends, Base, Hidden, Dynamic, SelfType, SelfValue,
        // M5
        None,
    ];
    assert_eq!(
        all_variants.len(),
        expected_count,
        "Token variant count changed from {expected_count} — update this test \
         with a // test-ratchet: <reason> comment and update the Token doc comment"
    );
}


#[test]
fn m1_source_produces_expected_tokens() {
    // WHY: The M2 lexer must not break any M1 token. A regression here means
    // something in the M2 extension changed an existing token's behaviour.
    let source = r#"function main() -> nothing { print("hello, yinz") }"#;
    let tokens = lex_tokens(source);
    assert_debug_snapshot!("m1_token_stream", tokens);
}


#[test]
fn m2_source_produces_expected_tokens() {
    // WHY: This is the exact token stream Phase 3 (parser) depends on for the
    // M2 smoke test. A silent change here means the parser's expectations break.
    let source = r#"function main() -> nothing {
  let price = 0.1 + 0.2
  let count: int = 42
  let active = true
  print(price)
  print(count * count - 1)
  print(active && (count > 0))
}"#;
    let tokens = lex_tokens(source);
    assert_debug_snapshot!("m2_token_stream", tokens);
}


#[test]
fn decimal_integer_literal() {
    // WHY: `42` must produce exactly IntLit(42), not a number or string.
    assert_eq!(lex_tokens("42"), vec![Token::IntLit(42), Token::Eof]);
}

#[test]
fn decimal_integer_with_underscores() {
    // WHY: Underscores are visual separators — `1_000_000` must equal 1000000.
    assert_eq!(
        lex_tokens("1_000_000"),
        vec![Token::IntLit(1_000_000), Token::Eof]
    );
}

#[test]
fn hex_integer_literal() {
    // WHY: `0xFF` must parse as 255 — the hex encoding of the same integer.
    assert_eq!(lex_tokens("0xFF"), vec![Token::IntLit(255), Token::Eof]);
    assert_eq!(lex_tokens("0xff"), vec![Token::IntLit(255), Token::Eof]);
    assert_eq!(
        lex_tokens("0xDEAD_BEEF"),
        vec![Token::IntLit(0xDEAD_BEEF), Token::Eof]
    );
}

#[test]
fn binary_integer_literal() {
    // WHY: `0b1010` must parse as 10 — the binary encoding of the integer.
    assert_eq!(lex_tokens("0b1010"), vec![Token::IntLit(10), Token::Eof]);
    assert_eq!(
        lex_tokens("0b1111_0000"),
        vec![Token::IntLit(0b1111_0000), Token::Eof]
    );
}


#[test]
fn decimal_number_literal_with_dot() {
    // WHY: `3.14` must produce NumberLit, not IntLit — the dot signals decimal.
    assert_eq!(
        lex_tokens("3.14"),
        vec![Token::NumberLit("3.14".into()), Token::Eof]
    );
}

#[test]
fn decimal_number_literal_scientific() {
    // WHY: `1e5` and `2.5e-3` are number literals — the exponent signals decimal.
    assert_eq!(
        lex_tokens("1e5"),
        vec![Token::NumberLit("1e5".into()), Token::Eof]
    );
    assert_eq!(
        lex_tokens("2.5e-3"),
        vec![Token::NumberLit("2.5e-3".into()), Token::Eof]
    );
}

#[test]
fn number_literal_underscores_stripped() {
    // WHY: Underscores in a number literal must be removed from the stored string.
    assert_eq!(
        lex_tokens("1_000.5"),
        vec![Token::NumberLit("1000.5".into()), Token::Eof]
    );
}

#[test]
fn integer_followed_by_dot_method_call() {
    // WHY: `42.toString()` must NOT consume the dot as part of the int literal.
    // The dot is a method call separator. If the lexer consumed it, `.toString()`
    // would be unreachable and method calls on int literals would be impossible.
    let tokens = lex_tokens("42.toString()");
    assert_eq!(tokens[0], Token::IntLit(42));
    assert_eq!(tokens[1], Token::Dot);
    assert_eq!(tokens[2], Token::Identifier("toString".into()));
}


#[test]
fn true_and_false_keywords() {
    // WHY: `true` and `false` must produce keyword tokens, not identifiers.
    assert_eq!(lex_tokens("true"), vec![Token::True, Token::Eof]);
    assert_eq!(lex_tokens("false"), vec![Token::False, Token::Eof]);
}


#[test]
fn two_char_operators_beat_single_char() {
    // WHY: Multi-char operators must be greedily matched — `==` must not become
    // `=` then `=`, and `<=` must not become `<` then `=`. If this fails,
    // every comparison in the language misparses.
    assert_eq!(lex_tokens("=="), vec![Token::EqEq, Token::Eof]);
    assert_eq!(lex_tokens("!="), vec![Token::NotEq, Token::Eof]);
    assert_eq!(lex_tokens("<="), vec![Token::LtEq, Token::Eof]);
    assert_eq!(lex_tokens(">="), vec![Token::GtEq, Token::Eof]);
    assert_eq!(lex_tokens("&&"), vec![Token::AmpAmp, Token::Eof]);
    assert_eq!(lex_tokens("||"), vec![Token::PipePipe, Token::Eof]);
    assert_eq!(lex_tokens("<<"), vec![Token::LtLt, Token::Eof]);
    assert_eq!(lex_tokens(">>"), vec![Token::GtGt, Token::Eof]);
}

#[test]
fn arrow_still_works() {
    // WHY: `->` must still produce Arrow in M2. The `-` arm checks for `>`
    // first so `->` beats the new `Minus` token.
    assert_eq!(
        lex_tokens("-> nothing"),
        vec![Token::Arrow, Token::Nothing, Token::Eof]
    );
}

#[test]
fn minus_without_gt_is_minus() {
    // WHY: A bare `-` must produce Minus, not Arrow. If the `->` check
    // accidentally consumes `-` without `>`, subtraction breaks.
    assert_eq!(
        lex_tokens("a - b"),
        vec![
            Token::Identifier("a".into()),
            Token::Minus,
            Token::Identifier("b".into()),
            Token::Eof
        ]
    );
}


#[test]
fn line_comment_is_skipped() {
    // WHY: `//` comments are not tokens. They must be invisible to the parser.
    // If a comment produces a token, the parser sees garbage after valid code.
    assert_eq!(
        lex_tokens("let x = 1 // this is a comment\nlet y = 2"),
        vec![
            Token::Let,
            Token::Identifier("x".into()),
            Token::Eq,
            Token::IntLit(1),
            Token::Let,
            Token::Identifier("y".into()),
            Token::Eq,
            Token::IntLit(2),
            Token::Eof
        ]
    );
}

#[test]
fn comment_at_end_of_file_does_not_panic() {
    // WHY: A comment on the last line (no trailing newline) must not read past EOF.
    let tokens = lex_tokens("let x = 1 // trailing comment");
    assert_eq!(tokens.last(), Some(&Token::Eof));
    let (_, diag_count) = lex_counts("let x = 1 // trailing comment");
    assert_eq!(diag_count, 0);
}

#[test]
fn slash_not_followed_by_slash_is_division() {
    // WHY: A single `/` must remain a Slash (division) token. Only `//`
    // triggers comment mode. If `/` alone becomes a comment, division is gone.
    assert_eq!(
        lex_tokens("a / b"),
        vec![
            Token::Identifier("a".into()),
            Token::Slash,
            Token::Identifier("b".into()),
            Token::Eof
        ]
    );
}


#[test]
fn two_dots_in_literal_produces_diagnostic() {
    // WHY: `1.2.3` is not a valid number. The lexer must emit a diagnostic at
    // the second dot and recover, not silently produce a wrong token.
    let (_, diag_count) = lex_counts("1.2.3");
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for `1.2.3`");

    let diags = lex_diags("1.2.3");
    assert!(
        diags[0].what.contains("decimal point"),
        "Diagnostic should mention 'decimal point', got: {:?}",
        diags[0].what
    );
}

#[test]
fn adjacent_underscores_produce_diagnostic() {
    // WHY: `1__000` is a formatting mistake. The lexer must catch it with a
    // teaching diagnostic rather than silently accepting or panicking.
    let (_, diag_count) = lex_counts("1__000");
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for `1__000`");

    let diags = lex_diags("1__000");
    assert!(
        diags[0].what.contains("adjacent"),
        "Diagnostic should mention 'adjacent', got: {:?}",
        diags[0].what
    );
}

#[test]
fn trailing_underscore_produces_diagnostic() {
    // WHY: `1_` is a formatting mistake. The trailing underscore has no visual
    // meaning and the lexer must reject it with a teaching diagnostic.
    let (_, diag_count) = lex_counts("1_");
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for `1_`");
}

#[test]
fn invalid_hex_digit_produces_diagnostic() {
    // WHY: `0xZZ` contains non-hex characters. The lexer must identify the bad
    // character with a span pointing directly at it.
    let (_, diag_count) = lex_counts("0xZZ");
    assert!(diag_count >= 1, "Expected at least 1 diagnostic for `0xZZ`");

    let diags = lex_diags("0xZZ");
    assert!(
        diags[0].what.contains("hex"),
        "Diagnostic should mention 'hex', got: {:?}",
        diags[0].what
    );
}

#[test]
fn invalid_binary_digit_produces_diagnostic() {
    // WHY: `0b22` contains non-binary digits. The lexer must teach the user
    // that binary literals only allow `0` and `1`.
    let (_, diag_count) = lex_counts("0b22");
    assert!(diag_count >= 1, "Expected at least 1 diagnostic for `0b22`");

    let diags = lex_diags("0b22");
    assert!(
        diags[0].what.contains("binary"),
        "Diagnostic should mention 'binary', got: {:?}",
        diags[0].what
    );
}

#[test]
fn integer_overflow_produces_diagnostic() {
    // WHY: Integers larger than i64::MAX must be caught at lex time with a
    // clear message pointing at `number` as the alternative type.
    let too_big = "99999999999999999999999";
    let (_, diag_count) = lex_counts(too_big);
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for overflow literal");

    let diags = lex_diags(too_big);
    assert!(
        diags[0].what.contains("int"),
        "Diagnostic should mention 'int', got: {:?}",
        diags[0].what
    );
}


#[test]
fn compound_assignment_plus_eq_produces_diagnostic() {
    // WHY: `+=` is not supported. The diagnostic must teach the user to write
    // `x = x + n` instead. If this passes silently, the parse layer gets confused.
    let (_, diag_count) = lex_counts("x += 1");
    assert_eq!(diag_count, 1, "Expected 1 diagnostic for `+=`");

    let diags = lex_diags("x += 1");
    assert!(
        diags[0].what.contains("+="),
        "Diagnostic should name the operator, got: {:?}",
        diags[0].what
    );
}

#[test]
fn increment_plus_plus_produces_diagnostic() {
    // WHY: `++` is not supported. Every mutation must be explicit: `x = x + 1`.
    let (_, diag_count) = lex_counts("x++");
    assert_eq!(diag_count, 1, "Expected 1 diagnostic for `++`");
}

#[test]
fn compound_assignment_minus_eq_produces_diagnostic() {
    let (_, diag_count) = lex_counts("x -= 1");
    assert_eq!(diag_count, 1, "Expected 1 diagnostic for `-=`");
}

#[test]
fn decrement_minus_minus_produces_diagnostic() {
    let (_, diag_count) = lex_counts("x--");
    assert_eq!(diag_count, 1, "Expected 1 diagnostic for `--`");
}

#[test]
fn compound_assignment_star_eq_produces_diagnostic() {
    let (_, diag_count) = lex_counts("x *= 2");
    assert_eq!(diag_count, 1, "Expected 1 diagnostic for `*=`");
}

#[test]
fn compound_assignment_slash_eq_produces_diagnostic() {
    let (_, diag_count) = lex_counts("x /= 2");
    assert_eq!(diag_count, 1, "Expected 1 diagnostic for `/=`");
}

#[test]
fn compound_assignment_percent_eq_produces_diagnostic() {
    let (_, diag_count) = lex_counts("x %= 2");
    assert_eq!(diag_count, 1, "Expected 1 diagnostic for `%=`");
}


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
            // M2/M3 — the source above does not contain these; span check is trivially true
            // by exhaustiveness (the compiler requires all arms).
            Token::Let
            | Token::Const
            | Token::True
            | Token::False
            | Token::IntLit(_)
            | Token::NumberLit(_)
            | Token::Plus
            | Token::Minus
            | Token::Star
            | Token::Slash
            | Token::Percent
            | Token::EqEq
            | Token::NotEq
            | Token::Lt
            | Token::LtEq
            | Token::Gt
            | Token::GtEq
            | Token::AmpAmp
            | Token::PipePipe
            | Token::Bang
            | Token::Amp
            | Token::Pipe
            | Token::Caret
            | Token::Tilde
            | Token::LtLt
            | Token::GtGt
            | Token::Eq
            | Token::Colon
            | Token::Dot
            | Token::LBracket
            | Token::RBracket
            | Token::Comma
            | Token::If
            | Token::Else
            | Token::While
            | Token::For
            | Token::In
            | Token::Return
            | Token::FatArrow
            | Token::Shape
            | Token::Follows
            | Token::Extends
            | Token::Base
            | Token::Hidden
            | Token::Dynamic
            | Token::SelfType
            | Token::SelfValue
            | Token::None => {
                // Span must be non-empty for every token that has a source location.
                assert!(
                    spanned.span.start < spanned.span.end,
                    "Token {:?} has empty span",
                    spanned.value
                );
            }
        }
    }
}

#[test]
fn m2_token_spans_cover_source() {
    // WHY: M2 tokens must have accurate spans so ariadne can point at the right
    // character for every error involving an operator, literal, or keyword.
    let source = "let x: int = 42 + 0xFF";
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    let output = lex_query(&db, sf);

    for spanned in &output.tokens {
        if spanned.value == Token::Eof {
            continue;
        }
        let reconstructed = &source[spanned.span.start..spanned.span.end];
        match &spanned.value {
            Token::Let => assert_eq!(reconstructed, "let"),
            Token::Colon => assert_eq!(reconstructed, ":"),
            Token::Eq => assert_eq!(reconstructed, "="),
            Token::IntLit(42) => assert_eq!(reconstructed, "42"),
            Token::IntLit(255) => assert_eq!(reconstructed, "0xFF"),
            Token::Plus => assert_eq!(reconstructed, "+"),
            Token::Identifier(s) => assert_eq!(reconstructed, s.as_str()),
            _ => {} // other M2 tokens not present in this source
        }
    }
}


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


#[test]
fn unknown_char_produces_diagnostic_and_continues() {
    // WHY: The lexer must not bail on the first unknown character. A broken
    // file should show all its errors at once, not just the first.
    let source = r#"function main() -> nothing { print($) }"#;
    let (token_count, diag_count) = lex_counts(source);
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for the unknown '$'");
    assert!(
        token_count > 1,
        "Lexer must produce a usable token stream after the error"
    );
}

#[test]
fn unterminated_string_produces_diagnostic_and_continues() {
    // WHY: Parser must not panic on unterminated strings — it needs a
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
    // WHY: M1/M2 strings are raw UTF-8 bytes passed through to codegen unchanged.
    // Non-ASCII content is NOT an error at lex time.
    let source = r#"function main() -> nothing { print("café") }"#;
    let (token_count, diag_count) = lex_counts(source);
    assert_eq!(diag_count, 0, "Non-ASCII in a string literal must not produce a diagnostic");

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
    let _ = token_count;
}


#[test]
fn m3_keywords_lex_correctly() {
    // WHY: All six M3 control-flow keywords must produce their dedicated token,
    // not fall through to Identifier. If any keyword becomes an identifier,
    // the parser can never recognize that construct and every M3 control-flow
    // test fails with a confusing "expected statement" error.
    assert_eq!(lex_tokens("if"), vec![Token::If, Token::Eof]);
    assert_eq!(lex_tokens("else"), vec![Token::Else, Token::Eof]);
    assert_eq!(lex_tokens("while"), vec![Token::While, Token::Eof]);
    assert_eq!(lex_tokens("for"), vec![Token::For, Token::Eof]);
    assert_eq!(lex_tokens("in"), vec![Token::In, Token::Eof]);
    assert_eq!(lex_tokens("return"), vec![Token::Return, Token::Eof]);
}

#[test]
fn fat_arrow_lexes_as_fat_arrow() {
    // WHY: `=>` is the multi-case arm separator. If it lexes as `=` + `>` the
    // parser sees two separate tokens and cannot parse any multi-case `if` arm.
    assert_eq!(lex_tokens("=>"), vec![Token::FatArrow, Token::Eof]);
}

#[test]
fn fat_arrow_distinct_from_eq_and_gt() {
    // WHY: Three closely-related token sequences — `=`, `>`, `=>` — must all
    // lex correctly. A greedy-match bug that turns `=` alone into `FatArrow`
    // would break every assignment. A bug that turns `=>` into `Eq` + `Gt`
    // would break every multi-case arm.
    assert_eq!(lex_tokens("="), vec![Token::Eq, Token::Eof]);
    assert_eq!(lex_tokens(">"), vec![Token::Gt, Token::Eof]);
    assert_eq!(lex_tokens("=>"), vec![Token::FatArrow, Token::Eof]);
    assert_eq!(lex_tokens("=="), vec![Token::EqEq, Token::Eof]);
    // `=>` inside a realistic multi-case arm context
    assert_eq!(
        lex_tokens("1 => 2"),
        vec![
            Token::IntLit(1),
            Token::FatArrow,
            Token::IntLit(2),
            Token::Eof,
        ]
    );
}

// ── M4 keyword tests ────────────────────────────────────────────────────────

#[test]
fn shape_keyword_lexes_correctly() {
    // WHY: `shape` is the M4 declaration keyword for data types. If it falls
    // through to Identifier, every shape declaration will fail to parse in P2.
    assert_eq!(lex_tokens("shape"), vec![Token::Shape, Token::Eof]);
}

#[test]
fn follows_keyword_lexes_correctly() {
    // WHY: `follows` declares that a shape satisfies a contract's signatures.
    assert_eq!(lex_tokens("follows"), vec![Token::Follows, Token::Eof]);
}

#[test]
fn extends_keyword_lexes_correctly() {
    // WHY: `extends` is data-only inheritance — child shape inherits parent fields.
    assert_eq!(lex_tokens("extends"), vec![Token::Extends, Token::Eof]);
}

#[test]
fn base_keyword_lexes_correctly() {
    // WHY: `base` marks a shape that cannot be instantiated directly.
    assert_eq!(lex_tokens("base"), vec![Token::Base, Token::Eof]);
}

#[test]
fn hidden_keyword_lexes_correctly() {
    // WHY: `hidden` marks a field as visible only within the declaring shape's functions.
    assert_eq!(lex_tokens("hidden"), vec![Token::Hidden, Token::Eof]);
}

#[test]
fn dynamic_keyword_lexes_correctly() {
    // WHY: `dynamic` opts into runtime polymorphism via fat pointer + vtable.
    assert_eq!(lex_tokens("dynamic"), vec![Token::Dynamic, Token::Eof]);
}

#[test]
fn self_type_keyword_lexes_correctly() {
    // WHY: `Self` (capital S) is the type keyword for the enclosing shape's concrete type
    // (used in contract method signatures). Must be distinct from `self` (lowercase).
    assert_eq!(lex_tokens("Self"), vec![Token::SelfType, Token::Eof]);
}

#[test]
fn self_value_keyword_lexes_correctly() {
    // WHY: `self` (lowercase) is the instance value keyword. Must be distinct from
    // `Self` (SelfType). Case-sensitive lexer already handles this by exact string match.
    assert_eq!(lex_tokens("self"), vec![Token::SelfValue, Token::Eof]);
}

#[test]
fn self_type_and_self_value_are_distinct() {
    // WHY: Golden Rule 13 — capital letter = type; lowercase = everything else.
    // `Self` is the type; `self` is the value. They must lex to different tokens so
    // the parser can enforce where each is valid.
    let self_type = lex_tokens("Self");
    let self_value = lex_tokens("self");
    assert_ne!(self_type, self_value, "Self (type) and self (value) must be distinct tokens");
    assert_eq!(self_type, vec![Token::SelfType, Token::Eof]);
    assert_eq!(self_value, vec![Token::SelfValue, Token::Eof]);
}

#[test]
fn m4_keywords_do_not_interfere_with_m3_keywords() {
    // WHY: M4 keywords are new identifiers — they must not accidentally shadow or
    // conflict with M3 keywords (if, else, while, for, in, return). Regression gate.
    assert_eq!(lex_tokens("if"), vec![Token::If, Token::Eof]);
    assert_eq!(lex_tokens("else"), vec![Token::Else, Token::Eof]);
    assert_eq!(lex_tokens("while"), vec![Token::While, Token::Eof]);
    assert_eq!(lex_tokens("for"), vec![Token::For, Token::Eof]);
    assert_eq!(lex_tokens("in"), vec![Token::In, Token::Eof]);
    assert_eq!(lex_tokens("return"), vec![Token::Return, Token::Eof]);
}

// ── M4 banned declaration keyword tests ─────────────────────────────────────

#[test]
fn type_keyword_produces_teaching_diagnostic() {
    // WHY: A user coming from TypeScript may write `type Player { ... }`.
    // The lexer redirects to `shape` with a three-part diagnostic. Token stream
    // must still emit Identifier("type") so the parser can recover rather than
    // cascading into meaningless parse errors.
    let (_, diag_count) = lex_counts("type");
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for `type`");

    let diags = lex_diags("type");
    assert!(
        diags[0].what.contains("type"),
        "Diagnostic should name the keyword, got: {:?}", diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("shape"),
        "Diagnostic should point to `shape`, got: {:?}", diags[0].what_instead
    );

    let tokens = lex_tokens("type");
    assert!(
        tokens.contains(&Token::Identifier("type".into())),
        "Lexer must emit Identifier(\"type\") for parser recovery"
    );
}

#[test]
fn struct_keyword_produces_teaching_diagnostic() {
    // WHY: A user coming from Rust/C may write `struct Player { ... }`.
    // Same teaching pattern — one diagnostic, identifier token for recovery.
    let (_, diag_count) = lex_counts("struct");
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for `struct`");

    let diags = lex_diags("struct");
    assert!(
        diags[0].what.contains("struct"),
        "Diagnostic should name the keyword, got: {:?}", diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("shape"),
        "Diagnostic should point to `shape`, got: {:?}", diags[0].what_instead
    );

    let tokens = lex_tokens("struct");
    assert!(
        tokens.contains(&Token::Identifier("struct".into())),
        "Lexer must emit Identifier(\"struct\") for parser recovery"
    );
}

#[test]
fn class_keyword_produces_teaching_diagnostic() {
    // WHY: A user coming from Java/Swift may write `class Player { ... }`.
    // The diagnostic must explicitly mention non-OOP framing — `shape` for data,
    // standalone functions for behavior.
    let (_, diag_count) = lex_counts("class");
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for `class`");

    let diags = lex_diags("class");
    assert!(
        diags[0].what.contains("class"),
        "Diagnostic should name the keyword, got: {:?}", diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("shape"),
        "Diagnostic should point to `shape`, got: {:?}", diags[0].what_instead
    );

    let tokens = lex_tokens("class");
    assert!(
        tokens.contains(&Token::Identifier("class".into())),
        "Lexer must emit Identifier(\"class\") for parser recovery"
    );
}

#[test]
fn interface_keyword_produces_teaching_diagnostic() {
    // WHY: A user coming from TypeScript/Java may write `interface Damageable { ... }`.
    // The diagnostic must point to the `shape` + `follows` pattern for contracts.
    let (_, diag_count) = lex_counts("interface");
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for `interface`");

    let diags = lex_diags("interface");
    assert!(
        diags[0].what.contains("interface"),
        "Diagnostic should name the keyword, got: {:?}", diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("follows"),
        "Diagnostic should mention `follows` for contracts, got: {:?}", diags[0].what_instead
    );

    let tokens = lex_tokens("interface");
    assert!(
        tokens.contains(&Token::Identifier("interface".into())),
        "Lexer must emit Identifier(\"interface\") for parser recovery"
    );
}

#[test]
fn enum_keyword_produces_teaching_diagnostic() {
    // WHY: A user coming from any typed language may write `enum Status { ... }`.
    // Redirects to `options` — the Yinz human-readable term for named value sets.
    let (_, diag_count) = lex_counts("enum");
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for `enum`");

    let diags = lex_diags("enum");
    assert!(
        diags[0].what.contains("enum"),
        "Diagnostic should name the keyword, got: {:?}", diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("options"),
        "Diagnostic should point to `options`, got: {:?}", diags[0].what_instead
    );

    let tokens = lex_tokens("enum");
    assert!(
        tokens.contains(&Token::Identifier("enum".into())),
        "Lexer must emit Identifier(\"enum\") for parser recovery"
    );
}

#[test]
fn abstract_keyword_produces_teaching_diagnostic() {
    // WHY: A user coming from Java/C# may write `abstract class Entity { ... }`.
    // Redirects to `base shape` — the Yinz form for a non-instantiable shape.
    let (_, diag_count) = lex_counts("abstract");
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for `abstract`");

    let diags = lex_diags("abstract");
    assert!(
        diags[0].what.contains("abstract"),
        "Diagnostic should name the keyword, got: {:?}", diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("base shape"),
        "Diagnostic should point to `base shape`, got: {:?}", diags[0].what_instead
    );

    let tokens = lex_tokens("abstract");
    assert!(
        tokens.contains(&Token::Identifier("abstract".into())),
        "Lexer must emit Identifier(\"abstract\") for parser recovery"
    );
}

#[test]
fn banned_declaration_keywords_each_emit_exactly_one_diagnostic() {
    // WHY: Each banned declaration keyword must produce exactly one diagnostic —
    // no silent swallowing, no duplicate warnings for a single occurrence.
    for keyword in &["type", "struct", "class", "interface", "enum", "abstract"] {
        let (_, count) = lex_counts(keyword);
        assert_eq!(
            count, 1,
            "Expected exactly 1 diagnostic for `{keyword}`, got {count}"
        );
    }
}

#[test]
fn m4_source_produces_expected_tokens() {
    // WHY: Snapshot locks the full M4 keyword token stream so any lexer
    // regression is immediately visible. The source exercises all eight new
    // token kinds in a realistic shape-declaration context.
    let source = r#"base shape Entity {
  name: string
}

shape Player extends Entity follows Damageable {
  hidden cache: int
}

function greet(share self: Player) -> string {
  return self.name
}

function example() -> nothing {
  let p: Player = { name: "Patrick", cache: 0 }
  let d: dynamic Damageable = p
  print(greet(p))
}"#;
    let tokens = lex_tokens(source);
    assert_debug_snapshot!("m4_token_stream", tokens);
}


#[test]
fn m5_none_keyword_lexes() {
    // WHY: `none` is a reserved keyword in M5 — it produces Token::None,
    // not an identifier. If a future change accidentally demotes it back
    // to an identifier, every maybe<T> program silently misparses.
    let tokens = lex_tokens("none");
    assert_eq!(tokens, vec![Token::None, Token::Eof], "`none` must lex as Token::None, not Identifier");
}

#[test]
fn match_keyword_produces_teaching_diagnostic() {
    // WHY: A user coming from Rust/Swift may write `match (x) { ... }`.
    // The lexer teaches them to use multi-case `if` instead. The token stream
    // must still contain an Identifier("match") so the parser can recover
    // rather than amplifying the error into a cascade of parse failures.
    let (_, diag_count) = lex_counts("match");
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for `match`");

    let diags = lex_diags("match");
    assert!(
        diags[0].what.contains("match"),
        "Diagnostic should name the keyword, got: {:?}",
        diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("if"),
        "Diagnostic should point to `if`, got: {:?}",
        diags[0].what_instead
    );

    // Recovery: the identifier token is still emitted so the parser doesn't cascade.
    let tokens = lex_tokens("match");
    assert!(
        tokens.contains(&Token::Identifier("match".into())),
        "Lexer must emit Identifier(\"match\") for parser recovery"
    );
}

#[test]
fn switch_keyword_produces_teaching_diagnostic() {
    // WHY: A user coming from JavaScript/C may write `switch (x) { ... }`.
    // Same teaching pattern as `match` above — one diagnostic, identifier
    // token emitted for parser recovery.
    let (_, diag_count) = lex_counts("switch");
    assert_eq!(diag_count, 1, "Expected exactly 1 diagnostic for `switch`");

    let diags = lex_diags("switch");
    assert!(
        diags[0].what.contains("switch"),
        "Diagnostic should name the keyword, got: {:?}",
        diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("if"),
        "Diagnostic should point to `if`, got: {:?}",
        diags[0].what_instead
    );

    let tokens = lex_tokens("switch");
    assert!(
        tokens.contains(&Token::Identifier("switch".into())),
        "Lexer must emit Identifier(\"switch\") for parser recovery"
    );
}

#[test]
fn m3_source_produces_expected_tokens() {
    // WHY: Snapshot locks the full M3 token stream so any lexer regression is
    // immediately visible. The source exercises all seven new token kinds.
    let source = r#"function fib(n: int) -> int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}

function main() -> nothing {
  let result = fib(10)
  while (result > 0) {
    result = result - 1
  }
  for (i in range(0, 5)) {
    if (i) {
      0 => print("zero")
      else => print("nonzero")
    }
  }
}"#;
    let tokens = lex_tokens(source);
    assert_debug_snapshot!("m3_token_stream", tokens);
}

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

    sf.set_text(&mut db)
        .to(r#"function main() -> nothing { print("hello") }"#.to_string());

    let output2 = lex_query(&db, sf);
    let token_count_2 = output2.tokens.len();

    assert_ne!(
        token_count_1, token_count_2,
        "Token count should differ after source text change"
    );
}
