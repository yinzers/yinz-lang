//! v0.3-M9 C3a: string ownership at the compiled-program boundary.
//!
//! A `string` is a pointer to NUL-terminated bytes with an 8-byte header in front (0 for static
//! bytes, the byte capacity for owned bytes); `ynz_string_release` frees only the owned ones.
//! These tests run whole programs; the runtime-level contract is `ynz-runtime/tests/string_release.rs`.
//!
//! All are High tier (memory safety: a string freed twice, freed while static, or aliased; and
//! number arithmetic whose operands are strings). Every case is one named test per optimization
//! level, so the runner parallelizes the builds (`.claude/rules/test-parallelism.md`).

mod support;

use support::{assert_asan_clean_and_prints, assert_fixture_prints, build, run_counted};

/// `plain_case!(case, fixture, expected_stdout, "what a failure means", "bug it catches (High)")`:
/// builds and runs the fixture, optimized and at `--no-optimize`, and asserts its stdout.
macro_rules! plain_case {
    ($case:ident, $fixture:literal, $expected:literal, $guards:literal, $bug:literal) => {
        #[doc = $bug]
        mod $case {
            use crate::assert_fixture_prints;

            #[test]
            fn optimized() {
                assert_fixture_prints($fixture, false, $expected, $guards);
            }

            #[test]
            fn no_optimize() {
                assert_fixture_prints($fixture, true, $expected, $guards);
            }
        }
    };
}

/// `asan_case!(case, fixture, expected_stdout, "bug it catches (High)")`: the same under
/// AddressSanitizer, which must report nothing.
macro_rules! asan_case {
    ($case:ident, $fixture:literal, $expected:literal, $bug:literal) => {
        #[doc = $bug]
        mod $case {
            use crate::assert_asan_clean_and_prints;

            #[test]
            fn optimized() {
                assert_asan_clean_and_prints($fixture, false, $expected);
            }

            #[test]
            fn no_optimize() {
                assert_asan_clean_and_prints($fixture, true, $expected);
            }
        }
    };
}

plain_case!(
    to_string_results_are_distinct,
    "v0_3_m9_c3a_to_string_distinct_results.ynz",
    "111\n222\n7.5\n9\n3.25\n4.75\n",
    "two live toString results must hold two values",
    "`int`/`float`/`number` `.toString()` returns a pointer into one shared thread-local buffer, so two live results both show the last value (`222 222`) (High)"
);

plain_case!(
    float_literal_is_an_f64_in_every_position,
    "v0_3_m9_c3a_float_literal_contexts.ynz",
    "0.317\n0.75\n0.5\n0.125\n2\n",
    "a decimal literal under a `float` type must be an f64 constant",
    "a decimal literal where typeck wants a `float` (annotation, shape field, argument, return) lowers to a decimal128 cell address used as the f64 bits, printing a denormal (High: wrong numeric value)"
);

plain_case!(
    mixed_precision_binop_uses_the_bignum_operation,
    "v0_3_m9_c3a_mixed_precision_binop.ynz",
    "4\n4\n-1\n3.75\n0.6\n4\n4\n4\n4\n-2.5\n",
    "`number<=34> op number>34` must run the bignum operation on bignum text",
    "`small op big` runs the decimal128 operation on the coerced bignum text: a wrong result (`2.5` for `1.5 + 2.5`) and a 16-byte read past an exact-size heap string (High: arithmetic, memory safety)"
);

asan_case!(
    mixed_precision_binop_releases_coerced_operands_once,
    "v0_3_m9_c3a_mixed_precision_binop.ynz",
    "4\n4\n-1\n3.75\n0.6\n4\n4\n4\n4\n-2.5\n",
    "releasing the bignum text a mixed-precision binop coerced frees it twice, frees a held string, or leaks past the operation (High)"
);

asan_case!(
    string_sources_release_only_owned_bytes,
    "v0_3_m9_c3a_string_sources_asan.ynz",
    "42\n3\n9.5\ntrue\nhigh\nliteral\nyinz-42-3-9.5-true-high\nPoint { x: 7, y: 3 }\n42\ntrue\n",
    "releasing the temporary text of a printed/interpolated number frees static bytes (a literal, `true`, an options display name) or frees the same owned bytes twice (High)"
);

asan_case!(
    debug_text_releases_only_owned_field_text,
    "v0_3_m9_c3a_debug_text_asan.ynz",
    "Bag { ints: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, ... and 3 more], floats: [1.5, 2.25], scores: { alice: 90 }, inner: Reading { count: 3, ratio: 0.5, price: 1.25, level: low, note: hi } }\n",
    "the debug print of a shape releases a field's, element's or `... and N more` text twice, or releases a static display name / string field (High)"
);

/// Bug caught: `int`/`float`/`number` text that used to live in a thread-local buffer is now heap,
/// so a `print`/`${}` that does not release its temporary leaks one string per use. (High)
#[test]
fn printing_and_interpolating_numbers_releases_their_temporaries() {
    const ITERATIONS: u64 = 1000;
    let built = build("v0_3_m9_c3a_print_temps_balance.ynz", false);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "stderr:\n{}", output.stderr);
    assert!(
        output.stdout.ends_with("1000\n"),
        "loop must run to the end:\n{}",
        output.stdout
    );
    // Three temporaries + one interpolation result per iteration are allocated, so this is not
    // vacuous; only the interpolation result may still be allocated (the C7 pass frees it).
    assert!(
        counters.string_alloc >= 4 * ITERATIONS,
        "expected at least {} string allocations, saw {}",
        4 * ITERATIONS,
        counters.string_alloc
    );
    let still_allocated = counters.string_alloc - counters.string_free;
    assert!(
        still_allocated <= ITERATIONS,
        "{still_allocated} strings stayed allocated after {ITERATIONS} iterations; only the \
         {ITERATIONS} interpolation results may (counters: {counters:?})"
    );
}

