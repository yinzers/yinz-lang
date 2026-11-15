//! v0.3-M9 C1e rework (#421): a plain value that lands in a UNION-typed shape field through a
//! hidden default must keep its variant tag, wrapped at the one coercion point. Tier: High (data
//! integrity / memory safety). Also holds the guard that keeps an index store of a plain variant
//! out of codegen, and the two refusals a hidden default's type check raises.
//!
//! The container stores `add` / `map.set` are #423's (a frame envelope cannot live in a
//! container): their fixtures and tests are at `feat/v0-3-m9-c1e-rework-containers@56b4986`.
//!
//! Count: 14 tests = 2 run fixtures x 4 runs + 6 compile-only guards, past the ~8 a High-tier
//! unit gets without a written reason. The two fixtures are not copies: `non_shape` is the
//! minimal red case (a raw int where the envelope pointer belongs); `per_instance` is what the
//! heap re-home of the default must keep true (two instances never share one envelope, and a
//! parent's default reaches a child built in a loop). The 4 runs per fixture are plain vs ASan
//! (a misread often prints plausible output; only the sanitizer sees the out-of-bounds read)
//! and -O0 vs -O2 (the optimizer can mask a misread). The 6 guards compile only.
//!
//! One named test per case: each fixture runs at two optimization levels, plainly (the VALUE)
//! and under AddressSanitizer (the MEMORY ACCESS), kept apart so a failure says which is wrong.

mod support;

use support::{
    assert_asan_clean_and_prints as assert_asan_clean, assert_fixture_prints as assert_prints,
};

const WHAT: &str = "a hidden default entering a union-typed field must carry its \
                    own variant tag";

macro_rules! hidden_default_tests {
    ($fixture:expr, $stdout:expr, $optimized:ident, $no_optimize:ident, $asan_optimized:ident, $asan_no_optimize:ident) => {
        #[test]
        fn $optimized() {
            // WHY: the field read back must be the variant its default stored.
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

// Bug: hidden default `extra: Big | int = 4096` landed as the raw int where the envelope pointer
// belongs (SIGSEGV at 0x1000). Tier: High.
hidden_default_tests!(
    "v0_3_m9_union_hidden_default_non_shape.ynz",
    "other\nend\n",
    v03_m9_union_hidden_default_optimized,
    v03_m9_union_hidden_default_no_optimize,
    v03_m9_union_hidden_default_asan_optimized,
    v03_m9_union_hidden_default_asan_no_optimize
);

/// `fixture_name` must fail `ynz build` with `refusal` in its diagnostics, and never with the
/// internal "compiler bug" / "failed to produce machine code" box.
fn assert_build_refused(fixture_name: &str, refusal: &str) {
    let compiled = support::compile(fixture_name);
    assert!(
        !compiled.success,
        "{fixture_name} compiled:\n{}",
        compiled.diagnostics
    );
    assert!(
        compiled.diagnostics.contains(refusal),
        "{fixture_name}: refusal must contain `{refusal}`; got:\n{}",
        compiled.diagnostics
    );
    for internal_error in ["compiler bug", "failed to produce machine code"] {
        assert!(
            !compiled.diagnostics.contains(internal_error),
            "{fixture_name}: a user-reachable refusal must not be `{internal_error}`; got:\n{}",
            compiled.diagnostics
        );
    }
}

#[test]
fn v03_m9_union_hidden_default_wrong_type_is_refused() {
    // Bug: `hidden extra: string = 4096` was never checked against its annotation, compiled, and
    // SIGSEGVed reading the raw int as a string. Tier: High.
    assert_build_refused(
        "v0_3_m9_union_hidden_default_string_refused.ynz",
        "Hidden field `extra` expects `string`, but its default is `int`",
    );
}

#[test]
fn v03_m9_union_hidden_default_shape_literal_into_union_is_refused() {
    // Bug: a shape-literal default into a union-typed hidden field stopped the build with the
    // internal "struct literal with non-shape type Error" instead of a teaching diagnostic.
    // Tier: High.
    assert_build_refused(
        "v0_3_m9_union_hidden_default_shape_literal_refused.ynz",
        "A shape value `{ ... }` cannot produce a `Tiny | Big` value",
    );
}

// Bug: a hidden union default re-homed out of the initializer was shared, dead or dropped (another
// instance's slot changed, a stack envelope outlived its frame, an inherited default went missing).
// Tier: High.
hidden_default_tests!(
    "v0_3_m9_union_hidden_default_per_instance.ynz",
    "big replacement\nint\nint\nint\nint\nint\n",
    v03_m9_union_hidden_default_per_instance_optimized,
    v03_m9_union_hidden_default_per_instance_no_optimize,
    v03_m9_union_hidden_default_per_instance_asan_optimized,
    v03_m9_union_hidden_default_per_instance_asan_no_optimize
);

#[test]
fn v03_m9_union_hidden_default_unknown_variant_reports_only_the_unknown_type() {
    // Bug: the default was checked against `Big | unknown`, adding "Hidden field `extra` expects
    // `Big | unknown`" after the real error, naming a type the user never wrote. Tier: Medium.
    let compiled = support::compile("v0_3_m9_union_hidden_default_unknown_variant.ynz");
    assert!(
        !compiled.success,
        "an unknown union variant compiled:\n{}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .diagnostics
            .contains("`Missing` is not a known type"),
        "the unknown type must be reported; got:\n{}",
        compiled.diagnostics
    );
    assert!(
        !compiled.diagnostics.contains("Hidden field")
            && !compiled.diagnostics.contains("unknown`"),
        "the default must not be checked against a broken type; got:\n{}",
        compiled.diagnostics
    );
}

#[test]
fn v03_m9_union_hidden_default_unknown_generic_variant_is_refused() {
    // Bug: `Nope<int>` resolved to an unreported error type, the default check skipped a union
    // holding one, and the program compiled and SIGSEGVed reading the raw int 4096 as the
    // envelope pointer. Tier: High.
    assert_build_refused(
        "v0_3_m9_union_hidden_default_unknown_generic_variant.ynz",
        "`Nope` is not a known type",
    );
}

#[test]
fn v03_m9_union_hidden_default_missing_type_argument_is_refused() {
    // Bug: `map<int>` resolved its missing value type to an unreported error type, with the same
    // compile-then-SIGSEGV as an unknown generic name. Tier: High.
    assert_build_refused(
        "v0_3_m9_union_hidden_default_missing_type_argument.ynz",
        "`map` takes 2 type arguments",
    );
}
