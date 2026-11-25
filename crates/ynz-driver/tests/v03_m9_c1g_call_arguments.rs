//! C1g (security): every call form compares each argument with its parameter's type and passes
//! the number of arguments the function declares. The pre-fix flow read a 1-byte `Tiny` as a
//! 16-byte `Big` (ASan stack-buffer-overflow READ), or reached LLVM with a mismatched call, so
//! the compiler reported its own bug instead of teaching.
//!
//! The fixtures go through the public compile boundary (`ynz build`). A refused case asserts the
//! compile fails, is not a compiler bug, and names each distinct rejection the fixture holds.
//! The legal calls must keep compiling and running.

mod support;

use support::{assert_asan_clean_and_prints, assert_fixture_prints, assert_refused_with};

/// One named test per call form of the `call argument type refusal` typeck unit; each case is a
/// test the runner names. `$phrases` holds one phrase per distinct rejection in the fixture.
macro_rules! call_argument_type_refusal_high_case {
    ($($case_name:ident, $fixture:literal, $phrases:expr;)+) => { $(
        // bug: typeck accepts an argument whose type does not fit its parameter, so the callee reads a small shape as a large one (stack-buffer-overflow READ) or LLVM fails | tier: High
        #[test]
        fn $case_name() {
            assert_refused_with($fixture, $phrases);
        }
    )+ };
}