// ---- bignum text must never be read as decimal128: comparisons, unary minus, `if` on a value ----

plain_case!(
    bignum_compared_with_bignum_is_an_exact_decimal_compare,
    "v0_3_m9_c3a_bignum_compare_big_big.ynz",
    "true\ntrue\nfalse\ntrue\nfalse\nfalse\nfalse\nfalse\nfalse\ntrue\ntrue\ntrue\nfalse\ntrue\ntrue\nfalse\nfalse\ntrue\n",
    "`number>34 <op> number>34` must compare the decimal text exactly",
    "a comparison between two bignums reads their text as 16 decimal128 bytes: `2.5 < 12.5` prints false and the read runs past a short heap string (High: wrong comparison, memory safety)"
);

plain_case!(
    small_number_compared_with_bignum_is_an_exact_decimal_compare,
    "v0_3_m9_c3a_bignum_compare_small_big.ynz",
    "true\ntrue\nfalse\ntrue\nfalse\nfalse\nfalse\ntrue\ntrue\nfalse\nfalse\ntrue\nfalse\nfalse\nfalse\ntrue\ntrue\ntrue\n",
    "`number<=34 <op> number>34` must compare as bignum text",
    "`small < big` coerces the small side to bignum text then reads it as decimal128 (High: wrong comparison, memory safety)"
);

plain_case!(
    bignum_compared_with_small_number_is_an_exact_decimal_compare,
    "v0_3_m9_c3a_bignum_compare_big_small.ynz",
    "false\nfalse\nfalse\ntrue\ntrue\ntrue\nfalse\ntrue\ntrue\nfalse\nfalse\ntrue\ntrue\ntrue\nfalse\ntrue\nfalse\nfalse\n",
    "`number>34 <op> number<=34` must compare as bignum text",
    "`big < small` reads the bignum text as decimal128 (High: wrong comparison, memory safety)"
);

asan_case!(
    bignum_compare_of_short_text_stays_inside_the_string,
    "v0_3_m9_c3a_bignum_compare_short_text.ynz",
    "true\ntrue\nfalse\ntrue\nfalse\nfalse\nfalse\nfalse\nfalse\ntrue\ntrue\ntrue\ntrue\ntrue\nfalse\ntrue\nfalse\nfalse\nfalse\nfalse\nfalse\ntrue\ntrue\ntrue\n",
    "a bignum of 4 bytes of text (`9.0`) compared as 16 decimal128 bytes reads past its exact-size heap string (High: heap over-read)"
);

asan_case!(
    mixed_precision_compare_releases_coerced_operands_once,
    "v0_3_m9_c3a_bignum_compare_small_big.ynz",
    "true\ntrue\nfalse\ntrue\nfalse\nfalse\nfalse\ntrue\ntrue\nfalse\nfalse\ntrue\nfalse\nfalse\nfalse\ntrue\ntrue\ntrue\n",
    "the bignum text a `small <op> big` compare coerced is freed twice, freed while held, or not freed (High)"
);

asan_case!(
    big_vs_small_compare_releases_coerced_operands_once,
    "v0_3_m9_c3a_bignum_compare_big_small.ynz",
    "false\nfalse\nfalse\ntrue\ntrue\ntrue\nfalse\ntrue\ntrue\nfalse\nfalse\ntrue\ntrue\ntrue\nfalse\ntrue\nfalse\nfalse\n",
    "the bignum text a `big <op> small` compare coerced is freed twice, freed while held, or not freed (High)"
);

plain_case!(
    negating_a_bignum_negates_its_decimal_text,
    "v0_3_m9_c3a_bignum_negate.ynz",
    "-2.5\n2.5\n-12.5\n0\n",
    "`-value` on a `number>34` must return the negated bignum text",
    "unary minus on a bignum runs decimal128 `0 - value` on its text and returns a 16-byte stack cell as bignum text: `-2.5` is not printed (High: wrong arithmetic, memory safety)"
);

asan_case!(
    negating_a_bignum_returns_an_owned_string,
    "v0_3_m9_c3a_bignum_negate.ynz",
    "-2.5\n2.5\n-12.5\n0\n",
    "a negated bignum that is a stack cell, not a heap string, is read or released as one (High)"
);

plain_case!(
    multi_case_if_on_a_bignum_compares_decimal_text,
    "v0_3_m9_c3a_bignum_branch_on_value.ynz",
    "two-five\nten\nother\n",
    "an `if` arm on a `number>34` value must match numerically equal text (`2.5` and `2.50`)",
    "the arm test reads the bignum text as decimal128, so `2.5` does not match the arm `2.50` (High: wrong branch)"
);

plain_case!(
    shape_debug_print_reads_a_bignum_field_as_text,
    "v0_3_m9_c3a_bignum_in_shape_debug_text.ynz",
    "Ledger { total: 4, entries: [1.5, 2.5], byName: { rent: 2.5 } }\n",
    "a `number>34` field of a printed shape must show its decimal text",
    "the shape debug print loads a bignum field (an 8-byte pointer) as a 16-byte decimal128: garbage text and a read past the field (High: memory safety)"
);

asan_case!(
    shape_debug_print_of_a_bignum_field_is_asan_clean,
    "v0_3_m9_c3a_bignum_in_shape_debug_text.ynz",
    "Ledger { total: 4, entries: [1.5, 2.5], byName: { rent: 2.5 } }\n",
    "the debug print of a bignum field reads past the field, or releases the borrowed bignum text (High)"
);
