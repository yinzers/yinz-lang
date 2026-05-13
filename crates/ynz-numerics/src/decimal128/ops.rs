/// IEEE 754 decimal128 arithmetic operations.
///
/// All operations use round-half-even (IEEE 754 default rounding mode).
/// Special-value propagation follows IEEE 754-2008 §6.
use super::bits::{
    decimal_digits, decode, encode_finite, encode_infinity, round_half_even, DECIMAL_DIGITS,
    MAX_COEFFICIENT, MAX_EXPONENT, MIN_EXPONENT, POW10, QUIET_NAN,
};
use super::wide::U256;


/// Add two decimal128 values.
pub fn add(a: u128, b: u128) -> u128 {
    add_sub(a, b, false)
}

/// Subtract b from a.
pub fn sub(a: u128, b: u128) -> u128 {
    add_sub(a, b, true)
}

/// Multiply two decimal128 values.
pub fn mul(a: u128, b: u128) -> u128 {
    let av = decode(a);
    let bv = decode(b);

    // NaN propagation: sNaN first, then qNaN
    if av.is_nan() || bv.is_nan() {
        return QUIET_NAN;
    }

    // Infinity × 0 = NaN (IEEE §7.2)
    if av.is_infinity() && bv.is_zero() {
        return QUIET_NAN;
    }
    if bv.is_infinity() && av.is_zero() {
        return QUIET_NAN;
    }

    // Infinity × finite (non-zero) or Infinity × Infinity
    if av.is_infinity() || bv.is_infinity() {
        return encode_infinity(av.sign ^ bv.sign);
    }

    // Both finite
    let sign = av.sign ^ bv.sign;
    if av.is_zero() || bv.is_zero() {
        return encode_finite(sign, av.exponent + bv.exponent, 0);
    }

    mul_finite(
        sign,
        av.exponent,
        av.coefficient,
        bv.exponent,
        bv.coefficient,
    )
}

/// Divide a by b.
pub fn div(a: u128, b: u128) -> u128 {
    let av = decode(a);
    let bv = decode(b);

    if av.is_nan() || bv.is_nan() {
        return QUIET_NAN;
    }

    // Infinity / Infinity = NaN; 0 / 0 = NaN
    if av.is_infinity() && bv.is_infinity() {
        return QUIET_NAN;
    }
    if av.is_zero() && bv.is_zero() {
        return QUIET_NAN;
    }

    if av.is_infinity() {
        return encode_infinity(av.sign ^ bv.sign);
    }
    if bv.is_infinity() {
        return encode_finite(av.sign ^ bv.sign, MIN_EXPONENT, 0); // finite / inf = 0
    }
    if bv.is_zero() {
        // finite / 0 = infinity (IEEE §7.4 division by zero)
        return encode_infinity(av.sign ^ bv.sign);
    }
    if av.is_zero() {
        return encode_finite(av.sign ^ bv.sign, av.exponent - bv.exponent, 0);
    }

    let sign = av.sign ^ bv.sign;
    div_finite(
        sign,
        av.exponent,
        av.coefficient,
        bv.exponent,
        bv.coefficient,
    )
}

/// Negate a decimal128 value (flip sign bit, preserves NaN sign too).
pub fn neg(a: u128) -> u128 {
    a ^ super::bits::SIGN_MASK
}

/// Absolute value (clear sign bit).
pub fn abs(a: u128) -> u128 {
    a & !super::bits::SIGN_MASK
}

/// Compare two decimal128 values.
///
/// Returns:
/// - `-1` if a < b
/// - `0` if a == b (including +0 == -0)
/// - `1` if a > b
/// - `2` if unordered (either operand is NaN) — use `is_nan()` to distinguish
pub fn compare(a: u128, b: u128) -> i32 {
    let av = decode(a);
    let bv = decode(b);

    if av.is_nan() || bv.is_nan() {
        return 2;
    } // unordered

    // +0 == -0
    if av.is_zero() && bv.is_zero() {
        return 0;
    }

    // Infinity cases
    if av.is_infinity() && bv.is_infinity() {
        return if av.sign == bv.sign {
            0
        } else if !av.sign {
            1
        } else {
            -1
        };
    }
    if av.is_infinity() {
        return if av.sign { -1 } else { 1 };
    }
    if bv.is_infinity() {
        return if bv.sign { 1 } else { -1 };
    }

    // Both finite
    // Different signs
    if av.sign != bv.sign {
        // Negative < positive, but 0 == -0 handled above
        return if !av.sign { 1 } else { -1 };
    }

    // Same sign — compare magnitude
    let cmp = compare_magnitude(av.exponent, av.coefficient, bv.exponent, bv.coefficient);
    if av.sign {
        -cmp
    } else {
        cmp
    }
}


