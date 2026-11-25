//! v0.3-M9 C6 follow-up: the program shapes the C6 work cannot lower are refused by typeck, not by
//! a codegen error: a map key that does not fit the key slot, and a union handed to a generic
//! `background` callee. The widest accepted key is the boundary of the first rule.

mod support;

use support::{assert_fixture_prints, assert_refused_with, compile};

const SENSITIVE_KEY_REFUSAL: &str = "A `Map` cannot be keyed by `sensitive String`.";

/// One named test per refused (or boundary-accepted) program of the C6b typeck decisions; each
/// body states the rejections it expects and nothing else.
macro_rules! c6b_decision_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: typeck accepts a map key the slot cannot hold or a union argument to a generic background callee, or refuses the widest key that fits | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

c6b_decision_high_case! {
    // case: a `number<50>` key compiled and was compared by address; it does not fit the 16-byte key slot
    bignum_map_key_is_refused => {
        assert_refused_with(
            "v0_3_m9_c6b_map_key_bignum_refused.ynz",
            &["A `Map` cannot be keyed by `Number<50>`."],
        );
    }
    // case: a `sensitive string` key compiled, so the map released the secret as an ordinary string and compared it with a comparison that is not constant-time; a shape field's or union alias's map type skipped the key check too (the field, the nested field and the union variant are each refused once)
    sensitive_string_map_key_is_refused_in_every_position => {
        assert_refused_with(
            "v0_3_m9_c6b_map_key_sensitive_refused.ynz",
            &[SENSITIVE_KEY_REFUSAL],
        );
        let in_shape = "v0_3_m9_c7_map_key_sensitive_in_shape_refused.ynz";
        assert_refused_with(in_shape, &[SENSITIVE_KEY_REFUSAL]);
        let refusals = compile(in_shape).diagnostics.matches(SENSITIVE_KEY_REFUSAL).count();
        assert_eq!(refusals, 3, "{in_shape}: the field, the nested field and the union variant are each refused once");
    }
    // case: a shape key compiled and was compared by address
    shape_map_key_is_refused => {
        assert_refused_with(
            "v0_3_m9_c6b_map_key_shape_refused.ynz",
            &["A `Map` can't be keyed by `Point` yet."],
        );
    }
    // case: the task read the spawner's dead frame through a union view
    union_argument_to_a_generic_background_callee_is_refused => {
        assert_refused_with(
            "v0_3_m9_c6b_background_union_generic_refused.ynz",
            &["Cannot give the union `Circle | Tag` to `eat`'s generic parameter `held`"],
        );
    }
    // case: an off-by-one in the 34-digit limit (`<` for `<=`) accepting a `number<35>` key
    number_map_key_one_digit_past_the_widest_is_refused => {
        assert_refused_with(
            "v0_3_m9_c6b_map_key_just_past_widest_number_refused.ynz",
            &["A `Map` cannot be keyed by `Number<35>`."],
        );
    }
    // case: the refusal also catching the 34-digit key that fits the 16-byte key slot
    number_map_key_of_the_widest_width_is_found_by_an_identical_number => {
        assert_fixture_prints(
            "v0_3_m9_c6b_map_key_widest_number_accepted.ynz",
            false,
            "7\n",
            "a `Number<34>` map key must be accepted and found by an identical number",
        );
    }
}
