//! v0.3-M9 C1f security round 1 (#423): a shape with an envelope-union field keeps the field's
//! variants INLINE, so its size depends on the variant shapes of the module that declares it. An
//! importer sizes that shape again, and looks the variants up by NAME. When the importer's name
//! means a different shape (its own, or another import's), the two modules size the same slot
//! two ways and one writes past the other's (memory safety).
//!
//! Each fixture is one way a name comes to mean two shapes, or to be laid out by an importer that
//! does not import it, not a copy of another:
//!   - `local`: the importer declares its own shape under the variant's name.
//!   - `two_imports`: two imported shapes each need a different unexported shape of one name.
//!   - `shared`: the guard; the importer imports the variant itself, so the name means one shape.
//!   - `equality_imported` and `imported_signature`: valid programs over unimported variants that
//!     failed to build.

mod support;

use support::{assert_asan_clean_and_prints_at_both_levels, assert_refused_with};

/// One named test per way a variant name is laid out by two modules of the `imported union
/// layout by variant name (memory safety)` unit.
macro_rules! union_layout_variant_name_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: an importer sizes an imported union slot by a different shape than the declaring module (or fails to lay out a valid one), so one module writes past the other's slot (ASan stack-buffer-overflow) or a valid program is refused | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

union_layout_variant_name_high_case! {
    // case: the importer sized `Holder.slot` by its own 1-byte `Big`; `lib` wrote its 48-byte `Big` into it (ASan stack-buffer-overflow at -O0 and -O2)
    a_local_shape_under_a_variants_name_is_refused => {
        assert_refused_with(
            "v0_3_m9_union_layout_variant_name_local",
            &["Two different shapes are named `Big`"],
        );
    }
    // case: the last import's `Big` sized both imported union fields (ASan stack-buffer-overflow)
    two_imports_of_one_variant_name_are_refused => {
        assert_refused_with(
            "v0_3_m9_union_layout_variant_name_two_imports",
            &["Two different shapes are named `Big`"],
        );
    }
    // case: the refusal must not reach an importer whose name means the one shape (at -O2 a misized slot is a stack-use past the alloca)
    an_importer_that_imports_the_variant_itself_still_runs => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_union_layout_variant_name_shared",
            "big lib 5\n7\nbig own 9\n",
        );
    }
    // case: shape equality looked an unimported (layout-only) variant shape up in the importer's shape table and refused to compile a valid program
    equality_over_an_unimported_variant_still_builds => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_union_equality_imported",
            "big same true\nbig payload false\nvariant false\nend\n",
        );
    }
    // case: an exported function's union (returned and taken) over shapes the importer never imports was sized only for an exported SHAPE's union field, so the importer could not lay out the slot
    an_imported_signature_over_unimported_variants_still_builds => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_union_imported_signature",
            "big 4\ntiny\nend\n",
        );
    }
}
