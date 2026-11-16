//! v0.3-M9 C7 slice 1: the plain-function scope-exit release (`Cg::emit_release_edge`).
//!
//! Counters assert what the emitter RELEASES (every allocation freed); AddressSanitizer asserts
//! what it must NOT release (a value some other holder still owns). Both optimization levels run
//! at once per test (`at_both_optimization_levels`), each in its own process.
//!
//! The 2,000-iteration leak fixtures and the C2 exit-class fixtures live in
//! `v03_m9_release_safety`; this file pins the owner kinds, the three exit edges and the cases
//! slice 1 must leave unreleased until slice 2's move sites null a slot.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    run_balanced,
};

const BLOCK_ENDS: &str = "
function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 100) {
    let per: array<int> = [i]
    if (i > 50) {
      let extra: array<int> = [i, i]
      total = total + extra.count()
    }
    total = total + per.count()
    i = i + 1
  }
  print(total)
}
";

/// A `while` body's and an `if` body's arrays leak at the block end, or the `if` end frees the
/// enclosing loop's array (High).
#[test]
fn block_ends_release_the_array_of_each_block_once() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_block_ends", BLOCK_ENDS, "198\n", no_optimize);
        assert_heap_balanced("c7_block_ends", &counters, no_optimize);
    });
}

const FOR_LOOPS: &str = "
function entrypoint() -> nothing {
  let rows: array<int> = [1, 2, 3]
  let total: int = 0
  for (row in rows) {
    let doubled: array<int> = [row, row]
    total = total + doubled.count()
  }
  for (index in range(3)) {
    let pair: array<int> = [index, index]
    total = total + pair.count()
  }
  let names: array<string> = [`a`, `b`]
  for (name in names) {
    let shout: string = name.toUpperCase()
    total = total + shout.count()
  }
  print(total)
}
";

/// A `for` body's locals (array, range and string iteration alike) leak per iteration, or are
/// released at the loop's end instead of each iteration's (High).
#[test]
fn for_loop_bodies_release_their_locals_per_iteration() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_for_loops", FOR_LOOPS, "14\n", no_optimize);
        assert_heap_balanced("c7_for_loops", &counters, no_optimize);
    });
}

const MATCH_ARMS: &str = "
function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 30) {
    let pick: int = i % 3
    if (pick) {
      0 => {
        let zero: array<int> = [i]
        total = total + zero.count()
      }
      1 => {
        let one: array<int> = [i, i]
        total = total + one.count()
      }
      else => {
        let other: array<int> = [i, i, i]
        total = total + other.count()
      }
    }
    i = i + 1
  }
  print(total)
}
";

/// A `match` arm's and an `else` arm's local leaks at the arm end (High).
#[test]
fn match_arms_release_their_locals() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_match_arms", MATCH_ARMS, "60\n", no_optimize);
        assert_heap_balanced("c7_match_arms", &counters, no_optimize);
    });
}

const MAP_LOCAL: &str = "
function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 50) {
    let scores: map<string, int> = { `alice`: i, `bob`: 2 }
    total = total + scores[`alice`].or(0) + scores.count()
    i = i + 1
  }
  print(total)
}
";

/// A `map` local (buffers, and the keys the map owns) leaks per iteration (High).
#[test]
fn a_map_local_is_released_with_its_keys() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_map_local", MAP_LOCAL, "1325\n", no_optimize);
        assert_heap_balanced("c7_map_local", &counters, no_optimize);
        assert_strings_balanced("c7_map_local", &counters, no_optimize);
    });
}

const SHAPE_LOCAL: &str = "
shape Tag {
  name: string
  size: int
}

function entrypoint() -> nothing {
  let stem: string = `tag`
  let i: int = 0
  let total: int = 0
  while (i < 50) {
    let tag: Tag = { name: stem.toUpperCase(), size: i }
    total = total + tag.name.count() + tag.size
    i = i + 1
  }
  print(total)
}
";