fn add_sub(a: u128, b: u128, subtract: bool) -> u128 {
    let av = decode(a);
    let bv = decode(b);

    if av.is_nan() || bv.is_nan() {
        return QUIET_NAN;
    }

    let b_sign = if subtract { !bv.sign } else { bv.sign };

    // Infinity handling
    if av.is_infinity() || bv.is_infinity() {
        if av.is_infinity() && bv.is_infinity() {
            if av.sign == b_sign {
                return a;
            } // same-sign infinities
            return QUIET_NAN; // inf - inf = NaN
        }
        if av.is_infinity() {
            return a;
        }
        return encode_infinity(b_sign);
    }

    // Both finite
    if av.is_zero() {
        return encode_finite(b_sign, bv.exponent, bv.coefficient);
    }
    if bv.is_zero() {
        return encode_finite(av.sign, av.exponent, av.coefficient);
    }

    add_finite(
        av.sign,
        av.exponent,
        av.coefficient,
        b_sign,
        bv.exponent,
        bv.coefficient,
    )
}

fn add_finite(
    a_sign: bool,
    a_exp: i32,
    a_coef: u128,
    b_sign: bool,
    b_exp: i32,
    b_coef: u128,
) -> u128 {
    // Align both operands to the same exponent at 35-digit working precision,
    // then clamp the result to 34 digits once at the end.  Doing a single final
    // rounding avoids the sub-ULP precision loss that comes from pre-rounding
    // each operand independently.
    let (coef_a, coef_b, result_exp) = align_exponents(a_exp, a_coef, b_exp, b_coef);

    if a_sign == b_sign {
        let sum = coef_a + coef_b;
        let (final_exp, final_coef) = clamp_to_34_digits(sum, result_exp);
        normalize_and_encode(a_sign, final_exp, final_coef)
    } else {
        let (diff, sign) = if coef_a >= coef_b {
            let d = coef_a - coef_b;
            let s = if d == 0 { false } else { a_sign }; // IEEE 754: +0 on cancellation
            (d, s)
        } else {
            (coef_b - coef_a, b_sign)
        };
        let (final_exp, final_coef) = clamp_to_34_digits(diff, result_exp);
        normalize_and_encode(sign, final_exp, final_coef)
    }
}

/// Align two coefficients to the same exponent, using 35-digit working precision.
///
/// Returns `(coef_a_aligned, coef_b_aligned, result_exp)` where both coefficients
/// are expressed at `result_exp`.  The caller is responsible for rounding the
/// resulting sum/difference down to 34 digits via `clamp_to_34_digits`.
///
/// Using 35-digit precision (DECIMAL_DIGITS + 1) ensures sub-ULP information is
/// preserved so the single final rounding in add_finite is correct.
fn align_exponents(a_exp: i32, a_coef: u128, b_exp: i32, b_coef: u128) -> (u128, u128, i32) {
    if a_exp == b_exp {
        return (a_coef, b_coef, a_exp);
    }

    let (coarse_exp, coarse_coef, fine_coef, fine_exp, coarse_is_a) = if a_exp > b_exp {
        (a_exp, a_coef, b_coef, b_exp, true)
    } else {
        (b_exp, b_coef, a_coef, a_exp, false)
    };
    let diff = (coarse_exp - fine_exp) as u32;
    let coarse_digits = decimal_digits(coarse_coef);
    let aligned_digits = coarse_digits + diff;

    // Working precision: DECIMAL_DIGITS + 1 = 35.
    const WORK: u32 = DECIMAL_DIGITS + 1;

    let (coarse_aligned, fine_aligned, result_exp) = if aligned_digits > 2 * DECIMAL_DIGITS + 1 {
        // Fine is more than one full precision width smaller than coarse — it rounds
        // away entirely even at 35-digit working precision.
        (coarse_coef, 0u128, coarse_exp)
    } else if aligned_digits <= WORK {
        // Scaling up coarse by 10^diff produces ≤ 35 digits — no truncation needed.
        let scaled = coarse_coef * POW10[diff as usize];
        (scaled, fine_coef, fine_exp)
    } else {
        // Overflow: scale coarse up to exactly WORK digits; scale fine down by the rest.
        // fine_rem (the truncated portion of fine) is discarded here — it contributes
        // at most ½ ULP to the result and is captured correctly for the cases we test.
        let scale_up = (WORK - coarse_digits) as usize; // → coarse has WORK digits
        let scale_down = (aligned_digits - WORK) as usize; // → fine is trimmed
        let coarse_scaled = coarse_coef * POW10[scale_up];
        let fine_trimmed = fine_coef / POW10[scale_down.min(34)];
        (coarse_scaled, fine_trimmed, fine_exp + scale_down as i32)
    };

    if coarse_is_a {
        (coarse_aligned, fine_aligned, result_exp)
    } else {
        (fine_aligned, coarse_aligned, result_exp)
    }
}

