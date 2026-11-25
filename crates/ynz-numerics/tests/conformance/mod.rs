/// IBM Hursley decimal128 conformance test harness.
///
/// Loads `.decTest` files from `tests/conformance/` if present (they are not checked
/// in — see `CORPUS.sha256` for the pinned version).  Falls back to embedded critical
/// test cases when the corpus files are absent.
///
/// The `.decTest` format per operation:
///
///   `id op operand1 [operand2] -> result [conditions]`
///
/// Only the M2 subset is exercised:
///   dqAdd, dqSubtract, dqMultiply, dqDivide, dqCompare, dqAbs, dqMinus, dqPlus
use ynz_numerics::{abs, add, compare, div, format, mul, neg, parse, sub};

// WHY: conformance must not silently pass when the IBM corpus is absent.
// These vectors cover the load-bearing invariants: associativity-breaking edge
// cases, NaN propagation, and the 0.1+0.2=0.3 exactness guarantee.

/// How a decimal128 result is compared with the expected text.
#[derive(Clone, Copy)]
enum ResultMatch {
    /// The formatted result must be the expected text, so a wrong scale ("3" for "3.0") fails.
    ExactText,
    /// The result may be the same value written at another scale (a different cohort member).
    SameValueAtAnyScale,
}

/// Applies one decimal128 operation to its operands and compares the result with the expected
/// text as `result_match` says.
fn run_vector(
    op: &str,
    a: &str,
    b: Option<&str>,
    expected: &str,
    result_match: ResultMatch,
) -> Result<(), String> {
    let a_bits = parse(a).ok_or_else(|| format!("failed to parse a={a}"))?;
    let b_bits = match b {
        Some(text) => Some(parse(text).ok_or_else(|| format!("failed to parse b={text}"))?),
        None => None,
    };
    let operand_b = || b_bits.ok_or_else(|| format!("{op} needs a second operand"));
    let result_bits = match op {
        "add" => add(a_bits, operand_b()?),
        "sub" => sub(a_bits, operand_b()?),
        "mul" => mul(a_bits, operand_b()?),
        "div" => div(a_bits, operand_b()?),
        "compare" => {
            let ordering = match compare(a_bits, operand_b()?) {
                -1 => "-1",
                0 => "0",
                1 => "1",
                _ => "NaN",
            };
            return if ordering == expected {
                Ok(())
            } else {
                Err(format!(
                    "compare({a}, {}) = {ordering}, expected {expected}",
                    b.unwrap_or("")
                ))
            };
        }
        "abs" => abs(a_bits),
        "neg" => neg(a_bits),
        _ => return Err(format!("unknown op {op}")),
    };

    let actual = format(result_bits);
    let same_value = matches!(result_match, ResultMatch::SameValueAtAnyScale)
        && parse(&actual)
            .and_then(|actual_bits| {
                parse(expected).map(|expected_bits| compare(actual_bits, expected_bits) == 0)
            })
            .unwrap_or(false);
    if actual == expected || same_value {
        Ok(())
    } else {
        Err(format!(
            "{op}({a}{}) = {actual}, expected {expected}",
            b.map(|second| format!(", {second}")).unwrap_or_default()
        ))
    }
}

/// One generated test per vector: `name: op, a, b => expected;` (`b` is `None` for unary ops),
/// compared as the leading `ResultMatch` says.
/// The Hursley-style vectors, the special-value cases and the audit vectors (N1, N2) share this
/// harness because each is one operation applied to parsed text; the audit vectors' expected
/// values are Python `decimal` oracle values (Context prec=34, Emax=6144, ROUND_HALF_EVEN, clamp=1).
macro_rules! decimal128_value_cases {
    ($result_match:expr; $($case:ident: $op:literal, $a:literal, $b:expr => $expected:literal;)+) => {
        $(
            // bug: a decimal128 add/sub/mul/div/compare/neg/abs returns the wrong value (digits, rounding, sign, zero, Infinity or NaN) | tier: High
            #[test]
            fn $case() {
                run_vector($op, $a, $b, $expected, $result_match)
                    .unwrap_or_else(|failure| panic!("{failure}"));
            }
        )+
    };
}

