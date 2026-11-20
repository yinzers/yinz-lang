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

use support::{build_asan, build_source, run_asan_binary, run_counted};

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
        "{declarations}\nfunction show(value: {value_type}) -> nothing {{\n{callee_body}}}\n\n\
         function launch() -> nothing {{\n{setup}{spawn_lines}{read}}}\n\n\
         function entrypoint() -> nothing {{\n  launch()\n  wait sleep(200)\n}}\n",
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

macro_rules! case_tests {
    ($name:ident, $bug:literal, $case:expr, $site:expr) => {
        #[doc = $bug]
        mod $name {
            use super::*;

            #[test]
            fn cpu_callee_releases_the_copy() {
                assert_spawns_balance(&$case, false, false, $site);
            }

            #[test]
            fn state_machine_callee_releases_the_copy() {
                assert_spawns_balance(&$case, true, false, $site);
            }

            #[test]
            fn cpu_callee_asan() {
                assert_asan_clean(&$case, false, false, $site);
            }

            #[test]
            fn state_machine_callee_asan() {
                assert_asan_clean(&$case, true, false, $site);
            }

            #[test]
            fn cpu_callee_releases_the_copy_no_optimize() {
                assert_spawns_balance(&$case, false, true, $site);
            }

            #[test]
            fn state_machine_callee_releases_the_copy_no_optimize() {
                assert_spawns_balance(&$case, true, true, $site);
            }

            #[test]
            fn cpu_callee_asan_no_optimize() {
                assert_asan_clean(&$case, false, true, $site);
            }

            #[test]
            fn state_machine_callee_asan_no_optimize() {
                assert_asan_clean(&$case, true, true, $site);
            }
        }
    };
}

/// The cases whose spawns sit one after another in the function body.
macro_rules! release_cases {
    ($name:ident, $bug:literal, $case:expr) => {
        case_tests!($name, $bug, $case, SpawnSite::Straight);
    };
}

/// The cases whose one spawn statement sits inside a `for` loop, so the task copy is made once per
/// iteration and a missed release grows with the loop.
macro_rules! looped_release_cases {
    ($name:ident, $bug:literal, $case:expr) => {
        case_tests!($name, $bug, $case, SpawnSite::InLoop);
    };
}

looped_release_cases!(
    looped_array_of_arrays,
    "parked 67: a nested array handed to a `background` spawn inside a loop leaks its inner arrays once per iteration (High)",
    Case {
        declarations: "",
        value_type: "array<array<int>>",
        setup: "  let first: array<int> = [1, 2]\n  let second: array<int> = [3]\n  let value: array<array<int>> = [first, second]\n",
        read: "  print(value.count())\n",
    }
);

looped_release_cases!(
    looped_array_of_arrays_of_strings,
    "parked 67: one allocation per leaf string and per inner array is made per iteration and each is released (High)",
    Case {
        declarations: "",
        value_type: "array<array<string>>",
        setup: "  let first: array<string> = [`a`, `b`]\n  let second: array<string> = [`c`]\n  let value: array<array<string>> = [first, second]\n",
        read: "  print(value.count())\n",
    }
);

looped_release_cases!(
    looped_map_of_ints,
    "parked 68: a spawn-cloned `map` in a spawn loop is released by nobody, so the heap grows with the loop (High)",
    Case {
        declarations: "",
        value_type: "map<int, int>",
        setup: "  let value: map<int, int> = {}\n  value.set(1, 2)\n",
        read: "  print(value.count())\n",
    }
);

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
    "a deep array copy's items (the inner arrays) are not released with it (High)",
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
    "a spawn-cloned `map` is released by nobody, so a spawn in a loop grows the heap (High)",
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
        declarations: "shape Tag {\n  name: String\n  id: Int\n}\nshape Bundle {\n  label: String\n  rows: Array<Int>\n  table: Map<String, Int>\n  inner: Tag\n  note: Maybe<String>\n  id: Int\n}\n",
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
    fixed_of_shapes,
    "a `fixed<Tag>` copy shares the original's heap cells and strings, so the task's release frees what the spawner still holds, or the helper refuses the type (High)",
    Case {
        declarations: TAG,
        value_type: "fixed<Tag>",
        setup: "  let first: Tag = { name: `a`, id: 1 }\n  let second: Tag = { name: `b`, id: 2 }\n  let fallback: Tag = { name: `none`, id: 0 }\n  let value: fixed<Tag> = [first, second]\n",
        read: "  let item: Tag = value[0].or(fallback)\n  print(item.id)\n",
    }
);

release_cases!(
    shape_with_a_union_field,
    "a shape's union slot copy (string variant) is shared with the original or never freed (High)",
    Case {
        declarations: "shape Circle {\n  radius: Float\n}\nshape Box {\n  held: Circle | String\n  id: Int\n}\n",
        value_type: "Box",
        setup: "  let value: Box = { held: `boxed`, id: 1 }\n",
        read: "  print(value.id)\n",
    }
);

