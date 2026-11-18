//! `ynz_bignum_to_float` backs `.toFloat()` / `.toInt()` on `number<N>`, N > 34.
//! Tier: High (parser). Bugs caught: a malformed bignum string silently becoming 0 (see
//! `unparseable_text_is_nan_not_zero`), and a well-formed one rounding to the wrong f64.

use std::ffi::CString;
use ynz_runtime::ynz_bignum_to_float;

fn convert(text: &str) -> f64 {
    let c_text = CString::new(text).unwrap();
    unsafe { ynz_bignum_to_float(c_text.as_ptr()) }
}

#[test]
fn bignum_text_converts_to_nearest_f64() {
    // Bug caught: wrong f64 for well-formed bignum text (sign, fraction, zero, 40-digit input).
    assert_eq!(convert("12345.5"), 12345.5);
    assert_eq!(convert("-0.25"), -0.25);
    assert_eq!(convert("0"), 0.0);
    assert_eq!(
        convert("0.3333333333333333333333333333333333333333"),
        1.0 / 3.0
    );
}

#[test]
fn unparseable_text_is_nan_not_zero() {
    // Bug caught: unparseable text silently converting to 0.0.
    assert!(convert("not a number").is_nan());
    assert!(convert("").is_nan());
}
