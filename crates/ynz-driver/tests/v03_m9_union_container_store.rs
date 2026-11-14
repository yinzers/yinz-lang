//! v0.3-M9 C1e rework (#421): plain values reaching a UNION-typed slot through the container and
//! hidden-default paths. The index-set refusal guard (`arr[i] = plain` never reaches codegen)
//! and the hidden shape-field default (`hidden extra: Big | int = 4096`) must keep every plain
//! variant behind the one coercion point. Tier: High (data integrity / memory safety).
//!
//! The container stores `add` / `map.set` are #423's (a frame envelope cannot live in a
//! container): their fixtures and tests are at `feat/v0-3-m9-c1e-rework-containers@56b4986`.
//!
//! One named test per case: each fixture runs at two optimization levels, plainly (the VALUE)
//! and under AddressSanitizer (the MEMORY ACCESS), kept apart so a failure says which is wrong.

mod support;

use support::{
    assert_asan_clean_and_prints as assert_asan_clean, assert_fixture_prints as assert_prints,
};

const WHAT: &str = "a plain value entering a union-typed slot must carry its \
                    own variant tag";

macro_rules! container_store_tests {
    ($fixture:expr, $stdout:expr, $optimized:ident, $no_optimize:ident, $asan_optimized:ident, $asan_no_optimize:ident) => {
        #[test]
        fn $optimized() {
            // WHY: the element read back must be the variant that was stored.
            assert_prints($fixture, false, $stdout, WHAT);
        }

        #[test]
        fn $no_optimize() {
            // WHY: same defect at -O0, where no optimizer can mask the misread.
            assert_prints($fixture, true, $stdout, WHAT);
        }

        #[test]
        fn $asan_optimized() {
            // WHY: a bare value read as a 16-byte envelope is out of bounds; plausible output
            // can hide it, so the sanitizer verdict stands alone.
            assert_asan_clean($fixture, false, $stdout);
        }

        #[test]
        fn $asan_no_optimize() {
            // WHY: same out-of-bounds envelope read at -O0.
            assert_asan_clean($fixture, true, $stdout);
        }
    };
}

#[test]
fn v03_m9_union_container_index_set_plain_variant_is_refused() {
    // Regression guard (no hole found): typeck compares an index store against the element type
    // strictly, so `things[0] = big` into `array<Thing>` never reaches codegen as a bare store.
    // If that refusal is ever relaxed to accept a variant, the store must wrap through the one
    // coercion point and this test becomes a run fixture. Tier: High.
    let compiled = support::compile("v0_3_m9_union_container_index_set_refused.ynz");
    assert!(
        !compiled.success,
        "a plain variant index-stored into a union element compiled:\n{}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .diagnostics
            .contains("This value is `Big`, but the array holds `Tiny | Big`"),
        "refusal text changed:\n{}",
        compiled.diagnostics
    );
    assert!(
        !compiled.diagnostics.contains("compiler bug"),
        "{}",
        compiled.diagnostics
    );
}

// Bug: a hidden shape-field default of a non-shape variant (`hidden extra: Big | int = 4096`) lands as the raw int where the
// envelope pointer belongs: SIGSEGV reading address 0x1000. Tier: High.
container_store_tests!(
    "v0_3_m9_union_hidden_default_non_shape.ynz",
    "other\nend\n",
    v03_m9_union_hidden_default_optimized,
    v03_m9_union_hidden_default_no_optimize,
    v03_m9_union_hidden_default_asan_optimized,
    v03_m9_union_hidden_default_asan_no_optimize
);
