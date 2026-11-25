//! v0.3-M9 C6: the one release helper frees exactly what a copy allocates, for every heap-backed
//! type a `background` argument can be.
//!
//! The spawn path is the one place the helper runs before C7 calls it at scope exits: a task
//! owns the copy `emit_owned_copy` made of its argument, and its drop ladder (SM callee) or its
//! closure (CPU callee) releases it with the type's release function. So each type is spawned
//! once and four times from a function that keeps its own value (which leaks until C7), and
//! the extra three spawns must add exactly as many frees as allocations: no copy leaks, per
//! type and per spawn arm. Each case also runs once under AddressSanitizer, where a release of
//! something the copy did not allocate (a shared array, a doubled free) is reported.
//!
//! Covers a deep array copy's items and a spawn-cloned map both spawned one after another and
//! from a `for` loop (parked entries 67 and 68): the `looped_*` modules.
//! A plain `string` is the one type whose copy is a bare allocation (no container around it).
//! High tier: data integrity (a leak per spawn, or a free of memory another holder owns).

mod support;

use support::{
    at_both_optimization_levels, build_asan, build_source, run_asan_binary, run_counted,
};

/// One program family: what the value is and how it is built, read after the spawn, and what the
/// shared declarations are.
struct Case {
    declarations: &'static str,
    value_type: &'static str,
    setup: &'static str,
    read: &'static str,
}

/// Where the spawn statements of a program sit: one after another, or the one statement inside a
/// `for` loop (the shape parked entries 67 and 68 name: a copy per iteration).
#[derive(Clone, Copy)]
enum SpawnSite {
    Straight,
    InLoop,
}

const TAG: &str = "shape Tag {\n  name: String\n  id: Int\n}\n";

fn program(case: &Case, suspending_callee: bool, spawns: usize, site: SpawnSite) -> String {
    let callee_body = if suspending_callee {
        "  wait sleep(1)\n  print(777)\n"
    } else {
        "  print(777)\n"
    };
    let spawn_lines = match site {
        SpawnSite::Straight => "  background show(value)\n".repeat(spawns),
        SpawnSite::InLoop => {
            format!("  for (round in range(0, {spawns})) {{\n    background show(value)\n  }}\n")
        }
    };
    format!(
        "{declarations}\nfunction show(value: {value_type}) -> Nothing {{\n{callee_body}}}\n\n\
         function launch() -> Nothing {{\n{setup}{spawn_lines}{read}}}\n\n\
         function entrypoint() -> Nothing {{\n  launch()\n  wait sleep(200)\n}}\n",
        declarations = case.declarations,
        value_type = case.value_type,
        setup = case.setup,
        read = case.read,
    )
}

/// `(cell alloc, cell free, string alloc, string free)` of one run of the program.
fn counted_run(
    case: &Case,
    suspending_callee: bool,
    no_optimize: bool,
    spawns: usize,
    site: SpawnSite,
) -> (u64, u64, u64, u64) {
    let source = program(case, suspending_callee, spawns, site);
    let built = build_source(&source, "c6 per-type program", no_optimize);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "{source}\nstderr:\n{}", output.stderr);
    assert_eq!(
        output.stdout.matches("777\n").count(),
        spawns,
        "every spawned task must have run to completion before the counters are read:\n{source}"
    );
    (
        counters.alloc,
        counters.free,
        counters.string_alloc,
        counters.string_free,
    )
}

/// Three more spawns allocate and free exactly the same number of cells and of heap strings:
/// whatever the task's copy allocated, its release freed.
fn assert_spawns_balance(case: &Case, suspending_callee: bool, no_optimize: bool, site: SpawnSite) {
    let (alloc_one, free_one, string_alloc_one, string_free_one) =
        counted_run(case, suspending_callee, no_optimize, 1, site);
    let (alloc_four, free_four, string_alloc_four, string_free_four) =
        counted_run(case, suspending_callee, no_optimize, 4, site);
    let extra_allocs = alloc_four - alloc_one;
    let extra_frees = free_four - free_one;
    let extra_string_allocs = string_alloc_four - string_alloc_one;
    let extra_string_frees = string_free_four - string_free_one;
    assert!(
        extra_allocs + extra_string_allocs > 0,
        "three more spawns allocated nothing: the program does not copy its argument\n{}",
        program(case, suspending_callee, 4, site)
    );
    assert_eq!(
        (extra_allocs, extra_string_allocs),
        (extra_frees, extra_string_frees),
        "three more spawns allocated {extra_allocs} cells and {extra_string_allocs} strings and \
         freed {extra_frees} cells and {extra_string_frees} strings: the task's copy of `{}` is \
         not released\n{}",
        case.value_type,
        program(case, suspending_callee, 4, site)
    );
}

