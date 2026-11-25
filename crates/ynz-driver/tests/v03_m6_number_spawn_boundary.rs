//! decimal128 (`number`) across a CONCURRENCY boundary, and the int-literal-into-`number` gate.
//!
//! - **`background` spawn `number` arg:** `prepare_bg_arg_for_ctx` had no `Number` arm, so a
//!   decimal128 arg rode by pointer to the SPAWNER's stack temp; the spawner suspended, its
//!   stack died, and the task read the dangling pointer (`0.000...` instead of the staged value).
//! - **cpu-member `number` arg:** `emit_cpu_member_spawn` loaded the member's slot as an i64
//!   where the callee expects a pointer-typed number param: an LLVM verifier ICE on valid code.
//! - **int literal into a `number` slot:** there is no int-to-number coercion yet, so every slot
//!   an `IntLit` (or `-IntLit`) can reach must refuse it with the ONE teaching diagnostic;
//!   before the gate each slot segfaulted, ICE'd or computed a silently wrong value.
//!
//! The suspending-CALL half of the decimal128 class is asserted by
//! `v03_m6_shape_arg_frame_backing.rs`.

mod support;

use support::{assert_fixture_prints, compile};

/// One named test per input class of the `number arg across a spawn boundary` unit; each case
/// is a test the runner names.
macro_rules! number_spawn_boundary_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal;)+) => { $(
        // bug: a `number` argument of a `background` spawn or a CPU group member reaches the task as a dangling spawner-stack pointer (prints 0.000...) or miscompiles with an LLVM verifier ICE | tier: High
        #[test]
        fn $case_name() {
            assert_fixture_prints(
                $fixture,
                false,
                $expected_stdout,
                "the task must read the decimal128 the spawner staged, not the corpse of the spawner's stack",
            );
        }
    )+ };
}

number_spawn_boundary_high_case! {
    // case: `background report(x)` with a non-suspending callee (blocking-pool arm) reads a dead stack temp
    v03_m6_number_arg_background_cpu_arm_yields_staged_value, "v0_3_m6_number_arg_background_cpu.ynz", "2.5\n";
    // case: `background fetchNum(x)` with a suspending callee (state-machine arm) reads a dead stack temp
    v03_m6_number_arg_background_sm_arm_yields_staged_value, "v0_3_m6_number_arg_background_sm.ynz", "4.5\n";
    // case: a number-arg member of an auto-parallel CPU group ICEs in the shared trampoline
    v03_m6_number_arg_cpu_member_compiles_and_yields_values, "v0_3_m6_number_arg_cpu_member.ynz", "2.5\n4.5\n";
    // case: a number-arg CPU member of a fused CPU+I/O group ICEs in the same shared trampoline
    v03_m6_number_arg_fused_cpu_member_compiles_and_yields_values, "v0_3_m6_number_arg_fused_cpu_member.ynz", "2.5\n7\n";
}

/// The compiler refuses `fixture` at compile time with the int-literal teaching diagnostic,
/// never a crash and never the internal-compiler-error banner.
fn assert_int_literal_number_refused(fixture: &str) {
    let compiled = compile(fixture);
    assert!(
        !compiled.success,
        "an int literal into a `Number` slot must be REJECTED at compile time (no coercion yet); \
         diagnostics:\n{}",
        compiled.diagnostics
    );
    assert!(
        compiled
            .diagnostics
            .contains("passing an int literal to a `Number`"),
        "the shared int-literal-to-number teaching diagnostic must fire; diagnostics:\n{}",
        compiled.diagnostics
    );
    assert!(
        !compiled.diagnostics.contains("compiler bug"),
        "a known, user-triggerable limitation must never render the ICE banner; diagnostics:\n{}",
        compiled.diagnostics
    );
}

/// One named test per input class of the `int literal into a number slot` unit; the refused
/// cases are the slots the gate must cover, the accepted cases are the neighbours it must not touch.
macro_rules! int_literal_number_gate_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: an int literal reaching a `number` slot segfaults, ICEs or computes a wrong value instead of being refused with the teaching error, or the gate refuses valid number code | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

