//! v0.3-M9 C1f (#423): a union stored into anything that outlives the frame that made it must
//! read back correctly: the slot holds `{ tag, payload }` BY VALUE with the payload inline, sized to
//! the largest variant (`IMP-unions.md` "Stored union slot"); only a recursive union boxes a shape
//! variant in a heap cell. Tier: High (memory safety / data integrity).
//!
//! Each fixture is one escape kind, not a copy of another:
//!   - `escape`: a union array returned from the function that filled it.
//!   - `loop`: plain variants stored into containers inside a loop (one per-site slot would be
//!     shared by every element).
//!   - `keep_give`: a callee that does not own the list stores a `give` union into it; both the
//!     plain-variant argument and the old `let as_thing: Thing = big` spelling.
//!   - `shape_field_escape`: a shape literal storing a plain variant in a union field and
//!     returned, plus a field store into a `lend` shape from a callee.
//!   - `add` / `map_set`: straight-line stores, the guard that the one coercion point of
//!     `check_argument_type` wraps a built-in container slot.
//!
//! Count: each escape-kind fixture runs four ways (plain and AddressSanitizer, -O0 and -O2); the
//! add / map_set / recursive-boxed / `wait`-staged / `background` guards run under ASan only, at
//! both levels (ASan asserts the stdout too); the refusals compile only. The plain runs are the
//! program as shipped: ASan changes the stack layout (redzones, fake frames), so a read that is
//! only wrong without the sanitizer shows there. -O0 vs -O2 keeps a frame the optimizer reuses
//! apart from one it does not (stack-use-after-return vs after-scope).
//!
//! The heap homes #410 and #421 built (union return, `wait`-staged argument, hidden default)
//! are guarded by `v03_m9_union_wrap.rs` and `v03_m9_union_hidden_default.rs`.

mod support;

use support::{
    assert_asan_clean_and_prints as assert_asan_clean, assert_fixture_prints as assert_prints,
};

const WHAT: &str = "a union stored where it outlives its frame must keep its tag and payload";

macro_rules! by_value_tests {
    ($fixture:expr, $stdout:expr, $optimized:ident, $no_optimize:ident, $asan_optimized:ident, $asan_no_optimize:ident) => {
        #[test]
        fn $optimized() {
            // WHY: the value read back must be the variant that was stored.
            assert_prints($fixture, false, $stdout, WHAT);
        }

        #[test]
        fn $no_optimize() {
            // WHY: same defect at -O0, where the dead frame's slots are untouched.
            assert_prints($fixture, true, $stdout, WHAT);
        }

        #[test]
        fn $asan_optimized() {
            // WHY: stack-use-after-scope names the dangling read; the output can look right.
            assert_asan_clean($fixture, false, $stdout);
        }

        #[test]
        fn $asan_no_optimize() {
            // WHY: stack-use-after-return at -O0.
            assert_asan_clean($fixture, true, $stdout);
        }
    };
}

// Bug: a union array returned from the function that filled it holds pointers into the dead
// frame (envelope and payload). Tier: High.
by_value_tests!(
    "v0_3_m9_union_container_escape.ynz",
    "big hello\ntiny\nend\n",
    v03_m9_union_container_escape_optimized,
    v03_m9_union_container_escape_no_optimize,
    v03_m9_union_container_escape_asan_optimized,
    v03_m9_union_container_escape_asan_no_optimize
);

// Bug: a loop stores into one per-site envelope slot, so every element reads the last value
// stored (ASan-clean, silently wrong). Tier: High.
by_value_tests!(
    "v0_3_m9_union_container_loop.ynz",
    "tiny\nbig b1\ntiny\nbig b3\nbig m0\nbig m1\nend\n",
    v03_m9_union_container_loop_optimized,
    v03_m9_union_container_loop_no_optimize,
    v03_m9_union_container_loop_asan_optimized,
    v03_m9_union_container_loop_asan_no_optimize
);

// Bug: `list.add(thing)` in a callee stored the caller-frame envelope pointer. Tier: High.
by_value_tests!(
    "v0_3_m9_union_keep_give.ynz",
    "big kept\nbig old\ntiny\nend\n",
    v03_m9_union_keep_give_optimized,
    v03_m9_union_keep_give_no_optimize,
    v03_m9_union_keep_give_asan_optimized,
    v03_m9_union_keep_give_asan_no_optimize
);

