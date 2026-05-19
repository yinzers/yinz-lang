/// IEEE 754-2008 decimal128 BID (Binary Integer Significand) encoding.
///
/// # Bit layout (128 bits, bit 127 = MSB)
///
/// ```text
/// [127]    S  — sign
/// [126:110] G  — 17-bit combination field
/// [109:0]  T  — 110-bit trailing coefficient
/// ```
///
/// # Combination field G[16:0] (G[16] = bit 126, G[0] = bit 110)
///
/// G[16:15] ≠ 11 (leading digit 0-7):
///   exp_high  = G[16:15]          (2 bits)
///   lead_dig  = G[14:12]          (3 bits, 0-7)
///   exp_low   = G[11:0]           (12 bits)
///
/// G[16:15] = 11, G[14:13] ≠ 11 (leading digit 8-9):
///   exp_high  = G[14:13]          (2 bits)
///   lead_dig  = 8 + G[12]         (1 bit, 8 or 9)
///   exp_low   = G[11:0]           (12 bits)
///
/// G[16:12] = 11110 → ±Infinity
/// G[16:12] = 11111 → NaN (G[11] = 0: quiet, G[11] = 1: signaling)
///
/// Biased exponent = (exp_high << 12) | exp_low; actual = biased − 6176
/// Full coefficient = lead_dig × 10^33 + T (non-canonical if > 10^34 − 1)
pub const EXPONENT_BIAS: i32 = 6176;
pub const MAX_EXPONENT: i32 = 6111;
pub const MIN_EXPONENT: i32 = -6176;
/// Maximum representable coefficient: 10^34 − 1
pub const MAX_COEFFICIENT: u128 = 9_999_999_999_999_999_999_999_999_999_999_999;
pub const DECIMAL_DIGITS: u32 = 34;
/// 10^33  (the value of one "leading digit slot")
pub const TEN_33: u128 = 1_000_000_000_000_000_000_000_000_000_000_000;
/// 10^34
pub const TEN_34: u128 = 10_000_000_000_000_000_000_000_000_000_000_000;

pub const SIGN_MASK: u128 = 1u128 << 127;
pub const G_MASK: u128 = 0x1_FFFF_u128 << 110; // 17 bits [126:110]
pub const T_MASK: u128 = (1u128 << 110) - 1; // 110 bits [109:0]

/// Well-known encoded bit patterns.
pub const POS_ZERO: u128 = encode_finite_raw(false, EXPONENT_BIAS as u32, 0, 0);
pub const NEG_ZERO: u128 = encode_finite_raw(true, EXPONENT_BIAS as u32, 0, 0);
pub const POS_INF: u128 = encode_infinity(false);
pub const NEG_INF: u128 = encode_infinity(true);
pub const QUIET_NAN: u128 = encode_qnan(false);

/// The decoded form used for arithmetic.  Only valid when `is_finite` is true.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct D128Value {
    pub sign: bool,
    pub exponent: i32,
    pub coefficient: u128,
    pub kind: D128Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum D128Kind {
    Finite,
    Infinity,
    QuietNaN,
    SignalingNaN,
}

impl D128Value {
    pub fn is_finite(self) -> bool {
        self.kind == D128Kind::Finite
    }
    pub fn is_nan(self) -> bool {
        matches!(self.kind, D128Kind::QuietNaN | D128Kind::SignalingNaN)
    }
    pub fn is_infinity(self) -> bool {
        self.kind == D128Kind::Infinity
    }
    pub fn is_zero(self) -> bool {
        self.is_finite() && self.coefficient == 0
    }
}

