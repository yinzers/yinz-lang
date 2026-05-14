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



#[test]
fn m3_fib_prints_55() {
    // WHY: M3 success criterion. fib(10) = 55. If this is wrong, recursion,
    // parameters, or return-value lowering is broken.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_fib.ynz"));
    assert_eq!(code, 0, "exit code must be 0; stderr:\n{stderr}");
    assert_eq!(stdout, "55\n", "fib(10) must print 55");
}

#[test]
fn m3_mutual_recursion_prints_0() {
    // WHY: mutual recursion requires forward declarations at the LLVM level.
    // ping(5) ping-pongs until n=0 and returns 0.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_mutual_recursion.ynz"));
    assert_eq!(code, 0, "exit code must be 0; stderr:\n{stderr}");
    assert_eq!(stdout, "0\n", "ping(5) must print 0");
}

#[test]
fn m3_while_countdown_prints_5_to_1() {
    // WHY: while loop with mutation must decrement correctly.
    // 5,4,3,2,1 on separate lines.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_while_countdown.ynz"));
    assert_eq!(code, 0, "exit code must be 0; stderr:\n{stderr}");
    assert_eq!(stdout, "5\n4\n3\n2\n1\n");
}

#[test]
fn m3_for_range_prints_0_to_4() {
    // WHY: for (i in range(0, 5)) must print 0,1,2,3,4.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_for_range.ynz"));
    assert_eq!(code, 0, "exit code must be 0; stderr:\n{stderr}");
    assert_eq!(stdout, "0\n1\n2\n3\n4\n");
}

#[test]
fn m3_multicase_int_prints_correct_arms() {
    // WHY: multi-case if on int scrutinee must dispatch to the right arm.
    // describe(1)=one, describe(2)=two, describe(5)=other (else arm).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_multicase_int.ynz"));
    assert_eq!(code, 0, "exit code must be 0; stderr:\n{stderr}");
    assert_eq!(stdout, "one\ntwo\nother\n");
}

#[test]
fn m3_multicase_string_matches_hello() {
    // WHY: multi-case if on string scrutinee uses ynz_string_eq (byte equality).
    // "hello" matches the first arm; must print "got hello".
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_multicase_string.ynz"));
    assert_eq!(code, 0, "exit code must be 0; stderr:\n{stderr}");
    assert_eq!(stdout, "got hello\n");
}

#[test]
fn m3_early_return_prints_sign_values() {
    // WHY: early return inside an `if` body must terminate the function path.
    // sign(42)=1, sign(-7)=-1, sign(0)=0.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_early_return.ynz"));
    assert_eq!(code, 0, "exit code must be 0; stderr:\n{stderr}");
    assert_eq!(stdout, "1\n-1\n0\n");
}

#[test]
fn m3_for_nested_loop_prints_sums() {
    // WHY: nested for loops require separate counter allocas per scope.
    // i+j for i in 0..3, j in 0..3 = 0,1,2,1,2,3,2,3,4.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_for_nested.ynz"));
    assert_eq!(code, 0, "exit code must be 0; stderr:\n{stderr}");
    assert_eq!(stdout, "0\n1\n2\n1\n2\n3\n2\n3\n4\n");
}

#[test]
fn m3_multicase_else_arm_dispatches_correctly() {
    // WHY: multi-case `if` with `else =>` catch-all must dispatch to the right
    // arm. classify(1)=10, classify(2)=20, classify(99)=0 (else arm).
    // Also validates that all-arms-return makes the merge_bb unreachable without
    // triggering an LLVM verify error.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_multicase_else.ynz"));
    assert_eq!(code, 0, "exit code must be 0; stderr:\n{stderr}");
    assert_eq!(stdout, "10\n20\n0\n");
}


#[test]
fn m3_param_mutation_produces_m4_deferral_diagnostic() {
    // WHY: assigning to a parameter must error with a three-part diagnostic
    // pointing at M4 (when `lend` lands). If this passes silently, the
    // read-only-param guarantee is gone.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_param_mutation.ynz"));
    assert_ne!(code, 0, "param mutation must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("parameter"),
        "diagnostic must mention 'parameter', got:\n{stderr}"
    );
    assert!(
        stderr.contains("milestone 4"),
        "deferral must name M4, got:\n{stderr}"
    );
}

#[test]
fn m3_missing_return_produces_diagnostic() {
    // WHY: a `-> int` function with no return on all paths must error.
    // Silent acceptance would mean the function exits with an undefined value.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_missing_return.ynz"));
    assert_ne!(code, 0, "missing return must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("foo"),
        "diagnostic must name the function, got:\n{stderr}"
    );
}

#[test]
fn m3_return_without_value_in_int_fn_produces_diagnostic() {
    // WHY: bare `return` in a `-> int` function promised a value but returned
    // nothing. Must error so the caller doesn't receive a garbage int.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_return_no_value_in_int.ynz"));
    assert_ne!(code, 0, "return without value in int fn must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("return") || stderr.contains("value"),
        "diagnostic must mention return/value, got:\n{stderr}"
    );
}

