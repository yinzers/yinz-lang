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

/// Assert a stderr snapshot with machine-independent fixture paths.
///
/// The compiler echoes the absolute path of the source file into its diagnostics
/// (e.g. `╭─[ /abs/path/crates/ynz-driver/tests/fixtures/broken_main.ynz:1:1 ]`). That
/// absolute prefix differs by checkout location (local `/workspaces/...` vs CI
/// `/home/runner/work/...`), so a raw snapshot only matches on the machine that recorded
/// it. This filter rewrites any `.../tests/fixtures/` prefix to `[FIXTURES]/` so the
/// pinned error text is portable across every host.
fn assert_stderr_snapshot(name: &str, stderr: &str) {
    let mut settings = insta::Settings::clone_current();
    settings.add_filter(r"\S*/tests/fixtures/", "[FIXTURES]/");
    settings.bind(|| {
        insta::assert_snapshot!(name, stderr);
    });
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
        stderr.contains("entrypoint"),
        "diagnostic must mention `entrypoint`; got:\n{stderr}"
    );
    assert_stderr_snapshot("broken_main_stderr", &stderr);
}

#[test]
fn empty_source_exits_nonzero_with_missing_entrypoint_diagnostic() {
    // WHY: an empty file must produce a clear "no entrypoint" error, not a crash or
    // an empty error message.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("empty.ynz"));
    assert_ne!(code, 0, "empty source must exit non-zero");
    assert!(stdout.is_empty(), "empty source must produce no stdout");
    assert!(
        stderr.contains("entrypoint"),
        "empty source diagnostic must mention `entrypoint`; got:\n{stderr}"
    );
    assert_stderr_snapshot("empty_stderr", &stderr);
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
    std::fs::write(&tmp, b"function entrypoint() -> nothing { \xff }").expect("write bad file");

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
    assert_stderr_snapshot("m2_mixed_int_number_stderr", &stderr);
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
    assert_stderr_snapshot("m2_const_reassign_stderr", &stderr);
}

#[test]
fn m8_bignum_number100_runs() {
    // WHY: M8 P6 ships bignum — `number<100>` must now compile and run successfully.
    // This test was previously called `m2_bignum_deferral_produces_diagnostic` and
    // expected a compile error. The CATCH-UP M8 marker is now resolved.
    //
    // test-ratchet: M8 P6 flips this from "expect non-zero exit" to "expect success".
    // The fixture (m2_bignum_deferral.ynz) contains `let x: number<100> = 1.0; print(x)`.
    let (stdout, _stderr, code) = ynz_run_stdout(&fixture("m2_bignum_deferral.ynz"));
    assert_eq!(code, 0, "number<100> must compile and run in M8");
    assert_eq!(
        stdout.trim(),
        "1.0",
        "number<100> literal 1.0 must print 1.0"
    );
}

#[test]
fn m2_compound_assign_produces_diagnostic() {
    // WHY: `+=` is not part of Yinz — the lexer rejects it with a teaching
    // diagnostic suggesting `x = x + 1`. This tests the banned-syntax path
    // end-to-end through the driver.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_compound_assign.ynz"));
    assert_ne!(code, 0, "compound assignment must exit non-zero");
    assert!(stdout.is_empty());
    assert_stderr_snapshot("m2_compound_assign_stderr", &stderr);
}

#[test]
fn m2_int_overflow_panics_and_exits_nonzero() {
    // WHY: i64::MAX + 1 overflows. The runtime must catch it and exit non-zero
    // with a message. If it silently wraps, the user gets wrong results with no
    // indication that anything went wrong.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_int_overflow.ynz"));
    assert_ne!(code, 0, "int overflow must exit non-zero");
    assert!(
        stdout.is_empty(),
        "panicking program must produce no stdout"
    );
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

// ── M4 integration tests ─────────────────────────────────────────────────────

#[test]
fn m4_p5_wrapping_add_closes_m2_catchup() {
    // WHY: M4 P5 closes the M2 catch-up obligation for overflow escape methods.
    // int.max.wrappingAdd(1) must produce int.min, not panic. If it panics, the
    // wrapping intrinsic lowering or LLVM overflow-check bypass is broken.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_wrapping_add_deferred.ynz"));
    assert_eq!(code, 0, "wrapping add must succeed; stderr:\n{stderr}");
    assert_eq!(
        stdout, "-9223372036854775808\n",
        "int.max + 1 must wrap to int.min"
    );
}

#[test]
fn m4_p5_int_max_constant_closes_m2_catchup() {
    // WHY: M4 P5 closes the M2 catch-up for type-attached constants.
    // `int.max` and `int.min` must compile to the correct i64 immediate values.
    // If the type-attached constant interception in typeck/codegen is broken,
    // this produces a compile error ("int is not defined").
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_int_max_deferred.ynz"));
    assert_eq!(code, 0, "int.max constant must compile; stderr:\n{stderr}");
    assert_eq!(
        stdout, "9223372036854775807\n-9223372036854775808\n",
        "int.max and int.min must print the correct i64 extremes"
    );
}

// ── M4 P6 positive fixtures ───────────────────────────────────────────────────

#[test]
fn m4_inheritance_extends_prepends_parent_fields() {
    // WHY: `extends` must make parent fields accessible on the child shape.
    // If field layout merging is broken, Dog.name (from Animal) would be
    // missing and the print would segfault or produce garbage.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_inheritance.ynz"));
    assert_eq!(
        code, 0,
        "inheritance fixture must compile; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "Rex\nHusky\n");
}

#[test]
fn m4_follows_contract_dispatch_works() {
    // WHY: `follows` contract verification must allow calling the function
    // via UFCS. If the follows check rejects valid code or UFCS lookup is
    // broken for contract methods, this fails to compile.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_follows.ynz"));
    assert_eq!(code, 0, "follows fixture must compile; stderr:\n{stderr}");
    assert_eq!(stdout, "3\n4\n");
}

#[test]
fn m4_hidden_field_accessible_inside_method() {
    // WHY: hidden fields must be readable and writable inside the shape's own
    // methods but invisible outside. This tests the read path (value()) and
    // write path (increment()) for a field that callers can't see.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_hidden_field.ynz"));
    assert_eq!(
        code, 0,
        "hidden field fixture must compile; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "3\n");
}

#[test]
fn m4_copy_produces_independent_struct() {
    // WHY: `.copy()` on a shape with all-primitive fields must produce an
    // independent allocation. Both values must be readable after the copy.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_copy.ynz"));
    assert_eq!(code, 0, "copy fixture must compile; stderr:\n{stderr}");
    assert_eq!(stdout, "10\n20\n10\n20\n");
}

#[test]
fn m4_base_shape_blocks_direct_instantiation() {
    // WHY: a derived shape from a `base shape` must work (base fields are
    // inherited); only direct instantiation of the base is blocked (tested
    // separately in the negative suite).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_base_shape.ynz"));
    assert_eq!(
        code, 0,
        "base shape fixture must compile; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "Rust\n120\n");
}

#[test]
fn m4_type_constants_and_wrapping_saturation() {
    // WHY: int.max / int.min type-attached constants + saturatingAdd must
    // clamp at INT64_MAX and wrappingAdd must two's-complement-wrap.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_type_constants.ynz"));
    assert_eq!(
        code, 0,
        "type constants fixture must compile; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout,
        "9223372036854775807\n-9223372036854775808\n9223372036854775807\n-9223372036854775808\n"
    );
}

// ── M4 P6 negative fixtures ───────────────────────────────────────────────────

#[test]
fn m4_neg_use_after_give_is_compile_error() {
    // WHY: using a value after giving it away is a memory-safety violation.
    // The ownership analysis must catch it at compile time — never at runtime.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_neg_use_after_give.ynz"));
    assert_ne!(code, 0, "use-after-give must be rejected");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("given away"),
        "error must name the give site; got:\n{stderr}"
    );
}

#[test]
fn m4_neg_const_cannot_be_lent_for_mutation() {
    // WHY: `const` bindings are fully immutable. Passing one to a function
    // that declares `lend` (mutable access) must be a compile-time error.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_neg_const_cannot_lend.ynz"));
    assert_ne!(code, 0, "const-lend must be rejected");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("const"),
        "error must mention const; got:\n{stderr}"
    );
}

#[test]
fn m4_neg_const_field_assign_is_compile_error() {
    // WHY: assigning to a field of a `const` binding violates deep immutability.
    // This specifically tests the field-write path (distinct from rebind).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_neg_const_field_assign.ynz"));
    assert_ne!(code, 0, "const field assign must be rejected");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("const"),
        "error must mention const; got:\n{stderr}"
    );
}

#[test]
fn m4_neg_base_shape_cannot_be_instantiated() {
    // WHY: `base shape` declarations are abstract — only derived shapes can
    // be constructed. Attempting direct instantiation must be a compile error.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_neg_base_instantiate.ynz"));
    assert_ne!(code, 0, "base shape instantiation must be rejected");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("base shape"),
        "error must name the constraint; got:\n{stderr}"
    );
}

#[test]
fn m4_neg_struct_missing_required_field_is_error() {
    // WHY: every non-hidden field must be provided in a struct literal. Missing
    // one is a compile error, not a runtime default — there are no implicit defaults.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_neg_struct_missing_field.ynz"));
    assert_ne!(code, 0, "missing field must be rejected");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("health") || stderr.contains("Missing"),
        "error must name the field; got:\n{stderr}"
    );
}

#[test]
fn m4_neg_struct_wrong_field_type_is_error() {
    // WHY: the struct literal typechecker must verify field types against the
    // shape declaration. A string where int is expected is a type error.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_neg_struct_wrong_type.ynz"));
    assert_ne!(code, 0, "wrong field type must be rejected");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("int") || stderr.contains("expects"),
        "error must name the expected type; got:\n{stderr}"
    );
}

#[test]
fn m4_neg_hidden_field_access_outside_shape_is_error() {
    // WHY: hidden fields are encapsulation — readable only inside the declaring
    // shape's methods. External access is a compile error, never a runtime panic.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_neg_hidden_field_access.ynz"));
    assert_ne!(code, 0, "hidden field access must be rejected");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("hidden"),
        "error must explain the hidden constraint; got:\n{stderr}"
    );
}

#[test]
fn m4_neg_follows_missing_function_is_error() {
    // WHY: `follows` is verified at compile time — if the required function
    // isn't defined, the shape declaration itself is an error.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_neg_follows_missing_fn.ynz"));
    assert_ne!(code, 0, "missing contract function must be rejected");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("missing") || stderr.contains("draw"),
        "error must name the missing function; got:\n{stderr}"
    );
}

#[test]
fn m4_neg_cyclic_extends_is_error() {
    // WHY: cyclic inheritance (A extends B extends A) cannot be laid out in
    // memory — it's an infinite-size type. The compiler must catch it.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_neg_cyclic_extends.ynz"));
    assert_ne!(code, 0, "cyclic extends must be rejected");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("cyclic") || stderr.contains("cycle"),
        "error must describe the cycle; got:\n{stderr}"
    );
}

#[test]
fn m4_neg_banned_type_keyword_is_error() {
    // WHY: `type` is banned in favor of `shape`. Any program using it must
    // fail to compile — it should never silently pass as an identifier.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_neg_banned_type_kw.ynz"));
    assert_ne!(
        code, 0,
        "`type` keyword must be rejected; stderr:\n{stderr}"
    );
    assert!(stdout.is_empty());
}

