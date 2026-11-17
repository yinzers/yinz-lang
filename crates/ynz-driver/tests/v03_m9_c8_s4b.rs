//! v0.3-M9 C8 slice 4b: errors-capable values in a resume function, and the plain-path
//! `maybe<Union> errors` release the slice found broken (a C7 regression, `b669d67`). Each program
//! frees every allocation, clean under AddressSanitizer, at both optimization levels.

mod support;

use support::{
    assert_built_asan_clean, assert_fixture_balanced_and_asan_clean as assert_fixture_balanced,
    assert_source_balanced_and_asan_clean as assert_source_balanced, at_both_optimization_levels,
    build, run_counted,
};

#[test]
fn a_plain_maybe_union_errors_hit_releases_its_view() {
    assert_fixture_balanced(
        "v0_3_m9_c8_s4b_maybe_union_errors_plain_hit.ynz",
        "big kept\nend\n",
    );
}

#[test]
fn a_plain_maybe_union_errors_miss_releases_nothing_it_does_not_own() {
    assert_fixture_balanced(
        "v0_3_m9_c8_s4b_maybe_union_errors_plain_miss.ynz",
        "none\nend\n",
    );
}

#[test]
fn a_plain_errors_local_of_every_inner_type_is_released_with_its_own_form() {
    assert_fixture_balanced(
        "v0_3_m9_c8_s4b_errors_inner_types_plain.ynz",
        &"false\n".repeat(9),
    );
}

const SUSPENDING_MAYBE_CALLEES: &str = "
shape Big {
  label: string
  count: int
}

function pickShape(index: int) -> maybe<Big> {
  wait sleep(1)
  let items: array<Big> = [{ label: `a`, count: 1 }]
  let found = items.get(index)
  return found
}

function pickNumber(index: int) -> maybe<number> {
  wait sleep(1)
  let seed = 5.25
  let fees: fixed<number> = [seed, seed]
  let found = fees.get(index)
  return found
}
";

#[test]
fn a_maybe_shape_a_suspending_callee_returns_is_released_when_bound_without_crossing() {
    let source = format!(
        "{SUSPENDING_MAYBE_CALLEES}
function entrypoint() -> nothing {{
  let picked = pickShape(0)
  print(picked.exists())
}}
"
    );
    assert_source_balanced("maybe_shape_return", &source, "true\n");
}

#[test]
fn a_maybe_number_a_suspending_callee_returns_is_released_when_bound_without_crossing() {
    let source = format!(
        "{SUSPENDING_MAYBE_CALLEES}
function entrypoint() -> nothing {{
  let picked = pickNumber(0)
  print(picked.exists())
}}
"
    );
    assert_source_balanced("maybe_number_return", &source, "true\n");
}

#[test]
fn a_maybe_shape_a_suspending_callee_returns_is_released_when_its_local_crosses_a_wait() {
    let source = format!(
        "{SUSPENDING_MAYBE_CALLEES}
function entrypoint() -> nothing {{
  let picked = pickShape(0)
  print(picked.exists())
  wait sleep(1)
  print(`after`)
}}
"
    );
    assert_source_balanced("maybe_shape_return_crossing", &source, "true\nafter\n");
}

/// Callees every errors-path test below shares: a suspending `string errors` that succeeds or
/// fails on request (`__testFallibleAsync` fails when given `true`).
const FALLIBLE_CALLEES: &str = "
function fetchName(fail: boolean) -> string errors {
  let status = wait __testFallibleAsync(fail)
  return `name${status}`
}
";

#[test]
fn an_auto_propagation_in_a_resume_function_releases_the_open_locals() {
    for (fail, expected) in [("false", "ok\n"), ("true", "failed\n")] {
        let source = format!(
            "{FALLIBLE_CALLEES}
function work(fail: boolean) -> int errors {{
  let rows: array<int> = [1, 2]
  let label = `ab`.toUpperCase()
  wait sleep(1)
  let name = fetchName(fail)
  let shown = name.toUpperCase()
  return rows.count() + shown.count()
}}

function entrypoint() -> nothing {{
  let result = work({fail})
  if (result.failed()) {{
    print(`failed`)
  }}
  if (result.failed() == false) {{
    print(`ok`)
  }}
}}
"
        );
        assert_source_balanced(&format!("propagation_{fail}"), &source, expected);
    }
}

