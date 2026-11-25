//! v0.3-M9 C8 slice 4a: a resume function releases the inline-aggregate kinds (flat shape, nested
//! shape, `maybe`, `fixed` list, envelope union): as a local whose release follows a `wait`, and
//! as a temporary of a statement that suspends. Each program leaks or crashes before the slice and
//! now frees every allocation, at both optimization levels, each in its own process.
//!
//! A local here is read only BEFORE the `wait`: it crosses the suspension through its own release
//! (`CrossingNames::release_only`), so the guards that reject a read after a `wait` do not apply.

mod support;

use support::assert_source_balanced_and_asan_clean as assert_balanced;

const CREW: &str = "
shape Crew {
  name: String
  size: Int
}

function later(value: Int) -> Int {
  wait sleep(1)
  return value
}
";

const PIRATE: &str = "
shape Badge {
  label: String
}
shape Pirate {
  badge: Badge
  rank: Int
}

function later(value: Int) -> Int {
  wait sleep(1)
  return value
}
";

const NAMES: &str = "
function later(value: Int) -> Int {
  wait sleep(1)
  return value
}

function firstName(names: Array<String>) -> Maybe<String> {
  return names.get(0)
}
";

const LISTS: &str = "
function later(value: Int) -> Int {
  wait sleep(1)
  return value
}

function makeList() -> Fixed<String> {
  return [`p-${1}`, `q-${2}`]
}
";

const THINGS: &str = "
shape Tiny {
  flag: Boolean
}
shape Big {
  label: String
  count: Int
}
shape Thing = Tiny | Big