fn assert_asan_clean(case: &Case, suspending_callee: bool, no_optimize: bool, site: SpawnSite) {
    let source = program(case, suspending_callee, 4, site);
    let built = build_source(&source, "c6 per-type asan program", no_optimize);
    let run = run_asan_binary(&build_asan(&built));
    assert!(
        !run.stderr.contains("AddressSanitizer"),
        "AddressSanitizer reported an error:\n{}\n{source}",
        run.stderr
    );
    assert_eq!(run.exit_code, 0, "stderr:\n{}\n{source}", run.stderr);
}

/// A spawn path of the `spawn argument release` unit: both levels, balance and AddressSanitizer.
fn assert_task_copy_released(case: &Case, suspending_callee: bool, site: SpawnSite) {
    at_both_optimization_levels(|no_optimize| {
        assert_spawns_balance(case, suspending_callee, no_optimize, site);
        assert_asan_clean(case, suspending_callee, no_optimize, site);
    });
}

/// One named test per type and spawn arm of the `spawn argument release` run unit.
macro_rules! spawn_copy_release_high_case {
    ($($case_name:ident, $case:expr, $suspending_callee:literal, $site:expr;)+) => { $(
        // bug: a spawned task's copy of a heap-backed argument is leaked by nobody releasing it, or freed twice or shared with the spawner, silently at exit 0 | tier: High
        #[test]
        fn $case_name() {
            assert_task_copy_released(&$case, $suspending_callee, $site);
        }
    )+ };
}

const LOOPED_ARRAY_OF_ARRAYS: Case = Case {
        declarations: "",
        value_type: "Array<Array<Int>>",
        setup: "  let first: Array<Int> = [1, 2]\n  let second: Array<Int> = [3]\n  let value: Array<Array<Int>> = [first, second]\n",
        read: "  print(value.count())\n",
    };

const LOOPED_ARRAY_OF_ARRAYS_OF_STRINGS: Case = Case {
        declarations: "",
        value_type: "Array<Array<String>>",
        setup: "  let first: Array<String> = [`a`, `b`]\n  let second: Array<String> = [`c`]\n  let value: Array<Array<String>> = [first, second]\n",
        read: "  print(value.count())\n",
    };

const LOOPED_MAP_OF_INTS: Case = Case {
    declarations: "",
    value_type: "Map<Int, Int>",
    setup: "  let value: Map<Int, Int> = {}\n  value.set(1, 2)\n",
    read: "  print(value.count())\n",
};

const ARRAY_OF_STRINGS: Case = Case {
    declarations: "",
    value_type: "Array<String>",
    setup: "  let value: Array<String> = [`a`, `b`]\n",
    read: "  print(value.count())\n",
};

const ARRAY_OF_ARRAYS: Case = Case {
        declarations: "",
        value_type: "Array<Array<Int>>",
        setup: "  let first: Array<Int> = [1, 2]\n  let second: Array<Int> = [3]\n  let value: Array<Array<Int>> = [first, second]\n",
        read: "  print(value.count())\n",
    };

const ARRAY_OF_SHAPES: Case = Case {
    declarations: TAG,
    value_type: "Array<Tag>",
    setup: "  let tag: Tag = { name: `t`, id: 1 }\n  let value: Array<Tag> = [tag]\n",
    read: "  print(value.count())\n",
};

const MAP_OF_INTS: Case = Case {
    declarations: "",
    value_type: "Map<Int, Int>",
    setup: "  let value: Map<Int, Int> = {}\n  value.set(1, 2)\n",
    read: "  print(value.count())\n",
};

const MAP_OF_STRINGS: Case = Case {
    declarations: "",
    value_type: "Map<String, String>",
    setup: "  let value: Map<String, String> = {}\n  value.set(`k`, `v`)\n",
    read: "  print(value.count())\n",
};

const MAP_OF_ARRAYS: Case = Case {
        declarations: "",
        value_type: "Map<String, Array<Int>>",
        setup: "  let rows: Array<Int> = [1]\n  let value: Map<String, Array<Int>> = {}\n  value.set(`k`, rows)\n",
        read: "  print(value.count())\n",
    };

