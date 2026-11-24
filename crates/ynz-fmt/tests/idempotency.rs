//! Idempotency sweeps: `format(format(x)) == format(x)` for every clean `.ynz` file in a
//! sweep root. One generated table, one named test per root.

mod common;

use std::path::{Path, PathBuf};

use common::{
    check_all_parallel, driver_fixture_files, example_files, fmt_fixture_files, parses_with_errors,
};

fn check_idempotent(path: &Path) {
    let source = std::fs::read_to_string(path).expect("could not read fixture");
    if parses_with_errors(&path.to_string_lossy(), &source) {
        // Fixtures with parse errors are intentional error-gallery files — not idempotency targets.
        // Log the skip explicitly so the test surface is visible.
        eprintln!(
            "idempotency: skipping {} — has parse errors",
            path.display()
        );
        return;
    }
    let formatted = ynz_fmt::format(&source).unwrap_or_else(|format_error| {
        panic!("format failed on clean {}: {format_error}", path.display())
    });
    let formatted_twice = ynz_fmt::format(&formatted).unwrap_or_else(|format_error| {
        panic!(
            "format(format(x)) failed on {}: {format_error}",
            path.display()
        )
    });
    assert_eq!(
        formatted,
        formatted_twice,
        "formatter is not idempotent on {}",
        path.display()
    );
}

macro_rules! idempotency_sweep_cases {
    ($($case:ident: $files:expr;)+) => {$(
        // bug: formatting an already formatted file changes it again, so every save rewrites the file and diffs never settle | tier: High
        #[test]
        fn $case() {
            let files: Vec<PathBuf> = $files;
            check_all_parallel(&files, check_idempotent);
        }
    )+};
}

idempotency_sweep_cases! {
    fmt_fixtures_are_idempotent: fmt_fixture_files();
    examples_are_idempotent: example_files();
    driver_fixtures_are_idempotent: driver_fixture_files();
}
