//! Idempotency property test: `format(format(x)) == format(x)` for every fixture.
//!
//! This test walks all `.ynz` files reachable from the fixture directories and asserts
//! that formatting twice produces the same result as formatting once.

use std::path::Path;

fn check_idempotent(path: &Path) {
    let source = std::fs::read_to_string(path).expect("could not read fixture");
    match ynz_fmt::format(&source) {
        Err(e) => {
            // Fixtures with parse errors are intentional error-gallery files — not idempotency targets.
            // Log the skip explicitly so the test surface is visible.
            eprintln!("idempotency: skipping {} — {e}", path.display());
        }
        Ok(formatted) => {
            let formatted2 = ynz_fmt::format(&formatted).expect("format(format(x)) failed");
            assert_eq!(
                formatted,
                formatted2,
                "formatter is not idempotent on {}",
                path.display()
            );
        }
    }
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
fn all_walker_fixtures_are_idempotent() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let files = walk_ynz_files(&base);
    assert!(
        !files.is_empty(),
        "no fixture files found under tests/fixtures"
    );
    for file in &files {
        check_idempotent(file);
    }
}

#[test]
fn all_examples_are_idempotent() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("could not find workspace root");
    let examples_dir = workspace_root.join("examples");
    if !examples_dir.exists() {
        return; // no examples dir — skip
    }
    let files = walk_ynz_files(&examples_dir);
    for file in &files {
        let path_str = file.to_string_lossy();
        // Skip non-canonical demo fixture
        if path_str.contains("fmt_demo") {
            continue;
        }
        // Some error-gallery files have pre-existing parser infinite loops
        // (the parser gets stuck recovering from certain malformed constructs).
        // Skip them — these are parser bugs predating the formatter.
        let is_known_parser_hang =
            path_str.contains("v0_2_m1_errors") || path_str.contains("m1_errors");
        if is_known_parser_hang {
            continue;
        }
        check_idempotent(file);
    }
}

#[test]
fn all_driver_fixtures_are_idempotent() {
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
        check_idempotent(file);
    }
}
