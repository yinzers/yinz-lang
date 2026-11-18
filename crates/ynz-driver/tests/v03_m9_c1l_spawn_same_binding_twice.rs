//! v0.3-M9 C1l (card #493): a `background` call that passes one binding in two positions hands the
//! binding over once, at its last read in the call; the earlier read is a copy.

mod support;

use support::{
    assert_built_balanced_and_asan_clean_printing_one_of, at_both_optimization_levels, build,
};

// Bug: the second read of `rows` cloned the null the first read left in its slot, and the
// program aborted (High: data integrity and release).
#[test]
fn one_array_binding_passed_twice_to_a_task_is_copied_twice_and_released_once() {
    let fixture = "v0_3_m9_c1l_spawn_same_binding_twice.ynz";
    // Nothing waits for a fire-and-forget task, so its prints may land after `done`.
    let accepted = ["3\n3\ndone\n", "done\n3\n3\n"];
    at_both_optimization_levels(|no_optimize| {
        let built = build(fixture, no_optimize);
        assert_built_balanced_and_asan_clean_printing_one_of(
            fixture, &built, &accepted, no_optimize,
        );
    });
}
