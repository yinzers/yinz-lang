// Map<K, V> typeck (M5 P3c): which map literals, bracket accesses, methods, `for` entries and
// error paths the checker accepts or rejects. One table of named cases: a case is either clean
// (no diagnostic at all) or exactly N diagnostics. Wording of a diagnostic is not asserted here.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput};

fn check(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let source_file = SourceFile::new(&db, "test.ynz".to_string(), source.to_string());
    (*check_query(&db, source_file)).clone()
}

fn assert_clean(source: &str) {
    let output = check(source);
    assert_eq!(
        output.diagnostics.len(),
        0,
        "Expected no diagnostics, got: {:#?}",
        output.diagnostics
    );
}

fn assert_diagnostic_count(source: &str, expected: usize) {
    let output = check(source);
    assert_eq!(
        output.diagnostics.len(),
        expected,
        "Expected {expected} diagnostic(s), got: {:#?}",
        output.diagnostics
    );
}

macro_rules! map_cases {
    ($($case:ident: $source:expr => $expected:tt;)+) => {
        $(
            // bug: typeck gives a map literal, bracket access, method or `for` entry the wrong type, accepts a duplicate key, a mistyped key or value, dot access or an unknown method/field on a map or entry (codegen lowers a wrong-typed map operation), or refuses a valid map program | tier: High
            #[test]
            fn $case() {
                map_case!($source, $expected);
            }
        )+
    };
}

macro_rules! map_case {
    ($source:expr, clean) => {
        assert_clean($source)
    };
    ($source:expr, $diagnostics:literal) => {
        assert_diagnostic_count($source, $diagnostics)
    };
}

map_cases! {
    a_map_literal_with_an_annotation_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90, `bob`: 85 }
}
"#
        => clean;
    a_map_literal_with_int_keys_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<Int, Boolean> = { 1: true, 2: false }
}
"#
        => clean;
    an_empty_map_literal_with_an_annotation_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = {}
}
"#
        => clean;
    a_map_literal_with_a_trailing_comma_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90, `bob`: 85, }
}
"#
        => clean;
    a_map_literal_with_identifier_keys_and_a_map_annotation_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { alice: 90 }
}
"#
        => clean;
    map_bracket_access_with_a_string_key_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    let v: Maybe<Int> = m[`alice`]
}
"#
        => clean;
    map_bracket_access_with_an_int_key_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<Int, String> = { 1: `one`, 2: `two` }
    let v: Maybe<String> = m[1]
}
"#
        => clean;
    map_bracket_assignment_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90, `bob`: 85 }
    m[`bob`] = 95
}
"#
        => clean;
    map_bracket_assignment_of_a_wrong_value_type_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    m[`alice`] = `not an int`
}
"#
        => 1;
    map_get_is_a_maybe_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    let v: Maybe<Int> = m.get(`alice`)
}
"#
        => clean;
    map_has_is_a_boolean_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    let exists: Boolean = m.has(`alice`)
}
"#
        => clean;
    map_set_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    m.set(`bob`, 85)
}
"#
        => clean;
    map_remove_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90, `bob`: 85 }
    m.remove(`alice`)
}
"#
        => clean;
    map_count_is_an_int_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    let n: Int = m.count()
}
"#
        => clean;
    map_keys_is_an_array_of_the_key_type_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    let k: Array<String> = m.keys()
}
"#
        => clean;
    map_values_is_an_array_of_the_value_type_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    let v: Array<Int> = m.values()
}
"#
        => clean;
    map_entries_is_an_array_of_map_entries_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    let entries = m.entries()
    for (e in entries) {
        print(e.key)
        print(e.value.toString())
    }
}
"#
        => clean;
    map_clear_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    m.clear()
}
"#
        => clean;
    map_find_is_a_maybe_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    let v: Maybe<Int> = m.find(`alice`)
}
"#
        => clean;
    a_for_loop_over_a_map_binds_the_entry_key_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90, `bob`: 85 }
    for (entry in m) {
        print(entry.key)
    }
}
"#
        => clean;
    a_for_loop_over_a_map_binds_the_entry_value_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90, `bob`: 85 }
    for (entry in m) {
        print(entry.value.toString())
    }
}
"#
        => clean;
    a_duplicate_string_key_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 1, `alice`: 2 }
}
"#
        => 1;
    a_duplicate_int_key_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Map<Int, Int> = { 1: 10, 2: 20, 1: 30 }
}
"#
        => 1;
    three_unique_keys_are_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `a`: 1, `b`: 2, `c`: 3 }
}
"#
        => clean;
    a_wrong_value_type_in_a_literal_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: `not an int` }
}
"#
        => 1;
    a_wrong_key_type_in_a_literal_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Map<Int, Int> = { `wrong`: 1, 2: 20 }
}
"#
        => 1;
    an_empty_map_literal_without_an_annotation_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m = {}
}
"#
        => 1;
    dot_access_on_a_map_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    let v = m.alice
}
"#
        => 1;
    an_unknown_map_method_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    m.nonexistent()
}
"#
        => 1;
    an_unknown_map_entry_field_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    for (entry in m) {
        let bad = entry.nonexistent
    }
}
"#
        => 1;
    a_method_call_on_a_map_entry_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `alice`: 90 }
    for (entry in m) {
        entry.someMethod()
    }
}
"#
        => 1;
}
