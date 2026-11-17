//! v0.3-M9 C8 slice 5: the spawn ladder and the leaks it left. Each program frees every
//! allocation, clean under AddressSanitizer, at both optimization levels.

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

/// Bug caught (High: leak): the value half of a `MapEntry` spawn argument (a shape cell and its
/// string) was never freed, for a plain task and for a suspending one.
#[test]
fn a_mapentry_spawn_argument_frees_its_shape_value_and_string() {
    assert_fixture_balanced("v0_3_m9_c8_s5_mapentry_shape_string_spawn.ynz", "a1\na1\n");
}

/// Bug caught (High: leak): the decimal key and value cells of a `MapEntry` spawn argument.
#[test]
fn a_mapentry_spawn_argument_frees_its_decimal_key_and_value_cells() {
    assert_fixture_balanced("v0_3_m9_c8_s5_mapentry_decimal_spawn.ynz", "2.5\n");
}
