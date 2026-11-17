//! v0.3-M9 C8 slice 6: a task dropped while Pending releases its frame through the drop entry of
//! the resume function. Each program parks a spawned task in a suspension, lets the parent
//! return, and requires every allocation freed and no AddressSanitizer report at both
//! optimization levels.

mod support;

use support::{assert_asan_clean, assert_heap_balanced, at_both_optimization_levels, run_balanced};

fn balanced_and_asan_clean(label: &str, program: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        assert_heap_balanced(label, &counters, no_optimize);
        assert_eq!(
            counters.string_free, counters.string_alloc,
            "{label}: {counters:?}"
        );
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

// ── one test per suspension site ────────────────────────────────────────────────────────────
//
// Each parks a spawned task at that site, lets the parent return, and requires every allocation
// freed and no AddressSanitizer report. Bug each catches: the site has no drop state, so the
// task's frame locals leak (or the dispatch falls through to `sm_dead` and the frame is not
// walked). Tier: Medium (one integration test per contract).

/// Conduit site: parked in `receive` on a channel nobody sends to.
const PARKED_IN_CONDUIT: &str = "
function worker() -> nothing {
  let mine: array<int> = [1, 2]
  let ch: channel<int> = channel<int>(2)
  let got = ch.receive()
  print(`${mine.count()}`)
}

function entrypoint() -> nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

#[test]
fn parked_in_a_conduit_receive_releases_its_frame() {
    balanced_and_asan_clean("c8s6_conduit", PARKED_IN_CONDUIT, "parent\n");
}

/// Independent I/O group: two suspending calls that overlap; both members are parked in their
/// own `wait`, each holding a local of its own, beside a local of the parent.
const PARKED_IN_INDEPENDENT_GROUP: &str = "
function slowFirst() -> int {
  let first: array<int> = [1, 2, 3]
  wait sleep(500)
  return first.count()
}

function slowSecond() -> int {
  let second: array<int> = [4]
  wait sleep(500)
  return second.count()
}

function worker() -> nothing {
  let mine: array<int> = [9]
  let a = slowFirst()
  let b = slowSecond()
  print(`${a + b + mine.count()}`)
}

function entrypoint() -> nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

#[test]
fn parked_in_an_independent_group_releases_every_member_frame() {
    balanced_and_asan_clean(
        "c8s6_independent_group",
        PARKED_IN_INDEPENDENT_GROUP,
        "parent\n",
    );
}

/// Independent I/O group where one member already returned a heap value: the value sits in the
/// finished member's return slot, owned by nobody else, until the group joins.
const PARKED_IN_GROUP_WITH_A_RETURNED_MEMBER: &str = "
function quick() -> array<int> {
  let made: array<int> = [1, 2, 3]
  wait sleep(1)
  return made
}

function slow() -> int {
  wait sleep(500)
  return 1
}

function worker() -> nothing {
  let a = quick()
  let b = slow()
  print(`${a.count() + b}`)
}

function entrypoint() -> nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

#[test]
fn parked_in_a_group_releases_the_value_a_finished_member_returned() {
    balanced_and_asan_clean(
        "c8s6_group_returned_member",
        PARKED_IN_GROUP_WITH_A_RETURNED_MEMBER,
        "parent\n",
    );
}

/// Back-edge yield: a wait-free loop in a state-machine function yields at its back edge; the
/// task is parked there holding a local declared before the loop and a loop-carried string.
const PARKED_AT_A_BACK_EDGE: &str = "
function hog() -> nothing {
  let mine: array<int> = [1, 2]
  let label: string = `x`.toUpperCase()
  let x = 0
  while (x < 2000000000) {
    x = x + 1
  }
  print(label)
  wait sleep(0)
}

function entrypoint() -> nothing {
  background hog()
  wait sleep(100)
  print(`parent`)
}
";

#[test]
fn parked_at_a_loop_back_edge_releases_its_frame() {
    balanced_and_asan_clean("c8s6_back_edge", PARKED_AT_A_BACK_EDGE, "parent\n");
}

/// Fused CPU + I/O group: the host is parked at the shared poll state with its CPU handle live
/// and its I/O member in a `wait`.
const PARKED_IN_FUSED_GROUP: &str = "
function crunch(n: int) -> int {
  let total = 0
  let i = 0
  while (i < 1000) {
    total = total + i
    i = i + 1
  }
  return total + n
}

function fetchSlow(n: int) -> int {
  let own: array<int> = [n]
  wait sleep(500)
  return own.count() + n
}

function fusedHost() -> nothing {
  let mine: array<int> = [7, 8]
  let a = crunch(1)
  let b = fetchSlow(2)
  print(a + b + mine.count())
}

function entrypoint() -> nothing {
  background fusedHost()
  wait sleep(100)
  print(`parent`)
}
";

#[test]
fn parked_in_a_fused_group_releases_its_frame() {
    balanced_and_asan_clean("c8s6_fused_group", PARKED_IN_FUSED_GROUP, "parent\n");
}

/// CPU spike group: a promoted host (no `wait` of its own; its CPU join is the suspension) is
/// parked at the join while the two CPU members still run.
const PARKED_AT_A_CPU_JOIN: &str = "
function buildFirst(n: int) -> string {
  let seed: int = 1
  let i: int = 0
  while (i < n) {
    seed = (seed * 1103515245 + 12345) % 2147483648
    i = i + 1
  }
  return `len=${seed}`
}

function buildSecond(n: int) -> string {
  let seed: int = 7
  let i: int = 0
  while (i < n) {
    seed = (seed * 1103515245 + 12345) % 2147483648
    i = i + 1
  }
  return `total=${seed}`
}

function host() -> nothing {
  let mine: array<int> = [1, 2]
  let a = buildFirst(4000000000)
  let b = buildSecond(4000000000)
  print(a)
  print(b)
  print(`${mine.count()}`)
}

function entrypoint() -> nothing {
  background host()
  wait sleep(50)
  print(`parent`)
}
";

/// The two CPU members are still running when the process exits: a spawned CPU task cannot be
/// cancelled (it owns its argument context until it ends), so the one context box of each stays
/// allocated. Everything the host's frame holds (`mine`, 2 allocations) is released by the drop
/// entry; without it 4 allocations leak.
#[test]
fn a_promoted_host_parked_at_its_cpu_join_releases_its_frame() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(
            "c8s6_cpu_join",
            PARKED_AT_A_CPU_JOIN,
            "parent\n",
            no_optimize,
        );
        assert_eq!(
            counters.alloc - counters.free,
            2,
            "c8s6_cpu_join (no_optimize={no_optimize}): only the two running members' contexts \n             may stay allocated: {counters:?}"
        );
        assert_asan_clean(
            "c8s6_cpu_join",
            PARKED_AT_A_CPU_JOIN,
            "parent\n",
            no_optimize,
        );
    });
}