int_literal_number_gate_high_case! {
    // case: `f(5)` to a `number` param of a CPU-member spawn (plain call form)
    v03_m6_int_literal_to_number_param_cpu_member_is_clean_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_cpu_member.ynz");
    }
    // case: `p.scale(5)` UFCS dot-call form bypassed the gate and ICE'd at codegen
    v03_m6_int_literal_to_number_param_ufcs_is_clean_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_ufcs.ynz");
    }
    // case: a concrete `number` param of a generic fn bypassed the gate and panicked at codegen
    v03_m6_int_literal_to_number_param_generic_fn_is_clean_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_generic_fn.ynz");
    }
    // case: generic explicit-type-arg param
    v03_m6_int_lit_number_generic_explicit_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_generic_explicit.ynz");
    }
    // case: generic sibling-bound param
    v03_m6_int_lit_number_generic_sibling_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_generic_sibling.ynz");
    }
    // case: array-add elem (segfaulted before the gate)
    v03_m6_int_lit_number_array_add_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_array_add.ynz");
    }
    // case: array-set elem
    v03_m6_int_lit_number_array_set_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_array_set.ynz");
    }
    // case: fixed-set elem
    v03_m6_int_lit_number_fixed_set_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_fixed_set.ynz");
    }
    // case: map-set value
    v03_m6_int_lit_number_map_set_value_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_map_set.ynz");
    }
    // case: maybe-or default
    v03_m6_int_lit_number_maybe_or_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_maybe_or.ynz");
    }
    // case: array-contains target (computed a silently wrong answer before the gate)
    v03_m6_int_lit_number_array_contains_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_array_contains.ynz");
    }
    // case: shape-field construction
    v03_m6_int_lit_number_struct_field_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_struct_field.ynz");
    }
    // case: array-literal element
    v03_m6_int_lit_number_array_lit_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_array_lit.ynz");
    }
    // case: fixed-literal element
    v03_m6_int_lit_number_fixed_lit_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_fixed_lit.ynz");
    }
    // case: map-literal value
    v03_m6_int_lit_number_map_lit_val_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_map_lit_val.ynz");
    }
    // case: array index-assign element
    v03_m6_int_lit_number_index_assign_array_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_index_assign_array.ynz");
    }
    // case: map index-assign value
    v03_m6_int_lit_number_index_assign_map_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_index_assign_map.ynz");
    }
    // case: shape-field assignment
    v03_m6_int_lit_number_field_assign_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_field_assign.ynz");
    }
    // case: `return 5` from a `-> number` fn
    v03_m6_int_lit_number_return_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_return.ynz");
    }
    // case: `return 5` from a `-> number errors` fn (the errors wrapper hid the number type from the gate)
    v03_m6_int_lit_number_return_errors_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_return_errors.ynz");
    }
    // case: multi-case-if arm pattern
    v03_m6_int_lit_number_match_arm_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_match_arm.ynz");
    }
    // case: map method key (computed a silently wrong answer before the gate)
    v03_m6_int_lit_number_map_method_key_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_map_number_key.ynz");
    }
    // case: map bracket key (computed a silently wrong answer before the gate)
    v03_m6_int_lit_number_map_bracket_key_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_map_bracket_key.ynz");
    }
    // case: array-remove elem
    v03_m6_int_lit_number_array_remove_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_array_remove.ynz");
    }
    // case: `let x: number = 5` store site ICE'd at codegen
    v03_m6_int_lit_number_let_store_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_let_store.ynz");
    }
    // case: `hidden f: number = 5` declaration-site default ICE'd at codegen
    v03_m6_int_lit_number_hidden_field_default_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_hidden_field_default.ynz");
    }
    // case: `-5` (negated literal) through a UFCS param
    v03_m6_neg_int_lit_number_ufcs_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_ufcs_neg.ynz");
    }
    // case: `-5` through a generic fn's concrete number param
    v03_m6_neg_int_lit_number_generic_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_generic_neg.ynz");
    }
    // case: `-5` through a plain-call param
    v03_m6_neg_int_lit_number_plain_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_arg_plain_neg.ynz");
    }
    // case: `-5` as an array-add elem (computed a silently wrong answer before the gate)
    v03_m6_neg_int_lit_number_array_add_is_teaching_error => {
        assert_int_literal_number_refused("v0_3_m6_int_literal_number_array_add_neg.ynz");
    }
    // case: a generic shape's type-param-typed hidden field is valid code the gate must not refuse
    v03_m6_generic_type_param_hidden_field_compiles_clean => {
        assert_fixture_prints(
            "v0_3_m6_int_literal_number_generic_hidden_field_ok.ynz",
            false,
            "generic-hidden-field-ok\n",
            "the hidden-field gate must not over-reach onto generic params",
        );
    }
    // case: decimal literals, number idents and every int-into-int neighbour must stay accepted and byte-exact
    v03_m6_int_literal_number_gate_false_positive_sweep_stays_clean => {
        assert_fixture_prints(
            "v0_3_m6_int_literal_number_false_positive_sweep.ynz",
            false,
            "5.0\ntoll-ok\ntoll-ok\n7\n3\n5\ntrue\n5\n5\n5\nmatch-int-ok\n",
            "the gate must fire only on an int literal into a `number` slot",
        );
    }
}
