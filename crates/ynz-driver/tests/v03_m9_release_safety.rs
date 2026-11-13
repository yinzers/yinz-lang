//! v0.3-M9 C2: red-first safety fixtures for the scope-exit release pass (C5–C8).
//!
//! Two groups, both High tier (data integrity: a wrong release is a use-after-free, a double
//! free or a leak, all silent at exit 0 without a sanitizer or a counter).
//!
//! ## Safety fixtures (AddressSanitizer) — 9 exit classes × 2 optimization levels
//!
//! One fixture per way a heap value can leave a scope: block exit, loop iteration end, normal
//! return, early return, the caller's `free_frame` after Ready, the spawned task's drop ladder,
//! and transfer by send, by give and by return. Each is built with `--emit-ir`, instrumented
//! (`sanitize_address`) and linked with `clang -fsanitize=address` (see `support`), then must
//! (a) exit 0 with no AddressSanitizer report and (b) print the exact values.
//!
//! These are GREEN today, and say so: nothing frees a scope's heap locals yet, so there is
//! nothing to free early or twice. They are the guard that the release pass does not. They
//! cannot be red before that pass exists, so `v03_m9_asan_harness_controls` proves the harness
//! reports an early and a double free; the leak group below is the red half. Leak detection
//! is off for them (it IS red today) — balance is asserted by the counters instead.
//!
//! Why 18 tests where the SOP says "about 8" per High-tier unit: the card requires one fixture
//! for each of nine exit classes, and each class is lowered by a different emitter site per
//! path (plain / state machine), so no two classes cover each other's bug. The second
//! optimization level is the same fixture at `--no-optimize`, where no optimizer can delete a
//! dead read of freed memory and hide the defect.
//!
//! ## Leak fixtures (alloc/free counters) — RED until the release pass lands
//!
//! A 2,000-iteration loop must free what it allocates, for heap locals (`ynz_alloc`/`ynz_free`
//! counters) and for strings (the string alloc/free counters of C2's runtime change).
//!
//! | test | red on the base because |
//! |---|---|
//! | `leak_heap_local_loop::*` | red until C7: no scope-exit release yet, 4,000 allocations / 0 frees |
//! | `leak_string_loop::*`     | red until C7: no scope-exit release yet, strings are never freed |
//!
//! Every test builds and runs its own program in its own process, and reads its own counter
//! file (`.claude/rules/test-parallelism.md`); one named test per case so the runner
//! parallelizes them and names the failing case.

mod support;

use support::{build, run_counted, run_under_asan};

fn assert_asan_clean_and_prints(fixture: &str, no_optimize: bool, expected_stdout: &str) {
    let run = run_under_asan(fixture, no_optimize);
    assert!(
        !run.stderr.contains("AddressSanitizer"),
        "{fixture} (no_optimize={no_optimize}): AddressSanitizer reported an error:\n{}",
        run.stderr
    );
    assert_eq!(
        run.exit_code, 0,
        "{fixture} (no_optimize={no_optimize}) must exit 0; stderr:\n{}",
        run.stderr
    );
    assert_eq!(
        run.stdout, expected_stdout,
        "{fixture} (no_optimize={no_optimize}): wrong values; stderr:\n{}",
        run.stderr
    );
}

/// `asan_case!(case, fixture, expected_stdout, "bug it catches (Tier: High)")`; the bug line
/// becomes the module doc.
macro_rules! asan_case {
    ($case:ident, $fixture:literal, $expected:literal, $bug:literal) => {
        #[doc = $bug]
        mod $case {
            use super::assert_asan_clean_and_prints;

            #[test]
            fn optimized() {
                assert_asan_clean_and_prints($fixture, false, $expected);
            }

            #[test]
            fn no_optimize() {
                // WHY: same fixture at -O0, where no optimizer can drop a dead read of freed memory.
                assert_asan_clean_and_prints($fixture, true, $expected);
            }
        }
    };
}

asan_case!(
    block_exit,
    "v0_3_m9_c2_block_exit.ynz",
    "2\n1\n5\n3\n3\n2\n3\n3\n",
    "release at an `if` block's end frees an enclosing binding that is read afterwards, or frees the block's own twice (High)"
);
asan_case!(
    loop_iteration_end,
    "v0_3_m9_c2_loop_iteration_end.ynz",
    "15\n18\n0\n1\n2\n3\n",
    "release at the loop back-edge frees a binding declared outside the loop (use-after-free on iteration 2) or releases the per-iteration local again at function exit (High)"
);
asan_case!(
    normal_return,
    "v0_3_m9_c2_normal_return.ynz",
    "5\n2\n9\n",
    "release runs before the `return` expression reads its locals, or runs at both the return edge and the fall-through end (High)"
);
asan_case!(
    early_return,
    "v0_3_m9_c2_early_return.ynz",
    "5\n3\n5\n3\n",
    "an early `return` inside a loop+if releases only the innermost frame, or releases outer frames twice with their normal scope ends (High)"
);
asan_case!(
    caller_free_frame,
    "v0_3_m9_c2_caller_free_frame.ynz",
    "8\n3\n2\n12\n",
    "the callee's Ready-exit release and the caller's `free_frame` both free a frame-held local, or the return slot is read after the frame is freed (High)"
);
asan_case!(
    spawn_drop_ladder,
    "v0_3_m9_c2_spawn_drop_ladder.ynz",
    "3\n7\n2\n8\n3\n",
    "the worker releases its `give` parameter and the task's drop ladder frees the same clone (double free), or the spawn releases the parent's own array (High)"
);
asan_case!(
    transfer_send,
    "v0_3_m9_c2_transfer_send.ynz",
    "2\n3\n10\n30\nend\n",
    "the producer releases a sent binding at scope exit (receiver reads freed memory), or the receiver releases a value the channel already dropped (High)"
);
asan_case!(
    transfer_give,
    "v0_3_m9_c2_transfer_give.ynz",
    "3\n3\nkept\n3\n7\n8\n3\ndone\ndone\n",
    "caller and callee both release a given array: definite, conditional, aliased and across a suspension (High)"
);
asan_case!(
    transfer_return,
    "v0_3_m9_c2_transfer_return.ynz",
    "3\n3\n2\n1\n3\n4\n",
    "the return edge releases the binding it returns (caller reads freed memory), skips the sibling it should release, or the caller's frame teardown frees the returned array (High)"
);

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

/// Leak, heap locals: a 2,000-iteration loop with an `array<int>` local per iteration leaves
/// every allocation unfreed (High).
mod leak_heap_local_loop {
    #[test]
    fn optimized() {
        // red until C7: no scope-exit release yet (4,000 allocs, 0 frees on the base)
        super::assert_heap_locals_balance(false);
    }

    #[test]
    fn no_optimize() {
        // WHY: same leak at -O0.
        // red until C7: no scope-exit release yet
        super::assert_heap_locals_balance(true);
    }
}

/// Leak, strings: a 2,000-iteration loop building strings through two allocators leaves every
/// string unfreed (High).
mod leak_string_loop {
    #[test]
    fn optimized() {
        // red until C7: no scope-exit release yet (4,000 string allocs, 0 frees on the base)
        super::assert_strings_balance(false);
    }

    #[test]
    fn no_optimize() {
        // WHY: same leak at -O0.
        // red until C7: no scope-exit release yet
        super::assert_strings_balance(true);
    }
}
