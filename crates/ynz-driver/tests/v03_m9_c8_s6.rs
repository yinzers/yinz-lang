//! v0.3-M9 C8 slice 6: a task dropped while Pending releases its frame through the drop entry of
//! the resume function. Each program parks a spawned task in a suspension, lets the parent
//! return, and requires every allocation freed and no AddressSanitizer report at both
//! optimization levels.

mod support;

use support::{
    assert_asan_clean, assert_source_balanced_and_asan_clean as balanced_and_asan_clean,
    at_both_optimization_levels, run_balanced,
};

/// The same callee is called twice from the parked task. Its embedded sub-frame is NOT cleared
/// between calls (only `resume_point` and `sleep_handle` are re-initialised), so after the first
/// call `b` still holds a pointer the first call already freed. The second call is parked before
/// `b` is bound: releasing every non-null slot would free it twice; the per-state list does not
/// list `b` there.
const STALE_CHILD_SLOT: &str = "
function inner(ms: Int) -> Nothing {
  let a: Array<Int> = [1, 2]
  wait sleep(ms)
  let b: Array<Int> = [3]
  wait sleep(ms)
}

function worker() -> Nothing {
  let outer: Array<Int> = [9]
  inner(1)
  inner(500)
}

function entrypoint() -> Nothing {
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
function child() -> Nothing {
  let theirs: Array<Int> = [5, 6]
  wait sleep(500)
}

function worker() -> Nothing {
  let mine: Array<Int> = [1]
  let handle = background child()
  wait sleep(500)
}

function entrypoint() -> Nothing {
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
function worker(rows: Array<Int>) -> Nothing {
  let before: Int = rows.count()
  let mine: Array<Int> = [1, 2]
  wait sleep(500)
  print(`${before + rows.count()}`)
}

function launch() -> Nothing {
  let rows: Array<Int> = [10, 20, 30]
  background worker(rows)
}

function entrypoint() -> Nothing {
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
function take(give rows: Array<Int>) -> Int {
  return rows.count()
}

function worker() -> Nothing {
  let moved: Array<Int> = [1, 2, 3]
  let kept: Array<Int> = [4]
  let count: Int = take(moved)
  wait sleep(500)
  print(`${count}`)
}

function entrypoint() -> Nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

#[test]
fn a_local_given_away_before_the_wait_is_released_once() {
    balanced_and_asan_clean("c8s6_moved_before_wait", MOVED_BEFORE_WAIT, "parent\n");
}

/// The two dropped-while-pending programs of `v03_m9_c8_s0`, under AddressSanitizer (their counters are pinned
/// there).
const DROPPED_AT_SHUTDOWN: &str = "
function worker(give rows: Array<Int>) -> Nothing {
  let mine: Array<Int> = [1, 2]
  let label: String = `x`.toUpperCase()
  wait sleep(500)
  print(`child done`)
}

function entrypoint() -> Nothing {
  let rows: Array<Int> = [10, 20, 30]
  background worker(rows)
  print(`parent`)
}
";

const DROPPED_IN_NESTED_CHILD: &str = "
function inner() -> Nothing {
  let deep: Array<Int> = [7, 8, 9]
  wait sleep(500)
  print(`inner done`)
}

function worker() -> Nothing {
  let outer: Array<Int> = [1]
  inner()
  print(`worker done`)
}

function entrypoint() -> Nothing {
  background worker()
  print(`parent`)
}
";

#[test]
fn a_task_dropped_at_shutdown_or_in_a_nested_child_is_asan_clean_and_balanced() {
    balanced_and_asan_clean("c8s6_dropped_at_shutdown", DROPPED_AT_SHUTDOWN, "parent\n");
    balanced_and_asan_clean(
        "c8s6_dropped_in_nested_child",
        DROPPED_IN_NESTED_CHILD,
        "parent\n",
    );
}

// ── one test per suspension site ────────────────────────────────────────────────────────────
//
// Each parks a spawned task at that site, lets the parent return, and requires every allocation
// freed and no AddressSanitizer report. Bug each catches: the site has no drop state, so the
// task's frame locals leak (or the dispatch falls through to `sm_dead` and the frame is not
// walked). Tier: Medium (one integration test per contract).

/// Conduit site: parked in `receive` on a channel nobody sends to.
const PARKED_IN_CONDUIT: &str = "
function worker() -> Nothing {
  let mine: Array<Int> = [1, 2]
  let ch: Channel<Int> = Channel<Int>(2)
  let got = ch.receive()
  print(`${mine.count()}`)
}

function entrypoint() -> Nothing {
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
function slowFirst() -> Int {
  let first: Array<Int> = [1, 2, 3]
  wait sleep(500)
  return first.count()
}

function slowSecond() -> Int {
  let second: Array<Int> = [4]
  wait sleep(500)
  return second.count()
}

function worker() -> Nothing {
  let mine: Array<Int> = [9]
  let a = slowFirst()
  let b = slowSecond()
  print(`${a + b + mine.count()}`)
}

function entrypoint() -> Nothing {
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
function quick() -> Array<Int> {
  let made: Array<Int> = [1, 2, 3]
  wait sleep(1)
  return made
}

function slow() -> Int {
  wait sleep(500)
  return 1
}

function worker() -> Nothing {
  let a = quick()
  let b = slow()
  print(`${a.count() + b}`)
}

function entrypoint() -> Nothing {
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
function hog() -> Nothing {
  let mine: Array<Int> = [1, 2]
  let label: String = `x`.toUpperCase()
  let x = 0
  while (x < 2000000000) {
    x = x + 1
  }
  print(label)
  wait sleep(0)
}

function entrypoint() -> Nothing {
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
function crunch(n: Int) -> Int {
  let total = 0
  let i = 0
  while (i < 1000) {
    total = total + i
    i = i + 1
  }
  return total + n
}

function fetchSlow(n: Int) -> Int {
  let own: Array<Int> = [n]
  wait sleep(500)
  return own.count() + n
}

function fusedHost() -> Nothing {
  let mine: Array<Int> = [7, 8]
  let a = crunch(1)
  let b = fetchSlow(2)
  print(a + b + mine.count())
}

function entrypoint() -> Nothing {
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
function buildFirst(n: Int) -> String {
  let seed: Int = 1
  let i: Int = 0
  while (i < n) {
    seed = (seed * 1103515245 + 12345) % 2147483648
    i = i + 1
  }
  return `len=${seed}`
}

function buildSecond(n: Int) -> String {
  let seed: Int = 7
  let i: Int = 0
  while (i < n) {
    seed = (seed * 1103515245 + 12345) % 2147483648
    i = i + 1
  }
  return `total=${seed}`
}

function host() -> Nothing {
  let mine: Array<Int> = [1, 2]
  let a = buildFirst(4000000000)
  let b = buildSecond(4000000000)
  print(a)
  print(b)
  print(`${mine.count()}`)
}

function entrypoint() -> Nothing {
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
            "c8s6_cpu_join (no_optimize={no_optimize}): only the two running members' contexts \
             may stay allocated: {counters:?}"
        );
        assert_asan_clean(
            "c8s6_cpu_join",
            PARKED_AT_A_CPU_JOIN,
            "parent\n",
            no_optimize,
        );
    });
}

// ── what a parked frame holds ────────────────────────────────────────────────────────────────
//
// Bug each catches: the drop entry skips a kind of frame local (its frame home differs: two slots,
// a region behind a moved flag, a copy a statement made), so a task dropped while parked leaks it
// or, for a moved one, frees it twice. Tier: Medium.

const PARKED_HOLDERS: &str = "
shape Big {
  label: String
  count: Int
}

function fetchName(fail: Boolean) -> String errors {
  let status = wait __testFallibleAsync(fail)
  return `name${status}`
}

function pickShape(index: Int) -> Maybe<Big> {
  wait sleep(1)
  let items: Array<Big> = [{ label: `a`, count: 1 }]
  let found = items.get(index)
  return found
}

function takeShape(give kept: Maybe<Big>) -> Int {
  return 1
}
";

/// An errors-capable local (two frame slots), a `maybe<Shape>` local (a region behind a moved
/// flag) and a `maybe` given away before the wait (its flag cleared: freeing it again is a double
/// free), all parked in one wait.
#[test]
fn a_parked_task_releases_errors_and_maybe_locals_and_skips_a_given_one() {
    let source = format!(
        "{PARKED_HOLDERS}
function worker() -> Nothing {{
  let name = fetchName(false)
  let big = pickShape(0)
  let given = pickShape(0)
  let count = takeShape(given)
  wait sleep(500)
  print(name.or(`x`))
  print(`${{count}}`)
}}

function entrypoint() -> Nothing {{
  background worker()
  wait sleep(100)
  print(`parent`)
}}
"
    );
    balanced_and_asan_clean("c8s6_holders", &source, "parent\n");
}

/// The copy a `for` loop makes of its variable (`hold_copy_in_frame`) is held in a frame slot of the
/// open iteration, beside the local the iteration declares; the task is parked in the first
/// iteration.
#[test]
fn a_parked_task_releases_the_loop_variable_copy_and_the_iteration_local() {
    let source = "
function makeNames() -> Array<String> {
  let names: Array<String> = [`a`.toUpperCase(), `b`.toUpperCase()]
  return names
}

function worker() -> Nothing {
  let names = makeNames()
  for (label in names) {
    let shout: String = label.toUpperCase()
    wait sleep(500)
    print(shout)
  }
}

function entrypoint() -> Nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";
    balanced_and_asan_clean("c8s6_for_copies", source, "parent\n");
}

/// A `for` over a range yields at its back edge: parked between two iterations.
#[test]
fn a_task_parked_at_a_for_back_edge_releases_its_frame() {
    let source = "
function hog() -> Nothing {
  let keep: String = `k`.toUpperCase()
  let sink = 0
  for (outer in range(0, 2000000000)) {
    sink = sink + outer
  }
  print(keep)
  wait sleep(0)
}

function entrypoint() -> Nothing {
  background hog()
  wait sleep(100)
  print(`parent`)
}
";
    balanced_and_asan_clean("c8s6_for_back_edge", source, "parent\n");
}

// ── a task result nobody receives ────────────────────────────────────────────────────────────
//
// Bug each catches: a task that finished with a heap result nobody `.receive()`s leaves the result
// in the handle's outbox (or drops it when the handle was freed before the task finished), and
// nothing releases it. Tier: Medium. The task finishes before the parent exits, so its result is
// parked; a second program frees the handle before the task finishes.

/// `spawn_source(result_function)`: a parent that starts `make()` as a handle task and never
/// receives it; the task has finished by the time the parent's block ends and frees the handle.
fn unreceived_after_finish(result_function: &str) -> String {
    format!(
        "{result_function}

function entrypoint() -> Nothing {{
  let pending = background make()
  wait sleep(100)
  print(`parent`)
}}
"
    )
}

#[test]
fn an_unreceived_string_result_is_released_with_the_handle() {
    let source = unreceived_after_finish(
        "function make() -> String {
  wait sleep(1)
  return `big`.toUpperCase()
}",
    );
    balanced_and_asan_clean("c8s6_unreceived_string", &source, "parent\n");
}

#[test]
fn an_unreceived_array_result_is_released_with_the_handle() {
    let source = unreceived_after_finish(
        "function make() -> Array<String> {
  wait sleep(1)
  let items: Array<String> = [`a`.toUpperCase(), `b`.toUpperCase()]
  return items
}",
    );
    balanced_and_asan_clean("c8s6_unreceived_array", &source, "parent\n");
}

#[test]
fn an_unreceived_maybe_result_is_released_with_the_handle() {
    let source = unreceived_after_finish(
        "function make() -> Maybe<String> {
  wait sleep(1)
  let items: Array<String> = [`a`.toUpperCase()]
  let found = items.get(0)
  return found
}",
    );
    balanced_and_asan_clean("c8s6_unreceived_maybe", &source, "parent\n");
}

#[test]
fn an_unreceived_errors_result_is_released_with_the_handle() {
    let source = unreceived_after_finish(
        "function make() -> String errors {
  wait sleep(1)
  return `big`.toUpperCase()
}",
    );
    balanced_and_asan_clean("c8s6_unreceived_errors", &source, "parent\n");
}

/// The handle is freed (block end) while the task is still running: the abort drops the parked
/// task through its drop entry, and no completion is ever delivered.
#[test]
fn a_handle_freed_while_its_task_runs_aborts_it_and_leaks_nothing() {
    let source = "
function make() -> String {
  wait sleep(100)
  return `big`.toUpperCase()
}

function spawnAndLeave() -> Nothing {
  let pending = background make()
  wait sleep(1)
}

function entrypoint() -> Nothing {
  spawnAndLeave()
  wait sleep(300)
  print(`parent`)
}
";
    balanced_and_asan_clean("c8s6_result_after_free", source, "parent\n");
}

/// The task failed: the unreceived completion is an error object, not a payload. Bug caught: the
/// result release frees what a failed completion does not own, or frees the error object twice
/// (ASan). The counters cannot see the error object (`ynz_error_new` uses raw `malloc`), so this
/// test passes without the release; it guards the failing path against a wrong release only.
#[test]
fn an_unreceived_error_result_is_released_with_the_handle() {
    let source = unreceived_after_finish(
        "function make() -> String errors {
  let status = wait __testFallibleAsync(true)
  return `name${status}`
}",
    );
    balanced_and_asan_clean("c8s6_unreceived_error", &source, "parent\n");
}

