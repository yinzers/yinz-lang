//! v0.3-M9 C8 slice 4b: errors-capable values in a resume function, and the plain-path
//! `maybe<Union> errors` release the slice found broken (a C7 regression, `b669d67`). Each program
//! frees every allocation, clean under AddressSanitizer, at both optimization levels.

mod support;

use support::{at_both_optimization_levels, build, build_asan, run_asan_binary, run_counted};

/// Build the fixture and run it; it must print `expected`, exit 0, free every heap allocation and
/// every string, and run clean under AddressSanitizer.
fn assert_fixture_balanced(fixture: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let built = build(fixture, no_optimize);
        let (run, counters) = run_counted(&built);
        assert_eq!(
            run.exit_code, 0,
            "{fixture} (no_optimize={no_optimize}) must exit 0; stderr:\n{}",
            run.stderr
        );
        assert_eq!(
            run.stdout, expected,
            "{fixture} (no_optimize={no_optimize})"
        );
        assert_eq!(
            counters.free, counters.alloc,
            "{fixture} (no_optimize={no_optimize}): every heap allocation must be freed: {counters:?}"
        );
        assert_eq!(
            counters.string_free, counters.string_alloc,
            "{fixture} (no_optimize={no_optimize}): every string must be freed: {counters:?}"
        );
        let sanitized = run_asan_binary(&build_asan(&built));
        assert!(
            !sanitized.stderr.contains("AddressSanitizer") && sanitized.exit_code == 0,
            "{fixture} (no_optimize={no_optimize}): AddressSanitizer reported an error:\n{}",
            sanitized.stderr
        );
    });
}

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

/// Build and run `source` the way the fixture helper does: exit 0, `expected` printed, nothing
/// leaked, clean under AddressSanitizer.
fn assert_source_balanced(label: &str, source: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let built = support::build_source(source, label, no_optimize);
        let (run, counters) = run_counted(&built);
        assert_eq!(
            run.exit_code, 0,
            "{label} (no_optimize={no_optimize}) must exit 0; stderr:\n{}",
            run.stderr
        );
        assert_eq!(run.stdout, expected, "{label} (no_optimize={no_optimize})");
        assert_eq!(
            counters.free, counters.alloc,
            "{label} (no_optimize={no_optimize}): every heap allocation must be freed: {counters:?}"
        );
        assert_eq!(
            counters.string_free, counters.string_alloc,
            "{label} (no_optimize={no_optimize}): every string must be freed: {counters:?}"
        );
        let sanitized = run_asan_binary(&build_asan(&built));
        assert!(
            !sanitized.stderr.contains("AddressSanitizer") && sanitized.exit_code == 0,
            "{label} (no_optimize={no_optimize}): AddressSanitizer reported an error:\n{}",
            sanitized.stderr
        );
    });
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