call_argument_type_refusal_high_case! {
    // case: `same(big, tiny)` binds `T = Big` from the first argument; the second was never compared with it
    v03_m9_c1g_generic_argument_for_a_type_parameter_another_argument_set_is_rejected, "v0_3_m9_c1g_generic_two_shapes_one_type_parameter.ynz", &["`tiny` has type `Tiny`, but `same` needs `Big` for `second`"];
    // case: `same<Big>(big, tiny)` fixes `T` by hand; the `Tiny` argument was not compared with it
    v03_m9_c1g_generic_argument_against_an_explicit_type_argument_is_rejected, "v0_3_m9_c1g_generic_explicit_type_argument.ynz", &["`tiny` has type `Tiny`, but `same` needs `Big` for `second`"];
    // case: a concrete `second: Big` parameter of a generic function was never compared with the argument
    v03_m9_c1g_generic_argument_for_a_concrete_parameter_is_rejected, "v0_3_m9_c1g_generic_concrete_parameter.ynz", &["This argument is `Tiny`, but `pick` expects `Big` here."];
    // case: `same(count, label)` (`T = int`, then a `string`) reached an LLVM module the verifier rejected
    v03_m9_c1g_generic_int_and_string_for_one_type_parameter_is_rejected, "v0_3_m9_c1g_generic_int_and_string.ynz", &["`label` has type `String`, but `same` needs `Int` for `second`"];
    // case: `big.useBig(tiny)`: the dot-call form compared non-receiver arguments for the union case only
    v03_m9_c1g_dot_call_argument_of_another_shape_is_rejected, "v0_3_m9_c1g_dot_call_other_shape.ynz", &["This argument is `Tiny`, but `useBig` expects `Big` here."];
    // case: `big.useBig(5)` reached LLVM and failed module verification
    v03_m9_c1g_dot_call_int_for_a_shape_is_rejected, "v0_3_m9_c1g_dot_call_int_for_shape.ynz", &["This argument is `Int`, but `useBig` expects `Big` here."];
    // case: the plain call form already compared every argument; five mismatches stay rejected while the other forms change
    v03_m9_c1g_plain_call_mismatches_stay_rejected, "v0_3_m9_c1g_plain_call_guard.ynz", &[
        "This argument is `Tiny`, but `useBig` expects `Big` here.",
        "This argument is `Int`, but `useString` expects `String` here.",
        "This argument is `String`, but `useInt` expects `Int` here.",
        "This argument is `Maybe<Int>`, but `useInt` expects `Int` here.",
        "This argument is `Array<Tiny>`, but `useBigs` expects `Array<Big>` here.",
    ];
    // case: `background pick(big, tiny)` of a generic callee with a concrete `second: Big` must be compared like the plain call
    v03_m9_c1g_rework_background_generic_argument_for_a_concrete_parameter_is_rejected, "v0_3_m9_c1g_rework_background_generic_concrete_parameter.ynz", &["This argument is `Tiny`, but `pick` expects `Big` here."];
    // case: `background pick(count, label)` (`T = int`, then a `string`) would fail LLVM verification
    v03_m9_c1g_rework_background_generic_two_types_for_one_type_parameter_is_rejected, "v0_3_m9_c1g_rework_background_generic_one_type_parameter.ynz", &["`label` has type `String`, but `pick` needs `Int` for `second`"];
    // case: `xs.add(tiny)`, `xs.set(0, tiny)`, `xs.contains(tiny)` on an `array<Big>` and `xs.get(`zero`)` (never compared; the last panicked the code generator)
    v03_m9_c1g_rework_builtin_array_method_with_wrong_argument_type_is_rejected, "v0_3_m9_c1g_rework_builtin_argument_type_array.ynz", &[
        "This argument is `Tiny`, but `add` expects `Big` here.",
        "This argument is `Tiny`, but `set` expects `Big` here.",
        "This argument is `Tiny`, but `contains` expects `Big` here.",
        "This argument is `String`, but `get` expects `Int` here.",
    ];
    // case: `m.set(`a`, tiny)` on a `map<string, Big>`, `m.get(12345)` and `nobody.or(tiny)` on a `maybe<Big>`
    v03_m9_c1g_rework_builtin_map_and_maybe_method_with_wrong_argument_type_is_rejected, "v0_3_m9_c1g_rework_builtin_argument_type_map_maybe.ynz", &[
        "This argument is `Tiny`, but `set` expects `Big` here.",
        "This argument is `Int`, but `get` expects `String` here.",
        "This argument is `Tiny`, but `or` expects `Big` here.",
    ];
    // case: `s.substring(`a`, `b`)` on a string and a `sensitive string`, and `result.or(tiny)` on a `Big errors` result
    v03_m9_c1g_rework_builtin_string_and_errors_method_with_wrong_argument_type_is_rejected, "v0_3_m9_c1g_rework_builtin_argument_type_string_errors.ynz", &[
        "This argument is `String`, but `substring` expects `Int` here.",
        "This argument is `Int`, but `contains` expects `String` here.",
        "This argument is `Tiny`, but `or` expects `Big` here.",
    ];
    // case: the `fixed<T>` arm, `array.remove`, the `map` lookups `has`/`remove` and the string `replace`, which no other fixture reaches
    v03_m9_c1g_rework_builtin_fixed_array_remove_map_lookup_and_replace_with_wrong_argument_type_is_rejected, "v0_3_m9_c1g_rework_builtin_argument_type_remaining_arms.ynz", &[
        "This argument is `String`, but `set` expects `Int` here.",
        "This argument is `String`, but `get` expects `Int` here.",
        "This argument is `Tiny`, but `contains` expects `Big` here.",
        "This argument is `Tiny`, but `remove` expects `Big` here.",
        "This argument is `Int`, but `has` expects `String` here.",
        "This argument is `Int`, but `remove` expects `String` here.",
        "This argument is `Int`, but `replace` expects `String` here.",
    ];
}

/// One named test per call form of the `call argument count refusal` typeck unit; each case is a
/// test the runner names.
macro_rules! call_argument_count_refusal_high_case {
    ($($case_name:ident, $fixture:literal, $phrases:expr;)+) => { $(
        // bug: typeck accepts a call with too few or too many arguments, so the code generator panics or LLVM reports "Incorrect number of arguments" as a compiler bug | tier: High
        #[test]
        fn $case_name() {
            assert_refused_with($fixture, $phrases);
        }
    )+ };
}

