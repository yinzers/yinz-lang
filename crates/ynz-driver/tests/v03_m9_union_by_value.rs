//! v0.3-M9 C1f (#423): a union stored into anything that outlives the frame that made it must
//! read back correctly: the slot holds `{ tag, payload }` BY VALUE with the payload inline, sized to
//! the largest variant (`IMP-unions.md` "Stored union slot"); only a recursive union boxes a shape
//! variant in a heap cell (memory safety / data integrity).
//!
//! Each fixture is one escape kind, not a copy of another. A case runs the program as shipped (plain,
//! stdout exact) and under AddressSanitizer, each at -O0 and -O2, all four at once: ASan changes
//! the stack layout (redzones, fake frames), so a read that is only wrong without the sanitizer
//! shows in the plain run; -O0 vs -O2 keeps a frame the optimizer reuses apart from one it does not
//! (stack-use-after-return vs after-scope). The add / map_set / recursive-boxed / `wait`-staged /
//! `background` guards run under ASan only (ASan asserts the stdout too).
//!
//! The heap homes #410 and #421 built (union return, `wait`-staged argument, hidden default)
//! are guarded by `v03_m9_union_coerce.rs` (the `union_wrap` cases) and `v03_m9_union_hidden_default.rs`.

mod support;

use support::{
    assert_asan_clean_and_prints_at_both_levels, assert_prints_and_asan_clean_at_both_levels,
};

/// The program as shipped and under AddressSanitizer, at both levels: the stored union reads back.
fn assert_stored_union_reads_back(fixture: &str, expected_stdout: &str) {
    assert_prints_and_asan_clean_at_both_levels(
        fixture,
        expected_stdout,
        "a union stored where it outlives its frame must keep its tag and payload",
    );
}

/// One named test per escape kind or stored form of the `stored union by value (feature run,
/// integrity)` run unit.
macro_rules! union_by_value_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a union stored where it outlives its frame (container, return, errors result, argument, binding, recursive box) reads a dead frame, the wrong variant or a shared box, silently at exit 0 | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

