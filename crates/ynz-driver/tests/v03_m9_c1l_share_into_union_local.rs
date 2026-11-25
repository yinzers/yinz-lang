//! C1l and C1n (cards #493, #387): typeck refuses a store that would leave two holders of one
//! value or tag a union value as the wrong variant: a `share` parameter stored into a union local
//! (the Chief ruled 2026-11-18 to keep the refusal over a hidden copy), a piece stored into a
//! literal, and a generic union return whose bound union is not first. The advised forms run
//! balanced (see `v03_m9_c1k_nonenvelope_union_release`).

mod support;

use support::assert_refused_with;

/// One named test per refused store of the `union store refusal` typeck unit; each case is a test
/// the runner names.
macro_rules! union_store_refusal_high_case {
    ($($case_name:ident, $fixture:literal, $phrases:expr;)+) => { $(
        // bug: typeck accepts a store that leaves two holders of one value (so it is freed twice) or tags a bound union as the wrong variant | tier: High
        #[test]
        fn $case_name() {
            assert_refused_with($fixture, $phrases);
        }
    )+ };
}

union_store_refusal_high_case! {
    // case: a `share` parameter silently stored into a union local leaves two holders of one array
    a_share_parameter_stored_into_a_union_local_is_refused_with_the_teaching_text, "v0_3_m9_c1l_share_into_union_local_refused.ynz", &["declared `share`", "gives it away"];
    // case: a piece stored into a literal (`mkm()` may return a piece someone else holds, so the same memory would be freed twice)
    a_piece_stored_into_a_literal_is_refused_with_a_why_that_names_the_holder, "v0_3_m9_c1l_piece_into_literal_refused.ynz", &["Give away a copy instead: `mkm().copy()`"];
    // case: `carry<T>(give item: T) -> nothing | T` called with a `Big | Tiny` stopped in codegen with "compiler bug"
    a_generic_union_return_with_nothing_first_is_refused, "v0_3_m9_c1n_generic_union_return_nothing_first_refused.ynz", &["Write `T` first in the return type of `carry`"];
    // case: `-> Tiny | T` called with a `Big | Tiny` printed `tiny` for a `Big` (the box keeps the tags of `Big | Tiny`)
    a_generic_union_return_with_a_variant_first_is_refused, "v0_3_m9_c1n_generic_union_return_variant_first_refused.ynz", &["Write `T` first in the return type of `carry`"];
}
