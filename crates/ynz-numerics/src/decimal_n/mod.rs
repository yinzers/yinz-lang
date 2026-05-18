/// Arbitrary-precision decimal arithmetic for `number<N>` with N ∈ (34, 4096].
///
/// Uses digit-vector representation with half-even rounding.
/// The N ≤ 34 hardware path in `decimal128` is unaffected by this module.
pub mod bignum;
pub mod format;
pub mod ops;
pub mod parse;

pub use bignum::BigNum;

/// Parse a bignum string at the given precision.
pub fn parse_bignum(s: &str, precision: u16) -> Option<BigNum> {
    BigNum::from_str(s, precision)
}

/// Format a bignum to its decimal string representation.
pub fn format_bignum(bn: &BigNum) -> String {
    bn.to_string_repr()
}

/// Add two BigNums with result precision = max(a.precision, b.precision).
pub fn add(a: &BigNum, b: &BigNum) -> BigNum {
    ops::add(a, b)
}

/// Subtract: a - b.
pub fn sub(a: &BigNum, b: &BigNum) -> BigNum {
    ops::sub(a, b)
}

/// Multiply.
pub fn mul(a: &BigNum, b: &BigNum) -> BigNum {
    ops::mul(a, b)
}

/// Divide a / b.
pub fn div(a: &BigNum, b: &BigNum) -> BigNum {
    ops::div(a, b)
}

/// Promote a decimal128 value to bignum at the given precision.
pub fn promote_from_d128(value_str: &str, precision: u16) -> BigNum {
    BigNum::from_str(value_str, precision).unwrap_or_else(|| BigNum::zero(precision))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_01_plus_02() {
        let a = parse_bignum("0.1", 100).unwrap();
        let b = parse_bignum("0.2", 100).unwrap();
        let c = add(&a, &b);
        let s = format_bignum(&c);
        assert_eq!(s, "0.3", "0.1 + 0.2 must equal 0.3, got: {s}");
    }
}
