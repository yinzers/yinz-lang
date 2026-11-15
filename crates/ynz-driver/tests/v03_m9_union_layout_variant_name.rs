//! v0.3-M9 C1f security round 1 (#423): a shape with an envelope-union field keeps the field's
//! variants INLINE, so its size depends on the variant shapes of the module that declares it. An
//! importer sizes that shape again, and looks the variants up by NAME. When the importer's name
//! means a different shape (its own, or another import's), the two modules size the same slot
//! two ways and one writes past the other's. Tier: High (memory safety).
//!
//! Each fixture is one way a name comes to mean two shapes, not a copy of another:
//!   - `local`: the importer declares its own shape under the variant's name.
//!   - `two_imports`: two imported shapes each need a different unexported shape of one name.
//!   - `shared`: the guard; the importer imports the variant itself, so the name means one shape.

mod support;

use support::{assert_asan_clean_and_prints as assert_asan_clean, compile};

fn assert_refused(fixture_name: &str) {
    let compiled = compile(fixture_name);
    assert!(
        !compiled.success,
        "{fixture_name}: a name that means two shapes, one of them sizing an imported union \
         field, must not compile:\n{}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .diagnostics
            .contains("Two different shapes are named `Big`"),
        "{fixture_name}: the refusal must name the clashing shape:\n{}",
        compiled.diagnostics
    );
}

#[test]
fn v03_m9_union_layout_variant_name_local_is_refused() {
    // Bug: the importer sized `Holder.slot` by its own 1-byte `Big`; `lib` wrote its 48-byte
    // `Big` into it (ASan stack-buffer-overflow at -O0 and -O2).
    assert_refused("v0_3_m9_union_layout_variant_name_local");
}

#[test]
fn v03_m9_union_layout_variant_name_two_imports_is_refused() {
    // Bug: the last import's `Big` sized both imported union fields (ASan stack-buffer-overflow).
    assert_refused("v0_3_m9_union_layout_variant_name_two_imports");
}

#[test]
fn v03_m9_union_layout_variant_name_shared_asan_no_optimize() {
    // WHY: the refusal must not reach an importer whose name means the one shape.
    assert_asan_clean(
        "v0_3_m9_union_layout_variant_name_shared",
        true,
        "big lib 5\n7\nbig own 9\n",
    );
}

#[test]
fn v03_m9_union_layout_variant_name_shared_asan_optimized() {
    // WHY: same at -O2, where a misized slot is a stack-use past the alloca.
    assert_asan_clean(
        "v0_3_m9_union_layout_variant_name_shared",
        false,
        "big lib 5\n7\nbig own 9\n",
    );
}
