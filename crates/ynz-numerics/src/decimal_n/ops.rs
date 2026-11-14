/// IEEE 754-2008 decimal arithmetic for BigNum at arbitrary precision.
///
/// All operations round the result to `precision` significant digits using
/// half-even (banker's) rounding. The result precision is max(a.precision, b.precision).
use super::bignum::{max_precision, BigNum};

/// Align two BigNum values to the same exponent for addition/subtraction.
/// Returns (longer_digits, shorter_digits, common_exponent).
fn align(a: &BigNum, b: &BigNum) -> (Vec<u8>, Vec<u8>, i64) {
    let exp_diff = a.exponent - b.exponent;
    if exp_diff == 0 {
        (a.digits.clone(), b.digits.clone(), a.exponent)
    } else if exp_diff > 0 {
        // a has higher exponent: pad a with trailing zeros
        let mut a_digits = a.digits.clone();
        a_digits.extend(std::iter::repeat(0).take(exp_diff.unsigned_abs() as usize));
        (a_digits, b.digits.clone(), b.exponent)
    } else {
        // b has higher exponent: pad b with trailing zeros
        let mut b_digits = b.digits.clone();
        b_digits.extend(std::iter::repeat(0).take(exp_diff.unsigned_abs() as usize));
        (a.digits.clone(), b_digits, a.exponent)
    }
}

/// Add two digit arrays (right-aligned, big-endian digits). Returns sum digits + carry.
fn add_digits(a: &[u8], b: &[u8]) -> Vec<u8> {
    let len = a.len().max(b.len());
    let a_pad = len - a.len(); // leading zero pads for a
    let b_pad = len - b.len(); // leading zero pads for b
    let mut result = vec![0u8; len + 1];
    let mut carry = 0u8;
    for i in (0..len).rev() {
        let da = if i >= a_pad { a[i - a_pad] } else { 0 };
        let db = if i >= b_pad { b[i - b_pad] } else { 0 };
        let sum = da + db + carry;
        result[i + 1] = sum % 10;
        carry = sum / 10;
    }
    result[0] = carry;
    result
}

/// Subtract b from a where |a| >= |b|. Both must be right-aligned (same exponent).
/// Left-pads the shorter array with leading zeros before subtracting.
fn sub_digits(a: &[u8], b: &[u8]) -> Vec<u8> {
    let len = a.len().max(b.len());
    let a_pad = len - a.len(); // leading zeros needed for a
    let b_pad = len - b.len(); // leading zeros needed for b
    let mut result = vec![0u8; len];
    let mut borrow = 0i8;
    for i in (0..len).rev() {
        let da = if i >= a_pad { a[i - a_pad] as i8 } else { 0 };
        let db = if i >= b_pad { b[i - b_pad] as i8 } else { 0 };
        let mut diff = da - db - borrow;
        if diff < 0 {
            diff += 10;
            borrow = 1;
        } else {
            borrow = 0;
        }
        result[i] = diff as u8;
    }
    result
}

/// Compare digit arrays (both assumed same length, aligned).
/// Returns `true` if a >= b.
/// Compare two digit arrays as numeric magnitudes (most-significant-digit first).
/// Left-pads the shorter array with leading zeros.
fn ge_digits(a: &[u8], b: &[u8]) -> bool {
    // Compare lengths first — more digits at the same scale means larger number.
    if a.len() != b.len() {
        return a.len() > b.len();
    }
    // Same length: compare digit by digit.
    for i in 0..a.len() {
        if a[i] > b[i] {
            return true;
        }
        if a[i] < b[i] {
            return false;
        }
    }
    true // equal
}