// Bug: a shape literal storing a plain variant in a union field, returned from its frame, read
// `none`/`int` at -O0 and segfaulted at -O2; a `lend` field store dangled likewise. Tier: High.
by_value_tests!(
    "v0_3_m9_union_shape_field_escape.ynz",
    "big escaped\ntiny\nbig stored\nend\n",
    v03_m9_union_shape_field_escape_optimized,
    v03_m9_union_shape_field_escape_no_optimize,
    v03_m9_union_shape_field_escape_asan_optimized,
    v03_m9_union_shape_field_escape_asan_no_optimize
);

// Guards (green on the base): straight-line `add` and `map.set` of a plain variant wrap through
// the one coercion point. Tier: High.
#[test]
fn v03_m9_union_container_add_asan_optimized() {
    // WHY: a bare shape read as a 16-byte envelope is an out-of-bounds read.
    assert_asan_clean(
        "v0_3_m9_union_container_add.ynz",
        false,
        "tiny\nbig hello\ntiny\nbig world\nend\n",
    );
}

#[test]
fn v03_m9_union_container_add_asan_no_optimize() {
    // WHY: same read at -O0.
    assert_asan_clean(
        "v0_3_m9_union_container_add.ynz",
        true,
        "tiny\nbig hello\ntiny\nbig world\nend\n",
    );
}

#[test]
fn v03_m9_union_container_map_set_asan_optimized() {
    // WHY: same out-of-bounds read through a map value.
    assert_asan_clean(
        "v0_3_m9_union_container_map_set.ynz",
        false,
        "tiny\nbig hello\nend\n",
    );
}

#[test]
fn v03_m9_union_container_map_set_asan_no_optimize() {
    // WHY: same read at -O0.
    assert_asan_clean(
        "v0_3_m9_union_container_map_set.ynz",
        true,
        "tiny\nbig hello\nend\n",
    );
}

// A recursive union cannot hold its payload inline (no finite size), so its slot boxes a shape
// variant in a heap cell. Bug caught: sizing the slot inline loops the layout, and a boxed slot
// that kept a frame pointer would dangle once `build` returns. Tier: High.
#[test]
fn v03_m9_union_recursive_boxed_asan_optimized() {
    // WHY: the list is read after the building frame is gone; ASan names a dangling cell.
    assert_asan_clean(
        "v0_3_m9_union_recursive_boxed.ynz",
        false,
        "6\n3 deep\nend\n",
    );
}

#[test]
fn v03_m9_union_recursive_boxed_asan_no_optimize() {
    // WHY: same read at -O0, where the dead frame is left untouched.
    assert_asan_clean(
        "v0_3_m9_union_recursive_boxed.ynz",
        true,
        "6\n3 deep\nend\n",
    );
}

// Bug caught: a `wait`-staged or `background` union argument that names the spawner's envelope
// dangles once the spawner suspends or returns; the callee's own `Param` region does not.
// Tier: High.
#[test]
fn v03_m9_union_wait_arg_asan_optimized() {
    // WHY: the argument is read after the callee resumes; the return rides the `Return` region.
    assert_asan_clean(
        "v0_3_m9_union_wait_arg_by_value.ynz",
        false,
        "big waited 7\ntiny\nbig again 8\nend\n",
    );
}

#[test]
fn v03_m9_union_wait_arg_asan_no_optimize() {
    // WHY: same read at -O0.
    assert_asan_clean(
        "v0_3_m9_union_wait_arg_by_value.ynz",
        true,
        "big waited 7\ntiny\nbig again 8\nend\n",
    );
}

#[test]
fn v03_m9_union_background_arg_asan_optimized() {
    // WHY: the task reads the argument long after the spawning frame moved on.
    assert_asan_clean(
        "v0_3_m9_union_background_arg.ynz",
        false,
        "big spawned 7\nend\n",
    );
}

#[test]
fn v03_m9_union_background_arg_asan_no_optimize() {
    // WHY: same read at -O0.
    assert_asan_clean(
        "v0_3_m9_union_background_arg.ynz",
        true,
        "big spawned 7\nend\n",
    );
}

