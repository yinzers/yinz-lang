//! v0.3-M9 C8 slice 4a: a resume function releases the inline-aggregate kinds (flat shape, nested
//! shape, `maybe`, `fixed` list, envelope union): as a local whose release follows a `wait`, and
//! as a temporary of a statement that suspends. Each program leaks or crashes before the slice and
//! now frees every allocation, at both optimization levels, each in its own process.
//!
//! A local here is read only BEFORE the `wait`: it crosses the suspension through its own release
//! (`CrossingNames::release_only`), so the guards that reject a read after a `wait` do not apply.

mod support;

use support::{
    at_both_optimization_levels, build_asan, build_source, run_asan_binary, run_counted,
};

/// Build and run `source` at both optimization levels; it must print `expected`, exit 0 and free
/// every heap allocation and every string it allocates.
fn assert_balanced(label: &str, source: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let built = build_source(source, label, no_optimize);
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

const CREW: &str = "
shape Crew {
  name: string
  size: int
}

function later(value: int) -> int {
  wait sleep(1)
  return value
}
";

#[test]
fn shape_local_crossing_a_wait_is_released() {
    let source = format!(
        "{CREW}
function entrypoint() -> nothing {{
  let crew: Crew = {{ name: `ab`.toUpperCase(), size: 3 }}
  print(crew.size)
  wait sleep(1)
  print(`done`)
}}
"
    );
    assert_balanced("shape_local", &source, "3\ndone\n");
}

#[test]
#[ignore = "planned RED (C8 S4a step 3): a temporary of a suspending statement has no frame region yet"]
fn shape_temporary_of_a_suspending_statement_is_released() {
    let source = format!(
        "{CREW}
function makeCrew() -> Crew {{
  return {{ name: `ab`.toUpperCase(), size: 3 }}
}}

function entrypoint() -> nothing {{
  let size: int = wait later(makeCrew().size)
  print(size)
}}
"
    );
    assert_balanced("shape_temporary", &source, "3\n");
}

const PIRATE: &str = "
shape Badge {
  label: string
}
shape Pirate {
  badge: Badge
  rank: int
}

function later(value: int) -> int {
  wait sleep(1)
  return value
}
";

#[test]
fn nested_shape_local_crossing_a_wait_is_released() {
    let source = format!(
        "{PIRATE}
function entrypoint() -> nothing {{
  let pirate: Pirate = {{ badge: {{ label: `ab`.toUpperCase() }}, rank: 2 }}
  print(pirate.rank)
  wait sleep(1)
  print(`done`)
}}
"
    );
    assert_balanced("nested_shape_local", &source, "2\ndone\n");
}

#[test]
#[ignore = "planned RED (C8 S4a step 3): a temporary of a suspending statement has no frame region yet"]
fn nested_shape_temporary_of_a_suspending_statement_is_released() {
    let source = format!(
        "{PIRATE}
function makePirate() -> Pirate {{
  return {{ badge: {{ label: `ab`.toUpperCase() }}, rank: 2 }}
}}

function entrypoint() -> nothing {{
  let rank: int = wait later(makePirate().rank)
  print(rank)
}}
"
    );
    assert_balanced("nested_shape_temporary", &source, "2\n");
}

const NAMES: &str = "
function later(value: int) -> int {
  wait sleep(1)
  return value
}

function firstName(names: array<string>) -> maybe<string> {
  return names.get(0)
}
";

#[test]
fn maybe_of_a_string_local_crossing_a_wait_is_released() {
    let source = format!(
        "{NAMES}
function entrypoint() -> nothing {{
  let names: array<string> = [`a-${{1}}`, `b-${{2}}`]
  let found: maybe<string> = names.get(0)
  print(found.or(`none`))
  wait sleep(1)
  print(`done`)
}}
"
    );
    assert_balanced("maybe_local", &source, "a-1\ndone\n");
}

#[test]
#[ignore = "planned RED (C8 S4a step 3): a temporary of a suspending statement has no frame region yet"]
fn maybe_of_a_string_temporary_of_a_suspending_statement_is_released() {
    let source = format!(
        "{NAMES}
function entrypoint() -> nothing {{
  let names: array<string> = [`a-${{1}}`, `b-${{2}}`]
  let length: int = wait later(firstName(names).or(`none`).count())
  print(length)
}}
"
    );
    assert_balanced("maybe_temporary", &source, "3\n");
}

const LISTS: &str = "
function later(value: int) -> int {
  wait sleep(1)
  return value
}

function makeList() -> fixed<string> {
  return [`p-${1}`, `q-${2}`]
}
";

#[test]
fn fixed_of_strings_local_crossing_a_wait_is_released() {
    let source = format!(
        "{LISTS}
function entrypoint() -> nothing {{
  let list: fixed<string> = [`p-${{1}}`, `q-${{2}}`]
  print(list.count())
  wait sleep(1)
  print(`done`)
}}
"
    );
    assert_balanced("fixed_local", &source, "2\ndone\n");
}

#[test]
#[ignore = "planned RED (C8 S4a step 3): a temporary of a suspending statement has no frame region yet"]
fn fixed_of_strings_temporary_of_a_suspending_statement_is_released() {
    let source = format!(
        "{LISTS}
function entrypoint() -> nothing {{
  let list: fixed<string> = [`p-${{1}}`, `q-${{2}}`]
  let count: int = wait later(list.copy().count())
  print(count)
}}
"
    );
    assert_balanced("fixed_temporary", &source, "2\n");
}

const THINGS: &str = "
shape Tiny {
  flag: boolean
}
shape Big {
  label: string
  count: int
}
shape Thing = Tiny | Big

function describe(share thing: Thing) -> string {
  if (thing) {
    is Tiny => {
      return `tiny`
    }
    is Big => {
      return `big ${thing.label}`
    }
  }
  return `none`
}

function later(value: int) -> int {
  wait sleep(1)
  return value
}

function makeThing() -> Thing {
  let big: Big = { label: `ab`.toUpperCase(), count: 3 }
  return big
}
";

#[test]
fn union_local_crossing_a_wait_is_released() {
    let source = format!(
        "{THINGS}
function entrypoint() -> nothing {{
  let big: Big = {{ label: `ab`.toUpperCase(), count: 3 }}
  let thing: Thing = big
  print(describe(thing))
  wait sleep(1)
  print(`done`)
}}
"
    );
    assert_balanced("union_local", &source, "big AB\ndone\n");
}

#[test]
#[ignore = "planned RED (C8 S4a step 3): a temporary of a suspending statement has no frame region yet"]
fn union_temporary_of_a_suspending_statement_is_released() {
    let source = format!(
        "{THINGS}
function entrypoint() -> nothing {{
  let length: int = wait later(describe(makeThing()).count())
  print(length)
}}
"
    );
    assert_balanced("union_temporary", &source, "6\n");
}

/// Bug it catches: a call result staged by pointer into a suspending callee sat on the dead resume
/// stack, so the callee read garbage after its own `wait` (probe: it printed 4241760, not 3, at both
/// optimization levels, and no sanitizer sees it). Planned red for the temporaries step; the fix is
/// the same frame region a temporary of a suspending statement gets.
#[test]
#[ignore = "planned RED (C8 S4a, temporaries of a suspending statement): a call result staged into a suspending callee points into the dead resume stack"]
fn shape_call_result_staged_into_a_suspending_callee_survives_the_callee_wait() {
    let source = "
shape Crew {
  name: string
  size: int
}

function makeCrew() -> Crew {
  return { name: `ab`.toUpperCase(), size: 3 }
}

function readLater(share crew: Crew) -> int {
  wait sleep(5)
  return crew.size
}

function entrypoint() -> nothing {
  let size: int = wait readLater(makeCrew())
  print(size)
}
";
    assert_balanced("shape_call_result_staged", source, "3\n");
}

/// Bug it catches: a shape given away on one path is `ReleaseIfLive`; its frame region is rebuilt
/// as a non-null pointer at every resume, so the scope exit after a `wait` released a shape the
/// callee had already released (a double free the move site's null could not stop).
#[test]
fn shape_given_away_on_one_path_is_released_once_across_a_wait() {
    let source = "
shape Crew {
  name: string
  size: int
}

function consume(give crew: Crew) -> int {
  return crew.size
}

function entrypoint() -> nothing {
  let crew: Crew = { name: `ab`.toUpperCase(), size: 3 }
  let flag: int = `1`.toInt().or(0)
  if (flag == 1) {
    print(consume(crew))
  }
  wait sleep(1)
  print(`done`)
}
";
    assert_balanced("shape_given_one_path", source, "3\ndone\n");
}

/// Bug it catches: the same double free for an envelope union region (a union given away on one
/// path, then a `wait`, was released again at the scope exit because the resume rebuilt its view
/// from the region).
#[test]
fn union_given_away_on_one_path_is_released_once_across_a_wait() {
    let source = format!(
        "{THINGS}
function consume(give thing: Thing) -> int {{
  return describe(thing).count()
}}

function entrypoint() -> nothing {{
  let big: Big = {{ label: `ab`.toUpperCase(), count: 3 }}
  let thing: Thing = big
  let flag: int = `1`.toInt().or(0)
  if (flag == 1) {{
    print(consume(thing))
  }}
  wait sleep(1)
  print(`done`)
}}
"
    );
    assert_balanced("union_given_one_path", &source, "6\ndone\n");
}

/// Bug it catches: a `maybe<Crew>` envelope and its shape payload sat in resume-stack slots, so a
/// release after the `wait` freed the strings of a dead stack copy (or a reassignment left the old
/// payload unreleased). Covers the payload region and the reassignment's snapshot.
#[test]
fn maybe_of_a_shape_local_with_a_payload_region_is_released_and_reassigned() {
    let source = format!(
        "{CREW}
function entrypoint() -> nothing {{
  let crews: array<Crew> = [{{ name: `ab`.toUpperCase(), size: 3 }}, {{ name: `cd`.toUpperCase(), size: 4 }}]
  let found: maybe<Crew> = crews.get(0)
  found = crews.get(1)
  let fallback: Crew = {{ name: `zz`.toUpperCase(), size: 0 }}
  let chosen: Crew = found.or(fallback)
  print(chosen.size)
  wait sleep(1)
  print(`done`)
}}
"
    );
    assert_balanced("maybe_shape_local", &source, "4\ndone\n");
}

/// Bug it catches: a `maybe` given away on one path was released again after the `wait` (the
/// resume re-derived the local from its region).
#[test]
fn maybe_given_away_on_one_path_is_released_once_across_a_wait() {
    let source = "
function consume(give name: maybe<string>) -> int {
  return name.or(`none`).count()
}

function entrypoint() -> nothing {
  let names: array<string> = [`a-${1}`, `b-${2}`]
  let found: maybe<string> = names.get(0)
  let flag: int = `1`.toInt().or(0)
  if (flag == 1) {
    print(consume(found))
  }
  wait sleep(1)
  print(`done`)
}
";
    assert_balanced("maybe_given_one_path", source, "3\ndone\n");
}

/// Bug it catches: a `fixed` list's cells sat on the resume stack, so a whole-list reassignment
/// and an element `set` before a `wait` released (or kept) cells of a dead block.
#[test]
fn fixed_list_reassigned_and_updated_before_a_wait_is_released_once() {
    let source = "
function entrypoint() -> nothing {
  let list: fixed<string> = [`p-${1}`, `q-${2}`]
  let other: fixed<string> = [`r-${3}`, `s-${4}`]
  list = other
  list.set(0, `t-${5}`)
  print(list.count())
  wait sleep(1)
  print(`done`)
}
";
    assert_balanced("fixed_reassigned", source, "2\ndone\n");
}
