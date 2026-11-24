// The lexer is the first stage of the pipeline. If it silently bails on invalid
// input, all downstream stages see an incomplete token stream and produce
// cascading, misleading errors. Every diagnostic case therefore asserts that the
// lexer reports exactly the expected number of diagnostics and, where the handler
// has one, still emits the recovery token the parser resumes from.
//
// Three generated tables, one named test per case so nextest runs them in parallel
// and names the failing case:
//   * `token_cases!`      valid lexemes and their exact token stream
//   * `diagnostic_cases!` invalid input: diagnostic count, a key phrase, recovery token
//   * `snapshot_cases!`   whole programs against insta snapshots (a different harness
//                         from the inline-expected table above, hence its own macro)

use insta::assert_debug_snapshot;
use ynz_diagnostics::DiagnosticBucket;
use ynz_parser::{lex_query, CompilerDb, SourceFile, Token};

const FILE: &str = "test.ynz";

/// Run the lexer on `source` and return the token values (spans dropped) together
/// with its diagnostics.
fn lex_with_diags(source: &str) -> (Vec<Token>, DiagnosticBucket) {
    let db = CompilerDb::default();
    let source_file = SourceFile::new(&db, FILE.to_string(), source.to_string());
    let output = lex_query(&db, source_file);
    let tokens = output
        .tokens
        .iter()
        .map(|spanned| spanned.value.clone())
        .collect();
    (tokens, output.diagnostics.clone())
}

/// How many diagnostics a case expects: a plain number means exactly that many.
trait ExpectedDiagnosticCount {
    fn accepts(&self, actual: usize) -> bool;
}

impl ExpectedDiagnosticCount for usize {
    fn accepts(&self, actual: usize) -> bool {
        actual == *self
    }
}

/// At least this many diagnostics; for inputs whose exact count is incidental.
struct AtLeast(usize);

impl ExpectedDiagnosticCount for AtLeast {
    fn accepts(&self, actual: usize) -> bool {
        actual >= self.0
    }
}

fn doc_comment(content: &str, break_after: bool) -> Token {
    Token::DocComment {
        content: content.to_string(),
        break_after,
    }
}

/// Source of 100 nested `${` + backtick openers, far past the interpolation depth cap.
fn over_deep_interpolation_source() -> String {
    let mut source = String::from("`");
    for _ in 0..100 {
        source.push_str("${`");
    }
    source
}

/// One case per lexeme: the source lexes to exactly these tokens (an `Eof` is appended
/// to the expected list) and produces no diagnostic.
macro_rules! token_cases {
    ($($case:ident: $source:expr => [$($token:expr),* $(,)?];)+) => {$(
        // bug: lexer emits the wrong token, or splits or merges a lexeme, for a valid source | tier: High
        #[test]
        fn $case() {
            let (tokens, diagnostics) = lex_with_diags($source);
            assert!(
                diagnostics.is_empty(),
                "valid source must lex without diagnostics, got: {diagnostics:?}"
            );
            let expected: Vec<Token> = vec![$($token,)* Token::Eof];
            assert_eq!(tokens, expected);
        }
    )+};
}

/// One case per invalid input: exactly `$count` diagnostics, optionally one key phrase
/// in the named field (`what` or `what_instead`) of some diagnostic, optionally a
/// recovery token the stream must still contain.
macro_rules! diagnostic_cases {
    ($($case:ident: $source:expr => $count:expr
        $(, says $field:ident $phrase:expr)?
        $(, recovers_with $recovery:expr)?;)+) => {$(
        // bug: lexer misses, duplicates or cascades the diagnostic for invalid input, or drops the recovery token | tier: High
        #[test]
        #[allow(unused_variables)] // `tokens` is read only by cases with a recovery token
        fn $case() {
            let (tokens, diagnostics) = lex_with_diags(&$source);
            assert!(
                ExpectedDiagnosticCount::accepts(&$count, diagnostics.len()),
                "diagnostic count {} for {:?}: {diagnostics:?}",
                diagnostics.len(),
                $source
            );
            $(
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic.$field.contains($phrase)),
                    "no diagnostic {} contains {:?}: {diagnostics:?}",
                    stringify!($field),
                    $phrase
                );
            )?
            $(
                let recovery: Token = $recovery;
                assert!(
                    tokens.contains(&recovery),
                    "recovery token {recovery:?} missing from {tokens:?}"
                );
            )?
        }
    )+};
}

