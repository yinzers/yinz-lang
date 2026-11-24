//! Card #419 (Golden Rule 2, silent wrong): an int literal that typeck types as `float`
//! (under a `float` hint) was lowered by codegen as an i64 constant, and the i64 bits were
//! then stored as f64 bits — `let whole: float = 2` printed a ~1e-323 denormal / zero.
//! Tier: High (data integrity: wrong value, no diagnostic). Each fixture is one place
//! typeck applies the float hint to an `IntLit`: a `let`, a call argument, a shape field.

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

/// One named test per case so the runner parallelizes them and names the failing one.
macro_rules! float_literal_case {
    ($test_name:ident, $fixture:literal, $expected:literal) => {
        // bug: an int literal under a `float` hint is lowered as i64 bits and printed as a ~1e-323 denormal or zero | tier: High
        #[test]
        fn $test_name() {
            let (stdout, stderr, code) = run_fixture($fixture);
            assert_eq!(code, 0, "{} must exit 0; stderr:\n{stderr}", $fixture);
            assert_eq!(
                stdout, $expected,
                "{}: int literal under a float hint",
                $fixture
            );
        }
    };
}

float_literal_case!(
    float_hinted_int_literal_let_prints_value,
    "float_int_literal_let.ynz",
    "2\n"
);
float_literal_case!(
    float_hinted_int_literal_argument_prints_value,
    "float_int_literal_arg.ynz",
    "3\n"
);
float_literal_case!(
    float_hinted_int_literal_shape_field_prints_value,
    "float_int_literal_field.ynz",
    "4\n"
);
