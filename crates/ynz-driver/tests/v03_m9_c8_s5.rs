//! v0.3-M9 C8 slice 5: the spawn ladder and the leaks it left. Each program frees every
//! allocation, clean under AddressSanitizer, at both optimization levels.

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

/// Bug caught (High: leak): the value half of a `MapEntry` spawn argument (a shape cell and its
/// string) was never freed, for a plain task and for a suspending one.
#[test]
fn a_mapentry_spawn_argument_frees_its_shape_value_and_string() {
    assert_fixture_balanced("v0_3_m9_c8_s5_mapentry_shape_string_spawn.ynz", "a1\na1\n");
}

/// Bug caught (High: leak): the decimal key and value cells of a `MapEntry` spawn argument.
#[test]
fn a_mapentry_spawn_argument_frees_its_decimal_key_and_value_cells() {
    assert_fixture_balanced("v0_3_m9_c8_s5_mapentry_decimal_spawn.ynz", "2.5\n");
}

use support::build_source;

/// Build `source` and run it; it must print `expected`, exit 0, free every heap allocation and
/// every string, and run clean under AddressSanitizer.
fn assert_source_balanced(label: &str, source: &str, expected: &str) {
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

/// A program whose `launch` spawns `eat` with the union parameter `held: Circle | Tag`
/// (`give` when `given`), a suspending callee when `suspending`.
fn union_spawn_program(suspending: bool, given: bool, launch_body: &str) -> String {
    format!(
        "shape Circle {{
  radius: float
}}
shape Tag {{
  name: string
}}
shape Box {{
  choice: Circle | Tag
  id: int
}}

function makeTag() -> Tag {{
  return {{ name: `made` }}
}}

function eat({give}held: Circle | Tag) -> nothing {{
  {pause}
  print(777)
}}

function launch() -> nothing {{
{launch_body}
}}

function entrypoint() -> nothing {{
  launch()
  wait sleep(300)
}}
",
        give = if given { "give " } else { "" },
        pause = if suspending {
            "wait sleep(1)"
        } else {
            "let ready: int = 1"
        },
    )
}

const SPAWN_CONSUMED_SHAPE: &str = "  let tag: Tag = { name: `boxed` }
  background eat(tag)";
const SPAWN_FRESH_CALL: &str = "  background eat(makeTag())";
const SPAWN_SHAPE_USED_AFTER: &str = "  let tag: Tag = { name: `boxed` }
  background eat(tag)
  let size: int = tag.name.count()
  if (size == 0) {
    print(0)
  }";
const SPAWN_CONSUMED_UNION_LOCAL: &str = "  let tag: Tag = { name: `boxed` }
  let held: Circle | Tag = tag
  background eat(held)";
const SPAWN_UNION_FIELD: &str = "  let tag: Tag = { name: `boxed` }
  let box: Box = { choice: tag, id: 1 }
  background eat(box.choice)";

/// Bug caught (High: leak): a shape, a fresh call result or a union local the spawn consumed went
/// to the task as a deep copy while the original was released by nobody (plain task: 2 strings
/// allocated, 1 freed; suspending task: 2 allocated, none freed); a value the spawner keeps (a
/// field, a binding used again) was cloned for a suspending task and the clone's string was
/// never freed. Every spawner form, plain and suspending task, `give` and borrowed parameter.
#[test]
fn a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it() {
    let spawners = [
        ("consumed_shape", SPAWN_CONSUMED_SHAPE, "777\n"),
        ("fresh_call", SPAWN_FRESH_CALL, "777\n"),
        ("shape_used_after", SPAWN_SHAPE_USED_AFTER, "777\n"),
        ("consumed_union_local", SPAWN_CONSUMED_UNION_LOCAL, "777\n"),
        ("union_field", SPAWN_UNION_FIELD, "777\n"),
    ];
    for (name, body, expected) in spawners {
        for suspending in [false, true] {
            for given in [false, true] {
                let source = union_spawn_program(suspending, given, body);
                if given && (name == "shape_used_after" || name == "union_field") {
                    // Giving a binding the spawner still reads, or a field, does not compile.
                    continue;
                }
                let label = format!(
                    "union_spawn_{name}_{}_{}",
                    if suspending { "sm" } else { "plain" },
                    if given { "give" } else { "borrow" }
                );
                assert_source_balanced(&label, &source, expected);
            }
        }
    }
}

/// Bug caught (High: leak): the callee of a spawned task releases the fields of a flat shape it
/// takes at a `give` position and nulls its frame slot, so the ladder, which read the cell's
/// pointer from that slot, never freed the 128-byte cell (4 allocations, 3 frees). The descriptor
/// now carries the cell's own pointer (`BG_ARG_KIND_OWNED_CELL`).
#[test]
fn a_shape_given_to_a_spawned_suspending_task_has_its_cell_freed_by_the_ladder() {
    assert_fixture_balanced("v0_3_m6_ufcs_background.ynz", "Mon\ndone\n");
    assert_fixture_balanced("v0_3_m6_ufcs_background_multifield.ynz", "Mon\n42\ndone\n");
}
