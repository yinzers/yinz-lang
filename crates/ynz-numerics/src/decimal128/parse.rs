/// Parse a decimal string into a decimal128 bit pattern.
///
/// Accepts the forms: `[+-]digits[.digits][[eE][+-]digits`
/// and special values: `Infinity`, `Inf`, `NaN` (case-insensitive).
///
/// Returns `None` if the string is not a valid decimal literal.
use super::bits::{
    encode_finite, encode_infinity, encode_qnan, round_half_even, MAX_COEFFICIENT, MAX_EXPONENT,
    MIN_EXPONENT,
};

pub fn parse(s: &str) -> Option<u128> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Sign
    let (s, negative) = if let Some(rest) = s.strip_prefix('-') {
        (rest, true)
    } else if let Some(rest) = s.strip_prefix('+') {
        (rest, false)
    } else {
        (s, false)
    };

    // Special values
    let lower = s.to_ascii_lowercase();
    if lower == "infinity" || lower == "inf" {
        return Some(encode_infinity(negative));
    }
    if lower == "nan" {
        return Some(encode_qnan(negative));
    }

    // Parse the numeric body: integer_part [. fractional_part] [e exponent]
    let (mantissa_str, exp_str) = if let Some(pos) = s.find(['e', 'E']) {
        (&s[..pos], Some(&s[pos + 1..]))
    } else {
        (s, None)
    };

    // Parse the explicit exponent
    let explicit_exp: i32 = if let Some(e) = exp_str {
        e.parse().ok()?
    } else {
        0
    };

    // Split on decimal point
    let (int_str, frac_str) = if let Some(pos) = mantissa_str.find('.') {
        (&mantissa_str[..pos], Some(&mantissa_str[pos + 1..]))
    } else {
        (mantissa_str, None)
    };

    // Build the full digit string and count fractional digits
    let frac_digits = frac_str.map(|s| s.len()).unwrap_or(0) as i32;
    let full_digits = format!("{}{}", int_str, frac_str.unwrap_or(""));

    if full_digits.is_empty() {
        return None;
    }
    if !full_digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    // Strip leading zeros (but keep at least one digit)
    let significant = full_digits.trim_start_matches('0');
    let significant = if significant.is_empty() {
        "0"
    } else {
        significant
    };

    // Adjust exponent: the decimal point is `frac_digits` from the right of full_digits
    // plus the explicit exponent.
    let base_exp = explicit_exp - frac_digits;

    // Convert the significant digits to a u128 coefficient, rounding to 34 digits
    let (coefficient, extra_exp) = digits_to_coefficient(significant);

    let exponent = base_exp + extra_exp;

    // Clamp exponent into representable range
    let exponent = exponent.clamp(MIN_EXPONENT, MAX_EXPONENT);

    Some(encode_finite(negative, exponent, coefficient))
}

/// Convert a decimal digit string (no leading zeros, no dot) to a (coefficient, extra_exp) pair.
///
/// If the string has > 34 digits, it is rounded to 34 and `extra_exp` is set accordingly.
fn digits_to_coefficient(digits: &str) -> (u128, i32) {
    if digits.len() <= 34 {
        let coef = digits.parse::<u128>().unwrap_or(0);
        return (coef, 0);
    }
    // Need to round. Take the first 35 chars for rounding purposes.
    let (head, tail) = digits.split_at(34);
    let mut coef: u128 = head.parse().unwrap_or(0);
    let first_dropped = tail.chars().next().unwrap().to_digit(10).unwrap() as u128;
    // Simplified round-half-even on the first dropped digit
    // (for parse, we only need to check if first_dropped >= 5)
    if first_dropped > 5 {
        coef += 1;
    } else if first_dropped == 5 && tail.len() > 1 {
        // There are more digits — it's > 0.5, round up
        coef += 1;
    } else if first_dropped == 5 {
        // Exactly half — round to even
        if coef % 2 != 0 {
            coef += 1;
        }
    }

    // If rounding caused overflow, adjust
    let extra_exp = if coef > MAX_COEFFICIENT {
        let (q, r) = (coef / 10, coef % 10);
        coef = round_half_even(q, r, 10);
        (digits.len() - 34 + 1) as i32
    } else {
        (digits.len() - 34) as i32
    };

    (coef, extra_exp)
}

#[cfg(test)]
mod tests {
    use super::super::bits::decode;
    use super::*;

    fn roundtrip(s: &str) -> String {
        let bits = parse(s).unwrap();
        super::super::format::format(bits)
    }

    #[test]
    fn parse_zero() {
        // WHY: zero is the identity element; if it doesn't parse, every zero
        // literal in a Yinz program produces an error.
        assert_eq!(roundtrip("0"), "0");
    }

    #[test]
    fn parse_one() {
        assert_eq!(roundtrip("1"), "1");
    }

    #[test]
    fn parse_negative() {
        assert_eq!(roundtrip("-1"), "-1");
    }

    #[test]
    fn parse_decimal() {
        assert_eq!(roundtrip("0.1"), "0.1");
        assert_eq!(roundtrip("3.14"), "3.14");
    }

    #[test]
    fn parse_scientific() {
        assert_eq!(roundtrip("1e5"), "100000");
        assert_eq!(roundtrip("2.5e-3"), "0.0025");
    }

    #[test]
    fn parse_infinity() {
        let bits = parse("Infinity").unwrap();
        let v = decode(bits);
        assert!(v.is_infinity());
    }

    #[test]
    fn parse_nan() {
        let bits = parse("NaN").unwrap();
        let v = decode(bits);
        assert!(v.is_nan());
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(parse("abc").is_none());
        assert!(parse("").is_none());
        assert!(parse("1.2.3").is_none()); // Two dots
    }
}
