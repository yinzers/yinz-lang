//! CLI integration tests for `ynz fmt`.
//!
//! Each test invokes the real `ynz` binary via `assert_cmd` and verifies
//! exit codes, stdout/stderr content, and (where relevant) on-disk file state.

use assert_cmd::Command;
use std::path::Path;

fn ynz() -> Command {
    Command::cargo_bin("ynz").expect("ynz binary not found")
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fmt")
        .join(name)
}

/// One named test per `ynz fmt` mode and outcome, sharing the `ynz()` / `fixture()` harness.
macro_rules! fmt_cli_case {
    ($test_name:ident => $body:block) => {
        // bug: `ynz fmt` exits with the wrong code, or rewrites a file it must leave alone (or skips one it must format), in one CLI mode | tier: Medium
        #[test]
        fn $test_name() $body
    };
}

// ── Single file ───────────────────────────────────────────────────────────────

// case: `ynz fmt` rewrites a file that is already canonical
fmt_cli_case!(single_file_already_canonical_is_not_rewritten => {
    let path = fixture("canonical.ynz");
    let original = std::fs::read_to_string(&path).unwrap();

    ynz().arg("fmt").arg(&path).assert().success();

    let after = std::fs::read_to_string(&path).unwrap();
    assert_eq!(original, after, "canonical file must not be rewritten");
});

// case: `ynz fmt` leaves a non-canonical file unformatted on disk
fmt_cli_case!(single_file_noncanonical_is_rewritten => {
    // A temp copy keeps the fixture non-canonical for future runs.
    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join("noncanonical.ynz");
    std::fs::copy(fixture("noncanonical.ynz"), &dst).unwrap();

    ynz().arg("fmt").arg(&dst).assert().success();

    let after = std::fs::read_to_string(&dst).unwrap();
    assert!(
        after.starts_with("function greet("),
        "expected formatted output; got: {after:?}"
    );
});

// case: a missing file exits 1 or 0 instead of the infra-error code 2
fmt_cli_case!(single_file_not_found_exits_2 => {
    // A missing file is an infra error (2), not a source error (1).
    ynz()
        .arg("fmt")
        .arg("/nonexistent/path/to/file.ynz")
        .assert()
        .failure()
        .code(2);
});

// case: a file with a parse error exits 0 or 2 instead of the source-error code 1
fmt_cli_case!(single_file_parse_error_exits_1 => {
    ynz()
        .arg("fmt")
        .arg(fixture("parse_error.ynz"))
        .assert()
        .failure()
        .code(1);
});

// ── --check ───────────────────────────────────────────────────────────────────

// case: `--check` on a canonical file exits non-zero or writes the file
fmt_cli_case!(check_canonical_exits_0_and_writes_nothing => {
    let path = fixture("canonical.ynz");
    let original = std::fs::read_to_string(&path).unwrap();

    ynz()
        .arg("fmt")
        .arg("--check")
        .arg(&path)
        .assert()
        .success();

    let after = std::fs::read_to_string(&path).unwrap();
    assert_eq!(original, after, "--check must not write anything");
});

// case: `--check` on a non-canonical file exits 0 or does not name the file it would reformat
fmt_cli_case!(check_noncanonical_exits_1_and_prints_would_reformat => {
    ynz()
        .arg("fmt")
        .arg("--check")
        .arg(fixture("noncanonical.ynz"))
        .assert()
        .failure()
        .code(1)
        .stderr(predicates::str::contains("Would reformat:"));
});

// ── --stdin ───────────────────────────────────────────────────────────────────

// case: `--stdin` writes nothing to stdout or does not format the input
fmt_cli_case!(stdin_formats_to_stdout => {
    let source = "function  f( )  ->  Nothing  { }\n";
    let out = ynz()
        .arg("fmt")
        .arg("--stdin")
        .write_stdin(source)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let formatted = String::from_utf8(out).unwrap();
    assert!(
        formatted.contains("function f()"),
        "expected canonical output; got: {formatted:?}"
    );
});

// case: `--stdin` of broken source exits 0 instead of 1
fmt_cli_case!(stdin_parse_error_exits_1 => {
    let broken = "function broken() -> Nothing {\n  let x = 1\n";
    ynz()
        .arg("fmt")
        .arg("--stdin")
        .write_stdin(broken)
        .assert()
        .failure()
        .code(1);
});

// ── --all (project mode) ──────────────────────────────────────────────────────

// case: `--all` outside a `yinz.toml` project exits 0 or 1 instead of the infra-error code 2
fmt_cli_case!(all_outside_project_exits_2 => {
    // --all requires a yinz.toml project root; no project is an infra error.
    ynz()
        .arg("fmt")
        .arg("--all")
        .arg("/tmp")
        .assert()
        .failure()
        .code(2)
        .stderr(predicates::str::contains("yinz.toml"));
});

// case: `--all` skips a non-canonical file of the project
fmt_cli_case!(all_inside_project_formats_all_files => {
    // A temp copy so the fixture project is not clobbered.
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("project"), dir.path()).unwrap();

    let noncanon_dst = dir.path().join("noncanonical.ynz");
    let before = std::fs::read_to_string(&noncanon_dst).unwrap();
    assert!(
        before.contains("function  helper"),
        "precondition: file is non-canonical"
    );

    ynz()
        .arg("fmt")
        .arg("--all")
        .arg(dir.path())
        .assert()
        .success();

    let after = std::fs::read_to_string(&noncanon_dst).unwrap();
    assert!(
        after.contains("function helper()"),
        "expected canonical output; got: {after:?}"
    );
});

// case: `--all` stops at the first broken file (skipping the rest) or exits 0 despite it
fmt_cli_case!(all_with_one_broken_file_continues_and_exits_1 => {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("project"), dir.path()).unwrap();

    let broken = dir.path().join("broken.ynz");
    std::fs::write(&broken, "function broken() -> Nothing {\n  let x = 1\n").unwrap();
    let good = dir.path().join("reformat_me.ynz");
    std::fs::write(&good, "function  g()  ->  Nothing  { }\n").unwrap();

    ynz()
        .arg("fmt")
        .arg("--all")
        .arg(dir.path())
        .assert()
        .failure()
        .code(1);

    // The good file must be reformatted even though broken.ynz had a parse error.
    let good_after = std::fs::read_to_string(&good).unwrap();
    assert!(
        good_after.starts_with("function g()"),
        "good file must be reformatted; got: {good_after:?}"
    );
});

// ── Helpers ───────────────────────────────────────────────────────────────────

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(src)?.flatten() {
        let target = dst.join(entry.file_name());
        if entry.path().is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