const SHAPE_WITH_EVERY_KIND_OF_FIELD: Case = Case {
        declarations: "shape Tag {\n  name: String\n  id: Int\n}\nshape Bundle {\n  label: String\n  rows: Array<Int>\n  table: Map<String, Int>\n  inner: Tag\n  note: Maybe<String>\n  id: Int\n}\n",
        value_type: "Bundle",
        setup: "  let labels: Array<String> = [`n`]\n  let rows: Array<Int> = [1, 2]\n  let table: Map<String, Int> = {}\n  table.set(`k`, 1)\n  let inner: Tag = { name: `i`, id: 2 }\n  let value: Bundle = { label: `l`, rows: rows, table: table, inner: inner, note: labels.get(0), id: 3 }\n",
        read: "  print(value.id)\n",
    };

const MAYBE_OF_STRING: Case = Case {
    declarations: "",
    value_type: "Maybe<String>",
    setup: "  let labels: Array<String> = [`n`]\n  let value = labels.get(0)\n",
    read: "  print(value.or(`none`))\n",
};

const MAYBE_OF_SHAPE: Case = Case {
        declarations: TAG,
        value_type: "Maybe<Tag>",
        setup: "  let tag: Tag = { name: `t`, id: 1 }\n  let tags: Array<Tag> = [tag]\n  let value = tags.get(0)\n",
        read: "  print(value.exists())\n",
    };

const FIXED_OF_STRINGS: Case = Case {
    declarations: "",
    value_type: "Fixed<String>",
    setup: "  let value: Fixed<String> = [`p`, `q`]\n",
    read: "  print(value[0].or(`none`))\n",
};

const FIXED_OF_SHAPES: Case = Case {
        declarations: TAG,
        value_type: "Fixed<Tag>",
        setup: "  let first: Tag = { name: `a`, id: 1 }\n  let second: Tag = { name: `b`, id: 2 }\n  let fallback: Tag = { name: `none`, id: 0 }\n  let value: Fixed<Tag> = [first, second]\n",
        read: "  let item: Tag = value[0].or(fallback)\n  print(item.id)\n",
    };

const SHAPE_WITH_A_UNION_FIELD: Case = Case {
    declarations:
        "shape Circle {\n  radius: Float\n}\nshape Box {\n  held: Circle | String\n  id: Int\n}\n",
    value_type: "Box",
    setup: "  let value: Box = { held: `boxed`, id: 1 }\n",
    read: "  print(value.id)\n",
};

const SHAPE_WITH_A_CHANNEL_FIELD: Case = Case {
    declarations: "shape Wire {\n  line: Channel<Int>\n  id: Int\n}\n",
    value_type: "Wire",
    setup:
        "  let line: Channel<Int> = Channel<Int>(2)\n  let value: Wire = { line: line, id: 1 }\n",
    read: "  print(value.id)\n",
};

const PLAIN_STRING: Case = Case {
        declarations: "",
        value_type: "String",
        setup: "  let labels: Array<String> = [`heap string`]\n  let maybeLabel: Maybe<String> = labels.get(0)\n  let value: String = maybeLabel.or(`none`)\n",
        read: "  print(value)\n",
    };