/// A flat shape given to a task: the spawn allocates its cell and the ladder frees it
/// (`OWNED_CELL`); the callee owns the fields. A task dropped while parked releases the fields
/// through its drop entry and must not free the cell a second time through the frame slot (S5: the
/// slot is null after the callee's own release, and the cell is the ladder's). Bug caught: a drop
/// entry that frees the cell, or reads the slot for it, double frees or leaks it. Tier: High
/// (ownership boundary between the drop entry and the spawn ladder).
const PARKED_WITH_A_GIVEN_SHAPE: &str = "
shape Crew {
  label: String
  members: Array<String>
}

function worker(give crew: Crew) -> Nothing {
  let note: String = `n`.toUpperCase()
  wait sleep(500)
  print(`${crew.label}${note}`)
}

function entrypoint() -> Nothing {
  let members: Array<String> = [`a`.toUpperCase()]
  let crew: Crew = { label: `L`.toUpperCase(), members: members }
  background worker(crew)
  wait sleep(100)
  print(`parent`)
}
";

#[test]
fn a_parked_task_given_a_flat_shape_releases_its_fields_and_the_ladder_frees_the_cell() {
    balanced_and_asan_clean("c8s6_given_shape", PARKED_WITH_A_GIVEN_SHAPE, "parent\n");
}

