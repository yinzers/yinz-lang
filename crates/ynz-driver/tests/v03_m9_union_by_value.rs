//! v0.3-M9 C1f (#423): a union stored into anything that outlives the frame that made it must
//! read back correctly: the slot holds the 16-byte `{tag, data}` envelope BY VALUE and the
//! payload it points to lives outside every frame. Tier: High (memory safety / data integrity).
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
//! Count: 4 red fixtures x 4 runs + 2 guard fixtures x 2 ASan runs = 20 tests, past the ~8 a
//! High-tier unit gets without a reason. The runs are not copies: plain vs AddressSanitizer
//! keeps the VALUE apart from the MEMORY ACCESS (a dangling read often prints plausible text,
//! and an out-of-bounds one can print the right text), and -O0 vs -O2 keeps a frame the
//! optimizer reuses apart from one it does not (stack-use-after-return vs after-scope).
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

// Bug: `list.add(t)` in a callee stored the caller-frame envelope pointer. Tier: High.
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

// RED until step 2 of #423 lands (crossing union bindings in frame regions): a `Thing errors`
// result held across a `wait` was a view pointer into the resume function's dead stack
// (SIGSEGV since the errors result went by value); the `let thing: Thing` binding that crosses
// was a heap envelope. Bug caught: a union binding read after a suspension. Tier: High.
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