/// Add two BigNum values, rounding to max(a.precision, b.precision).
pub fn add(a: &BigNum, b: &BigNum) -> BigNum {
    let prec = max_precision(a.precision, b.precision);

    if a.is_nan || b.is_nan {
        return BigNum {
            precision: prec,
            sign: false,
            digits: vec![],
            exponent: 0,
            is_infinity: false,
            is_nan: true,
        };
    }
    if a.is_infinity || b.is_infinity {
        if a.is_infinity && b.is_infinity {
            if a.sign != b.sign {
                // +inf + -inf = NaN
                return BigNum {
                    precision: prec,
                    sign: false,
                    digits: vec![],
                    exponent: 0,
                    is_infinity: false,
                    is_nan: true,
                };
            }
            return BigNum {
                precision: prec,
                sign: a.sign,
                digits: vec![],
                exponent: 0,
                is_infinity: true,
                is_nan: false,
            };
        }
        let inf = if a.is_infinity { a } else { b };
        return BigNum {
            precision: prec,
            sign: inf.sign,
            digits: vec![],
            exponent: 0,
            is_infinity: true,
            is_nan: false,
        };
    }

    let (a_digits, b_digits, common_exp) = align(a, b);

    let result_digits;
    let mut result_sign;
    let result_exp = common_exp;

    if a.sign == b.sign {
        // Same sign: add magnitudes
        result_digits = add_digits(&a_digits, &b_digits);
        result_sign = a.sign;
    } else {
        // Different signs: subtract smaller from larger
        if ge_digits(&a_digits, &b_digits) {
            result_digits = sub_digits(&a_digits, &b_digits);
            result_sign = a.sign;
        } else {
            result_digits = sub_digits(&b_digits, &a_digits);
            result_sign = b.sign;
        }
        // +0 + -0 = +0 per IEEE 754
        if result_digits.iter().all(|&d| d == 0) {
            result_sign = false;
        }
    }

    let mut bn = BigNum {
        precision: prec,
        sign: result_sign,
        digits: result_digits,
        exponent: result_exp,
        is_infinity: false,
        is_nan: false,
    };
    bn.round_to_precision();
    bn.normalize();
    bn
}

/// Subtract: a - b = a + (-b).
pub fn sub(a: &BigNum, b: &BigNum) -> BigNum {
    let neg_b = BigNum {
        sign: !b.sign,
        ..b.clone()
    };
    add(a, &neg_b)
}

/// Multiply two BigNum values.
///
/// Time: O(n × m) where n = a.digits.len(), m = b.digits.len(). Space: O(n + m) for the result digit vector.
pub fn mul(a: &BigNum, b: &BigNum) -> BigNum {
    let prec = max_precision(a.precision, b.precision);

    if a.is_nan || b.is_nan {
        return BigNum {
            precision: prec,
            sign: false,
            digits: vec![],
            exponent: 0,
            is_infinity: false,
            is_nan: true,
        };
    }
    if a.is_infinity || b.is_infinity {
        let zero_a = !a.is_infinity && a.is_zero();
        let zero_b = !b.is_infinity && b.is_zero();
        if zero_a || zero_b {
            return BigNum {
                precision: prec,
                sign: false,
                digits: vec![],
                exponent: 0,
                is_infinity: false,
                is_nan: true,
            };
        }
        return BigNum {
            precision: prec,
            sign: a.sign ^ b.sign,
            digits: vec![],
            exponent: 0,
            is_infinity: true,
            is_nan: false,
        };
    }

    // Schoolbook decimal multiplication
    let n = a.digits.len();
    let m = b.digits.len();
    let mut result = vec![0u64; n + m];

    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let product = a.digits[i] as u64 * b.digits[j] as u64;
            let pos = i + j + 1;
            result[pos] += product;
        }
    }

    // Propagate carries right-to-left. result[0] accumulates all cascaded carry and
    // may exceed a single digit after this pass.
    for i in (1..result.len()).rev() {
        result[i - 1] += result[i] / 10;
        result[i] %= 10;
    }
    // Reduce result[0] without Vec::insert(0, carry) O(n) shifts.
    // Count how many extra leading digits result[0] needs, then pre-extend once.
    let mut head = result[0];
    let mut extra_digits: Vec<u64> = Vec::new();
    head %= 10; // keep the original slot's value after extracting carries
    let mut carry = result[0] / 10;
    while carry >= 10 {
        extra_digits.push(carry % 10);
        carry /= 10;
    }
    if carry > 0 {
        extra_digits.push(carry);
    }
    result[0] = head;
    // extra_digits is least-significant-first; reverse for most-significant-first prepend.
    // Build final digit array by pre-allocating the exact size needed.
    let extra = extra_digits.len();
    let mut result_digits: Vec<u8> = Vec::with_capacity(extra + result.len());
    for d in extra_digits.into_iter().rev() {
        result_digits.push(d as u8);
    }
    for d in result.iter() {
        result_digits.push(*d as u8);
    }
    let result_exp = a.exponent + b.exponent;
    let result_sign = a.sign ^ b.sign;

    let mut bn = BigNum {
        precision: prec,
        sign: result_sign,
        digits: result_digits,
        exponent: result_exp,
        is_infinity: false,
        is_nan: false,
    };
    bn.round_to_precision();
    bn.normalize();
    bn
}

