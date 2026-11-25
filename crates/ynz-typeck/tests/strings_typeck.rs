// String method typecheck (M7 P3b): each string method returns the type the program annotates,
// interpolation accepts primitives and shapes with a standalone `toString` and rejects the rest,
// an unknown method is an error, and bracket access is `Maybe<String>`. One table of named
// cases: a case is either clean (no diagnostic) or exactly N errors.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::check_query;

fn check(source: &str) -> ynz_typeck::CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, "test.ynz".to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

fn assert_clean(source: &str) {
    let out = check(source);
    assert_eq!(
        out.diagnostics.len(),
        0,
        "Expected no diagnostics, got: {:#?}",
        out.diagnostics
    );
}

fn assert_error_count(source: &str, expected: usize) {
    let out = check(source);
    let error_count = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .count();
    assert_eq!(
        error_count, expected,
        "Expected {expected} error(s), got: {:#?}",
        out.diagnostics
    );
}

macro_rules! string_cases {
    ($($case:ident: $source:expr => $expected:tt;)+) => {
        $(
            // bug: typeck gives a string method, interpolation or bracket access the wrong type (a valid program refused, or a shape without toString / an unknown method accepted for codegen to lower) | tier: High
            #[test]
            fn $case() {
                string_case!($source, $expected);
            }
        )+
    };
}

macro_rules! string_case {
    ($source:expr, clean) => {
        assert_clean($source)
    };
    ($source:expr, $errors:literal) => {
        assert_error_count($source, $errors)
    };
}

string_cases! {
    string_to_upper_case_returns_string:
        r#"function entrypoint() -> Nothing {
    let s = `hello`
    let t: String = s.toUpperCase()
}"#
        => clean;
    string_to_lower_case_returns_string:
        r#"function entrypoint() -> Nothing {
    let s = `HELLO`
    let t: String = s.toLowerCase()
}"#
        => clean;
    string_trim_returns_string:
        r#"function entrypoint() -> Nothing {
    let s = `  hello  `
    let t: String = s.trim()
}"#
        => clean;
    string_count_returns_int:
        r#"function entrypoint() -> Nothing {
    let s = `hello`
    let n: Int = s.count()
}"#
        => clean;
    string_byte_count_returns_int:
        r#"function entrypoint() -> Nothing {
    let s = `café`
    let n: Int = s.byteCount()
}"#
        => clean;
    string_grapheme_count_returns_int:
        r#"function entrypoint() -> Nothing {
    let s = `café`
    let n: Int = s.graphemeCount()
}"#
        => clean;
    string_contains_returns_bool:
        r#"function entrypoint() -> Nothing {
    let s = `hello world`
    let b: Boolean = s.contains(`world`)
}"#
        => clean;
    string_starts_with_returns_bool:
        r#"function entrypoint() -> Nothing {
    let s = `hello`
    let b: Boolean = s.startsWith(`he`)
}"#
        => clean;
    string_ends_with_returns_bool:
        r#"function entrypoint() -> Nothing {
    let s = `hello`
    let b: Boolean = s.endsWith(`lo`)
}"#
        => clean;
    string_index_of_returns_maybe_int:
        r#"function entrypoint() -> Nothing {
    let s = `hello`
    let idx: Maybe<Int> = s.indexOf(`ll`)
}"#
        => clean;
    string_byte_at_returns_maybe_int:
        r#"function entrypoint() -> Nothing {
    let s = `hello`
    let b: Maybe<Int> = s.byteAt(2)
}"#
        => clean;
    string_grapheme_at_returns_maybe_string:
        r#"function entrypoint() -> Nothing {
    let s = `café`
    let g: Maybe<String> = s.graphemeAt(0)
}"#
        => clean;
    string_get_returns_maybe_string:
        r#"function entrypoint() -> Nothing {
    let s = `hello`
    let c: Maybe<String> = s.get(0)
}"#
        => clean;
    string_substring_returns_string:
        r#"function entrypoint() -> Nothing {
    let s = `hello world`
    let sub: String = s.substring(0, 5)
}"#
        => clean;
    string_split_returns_array_of_string:
        r#"function entrypoint() -> Nothing {
    let s = `a,b,c`
    let parts: Array<String> = s.split(`,`)
}"#
        => clean;
    string_replace_returns_string:
        r#"function entrypoint() -> Nothing {
    let s = `hello world`
    let r: String = s.replace(`world`, `yinz`)
}"#
        => clean;
    interpolated_string_with_int_is_clean:
        r#"function entrypoint() -> Nothing {
    let n = 42
    let s = `count is ${n}`
}"#
        => clean;
    interpolated_string_with_bool_is_clean:
        r#"function entrypoint() -> Nothing {
    let b = true
    let s = `active: ${b}`
}"#
        => clean;
    interpolated_string_with_float_is_clean:
        r#"function entrypoint() -> Nothing {
    let f: Float = 3.14
    let s = `pi is ${f}`
}"#
        => clean;
    interpolated_string_with_string_is_clean:
        r#"function entrypoint() -> Nothing {
    let name = `world`
    let s = `hello ${name}`
}"#
        => clean;
    interpolated_string_with_shape_with_to_string_is_clean:
        r#"shape Player {
    name: String
}

function toString(share self: Player) -> String {
    return self.name
}

function entrypoint() -> Nothing {
    let p: Player = { name: `Patrick` }
    let s = `player: ${p}`
}"#
        => clean;
    interpolated_string_with_shape_without_to_string_produces_error:
        r#"shape Enemy {
    name: String
}

function entrypoint() -> Nothing {
    let e: Enemy = { name: `Goblin` }
    let s = `enemy: ${e}`
}"#
        => 1;
    string_unknown_method_produces_error:
        r#"function entrypoint() -> Nothing {
    let s = `hello`
    let x = s.nonExistent()
}"#
        => 1;
    string_bracket_access_returns_maybe_string:
        r#"function entrypoint() -> Nothing {
    let s = `hello`
    let c: Maybe<String> = s[0]
}"#
        => clean;
    string_to_int_still_works:
        r#"function entrypoint() -> Nothing {
    let s = `42`
    let n: Maybe<Int> = s.toInt()
}"#
        => clean;
    string_to_float_still_works:
        r#"function entrypoint() -> Nothing {
    let s = `3.14`
    let f: Maybe<Float> = s.toFloat()
}"#
        => clean;
}