// Bug caught: an `errors` result's 8-byte success word cannot hold a union slot; the value used to
// ride heap cells and, if re-pointed at the callee's frame, would dangle. The result carries the
// slot by value. Tier: High.
by_value_tests!(
    "v0_3_m9_union_errors_return.ynz",
    "big made 1\ntiny\nbig source 2\nbig made 4\nend\n",
    v03_m9_union_errors_return_optimized,
    v03_m9_union_errors_return_no_optimize,
    v03_m9_union_errors_return_asan_optimized,
    v03_m9_union_errors_return_asan_no_optimize
);

// Bug caught: a suspending `errors` function's slot lives in its frame's `Return` region, which
// the waiting caller must copy out before the child frame is freed. Tier: High.
by_value_tests!(
    "v0_3_m9_union_errors_return_suspending.ynz",
    "big slow 5\ntiny\nfailed\nend\n",
    v03_m9_union_errors_return_suspending_optimized,
    v03_m9_union_errors_return_suspending_no_optimize,
    v03_m9_union_errors_return_suspending_asan_optimized,
    v03_m9_union_errors_return_suspending_asan_no_optimize
);

// Bug caught: a `Thing errors` result held across a `wait` was a view pointer into the resume
// function's dead stack (SIGSEGV since the errors result went by value); the `let thing: Thing`
// binding that crosses was a heap envelope. A union binding must read back after a suspension.
// Tier: High.
by_value_tests!(
    "v0_3_m9_union_crossing_binding.ynz",
    "big crossing 3\nbig slow 8\nend\n",
    v03_m9_union_crossing_binding_optimized,
    v03_m9_union_crossing_binding_no_optimize,
    v03_m9_union_crossing_binding_asan_optimized,
    v03_m9_union_crossing_binding_asan_no_optimize
);

// Probe: a union binding copied from another binding inside a loop keeps its value when the source
// is rebound (a binding is a value, not a reference to a per-site envelope). Tier: High.
by_value_tests!(
    "v0_3_m9_union_binding_copy.ynz",
    "tiny\nbig b0\nbig b0\nbig b1\nbig b1\nbig b2\nend\n",
    v03_m9_union_binding_copy_optimized,
    v03_m9_union_binding_copy_no_optimize,
    v03_m9_union_binding_copy_asan_optimized,
    v03_m9_union_binding_copy_asan_no_optimize
);

// Probe: `maybe<Thing>` returned holding each variant and `none`, read after the callee's frame
// is gone. Tier: High.
by_value_tests!(
    "v0_3_m9_union_maybe.ynz",
    "tiny\nbig kept\nnone\nend\n",
    v03_m9_union_maybe_optimized,
    v03_m9_union_maybe_no_optimize,
    v03_m9_union_maybe_asan_optimized,
    v03_m9_union_maybe_asan_no_optimize
);

// Probe: shape equality over a union field compares the variant and its payload, not an address.
// Tier: High.
by_value_tests!(
    "v0_3_m9_union_shape_equality.ynz",
    "big same true\nbig payload false\nvariant false\ntiny same true\nend\n",
    v03_m9_union_shape_equality_optimized,
    v03_m9_union_shape_equality_no_optimize,
    v03_m9_union_shape_equality_asan_optimized,
    v03_m9_union_shape_equality_asan_no_optimize
);

// Probe: a `maybe<Thing>` bound before a `wait` and read after it, and one returned by a
// suspending function (its slot rides the frame's `Return` region). Tier: High.
by_value_tests!(
    "v0_3_m9_union_maybe_crossing.ynz",
    "tiny\nbig slow\nnone\nend\n",
    v03_m9_union_maybe_crossing_optimized,
    v03_m9_union_maybe_crossing_no_optimize,
    v03_m9_union_maybe_crossing_asan_optimized,
    v03_m9_union_maybe_crossing_asan_no_optimize
);

// Probe: a `maybe<Thing>` returned by an `errors` function, plain and suspending: the result
// carries the union's slot, not a pointer into the callee frame. Tier: High.
by_value_tests!(
    "v0_3_m9_union_maybe_errors.ynz",
    "big kept\ntiny\nnone\nbig kept\nend\n",
    v03_m9_union_maybe_errors_optimized,
    v03_m9_union_maybe_errors_no_optimize,
    v03_m9_union_maybe_errors_asan_optimized,
    v03_m9_union_maybe_errors_asan_no_optimize
);