/// The same task never polled (dropped in state 0): the given parameter is still a whole shape in
/// its frame slot.
#[test]
fn a_task_dropped_before_its_first_poll_releases_a_given_shape() {
    let source = "
shape Crew {
  label: String
  members: Array<String>
}

function worker(give crew: Crew) -> Nothing {
  wait sleep(500)
  print(crew.label)
}

function entrypoint() -> Nothing {
  let members: Array<String> = [`a`.toUpperCase()]
  let crew: Crew = { label: `L`.toUpperCase(), members: members }
  background worker(crew)
  print(`parent`)
}
";
    balanced_and_asan_clean("c8s6_given_shape_state0", source, "parent\n");
}

// ── a value given to the callee the task is parked in ───────────────────────────────────────
//
// The parked statement hands a local to a suspending callee, which owns it from then on. The local
// is not read after the park, so it has no frame slot: its stack slot is never written on the drop
// path. Bug each catches: the drop state reads that uninitialized stack slot and frees whatever it
// holds (a double free of the callee's value, or a free of a stray pointer), or the callee's own
// drop entry is skipped and the value leaks. Tier: High (memory safety of the drop entry).

/// One local given to the parked callee.
const GIVEN_TO_PARKED_CHILD: &str = "
function inner(give rows: Array<Int>) -> Nothing {
  wait sleep(500)
  print(`${rows.count()}`)
}

