//! v0.3-M9 C8 slice 4b: errors-capable values in a resume function, and the plain-path
//! `maybe<Union> errors` release the slice found broken (a C7 regression, `b669d67`). Each program
//! frees every allocation, clean under AddressSanitizer, at both optimization levels.

mod support;

use support::{at_both_optimization_levels, build, build_asan, run_asan_binary, run_counted};

/// Build the fixture and run it; it must print `expected`, exit 0, free every heap allocation and
/// every string, and run clean under AddressSanitizer.
fn assert_fixture_balanced(fixture: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let built = build(fixture, no_optimize);
        let (run, counters) = run_counted(&built);
        assert_eq!(
            run.exit_code, 0,
            "{fixture} (no_optimize={no_optimize}) must exit 0; stderr:\n{}",
            run.stderr
        );
        assert_eq!(
            run.stdout, expected,
            "{fixture} (no_optimize={no_optimize})"
        );
        assert_eq!(
            counters.free, counters.alloc,
            "{fixture} (no_optimize={no_optimize}): every heap allocation must be freed: {counters:?}"
        );
        assert_eq!(
            counters.string_free, counters.string_alloc,
            "{fixture} (no_optimize={no_optimize}): every string must be freed: {counters:?}"
        );
        let sanitized = run_asan_binary(&build_asan(&built));
        assert!(
            !sanitized.stderr.contains("AddressSanitizer") && sanitized.exit_code == 0,
            "{fixture} (no_optimize={no_optimize}): AddressSanitizer reported an error:\n{}",
            sanitized.stderr
        );
    });
}

#[test]
fn a_plain_maybe_union_errors_hit_releases_its_view() {
    assert_fixture_balanced(
        "v0_3_m9_c8_s4b_maybe_union_errors_plain_hit.ynz",
        "big kept\nend\n",
    );
}

#[test]
fn a_plain_maybe_union_errors_miss_releases_nothing_it_does_not_own() {
    assert_fixture_balanced(
        "v0_3_m9_c8_s4b_maybe_union_errors_plain_miss.ynz",
        "none\nend\n",
    );
}

#[test]
fn a_plain_errors_local_of_every_inner_type_is_released_with_its_own_form() {
    assert_fixture_balanced(
        "v0_3_m9_c8_s4b_errors_inner_types_plain.ynz",
        &"false\n".repeat(9),
    );
}
