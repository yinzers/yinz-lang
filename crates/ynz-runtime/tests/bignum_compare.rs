//! v0.3-M9 C3a: the bignum runtime entry points that generated code calls for a comparison and
//! for unary minus. The compiled-program contract is `ynz-driver/tests/v03_m9_string_ownership.rs`;
//! this pins what a program cannot reach: the unordered (NaN) answer and parse failure.
//!
//! Tier: High (a comparison of money-sized numbers; text read as another type).

use std::ffi::{c_char, CStr, CString};

use ynz_runtime::{ynz_bignum_compare, ynz_bignum_neg, ynz_string_release};

fn compare(left: &str, right: &str) -> i32 {
    let left = CString::new(left).expect("no NUL in test text");
    let right = CString::new(right).expect("no NUL in test text");
    // SAFETY: both are valid NUL-terminated strings.
    unsafe { ynz_bignum_compare(left.as_ptr(), right.as_ptr(), 100) }
}

/// Bug caught: the compare answer is wrong or collapses "unordered" into an ordering, so
/// `NaN > x` or `NaN >= x` reads as true in generated code (-1 / 0 / 1 / 2). (High)
#[test]
fn compare_answers_less_equal_greater_and_unordered() {
    let cases = [
        ("9", "10", -1),
        ("10", "9", 1),
        ("2.5", "2.50", 0),
        ("-0", "0", 0),
        ("nan", "1", 2),
        ("1", "nan", 2),
        ("nan", "nan", 2),
    ];
    for (left, right, expected) in cases {
        assert_eq!(compare(left, right), expected, "compare({left}, {right})");
    }
}

/// Bug caught: negation returns something other than an owned heap string holding `0 - value`
/// (the decimal128 path returned a 16-byte stack cell). (High)
#[test]
fn negation_is_an_owned_heap_string() {
    for (value, expected) in [("2.5", "-2.5"), ("-2.5", "2.5"), ("0", "0")] {
        let value = CString::new(value).expect("no NUL in test text");
        // SAFETY: `value` is a valid NUL-terminated string; the result is a heap string that
        // is read once and released once.
        unsafe {
            let negated = ynz_bignum_neg(value.as_ptr(), 100);
            let negated_text = CStr::from_ptr(negated as *const c_char).to_str().unwrap();
            assert_eq!(negated_text, expected);
            ynz_string_release(negated as *const u8);
        }
    }
}