spawn_copy_release_high_case! {
    // case: parked 67: a nested array handed to a `background` spawn inside a loop leaks its inner arrays once per iteration (CPU callee)
    looped_array_of_arrays_cpu_callee, LOOPED_ARRAY_OF_ARRAYS, false, SpawnSite::InLoop;
    // case: parked 67: a nested array handed to a `background` spawn inside a loop leaks its inner arrays once per iteration (state-machine callee)
    looped_array_of_arrays_state_machine_callee, LOOPED_ARRAY_OF_ARRAYS, true, SpawnSite::InLoop;
    // case: parked 67: one allocation per leaf string and per inner array is made per iteration and each is released (CPU callee)
    looped_array_of_arrays_of_strings_cpu_callee, LOOPED_ARRAY_OF_ARRAYS_OF_STRINGS, false, SpawnSite::InLoop;
    // case: parked 67: one allocation per leaf string and per inner array is made per iteration and each is released (state-machine callee)
    looped_array_of_arrays_of_strings_state_machine_callee, LOOPED_ARRAY_OF_ARRAYS_OF_STRINGS, true, SpawnSite::InLoop;
    // case: parked 68: a spawn-cloned `map` in a spawn loop is released by nobody, so the heap grows with the loop (CPU callee)
    looped_map_of_ints_cpu_callee, LOOPED_MAP_OF_INTS, false, SpawnSite::InLoop;
    // case: parked 68: a spawn-cloned `map` in a spawn loop is released by nobody, so the heap grows with the loop (state-machine callee)
    looped_map_of_ints_state_machine_callee, LOOPED_MAP_OF_INTS, true, SpawnSite::InLoop;
    // case: an `Array<String>` copy's strings are freed by nobody (CPU callee)
    array_of_strings_cpu_callee, ARRAY_OF_STRINGS, false, SpawnSite::Straight;
    // case: an `Array<String>` copy's strings are freed by nobody (state-machine callee)
    array_of_strings_state_machine_callee, ARRAY_OF_STRINGS, true, SpawnSite::Straight;
    // case: a deep array copy's items (the inner arrays) are not released with it (CPU callee)
    array_of_arrays_cpu_callee, ARRAY_OF_ARRAYS, false, SpawnSite::Straight;
    // case: a deep array copy's items (the inner arrays) are not released with it (state-machine callee)
    array_of_arrays_state_machine_callee, ARRAY_OF_ARRAYS, true, SpawnSite::Straight;
    // case: an `Array<Tag>` copy's elements (inline shapes holding strings) are not released (CPU callee)
    array_of_shapes_cpu_callee, ARRAY_OF_SHAPES, false, SpawnSite::Straight;
    // case: an `Array<Tag>` copy's elements (inline shapes holding strings) are not released (state-machine callee)
    array_of_shapes_state_machine_callee, ARRAY_OF_SHAPES, true, SpawnSite::Straight;
    // case: a spawn-cloned `map` is released by nobody, so a spawn in a loop grows the heap (CPU callee)
    map_of_ints_cpu_callee, MAP_OF_INTS, false, SpawnSite::Straight;
    // case: a spawn-cloned `map` is released by nobody, so a spawn in a loop grows the heap (state-machine callee)
    map_of_ints_state_machine_callee, MAP_OF_INTS, true, SpawnSite::Straight;
    // case: a `Map<String, String>` copy's keys (held by raw pointer) and values are not released (CPU callee)
    map_of_strings_cpu_callee, MAP_OF_STRINGS, false, SpawnSite::Straight;
    // case: a `Map<String, String>` copy's keys (held by raw pointer) and values are not released (state-machine callee)
    map_of_strings_state_machine_callee, MAP_OF_STRINGS, true, SpawnSite::Straight;
    // case: a map's array values are cloned with the map but freed by nobody (CPU callee)
    map_of_arrays_cpu_callee, MAP_OF_ARRAYS, false, SpawnSite::Straight;
    // case: a map's array values are cloned with the map but freed by nobody (state-machine callee)
    map_of_arrays_state_machine_callee, MAP_OF_ARRAYS, true, SpawnSite::Straight;
    // case: a shape copy's string, array, map, nested shape and `maybe` fields are copied but not released, or shared with the original and released twice (CPU callee)
    shape_with_every_kind_of_field_cpu_callee, SHAPE_WITH_EVERY_KIND_OF_FIELD, false, SpawnSite::Straight;
    // case: a shape copy's string, array, map, nested shape and `maybe` fields are copied but not released, or shared with the original and released twice (state-machine callee)
    shape_with_every_kind_of_field_state_machine_callee, SHAPE_WITH_EVERY_KIND_OF_FIELD, true, SpawnSite::Straight;
    // case: a `Maybe<String>` argument's envelope cell and payload string are not freed (CPU callee)
    maybe_of_string_cpu_callee, MAYBE_OF_STRING, false, SpawnSite::Straight;
    // case: a `Maybe<String>` argument's envelope cell and payload string are not freed (state-machine callee)
    maybe_of_string_state_machine_callee, MAYBE_OF_STRING, true, SpawnSite::Straight;
    // case: a `Maybe<Tag>` argument's envelope cell, payload heap cell and the payload's string are not freed (CPU callee)
    maybe_of_shape_cpu_callee, MAYBE_OF_SHAPE, false, SpawnSite::Straight;
    // case: a `Maybe<Tag>` argument's envelope cell, payload heap cell and the payload's string are not freed (state-machine callee)
    maybe_of_shape_state_machine_callee, MAYBE_OF_SHAPE, true, SpawnSite::Straight;
    // case: a `Fixed<String>` argument's cells hold strings its copy cloned and nobody freed (CPU callee)
    fixed_of_strings_cpu_callee, FIXED_OF_STRINGS, false, SpawnSite::Straight;
    // case: a `Fixed<String>` argument's cells hold strings its copy cloned and nobody freed (state-machine callee)
    fixed_of_strings_state_machine_callee, FIXED_OF_STRINGS, true, SpawnSite::Straight;
    // case: a `Fixed<Tag>` copy shares the original's heap cells and strings, so the task's release frees what the spawner still holds, or the helper refuses the type (CPU callee)
    fixed_of_shapes_cpu_callee, FIXED_OF_SHAPES, false, SpawnSite::Straight;
    // case: a `Fixed<Tag>` copy shares the original's heap cells and strings, so the task's release frees what the spawner still holds, or the helper refuses the type (state-machine callee)
    fixed_of_shapes_state_machine_callee, FIXED_OF_SHAPES, true, SpawnSite::Straight;
    // case: a shape's union slot copy (string variant) is shared with the original or never freed (CPU callee)
    shape_with_a_union_field_cpu_callee, SHAPE_WITH_A_UNION_FIELD, false, SpawnSite::Straight;
    // case: a shape's union slot copy (string variant) is shared with the original or never freed (state-machine callee)
    shape_with_a_union_field_state_machine_callee, SHAPE_WITH_A_UNION_FIELD, true, SpawnSite::Straight;
    // case: a shape copy shares its channel field without taking a counted reference, so the task's release gives back a reference it never held and frees the channel under the spawner (CPU callee)
    shape_with_a_channel_field_cpu_callee, SHAPE_WITH_A_CHANNEL_FIELD, false, SpawnSite::Straight;
    // case: a shape copy shares its channel field without taking a counted reference, so the task's release gives back a reference it never held and frees the channel under the spawner (state-machine callee)
    shape_with_a_channel_field_state_machine_callee, SHAPE_WITH_A_CHANNEL_FIELD, true, SpawnSite::Straight;
    // case: a plain `string` argument's spawn-time clone is freed by nobody, so every spawn leaks one string (CPU callee)
    plain_string_cpu_callee, PLAIN_STRING, false, SpawnSite::Straight;
    // case: a plain `string` argument's spawn-time clone is freed by nobody, so every spawn leaks one string (state-machine callee)
    plain_string_state_machine_callee, PLAIN_STRING, true, SpawnSite::Straight;
}

