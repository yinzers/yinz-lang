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

    /// Parse then format: a literal must survive as the same number.
    fn roundtrip(s: &str) -> String {
        let bits = parse(s).unwrap();
        super::super::format::format(bits)
    }

    macro_rules! parse_round_trip_cases {
        ($($case:ident: $text:literal => $formatted:literal;)+) => {
            $(
                // bug: decimal128 parse reads a number literal as a different value, so every such literal in a Yinz program is wrong | tier: High
                #[test]
                fn $case() {
                    assert_eq!(roundtrip($text), $formatted);
                }
            )+
        };
    }

    parse_round_trip_cases! {
        zero_parses: "0" => "0";
        positive_integer_parses: "1" => "1";
        negative_integer_parses: "-1" => "-1";
        fraction_parses: "0.1" => "0.1";
        positive_exponent_parses: "1e5" => "100000";
        negative_exponent_parses: "2.5e-3" => "0.0025";
    }

    // bug: decimal128 parse rejects the text "Infinity" or reads it as a finite number | tier: High
    #[test]
    fn infinity_parses_as_infinity() {
        assert!(decode(parse("Infinity").unwrap()).is_infinity());
    }

    // bug: decimal128 parse rejects the text "NaN" or reads it as a finite number | tier: High
    #[test]
    fn nan_parses_as_nan() {
        assert!(decode(parse("NaN").unwrap()).is_nan());
    }

    macro_rules! parse_invalid_cases {
        ($($case:ident: $text:literal;)+) => {
            $(
                // bug: decimal128 parse accepts malformed text and returns a number instead of None | tier: High
                #[test]
                fn $case() {
                    assert!(parse($text).is_none());
                }
            )+
        };
    }

    parse_invalid_cases! {
        letters_are_rejected: "abc";
        empty_text_is_rejected: "";
        two_decimal_points_are_rejected: "1.2.3";
    }
}
