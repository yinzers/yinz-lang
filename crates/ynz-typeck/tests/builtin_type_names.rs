// WHY: every built-in type is PascalCase and the old lowercase spellings are not aliases. These
// tests pin the compile errors that enforce it, and that the PascalCase spelling compiles clean.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput};

fn check(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, "test.ynz".to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

fn assert_error_what_contains(source: &str, phrase: &str) {
    let out = check(source);
    assert!(
        out.diagnostics.iter().any(|d| d.what.contains(phrase)),
        "expected an error containing {phrase:?}, got: {:#?}",
        out.diagnostics
    );
}

macro_rules! lowercase_spelling_cases {
    ($($case:ident: $source:expr;)+) => {
        $(
            // bug: typeck accepts a lowercase built-in type spelling (`int`, `array<..>`, `channel<..>`) as an alias | tier: High
            #[test]
            fn $case() {
                assert_error_what_contains($source, "is not a type name");
            }
        )+
    };
}

lowercase_spelling_cases! {
    old_lowercase_type_spelling_is_refused:
        "function entrypoint() -> Nothing {\n  let count: int = 1\n  print(count)\n}\n";
    old_lowercase_generic_spelling_is_refused:
        "function f(items: array<Int>) -> Nothing {\n}\n";
    old_lowercase_channel_construction_is_refused:
        "function entrypoint() -> Nothing {\n  let c = channel<Int>(8)\n}\n";
}

// bug: typeck refuses the PascalCase built-in spellings (Array, Map, Float, Boolean, Number, Channel) | tier: High
#[test]
fn pascal_case_built_in_types_compile_clean() {
    let out = check(
        "function entrypoint() -> Nothing {\n  \
           let items: Array<Int> = [1, 2]\n  \
           let names: Map<String, Int> = { `a`: 1 }\n  \
           let ratio: Float = 1.5\n  \
           let flag: Boolean = true\n  \
           let big: Number = 1.0\n  \
           let c = Channel<Int>(8)\n  \
           print(items.count())\n}\n",
    );
    assert!(
        out.diagnostics.is_empty(),
        "expected no diagnostics, got: {:#?}",
        out.diagnostics
    );
}

fn shape_source(shape_name: &str) -> String {
    format!("shape {shape_name} {{\n  value: Int\n}}\n\nfunction entrypoint() -> Nothing {{\n}}\n")
}

fn assert_shape_name_reserved(shape_name: &str) {
    assert_error_what_contains(
        &shape_source(shape_name),
        "is a reserved built-in type name",
    );
}

fn assert_shape_name_not_reserved(shape_name: &str) {
    let out = check(&shape_source(shape_name));
    assert!(
        !out.diagnostics.iter().any(|d| d.what.contains("reserved")),
        "got: {:#?}",
        out.diagnostics
    );
}

macro_rules! reserved_shape_name_cases {
    ($($case:ident: $check:ident($shape_name:literal);)+) => {
        $(
            // bug: typeck accepts a shape named like a built-in (hiding it), or refuses an ordinary shape name as reserved | tier: High
            #[test]
            fn $case() {
                $check($shape_name);
            }
        )+
    };
}

reserved_shape_name_cases! {
    shape_named_string_is_reserved: assert_shape_name_reserved("String");
    shape_named_map_is_reserved: assert_shape_name_reserved("Map");
    shape_named_range_is_reserved: assert_shape_name_reserved("Range");
    shape_named_int_is_reserved: assert_shape_name_reserved("Int");
    shape_named_channel_is_reserved: assert_shape_name_reserved("Channel");
    shape_named_nothing_is_reserved: assert_shape_name_reserved("Nothing");
    ordinary_shape_name_is_not_reserved: assert_shape_name_not_reserved("Player");
}
