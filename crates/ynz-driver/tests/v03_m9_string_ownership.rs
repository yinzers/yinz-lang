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