/// One case per realistic program: its whole token stream against an insta snapshot.
macro_rules! snapshot_cases {
    ($($case:ident: $snapshot_name:literal, $source:expr;)+) => {$(
        // bug: lexer emits a wrong token for a keyword, operator or literal in a realistic program | tier: High
        #[test]
        fn $case() {
            let (tokens, diagnostics) = lex_with_diags($source);
            assert!(
                diagnostics.is_empty(),
                "valid program must lex without diagnostics, got: {diagnostics:?}"
            );
            assert_debug_snapshot!($snapshot_name, tokens);
        }
    )+};
}

token_cases! {
    // ── boundaries: nothing to lex ───────────────────────────────────────────
    empty_source_is_only_eof: "" => [];
    whitespace_only_source_is_only_eof: "   \n\t  " => [];

    // ── integer literals ─────────────────────────────────────────────────────
    decimal_integer: "42" => [Token::IntLit(42)];
    decimal_integer_with_underscores: "1_000_000" => [Token::IntLit(1_000_000)];
    hex_integer_uppercase: "0xFF" => [Token::IntLit(255)];
    hex_integer_lowercase: "0xff" => [Token::IntLit(255)];
    hex_integer_with_underscore: "0xDEAD_BEEF" => [Token::IntLit(0xDEAD_BEEF)];
    binary_integer: "0b1010" => [Token::IntLit(10)];
    binary_integer_with_underscore: "0b1111_0000" => [Token::IntLit(0b1111_0000)];
    // `42.toString()`: the dot is a method-call separator, not part of the literal.
    integer_before_method_call_keeps_dot_separate: "42.toString()" => [
        Token::IntLit(42),
        Token::Dot,
        Token::Identifier("toString".into()),
        Token::LParen,
        Token::RParen,
    ];

    // ── number literals ──────────────────────────────────────────────────────
    number_with_dot: "3.14" => [Token::NumberLit("3.14".into())];
    number_with_exponent: "1e5" => [Token::NumberLit("1e5".into())];
    number_with_negative_exponent: "2.5e-3" => [Token::NumberLit("2.5e-3".into())];
    number_underscores_are_stripped: "1_000.5" => [Token::NumberLit("1000.5".into())];

    // ── operators: the longest match wins ────────────────────────────────────
    eq_eq_is_one_token: "==" => [Token::EqEq];
    not_eq_is_one_token: "!=" => [Token::NotEq];
    lt_eq_is_one_token: "<=" => [Token::LtEq];
    gt_eq_is_one_token: ">=" => [Token::GtEq];
    amp_amp_is_one_token: "&&" => [Token::AmpAmp];
    pipe_pipe_is_one_token: "||" => [Token::PipePipe];
    shift_left_is_one_token: "<<" => [Token::LtLt];
    shift_right_is_one_token: ">>" => [Token::GtGt];
    fat_arrow_is_one_token: "=>" => [Token::FatArrow];
    lone_eq_is_not_fat_arrow: "=" => [Token::Eq];
    lone_gt_is_not_fat_arrow: ">" => [Token::Gt];
    arrow_beats_minus: "-> Nothing" => [Token::Arrow, Token::Identifier("Nothing".into())];
    minus_without_gt_is_minus: "a - b" => [
        Token::Identifier("a".into()),
        Token::Minus,
        Token::Identifier("b".into()),
    ];
    single_slash_is_division: "a / b" => [
        Token::Identifier("a".into()),
        Token::Slash,
        Token::Identifier("b".into()),
    ];

    // ── comments ─────────────────────────────────────────────────────────────
    line_comment_is_skipped: "let x = 1 // this is a comment\nlet y = 2" => [
        Token::Let,
        Token::Identifier("x".into()),
        Token::Eq,
        Token::IntLit(1),
        Token::Let,
        Token::Identifier("y".into()),
        Token::Eq,
        Token::IntLit(2),
    ];
    // A comment on the last line, with no trailing newline, must not read past EOF.
    line_comment_at_end_of_file: "let x = 1 // trailing comment" => [
        Token::Let,
        Token::Identifier("x".into()),
        Token::Eq,
        Token::IntLit(1),
    ];
    // `//` at the start of a line must not be mistaken for the `///` doc-comment prefix.
    double_slash_comment_is_not_a_doc_comment: "// regular comment\nlet x = 1" => [
        Token::Let,
        Token::Identifier("x".into()),
        Token::Eq,
        Token::IntLit(1),
    ];
    doc_comment_followed_by_item: "/// foo\nexport" => [
        doc_comment("foo", false),
        Token::Export,
    ];
    // A blank line after the doc comment sets `break_after`, so the parser drops the orphan.
    doc_comment_before_blank_line_breaks_after: "/// orphan\n\nexport" => [
        doc_comment("orphan", true),
        Token::Export,
    ];
    // A plain `//` line between two `///` lines does not break the chain.
    plain_comment_between_doc_comments_does_not_break_chain:
        "/// line1\n// internal\n/// line2\nexport" => [
            doc_comment("line1", false),
            doc_comment("line2", false),
            Token::Export,
        ];
    // Off-by-one guard on the `///` prefix strip: the fourth slash is content.
    four_slashes_is_doc_comment_with_slash_content: "////\nexport" => [
        doc_comment("/", false),
        Token::Export,
    ];

    // ── keywords: each must lex as its own token, never as an identifier ─────
    keyword_true: "true" => [Token::True];
    keyword_false: "false" => [Token::False];
    keyword_if: "if" => [Token::If];
    keyword_else: "else" => [Token::Else];
    keyword_while: "while" => [Token::While];
    keyword_for: "for" => [Token::For];
    keyword_in: "in" => [Token::In];
    keyword_return: "return" => [Token::Return];
    keyword_shape: "shape" => [Token::Shape];
    keyword_follows: "follows" => [Token::Follows];
    keyword_extends: "extends" => [Token::Extends];
    keyword_base: "base" => [Token::Base];
    keyword_hidden: "hidden" => [Token::Hidden];
    keyword_dynamic: "dynamic" => [Token::Dynamic];
    // Golden Rule 10: `Self` (type) and `self` (value) are different tokens.
    keyword_self_type_capitalised: "Self" => [Token::SelfType];
    keyword_self_value_lowercase: "self" => [Token::SelfValue];
    keyword_none: "none" => [Token::None];
    keyword_options: "options" => [Token::Options];
    keyword_is: "is" => [Token::Is];
    keyword_errors: "errors" => [Token::Errors];
    keyword_import: "import" => [Token::Import];
    keyword_export: "export" => [Token::Export];
    keyword_sensitive: "sensitive" => [Token::Sensitive];
    keyword_wait: "wait" => [Token::Wait];
    keyword_background: "background" => [Token::Background];

    // ── backtick strings and interpolation ───────────────────────────────────
    backtick_string: "`hello`" => [Token::BacktickString(b"hello".to_vec())];
    backtick_empty_string: "``" => [Token::BacktickString(vec![])];
    // Non-ASCII bytes pass through to codegen unchanged and are not a lex error.
    backtick_string_keeps_utf8_bytes: "`café`" => [Token::BacktickString("café".as_bytes().to_vec())];
    backtick_string_preserves_newline: "`line1\nline2`" => [Token::BacktickString(b"line1\nline2".to_vec())];
    backtick_escapes_are_decoded: "`\\n\\t\\\\`" => [Token::BacktickString(b"\n\t\\".to_vec())];
    backtick_interpolation_in_middle: "`hello ${name}!`" => [
        Token::BacktickString(b"hello ".to_vec()),
        Token::InterpolationStart,
        Token::Identifier("name".into()),
        Token::InterpolationEnd,
        Token::BacktickString(b"!".to_vec()),
    ];
    // The parser always sees a string segment first, so a leading `${` is preceded by an empty one.
    backtick_interpolation_at_start_gets_empty_leading_segment: "`${x} world`" => [
        Token::BacktickString(vec![]),
        Token::InterpolationStart,
        Token::Identifier("x".into()),
        Token::InterpolationEnd,
        Token::BacktickString(b" world".to_vec()),
    ];
    // An interpolation at the end is followed by an empty closing segment.
    backtick_interpolation_at_end_gets_empty_trailing_segment: "`${a + 1}`" => [
        Token::BacktickString(vec![]),
        Token::InterpolationStart,
        Token::Identifier("a".into()),
        Token::Plus,
        Token::IntLit(1),
        Token::InterpolationEnd,
        Token::BacktickString(vec![]),
    ];
}

