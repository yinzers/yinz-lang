//! v0.3-M9 C7 slice 2: move sites null the founder's slot, `give` parameters are released by the
//! callee, a reassignment frees the old value, `MustCopy` bindings are owners, and a temporary no
//! binding holds is freed at the end of its statement (`IMP-ownership.md` "Scope-exit release on
//! the plain path").
//!
//! Each case runs at both optimization levels and checks two things at once. The counters say
//! every allocation was freed (the LEAK side: a missing release), and AddressSanitizer with
//! `free_fill_byte=0` says nothing was freed early or twice and the output is the right one (the
//! SAFETY side: a release under a live holder). All are High tier: a wrong release is a
//! use-after-free, a double free or a leak, silent at exit 0.
//!
//! Why this file has more than 8 cases: the exit test of the slice asks for one red-first fixture
//! for each of its ten items, and each item is a different emitter path (a store, a give, a
//! reassignment, a temporary); no case covers another's bug. The table form is one named test per
//! case so the runner parallelizes them and names the failing one.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    build, run_balanced, run_counted,
};

/// What a case checks the counters of.
#[derive(Clone, Copy)]
enum Balance {
    Heap,
    HeapAndStrings,
    Strings,
}

fn check(label: &str, program: &str, expected: &str, balance: Balance) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        match balance {
            Balance::Heap => assert_heap_balanced(label, &counters, no_optimize),
            Balance::HeapAndStrings => {
                assert_heap_balanced(label, &counters, no_optimize);
                assert_strings_balanced(label, &counters, no_optimize);
            }
            Balance::Strings => assert_strings_balanced(label, &counters, no_optimize),
        }
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

/// `case!(name, PROGRAM, "stdout", Balance::X, "the bug it catches (High)")`.
macro_rules! case {
    ($case:ident, $program:expr, $expected:literal, $balance:expr, $bug:literal) => {
        #[doc = $bug]
        #[test]
        fn $case() {
            check(stringify!($case), $program, $expected, $balance);
        }
    };
}

// ── Item 1: a move site nulls the founder's slot at the take ────────────────────────────────

const STORE_ADD: &str = "
function keep(flag: boolean) -> array<array<int>> {
  let rows: array<array<int>> = []
  let item: array<int> = [1, 2]
  if (flag) {
    rows.add(item)
  }
  return rows
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    let kept: array<array<int>> = keep(true)
    let dropped: array<array<int>> = keep(false)
    total = total + kept.count() + dropped.count()
    i = i + 1
  }
  print(total)
}
";

case!(
    a_value_added_on_one_path_is_released_on_the_other,
    STORE_ADD,
    "20\n",
    Balance::Heap,
    "`rows.add(item)` on one path: `item` is released on the path that kept it (leak) or on both (double free) (High)"
);

const STORE_SET: &str = "
function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    let table: map<string, array<int>> = {}
    let item: array<int> = [i, i]
    if (i % 2 == 0) {
      table.set(`k`, item)
    }
    total = total + table.count()
    i = i + 1
  }
  print(total)
}
";

case!(
    a_value_set_into_a_map_on_one_path_is_released_on_the_other,
    STORE_SET,
    "10\n",
    Balance::HeapAndStrings,
    "`table.set(k, item)` on one path: the take does not null `item`'s slot, so the scope frees what the map owns (High)"
);

const STORE_FIELD: &str = "
shape Bucket {
  rows: array<int>
  tag: int
}

function fill(flag: boolean) -> int {
  let bucket: Bucket = { rows: [0], tag: 1 }
  let item: array<int> = [1, 2]
  if (flag) {
    bucket.rows = item
  }
  return bucket.rows.count()
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    total = total + fill(true) + fill(false)
    i = i + 1
  }
  print(total)
}
";

case!(
    a_value_stored_in_a_field_on_one_path_is_released_on_the_other,
    STORE_FIELD,
    "60\n",
    Balance::Heap,
    "`bucket.rows = item` on one path: a field assignment is a take that must null `item` (High)"
);

const STORE_INDEX: &str = "
function fill(flag: boolean) -> int {
  let grid: array<array<int>> = [[0]]
  let item: array<int> = [1, 2]
  if (flag) {
    grid[0] = item
  }
  return grid[0].or([]).count()
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    total = total + fill(true) + fill(false)
    i = i + 1
  }
  print(total)
}
";

case!(
    a_value_stored_by_index_on_one_path_is_released_on_the_other,
    STORE_INDEX,
    "60\n",
    Balance::Heap,
    "`grid[0] = item` on one path: an index assignment is a take that must null `item` (High)"
);

