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

/// The handle-form spawn `let task = background work(value)` of a consumed binding, in a plain
/// spawner (it holds the thread with `sleepBlocking` until the task is done) and in a suspending
/// one. `value_type` and `value_literal` build the argument; the callee `give`s it and suspends.
fn handle_form_program(
    suspending_spawner: bool,
    shape_preamble: &str,
    value_type: &str,
    value_literal: &str,
    callee_body: &str,
) -> String {
    let (spawner_signature, wait_for_task, entry_call) = if suspending_spawner {
        ("function entrypoint() -> nothing", "  wait sleep(150)", "")
    } else {
        (
            "function launch() -> nothing",
            "  sleepBlocking(150)",
            "function entrypoint() -> nothing {\n  launch()\n}\n",
        )
    };
    format!(
        "{shape_preamble}
function work(give value: {value_type}) -> nothing {{
  wait sleep(20)
{callee_body}
}}

{spawner_signature} {{
  let value: {value_type} = {value_literal}
  let task = background work(value)
{wait_for_task}
  print(`done`)
}}
{entry_call}"
    )
}

/// Bug caught: pre-existing; handle-form spawn leaked the consumed original; tier Medium. The
/// task got a copy and nothing released the binding the spawn consumed (a shape with a string:
/// 2 strings allocated, 1 freed; an array of strings likewise), on the plain path and in a
/// suspending spawner. The original is freed exactly once: counters balance and ASan is clean.
#[test]
fn a_handle_form_spawn_releases_the_consumed_original_once() {
    let shape = "shape Barge {
  name: string
  tonnage: int
}
";
    for suspending_spawner in [false, true] {
        let path = if suspending_spawner { "sm" } else { "plain" };
        let shape_program = handle_form_program(
            suspending_spawner,
            shape,
            "Barge",
            "{ name: `Mon${1}`, tonnage: 7 }",
            "  print(value.name)",
        );
        assert_source_balanced(
            &format!("handle_form_shape_{path}"),
            &shape_program,
            "Mon1\ndone\n",
        );
        let array_program = handle_form_program(
            suspending_spawner,
            "",
            "array<string>",
            "[`a${1}`, `b${2}`]",
            "  print(value.count().toString())",
        );
        assert_source_balanced(
            &format!("handle_form_array_{path}"),
            &array_program,
            "2\ndone\n",
        );
    }
}

/// Bug caught (High: leak): a fresh shape (a call result or a literal) a `background` call takes is
/// re-homed by a copy, and `Expr::Background` suppresses its statement-end temporary, so the original's
/// string had no releaser (`c3c_field_assign`: 3 strings allocated, 2 freed). Plain and suspending
/// callee, plain and suspending spawner.
#[test]
fn a_fresh_shape_a_spawn_copies_has_its_original_released() {
    for suspending_callee in [false, true] {
        for suspending_spawner in [false, true] {
            for fresh in ["makeTag(1)", "{ name: `lit${2}`, id: 2 }"] {
                let source = format!(
                    "shape Tag {{
  name: string
  id: int
}}

function makeTag(count: int) -> Tag {{
  let tag: Tag = {{ name: `tag-${{count}}`, id: 1 }}
  tag.name = `renamed`
  return tag
}}

function show(tag: Tag) -> nothing {{
  {pause}
  print(tag.id.toString())
}}

function launch() -> nothing {{
  background show({fresh})
  {hold}
}}

function entrypoint() -> nothing {{
  launch()
  wait sleep(150)
}}
",
                    pause = if suspending_callee {
                        "wait sleep(1)"
                    } else {
                        "let ready: int = 1"
                    },
                    hold = if suspending_spawner {
                        "wait sleep(1)"
                    } else {
                        "let ready: int = 1"
                    },
                );
                let expected = if fresh == "makeTag(1)" { "1\n" } else { "2\n" };
                assert_source_balanced(
                    &format!(
                        "fresh_shape_spawn_{}_{}_{}",
                        if suspending_callee {
                            "smcallee"
                        } else {
                            "plaincallee"
                        },
                        if suspending_spawner {
                            "smspawner"
                        } else {
                            "plainspawner"
                        },
                        if fresh == "makeTag(1)" {
                            "call"
                        } else {
                            "literal"
                        },
                    ),
                    &source,
                    expected,
                );
            }
        }
    }
}

/// Bug caught (High: leak): `h.receive()` of a `maybe<T>` or `maybe<T> errors` task result left the
/// received binding's payload (a string, a shape's cell) and, for an errors result, the 16-byte
/// envelope cell with no owner: the binding was treated as borrowing a handle buffer. A plain
/// `maybe<T>` now arrives in a counted cell copied from the return slot, so the binding owns
/// every maybe it receives; a `number` still sits in the handle's buffer.
#[test]
fn a_maybe_received_from_a_handle_is_owned_by_its_binding() {
    let shape = "shape Part {
  name: string
  qty: int
}
";
    let cases = [
        ("maybe<string>", "items.get(0)", "print(m.value)", "s1\n"),
        (
            "maybe<string> errors",
            "items.get(0)",
            "print(m.value)",
            "s1\n",
        ),
        ("maybe<Part>", "parts.get(0)", "print(m.value.name)", "p1\n"),
        (
            "maybe<Part> errors",
            "parts.get(0)",
            "print(m.value.name)",
            "p1\n",
        ),
        (
            "maybe<int>",
            "[1].get(0)",
            "print(m.value.toString())",
            "1\n",
        ),
        (
            "maybe<int> errors",
            "[1].get(0)",
            "print(m.value.toString())",
            "1\n",
        ),
    ];
    for (index, (return_type, returned, read, printed)) in cases.into_iter().enumerate() {
        let source = format!(
            "{shape}
function pick() -> {return_type} {{
  wait sleep(5)
  let items: array<string> = [`s${{1}}`]
  let parts: array<Part> = [{{ name: `p${{1}}`, qty: 3 }}]
  return {returned}
}}

function consume() -> nothing errors {{
  let h = background pick()
  let m = h.receive()
  if (m.exists()) {{
    {read}
  }}
  print(`done`)
}}

function entrypoint() -> nothing {{
  let r = consume()
  if (r.failed()) {{
    print(`failed`)
  }}
}}
"
        );
        assert_source_balanced(
            &format!("handle_receive_maybe_{index}"),
            &source,
            &format!("{printed}done\n"),
        );
    }
}

/// A `number` received from a handle sits in the handle's own buffer: the binding frees nothing of
/// it, and the program still balances (no double free of the buffer).
#[test]
fn a_number_received_from_a_handle_is_not_freed_by_its_binding() {
    let source = "function pick() -> number<20> {
  wait sleep(5)
  return 2.5
}

function pickFail() -> number<20> errors {
  wait sleep(5)
  return 3.5
}

function consume() -> nothing errors {
  let h = background pick()
  let m = h.receive()
  print(m)
  let g = background pickFail()
  let n = g.receive()
  print(n)
  print(`done`)
}

function entrypoint() -> nothing {
  let r = consume()
  if (r.failed()) {
    print(`failed`)
  }
}
";
    assert_source_balanced("handle_receive_number", source, "2.5\n3.5\ndone\n");
}