decimal128_value_cases! {
    ResultMatch::ExactText;
    // add: same sign, decimal exactness (0.1 + 0.2 must be 0.3, not 0.30000000000000004), alignment, cancellation
    add_same_sign_integers: "add", "1", Some("1") => "2";
    add_decimal_exactness: "add", "0.1", Some("0.2") => "0.3";
    add_opposite_signs_cancel_to_zero: "add", "-1", Some("1") => "0";
    add_opposite_signs_cancel_to_zero_at_a_positive_exponent: "add", "1e10", Some("-1e10") => "0";
    add_opposite_signs_with_a_larger_positive_operand: "add", "-1", Some("3") => "2";
    add_operands_with_different_exponents: "add", "1.5", Some("2.25") => "3.75";
    // the fine operand has 34 digits and the coarse one 4: alignment must not drop the fine digits
    add_34_digit_fraction_to_a_larger_integer: "add", "0.3333333333333333333333333333333333", Some("1000") => "1000.333333333333333333333333333333";
    add_infinity_plus_infinity: "add", "Infinity", Some("Infinity") => "Infinity";
    add_infinity_plus_negative_infinity_is_nan: "add", "Infinity", Some("-Infinity") => "NaN";
    add_propagates_nan: "add", "NaN", Some("1") => "NaN";
    // sub
    sub_positive_result: "sub", "2", Some("1") => "1";
    sub_decimal_exactness: "sub", "0.3", Some("0.2") => "0.1";
    sub_negative_result: "sub", "1", Some("3") => "-2";
    // N1: tail above half a ULP rounds the 34th digit down, exactly half ties to even, below half rounds up
    sub_unclamped_tail_above_half: "sub", "1E33", Some("0.16") => "999999999999999999999999999999999.8";
    sub_unclamped_tail_exactly_half_ties_to_even: "sub", "1E33", Some("0.15") => "999999999999999999999999999999999.8";
    sub_unclamped_tail_below_half: "sub", "1E33", Some("0.14") => "999999999999999999999999999999999.9";
    // mul
    mul_integers: "mul", "42", Some("42") => "1764";
    mul_keeps_the_scaled_exponent: "mul", "1.5", Some("2") => "3.0";
    mul_negative_operand: "mul", "-3", Some("4") => "-12";
    mul_by_zero: "mul", "0", Some("99") => "0";
    // div
    div_exact_quotient: "div", "10", Some("2") => "5";
    div_repeating_quotient_rounds_down_at_34_digits: "div", "1", Some("3") => "0.3333333333333333333333333333333333";
    div_repeating_quotient_rounds_up_at_34_digits: "div", "22", Some("7") => "3.142857142857142857142857142857143";
    // IEEE 754 section 7.4: a non-zero numerator over zero is +/-Infinity
    div_by_zero_is_infinity: "div", "1", Some("0") => "Infinity";
    // compare
    compare_less: "compare", "1", Some("2") => "-1";
    compare_equal: "compare", "2", Some("2") => "0";
    compare_greater: "compare", "3", Some("2") => "1";
    compare_positive_zero_equals_negative_zero: "compare", "0", Some("-0") => "0";
    // neg and abs
    abs_of_a_negative: "abs", "-5", None => "5";
    abs_of_a_positive: "abs", "5", None => "5";
    neg_of_a_positive: "neg", "5", None => "-5";
    neg_of_a_negative: "neg", "-5", None => "5";
}

// The audit oracle values below are the same value as the result at a different scale (the
// result is a different cohort member), which is how these vectors have always been compared.
decimal128_value_cases! {
    ResultMatch::SameValueAtAnyScale;
    // N1: the truncated tail of the smaller operand below the 34-digit LSB must round the 34th digit up
    add_sticky_tail_from_alignment_truncation: "add", "93296E45", Some("578E13") => "9.329600000000000000000000000000001E+49";
    // N2: excess digits are reduced in ONE half-even step, not digit by digit (a digit loop gave ...106)
    mul_excess_digits_rounded_in_one_step: "mul", "2165452767410561414527036521052E58", Some("7363056") => "1.594434999179893868660178341855105E+95";
    // N2: a 38-digit product whose dropped digits "4780" are judged as one unit, < 5000 rounds down (a digit loop gave ...750)
    mul_dropped_digits_judged_as_one_unit: "mul", "4705894581252909670E-51", Some("7189456187232149834E-33") => "3.383282291365097828459299078190749E-47";
    // N1: the discarded tail makes the true result strictly below the computed difference (...999)
    sub_sticky_tail_from_alignment_truncation: "sub", "93296E45", Some("578E13") => "9.329599999999999999999999999999999E+49";
}

