//! v0.3-M9 C8 slice 0: red tests for the suspension-frame release (card #385, plan #17).
//!
//! C7 releases every heap local on the plain path. A state-machine function (one that suspends)
//! releases nothing yet, and a task dropped while Pending never releases its frame. Each test
//! here fails today for the producer its name gives; the slice that fixes the producer removes
//! the `#[ignore]` and keeps the assertion byte-identical (the planned-RED ratchet of
//! `frago002_c1_c2_planned_red.rs`). Run them all:
//!
//! ```text
//! cargo test -p ynz-driver --test v03_m9_c8_s0_red -- --ignored
//! ```
//!
//! Groups:
//! - `c2_*`: the C2 frame-retirement fixtures (the caller's `free_frame` after Ready, the spawned
//!   task's drop ladder). Their ASan guard passes today and must stay green; their balance is red.
//! - `dropped_while_pending_*` (producer P9): a task retired before it finished leaks the locals in its frame
//!   (the card step 774 amendment).
//! - `producer_*`: one leak test per producer of the C8 producer table (the report names them).
//!   Each program or fixture is the smallest one that leaks for that producer alone, or the
//!   corpus fixtures that STILL leak once the producers before it are fixed (measured with a
//!   throwaway prototype of the earlier producers, not committed).
//! - `plain_*`: leaks on the plain path (no state machine), outside the sweep's list.
//!
//! The handle-scope tests are in `v03_m8_handle_scope_pin.rs` (rewritten by this slice).
//!
//! Every test runs both optimization levels at once (`at_both_optimization_levels`), each in its
//! own process, and reads its own counter file (`.claude/rules/test-parallelism.md`).

mod support;

use support::{
    assert_asan_clean_and_prints, at_both_optimization_levels, build, build_source, run_counted,
};

/// Run `built` with the counters on and require every heap allocation and every string freed.
/// `what` names the producer in the failure message. The run must exit 0 and print `expected`
/// when one is given (`None` for a corpus fixture whose output other suites pin).
fn assert_run_balances(built: &support::Built, what: &str, expected: Option<&str>, no_opt: bool) {
    let (run, counters) = run_counted(built);
    assert_eq!(
        run.exit_code, 0,
        "{what} (no_optimize={no_opt}) must exit 0; stderr:\n{}",
        run.stderr
    );
    if let Some(expected) = expected {
        assert_eq!(run.stdout, expected, "{what} (no_optimize={no_opt})");
    }
    assert_eq!(
        counters.free, counters.alloc,
        "{what} (no_optimize={no_opt}): every heap allocation must be freed: {counters:?}"
    );
    assert_eq!(
        counters.string_free, counters.string_alloc,
        "{what} (no_optimize={no_opt}): every string must be freed: {counters:?}"
    );
}

/// `green_program!(case, SOURCE, "stdout", "producer: what leaks")`: the generated program, built and run at
/// both optimization levels, prints `stdout` and frees what it allocates.
macro_rules! green_program {
    ($case:ident, $source:expr, $expected:literal, $bug:literal) => {
        #[doc = $bug]
        #[test]
        fn $case() {
            at_both_optimization_levels(|no_optimize| {
                let built = build_source($source, stringify!($case), no_optimize);
                assert_run_balances(&built, $bug, Some($expected), no_optimize);
            });
        }
    };
}

/// `green_fixture!(case, "fixture.ynz", "producer: what leaks")`: the fixture, built and run at both
/// optimization levels, frees what it allocates.
macro_rules! green_fixture {
    ($case:ident, $fixture:literal, $bug:literal) => {
        #[doc = $bug]
        #[test]
        fn $case() {
            at_both_optimization_levels(|no_optimize| {
                let built = build($fixture, no_optimize);
                assert_run_balances(&built, $bug, None, no_optimize);
            });
        }
    };
}

// ── (a) C2 frame-retirement fixtures ────────────────────────────────────────────────────────