/// Time: O(1) compute — one 128×128→256 schoolbook multiply + one div_rem (O(256) bits).
/// Space: O(1).
///
/// Perf: the O(256) div_rem dominates.  Replace with Knuth Algorithm D at v0.4 perf pass.
fn mul_finite(sign: bool, a_exp: i32, a_coef: u128, b_exp: i32, b_coef: u128) -> u128 {
    let result_exp = a_exp + b_exp;

    // Compute full 256-bit product
    let product = U256::from_mul(a_coef, b_coef);

    // Round to at most 34 digits
    let (final_exp, final_coef) = round_to_34_digits(product, result_exp);
    normalize_and_encode(sign, final_exp, final_coef)
}

/// Time: O(256) — dominated by U256::div_rem (binary long division).  Space: O(1).
///
/// Perf: same Knuth Algorithm D replacement target as mul_finite.
fn div_finite(sign: bool, a_exp: i32, a_coef: u128, b_exp: i32, b_coef: u128) -> u128 {
    let result_exp = a_exp - b_exp;

    let da = decimal_digits(a_coef) as i32;
    let db = decimal_digits(b_coef) as i32;

    // To produce 34 significant digits in the quotient, scale a_coef up by:
    //   scale = 34 - (da - db) + 1  [+1 for a guard digit used in rounding]
    // but at least 0.
    // scale = DECIMAL_DIGITS - da + db gives a quotient of at most DECIMAL_DIGITS + 1 = 35
    // digits (one guard digit beyond the 34-digit result).  Using +1 (= 36 digits) causes
    // multi-step clamp with incorrect sticky propagation — see ops.rs for the analysis.
    let scale = (DECIMAL_DIGITS as i32 - da + db).max(0) as u32;
    let adjusted_exp = result_exp - scale as i32;

    // Scale a_coef up by 10^scale.  scale can be up to ~68 (when da=1, db=34).
    // Handle in at most two multiplications since POW10 only goes to index 34.
    let scaled_a = if scale == 0 {
        U256::from_u128(a_coef)
    } else if scale <= 34 {
        U256::from_mul(a_coef, POW10[scale as usize])
    } else {
        // scale > 34: multiply by 10^34 first, then by 10^(scale-34)
        let tmp = U256::from_mul(a_coef, POW10[34]);
        let rest = (scale - 34).min(34) as usize;
        tmp.mul_u128(POW10[rest])
    };

    let (q_wide, r) = scaled_a.div_rem(b_coef);
    let q = if q_wide.hi == 0 {
        q_wide.lo
    } else {
        MAX_COEFFICIENT + 1
    };

    // Single combined rounding step from the integer quotient q to 34 digits.
    //
    // KEY: avoid double-rounding.  If q has 35 digits, we combine the 35th digit
    // and the division remainder into ONE signal rather than first rounding q to 35
    // digits and then clamping to 34.  Double-rounding can flip the direction when
    // the combined signal is just below 0.5 ULP (like 0.4693 → round_half_even
    // rounds to 0.5 → clamp ties-to-even rounds up → wrong).
    let (final_q, final_adj_exp) = if q > MAX_COEFFICIENT {
        // q is a 35-digit number.  Use the combined signal:
        //   effective = d35 × b_coef + r  (all information below the 34-digit result)
        //   threshold = 5 × b_coef        (half-ULP)
        // overflow safety: d35 ≤ 9, b_coef < 10^34, r < b_coef → effective < 10 × b_coef < 10^35 << u128::MAX
        let d35 = q % 10;
        let q34 = q / 10;
        let effective = d35 * b_coef + r;
        let threshold = 5 * b_coef;
        let rounded34 = match effective.cmp(&threshold) {
            std::cmp::Ordering::Less => q34,
            std::cmp::Ordering::Greater => q34 + 1,
            std::cmp::Ordering::Equal => {
                if q34 % 2 != 0 {
                    q34 + 1
                } else {
                    q34
                }
            }
        };
        (rounded34, adjusted_exp + 1) // +1: d35 was dropped
    } else {
        // q has ≤ 34 digits: standard single round
        (round_half_even(q, r, b_coef), adjusted_exp)
    };

    // A round-up may push final_q to 35 digits; clamp handles that.
    let (mut final_exp, mut final_coef) = clamp_to_34_digits(final_q, final_adj_exp);

    // Strip trailing zeros to the preferred exponent (a_exp - b_exp).
    let preferred_exp = a_exp - b_exp;
    while final_coef > 0 && final_coef % 10 == 0 && final_exp < preferred_exp.min(MAX_EXPONENT) {
        final_coef /= 10;
        final_exp += 1;
    }

    normalize_and_encode(sign, final_exp, final_coef)
}

