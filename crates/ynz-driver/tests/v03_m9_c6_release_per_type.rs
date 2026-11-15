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
//! Covers parked 67 (a deep array copy's items) and parked 68 (a spawn-cloned map in a loop).
//! High tier: data integrity (a leak per spawn, or a free of memory another holder owns).

mod support;

use support::{build_asan, build_source, run_asan_binary, run_counted};

/// One program family: what the value is and how it is built, read after the spawn, and what the
/// shared declarations are.
struct Case {
    declarations: &'static str,
    value_type: &'static str,
    setup: &'static str,
    read: &'static str,
}

const TAG: &str = "shape Tag {\n  name: string\n  id: int\n}\n";

fn program(case: &Case, suspending_callee: bool, spawns: usize) -> String {
    let callee_body = if suspending_callee {
        "  wait sleep(1)\n  print(777)\n"
    } else {
        "  print(777)\n"
    };
    let spawn_lines = "  background show(value)\n".repeat(spawns);
    format!(
        "{declarations}\nfunction show(value: {value_type}) -> nothing {{\n{callee_body}}}\n\n\
         function launch() -> nothing {{\n{setup}{spawn_lines}{read}}}\n\n\
         function entrypoint() -> nothing {{\n  launch()\n  wait sleep(200)\n}}\n",
        declarations = case.declarations,
        value_type = case.value_type,
        setup = case.setup,
        read = case.read,
    )
}

/// `(alloc, free)` of one run of the program.
fn counted_run(case: &Case, suspending_callee: bool, spawns: usize) -> (u64, u64) {
    let source = program(case, suspending_callee, spawns);
    let built = build_source(&source, "c6 per-type program", false);
    let (output, counters) = run_counted(&built);
    assert_eq!(
        output.exit_code, 0,
        "{source}\nstderr:\n{}",
        output.stderr
    );
    assert_eq!(
        output.stdout.matches("777\n").count(),
        spawns,
        "every spawned task must have run to completion before the counters are read:\n{source}"
    );
    (counters.alloc, counters.free)
}

/// Three more spawns allocate and free exactly the same number of cells: whatever the task's
/// copy allocated, its release freed.
fn assert_spawns_balance(case: &Case, suspending_callee: bool) {
    let (alloc_one, free_one) = counted_run(case, suspending_callee, 1);
    let (alloc_four, free_four) = counted_run(case, suspending_callee, 4);
    let extra_allocs = alloc_four - alloc_one;
    let extra_frees = free_four - free_one;
    assert!(
        extra_allocs > 0,
        "three more spawns allocated nothing: the program does not copy its argument\n{}",
        program(case, suspending_callee, 4)
    );
    assert_eq!(
        extra_allocs,
        extra_frees,
        "three more spawns allocated {extra_allocs} cells and freed {extra_frees}: the task's \
         copy of `{}` is not released\n{}",
        case.value_type,
        program(case, suspending_callee, 4)
    );
}

fn assert_asan_clean(case: &Case, suspending_callee: bool) {
    let source = program(case, suspending_callee, 4);
    let built = build_source(&source, "c6 per-type asan program", false);
    let run = run_asan_binary(&build_asan(&built));
    assert!(
        !run.stderr.contains("AddressSanitizer"),
        "AddressSanitizer reported an error:\n{}\n{source}",
        run.stderr
    );
    assert_eq!(run.exit_code, 0, "stderr:\n{}\n{source}", run.stderr);
}

macro_rules! release_cases {
    ($name:ident, $bug:literal, $case:expr) => {
        #[doc = $bug]
        mod $name {
            use super::*;

            #[test]
            fn cpu_callee_releases_the_copy() {
                assert_spawns_balance(&$case, false);
            }

            #[test]
            fn state_machine_callee_releases_the_copy() {
                assert_spawns_balance(&$case, true);
            }

            #[test]
            fn cpu_callee_asan() {
                assert_asan_clean(&$case, false);
            }

            #[test]
            fn state_machine_callee_asan() {
                assert_asan_clean(&$case, true);
            }
        }
    };
}

release_cases!(
    array_of_strings,
    "an `array<string>` copy's strings are freed by nobody (High)",
    Case {
        declarations: "",
        value_type: "array<string>",
        setup: "  let value: array<string> = [`a`, `b`]\n",
        read: "  print(value.count())\n",
    }
);

release_cases!(
    array_of_arrays,
    "parked 67: a deep array copy's items (the inner arrays) are not released with it (High)",
    Case {
        declarations: "",
        value_type: "array<array<int>>",
        setup: "  let first: array<int> = [1, 2]\n  let second: array<int> = [3]\n  let value: array<array<int>> = [first, second]\n",
        read: "  print(value.count())\n",
    }
);