/// Divide a by b, computing `precision` significant digits with half-even rounding.
///
/// Time: O(n × m) where n = a.digits.len(), m = b.digits.len(). Space: O(n + m) for the result digit vector.
pub fn div(a: &BigNum, b: &BigNum) -> BigNum {
    let prec = max_precision(a.precision, b.precision);

    if a.is_nan || b.is_nan {
        return BigNum {
            precision: prec,
            sign: false,
            digits: vec![],
            exponent: 0,
            is_infinity: false,
            is_nan: true,
        };
    }
    if b.is_zero() {
        if a.is_zero() {
            return BigNum {
                precision: prec,
                sign: false,
                digits: vec![],
                exponent: 0,
                is_infinity: false,
                is_nan: true,
            };
        }
        return BigNum {
            precision: prec,
            sign: a.sign ^ b.sign,
            digits: vec![],
            exponent: 0,
            is_infinity: true,
            is_nan: false,
        };
    }
    if a.is_infinity {
        if b.is_infinity {
            return BigNum {
                precision: prec,
                sign: false,
                digits: vec![],
                exponent: 0,
                is_infinity: false,
                is_nan: true,
            };
        }
        return BigNum {
            precision: prec,
            sign: a.sign ^ b.sign,
            digits: vec![],
            exponent: 0,
            is_infinity: true,
            is_nan: false,
        };
    }
    if a.is_zero() {
        return BigNum::zero(prec);
    }

    // Long division: produce prec+1 digits then round.
    // Scale: dividend = a_coeff × 10^a_exp, divisor = b_coeff × 10^b_exp
    // result = (a_coeff / b_coeff) × 10^(a_exp - b_exp)
    //
    // To get prec+1 significant digits, we compute:
    //   a_coeff × 10^(prec+1 - len(a_coeff) + len(b_coeff)) / b_coeff
    // then adjust exponent.

    let numerator_digits = (prec as usize + 1).saturating_add(b.digits.len());
    let extra = numerator_digits.saturating_sub(a.digits.len());
    // Compute scaled dividend: a_digits × 10^extra
    let mut dividend: Vec<u8> = a.digits.clone();
    dividend.extend(std::iter::repeat(0).take(extra));

    // Long division by b_digits
    let divisor = &b.digits;
    let mut quotient = Vec::new();
    let mut remainder: Vec<u8> = Vec::new();
    // Reuse scratch buffers across iterations to avoid per-step Vec allocation.
    let mut mul_scratch: Vec<u8> = Vec::with_capacity(divisor.len() + 1);
    let mut sub_scratch: Vec<u8> = Vec::new();

    for &digit in &dividend {
        remainder.push(digit);
        // Find how many times divisor fits into remainder
        let q = long_div_step(&remainder, divisor);
        quotient.push(q);
        // Compute q × divisor into scratch buffer, then subtract.
        mul_single_into(divisor, q, &mut mul_scratch);
        sub_long_into(&remainder, &mul_scratch, &mut sub_scratch);
        std::mem::swap(&mut remainder, &mut sub_scratch);
        // Remove leading zeros from remainder without shifting.
        let start = remainder
            .iter()
            .position(|&d| d != 0)
            .unwrap_or(remainder.len() - 1);
        if start > 0 {
            remainder.drain(0..start);
        }
    }

    let result_exp = a.exponent - b.exponent - extra as i64;
    let result_sign = a.sign ^ b.sign;

    // The final long-division remainder is the value's tail below the computed
    // quotient digits — it must reach the rounding as a sticky signal, or a
    // quotient ending exactly on the half boundary ties-to-even when the true
    // value is strictly above half.
    let remainder_nonzero = remainder.iter().any(|&d| d != 0);

    let mut bn = BigNum {
        precision: prec,
        sign: result_sign,
        digits: quotient,
        exponent: result_exp,
        is_infinity: false,
        is_nan: false,
    };
    bn.round_to_precision_sticky(remainder_nonzero);
    bn.normalize();
    bn
}

