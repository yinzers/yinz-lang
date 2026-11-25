//! v0.3-M9 C2/C5a: the safety guards of the scope-exit release pass.
//!
//! Every way a heap value can leave a scope (block exit, loop iteration end, normal and early
//! return, the caller's `free_frame` after Ready, the spawned task's drop ladder, transfer by send,
//! give and return) and every way a value is stored without a sink (field set, container add,
//! index set, assignment from a loop local, multi-root literal, store by a callee, a give of a
//! copy) has one fixture, built `--emit-ir`, instrumented and linked with `clang -fsanitize=address`
//! (see `support`). Each must exit 0 with no AddressSanitizer report and print the exact values,
//! at -O2 and at -O0 (where no optimizer can delete a dead read of freed memory). Leak detection is
//! off for them: the two leak cases assert balance with the alloc/free counters instead.
//! `v03_m9_asan_harness_controls` proves the harness reports an early and a double free.
//!
//! The nine exit classes are lowered by a different emitter site per path (plain or state
//! machine), so no two classes cover each other's bug: each is a case of its own.

mod support;

use support::{
    assert_asan_clean_and_prints_at_both_levels, at_both_optimization_levels, build, run_counted,
};

/// Run a leak fixture and return (stdout, counters).
fn run_leak_fixture(fixture: &str, no_optimize: bool) -> (String, support::Counters) {
    let built = build(fixture, no_optimize);
    let (run, counters) = run_counted(&built);
    assert_eq!(
        run.exit_code, 0,
        "{fixture} must exit 0; stderr:\n{}",
        run.stderr
    );
    (run.stdout, counters)
}

fn assert_heap_locals_balance(no_optimize: bool) {
    let (stdout, counters) = run_leak_fixture("v0_3_m9_c2_leak_heap_local_loop.ynz", no_optimize);
    assert_eq!(stdout, "2000\n", "the loop must run to completion");
    // Non-vacuous: a program that allocated nothing would "balance" trivially.
    assert!(
        counters.alloc >= 2000,
        "expected at least one allocation per iteration, got {counters:?}"
    );
    assert_eq!(
        counters.free, counters.alloc,
        "every heap local allocated in the loop must be freed by exit: {counters:?}"
    );
}

fn assert_strings_balance(no_optimize: bool) {
    let (stdout, counters) = run_leak_fixture("v0_3_m9_c2_leak_string_loop.ynz", no_optimize);
    assert_eq!(stdout, "24000\n", "the loop must run to completion");
    assert!(
        counters.string_alloc >= 2000,
        "expected at least one string allocation per iteration, got {counters:?}"
    );
    assert_eq!(
        counters.string_free, counters.string_alloc,
        "every string built in the loop must be freed by exit: {counters:?}"
    );
    assert_eq!(
        counters.string_free_bytes, counters.string_alloc_bytes,
        "freed string bytes must equal allocated string bytes: {counters:?}"
    );
}

/// One named test per scope-exit class, store form or leak loop of the `scope-exit release safety
/// (feature run, integrity)` run unit; each case runs at both optimization levels.
macro_rules! release_safety_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: the scope-exit release pass frees a heap value early or twice, or one a store or transfer still holds, or leaks a loop's locals, silently at exit 0 | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

release_safety_high_case! {
    // case: release at an `if` block's end frees an enclosing binding that is read afterwards, or frees the block's own twice
    block_exit_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c2_block_exit.ynz",
            "2\n1\n5\n3\n3\n2\n3\n3\n",
        );
    }
    // case: release at the loop back-edge frees a binding declared outside the loop (use-after-free on iteration 2) or releases the per-iteration local again at function exit
    loop_iteration_end_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c2_loop_iteration_end.ynz",
            "15\n18\n0\n1\n2\n3\n",
        );
    }
    // case: release runs before the `return` expression reads its locals, or runs at both the return edge and the fall-through end
    normal_return_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c2_normal_return.ynz",
            "5\n2\n9\n",
        );
    }
    // case: an early `return` inside a loop+if releases only the innermost frame, or releases outer frames twice with their normal scope ends
    early_return_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c2_early_return.ynz",
            "5\n3\n5\n3\n",
        );
    }
    // case: the callee's Ready-exit release and the caller's `free_frame` both free a frame-held local, or the return slot is read after the frame is freed
    caller_free_frame_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c2_caller_free_frame.ynz",
            "8\n3\n2\n12\n",
        );
    }
    // case: the worker releases its `give` parameter and the task's drop ladder frees the same clone (double free), or the spawn releases the parent's own array
    spawn_drop_ladder_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c2_spawn_drop_ladder.ynz",
            "3\n7\n2\n8\n3\n",
        );
    }
    // case: the producer releases a sent binding at scope exit (receiver reads freed memory), or the receiver releases a value the channel already dropped
    transfer_send_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c2_transfer_send.ynz",
            "2\n3\n10\n30\nend\n",
        );
    }
    // case: caller and callee both release a given array: definite, conditional, aliased and across a suspension
    transfer_give_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c2_transfer_give.ynz",
            "3\n3\nkept\n3\n7\n8\n3\ndone\ndone\n",
        );
    }
    // case: the return edge releases the binding it returns (caller reads freed memory), skips the sibling it should release, or the caller's frame teardown frees the returned array
    transfer_return_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c2_transfer_return.ynz",
            "3\n3\n2\n1\n3\n4\n",
        );
    }
    // case: `b.items = a; return b`: the pass releases `a` at scope exit while the returned `b` holds it
    store_field_set_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c5a_field_set_store.ynz",
            "3\n",
        );
    }
    // case: `o.add(a); return o`: the pass releases `a` while the returned container holds it
    store_container_add_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c5a_container_add_store.ynz",
            "3\n",
        );
    }
    // case: `o[0] = a; return o`: the pass releases `a` while the returned container holds it
    store_index_set_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c5a_index_set_store.ynz",
            "3\n",
        );
    }
    // case: `acc = per` in a loop: the back-edge release frees `per` while the outer `acc` aliases it
    store_assign_from_loop_local_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c5a_assign_from_loop_local.ynz",
            "3\n",
        );
    }
    // case: `{ first: a, second: c }` returned: the pass joins only the first root and frees `c` under the returned pair
    store_multi_root_literal_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c5a_multi_root_literal.ynz",
            "3\n2\n",
        );
    }
    // case: `put(o, a)` stores `a` into `o` inside the callee, then `o` is returned: the pass frees `a` under the returned container
    store_by_callee_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c5a_call_stores_argument.ynz",
            "3\n",
        );
    }
    // case: a loop giving `x.copy()` each pass: the pass frees the outer `x` with the callee's clone, or frees a clone twice
    loop_give_of_a_copy_frees_nothing_early_or_twice => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_c5a_give_copy_in_loop.ynz",
            "3\n3\n3\n3\n",
        );
    }
    // case: a 2,000-iteration loop with an `array<int>` local per iteration leaves every allocation unfreed (pre-C7: 4,000 allocations, 0 frees)
    a_loop_frees_every_heap_local_it_allocates => {
        at_both_optimization_levels(assert_heap_locals_balance);
    }
    // case: a 2,000-iteration loop building strings through two allocators leaves every string unfreed (pre-C7: 4,000 string allocations, 0 frees)
    a_loop_frees_every_string_it_builds => {
        at_both_optimization_levels(assert_strings_balance);
    }
}
