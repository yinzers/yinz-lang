//! v0.3-M9 C1g (security): every call form compares each argument with its parameter's type.
//! Tier: High (type checker; untrusted program input; the pre-fix flow read a 1-byte `Tiny` as a
//! 16-byte `Big` — ASan stack-buffer-overflow READ — or reached LLVM with a mismatched call).
//!
//! The fixtures go through the public compile boundary (`ynz build`). On the parent of the fix
//! commit every rejected-fixture test below fails because the program COMPILES (the generic
//! call discarded `unify_param`'s mismatch; the dot-call form checked non-receiver arguments
//! for the union case only); built under ASan at -O0 the first four overflow the stack.
//!
//! Test count: 13 — one per refused case and per call form (generic with a sibling argument,
//! with an explicit type argument, with a concrete parameter, with two scalar types, with a
//! shape that fits a union parameter only through its type parameter; dot-call
//! with a shape and with an int), the plain-form guard (already clean when probed), and the
//! legal calls, container parameters, union-bound type parameters and the instance each
//! generic call was checked with, and an instance name no other function holds, that must keep
//! running.

mod support;

use support::compile;

/// The compile must fail, and the diagnostic must hold every fragment: the value, the
/// parameter, WHAT-INSTEAD and WHY, all in the program's own names.
fn assert_rejected_with(fixture_name: &str, fragments: &[&str]) {
    let compiled = compile(fixture_name);
    assert!(
        !compiled.success,
        "{fixture_name}: a mismatched argument reached a parameter and compiled:\n{}",
        compiled.diagnostics
    );
    for fragment in fragments {
        assert!(
            compiled.diagnostics.contains(fragment),
            "{fixture_name}: diagnostic must contain {fragment:?}; got:\n{}",
            compiled.diagnostics
        );
    }
}

#[test]
fn v03_m9_c1g_generic_argument_for_a_type_parameter_another_argument_set_is_rejected() {
    // BUG: `same(big, tiny)` binds `T = Big` from the first argument; the second argument's
    // `Tiny` was never compared with the substituted `Big`, so `same` read it as a `Big`
    // (ASan stack-buffer-overflow READ). TIER: High (type checker, memory safety).
    assert_rejected_with(
        "v0_3_m9_c1g_generic_two_shapes_one_type_parameter.ynz",
        &[
            // WHAT: the value and its type, the callee, the parameter and the cause.
            "`tiny` has type `Tiny`, but `same` needs `Big` for `second`",
            "`second` and `first` share the type parameter `T`",
            "the argument `big` for `first` already made it `Big`",
            // WHAT INSTEAD: only prose and names that exist in the program.
            "Pass a value of type `Big` for `second`",
            "Or give `first` a `Tiny` too, so `first` and `second` have the same type",
            // WHY: what happens to the bytes.
            "reading it as `Big`",
            "past the end of the `Tiny`",
        ],
    );
}

#[test]
fn v03_m9_c1g_generic_argument_against_an_explicit_type_argument_is_rejected() {
    // BUG: `same<Big>(big, tiny)` fixes `T` by hand; the `Tiny` argument was not compared with
    // it and was read as a `Big` (ASan stack-buffer-overflow READ). TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_generic_explicit_type_argument.ynz",
        &[
            "`tiny` has type `Tiny`, but `same` needs `Big` for `second`",
            "`same<Big>` sets the type parameter `T` to `Big`",
            "Or write `same<Tiny>` instead, if every argument that takes `T` is a `Tiny`",
        ],
    );
}

#[test]
fn v03_m9_c1g_generic_argument_for_a_concrete_parameter_is_rejected() {
    // BUG: `pick(big, tiny)` with a concrete `second: Big` parameter: no type parameter is
    // involved, yet the argument was never compared with the declared type (`Big` read from a
    // `Tiny`, ASan stack-buffer-overflow READ). TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_generic_concrete_parameter.ynz",
        &[
            "This argument is `Tiny`, but `pick` expects `Big` here.",
            "Pass a `Big` value.",
        ],
    );
}

#[test]
fn v03_m9_c1g_generic_int_and_string_for_one_type_parameter_is_rejected() {
    // BUG: `same(count, label)` (`T = int`, then a `string`) compiled to an LLVM module the
    // verifier rejected — a "compiler bug" report instead of a diagnostic. TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_generic_int_and_string.ynz",
        &[
            "`label` has type `string`, but `same` needs `int` for `second`",
            "the argument `count` for `first` already made it `int`",
        ],
    );
}

#[test]
fn v03_m9_c1g_dot_call_argument_of_another_shape_is_rejected() {
    // BUG: `big.useBig(tiny)` — the dot-call form compared non-receiver arguments for the union
    // case only, so a `Tiny` was read as a `Big` (ASan stack-buffer-overflow READ). The text
    // must be the plain form's. TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_dot_call_other_shape.ynz",
        &[
            "This argument is `Tiny`, but `useBig` expects `Big` here.",
            "Pass a `Big` value.",
        ],
    );
}

