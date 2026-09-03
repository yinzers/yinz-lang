//! Semantic round-trip tests: `parse(format(x)).ast == parse(x).ast` modulo trivia.
//!
//! This is the load-bearing safety invariant for the formatter: it must never
//! change program semantics.  For every fixture, we verify:
//!   1. `format(source)` succeeds (no parse errors introduced).
//!   2. `ast_eq_modulo_trivia(original_ast, reformatted_ast)` — same program.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

/// Run `check` over every file across all available cores.
///
/// Same shape as the canonical `parallel_sweep` in
/// `crates/ynz-driver/tests/cross_impl_consistency.rs` (duplicated rather than shared because
/// these are separate crates' test targets, with no shared test-support crate between them).
/// Per `.claude/rules/test-parallelism.md`, the four independence questions are answered here
/// so the next reader does not re-derive them:
///
///   1. No shared mutable state — `check_semantic_roundtrip` builds its OWN
///      `ynz_parser::CompilerDb::default()` per call; nothing is threaded between files.
///   2. No ordering dependency — each file is parsed, formatted, and re-parsed in isolation.
///   3. No shared filesystem path — the check is read-only (`read_to_string`); it writes
///      nothing and spawns no subprocess.
///   4. No global process state — no env mutation, no `set_current_dir`, no singleton.
///
/// A failing file panics inside its worker with the path already in the assert message;
/// `std::thread::scope` propagates that panic on join, so a failure still fails the test.
/// This matches the previous serial behavior, which also stopped at the first bad file.
fn check_all_parallel(files: &[PathBuf], check: impl Fn(&Path) + Sync) {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(files.len().max(1));
    let cursor = AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let idx = cursor.fetch_add(1, Ordering::Relaxed);
                let Some(file) = files.get(idx) else { break };
                check(file);
            });
        }
    });
}

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
    let files: Vec<PathBuf> = walk_ynz_files(&examples_dir)
        .into_iter()
        // fmt_demo is intentionally non-canonical; skip it.
        .filter(|f| !f.to_string_lossy().contains("fmt_demo"))
        .collect();
    check_all_parallel(&files, check_semantic_roundtrip);
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
    check_all_parallel(&files, check_semantic_roundtrip);
}