release_cases!(
    shape_with_a_channel_field,
    "a shape copy shares its channel field without taking a counted reference, so the task's release gives back a reference it never held and frees the channel under the spawner (High)",
    Case {
        declarations: "shape Wire {\n  line: Channel<Int>\n  id: Int\n}\n",
        value_type: "Wire",
        setup: "  let line: channel<int> = channel<int>(2)\n  let value: Wire = { line: line, id: 1 }\n",
        read: "  print(value.id)\n",
    }
);

release_cases!(
    plain_string,
    "a plain `string` argument's spawn-time clone is freed by nobody, so every spawn leaks one string (High)",
    Case {
        declarations: "",
        value_type: "string",
        setup: "  let labels: array<string> = [`heap string`]\n  let maybeLabel: maybe<string> = labels.get(0)\n  let value: string = maybeLabel.or(`none`)\n",
        read: "  print(value)\n",
    }
);

/// A `string` the spawner no longer holds (its last use is the spawn, a fresh value, a
/// literal) is handed over as it is, not copied: the task owns it, and its release must free it
/// once, never twice and never a static. A callee that declares `give value` may store the
/// string in a container, so its argument keeps the flat behaviour (the string is not released,
/// a recorded deferral to C7): that form must still run clean, freeing nothing twice.
mod string_handed_over_whole {
    use super::*;

    fn source(setup: &str, argument: &str, suspending_callee: bool, give_callee: bool) -> String {
        let wait_line = if suspending_callee {
            "  wait sleep(1)\n"
        } else {
            ""
        };
        let store_line = if give_callee {
            "  let held: array<string> = []\n  held.add(value)\n"
        } else {
            ""
        };
        let parameter = if give_callee {
            "give value: string"
        } else {
            "value: string"
        };
        format!(
            "function show({parameter}) -> nothing {{\n{wait_line}{store_line}  print(777)\n}}\n\n\
             function launch() -> nothing {{\n{setup}  background show({argument})\n}}\n\n\
             function entrypoint() -> nothing {{\n  launch()\n  wait sleep(200)\n}}\n"
        )
    }

    fn assert_runs_clean(
        setup: &str,
        argument: &str,
        task_owns_the_only_string: bool,
        suspending_callee: bool,
        give_callee: bool,
        no_optimize: bool,
    ) {
        let program = source(setup, argument, suspending_callee, give_callee);
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
    }

    macro_rules! handed_over_case {
        ($name:ident, $owns_only_string:literal, $setup:literal, $argument:literal) => {
            mod $name {
                use super::*;

                #[test]
                fn cpu_callee() {
                    assert_runs_clean($setup, $argument, $owns_only_string, false, false, false);
                }

                #[test]
                fn state_machine_callee() {
                    assert_runs_clean($setup, $argument, $owns_only_string, true, false, false);
                }

                #[test]
                fn cpu_callee_no_optimize() {
                    assert_runs_clean($setup, $argument, $owns_only_string, false, false, true);
                }

                #[test]
                fn state_machine_callee_no_optimize() {
                    assert_runs_clean($setup, $argument, $owns_only_string, true, false, true);
                }

                #[test]
                fn give_cpu_callee() {
                    assert_runs_clean($setup, $argument, $owns_only_string, false, true, false);
                }

                #[test]
                fn give_state_machine_callee() {
                    assert_runs_clean($setup, $argument, $owns_only_string, true, true, false);
                }

                #[test]
                fn give_cpu_callee_no_optimize() {
                    assert_runs_clean($setup, $argument, $owns_only_string, false, true, true);
                }

                #[test]
                fn give_state_machine_callee_no_optimize() {
                    assert_runs_clean($setup, $argument, $owns_only_string, true, true, true);
                }
            }
        };
    }

    handed_over_case!(
        owned_fresh_value,
        true,
        "  let count: int = 5\n",
        "`n=${count}`"
    );
    handed_over_case!(
        owned_last_use_of_a_binding,
        true,
        "  let count: int = 5\n  let value: string = `n=${count}`\n",
        "value"
    );
    handed_over_case!(
        last_use_of_a_binding,
        false,
        "  let labels: array<string> = [`heap string`]\n  let maybeLabel: maybe<string> = labels.get(0)\n  let value: string = maybeLabel.or(`none`)\n",
        "value"
    );
    handed_over_case!(
        fresh_value,
        false,
        "  let labels: array<string> = [`heap string`]\n  let maybeLabel: maybe<string> = labels.get(0)\n",
        "maybeLabel.or(`none`)"
    );
    handed_over_case!(literal, false, "", "`literal`");
}