/// ASan guard, green today and after C8: the caller's `free_frame` after Ready neither double
/// frees a frame-held local nor reads the freed return slot. C8's frame release must keep it so.
#[test]
fn c2_caller_free_frame_stays_asan_clean() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean_and_prints(
            "v0_3_m9_c2_caller_free_frame.ynz",
            no_optimize,
            "8\n3\n2\n12\n",
        );
    });
}

/// ASan guard, green today and after C8: the worker's own release of its `give` parameter and the
/// spawn drop ladder never free the same clone.
#[test]
fn c2_spawn_drop_ladder_stays_asan_clean() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean_and_prints(
            "v0_3_m9_c2_spawn_drop_ladder.ynz",
            no_optimize,
            "3\n7\n2\n8\n3\n",
        );
    });
}

green_fixture!(
    c2_caller_free_frame_balances,
    "v0_3_m9_c2_caller_free_frame.ynz",
    "producer P1 (frame locals): `kept`, `extra`, `inner`, `mine` of a suspending callee leak (30 allocations, 6 frees)"
);

green_fixture!(
    c2_spawn_drop_ladder_balances,
    "v0_3_m9_c2_spawn_drop_ladder.ynz",
    "producers P1 + P8 (worker locals, drop ladder beside the worker's own `give` release): 17 allocations, 9 frees"
);

// ── (c) a task dropped while Pending ────────────────────────────────────────────────────────

/// The child holds heap locals across a long sleep; the parent returns first, so the runtime
/// retires the task while it is Pending. The ladder frees the `give` argument; the locals in the
/// frame (`mine`, `label`) are freed by nobody.
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

green_program!(
    dropped_while_pending_at_shutdown,
    DROPPED_AT_SHUTDOWN,
    "parent\n",
    "producer P9 (task dropped while Pending): the frame locals of a task retired mid-wait leak (8 allocations, 6 frees; 1 string, 0 freed)"
);

/// The Pending task is parked inside a nested suspending callee, so the locals sit in a child
/// frame the parent's drop must reach too (card step 774: "covers nested child frames").
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

green_program!(
    dropped_while_pending_in_nested_child_frame,
    DROPPED_IN_NESTED_CHILD,
    "parent\n",
    "producer P9 (task dropped while Pending in a nested callee): the locals of the parent frame and of the child frame leak"
);

// ── (d) one leak test per producer ──────────────────────────────────────────────────────────

/// P1: heap locals of a suspending function, across a `wait`, at a block end and at a `return`.
/// Nothing here builds a temporary (`print` takes an int), so only the scope-exit release is
/// missing: `rows` and `label` of both calls, `inner` of the first.
const SM_LOCALS: &str = "
function holder(enter: boolean) -> int {
  let rows: array<int> = [1, 2, 3]
  let label: string = `crew`.toUpperCase()
  wait sleep(1)
  if (enter) {
    let inner: array<int> = [4]
    wait sleep(1)
    return rows.count() + label.count() + inner.count()
  }
  return rows.count()
}

function entrypoint() -> nothing {
  let first: int = holder(true)
  let second: int = holder(false)
  print(first)
  print(second)
}
";

green_program!(
    producer_p1_frame_locals_at_scope_exit,
    SM_LOCALS,
    "8\n3\n",
    "producer P1 (frame locals): a suspending function's heap locals are never released at a block end or `return`"
);

/// P2: statement-end temporaries in a suspending function. No heap local is bound: the array
/// `make()` returns and the interpolated string each die at the end of their statement.
const SM_TEMPORARIES: &str = "
function make() -> array<int> {
  return [1, 2, 3]
}

function entrypoint() -> nothing {
  wait sleep(1)
  let n: int = make().count()
  print(n)
  print(`total ${n}`)
}
";

green_program!(
    producer_p2_statement_end_temporaries,
    SM_TEMPORARIES,
    "3\ntotal 3\n",
    "producer P2 (temporaries): a call result and a fresh string in a suspending function are never released at their statement end"
);

