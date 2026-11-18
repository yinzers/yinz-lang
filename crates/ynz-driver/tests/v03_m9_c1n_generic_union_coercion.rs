//! v0.3-M9 C1n (card #496): a generic instance wraps the value it returns into its union's
//! envelope, as the non-generic function does, so a caller that narrows the result reads the
//! payload and the value is released once.

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

// Bug: `wrap<Big>` copied the shape's bytes raw instead of wrapping them in the `Big | nothing`
// envelope: the caller printed `absent` for a present value and 4 of 10 allocations leaked
// (High: data integrity and release).
#[test]
fn a_generic_union_return_of_a_shape_is_wrapped_and_released() {
    assert_fixture_balanced(
        "v0_3_m9_c1n_generic_union_return_shape.ynz",
        &["hello\nbye\n"],
    );
}

// Bug: a generic `T | nothing` instantiated with `T` = an envelope union (`Big | Tiny`) failed to
// build ("no envelope lowering") once the generic return was recorded as a coercion; it built and
// printed `hello` before (High: a regression of a program that was correct).
#[test]
fn a_generic_union_return_of_a_union_value_is_returned_as_it_is() {
    assert_fixture_balanced(
        "v0_3_m9_c1n_generic_union_return_union_bound.ynz",
        &["hello\n"],
    );
}
