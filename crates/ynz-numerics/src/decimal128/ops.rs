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
        // Clamp the preferred exponent to [MIN_EXPONENT, MAX_EXPONENT] per IEEE 754-2008.
        // Without clamping, extreme inputs (e.g. av.exponent=MIN_EXPONENT=-6176,
        // bv.exponent=MAX_EXPONENT=6111) produce a difference of -12287, which is
        // out of range and causes a debug-assert panic / garbage bits in release builds.
        let preferred_exp = (av.exponent - bv.exponent).clamp(MIN_EXPONENT, MAX_EXPONENT);
        return encode_finite(av.sign ^ bv.sign, preferred_exp, 0);
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

fn add_sub(a: u128, b: u128, negate_b: bool) -> u128 {
    let av = decode(a);
    let bv = decode(b);

    if av.is_nan() || bv.is_nan() {
        return QUIET_NAN;
    }

    let b_sign = if negate_b { !bv.sign } else { bv.sign };

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
    // then round the result to 34 digits once at the end.  Doing a single final
    // rounding avoids the sub-ULP precision loss that comes from pre-rounding
    // each operand independently.  `tail` classifies the fine operand's truncated
    // tail `f` (0 < f < 1 aligned units) relative to half a unit — it must reach
    // the final rounding, or a result sitting on a rounding boundary picks the
    // wrong neighbor.
    let (coef_a, coef_b, result_exp, tail) = align_exponents(a_exp, a_coef, b_exp, b_coef);

    if a_sign == b_sign {
        // True sum = coef_a + coef_b + f: strictly above the computed sum.
        let sum = coef_a + coef_b;
        let (final_exp, final_coef) = if sum > MAX_COEFFICIENT {
            // Clamp drops ≥ 1 digit: the dropped remainder r plus f satisfies
            // r ≤ half-1 → r + f < half regardless of f, so f only matters at an
            // exact-half r, where any non-zero f promotes the tie to round-up.
            clamp_to_34_digits_sticky(sum, result_exp, tail != TruncatedTail::Exact)
        } else {
            // No digit dropped: round sum + f to the integer grid directly.
            // (Reachable only via the fine-vanishes alignment branch, where
            // f < 1/2 always — but handle all classes for uniformity.)
            round_tail_to_grid(sum, result_exp, tail)
        };
        normalize_and_encode(a_sign, final_exp, final_coef)
    } else {
        // Effective subtraction.  The truncated (fine) operand is always the smaller
        // aligned magnitude, so it is always the subtrahend: true diff = d - f.
        // Rewrite as (d - 1) + (1 - f) with 0 < 1 - f < 1 — borrow one and flip the
        // tail class (below-half ↔ above-half; an exact half stays a half).
        let (diff, sign) = if coef_a >= coef_b {
            let d = coef_a - coef_b;
            let s = if d == 0 { false } else { a_sign }; // IEEE 754: +0 on cancellation
            (d, s)
        } else {
            (coef_b - coef_a, b_sign)
        };
        let (final_exp, final_coef) = if tail == TruncatedTail::Exact {
            clamp_to_34_digits(diff, result_exp)
        } else {
            // Truncation implies coarse > fine strictly, so diff ≥ 1 — the borrow
            // cannot underflow.
            let borrowed = diff - 1;
            if borrowed > MAX_COEFFICIENT {
                // Clamp drops ≥ 1 digit: dropped remainder r plus (1-f) < half
                // whenever r ≤ half-1, and any r ≥ half rounds up since 1-f > 0 —
                // a boolean sticky is exact here (no reachable tie).
                clamp_to_34_digits_sticky(borrowed, result_exp, true)
            } else {
                round_tail_to_grid(borrowed, result_exp, tail.flip())
            }
        };
        normalize_and_encode(sign, final_exp, final_coef)
    }
}

/// Classification of the sub-ULP tail truncated off the smaller operand during
/// exponent alignment, relative to half of one aligned unit.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TruncatedTail {
    /// Nothing truncated — the aligned coefficients are exact.
    Exact,
    /// 0 < f < 1/2 unit.
    BelowHalf,
    /// f == 1/2 unit exactly.
    Half,
    /// 1/2 unit < f < 1 unit.
    AboveHalf,
}

impl TruncatedTail {
    /// The class of `1 - f`, used when the tail is subtracted rather than added.
    /// `Exact` never flips (the caller branches on it before borrowing).
    fn flip(self) -> Self {
        match self {
            TruncatedTail::Exact => TruncatedTail::Exact,
            TruncatedTail::BelowHalf => TruncatedTail::AboveHalf,
            TruncatedTail::Half => TruncatedTail::Half,
            TruncatedTail::AboveHalf => TruncatedTail::BelowHalf,
        }
    }
}