#[test]
fn v03_m9_c1g_dot_call_int_for_a_shape_is_rejected() {
    // BUG: `big.useBig(5)` reached LLVM and failed module verification, reported as a compiler
    // bug instead of a diagnostic. TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_dot_call_int_for_shape.ynz",
        &["This argument is `int`, but `useBig` expects `Big` here."],
    );
}

#[test]
fn v03_m9_c1g_plain_call_mismatches_stay_rejected() {
    // GUARD, no bug on the base: probed clean. The plain form already compared every argument
    // with its parameter; this keeps it so while the other forms change. TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_plain_call_guard.ynz",
        &[
            "This argument is `Tiny`, but `useBig` expects `Big` here.",
            "This argument is `int`, but `useString` expects `string` here.",
            "This argument is `string`, but `useInt` expects `int` here.",
            "This argument is `maybe<int>`, but `useInt` expects `int` here.",
            "This argument is `array<Tiny>`, but `useBigs` expects `array<Big>` here.",
        ],
    );
}

#[test]
fn v03_m9_c1g_calls_whose_arguments_match_their_parameters_still_run() {
    // GUARD against over-rejecting: agreeing generic arguments (inferred and explicit), a
    // concrete extra parameter and a matching dot-call argument all keep compiling and running.
    // TIER: High (the check must not reject a correct program).
    support::assert_fixture_prints(
        "v0_3_m9_c1g_legal_calls.ynz",
        false,
        "ten\n7\ntwo\n3\n9\n6\n",
        "legal calls must still compile and run",
    );
}

#[test]
fn v03_m9_c1g_generic_parameters_holding_the_type_parameter_in_a_container_are_accepted() {
    // BUG (found in review): the signature resolved `maybe<T>` with `T` as an unknown type and
    // `array<T>` as a user generic shape, so the argument comparison refused the correct calls
    // and the instance table recorded types no argument ever had. TIER: High (the check must
    // not reject a correct program).
    support::assert_fixture_prints(
        "v0_3_m9_c1g_generic_container_parameters.ynz",
        false,
        "3\n3\n3\n3\ntiny\n",
        "container parameters of a generic function must still compile and run",
    );
}

#[test]
fn v03_m9_c1g_shape_fitting_only_through_a_type_parameter_inside_a_union_is_rejected() {
    // BUG (found in security review): `first: T | nothing` with `T = Big` took a plain `Big`;
    // no tag exists for it in the declared union, so the callee read the bare shape as the
    // tagged box (wrong variant, `absent` for `hello`) or codegen failed. TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_generic_type_parameter_inside_union.ynz",
        &[
                "`big` is a `Big`, but `pick` takes it as `T | nothing`, which this call makes a `Big | nothing`.",
                "`big` is a `Big`, but `pickOrTiny` takes it as `T | Tiny`, which this call makes a `Big | Tiny`.",
        "Put it in a `Big | nothing` first",
        ],
    );
}

#[test]
fn v03_m9_c1g_union_bound_to_a_type_parameter_inside_a_union_parameter_is_accepted() {
    // BUG (found in round 2): `T | nothing` with `T = Big | Tiny` was a union nested in a union,
    // a different type from the flat `Big | Tiny | nothing` the argument has, so a correct call
    // that compiled and ran before C1g was refused. TIER: High (the check must not reject a
    // correct program).
    support::assert_fixture_prints(
        "v0_3_m9_c1g_generic_union_argument_in_union_parameter.ynz",
        false,
        "hello\ntiny2\n",
        "a union bound to a type parameter inside a union parameter must still compile and run",
    );
}

#[test]
fn v03_m9_c1g_generic_call_runs_the_instance_the_type_checker_chose() {
    // BUG (found in round 2): codegen re-derived each call's instance from its argument types;
    // `pickA(tiny, big)` fits both `pickA<Big>` and `pickA<Big | Tiny>`, so a legal program
    // stopped with an internal compiler error, and a wrong pick reads a bare `Big` as a tagged
    // box. Codegen now reads the type arguments typeck recorded for the call. TIER: High (call
    // dispatch, memory safety).
    support::assert_fixture_prints(
        "v0_3_m9_c1g_generic_call_runs_its_checked_instance.ynz",
        false,
        "hello\nhello\ntiny\n",
        "every generic call must run the instance it was checked with",
    );
}

#[test]
fn v03_m9_c1g_generic_instance_name_never_belongs_to_another_function() {
    // BUG (found in round 3): instance names were `_`-joined and identifiers may hold `_`, so
    // `show<Big>` took the name of a user function `show_shape_Big(Tiny)` (which then ran on a
    // `Big`, reading its `label` pointer as an int), and two instances of `second` shared one
    // name. TIER: High (call dispatch, memory safety).
    support::assert_fixture_prints(
        "v0_3_m9_c1g_generic_instance_name_is_its_own.ynz",
        false,
        "generic\n42\nx\n",
        "every generic call must run its own instance",
    );
}
