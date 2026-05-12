// WHY: this is the contract for the entire compiler. If any of these tests flip
// green-to-red without an intentional behaviour change, something foundational
// broke. These tests run the actual `ynz` binary on the actual host — not a
// library shim.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn ynz_binary() -> PathBuf {
    // Build the binary relative to the workspace root, or use the pre-built
    // path set by cargo test. CARGO_BIN_EXE_ynz is set by cargo test when the
    // [[bin]] target exists in the crate's Cargo.toml.
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn run_ynz(args: &[&str]) -> Output {
    Command::new(ynz_binary())
        .args(args)
        .env("CLICOLOR", "0") // disable ANSI codes in output
        .output()
        .expect("failed to spawn ynz binary")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture(name: &str) -> PathBuf {
    fixtures_dir().join(name)
}

/// Run `ynz run <file>` and return stdout as a String.
fn ynz_run_stdout(source_path: &Path) -> (String, String, i32) {
    let out = Command::new(ynz_binary())
        .args(["run", source_path.to_str().unwrap()])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz binary");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let code = out.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

// ─── Golden path ─────────────────────────────────────────────────────────────

#[test]
fn hello_ynz_prints_hello_yinz_and_exits_zero() {
    // WHY: this is the M1 success criterion. Every other test is secondary.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("hello.ynz"));
    assert_eq!(code, 0, "exit code must be 0; stderr:\n{stderr}");
    assert_eq!(
        stdout, "hello, yinz\n",
        "stdout must be exactly `hello, yinz\\n`"
    );
}

// ─── Build subcommand ─────────────────────────────────────────────────────────

#[test]
fn build_produces_executable_and_exits_zero() {
    // WHY: `ynz build` must leave an executable on disk. If it doesn't, the
    // user can't distribute or inspect the produced binary.
    //
    // Use a temp dir to avoid racing with the ynz run tests that use the
    // same fixture file (parallel tests write to the same binary path otherwise).
    use std::os::unix::fs::PermissionsExt;

    let tmp = std::env::temp_dir().join(format!("ynz-build-test-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("create temp dir");
    let src = tmp.join("hello.ynz");
    std::fs::copy(fixture("hello.ynz"), &src).expect("copy fixture");
    let binary = src.with_extension("");

    let out = run_ynz(&["build", src.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "ynz build must exit 0; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(binary.exists(), "binary must exist after ynz build");

    let meta = std::fs::metadata(&binary).expect("binary metadata");
    let mode = meta.permissions().mode();
    assert!(mode & 0o111 != 0, "binary must be executable");

    // Clean up.
    let _ = std::fs::remove_dir_all(&tmp);
}

// ─── Error cases ─────────────────────────────────────────────────────────────

#[test]
fn broken_main_exits_nonzero_with_diagnostic() {
    // WHY: if a broken file exits 0 or produces no stderr, the user would
    // silently ship wrong code. This locks the exact error output shape.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("broken_main.ynz"));
    assert_ne!(code, 0, "broken_main must exit non-zero");
    assert!(stdout.is_empty(), "broken_main must produce no stdout");
    assert!(
        !stderr.is_empty(),
        "broken_main must produce diagnostic output"
    );
    // The diagnostic must mention the missing return type.
    assert!(
        stderr.contains("main"),
        "diagnostic must mention `main`; got:\n{stderr}"
    );
    insta::assert_snapshot!("broken_main_stderr", stderr);
}

#[test]
fn empty_source_exits_nonzero_with_missing_main_diagnostic() {
    // WHY: an empty file must produce a clear "no main" error, not a crash or
    // an empty error message.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("empty.ynz"));
    assert_ne!(code, 0, "empty source must exit non-zero");
    assert!(stdout.is_empty(), "empty source must produce no stdout");
    assert!(
        stderr.contains("main"),
        "empty source diagnostic must mention `main`; got:\n{stderr}"
    );
    insta::assert_snapshot!("empty_stderr", stderr);
}

// ─── File path with spaces ────────────────────────────────────────────────────

#[test]
fn file_path_with_spaces_runs_correctly() {
    // WHY: file paths with spaces are the canonical shell-injection and quoting
    // trap. If the driver quotes paths via format strings instead of Command::arg,
    // this test will fail (or worse, silently produce wrong output).
    let tmp = std::env::temp_dir().join("ynz test fixtures");
    std::fs::create_dir_all(&tmp).expect("create temp dir");
    let src = tmp.join("hello with spaces.ynz");
    std::fs::copy(fixture("hello.ynz"), &src).expect("copy fixture");

    let (stdout, stderr, code) = ynz_run_stdout(&src);

    let _ = std::fs::remove_file(&src);

    assert_eq!(code, 0, "path with spaces must exit 0; stderr:\n{stderr}");
    assert_eq!(stdout, "hello, yinz\n");
}

// ─── Invalid UTF-8 ──────────────────────────────────────────────────────────

#[test]
fn invalid_utf8_source_exits_nonzero_with_diagnostic() {
    // WHY: a source file with invalid UTF-8 must produce a clear error,
    // not a panic or garbled output.
    let tmp = std::env::temp_dir().join("ynz-bad-utf8.ynz");
    std::fs::write(&tmp, b"function main() -> nothing { \xff }").expect("write bad file");

    let (_, stderr, code) = ynz_run_stdout(&tmp);
    let _ = std::fs::remove_file(&tmp);

    assert_ne!(code, 0, "invalid UTF-8 must exit non-zero");
    assert!(
        !stderr.is_empty(),
        "invalid UTF-8 must produce a diagnostic"
    );
}
