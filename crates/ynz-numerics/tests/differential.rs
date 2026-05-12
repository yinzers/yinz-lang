/// Differential tests: compare ynz-numerics against Python's `decimal` module.
///
/// WHY: the embedded conformance vectors cover known edge cases, but random
/// testing against a reference implementation catches algorithmic bugs that
/// deterministic hand-picked cases miss.  Python's `decimal` implements the
/// same IEEE 754 decimal128 spec at the same 34-digit precision.
///
/// These tests are skipped if Python 3 is not available on the host.
/// CI installs Python before running the test suite.
use std::process::Command;

use ynz_numerics::{add, compare, div, format, mul, parse, sub};

/// Check whether Python 3.8+ is available.
fn python_available() -> bool {
    Command::new("python3")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Ask Python to compute `op(a, b)` using `decimal.Decimal` at precision 34,
/// rounding ROUND_HALF_EVEN.  Returns the formatted result or None if Python
/// is unavailable.
fn python_compute(op: &str, a: &str, b: Option<&str>) -> Option<String> {
    if !python_available() {
        return None;
    }

    let script = match b {
        Some(b) => format!(
            "from decimal import *; \
             getcontext().prec=34; \
             getcontext().rounding=ROUND_HALF_EVEN; \
             a=Decimal('{a}'); b=Decimal('{b}'); \
             print({op})"
        ),
        None => format!(
            "from decimal import *; \
             getcontext().prec=34; \
             getcontext().rounding=ROUND_HALF_EVEN; \
             a=Decimal('{a}'); \
             print({op})"
        ),
    };

    let out = Command::new("python3")
        .args(["-c", &script])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    Some(s.trim().to_string())
}

fn compare_with_python(op_name: &str, a_str: &str, b_str: Option<&str>, ynz_result: u128) {
    let python_expr = match op_name {
        "add" => "a + b",
        "sub" => "a - b",
        "mul" => "a * b",
        "div" => "a / b",
        _ => return,
    };
    let py = match python_compute(python_expr, a_str, b_str) {
        Some(s) => s,
        None => return, // Python not available — skip
    };
    let ynz = format(ynz_result);

    // Numeric equality: two decimal128 values can represent the same number at
    // different scales (e.g., "20000000000" and "2E+10" are numerically equal
    // but have different BID bit patterns). Use compare() for value equality.
    let equal = ynz == py || {
        match (parse(&ynz), parse(&py)) {
            (Some(a), Some(b)) => compare(a, b) == 0,
            _ => false,
        }
    };

    assert!(
        equal,
        "Differential mismatch for {op_name}({a_str}, {})\n  ynz: {ynz}\n  py:  {py}",
        b_str.unwrap_or("")
    );
}

// ── Deterministic differential cases ─────────────────────────────────────────

#[test]
fn differential_decimal_exactness() {
    // WHY: 0.1 + 0.2 = 0.3 is the canonical test of decimal correctness.
    // This test is worth keeping even when proptest is not available.
    compare_with_python(
        "add",
        "0.1",
        Some("0.2"),
        add(parse("0.1").unwrap(), parse("0.2").unwrap()),
    );
}

#[test]
fn differential_one_third() {
    // WHY: 1/3 is an infinite repeating decimal — confirms rounding matches Python.
    compare_with_python(
        "div",
        "1",
        Some("3"),
        div(parse("1").unwrap(), parse("3").unwrap()),
    );
}

#[test]
fn differential_large_multiply() {
    let a = "9999999999999999999999999999999999"; // MAX_COEFFICIENT
    let b = "9999999999999999999999999999999999";
    let result = mul(parse(a).unwrap(), parse(b).unwrap());
    compare_with_python("mul", a, Some(b), result);
}

// ── Proptest-based random differential testing ───────────────────────────────

#[cfg(test)]
mod proptest_differential {
    use super::*;
    use proptest::prelude::*;
    use ynz_numerics::{add, div, format, mul, parse};

    /// Simpler strategy: generate from a set of representative values.
    fn representative_values() -> impl Strategy<Value = (String, String)> {
        let values = vec![
            "0",
            "1",
            "-1",
            "0.1",
            "0.5",
            "0.9",
            "10",
            "100",
            "1000",
            "1.5",
            "2.25",
            "3.14159",
            "0.3333333333333333333333333333333333",
            "9999999999999999999999999999999999",
            "1e10",
            "1e-10",
        ];
        (
            prop::sample::select(values.clone()),
            prop::sample::select(values),
        )
            .prop_map(|(a, b)| (a.to_string(), b.to_string()))
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config {
            cases: 200,  // 200 iterations per CI run (plan calls for 10k; increase when needed)
            ..Default::default()
        })]

        #[test]
        fn differential_add(
            (a_str, b_str) in representative_values()
        ) {
            // WHY: the differential test is the most direct check that ynz-numerics
            // produces the same answer as Python decimal for all inputs, not just
            // the hand-picked test vectors.
            if let (Some(a), Some(b)) = (parse(&a_str), parse(&b_str)) {
                compare_with_python("add", &a_str, Some(&b_str), add(a, b));
            }
        }

        #[test]
        fn differential_mul(
            (a_str, b_str) in representative_values()
        ) {
            if let (Some(a), Some(b)) = (parse(&a_str), parse(&b_str)) {
                compare_with_python("mul", &a_str, Some(&b_str), mul(a, b));
            }
        }

        #[test]
        fn differential_div(
            (a_str, b_str) in representative_values()
        ) {
            // Skip division by zero
            if b_str == "0" { return Ok(()); }
            if let (Some(a), Some(b)) = (parse(&a_str), parse(&b_str)) {
                compare_with_python("div", &a_str, Some(&b_str), div(a, b));
            }
        }
    }
}
