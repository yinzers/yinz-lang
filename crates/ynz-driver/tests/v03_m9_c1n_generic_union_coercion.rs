//! v0.3-M9 C1n (card #496): a generic instance wraps the value it returns into its union's
//! envelope, as the non-generic function does, so a caller that narrows the result reads the
//! payload and the value is released once.

mod support;

use support::{
    assert_built_balanced_and_asan_clean_printing_one_of, at_both_optimization_levels, build,
    compile,
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

// Bug (C10 blocker, card #387): `carry<T>(give item: T) -> nothing | T` and `-> Tiny | T` called
// with a `Big | Tiny` value stopped in codegen with "compiler bug" (the plan) or, for `Tiny | T`,
// printed `tiny` for a `Big` (`main`): the returned box keeps the tags of `Big | Tiny` while the
// instance's union numbers its variants from `nothing` / `Tiny`. No union-into-wider-union
// conversion exists, so typeck refuses the call (High: data integrity).
#[test]
fn a_generic_union_return_with_the_bound_union_not_first_is_refused() {
    for fixture in [
        "v0_3_m9_c1n_generic_union_return_nothing_first_refused.ynz",
        "v0_3_m9_c1n_generic_union_return_variant_first_refused.ynz",
    ] {
        let compiled = compile(fixture);
        assert!(
            !compiled.success,
            "{fixture} compiled:\n{}",
            compiled.diagnostics
        );
        assert!(
            !compiled.diagnostics.contains("compiler bug"),
            "{fixture}: a user-reachable refusal must not say `compiler bug`:\n{}",
            compiled.diagnostics
        );
        assert!(
            compiled
                .diagnostics
                .contains("Write `T` first in the return type of `carry`"),
            "{fixture}: the refusal must give the registry's WHAT INSTEAD:\n{}",
            compiled.diagnostics
        );
    }
}