const GIVE_AND_RETURN: &str = "
function eat(give rows: array<int>) -> int {
  return rows.count()
}

function pass(flag: boolean) -> int {
  let rows: array<int> = [1, 2, 3]
  if (flag) {
    return eat(rows)
  }
  return rows.count()
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    total = total + pass(true) + pass(false)
    i = i + 1
  }
  print(total)
}
";

case!(
    a_value_given_on_one_path_is_released_by_its_callee_or_its_caller_never_both,
    GIVE_AND_RETURN,
    "120\n",
    Balance::Heap,
    "`eat(rows)` on one path: the callee does not release a `give` parameter (leak) or the caller releases it too (double free) (High)"
);

const STRING_STORED_ON_ONE_PATH: &str = "
function collect(flag: boolean) -> array<string> {
  let rows: array<string> = []
  let word: string = `row`.toUpperCase()
  if (flag) {
    rows.add(word)
  }
  return rows
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    let kept: array<string> = collect(true)
    let dropped: array<string> = collect(false)
    total = total + kept.count() + dropped.count()
    i = i + 1
  }
  print(total)
}
";

case!(
    a_string_stored_on_one_path_is_released_on_the_other,
    STRING_STORED_ON_ONE_PATH,
    "20\n",
    Balance::HeapAndStrings,
    "a last-use string the store seam takes on one path is not nulled (double free) or never released where it stayed (leak) (High)"
);

const STRING_BORROWED_AT_LAST_USE: &str = "
function size(share text: string) -> int {
  return text.count()
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    let word: string = `row`.toUpperCase()
    total = total + size(word)
    i = i + 1
  }
  print(total)
}
";

case!(
    a_last_use_string_passed_to_a_borrowing_call_is_released,
    STRING_BORROWED_AT_LAST_USE,
    "60\n",
    Balance::Strings,
    "a string at its last use passed to a call that only borrows it is treated as taken and leaks (High)"
);

// ── Item 3: `give` parameters are released by the callee, the spawn ladder stops freeing them ──

const SPAWN_GIVE: &str = "
function worker(give text: string, give rows: array<array<int>>, tag: int) -> nothing {
  print(text.count() + rows.count())
}

function entrypoint() -> nothing {
  let i: int = 0
  while (i < 5) {
    let text: string = `row`.toUpperCase()
    let rows: array<array<int>> = [[1, 2], [3]]
    background worker(text, rows, i)
    i = i + 1
  }
}
";

case!(
    a_spawned_callee_releases_the_give_parameters_the_ladder_no_longer_frees,
    SPAWN_GIVE,
    "5\n5\n5\n5\n5\n",
    Balance::HeapAndStrings,
    "C6's deferral: the spawn ladder flat-frees (or does not free) arguments at `give` positions, leaking items and strings on every spawn, or frees what the callee also releases (High)"
);

// ── Item 4: a reassignment frees the old value ───────────────────────────────────────────────

const REASSIGN_OLD: &str = "
function entrypoint() -> nothing {
  let rows: array<int> = [0]
  let label: string = `a`.toUpperCase()
  let i: int = 0
  while (i < 20) {
    rows = [i, i]
    label = label.toLowerCase()
    i = i + 1
  }
  print(rows.count() + label.count())
}
";

case!(
    a_reassignment_frees_the_old_array_and_string,
    REASSIGN_OLD,
    "3\n",
    Balance::HeapAndStrings,
    "`rows = [..]` and `label = ..` leave the old value unreleased (`Omitted(Reassigned)`): one leak per assignment (High)"
);

const REASSIGN_MOVED_FROM: &str = "
function entrypoint() -> nothing {
  let first: array<int> = [1, 2]
  let second: array<int> = first
  first = [3]
  print(first.count() + second.count())
}
";

case!(
    a_reassignment_after_a_move_does_not_free_the_moved_value,
    REASSIGN_MOVED_FROM,
    "3\n",
    Balance::Heap,
    "`let second = first; first = [3]`: the assignment frees the value `second` now owns (double free) (High)"
);

const REASSIGN_SHAPE: &str = "
shape Tag {
  name: string
  size: int
}

function makeTag(label: string, size: int) -> Tag {
  let made: Tag = { name: label.toUpperCase(), size: size }
  return made
}

function entrypoint() -> nothing {
  let tag: Tag = makeTag(`a`, 0)
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    tag = makeTag(`tag`, i)
    total = total + tag.name.count() + tag.size
    i = i + 1
  }
  print(total)
}
";

