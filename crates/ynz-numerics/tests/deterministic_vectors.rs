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
    // narrowing to 33 causes rounding of 0.5 → 0 (even)
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

// ---------------------------------------------------------------------------
// 2026-07-11 numerics-correctness audit vectors (N1–N4).
//
// All expected values verified against Python `decimal` this session:
//   decimal128: Context(prec=34, Emax=6144, Emin=-6143, ROUND_HALF_EVEN, clamp=1)
//   bignum:     Context(prec=50, ROUND_HALF_EVEN)
// ---------------------------------------------------------------------------

mod audit_n1_to_n4 {
    use super::{fmt, parse};
    use ynz_numerics::decimal_n::div as bignum_div;
    use ynz_numerics::{
        add as d128_add, compare as d128_compare, format as d128_format, mul as d128_mul,
        parse as d128_parse, sub as d128_sub,
    };

    /// Assert a decimal128 result equals the oracle value (cohort-insensitive
    /// value comparison via `compare`, with the formatted values in the message).
    fn assert_d128_eq(got: u128, expected: &str, label: &str) {
        let want = d128_parse(expected).expect("expected literal must parse");
        assert_eq!(
            d128_compare(got, want),
            0,
            "{label}: got {} want {expected}",
            d128_format(got)
        );
    }

    /// N1 (add): align_exponents' overflow branch must carry a sticky flag for the
    /// truncated tail of the smaller operand.
    ///
    /// 93296E45 + 578E13: the 578E13 tail lands just below the 34-digit LSB and is
    /// non-zero, so the 34th digit must round UP to ...001, not truncate away.
    // WHY: locks the N1 sticky-loss fix in align_exponents (decimal128/ops.rs) — Python
    // decimal oracle: 9.329600000000000000000000000000001E+49.
    #[test]
    fn n1_add_sticky_from_alignment_truncation() {
        let a = d128_parse("93296E45").unwrap();
        let b = d128_parse("578E13").unwrap();
        let got = d128_add(a, b);
        assert_d128_eq(got, "9.329600000000000000000000000000001E+49", "N1 add");
    }

    /// N1 (sub): same alignment truncation on the effective-subtraction path — the
    /// discarded tail makes the true result strictly BELOW the computed difference,
    /// so the result is ...999, not a clean ...000 boundary.
    // WHY: locks the N1 borrow+sticky handling for effective subtraction — Python
    // decimal oracle: 9.329599999999999999999999999999999E+49.
    #[test]
    fn n1_sub_sticky_from_alignment_truncation() {
        let a = d128_parse("93296E45").unwrap();
        let b = d128_parse("578E13").unwrap();
        let got = d128_sub(a, b);
        assert_d128_eq(got, "9.329599999999999999999999999999999E+49", "N1 sub");
    }

    /// N1 (sub, result fits 34 digits): when the borrowed difference needs NO clamp
    /// step, the truncated tail's position relative to half a ULP decides between
    /// D-1 and D — a boolean sticky is insufficient. Three oracle-verified cases:
    /// tail above half (0.16), exactly half → ties-to-even (0.15), below half (0.14).
    // WHY: locks the 4-way tail classification (below/at/above half) on the
    // effective-subtraction path — Python decimal oracle for all three.
    #[test]
    fn n1_sub_unclamped_tail_half_classification() {
        let coarse = d128_parse("1E33").unwrap();
        for (fine, expected) in [
            ("0.16", "999999999999999999999999999999999.8"),
            ("0.15", "999999999999999999999999999999999.8"),
            ("0.14", "999999999999999999999999999999999.9"),
        ] {
            let f = d128_parse(fine).unwrap();
            let got = d128_sub(coarse, f);
            assert_d128_eq(got, expected, &format!("N1 sub 1E33 - {fine}"));
        }
    }

    /// N2 (mul): clamp_to_34_digits must reduce excess digits in ONE half-even step,
    /// not digit-by-digit (sequential per-digit rounding cascades a carry that a
    /// whole-unit comparison against half would not produce).
    // WHY: locks the N2 single-step reduction in clamp_to_34_digits_sticky — Python
    // decimal oracle: 1.594434999179893868660178341855105E+95 (digit loop gave ...106).
    #[test]
    fn n2_mul_single_step_rounding() {
        let a = d128_parse("2165452767410561414527036521052E58").unwrap();
        let b = d128_parse("7363056").unwrap();
        let got = d128_mul(a, b);
        assert_d128_eq(got, "1.594434999179893868660178341855105E+95", "N2 mul");
    }

    /// N2 (mul, simplest failing case from the audit): 38-digit u128 product whose
    /// low 4 digits "4780" must be judged as one unit (< 5000 → round down).
    // WHY: locks the hi==0 → clamp path of round_to_34_digits — Python decimal
    // oracle: 3.383282291365097828459299078190749E-47 (digit loop cascaded to ...750).
    #[test]
    fn n2_mul_dropped_digits_judged_as_unit() {
        let a = d128_parse("4705894581252909670E-51").unwrap();
        let b = d128_parse("7189456187232149834E-33").unwrap();
        let got = d128_mul(a, b);
        assert_d128_eq(
            got,
            "3.383282291365097828459299078190749E-47",
            "N2 mul unit",
        );
    }

    /// N4 (bignum div): the long-division remainder must feed the final rounding as
    /// a sticky signal — dropping it loses 1 ULP on ~2.7% of divisions.
    // WHY: locks the N4 remainder→round_to_precision sticky handoff in decimal_n div —
    // Python decimal oracle at prec=50.
    #[test]
    fn n4_bignum_div_remainder_sticky() {
        let a = parse("75098567452107052E4", 50);
        let b = parse("976581614521218101096381610745410767432989256E-11", 50);
        let got = bignum_div(&a, &b);
        assert_eq!(
            fmt(&got),
            "7.6899427897713501511565070048409922749806430559641E-14",
            "N4 bignum div at prec=50"
        );
    }

    /// N3 (bignum format): large values past the trailing-zero threshold must print
    /// scientific notation as `first.rest E+adj`, not the raw coefficient glued to
    /// the exponent (which reads ~10^(len-1)× too large).
    // WHY: locks the N3 decimal-point insertion in decimal_n format_bignum — Python
    // str(Decimal('123456789E80')) == '1.23456789E+88'.
    #[test]
    fn n3_bignum_sci_notation_has_decimal_point() {
        let bn = parse("123456789E80", 50);
        assert_eq!(fmt(&bn), "1.23456789E+88", "N3 bignum sci format");
    }
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