function worker() -> Nothing {
  let rows: Array<Int> = [1, 2, 3]
  inner(rows)
}

function entrypoint() -> Nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

/// A string given to the parked callee beside a local the task keeps.
const STRING_GIVEN_TO_PARKED_CHILD: &str = "
function inner(give label: String) -> Nothing {
  wait sleep(500)
  print(label)
}

function worker() -> Nothing {
  let label: String = `x`.toUpperCase()
  let kept: String = `y`.toUpperCase()
  inner(label)
  print(kept)
}

function entrypoint() -> Nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

/// Two locals given to the two members of an independent group the task is parked in.
const GIVEN_TO_PARKED_GROUP: &str = "
function first(give rows: Array<Int>) -> Int {
  wait sleep(500)
  return rows.count()
}

function second(give rows: Array<Int>) -> Int {
  wait sleep(500)
  return rows.count()
}

function worker() -> Nothing {
  let firstRows: Array<Int> = [1, 2, 3]
  let secondRows: Array<Int> = [4]
  let firstCount = first(firstRows)
  let secondCount = second(secondRows)
  print(`${firstCount + secondCount}`)
}

function entrypoint() -> Nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

/// The callee is called in a loop (its embedded frame reused) and parks on a later iteration.
const GIVEN_TO_PARKED_CHILD_IN_A_LOOP: &str = "
function inner(give rows: Array<Int>, sleepMilliseconds: Int) -> Nothing {
  let local: Array<Int> = [1]
  wait sleep(sleepMilliseconds)
  print(`${rows.count()}`)
}

function worker() -> Nothing {
  let i = 0
  while (i < 3) {
    let rows: Array<Int> = [i]
    inner(rows, i * 400)
    i = i + 1
  }
}

function entrypoint() -> Nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

/// Recursion that gives a local to the next level, parked at the deepest one.
const GIVEN_DOWN_A_PARKED_RECURSION: &str = "
function down(depth: Int, give carried: Array<Int>) -> Nothing {
  if (depth == 0) {
    wait sleep(500)
    print(`${carried.count()}`)
    return
  }
  let next: Array<Int> = [depth]
  down(depth - 1, next)
  print(`${carried.count()}`)
}

function entrypoint() -> Nothing {
  let start: Array<Int> = [0]
  background down(3, start)
  wait sleep(100)
  print(`parent`)
}
";

#[test]
fn a_local_given_to_the_parked_callee_is_released_once_by_the_callee() {
    balanced_and_asan_clean(
        "c8s6_given_to_parked_child",
        GIVEN_TO_PARKED_CHILD,
        "parent\n",
    );
    balanced_and_asan_clean(
        "c8s6_string_given_to_parked_child",
        STRING_GIVEN_TO_PARKED_CHILD,
        "parent\n",
    );
    balanced_and_asan_clean(
        "c8s6_given_to_parked_group",
        GIVEN_TO_PARKED_GROUP,
        "parent\n",
    );
    balanced_and_asan_clean(
        "c8s6_given_to_parked_child_in_a_loop",
        GIVEN_TO_PARKED_CHILD_IN_A_LOOP,
        "1\nparent\n",
    );
    balanced_and_asan_clean(
        "c8s6_given_down_a_parked_recursion",
        GIVEN_DOWN_A_PARKED_RECURSION,
        "parent\n",
    );
}

// ── `return suspendingCall()` ───────────────────────────────────────────────────────────────

/// Bug caught (High: leak, and a free of a never-written slot): `return suspendingCall()` stored the
/// callee's result and returned Ready without releasing the open locals (`mine`, `more` leaked), and
/// typeck's crossing scan did not count the call as a suspension, so those locals had no frame slot:
/// the resumed state released stack slots it never wrote. The second program parks the same function
/// in that call and drops it, so the drop entry reads the same slots.
#[test]
fn return_of_a_suspending_call_releases_the_open_locals() {
    let program = "
function inner() -> Int {
  wait sleep(1)
  return 7
}

function mid() -> Int {
  let mine: Array<Int> = [4, 5]
  let more: String = `x${mine.count()}`
  return inner()
}

function entrypoint() -> Nothing {
  let result = mid()
  print(`${result}`)
}
";
    balanced_and_asan_clean("c8s6_return_suspending_call", program, "7\n");
}

