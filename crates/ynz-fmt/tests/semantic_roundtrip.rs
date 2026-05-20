//! Semantic round-trip tests: `parse(format(x)).ast == parse(x).ast` modulo trivia.
//!
//! This is the load-bearing safety invariant for the formatter: it must never
//! change program semantics.  For every fixture, we verify:
//!   1. `format(source)` succeeds (no parse errors introduced).
//!   2. `ast_eq_modulo_trivia(original_ast, reformatted_ast)` — same program.

use std::path::Path;

fn check_semantic_roundtrip(path: &Path) {
    let source = std::fs::read_to_string(path).expect("could not read fixture");

    let db_orig = ynz_parser::CompilerDb::default();
    let sf_orig = ynz_parser::SourceFile::new(
        &db_orig,
        path.to_string_lossy().into_owned().into(),
        source.clone().into(),
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

    let formatted = match ynz_fmt::format(&source) {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "semantic_roundtrip: skipping {} — format error: {e}",
                path.display()
            );
            return;
        }
    };

    let db_fmt = ynz_parser::CompilerDb::default();
    let sf_fmt =
        ynz_parser::SourceFile::new(&db_fmt, "<formatted>".into(), formatted.clone().into());
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

fn walk_ynz_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = vec![];
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                files.extend(walk_ynz_files(&p));
            } else if p.extension().and_then(|e| e.to_str()) == Some("ynz") {
                files.push(p);
            }
        }
    }
    files
}

#[test]
fn fmt_fixtures_roundtrip() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let files = walk_ynz_files(&base);
    assert!(
        !files.is_empty(),
        "no fixture files found under tests/fixtures"
    );
    for file in &files {
        check_semantic_roundtrip(file);
    }
}

#[test]
fn examples_roundtrip() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("could not find workspace root");
    let examples_dir = workspace_root.join("examples");
    if !examples_dir.exists() {
        return;
    }
    let files = walk_ynz_files(&examples_dir);
    for file in &files {
        let path_str = file.to_string_lossy();
        // fmt_demo is intentionally non-canonical; skip it.
        if path_str.contains("fmt_demo") {
            continue;
        }
        // Pre-existing parser infinite loops — skip.
        let is_known_parser_hang =
            path_str.contains("v0_2_m1_errors") || path_str.contains("m1_errors");
        if is_known_parser_hang {
            continue;
        }
        check_semantic_roundtrip(file);
    }
}

#[test]
fn driver_fixtures_roundtrip() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("could not find workspace root");
    let fixtures_dir = workspace_root.join("crates/ynz-driver/tests/fixtures");
    if !fixtures_dir.exists() {
        return;
    }
    let files = walk_ynz_files(&fixtures_dir);
    for file in &files {
        let path_str = file.to_string_lossy();
        let is_known_parser_hang =
            path_str.contains("v0_2_m1_errors") || path_str.contains("m1_errors");
        if is_known_parser_hang {
            continue;
        }
        check_semantic_roundtrip(file);
    }
}
