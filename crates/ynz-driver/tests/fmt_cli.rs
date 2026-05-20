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

// ── Single-file happy path ────────────────────────────────────────────────────

#[test]
fn single_file_already_canonical_exits_0() {
    // WHY: idempotent; formatting a canonical file must not rewrite it or bump mtime.
    let path = fixture("canonical.ynz");
    let original = std::fs::read_to_string(&path).unwrap();

    ynz().arg("fmt").arg(&path).assert().success();

    let after = std::fs::read_to_string(&path).unwrap();
    assert_eq!(original, after, "canonical file must not be rewritten");
}

#[test]
fn single_file_noncanonical_rewrites_exits_0() {
    // WHY: formatter must write the canonical form when the input differs.
    // We use a temp copy so the fixture stays non-canonical for future runs.
    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join("noncanonical.ynz");
    std::fs::copy(fixture("noncanonical.ynz"), &dst).unwrap();

    ynz().arg("fmt").arg(&dst).assert().success();

    let after = std::fs::read_to_string(&dst).unwrap();
    assert!(
        after.starts_with("function greet("),
        "expected formatted output; got: {after:?}"
    );
}

#[test]
fn single_file_not_found_exits_2() {
    // WHY: missing file is an infra error, not a source error.
    ynz()
        .arg("fmt")
        .arg("/nonexistent/path/to/file.ynz")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn single_file_parse_error_exits_1() {
    // WHY: source errors (parse failures) are exit 1, not exit 2.
    ynz()
        .arg("fmt")
        .arg(fixture("parse_error.ynz"))
        .assert()
        .failure()
        .code(1);
}

// ── Check mode ────────────────────────────────────────────────────────────────

#[test]
fn check_canonical_exits_0() {
    // WHY: --check on an already-canonical file must exit 0 and not write anything.
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
}

#[test]
fn check_noncanonical_exits_1_and_prints_would_reformat() {
    // WHY: --check on a non-canonical file must exit 1 and print "Would reformat:".
    ynz()
        .arg("fmt")
        .arg("--check")
        .arg(fixture("noncanonical.ynz"))
        .assert()
        .failure()
        .code(1)
        .stderr(predicates::str::contains("Would reformat:"));
}

// ── Stdin mode ────────────────────────────────────────────────────────────────

#[test]
fn stdin_happy_path_exits_0_and_writes_stdout() {
    // WHY: --stdin reads from stdin and writes formatted output to stdout.
    let source = "function  f( )  ->  nothing  { }\n";
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
}

#[test]
fn stdin_parse_error_exits_1() {
    // WHY: stdin parse errors exit 1 and print diagnostics to stderr.
    let broken = "function broken() -> nothing {\n  let x = 1\n";
    ynz()
        .arg("fmt")
        .arg("--stdin")
        .write_stdin(broken)
        .assert()
        .failure()
        .code(1);
}

#[test]
fn stdin_parse_error_no_internal_source_label_leak() {
    // WHY: diagnostics must show `<stdin>` as the source label, not `<fmt>` (the
    // internal name used by the formatting library).  The `<fmt>` label also
    // caused ariadne to print "Unable to fetch source '<fmt>'" to stderr, which
    // is confusing user-facing noise.
    let broken = "function broken() -> nothing {\n  let x = 1\n";
    let out = ynz()
        .arg("fmt")
        .arg("--stdin")
        .write_stdin(broken)
        .assert()
        .failure()
        .code(1)
        .get_output()
        .stderr
        .clone();
    let stderr = String::from_utf8(out).unwrap();
    assert!(
        !stderr.contains("Unable to fetch source"),
        "leaked internal source label in stderr: {stderr}"
    );
}

// ── Project mode (--all) ──────────────────────────────────────────────────────

#[test]
fn all_outside_project_exits_2() {
    // WHY: --all requires a yinz.toml project root; no project = infra error.
    ynz()
        .arg("fmt")
        .arg("--all")
        .arg("/tmp")
        .assert()
        .failure()
        .code(2)
        .stderr(predicates::str::contains("yinz.toml"));
}

#[test]
fn all_inside_project_formats_all_files() {
    // WHY: --all must canonicalize every .ynz file under the project root.
    // Use a temp copy so we don't clobber the fixture.
    let dir = tempfile::tempdir().unwrap();
    let project_src = fixture("project");
    copy_dir(&project_src, dir.path()).unwrap();

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
}

#[test]
fn all_with_one_broken_file_continues_and_exits_1() {
    // WHY: --all must continue processing good files even when one has a parse error.
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("project"), dir.path()).unwrap();

    // Add a broken file to the project
    let broken = dir.path().join("broken.ynz");
    std::fs::write(&broken, "function broken() -> nothing {\n  let x = 1\n").unwrap();

    // Add a non-canonical good file
    let good = dir.path().join("reformat_me.ynz");
    std::fs::write(&good, "function  g()  ->  nothing  { }\n").unwrap();

    ynz()
        .arg("fmt")
        .arg("--all")
        .arg(dir.path())
        .assert()
        .failure()
        .code(1);

    // The good file must have been reformatted even though broken.ynz had a parse error
    let good_after = std::fs::read_to_string(&good).unwrap();
    assert!(
        good_after.starts_with("function g()"),
        "good file must be reformatted; got: {good_after:?}"
    );
}

// ── Argument conflict rejection ───────────────────────────────────────────────

#[test]
fn check_plus_stdin_rejected_by_clap() {
    // WHY: --check and --stdin are semantically incompatible: --check would need to
    // compare formatted output against the original file, but --stdin has no file.
    // Plan Step 5 requires clap to reject this combination so no ambiguous exit-code
    // semantics ship.
    ynz()
        .arg("fmt")
        .arg("--check")
        .arg("--stdin")
        .write_stdin("function f() -> nothing { }")
        .assert()
        .failure();
}

// ── Check + all combination ───────────────────────────────────────────────────

#[test]
fn check_all_noncanonical_exits_1() {
    // WHY: --check --all on a project with non-canonical files must exit 1.
    ynz()
        .arg("fmt")
        .arg("--check")
        .arg("--all")
        .arg(fixture("project"))
        .assert()
        .failure()
        .code(1)
        .stderr(predicates::str::contains("Would reformat:"));
}

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