#[test]
fn a_task_parked_in_a_returned_suspending_call_releases_the_open_locals() {
    let program = "
function inner() -> Int {
  wait sleep(500)
  return 7
}

function worker() -> Int {
  let mine: Array<Int> = [4, 5]
  let more: String = `x${mine.count()}`
  return inner()
}

function entrypoint() -> Nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";
    balanced_and_asan_clean("c8s6_return_suspending_call_parked", program, "parent\n");
}

/// Bug caught (High: a free of a never-written slot, or a compile-time internal error): a
/// `return suspendingCall()` nested in an `if` (or in an `if` inside a loop) was not a suspension
/// to typeck's crossing scan, which only enters a branch whose body suspends. The open locals got
/// no frame slot: the resumed return released stack slots it never wrote (bad free / SEGV at -O0),
/// and since the drop entry refuses a binding with no frame slot, the program failed to compile.
/// Each program completes one call and drops a second task parked in the nested return.
#[test]
fn a_return_of_a_suspending_call_nested_in_a_branch_releases_the_open_locals() {
    let in_a_branch = "
function inner(n: Int) -> Int {
  wait sleep(n)
  return n
}

function worker(n: Int) -> Int {
  let keep: Array<Int> = [1, 2]
  let label: String = `k`.toUpperCase()
  if (n > 0) {
    return inner(n)
  }
  print(`${keep.count()} ${label}`)
  return 0
}

function entrypoint() -> Nothing {
  let done = worker(5)
  print(`${done}`)
  background worker(500)
  wait sleep(100)
  print(`parent`)
}
";
    balanced_and_asan_clean(
        "c8s6_return_suspending_in_branch",
        in_a_branch,
        "5\nparent\n",
    );

    let in_a_loop = "
function inner(n: Int) -> Int {
  wait sleep(n)
  return n
}

function worker(n: Int) -> Int {
  let keep: Array<Int> = [1, 2]
  let i = 0
  while (i < 3) {
    let local: String = `x`.toUpperCase()
    if (i == 1) {
      return inner(n)
    }
    i = i + 1
  }
  return keep.count()
}

function entrypoint() -> Nothing {
  let done = worker(5)
  print(`${done}`)
  background worker(500)
  wait sleep(100)
  print(`parent`)
}
";
    balanced_and_asan_clean("c8s6_return_suspending_in_loop", in_a_loop, "5\nparent\n");
}

/// Bug caught: pre-existing, found by the C8 round-2 review; a task dropped while its EMBEDDED child
/// is parked in `wait sleep` leaked the child's Tokio `Sleep` box (120 bytes, invisible to the alloc
/// counters, so LeakSanitizer is the check): the runtime's drop freed the sleep handle of the root
/// frame and of the recursion chain only. Tier Medium. Each frame's own wait-sleep drop state now
/// frees its handle once, so the same run also pins that the root frame and a recursion child (whose
/// handles the runtime used to free) are freed exactly once, not twice.
const EMBEDDED_CHILD_SLEEP: &str = "
function inner() -> Nothing {
  wait sleep(5000)
}

function worker() -> Nothing {
  inner()
}
";

const ROOT_SLEEP: &str = "
function worker() -> Nothing {
  wait sleep(5000)
}
";

const RECURSION_CHILD_SLEEP: &str = "
function worker(depth: Int) -> Nothing {
  if (depth > 0) {
    worker(depth - 1)
  }
  wait sleep(5000)
}
";

#[test]
fn a_task_dropped_while_a_frame_sleeps_frees_that_frames_sleep_box_once() {
    for (name, worker, spawn) in [
        ("embedded_child", EMBEDDED_CHILD_SLEEP, "worker()"),
        ("root", ROOT_SLEEP, "worker()"),
        ("recursion_child", RECURSION_CHILD_SLEEP, "worker(2)"),
    ] {
        let source = format!(
            "{worker}
function entrypoint() -> Nothing {{
  background {spawn}
  wait sleep(100)
  print(`parent`)
}}
"
        );
        support::assert_source_asan_and_leaks_clean(
            &format!("c8s6_sleep_{name}"),
            &source,
            "parent\n",
        );
    }
}

/// Bug caught: pre-existing, a leak found by the C8 round-2 review: `callee_result_type` listed only
/// a string, array, map and an errors result of those or of a scalar, so a `maybe`, a union or a
/// bignum a finished group member left in its return slot leaked when the task was dropped Pending
/// before the group joined. Tier Medium. One program per result type: `quick` finishes with the
/// value, `slow` keeps the group parked, and the parent returns while it waits.
fn finished_member_program(return_type: &str, body: &str) -> String {
    format!(
        "shape Tiny {{
  flag: Boolean
}}
shape Big {{
  label: String
  count: Int
}}
shape Thing = Tiny | Big

function quick() -> {return_type} {{
  wait sleep(1)
{body}
}}

function slow() -> Int {{
  wait sleep(500)
  return 1
}}

