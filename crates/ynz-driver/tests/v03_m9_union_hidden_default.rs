//! v0.3-M9 C1e rework (#421): a plain value that lands in a UNION-typed shape field through a
//! hidden default must keep its variant tag, wrapped at the one coercion point, in the declaring
//! module and in an importing one (data integrity / memory safety). Also holds the guard that
//! keeps an index store of a plain variant out of codegen, and the refusals a hidden default's
//! type check raises.
//!
//! The container stores `add` / `map.set` are #423's (a frame envelope cannot live in a
//! container): their fixtures and tests are at `feat/v0-3-m9-c1e-rework-containers@56b4986`.
//!
//! The run table has one fixture per path a default takes, each run as shipped (the VALUE) and
//! under AddressSanitizer (the MEMORY ACCESS) at -O0 and -O2: a misread often prints plausible
//! output and only the sanitizer sees the out-of-bounds read, and the optimizer can mask a
//! misread. Imported fixtures are multi-module projects (a directory). The refusal table
//! compiles only.

mod support;

use support::{assert_prints_and_asan_clean_at_both_levels, assert_refused_with};

/// One named test per path a hidden default takes of the `hidden default of a union-typed field
/// (feature run, integrity)` run unit.
macro_rules! hidden_default_run_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal;)+) => { $(
        // bug: a hidden default entering a field is skipped, shared between instances, dead after its frame or stored without its variant tag, so the field reads back 0, a null envelope (SIGSEGV), another instance's value or the wrong variant | tier: High
        #[test]
        fn $case_name() {
            assert_prints_and_asan_clean_at_both_levels(
                $fixture,
                $expected_stdout,
                "a hidden default entering a field must keep its value and its own variant tag",
            );
        }
    )+ };
}

hidden_default_run_high_case! {
    // case: hidden default `extra: Big | int = 4096` landed as the raw int where the envelope pointer belongs (SIGSEGV at 0x1000)
    a_non_shape_default_in_a_union_field_is_wrapped, "v0_3_m9_union_hidden_default_non_shape.ynz", "other\nend\n";
    // case: a hidden union default re-homed out of the initializer was shared, dead or dropped (another instance's slot changed, a stack envelope outlived its frame, an inherited default went missing)
    a_union_default_is_per_instance, "v0_3_m9_union_hidden_default_per_instance.ynz", "big replacement\nint\nint\nint\nint\nint\n";
    // case: an IMPORTED shape's defaults were skipped (the importer looked for the declaration only in its own source): an `int` default read 0 and a union default was a null envelope pointer
    an_imported_shape_keeps_its_hidden_defaults, "v0_3_m9_union_hidden_default_imported", "5\nother\nend\n";
    // case: the child's initializer applies its parent's defaults too (union one included); `extends` cannot cross files today, so parent and child share the declaring module
    an_imported_child_applies_its_parents_defaults, "v0_3_m9_union_hidden_default_inherited_imported", "5\n7\nother\nend\n";
    // case: two modules each declare a DIFFERENT shape `Thing`: both defined `ynz_defaults<shape:Thing>` (a duplicate-symbol link failure), and a call into the other module's same-named initializer would write that shape's layout into this one
    same_named_shapes_of_two_modules_keep_their_own_defaults, "v0_3_m9_union_hidden_default_same_name_shapes", "11a\n11a\n22b\n";
}

/// One named test per refused path of the `hidden default and union store refusal (typeck
/// accept/reject)` typeck unit.
macro_rules! hidden_default_refusal_high_case {
    ($($case_name:ident, $fixture:literal, $refusal:literal;)+) => { $(
        // bug: typeck accepts a hidden default or an element store that does not fit its slot, so the build ends in codegen's internal error or the program SIGSEGVs reading a raw value as the envelope pointer | tier: High
        #[test]
        fn $case_name() {
            assert_refused_with($fixture, &[$refusal]);
        }
    )+ };
}

hidden_default_refusal_high_case! {
    // case: regression guard (no hole found): typeck compares an index store against the element type strictly, so `things[0] = big` into `array<Thing>` never reaches codegen as a bare store. If that refusal is relaxed, the store must wrap through the one coercion point and this becomes a run fixture
    a_plain_variant_index_stored_into_a_union_element_is_refused, "v0_3_m9_union_container_index_set_refused.ynz", "This value is `Big`, but the array holds `Tiny | Big`";
    // case: `hidden extra: string = 4096` was never checked against its annotation, compiled, and SIGSEGVed reading the raw int as a string
    a_default_of_the_wrong_type_is_refused, "v0_3_m9_union_hidden_default_string_refused.ynz", "Hidden field `extra` expects `String`, but its default is `Int`";
    // case: a shape-literal default into a union-typed hidden field stopped the build with the internal "struct literal with non-shape type Error" instead of a teaching diagnostic
    a_shape_literal_default_into_a_union_is_refused, "v0_3_m9_union_hidden_default_shape_literal_refused.ynz", "A shape value `{ ... }` cannot produce a `Tiny | Big` value";
    // case: `Nope<int>` resolved to an unreported error type, the default check skipped a union holding one, and the program compiled and SIGSEGVed reading the raw int 4096 as the envelope pointer
    an_unknown_generic_variant_is_refused, "v0_3_m9_union_hidden_default_unknown_generic_variant.ynz", "`Nope` is not a known type";
    // case: `map<int>` resolved its missing value type to an unreported error type, with the same compile-then-SIGSEGV as an unknown generic name
    a_missing_type_argument_is_refused, "v0_3_m9_union_hidden_default_missing_type_argument.ynz", "`Map` takes 2 type arguments";
}
