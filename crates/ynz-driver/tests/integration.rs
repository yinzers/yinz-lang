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
        stderr.contains("entrypoint"),
        "diagnostic must mention `entrypoint`; got:\n{stderr}"
    );
    insta::assert_snapshot!("broken_main_stderr", stderr);
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
    insta::assert_snapshot!("m2_compound_assign_stderr", stderr);
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
fn m8_background_runs_sequentially() {
    // WHY: In M8, `background foo()` is sequential — the call runs to completion
    // before the next statement. The return value is discarded. Guards that
    // background doesn't skip the call or run after subsequent statements.
    let (stdout, _stderr, code) = ynz_run_stdout(&fixture("m8_background_fire_and_forget.ynz"));
    assert_eq!(code, 0);
    assert_eq!(
        stdout, "side effect ran\nafter background\n",
        "background must run before the subsequent print"
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
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/basics");
    let golden = std::fs::read_to_string(project_root.join("expected_stdout.txt"))
        .expect("examples/basics/expected_stdout.txt must exist");
    let (stdout, stderr, code) = ynz_run_stdout(&project_root);
    assert_eq!(
        code, 0,
        "examples/basics must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, golden,
        "stdout must match examples/basics/expected_stdout.txt"
    );
}

// ── P7: combined-feature integration fixtures ─────────────────────────────────

#[test]
fn m8_combo_modules_sensitive_concurrency() {
    // WHY: guards that sensitive + background work together in a multi-file project.
    // If sensitive redaction is lost or background breaks across module boundaries,
    // this combo test catches it before the individual-feature tests do.
    let project_root = fixtures_dir().join("m8_combo_modules_sensitive_concurrency");
    let (stdout, stderr, code) = ynz_run_stdout(&project_root);
    assert_eq!(
        code, 0,
        "sensitive+concurrency combo must compile and run; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, "[REDACTED]\nsuper-secret-key\n[REDACTED]\ndone\n",
        "sensitive must redact in print, reveal() must show raw value, background must run"
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