function worker() -> Nothing {{
  let a = quick()
  let b = slow()
  print(`${{b}}`)
}}

function entrypoint() -> Nothing {{
  background worker()
  wait sleep(100)
  print(`parent`)
}}
"
    )
}

macro_rules! finished_member_cases {
    ($($name:ident: $return_type:literal, $body:literal;)+) => {
        $(
            #[test]
            fn $name() {
                let source = finished_member_program($return_type, $body);
                balanced_and_asan_clean(stringify!($name), &source, "parent\n");
            }
        )+
    };
}

finished_member_cases! {
    a_finished_member_returning_a_maybe_string_is_released_when_the_task_is_dropped: "Maybe<String>",
        "  let items: Array<String> = [`a`.toUpperCase()]\n  return items.get(0)";
    a_finished_member_returning_a_maybe_shape_is_released_when_the_task_is_dropped: "Maybe<Big>",
        "  let items: Array<Big> = [{ label: `a`, count: 1 }]\n  return items.get(0)";
    a_finished_member_returning_a_maybe_decimal_is_released_when_the_task_is_dropped: "Maybe<Number<20> >",
        "  let items: Array<Number<20> > = [2.5]\n  return items.get(0)";
    a_finished_member_returning_a_union_is_released_when_the_task_is_dropped: "Thing",
        "  let big: Big = { label: `a`.toUpperCase(), count: 1 }\n  return big";
    a_finished_member_returning_a_maybe_string_errors_is_released_when_the_task_is_dropped: "Maybe<String> errors",
        "  let items: Array<String> = [`a`.toUpperCase()]\n  return items.get(0)";
    a_finished_member_returning_a_maybe_shape_errors_is_released_when_the_task_is_dropped: "Maybe<Big> errors",
        "  let items: Array<Big> = [{ label: `a`, count: 1 }]\n  return items.get(0)";
    a_finished_member_returning_a_union_errors_is_released_when_the_task_is_dropped: "Thing errors",
        "  let big: Big = { label: `a`.toUpperCase(), count: 1 }\n  return big";
    a_finished_member_returning_a_bignum_errors_is_released_when_the_task_is_dropped: "Number<40> errors",
        "  let made: Number<40> = 2.5\n  return made";
}

/// Bug caught: pre-existing, a leak found by the C8 round-2 review: `handle_result_release_fn`
/// released the unreceived completion of a task only for a string, array, map, plain `maybe` and an
/// errors result of a plain word or scalar, so a `maybe<string> errors` (and every other result type
/// a handle can carry that the list lacked) leaked when nobody received it. Tier Medium. One program
/// per result type: the task finishes, the parent never receives it and frees the handle.
fn unreceived_result_program(return_type: &str, body: &str) -> String {
    unreceived_after_finish(&format!(
        "shape Tiny {{
  flag: Boolean
}}
shape Big {{
  label: String
  count: Int
}}
shape Thing = Tiny | Big

function make() -> {return_type} {{
  wait sleep(1)
{body}
}}"
    ))
}

macro_rules! unreceived_result_cases {
    ($($name:ident: $return_type:literal, $body:literal;)+) => {
        $(
            #[test]
            fn $name() {
                let source = unreceived_result_program($return_type, $body);
                balanced_and_asan_clean(stringify!($name), &source, "parent\n");
            }
        )+
    };
}

unreceived_result_cases! {
    an_unreceived_maybe_shape_result_is_released_with_the_handle: "Maybe<Big>",
        "  let items: Array<Big> = [{ label: `a`, count: 1 }]\n  return items.get(0)";
    an_unreceived_maybe_decimal_result_is_released_with_the_handle: "Maybe<Number<20> >",
        "  let items: Array<Number<20> > = [2.5]\n  return items.get(0)";
    an_unreceived_maybe_string_errors_result_is_released_with_the_handle: "Maybe<String> errors",
        "  let items: Array<String> = [`a`.toUpperCase()]\n  return items.get(0)";
    an_unreceived_maybe_shape_errors_result_is_released_with_the_handle: "Maybe<Big> errors",
        "  let items: Array<Big> = [{ label: `a`, count: 1 }]\n  return items.get(0)";
    an_unreceived_maybe_decimal_errors_result_is_released_with_the_handle: "Maybe<Number<20> > errors",
        "  let items: Array<Number<20> > = [2.5]\n  return items.get(0)";
    an_unreceived_bignum_errors_result_is_released_with_the_handle: "Number<40> errors",
        "  let made: Number<40> = 2.5\n  return made";
}

// ── a union a task returns through its handle ───────────────────────────────────────────────
//
// Bug caught: pre-existing (wrong value on main @ e730851, use-after-free from C7 on): the
// completion word of a union result was the address of the union's region INSIDE the finished
// child's frame, which is freed right after, so a received union was read from freed memory and an
// unreceived one could not be released. Tier High (a use-after-free on a data path). The spawn
// wrapper now hands the handle a move of the slot into one cell of its own; the receive side takes
// that cell, an unreceived result is released through the one union release.

fn union_handle_program(return_type: &str, receive_lines: &str) -> String {
    format!(
        "shape Tiny {{
  flag: Boolean
}}
shape Big {{
  label: String
  count: Int
}}
shape Thing = Tiny | Big