green_fixture!(
    producer_p3_loop_copy_overwrite_before_wait,
    "v0_3_m9_c6_borrow_sm_loop_map_overwrite.ynz",
    "producer P3 (loop copies): the map-entry copy a suspending `for` body reads before its `wait` after an overwrite is never released"
);

green_fixture!(
    producer_p3_loop_copy_body_write,
    "v0_3_m3a_p1_for_body_write.ynz",
    "producer P3 (loop copies): the copy of a loop variable written in a suspending body is never released"
);

green_fixture!(
    producer_p4_suspending_call_argument_copy,
    "v0_3_m6_anon_struct_arg_pure_call.ynz",
    "producer P4 (suspending-call argument copies): the shape literal staged into `wait crew({ ... })` is copied and never released"
);

green_fixture!(
    producer_p4_suspending_call_maybe_argument_copy,
    "v0_3_m6_maybe_arg_pure_call.ynz",
    "producer P4 (suspending-call argument copies): the `maybe` staged into a suspending call is copied and never released"
);

green_fixture!(
    producer_p5_cpu_group_result_string,
    "v0_3_m3d_return_class_string.ynz",
    "producer P5 (CPU-group results): the strings two spike-group members return into the host's result slots are never released"
);

green_fixture!(
    producer_p5_cpu_group_result_array,
    "v0_3_m3d_return_class_array.ynz",
    "producer P5 (CPU-group results): the arrays two spike-group members return into the host's result slots are never released"
);

green_fixture!(
    producer_p6_errors_propagation_crossing_local,
    "v0_3_m3a_p1_ec_crossing_local_propagated_string.ynz",
    "producer P6 (errors in a suspending function): an auto-propagation edge and an errors-capable local release nothing in a resume function"
);

green_fixture!(
    producer_p7_union_in_suspending_function,
    "v0_3_m9_union_wrap_arg.ynz",
    "producer P7 (unions in a suspending function): a union envelope and the string it holds, built or wrapped in a resume function, leak"
);

green_fixture!(
    producer_p7_union_crossing_binding,
    "v0_3_m9_union_crossing_binding.ynz",
    "producer P7 (unions in a suspending function): the `Thing errors` local `held` that crosses a `wait` leaks its slot and string"
);

green_fixture!(
    producer_p8_spawn_ladder_array_argument,
    "bg_arg_container_add_gives_row.ynz",
    "producer P8 (spawn ladder, suspending spawner): an array argument copy and the spawner's own locals leak"
);

green_fixture!(
    producer_p8_spawn_ladder_channel_send_array,
    "bg_arg_channel_send_array.ynz",
    "producer P8 (spawn ladder, suspending spawner): a sent array argument leaks beside a channel"
);

green_fixture!(
    producer_p8_spawn_ladder_array_shape_give,
    "m5_p3_sweep_bg_array_shape_give_wait.ynz",
    "producer P8 (spawn ladder, suspending spawner): a given array and a shape argument leak"
);

/// P8: the state-machine union spawn string leak of the hand-over: a suspending task is handed a
/// union holding a string. The task's slot is freed flat by the ladder; its string never is
/// (2 strings allocated, 1 freed).
const SM_UNION_SPAWN: &str = "
shape Circle {
  radius: float
}
shape Tag {
  name: string
}
shape Box {
  choice: Circle | Tag
  id: int
}

function eat(held: Circle | Tag) -> nothing {
  wait sleep(1)
  print(777)
}

function launch() -> nothing {
  let tag: Tag = { name: `boxed` }
  let box: Box = { choice: tag, id: 1 }
  background eat(box.choice)
}

function entrypoint() -> nothing {
  launch()
  wait sleep(300)
}
";

green_program!(
    producer_p8_state_machine_union_spawn_string,
    SM_UNION_SPAWN,
    "777\n",
    "producer P8 (spawn ladder): the string inside a union argument of a suspending task is never freed"
);