case!(
    a_reassigned_flat_shape_frees_the_old_fields_before_its_region_is_reused,
    REASSIGN_SHAPE,
    "250\n",
    Balance::Strings,
    "a flat shape reassigned in a loop reuses one region: releasing the old fields after the copy frees the new ones (High)"
);

// ── Item 5 and 9: a `MustCopy` binding owns its copy ────────────────────────────────────────

const MUST_COPY_LET: &str = "
shape Bucket {
  rows: array<int>
  id: int
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    let bucket: Bucket = { rows: [10, 20, 30], id: 1 }
    let rows = bucket.rows
    bucket.rows = [9]
    total = total + rows.count()
    i = i + 1
  }
  print(total)
}
";

case!(
    a_let_copy_of_a_field_that_is_overwritten_is_released_once,
    MUST_COPY_LET,
    "60\n",
    Balance::Heap,
    "a `MustCopy` binding keeps `Origin::Reaches`, so nothing releases its copy: one copy leaked per run of its scope (High)"
);

const SHAPE_FROM_PIECE_VIEW: &str = "
shape Tag {
  name: string
  size: int
}

function makeTag(label: string) -> Tag {
  let made: Tag = { name: label.toUpperCase(), size: 1 }
  return made
}

function entrypoint() -> nothing {
  let tags: array<Tag> = [makeTag(`a`), makeTag(`b`)]
  let i: int = 0
  let total: int = 0
  while (i < 10) {
    let fallback: Tag = makeTag(`z`)
    let picked: Tag = tags[i % 2].or(fallback)
    total = total + picked.name.count()
    i = i + 1
  }
  print(total)
}
";

case!(
    a_shape_bound_from_a_piece_shares_the_owners_strings_and_is_not_released,
    SHAPE_FROM_PIECE_VIEW,
    "10\n",
    Balance::Strings,
    "`let picked = tags[i].or(fallback)` shares the owner's strings; releasing `picked` too frees them twice (High)"
);

const SHAPE_FROM_PIECE_COPY: &str = "
shape Tag {
  name: string
  size: int
}

function makeTag(label: string) -> Tag {
  let made: Tag = { name: label.toUpperCase(), size: 1 }
  return made
}

function entrypoint() -> nothing {
  let tags: array<Tag> = [makeTag(`a`), makeTag(`b`)]
  let i: int = 0
  let total: int = 0
  while (i < 10) {
    let picked: Tag = tags[0].or(makeTag(`z`))
    tags[0] = makeTag(`c`)
    total = total + picked.name.count()
    i = i + 1
  }
  print(total)
}
";

case!(
    a_shape_copied_from_a_piece_that_is_overwritten_is_an_owner_and_released,
    SHAPE_FROM_PIECE_COPY,
    "10\n",
    Balance::Strings,
    "a `MustCopy` shape binding owns deep copies of the owner's strings and leaks them when it is not released (High)"
);

// ── Item 6: a temporary no binding holds is freed at the end of its statement ───────────────

const TEMP_FRESH: &str = "
function makeRows() -> array<int> {
  let rows: array<int> = [1, 2, 3]
  return rows
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    total = total + makeRows().count()
    let note: string = `n=${i.toString()}`
    total = total + note.count()
    i = i + 1
  }
  print(total)
}
";

case!(
    an_unbound_fresh_result_and_an_interpolated_temporary_are_freed_at_statement_end,
    TEMP_FRESH,
    "130\n",
    Balance::HeapAndStrings,
    "`makeRows().count()` and `i.toString()` in an interpolation: the temporary is never freed (High)"
);

const TEMP_PIECE: &str = "
shape Bucket {
  rows: array<int>
  id: int
}

function makeBucket() -> Bucket {
  let bucket: Bucket = { rows: [9, 8], id: 1 }
  return bucket
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    total = total + makeBucket().rows.count()
    i = i + 1
  }
  print(total)
}
";

case!(
    a_piece_read_of_a_temporary_frees_the_whole_temporary_at_statement_end,
    TEMP_PIECE,
    "40\n",
    Balance::Heap,
    "`makeBucket().rows.count()`: the piece is read, the temporary behind it is never freed (High)"
);

const TEMP_LOOP_CELL: &str = "
function makeRows() -> array<array<int>> {
  let rows: array<array<int>> = [[1], [2, 3]]
  return rows
}

function entrypoint() -> nothing {
  let total: int = 0
  let i: int = 0
  while (i < 10) {
    for (row in makeRows()) {
      total = total + row.count()
    }
    i = i + 1
  }
  print(total)
}
";

