//! v0.3-M9 C6 follow-up: the program shapes the C6 work cannot lower are refused by typeck with a
//! teaching diagnostic (what happened, what to do instead, why), not by a codegen error.
//!
//! Medium tier: each is a user-reachable refusal whose message is the contract.

mod support;

use support::{assert_fixture_prints, compile};

fn assert_refused_with(fixture_name: &str, fragments: &[&str]) {
    let compiled = compile(fixture_name);
    assert!(
        !compiled.success,
        "{fixture_name}: must be refused, but compiled:\n{}",
        compiled.diagnostics
    );
    for fragment in fragments {
        assert!(
            compiled.diagnostics.contains(fragment),
            "{fixture_name}: diagnostic must contain {fragment:?}; got:\n{}",
            compiled.diagnostics
        );
    }
    for internal in ["codegen:", "internal error", "compiler bug"] {
        assert!(
            !compiled.diagnostics.contains(internal),
            "{fixture_name}: a user error must come from typeck, not codegen ({internal:?}):\n{}",
            compiled.diagnostics
        );
    }
}

#[test]
fn bignum_map_key_is_refused_with_a_teaching_diagnostic() {
    // Bug caught: a `number<50>` key compiled and was compared by address. Tier: Medium.
    assert_refused_with(
        "v0_3_m9_c6b_map_key_bignum_refused.ynz",
        &[
            "A `map` cannot be keyed by `number<50>`.",
            "Key the map by an `int` or a `string` id, or by a `number` of at most 34 digits",
            "compare where it sits in memory",
        ],
    );
}

#[test]
fn sensitive_string_map_key_is_refused_with_a_teaching_diagnostic() {
    // Bug caught: a `sensitive` key compiled and was compared by address. Tier: Medium.
    assert_refused_with(
        "v0_3_m9_c6b_map_key_sensitive_refused.ynz",
        &[
            "A `map` cannot be keyed by `sensitive string`.",
            "Key the map by a plain `string`",
            "compare where it sits in memory",
        ],
    );
}

#[test]
fn shape_map_key_is_refused_with_a_teaching_diagnostic() {
    // Bug caught: a shape key compiled and was compared by address. Tier: Medium.
    assert_refused_with(
        "v0_3_m9_c6b_map_key_shape_refused.ynz",
        &[
            "A `map` cannot be keyed by `Point`.",
            "Key the map by one of its fields",
            "`map<string, Point>`",
            "compare where it sits in memory",
        ],
    );
}

#[test]
fn union_argument_to_a_generic_background_callee_is_refused_with_a_teaching_diagnostic() {
    // Bug caught: the task read the spawner's dead frame through a union view. Tier: Medium.
    assert_refused_with(
        "v0_3_m9_c6b_background_union_generic_refused.ynz",
        &[
            "Cannot give the union `Circle | Tag` to `eat`'s generic parameter `held`",
            "Declare `held` as `Circle | Tag` in `eat`'s signature",
            "gives it no union slot to copy into",
        ],
    );
}

#[test]
fn number_map_key_one_digit_past_the_widest_is_refused() {
    // Bug caught: an off-by-one in the 34-digit limit (`<` for `<=`, or the reverse) refusing or
    // accepting the wrong width. Tier: Medium.
    assert_refused_with(
        "v0_3_m9_c6b_map_key_just_past_widest_number_refused.ynz",
        &["A `map` cannot be keyed by `number<35>`."],
    );
}

#[test]
fn number_map_key_of_the_widest_width_is_found_by_value() {
    // Bug caught: the refusal also catching the 34-digit key a map does compare by value.
    // Tier: Medium.
    assert_fixture_prints(
        "v0_3_m9_c6b_map_key_widest_number_accepted.ynz",
        false,
        "7\n",
        "a `number<34>` map key must be accepted and found by value",
    );
}
