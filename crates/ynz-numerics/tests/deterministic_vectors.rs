/// Deterministic test vectors from the M8 plan (14 rows from P6 Step 10),
/// plus the 2026-07-11 numerics-correctness audit vectors (N1–N4).
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
// bug: bignum add of 0.1 and 0.2 is not exactly 0.3 (binary-float rounding leaks in) | tier: High
#[test]
fn row01_point1_plus_point2() {
    let a = parse("0.1", 100);
    let b = parse("0.2", 100);
    let c = add(&a, &b);
    assert_eq!(fmt(&c), "0.3", "0.1 + 0.2 must equal 0.3 at prec=100");
}

/// Row 5: division of repeating fraction at precision 100
// bug: bignum division of a repeating fraction at precision 100 returns too few correct digits | tier: High
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
// bug: bignum add of +0 and -0 returns -0 instead of +0 (IEEE 754) | tier: High
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
// bug: bignum multiply of -0 by 1 loses the sign and returns +0 (IEEE 754) | tier: High
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
// bug: bignum multiply returns a wrong product at precision 100 | tier: High
#[test]
fn basic_mul_at_100() {
    let a = parse("3", 100);
    let b = parse("7", 100);
    let c = mul(&a, &b);
    assert_eq!(fmt(&c), "21", "3 * 7 = 21 at prec=100");
}

// bug: bignum subtract returns a wrong difference at precision 50 | tier: High
#[test]
fn basic_sub_at_50() {
    let a = parse("1", 50);
    let b = parse("0.5", 50);
    let c = sub(&a, &b);
    assert_eq!(fmt(&c), "0.5", "1 - 0.5 = 0.5 at prec=50");
}

// bug: bignum add of a large number and its negation is not exactly zero | tier: High
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

// 2026-07-11 numerics-correctness audit vectors (N3, N4), verified against Python `decimal`
// (bignum: Context(prec=50, ROUND_HALF_EVEN)). The decimal128 audit vectors (N1, N2) are cases
// of `decimal128_value_cases!` in tests/conformance/mod.rs.

/// N4 (bignum div): the long-division remainder must feed the final rounding as
/// a sticky signal — dropping it loses 1 ULP on ~2.7% of divisions.
// bug: bignum division drops the long-division remainder before rounding and returns a result 1 ULP off | tier: High
#[test]
fn n4_bignum_div_remainder_sticky() {
    let a = parse("75098567452107052E4", 50);
    let b = parse("976581614521218101096381610745410767432989256E-11", 50);
    let got = div(&a, &b);
    assert_eq!(
        fmt(&got),
        "7.6899427897713501511565070048409922749806430559641E-14",
        "N4 bignum div at prec=50"
    );
}

/// N3 (bignum format): large values past the trailing-zero threshold must print
/// scientific notation as `first.rest E+adj`, not the raw coefficient glued to
/// the exponent (which reads ~10^(len-1)× too large).
// bug: bignum format prints a large value in scientific notation without the decimal point, ~10^(len-1) times too large | tier: High
#[test]
fn n3_bignum_sci_notation_has_decimal_point() {
    let bn = parse("123456789E80", 50);
    assert_eq!(fmt(&bn), "1.23456789E+88", "N3 bignum sci format");
}

/// `format_bignum(parse_bignum(s))` must re-parse and re-format to the same text.
fn assert_bignum_text_round_trips(text: &str) {
    let formatted = fmt(&parse(text, 100));
    let reformatted = fmt(&parse(&formatted, 100));
    assert_eq!(
        formatted, reformatted,
        "round-trip failed for {text}: {text} → {formatted} → {reformatted}"
    );
}

macro_rules! bignum_round_trip_cases {
    ($($case:ident: $text:literal;)+) => {
        $(
            // bug: bignum format output does not re-parse to the same number, so a printed value cannot be read back | tier: High
            #[test]
            fn $case() {
                assert_bignum_text_round_trips($text);
            }
        )+
    };
}

bignum_round_trip_cases! {
    round_trip_fraction: "0.1";
    round_trip_integer: "42";
    round_trip_integer_with_fraction: "100.001";
}

/// Half-even at an exact tie, one significant digit (Python `Decimal` with ROUND_HALF_EVEN and
/// precision 1): the kept digit moves to the even neighbour, so an odd kept digit rounds up and an
/// even one stays.
macro_rules! half_even_tie_cases {
    ($($case:ident: $text:literal rounds_to $expected:literal;)+) => {
        $(
            // bug: bignum rounds an exact tie (…5 with nothing below) up or down regardless of the kept digit's parity, so results differ from ROUND_HALF_EVEN by 1 ULP | tier: High
            #[test]
            fn $case() {
                assert_eq!(fmt(&parse($text, 1)), $expected);
            }
        )+
    };
}

half_even_tie_cases! {
    half_even_tie_1_5_rounds_up_to_the_even_2: "1.5" rounds_to "2";
    half_even_tie_2_5_rounds_down_to_the_even_2: "2.5" rounds_to "2";
    half_even_tie_3_5_rounds_up_to_the_even_4: "3.5" rounds_to "4";
}