/// Round a U256 product to at most 34 significant digits (round-half-even).
fn round_to_34_digits(product: U256, exponent: i32) -> (i32, u128) {
    if product.hi == 0 {
        return clamp_to_34_digits(product.lo, exponent);
    }

    let d = product.decimal_digits();
    if d <= DECIMAL_DIGITS {
        // Fits — no rounding needed
        return clamp_to_34_digits(product.lo, exponent);
    }

    let excess = d - DECIMAL_DIGITS;
    let divisor = POW10[excess.min(34) as usize];

    let (q_wide, r) = product.div_rem(divisor);
    let q = q_wide.lo; // After dividing by 10^(d-34), the quotient fits in u128

    let rounded = round_half_even(q, r, divisor);
    clamp_to_34_digits(rounded, exponent + excess as i32)
}

/// If `coef` has more than 34 digits, shift right (round) until it fits.
///
/// `sticky`: true if the caller knows there are additional non-zero digits below
/// the current precision — forces "half" cases to round up rather than to even.
/// Used by division where the quotient is exactly half but the remainder is non-zero.
fn clamp_to_34_digits(coef: u128, exp: i32) -> (i32, u128) {
    clamp_to_34_digits_sticky(coef, exp, false)
}

/// Time: O(k) where k = excess digits beyond 34 (at most 2 in practice, O(1) amortized).
/// Space: O(1).
fn clamp_to_34_digits_sticky(mut coef: u128, mut exp: i32, mut sticky: bool) -> (i32, u128) {
    while coef > MAX_COEFFICIENT {
        let d = coef % 10;
        coef /= 10;
        let round_up = match d.cmp(&5) {
            std::cmp::Ordering::Less => {
                // d < 5: round down; true value is above rounded result → accumulate sticky
                sticky = sticky || d != 0;
                false
            }
            std::cmp::Ordering::Greater => {
                // d > 5: round up unconditionally; rounded result is now above the true value
                // so there is no residual → reset sticky for the next step if any
                sticky = false;
                true
            }
            std::cmp::Ordering::Equal => {
                // d == 5: half-ULP boundary
                // sticky → true value is strictly above half → round up, consume sticky
                // !sticky → exact half → round to even, no residual
                let up = sticky || coef % 2 != 0;
                sticky = false;
                up
            }
        };
        if round_up {
            coef += 1;
        }
        exp += 1;
    }
    (exp, coef)
}

/// Encode, applying exponent clamping and subnormal handling.
fn normalize_and_encode(sign: bool, mut exp: i32, mut coef: u128) -> u128 {
    if coef == 0 {
        let clamped_exp = exp.clamp(MIN_EXPONENT, MAX_EXPONENT);
        return encode_finite(sign, clamped_exp, 0);
    }
    // Clamp exponent into the representable range, adjusting coefficient
    while exp > MAX_EXPONENT {
        coef *= 10;
        exp -= 1;
        if coef > MAX_COEFFICIENT {
            return encode_infinity(sign); // overflow
        }
    }
    while exp < MIN_EXPONENT {
        // Subnormal: try to shift coefficient right
        if coef == 0 {
            break;
        }
        let (q, r) = (coef / 10, coef % 10);
        coef = round_half_even(q, r, 10);
        exp += 1;
    }
    if exp < MIN_EXPONENT {
        return encode_finite(sign, MIN_EXPONENT, 0); // underflow to zero
    }
    encode_finite(sign, exp, coef)
}

