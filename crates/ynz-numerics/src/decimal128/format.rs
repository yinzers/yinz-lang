/// Format a decimal128 bit pattern as a decimal string.
///
/// Produces the shortest round-trip representation.
/// Special values: "Infinity", "-Infinity", "NaN".
use super::bits::{decode, D128Kind};

pub fn format(bits: u128) -> String {
    let v = decode(bits);
    match v.kind {
        D128Kind::Infinity => {
            if v.sign {
                "-Infinity".to_string()
            } else {
                "Infinity".to_string()
            }
        }
        D128Kind::QuietNaN | D128Kind::SignalingNaN => "NaN".to_string(),
        D128Kind::Finite => format_finite(v.sign, v.exponent, v.coefficient),
    }
}

fn format_finite(sign: bool, exponent: i32, coefficient: u128) -> String {
    if coefficient == 0 {
        if exponent >= 0 {
            return if sign {
                "-0".to_string()
            } else {
                "0".to_string()
            };
        } else {
            // e.g., exponent = -2 → "0.00"
            let zeros = "0".repeat((-exponent) as usize);
            let s = format!("0.{zeros}");
            return if sign { format!("-{s}") } else { s };
        }
    }

    let digits = format!("{coefficient}");
    let n = digits.len() as i32;

    // The coefficient represents `coefficient × 10^exponent`.
    // `adjusted_exp = n - 1 + exponent` is the scientific notation exponent.
    let s = if exponent >= 0 {
        // Integer result (trailing zeros implied by positive exponent)
        format!("{}{}", digits, "0".repeat(exponent as usize))
    } else if (-exponent) < n {
        // Decimal point inside the digit string
        let dot_pos = (n + exponent) as usize;
        let (int_part, frac_part) = digits.split_at(dot_pos);
        format!("{int_part}.{frac_part}")
    } else {
        // Decimal point before the digits (leading zeros after the dot)
        let leading = (-exponent - n) as usize;
        format!("0.{}{}", "0".repeat(leading), digits)
    };

    if sign {
        format!("-{s}")
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::super::bits::encode_finite;
    use super::*;

    #[test]
    fn format_integer() {
        // WHY: the smoke fixture prints an int-typed value via print();
        // the number codec must round-trip.
        let bits = encode_finite(false, 0, 42);
        assert_eq!(format(bits), "42");
    }

    #[test]
    fn format_zero() {
        let bits = encode_finite(false, 0, 0);
        assert_eq!(format(bits), "0");
    }

    #[test]
    fn format_negative() {
        let bits = encode_finite(true, 0, 15); // -15
        assert_eq!(format(bits), "-15");
    }

    #[test]
    fn format_decimal() {
        let bits = encode_finite(false, -2, 314); // 3.14
        assert_eq!(format(bits), "3.14");
    }

    #[test]
    fn format_small_decimal() {
        let bits = encode_finite(false, -4, 1); // 0.0001
        assert_eq!(format(bits), "0.0001");
    }

    #[test]
    fn format_with_trailing_exponent_zeros() {
        let bits = encode_finite(false, 2, 1); // 100
        assert_eq!(format(bits), "100");
    }
}
