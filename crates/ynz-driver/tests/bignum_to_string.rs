//! Card #419 item 2: `.toString()` (and `.toFloat()` / `.toInt()`) on `number<N>` for every
//! precision. Before the fix `number<N>` for any N != 34 had no methods ("`number` does not
//! have a method called `toString`"), and a bignum's pointer would have been read as a
//! decimal128 value had typeck let it through.

use std::path::PathBuf;
use std::process::Command;

fn run_fixture(name: &str) -> (String, String, i32) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let out = Command::new(env!("CARGO_BIN_EXE_ynz"))
        .args(["run", path.to_str().unwrap()])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz binary");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

fn assert_stdout_lines(fixture: &str, expected: &[&str]) {
    let (stdout, stderr, code) = run_fixture(fixture);
    assert_eq!(code, 0, "{fixture} must compile and run; stderr:\n{stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, expected, "{fixture} stdout");
}

#[test]
fn bignum_n40_to_string_prints_all_forty_digits() {
    // Tier: Medium (user flow). Bug caught: `number<40>.toString()` refused at compile time, and a
    // bignum pointer read as a decimal128 value if it got through.
    // 1/3 at 40 digits: "0." followed by exactly forty 3s.
    assert_stdout_lines(
        "bignum_to_string_n40.ynz",
        &["0.3333333333333333333333333333333333333333"],
    );
}

#[test]
fn bignum_n50_to_string_prints_all_fifty_digits() {
    // Tier: Medium (user flow). Bug caught: `toString` refused for any N other than 34; this is a
    // second precision, and its last digit rounds up rather than truncating.
    // 2/3 at 50 digits: "0." followed by exactly fifty 6s, the last rounded to 7.
    assert_stdout_lines(
        "bignum_to_string_n50.ynz",
        &["0.66666666666666666666666666666666666666666666666667"],
    );
}

#[test]
fn bignum_shape_field_to_string_matches_print_of_the_field() {
    // Tier: Medium (user flow). Bug caught: `toString` on a bignum shape-field receiver refused or
    // mis-lowered, giving a string unlike `print` of the same field.
    let digits = "0.3333333333333333333333333333333333333333";
    assert_stdout_lines("bignum_to_string_shape_field.ynz", &[digits, digits]);
}

#[test]
fn narrow_number_to_string_results_do_not_share_a_buffer() {
    // Tier: Medium (user flow). Bug caught: both strings aliasing the runtime's thread-local
    // format buffer (printed 2.5 twice).
    assert_stdout_lines("number_to_string_owned.ynz", &["1.5", "2.5"]);
}

#[test]
fn number_conversions_read_the_right_representation_at_every_precision() {
    // Tier: Medium (user flow). Bug caught: `toFloat`/`toInt` on `number<N>` reading a bignum
    // string as a decimal128 cell (or the reverse) once method lookup covers every precision.
    assert_stdout_lines(
        "bignum_number_conversions.ynz",
        &[
            "0.3333333333333333", // number<40>.toFloat()
            "0",                  // number<40>.toInt(): 0.333.. truncates to 0
            "12345",              // number<50>.toInt()
            "1.5",                // number<20>.toString()
            "1.5",                // number<20>.toFloat()
            "1",                  // number<20>.toInt()
        ],
    );
}