#[test]
fn m3_return_value_in_nothing_fn_produces_diagnostic() {
    // WHY: `return 42` in a `-> nothing` function contradicts the declaration.
    // Must error so callers don't expect a value where none is promised.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_return_value_in_nothing.ynz"));
    assert_ne!(code, 0, "return value in nothing fn must exit non-zero");
    assert!(stdout.is_empty());
}

#[test]
fn m3_dead_code_produces_warning_but_exits_zero() {
    // WHY: dead code is a warning, not an error — the program compiles and runs.
    // The warning is informational. If this exits non-zero, the severity mapping
    // between Warning and Error is broken.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_dead_code.ynz"));
    assert_eq!(code, 0, "dead code warning must exit 0; stderr:\n{stderr}");
    assert_eq!(stdout, "1\n");
    assert!(
        stderr.contains("never run") || stderr.contains("unreachable"),
        "warning must mention unreachable code, got:\n{stderr}"
    );
}

#[test]
fn m3_duplicate_function_produces_diagnostic() {
    // WHY: two functions named `foo` must error. Silent acceptance would mean
    // one definition silently shadows the other.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_duplicate_function.ynz"));
    assert_ne!(code, 0, "duplicate function must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("foo"),
        "diagnostic must name the duplicate function, got:\n{stderr}"
    );
}

#[test]
fn m3_undefined_function_suggests_levenshtein() {
    // WHY: `mann()` is close to `main`. The Levenshtein suggestion must fire.
    // Without it, the user gets a bare "not defined" with no hint of what they
    // probably meant to write.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_undefined_function.ynz"));
    assert_ne!(code, 0, "undefined function must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("main"),
        "Levenshtein must suggest `main` for `mann`, got:\n{stderr}"
    );
}

#[test]
fn m3_arg_type_mismatch_produces_diagnostic() {
    // WHY: passing `int` where `string` is expected must error. Silent
    // acceptance would produce a wrong-type runtime value.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_arg_type_mismatch.ynz"));
    assert_ne!(code, 0, "arg type mismatch must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("int") || stderr.contains("string"),
        "diagnostic must mention the types, got:\n{stderr}"
    );
}

#[test]
fn m3_arg_arity_mismatch_produces_diagnostic() {
    // WHY: calling `add(1, 2, 3)` when it takes 2 args must error.
    // Silent acceptance would produce undefined behavior in codegen.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_arg_arity_mismatch.ynz"));
    assert_ne!(code, 0, "arg arity mismatch must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("add"),
        "diagnostic must name the function, got:\n{stderr}"
    );
}

#[test]
fn m3_loop_var_mutation_produces_diagnostic() {
    // WHY: assigning to the `for` loop variable inside the body must error.
    // The loop counter is managed by the runtime; letting user code overwrite it
    // would produce confusing iteration behavior.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_loop_var_mutation.ynz"));
    assert_ne!(code, 0, "loop var mutation must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("loop variable") || stderr.contains("i"),
        "diagnostic must identify the loop variable, got:\n{stderr}"
    );
}


#[test]
fn m3_is_type_arm_produces_m6_deferral() {
    // WHY: `is Circle =>` pattern matching is M6 work (union types).
    // The diagnostic must point at M6 so the user knows when to expect it.
    // If this compiles silently, we've shipped a half-baked feature.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_is_type_deferral.ynz"));
    assert_ne!(code, 0, "is-type deferral must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("milestone 6") || stderr.contains("M6"),
        "deferral must name M6, got:\n{stderr}"
    );
}

#[test]
fn m3_share_param_produces_m4_deferral() {
    // WHY: ownership annotations (`share`, `lend`, `give`) on parameters are M4.
    // The parser emits a deferral diagnostic and recovers (param still works).
    // This fixture tests both the deferral AND the recovery (function is callable).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_share_param_deferral.ynz"));
    assert_ne!(code, 0, "share param deferral must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("milestone 4") || stderr.contains("M4"),
        "deferral must name M4, got:\n{stderr}"
    );
}

#[test]
fn m3_range_outside_for_produces_m7_deferral() {
    // WHY: `let r = range(0, 5)` is M7 (Iterable[T] protocol).
    // The diagnostic must point at M7. This also confirms Range is restricted
    // to the for-loop iterable position in M3.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_range_outside_for_deferral.ynz"));
    assert_ne!(code, 0, "range-outside-for deferral must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("milestone 7") || stderr.contains("M7"),
        "deferral must name M7, got:\n{stderr}"
    );
}

#[test]
fn m3_match_keyword_produces_teaching_diagnostic() {
    // WHY: `match` is not a Yinz keyword — the lexer must teach the user to
    // use multi-case `if` instead. The identifier `match` is still valid for
    // recovery, so the parser doesn't hard-fail.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_match_keyword_banned.ynz"));
    assert_ne!(code, 0, "match keyword must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("match") || stderr.contains("if"),
        "teaching diagnostic must reference match or if, got:\n{stderr}"
    );
}

#[test]
fn m3_switch_keyword_produces_teaching_diagnostic() {
    // WHY: `switch` is not a Yinz keyword. Same teaching pattern as `match`.
    // Both are flagged because developers coming from C/JS/Rust expect them.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_switch_keyword_banned.ynz"));
    assert_ne!(code, 0, "switch keyword must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("switch") || stderr.contains("if"),
        "teaching diagnostic must reference switch or if, got:\n{stderr}"
    );
}