function make() -> {return_type} {{
  wait sleep(1)
  let big: Big = {{ label: `abc`.toUpperCase(), count: 7 }}
  return big
}}

function entrypoint() -> Nothing {{
  let pending = background make()
  wait sleep(50)
{receive_lines}
}}
"
    )
}

const RECEIVE_AND_NARROW: &str = "  let got = pending.receive()
  let spare: Tiny = { flag: true }
  let fallback: Thing = spare
  let value = got.or(fallback)
  if (value) {
    is Big => {
      print(value.label)
      print(value.count)
    }
    is Tiny => {
      print(`tiny`)
    }
  }";

const RECEIVE_AND_CHECK: &str = "  let got = pending.receive()
  print(got.failed())";

const NEVER_RECEIVE: &str = "  print(`parent`)";

macro_rules! union_handle_cases {
    ($($name:ident: $return_type:literal, $receive:expr, $expected:literal;)+) => {
        $(
            #[test]
            fn $name() {
                let source = union_handle_program($return_type, $receive);
                balanced_and_asan_clean(stringify!($name), &source, $expected);
            }
        )+
    };
}

union_handle_cases! {
    a_union_a_task_returns_is_received_and_narrowed_once: "Thing", RECEIVE_AND_NARROW, "ABC\n7\n";
    a_union_errors_a_task_returns_is_received_and_narrowed_once: "Thing errors", RECEIVE_AND_NARROW, "ABC\n7\n";
    a_union_a_task_returns_is_received_without_narrowing_and_released_once: "Thing", RECEIVE_AND_CHECK, "false\n";
    a_union_errors_a_task_returns_is_received_without_narrowing_and_released_once: "Thing errors", RECEIVE_AND_CHECK, "false\n";
    an_unreceived_union_result_is_released_with_the_handle: "Thing", NEVER_RECEIVE, "parent\n";
    an_unreceived_union_errors_result_is_released_with_the_handle: "Thing errors", NEVER_RECEIVE, "parent\n";
}

/// Bug caught: the union-through-a-handle fix moving the embedded return into a cell too (a regression
/// the cell tests above would not see); tier High. The embedded call keeps the frame-region form: a
/// union a suspending callee returns to a plain call site is read from the callee's region before its
/// frame is freed, as before.
#[test]
fn a_union_an_embedded_suspending_call_returns_is_unchanged() {
    let source = "shape Tiny {
  flag: Boolean
}
shape Big {
  label: String
  count: Int
}
shape Thing = Tiny | Big

function make() -> Thing {
  wait sleep(1)
  let big: Big = { label: `abc`.toUpperCase(), count: 7 }
  return big
}

function entrypoint() -> Nothing {
  let value = make()
  if (value) {
    is Big => {
      print(value.label)
    }
    is Tiny => {
      print(`tiny`)
    }
  }
}
";
    balanced_and_asan_clean("c8s6_embedded_union", source, "ABC\n");
}

/// Bug caught: the union-through-a-handle move freeing or reading a cell the failing path never made
/// (a double free); tier High. The failing `-> Thing errors` task: no cell exists (the move glue
/// leaves `ok` 0), the error object is received or released, and nothing is freed twice.
#[test]
fn a_failed_union_errors_task_delivers_its_error_and_no_cell() {
    for (name, receive, expected) in [
        (
            "received",
            "  let got = pending.receive()\n  print(got.failed())",
            "true\n",
        ),
        ("unreceived", "  print(`parent`)", "parent\n"),
    ] {
        let source = format!(
            "shape Tiny {{
  flag: Boolean
}}
shape Big {{
  label: String
  count: Int
}}
shape Thing = Tiny | Big

function make() -> Thing errors {{
  let status = wait __testFallibleAsync(true)
  let tiny: Tiny = {{ flag: status == 42 }}
  return tiny
}}

function entrypoint() -> Nothing {{
  let pending = background make()
  wait sleep(50)
{receive}
}}
"
        );
        balanced_and_asan_clean(&format!("c8s6_failed_union_{name}"), &source, expected);
    }
}

/// Bug caught: a leak found by the C8 round-3 review: a handle receive of a `number<40> errors`
/// result marked the binding's ok word as the handle's own (`ec_borrowed_ok`, right for a `number`
/// the handle copies into its buffer), but a bignum's ok word is the text the task allocated and the
/// handle hands over (`RET_KIND_EC_WORD`), so the binding's scope exit freed none of it. Tier Medium.
/// Received and checked, received and read with `or`.
#[test]
fn a_received_bignum_errors_result_is_released_with_its_binding() {
    for (name, read) in [
        ("failed", "print(got.failed())"),
        (
            "or",
            "let fallback: Number<40> = 0.5\n  print(got.or(fallback))",
        ),
    ] {
        let source = format!(
            "function make() -> Number<40> errors {{
  wait sleep(1)
  let made: Number<40> = 1.5
  return made
}}

function entrypoint() -> Nothing {{
  let pending = background make()
  wait sleep(20)
  let got = pending.receive()
  {read}
}}
"
        );
        let expected = if name == "failed" { "false\n" } else { "1.5\n" };
        balanced_and_asan_clean(&format!("c8s6_received_bignum_{name}"), &source, expected);
    }
}