/// A `string` the spawner no longer holds (its last use is the spawn, a fresh value, a
/// literal) is handed over as it is, not copied: the task owns it, and its release must free it
/// once, never twice and never a static. A callee that declares `give value` may store the
/// string in a container, so its argument keeps the flat behaviour (the string is not released,
/// a recorded deferral to C7): that form must still run clean, freeing nothing twice.
fn handed_over_source(
    setup: &str,
    argument: &str,
    suspending_callee: bool,
    give_callee: bool,
) -> String {
    let wait_line = if suspending_callee {
        "  wait sleep(1)\n"
    } else {
        ""
    };
    let store_line = if give_callee {
        "  let held: Array<String> = []\n  held.add(value)\n"
    } else {
        ""
    };
    let parameter = if give_callee {
        "give value: String"
    } else {
        "value: String"
    };
    format!(
        "function show({parameter}) -> Nothing {{\n{wait_line}{store_line}  print(777)\n}}\n\n\
         function launch() -> Nothing {{\n{setup}  background show({argument})\n}}\n\n\
         function entrypoint() -> Nothing {{\n  launch()\n  wait sleep(200)\n}}\n"
    )
}

fn assert_handed_over_string_runs_clean(
    setup: &str,
    argument: &str,
    task_owns_the_only_string: bool,
    suspending_callee: bool,
    give_callee: bool,
) {
    at_both_optimization_levels(|no_optimize| {
        let program = handed_over_source(setup, argument, suspending_callee, give_callee);
        let built = build_source(&program, "c6 handed-over string", no_optimize);
        let (output, counters) = run_counted(&built);
        assert_eq!(output.exit_code, 0, "{program}\n{}", output.stderr);
        assert_eq!(output.stdout.matches("777\n").count(), 1, "{program}");
        assert!(
            counters.string_free <= counters.string_alloc,
            "{} strings freed for {} allocated\n{program}",
            counters.string_free,
            counters.string_alloc
        );
        if task_owns_the_only_string && !give_callee {
            assert_eq!(
                counters.string_alloc, counters.string_free,
                "the task owns the only string and must free it\n{program}"
            );
        }
        let run = run_asan_binary(&build_asan(&built));
        assert!(
            !run.stderr.contains("AddressSanitizer"),
            "AddressSanitizer reported an error:\n{}\n{program}",
            run.stderr
        );
        assert_eq!(run.exit_code, 0, "{}\n{program}", run.stderr);
    });
}

