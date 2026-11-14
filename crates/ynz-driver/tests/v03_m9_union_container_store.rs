//! v0.3-M9 C1e rework (#421): a plain value stored into a UNION-typed container element
//! (`array<Thing>.add(plain)`, `arr[i] = plain`, `map.set(key, plain)`) must be wrapped in the
//! tagged `{tag, data}` envelope at the one coercion point, so reading the element back through
//! an `is` arm picks the variant that went in. Tier: High (data integrity / memory safety).
//!
//! One named test per case: each fixture runs at two optimization levels, plainly (the VALUE)
//! and under AddressSanitizer (the MEMORY ACCESS), kept apart so a failure says which is wrong.

mod support;

use support::{
    assert_asan_clean_and_prints as assert_asan_clean, assert_fixture_prints as assert_prints,
};

const WHAT: &str = "a plain value stored into a union-typed container element must carry its \
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

// Bug: `array<Thing>.add(plain)` stored the bare shape as the element. Tier: High.
container_store_tests!(
    "v0_3_m9_union_container_add.ynz",
    "tiny\nbig hello\ntiny\nbig world\nend\n",
    v03_m9_union_container_add_optimized,
    v03_m9_union_container_add_no_optimize,
    v03_m9_union_container_add_asan_optimized,
    v03_m9_union_container_add_asan_no_optimize
);

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

// Bug: `map.set(key, plain)` stored the bare shape as the value of a union-valued map. Tier: High.
container_store_tests!(
    "v0_3_m9_union_container_map_set.ynz",
    "tiny\nbig hello\nend\n",
    v03_m9_union_container_map_set_optimized,
    v03_m9_union_container_map_set_no_optimize,
    v03_m9_union_container_map_set_asan_optimized,
    v03_m9_union_container_map_set_asan_no_optimize
);

// RED UNTIL #423 (by-value envelope with an inline payload; a frame envelope cannot pass this).
// Bug: a union container filled in a loop shares one per-site envelope slot (every element reads
// the last value stored, ASan-clean), so the elements of one site must each own their envelope
// and payload. Tier: High.
#[test]
fn v03_m9_union_container_loop_elements_keep_their_own_variant() {
    // WHY: -O0, where nothing optimizes the aliasing away; the output is the verdict.
    assert_prints(
        "v0_3_m9_union_container_loop.ynz",
        true,
        "tiny\nbig b1\ntiny\nbig b3\nbig m0\nbig m1\nend\n",
        WHAT,
    );
}

// RED UNTIL #423. Bug: a union container returned from the function that filled it holds
// pointers into the dead frame (envelope and payload): stack-use-after-return, `none` printed.
// Tier: High.
#[test]
fn v03_m9_union_container_returned_from_its_frame_stays_valid() {
    // WHY: ASan names the dangling read; -O0 keeps the dead frame's slots untouched in the output.
    assert_asan_clean(
        "v0_3_m9_union_container_escape.ynz",
        true,
        "big hello\ntiny\nend\n",
    );
}

// RED, awaiting the lead's go-ahead to fix (question 4 of the card). Bug: a hidden shape-field
// default of a non-shape variant (`hidden extra: Big | int = 4096`) lands as the raw int where the
// envelope pointer belongs: SIGSEGV reading address 0x1000. Tier: High.
container_store_tests!(
    "v0_3_m9_union_hidden_default_non_shape.ynz",
    "other\nend\n",
    v03_m9_union_hidden_default_optimized,
    v03_m9_union_hidden_default_no_optimize,
    v03_m9_union_hidden_default_asan_optimized,
    v03_m9_union_hidden_default_asan_no_optimize
);