diagnostic_cases! {
    // ── numeric literals ─────────────────────────────────────────────────────
    two_decimal_points: "1.2.3" => 1, says what "decimal point";
    adjacent_underscores: "1__000" => 1, says what "adjacent";
    trailing_underscore: "1_" => 1;
    invalid_hex_digit: "0xZZ" => 2, says what "hex";
    invalid_binary_digit: "0b22" => 2, says what "binary";
    integer_overflow: "99999999999999999999999" => 1, says what "int";
    leading_underscore_in_hex: "let bad = 0x_FF" => 1, says what "_";
    leading_underscore_in_binary: "let bad = 0b_1010" => 1, says what "_";
    bare_dot_float: "let x = 3." => 1, says what "ecimal";

    // ── compound assignment and increment operators (one diagnostic each) ────
    compound_plus_eq: "x += 1" => 1, says what "+=";
    compound_minus_eq: "x -= 1" => 1, says what "-=";
    compound_star_eq: "x *= 2" => 1, says what "*=";
    compound_slash_eq: "x /= 2" => 1, says what "/=";
    compound_percent_eq: "x %= 2" => 1, says what "%=";
    increment_plus_plus: "x++" => 1, says what "++";
    decrement_minus_minus: "x--" => 1, says what "--";

    // ── foreign-character handlers (teaching-surfaces.md: one case per handler) ─
    foreign_double_quote_string: r#""hello""# => 1,
        says what "Double-quoted", recovers_with Token::BacktickString(vec![]);
    // A multi-line "string" reports once at the opening quote, not a cascade from line two.
    foreign_double_quote_multiline_string: "\"hello\nworld\"" => 1, says what "Double-quoted";
    foreign_single_quote_string: "'hello world'" => 1,
        says what "Single-quoted", recovers_with Token::BacktickString(vec![]);
    // `#` comment: the handler consumes the whole line, so no cascade from `this is a comment`.
    foreign_hash_comment: "# this is a comment" => 1, says what_instead "//";
    foreign_semicolon: "let x = 5;" => 1, says what "Semicolons",
        recovers_with Token::IntLit(5);
    foreign_dollar_prefix: "$x" => 1, says what "`$`", recovers_with Token::Identifier("x".into());
    foreign_question_mark_suffix: "Int?" => 1, says what_instead "Maybe";
    // The generic handler: one diagnostic per codepoint (3), not per byte (9).
    foreign_non_ascii_chars_one_diagnostic_per_codepoint: "日本語" => 3;

    // ── recovery: the lexer keeps going after the error ──────────────────────
    dollar_inside_call_keeps_lexing: "function entrypoint() -> Nothing { print($) }" => 1,
        recovers_with Token::RBrace;
    unterminated_double_quote_string_keeps_lexing:
        "function entrypoint() -> Nothing { print(\"oops) }" => 1,
        says what "Double-quoted";
    unterminated_backtick_string: "`unterminated" => 1, says what "backtick";

    // ── banned declaration keywords: teaching diagnostic, identifier kept for the parser ─
    banned_keyword_type: "type" => 1,
        says what_instead "shape", recovers_with Token::Identifier("type".into());
    banned_keyword_struct: "struct" => 1,
        says what_instead "shape", recovers_with Token::Identifier("struct".into());
    banned_keyword_class: "class" => 1,
        says what_instead "shape", recovers_with Token::Identifier("class".into());
    banned_keyword_interface: "interface" => 1,
        says what_instead "follows", recovers_with Token::Identifier("interface".into());
    banned_keyword_enum: "enum" => 1,
        says what_instead "options", recovers_with Token::Identifier("enum".into());
    banned_keyword_enum_before_options_declaration: "enum Status { active }" => 1,
        says what_instead "options", recovers_with Token::Identifier("enum".into());
    banned_keyword_abstract: "abstract" => 1,
        says what_instead "base shape", recovers_with Token::Identifier("abstract".into());
    banned_keyword_match: "match" => 1,
        says what_instead "if", recovers_with Token::Identifier("match".into());
    banned_keyword_switch: "switch" => 1,
        says what_instead "if", recovers_with Token::Identifier("switch".into());
    banned_keyword_async: "async function foo() -> Nothing {}" => 1,
        says what_instead "wait", recovers_with Token::Identifier("async".into());
    banned_keyword_pub: "pub function foo() -> Nothing {}" => 1,
        says what_instead "export", recovers_with Token::Identifier("pub".into());
    // `test` is reserved for the v0.13 test framework so user code does not break later.
    reserved_keyword_test: "function test() -> Nothing {}" => 1,
        says what_instead "v0.13", recovers_with Token::Identifier("test".into());

    // ── sized numeric type names are not Yinz types ──────────────────────────
    sized_type_f32: "let x: f32 = 1" => 1, says what "f32", recovers_with Token::Identifier("f32".into());
    sized_type_f64: "let x: f64 = 1" => 1, says what "f64", recovers_with Token::Identifier("f64".into());
    sized_type_i8: "let x: i8 = 1" => 1, says what "i8", recovers_with Token::Identifier("i8".into());
    sized_type_i16: "let x: i16 = 1" => 1, says what "i16", recovers_with Token::Identifier("i16".into());
    sized_type_i32: "let x: i32 = 1" => 1, says what "i32", recovers_with Token::Identifier("i32".into());
    sized_type_i64: "let x: i64 = 1" => 1, says what "i64", recovers_with Token::Identifier("i64".into());
    sized_type_u8: "let x: u8 = 1" => 1, says what "u8", recovers_with Token::Identifier("u8".into());
    sized_type_u16: "let x: u16 = 1" => 1, says what "u16", recovers_with Token::Identifier("u16".into());
    sized_type_u32: "let x: u32 = 1" => 1, says what "u32", recovers_with Token::Identifier("u32".into());
    sized_type_u64: "let x: u64 = 1" => 1, says what "u64", recovers_with Token::Identifier("u64".into());

    // ── resource caps: input sized to exhaust memory is cut off with a diagnostic ─
    identifier_over_1024_bytes: "a".repeat(2000) => 1, says what "1024";
    interpolation_nested_past_64_levels: over_deep_interpolation_source() => AtLeast(1), says what "64";
    // `\0` inside a string would silently truncate at print time, so it is refused at lex time.
    nul_escape_in_backtick_string: "`hello\\0world`" => 1, says what "NUL";
}

snapshot_cases! {
    realistic_operators_and_literals_program: "m2_token_stream",
        r#"function entrypoint() -> Nothing {
  let price = 0.1 + 0.2
  let count: Int = 42
  let active = true
  print(price)
  print(count * count - 1)
  print(active && (count > 0))
}"#;
    realistic_control_flow_program: "m3_token_stream",
        "function fib(n: Int) -> Int {\n  if (n < 2) {\n    return n\n  }\n  return fib(n - 1) + fib(n - 2)\n}\n\nfunction entrypoint() -> Nothing {\n  let result = fib(10)\n  while (result > 0) {\n    result = result - 1\n  }\n  for (i in range(0, 5)) {\n    if (i) {\n      0 => print(`zero`)\n      else => print(`nonzero`)\n    }\n  }\n}";
    realistic_shape_program: "m4_token_stream",
        "base shape Entity {\n  name: String\n}\n\nshape Player extends Entity follows Damageable {\n  hidden cache: Int\n}\n\nfunction greet(share self: Player) -> String {\n  return self.name\n}\n\nfunction example() -> Nothing {\n  let p: Player = { name: `Patrick`, cache: 0 }\n  let d: dynamic Damageable = p\n  print(greet(p))\n}";
}