/// Decode a raw BID bit-pattern into the canonical representation.
pub fn decode(bits: u128) -> D128Value {
    let sign = (bits & SIGN_MASK) != 0;
    let g = ((bits >> 110) & 0x1_FFFF) as u32; // G[16:0], 17 bits
    let t = bits & T_MASK; // trailing 110 bits

    let top5 = g >> 12; // G[16:12]

    match top5 {
        0b11111 => D128Value {
            sign,
            exponent: 0,
            coefficient: 0,
            kind: D128Kind::QuietNaN,
        },
        0b11110 => D128Value {
            sign,
            exponent: 0,
            coefficient: 0,
            kind: D128Kind::Infinity,
        },
        _ => {
            let (exp_high, lead, exp_low) = if g >> 15 != 3 {
                // G[16:15] ≠ 11: lead digit 0-7
                let exp_high = g >> 15; // 2 bits
                let lead = (g >> 12) & 0x7; // 3 bits
                let exp_low = g & 0xFFF; // 12 bits
                (exp_high, lead, exp_low)
            } else {
                // G[16:15] = 11: lead digit 8-9
                let exp_high = (g >> 13) & 0x3; // G[14:13]
                let lead = 8 + ((g >> 12) & 0x1); // G[12]
                let exp_low = g & 0xFFF; // G[11:0]
                (exp_high, lead, exp_low)
            };

            let biased = (exp_high << 12) | exp_low;
            let exponent = biased as i32 - EXPONENT_BIAS;
            let coefficient = lead as u128 * TEN_33 + t;

            // Non-canonical: coefficient too large → treat as 0 (IEEE 754 §3.5.2)
            let coefficient = if coefficient > MAX_COEFFICIENT {
                0
            } else {
                coefficient
            };

            D128Value {
                sign,
                exponent,
                coefficient,
                kind: D128Kind::Finite,
            }
        }
    }
}

/// Encode a finite value.  `coefficient` must be in [0, 10^34 − 1].
pub fn encode_finite(sign: bool, exponent: i32, coefficient: u128) -> u128 {
    debug_assert!(coefficient <= MAX_COEFFICIENT, "coefficient overflow");
    debug_assert!(
        (MIN_EXPONENT..=MAX_EXPONENT).contains(&exponent),
        "exponent out of range: {exponent}"
    );

    let biased = (exponent + EXPONENT_BIAS) as u32;
    let exp_high = biased >> 12; // top 2 bits of the 14-bit biased exp
    let exp_low = biased & 0xFFF; // bottom 12 bits

    let lead = (coefficient / TEN_33) as u32;
    let trailing = coefficient % TEN_33;

    let g: u32 = if lead <= 7 {
        // G[16:15] = exp_high, G[14:12] = lead, G[11:0] = exp_low
        (exp_high << 15) | (lead << 12) | exp_low
    } else {
        // lead = 8 or 9 → G[16:15] = 11, G[14:13] = exp_high, G[12] = lead-8, G[11:0] = exp_low
        (0b11 << 15) | (exp_high << 13) | ((lead - 8) << 12) | exp_low
    };

    let sign_bit = if sign { SIGN_MASK } else { 0 };
    sign_bit | ((g as u128) << 110) | trailing
}

/// Encode ±Infinity per IEEE 754-2008 §3.5.2 decimal128 BID format.
///
/// Bit pattern: G[16:12] = `11110`. Coefficient and exponent fields are ignored
/// by decoders for Infinity (IEEE 754-2008 §6.2).
pub const fn encode_infinity(sign: bool) -> u128 {
    // G[16:12] = 11110
    let sign_bit: u128 = if sign { SIGN_MASK } else { 0 };
    sign_bit | (0b11110_u128 << 122) // bits [126:122]
}

/// Encode a quiet NaN per IEEE 754-2008 §3.5.2 decimal128 BID format.
///
/// Bit pattern: G[16:12] = `11111`, G[11] = 0 (quiet; 1 would be signaling NaN).
/// Payload bits T[109:0] are zero (no diagnostic information encoded).
pub const fn encode_qnan(sign: bool) -> u128 {
    // G[16:12] = 11111, G[11] = 0 (quiet)
    let sign_bit: u128 = if sign { SIGN_MASK } else { 0 };
    sign_bit | (0b11111_u128 << 122)
}

/// Internal: encode without range checks (used for const initializers).
const fn encode_finite_raw(sign: bool, biased: u32, lead: u32, trailing: u128) -> u128 {
    let exp_high = biased >> 12;
    let exp_low = biased & 0xFFF;
    let g = (exp_high << 15) | (lead << 12) | exp_low;
    let sign_bit: u128 = if sign { SIGN_MASK } else { 0 };
    sign_bit | ((g as u128) << 110) | trailing
}