call_argument_count_refusal_high_case! {
    // case: `big.useBig()` is `useBig(big)` for a two-parameter function; the typeck zip dropped the missing argument
    v03_m9_c1g_rework_dot_call_without_a_required_argument_is_rejected, "v0_3_m9_c1g_rework_dot_call_too_few_arguments.ynz", &["`useBig` takes 2 argument(s), but 1 were given."];
    // case: `big.useBig(big, big)` is a three-argument call; the zip dropped the extras
    v03_m9_c1g_rework_dot_call_with_extra_arguments_is_rejected, "v0_3_m9_c1g_rework_dot_call_too_many_arguments.ynz", &["`useBig` takes 2 argument(s), but 3 were given."];
    // case: the `dynamic` receiver's dot-call path never compared the count with the contract's declaration
    v03_m9_c1g_rework_dynamic_dot_call_without_a_required_argument_is_rejected, "v0_3_m9_c1g_rework_dot_call_dynamic_too_few_arguments.ynz", &["`display` takes 2 argument(s), but 1 were given."];
    // case: `xs.add()` panicked the code generator, `xs.count(1)` dropped its argument
    v03_m9_c1g_rework_builtin_array_method_with_wrong_argument_count_is_rejected, "v0_3_m9_c1g_rework_builtin_arity_array.ynz", &[
        "`add` takes 2 argument(s), but 1 were given.",
        "`get` takes 2 argument(s), but 1 were given.",
        "`count` takes 1 argument(s), but 2 were given.",
    ];
    // case: the count table's own `fixed<T>` arm: `fs.get()` read an index never passed, `fs.count(1)` dropped its argument
    v03_m9_c1g_rework_builtin_fixed_method_with_wrong_argument_count_is_rejected, "v0_3_m9_c1g_rework_builtin_arity_fixed.ynz", &[
        "`get` takes 2 argument(s), but 1 were given.",
        "`count` takes 1 argument(s), but 2 were given.",
    ];
    // case: `m.set(`a`)` panicked the code generator
    v03_m9_c1g_rework_builtin_map_method_with_wrong_argument_count_is_rejected, "v0_3_m9_c1g_rework_builtin_arity_map.ynz", &[
        "`set` takes 3 argument(s), but 2 were given.",
        "`get` takes 2 argument(s), but 3 were given.",
    ];
    // case: `xs.first().or()` panicked the code generator
    v03_m9_c1g_rework_builtin_maybe_method_with_wrong_argument_count_is_rejected, "v0_3_m9_c1g_rework_builtin_arity_maybe.ynz", &[
        "`or` takes 2 argument(s), but 1 were given.",
        "`exists` takes 1 argument(s), but 2 were given.",
    ];
    // case: `s.contains()` and `s.substring(1)` panicked the code generator, `s.trim(1)` dropped its argument
    v03_m9_c1g_rework_builtin_string_method_with_wrong_argument_count_is_rejected, "v0_3_m9_c1g_rework_builtin_arity_string.ynz", &[
        "`contains` takes 2 argument(s), but 1 were given.",
        "`substring` takes 3 argument(s), but 2 were given.",
        "`trim` takes 1 argument(s), but 2 were given.",
    ];
    // case: `n.toString(1, 2)` compiled and dropped both arguments, `n.wrappingAdd()` was refused as an unknown method
    v03_m9_c1g_rework_builtin_number_method_with_wrong_argument_count_is_rejected, "v0_3_m9_c1g_rework_builtin_arity_scalar.ynz", &[
        "`toString` takes 1 argument(s), but 3 were given.",
        "`wrappingAdd` takes 2 argument(s), but 1 were given.",
    ];
    // case: `sensitive string`, `errors`-capable and `options` receivers still panicked the code generator or dropped an argument
    v03_m9_c1g_rework_builtin_method_on_a_wrapped_receiver_with_wrong_argument_count_is_rejected, "v0_3_m9_c1g_rework_builtin_arity_wrapped_receivers.ynz", &[
        "`substring` takes 3 argument(s), but 2 were given.",
        "`contains` takes 2 argument(s), but 1 were given.",
        "`or` takes 2 argument(s), but 1 were given.",
        "`failed` takes 1 argument(s), but 2 were given.",
        "`toString` takes 1 argument(s), but 2 were given.",
    ];
}

