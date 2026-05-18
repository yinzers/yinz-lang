use super::bignum::BigNum;

/// Parse a decimal string into a BigNum at the given precision.
///
/// Accepts: `[+-]? [0-9]* [.] [0-9]* ([eE] [+-]? [0-9]+)?`
/// Returns None on invalid input.
pub fn parse_str(s: &str, precision: u16) -> Option<BigNum> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Handle special values
    let lower = s.to_lowercase();
    if lower == "inf" || lower == "infinity" || lower == "+inf" {
        return Some(BigNum {
            precision,
            sign: false,
            digits: vec![],
            exponent: 0,
            is_infinity: true,
            is_nan: false,
        });
    }
    if lower == "-inf" || lower == "-infinity" {
        return Some(BigNum {
            precision,
            sign: true,
            digits: vec![],
            exponent: 0,
            is_infinity: true,
            is_nan: false,
        });
    }
    if lower == "nan" {
        return Some(BigNum {
            precision,
            sign: false,
            digits: vec![],
            exponent: 0,
            is_infinity: false,
            is_nan: true,
        });
    }

    let bytes = s.as_bytes();
    let mut pos = 0;

    // Sign
    let sign = if pos < bytes.len() && bytes[pos] == b'-' {
        pos += 1;
        true
    } else if pos < bytes.len() && bytes[pos] == b'+' {
        pos += 1;
        false
    } else {
        false
    };

    // Collect integer and fractional digits
    let mut digits: Vec<u8> = Vec::new();
    let mut dot_pos: Option<usize> = None;

    while pos < bytes.len() {
        match bytes[pos] {
            b'0'..=b'9' => {
                digits.push(bytes[pos] - b'0');
                pos += 1;
            }
            b'.' if dot_pos.is_none() => {
                dot_pos = Some(digits.len());
                pos += 1;
            }
            b'_' => {
                pos += 1; // skip underscores
            }
            b'e' | b'E' => break,
            _ => return None,
        }
    }

    // Exponent from position of decimal point
    let frac_len = match dot_pos {
        Some(dp) => digits.len() - dp,
        None => 0,
    };
    let mut exponent = -(frac_len as i64);

    // Parse explicit exponent if present
    if pos < bytes.len() && (bytes[pos] == b'e' || bytes[pos] == b'E') {
        pos += 1;
        let exp_sign = if pos < bytes.len() && bytes[pos] == b'-' {
            pos += 1;
            -1i64
        } else if pos < bytes.len() && bytes[pos] == b'+' {
            pos += 1;
            1i64
        } else {
            1i64
        };
        let mut exp_val = 0i64;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            exp_val = exp_val * 10 + (bytes[pos] - b'0') as i64;
            pos += 1;
        }
        exponent += exp_sign * exp_val;
    }

    if pos < bytes.len() {
        return None; // trailing garbage
    }

    if digits.is_empty() {
        return None;
    }

    let mut bn = BigNum {
        precision,
        sign,
        digits,
        exponent,
        is_infinity: false,
        is_nan: false,
    };
    // Remove leading zeros and round to precision
    while bn.digits.len() > 1 && bn.digits[0] == 0 {
        bn.digits.remove(0);
    }
    bn.round_to_precision();
    Some(bn)
}