union_by_value_high_case! {
    // case: a union array returned from the function that filled it holds pointers into the dead frame (envelope and payload)
    a_union_array_returned_from_the_function_that_filled_it_reads_back => {
        assert_stored_union_reads_back("v0_3_m9_union_container_escape.ynz", "big hello\ntiny\nend\n");
    }
    // case: a loop stores into one per-site envelope slot, so every element reads the last value stored (ASan-clean, silently wrong)
    plain_variants_stored_in_a_loop_keep_one_slot_each => {
        assert_stored_union_reads_back("v0_3_m9_union_container_loop.ynz", "tiny\nbig b1\ntiny\nbig b3\nbig m0\nbig m1\nend\n");
    }
    // case: `list.add(thing)` in a callee stored the caller-frame envelope pointer
    a_give_union_stored_by_a_callee_into_the_callers_list_reads_back => {
        assert_stored_union_reads_back("v0_3_m9_union_keep_give.ynz", "big kept\nbig old\ntiny\nend\n");
    }
    // case: a shape literal storing a plain variant in a union field, returned from its frame, read `none`/`int` at -O0 and segfaulted at -O2; a `lend` field store dangled likewise
    a_union_field_of_a_returned_shape_and_a_lend_field_store_read_back => {
        assert_stored_union_reads_back("v0_3_m9_union_shape_field_escape.ynz", "big escaped\ntiny\nbig stored\nend\n");
    }
    // case: straight-line `add` of a plain variant skipped the one coercion point: a bare shape read as a 16-byte envelope is an out-of-bounds read (guard, green on the base)
    a_union_added_to_a_list_is_wrapped_in_its_slot => {
        assert_asan_clean_and_prints_at_both_levels("v0_3_m9_union_container_add.ynz", "tiny\nbig hello\ntiny\nbig world\nend\n");
    }
    // case: straight-line `map.set` of a plain variant skipped the one coercion point: the same out-of-bounds read through a map value (guard, green on the base)
    a_union_set_into_a_map_is_wrapped_in_its_slot => {
        assert_asan_clean_and_prints_at_both_levels("v0_3_m9_union_container_map_set.ynz", "tiny\nbig hello\nend\n");
    }
    // case: a recursive union cannot hold its payload inline (no finite size): sizing the slot inline loops the layout, and a boxed slot that kept a frame pointer would dangle once `build` returns
    a_recursive_union_boxes_its_shape_variant_and_reads_back => {
        assert_asan_clean_and_prints_at_both_levels("v0_3_m9_union_recursive_boxed.ynz", "6\n3 deep\nend\n");
    }
    // case: a `wait`-staged union argument that names the spawner's envelope dangles once the spawner suspends; the callee's own `Param` region does not
    a_wait_staged_union_argument_reads_back_after_the_callee_resumes => {
        assert_asan_clean_and_prints_at_both_levels("v0_3_m9_union_wait_arg_by_value.ynz", "big waited 7\ntiny\nbig again 8\nend\n");
    }
    // case: a `background` union argument that names the spawner's envelope dangles once the spawner returns: the task reads it long after the spawning frame moved on
    a_background_union_argument_outlives_the_spawning_frame => {
        assert_asan_clean_and_prints_at_both_levels("v0_3_m9_union_background_arg.ynz", "big spawned 7\nend\n");
    }
    // case: an `errors` result's 8-byte success word cannot hold a union slot; the value used to ride heap cells and, if re-pointed at the callee's frame, would dangle
    an_errors_result_carries_its_union_slot_by_value => {
        assert_stored_union_reads_back("v0_3_m9_union_errors_return.ynz", "big made 1\ntiny\nbig source 2\nbig made 4\nend\n");
    }
    // case: a suspending `errors` function's slot lives in its frame's `Return` region, which the waiting caller must copy out before the child frame is freed
    a_suspending_errors_result_copies_its_union_slot_out_of_the_child_frame => {
        assert_stored_union_reads_back("v0_3_m9_union_errors_return_suspending.ynz", "big slow 5\ntiny\nfailed\nend\n");
    }
    // case: a `Thing errors` result held across a `wait` was a view pointer into the resume function's dead stack (SIGSEGV since the errors result went by value); the `let thing: Thing` binding that crosses was a heap envelope
    a_union_binding_crossing_a_wait_reads_back => {
        assert_stored_union_reads_back("v0_3_m9_union_crossing_binding.ynz", "big crossing 3\nbig slow 8\nend\n");
    }
    // case: a union binding copied from another binding inside a loop read the rebound source (a binding is a value, not a reference to a per-site envelope)
    a_union_binding_copied_in_a_loop_keeps_its_value_when_the_source_is_rebound => {
        assert_stored_union_reads_back("v0_3_m9_union_binding_copy.ynz", "tiny\nbig b0\nbig b0\nbig b1\nbig b1\nbig b2\nend\n");
    }
    // case: `maybe<Thing>` returned holding each variant and `none`, read after the callee's frame is gone
    a_maybe_union_returned_holding_each_variant_reads_back => {
        assert_stored_union_reads_back("v0_3_m9_union_maybe.ynz", "tiny\nbig kept\nnone\nend\n");
    }
    // case: shape equality over a union field compared an address, not the variant and its payload
    shape_equality_over_a_union_field_compares_variant_and_payload => {
        assert_stored_union_reads_back("v0_3_m9_union_shape_equality.ynz", "big same true\nbig payload false\nvariant false\ntiny same true\nend\n");
    }
    // case: a `maybe<Thing>` bound before a `wait` and read after it, and one returned by a suspending function (its slot rides the frame's `Return` region), read a dead slot
    a_maybe_union_crossing_a_wait_reads_back => {
        assert_stored_union_reads_back("v0_3_m9_union_maybe_crossing.ynz", "tiny\nbig slow\nnone\nend\n");
    }
    // case: a `maybe<Thing>` returned by an `errors` function, plain and suspending, carried a pointer into the callee frame instead of the union's slot
    a_maybe_union_errors_result_carries_the_slot_not_a_frame_pointer => {
        assert_stored_union_reads_back("v0_3_m9_union_maybe_errors.ynz", "big kept\ntiny\nnone\nbig kept\nend\n");
    }
    // case: shape equality over a recursive (boxed) union field read the payload word of a non-shape variant as a heap cell pointer and recursed forever in the compiler
    shape_equality_over_a_recursive_union_field_terminates_and_compares_payload => {
        assert_stored_union_reads_back("v0_3_m9_union_shape_equality_recursive.ynz", "same int true\nother int false\nvariant false\nsame node true\ndeep differs false\nend\n");
    }
    // case: copying a recursive union moved the variant's bytes into one new cell, so the copy's nested cells were the original's cells (the allocation count of `v03_m9_union_alloc` pins that every box is cloned)
    copying_a_recursive_union_deep_copies_every_box => {
        assert_stored_union_reads_back("v0_3_m9_union_alloc_recursive_copy.ynz", "300\n");
    }
    // case: a copy of a value that owns a recursive union's boxes (`.copy()`, a binding made from a field, an array element, a `maybe` read) shared them with its source, and a moved value read a box its old owner had given up
    a_copy_or_move_of_union_boxes_survives_reassigning_the_other_side => {
        assert_stored_union_reads_back("v0_3_m9_union_move_copy_reassign.ynz", "3 2\n3 1\n3 1\nend\n");
    }
    // case: a view of a field read (`let rest = head.next` over a `share` parameter) points into the place it names; a recursive walk that reads it through every frame must read a frame or cell that is gone
    a_recursive_walk_over_a_field_view_reads_live_memory => {
        assert_stored_union_reads_back("v0_3_m9_union_view_walk_10.ynz", "45\n10\n");
    }
}

// bug: typeck accepts an `errors maybe<Thing>` binding held across a `wait`, which read a dead resume-stack slot (silently `none`) | tier: High
#[test]
fn a_maybe_union_errors_binding_held_across_a_wait_is_refused() {
    support::assert_refused_with(
        "v0_3_m9_union_maybe_errors_crossing_refused.ynz",
        &["cannot yet cross a `wait`"],
    );
}
