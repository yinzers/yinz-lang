// WHY: every built-in type is PascalCase and the old lowercase spellings are not aliases. These
// tests pin the two compile errors that enforce it, and that the PascalCase spelling compiles
// clean. High tier: a regression here silently accepts the old spelling (an alias the project
// forbids) or lets a user shape hide a built-in.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput};

fn check(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, "test.ynz".to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

// Bug it catches: `let x: int` compiling (a lowercase alias), or the error text drifting from
// the approved WHAT / WHAT INSTEAD / WHY.
#[test]
fn old_lowercase_type_spelling_gets_the_teaching_error() {
    let out =
        check("function entrypoint() -> Nothing {\n  let count: int = 1\n  print(count)\n}\n");
    let teaching: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| d.what == "`int` is not a type name.")
        .collect();
    assert_eq!(teaching.len(), 1, "got: {:#?}", out.diagnostics);
    assert_eq!(teaching[0].what_instead, "Write `Int`.");
    assert_eq!(
        teaching[0].why,
        "Every type in Yinz starts with a capital letter, built-ins included."
    );
}

// Bug it catches: the teaching error missing for generic built-ins and the `channel<T>(n)`
// construction form, which take a different parser/typeck path than a plain `int`.
#[test]
fn old_lowercase_generic_and_channel_spellings_get_the_teaching_error() {
    let generic = check("function f(items: array<Int>) -> Nothing {\n}\n");
    assert!(
        generic
            .diagnostics
            .iter()
            .any(|d| d.what == "`array` is not a type name." && d.what_instead == "Write `Array`."),
        "got: {:#?}",
        generic.diagnostics
    );
    let channel = check("function entrypoint() -> Nothing {\n  let c = channel<Int>(8)\n}\n");
    assert!(
        channel
            .diagnostics
            .iter()
            .any(|d| d.what == "`channel` is not a type name."
                && d.what_instead == "Write `Channel`."),
        "got: {:#?}",
        channel.diagnostics
    );
}

// Bug it catches: the PascalCase spellings being rejected (the old check.rs rule that refused
// capitalised Array/Fixed/Map/Channel left in place), or `Channel<Int>(8)` not constructing.
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
    let _ = (); // names/ratio/flag/big/c are only declared: unused locals are not errors
}

// Bug it catches: a user shape named like a built-in compiling and hiding that built-in.
#[test]
fn shape_named_like_a_built_in_is_a_reserved_name_error() {
    for reserved in ["String", "Map", "Range", "Int", "Channel", "Nothing"] {
        let out = check(&format!(
            "shape {reserved} {{\n  value: Int\n}}\n\nfunction entrypoint() -> Nothing {{\n}}\n"
        ));
        let found = out
            .diagnostics
            .iter()
            .find(|d| d.what == format!("`{reserved}` is a reserved built-in type name."))
            .unwrap_or_else(|| {
                panic!(
                    "no reserved-name error for {reserved}: {:#?}",
                    out.diagnostics
                )
            });
        assert!(
            found
                .why
                .contains(&format!("built-in type called `{reserved}`")),
            "WHY must name the shape: {found:?}"
        );
    }
}

// Bug it catches: the reserved-name check firing on ordinary shape names.
#[test]
fn ordinary_shape_name_is_not_reserved() {
    let out = check("shape Player {\n  health: Int\n}\n\nfunction entrypoint() -> Nothing {\n}\n");
    assert!(
        !out.diagnostics.iter().any(|d| d.what.contains("reserved")),
        "got: {:#?}",
        out.diagnostics
    );
}
