//! v0.3-M9 C8 slice 4a: a resume function releases the inline-aggregate kinds (flat shape, nested
//! shape, `maybe`, `fixed` list, envelope union): as a local whose release follows a `wait`, and
//! as a temporary of a statement that suspends. Each program leaks or crashes before the slice and
//! now frees every allocation, at both optimization levels, each in its own process.
//!
//! A local here is read only BEFORE the `wait`: it crosses the suspension through its own release
//! (`CrossingNames::release_only`), so the guards that reject a read after a `wait` do not apply.

mod support;

use support::{at_both_optimization_levels, build_source, run_counted};

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
fn fixed_of_strings_temporary_of_a_suspending_statement_is_released() {
    let source = format!(
        "{LISTS}
function entrypoint() -> nothing {{
  let count: int = wait later(makeList().count())
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