/// Round `coef + f` (where `tail` classifies f against half a unit) to the integer
/// grid, half-even.  `coef` must already fit in 34 digits.
fn round_tail_to_grid(coef: u128, exp: i32, tail: TruncatedTail) -> (i32, u128) {
    let increment = match tail {
        TruncatedTail::Exact | TruncatedTail::BelowHalf => false,
        TruncatedTail::AboveHalf => true,
        TruncatedTail::Half => coef % 2 != 0,
    };
    let rounded = coef + increment as u128;
    if rounded > MAX_COEFFICIENT {
        // 999…9 + 1 = 10^34 — exact shift, no re-rounding.
        (exp + 1, rounded / 10)
    } else {
        (exp, rounded)
    }
}

/// Align two coefficients to the same exponent, using 35-digit working precision.
///
/// Returns `(coef_a_aligned, coef_b_aligned, result_exp, tail)` where both
/// coefficients are expressed at `result_exp`.  `tail` classifies the non-zero
/// digits truncated off the smaller ("fine") operand during alignment — the true
/// fine value is `fine_aligned + f` with `0 < f < 1` in aligned units — relative
/// to half a unit.  The caller is responsible for threading the class into the
/// single final rounding in add_finite (never dropping it).
///
/// Using 35-digit precision (DECIMAL_DIGITS + 1) ensures sub-ULP information is
/// preserved so the single final rounding in add_finite is correct.
fn align_exponents(
    a_exp: i32,
    a_coef: u128,
    b_exp: i32,
    b_coef: u128,
) -> (u128, u128, i32, TruncatedTail) {
    if a_exp == b_exp {
        return (a_coef, b_coef, a_exp, TruncatedTail::Exact);
    }

    let (coarse_exp, coarse_coef, fine_coef, fine_exp, large_is_a) = if a_exp > b_exp {
        (a_exp, a_coef, b_coef, b_exp, true)
    } else {
        (b_exp, b_coef, a_coef, a_exp, false)
    };
    let diff = (coarse_exp - fine_exp) as u32;
    let coarse_digits = decimal_digits(coarse_coef);
    let aligned_digits = coarse_digits + diff;

    // Working precision: DECIMAL_DIGITS + 1 = 35.
    const WORK: u32 = DECIMAL_DIGITS + 1;

    let (coarse_aligned, fine_aligned, result_exp, tail) =
        if aligned_digits > 2 * DECIMAL_DIGITS + 1 {
            // Fine is more than one full precision width smaller than coarse — it
            // rounds away entirely even at 35-digit working precision.  Its whole
            // (non-zero) value is the truncated tail; the digit gap guarantees
            // f < 10^-1 < 1/2, so the class is always BelowHalf.
            let tail = if fine_coef != 0 {
                TruncatedTail::BelowHalf
            } else {
                TruncatedTail::Exact
            };
            (coarse_coef, 0u128, coarse_exp, tail)
        } else if aligned_digits <= WORK {
            // Scaling up coarse by 10^diff produces ≤ 35 digits — no truncation needed.
            let scaled = coarse_coef * POW10[diff as usize];
            (scaled, fine_coef, fine_exp, TruncatedTail::Exact)
        } else {
            // Overflow: scale coarse up to exactly WORK digits; scale fine down by
            // the rest.  The truncated remainder of fine contributes < 1 aligned
            // unit; classify it against half a unit so the final rounding in
            // add_finite still sees it.
            let scale_up = (WORK - coarse_digits) as usize; // → coarse has WORK digits
            let scale_down = (aligned_digits - WORK) as usize; // → fine is trimmed
            let unit = POW10[scale_down.min(34)];
            let coarse_scaled = coarse_coef * POW10[scale_up];
            let fine_trimmed = fine_coef / unit;
            let fine_rem = fine_coef % unit;
            let tail = if fine_rem == 0 {
                TruncatedTail::Exact
            } else {
                // fine_rem < unit ≤ 10^34, so the ×2 cannot overflow u128.
                match (fine_rem * 2).cmp(&unit) {
                    std::cmp::Ordering::Less => TruncatedTail::BelowHalf,
                    std::cmp::Ordering::Equal => TruncatedTail::Half,
                    std::cmp::Ordering::Greater => TruncatedTail::AboveHalf,
                }
            };
            (
                coarse_scaled,
                fine_trimmed,
                fine_exp + scale_down as i32,
                tail,
            )
        };

    if large_is_a {
        (coarse_aligned, fine_aligned, result_exp, tail)
    } else {
        (fine_aligned, coarse_aligned, result_exp, tail)
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

/// If `coef` has more than 34 digits, round it to 34 in a single half-even step.
///
/// `sticky`: true if the caller knows there are additional non-zero digits below
/// the current precision (e.g. a tail truncated during exponent alignment, or a
/// division remainder) — a "half" remainder is then strictly above half, so it
/// rounds up rather than to even.
fn clamp_to_34_digits(coef: u128, exp: i32) -> (i32, u128) {
    clamp_to_34_digits_sticky(coef, exp, false)
}

/// Time: O(1) — one division by a power of 10 plus one comparison.  Space: O(1).
///
/// All excess digits are dropped as ONE unit compared against half a ULP.
/// Rounding one digit at a time is textbook multi-rounding: a low digit > 5
/// cascades a carry upward that the whole dropped unit (judged against half)
/// would not produce — e.g. dropping "4780" digit-by-digit rounds up at the "8",
/// but 4780 < 5000, so the correct single-step round is DOWN.
fn clamp_to_34_digits_sticky(coef: u128, exp: i32, sticky: bool) -> (i32, u128) {
    if coef <= MAX_COEFFICIENT {
        return (exp, coef);
    }
    // decimal_digits() saturates at 34 (its search table stops there), so count the
    // excess from the high part instead: coef > MAX means coef / 10^34 ∈ [1, ~34028]
    // (u128 caps at 39 digits), and digits(coef) = 34 + digits(coef / 10^34).
    let excess = decimal_digits(coef / POW10[DECIMAL_DIGITS as usize]); // ∈ [1, 5]
    let divisor = POW10[excess as usize];
    let q = coef / divisor;
    let r = coef % divisor;
    // Half-even on the whole dropped unit; sticky promotes an exact half to
    // "strictly above half" (the true value carries non-zero digits below r).
    let rounded = if sticky && r * 2 == divisor {
        q + 1
    } else {
        round_half_even(q, r, divisor)
    };
    // A round-up can push 999…9 (34 digits) to 10^34 — one exact shift fixes it.
    if rounded > MAX_COEFFICIENT {
        (exp + excess as i32 + 1, rounded / 10)
    } else {
        (exp + excess as i32, rounded)
    }
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

    // Compare by aligning exponents.  The truncated tail is ignorable here:
    // truncation only happens to the strictly-smaller aligned operand, so it can
    // never flip an ordering (the coarse operand always dominates).
    let (ca, cb, _, _) = align_exponents(a_exp, a_coef, b_exp, b_coef);
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
    fn div_zero_by_finite_at_exponent_extremes_produces_defined_zero() {
        // WHY: when av.exponent=MIN_EXPONENT and bv.exponent=MAX_EXPONENT, the naive
        // av.exponent - bv.exponent = -12287 is outside [MIN_EXPONENT, MAX_EXPONENT].
        // Without clamping, debug builds panic and release builds produce garbage bits.
        // The fix clamps to MIN_EXPONENT. Python decimal with strict decimal128 context
        // agrees the result is zero (both ctx.divide(0E-6176, 1E+6111) and the clamped
        // Yinz result decode as finite zero with correct sign).
        let zero_at_min_exp = encode_finite(false, MIN_EXPONENT, 0);
        let one_at_max_exp = encode_finite(false, MAX_EXPONENT, 1);
        let result = decode(div(zero_at_min_exp, one_at_max_exp));
        assert_eq!(
            result.kind,
            D128Kind::Finite,
            "div(0@MIN_EXP, 1@MAX_EXP) must be finite"
        );
        assert!(result.is_zero(), "div(0@MIN_EXP, 1@MAX_EXP) must be zero");
        assert!(!result.sign, "div(+0@MIN_EXP, +1@MAX_EXP) must be +0");

        // Reverse: zero at MAX_EXP divided by nonzero at MIN_EXP — exponent difference is +12287,
        // clamped to MAX_EXPONENT=6111. Python: div(0E+6111, 1E-6176) = 0E+6111 = zero. Agrees.
        let zero_at_max_exp = encode_finite(false, MAX_EXPONENT, 0);
        let one_at_min_exp = encode_finite(false, MIN_EXPONENT, 1);
        let result2 = decode(div(zero_at_max_exp, one_at_min_exp));
        assert_eq!(
            result2.kind,
            D128Kind::Finite,
            "div(0@MAX_EXP, 1@MIN_EXP) must be finite"
        );
        assert!(result2.is_zero(), "div(0@MAX_EXP, 1@MIN_EXP) must be zero");
        assert!(!result2.sign, "div(+0@MAX_EXP, +1@MIN_EXP) must be +0");

        // Negative zero: sign propagation.
        let neg_zero = encode_finite(true, MIN_EXPONENT, 0);
        let result3 = decode(div(neg_zero, one_at_max_exp));
        assert_eq!(result3.kind, D128Kind::Finite);
        assert!(result3.is_zero());
        assert!(result3.sign, "div(-0@MIN_EXP, +1@MAX_EXP) must be -0");
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

    #[test]
    fn neg_and_abs() {
        let v = from_str("3.14");
        let neg_v = neg(v);
        assert_eq!(to_str(neg_v), "-3.14");
        assert_eq!(to_str(abs(neg_v)), "3.14");
    }
}
