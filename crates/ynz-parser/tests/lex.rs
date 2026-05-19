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
fn lex_diags(source: &str) -> ynz_diagnostics::DiagnosticBucket {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    lex_query(&db, sf).diagnostics.clone()
}

/// Run the lexer and return (tokens, diagnostics) together.
fn lex_with_diags(source: &str) -> (Vec<Token>, ynz_diagnostics::DiagnosticBucket) {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    let output = lex_query(&db, sf);
    let toks = output.tokens.iter().map(|s| s.value.clone()).collect();
    (toks, output.diagnostics.clone())
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
    //
    // test-ratchet: M7 P1 migrates source to backtick syntax — double-quotes now
    // produce an error diagnostic so the snapshot must use backtick strings.
    let source = "function entrypoint() -> nothing { print(`hello, yinz`) }";
    let tokens = lex_tokens(source);
    assert_debug_snapshot!("m1_token_stream", tokens);
}

#[test]
fn m2_source_produces_expected_tokens() {
    // WHY: This is the exact token stream Phase 3 (parser) depends on for the
    // M2 smoke test. A silent change here means the parser's expectations break.
    let source = r#"function entrypoint() -> nothing {
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
    assert_eq!(
        diag_count, 1,
        "Expected exactly 1 diagnostic for overflow literal"
    );

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
    //
    // test-ratchet: M7 P1 changes the source to use backtick strings (double-quotes
    // now produce an error diagnostic). New Token variants are added to the exhaustive
    // match so the test remains a full-coverage span check.
    let source = r#"function entrypoint() -> nothing { print(`hello`) }"#;
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
            | Token::None
            | Token::Options
            | Token::Is
            // test-ratchet: M7 P1 adds BacktickString, InterpolationStart, InterpolationEnd, Errors
            // test-ratchet: M8 P1 adds Import, Export, Sensitive, Wait, Background, DocComment
            | Token::BacktickString(_)
            | Token::InterpolationStart
            | Token::InterpolationEnd
            | Token::Errors
            | Token::Import
            | Token::Export
            | Token::Sensitive
            | Token::Wait
            | Token::Background
            | Token::DocComment { .. } => {
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
    let source = r#"function entrypoint() -> nothing { print($) }"#;
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
    // WHY: Parser must not panic on unterminated strings — it needs a
    // complete token stream to detect all parse errors at once.
    let source = r#"function entrypoint() -> nothing { print("oops) }"#;
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
    // WHY: M7 backtick strings must pass raw UTF-8 bytes through to codegen unchanged.
    // Non-ASCII content inside a backtick string is NOT an error at lex time.
    //
    // test-ratchet: M7 P1 — migrated from double-quoted to backtick syntax.
    // Double-quotes now produce a diagnostic; backtick strings carry raw bytes.
    // The token is now BacktickString instead of StringLit.
    let source = "function entrypoint() -> nothing { print(`café`) }";
    let (token_count, diag_count) = lex_counts(source);
    assert_eq!(
        diag_count, 0,
        "Non-ASCII in a backtick string literal must not produce a diagnostic"
    );

    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    let output = lex_query(&db, sf);
    let string_lit = output
        .tokens
        .iter()
        .find(|t| matches!(t.value, Token::BacktickString(_)))
        .expect("should find a BacktickString token");

    if let Token::BacktickString(bytes) = &string_lit.value {
        let expected = "café".as_bytes();
        assert_eq!(
            bytes.as_slice(),
            expected,
            "BacktickString bytes must match the UTF-8 encoding of the literal"
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
    assert_ne!(
        self_type, self_value,
        "Self (type) and self (value) must be distinct tokens"
    );
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
        "Diagnostic should name the keyword, got: {:?}",
        diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("shape"),
        "Diagnostic should point to `shape`, got: {:?}",
        diags[0].what_instead
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
        "Diagnostic should name the keyword, got: {:?}",
        diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("shape"),
        "Diagnostic should point to `shape`, got: {:?}",
        diags[0].what_instead
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
        "Diagnostic should name the keyword, got: {:?}",
        diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("shape"),
        "Diagnostic should point to `shape`, got: {:?}",
        diags[0].what_instead
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
    assert_eq!(
        diag_count, 1,
        "Expected exactly 1 diagnostic for `interface`"
    );

    let diags = lex_diags("interface");
    assert!(
        diags[0].what.contains("interface"),
        "Diagnostic should name the keyword, got: {:?}",
        diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("follows"),
        "Diagnostic should mention `follows` for contracts, got: {:?}",
        diags[0].what_instead
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
        "Diagnostic should name the keyword, got: {:?}",
        diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("options"),
        "Diagnostic should point to `options`, got: {:?}",
        diags[0].what_instead
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
    assert_eq!(
        diag_count, 1,
        "Expected exactly 1 diagnostic for `abstract`"
    );

    let diags = lex_diags("abstract");
    assert!(
        diags[0].what.contains("abstract"),
        "Diagnostic should name the keyword, got: {:?}",
        diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("base shape"),
        "Diagnostic should point to `base shape`, got: {:?}",
        diags[0].what_instead
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
    //
    // test-ratchet: M7 P1 migrates source to backtick syntax.
    let source = "base shape Entity {\n  name: string\n}\n\nshape Player extends Entity follows Damageable {\n  hidden cache: int\n}\n\nfunction greet(share self: Player) -> string {\n  return self.name\n}\n\nfunction example() -> nothing {\n  let p: Player = { name: `Patrick`, cache: 0 }\n  let d: dynamic Damageable = p\n  print(greet(p))\n}";
    let tokens = lex_tokens(source);
    assert_debug_snapshot!("m4_token_stream", tokens);
}

#[test]
fn m5_none_keyword_lexes() {
    // WHY: `none` is a reserved keyword in M5 — it produces Token::None,
    // not an identifier. If a future change accidentally demotes it back
    // to an identifier, every maybe<T> program silently misparses.
    let tokens = lex_tokens("none");
    assert_eq!(
        tokens,
        vec![Token::None, Token::Eof],
        "`none` must lex as Token::None, not Identifier"
    );
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
    //
    // test-ratchet: M7 P1 migrates source to backtick syntax.
    let source = "function fib(n: int) -> int {\n  if (n < 2) {\n    return n\n  }\n  return fib(n - 1) + fib(n - 2)\n}\n\nfunction entrypoint() -> nothing {\n  let result = fib(10)\n  while (result > 0) {\n    result = result - 1\n  }\n  for (i in range(0, 5)) {\n    if (i) {\n      0 => print(`zero`)\n      else => print(`nonzero`)\n    }\n  }\n}";
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
        "function entrypoint() -> nothing { }".to_string(),
    );

    let output1 = lex_query(&db, sf);
    let token_count_1 = output1.tokens.len();

    // test-ratchet: M7 P1 — migrated to backtick syntax
    sf.set_text(&mut db)
        .to("function entrypoint() -> nothing { print(`hello`) }".to_string());

    let output2 = lex_query(&db, sf);
    let token_count_2 = output2.tokens.len();

    assert_ne!(
        token_count_1, token_count_2,
        "Token count should differ after source text change"
    );
}

// ── M6: options + is ─────────────────────────────────────────────────────────

#[test]
fn m6_token_variant_count_locked() {
    // WHY: pins the token vocabulary at the M6 P1 surface.
    //
    // test-ratchet: M6 P1 adds 2 variants over M5's 58:
    //   Keyword (2): Options (`options` declaration keyword), Is (`is` type-narrowing keyword)
    //   Total M1+M2+M3+M4+M5+M6: 58 + 2 = 60
    let expected_count = 60usize;

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
        // M5
        None,
        // M6
        Options,
        Is,
    ];
    assert_eq!(
        all_variants.len(),
        expected_count,
        "Token variant count changed from {expected_count} — update this test \
         with a // test-ratchet: <reason> comment and update the Token doc comment"
    );
}

#[test]
fn m6_options_keyword_lexes() {
    // WHY: `options` must produce Token::Options, not Token::Identifier("options").
    // If this fails, every options-declaration parse test will see an identifier
    // where the parser expects the keyword, producing cascading parse errors.
    let tokens = lex_tokens("options Status { active }");
    assert_eq!(
        tokens[0],
        Token::Options,
        "expected Token::Options for 'options'"
    );
    assert_eq!(
        tokens[1],
        Token::Identifier("Status".into()),
        "expected identifier after options"
    );
}

#[test]
fn m6_is_keyword_lexes() {
    // WHY: `is` must produce Token::Is, not Token::Identifier("is").
    // If this fails, the parser cannot produce Is-arm AST nodes in multi-case blocks,
    // and union narrowing is completely non-functional.
    let tokens = lex_tokens("if (x) { is Circle => print(\"hit\") }");
    let is_pos = tokens.iter().position(|t| *t == Token::Is);
    assert!(
        is_pos.is_some(),
        "expected Token::Is in the token stream for 'is'"
    );
}

#[test]
fn m6_enum_banned_keyword_still_fires() {
    // WHY: The `enum` banned-keyword diagnostic must still fire after adding
    // Token::Options — the ban redirects to `options`, and Options is now a
    // real keyword. Both the diagnostic and the fallback identifier behavior
    // must coexist correctly.
    let (tok_count, diag_count) = lex_counts("enum Status { active }");
    assert_eq!(
        diag_count, 1,
        "expected exactly 1 banned-keyword diagnostic for 'enum'"
    );
    assert!(
        tok_count > 0,
        "expected tokens to be emitted (tolerant recovery)"
    );
}

// ── M7 P1: backtick strings + errors keyword ─────────────────────────────────

#[test]
fn m7_token_variant_count_locked() {
    // WHY: Pins the token vocabulary at the M7 P1 surface.
    //
    // test-ratchet: M7 P1 adds 4 variants over M6's 60:
    //   BacktickString, InterpolationStart, InterpolationEnd, Errors
    //   Total M1+M2+M3+M4+M5+M6+M7: 60 + 4 = 64
    let expected_count = 64usize;

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
        // M5
        None,
        // M6
        Options,
        Is,
        // M7 P1
        BacktickString(vec![]),
        InterpolationStart,
        InterpolationEnd,
        Errors,
    ];
    assert_eq!(
        all_variants.len(),
        expected_count,
        "Token variant count changed from {expected_count} — update this test \
         with a // test-ratchet: <reason> comment and update the Token doc comment"
    );
}

#[test]
fn m7_backtick_simple_string_tokenizes() {
    // WHY: The simplest backtick string `` `hello` `` must produce a single
    // BacktickString token with the correct bytes. If the lexer treats the
    // backtick as an unknown character, all string literals produce errors.
    let tokens = lex_tokens("`hello`");
    assert_eq!(
        tokens,
        vec![Token::BacktickString(b"hello".to_vec()), Token::Eof],
        "Simple backtick string must produce BacktickString with correct bytes"
    );
}

#[test]
fn m7_backtick_empty_string_tokenizes() {
    // WHY: An empty backtick string `` `` `` must produce a BacktickString with
    // an empty byte vector. If empty strings panic or produce Error tokens,
    // default values and empty-string checks in real programs break.
    let tokens = lex_tokens("``");
    assert_eq!(
        tokens,
        vec![Token::BacktickString(vec![]), Token::Eof],
        "Empty backtick string must produce BacktickString with empty bytes"
    );
}

#[test]
fn m7_backtick_string_with_one_interpolation() {
    // WHY: `` `hello ${name}!` `` must produce BacktickString("hello "),
    // InterpolationStart, Identifier("name"), InterpolationEnd, BacktickString("!").
    // If any of these tokens are missing or reordered, string interpolation is broken.
    let tokens = lex_tokens("`hello ${name}!`");
    assert!(
        tokens
            .iter()
            .any(|t| *t == Token::BacktickString(b"hello ".to_vec())),
        "Must contain BacktickString(b\"hello \")"
    );
    assert!(
        tokens.iter().any(|t| *t == Token::InterpolationStart),
        "Must contain InterpolationStart"
    );
    assert!(
        tokens
            .iter()
            .any(|t| *t == Token::Identifier("name".into())),
        "Must contain Identifier(name)"
    );
    assert!(
        tokens.iter().any(|t| *t == Token::InterpolationEnd),
        "Must contain InterpolationEnd"
    );
    assert!(
        tokens
            .iter()
            .any(|t| *t == Token::BacktickString(b"!".to_vec())),
        "Must contain BacktickString(b\"!\")"
    );
}

#[test]
fn m7_backtick_string_interpolation_at_start() {
    // WHY: `` `${x} world` `` — interpolation at the very start — must produce
    // BacktickString("") (empty) before InterpolationStart. The empty segment
    // is required so the parser always sees BacktickString first.
    let tokens = lex_tokens("`${x} world`");
    let first = &tokens[0];
    assert_eq!(
        *first,
        Token::BacktickString(vec![]),
        "Interpolation at start must be preceded by an empty BacktickString"
    );
    assert!(
        tokens.iter().any(|t| *t == Token::InterpolationStart),
        "Must contain InterpolationStart"
    );
    assert!(
        tokens
            .iter()
            .any(|t| *t == Token::BacktickString(b" world".to_vec())),
        "Must contain BacktickString(b\" world\") after interpolation"
    );
}

#[test]
fn m7_backtick_multiline_string_preserves_newlines() {
    // WHY: Backtick strings must span multiple lines without errors — they are
    // not constrained to a single line. The newline byte must appear in the
    // BacktickString payload.
    let source = "`line1\nline2`";
    let (_, diag_count) = lex_counts(source);
    assert_eq!(
        diag_count, 0,
        "Multi-line backtick string must produce no diagnostics"
    );

    let tokens = lex_tokens(source);
    let bs = tokens
        .iter()
        .find(|t| matches!(t, Token::BacktickString(_)));
    assert!(bs.is_some(), "Must produce a BacktickString token");
    if let Some(Token::BacktickString(bytes)) = bs {
        assert!(
            bytes.contains(&b'\n'),
            "Newline must be preserved in BacktickString bytes"
        );
    }
}

#[test]
fn m7_backtick_escape_sequences_decoded() {
    // WHY: `` `\n\t\\` `` must produce BacktickString bytes [10, 9, 92].
    // If escape sequences are passed through literally, programs cannot embed
    // newlines or tabs in strings.
    let tokens = lex_tokens("`\\n\\t\\\\`");
    let bs = tokens
        .iter()
        .find(|t| matches!(t, Token::BacktickString(_)));
    assert!(bs.is_some(), "Must produce a BacktickString");
    if let Some(Token::BacktickString(bytes)) = bs {
        assert_eq!(bytes.as_slice(), &[b'\n', b'\t', b'\\']);
    }
}

#[test]
fn m7_backtick_nested_braces_inside_interpolation() {
    // WHY: `` `${if (x) { 1 } else { 2 }}` `` — braces inside the interpolated
    // expression must NOT trigger InterpolationEnd prematurely. The brace nesting
    // counter must track them correctly.
    let source = "`${a + 1}`";
    let (_, diag_count) = lex_counts(source);
    assert_eq!(
        diag_count, 0,
        "Simple interpolation with no inner braces must produce no diagnostics"
    );

    let tokens = lex_tokens(source);
    // Should end with a BacktickString token (the empty suffix)
    let interp_end_idx = tokens.iter().position(|t| *t == Token::InterpolationEnd);
    let backtick_after = interp_end_idx.map(|i| tokens.get(i + 1));
    assert!(
        backtick_after
            .flatten()
            .map(|t| matches!(t, Token::BacktickString(_)))
            .unwrap_or(false),
        "After InterpolationEnd there must be a BacktickString"
    );
}

#[test]
fn m7_unterminated_backtick_produces_diagnostic() {
    // WHY: An unterminated backtick string must produce exactly one diagnostic
    // pointing at the missing closing backtick. If the lexer panics or produces
    // zero diagnostics, malformed programs silently produce broken IR.
    let (_, diag_count) = lex_counts("`unterminated");
    assert_eq!(
        diag_count, 1,
        "Unterminated backtick string must produce exactly 1 diagnostic"
    );

    let diags = lex_diags("`unterminated");
    assert!(
        diags[0].what.contains("backtick"),
        "Diagnostic must mention 'backtick', got: {:?}",
        diags[0].what
    );
}

#[test]
fn m7_double_quote_produces_error_diagnostic() {
    // WHY: Double-quoted strings are banned in Yinz. The lexer must emit a
    // teaching diagnostic redirecting to backtick syntax. A recovery token
    // (BacktickString) must also be emitted so the parser can continue.
    let (_, diag_count) = lex_counts(r#""hello""#);
    assert_eq!(
        diag_count, 1,
        "Double-quoted string must produce exactly 1 error diagnostic"
    );

    let diags = lex_diags(r#""hello""#);
    assert!(
        diags[0].what.contains("Double-quoted"),
        "Diagnostic must mention double-quoted strings, got: {:?}",
        diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("backtick"),
        "Diagnostic must redirect to backtick syntax, got: {:?}",
        diags[0].what_instead
    );

    // Recovery: a BacktickString token must still be emitted
    let tokens = lex_tokens(r#""hello""#);
    assert!(
        tokens.iter().any(|t| matches!(t, Token::BacktickString(_))),
        "Lexer must emit BacktickString recovery token after double-quote error"
    );
}

#[test]
fn single_quote_produces_one_error_with_recovery() {
    // WHY: `print('hello world')` used to produce 5 cascade errors (two invalid-char
    // errors + two undefined-name errors + a wrong-arity error) because the generic
    // unknown-byte handler didn't consume the string content. Single quotes must get
    // the same dedicated handler as double quotes: one diagnostic, one recovery token,
    // zero cascade noise.
    let (_, diag_count) = lex_counts("'hello world'");
    assert_eq!(
        diag_count, 1,
        "Single-quoted string must produce exactly 1 diagnostic, got {diag_count}"
    );

    let diags = lex_diags("'hello world'");
    assert!(
        diags[0].what.contains("Single-quoted"),
        "Diagnostic must mention single-quoted strings, got: {:?}",
        diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("backtick"),
        "Diagnostic must redirect to backtick syntax, got: {:?}",
        diags[0].what_instead
    );

    let tokens = lex_tokens("'hello world'");
    assert!(
        tokens.iter().any(|t| matches!(t, Token::BacktickString(_))),
        "Lexer must emit BacktickString recovery token after single-quote error"
    );
}

#[test]
fn hash_comment_produces_one_error_no_cascade() {
    // WHY: `# this is a comment` used to produce cascade errors because `#` was
    // consumed but `this`, `is`, `a`, `comment` were still lexed as identifiers.
    // The dedicated handler must consume the whole line — guard against regression.
    let (_, diag_count) = lex_counts("# this is a comment");
    assert_eq!(
        diag_count, 1,
        "Hash comment must produce exactly 1 diagnostic, got {diag_count}"
    );
    let diags = lex_diags("# this is a comment");
    assert!(
        diags[0].what_instead.contains("//"),
        "Must redirect to `//` syntax, got: {:?}",
        diags[0].what_instead
    );
}

#[test]
fn semicolon_produces_teaching_error() {
    // WHY: `let x = 5;` from JS/Rust habit must tell the user semicolons aren't needed,
    // not just "character not valid". Guards against falling back to emit_unknown_byte.
    let (_, diag_count) = lex_counts("let x = 5;");
    assert_eq!(diag_count, 1, "Semicolon must produce exactly 1 diagnostic");
    let diags = lex_diags("let x = 5;");
    assert!(
        diags[0].what.contains("Semicolons") || diags[0].what.contains("semicolon"),
        "Must mention semicolons, got: {:?}",
        diags[0].what
    );
    assert!(
        diags[0].what_instead.contains("newline") || diags[0].what_instead.contains("Remove"),
        "Must tell user to remove it, got: {:?}",
        diags[0].what_instead
    );
}

#[test]
fn dollar_prefix_produces_teaching_error() {
    // WHY: `$x` from PHP/shell must tell the user no prefix is needed, not generic.
    let diags = lex_diags("$x");
    assert!(!diags.is_empty(), "$ must produce a diagnostic");
    assert!(
        diags[0].what_instead.contains("$") || diags[0].what.contains("$"),
        "Must mention the $ prefix, got: {:?}",
        diags[0]
    );
}

#[test]
fn question_mark_produces_teaching_error() {
    // WHY: `int?` from Swift/Kotlin must redirect to `maybe<T>`, not generic.
    let diags = lex_diags("int?");
    assert!(!diags.is_empty(), "? must produce a diagnostic");
    assert!(
        diags[0].what_instead.contains("maybe") || diags[0].why.contains("maybe"),
        "Must redirect to maybe<T>, got: {:?}",
        diags[0]
    );
}

#[test]
fn m7_errors_keyword_lexes() {
    // WHY: `errors` is a new M7 keyword — it must produce Token::Errors, not
    // an Identifier. If it falls through to Identifier, the parser cannot
    // recognize the `-> T errors` fallible return type syntax.
    let tokens = lex_tokens("errors");
    assert_eq!(
        tokens,
        vec![Token::Errors, Token::Eof],
        "`errors` must lex as Token::Errors"
    );
}

#[test]
fn m7_errors_keyword_in_context() {
    // WHY: `-> string errors` must produce Arrow, Named/string, Errors. If `errors`
    // is lexed as an Identifier, the parser sees `-> string identifier` and cannot
    // produce a FunctionDecl with `errors_capable: true`.
    let tokens = lex_tokens("-> string errors");
    assert!(
        tokens.iter().any(|t| *t == Token::Errors),
        "Token::Errors must appear in the token stream for `errors` keyword"
    );
    // The token immediately before Errors should be an Identifier("string") or similar
    let errors_pos = tokens.iter().position(|t| *t == Token::Errors).unwrap();
    assert!(errors_pos > 0, "Token::Errors must not be the first token");
}

// ── M8 P1: import/export/sensitive/wait/background keywords + doc comments ───

#[test]
fn m8_token_variant_count_locked() {
    // WHY: Pins the token vocabulary at the M8 P1 surface. Adding a token
    // without updating this test signals an unplanned API change.
    //
    // test-ratchet: M8 P1 adds 6 variants over M7's 64:
    //   Import, Export, Sensitive, Wait, Background, DocComment
    //   Total M1+...+M8: 64 + 6 = 70
    let expected_count = 70usize;

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
        // M5
        None,
        // M6
        Options,
        Is,
        // M7 P1
        BacktickString(vec![]),
        InterpolationStart,
        InterpolationEnd,
        Errors,
        // M8 P1
        Import,
        Export,
        Sensitive,
        Wait,
        Background,
        DocComment {
            content: String::new(),
            break_after: false,
        },
    ];
    assert_eq!(
        all_variants.len(),
        expected_count,
        "Token variant count changed from {expected_count} — update this test \
         with a // test-ratchet: <reason> comment and update the Token doc comment"
    );
}

#[test]
fn m8_import_keyword_tokenizes() {
    // WHY: `import` must lex as Token::Import, not as an Identifier. If it
    // lexes as Identifier, the parser cannot build ImportDecl nodes.
    let tokens = lex_tokens("import");
    assert!(
        tokens.iter().any(|t| *t == Token::Import),
        "Token::Import must appear for `import` keyword"
    );
}

#[test]
fn m8_export_keyword_tokenizes() {
    // WHY: `export` must lex as Token::Export, not as an Identifier.
    let tokens = lex_tokens("export");
    assert!(
        tokens.iter().any(|t| *t == Token::Export),
        "Token::Export must appear for `export` keyword"
    );
}

#[test]
fn m8_sensitive_keyword_tokenizes() {
    // WHY: `sensitive` must lex as Token::Sensitive for type-modifier parsing.
    let tokens = lex_tokens("sensitive");
    assert!(
        tokens.iter().any(|t| *t == Token::Sensitive),
        "Token::Sensitive must appear for `sensitive` keyword"
    );
}

#[test]
fn m8_wait_keyword_tokenizes() {
    // WHY: `wait` must lex as Token::Wait, not an Identifier.
    let tokens = lex_tokens("wait");
    assert!(
        tokens.iter().any(|t| *t == Token::Wait),
        "Token::Wait must appear for `wait` keyword"
    );
}

#[test]
fn m8_background_keyword_tokenizes() {
    // WHY: `background` must lex as Token::Background.
    let tokens = lex_tokens("background");
    assert!(
        tokens.iter().any(|t| *t == Token::Background),
        "Token::Background must appear for `background` keyword"
    );
}

#[test]
fn m8_doc_comment_simple() {
    // WHY: `/// foo` must produce Token::DocComment{content:"foo", break_after:false}
    // when followed immediately by a real token. If it produces no token or
    // an Identifier, doc-comment attachment is impossible.
    let tokens = lex_tokens("/// foo\nexport function f() -> nothing {}");
    let doc = tokens.iter().find_map(|t| {
        if let Token::DocComment {
            content,
            break_after,
        } = t
        {
            Some((content.clone(), *break_after))
        } else {
            None
        }
    });
    assert!(
        doc.is_some(),
        "DocComment token must be emitted for `/// foo`"
    );
    let (content, break_after) = doc.unwrap();
    assert_eq!(content, "foo", "Content must be stripped of `/// ` prefix");
    assert!(
        !break_after,
        "break_after must be false when next token follows immediately"
    );
}

#[test]
fn m8_doc_comment_empty_line_is_break_after() {
    // WHY: A blank line between a doc-comment and the following item sets
    // break_after = true. The parser uses this to discard orphaned doc chains.
    let src = "/// orphan\n\nexport function f() -> nothing {}";
    let tokens = lex_tokens(src);
    let doc = tokens.iter().find_map(|t| {
        if let Token::DocComment {
            content,
            break_after,
        } = t
        {
            Some((content.clone(), *break_after))
        } else {
            None
        }
    });
    assert!(
        doc.is_some(),
        "DocComment must be emitted even when break_after is true"
    );
    let (_content, break_after) = doc.unwrap();
    assert!(
        break_after,
        "break_after must be true when a blank line separates doc from item"
    );
}

#[test]
fn m8_doc_comment_regular_comment_not_a_break() {
    // WHY: A regular `//` line between two `///` lines does NOT break the chain.
    // Source: `/// line1\n// internal\n/// line2\nexport ...`
    // Lexer must emit two DocComment tokens, both with break_after = false.
    let src = "/// line1\n// internal\n/// line2\nexport function f() -> nothing {}";
    let tokens = lex_tokens(src);
    let docs: Vec<(String, bool)> = tokens
        .iter()
        .filter_map(|t| {
            if let Token::DocComment {
                content,
                break_after,
            } = t
            {
                Some((content.clone(), *break_after))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        docs.len(),
        2,
        "Two DocComment tokens expected; got {}",
        docs.len()
    );
    assert!(!docs[0].1, "First doc must have break_after = false");
    assert!(!docs[1].1, "Second doc must have break_after = false");
}

#[test]
fn m8_double_slash_still_skipped() {
    // WHY: `// regular comment` must NOT produce a DocComment token; it should
    // be consumed as trivia. Regression guard against `///` detection logic
    // accidentally matching `//`.
    let tokens = lex_tokens("// regular comment\nlet x = 1");
    let has_doc = tokens.iter().any(|t| matches!(t, Token::DocComment { .. }));
    assert!(
        !has_doc,
        "Regular `//` comment must not produce DocComment token"
    );
}

#[test]
fn m8_four_slashes_is_doc_with_slash_content() {
    // WHY: `////` must produce DocComment{content: "/", ...}. The fourth slash
    // is part of the content. Guards against off-by-one in the `///` prefix strip.
    let tokens = lex_tokens("////\nexport function f() -> nothing {}");
    let doc = tokens.iter().find_map(|t| {
        if let Token::DocComment { content, .. } = t {
            Some(content.clone())
        } else {
            None
        }
    });
    assert!(doc.is_some(), "DocComment must be emitted for `////`");
    assert_eq!(
        doc.unwrap(),
        "/",
        "Content of `////` must be `/` (fourth slash is content)"
    );
}

#[test]
fn m8_async_banned_keyword_produces_diagnostic() {
    // WHY: `async` is a banned concurrency keyword in Yinz. The lexer must emit
    // a teaching diagnostic and degrade to Identifier so parsing can continue.
    let (toks, diags) = lex_with_diags("async function foo() -> nothing {}");
    assert_eq!(diags.len(), 1, "One diagnostic expected for `async`");
    assert!(
        diags[0].what.contains("async") || diags[0].what_instead.contains("wait"),
        "Diagnostic must mention `async` or redirect to `wait`"
    );
    // Token degrades to Identifier so parsing continues.
    assert!(
        toks.iter().any(|t| matches!(t, Token::Identifier(_))),
        "Identifier must appear in stream after async banned-keyword diagnostic"
    );
}

#[test]
fn m8_pub_banned_keyword_produces_diagnostic() {
    // WHY: `pub` is a banned visibility keyword in Yinz. Must redirect to `export`.
    let (toks, diags) = lex_with_diags("pub function foo() -> nothing {}");
    assert_eq!(diags.len(), 1, "One diagnostic expected for `pub`");
    assert!(
        diags[0].what_instead.contains("export"),
        "Diagnostic must redirect to `export`, got: {:?}",
        diags[0].what_instead
    );
    assert!(
        toks.iter().any(|t| matches!(t, Token::Identifier(_))),
        "Identifier must appear in stream after pub banned-keyword diagnostic"
    );
}

#[test]
fn test_keyword_reserved_for_v013() {
    // WHY: `test` is reserved in v0.1 for the v0.13 built-in test framework.
    //      Without this check, users who name a function or shape `test`
    //      will have working code that suddenly breaks when v0.13 ships.
    //      The reservation gives them a clear message now rather than a
    //      surprise incompatibility later.
    let (toks, diags) = lex_with_diags("function test() -> nothing {}");
    assert_eq!(diags.len(), 1, "One diagnostic expected for `test`");
    assert!(
        diags[0].what.contains("`test`") || diags[0].what_instead.contains("v0.13"),
        "Diagnostic must mention `test` or redirect to v0.13, got: {:?}",
        diags[0]
    );
    assert!(
        toks.iter().any(|t| matches!(t, Token::Identifier(_))),
        "Identifier must appear in stream after test reserved-keyword diagnostic"
    );
}

// ── f32 / sized-numeric-type teaching diagnostics ────────────────────────────

#[test]
fn f32_keyword_rejected_with_teaching_diagnostic() {
    // WHY: `f32` and friends are not Yinz numeric types in v0.1. Users coming
    // from Rust/C/Go may write `let x: f32 = 1.5` — they need a clear redirect
    // to `float` or `number`. Without this, the lexer produces a generic
    // "unexpected token" cascade that gives no hint about what to use instead.
    // Parser recovery: the banned keyword degrades to Identifier so downstream
    // parsing can continue without a cascade of unrelated errors.
    let sized_types = ["f32", "f64", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"];
    for kw in sized_types {
        let src = format!("let x: {kw} = 1");
        let (toks, diags) = lex_with_diags(&src);
        assert_eq!(
            diags.len(),
            1,
            "exactly 1 diagnostic expected for `{kw}`, got {}:\n{:#?}",
            diags.len(),
            diags
        );
        assert!(
            diags[0].what.contains(kw),
            "`{kw}` diagnostic WHAT must mention the keyword, got: {:?}",
            diags[0].what
        );
        assert!(
            diags[0].what_instead.contains("int")
                || diags[0].what_instead.contains("float")
                || diags[0].what_instead.contains("number"),
            "`{kw}` diagnostic WHAT_INSTEAD must redirect to a Yinz numeric type, got: {:?}",
            diags[0].what_instead
        );
        assert!(
            toks.iter().any(|t| matches!(t, Token::Identifier(_))),
            "`{kw}` must degrade to Identifier after diagnostic"
        );
    }
}

// ── 5a security/robustness fixes ─────────────────────────────────────────────

#[test]
fn hardening_identifier_too_long_produces_diagnostic() {
    // WHY: A 10 MB single identifier must not exhaust heap without bound.
    // The cap at 1024 bytes must fire and produce one diagnostic; the lexer
    // must not hang or OOM.
    let long_id: String = "a".repeat(2000);
    let (_, diags) = lex_with_diags(&long_id);
    assert_eq!(diags.len(), 1, "Exactly one diagnostic expected for over-long identifier");
    assert!(
        diags[0].what.contains("1024"),
        "Diagnostic must mention the 1024 limit, got: {:?}",
        diags[0].what
    );
}

#[test]
fn hardening_unicode_error_one_diagnostic_per_codepoint() {
    // WHY: Multi-byte codepoints outside any token must produce one diagnostic
    // per codepoint, NOT one per byte. `日本語` is three 3-byte codepoints →
    // 3 diagnostics, not 9.
    let source = "日本語";
    let (_, diags) = lex_with_diags(source);
    assert_eq!(
        diags.len(),
        3,
        "Expected 3 diagnostics (one per codepoint), got {}: {:?}",
        diags.len(),
        diags
    );
}

#[test]
fn hardening_leading_underscore_hex_rejected() {
    // WHY: `0x_FF` has a leading separator that the spec forbids. The validator
    // missed it before this fix; now it must produce exactly one diagnostic.
    let (_, diags) = lex_with_diags("let bad = 0x_FF");
    assert_eq!(diags.len(), 1, "Exactly one diagnostic expected for 0x_FF, got: {:?}", diags);
    assert!(
        diags[0].what.contains("_"),
        "Diagnostic must mention the underscore, got: {:?}",
        diags[0].what
    );
}

#[test]
fn hardening_leading_underscore_binary_rejected() {
    // WHY: Same as the hex case — `0b_1010` must be rejected for the same reason.
    let (_, diags) = lex_with_diags("let bad = 0b_1010");
    assert_eq!(diags.len(), 1, "Exactly one diagnostic expected for 0b_1010, got: {:?}", diags);
    assert!(
        diags[0].what.contains("_"),
        "Diagnostic must mention the underscore, got: {:?}",
        diags[0].what
    );
}

#[test]
fn hardening_interpolation_depth_cap() {
    // WHY: Deeply nested `${...}` must not grow the interp_depth_stack without
    // bound. The cap at 64 levels must fire with a diagnostic. Nested interpolation
    // structure: `` ` `` then 100× `` ${` `` (each opens an interpolation then starts
    // an inner backtick string inside the expression) drives the depth well past 64.
    let mut source = String::from("`");
    for _ in 0..100 {
        source.push_str("${`");
    }
    let (_, diags) = lex_with_diags(&source);
    assert!(
        !diags.is_empty(),
        "At least one diagnostic expected for over-deep interpolation"
    );
    let depth_diag = diags.iter().find(|d| d.what.contains("64"));
    assert!(
        depth_diag.is_some(),
        "A diagnostic mentioning the 64-level cap must be present, got: {:?}",
        diags
    );
}

#[test]
fn hardening_nul_byte_in_string_rejected() {
    // WHY: `\0` (NUL byte) inside a backtick string must be rejected at the
    // lexer so the silent-truncation footgun never reaches the runtime.
    // `hello\0world` — the NUL would silently truncate to `hello` at print time.
    let (_, diags) = lex_with_diags("`hello\\0world`");
    assert_eq!(diags.len(), 1, "Exactly one diagnostic expected for NUL escape, got: {:?}", diags);
    assert!(
        diags[0].what.contains("\\0") || diags[0].what.contains("NUL"),
        "Diagnostic must mention the NUL escape, got: {:?}",
        diags[0].what
    );
}

#[test]
fn hardening_bare_dot_float_rejected() {
    // WHY: `3.` without a fractional digit is ambiguous — could be a float literal
    // or a typo. The compiler must reject it with a clear diagnostic.
    let (_, diags) = lex_with_diags("let x = 3.");
    assert_eq!(diags.len(), 1, "Exactly one diagnostic expected for `3.`, got: {:?}", diags);
    assert!(
        diags[0].what.contains("Decimal") || diags[0].what.contains("decimal"),
        "Diagnostic must mention the missing digits, got: {:?}",
        diags[0].what
    );
}

#[test]
fn hardening_double_quote_multiline_one_error() {
    // WHY: A "string" with a literal newline inside (`"hello\nworld"`) must
    // produce exactly ONE error pointing at the opening `"`, not a cascade of
    // parse errors from the second line being re-lexed as code.
    // The error span must point at the opening quote, not the whole "string".
    let source = "\"hello\nworld\"";
    let (_, diags) = lex_with_diags(source);
    // The double-quote error fires once for the opening `"`.
    // After recovery, `world` is lexed as an identifier (no diagnostic for that).
    // The closing `"` on the second line fires a second double-quote diagnostic.
    // So we expect exactly 2 diagnostics (one per `"`), NOT a cascade of "world
    // is undefined" or similar.
    let double_quote_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.what.contains("Double-quoted"))
        .collect();
    assert_eq!(
        double_quote_diags.len(),
        1,
        "Exactly one double-quote diagnostic expected for the opening `\"`, got {} total diags: {:?}",
        diags.len(),
        diags
    );
    // The diagnostic span must point at just the opening `"` (position 0), not
    // the full string content.
    assert_eq!(
        double_quote_diags[0].span.start,
        0,
        "Diagnostic span must start at the opening `\"`"
    );
}