#[test]
fn a_narrowed_re_read_of_an_errors_local_in_a_resume_function_releases_what_it_owns() {
    for (fail, expected) in [("false", "NAME42\nNAME42\n"), ("true", "failed\n")] {
        let source = format!(
            "{FALLIBLE_CALLEES}
function entrypoint() -> nothing {{
  let tag = `t`.toUpperCase()
  let name = fetchName({fail})
  wait sleep(1)
  if (name.failed()) {{
    print(`failed`)
    return
  }}
  print(name.toUpperCase())
  print(name.toUpperCase())
}}
"
        );
        assert_source_balanced(&format!("narrowed_{fail}"), &source, expected);
    }
}

#[test]
fn a_crossing_errors_local_whose_string_is_taken_is_released_once() {
    let source = format!(
        "{FALLIBLE_CALLEES}
function entrypoint() -> nothing {{
  let name = fetchName(false)
  wait sleep(1)
  let kept = name.or(`fallback`)
  wait sleep(1)
  print(kept)
}}
"
    );
    assert_source_balanced("crossing_taken", &source, "name42\n");
}

#[test]
fn a_reassigned_errors_local_in_a_resume_function_releases_its_old_value() {
    let source = format!(
        "{FALLIBLE_CALLEES}
function plainName() -> string errors {{
  return `plain`.toUpperCase()
}}

function entrypoint() -> nothing {{
  let name = fetchName(false)
  name = plainName()
  wait sleep(1)
  print(name.or(`fallback`))
}}
"
    );
    assert_source_balanced("reassigned", &source, "PLAIN\n");
}

#[test]
fn an_errors_local_reassigned_in_a_loop_across_waits_releases_each_old_value() {
    let source = format!(
        "{FALLIBLE_CALLEES}
function plainName() -> string errors {{
  return `plain`.toUpperCase()
}}

function entrypoint() -> nothing {{
  let name = fetchName(false)
  let pass = 0
  while (pass < 3) {{
    name = plainName()
    wait sleep(1)
    pass = pass + 1
  }}
  print(name.or(`fallback`))
}}
"
    );
    assert_source_balanced("reassigned_in_loop", &source, "PLAIN\n");
}

#[test]
fn a_given_maybe_call_result_is_staged_in_the_frame_for_a_suspending_callee() {
    let source = "
function makeNote() -> maybe<string> {
  let items: array<string> = [`note`.toUpperCase()]
  let found = items.get(0)
  return found
}

function consume(give note: maybe<string>) -> int {
  wait sleep(1)
  let text = note.or(`none`)
  return text.count()
}

function entrypoint() -> nothing {
  let size: int = wait consume(makeNote())
  print(size)
}
";
    assert_source_balanced("given_maybe_call_result", source, "4\n");
}

/// A fixture that must compile, run, print `expected` and be clean under AddressSanitizer at both
/// optimization levels. It may leak: its leak is a known plain-path one (`KNOWN_PLAIN_LEAKS`).
fn assert_fixture_runs_clean(fixture: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let built = build(fixture, no_optimize);
        let (run, _counters) = run_counted(&built);
        assert_eq!(
            run.exit_code, 0,
            "{fixture} (no_optimize={no_optimize}) must exit 0; stderr:\n{}",
            run.stderr
        );
        assert_eq!(
            run.stdout, expected,
            "{fixture} (no_optimize={no_optimize})"
        );
        assert_built_asan_clean(fixture, &built, no_optimize);
    });
}

/// A `fixed<T>` in a signature is unsized, so the release plan omits it (`UnsizedFixed`) and its
/// items leak: the programs must still compile (a C7 regression made `plain_release_of` refuse
/// them) and never free wrongly.
#[test]
fn an_unsized_fixed_string_returned_by_a_function_compiles_and_runs_clean() {
    assert_fixture_runs_clean("v0_3_m9_c8_s4b_unsized_fixed_string_return.ynz", "built\n");
}

