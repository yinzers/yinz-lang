// WHY: this is the contract for the entire compiler. If any of these tests flip
// green-to-red without an intentional behaviour change, something foundational
// broke. These tests run the actual `ynz` binary on the actual host — not a
// library shim.
//
// M2 section (below the M1 section): every M2 happy-path and failure-mode
// is covered. Each negative fixture has a committed stderr snapshot so the
// exact error message is pinned.

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

// ─── M2: happy path ───────────────────────────────────────────────────────────

#[test]
fn m2_smoke_prints_expected_stdout() {
    // WHY: this is the M2 success criterion. `0.3` verifies decimal128 exactness
    // (0.1 + 0.2 in floating-point would give 0.30000000000000004). `1763` verifies
    // integer overflow-checked arithmetic + Pratt precedence (* before -). `true`
    // verifies boolean expression lowering with short-circuit `&&`.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_smoke.ynz"));
    assert_eq!(code, 0, "M2 smoke must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout, "0.3\n1763\ntrue\n",
        "M2 smoke stdout must be exactly `0.3\\n1763\\ntrue\\n`"
    );
}

#[test]
fn m2_decimal_exactness_prints_0_3() {
    // WHY: this is the load-bearing M2 demo. Binary floating-point gives
    // 0.30000000000000004; decimal128 gives exactly 0.3. If this fails,
    // the entire premise of the `number` type is broken.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_decimal_exactness.ynz"));
    assert_eq!(code, 0, "decimal exactness must exit 0; stderr:\n{stderr}");
    assert_eq!(stdout, "0.3\n", "0.1 + 0.2 must print exactly `0.3`");
}

// ─── M2: compile-time errors ─────────────────────────────────────────────────

#[test]
fn m2_mixed_int_number_produces_diagnostic() {
    // WHY: `int + number` is a type error — Yinz has no implicit numeric coercion.
    // The diagnostic must name `.toNumber()` so the user knows what to do.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_mixed_int_number.ynz"));
    assert_ne!(code, 0, "mixed int+number must exit non-zero");
    assert!(stdout.is_empty(), "compile error must produce no stdout");
    assert!(
        stderr.contains("toNumber"),
        "diagnostic must suggest `.toNumber()`, got:\n{stderr}"
    );
    insta::assert_snapshot!("m2_mixed_int_number_stderr", stderr);
}

#[test]
fn m2_const_reassignment_produces_diagnostic() {
    // WHY: `const` cannot be reassigned. The diagnostic must say so and suggest
    // `let` as the fix. If this passes silently, the const guarantee is broken.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_const_reassign.ynz"));
    assert_ne!(code, 0, "const reassignment must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("const"),
        "diagnostic must mention `const`, got:\n{stderr}"
    );
    insta::assert_snapshot!("m2_const_reassign_stderr", stderr);
}

#[test]
fn m2_bignum_deferral_produces_diagnostic() {
    // WHY: `number[100]` is reserved syntax but not implemented until M8.
    // The diagnostic must point at M8 so the user knows it's coming.
    // The fixture is the catch-up marker — M8 updates this snapshot.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_bignum_deferral.ynz"));
    assert_ne!(code, 0, "number[N!=34] must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("v0.8") || stderr.contains("M8") || stderr.contains("34"),
        "deferral diagnostic must mention v0.8 or 34, got:\n{stderr}"
    );
    // CATCH-UP M8: when bignum lands, delete this fixture or update the snapshot.
    insta::assert_snapshot!("m2_bignum_deferral_stderr", stderr);
}

#[test]
fn m2_compound_assign_produces_diagnostic() {
    // WHY: `+=` is not part of Yinz — the lexer rejects it with a teaching
    // diagnostic suggesting `x = x + 1`. This tests the banned-syntax path
    // end-to-end through the driver.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_compound_assign.ynz"));
    assert_ne!(code, 0, "compound assignment must exit non-zero");
    assert!(stdout.is_empty());
    insta::assert_snapshot!("m2_compound_assign_stderr", stderr);
}

// ─── M2: runtime panics ───────────────────────────────────────────────────────

#[test]
fn m2_int_overflow_panics_and_exits_nonzero() {
    // WHY: i64::MAX + 1 overflows. The runtime must catch it and exit non-zero
    // with a message. If it silently wraps, the user gets wrong results with no
    // indication that anything went wrong.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_int_overflow.ynz"));
    assert_ne!(code, 0, "int overflow must exit non-zero");
    assert!(stdout.is_empty(), "panicking program must produce no stdout");
    assert!(
        !stderr.is_empty(),
        "int overflow must print a runtime error message"
    );
}

#[test]
fn m2_int_div_by_zero_panics_and_exits_nonzero() {
    // WHY: dividing by zero must produce a runtime error, not undefined behaviour.
    // The Yinz runtime checks the divisor before dividing.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_int_div_by_zero.ynz"));
    assert_ne!(code, 0, "int div-by-zero must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        !stderr.is_empty(),
        "div-by-zero must print a runtime error message"
    );
}
