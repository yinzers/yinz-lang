//! v0.3-M9 C3a: `ynz_string_release` frees a string's owned bytes and never static ones, and the
//! owned `toString` conversions return independent strings.
//!
//! Tier: High (memory safety). Run under AddressSanitizer (the `sanitizers` CI lane,
//! `RUSTFLAGS=-Zsanitizer=address cargo +nightly test -p ynz-runtime`): a release that frees
//! static bytes is an invalid free, a second release of owned bytes is a double free, and a
//! string that a conversion left in a shared buffer is an invalid free too. Without the
//! sanitizer the same tests still check the bytes survive.
//!
//! That every allocator writes the header and is released clean is pinned once, for all of them,
//! by `string_counter.rs` (they share one allocation point, `heap_string_buffer`).

use std::ffi::CStr;

use ynz_runtime::{
    ynz_float_to_string, ynz_int_to_string, ynz_sensitive_to_string, ynz_string_release,
    StaticString,
};

/// A literal as codegen emits it: header word 0, then the bytes and a NUL.
static LITERAL: StaticString<6> = StaticString::new(*b"hello\0");

fn text<'string>(string: *const u8) -> &'string str {
    // SAFETY: every caller passes a live NUL-terminated string.
    unsafe { CStr::from_ptr(string as *const i8) }
        .to_str()
        .expect("test strings are UTF-8")
}

/// Bug caught: a release that frees a literal's (or `[REDACTED]`'s) bytes: an invalid free
/// (SEGV on .rodata), the S1 crash of t01/t31/t32. (High)
#[test]
fn releasing_static_bytes_leaves_them_untouched() {
    let literal = LITERAL.string_pointer();
    unsafe {
        ynz_string_release(literal);
        ynz_string_release(literal);
    }
    assert_eq!(
        text(literal),
        "hello",
        "a literal survives any number of releases"
    );

    // Without YNZ_REVEAL_SENSITIVE the sensitive-to-string result is the static redaction.
    let redacted = unsafe { ynz_sensitive_to_string(literal) };
    assert_ne!(redacted, literal, "the test needs the redacted branch");
    unsafe { ynz_string_release(redacted) };
    assert_eq!(text(redacted), "[REDACTED]");
}

/// One test per boundary of the `int` conversion's hand-written digit loop:
/// `int_to_string_case!("bug it catches (High)"; case: value => "text", ...)`.
macro_rules! int_to_string_case {
    ($bug:literal; $($case:ident: $value:expr => $expected:expr,)+) => {
        $(
            #[doc = $bug]
            #[test]
            fn $case() {
                let string = ynz_int_to_string($value);
                assert_eq!(text(string), $expected);
                unsafe { ynz_string_release(string) };
            }
        )+
    };
}

int_to_string_case!(
    "Bug caught: the `int` conversion's digit loop is wrong at a boundary: zero, a carry into a new digit, the sign, `i64::MIN` (whose magnitude does not fit an i64), the 20-byte maximum. (High)";
    int_to_string_of_zero: 0 => "0",
    int_to_string_of_ten: 10 => "10",
    int_to_string_of_minus_one: -1 => "-1",
    int_to_string_of_i64_max: i64::MAX => "9223372036854775807",
    int_to_string_of_i64_min: i64::MIN => "-9223372036854775808",
);

/// Bug caught: two conversions alive at once share one buffer (the `222 222` bug, S1 t06). (High)
#[test]
fn two_live_conversions_hold_two_values() {
    let first = ynz_int_to_string(111);
    let second = ynz_int_to_string(222);
    let first_float = ynz_float_to_string(7.0);
    let second_float = ynz_float_to_string(9.0);
    assert_ne!(first, second);
    assert_eq!((text(first), text(second)), ("111", "222"));
    assert_eq!((text(first_float), text(second_float)), ("7", "9"));
    for string in [first, second, first_float, second_float] {
        unsafe { ynz_string_release(string) };
    }
}

/// One test per shape of long float text, which the old 31-byte thread-local buffer cut (and
/// that hid a wrong float literal behind a truncated denormal):
/// `float_to_string_case!("bug it catches (High)"; case: value => expected_text, ...)`.
macro_rules! float_to_string_case {
    ($bug:literal; $($case:ident: $value:expr => $expected:expr,)+) => {
        $(
            #[doc = $bug]
            #[test]
            fn $case() {
                let string = ynz_float_to_string($value);
                assert_eq!(text(string), $expected);
                unsafe { ynz_string_release(string) };
            }
        )+
    };
}

float_to_string_case!(
    "Bug caught: a float's text is cut to a fixed buffer again. (High)";
    float_to_string_keeps_all_301_digits_of_1e300: 1e300 => format!("1{}", "0".repeat(300)),
    float_to_string_keeps_the_long_fraction_of_the_smallest_subnormal: 5e-324 => format!("0.{}5", "0".repeat(323)),
);
