//! v0.3-M9 C8 slice 6 spike: a task dropped while Pending releases its frame through the drop
//! entry of the resume function. Each program parks a spawned task in a `wait`, lets the parent
//! return, and requires every allocation freed and no AddressSanitizer report at both
//! optimization levels.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, at_both_optimization_levels, run_balanced,
};

fn balanced_and_asan_clean(label: &str, program: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        assert_heap_balanced(label, &counters, no_optimize);
        assert_eq!(counters.string_free, counters.string_alloc, "{label}: {counters:?}");
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

/// The same callee is called twice from the parked task. Its embedded sub-frame is NOT cleared
/// between calls (only `resume_point` and `sleep_handle` are re-initialised), so after the first
/// call `b` still holds a pointer the first call already freed. The second call is parked before
/// `b` is bound: releasing every non-null slot would free it twice; the per-state list does not
/// list `b` there.
const STALE_CHILD_SLOT: &str = "
function inner(ms: int) -> nothing {
  let a: array<int> = [1, 2]
  wait sleep(ms)
  let b: array<int> = [3]
  wait sleep(ms)
}

function worker() -> nothing {
  let outer: array<int> = [9]
  inner(1)
  inner(500)
}

function entrypoint() -> nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

#[test]
fn child_called_twice_does_not_release_a_stale_slot() {
    balanced_and_asan_clean("c8s6_stale_child_slot", STALE_CHILD_SLOT, "parent\n");
}

/// The parked task holds the handle of its own child. The drop entry releases the handle
/// (`ynz_handle_free` aborts the child); the child's own future frees the child's frame.
const OWN_CHILD_HANDLE: &str = "
function child() -> nothing {
  let theirs: array<int> = [5, 6]
  wait sleep(500)
}

function worker() -> nothing {
  let mine: array<int> = [1]
  let handle = background child()
  wait sleep(500)
}

function entrypoint() -> nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

#[test]
fn parked_task_holding_its_childs_handle_composes() {
    balanced_and_asan_clean("c8s6_own_child_handle", OWN_CHILD_HANDLE, "parent\n");
}

/// A borrowed parameter (read again after the wait, so it sits in a frame slot) is in non-null frame slots the task does not own: the
/// spawn ladder frees the argument's clone, so the drop entry must not.
const BORROWED_PARAMETER: &str = "
function worker(rows: array<int>) -> nothing {
  let before: int = rows.count()
  let mine: array<int> = [1, 2]
  wait sleep(500)
  print(`${before + rows.count()}`)
}

function launch() -> nothing {
  let rows: array<int> = [10, 20, 30]
  background worker(rows)
}

function entrypoint() -> nothing {
  launch()
  wait sleep(100)
  print(`parent`)
}
";

#[test]
fn borrowed_parameter_and_view_are_not_released_by_the_drop_entry() {
    balanced_and_asan_clean("c8s6_borrowed_parameter", BORROWED_PARAMETER, "parent\n");
}

/// A local given away before the wait is not released by the drop entry (the slot is null).
const MOVED_BEFORE_WAIT: &str = "
function take(give rows: array<int>) -> int {
  return rows.count()
}

function worker() -> nothing {
  let moved: array<int> = [1, 2, 3]
  let kept: array<int> = [4]
  let count: int = take(moved)
  wait sleep(500)
  print(`${count}`)
}

function entrypoint() -> nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

#[test]
fn a_local_given_away_before_the_wait_is_released_once() {
    balanced_and_asan_clean("c8s6_moved_before_wait", MOVED_BEFORE_WAIT, "parent\n");
}

/// The two red tests of `v03_m9_c8_s0_red`, under AddressSanitizer (their counters are pinned
/// there).
const DROPPED_AT_SHUTDOWN: &str = "
function worker(give rows: array<int>) -> nothing {
  let mine: array<int> = [1, 2]
  let label: string = `x`.toUpperCase()
  wait sleep(500)
  print(`child done`)
}

function entrypoint() -> nothing {
  let rows: array<int> = [10, 20, 30]
  background worker(rows)
  print(`parent`)
}
";

const DROPPED_IN_NESTED_CHILD: &str = "
function inner() -> nothing {
  let deep: array<int> = [7, 8, 9]
  wait sleep(500)
  print(`inner done`)
}

function worker() -> nothing {
  let outer: array<int> = [1]
  inner()
  print(`worker done`)
}

function entrypoint() -> nothing {
  background worker()
  print(`parent`)
}
";

#[test]
fn the_two_red_programs_are_asan_clean_and_balanced() {
    balanced_and_asan_clean("c8s6_red_shutdown", DROPPED_AT_SHUTDOWN, "parent\n");
    balanced_and_asan_clean("c8s6_red_nested", DROPPED_IN_NESTED_CHILD, "parent\n");
}
