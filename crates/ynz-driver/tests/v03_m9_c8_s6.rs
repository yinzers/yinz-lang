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
  label: string
  count: int
}

function fetchName(fail: boolean) -> string errors {
  let status = wait __testFallibleAsync(fail)
  return `name${status}`
}

function pickShape(index: int) -> maybe<Big> {
  wait sleep(1)
  let items: array<Big> = [{ label: `a`, count: 1 }]
  let found = items.get(index)
  return found
}

function takeShape(give kept: maybe<Big>) -> int {
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
function worker() -> nothing {{
  let name = fetchName(false)
  let big = pickShape(0)
  let given = pickShape(0)
  let count = takeShape(given)
  wait sleep(500)
  print(name.or(`x`))
  print(`${{count}}`)
}}

function entrypoint() -> nothing {{
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
function makeNames() -> array<string> {
  let names: array<string> = [`a`.toUpperCase(), `b`.toUpperCase()]
  return names
}

function worker() -> nothing {
  let names = makeNames()
  for (label in names) {
    let shout: string = label.toUpperCase()
    wait sleep(500)
    print(shout)
  }
}

function entrypoint() -> nothing {
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
function hog() -> nothing {
  let keep: string = `k`.toUpperCase()
  let sink = 0
  for (outer in range(0, 2000000000)) {
    sink = sink + outer
  }
  print(keep)
  wait sleep(0)
}

function entrypoint() -> nothing {
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

function entrypoint() -> nothing {{
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
        "function make() -> string {
  wait sleep(1)
  return `big`.toUpperCase()
}",
    );
    balanced_and_asan_clean("c8s6_unreceived_string", &source, "parent\n");
}

#[test]
fn an_unreceived_array_result_is_released_with_the_handle() {
    let source = unreceived_after_finish(
        "function make() -> array<string> {
  wait sleep(1)
  let items: array<string> = [`a`.toUpperCase(), `b`.toUpperCase()]
  return items
}",
    );
    balanced_and_asan_clean("c8s6_unreceived_array", &source, "parent\n");
}

#[test]
fn an_unreceived_maybe_result_is_released_with_the_handle() {
    let source = unreceived_after_finish(
        "function make() -> maybe<string> {
  wait sleep(1)
  let items: array<string> = [`a`.toUpperCase()]
  let found = items.get(0)
  return found
}",
    );
    balanced_and_asan_clean("c8s6_unreceived_maybe", &source, "parent\n");
}

#[test]
fn an_unreceived_errors_result_is_released_with_the_handle() {
    let source = unreceived_after_finish(
        "function make() -> string errors {
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
function make() -> string {
  wait sleep(100)
  return `big`.toUpperCase()
}

function spawnAndLeave() -> nothing {
  let pending = background make()
  wait sleep(1)
}

function entrypoint() -> nothing {
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
        "function make() -> string errors {
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
  label: string
  members: array<string>
}

function worker(give crew: Crew) -> nothing {
  let note: string = `n`.toUpperCase()
  wait sleep(500)
  print(`${crew.label}${note}`)
}

function entrypoint() -> nothing {
  let members: array<string> = [`a`.toUpperCase()]
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
  label: string
  members: array<string>
}

function worker(give crew: Crew) -> nothing {
  wait sleep(500)
  print(crew.label)
}

function entrypoint() -> nothing {
  let members: array<string> = [`a`.toUpperCase()]
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
function inner(give rows: array<int>) -> nothing {
  wait sleep(500)
  print(`${rows.count()}`)
}

function worker() -> nothing {
  let rows: array<int> = [1, 2, 3]
  inner(rows)
}

function entrypoint() -> nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

/// A string given to the parked callee beside a local the task keeps.
const STRING_GIVEN_TO_PARKED_CHILD: &str = "
function inner(give label: string) -> nothing {
  wait sleep(500)
  print(label)
}

function worker() -> nothing {
  let label: string = `x`.toUpperCase()
  let kept: string = `y`.toUpperCase()
  inner(label)
  print(kept)
}

function entrypoint() -> nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

/// Two locals given to the two members of an independent group the task is parked in.
const GIVEN_TO_PARKED_GROUP: &str = "
function first(give rows: array<int>) -> int {
  wait sleep(500)
  return rows.count()
}

function second(give rows: array<int>) -> int {
  wait sleep(500)
  return rows.count()
}

function worker() -> nothing {
  let a: array<int> = [1, 2, 3]
  let b: array<int> = [4]
  let x = first(a)
  let y = second(b)
  print(`${x + y}`)
}

function entrypoint() -> nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

/// The callee is called in a loop (its embedded frame reused) and parks on a later iteration.
const GIVEN_TO_PARKED_CHILD_IN_A_LOOP: &str = "
function inner(give rows: array<int>, ms: int) -> nothing {
  let local: array<int> = [1]
  wait sleep(ms)
  print(`${rows.count()}`)
}

function worker() -> nothing {
  let i = 0
  while (i < 3) {
    let rows: array<int> = [i]
    inner(rows, i * 400)
    i = i + 1
  }
}

function entrypoint() -> nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";

/// Recursion that gives a local to the next level, parked at the deepest one.
const GIVEN_DOWN_A_PARKED_RECURSION: &str = "
function down(n: int, give acc: array<int>) -> nothing {
  if (n == 0) {
    wait sleep(500)
    print(`${acc.count()}`)
    return
  }
  let next: array<int> = [n]
  down(n - 1, next)
  print(`${acc.count()}`)
}

function entrypoint() -> nothing {
  let start: array<int> = [0]
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
function inner() -> int {
  wait sleep(1)
  return 7
}

function mid() -> int {
  let mine: array<int> = [4, 5]
  let more: string = `x${mine.count()}`
  return inner()
}

function entrypoint() -> nothing {
  let result = mid()
  print(`${result}`)
}
";
    balanced_and_asan_clean("c8s6_return_suspending_call", program, "7\n");
}

#[test]
fn a_task_parked_in_a_returned_suspending_call_releases_the_open_locals() {
    let program = "
function inner() -> int {
  wait sleep(500)
  return 7
}

function worker() -> int {
  let mine: array<int> = [4, 5]
  let more: string = `x${mine.count()}`
  return inner()
}

function entrypoint() -> nothing {
  background worker()
  wait sleep(100)
  print(`parent`)
}
";
    balanced_and_asan_clean("c8s6_return_suspending_call_parked", program, "parent\n");
}
