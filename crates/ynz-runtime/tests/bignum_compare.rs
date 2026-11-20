//! v0.3-M9 C3a: the answer word of the bignum compare entry point that generated code branches on.
//! The compiled-program contract (orderings, mixed precision, release) is
//! `ynz-driver/tests/v03_m9_string_ownership.rs`; this pins the numeric answer codes, including
//! the unordered (NaN) one, which that fixture cannot show.
//!
//! Tier: High (a comparison of money-sized numbers; the codegen/runtime boundary).

use std::ffi::CString;

use ynz_runtime::ynz_bignum_compare;

/// One test per answer code: `compare_case!(case, left, right, answer, "bug it catches (High)")`.
macro_rules! compare_case {
    ($case:ident, $left:literal, $right:literal, $answer:literal, $bug:literal) => {
        #[doc = $bug]
        #[test]
        fn $case() {
            let left = CString::new($left).expect("no NUL in test text");
            let right = CString::new($right).expect("no NUL in test text");
            // SAFETY: both are valid NUL-terminated strings.
            let answer = unsafe { ynz_bignum_compare(left.as_ptr(), right.as_ptr(), 100) };
            assert_eq!(answer, $answer, "compare({}, {})", $left, $right);
        }
    };
}

compare_case!(
    less_is_minus_one_and_numeric,
    "9",
    "10",
    -1,
    "Bug caught: a textual compare (`\"9\" > \"10\"`) or a wrong code for less-than. (High)"
);

compare_case!(
    equal_is_zero_across_trailing_zeros,
    "2.5",
    "2.50",
    0,
    "Bug caught: `2.5` and `2.50` compare unequal, so `==` and a multi-case `if` miss. (High)"
);

compare_case!(
    greater_is_one,
    "10",
    "9",
    1,
    "Bug caught: a wrong code for greater-than. (High)"
);

compare_case!(
    unordered_is_two_not_an_ordering,
    "nan",
    "1",
    2,
    "Bug caught: the unordered answer collapses into an ordering, so `NaN > x` or `NaN >= x` reads as true in generated code. (High)"
);