release_cases!(
    array_of_shapes,
    "an `array<Tag>` copy's elements (inline shapes holding strings) are not released (High)",
    Case {
        declarations: TAG,
        value_type: "array<Tag>",
        setup: "  let tag: Tag = { name: `t`, id: 1 }\n  let value: array<Tag> = [tag]\n",
        read: "  print(value.count())\n",
    }
);

release_cases!(
    map_of_ints,
    "parked 68: a spawn-cloned `map` is released by nobody, so a spawn in a loop grows the heap (High)",
    Case {
        declarations: "",
        value_type: "map<int, int>",
        setup: "  let value: map<int, int> = {}\n  value.set(1, 2)\n",
        read: "  print(value.count())\n",
    }
);

release_cases!(
    map_of_strings,
    "a `map<string, string>` copy's keys (held by raw pointer) and values are not released (High)",
    Case {
        declarations: "",
        value_type: "map<string, string>",
        setup: "  let value: map<string, string> = {}\n  value.set(`k`, `v`)\n",
        read: "  print(value.count())\n",
    }
);

release_cases!(
    map_of_arrays,
    "a map's array values are cloned with the map but freed by nobody (High)",
    Case {
        declarations: "",
        value_type: "map<string, array<int>>",
        setup: "  let rows: array<int> = [1]\n  let value: map<string, array<int>> = {}\n  value.set(`k`, rows)\n",
        read: "  print(value.count())\n",
    }
);

release_cases!(
    shape_with_every_kind_of_field,
    "a shape copy's string, array, map, nested shape and `maybe` fields are copied but not released, or shared with the original and released twice (High)",
    Case {
        declarations: "shape Tag {\n  name: string\n  id: int\n}\nshape Bundle {\n  label: string\n  rows: array<int>\n  table: map<string, int>\n  inner: Tag\n  note: maybe<string>\n  id: int\n}\n",
        value_type: "Bundle",
        setup: "  let labels: array<string> = [`n`]\n  let rows: array<int> = [1, 2]\n  let table: map<string, int> = {}\n  table.set(`k`, 1)\n  let inner: Tag = { name: `i`, id: 2 }\n  let value: Bundle = { label: `l`, rows: rows, table: table, inner: inner, note: labels.get(0), id: 3 }\n",
        read: "  print(value.id)\n",
    }
);

release_cases!(
    maybe_of_string,
    "a `maybe<string>` argument's envelope cell and payload string are not freed (High)",
    Case {
        declarations: "",
        value_type: "maybe<string>",
        setup: "  let labels: array<string> = [`n`]\n  let value = labels.get(0)\n",
        read: "  print(value.or(`none`))\n",
    }
);

release_cases!(
    maybe_of_shape,
    "a `maybe<Tag>` argument's envelope cell, payload heap cell and the payload's string are not freed (High)",
    Case {
        declarations: TAG,
        value_type: "maybe<Tag>",
        setup: "  let tag: Tag = { name: `t`, id: 1 }\n  let tags: array<Tag> = [tag]\n  let value = tags.get(0)\n",
        read: "  print(value.exists())\n",
    }
);

release_cases!(
    fixed_of_strings,
    "a `fixed<string>` argument's cells hold strings its copy cloned and nobody freed (High)",
    Case {
        declarations: "",
        value_type: "fixed<string>",
        setup: "  let value: fixed<string> = [`p`, `q`]\n",
        read: "  print(value[0].or(`none`))\n",
    }
);

release_cases!(
    shape_with_a_union_field,
    "a shape's union slot copy (string variant) is shared with the original or never freed (High)",
    Case {
        declarations: "shape Circle {\n  radius: float\n}\nshape Box {\n  held: Circle | string\n  id: int\n}\n",
        value_type: "Box",
        setup: "  let value: Box = { held: `boxed`, id: 1 }\n",
        read: "  print(value.id)\n",
    }
);

release_cases!(
    shape_with_a_channel_field,
    "a shape copy shares its channel field without taking a counted reference, so the task's release gives back a reference it never held and frees the channel under the spawner (High)",
    Case {
        declarations: "shape Wire {\n  line: channel<int>\n  id: int\n}\n",
        value_type: "Wire",
        setup: "  let line: channel<int> = channel<int>(2)\n  let value: Wire = { line: line, id: 1 }\n",
        read: "  print(value.id)\n",
    }
);
