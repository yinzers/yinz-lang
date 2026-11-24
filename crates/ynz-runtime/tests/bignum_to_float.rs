//! `ynz_bignum_to_float` backs `.toFloat()` / `.toInt()` on `number<N>`, N > 34.
//! Tier: High (parser). Bugs caught: a malformed bignum string silently becoming 0 (see
//! `unparseable_text_is_nan_not_zero`), and a well-formed one rounding to the wrong f64.

use std::ffi::CString;
use ynz_runtime::ynz_bignum_to_float;

fn convert(text: &str) -> f64 {
    let c_text = CString::new(text).unwrap();
    unsafe { ynz_bignum_to_float(c_text.as_ptr()) }
}

macro_rules! bignum_to_float_cases {
    ($($case:ident: $text:literal => $expected:expr;)+) => {
        $(
            // bug: ynz_bignum_to_float returns the wrong f64 for well-formed bignum text (sign, fraction, zero, 40-digit input), so `.toFloat()` and `.toInt()` on number<N> are wrong | tier: High
            #[test]
            fn $case() {
                assert_eq!(convert($text), $expected);
            }
        )+
    };
}

bignum_to_float_cases! {
    positive_fraction_converts_exactly: "12345.5" => 12345.5;
    negative_fraction_converts_exactly: "-0.25" => -0.25;
    zero_converts_to_zero: "0" => 0.0;
    forty_digit_input_rounds_to_the_nearest_f64: "0.3333333333333333333333333333333333333333" => 1.0 / 3.0;
}

macro_rules! unparseable_bignum_text_cases {
    ($($case:ident: $text:literal;)+) => {
        $(
            // bug: ynz_bignum_to_float turns unparseable text silently into 0.0 instead of NaN | tier: High
            #[test]
            fn $case() {
                assert!(convert($text).is_nan());
            }
        )+
    };
}

unparseable_bignum_text_cases! {
    words_are_nan_not_zero: "not a number";
    empty_text_is_nan_not_zero: "";
}