function describe(share thing: Thing) -> String {
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

function later(value: Int) -> Int {
  wait sleep(1)
  return value
}

function makeThing() -> Thing {
  let big: Big = { label: `ab`.toUpperCase(), count: 3 }
  return big
}
";

const STAGED_CREW: &str = "
shape Crew {
  name: String
  size: Int
}

shape Tiny {
  flag: Boolean
}

function makeCrew() -> Crew {
  return { name: `ab`.toUpperCase(), size: 3 }
}

function makeTiny() -> Tiny {
  return { flag: true }
}

function takeLater(give crew: Crew) -> Int {
  wait sleep(5)
  return crew.size
}

function readTinyLater(share tiny: Tiny) -> Int {
  wait sleep(5)
  if (tiny.flag) {
    return 1
  }
  return 0
}
";

/// One named test per aggregate kind and position of the `slice 4a inline aggregates in a resume function` run unit; each case is balanced (heap and strings) and ASan clean at both optimization levels.
macro_rules! s4a_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a resume function leaks or frees twice an inline aggregate (flat or nested shape, maybe, fixed list, envelope union) it holds as a local across a wait or as a temporary of a suspending statement, silently at exit 0 | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

s4a_high_case! {
    shape_local_crossing_a_wait_is_released => {
    let source = format!(
        "{CREW}
function entrypoint() -> Nothing {{
  let crew: Crew = {{ name: `ab`.toUpperCase(), size: 3 }}
  print(crew.size)
  wait sleep(1)
  print(`done`)
}}
"
    );
    assert_balanced("shape_local", &source, "3\ndone\n");
    }
    shape_temporary_of_a_suspending_statement_is_released => {
    let source = format!(
        "{CREW}
function makeCrew() -> Crew {{
  return {{ name: `ab`.toUpperCase(), size: 3 }}
}}

function entrypoint() -> Nothing {{
  let size: Int = wait later(makeCrew().size)
  print(size)
}}
"
    );
    assert_balanced("shape_temporary", &source, "3\n");
    }
    nested_shape_local_crossing_a_wait_is_released => {
    let source = format!(
        "{PIRATE}
function entrypoint() -> Nothing {{
  let pirate: Pirate = {{ badge: {{ label: `ab`.toUpperCase() }}, rank: 2 }}
  print(pirate.rank)
  wait sleep(1)
  print(`done`)
}}
"
    );
    assert_balanced("nested_shape_local", &source, "2\ndone\n");
    }
    nested_shape_temporary_of_a_suspending_statement_is_released => {
    let source = format!(
        "{PIRATE}
function makePirate() -> Pirate {{
  return {{ badge: {{ label: `ab`.toUpperCase() }}, rank: 2 }}
}}

function entrypoint() -> Nothing {{
  let rank: Int = wait later(makePirate().rank)
  print(rank)
}}
"
    );
    assert_balanced("nested_shape_temporary", &source, "2\n");
    }
    maybe_of_a_string_local_crossing_a_wait_is_released => {
    let source = format!(
        "{NAMES}
function entrypoint() -> Nothing {{
  let names: Array<String> = [`a-${{1}}`, `b-${{2}}`]
  let found: Maybe<String> = names.get(0)
  print(found.or(`none`))
  wait sleep(1)
  print(`done`)
}}
"
    );
    assert_balanced("maybe_local", &source, "a-1\ndone\n");
    }
    maybe_of_a_string_temporary_of_a_suspending_statement_is_released => {
    let source = format!(
        "{NAMES}
function entrypoint() -> Nothing {{
  let names: Array<String> = [`a-${{1}}`, `b-${{2}}`]
  let length: Int = wait later(firstName(names).or(`none`).count())
  print(length)
}}
"
    );
    assert_balanced("maybe_temporary", &source, "3\n");
    }
    fixed_of_strings_local_crossing_a_wait_is_released => {
    let source = format!(
        "{LISTS}
function entrypoint() -> Nothing {{
  let list: Fixed<String> = [`p-${{1}}`, `q-${{2}}`]
  print(list.count())
  wait sleep(1)
  print(`done`)
}}
"
    );
    assert_balanced("fixed_local", &source, "2\ndone\n");
    }
    fixed_of_strings_temporary_of_a_suspending_statement_is_released => {
    let source = format!(
        "{LISTS}
function entrypoint() -> Nothing {{
  let list: Fixed<String> = [`p-${{1}}`, `q-${{2}}`]
  let count: Int = wait later(list.copy().count())
  print(count)
}}
"
    );
    assert_balanced("fixed_temporary", &source, "2\n");
    }
    union_local_crossing_a_wait_is_released => {
    let source = format!(
        "{THINGS}
function entrypoint() -> Nothing {{
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
    union_temporary_of_a_suspending_statement_is_released => {
    let source = format!(
        "{THINGS}
function entrypoint() -> Nothing {{
  let length: Int = wait later(describe(makeThing()).count())
  print(length)
}}
"
    );
    assert_balanced("union_temporary", &source, "6\n");
    }
    // case: a call result staged by pointer into a suspending callee sat on the dead resume stack, so the callee read garbage after its own `wait` (probe: it printed 4241760, not 3, at both optimization levels, and no sanitizer sees it). Planned red for the temporaries step; the fix is the same frame region a temporary of a suspending statement gets.
    shape_call_result_staged_into_a_suspending_callee_survives_the_callee_wait => {
    let source = "
shape Crew {
  name: String
  size: Int
}

function makeCrew() -> Crew {
  return { name: `ab`.toUpperCase(), size: 3 }
}

function readLater(share crew: Crew) -> Int {
  wait sleep(5)
  return crew.size
}

function entrypoint() -> Nothing {
  let size: Int = wait readLater(makeCrew())
  print(size)
}
";
    assert_balanced("shape_call_result_staged", source, "3\n");
    }
    // case: a shape given away on one path is `ReleaseIfLive`; its frame region is rebuilt as a non-null pointer at every resume, so the scope exit after a `wait` released a shape the callee had already released (a double free the move site's null could not stop).
    shape_given_away_on_one_path_is_released_once_across_a_wait => {
    let source = "
shape Crew {
  name: String
  size: Int
}

function consume(give crew: Crew) -> Int {
  return crew.size
}

function entrypoint() -> Nothing {
  let crew: Crew = { name: `ab`.toUpperCase(), size: 3 }
  let flag: Int = `1`.toInt().or(0)
  if (flag == 1) {
    print(consume(crew))
  }
  wait sleep(1)
  print(`done`)
}
";
    assert_balanced("shape_given_one_path", source, "3\ndone\n");
    }
    // case: the same double free for an envelope union region (a union given away on one path, then a `wait`, was released again at the scope exit because the resume rebuilt its view from the region).
    union_given_away_on_one_path_is_released_once_across_a_wait => {
    let source = format!(
        "{THINGS}
function consume(give thing: Thing) -> Int {{
  return describe(thing).count()
}}

function entrypoint() -> Nothing {{
  let big: Big = {{ label: `ab`.toUpperCase(), count: 3 }}
  let thing: Thing = big
  let flag: Int = `1`.toInt().or(0)
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
    // case: a `maybe<Crew>` envelope and its shape payload sat in resume-stack slots, so a release after the `wait` freed the strings of a dead stack copy (or a reassignment left the old payload unreleased). Covers the payload region and the reassignment's snapshot.
    maybe_of_a_shape_local_with_a_payload_region_is_released_and_reassigned => {
    let source = format!(
        "{CREW}
function entrypoint() -> Nothing {{
  let crews: Array<Crew> = [{{ name: `ab`.toUpperCase(), size: 3 }}, {{ name: `cd`.toUpperCase(), size: 4 }}]
  let found: Maybe<Crew> = crews.get(0)
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
    // case: a `maybe` given away on one path was released again after the `wait` (the resume re-derived the local from its region).
    maybe_given_away_on_one_path_is_released_once_across_a_wait => {
    let source = "
function consume(give name: Maybe<String>) -> Int {
  return name.or(`none`).count()
}

function entrypoint() -> Nothing {
  let names: Array<String> = [`a-${1}`, `b-${2}`]
  let found: Maybe<String> = names.get(0)
  let flag: Int = `1`.toInt().or(0)
  if (flag == 1) {
    print(consume(found))
  }
  wait sleep(1)
  print(`done`)
}
";
    assert_balanced("maybe_given_one_path", source, "3\ndone\n");
    }
    // case: a `fixed` list's cells sat on the resume stack, so a whole-list reassignment and an element `set` before a `wait` released (or kept) cells of a dead block.
    fixed_list_reassigned_and_updated_before_a_wait_is_released_once => {
    let source = "
function entrypoint() -> Nothing {
  let list: Fixed<String> = [`p-${1}`, `q-${2}`]
  let other: Fixed<String> = [`r-${3}`, `s-${4}`]
  list = other
  list.set(0, `t-${5}`)
  print(list.count())
  wait sleep(1)
  print(`done`)
}
";
    assert_balanced("fixed_reassigned", source, "2\ndone\n");
    }
    // case: `let badge: Badge = makePirate().badge` takes the nested shape out of a temporary; the binding is a crossing local, released in the resume function after the `wait`. The take and the temporary's own release (or the local's region) both freed the nested cell (`free(): double free detected`, at both optimization levels).
    nested_shape_taken_out_of_a_temporary_into_a_crossing_local_is_released_once => {
    let source = format!(
        "{PIRATE}
function makePirate() -> Pirate {{
  return {{ badge: {{ label: `ab`.toUpperCase() }}, rank: 2 }}
}}

function entrypoint() -> Nothing {{
  let badge: Badge = makePirate().badge
  wait sleep(1)
  print(badge.label)
}}
"
    );
    assert_balanced("nested_take", &source, "AB\n");
    }
    // case: a call result given to a suspending callee was staged by pointer into the dead resume stack (probe: 4242055 instead of 3, both optimization levels, ASan silent).
    shape_call_result_given_to_a_suspending_callee_survives_the_callee_wait => {
    let source = format!(
        "{STAGED_CREW}
function entrypoint() -> Nothing {{
  let size: Int = wait takeLater(makeCrew())
  print(size)
}}
"
    );
    assert_balanced("shape_given_call_result", &source, "3\n");
    }
    // case: an anonymous literal given to a suspending callee sat in a heap cell the callee never freed (1 allocation left over).
    shape_literal_given_to_a_suspending_callee_leaves_no_cell => {
    let source = format!(
        "{STAGED_CREW}
function entrypoint() -> Nothing {{
  let size: Int = wait takeLater({{ name: `cd`.toUpperCase(), size: 4 }})
  print(size)
}}
"
    );
    assert_balanced("shape_given_literal", &source, "4\n");
    }
    // case: a call result of a shape that owns no heap is no temporary, so nothing kept its bytes either: the suspending callee read the dead resume stack (`share` position).
    heap_free_shape_call_result_survives_a_suspending_callee_wait => {
    let source = format!(
        "{STAGED_CREW}
function entrypoint() -> Nothing {{
  let flag: Int = wait readTinyLater(makeTiny())
  print(flag)
}}
"
    );
    assert_balanced("shape_heap_free_call_result", &source, "1\n");
    }
    // case: a shape given to a suspending callee's union parameter was persisted into the callee's `Param` region as a deep clone, while the binding it came from is moved and never released: the original's string leaked (2 strings allocated, 1 freed).
    shape_given_to_a_suspending_callees_union_parameter_is_released_once => {
    let source = format!(
        "{THINGS}
function slowTake(give thing: Thing) -> Int {{
  wait sleep(1)
  return describe(thing).count()
}}

function entrypoint() -> Nothing {{
  let big: Big = {{ label: `ab`.toUpperCase(), count: 3 }}
  let length: Int = wait slowTake(big)
  print(length)
}}
"
    );
    assert_balanced("union_param_given", &source, "6\n");
    }
    // case: a `maybe` of an envelope union kept its union in a slot of the resume stack, so a local of that type was no owner a resume function released (the union's string leaked), and a release or read after a `wait` would have read a dead stack. The union now lives in the local's frame region; covers a bind from an element read, a reassignment, a bind from a suspending call's result, and the release after the `wait` (a read after it is refused, as for every `maybe`).
    maybe_of_a_union_local_crossing_a_wait_is_released_and_reassigned => {
    let source = format!(
        "{THINGS}
function slowFetch(share things: Array<Thing>, index: Int) -> Maybe<Thing> {{
  wait sleep(1)
  let found = things.get(index)
  return found
}}

function entrypoint() -> Nothing {{
  let things: Array<Thing> = []
  let tiny: Tiny = {{ flag: true }}
  things.add(tiny)
  let big: Big = {{ label: `ab`.toUpperCase(), count: 3 }}
  things.add(big)
  let found: Maybe<Thing> = things.get(0)
  found = things.get(1)
  if (found.exists()) {{
    print(describe(found.value))
  }}
  let later: Maybe<Thing> = wait slowFetch(things, 1)
  if (later.exists()) {{
    print(describe(later.value))
  }}
  wait sleep(1)
  print(`done`)
}}
"
    );
    assert_balanced("maybe_union_local", &source, "big AB\nbig AB\ndone\n");
    }
}