/// Run the full IBM Hursley corpus if the files are present in `tests/conformance/`.
///
/// WHY: the embedded vectors above cover the key invariants, but the full corpus
/// exercises hundreds of edge cases that only IBM's test suite enumerates.
/// When `CORPUS.sha256` is present, CI verifies the corpus hash before running.
// bug: a decimal128 operation disagrees with a line of the IBM Hursley corpus (corpus files are not checked in, so this runs only where they are present) | tier: High
#[test]
fn ibm_hursley_corpus_if_present() {
    let corpus_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/conformance");

    let ops = [
        "dqAdd",
        "dqSubtract",
        "dqMultiply",
        "dqDivide",
        "dqCompare",
        "dqAbs",
        "dqMinus",
        "dqPlus",
    ];

    let mut total = 0usize;
    let mut failures = Vec::new();
    let mut skipped = 0usize;

    for op in &ops {
        let path = corpus_dir.join(format!("{op}.decTest"));
        if !path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&path).expect("read corpus file");
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("--") || line.starts_with('#') {
                continue;
            }
            // Directives (version:, precision:, etc.) — skip
            if line.contains(':') && !line.contains("->") {
                continue;
            }
            // Parse test line
            if let Some(result) = run_corpus_line(line, op) {
                total += 1;
                match result {
                    Ok(()) => {}
                    Err(msg) => failures.push(msg),
                }
            } else {
                skipped += 1;
            }
        }
    }

    if total == 0 {
        // No corpus files found — not a failure, just informational
        return;
    }

    println!(
        "IBM Hursley corpus: {total} run, {skipped} skipped, {} failed",
        failures.len()
    );

    if !failures.is_empty() {
        panic!(
            "{} corpus failure(s) (first 10):\n{}",
            failures.len(),
            failures
                .iter()
                .take(10)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

fn run_corpus_line(line: &str, op_name: &str) -> Option<Result<(), String>> {
    // Format: `id opname operand1 [operand2] -> result [flags]`
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }

    let arrow_pos = parts.iter().position(|&p| p == "->")?;
    if arrow_pos < 2 {
        return None;
    }
    let expected_raw = parts.get(arrow_pos + 1)?;

    // Skip special values we don't yet support (e.g., #Infinity, sNaN)
    if expected_raw.starts_with('#') || expected_raw.to_lowercase().contains("snan") {
        return None;
    }

    let operands = &parts[2..arrow_pos];

    if operands.is_empty() {
        return None;
    }

    let parse_or_skip = |s: &str| -> Option<u128> {
        // Skip infinite/NaN operands for ops we haven't verified yet
        if s.to_lowercase().contains("inf") || s.to_lowercase().contains("nan") {
            return None; // will cause the test to be skipped
        }
        parse(s)
    };

    let a_bits = parse_or_skip(operands[0])?;

    let op_fn = op_name.to_lowercase();
    let result_bits = match op_fn.trim_start_matches("dq") {
        "add" | "plus" if operands.len() >= 2 => {
            let b = parse_or_skip(operands[1])?;
            add(a_bits, b)
        }
        "subtract" | "minus" if op_fn.contains("minus") => neg(a_bits),
        "subtract" if operands.len() >= 2 => {
            let b = parse_or_skip(operands[1])?;
            sub(a_bits, b)
        }
        "multiply" if operands.len() >= 2 => {
            let b = parse_or_skip(operands[1])?;
            mul(a_bits, b)
        }
        "divide" if operands.len() >= 2 => {
            let b = parse_or_skip(operands[1])?;
            div(a_bits, b)
        }
        "abs" => abs(a_bits),
        "minus" => neg(a_bits),
        "plus" => a_bits,
        "compare" if operands.len() >= 2 => {
            let b = parse_or_skip(operands[1])?;
            let c = compare(a_bits, b);
            let s = match c {
                -1 => "-1",
                0 => "0",
                1 => "1",
                _ => "NaN",
            };
            let expected = expected_raw.trim_matches('\'');
            return Some(if s == expected || parse(s) == parse(expected) {
                Ok(())
            } else {
                Err(format!("{line}: compare = {s}, expected {expected}"))
            });
        }
        _ => return None,
    };

    let actual = format(result_bits);
    let expected = expected_raw.trim_matches('\'');

    // Numeric equality: parse both and compare bits if strings differ
    let equal = actual == expected || {
        match (parse(&actual), parse(expected)) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    };

    Some(if equal {
        Ok(())
    } else {
        Err(format!("{line}\n  got: {actual}\n  exp: {expected}"))
    })
}
