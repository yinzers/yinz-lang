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

    /// `format(encode_finite(sign, exponent, coefficient))` is the text the program prints.
    macro_rules! format_finite_cases {
        ($($case:ident: $sign:literal, $exponent:literal, $coefficient:literal => $text:literal;)+) => {
            $(
                // bug: decimal128 format prints a finite number with the wrong digits, sign, decimal point or exponent padding | tier: High
                #[test]
                fn $case() {
                    assert_eq!(format(encode_finite($sign, $exponent, $coefficient)), $text);
                }
            )+
        };
    }

    format_finite_cases! {
        integer_prints_its_digits: false, 0, 42 => "42";
        zero_prints_as_zero: false, 0, 0 => "0";
        negative_prints_a_leading_minus: true, 0, 15 => "-15";
        fraction_prints_a_decimal_point: false, -2, 314 => "3.14";
        leading_fraction_zeros_are_padded: false, -4, 1 => "0.0001";
        positive_exponent_pads_trailing_zeros: false, 2, 1 => "100";
    }
}