/// A flat shape local's string field leaks per iteration, or the release frees the shape's
/// entry-block region as if it were a heap cell (High).
#[test]
fn a_flat_shape_local_releases_its_fields_not_its_region() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_shape_local", SHAPE_LOCAL, "1375\n", no_optimize);
        assert_strings_balanced("c7_shape_local", &counters, no_optimize);
    });
}

const EARLY_RETURN: &str = "
function pick(stopAt: int) -> int {
  let anchor: array<int> = [1, 2, 3]
  let i: int = 0
  while (i < 3) {
    let per: array<int> = [i, i]
    if (i == stopAt) {
      let marker: array<int> = [7]
      return per.count() + anchor.count() + marker.count() - 1
    }
    i = i + 1
  }
  return anchor.count()
}

function entrypoint() -> nothing {
  let round: int = 0
  let total: int = 0
  while (round < 40) {
    total = total + pick(round % 5)
    round = round + 1
  }
  print(total)
}
";

/// An early `return` releases only the innermost frame, releases an outer frame twice (once at
/// the return and once at its own end), or releases before the return value is built (High).
#[test]
fn an_early_return_releases_every_open_frame_exactly_once() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_early_return", EARLY_RETURN, "168\n", no_optimize);
        assert_heap_balanced("c7_early_return", &counters, no_optimize);
        assert_asan_clean("c7_early_return", EARLY_RETURN, "168\n", no_optimize);
    });
}

const MAIN_RETURN: &str = "
function entrypoint() -> nothing {
  let outer: array<int> = [1, 2]
  let words: string = `done`.toUpperCase()
  if (outer.count() == 2) {
    let inner: array<int> = [3]
    print(words)
    print(inner.count())
    return
  }
  print(`unreachable`)
}
";

/// `main`'s `return` leaks what is open, or releases it after the runtime has shut down (High).
#[test]
fn main_releases_every_open_binding_at_its_return() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_main_return", MAIN_RETURN, "DONE\n1\n", no_optimize);
        assert_heap_balanced("c7_main_return", &counters, no_optimize);
        assert_strings_balanced("c7_main_return", &counters, no_optimize);
    });
}

const GENERIC_BODY: &str = "
function describe<T>(value: T) -> int {
  let scratch: array<int> = [1, 2, 3]
  return scratch.count()
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    total = total + describe(i)
    i = i + 1
  }
  print(total)
}
";

/// A generic function's instance leaks its concrete-typed local (the plan is made once, for the
/// body with its type parameter unresolved) (High).
#[test]
fn a_generic_instance_releases_its_concrete_locals() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_generic", GENERIC_BODY, "60\n", no_optimize);
        assert_heap_balanced("c7_generic", &counters, no_optimize);
    });
}

const ERRORS_PLAIN: &str = "
function inner() -> int errors {
  return 4
}

function outer() -> int errors {
  let scratch: array<int> = [1, 2]
  let value = inner()
  return value + scratch.count()
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    total = total + outer().or(0)
    i = i + 1
  }
  print(total)
}
";

/// An errors-capable plain function leaks its locals at the success `return` or at the
/// propagation edge, or the edge breaks the function's IR (High).
#[test]
fn an_errors_capable_plain_function_releases_its_locals() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_errors", ERRORS_PLAIN, "120\n", no_optimize);
        assert_heap_balanced("c7_errors", &counters, no_optimize);
    });
}

const STRING_TAKEN_BY_STORE: &str = "
function collect() -> array<string> {
  let rows: array<string> = []
  let i: int = 0
  while (i < 20) {
    let row: string = `row`.toUpperCase()
    rows.add(row)
    i = i + 1
  }
  return rows
}

function entrypoint() -> nothing {
  let rows: array<string> = collect()
  print(rows.count().toString())
  print(rows[3].or(`none`))
}
";

