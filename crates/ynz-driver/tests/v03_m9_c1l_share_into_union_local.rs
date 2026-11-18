//! v0.3-M9 C1l (card #493): a `share` parameter stored into a union local is refused today, and
//! the two ways the refusal advises (`.copy()`, a `give` parameter) run balanced and ASan-clean.
//! Both refusals (this one and `TransferNeedsCopy` on a piece stored into a literal) are kept over
//! a hidden copy — the Chief ruled 2026-11-18: "agreed as long as it follows the golden rules and
//! foundatinal rule." So the WHY must name this holder, and the advice is a `give` or a written
//! `.copy()` (Golden Rule 11, Golden Rule 10).

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
        // The WHY (Golden Rule 11) names this function and the dangling read, not a generic one.
        "the caller's own binding stays usable after `keep` returns",
        "the caller would be reading memory that is already gone",
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

// Guards a generic WHY on `TransferNeedsCopy` (Medium: the teaching text is the refusal's whole
// contract, Golden Rule 11): the WHY must name this holder (`mkm`) and say what goes wrong, not
// offer a menu of places a value might be held.
#[test]
fn a_piece_stored_into_a_literal_is_refused_with_a_why_that_names_the_holder() {
    let fixture = "v0_3_m9_c1l_piece_into_literal_refused.ynz";
    let compiled = compile(fixture);
    assert!(
        !compiled.success,
        "{fixture}: must be refused, but compiled:\n{}",
        compiled.diagnostics
    );
    for fragment in [
        "Give away a copy instead: `mkm().copy()`",
        "`mkm()` is what `mkm` returns, and `mkm` may return a piece of something someone else holds, so another holder may free it too",
        "the same memory would be freed twice",
    ] {
        assert!(
            compiled.diagnostics.contains(fragment),
            "{fixture}: diagnostic must contain {fragment:?}; got:\n{}",
            compiled.diagnostics
        );
    }
    assert!(
        !compiled
            .diagnostics
            .contains("through the field, the item, the loop"),
        "{fixture}: the WHY must not be the generic menu:\n{}",
        compiled.diagnostics
    );
}
