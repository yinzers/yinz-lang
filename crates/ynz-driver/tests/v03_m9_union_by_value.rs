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
