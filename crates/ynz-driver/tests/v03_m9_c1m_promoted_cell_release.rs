//! v0.3-M9 C1m (card #494): a value held in a promoted union or `maybe` cell is released once by
//! the one release helper, whether it is a `fixed<string>` local or a `maybe<array<int>>` return.

mod support;

use support::{
    assert_built_balanced_and_asan_clean_printing_one_of, at_both_optimization_levels, build,
};

fn assert_fixture_balanced(fixture: &str, accepted: &[&str]) {
    at_both_optimization_levels(|no_optimize| {
        let built = build(fixture, no_optimize);
        assert_built_balanced_and_asan_clean_printing_one_of(
            fixture,
            &built,
            accepted,
            no_optimize,
        );
    });
}

// Bug: a `fixed<string> | nothing` local leaked its two strings (High: data integrity and
// release).
#[test]
fn a_fixed_string_list_held_by_a_union_local_is_released() {
    assert_fixture_balanced("v0_3_m9_c1m_fixed_string_union_local.ynz", &["kept\n"]);
}

// Bug: a `maybe<array<int>>` returned from a function leaked 4 of 8 allocations (High: data
// integrity and release).
#[test]
#[ignore = "deferred to #497: maybe<array<T>> returned from a local is the caller's copy to \
             make (copied_at_return), but owned_copy_plan refuses that copy; PM 2026-11-18"]
fn a_maybe_array_returned_from_a_function_is_released() {
    assert_fixture_balanced("v0_3_m9_c1m_maybe_array_returned.ynz", &["true\n"]);
}
