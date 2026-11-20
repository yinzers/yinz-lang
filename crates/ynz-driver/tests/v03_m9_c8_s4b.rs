//! v0.3-M9 C8 slice 4b: errors-capable values in a resume function, and the plain-path
//! `maybe<Union> errors` release the slice found broken (a C7 regression, `b669d67`). Each program
//! frees every allocation, clean under AddressSanitizer, at both optimization levels.

mod support;

use support::{
    assert_fixture_balanced_and_asan_clean as assert_fixture_balanced,
    assert_source_balanced_and_asan_clean as assert_source_balanced,
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
  label: String
  count: Int
}

function pickShape(index: Int) -> Maybe<Big> {
  wait sleep(1)
  let items: Array<Big> = [{ label: `a`, count: 1 }]
  let found = items.get(index)
  return found
}

function pickNumber(index: Int) -> Maybe<Number> {
  wait sleep(1)
  let seed = 5.25
  let fees: Fixed<Number> = [seed, seed]
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
function fetchName(fail: Boolean) -> String errors {
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
function makeNote() -> Maybe<String> {
  let items: Array<String> = [`note`.toUpperCase()]
  let found = items.get(0)
  return found
}

function consume(give note: Maybe<String>) -> Int {
  wait sleep(1)
  let text = note.or(`none`)
  return text.count()
}

function entrypoint() -> Nothing {
  let size: Int = wait consume(makeNote())
  print(size)
}
";
    assert_source_balanced("given_maybe_call_result", source, "4\n");
}

#[test]
fn a_maybe_of_a_union_temporary_of_a_suspending_statement_is_released() {
    let source = "
shape Tiny {
  flag: Boolean
}
shape Big {
  label: String
  count: Int
}
shape Thing = Tiny | Big

function find(share things: Array<Thing>, index: Int) -> Maybe<Thing> {
  let found = things.get(index)
  return found
}

function later(present: Boolean) -> Int {
  wait sleep(1)
  if (present) {
    return 1
  }
  return 0
}

function entrypoint() -> Nothing {
  let things: Array<Thing> = []
  let big: Big = { label: `kept`, count: 1 }
  things.add(big)
  let hit: Int = wait later(find(things, 0).exists())
  let miss: Int = wait later(find(things, 5).exists())
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
function battingAverage(hits: Int, atBats: Int) -> Int errors {
  return (hits * 1000) / atBats
}

function entrypoint() -> Nothing {
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

/// Bug caught: pre-existing, found with item 1; the crash appears only on the plan branch (C7's
/// release makes it an invalid free): a plain `-> number<40>` returned a narrower value (a
/// `number<20>` local, a `number<20>` sum) as the decimal128 cell it was, which the caller then read
/// and freed as bignum text (wrong value, `free(): invalid pointer`). Tier Medium. The function is
/// plain; its caller is plain or suspending. (A bare suspending `-> number<40>` does not compile
/// today, "SM wrapper-return has no arm", a separate bug.)
#[test]
fn a_plain_number_40_return_of_a_narrower_value_is_converted_to_the_declared_type() {
    for (suspending, wait_line) in [(false, ""), (true, "wait sleep(1)")] {
        let source = format!(
            "function narrowed() -> number<40> {{
  let small: number<20> = 1.25
  return small
}}

function summed() -> number<40> {{
  let small: number<20> = 1.25
  return small + small
}}

function entrypoint() -> nothing {{
  {wait_line}
  let first = narrowed()
  print(first)
  let second = summed()
  print(second)
}}
"
        );
        assert_source_balanced(
            &format!("plain_number_40_return_{suspending}"),
            &source,
            "1.25\n2.50\n",
        );
    }
}