// Bug: shape equality over a recursive (boxed) union field read the payload word of a
// non-shape variant as a heap cell pointer and recursed forever in the compiler. Tier: High.
by_value_tests!(
    "v0_3_m9_union_shape_equality_recursive.ynz",
    "same int true\nother int false\nvariant false\nsame node true\ndeep differs false\nend\n",
    v03_m9_union_shape_equality_recursive_optimized,
    v03_m9_union_shape_equality_recursive_no_optimize,
    v03_m9_union_shape_equality_recursive_asan_optimized,
    v03_m9_union_shape_equality_recursive_asan_no_optimize
);

// Bug: an `errors maybe<Thing>` binding held across a `wait` read a dead resume-stack slot
// (silently `none`); typeck refuses it like a plain `maybe<Thing>` binding. Tier: High.
#[test]
fn v03_m9_union_maybe_errors_crossing_is_refused() {
    let compiled = support::compile("v0_3_m9_union_maybe_errors_crossing_refused.ynz");
    assert!(
        !compiled.success,
        "an `errors maybe<Thing>` binding held across a `wait` must not compile:\n{}",
        compiled.diagnostics
    );
    assert!(
        compiled.diagnostics.contains("cannot yet cross a `wait`"),
        "the refusal must say what is not supported:\n{}",
        compiled.diagnostics
    );
}

// Bug caught: a callee that does not own a union stored its `share` parameter in a list, so the
// caller and the list both held one payload (the slot now copies bytes, but the ownership rule
// is what keeps a release pass from freeing the caller's payload). `give` is the one way to
// store a union you were passed (`v0_3_m9_union_keep_give.ynz`). Tier: High.
#[test]
fn v03_m9_union_share_param_stored_in_list_is_refused() {
    let compiled = support::compile("v0_3_m9_union_keep_share_refused.ynz");
    assert!(
        !compiled.success,
        "storing a `share` union parameter in a list must not compile:\n{}",
        compiled.diagnostics
    );
    for fragment in [
        "`thing` is a parameter of `keep` that is declared `share`",
        "needs `give`",
        "function keep(give thing: Tiny | Big, …)",
    ] {
        assert!(
            compiled.diagnostics.contains(fragment),
            "the refusal must contain {fragment:?}:\n{}",
            compiled.diagnostics
        );
    }
}

// Bug caught: copying a recursive union moved the variant's bytes into one new cell, so the
// copy's nested cells were the original's cells. A deep copy reads back the whole chain and stays
// clean under AddressSanitizer; the allocation count test pins that every box is cloned. Tier: High.
by_value_tests!(
    "v0_3_m9_union_alloc_recursive_copy.ynz",
    "300\n",
    v03_m9_union_recursive_copy_optimized,
    v03_m9_union_recursive_copy_no_optimize,
    v03_m9_union_recursive_copy_asan_optimized,
    v03_m9_union_recursive_copy_asan_no_optimize
);

// Bug: a copy of a value that owns a recursive union's boxes (`.copy()`, a binding made from a
// field, an array element, a `maybe` read) shared them with its source, and a moved value read
// a box its old owner had given up. Reassigning one side and reading the other's whole chain
// shows a shared or dangling box under AddressSanitizer. Tier: High.
by_value_tests!(
    "v0_3_m9_union_move_copy_reassign.ynz",
    "3 2\n3 1\n3 1\nend\n",
    v03_m9_union_move_copy_reassign_optimized,
    v03_m9_union_move_copy_reassign_no_optimize,
    v03_m9_union_move_copy_reassign_asan_optimized,
    v03_m9_union_move_copy_reassign_asan_no_optimize
);

// Bug: a binding of a place read made a view although the place was replaced while the binding
// was still read: the view read the new chain (`1`) where the binding must keep the old one (`3`).
// Condition (b) of a view failed, so it owns a deep copy. Tier: High.
by_value_tests!(
    "v0_3_m9_union_view_root_written.ynz",
    "3\n1\n",
    v03_m9_union_view_root_written_optimized,
    v03_m9_union_view_root_written_no_optimize,
    v03_m9_union_view_root_written_asan_optimized,
    v03_m9_union_view_root_written_asan_no_optimize
);