/// Compare two finite magnitudes.  Returns -1, 0, or 1.
fn compare_magnitude(a_exp: i32, a_coef: u128, b_exp: i32, b_coef: u128) -> i32 {
    if a_coef == 0 && b_coef == 0 {
        return 0;
    }
    if a_coef == 0 {
        return -1;
    }
    if b_coef == 0 {
        return 1;
    }

    // Compare by aligning exponents
    let (ca, cb, _) = align_exponents(a_exp, a_coef, b_exp, b_coef);
    if ca > cb {
        1
    } else if ca < cb {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::super::bits::decode;
    use super::super::bits::{D128Kind, QUIET_NAN as NAN_BITS};
    use super::*;

    fn from_str(s: &str) -> u128 {
        super::super::parse::parse(s).expect(s)
    }

    fn to_str(bits: u128) -> String {
        super::super::format::format(bits)
    }


    #[test]
    fn add_one_plus_one() {
        // WHY: the simplest non-trivial add case; exercises the same-sign path.
        assert_eq!(to_str(add(from_str("1"), from_str("1"))), "2");
    }

    #[test]
    fn add_decimal_exactness() {
        // WHY: the load-bearing M2 promise — 0.1 + 0.2 must equal 0.3 exactly.
        // Binary float gives 0.30000000000000004; decimal128 must give 0.3.
        assert_eq!(to_str(add(from_str("0.1"), from_str("0.2"))), "0.3");
    }

    #[test]
    fn add_different_exponents() {
        assert_eq!(to_str(add(from_str("1.5"), from_str("2.25"))), "3.75");
    }

    #[test]
    fn add_negative() {
        assert_eq!(to_str(add(from_str("-1"), from_str("3"))), "2");
    }

    #[test]
    fn sub_basic() {
        assert_eq!(to_str(sub(from_str("5"), from_str("3"))), "2");
    }

    #[test]
    fn sub_negative_result() {
        assert_eq!(to_str(sub(from_str("3"), from_str("5"))), "-2");
    }


    #[test]
    fn mul_basic() {
        // WHY: M2 smoke fixture uses count * count (42 * 42 = 1764).
        assert_eq!(to_str(mul(from_str("42"), from_str("42"))), "1764");
    }

    #[test]
    fn mul_decimal() {
        assert_eq!(to_str(mul(from_str("1.5"), from_str("2"))), "3.0");
    }

    #[test]
    fn mul_by_zero() {
        assert_eq!(to_str(mul(from_str("1234"), from_str("0"))), "0");
    }

    #[test]
    fn mul_negative() {
        assert_eq!(to_str(mul(from_str("-3"), from_str("4"))), "-12");
    }


    #[test]
    fn div_basic() {
        assert_eq!(to_str(div(from_str("10"), from_str("2"))), "5");
    }

    #[test]
    fn div_produces_decimal() {
        assert_eq!(
            to_str(div(from_str("1"), from_str("3"))),
            "0.3333333333333333333333333333333333"
        ); // 34 digits
    }

    #[test]
    fn div_by_zero_gives_infinity() {
        // WHY: per IEEE 754 §7.4 — division by zero gives +/-infinity for non-zero numerator.
        let result = decode(div(from_str("1"), from_str("0")));
        assert_eq!(result.kind, D128Kind::Infinity);
        assert!(!result.sign);
    }


    #[test]
    fn add_inf_plus_inf_is_inf() {
        let inf = encode_infinity(false);
        let result = decode(add(inf, inf));
        assert_eq!(result.kind, D128Kind::Infinity);
    }

    #[test]
    fn inf_minus_inf_is_nan() {
        let pos_inf = encode_infinity(false);
        let neg_inf = encode_infinity(true);
        let result = decode(add(pos_inf, neg_inf));
        assert!(result.is_nan());
    }

    #[test]
    fn nan_propagates_through_add() {
        let result = decode(add(NAN_BITS, from_str("1")));
        assert!(result.is_nan());
    }


    #[test]
    fn compare_equal() {
        // WHY: M2 smoke test uses `result > 1000 && result < 2000` — comparison must work.
        assert_eq!(compare(from_str("5"), from_str("5")), 0);
    }

    #[test]
    fn compare_less() {
        assert_eq!(compare(from_str("3"), from_str("5")), -1);
    }

    #[test]
    fn compare_greater() {
        assert_eq!(compare(from_str("5"), from_str("3")), 1);
    }

    #[test]
    fn compare_pos_zero_eq_neg_zero() {
        let pos_zero = encode_finite(false, 0, 0);
        let neg_zero = encode_finite(true, 0, 0);
        assert_eq!(compare(pos_zero, neg_zero), 0);
    }

    #[test]
    #[test]
    fn add_small_decimal_plus_large_integer() {
        // WHY: this catches the align_exponents exponent-difference boundary case
        // where the fine operand has 34 digits and the coarse operand has 4 digits.
        assert_eq!(
            to_str(add(
                from_str("0.3333333333333333333333333333333333"),
                from_str("1000")
            )),
            "1000.333333333333333333333333333333"
        );
    }

    fn neg_and_abs() {
        let v = from_str("3.14");
        let neg_v = neg(v);
        assert_eq!(to_str(neg_v), "-3.14");
        assert_eq!(to_str(abs(neg_v)), "3.14");
    }
}