/// One named test per call shape of the `legal call run` unit; each case is a test the runner
/// names. A correct call must not be refused, and must run the instance typeck chose.
macro_rules! legal_call_run_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: typeck refuses a correct call, or codegen runs another generic instance than the one typeck chose and reads a bare shape as a tagged box | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

legal_call_run_high_case! {
    // case: agreeing generic arguments (inferred and explicit), a concrete extra parameter and a matching dot-call argument
    v03_m9_c1g_calls_whose_arguments_match_their_parameters_still_run => {
        assert_fixture_prints("v0_3_m9_c1g_legal_calls.ynz", false, "ten\n7\ntwo\n3\n9\n6\n", "legal calls must still compile and run");
    }
    // case: `maybe<T>` and `array<T>` parameters were resolved with `T` unknown, so correct calls were refused
    v03_m9_c1g_generic_parameters_holding_the_type_parameter_in_a_container_are_accepted => {
        assert_fixture_prints("v0_3_m9_c1g_generic_container_parameters.ynz", false, "3\n3\n3\n3\ntiny\n", "container parameters of a generic function must still compile and run");
    }
    // case: `first: T | nothing` with `T = Big` took a plain `Big` with no tag; the one coercion point wraps it (ASan-clean at both levels)
    v03_m9_c1g_shape_fitting_only_through_a_type_parameter_inside_a_union_is_wrapped => {
        support::at_both_optimization_levels(|no_optimize| {
            assert_asan_clean_and_prints("v0_3_m9_c1g_generic_type_parameter_inside_union.ynz", no_optimize, "hello\nhello\n");
        });
    }
    // case: `T | nothing` with `T = Big | Tiny` is a union in a union, a different type from the flat argument, so a correct call was refused
    v03_m9_c1g_union_bound_to_a_type_parameter_inside_a_union_parameter_is_accepted => {
        assert_fixture_prints("v0_3_m9_c1g_generic_union_argument_in_union_parameter.ynz", false, "hello\ntiny2\n", "a union bound to a type parameter inside a union parameter must still compile and run");
    }
    // case: `pickA(tiny, big)` fits both `pickA<Big>` and `pickA<Big | Tiny>`; codegen re-derived the instance and a wrong pick reads a bare `Big` as a tagged box
    v03_m9_c1g_generic_call_runs_the_instance_the_type_checker_chose => {
        assert_fixture_prints("v0_3_m9_c1g_generic_call_runs_its_checked_instance.ynz", false, "hello\nhello\ntiny\n", "every generic call must run the instance it was checked with");
    }
    // case: `show<Big>` took the name of a user function `show_shape_Big(Tiny)`, which then ran on a `Big`
    v03_m9_c1g_generic_instance_name_never_belongs_to_a_user_function => {
        assert_fixture_prints("v0_3_m9_c1g_generic_instance_name_is_its_own.ynz", false, "generic\n", "`show(big)` must run the `show<Big>` instance, not the user function with its old name");
    }
    // case: `second<A_shape_B, int>` and `second<A, B_int>` were both `second_shape_A_shape_B_int`, so one call ran the other's body
    v03_m9_c1g_generic_instances_never_share_a_name => {
        assert_fixture_prints("v0_3_m9_c1g_generic_instances_do_not_share_a_name.ynz", false, "42\nx\n", "each instance of `second` must run its own body");
    }
    // case: a legal `background` call of a generic function keeps compiling and starts the instance typeck recorded
    v03_m9_c1g_rework_legal_generic_background_call_still_runs => {
        assert_fixture_prints("v0_3_m9_c1g_rework_legal_generic_background_call.ynz", false, "q3\ngeneric-background-done\n", "a legal generic background call must still compile and run");
    }
}
