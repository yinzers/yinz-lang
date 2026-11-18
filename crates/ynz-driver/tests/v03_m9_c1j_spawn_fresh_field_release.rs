//! v0.3-M9 C1j (card #491): a `background` call that passes a field of a fresh call result
//! (`make().items`) releases the whole temporary once, handing the task only a copy of the field.

mod support;

use support::{
    assert_built_balanced_and_asan_clean_printing_one_of, at_both_optimization_levels, build,
    compile,
};

/// Every fixture ends `wait sleep(50)` then `print(done)`, while the task it spawned prints
/// `task_output`. Nothing waits for a fire-and-forget task, so the language fixes neither order:
/// the balance and ASan are asserted in both orders.
fn assert_fixture_balanced(fixture: &str, task_output_then_done: &str) {
    let task_output = task_output_then_done
        .strip_suffix("done\n")
        .expect("expected output ends with the spawner's `done` line");
    let done_first = format!("done\n{task_output}");
    at_both_optimization_levels(|no_optimize| {
        let built = build(fixture, no_optimize);
        assert_built_balanced_and_asan_clean_printing_one_of(
            fixture,
            &built,
            &[task_output_then_done, done_first.as_str()],
            no_optimize,
        );
    });
}

// Bug: 2 of 4 strings leaked; the original `fixed` field was never released (High: data
// integrity and release).
#[test]
fn a_fixed_field_of_a_fresh_call_result_given_to_a_forgotten_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_fresh_field_fixed.ynz", "2\nA\ndone\n");
}

// Bug: the original `array` field and two of its strings leaked (High: data integrity and release).
#[test]
fn an_array_field_of_a_fresh_call_result_given_to_a_forgotten_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_fresh_field_array.ynz", "2\nA\ndone\n");
}

// Bug: the original shape field's strings leaked (High: data integrity and release).
#[test]
fn a_shape_field_of_a_fresh_call_result_given_to_a_forgotten_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_fresh_field_shape.ynz", "A\nB\ndone\n");
}

// The same producer through a longer field chain: the whole temporary (outer shape, inner shape,
// array, strings) is released once (High: data integrity and release).
#[test]
fn a_field_two_levels_deep_of_a_fresh_call_result_given_to_a_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_fresh_field_nested.ynz", "2\nA\ndone\n");
}

// The same producer through an index: `make()[0]` is a `maybe<string>` read out of the fresh array.
#[test]
fn an_element_of_a_fresh_call_result_given_to_a_task_is_released() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_fresh_index.ynz", "A\ndone\n");
}

// A piece with no owned copy (`fixed<array<string>>`) is handed to the task as it is, so
// releasing the temporary at the statement end would free what the task still reads (garbage
// output), and not releasing it leaks silently. The spawn is refused at compile time instead,
// with the owned-copy table's own text (High: data integrity and release).
#[test]
fn a_field_with_no_owned_copy_of_a_fresh_call_result_is_refused_for_a_task() {
    let fixture = "v0_3_m9_c1j_spawn_fresh_field_refused.ynz";
    let compiled = compile(fixture);
    assert!(
        !compiled.success,
        "{fixture}: must be refused, but compiled:\n{}",
        compiled.diagnostics
    );
    for fragment in [
        "A `fixed<array<string>>` read out of a value this line builds cannot be handed to a background task",
        "`array<string>` values",
        "released when the line ends",
        "background crunch(make().rows)",
    ] {
        assert!(
            compiled.diagnostics.contains(fragment),
            "{fixture}: diagnostic must contain {fragment:?}; got:\n{}",
            compiled.diagnostics
        );
    }
    for internal in ["codegen:", "internal error", "compiler bug"] {
        assert!(
            !compiled.diagnostics.contains(internal),
            "{fixture}: a user error must come from typeck ({internal:?}):\n{}",
            compiled.diagnostics
        );
    }
}
