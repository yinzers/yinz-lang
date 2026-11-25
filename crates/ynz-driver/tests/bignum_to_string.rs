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

/// One named test per input class (precision, receiver position, aliasing, conversion kind);
/// each case runs one fixture and compares its stdout lines exactly.
macro_rules! number_text_case {
    ($test_name:ident, $fixture:literal, $expected_lines:expr) => {
        // bug: `number<N>` toString/toFloat/toInt prints a wrong digit string, reads a bignum as a decimal128 cell, or two results share one buffer | tier: High
        #[test]
        fn $test_name() {
            assert_stdout_lines($fixture, $expected_lines);
        }
    };
}

// 1/3 at 40 digits: "0." followed by exactly forty 3s (`toString` refused for N != 34).
number_text_case!(
    bignum_n40_to_string_prints_all_forty_digits,
    "bignum_to_string_n40.ynz",
    &["0.3333333333333333333333333333333333333333"]
);

// 2/3 at 50 digits: "0." followed by exactly fifty 6s, the last rounded up to 7.
number_text_case!(
    bignum_n50_to_string_rounds_the_last_digit_up,
    "bignum_to_string_n50.ynz",
    &["0.66666666666666666666666666666666666666666666666667"]
);

// `toString` on a bignum shape-field receiver matches `print` of the same field.
number_text_case!(
    bignum_shape_field_to_string_matches_print_of_the_field,
    "bignum_to_string_shape_field.ynz",
    &[
        "0.3333333333333333333333333333333333333333",
        "0.3333333333333333333333333333333333333333"
    ]
);

// Both strings aliasing the runtime's thread-local format buffer printed 2.5 twice.
number_text_case!(
    narrow_number_to_string_results_do_not_share_a_buffer,
    "number_to_string_owned.ynz",
    &["1.5", "2.5"]
);

// `toFloat`/`toInt` must read the representation of each precision.
number_text_case!(
    number_conversions_read_the_right_representation_at_every_precision,
    "bignum_number_conversions.ynz",
    &[
        "0.3333333333333333", // number<40>.toFloat()
        "0",                  // number<40>.toInt(): 0.333.. truncates to 0
        "12345",              // number<50>.toInt()
        "1.5",                // number<20>.toString()
        "1.5",                // number<20>.toFloat()
        "1",                  // number<20>.toInt()
    ]
);
