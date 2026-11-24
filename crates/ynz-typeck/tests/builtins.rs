// Built-in collection typeck (M5 P3b): which `Array<T>`, `Fixed<T>`, `Maybe<T>` programs the checker
// accepts or rejects: literals, method result types, bracket access, flow-sensitive `.value`, `for`
// element binding, bare type names. One table of named cases: a case is either clean (no
// diagnostic at all) or exactly N diagnostics. Wording of a diagnostic is not asserted here.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::check_query;

fn check(source: &str) -> ynz_typeck::CheckOutput {
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

macro_rules! collection_cases {
    ($($case:ident: $source:expr => $expected:tt;)+) => {
        $(
            // bug: typeck gives an Array/Fixed/Maybe literal, method, bracket access, `.value` or `for` binding the wrong type (a valid program refused, or a wrong-typed one, an unguarded `.value` or a bare collection type accepted for codegen to lower) | tier: High
            #[test]
            fn $case() {
                collection_case!($source, $expected);
            }
        )+
    };
}

macro_rules! collection_case {
    ($source:expr, clean) => {
        assert_clean($source)
    };
    ($source:expr, $diagnostics:literal) => {
        assert_diagnostic_count($source, $diagnostics)
    };
}

collection_cases! {
    an_array_literal_with_an_annotation_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
}
"#
        => clean;
    an_empty_array_with_an_annotation_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = []
}
"#
        => clean;
    an_empty_array_without_an_annotation_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let arr = []
}
"#
        => 1;
    an_array_literal_with_a_wrong_element_type_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, `hello`]
}
"#
        => 1;
    array_count_is_an_int_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let n: Int = arr.count()
}
"#
        => clean;
    array_contains_is_a_boolean_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let found: Boolean = arr.contains(2)
}
"#
        => clean;
    array_get_is_a_maybe_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let m: Maybe<Int> = arr.get(0)
}
"#
        => clean;
    array_first_is_a_maybe_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let m: Maybe<Int> = arr.first()
}
"#
        => clean;
    array_last_is_a_maybe_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<String> = [`a`, `b`]
    let m: Maybe<String> = arr.last()
}
"#
        => clean;
    array_sort_is_an_array_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [3, 1, 2]
    let sorted: Array<Int> = arr.sort()
}
"#
        => clean;
    array_filter_is_an_array_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let filtered: Array<Int> = arr.filter()
}
"#
        => clean;
    array_add_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    arr.add(4)
}
"#
        => clean;
    array_freeze_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    arr.freeze()
}
"#
        => clean;
    an_unknown_array_method_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    arr.frobnicate()
}
"#
        => 1;
    array_bracket_access_is_a_maybe_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let m: Maybe<Int> = arr[0]
}
"#
        => clean;
    array_bracket_assign_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    arr[0] = 5
}
"#
        => clean;
    array_bracket_assign_of_a_wrong_type_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    arr[0] = `hello`
}
"#
        => 1;
    a_fixed_literal_with_an_annotation_is_clean:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<String> = [`a`, `b`, `c`]
}
"#
        => clean;
    an_empty_fixed_with_an_annotation_is_clean:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = []
}
"#
        => clean;
    a_fixed_literal_with_a_wrong_element_type_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, `bad`]
}
"#
        => 1;
    fixed_count_is_an_int_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    let n: Int = f.count()
}
"#
        => clean;
    fixed_get_is_a_maybe_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    let m: Maybe<Int> = f.get(0)
}
"#
        => clean;
    fixed_contains_is_a_boolean_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    let found: Boolean = f.contains(2)
}
"#
        => clean;
    fixed_sort_is_a_fixed_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [3, 1, 2]
    let sorted: Fixed<Int> = f.sort()
}
"#
        => clean;
    fixed_add_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    f.add(4)
}
"#
        => 1;
    an_unknown_fixed_method_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2]
    f.badMethod()
}
"#
        => 1;
    fixed_bracket_access_is_a_maybe_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    let m: Maybe<Int> = f[0]
}
"#
        => clean;
    fixed_bracket_assign_is_clean:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    f[0] = 5
}
"#
        => clean;
    fixed_bracket_assign_of_a_wrong_type_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    f[0] = `bad`
}
"#
        => 1;
    none_with_a_maybe_annotation_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
}
"#
        => clean;
    none_without_an_annotation_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m = none
}
"#
        => 1;
    none_where_an_int_is_expected_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Int = none
}
"#
        => 1;
    maybe_exists_is_a_boolean_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    let b: Boolean = m.exists()
}
"#
        => clean;
    maybe_or_is_the_inner_type_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    let n: Int = m.or(0)
}
"#
        => clean;
    an_unknown_maybe_method_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    m.badMethod()
}
"#
        => 1;
    maybe_value_without_an_exists_guard_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    let n: Int = m.value
}
"#
        => 1;
    maybe_value_inside_an_exists_guard_is_clean:
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    if (m.exists()) {
        let n: Int = m.value
    }
}
"#
        => clean;
    maybe_value_after_the_guard_block_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    if (m.exists()) {
        let n: Int = m.value
    }
    let outside: Int = m.value
}
"#
        => 1;
    an_unknown_maybe_field_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    let x = m.foo
}
"#
        => 1;
    a_for_loop_over_an_array_binds_the_element_type_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    for (x in arr) {
        let n: Int = x
    }
}
"#
        => clean;
    a_for_loop_over_a_fixed_binds_the_element_type_and_is_clean:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<String> = [`a`, `b`, `c`]
    for (x in f) {
        let s: String = x
    }
}
"#
        => clean;
    a_for_loop_variable_used_as_the_wrong_type_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    for (x in arr) {
        let s: String = x
    }
}
"#
        => 1;
    a_bare_array_type_without_an_argument_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let arr: Array = []
}
"#
        => 1;
    a_bare_fixed_type_without_an_argument_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let f: Fixed = []
}
"#
        => 1;
    a_bare_maybe_type_without_an_argument_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let m: Maybe = none
}
"#
        => 1;
    bracket_access_on_an_int_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let x: Int = 42
    let y = x[0]
}
"#
        => 1;
    bracket_assign_on_a_boolean_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
    let x: Boolean = true
    x[0] = 5
}
"#
        => 1;
}
