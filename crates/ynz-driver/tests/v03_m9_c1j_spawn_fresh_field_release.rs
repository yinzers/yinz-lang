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
// with the text of the one spawn refusal (High: data integrity and release).
#[test]
fn a_field_with_no_owned_copy_of_a_fresh_call_result_is_refused_for_a_task() {
    assert_refused_with(
        "v0_3_m9_c1j_spawn_fresh_field_refused.ynz",
        &[
            "A `Fixed<Array<String>>` read out of another value",
            "cannot be handed to a background task",
            "`Array<String>` values",
            "when this line ends",
            "background crunch(make().rows)",
        ],
    );
}

/// `fixture` must be refused by typeck: a user error carrying every fragment, never a backend
/// error.
fn assert_refused_with(fixture: &str, fragments: &[&str]) {
    let compiled = compile(fixture);
    assert!(
        !compiled.success,
        "{fixture}: must be refused, but compiled:\n{}",
        compiled.diagnostics
    );
    for fragment in fragments {
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

// A channel piece of a fresh value is not refused: the spawn shares a channel with the task by a
// counted reference (`spawn_rehoming`), whatever else the argument is. Bug: the piece refusal read
// the owned-copy table, which refuses a channel, and rejected a program `main` runs correctly
// (3 allocations, 3 frees) (High: data integrity and release).
#[test]
fn a_channel_field_of_a_fresh_call_result_is_shared_with_the_task_and_the_rest_released() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_fresh_channel_field.ynz", "sent\ndone\n");
}

// An array of channels has no independent copy: it stays refused for a fresh value.
#[test]
fn an_array_of_channels_field_of_a_fresh_call_result_stays_refused_for_a_task() {
    assert_refused_with(
        "v0_3_m9_c1j_spawn_fresh_channels_array_refused.ynz",
        &[
            "`Array<Channel<Int>>`",
            "cannot be handed to a background task",
            "background ping(make().lines)",
        ],
    );
}

// Bug: `background f(bag.rows)` with an uncopyable `rows` handed the task the array `bag` still
// owns, and `launch` released `bag` while the task read it (garbage count, 4 allocations and 4
// frees) (High: data integrity and release).
#[test]
fn an_array_of_channels_field_of_a_named_value_is_refused_for_a_task() {
    assert_refused_with(
        "v0_3_m9_c1j_spawn_named_channels_array_refused.ynz",
        &[
            "`Array<Channel<Int>>`",
            "cannot be handed to a background task",
            "background f(bag.rows)",
        ],
    );
}

// Bug: the same through a `fixed<array<string>, 2>` field: the task read the dead frame (crash,
// ASan stack-use-after-return) (High: data integrity and release).
#[test]
fn a_fixed_of_arrays_field_of_a_named_value_is_refused_for_a_task() {
    assert_refused_with(
        "v0_3_m9_c1j_spawn_named_fixed_refused.ynz",
        &[
            "`Fixed<Array<String>>`",
            "cannot be handed to a background task",
            "background f(bag.rows)",
        ],
    );
}

// Control: a named piece the task gets its own copy of stays accepted.
#[test]
fn a_copyable_field_of_a_named_value_given_to_a_task_stays_accepted() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_named_copyable_field.ynz", "2\nA\ndone\n");
}

// A union field of a named value stays accepted: a union parameter gets a slot of its own in the
// task (`spawn_rehoming`: `UnionSlot`), so `launch` can return and release `bag` while the task
// still reads its copy. Bug it guards: on `main` this ran wrong (the wrong variant at O2, nothing
// at O0, ASan stack-use-after-return) (High: data integrity and release).
#[test]
fn a_union_field_of_a_named_value_given_to_a_task_stays_accepted() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_named_union_field.ynz", "BOXED\ndone\n");
}

// A union variant that owns heap: the slot copies the payload and releases it once. Bug it
// catches: a slot that shares the array or the string with `bag` frees them twice, or leaks them
// (High: data integrity and release).
#[test]
fn a_union_field_with_a_heap_payload_of_a_named_value_is_copied_for_the_task() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_union_heap_field.ynz", "2\nA\nL\ndone\n");
}

// The same payload read out of a fresh call result: the temporary is released once and the task
// reads its own copy (High: data integrity and release).
#[test]
fn a_union_field_with_a_heap_payload_of_a_fresh_call_result_is_copied_for_the_task() {
    assert_fixture_balanced("v0_3_m9_c1j_spawn_union_heap_fresh.ynz", "2\nA\nL\ndone\n");
}

// A named union that owns heap is read by the spawner after the spawn and by the task after the
// spawner has returned: neither side may read freed storage (High: data integrity and release).
#[test]
fn a_named_union_with_a_heap_payload_read_after_the_spawn_is_copied_for_the_task() {
    assert_fixture_balanced(
        "v0_3_m9_c1j_spawn_union_heap_reused.ynz",
        "L\n2\nA\nL\ndone\n",
    );
}

// A union with no shape variant and no tag (`array<int> | nothing`, `int | float`) is the variant's
// raw pointer or bits, and codegen makes a task slot only for an envelope union. The spawn must not
// promise one: a named one is given as on `main`. Bug: `spawn_rehoming` said `UnionSlot` for every
// union, so this program stopped the build with an internal error `main` never gave (High: data
// integrity and release).
#[test]
fn a_named_union_without_an_envelope_is_given_to_a_task_as_before() {
    assert_fixture_balanced(
        "v0_3_m9_c1j_spawn_plain_union_given.ynz",
        "task\ntask\ndone\n",
    );
}

// The piece form of the same union: no slot to copy into, so the task would share the array with
// the value it is read from. Refused by typeck with the one spawn refusal, never a backend error.
#[test]
fn a_field_of_a_union_without_an_envelope_of_a_fresh_call_result_is_refused_for_a_task() {
    assert_refused_with(
        "v0_3_m9_c1j_spawn_plain_union_field_refused.ynz",
        &[
            "`Array<Int> | Nothing`",
            "cannot be handed to a background task",
            "background crunch(make().items)",
        ],
    );
}
