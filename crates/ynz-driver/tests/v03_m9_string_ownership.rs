//! v0.3-M9 C3a: string ownership at the compiled-program boundary.
//!
//! A `string` is a pointer to NUL-terminated bytes with an 8-byte header in front (0 for static
//! bytes, the byte capacity for owned bytes); `ynz_string_release` frees only the owned ones.
//! These tests run whole programs; the runtime-level contract is `ynz-runtime/tests/string_release.rs`.
//!
//! All three are High tier (memory safety: a string freed twice, freed while static, or aliased).

mod support;

use support::{assert_asan_clean_and_prints, assert_fixture_prints, build, run_counted};

/// Bug caught: `int`/`float`/`number` `.toString()` returns a pointer into one shared
/// thread-local buffer, so two live results both show the last value (`222 222`). (High)
#[test]
fn to_string_results_are_distinct_strings() {
    for no_optimize in [false, true] {
        assert_fixture_prints(
            "v0_3_m9_c3a_to_string_distinct_results.ynz",
            no_optimize,
            "111\n222\n7\n9\n3.25\n4.75\n",
            "two live toString results must hold two values",
        );
    }
}

/// Bug caught: releasing the temporary text of a printed/interpolated number frees static bytes
/// (a literal, `true`, an options display name) or frees the same owned bytes twice. (High)
#[test]
fn string_sources_release_only_owned_bytes_under_asan() {
    for no_optimize in [false, true] {
        assert_asan_clean_and_prints(
            "v0_3_m9_c3a_string_sources_asan.ynz",
            no_optimize,
            "42\n3\n9.5\ntrue\nhigh\nliteral\nyinz-42-3-9.5-true-high\nPoint { x: 7, y: 3 }\n42\ntrue\n",
        );
    }
}

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