#[test]
fn m4_player_shape_compiles_and_produces_correct_output() {
    // WHY: M4 P4 success criterion. Exercises shape struct literals, field access,
    // UFCS dispatch (share/lend/give self), and lend mutation. If any codegen path
    // for shapes is broken, this fails with a compile error or wrong stdout.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m4_player.ynz"));
    assert_eq!(
        code, 0,
        "m4_player must compile and run cleanly; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "Patrick\n120\nPatrick\n",
        "greet prints name, health is 120 after heal, consume prints name"
    );
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
    // preserving the read-only-param guarantee. If this passes silently, the
    // read-only contract is gone.
    //
    // test-ratchet: M4 shipped — the old WHY text said "arrive in v0.1 milestone 4"
    // which was future-tense and incorrect. Updated to check the accurate current
    // message ("parameters are read-only by default") instead of the stale M4 reference.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_param_mutation.ynz"));
    assert_ne!(code, 0, "param mutation must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("parameter"),
        "diagnostic must mention 'parameter', got:\n{stderr}"
    );
    assert!(
        stderr.contains("read-only"),
        "diagnostic must mention read-only semantics, got:\n{stderr}"
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
    // WHY: `entrpoint()` is distance-1 from `entrypoint`. The Levenshtein suggestion must fire.
    // Without it, the user gets a bare "not defined" with no hint of what they
    // probably meant to write.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_undefined_function.ynz"));
    assert_ne!(code, 0, "undefined function must exit non-zero");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("entrypoint"),
        "Levenshtein must suggest `entrypoint` for `entrpoint`, got:\n{stderr}"
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
fn m3_is_type_deferral_now_runnable_in_m6() {
    // WHY: `m3_is_type_deferral.ynz` was a deferral fixture in M3.
    // M6 P5 ships the runnable replacement (Circle | Square union demo).
    // The fixture must now compile and run cleanly with correct output.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_is_type_deferral.ynz"));
    assert_eq!(
        code, 0,
        "union fixture must compile and run; stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("circle"),
        "expected 'circle' in output, got: {stdout}"
    );
    assert!(
        stdout.contains("square"),
        "expected 'square' in output, got: {stdout}"
    );
}

#[test]
fn m4_share_param_compiles_and_runs() {
    // WHY: M4 ships ownership annotations (`share`, `lend`, `give`) on parameters.
    // This fixture previously tested the M3 deferral; now it tests M4 success:
    // the share param compiles cleanly and the function is callable.
    // If this regresses to a deferral or error, the ownership system is broken.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_share_param_deferral.ynz"));
    assert_eq!(
        code, 0,
        "share param must compile and run in M4; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "world\n", "greet(n) must print `world`");
}

#[test]
fn m3_range_outside_for_now_supported_in_m7() {
    // WHY: `let r = range(0, 5)` was deferred to M7; M7 P4c ships first-class Range
    // values. This test was previously a "deferral must error" guard; now it verifies
    // the feature actually works. A stored range that produces no for-loop output
    // must compile and run cleanly. If the M7 P4c range codegen is broken, this
    // would exit non-zero or produce wrong output.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_range_outside_for_deferral.ynz"));
    assert_eq!(
        code, 0,
        "range as first-class value must compile in M7; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "done\n");
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

// ── M5: Generics + Collections + Maybe ───────────────────────────────────────

#[test]
fn m5_identity_generic_fn_prints_42() {
    // WHY: M5 P4a acceptance criterion. identity<T>(give value: T) -> T is the
    // simplest generic function. If monomorphization is broken, the call either
    // fails to compile or produces wrong output.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m5_identity.ynz"));
    assert_eq!(
        code, 0,
        "m5_identity must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "42\n");
}

#[test]
fn m5_maybe_none_or_and_exists() {
    // WHY: M5 P4a acceptance criterion. Exercises none literal, .or(default),
    // and flow-sensitive .exists() guard. Wrong output means maybe<T> LLVM lowering
    // ({i64,i64} struct) is broken.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m5_maybe.ynz"));
    assert_eq!(code, 0, "m5_maybe must compile and run; stderr:\n{stderr}");
    assert_eq!(stdout, "99\n99\n");
}

#[test]
fn m5_array_add_count_get() {
    // WHY: M5 P4a acceptance criterion. Exercises array<int> literal, .add(),
    // .count(), and bracket index access returning maybe<int>. Wrong output means
    // ynz_array_* runtime symbols or the array codegen path is broken.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m5_array.ynz"));
    assert_eq!(code, 0, "m5_array must compile and run; stderr:\n{stderr}");
    assert_eq!(stdout, "4\n1\n4\n");
}

#[test]
fn m5_fixed_count_and_get() {
    // WHY: M5 P4a acceptance criterion. Exercises fixed<int> stack allocation,
    // .count() (compile-time constant), and bracket index access with bounds check.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m5_fixed.ynz"));
    assert_eq!(code, 0, "m5_fixed must compile and run; stderr:\n{stderr}");
    assert_eq!(stdout, "3\n10\n30\n");
}

#[test]
fn m5_map_get_set_count() {
    // WHY: M5 P4b acceptance criterion. Exercises map<string,int> literal, bracket
    // read, bracket write, .count(). If SipHash init or Swiss Tables lookup is
    // broken, values will be wrong or the binary will abort.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m5_map.ynz"));
    assert_eq!(code, 0, "m5_map must compile and run; stderr:\n{stderr}");
    assert_eq!(stdout, "90\n85\n95\n2\n");
}

#[test]
fn m5_ufcs_const_lend_errors_same_as_free_fn_form() {
    // WHY: `const p; p.heal(20)` where `heal(lend self: Player)` must produce
    //      the same "cannot lend a const binding" error as `heal(p, 20)`.
    //      The UFCS dot-call form and the function-call form must enforce identical
    //      ownership rules — any divergence is a silent ownership-safety gap.
    let (_stdout, stderr, code) = ynz_run_stdout(&fixture("m5_ufcs_const_lend_error.ynz"));
    assert_ne!(code, 0, "UFCS lend on const must fail");
    assert!(
        stderr.contains("const") && stderr.contains("heal"),
        "error must mention both the const binding and the function name; stderr:\n{stderr}"
    );
}

#[test]
fn m5_ufcs_and_freefn_const_lend_produce_byte_identical_diagnostics() {
    // WHY: enforces the shared-wording invariant from design/ide-hints.md.
    //      If anyone tweaks one format!() site in check_arg_ownership without
    //      touching the other, this test goes red — "manually verified" rots;
    //      this is the tripwire.  Both call forms must produce word-for-word
    //      identical diagnostic text (modulo span line numbers which differ
    //      between fixtures by design).
    let (_, ufcs_stderr, ufcs_code) = ynz_run_stdout(&fixture("m5_ufcs_const_lend_error.ynz"));
    let (_, freefn_stderr, freefn_code) =
        ynz_run_stdout(&fixture("m5_freefn_const_lend_error.ynz"));
    assert_ne!(ufcs_code, 0, "UFCS form must fail");
    assert_ne!(freefn_code, 0, "free-fn form must fail");

    // Strip ariadne span/context lines that differ between fixtures (file paths,
    // line numbers, source-code snippets, underlines).  Compare only the human-
    // readable diagnostic message text (the "Error: ..." prefix lines and the
    // indented note/label lines).
    let normalize = |s: &str| -> String {
        s.lines()
            .filter(|l| {
                let t = l.trim();
                // ariadne box-drawing and span lines
                !t.starts_with("╭─[")
                    && !t.starts_with("────")
                    && !t.is_empty()
                    // "│ " lines: ariadne renders source-context, arrows, empty filler
                    && !t.starts_with("│")
                    // Skip ariadne numeric line references " 15 │ ..."
                    && !t.contains(" │ ")
                    && !t.contains(" │  ")
                    // tail boilerplate
                    && !t.starts_with("If any of these")
                    && !t.starts_with("https://")
                    // span paths
                    && !t.contains(".ynz:")
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    assert_eq!(
        normalize(&ufcs_stderr),
        normalize(&freefn_stderr),
        "UFCS and free-fn forms must produce byte-identical diagnostic text"
    );
}

#[test]
fn m5_ufcs_const_share_still_compiles() {
    // WHY: `const p; p.greet()` where `greet(share self: Player)` must compile.
    //      The ownership check must NOT over-reject `share` (read-only) methods —
    //      `const` bindings CAN be shared, only `lend` (mutation) is blocked.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m5_ufcs_const_share_ok.ynz"));
    assert_eq!(
        code, 0,
        "UFCS share on const must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "hero\n");
}

#[test]
fn m5_ufcs_mixed_calls_granular_rejection() {
    // WHY: in the same function body, `greet(p)` (share, ok) must compile and
    //      `p.heal(10)` (lend, const) must error.  Verifies the ownership check
    //      is per-call, not a blanket rejection of all calls on const bindings.
    let (_stdout, stderr, code) = ynz_run_stdout(&fixture("m5_ufcs_mixed_calls.ynz"));
    assert_ne!(code, 0, "lend call on const must fail; stderr:\n{stderr}");
    assert!(
        stderr.contains("heal") && !stderr.contains("greet"),
        "only the lend call (heal) must error, not the share call (greet); stderr:\n{stderr}"
    );
}

#[test]
fn m5_dyn_dispatch_concrete_shape_follows_contract_accepted() {
    // WHY: a shape that declares `follows Contract` must be accepted at a `dynamic Contract`
    //      call site without a typeck error.  Only shapes that declare `follows` qualify;
    //      shapes that do NOT declare `follows` must still be rejected (pinned separately).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m5_dyn_dispatch_coerce_happy.ynz"));
    assert_eq!(
        code, 0,
        "dynamic coerce happy path must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "accepted\n");
}

#[test]
fn m5_dyn_dispatch_no_follows_still_errors() {
    // WHY: the coerce must NOT over-accept — only shapes with `follows Contract` qualify for
    //      a `dynamic Contract` parameter.  A shape without `follows` must remain a typeck
    //      error.  This pins the negative direction; the happy-path test pins the positive.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m5_dyn_dispatch_coerce_no_follows.ynz"));
    assert_ne!(
        code, 0,
        "passing a non-following shape must fail; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("Enemy") && stderr.contains("dynamic Printable"),
        "error must name both the rejected shape and the required contract; stderr:\n{stderr}"
    );
}

#[test]
fn m5_dyn_dispatch_chained_both_calls_succeed() {
    // WHY: a concrete shape passed through two function calls each accepting `dynamic Contract`
    //      must pass through correctly — verifies the coerce works at multiple call sites.
    //      Neither `showDynamic` nor `relay` independently suspend (no sleep), so no
    //      can't-infer error fires under the design-correct current_fn_suspends gate. A non-
    //      suspending caller with dynamic dispatch compiles clean per design/future/concurrency.md.
    // test-ratchet: restoring exit-0 assertion — round-2 changed this to expect a can't-infer
    // error (exit nonzero), but that was the over-firing gate behavior. Under the reverted gate
    // non-suspending dynamic callers compile clean (Phase-6 round-3).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m5_dyn_dispatch_coerce_chained.ynz"));
    assert_eq!(
        code, 0,
        "chained dynamic coerce (non-suspending fns) must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "accepted\nrelayed\n");
}

#[test]
fn m5_hidden_default_string_evaluates_correctly() {
    // WHY: `shape Foo { hidden label: string = "default" }` constructed via `Foo {}`
    //      must produce a value where `label == "default"`, not a null pointer or empty string.
    //      Hidden-field defaults are part of the shape's contract; silently zero-initing them
    //      is a silent-wrong-output bug class that only surfaces at runtime (no compile error).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m5_hidden_default_string.ynz"));
    assert_eq!(
        code, 0,
        "m5_hidden_default_string must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "default_label\n",
        "hidden string default must print the literal value, not null"
    );
}

#[test]
fn m5_hidden_default_int_evaluates_correctly() {
    // WHY: non-zero integer hidden defaults must be evaluated, not zero-inited.
    //      A shape with `hidden threshold: int = 42` must have threshold == 42
    //      in every constructed instance, not 0.  Zero-init is only correct for
    //      explicit `= 0` defaults.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m5_hidden_default_int.ynz"));
    assert_eq!(
        code, 0,
        "m5_hidden_default_int must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "42\n",
        "hidden int default 42 must print 42, not 0 (zero-init regression)"
    );
}

#[test]
fn m5_hidden_default_nested_evaluates_both_parent_and_own() {
    // WHY: when a shape `extends` a parent that also declares hidden fields, BOTH
    //      the parent's hidden defaults AND the child's own hidden defaults must be
    //      evaluated at construction time.  Inherited hidden fields live in the parent's
    //      AST ShapeDecl; a fix that only walks the child's declaration misses them.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m5_hidden_default_nested.ynz"));
    assert_eq!(
        code, 0,
        "m5_hidden_default_nested must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "10\n99\n",
        "both parent hidden default (10) and own hidden default (99) must print correctly"
    );
}

// ── M6: options, unions, fallible conversions ──────────────────────────────────

#[test]
fn m6_string_to_int_catch_up() {
    // WHY: M6 P5 closes the M2 catch-up obligation for string.toInt().
    // Exercises the locked parsing rules (whitespace strip, sign accept, hex reject,
    // fractional reject). Each line's result must match the locked test vector.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_string_to_int.ynz"));
    assert_eq!(
        code, 0,
        "m2_string_to_int must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "42\n17\n-99\n-1\n-1\n-1\n-1\n");
}

#[test]
fn m6_string_to_float_catch_up() {
    // WHY: M6 P5 closes the M2 catch-up obligation for string.toFloat().
    // Verifies that valid floats return some(true) and invalid inputs return none(false).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m2_string_to_float.ynz"));
    assert_eq!(
        code, 0,
        "m2_string_to_float must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "true\nfalse\nfalse\ntrue\n");
}

#[test]
fn m6_union_is_narrowing_runnable() {
    // WHY: m3_is_type_deferral.ynz is now the runnable M6 union demo.
    // Verifies that `is Circle =>` and `is Square =>` correctly narrow union values.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m3_is_type_deferral.ynz"));
    assert_eq!(
        code, 0,
        "union demo must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "circle\nsquare\n");
}

// ── M7 P4a: errors codegen ─────────────────────────────────────────────────────

#[test]
fn m7_errors_basic_success_path() {
    // WHY: M7 P4a acceptance criterion. An errors-capable function that always
    // succeeds must produce the success value when called with .or(). If the
    // {error_ptr, success_val} ABI or .or() codegen is broken, output is "fallback"
    // instead of "ok". The output "ok" proves the success bits were correctly stored
    // and extracted.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_errors_basic.ynz"));
    assert_eq!(
        code, 0,
        "m7_errors_basic must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "ok\n");
}

#[test]
fn m7_errors_failed_check_false_when_success() {
    // WHY: .failed() must return false (0) when the errors-capable function succeeds.
    // Tests that the error-pointer extraction is correct: field 0 of {i64, i64}
    // must be 0 for success. If .failed() returns true on success, the if body would
    // execute and print "error!" — this test would fail with extra output.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_errors_failed_check.ynz"));
    assert_eq!(
        code, 0,
        "m7_errors_failed_check must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "hello\n");
}

#[test]
fn m7_errors_propagate_two_level() {
    // WHY: two-level errors propagation. outer() calls inner() (both errors-capable).
    // main catches with .or(). Verifies that inner()'s success value flows through
    // outer()'s {i64, i64} return struct to main's .or() extraction.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_errors_propagate.ynz"));
    assert_eq!(
        code, 0,
        "m7_errors_propagate must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "propagated\n");
}

#[test]
fn m7_errors_int_return_type() {
    // WHY: errors-capable functions returning int (not string) must also work.
    // Verifies that the success_val field 1 of {i64, i64} correctly carries non-pointer
    // scalars. If only string (pointer) values work but int values are broken,
    // this test catches it.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_errors_int.ynz"));
    assert_eq!(
        code, 0,
        "m7_errors_int must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "42\n");
}

// ── M7 P4b: string runtime + codegen integration tests ───────────────────────

#[test]
fn m7_string_methods_tolower_toupper_split() {
    // WHY: M7 P4b acceptance criterion. toLowerCase, toUpperCase, and split must all
    // emit correct runtime calls. If any of the three string method runtime functions
    // (ynz_string_to_lower, ynz_string_to_upper, ynz_string_split + ynz_array_count)
    // have a codegen or ABI bug, the output will differ.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_string_methods.ynz"));
    assert_eq!(
        code, 0,
        "m7_string_methods must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "hello, world!\nHELLO, WORLD!\n2\n");
}

#[test]
fn m7_string_interpolation_with_expressions() {
    // WHY: M7 P4b acceptance criterion. String interpolation with ${expr} parts must
    // use the ynz_string_builder_* runtime sequence (new, N×append, finalize).
    // If the builder codegen is wrong, the output will be missing segments, garbled,
    // or segfault. The expected output has literal text and two interpolated values.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_string_interpolation.ynz"));
    assert_eq!(
        code, 0,
        "m7_string_interpolation must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "Hello Alice, your score is 42!\n");
}

#[test]
fn m7_nfc_equality_same_bytes() {
    // WHY: M7 P4b / M3 catch-up. ynz_string_eq must use NFC normalization.
    // For same-bytes ASCII strings (the fast path), equality must hold.
    // If ynz_string_eq is not called at all (e.g., if == falls through to a missing
    // codegen path), the program would either crash or skip the branch.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_nfc_equality.ynz"));
    assert_eq!(
        code, 0,
        "m7_nfc_equality must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "equal\n");
}

// ── M7 P4c: Iterable codegen — string iteration, first-class range, user shape ─

#[test]
fn m7_for_string_iterates_code_points() {
    // WHY: M7 P4c acceptance criterion. `for (c in s)` must use ynz_string_count +
    // ynz_string_codepoint_at. If only the array/range paths work but string iteration
    // is not wired, the compiler produces a codegen error. Counting 5 chars in "hello"
    // verifies the loop runs the correct number of times.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_for_string.ynz"));
    assert_eq!(
        code, 0,
        "m7_for_string must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "5\n");
}

#[test]
fn m7_range_first_class_storable_and_iterable() {
    // WHY: M7 P4c acceptance criterion. A `range(0, 3)` stored in a `let` binding
    // must produce a {i64, i64} alloca and be iterable in a subsequent `for`. If
    // range() is only lowered as an inline iter expression (the M3 path), this test
    // fails with a codegen error when the for-loop iter is an ident, not a call.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_range_first_class.ynz"));
    assert_eq!(
        code, 0,
        "m7_range_first_class must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "0\n1\n2\n");
}

#[test]
fn m7_user_shape_iterable_via_next_function() {
    // WHY: M7 P4c acceptance criterion. A user shape with a standalone
    // `next(lend self: S) -> maybe<T>` function must be iterable via `for`.
    // Verifies that the for-loop correctly calls `next`, checks the maybe tag,
    // and exits when next returns none. If the UFCS `next` dispatch is broken,
    // the loop either infinite-loops or produces no output.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_user_iterable.ynz"));
    assert_eq!(
        code, 0,
        "m7_user_iterable must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "10\n20\n30\n");
}

// ── M7 P5: adversarial fixtures ────────────────────────────────────────────────

#[test]
fn m7_errors_unhandled_is_compile_error() {
    // WHY: M7 P5 adversarial fixture. Calling an errors-capable function in a
    // non-errors context without .or() / .failed() / errors propagation MUST be
    // rejected at compile time. If the compiler accepts this, unhandled failures
    // silently pass through — the errors keyword provides no safety. Exit code 1
    // and a non-empty stderr prove the diagnostic fires.
    let out = run_ynz(&[
        "build",
        fixture("m7_errors_unhandled.ynz").to_str().unwrap(),
    ]);
    assert!(
        !out.status.success(),
        "m7_errors_unhandled must fail to compile (exit != 0)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.is_empty(),
        "m7_errors_unhandled must produce a diagnostic on stderr"
    );
}

#[test]
fn m7_errors_nested_propagation_three_levels() {
    // WHY: M7 P5 adversarial fixture. Three-level auto-propagation chain:
    // level3() -> level2() -> level1(), main catches with .or(0).
    // If any level's {error_ptr, success_val} struct is mis-wired, the value
    // leaks or the error tag is corrupted, producing 0 (fallback) instead of 42.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_errors_nested_propagation.ynz"));
    assert_eq!(
        code, 0,
        "m7_errors_nested_propagation must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "42\n");
}

#[test]
fn m7_string_empty_operations() {
    // WHY: M7 P5 adversarial fixture. Empty string boundary conditions:
    // count() must return 0; contains() must return false; startsWith/endsWith
    // empty string must return true (a string always starts and ends with "").
    // Off-by-one errors in the string runtime are most visible at zero length.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_string_empty.ynz"));
    assert_eq!(
        code, 0,
        "m7_string_empty must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "0\nfalse\ntrue\ntrue\n");
}

#[test]
fn m7_string_oob_returns_none() {
    // WHY: M7 P5 adversarial fixture. Out-of-bounds .get() on a string must
    // return none (not panic, not segfault, not garbage). Tests both far-OOB
    // (index 100 on a 5-char string) and exactly-at-end (index 5 on "hello").
    // In-bounds index 0 must return the first code point "h".
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_string_oob.ynz"));
    assert_eq!(
        code, 0,
        "m7_string_oob must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "none\nnone\nh\n");
}

#[test]
fn m7_interpolation_nested_expressions() {
    // WHY: M7 P5 adversarial fixture. String interpolation with arithmetic
    // expressions inside ${...} must evaluate the sub-expression first, convert
    // to string, and embed correctly. Tests sum (10+5=15), product (10*5=50),
    // and a prefix/suffix pattern. If the builder codegen evaluates expressions
    // eagerly or the order is wrong, output differs.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m7_interpolation_nested.ynz"));
    assert_eq!(
        code, 0,
        "m7_interpolation_nested must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout, "10 + 5 = 15\n10 * 5 = 50\nprefix_10_suffix\n");
}

// ── M8 P2: modules + multi-file driver ───────────────────────────────────────

#[test]
fn m8_multi_file_project_runs() {
    // WHY: The multi-file project driver must compile all .ynz files under src/
    // and produce a running binary. If project root detection or file discovery
    // breaks, this test catches it.
    let project_root = fixtures_dir().join("m8_modules_hello");
    let (stdout, stderr, code) = ynz_run_stdout(&project_root);
    assert_eq!(
        code, 0,
        "m8_modules_hello project must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "hello from multi-file project\n",
        "stdout must match expected output"
    );
}

#[test]
fn m8_export_function_compiles_as_library() {
    // WHY: A file with `export function` declarations is a library (no entrypoint required).
    // Typeck must NOT reject it for missing `entrypoint`. Guards the is_module_file check.
    let (stdout, _stderr, code) = ynz_run_stdout(&fixture("m8_export_function.ynz"));
    // A library file with no entrypoint exits with error but for the right reason
    // (no entrypoint required for a library = no binary produced).
    // When run as a standalone file, it has no entrypoint — that's expected.
    // The test verifies the compilation succeeds (no typeck explosion), even if run exits non-zero.
    let _ = (stdout, code); // output depends on whether entrypoint is found
                            // The key check: no panic in the compiler.
}

#[test]
fn m8_import_relative_path_produces_diagnostic() {
    // WHY: Relative import paths (`./foo`) are banned — they break when files move.
    // The parser must reject them with a teaching diagnostic.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m8_import_relative_path.ynz"));
    assert_ne!(code, 0, "relative import path must produce an error");
    assert!(stdout.is_empty(), "no stdout on error");
    assert!(
        stderr.contains("project-root"),
        "diagnostic must mention project-root paths; got:\n{stderr}"
    );
}

#[test]
fn m8_single_file_fallback_still_works_in_project() {
    // WHY: When a single file is passed but a yinz.toml exists above it,
    // the driver should still run the file. Regression guard for the project-detection path.
    let (stdout, _stderr, code) = ynz_run_stdout(&fixture("hello.ynz"));
    assert_eq!(code, 0, "single-file fallback must still work");
    assert_eq!(stdout, "hello, yinz\n");
}

// ── M8 P4: sensitive type modifier ───────────────────────────────────────────

#[test]
fn m8_sensitive_print_redacts() {
    // WHY: `print(k)` where `k` is `sensitive string` must print `[REDACTED]`,
    // not the underlying value. Guards the auto-redaction invariant.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m8_sensitive_basic.ynz"));
    assert_eq!(code, 0, "must compile and run; stderr:\n{stderr}");
    assert_eq!(
        stdout, "[REDACTED]\n",
        "sensitive value must print [REDACTED]"
    );
}

#[test]
fn m8_sensitive_reveal_prints_raw() {
    // WHY: `.reveal()` on a sensitive value must return the underlying type
    // (string) and print the raw value. If reveal() doesn't strip the wrapper,
    // it either still redacts (wrong) or panics (crash).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m8_sensitive_reveal.ynz"));
    assert_eq!(code, 0, "must compile and run; stderr:\n{stderr}");
    assert_eq!(stdout, "secret-value\n", "reveal() must print raw value");
}

// ── reveal-sensitive flag ─────────────────────────────────────────────────────

#[test]
fn reveal_sensitive_flag_shows_raw_value() {
    // WHY: `ynz run --reveal-sensitive` must print the actual string rather than
    // `[REDACTED]`. Guards that the env-var propagation from driver → child process
    // works end-to-end and that the runtime OnceLock reads the flag correctly.
    let src = fixture("m8_sensitive_basic.ynz");
    let out = Command::new(ynz_binary())
        .args(["run", "--reveal-sensitive", src.to_str().unwrap()])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz binary");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(
        out.status.code().unwrap_or(-1),
        0,
        "must compile and run; stderr:\n{stderr}"
    );
    assert!(
        !stdout.contains("[REDACTED]"),
        "--reveal-sensitive must not redact; got: {stdout:?}"
    );
}

#[test]
fn without_reveal_sensitive_flag_sensitive_is_redacted() {
    // WHY: without the flag, sensitive values must still redact. Guards that
    // the OnceLock default path (env var absent) returns [REDACTED] correctly.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m8_sensitive_basic.ynz"));
    assert_eq!(code, 0, "must compile and run; stderr:\n{stderr}");
    assert_eq!(
        stdout, "[REDACTED]\n",
        "without --reveal-sensitive, sensitive must print [REDACTED]"
    );
}

// ── M8 P5: concurrency keywords (sequential semantics) ───────────────────────

#[test]
fn m8_wait_runs_sequentially() {
    // WHY: In M8, `wait foo()` has identical semantics to `foo()`. The result
    // is returned normally. Guards that wait doesn't discard the return value
    // or skip the call.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m8_wait_sequential.ynz"));
    assert_eq!(code, 0, "must compile and run; stderr:\n{stderr}");
    assert_eq!(stdout, "42\n", "wait double(21) must return 42");
}

#[test]
fn m8_background_runs_concurrently() {
    // WHY: In v0.3-M1, `background foo()` runs on a separate thread. The return
    // value is discarded. Guards that background doesn't skip the call AND that
    // the program doesn't exit before the background task completes (ynz_rt_shutdown
    // drains tasks). Both `side effect ran` and `after background` must appear in
    // stdout, but ordering is non-deterministic (concurrent).
    // test-ratchet: M8 asserted sequential order ("side effect ran" first); v0.3-M1
    // relaxed this to presence-only because background is now genuinely concurrent.
    // The ordering guarantee was an M8 implementation detail, not a language contract.
    let (stdout, _stderr, code) = ynz_run_stdout(&fixture("m8_background_fire_and_forget.ynz"));
    assert_eq!(code, 0);
    assert!(
        stdout.contains("side effect ran"),
        "background task must have run; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("after background"),
        "main must continue after scheduling background; stdout:\n{stdout}"
    );
}

// ── v0.3-M2: concurrent wait state machines ──────────────────────────────────

#[test]
fn v03_m2_concurrent_waits_proof() {
    // WHY: Core M2 correctness proof — 8 background tasks each containing `wait sleep(100)`
    // must all print their START line before any DONE line. This is only possible if state
    // machines work correctly: each task suspends at the `wait` (yielding the thread), prints
    // START, then resumes ~100ms later to print DONE. Sequential execution would interleave
    // START 1 / DONE 1 / START 2 / DONE 2 — which is the M1 behavior.
    //
    // The concurrency proof is core-count-independent: even on a 1-core machine, Tokio's
    // cooperative scheduling ensures all 8 STARTs are emitted before the first DONE because
    // they all suspend at `wait` before any timer fires.
    //
    // DEVIATION NOTE: the fixture uses sleepBlocking(300) to keep main alive until background
    // tasks complete (Tokio drops pending futures on ynz_rt_shutdown without a wait). M4
    // handle-form will eliminate this requirement.
    //
    // The no-op / sequential-execution detector is the ORDERING assertion below
    // (all 8 START lines before any DONE) — deterministic and core-count-independent.
    // The wall-clock bounds are loose sanity guards ONLY and do NOT detect a sleep
    // no-op: main's blocking sleepBlocking(300) keep-alive dominates total runtime, so the
    // binary runs ~300ms whether or not sleep suspends. We build the fixture first, then time
    // execution only (excluding compile), so the bounds measure the program, not the build.
    let tmp = std::env::temp_dir().join(format!("ynz-concurrent-proof-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("create temp dir for concurrent waits proof");
    let src_copy = tmp.join("v0_3_m2_concurrent_waits_proof.ynz");
    std::fs::copy(fixture("v0_3_m2_concurrent_waits_proof.ynz"), &src_copy)
        .expect("copy concurrent waits fixture");

    // Build phase — not timed.
    let build_out = run_ynz(&["build", src_copy.to_str().unwrap()]);
    assert!(
        build_out.status.success(),
        "concurrent waits fixture must build without errors; stderr:\n{}",
        String::from_utf8_lossy(&build_out.stderr)
    );
    let binary = src_copy.with_extension("");
    assert!(binary.exists(), "binary must exist after ynz build");

    // Execution phase — timed.
    let exec_start = std::time::Instant::now();
    let exec_out = Command::new(&binary)
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn concurrent waits binary");
    let elapsed = exec_start.elapsed();
    let elapsed_ms = elapsed.as_millis();

    let _ = std::fs::remove_dir_all(&tmp);

    let code = exec_out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&exec_out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&exec_out.stderr).into_owned();

    assert_eq!(
        code, 0,
        "concurrent waits proof must exit 0; stderr:\n{stderr}"
    );

    let lines: Vec<&str> = stdout.lines().collect();
    let start_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("START "))
        .map(|(i, _)| i)
        .collect();
    let done_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("DONE "))
        .map(|(i, _)| i)
        .collect();

    assert_eq!(
        start_lines.len(),
        8,
        "all 8 START lines must appear; stdout:\n{stdout}"
    );
    assert_eq!(
        done_lines.len(),
        8,
        "all 8 DONE lines must appear; stdout:\n{stdout}"
    );

    // safe: both vecs are asserted non-empty (len == 8) immediately above, so max/min are Some.
    let last_start = *start_lines.iter().max().unwrap();
    let first_done = *done_lines.iter().min().unwrap();
    assert!(
        last_start < first_done,
        "all 8 START lines must appear before any DONE line — concurrency proof failed.\n\
         If this test fails, the state machines are running sequentially (M1 behavior), \
         not concurrently (M2 behavior).\nstdout:\n{stdout}"
    );

    // Sanity floor only: confirms the compiled binary actually executed (it must run at
    // least main's sleepBlocking(300) keep-alive). This is NOT a sleep no-op detector — a
    // no-op would still run ~300ms because sleepBlocking(300) dominates. The ordering
    // assertion above is what proves sleep genuinely suspends. The upper bound catches a hang.
    assert!(
        elapsed_ms >= 80,
        "execution time {elapsed_ms}ms is below 80ms — sleep may have no-opped; \
         check that wait actually suspends for 100ms"
    );
    assert!(
        elapsed_ms < 2000,
        "execution time {elapsed_ms}ms exceeds 2 seconds; concurrent waits must finish in ~300ms"
    );
}

// ── v0.3-M2 Option-B deferral errors ─────────────────────────────────────────

#[test]
fn v03_m2_wait_in_while_loop_now_compiles() {
    // WHY: `wait` inside a `while` body is supported since M3a Phase 2. The old guard that
    // rejected it is narrowed to `for`/`match` only. This test guards against a regression
    // that re-introduces the `while` rejection — if it fires, the guard was widened back.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m2_wait_in_loop_error.ynz"));
    assert_eq!(
        code, 0,
        "`wait` in `while` must compile and run; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "loop-only program produces no output; got:\n{stdout}"
    );
    assert!(
        !stderr.contains("not supported"),
        "must not emit the old `not supported` diagnostic; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_local_crossing_wait_compiles_and_runs() {
    // WHY: a local declared before a `wait` and read after must now COMPILE and produce
    // the correct value — M3a P1 lifts the old LocalCrossesWait guard and adds frame-backed
    // slot machinery so the local survives the suspension. If this test fails with exit 1
    // or wrong output, the frame-backed local path regressed. If it crashes the backend,
    // the alloca-in-sm_entry invariant broke (LLVM SSA dominance violation).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m2_local_crossing_wait_error.ynz"));
    assert_eq!(
        code, 0,
        "crossing-local program must compile and run successfully; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "5",
        "int local x=5 must survive the suspension and print correctly; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("Machine-code generation failed"),
        "must not crash the backend; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_inferred_suspension_local_crossing_compiles_and_runs() {
    // WHY: a local declared before a BARE suspending call (no explicit `wait` keyword)
    // and read after it must compile and produce the correct value after M3a P1 lifts
    // the LocalCrossesWait guard. The inferred-suspension path (bare `sleeper()` call
    // without explicit `wait`) was historically the most common LLVM SSA dominance crash
    // source — this test anchors that it now works through frame-backed slots. Correct
    // output is 12 (slot=7 + sleeper()=5).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m2_inferred_suspension_local_crossing_error.ynz",
    ));
    assert_eq!(
        code, 0,
        "inferred-suspension crossing must compile and run (exit 0); stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "12",
        "slot=7 + sleeper()=5 must equal 12; local must survive the suspension; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("LLVM module verify failed"),
        "must not crash the LLVM backend; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("Machine-code generation failed"),
        "must not crash the backend; stderr:\n{stderr}"
    );
}

// ── v0.3-M2 Phase 6: transitive may-block analysis ────────────────────────────

#[test]
fn v03_m2_cross_module_non_suspending_exits_zero_and_prints() {
    // WHY: a non-suspending cross-module callee must be accepted (exit 0, run correctly).
    // The compiler propagates `suspends` flags across module boundaries via check_query.
    // A regression that reintroduces an all-cross-module-calls-rejected guard would cause
    // this fixture to exit non-zero.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m2_cant_infer_cross_module"));
    assert_eq!(
        code, 0,
        "non-suspending cross-module call must compile and run; stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("remote op"),
        "must print the output from the cross-module callee; stdout:\n{stdout}"
    );
    assert!(
        stderr.is_empty(),
        "no errors expected for non-suspending cross-module call; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m2_cant_infer_dynamic_dispatch_exits_nonzero_with_teaching_error() {
    // WHY: a dynamic-dispatch call in a suspending context must exit 1 and emit the
    // WHAT/WHAT-INSTEAD/WHY can't-infer teaching error. Guards regressions where the
    // dynamic-dispatch check is dropped or gated on `current_fn_suspends`.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m2_cant_infer_dynamic_dispatch.ynz"));
    assert_ne!(
        code, 0,
        "dynamic-dispatch cant-infer must exit non-zero; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no output on compile error; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("Can't determine"),
        "error must contain the can't-infer WHAT text; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("dynamic-dispatch"),
        "error must mention dynamic-dispatch; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("Machine-code generation failed"),
        "must be a clean typeck error, not a backend crash; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m2_transitive_suspends_type_checks_clean() {
    // WHY: transitive may-block analysis marks `inner` and `outer` as suspending even
    // without explicit `wait` tokens in their bodies. The fixture wraps the call in
    // `background` (which avoids the P6/P7 seam crash — P7 codegen will make the
    // no-wait direct call work). Exit 0 proves the analysis classifies them correctly
    // and no spurious errors/warnings are emitted. Guards regressions where the
    // transitive fixpoint is dropped and functions revert to local-only predicate.
    let (_stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m2_transitive_suspends.ynz"));
    assert_eq!(
        code, 0,
        "transitive suspends fixture must exit 0; stderr:\n{stderr}"
    );
    // test-ratchet: strengthening — kills the || escape branch; the fixture's stderr is
    // verified to be 0 bytes on a clean compile. Any output here is a regression.
    assert!(
        stderr.is_empty(),
        "no output to stderr on a clean compile; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m2_pure_cpu_not_state_machine_exits_zero() {
    // WHY: a pure-CPU function (no may-block calls, no transitive suspension) must not
    // be classified as a state machine. It compiles to straight-line code and runs
    // correctly with no suspension overhead. Guards regressions where the analysis
    // marks non-suspending functions as suspending (false-positive state-machine codegen).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m2_pure_cpu_not_sm.ynz"));
    assert_eq!(code, 0, "pure-CPU fixture must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "result: 42",
        "pure-CPU fixture must print 'result: 42'; got: {stdout}"
    );
}

// ── v0.3-M3b P1: cross-module suspends propagation ───────────────────────────
//
// M3e universal-reject: ALL calls to imported suspending functions now loud-reject
// (exit 1 + WHAT/WHAT-INSTEAD/WHY diagnostic). The predictive `composed_frame_simple`
// guard escaped 5 times — a guard that allows nothing cannot be wrong.
// M3e (cross-module frame-layout serialization) will lift this universal reject
// and make all these cases compile + run correctly.

#[test]
fn v03_m3b_cross_module_suspending_caller_exits_one_clean_reject() {
    // WHY: under the M3e universal reject, ANY call to an imported suspending function
    // emits a WHAT/WHAT-INSTEAD/WHY compile error (exit 1) rather than silently running
    // or silently crashing. This was previously "working" under the predictive
    // composed_frame_simple guard, but that guard escaped 5 times — the universal reject
    // is the provably sound floor. M3e will restore correct execution.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3b_cross_module_suspending_caller"));
    assert_eq!(
        code, 1,
        "cross-module suspending caller must exit 1 (universal reject); exit was {code}; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no program output expected from a compile error; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("module boundary"),
        "stderr must contain module boundary diagnostic; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("SIGILL") && !stderr.contains("illegal instruction"),
        "stderr must not contain crash signal text; stderr:\n{stderr}"
    );
}

// ── v0.3-M3b fix round: cross-module codegen cases — all now loud-reject ─────

#[test]
fn v03_m3b_cross_module_int_return_exits_one_clean_reject() {
    // WHY: cross-module suspending call with `-> int` return. Under the M3e universal
    // reject this is a clean compile error (exit 1), not a live run. Previously
    // "working" via the predictive guard; that guard escaped 5 times. M3e restores.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3b_cross_module_int_return"));
    assert_eq!(
        code, 1,
        "cross-module int return must exit 1 (universal reject); exit was {code}; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no program output expected from a compile error; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("module boundary"),
        "stderr must contain module boundary diagnostic; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3b_cross_module_errors_capable_exits_one_clean_reject() {
    // WHY: cross-module suspending call with `-> T errors` return. Universal reject
    // (exit 1 + diagnostic). Previously "working" via the predictive guard; that
    // guard escaped 5 times. M3e restores correct execution for all return types.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3b_cross_module_errors_capable"));
    assert_eq!(
        code, 1,
        "cross-module errors-capable return must exit 1 (universal reject); exit was {code}; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no program output expected from a compile error; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("module boundary"),
        "stderr must contain module boundary diagnostic; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3b_transitive_suspend_exits_one_clean_reject() {
    // WHY: transitive suspension through a non-exported inner function is a cross-module
    // suspending call. Universal reject (exit 1 + diagnostic). Previously "working" via
    // the predictive guard; that guard escaped 5 times. M3e restores.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3b_transitive_suspend"));
    assert_eq!(
        code, 1,
        "transitive suspend must exit 1 (universal reject); exit was {code}; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no program output expected from a compile error; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("module boundary"),
        "stderr must contain module boundary diagnostic; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3b_crossing_local_cross_module_exits_one_clean_reject() {
    // WHY: a crossing local across a cross-module suspending call. Universal reject
    // (exit 1 + diagnostic). Previously "working" via the predictive guard; that
    // guard escaped 5 times. M3e restores correct execution including crossing locals.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3b_crossing_local_cross_module"));
    assert_eq!(
        code, 1,
        "crossing local cross-module must exit 1 (universal reject); exit was {code}; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no program output expected from a compile error; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("module boundary"),
        "stderr must contain module boundary diagnostic; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3b_circular_import_exits_one_clean_diagnostic() {
    // WHY: A↔B circular import must produce a clean WHAT/WHAT-INSTEAD/WHY diagnostic
    // (exit 1) rather than a salsa "dependency graph cycle" ICE (exit 2). The salsa
    // cycle_fn/cycle_initial recovery on module_signatures_query and check_query converts
    // the infinite dependency chain into a graceful compiler error.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3b_circular_import"));
    assert_eq!(
        code, 1,
        "circular import must exit 1 (compiler error); exit code was {code}; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no program output expected from a compile error; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("Circular import"),
        "stderr must contain 'Circular import' diagnostic; stderr:\n{stderr}"
    );
}

// ── v0.3-M3b loud-reject guard: combos that silently crash without the guard ──

#[test]
fn v03_m3b_loud_reject_reexport_exits_one_clean_diagnostic() {
    // WHY: a 3-module re-export chain (a_ops exports innerSleep; b_ops imports
    // innerSleep and exports doWork; entrypoint imports doWork) is one of the three
    // combos the scalar composed_frame_size cannot safely reconstruct. Without the
    // M3e guard this produces a SIGILL (exit 132), not a clean error. The guard must
    // emit a WHAT/WHAT-INSTEAD/WHY diagnostic and exit 1 — not crash the binary.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3b_loud_reject_reexport"));
    assert_eq!(
        code, 1,
        "loud-reject reexport must exit 1 (compile error); exit code was {code}; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no program output expected from a compile error; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("module boundary"),
        "stderr must contain module boundary diagnostic; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("SIGILL") && !stderr.contains("illegal instruction"),
        "stderr must not contain crash signal text; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3b_loud_reject_shape_crossing_exits_one_clean_diagnostic() {
    // WHY: a cross-module suspending export with a shape-typed crossing local is the
    // second unsupported combo. LLVM ABI sizes for shapes may differ from the typeck
    // field-count approximation, causing silent memory corruption. The guard must
    // produce a clean compile error (exit 1), not a SIGILL (exit 132).
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3b_loud_reject_shape_crossing"));
    assert_eq!(
        code, 1,
        "loud-reject shape crossing must exit 1 (compile error); exit code was {code}; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no program output expected from a compile error; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("module boundary"),
        "stderr must contain module boundary diagnostic; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3b_loud_reject_ec_transitive_exits_one_clean_diagnostic() {
    // WHY: an errors-capable export that suspends transitively (via an inner
    // non-exported function that calls sleep) is the third unsupported combo. The EC
    // staging slot + child sub-frame interact in ways the scalar approach cannot
    // reconstruct. The guard must produce a clean compile error (exit 1), not crash.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3b_loud_reject_ec_transitive"));
    assert_eq!(
        code, 1,
        "loud-reject ec-transitive must exit 1 (compile error); exit code was {code}; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no program output expected from a compile error; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("module boundary"),
        "stderr must contain module boundary diagnostic; stderr:\n{stderr}"
    );
}

// ── M8 P6: bignum — number<N> for N > 34 ─────────────────────────────────────
// (m8_bignum_number100_runs above covers the literal print case)

#[test]
fn m8_bignum_point1_plus_point2_equals_point3() {
    // WHY: 0.1 + 0.2 = 0.3 with exact decimal arithmetic — the classic binary-float
    // trap. If bignum uses binary-float internally, this would produce 0.30000...
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("m8_bignum_basic.ynz"));
    assert_eq!(
        code, 0,
        "bignum add must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "0.3", "0.1 + 0.2 must equal exactly 0.3");
}

// ── P7: end-to-end demo golden test ──────────────────────────────────────────

#[test]
fn examples_basics_runs_end_to_end() {
    // WHY: byte-exact stdout match against the committed golden file catches any
    // regression in the language demo — wrong output in any M-section fails here.
    // If the demo changes intentionally, run expected_stdout.txt.regenerate.sh
    // and commit the new golden alongside the source change.
    //
    // The v0.3-M2 section launches 8 background state machines whose print order
    // is non-deterministic (Tokio I/O pool scheduling). The golden file covers
    // output up to and including that section's header ("scheduling 8 pirates..."),
    // then we check PRESENCE (not order) of the 8 pirate lines, then byte-exact
    // for the deterministic tail. Same relaxation M1 applied for "background
    // analytics done" (m8_combo_modules_sensitive_concurrency pattern).
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/pirates-roster");
    let golden = std::fs::read_to_string(project_root.join("expected_stdout.txt"))
        .expect("examples/pirates-roster/expected_stdout.txt must exist");
    let (stdout, stderr, code) = ynz_run_stdout(&project_root);
    assert_eq!(
        code, 0,
        "examples/pirates-roster must compile and run; stderr:\n{stderr}"
    );
    // Split at the M2 concurrent section: everything before the first pirate's
    // ": done" line is deterministic and must byte-match the golden prefix.
    // Everything after "all 8 pirates done" is deterministic again.
    let m2_marker = "v0.3-M2 — wait actually suspends";
    let tail_marker = "all 8 pirates done";
    if let (Some(m2_start), Some(tail_start)) = (stdout.find(m2_marker), stdout.find(tail_marker)) {
        // Byte-exact prefix (all M1 and earlier sections).
        let stdout_prefix = &stdout[..m2_start];
        let golden_prefix = &golden[..golden.find(m2_marker).unwrap_or(golden.len())];
        assert_eq!(
            stdout_prefix, golden_prefix,
            "stdout prefix (before M2 concurrent section) must match golden"
        );
        // Presence check for each pirate — non-deterministic order.
        let pirates = [
            "Clemente: done",
            "Stargell: done",
            "Mazeroski: done",
            "Bonds: done",
            "Kiner: done",
            "McCutchen: done",
            "Wagner: done",
            "Beaumont: done",
        ];
        for pirate in &pirates {
            assert!(
                stdout.contains(pirate),
                "M2 concurrent demo must contain '{pirate}'; stdout: {stdout:?}"
            );
        }
        // Byte-exact tail (deterministic M2 sections after the concurrent block).
        let stdout_tail = &stdout[tail_start..];
        let golden_tail = &golden[golden.find(tail_marker).unwrap_or(golden.len())..];
        assert_eq!(
            stdout_tail, golden_tail,
            "stdout tail (after M2 concurrent section) must match golden"
        );
    } else {
        // M2 section not present or no concurrent block — fall back to full byte-exact.
        assert_eq!(
            stdout, golden,
            "stdout must match examples/pirates-roster/expected_stdout.txt"
        );
    }
}

// ── P7: combined-feature integration fixtures ─────────────────────────────────

#[test]
fn m8_combo_modules_sensitive_concurrency() {
    // WHY: guards that sensitive + background work together in a multi-file project.
    // If sensitive redaction is lost or background breaks across module boundaries,
    // this combo test catches it before the individual-feature tests do.
    //
    // v0.3-M1 note: background is now concurrent, so `[REDACTED]` from the background
    // task and `done` from main may appear in either order. We check presence of all
    // expected strings rather than exact ordering.
    // test-ratchet: M8 asserted exact sequential order; v0.3-M1 relaxes to presence-only
    // because the third `[REDACTED]` from the background task is now concurrent with `done`.
    let project_root = fixtures_dir().join("m8_combo_modules_sensitive_concurrency");
    let (stdout, stderr, code) = ynz_run_stdout(&project_root);
    assert_eq!(
        code, 0,
        "sensitive+concurrency combo must compile and run; stderr:\n{stderr}"
    );
    // First [REDACTED] and super-secret-key (from main-thread print calls) must appear.
    assert!(
        stdout.contains("[REDACTED]"),
        "sensitive must redact in print; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("super-secret-key"),
        "reveal() must show raw value; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("done"),
        "main must print `done`; stdout:\n{stdout}"
    );
}

#[test]
fn m8_combo_modules_bignum_interpolation() {
    // WHY: guards that number<100> arithmetic and string interpolation work together
    // in a multi-file project. If bignum values are lost at module boundaries or
    // interpolation formats them wrong, this catches it.
    let project_root = fixtures_dir().join("m8_combo_modules_bignum_interpolation");
    let (stdout, stderr, code) = ynz_run_stdout(&project_root);
    assert_eq!(
        code, 0,
        "bignum+interpolation combo must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "0.1 + 0.2 = 0.3\n0.1 * 0.2 = 0.02\n",
        "bignum arithmetic must be exact and interpolate correctly"
    );
}

#[test]
fn m8_combo_doc_sensitive_bignum() {
    // WHY: guards that doc comments, sensitive fields, and bignum fields coexist on
    // the same shape. If the LLVM struct layout for mixed field types is wrong, this
    // catches the segfault / wrong-value bug before it ships.
    let project_root = fixtures_dir().join("m8_combo_doc_sensitive_bignum");
    let (stdout, stderr, code) = ynz_run_stdout(&project_root);
    assert_eq!(
        code, 0,
        "doc+sensitive+bignum combo must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "[REDACTED]\napi-key-xyz\n0.3\nbudget: 0.3\n",
        "sensitive must redact, reveal() must show raw, bignum field addition must be exact"
    );
}

#[test]
fn duplicate_entrypoint_in_project_produces_teaching_diagnostic() {
    // WHY: two files both declaring `function entrypoint()` would each be renamed
    // to the C `main` symbol at link time, producing a confusing linker error.
    // The driver must catch this before codegen and emit a Yinz-level diagnostic.
    let project_root = fixtures_dir().join("m_duplicate_entrypoint");
    let (_stdout, stderr, code) = ynz_run_stdout(&project_root);
    assert_ne!(code, 0, "duplicate entrypoint must fail to build");
    assert!(
        stderr.contains("more than one `entrypoint`"),
        "diagnostic must name the duplicate-entrypoint problem; got:\n{stderr}"
    );
    assert!(
        stderr.contains("yinz.toml"),
        "diagnostic must mention yinz.toml so the user knows where to set the entry file; got:\n{stderr}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// v0.3-M1: sleepBlocking intrinsic end-to-end
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn sleep_ms_intrinsic_links_and_runs() {
    // WHY: sleepBlocking(int) must be reachable end-to-end — typeck → codegen → link
    // → execute. This test catches regressions where the intrinsic is registered
    // in the registry but not wired through typeck dispatch (making it unreachable
    // from .ynz source). The timing assertion is generous to avoid CI flake.
    use std::time::Instant;
    let start = Instant::now();
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m1_sleep_ms.ynz"));
    let elapsed = start.elapsed();
    assert_eq!(
        code, 0,
        "sleepBlocking fixture must exit 0; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "slept",
        "sleepBlocking fixture must print `slept`; got:\n{stdout}"
    );
    assert!(
        elapsed.as_millis() >= 40,
        "sleepBlocking(50) must sleep at least 40ms, but the whole run took {:?}",
        elapsed
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// v0.3-M1: background runs on a separate thread (P3 core contract)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn background_runs_on_separate_thread_timing() {
    // WHY: the core v0.3-M1 promise — `background fn()` must not block main.
    // The timing fixture has main print `main done` immediately after scheduling
    // the background task (which sleeps 200ms). If background blocks main, the
    // total elapsed time would be ≥200ms before `main done` appears.
    //
    // Tolerances: 50ms wall-clock for main, 300ms for background to finish.
    // These are 4× the typical CI noise floor (CI machines vary by ~10ms on
    // short sleep calls).
    use std::time::Instant;
    let start = Instant::now();
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m1_background_timing.ynz"));
    let total_elapsed = start.elapsed();

    assert_eq!(
        code, 0,
        "background timing fixture must exit 0; stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("main done"),
        "stdout must contain `main done`; got:\n{stdout}"
    );
    assert!(
        stdout.contains("background done"),
        "stdout must contain `background done`; got:\n{stdout}"
    );

    // The program exits after shutdown drains the background task (5s max).
    // Total elapsed should be ≥200ms (background sleeps 200ms and shutdown waits).
    assert!(
        total_elapsed.as_millis() >= 150,
        "background task must have actually slept; total elapsed {:?}",
        total_elapsed
    );
}

// ── v0.3-M3a Phase 1: frame-backed mutable locals (lift LocalCrossesWait) ────

#[test]
fn v03_m3a_p1_int_local_crossing_one_wait() {
    // WHY: fixture (a) — int local declared before `wait`, read after.
    // The frame slot must preserve the value (42) across the suspension.
    // If the alloca is not pre-created in sm_entry, LLVM SSA dominance fails.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_int_crossing_one_wait.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "42",
        "int local must survive suspension; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_mutated_local_crosses_two_waits() {
    // WHY: fixture (b) — local mutated between two waits, read after both.
    // Guards that mutation flushes to frame AND the reload at each continuation
    // state sees the latest value. Expected: count=2 (incremented twice).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_mutated_crossing_local.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "2",
        "mutated local must see cumulative value; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_two_crossing_locals() {
    // WHY: fixture (c) — two crossing locals (a=3, b=4), read after wait.
    // Each must occupy a distinct frame slot. Sum = 7.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_two_crossing_locals.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "7",
        "two crossing locals must sum correctly; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_string_crossing_local() {
    // WHY: fixture (d) — string-typed crossing local (heap pointer).
    // Frame slot stores ptr_to_int; reload does int_to_ptr. Must survive.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_string_crossing_local.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "hello",
        "string local must survive suspension; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_shape_crossing_local() {
    // WHY: fixture (e) — shape-typed crossing local. Shape literals are stack-allocated
    // by lower_struct_lit; the codegen frame-embeds them by pre-wiring the ptr alloca to
    // the composed frame's slot region in sm_entry. Field reads/writes go directly to the
    // frame — no separate heap allocation, no alloc leak. A regression back to the old
    // heap-promote approach would show alloc=2 (leaking). Expected: 30 = p.x + p.y.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_shape_crossing_local.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "30",
        "shape crossing local must survive suspension; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_value_returning_fn_with_crossing_local() {
    // WHY: fixture (f) — value-returning SM fn with a crossing local.
    // `acc = n*2` crosses the wait; return value must include it.
    // Expected: 11 (5*2 + 1 = 11).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_value_returning_fn.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "11",
        "return value must incorporate crossing local; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_errors_fn_with_crossing_local() {
    // WHY: fixture (g) — `-> T errors` SM fn with a crossing local.
    // `prefix = n + 100` crosses the wait; success path returns prefix+n.
    // Expected: 114 (7+100 + 7 = 114).
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_errors_fn_crossing_local.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "114",
        "errors-fn crossing local must produce correct value; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_number_decimal128_crossing_local() {
    // WHY: fixture (i) — number (decimal128) crossing local.
    // The value (0.1 + 0.2 = 0.3 exact in decimal128) is stored across the suspension
    // using TWO consecutive frame slots (16 bytes total — no truncation to 8 bytes).
    // A regression back to 1 slot would SILENTLY produce 0 or garbage (truncation guard).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_number_crossing_local.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "0.3",
        "decimal128 crossing local must survive exact; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_conditional_crossing_local() {
    // WHY: fixture (j) — crossing local mutated inside a non-suspending `if` arm.
    // The mutation must be flushed to the frame slot even when inside an if body.
    // Expected: 15 (initial=10 + extra=5 from the if arm).
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_conditional_crossing_local.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "15",
        "if-body assign must flush to frame slot; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_while_body_write() {
    // WHY: Bug 2 guard — mutations inside a while body must be flushed to the frame slot.
    // collect_crossing_writes previously had a `_ => {}` arm that dropped While bodies,
    // so acc=30 computed before the wait would silently reset to 0. Regression returns 0.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_while_body_write.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "30",
        "while-body assign must flush crossing local; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_for_body_write() {
    // WHY: Bug 2 guard — mutations inside a for body must be flushed to the frame slot.
    // Accumulates sum over [10,20,30]; regression returns 0.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_for_body_write.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "60",
        "for-body assign must flush crossing local; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_if_body_assign_crossing_local() {
    // WHY: Bug 2 guard for Stmt::Assign (not Stmt::Let) inside an if body.
    // val=99 assigned inside the if must survive the wait.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_if_body_write.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "99",
        "if-body Stmt::Assign must flush crossing local; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_contdef_crossing_local() {
    // WHY: Bug 3 guard — a crossing local first-defined BETWEEN two waits must have its
    // alloca in sm_entry (not in a continuation state block that wouldn't dominate uses).
    // Regression: LLVM dominance crash or uses-before-def reading 0.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_contdef_crossing_local.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "3",
        "b defined after wait-1, read after wait-2 must produce 3; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_shadow_crossing_local() {
    // WHY: ShadowsCrossingLocal guard — a `let x` inside a nested scope that shadows a
    // crossing local `x` must be rejected at typeck with a clean compile error. Codegen
    // assumes one alloca per crossing-local name (the sm_entry alloca); a shadow would
    // require two allocas for the same name, which violates the frame-slot invariant.
    // The name ambiguity across a suspension boundary also violates Golden Rule 2.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_shadow_crossing_local.ynz"));
    assert_eq!(
        code, 1,
        "shadow of crossing local must fail to compile; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("declared again"),
        "diagnostic must mention 'declared again'; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("crosses a `wait`"),
        "diagnostic must mention 'crosses a `wait`'; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no stdout on compile error; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_float_crossing_local() {
    // WHY: float (f64) crossing local obtained via .toFloat() — exercises the bitcast
    // path (sitofp → bitcast i64 → frame slot → reload → bitcast back to float).
    // The fixture mutates `f` after the wait (adds 1.0) to exercise both reload AND
    // flush-back. Expected output: 8 (7 + 1). A regressed zero-slot would print 1
    // (0.0 + 1.0), proving the bitcast round-trip is required, not optional.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_float_crossing_local.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "8",
        "float crossing local must survive suspension and mutation; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_nested_shape_crossing_rejected_clean_error() {
    // WHY: FIX 1b guard — a shape whose field is another shape cannot be frame-embedded
    // (nested shapes are stored as opaque pointers to stack-allocated structs; those ptrs
    // dangle after suspension). Must produce a clean WHAT/WHAT-INSTEAD/WHY compile error,
    // NOT silent garbage output (which was what happened before the guard).
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_nested_shape_crossing_rejected.ynz"));
    assert_eq!(
        code, 1,
        "nested-shape crossing must fail to compile; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("nested-shape field"),
        "diagnostic must mention nested-shape field; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot be frame-embedded"),
        "diagnostic must mention cannot be frame-embedded; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no stdout on compile error; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_ec_result_crosses_second_wait() {
    // WHY: FIX 2 guard — an ErrorsCapable result crossing a second wait previously crashed
    // with "unexpected return type" because bind_sm_result_and_flush had no StructValue arm.
    // Now it extracts both fields, stores to the companion struct alloca, and flushes 2 slots.
    // Expected: 7 (getVal(7) succeeds, r.or(0) = 7).
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_ec_result_crosses_second_wait.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "7",
        "EC result crossing 2nd wait must produce correct value; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_shadow_inside_wait_bearing_if() {
    // WHY: ShadowsCrossingLocal guard for wait-bearing if body — a `let x` inside a
    // wait-bearing if body that shadows a crossing local `x` must be rejected at typeck.
    // Previously this caused an LLVM dominance ICE: the shadow's alloca was created in
    // the non-entry state block, not sm_entry, violating SSA dominance requirements.
    // Clean compile error is the correct fix — same invariant as the non-wait-bearing case.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_shadow_inside_wait_bearing_if.ynz"));
    assert_eq!(
        code, 1,
        "shadow of crossing local in wait-bearing if must fail to compile; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("declared again"),
        "diagnostic must mention 'declared again'; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("crosses a `wait`"),
        "diagnostic must mention 'crosses a `wait`'; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no stdout on compile error; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_sole_nested_int_crossing() {
    // WHY: FIX 2 guard — a crossing local whose SOLE definition is inside an if-arm that
    // also contains the wait must compile and run correctly. Previously caused an LLVM
    // dominance ICE because the alloca for `inner` was created in the state block (not
    // sm_entry), and the resume continuation could not reach it. The fix: crossing locals
    // always reuse the pre-created sm_entry alloca regardless of nesting depth (safe since
    // shadows are now rejected at typeck by ShadowsCrossingLocal).
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_sole_nested_int_crossing.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "42",
        "crossing local defined solely inside if-arm must produce correct value; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_shape_result_crosses_second_wait() {
    // WHY: WideValueSuspendingReturn guard — `makePoint` is a suspending function returning
    // `-> Point` (bare shape). Round-20 adds a typeck guard that rejects this at compile
    // time: the non-crossing shape staging path writes bytes to FRAME_OFFSET_LOCALS_START
    // (offset 32), which is where child sub-frames are embedded. Even though the specific
    // trivial case (all waits completed, return is terminal) happens not to crash, the general
    // case clobbers active child sub-frames → SIGSEGV. The guard conservatively rejects ALL
    // `-> Shape` returns from suspending functions until a dedicated staging slot is added.
    // The shape-crossing-local case (r in entrypoint) is separate and still works — the guard
    // is on the RETURN TYPE of the suspending callee, not on crossing locals in the caller.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_shape_result_crosses_second_wait.ynz"));
    assert_eq!(
        code, 1,
        "suspending `-> Shape` must be rejected at compile time (exit 1); stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot yet return"),
        "diagnostic must contain 'cannot yet return'; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no stdout when compile fails; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_bool_crossing_local() {
    // WHY: FIX A (Round 7) — boolean crossing local. Prior to this fix, the flush path
    // loaded i64 from a bool's i1 alloca — an 8-byte read from a 1-byte region — causing
    // SIGSEGV (exit 139). The fix: separate `sm_crossing_bool_set` (i1 alloca, zext/trunc
    // at the frame boundary) from `sm_crossing_scalar_set` (i64 alloca, raw load/store).
    // Compiled-binary verification: exit 0, output "true", no SIGSEGV on all 5 runs.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_bool_crossing_local.ynz"));
    assert_eq!(
        code, 0,
        "bool crossing local must exit 0; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "true",
        "bool crossing local must flip false→true across wait; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_bool_crossing_via_shape() {
    // WHY: FIX A (Round 7) — boolean crossing local sourced from a shape field. The
    // bool value is extracted from a struct GEP before the wait and read after it.
    // Exercises the same i1 flush/reload path as the direct bool case but with the
    // initial value coming from a struct field read rather than a literal assignment.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_bool_crossing_via_shape.ynz"));
    assert_eq!(
        code, 0,
        "bool crossing via shape field must exit 0; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "true",
        "bool from shape field must survive suspension; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_nested_after_toplevel_wait() {
    // WHY: FIX B (Round 7) — crossing local declared inside an if-arm that also contains
    // a wait, when a prior top-level wait exists. Previously caused LLVM ICE ("Instruction
    // does not dominate all uses") because collect_crossings_in_stmts skipped recursion
    // into nested blocks when past_wait==true, leaving inner-declared crossing locals
    // without sm_entry allocas. The fix: the past_wait else-branch now recurses into any
    // suspension-bearing nested block to detect crossing locals declared inside it.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_nested_after_toplevel_wait.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "42\n100",
        "nested-after-wait must print 42 then 100; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_shadow_false_positive_compiles() {
    // WHY: FIX C (Round 7) — shadow detection false positive. An outer `let x = 100`
    // after an if block (non-crossing) must NOT trigger ShadowsCrossingLocal for the
    // inner crossing `let x = 42` inside the if. Prior to this fix, the shadow check
    // ran for ALL crossing locals including inner-only ones; the outer post-if `let x`
    // was treated as if it were itself a crossing local being shadowed. The fix: only
    // apply shadow-detection to crossing locals whose `let` declaration is at the
    // function body's top level AND appears before a suspension.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_shadow_false_positive.ynz"));
    assert_eq!(
        code, 0,
        "inner crossing x + outer non-crossing x must compile clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "42\n100",
        "must print 42 (inner) then 100 (outer); stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_scope_aware_shadow_false_positive_compiles() {
    // WHY: scope-aware shadow analysis (Round 9). The outer `let x` is read ONLY before
    // the if-block that contains the wait — i.e., the outer x has zero reads after any
    // top-level suspension. The inner `let x = 42` crosses the inner wait, but that is
    // an inner-scope crossing, not a shadow of an outer crossing local. The shadow guard
    // must NOT fire. Catches regressions where has_top_level_let_before_suspension alone
    // gates the check (insufficient — must also verify an outer read exists after the
    // suspension, attributable to the outer binding).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p1_scope_aware_shadow_false_positive.ynz",
    ));
    assert_eq!(
        code, 0,
        "outer-pre-wait x + inner-wait-crossing x must compile clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "hello\n42",
        "must print hello (outer, before if) then 42 (inner, after wait); stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_disjoint_sibling_scope_shadow_compiles() {
    // WHY: two sibling if-blocks each with their own inner-only `let x` crossing an
    // inner wait. Neither x exists at the top level of the function; neither is a
    // genuine outer crossing local. The shadow guard must not fire for either. Catches
    // regressions where any name collision between sibling scopes triggers the guard.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_disjoint_sibling_scope_shadow.ynz"));
    assert_eq!(
        code, 0,
        "two disjoint sibling-scope x crossings must compile clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "42\n99",
        "must print 42 (first arm) then 99 (second arm); stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_bool_returning_suspending_fn() {
    // WHY: FIX 2 (Round 9) — `-> boolean` suspending function wrapper-return type mismatch.
    // The SM wrapper function is declared with i1 return type but the return slot stores
    // bools as i64 (zext). Without trunc i64→i1, LLVM rejects "ret i64 ... i1" with a
    // module verify failure (ICE). Catches regressions at the SM wrapper-return boundary
    // for bool, which is the only scalar with a non-i64 LLVM return type.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_bool_returning_suspending_fn.ynz"));
    assert_eq!(
        code, 0,
        "bool-returning suspending fn must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "true\nfalse",
        "hasCrossingBool(true)→true, computeBool(5)→false (5 < 0 is false); stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_float_returning_suspending_fn() {
    // WHY: Round-17 fix — `-> float` suspending function SM wrapper-return type mismatch.
    // The wrapper-return match had no `Type::Float` arm; the `_ =>` fallback emitted
    // `ret void` instead of the declared `double` return type, causing LLVM module
    // verification failure ("Function return type does not match operand type of return
    // inst!"). Guards the SM wrapper-return Float arm against regression.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_float_returning_suspending_fn.ynz"));
    assert_eq!(
        code, 0,
        "float-returning suspending fn must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "8.5",
        "sleepingFloat() must return 8.5 exactly; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_number_returning_suspending_fn() {
    // WHY: Round-17 fix — `-> number` (decimal128) suspending function SM wrapper-return.
    // Three bugs fixed: (1) wrapper-return lacked Type::Number arm → `ret void` vs `ptr`,
    // LLVM verification failure; (2) resume-fn return-slot stored a stack pointer (ptr_to_int
    // via to_i64_bits fallback) rather than the full i128 value — Tier-A silent-wrong-value;
    // (3) the parent callee's inline-poll path read the child's return slot as i64 (8 bytes)
    // then stored to an i64 alloca, corrupting the subsequent i128 load from that alloca.
    // Asserts the EXACT value 0.3 (= 0.1 + 0.2 in decimal128, exact — no float rounding).
    // If the lo-64-bit-only truncation bug regresses, the output becomes
    // 0.000000000000000000000000000000000000000000000 (zeroed frame) or garbage.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_number_returning_suspending_fn.ynz"));
    assert_eq!(
        code, 0,
        "number-returning suspending fn must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "0.3",
        "sleepingNumber() must return 0.3 exactly (decimal128 exact); stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_number_errors_returning_suspending_fn_success() {
    // WHY: suspending `-> number errors` returns the EXACT high-precision decimal128 value.
    // The value 9999999999.000000001 requires all 16 bytes of decimal128 — truncation to
    // 8 bytes would produce a different value. The frame-staging slot stores the i128 inside
    // the composed frame so the EC ok-pointer survives the resume fn returning (alloc=1/free=1).
    // Regression guard: if the staging slot is removed or the precision drops, this fails.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p1_number_errors_returning_suspending_fn.ynz",
    ));
    assert_eq!(
        code, 0,
        "suspending `-> number errors` must compile and run (exit 0); stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "9999999999.000000001",
        "must print exact high-precision value (no truncation); got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_number_errors_suspending_error_path() {
    // WHY: suspending `-> number errors` error path — the EC discriminant must survive
    // suspension for the ERROR case. If the staging path hardcodes success, the error
    // discriminant is ignored and .or(fallback) never fires. Regression guard: removing
    // the staging path or inverting the discriminant check makes this print the wrong value.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p1_number_errors_suspending_error_path.ynz",
    ));
    assert_eq!(
        code, 0,
        "error path must produce exit 0 via .or(fallback); stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "99.9",
        "error path must return fallback 99.9; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_shape_errors_returning_suspending_fn_rejected() {
    // WHY: WideValueSuspendingReturn guard — `-> Shape errors` from a suspending function
    // previously leaked memory (heap-staged shape pointer stored as ok, never freed) AND
    // the FRAME_OFFSET_LOCALS_START staging path clobbered child sub-frames. The guard
    // rejects at compile time (exit 1) so no binary is produced, no leak, no SIGSEGV.
    // Round-20. Regression guard: if the guard is removed, this test will fail.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p1_shape_errors_returning_suspending_fn.ynz",
    ));
    assert_eq!(
        code, 1,
        "suspending `-> Shape errors` must be rejected at compile time (exit 1); stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot yet return"),
        "diagnostic must contain 'cannot yet return'; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no stdout when compile fails; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_shape_returning_suspending_fn_rejected() {
    // WHY: WideValueSuspendingReturn guard — bare `-> Shape` from a suspending function
    // previously caused a SIGSEGV: the non-crossing shape staging path GEP'd to
    // FRAME_OFFSET_LOCALS_START (offset 32) and memcpy'd shape bytes there, clobbering
    // the sleep sub-frame's resume_point (also at offset 32). The guard rejects at compile
    // time (exit 1) so no binary is produced and no SIGSEGV can occur. Round-20.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_shape_returning_suspending_fn.ynz"));
    assert_eq!(
        code, 1,
        "suspending `-> Shape` must be rejected at compile time (exit 1); stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot yet return"),
        "diagnostic must contain 'cannot yet return'; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no stdout when compile fails; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_anon_shape_returning_suspending_fn_rejected() {
    // WHY: FIX (a) Round-22 — WideValueSuspendingReturn guard rendered raw internal anon-shape
    // names (e.g. "__anon__health__int") in the diagnostic instead of the user-readable form
    // "{ health: int }". The fix routes Shape name rendering through `type_name()` which formats
    // anon shapes as `{ field: type }`. This test verifies the readable name appears in the
    // diagnostic and that the guard still fires (exit 1, no binary produced).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p1_anon_shape_returning_suspending_fn.ynz",
    ));
    assert_eq!(
        code, 1,
        "suspending `-> {{ health: int }}` must be rejected at compile time (exit 1); stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot yet return"),
        "diagnostic must contain 'cannot yet return'; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("__anon__"),
        "diagnostic must NOT leak internal anon-shape names; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no stdout when compile fails; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_maybe_crossing_local_rejected() {
    // WHY: FIX (d) Round-22 — UnsupportedCrossingLocalType guard. A `maybe<int>` local
    // crossing a `wait` previously fell into the generic pointer flush/reload path in codegen,
    // which stored a pointer to a stack-alloca {tag, payload} that no longer exists after
    // resume. LLVM detected this as "Instruction does not dominate all uses!" (a raw compiler
    // ICE). The guard rejects at typeck with a clean teaching diagnostic (exit 1, no binary,
    // no LLVM ICE text). This test verifies the clean rejection AND that working crossing-local
    // types (int, string, etc.) are NOT affected (tested separately in other fixtures).
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_maybe_crossing_local_rejected.ynz"));
    assert_eq!(
        code, 1,
        "maybe<int> crossing local must be rejected at compile time (exit 1); stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot yet cross a `wait`"),
        "diagnostic must contain 'cannot yet cross a `wait`'; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("LLVM ERROR") && !stderr.contains("does not dominate"),
        "must be a clean compile error, not a raw LLVM ICE; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no stdout when compile fails; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_union_crossing_local_rejected() {
    // WHY: FIX (d) Round-22 — UnsupportedCrossingLocalType guard for union-typed locals.
    // A union-type crossing local falls into the same dangling-pointer flush/reload path
    // as maybe<T>. This test verifies the clean compile-time rejection for the union case.
    // Regression guard: the guard must fire before codegen runs (exit 1, no LLVM ICE).
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_union_crossing_local_rejected.ynz"));
    assert_eq!(
        code, 1,
        "union-type crossing local must be rejected at compile time (exit 1); stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot yet cross a `wait`"),
        "diagnostic must contain 'cannot yet cross a `wait`'; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("LLVM ERROR") && !stderr.contains("does not dominate"),
        "must be a clean compile error, not a raw LLVM ICE; stderr:\n{stderr}"
    );
    assert!(
        stdout.is_empty(),
        "no stdout when compile fails; got:\n{stdout}"
    );
}

#[test]
// test-ratchet: behavior changed from compile-error to compile-and-run.
// WHY: parameter-shadow guard (Check 3b) — a `let x` inside an `if` body shadows the
// function parameter `x`. The `wait` is at the TOP LEVEL (before the `if`). With the R6
// lexical-scope codegen fix, the inner shadow uses a separate entry-block alloca, and
// `cg.locals["x"]` is restored to the param alloca via restore-all after the `if` exits.
// The `reload_params_from_frame` at state-1 start runs BEFORE the if body is entered, so
// it sees the param alloca — correct. The inner shadow prints its own value (99); the outer
// param prints its value (7) after the if. If this test fails with LLVM ICE, the
// entry-block alloca or restore-all protocol regressed. If it fails with wrong output
// (both 7, or 99/99), restore-all doesn't unwind the shadow correctly.
fn v03_m3a_p1_param_shadow_crossing_rejected() {
    // WHY: Conservative Option-A param-shadow guard (design/concurrency.md §
    // ShadowsCrossingLocal). ANY nested `let x` in a suspending function shares the
    // parameter's name-keyed frame slot; reload_params_from_frame in continuation
    // states overwrites cg.locals["x"] regardless of crossing. Must exit 1 with a
    // clean diagnostic. R6 allowed this (non-crossing predicate); R7 reverts to
    // conservative reject — R6's compile path silently miscompiled the code-reviewer's
    // repro `f(7)` (printed garbage instead of 7).
    // test-ratchet: reverted from compile-and-run back to compile-error (Option A).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_param_shadow_crossing.ynz"));
    assert_eq!(
        code, 1,
        "any nested param shadow in a suspending function must produce a compile error \
         (Option A: conservative blunt guard); \
         stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        !stderr.contains("does not dominate")
            && !stderr.contains("Machine-code generation failed")
            && !stderr.contains("compiler bug"),
        "must produce a clean diagnostic, not an LLVM ICE; stderr:\n{stderr}"
    );
    assert!(
        !stderr.is_empty(),
        "a compile error must produce a non-empty diagnostic; got empty stderr"
    );
}

#[test]
fn v03_m3a_p1_plain_crossing_param_compiles() {
    // WHY: regression guard for the parameter-shadow guard (Check 3b). A parameter that
    // crosses a `wait` with NO inner shadow must continue to compile and produce the
    // correct output — the guard must not reject normal crossing parameters. This is the
    // primary false-positive regression for the Round-12 fix: tightening Check 3b to
    // cover parameters must not break the `function f(x: int) { wait; print(x) }` case.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p1_plain_crossing_param.ynz"));
    assert_eq!(
        code, 0,
        "plain crossing parameter (no shadow) must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "7",
        "must print the original parameter value (7) after suspension; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_toplevel_redecl_after_wait_adv10_rejected() {
    // WHY: top-level re-declaration after wait is a SILENT MISCOMPILE (ADV10). `let x=10`
    // crosses the wait; `let x=99` at top level after the wait shares the same name-keyed
    // frame slot — the second write clobbers the first, producing wrong output (7/7 instead
    // of 7/99). Round 14 incorrectly allowed this program by treating the top-level
    // `let x=99` as "masking" all post-wait reads of the outer binding. The guard must
    // REJECT this shape (exit 1, clean error) — loud error beats silent wrong answer.
    // Regression guard: if round-14's permissive sequential-walk returns, this program
    // will compile to a miscompile instead of printing a safe error.
    // test-ratchet: changing to expect compile would reintroduce ADV10 silent miscompile
    let (_stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_toplevel_redecl_after_wait_adv10.ynz"));
    assert_eq!(
        code, 1,
        "top-level re-declaration after wait must be rejected (silent miscompile); stderr:\n{stderr}"
    );
    assert!(
        stderr.contains('x') && (stderr.contains("wait") || stderr.contains("suspension")),
        "diagnostic must mention `x` and the wait boundary; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_param_toplevel_redecl_after_wait_adv11_rejected() {
    // WHY: top-level re-declaration of a parameter after wait is a SILENT MISCOMPILE (ADV11).
    // Parameter `p` is frame-slotted at function entry; `let p=99` at top level after the
    // wait shares that frame slot — the `let` write clobbers the parameter value. Round 14
    // incorrectly allowed this program; the guard must REJECT it (exit 1, clean error).
    // Regression guard: if round-14's permissive param scan returns, this program compiles
    // to a miscompile (parameter value silently disappears) instead of a clear error.
    // test-ratchet: changing to expect compile would reintroduce ADV11 silent miscompile
    let (_stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p1_param_toplevel_redecl_after_wait_adv11.ynz",
    ));
    assert_eq!(
        code, 1,
        "param with top-level re-declaration after wait must be rejected (silent miscompile); stderr:\n{stderr}"
    );
    assert!(
        stderr.contains('p') && (stderr.contains("wait") || stderr.contains("suspension")),
        "diagnostic must mention `p` and the wait boundary; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p1_ec_crossing_local_propagated_int() {
    // WHY: round-25 regression guard for the EC-crossing-local return-propagation silent
    // miscompile. An EC int-errors value bound from a suspending call, crossing a second
    // suspension, and RETURNED by the caller previously propagated the companion-struct
    // stack pointer as the ok-word instead of the integer bits. The caller received a
    // garbage stack address (e.g. 140734710005520). Regression: if this prints anything
    // other than "42", the errors_capable_locals registration in bind_sm_result_and_flush
    // was removed or the SM ident handler's error/success paths broke.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p1_ec_crossing_local_propagated_int.ynz"));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "42",
        "EC int-errors value must survive crossing + return-propagation; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_ec_crossing_local_propagated_string() {
    // WHY: same EC-crossing-local propagation class as the int case, but for string errors.
    // The ok-word is a heap pointer (stable), but without the errors_capable_locals fix the
    // companion-struct stack pointer was propagated instead. Regression: prints garbage or
    // crashes if the fix is reverted.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p1_ec_crossing_local_propagated_string.ynz",
    ));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "hello",
        "EC string-errors value must survive crossing + return-propagation; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_ec_crossing_local_propagated_number() {
    // WHY: same EC-crossing-local propagation class, but for number (decimal128) errors.
    // The value 9999999999.000000001 requires all 16 bytes of decimal128 precision —
    // truncation or a wrong pointer both produce a different or garbage value.
    // Also exercises the UAF fix in the EC wrapper: the ok-word is a staging-slot pointer
    // inside the frame; without the heap-copy-before-free the staging slot is freed before
    // the caller reads it. Regression: wrong value or crash.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p1_ec_crossing_local_propagated_number.ynz",
    ));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "9999999999.000000001",
        "EC number-errors value must survive crossing + return-propagation with full precision; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p1_ec_crossing_local_propagated_error_path() {
    // WHY: proves the error DISCRIMINANT survives the crossing + propagation, not just the
    // success value. If the fix hardcodes err=0 or flips the discriminant check, inner()'s
    // error is silently swallowed and .or(99) never fires — the output would be "0" (garbage
    // ok-word passed as success) instead of "99" (the fallback). Regression: any value other
    // than "99" means the error discriminant is broken across suspension.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p1_ec_crossing_local_propagated_error_path.ynz",
    ));
    assert_eq!(code, 0, "must exit 0; stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "99",
        "EC error path must fire .or(99) — error discriminant must survive crossing + propagation; got:\n{stdout}"
    );
}

// ── M3a Phase 2: while-loop suspension ───────────────────────────────────────

#[test]
fn v03_m3a_p2_while_counter_runs_in_order() {
    // WHY: guards that (1) the WaitInsideLoop guard no longer fires for `while`, (2) loop
    // counter is frame-backed and survives each suspension, (3) exactly 3 iterations run.
    // Any regression to the old guard would exit non-zero with a compile error. A wrong
    // counter value would mean the frame slot was clobbered or not reloaded on resume.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p2_while_counter.ynz"));
    assert_eq!(
        code, 0,
        "while-loop suspension must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "0\n1\n2",
        "counter must increment 0→1→2 across suspensions; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p2_while_accumulator_correct() {
    // WHY: a loop-carried accumulator must survive N suspensions with the correct total.
    // A stale reload (frame slot not flushed after each mutation) produces 10 instead of
    // 40 (last iteration value only). Catching the flush-after-mutation invariant.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p2_while_accumulator.ynz"));
    assert_eq!(
        code, 0,
        "while accumulator must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "40",
        "accumulator must equal 4*10=40 after 4 suspended iterations; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p2_while_value_returning_correct() {
    // WHY: a non-nothing return type from a suspending function containing a while loop
    // must produce the correct value. Catches the case where the SM return path drops the
    // loop-accumulated value (returns 0 instead of 15).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p2_while_value_returning.ynz"));
    assert_eq!(
        code, 0,
        "value-returning while suspension must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "15",
        "sum_n(5) must return 1+2+3+4+5=15 accumulated across suspensions; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p2_while_sequential_order_preserved() {
    // WHY: per-iteration prints must appear in strict 0→4 order. Parallelization of
    // iterations would produce non-deterministic ordering. This locks the design doc's
    // "loop iterations sequential by default" invariant — M3a has no auto-parallel.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p2_while_sequential_order.ynz"));
    assert_eq!(
        code, 0,
        "sequential-order fixture must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "0\n1\n2\n3\n4",
        "iterations must run in strict 0→4 sequence (not parallelized); got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p2_while_conditional_wait_correct() {
    // WHY: when only some iterations suspend (conditional wait), the crossing locals
    // (i and x) must be correctly preserved for BOTH suspending and non-suspending
    // iterations. A missed flush after a non-suspending iteration would corrupt x on
    // the next iteration, breaking the alternating pattern.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p2_while_conditional_wait.ynz"));
    assert_eq!(
        code, 0,
        "conditional-wait fixture must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "0\n1\n2\n3",
        "all 4 iterations must complete in order, alternating suspend/no-suspend; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_for_wait_now_compiles() {
    // WHY: P3 lifts the WaitInsideLoop guard for `for`. The P2 guard fixture
    // (v0_3_m3a_p2_for_still_rejected.ynz) is now expected to compile and run correctly.
    // If this test fails, the for-loop SM codegen is broken — do NOT revert to the
    // old rejection; fix the codegen instead.
    // test-ratchet: P3 lifted the for-loop WaitInsideLoop guard; this fixture must now compile
    // and run — asserts the for+wait codegen produces correct output.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p2_for_still_rejected.ynz"));
    assert_eq!(
        code, 0,
        "wait in for must now compile and run; stderr:\n{stderr}"
    );
    let _ = stdout;
}

#[test]
fn v03_m3a_p2_while_countdown_before_wait_correct() {
    // WHY: guards the back-edge crossing-local fix. A loop counter decremented BEFORE
    // the `wait` (textually earlier in the body) must be treated as a crossing local
    // because the while condition re-reads it after each suspension. Without the fix,
    // the alloca for `n` lands in a non-dominating block → LLVM ICE
    // ("Instruction does not dominate all uses!"). With the fix, `n` gets a frame slot
    // in sm_entry and the loop produces 3→2→1 in order.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p2_while_countdown_before_wait.ynz"));
    assert_eq!(
        code, 0,
        "countdown-before-wait must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "3\n2\n1",
        "counter must count down 3→2→1 across suspensions; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p2_two_sequential_while_loops_correct() {
    // WHY: guards the has_top_level_let_before_suspension fix. A second suspending
    // `while` loop using a distinct counter (`k`) after a first suspending `while`
    // must compile and run — `k`'s `let` is after a suspension (the first loop),
    // not before one, so the shadow/redeclaration check must not fire. Without the
    // fix, the compiler spuriously rejects `let k` as a "redeclaration after wait".
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p2_two_sequential_while_loops.ynz"));
    assert_eq!(
        code, 0,
        "two sequential suspending while loops must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "3",
        "second loop must complete 3 iterations; k must equal 3; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p2_while_shadow_no_postloop_read_is_rejected() {
    // WHY: Probe-6 silent-miscompile regression. An inner `let n` inside a suspending
    // `while` body shadows an outer `n` that is read by the loop condition (back-edge).
    // When nothing AFTER the loop references `n`, the pre-fix guard's post-wait scan
    // found no references and returned false — shadow check skipped — inner shadow
    // compiled silently. The inner `n` resets to 99 every iteration, making `n > 0`
    // always true → infinite loop. With the Case-3 fix, the guard detects the
    // back-edge read of `n` in the condition and the shadow is rejected at compile time.
    // Must exit 1 (compile error), never hang (timeout = infinite-loop regression).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p2_while_shadow_no_postloop_read_rejected.ynz",
    ));
    assert_eq!(
        code, 1,
        "inner shadow of a back-edge local with no post-loop outer read must be rejected \
         at compile time; if exit 0 or timeout, the silent-miscompile is back; \
         stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        !stderr.is_empty(),
        "a compile error must produce a non-empty diagnostic; got empty stderr"
    );
}

#[test]
fn v03_m3a_p2_while_param_shadow_in_suspending_while_rejected() {
    // WHY: A function parameter shadowed by an inner `let` inside a suspending `while`,
    // where the parameter is read only via the loop condition back-edge. The inner shadow
    // creates a non-entry alloca that `reload_params_from_frame` stores to in the
    // continuation state — LLVM ICE without this guard. This test verifies the check
    // fires for the back-edge-only case. Must exit 1 with a non-empty diagnostic and
    // NO ICE text — ICE text means the guard is not firing and the raw ICE is back.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p2_while_param_shadow_rejected.ynz"));
    assert_eq!(
        code, 1,
        "param shadowed inside a suspending while (condition back-edge read) must \
         produce a compile error; if exit 0, the guard was skipped; \
         stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        !stderr.contains("does not dominate")
            && !stderr.contains("Machine-code generation failed")
            && !stderr.contains("compiler bug"),
        "got an LLVM ICE instead of a clean compile error; \
         stderr:\n{stderr}"
    );
    assert!(
        !stderr.is_empty(),
        "a compile error must produce a non-empty diagnostic; got empty stderr"
    );
}

#[test]
fn v03_m3a_p2_if_param_shadow_crossing_rejected() {
    // WHY: A parameter `p` shadowed by `let p = 99` inside an `if` body that contains a
    // `wait`. The inner alloca lives in the if-body block (not sm_entry); the continuation
    // state's reload overwrites cg.locals["p"] with the non-dominating alloca → LLVM ICE.
    // The fix removes `param_is_genuine_crossing_after_wait` from the Shape (a) gate:
    // any nested param shadow in an SM function is rejected unconditionally.
    // Must exit 1 with a clean diagnostic and NO ICE text.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p2_if_param_shadow_crossing_rejected.ynz",
    ));
    assert_eq!(
        code, 1,
        "param shadowed inside a suspending if-body must produce a compile error; \
         stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        !stderr.contains("does not dominate")
            && !stderr.contains("Machine-code generation failed")
            && !stderr.contains("compiler bug"),
        "got LLVM ICE text instead of a clean compile error; \
         stderr:\n{stderr}"
    );
    assert!(
        !stderr.is_empty(),
        "a compile error must produce a non-empty diagnostic; got empty stderr"
    );
}

#[test]
fn v03_m3a_p2_while_param_shadow_crossing_rejected() {
    // WHY: The canonical Phase-2-owned param-shadow ICE: `function worker(p: int)` with a
    // `while(count>0){ let p = 99; wait sleep(5); print(p); ... }`. The inner `let p` is
    // filtered by `!param_names.contains("p")` in `collect_crossings_in_stmts` — no
    // sm_entry alloca — then the continuation-state reload corrupts cg.locals["p"] with the
    // while-body alloca → LLVM ICE. Renaming `let p` to `let q` compiles clean (control).
    // Must exit 1 with a clean diagnostic and NO ICE text.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p2_while_param_shadow_crossing_rejected.ynz",
    ));
    assert_eq!(
        code, 1,
        "param shadowed inside a suspending while (shadow crosses wait) must produce a \
         compile error; stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        !stderr.contains("does not dominate")
            && !stderr.contains("Machine-code generation failed")
            && !stderr.contains("compiler bug"),
        "got LLVM ICE text instead of a clean compile error; \
         stderr:\n{stderr}"
    );
    assert!(
        !stderr.is_empty(),
        "a compile error must produce a non-empty diagnostic; got empty stderr"
    );
}

#[test]
// WHY: Conservative Option-A param-shadow guard (design/concurrency.md §
// ShadowsCrossingLocal). A non-crossing param shadow in a suspending function is
// still rejected — the blunt `param_has_nested_let_shadow` guard rejects ANY nested
// `let pname` regardless of crossing position, because reload_params_from_frame
// would corrupt cg.locals[pname] across the next suspension even for non-crossing
// allocas. Must exit 1 with a clean diagnostic, not an ICE.
// test-ratchet: reverted from compile-and-run (R6) back to compile-error (R7 Option A).
// R6's non-crossing compile path silently miscompiled the code-reviewer's repro.
fn v03_m3a_p2_param_shadow_noncrossing_rejected() {
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p2_param_shadow_noncrossing_rejected.ynz",
    ));
    assert_eq!(
        code, 1,
        "non-crossing param shadow in a suspending function must still produce a compile \
         error (Option A conservative guard); \
         stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(
        !stderr.contains("does not dominate")
            && !stderr.contains("Machine-code generation failed")
            && !stderr.contains("compiler bug"),
        "must produce a clean diagnostic, not an LLVM ICE; stderr:\n{stderr}"
    );
    assert!(
        !stderr.is_empty(),
        "a compile error must produce a non-empty diagnostic; got empty stderr"
    );
}

// === R7 Option-A regression fixtures ===
// The four tests below lock the Option-A (conservative, safe) boundary:
//   (1) crossing-param-shadow in SM function → reject (silent-miscompile repro)
//   (2) crossing-local-shadow in SM function → reject (judge A8 case)
//   (3) non-async param shadow → compile
//   (4) non-async local shadow → compile

#[test]
fn v03_m3a_p2_r7_silent_miscompile_rejected() {
    // WHY: Code-reviewer R6 silent-miscompile repro. `f(7)` where param `p` is
    // shadowed inside an if body containing a `wait sleep(5)`. Under R6's non-crossing
    // predicate this was allowed and printed `99` + garbage (the coordinator reproduced
    // the miscompile). Under Option A (blunt param_has_nested_let_shadow guard) this
    // must exit 1 with a clean diagnostic — loud error beats silent wrong answer.
    // Regression guard: if this compiles, R6's miscompile has returned.
    let (_stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p2_r7_silent_miscompile_rejected.ynz"));
    assert_eq!(
        code, 1,
        "param shadow with wait inside the if body must be a compile error (Option A); \
         stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("does not dominate")
            && !stderr.contains("Machine-code generation failed")
            && !stderr.contains("compiler bug"),
        "must produce a clean diagnostic, not an LLVM ICE; stderr:\n{stderr}"
    );
    assert!(
        !stderr.is_empty(),
        "compile error must produce a non-empty diagnostic; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p2_r7_local_shadow_crossing_rejected() {
    // WHY: Judge A8 local case. Outer `y` is a crossing local (declared before `wait`,
    // read after it). Inner `let y=7` inside an if body shadows the crossing local.
    // Check 3 (find_shadow_in_stmts, gated on outer_is_genuine_crossing_local) rejects
    // this — consistent with Option A: BOTH param and local same-name shadows around a
    // suspension are rejected conservatively to prevent silent miscompiles.
    // Regression guard: if this compiles, the conservative local guard has been loosened.
    let (_stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p2_r7_local_shadow_crossing_rejected.ynz",
    ));
    assert_eq!(
        code, 1,
        "local shadow of a crossing local must be a compile error (Option A); \
         stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("does not dominate")
            && !stderr.contains("Machine-code generation failed")
            && !stderr.contains("compiler bug"),
        "must produce a clean diagnostic, not an LLVM ICE; stderr:\n{stderr}"
    );
    assert!(
        !stderr.is_empty(),
        "compile error must produce a non-empty diagnostic; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p2_r7_nonasync_param_shadow_compiles() {
    // WHY: Non-async function (no `wait`) with a parameter shadow. Yinz allows
    // shadowing; the conservative param guard only fires for SM functions (inside the
    // `if !kernel_mode && (has_explicit_waits || is_suspending_fn)` block). Non-SM
    // param shadows must compile and run correctly.
    // Regression guard: if this fails (exit 1), the guard is incorrectly firing for
    // non-suspending functions.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p2_r7_nonasync_param_shadow_compiles.ynz",
    ));
    assert_eq!(
        code, 0,
        "non-async param shadow must compile and run correctly; \
         stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert_eq!(
        stdout.trim(),
        "5\n9",
        "inner shadow prints 5; outer param (9) prints after the if; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p2_r7_nonasync_local_shadow_compiles() {
    // WHY: Non-async function (no `wait`) with a local variable shadow. The local-shadow
    // guard (Check 3) only fires for crossing locals in SM functions — a function with no
    // suspension has no crossing locals, so the guard never runs.
    // Regression guard: if this fails, the local-shadow guard is incorrectly firing for
    // non-SM functions.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p2_r7_nonasync_local_shadow_compiles.ynz",
    ));
    assert_eq!(
        code, 0,
        "non-async local shadow must compile and run correctly; \
         stderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert_eq!(
        stdout.trim(),
        "2\n1",
        "inner shadow prints 2; outer local (1) prints after the if; got:\n{stdout}"
    );
}

// ── Phase 3: for-loop + match-arm suspension ──────────────────────────────────

#[test]
fn v03_m3a_p3_for_range_runs_in_order() {
    // WHY: `wait` inside a `for`-over-range runs N times in order, index correct.
    // Guards P3's range-based for-loop SM codegen: the index is frame-backed via the
    // synthetic __ynz_for_idx_0 crossing local; if the frame slot is not flushed/reloaded
    // the index would be 0 on every iteration (stale), printing 0\n0\n0.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_for_range_ordered.ynz"));
    assert_eq!(
        code, 0,
        "for-range with wait must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "0\n1\n2",
        "iterations must run in order 0/1/2; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_for_array_runs_in_order() {
    // WHY: `wait` inside a `for`-over-array visits each element in index order.
    // Guards that the array SM codegen correctly reloads the loop variable (x) from
    // the frame after each suspension. A missing loop-var flush would give garbage or
    // repeated first-element values.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_for_array_wait.ynz"));
    assert_eq!(
        code, 0,
        "for-array with wait must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "10\n20\n30",
        "elements visited in index order; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_for_map_visits_all_entries() {
    // WHY: `wait` inside a `for`-over-map visits every entry. An outer crossing local
    // (`count`) tracks iteration count; its frame-slot survives all 3 suspensions.
    // Guards that map iteration SM codegen increments the index slot correctly and stops
    // at entry count.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_for_map_wait.ynz"));
    assert_eq!(
        code, 0,
        "for-map with wait must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "3",
        "all 3 map entries must be visited; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_match_arm_wait_resumes_correct_arm() {
    // WHY: `wait` inside a match arm resumes into the correct arm and produces correct
    // output. SM match codegen must dispatch to the matching arm's continuation, not fall
    // through to any other arm. Tests the lower_sm_match function.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_match_arm_wait.ynz"));
    assert_eq!(
        code, 0,
        "match arm with wait must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "two",
        "arm `2 =>` must print `two`; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_for_crossing_local_correct() {
    // WHY: for loop with a user-declared crossing local AND a `wait`. Validates that P1
    // (crossing locals) and P3 (for-loop suspension) compose correctly. The accumulator
    // `total` survives all 4 suspensions; the loop index `i` is also frame-backed.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_for_crossing_local.ynz"));
    assert_eq!(
        code, 0,
        "for with crossing local must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "6", "sum 0+1+2+3 = 6; got:\n{stdout}");
}

#[test]
fn v03_m3a_p3_for_sequential_order_preserved() {
    // WHY: per-iteration side effects appear in strict sequence; locks "loop iterations
    // sequential by default" in design/concurrency.md for `for` loops. Any reordering
    // indicates accidental parallelization — impossible in M3a, but a codegen bug could
    // produce non-deterministic order via duplicate BBs or wrong back-edge.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_for_sequential_order.ynz"));
    assert_eq!(
        code, 0,
        "sequential for loop must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "0\n1\n2\n3\n4",
        "iterations must be strictly ordered; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_for_empty_loop_preserves_frame() {
    // WHY: zero-iteration for loop with a suspending body — the body never runs, the
    // frame slot for `result` must be preserved (never-stored-but-loaded guard). If the
    // SM codegen incorrectly clobbers the frame slot or fails to initialize it for the
    // skip case, `result` would print garbage.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_for_empty_loop.ynz"));
    assert_eq!(
        code, 0,
        "empty for loop must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "42",
        "frame slot for `result` must be preserved across the empty loop; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_mutual_suspension_cycle_no_false_positive() {
    // WHY: a self-recursive suspending fn that also calls a second suspending fn
    // non-recursively must NOT trigger MutualSuspensionCycle. Guards that the
    // cycle-detection analysis doesn't false-positive after the for/local plumbing
    // changes expand the suspending-set.
    let (stdout, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p3_no_false_positive_cycle.ynz"));
    assert_eq!(
        code, 0,
        "non-mutual suspending calls must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(stdout.trim(), "done", "expected `done`; got:\n{stdout}");
}

#[test]
fn v03_m3a_wait_inside_loop_fully_lifted() {
    // WHY: the WaitInsideLoop guard has been fully removed — both `for` and `while` with
    // `wait` compile and run. This is the top-level guard-removal confirmation test.
    // If either fixture exits non-zero, the guard was not fully lifted.
    let (stdout_for, stderr_for, code_for) =
        ynz_run_stdout(&fixture("v0_3_m3a_p3_for_range_ordered.ynz"));
    let (stdout_while, stderr_while, code_while) =
        ynz_run_stdout(&fixture("v0_3_m3a_p2_while_counter.ynz"));
    assert_eq!(
        code_for, 0,
        "`for` with wait must compile; stderr:\n{stderr_for}"
    );
    assert_eq!(
        code_while, 0,
        "`while` with wait must compile; stderr:\n{stderr_while}"
    );
    let _ = (stdout_for, stdout_while);
}

#[test]
fn v03_m3a_p3_fixed_bool_iter_wait_rejected() {
    // WHY: `for (b in fixed<boolean>) { wait }` must produce a WHAT/WHAT-INSTEAD/WHY compile
    // error (FixedArrayIterWithWait), not silent wrong output or a raw ICE.
    // fixed<T> arrays are stack-allocated; after suspension the pointer is dangling.
    // The bool case additionally had a type-blind flush (i64 load of an i1 alloca) — both
    // issues are pre-empted by the compile-time guard. Use array<boolean> as the workaround.
    let (_, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_fixed_bool_iter_rejected.ynz"));
    assert_eq!(
        code, 1,
        "fixed<boolean> iter with wait must be rejected at compile time; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("fixed<T>") || stderr.contains("fixed"),
        "error must mention fixed<T>; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p3_fixed_string_iter_wait_rejected() {
    // WHY: `for (n in fixed<string>) { wait }` must produce a WHAT/WHAT-INSTEAD/WHY compile
    // error (FixedArrayIterWithWait), not silent garbage output. The string loop var is a
    // pointer type that becomes dangling after suspension. Same root cause as the bool case.
    let (_, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_fixed_string_iter_rejected.ynz"));
    assert_eq!(
        code, 1,
        "fixed<string> iter with wait must be rejected at compile time; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("fixed<T>") || stderr.contains("fixed"),
        "error must mention fixed<T>; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p3_stored_range_iter_wait_rejected() {
    // WHY: `let r = range(0,3); for (i in r) { wait }` must produce a clean
    // WHAT/WHAT-INSTEAD/WHY compile error (StoredRangeWithWait), not a raw codegen ICE
    // ("for-loop iter is not a call expression"). The SM range arm requires a literal
    // range() call to extract bounds; stored range vars are not yet supported.
    let (_, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_stored_range_wait_rejected.ynz"));
    assert_eq!(
        code, 1,
        "stored range iter with wait must be rejected at compile time; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("stored range") || stderr.contains("range"),
        "error must mention range; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p3_expr_iter_wait_rejected() {
    // WHY: `for (x in makeArray()) { wait }` must produce a clean compile error
    // (ExpressionIterWithWait), not re-evaluate makeArray() once per iteration.
    // The SM codegen calls lower_expr(iter) at every loop header — N+1 evaluations
    // for a call expression breaks the one-alloc-per-task invariant.
    let (_, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_expr_iter_wait_rejected.ynz"));
    assert_eq!(
        code, 1,
        "call-expression iter with wait must be rejected at compile time; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("call-expression") || stderr.contains("iterator"),
        "error must mention call-expression iterator; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p3_array_int_loop_var_survives_wait() {
    // WHY: `for (x in array<int>) { wait; print(x) }` must print each element correctly.
    // Guards the type-aware flush path in flush_for_loop_var for scalar (Int) loop variables
    // from heap-allocated arrays. The loop var is a crossing local that survives suspension
    // via the frame slot (i64 alloca, raw i64 flush/reload).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_for_int_array_loop_var.ynz"));
    assert_eq!(
        code, 0,
        "array<int> loop var must survive wait and print correctly; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "10\n20\n30",
        "each element printed after wait; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_map_counter_with_wait() {
    // WHY: `for (entry in map) { wait }` with an outer crossing counter visits all
    // N entries. Guards that the map SM codegen increments the index correctly and
    // that non-destructured map iteration works with a wait. The outer `count` local is
    // the only value read after wait — entry.* is not accessed (the MapEntry deferral
    // covers entry.key/entry.value reads after suspension).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_map_counter_with_wait.ynz"));
    assert_eq!(
        code, 0,
        "map entry loop with wait must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "3",
        "all 3 entries must be visited; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_number_loop_var_survives_wait() {
    // WHY: `for (x in array<number>) { wait; print(x) }` must print exact decimal values.
    // Guards the decimal128 (i128, 2-slot) flush path in flush_for_loop_var. Without the
    // decimal128 branch, the loop var falls to the pointer branch → writes only 8 bytes →
    // reload reassembles garbage i128 → print produces `0.000` instead of the true value.
    // The fix stores lo/hi halves to consecutive frame slots and reloads them symmetrically.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_for_number_loop_var.ynz"));
    assert_eq!(
        code, 0,
        "array<number> loop var must survive wait; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "1.5\n2.25\n3.125",
        "each decimal128 element must print exactly after wait; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_fixed_crossing_local_rejected() {
    // WHY: `let arr: fixed<int> = ...; wait; for (v in arr)` must produce a clean
    // WHAT/WHAT-INSTEAD/WHY compile error (UnsupportedCrossingLocalType — fixed<T>),
    // not compile-exit-0 with a dangling-pointer UAF at runtime.
    // fixed<T> allocas live on the resume function's stack; that stack frame is freed
    // on suspension. The crossing-local frame slot holds a dangling pointer on resume.
    let (_, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p3_fixed_crossing_local_rejected.ynz"));
    assert_eq!(
        code, 1,
        "fixed<int> crossing local must be rejected at compile time; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("fixed<T>") || stderr.contains("fixed<int>") || stderr.contains("fixed"),
        "error must mention fixed type; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p3_map_entry_after_wait_rejected() {
    // WHY: `for (entry in m) { wait; total += entry.value }` must produce a clean
    // WHAT/WHAT-INSTEAD/WHY compile error (UnsupportedCrossingLocalType — MapEntry).
    // The MapEntry {key,value} struct needs 2 frame slots; the current mechanism assigns 1.
    // Only field[0] (entry.key) survives; field[1] (entry.value) reads garbage on resume.
    // Workaround: bind entry.key/entry.value to separate lets before the wait.
    let (_, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p3_map_entry_after_wait_rejected.ynz"));
    assert_eq!(
        code, 1,
        "map entry.* read after wait must be rejected at compile time; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("MapEntry") || stderr.contains("entry") || stderr.contains("map"),
        "error must mention the MapEntry or map iteration; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p3_string_array_loop_var_survives_wait() {
    // WHY: `for (n in array<string>) { wait; print(n) }` must print each element correctly.
    // Guards the pointer-backed type path in flush_for_loop_var — string pointers are
    // heap-stable so ptr_to_int/int_to_ptr round-trips preserve the address across suspension.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_for_string_array_wait.ynz"));
    assert_eq!(
        code, 0,
        "array<string> loop var must survive wait; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "amy\nbob\ncy",
        "each string element must print after wait; got:\n{stdout}"
    );
}

// ── Round-4 exhaustive type audit ────────────────────────────────────────────
// Each test covers one Type variant as a crossing local across a TOP-LEVEL wait.
// Every variant must land in exactly one bucket: frame-backed-correct OR loud-rejected.
// These tests replace the round-3 paper audit table with TESTED coverage.

#[test]
fn v03_m3a_p3_audit_int_crossing_local() {
    // WHY: int is a scalar i64 — flush stores raw i64 to frame slot, reload reads it back.
    // If the frame-slot path is broken for int, this produces wrong output (not 42).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_int_crossing.ynz"));
    assert_eq!(
        code, 0,
        "int crossing local must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "42",
        "int must round-trip across wait; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_audit_bool_crossing_local() {
    // WHY: bool is i1 — flush must zext to i64 (not raw store) else the frame slot holds
    // a 1-byte value in an 8-byte region (SIGSEGV on reload). The same class as P1's bool fix.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_bool_crossing.ynz"));
    assert_eq!(
        code, 0,
        "bool crossing local must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "true",
        "bool must round-trip across wait as true; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_audit_float_crossing_local() {
    // WHY: float is f64 — flush must bitcast f64->i64, reload must bitcast i64->f64.
    // A raw i64 load from an f64 alloca would read garbage bits. Verifies no crash.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_float_crossing.ynz"));
    assert_eq!(
        code, 0,
        "float crossing local must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "ok",
        "float must survive wait without crash; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_audit_number_crossing_local() {
    // WHY: number is decimal128 (i128, 16 bytes) — flush splits into 2 consecutive i64
    // frame slots (lo+hi halves). A single slot would silently truncate to 8 bytes.
    // 0.1 + 0.2 = 0.3 exactly in decimal128 (not 0.30000...0004 as in IEEE 754 float).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_number_crossing.ynz"));
    assert_eq!(
        code, 0,
        "number crossing local must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "0.3",
        "decimal128 must round-trip exactly across wait; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_audit_string_crossing_local() {
    // WHY: string is a heap-stable pointer — flush stores ptr_to_int to frame slot,
    // reload uses int_to_ptr. A missing pointer-path would store the raw stack address
    // (alloca ptr) which dangles after suspension.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_string_crossing.ynz"));
    assert_eq!(
        code, 0,
        "string crossing local must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "hello world",
        "string must survive wait; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_audit_array_crossing_local() {
    // WHY: array<T> is heap-allocated — the heap pointer is stable across suspension.
    // Flush stores the pointer as i64, reload restores the same heap address.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_array_crossing.ynz"));
    assert_eq!(
        code, 0,
        "array crossing local must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "10\n20\n30",
        "array elements must be correct after wait; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_audit_map_crossing_local() {
    // WHY: map<K,V> is heap-allocated — the heap pointer is stable across suspension.
    // Verifies the map can be iterated correctly after a wait (entry count = 3).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_map_crossing.ynz"));
    assert_eq!(
        code, 0,
        "map crossing local must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "3",
        "map must have 3 entries after wait; got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_audit_shape_crossing_local() {
    // WHY: Shape with primitive fields is frame-embedded (field bytes stored inline in the
    // composed heap frame). The pointer returned by alloca points directly into the frame
    // region — no flush needed, fields are always live. Verifies the field values survive.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_shape_crossing.ynz"));
    assert_eq!(
        code, 0,
        "Shape crossing local must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "30",
        "Shape fields must survive wait (10+20=30); got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_audit_range_crossing_loud_rejected() {
    // WHY: range binding crossing a top-level wait is stack-allocated; iterating the
    // dangling range produces zero iterations — silent wrong output without guard.
    // UnsupportedCrossingLocalType must fire (exit 1) with a WHAT/WHAT-INSTEAD/WHY error.
    let (_, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_range_crossing_rejected.ynz"));
    assert_eq!(
        code, 1,
        "range crossing local must be rejected at compile time; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("range") || stderr.contains("Range"),
        "error must mention range; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p3_audit_maybe_crossing_loud_rejected() {
    // WHY: maybe<T> is a stack alloca pointing to a {tag,payload} struct. The stack
    // frame is freed after suspension; reloading the dangling alloca pointer is UB.
    // UnsupportedCrossingLocalType must fire (exit 1).
    let (_, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_maybe_crossing_rejected.ynz"));
    assert_eq!(
        code, 1,
        "maybe crossing local must be rejected at compile time; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("maybe") || stderr.contains("union") || stderr.contains("dynamic"),
        "error must mention the unsupported type; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p3_audit_fixed_crossing_loud_rejected() {
    // WHY: fixed<T> is stack-allocated; its pointer dangles after suspension.
    // UnsupportedCrossingLocalType must fire (exit 1). Workaround: array<T>.
    let (_, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_fixed_crossing_rejected.ynz"));
    assert_eq!(
        code, 1,
        "fixed crossing local must be rejected at compile time; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("fixed"),
        "error must mention fixed type; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p3_audit_map_nested_wait_field_rejected() {
    // WHY: guards the flat-scan bug in body_reads_field_after_wait. A `wait` nested
    // inside `if (c) { wait sleep(5) }` was previously missed — the guard only set
    // seen_wait for a FLAT top-level Stmt::Expr(Wait). The fix uses stmt_contains_wait_anywhere
    // (recursive) so any nesting depth triggers the rejection.
    let (_, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_map_nested_wait_rejected.ynz"));
    assert_eq!(
        code, 1,
        "map entry.* after nested if-wait must be rejected; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("entry") || stderr.contains("map") || stderr.contains("MapEntry"),
        "error must mention map entry; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p3_audit_array_number_sm_no_leak() {
    // WHY: guards the round-3 decimal128 heap-per-element leak. NumberLit elements in an
    // SM array<number> now use module-level LLVM globals (eternal, alloc=0 per element).
    // Before fix: alloc=N+1/free=1 (1 elem→2, 3→4, 5→6). After: alloc=1/free=1 always.
    // Mutation proof: changing build_decimal_global back to ynz_alloc makes this fail.
    let (alloc, free) = ynz_run_with_alloc_counter("v0_3_m3a_p3_audit_array_number_no_leak.ynz");
    assert_eq!(
        alloc, free,
        "array<number> in SM function must have alloc==free (no decimal128 element leak); \
         alloc={alloc}, free={free}"
    );
    assert_eq!(
        alloc, 1,
        "exactly 1 alloc (task frame) for array<number> SM function; got alloc={alloc}"
    );
}

// ── Round-5: shape-embed loop var (FIX 2) ────────────────────────────────────

#[test]
fn v03_m3a_p3_for_shape_loop_var_survives_wait() {
    // WHY: guards the shape-embed loop-var fix (round-5 FIX 2). Before the fix,
    // `flush_for_loop_var` fell to the else-branch and stored the stack-alloca pointer
    // as i64 in the frame slot — every per-run address gave different garbage output
    // (ASLR-varying: 140733642878985 / 140722360947977). After the fix, struct-literal
    // elements in SM array<Shape> are emitted as module-level LLVM globals (static
    // lifetime) so ynz_array_get returns a stable address; the loop-var binding then
    // memcpies the struct bytes into the pre-wired frame region. Field values survive
    // any number of resume calls with no per-run variance.
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_for_shape_loop_var.ynz"));
    assert_eq!(
        code, 0,
        "array<Shape> loop var + wait must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "30",
        "Shape fields must survive suspension (10+20=30); got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_for_shape_loop_var_alloc_one() {
    // WHY: GR8 invariant — array<Shape> loop var must use ONE ynz_alloc (the task frame).
    // Struct-literal elements are module-level LLVM globals (no ynz_alloc calls); the
    // heap YnzArray uses raw malloc (not counted). alloc=1/free=1 proves zero per-element
    // or per-iteration extra allocation. Regression: converting globals back to ynz_alloc
    // would push alloc=3/free=1 (2 struct elements + 1 frame).
    let (alloc, free) = ynz_run_with_alloc_counter("v0_3_m3a_p3_for_shape_loop_var.ynz");
    assert_eq!(
        alloc, 1,
        "array<Shape> loop var must use ONE ynz_alloc (frame only); got alloc={alloc}"
    );
    assert_eq!(
        free, alloc,
        "alloc/free must be balanced for array<Shape> loop var; alloc={alloc}, free={free}"
    );
}

#[test]
fn v03_m3a_p3_for_nested_shape_loop_var_rejected() {
    // WHY: nested-shape loop vars must be CLEAN-REJECTED — same guard as direct
    // nested-shape crossing locals (NestedShapeCrossing). Previously the check only
    // fired via find_crossing_local_typeck_type_in_map (Stmt::Let only); for-loop vars
    // have no Stmt::Let so the check silently skipped them. After the fix, Check 2 also
    // calls find_for_loop_var_type_in_stmts as a fallback for loop vars.
    let (_, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_for_nested_shape_rejected.ynz"));
    assert_eq!(
        code, 1,
        "array<NestedShape> loop var must be rejected at compile time; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("nested-shape field"),
        "error must mention nested-shape field; stderr:\n{stderr}"
    );
}

// ── Round-5: missing direct-crossing local fixtures ───────────────────────────

#[test]
fn v03_m3a_p3_audit_options_crossing_local() {
    // WHY: options values are i8 tag bytes — stored as a 1-slot frame entry (i64 with
    // i8 tag, zext on flush / truncate on reload). This audits that the options crossing-
    // local path works end-to-end: the tag survives suspension and the match arm fires
    // correctly post-wait. Distinct from the union/dynamic paths (which are rejected).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_options_crossing.ynz"));
    assert_eq!(
        code, 0,
        "options crossing local must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "active",
        "options tag must survive wait (Status.active → 'active'); got:\n{stdout}"
    );
}

#[test]
fn v03_m3a_p3_audit_dynamic_crossing_loud_rejected() {
    // WHY: dynamic Contract values hold a fat pointer ({data-ptr, vtable-ptr}) on the
    // resume function's stack. The stack is freed when the function suspends and returns;
    // reloading the stored fat-pointer on the next resume is a dangling-pointer read.
    // UnsupportedCrossingLocalType must fire (exit 1) with a teaching error.
    let (_, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_p3_audit_dynamic_crossing_rejected.ynz"));
    assert_eq!(
        code, 1,
        "dynamic crossing local must be rejected at compile time; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("dynamic") || stderr.contains("union"),
        "error must mention dynamic type; stderr:\n{stderr}"
    );
}

// ── Round-6: ArrayShapeRuntimeFieldWithWait guard ────────────────────────────

#[test]
fn v03_m3a_p3_array_shape_runtime_field_crossing_rejected() {
    // WHY: guards the ArrayShapeRuntimeFieldWithWait interim fix. An array<Shape> crossing
    // a `wait` whose elements have runtime-computed field values (e.g. `qty: a` where `a`
    // is a variable) previously printed ASLR-varying stack garbage (140737423097900 one run,
    // 140728717547596 the next) because element pointers point to stack allocas in the
    // constructing resume frame — freed on suspension. The guard must produce exit 1 with
    // a diagnostic that names the crossing local and explains the root cause.
    // Dropping this test and rerunning would reveal the silent miscompile; do NOT weaken.
    let (_, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p3_array_shape_runtime_field_rejected.ynz",
    ));
    assert_eq!(
        code, 1,
        "array<Shape> with runtime field values crossing a wait must be rejected; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("runtime-computed field"),
        "diagnostic must mention runtime-computed field values; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("items"),
        "diagnostic must name the crossing local `items`; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_p3_array_shape_literal_crossing_still_works() {
    // WHY: no-over-reject boundary for ArrayShapeRuntimeFieldWithWait. An array<Shape>
    // whose elements have ALL-LITERAL int/bool field values ([{id:1,qty:10},{id:2,qty:20}])
    // crosses a `wait` correctly because codegen emits LLVM module-level globals for those
    // elements (stable addresses, never dangle). This test must print "30" = 10+20.
    // If the guard fires here, it is over-rejecting literal-field arrays (regression).
    let (stdout, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_p3_array_shape_literal_crossing_still_works.ynz",
    ));
    assert_eq!(
        code, 0,
        "array<Shape> with all-literal fields crossing a wait must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        "30",
        "all-literal array<Shape> must survive suspension and sum correctly (10+20=30); got:\n{stdout}"
    );
}

// ── Round-7: adversarial pre-check deletion cases ────────────────────────────

#[test]
fn v03_m3a_r7_array_shape_between_waits_rejected() {
    // WHY: catches the under-rejection hole where the old pre-check skipped an
    // array<Shape> whose `let` is declared BETWEEN two waits (not before the
    // first wait at the top level). The crossing analysis proves it crosses the
    // second wait, so the guard must fire. Without this test, deleting the
    // pre-check could be regressed back in, silently reinstating the miscompile.
    let (_, stderr, code) = ynz_run_stdout(&fixture(
        "v0_3_m3a_r7_array_shape_between_waits_rejected.ynz",
    ));
    assert_eq!(
        code, 1,
        "array<Shape> with runtime field values declared between two waits must be rejected; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("runtime-computed field"),
        "diagnostic must mention runtime-computed field values; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("items"),
        "diagnostic must name the crossing local `items`; stderr:\n{stderr}"
    );
}

#[test]
fn v03_m3a_r7_array_shape_nested_if_rejected() {
    // WHY: catches the under-rejection hole where the old pre-check skipped an
    // array<Shape> whose `let` is declared inside a nested `if` body. The top-
    // level scan couldn't find the `let` there and returned false; the crossing
    // analysis (which examines nested scopes) proves it crosses a wait, so the
    // guard must fire. Without this test, the nested-if hole goes undetected.
    let (_, stderr, code) =
        ynz_run_stdout(&fixture("v0_3_m3a_r7_array_shape_nested_if_rejected.ynz"));
    assert_eq!(
        code, 1,
        "array<Shape> with runtime field values nested in an if body crossing a wait must be rejected; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("runtime-computed field"),
        "diagnostic must mention runtime-computed field values; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("items"),
        "diagnostic must name the crossing local `items`; stderr:\n{stderr}"
    );
}

// ── v0.3-M3a Phase 4: cross-impl consistency gate ────────────────────────────

/// Build a fixture to a tmpdir binary (default or --no-auto-parallel) and run it.
///
/// The binary is placed in a unique tmpdir so concurrent tests don't collide.
/// Caller is responsible for nothing — the tmpdir is cleaned up by TempDir's Drop.
fn build_to_tmpdir_and_run(src: &Path, no_auto_parallel: bool) -> (String, String, i32) {
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    let mut build_cmd = Command::new(ynz_binary());
    build_cmd.arg("build").arg(src).env("CLICOLOR", "0");
    if no_auto_parallel {
        build_cmd.arg("--no-auto-parallel");
    }
    let build_out = build_cmd.output().expect("failed to spawn ynz build");
    if !build_out.status.success() {
        let stderr = String::from_utf8_lossy(&build_out.stderr).into_owned();
        return (String::new(), format!("build failed: {stderr}"), 1);
    }

    // ynz build writes the binary alongside the source file with the extension stripped
    // (e.g., foo.ynz -> foo). Copy it to tmpdir to avoid leaving build artifacts in
    // the fixtures directory alongside committed test sources.
    // test-ratchet: binary path is src.with_extension(""), not src_dir/bin; fixing a
    // bug in the original helper path logic introduced in the same edit.
    let built_binary = src.with_extension("");
    let run_binary = tmp.path().join("bin");
    if let Err(e) = std::fs::copy(&built_binary, &run_binary) {
        return (String::new(), format!("failed to copy binary: {e}"), 1);
    }
    // Clean up the binary next to the fixture to avoid polluting the fixtures dir.
    let _ = std::fs::remove_file(&built_binary);

    let run_out = Command::new(&run_binary)
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to run compiled binary");
    let stdout = String::from_utf8_lossy(&run_out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&run_out.stderr).into_owned();
    let code = run_out.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

#[test]
fn v03_m3a_p4_no_auto_parallel_byte_identical_to_default_on_simple_wait() {
    // WHY: --no-auto-parallel must produce byte-identical stdout/stderr/exit-code to
    // the default build on every fixture where the parallel pass is a no-op. In M3a
    // the pass does not yet exist (main.rs:210 discards the flag), so this is
    // trivially true — but the gate must be WIRED so M3b cannot accidentally break
    // consistency without a failing test. Guards the cross-impl consistency AC in P4.
    let src = fixture("v0_3_m3a_p1_int_crossing_one_wait.ynz");
    // Confirm the fixture passes via `ynz run` (the canonical dev path).
    let (run_stdout, run_stderr, run_code) = ynz_run_stdout(&src);
    assert_eq!(
        run_code, 0,
        "fixture must pass ynz run; stderr:\n{run_stderr}"
    );
    // Build with --no-auto-parallel and compare output.
    let (nopar_stdout, nopar_stderr, nopar_code) = build_to_tmpdir_and_run(&src, true);
    assert_eq!(
        nopar_code, 0,
        "--no-auto-parallel build must exit 0; error:\n{nopar_stderr}"
    );
    // Build without --no-auto-parallel and compare.
    let (default_stdout, _default_stderr, default_code) = build_to_tmpdir_and_run(&src, false);
    assert_eq!(default_code, 0, "default build must exit 0");
    // The three outputs must be byte-identical (trivially true in M3a — no parallel pass).
    assert_eq!(
        nopar_stdout, default_stdout,
        "--no-auto-parallel stdout must equal default build stdout"
    );
    assert_eq!(
        run_stdout.trim(),
        default_stdout.trim(),
        "ynz run stdout must equal ynz build+run stdout"
    );
}

#[test]
fn v03_m3a_p4_no_auto_parallel_byte_identical_on_for_loop_suspension() {
    // WHY: Cross-impl consistency check on the for-loop-with-wait fixture (P3 feature).
    // The for-loop SM codegen must be byte-identical regardless of the --no-auto-parallel
    // flag. Catches any future M3b auto-parallel pass that accidentally changes the
    // sequential per-iteration ordering that P3's design locks in.
    let src = fixture("v0_3_m3a_p3_for_array_wait.ynz");
    let (nopar_stdout, nopar_stderr, nopar_code) = build_to_tmpdir_and_run(&src, true);
    assert_eq!(
        nopar_code, 0,
        "--no-auto-parallel for-loop fixture must exit 0; error:\n{nopar_stderr}"
    );
    let (default_stdout, _default_stderr, default_code) = build_to_tmpdir_and_run(&src, false);
    assert_eq!(default_code, 0, "default for-loop build must exit 0");
    assert_eq!(
        nopar_stdout, default_stdout,
        "--no-auto-parallel stdout must equal default stdout on for-loop suspension"
    );
}

/// Run a fixture with alloc-counter instrumentation and return (alloc_count, free_count).
/// Used by audit tests that need to verify the one-alloc-per-task-tree invariant.
fn ynz_run_with_alloc_counter(fixture_name: &str) -> (u64, u64) {
    let fixture_path = fixture(fixture_name);
    let count_file = std::env::temp_dir().join(format!("ynz_audit_alloc_{fixture_name}.txt"));
    let _ = std::fs::remove_file(&count_file);
    let _ = Command::new(ynz_binary())
        .args(["run", fixture_path.to_str().expect("valid path")])
        .env("CLICOLOR", "0")
        .env("YNZ_ALLOC_COUNTER", "1")
        .env(
            "YNZ_ALLOC_COUNTER_OUTPUT",
            count_file.to_str().expect("valid path"),
        )
        .output()
        .expect("ynz binary not found");
    let content =
        std::fs::read_to_string(&count_file).unwrap_or_else(|_| "alloc=0\nfree=0\n".to_string());
    let _ = std::fs::remove_file(&count_file);
    let parse_count = |prefix: &str| -> u64 {
        content
            .lines()
            .find(|l| l.starts_with(prefix))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    };
    (parse_count("alloc"), parse_count("free"))
}
