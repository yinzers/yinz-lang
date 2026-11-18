//! v0.3-M9 C1i (card #472): a `fixed` value whose items cannot be copied is refused as a
//! `background` argument at compile time, whether or not the spawner reads it afterwards. A
//! `fixed` value is kept with the function that made it; the spawn used to hand the task a pointer
//! into the dead frame, and nothing released the items.
//!
//! High tier: a compiled program that read freed memory (data integrity, memory safety).

mod support;

use support::{assert_fixture_balanced_and_asan_clean as assert_fixture_balanced, compile};

fn assert_refused_as_a_handed_over_fixed(fixture_name: &str, fixed_type: &str, array_type: &str) {
    let compiled = compile(fixture_name);
    assert!(
        !compiled.success,
        "{fixture_name}: must be refused, but compiled:\n{}",
        compiled.diagnostics
    );
    let fragments = [
        format!(
            "`xs` is a `{fixed_type}`, and a `{fixed_type}` cannot be handed to a background task"
        ),
        format!("Declare `xs` as an `array<{array_type}>` instead"),
        "kept with the function that made it".to_string(),
    ];
    for fragment in &fragments {
        assert!(
            compiled.diagnostics.contains(fragment),
            "{fixture_name}: diagnostic must contain {fragment:?}; got:\n{}",
            compiled.diagnostics
        );
    }
    // The text for a spawner that keeps reading the value would teach something that does not
    // work here: handing it over is what this program already does.
    for wrong in [
        "still reads it after the task starts",
        "Hand `xs` over instead",
    ] {
        assert!(
            !compiled.diagnostics.contains(wrong),
            "{fixture_name}: the hand-over advice does not apply ({wrong:?}); got:\n{}",
            compiled.diagnostics
        );
    }
    for internal in ["codegen:", "internal error", "compiler bug"] {
        assert!(
            !compiled.diagnostics.contains(internal),
            "{fixture_name}: a user error must come from typeck ({internal:?}):\n{}",
            compiled.diagnostics
        );
    }
}

#[test]
fn a_handed_over_fixed_of_arrays_is_refused_for_a_task() {
    assert_refused_as_a_handed_over_fixed(
        "v0_3_m9_c1i_spawn_fixed_array_refused.ynz",
        "fixed<array<string>>",
        "array<string>",
    );
}

#[test]
fn a_handed_over_fixed_of_maps_is_refused_for_a_task() {
    assert_refused_as_a_handed_over_fixed(
        "v0_3_m9_c1i_spawn_fixed_map_refused.ynz",
        "fixed<map<string, int>>",
        "map<string, int>",
    );
}

// The recovery the diagnostic teaches must work: the items in an `array` are handed to a task and
// released (frees == allocations at both levels, no AddressSanitizer report).
#[test]
fn an_array_of_arrays_handed_to_a_task_is_read_and_released() {
    assert_fixture_balanced("v0_3_m9_c1i_spawn_array_of_arrays_given.ynz", "2\ndone\n");
}