/// One named test per string source and callee arm of the `handed-over string` run unit (a
/// different harness from the table above: the string is not copied, so there is no spawn-count
/// delta to compare).
macro_rules! handed_over_string_high_case {
    ($($case_name:ident, $owns_only_string:literal, $setup:literal, $argument:literal, $suspending_callee:literal, $give_callee:literal;)+) => { $(
        // bug: a string handed whole to a spawned task is freed twice, freed while static, or leaked, silently at exit 0 | tier: High
        #[test]
        fn $case_name() {
            assert_handed_over_string_runs_clean($setup, $argument, $owns_only_string, $suspending_callee, $give_callee);
        }
    )+ };
}

handed_over_string_high_case! {
    owned_fresh_value_cpu_callee, true, "  let count: Int = 5\n", "`n=${count}`", false, false;
    owned_fresh_value_state_machine_callee, true, "  let count: Int = 5\n", "`n=${count}`", true, false;
    owned_fresh_value_give_cpu_callee, true, "  let count: Int = 5\n", "`n=${count}`", false, true;
    owned_fresh_value_give_state_machine_callee, true, "  let count: Int = 5\n", "`n=${count}`", true, true;
    owned_last_use_of_a_binding_cpu_callee, true, "  let count: Int = 5\n  let value: String = `n=${count}`\n", "value", false, false;
    owned_last_use_of_a_binding_state_machine_callee, true, "  let count: Int = 5\n  let value: String = `n=${count}`\n", "value", true, false;
    owned_last_use_of_a_binding_give_cpu_callee, true, "  let count: Int = 5\n  let value: String = `n=${count}`\n", "value", false, true;
    owned_last_use_of_a_binding_give_state_machine_callee, true, "  let count: Int = 5\n  let value: String = `n=${count}`\n", "value", true, true;
    last_use_of_a_binding_cpu_callee, false, "  let labels: Array<String> = [`heap string`]\n  let maybeLabel: Maybe<String> = labels.get(0)\n  let value: String = maybeLabel.or(`none`)\n", "value", false, false;
    last_use_of_a_binding_state_machine_callee, false, "  let labels: Array<String> = [`heap string`]\n  let maybeLabel: Maybe<String> = labels.get(0)\n  let value: String = maybeLabel.or(`none`)\n", "value", true, false;
    last_use_of_a_binding_give_cpu_callee, false, "  let labels: Array<String> = [`heap string`]\n  let maybeLabel: Maybe<String> = labels.get(0)\n  let value: String = maybeLabel.or(`none`)\n", "value", false, true;
    last_use_of_a_binding_give_state_machine_callee, false, "  let labels: Array<String> = [`heap string`]\n  let maybeLabel: Maybe<String> = labels.get(0)\n  let value: String = maybeLabel.or(`none`)\n", "value", true, true;
    fresh_value_cpu_callee, false, "  let labels: Array<String> = [`heap string`]\n  let maybeLabel: Maybe<String> = labels.get(0)\n", "maybeLabel.or(`none`)", false, false;
    fresh_value_state_machine_callee, false, "  let labels: Array<String> = [`heap string`]\n  let maybeLabel: Maybe<String> = labels.get(0)\n", "maybeLabel.or(`none`)", true, false;
    fresh_value_give_cpu_callee, false, "  let labels: Array<String> = [`heap string`]\n  let maybeLabel: Maybe<String> = labels.get(0)\n", "maybeLabel.or(`none`)", false, true;
    fresh_value_give_state_machine_callee, false, "  let labels: Array<String> = [`heap string`]\n  let maybeLabel: Maybe<String> = labels.get(0)\n", "maybeLabel.or(`none`)", true, true;
    literal_cpu_callee, false, "", "`literal`", false, false;
    literal_state_machine_callee, false, "", "`literal`", true, false;
    literal_give_cpu_callee, false, "", "`literal`", false, true;
    literal_give_state_machine_callee, false, "", "`literal`", true, true;
}
