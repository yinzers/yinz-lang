//! v0.3-M9 C1g rework (#424): a generic function started with `background` compares each
//! argument with its parameter, and a dot-call passes the number of arguments the function
//! declares. Tier: High (type checker; untrusted program input; the pre-fix flow read a 1-byte
//! `Tiny` as a 16-byte `Big`, or reached LLVM with a call whose argument count was wrong).
//!
//! The fixtures go through the public compile boundary (`ynz build`). Each refused case asserts
//! the teaching text (WHAT, WHAT-INSTEAD, WHY from the same diagnostics the plain call form
//! gives) and that the compiler did not report one of its own bugs.
//!
//! The two `background` cases are GUARDS: probed at 3a4de04 they were already refused (the
//! statement form reaches `check_generic_fn_call`), so they pin that route; the dot-call cases
//! were red there.
//!
//! Round 1 (rule fixer): the built-in collection, `maybe`, string and number methods are dot-calls
//! too. The type checker never counted their arguments, so `xs.add()` panicked the code generator
//! and `n.toString(1, 2)` dropped its arguments; each method family has its own fixture.
//!
//! Round 3 (security fixer): a built-in method's argument TYPES were never compared with its
//! parameters, so `xs.add(tiny)` on an `array<Big>` read a `Tiny` as a `Big`; three fixtures, one
//! per receiver group.
//!
//! Test count: 18 — one per refused case (two `background` generic calls, three dot-call argument
//! counts: too few, too many, through a `dynamic` receiver), the teaching text of the dot-call
//! form, one per built-in method family (array, `fixed`, map, `maybe`, string, number), one for
//! the `sensitive`, `errors`-capable and `options` receivers (round 2), three for built-in
//! argument types (round 3), one for the remaining typed positions (round 3, Test) and the
//! legal generic `background` call that must keep running.
//! Justified above 13: every test is one compile of its own fixture and names one bug on one
//! receiver family, so a regression in one arm of `builtin_method_argument_count` or
//! `builtin_method_argument_slots` fails one named test.

mod support;

use support::compile;

