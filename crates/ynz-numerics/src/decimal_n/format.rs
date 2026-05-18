use super::bignum::BigNum;

/// Format a BigNum as a decimal string.
///
/// Uses the same format as Python's `str(Decimal(...))`:
/// - Coefficient × 10^exponent presentation
/// - Scientific notation when exponent is very large or very small
pub fn format_bignum(bn: &BigNum) -> String {
    if bn.is_nan {
        return "NaN".to_string();
    }
    if bn.is_infinity {
        return if bn.sign { "-Infinity" } else { "Infinity" }.to_string();
    }

    let sign = if bn.sign { "-" } else { "" };

    if bn.digits.is_empty() || bn.digits.iter().all(|&d| d == 0) {
        return format!("{sign}0");
    }

    // Convert digits to string
    let coeff: String = bn.digits.iter().map(|&d| (b'0' + d) as char).collect();
    let num_digits = coeff.len() as i64;
    let exp = bn.exponent;

    // The value is coeff × 10^exp where coeff has `num_digits` digits.
    // The "adjusted exponent" (position of the most significant digit) is:
    let adjusted = exp + num_digits - 1;

    if exp >= 0 {
        // Integer or needs trailing zeros: e.g., "123" with exp=2 → "12300"
        let zeros = exp as usize;
        if zeros <= 6 {
            return format!("{sign}{coeff}{}", "0".repeat(zeros));
        }
        // Too many trailing zeros — use scientific notation
        format!("{sign}{}e+{}", &coeff, adjusted)
    } else if adjusted >= 0 {
        // Has a decimal point within the digits: e.g., "12345" exp=-2 → "123.45"
        let int_len = (num_digits + exp) as usize; // digits before decimal
        let frac = &coeff[int_len..];
        let int_part = &coeff[..int_len];
        if frac.is_empty() {
            format!("{sign}{int_part}")
        } else {
            format!("{sign}{int_part}.{frac}")
        }
    } else {
        // Adjusted < 0: needs leading "0.000...": e.g., "123" exp=-6 → "0.000123"
        let leading_zeros = (-adjusted - 1) as usize;
        if leading_zeros <= 8 {
            format!("{sign}0.{}{coeff}", "0".repeat(leading_zeros))
        } else {
            // Very small — use scientific notation
            let (first, rest) = coeff.split_at(1);
            if rest.is_empty() {
                format!("{sign}{first}E{adjusted}")
            } else {
                format!("{sign}{first}.{rest}E{adjusted}")
            }
        }
    }
}
