//! v0.3-M9 C3a: `ynz_string_release` frees a string's owned bytes and never static ones.
//!
//! Tier: High (memory safety). Run under AddressSanitizer (the `sanitizers` CI lane,
//! `RUSTFLAGS=-Zsanitizer=address cargo +nightly test -p ynz-runtime`): a release that frees
//! static bytes is an invalid free, a second release of owned bytes is a double free, and a
//! string that a conversion left in a shared buffer is an invalid free too. Without the
//! sanitizer the same tests still check the bytes survive.

use std::ffi::CStr;

use ynz_runtime::{
    ynz_bignum_add, ynz_decimal_from_int, ynz_decimal_to_string, ynz_float_to_string,
    ynz_int_to_string, ynz_sensitive_to_string, ynz_string_builder_append,
    ynz_string_builder_finalize, ynz_string_builder_new, ynz_string_concat, ynz_string_from_static,
    ynz_string_release, ynz_string_to_upper, StaticString,
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

/// Bug caught: an owned string from some allocator that `ynz_string_release` cannot free (it
/// lacks the header, so the release reads the wrong word) or frees twice. One row per allocator
/// that returns a string. (High)
#[test]
fn releasing_owned_bytes_from_every_allocator_is_clean() {
    let mut decimal = [0u8; 16];
    ynz_decimal_from_int(7, decimal.as_mut_ptr().cast());
    let owned: Vec<(&str, *const u8, &str)> = unsafe {
        vec![
            (
                "from_static",
                ynz_string_from_static(b"abc".as_ptr(), 3),
                "abc",
            ),
            (
                "concat",
                ynz_string_concat(c"foo".as_ptr().cast(), c"bar".as_ptr().cast()),
                "foobar",
            ),
            (
                "to_upper",
                ynz_string_to_upper(c"abc".as_ptr().cast()),
                "ABC",
            ),
            ("int_to_string", ynz_int_to_string(-12), "-12"),
            ("float_to_string", ynz_float_to_string(2.5), "2.5"),
            (
                "decimal_to_string",
                ynz_decimal_to_string(decimal.as_ptr().cast()),
                "7",
            ),
            (
                "bignum_add",
                ynz_bignum_add(c"1".as_ptr().cast(), c"2".as_ptr().cast(), 40) as *const u8,
                "3",
            ),
            (
                "builder_finalize",
                {
                    let builder = ynz_string_builder_new();
                    ynz_string_builder_append(builder, c"x".as_ptr().cast());
                    ynz_string_builder_finalize(builder)
                },
                "x",
            ),
        ]
    };
    for (allocator, string, expected) in owned {
        assert_eq!(text(string), expected, "{allocator}");
        unsafe { ynz_string_release(string) };
    }
}

/// Bug caught: the `int` conversion's hand-written digit loop is wrong at a boundary (zero,
/// sign, `i64::MIN` whose magnitude does not fit an i64, the 20-byte maximum). (High)
#[test]
fn int_to_string_is_exact_at_the_boundaries() {
    let table: [(i64, &str); 7] = [
        (0, "0"),
        (9, "9"),
        (10, "10"),
        (-1, "-1"),
        (i64::MAX, "9223372036854775807"),
        (i64::MIN, "-9223372036854775808"),
        (1_000_000, "1000000"),
    ];
    for (value, expected) in table {
        let string = ynz_int_to_string(value);
        assert_eq!(text(string), expected);
        unsafe { ynz_string_release(string) };
    }
}

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
