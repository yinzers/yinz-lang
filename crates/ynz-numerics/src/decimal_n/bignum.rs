/// Arbitrary-precision decimal floating-point number.
///
/// Stores the value as `sign × coefficient × 10^exponent` where the coefficient
/// is represented as a Vec of decimal digits (base-10, most-significant-first).
/// `precision` is the maximum number of significant digits — the result of any
/// operation is rounded to this many digits using half-even (banker's rounding).
///
/// This is a pure Rust implementation following IEEE 754-2008 decimal semantics.
/// For N ≤ 34 digits, the existing decimal128 hardware path is used instead.
#[derive(Clone, Debug)]
pub struct BigNum {
    /// Max significant decimal digits (35..=4096).
    pub precision: u16,
    /// True when the value is negative.
    pub sign: bool,
    /// Decimal digits 0-9, most-significant-first. Empty iff is_special.
    pub digits: Vec<u8>,
    /// Decimal exponent: value = coefficient × 10^exponent.
    pub exponent: i64,
    /// True for ±Infinity.
    pub is_infinity: bool,
    /// True for NaN.
    pub is_nan: bool,
}

impl BigNum {
    /// Zero at the given precision.
    pub fn zero(precision: u16) -> Self {
        BigNum {
            precision,
            sign: false,
            digits: vec![0],
            exponent: 0,
            is_infinity: false,
            is_nan: false,
        }
    }

    /// Create from a decimal string representation, rounded to `precision` digits.
    ///
    /// Accepts the same format as Python `Decimal(s)`: optional sign, digits,
    /// optional `.`, optional `e/E` exponent.
    pub fn from_str(s: &str, precision: u16) -> Option<Self> {
        super::parse::parse_str(s, precision)
    }

    /// True if the value is zero (positive or negative).
    ///
    /// After `normalize()`, zero is canonically `digits = [0]` (a single zero digit),
    /// so the check is O(1) rather than O(digits.len()).
    pub fn is_zero(&self) -> bool {
        !self.is_infinity && !self.is_nan && self.digits.len() == 1 && self.digits[0] == 0
    }

    /// Adjust this number to have exactly `precision` significant digits,
    /// using half-even rounding, and normalize the exponent.
    ///
    /// Time: O(precision) — single truncate pass + at most one additive carry pass. Space: O(1) — mutates in place.
    pub fn round_to_precision(&mut self) {
        if self.is_special() {
            return;
        }
        let p = self.precision as usize;
        if self.digits.is_empty() {
            self.digits = vec![0];
            return;
        }
        // Remove leading zeros
        while self.digits.len() > 1 && self.digits[0] == 0 {
            self.digits.remove(0);
        }
        if self.digits.len() > p {
            // Need to round: keep first `p` digits, round based on the (p+1)th digit.
            let round_digit = self.digits[p];
            let last_kept = self.digits[p - 1];
            let mut increment = false;
            if round_digit > 5 {
                increment = true;
            } else if round_digit == 5 {
                // Half-even: round to even
                let rest_nonzero = self.digits[p + 1..].iter().any(|&d| d != 0);
                if rest_nonzero {
                    increment = true;
                } else {
                    // Exactly 0.5: round to even (last kept digit must be even)
                    increment = last_kept % 2 != 0;
                }
            }
            let removed = self.digits.len() - p;
            self.exponent += removed as i64;
            self.digits.truncate(p);
            if increment {
                self.add_one_to_digits();
            }
            // Remove trailing zeros (they're already accounted in the exponent)
        }
    }

    /// Normalize: remove trailing zeros and adjust exponent.
    pub fn normalize(&mut self) {
        if self.is_special() {
            return;
        }
        // Remove trailing zeros
        while self.digits.len() > 1 && *self.digits.last().unwrap() == 0 {
            self.digits.pop();
            self.exponent += 1;
        }
        // Remove leading zeros
        while self.digits.len() > 1 && self.digits[0] == 0 {
            self.digits.remove(0);
        }
        // Invariant: after normalize(), a non-zero value has digits[0] != 0,
        // so is_zero() can check O(1) via `digits == [0]`.
        debug_assert!(
            self.digits.is_empty() || self.digits[0] != 0 || self.digits.len() == 1,
            "normalize() invariant violated: non-zero value has leading zero digit"
        );
    }

    fn add_one_to_digits(&mut self) {
        // Add 1 to the digit array (bignum increment).
        // The coefficient represents value = digits × 10^exponent.
        // Adding 1 to the digits changes the coefficient; the exponent is NOT changed
        // by the increment itself (a carry from [9] → [1, 0] gives coefficient 10,
        // value = 10 × 10^exponent = same position in the number line as before + 1 ULP).
        let mut carry = 1u8;
        for d in self.digits.iter_mut().rev() {
            let sum = *d + carry;
            *d = sum % 10;
            carry = sum / 10;
            if carry == 0 {
                break;
            }
        }
        if carry > 0 {
            self.digits.insert(0, carry);
            // The value is now (p+1) digits. If this exceeds precision, truncate by
            // removing the last digit and adjusting exponent (to preserve magnitude).
            if self.digits.len() > self.precision as usize {
                let excess = self.digits.len() - self.precision as usize;
                self.exponent += excess as i64;
                self.digits.truncate(self.precision as usize);
            }
        }
    }

    pub fn is_special(&self) -> bool {
        self.is_infinity || self.is_nan
    }

    /// Format as a decimal string (coefficient × 10^exponent form).
    pub fn to_string_repr(&self) -> String {
        super::format::format_bignum(self)
    }
}

/// Return the larger of two precisions.
pub fn max_precision(a: u16, b: u16) -> u16 {
    a.max(b)
}