/// Long-division step: how many times does `divisor` fit into `dividend`?
/// Both are digit arrays; dividend may be shorter than divisor (returns 0).
fn long_div_step(dividend: &[u8], divisor: &[u8]) -> u8 {
    // Remove leading zeros from dividend for comparison
    let mut d = dividend.to_vec();
    while d.len() > 1 && d[0] == 0 {
        d.remove(0);
    }

    if d.len() < divisor.len() {
        return 0;
    }
    if d.len() == divisor.len() && !ge_digits(&d, divisor) {
        return 0;
    }

    // Binary search for quotient digit 0..9
    let mut lo = 0u8;
    let mut hi = 9u8;
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let trial = mul_single(divisor, mid);
        if ge_digits_padded(&d, &trial) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

/// Multiply digit array by a single digit 0..9.
fn mul_single(digits: &[u8], factor: u8) -> Vec<u8> {
    let mut result = vec![0u8; digits.len() + 1];
    let mut carry = 0u8;
    for i in (0..digits.len()).rev() {
        let product = digits[i] * factor + carry;
        result[i + 1] = product % 10;
        carry = product / 10;
    }
    result[0] = carry;
    // Strip leading zeros via slice-from-offset — no O(n) shift per removed zero.
    let start = result
        .iter()
        .position(|&d| d != 0)
        .unwrap_or(result.len() - 1);
    result[start..].to_vec()
}

/// Scratch-buffer variant of sub_long: writes `a - b` into `out`, reusing its allocation.
fn sub_long_into(a: &[u8], b: &[u8], out: &mut Vec<u8>) {
    let len = a.len().max(b.len());
    let a_pad = len - a.len();
    let b_pad = len - b.len();
    out.clear();
    out.resize(len, 0u8);
    let mut borrow = 0i8;
    for i in (0..len).rev() {
        let da = if i >= a_pad { a[i - a_pad] as i8 } else { 0 };
        let db = if i >= b_pad { b[i - b_pad] as i8 } else { 0 };
        let mut diff = da - db - borrow;
        if diff < 0 {
            diff += 10;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out[i] = diff as u8;
    }
}

/// Scratch-buffer variant of mul_single: writes result into `out`, reusing its allocation.
fn mul_single_into(digits: &[u8], factor: u8, out: &mut Vec<u8>) {
    out.clear();
    out.resize(digits.len() + 1, 0u8);
    let mut carry = 0u8;
    for i in (0..digits.len()).rev() {
        let product = digits[i] * factor + carry;
        out[i + 1] = product % 10;
        carry = product / 10;
    }
    out[0] = carry;
    // Strip leading zeros in place — drain is O(n) but fires at most once on the first slot.
    let start = out.iter().position(|&d| d != 0).unwrap_or(out.len() - 1);
    if start > 0 {
        out.drain(0..start);
    }
}

/// Compare digit arrays of potentially different lengths (compare magnitudes).
/// Returns true if `a` (length-padded) >= b.
/// Compare digit arrays of potentially different lengths as NUMERIC magnitudes.
///
/// Left-pads the shorter array with leading zeros for comparison.
/// Example: [1, 0] (=10) vs [9] (=9) → pads [9] to [0, 9] → 10 > 9 → returns true.
fn ge_digits_padded(a: &[u8], b: &[u8]) -> bool {
    let len = a.len().max(b.len());
    let a_offset = len.saturating_sub(a.len()); // leading zeros for a
    let b_offset = len.saturating_sub(b.len()); // leading zeros for b
    for i in 0..len {
        let da = if i >= a_offset { a[i - a_offset] } else { 0 };
        let db = if i >= b_offset { b[i - b_offset] } else { 0 };
        if da > db {
            return true;
        }
        if da < db {
            return false;
        }
    }
    true
}

/// Exact numeric comparison of two BigNums; precision plays no part.
///
/// `None` when unordered (either side is NaN). `-0` equals `+0`; `2.5` equals `2.50`.
///
/// Time O(digits), space O(1): compares the position of the most significant digit first, then
/// the digits one by one (a missing digit counts as 0), so no operand is aligned or copied.
pub fn compare(left: &BigNum, right: &BigNum) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    if left.is_nan || right.is_nan {
        return None;
    }
    let left_class = sign_class(left);
    let right_class = sign_class(right);
    if left_class != right_class {
        return Some(left_class.cmp(&right_class));
    }
    match left_class {
        // both -inf, both zero, or both +inf
        -2 | 0 | 2 => Some(Ordering::Equal),
        1 => Some(compare_magnitudes(left, right)),
        // both negative: the larger magnitude is the smaller number
        _ => Some(compare_magnitudes(right, left)),
    }
}

/// -2 = -infinity, -1 = negative, 0 = zero (of either sign), 1 = positive, 2 = +infinity.
fn sign_class(value: &BigNum) -> i8 {
    if value.is_infinity {
        return if value.sign { -2 } else { 2 };
    }
    if value.digits.iter().all(|&digit| digit == 0) {
        return 0;
    }
    if value.sign {
        -1
    } else {
        1
    }
}

/// Compare |left| and |right|, both finite and non-zero.
fn compare_magnitudes(left: &BigNum, right: &BigNum) -> std::cmp::Ordering {
    let left_leading_zeros = left.digits.iter().take_while(|&&digit| digit == 0).count();
    let right_leading_zeros = right.digits.iter().take_while(|&&digit| digit == 0).count();
    let left_digits = &left.digits[left_leading_zeros..];
    let right_digits = &right.digits[right_leading_zeros..];
    // Position of the most significant digit: value = 0.d1d2.. × 10^(exponent + len).
    let left_magnitude = left.exponent + left_digits.len() as i64;
    let right_magnitude = right.exponent + right_digits.len() as i64;
    if left_magnitude != right_magnitude {
        return left_magnitude.cmp(&right_magnitude);
    }
    let longest = left_digits.len().max(right_digits.len());
    for position in 0..longest {
        let left_digit = left_digits.get(position).copied().unwrap_or(0);
        let right_digit = right_digits.get(position).copied().unwrap_or(0);
        if left_digit != right_digit {
            return left_digit.cmp(&right_digit);
        }
    }
    std::cmp::Ordering::Equal
}

#[cfg(test)]
mod compare_tests {
    use super::*;
    use std::cmp::Ordering::{Equal, Greater, Less};

    fn parsed(text: &str) -> BigNum {
        BigNum::from_str(text, 100).expect("test literal parses")
    }

    /// One test per edge case of the compare contract (High: pure math, the edge cases are the
    /// contract): `compare_case!(case, left, right, expected, "bug it catches")`.
    macro_rules! compare_case {
        ($case:ident, $left:literal, $right:literal, $expected:expr, $bug:literal) => {
            #[doc = $bug]
            #[test]
            fn $case() {
                assert_eq!(
                    compare(&parsed($left), &parsed($right)),
                    $expected,
                    "compare({}, {})",
                    $left,
                    $right
                );
            }
        };
    }

    compare_case!(
        nine_is_less_than_ten,
        "9",
        "10",
        Some(Less),
        "Bug caught: a textual comparison (`\"9\" > \"10\"`). (High)"
    );
    compare_case!(
        trailing_zeros_are_equal,
        "2.5",
        "2.50",
        Some(Equal),
        "Bug caught: operands of different digit counts compare unequal. (High)"
    );
    compare_case!(
        a_smaller_exponent_is_less,
        "0.001",
        "0.01",
        Some(Less),
        "Bug caught: the exponent is ignored, so equal digits compare equal. (High)"
    );
    compare_case!(
        a_larger_exponent_beats_more_digits,
        "1e3",
        "999",
        Some(Greater),
        "Bug caught: the digit count decides instead of the position of the leading digit. (High)"
    );
    compare_case!(
        a_negative_is_less_than_a_positive,
        "-1",
        "1",
        Some(Less),
        "Bug caught: the sign is ignored. (High)"
    );
    compare_case!(
        between_negatives_the_larger_magnitude_is_less,
        "-9",
        "-10",
        Some(Greater),
        "Bug caught: negatives are ordered by magnitude, not reversed. (High)"
    );
    compare_case!(
        negative_zero_equals_zero,
        "-0",
        "0",
        Some(Equal),
        "Bug caught: `-0` orders below `0` because its sign flag is set. (High)"
    );
    compare_case!(
        a_negative_fraction_is_less_than_zero,
        "-0.5",
        "0",
        Some(Less),
        "Bug caught: a negative against zero compares as its magnitude. (High)"
    );
    compare_case!(
        a_difference_in_the_last_of_39_digits_decides,
        "123456789012345678901234567890123456789",
        "123456789012345678901234567890123456788",
        Some(Greater),
        "Bug caught: the digits are compared through a narrower number (u64, f64) and the last digits are lost. (High)"
    );
    compare_case!(
        infinity_is_greater_than_any_finite,
        "inf",
        "1e400",
        Some(Greater),
        "Bug caught: infinity compared by its (empty) digits. (High)"
    );
    compare_case!(
        negative_infinity_is_less_than_any_finite,
        "-inf",
        "-1e400",
        Some(Less),
        "Bug caught: negative infinity compared by its (empty) digits. (High)"
    );
    compare_case!(
        infinity_equals_infinity,
        "inf",
        "inf",
        Some(Equal),
        "Bug caught: two infinities of one sign compare by digits and are not equal. (High)"
    );
    compare_case!(
        nan_on_the_left_is_unordered,
        "nan",
        "1",
        None,
        "Bug caught: NaN orders against a number. (High)"
    );
    compare_case!(
        nan_on_the_right_is_unordered,
        "1",
        "nan",
        None,
        "Bug caught: only the left operand is checked for NaN. (High)"
    );
}
