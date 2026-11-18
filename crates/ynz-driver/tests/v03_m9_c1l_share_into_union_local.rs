//! v0.3-M9 C1l (card #493): a `share` parameter stored into a union local is refused today, and
//! the two ways the refusal advises (`.copy()`, a `give` parameter) run balanced and ASan-clean.
//! Pins the refusal pending the Chief's ruling (Chief Architect, 2026-11-18).

mod support;

use support::{assert_fixture_balanced_and_asan_clean, compile};

// Guards a `share` parameter silently stored away, which leaves two holders of one array (High:
// data integrity and release).
#[test]
fn a_share_parameter_stored_into_a_union_local_is_refused_with_the_teaching_text() {
    let fixture = "v0_3_m9_c1l_share_into_union_local_refused.ynz";
    let compiled = compile(fixture);
    assert!(
        !compiled.success,
        "{fixture}: must be refused, but compiled:\n{}",
        compiled.diagnostics
    );
    for fragment in [
        "declared `share`",
        "gives it away",
        "Declare it `give` in `keep`'s signature",
        "let held = first.copy()",
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

// Guards the advised `.copy()` form leaking or double-freeing the array (High: data integrity and
// release).
#[test]
fn a_copy_of_a_share_parameter_stored_into_a_union_local_is_released() {
    assert_fixture_balanced_and_asan_clean(
        "v0_3_m9_c1l_share_into_union_local_copy.ynz",
        "kept\n3\n",
    );
}

// Guards the advised `give` form leaking or double-freeing the array (High: data integrity and
// release).
#[test]
fn a_give_parameter_stored_into_a_union_local_is_released() {
    assert_fixture_balanced_and_asan_clean("v0_3_m9_c1l_share_into_union_local_give.ynz", "kept\n");
}
