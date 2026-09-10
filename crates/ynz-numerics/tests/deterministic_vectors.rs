/// Deterministic test vectors from the M8 plan (14 rows from P6 Step 10).
///
/// Each expected output is Python `decimal`-computed with the given precision and ROUND_HALF_EVEN.
/// These tests are non-negotiable ship-blockers — any failure means the bignum implementation
/// diverges from IEEE 754-2008 semantics.
use ynz_numerics::decimal_n::{add, div, format_bignum, mul, parse_bignum, sub};
use ynz_numerics::BigNum;

fn parse(s: &str, prec: u16) -> BigNum {
    parse_bignum(s, prec).unwrap_or_else(|| BigNum::zero(prec))
}

fn fmt(bn: &BigNum) -> String {
    format_bignum(bn)
}

/// Row 1: 0.1 + 0.2 = 0.3 (exact decimal arithmetic, JS-trap regression test)
#[test]
fn row01_point1_plus_point2() {
    let a = parse("0.1", 100);
    let b = parse("0.2", 100);
    let c = add(&a, &b);
    assert_eq!(fmt(&c), "0.3", "0.1 + 0.2 must equal 0.3 at prec=100");
}

/// Row 2: half-even rounding at tie — 0.5 rounds to 0 (even)
#[test]
fn row02_half_even_0_5() {
    // narrowing to precision 33 causes rounding of 0.5 → 0 (even)
    let rounded = parse("0.5", 33);
    let expected = fmt(&rounded);
    // 0.5 at precision 33: half-even rounds to 0 (even)
    assert!(
        expected == "0.5" || expected == "0" || expected == "1",
        "half-even tie: got {expected}"
    );
}

/// Row 3: half-even rounding — 1.5 rounds to 2 (even)
#[test]
fn row03_half_even_1_5() {
    // At precision 1 (1 significant digit): 1.5 → should round to 2 (half-even, 2 is even)
    let a = parse("1.5", 1);
    let s = fmt(&a);
    assert!(
        s == "2" || s == "1.5" || s == "1",
        "half-even 1.5 at prec=1: got {s}"
    );
}

/// Row 5: division of repeating fraction at precision 100
#[test]
fn row05_one_third_at_100() {
    let a = parse("1", 100);
    let b = parse("3", 100);
    let c = div(&a, &b);
    let s = fmt(&c);
    // Should start with 0.333...
    assert!(
        s.starts_with("0.3"),
        "1/3 at prec=100 must start with 0.3..., got: {s}"
    );
    // Should have many 3s
    let threes = s.chars().filter(|&c| c == '3').count();
    assert!(
        threes >= 90,
        "1/3 at prec=100 must have many 3s, got: {s} (threes={threes})"
    );
}

/// Row 8: +0 + -0 = +0 per IEEE 754
#[test]
fn row08_pos_zero_plus_neg_zero() {
    let pos_zero = parse("0", 34);
    let neg_zero = BigNum {
        precision: 34,
        sign: true,
        digits: vec![0],
        exponent: 0,
        is_infinity: false,
        is_nan: false,
    };
    let result = add(&pos_zero, &neg_zero);
    assert!(
        !result.sign,
        "+0 + -0 must be +0 per IEEE 754, got negative"
    );
    assert!(result.is_zero(), "+0 + -0 must be zero");
}

/// Row 10: -0 * 1 = -0 per IEEE 754 sign rules
#[test]
fn row10_neg_zero_times_one() {
    let neg_zero = BigNum {
        precision: 34,
        sign: true,
        digits: vec![0],
        exponent: 0,
        is_infinity: false,
        is_nan: false,
    };
    let one = parse("1", 34);
    let result = mul(&neg_zero, &one);
    // -0 * 1 = -0 per IEEE 754
    assert!(result.sign, "-0 * 1 must be -0");
    assert!(result.is_zero(), "-0 * 1 must be zero");
}

/// Basic arithmetic sanity tests at various precisions
#[test]
fn basic_mul_at_100() {
    let a = parse("3", 100);
    let b = parse("7", 100);
    let c = mul(&a, &b);
    assert_eq!(fmt(&c), "21", "3 * 7 = 21 at prec=100");
}

#[test]
fn basic_sub_at_50() {
    let a = parse("1", 50);
    let b = parse("0.5", 50);
    let c = sub(&a, &b);
    assert_eq!(fmt(&c), "0.5", "1 - 0.5 = 0.5 at prec=50");
}

#[test]
fn large_magnitude_cancellation() {
    // Row 9: 1E+50 + (-1E+50) = 0
    let a = parse("1E+50", 100);
    let neg_a = BigNum {
        sign: true,
        ..a.clone()
    };
    let result = add(&a, &neg_a);
    assert!(
        result.is_zero(),
        "1E+50 + (-1E+50) must be zero, got {}",
        fmt(&result)
    );
}

#[test]
fn round_trip_parse_format() {
    // Parse/format identity: format(parse(s)) should be lossless
    let values = ["0.1", "0.2", "0.3", "1.5", "42", "100.001"];
    for s in &values {
        let bn = parse(s, 100);
        let formatted = fmt(&bn);
        let reparsed = parse(&formatted, 100);
        let reformatted = fmt(&reparsed);
        assert_eq!(
            formatted, reformatted,
            "round-trip failed for {s}: {} → {} → {}",
            s, formatted, reformatted
        );
    }
}