// ── plain-path leaks the sweep does not list ────────────────────────────────────────────────

green_fixture!(
    plain_mapentry_loop_variable_spawn_escape,
    "m5_p3_sweep_mapentry_bg_escape.ynz",
    "plain producer: a `MapEntry` loop variable passed to `background` gets a deep copy the spawn drop ladder never frees (8 allocations, 7 frees)"
);

/// A shape handed to a union parameter of a spawned CPU task (a callee with no `wait`): the
/// spawner's `launch` is a plain function and the task is not a state machine, yet 2 strings are
/// allocated and 1 freed. The sweep does not see it (no fixture spawns this way and balances
/// counters), and it is not in `KNOWN_PLAIN_LEAKS`.
const PLAIN_SHAPE_INTO_UNION_SPAWN: &str = "
shape Circle {
  radius: float
}
shape Tag {
  name: string
}

function eat(held: Circle | Tag) -> nothing {
  print(777)
}

function launch() -> nothing {
  let tag: Tag = { name: `boxed` }
  background eat(tag)
}

function entrypoint() -> nothing {
  launch()
  wait sleep(300)
}
";

green_program!(
    plain_shape_into_union_parameter_spawn,
    PLAIN_SHAPE_INTO_UNION_SPAWN,
    "777\n",
    "plain producer: a shape spawned into a union parameter of a non-suspending task leaves one of its two strings unfreed"
);

// ── C8 slice 3: regressions the producer fixtures do not pin ────────────────────────────────

/// A suspending `for` over a fresh call result calls the function once, not once per header and
/// body evaluation (Medium: before slice 3 `made` printed 7 times and 6 arrays leaked).
const SM_LOOP_ITERABLE_ONCE: &str = "
function make() -> array<int> {
  print(`made`)
  return [1, 2, 3]
}

function entrypoint() -> nothing {
  let total = 0
  for (x in make()) {
    total = total + x
  }
  wait sleep(1)
  print(total.toString())
}
";

green_program!(
    sm_loop_over_a_fresh_collection_evaluates_it_once,
    SM_LOOP_ITERABLE_ONCE,
    "made\n6\n",
    "slice 3: the iterated call of a suspending `for` is evaluated once and its array freed once"
);

/// A `return` inside a suspending loop body releases the copy of the loop variable (Medium: the
/// early exit left the entry's copy unreleased).
const SM_LOOP_COPY_EARLY_RETURN: &str = "
function scan() -> int {
  let table: map<string, string> = { `k`: `v-${1}` }
  for (entry in table) {
    table[`k`] = `w-${2}`
    print(entry.value)
    wait sleep(1)
    return 7
  }
  return 0
}

function entrypoint() -> nothing {
  let found = scan()
  print(found.toString())
}
";

green_program!(
    sm_loop_copy_is_released_by_a_return_in_the_body,
    SM_LOOP_COPY_EARLY_RETURN,
    "v-1\n7\n",
    "slice 3: a `return` inside a suspending loop body leaves the loop variable's copy unreleased"
);

/// P4 with a string, array or map copy: an argument read from a place another argument of the same suspending
/// call lends is copied (`MustCopy`), and the caller releases the copy when the statement ends
/// (High-risk class: a data-integrity pairing, Medium tier: one integration test). Before slice 3 the
/// copy was never released.
const SM_STAGED_ARGUMENT_COPY: &str = "
function use(share first: array<int>, lend rows: array<array<int>>) -> int {
  wait sleep(1)
  rows.add([9])
  return first.count()
}

function entrypoint() -> nothing {
  let rows: array<array<int>> = [[1, 2, 3]]
  let fallback: array<int> = [0]
  let counted = wait use(rows[0].or(fallback), rows)
  print(counted.toString())
}
";

green_program!(
    sm_staged_argument_copy_is_released_when_the_statement_ends,
    SM_STAGED_ARGUMENT_COPY,
    "3\n",
    "slice 3: the copy of an argument staged into a suspending call is released at the statement end"
);