/// Powers of 10 table for coefficients (indices 0..=34).
pub const POW10: [u128; 35] = [
    1,
    10,
    100,
    1_000,
    10_000,
    100_000,
    1_000_000,
    10_000_000,
    100_000_000,
    1_000_000_000,
    10_000_000_000,
    100_000_000_000,
    1_000_000_000_000,
    10_000_000_000_000,
    100_000_000_000_000,
    1_000_000_000_000_000,
    10_000_000_000_000_000,
    100_000_000_000_000_000,
    1_000_000_000_000_000_000,
    10_000_000_000_000_000_000,
    100_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000,
    100_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000_000,
    100_000_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000_000_000,
    100_000_000_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000_000_000_000,
    100_000_000_000_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000_000_000_000_000, // 10^34
];

/// Number of decimal digits in `n` (returns 1 for n=0).
///
/// Time: O(log₂(34)) = O(5) — binary search over the 35-entry POW10 table.
/// Space: O(1).
pub fn decimal_digits(n: u128) -> u32 {
    if n == 0 {
        return 1;
    }
    // Binary search over the powers table
    let mut lo = 1u32;
    let mut hi = 34u32;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if n < POW10[mid as usize] {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

/// Round to nearest, ties-to-even.
///
/// Uses `2 * remainder` vs `divisor` to avoid the fractional comparison trap:
/// `remainder == divisor/2` (integer division) produces false "exact half" results
/// for odd divisors (e.g. divisor=3, remainder=1 → 1/3 < 0.5 but floor(3/2)=1 looks equal).
///
/// Safe: remainder < divisor ≤ MAX_COEFFICIENT < 2^113, so 2*remainder < 2^114 << u128::MAX.
pub fn round_half_even(quotient: u128, remainder: u128, divisor: u128) -> u128 {
    let double_r = remainder.wrapping_mul(2);
    match double_r.cmp(&divisor) {
        std::cmp::Ordering::Less => quotient,
        std::cmp::Ordering::Greater => quotient + 1,
        std::cmp::Ordering::Equal => {
            if quotient % 2 != 0 {
                quotient + 1
            } else {
                quotient
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_zero() {
        // WHY: POS_ZERO is the bit pattern used by the codegen for `number` zero literals.
        // If encode/decode don't round-trip, every zero comparison breaks.
        let v = decode(POS_ZERO);
        assert_eq!(v.kind, D128Kind::Finite);
        assert!(!v.sign);
        assert_eq!(v.coefficient, 0);
    }

    #[test]
    fn encode_decode_one() {
        let bits = encode_finite(false, 0, 1);
        let v = decode(bits);
        assert_eq!(v.kind, D128Kind::Finite);
        assert!(!v.sign);
        assert_eq!(v.exponent, 0);
        assert_eq!(v.coefficient, 1);
    }

    #[test]
    fn encode_decode_negative() {
        let bits = encode_finite(true, -1, 15); // -1.5
        let v = decode(bits);
        assert!(v.sign);
        assert_eq!(v.exponent, -1);
        assert_eq!(v.coefficient, 15);
    }

    #[test]
    fn encode_decode_max_coefficient() {
        // WHY: the leading-digit-8/9 BID path is only exercised with large coefficients.
        let bits = encode_finite(false, 0, MAX_COEFFICIENT);
        let v = decode(bits);
        assert_eq!(v.coefficient, MAX_COEFFICIENT);
    }

    #[test]
    fn infinity_roundtrip() {
        let v = decode(POS_INF);
        assert_eq!(v.kind, D128Kind::Infinity);
        assert!(!v.sign);
    }

    #[test]
    fn nan_roundtrip() {
        let v = decode(QUIET_NAN);
        assert_eq!(v.kind, D128Kind::QuietNaN);
    }

    #[test]
    fn decimal_digits_values() {
        assert_eq!(decimal_digits(0), 1);
        assert_eq!(decimal_digits(9), 1);
        assert_eq!(decimal_digits(10), 2);
        assert_eq!(decimal_digits(99), 2);
        assert_eq!(decimal_digits(100), 3);
        assert_eq!(decimal_digits(MAX_COEFFICIENT), 34);
    }
}