/// A string a store took (`rows.add(row)`, the last use of `row`) is freed at the scope end while
/// the array still holds it: a double free or use-after-free (High).
#[test]
fn a_string_a_store_took_is_not_released_by_its_old_binding() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean(
            "c7_string_taken_by_store",
            STRING_TAKEN_BY_STORE,
            "20\nROW\n",
            no_optimize,
        );
    });
}

const STRING_TAKEN_ON_SOME_PATHS: &str = "
function keep(flag: boolean) -> array<string> {
  let rows: array<string> = []
  let row: string = `kept`.toUpperCase()
  if (flag) {
    rows.add(row)
  }
  return rows
}

function entrypoint() -> nothing {
  let a: array<string> = keep(true)
  let b: array<string> = keep(false)
  print(a.count().toString())
  print(b.count().toString())
  print(a[0].or(`none`))
}
";

/// A string stored on one path only is released on the path that stored it (double free), the
/// slot null check cannot tell without a move site's nulling (High).
#[test]
fn a_string_taken_on_some_paths_is_never_released_under_its_new_holder() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean(
            "c7_string_some_paths",
            STRING_TAKEN_ON_SOME_PATHS,
            "1\n0\nKEPT\n",
            no_optimize,
        );
    });
}

const STRING_GIVEN: &str = "
function consume(give text: string) -> int {
  return text.count()
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    let text: string = `item`.toUpperCase()
    total = total + consume(text)
    i = i + 1
  }
  print(total)
}
";

/// A string given to a `give` parameter is freed by the caller's scope end and by the callee's
/// (High).
#[test]
fn a_given_string_is_released_by_at_most_one_side() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean("c7_string_given", STRING_GIVEN, "80\n", no_optimize);
    });
}

const STRING_RETURNED: &str = "
function label(index: int) -> string {
  let text: string = `label`.toUpperCase()
  return text
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  while (i < 20) {
    let got: string = label(i)
    total = total + got.count()
    i = i + 1
  }
  print(total)
}
";

/// A returned local string is released by the return edge under the caller (High).
#[test]
fn a_returned_local_string_is_not_released_at_the_return() {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean("c7_string_returned", STRING_RETURNED, "100\n", no_optimize);
        let counters = run_balanced("c7_string_returned", STRING_RETURNED, "100\n", no_optimize);
        assert_strings_balanced("c7_string_returned", &counters, no_optimize);
    });
}

const GENERIC_CALL_RESULT_AND_ARGUMENT_TEMPORARY: &str = "
function ident<T>(share value: T) -> T {
  return value
}
function entrypoint() -> nothing {
  let total: int = 0
  for (round in range(0, 10)) {
    let kept: array<int> = [round]
    let bound: array<int> = ident(kept)
    let from_literal: array<int> = ident([round, 1])
    total = total + bound.count() + from_literal.count()
    total = total + ident([round, 1, 2]).count()
  }
  print(total)
}
";

/// A call of a generic function whose result is a heap type: its `return` copies, so the call is
/// an owned value (bound by move, or a temporary released at the statement end), and an array
/// literal passed to its `share` parameter is a temporary the caller releases. Typeck judged the
/// result by the unresolved `T` (a `MayAlias`: the `let` copied it again and nothing released
/// the result), and a generic callee was absent from the signature table, so its argument
/// was assumed taken. Both leaked 1 to 3 arrays per call (High: leak).
#[test]
fn a_generic_call_result_and_its_argument_temporary_are_released() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(
            "c7_generic_call",
            GENERIC_CALL_RESULT_AND_ARGUMENT_TEMPORARY,
            "60\n",
            no_optimize,
        );
        assert_heap_balanced("c7_generic_call", &counters, no_optimize);
        assert_asan_clean(
            "c7_generic_call",
            GENERIC_CALL_RESULT_AND_ARGUMENT_TEMPORARY,
            "60\n",
            no_optimize,
        );
    });
}