/// The compile must fail with a diagnostic that holds every fragment, and the failure must be
/// the program's, not the compiler's.
fn assert_rejected_with(fixture_name: &str, fragments: &[&str]) {
    let compiled = compile(fixture_name);
    assert!(
        !compiled.success,
        "{fixture_name}: the call compiled:\n{}",
        compiled.diagnostics
    );
    assert!(
        !compiled.diagnostics.contains("compiler bug"),
        "{fixture_name}: the compiler reported its own bug instead of teaching:\n{}",
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
fn v03_m9_c1g_rework_background_generic_argument_for_a_concrete_parameter_is_rejected() {
    // GUARD (reported bug, not reproducible at 3a4de04): `background pick(big, tiny)` with
    // `second: Big` on a generic callee must be compared like the plain call, or the task reads a
    // `Tiny` as a `Big`.
    // TIER: High (type checker, memory safety).
    assert_rejected_with(
        "v0_3_m9_c1g_rework_background_generic_concrete_parameter.ynz",
        &[
            "This argument is `Tiny`, but `pick` expects `Big` here.",
            "Pass a `Big` value.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_background_generic_two_types_for_one_type_parameter_is_rejected() {
    // GUARD (reported bug, not reproducible at 3a4de04): `background pick(count, label)`
    // (`T = int`, then a `string`) would fail LLVM
    // verification, a compiler-bug report instead of a diagnostic. TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_rework_background_generic_one_type_parameter.ynz",
        &[
            "`label` has type `String`, but `pick` needs `Int` for `second`",
            "the argument `count` for `first` already made it `Int`",
            "Pass a value of type `Int` for `second`",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_dot_call_without_a_required_argument_is_rejected() {
    // BUG: `big.useBig()` is `useBig(big)` for a two-parameter function; the typeck zip dropped
    // the missing argument and LLVM reported "Incorrect number of arguments" as a compiler bug.
    // TIER: High (type checker).
    assert_rejected_with(
        "v0_3_m9_c1g_rework_dot_call_too_few_arguments.ynz",
        &[
            "`useBig` takes 2 argument(s), but 1 were given.",
            "Call it with 2 argument(s).",
            "Every function call must match the number of arguments the function declares.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_dot_call_with_extra_arguments_is_rejected() {
    // BUG: `big.useBig(big, big)` is a three-argument call to a two-parameter function; the
    // typeck zip dropped the extras and LLVM reported a compiler bug. TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_rework_dot_call_too_many_arguments.ynz",
        &[
            "`useBig` takes 2 argument(s), but 3 were given.",
            "Call it with 2 argument(s).",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_dynamic_dot_call_without_a_required_argument_is_rejected() {
    // BUG: the `dynamic` receiver's dot-call path never compared the argument count with the
    // contract's declaration. TIER: High (type checker).
    assert_rejected_with(
        "v0_3_m9_c1g_rework_dot_call_dynamic_too_few_arguments.ynz",
        &[
            "`display` takes 2 argument(s), but 1 were given.",
            "Call it with 2 argument(s).",
            "Put 1 argument(s) inside the parentheses of `shown.display(…)`.",
            "A dot call counts the value before the dot, `shown`, as the first argument.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_dot_call_arity_teaches_the_receiver_in_every_slot() {
    // A dot call counts the value before the dot as the first argument, so "takes 2, but 1 were
    // given" for `big.useBig()` only makes sense once the text says so (Golden Rule 3: WHAT,
    // WHAT-INSTEAD and WHY, each tied to this call). TIER: High (a diagnostic that sends the
    // reader to write a second wrong call teaches nothing).
    assert_rejected_with(
        "v0_3_m9_c1g_rework_dot_call_too_few_arguments.ynz",
        &[
            "Put 1 argument(s) inside the parentheses of `big.useBig(…)`.",
            "A dot call counts the value before the dot, `big`, as the first argument.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_builtin_array_method_with_wrong_argument_count_is_rejected() {
    // BUG (round 1, Security): `xs.add()` panicked the code generator; `xs.count(1)` dropped its
    // argument. TIER: High (type checker).
    assert_rejected_with(
        "v0_3_m9_c1g_rework_builtin_arity_array.ynz",
        &[
            "`add` takes 2 argument(s), but 1 were given.",
            "`get` takes 2 argument(s), but 1 were given.",
            "`count` takes 1 argument(s), but 2 were given.",
            "Put 1 argument(s) inside the parentheses of `xs.add(…)`.",
            "Put nothing inside the parentheses of `xs.count()`.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_builtin_fixed_method_with_wrong_argument_count_is_rejected() {
    // BUG (round 2, Test): the count table has its own `fixed<T>` arm and no test pinned it;
    // `fs.get()` read an index that was never passed and `fs.count(1)` dropped its argument.
    // TIER: High (type checker).
    assert_rejected_with(
        "v0_3_m9_c1g_rework_builtin_arity_fixed.ynz",
        &[
            "`get` takes 2 argument(s), but 1 were given.",
            "`count` takes 1 argument(s), but 2 were given.",
            "Put 1 argument(s) inside the parentheses of `fs.get(…)`.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_builtin_map_method_with_wrong_argument_count_is_rejected() {
    // BUG (round 1, Security): `m.set(`a`)` panicked the code generator. TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_rework_builtin_arity_map.ynz",
        &[
            "`set` takes 3 argument(s), but 2 were given.",
            "`get` takes 2 argument(s), but 3 were given.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_builtin_maybe_method_with_wrong_argument_count_is_rejected() {
    // BUG (round 1, Security): `xs.first().or()` panicked the code generator. TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_rework_builtin_arity_maybe.ynz",
        &[
            "`or` takes 2 argument(s), but 1 were given.",
            "`exists` takes 1 argument(s), but 2 were given.",
            "counts the value before the dot, `xs.first()`, as the first argument.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_builtin_string_method_with_wrong_argument_count_is_rejected() {
    // BUG (round 1, Security): `s.contains()` and `s.substring(1)` panicked the code generator;
    // `s.trim(1)` dropped its argument. TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_rework_builtin_arity_string.ynz",
        &[
            "`contains` takes 2 argument(s), but 1 were given.",
            "`substring` takes 3 argument(s), but 2 were given.",
            "`trim` takes 1 argument(s), but 2 were given.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_builtin_number_method_with_wrong_argument_count_is_rejected() {
    // BUG (round 1, Security): `n.toString(1, 2)` compiled and dropped both arguments;
    // `n.wrappingAdd()` was refused as an unknown method. TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_rework_builtin_arity_scalar.ynz",
        &[
            "`toString` takes 1 argument(s), but 3 were given.",
            "`wrappingAdd` takes 2 argument(s), but 1 were given.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_legal_generic_background_call_still_runs() {
    // GUARD against over-rejecting: a `background` call of a generic function whose arguments
    // match its parameters keeps compiling and running (the instance typeck recorded for the
    // call is the one codegen starts). TIER: High (the check must not reject a correct program).
    support::assert_fixture_prints(
        "v0_3_m9_c1g_rework_legal_generic_background_call.ynz",
        false,
        "q3\ngeneric-background-done\n",
        "a legal generic background call must still compile and run",
    );
}

#[test]
fn v03_m9_c1g_rework_builtin_method_on_a_wrapped_receiver_with_wrong_argument_count_is_rejected() {
    // BUG (round 2): the argument count was only checked for the plain collection, `maybe`,
    // string and number receivers; `secret.substring(1)` (`sensitive string`) and
    // `readConfig().or()` (`errors`-capable) still panicked the code generator, and
    // `color.toString(1)` (`options`) dropped its argument. TIER: High (type checker).
    assert_rejected_with(
        "v0_3_m9_c1g_rework_builtin_arity_wrapped_receivers.ynz",
        &[
            "`substring` takes 3 argument(s), but 2 were given.",
            "`contains` takes 2 argument(s), but 1 were given.",
            "`or` takes 2 argument(s), but 1 were given.",
            "`failed` takes 1 argument(s), but 2 were given.",
            "`toString` takes 1 argument(s), but 2 were given.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_builtin_array_method_with_wrong_argument_type_is_rejected() {
    // BUG (round 3, Security): only the count of a built-in method's arguments was checked, not
    // their types. `xs.add(tiny)`, `xs.set(0, tiny)` and `xs.contains(tiny)` on an `array<Big>`
    // compiled and read the 1-byte `Tiny` as a 16-byte `Big` (ASan stack-buffer-overflow READ);
    // `xs.get(`zero`)` panicked the code generator. TIER: High (type checker, memory safety).
    assert_rejected_with(
        "v0_3_m9_c1g_rework_builtin_argument_type_array.ynz",
        &[
            "This argument is `Tiny`, but `add` expects `Big` here.",
            "This argument is `Tiny`, but `set` expects `Big` here.",
            "This argument is `Tiny`, but `contains` expects `Big` here.",
            "This argument is `String`, but `get` expects `Int` here.",
            "Pass a `Big` value.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_builtin_map_and_maybe_method_with_wrong_argument_type_is_rejected() {
    // BUG (round 3, Security): `m.set(`a`, tiny)` on a `map<string, Big>` and `nobody.or(tiny)`
    // on a `maybe<Big>` compiled and read the `Tiny` as a `Big` (ASan stack-buffer-overflow
    // READ); `m.get(12345)` failed LLVM verification. TIER: High (type checker, memory safety).
    assert_rejected_with(
        "v0_3_m9_c1g_rework_builtin_argument_type_map_maybe.ynz",
        &[
            "This argument is `Tiny`, but `set` expects `Big` here.",
            "This argument is `Int`, but `get` expects `String` here.",
            "This argument is `Tiny`, but `or` expects `Big` here.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_builtin_string_and_errors_method_with_wrong_argument_type_is_rejected() {
    // BUG (round 3, Security): `s.substring(`a`, `b`)` on a `string` and on a `sensitive string`
    // panicked the code generator; `result.or(tiny)` on a `Big errors` result compiled with a
    // `Tiny` fallback that is read as a `Big` when the call fails. TIER: High (type checker).
    assert_rejected_with(
        "v0_3_m9_c1g_rework_builtin_argument_type_string_errors.ynz",
        &[
            "This argument is `String`, but `substring` expects `Int` here.",
            "This argument is `Int`, but `contains` expects `String` here.",
            "This argument is `Tiny`, but `or` expects `Big` here.",
        ],
    );
}

#[test]
fn v03_m9_c1g_rework_builtin_fixed_array_remove_map_lookup_and_replace_with_wrong_argument_type_is_rejected(
) {
    // BUG (round 3, Test): the position table has its own `fixed<T>` arm, `array.remove`, the
    // `map` lookups `has`/`remove` and the string `replace`, and no other fixture reaches them:
    // deleting one arm leaves the three fixtures above green while `fs.contains(tiny)` reads a
    // `Tiny` as a `Big`. Each fragment below appears for one receiver only. TIER: High.
    assert_rejected_with(
        "v0_3_m9_c1g_rework_builtin_argument_type_remaining_arms.ynz",
        &[
            "This argument is `String`, but `set` expects `Int` here.",
            "This argument is `String`, but `get` expects `Int` here.",
            "This argument is `Tiny`, but `contains` expects `Big` here.",
            "This argument is `Tiny`, but `remove` expects `Big` here.",
            "This argument is `Int`, but `has` expects `String` here.",
            "This argument is `Int`, but `remove` expects `String` here.",
            "This argument is `Int`, but `replace` expects `String` here.",
        ],
    );
}