case!(
    a_loop_over_a_temporary_frees_it_after_the_loop,
    TEMP_LOOP_CELL,
    "30\n",
    Balance::Heap,
    "`for (row in makeRows())`: the iterated temporary is freed before the loop ends (use-after-free) or never (High)"
);

#[test]
fn an_imported_calls_unbound_result_is_freed_at_statement_end() {
    // Bug: `freshRows(i).count()` holds an untraceable (`NoOwner`) result that nothing frees (High).
    at_both_optimization_levels(|no_optimize| {
        let fixture = "v0_3_m9_c7_imported_temp";
        let built = build(fixture, no_optimize);
        let (run, counters) = run_counted(&built);
        assert_eq!(run.exit_code, 0, "stderr:\n{}", run.stderr);
        assert_eq!(run.stdout, "40\n");
        assert_heap_balanced(fixture, &counters, no_optimize);
        support::assert_asan_clean_and_prints(fixture, no_optimize, "40\n");
    });
}

const TAKE_PIECE: &str = "
shape Bucket {
  rows: array<int>
  id: int
}

function makeBucket() -> Bucket {
  let bucket: Bucket = { rows: [9, 8], id: 1 }
  return bucket
}

function entrypoint() -> nothing {
  let rows: array<int> = makeBucket().rows
  print(rows.count())
}
";

#[test]
fn binding_a_piece_of_a_fresh_temporary_takes_it_without_a_copy() {
    // Bug (cost, Golden Rule 8): `let rows = makeBucket().rows` copies the field, where Rust moves
    // it out of the temporary and drops the rest. The one array (header + buffer: 2 allocations)
    // is allocated once, taken by `rows` and freed once (High: a wrong take is a double free).
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("take_piece", TAKE_PIECE, "2\n", no_optimize);
        assert_eq!(
            (counters.alloc, counters.free),
            (2, 2),
            "no_optimize={no_optimize}: {counters:?}"
        );
        assert_asan_clean("take_piece", TAKE_PIECE, "2\n", no_optimize);
    });
}

// ── Item 7: an early return inside a loop releases the loop variable's copy ─────────────────

const LOOP_EARLY_RETURN: &str = "
function first(rows: array<array<int>>) -> int {
  for (row in rows) {
    rows[0] = [9]
    return row.count()
  }
  return 0
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    let rows: array<array<int>> = [[1], [2]]
    total = total + first(rows)
    i = i + 1
  }
  print(total)
}
";

case!(
    a_return_inside_a_loop_releases_the_loop_variables_copy,
    LOOP_EARLY_RETURN,
    "20\n",
    Balance::Heap,
    "`return` in a loop body whose copy of the loop variable is released only at the iteration end: one copy leaked per early return (High)"
);

// ── Item 8: provenance must not read a builtin as a user function ───────────────────────────

const BUILTIN_OR_SHADOWED: &str = "
function or(share first: array<int>, share second: array<int>) -> array<int> {
  return first.copy()
}

function entrypoint() -> nothing {
  let rows: array<array<int>> = [[1, 2]]
  let found = rows.get(0)
  let backup: array<int> = [7]
  let picked = found.or(backup)
  let again = or(backup, backup)
  print(picked.count())
  print(again.count())
}
";

case!(
    a_builtin_or_is_not_classified_by_a_user_function_of_the_same_name,
    BUILTIN_OR_SHADOWED,
    "2\n1\n",
    Balance::Heap,
    "`found.or(backup)` read as a call of the user's `or` is `Fresh`: `picked` becomes a second owner of `backup` and frees it twice (High)"
);

// ── Item 2: a class with no static founder gets a run-time drop flag ────────────────────────

const NO_STATIC_FOUNDER: &str = "
shape Bucket {
  rows: array<int>
}

function make() -> Bucket {
  let bucket: Bucket = { rows: [9] }
  return bucket
}

function pick(condition: boolean) -> int {
  let cell: array<int> = make().rows
  if (condition) {
    let owner: Bucket = make()
    cell = owner.rows
  }
  return cell.count()
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    total = total + pick(true) + pick(false)
    i = i + 1
  }
  print(total)
}
";

case!(
    a_class_with_no_static_founder_is_released_by_a_run_time_flag,
    NO_STATIC_FOUNDER,
    "40\n",
    Balance::Heap,
    "`cell = owner.rows` where `cell` outlives `owner`: releasing `owner` at its block end leaves `cell` dangling, releasing nothing leaks both (High)"
);