#[test]
fn an_unsized_fixed_string_given_to_a_function_compiles_and_runs_clean() {
    assert_fixture_runs_clean("v0_3_m9_c8_s4b_unsized_fixed_string_param.ynz", "1\n");
}

#[test]
fn an_unsized_fixed_of_shapes_returned_by_a_function_compiles_and_runs_clean() {
    assert_fixture_runs_clean("v0_3_m9_c8_s4b_unsized_fixed_shape_return.ynz", "built\n");
}

#[test]
fn an_unsized_fixed_of_shapes_given_to_a_function_compiles_and_runs_clean() {
    assert_fixture_runs_clean("v0_3_m9_c8_s4b_unsized_fixed_shape_param.ynz", "1\n");
}

#[test]
fn a_maybe_of_a_union_temporary_of_a_suspending_statement_is_released() {
    let source = "
shape Tiny {
  flag: boolean
}
shape Big {
  label: string
  count: int
}
shape Thing = Tiny | Big

function find(share things: array<Thing>, index: int) -> maybe<Thing> {
  let found = things.get(index)
  return found
}

function later(present: boolean) -> int {
  wait sleep(1)
  if (present) {
    return 1
  }
  return 0
}

function entrypoint() -> nothing {
  let things: array<Thing> = []
  let big: Big = { label: `kept`, count: 1 }
  things.add(big)
  let hit: int = wait later(find(things, 0).exists())
  let miss: int = wait later(find(things, 5).exists())
  print(hit)
  print(miss)
}
";
    assert_source_balanced("maybe_union_temporary", source, "1\n0\n");
}

#[test]
fn a_maybe_a_handle_delivers_is_not_freed_by_its_binding_because_the_handle_owns_it() {
    // `h.receive()` of a `maybe<int>` is a pointer into the handle's own 16-byte buffer: releasing
    // it as a heap cell was a double free (ASan sweep, before the borrowed ok word was recorded).
    assert_fixture_balanced("v0_3_m8_p4_handle_maybe_return.ynz", "7\ndone\n");
}

#[test]
fn a_narrowed_read_of_an_errors_local_that_crosses_only_through_its_release_still_builds() {
    // Bug: the narrowed read re-pointed `locals` at an alloca made in the read's own branch, and
    // the reload after a later `wait` wrote into it: LLVM "Instruction does not dominate all
    // uses" (examples/pirates-roster, found by `examples_basics_runs_end_to_end`).
    let source = "
function battingAverage(hits: int, atBats: int) -> int errors {
  return (hits * 1000) / atBats
}

function entrypoint() -> nothing {
  const average = battingAverage(50, 200)
  if (average.failed()) {
    print(`failed`)
  }
  print(average.toString())
  wait sleep(1)
  print(`end`)
}
";
    assert_source_balanced("narrowed_read_release_only", source, "250\nend\n");
}

/// Bug caught: pre-existing; errors return packed a decimal128 into a bignum ok word (wrong value,
/// invalid free on release); tier Medium. An `errors` function declared `number<40>` returns a
/// literal or a `number<34>`-or-narrower expression. The `return` must convert the value to the
/// declared success type before it packs the ok word. Plain and suspending function alike.
#[test]
fn a_number_40_errors_return_of_a_narrower_value_is_converted_to_the_declared_type() {
    for (suspending, wait_line) in [(false, ""), (true, "wait sleep(1)")] {
        let source = format!(
            "function literal() -> number<40> errors {{
  {wait_line}
  return 2.5
}}

function expression(n: int) -> number<40> errors {{
  {wait_line}
  let small: number<20> = 1.25
  return small + small
}}

function entrypoint() -> nothing {{
  let fallback: number<40> = 0.5
  let first = literal()
  print(first.or(fallback))
  let second = expression(1)
  print(second.or(fallback))
}}
"
        );
        assert_source_balanced(
            &format!("number_40_errors_return_{suspending}"),
            &source,
            "2.5\n2.50\n",
        );
    }
}