// ── a `maybe` of a union a task returns ─────────────────────────────────────────────────────
//
// Bug caught: pre-existing, found by the C8 round-3 review (the same mechanism as the union cases
// above): `-> maybe<Thing>` and `-> maybe<Thing> errors` left the union's region address in the
// completion, so the finished frame's region was read after it was freed on a receive, and the
// payload string leaked when the completion was unreceived or a finished group member was dropped.
// Tier High (a use-after-free on a data path). They take the same move and release as a plain union.

fn maybe_union_program(return_type: &str, body: &str, entry: &str) -> String {
    format!(
        "shape Tiny {{
  flag: Boolean
}}
shape Big {{
  label: String
  count: Int
}}
shape Thing = Tiny | Big

function make(flag: Boolean) -> {return_type} {{
  wait sleep(1)
{body}
}}

function makeBig() -> {return_type} {{
  wait sleep(1)
  let big: Big = {{ label: `abc`.toUpperCase(), count: 7 }}
  let wrapped: Thing = big
  let items: Array<Thing> = [wrapped]
  return items.get(0)
}}

function slow() -> Int {{
  wait sleep(500)
  return 1
}}

function worker() -> Nothing {{
  let first = makeBig()
  let second = slow()
  print(`${{second}}`)
}}

function entrypoint() -> Nothing {{
{entry}
}}
"
    )
}

const MAYBE_UNION_SOME: &str = "  if (flag) {
    let tiny: Tiny = { flag: true }
    let wrapped: Thing = tiny
    let items: Array<Thing> = [wrapped]
    return items.get(0)
  }
  let big: Big = { label: `abc`.toUpperCase(), count: 7 }
  let wrapped: Thing = big
  let items: Array<Thing> = [wrapped]
  return items.get(0)";

const MAYBE_UNION_NONE: &str = "  let items: Array<Thing> = []
  return items.get(0)";

const RECEIVE_MAYBE_UNION: &str = "  let pending = background make(false)
  wait sleep(50)
  let got = pending.receive()
  let fallback: Maybe<Thing> = none
  let held = got.or(fallback)
  print(held.exists())
  if (held.exists()) {
    let value = held.value
    if (value) {
      is Big => {
        print(value.label)
        print(value.count)
      }
      is Tiny => {
        print(`tiny`)
      }
    }
  }";

const NEVER_RECEIVE_MAYBE_UNION: &str = "  let pending = background make(false)
  wait sleep(50)
  print(`parent`)";

const DROP_GROUP_WITH_MAYBE_UNION: &str = "  background worker()
  wait sleep(100)
  print(`parent`)";

macro_rules! maybe_union_handle_cases {
    ($($name:ident: $return_type:literal, $body:expr, $entry:expr, $expected:literal;)+) => {
        $(
            #[test]
            fn $name() {
                let source = maybe_union_program($return_type, $body, $entry);
                balanced_and_asan_clean(stringify!($name), &source, $expected);
            }
        )+
    };
}

maybe_union_handle_cases! {
    a_maybe_union_a_task_returns_is_received_and_read_once: "Maybe<Thing>", MAYBE_UNION_SOME, RECEIVE_MAYBE_UNION, "true\nABC\n7\n";
    a_maybe_union_errors_a_task_returns_is_received_and_read_once: "Maybe<Thing> errors", MAYBE_UNION_SOME, RECEIVE_MAYBE_UNION, "true\nABC\n7\n";
    a_none_maybe_union_a_task_returns_is_received_once: "Maybe<Thing>", MAYBE_UNION_NONE, RECEIVE_MAYBE_UNION, "false\n";
    a_none_maybe_union_errors_a_task_returns_is_received_once: "Maybe<Thing> errors", MAYBE_UNION_NONE, RECEIVE_MAYBE_UNION, "false\n";
    an_unreceived_maybe_union_result_is_released_with_the_handle: "Maybe<Thing>", MAYBE_UNION_SOME, NEVER_RECEIVE_MAYBE_UNION, "parent\n";
    an_unreceived_maybe_union_errors_result_is_released_with_the_handle: "Maybe<Thing> errors", MAYBE_UNION_SOME, NEVER_RECEIVE_MAYBE_UNION, "parent\n";
    an_unreceived_none_maybe_union_result_is_released_with_the_handle: "Maybe<Thing>", MAYBE_UNION_NONE, NEVER_RECEIVE_MAYBE_UNION, "parent\n";
    a_finished_group_member_returning_a_maybe_union_is_released_when_the_task_is_dropped: "Maybe<Thing>", MAYBE_UNION_SOME, DROP_GROUP_WITH_MAYBE_UNION, "parent\n";
    a_finished_group_member_returning_a_maybe_union_errors_is_released_when_the_task_is_dropped: "Maybe<Thing> errors", MAYBE_UNION_SOME, DROP_GROUP_WITH_MAYBE_UNION, "parent\n";
}
