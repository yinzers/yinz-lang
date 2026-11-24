//! Semantic round-trip tests: `parse(format(x)).ast == parse(x).ast` modulo trivia.
//!
//! This is the load-bearing safety invariant for the formatter: it must never
//! change program semantics.  For every clean `.ynz` file in a sweep root we verify:
//!   1. `format(source)` succeeds (no parse errors introduced).
//!   2. `ast_eq_modulo_trivia(original_ast, reformatted_ast)` — same program.
//!
//! One generated table of sweeps (one named test per root), plus two named edge cases.

mod common;

use std::path::{Path, PathBuf};

use common::{check_all_parallel, driver_fixture_files, example_files, fmt_fixture_files};

fn check_semantic_roundtrip(path: &Path) {
    let source = std::fs::read_to_string(path).expect("could not read fixture");

    let db_orig = ynz_parser::CompilerDb::default();
    let sf_orig = ynz_parser::SourceFile::new(
        &db_orig,
        path.to_string_lossy().into_owned(),
        source.clone(),
    );
    let original_parse = ynz_parser::parse_query(&db_orig, sf_orig);

    if !original_parse.diagnostics.is_empty() {
        // Fixtures with pre-existing parse errors are intentional error-gallery files.
        // They are not semantic round-trip targets.
        eprintln!(
            "semantic_roundtrip: skipping {} — has parse errors",
            path.display()
        );
        return;
    }

    let formatted = ynz_fmt::format(&source).unwrap_or_else(|format_error| {
        panic!("format failed on clean {}: {format_error}", path.display())
    });

    let db_fmt = ynz_parser::CompilerDb::default();
    let sf_fmt = ynz_parser::SourceFile::new(&db_fmt, "<formatted>".into(), formatted.clone());
    let reformatted_parse = ynz_parser::parse_query(&db_fmt, sf_fmt);

    assert!(
        reformatted_parse.diagnostics.is_empty(),
        "format() introduced parse errors in {}: {:?}",
        path.display(),
        reformatted_parse.diagnostics
    );

    assert!(
        ynz_ast::ast_eq_modulo_trivia(&original_parse.module, &reformatted_parse.module),
        "semantic round-trip failed for {}: AST changed after formatting",
        path.display()
    );
}

/// One sweep per fixture root: every clean file in it formats to the same program.
macro_rules! roundtrip_sweep_cases {
    ($($case:ident: $files:expr;)+) => {$(
        // bug: format() changes what a program means, or emits source that no longer parses, so format-on-save silently breaks user code | tier: High
        #[test]
        fn $case() {
            let files: Vec<PathBuf> = $files;
            check_all_parallel(&files, check_semantic_roundtrip);
        }
    )+};
}

roundtrip_sweep_cases! {
    fmt_fixtures_roundtrip: fmt_fixture_files();
    examples_roundtrip: example_files();
    driver_fixtures_roundtrip: driver_fixture_files();
}

// bug: format() errors on an empty file, so format-on-save breaks on a new file | tier: Medium
#[test]
fn empty_source_formats_without_error() {
    let formatted = ynz_fmt::format("").expect("empty source should not error");
    assert!(
        formatted.is_empty() || formatted == "\n",
        "empty source produced unexpected output: {formatted:?}"
    );
}

// WHY: the lexer reads `>>` as ONE token (no maximal-munch split), so a
// nested generic in final argument position MUST keep the space between
// the consecutive `>`s or format's output does not re-parse (RED-proven:
// the driver-fixture idempotency sweep failed on the first nested-generic
// annotation in the tree, v0.3-M5 P2 fix round 2).
// bug: format() joins the closing `>`s of a nested generic into `>>`, so its output no longer re-parses | tier: High
#[test]
fn nested_generic_keeps_lexer_required_space() {
    let source =
        "function f() -> Nothing {\n  let picks: Map<String, Maybe<Int> > = { `a`: 1 }\n}\n";
    let formatted = ynz_fmt::format(source).expect("format failed");
    assert!(
        formatted.contains("Map<String, Maybe<Int> >"),
        "nested generic must keep the space before the outer `>`; got:\n{formatted}"
    );
    let reformatted = ynz_fmt::format(&formatted).expect("format output must re-parse");
    assert_eq!(
        formatted, reformatted,
        "formatter is not idempotent on nested generics"
    );
}
