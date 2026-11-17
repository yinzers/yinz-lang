//! v0.3-M9 C8 slice 5: the spawn ladder and the leaks it left. Each program frees every
//! allocation, clean under AddressSanitizer, at both optimization levels.

mod support;

use support::{
    assert_fixture_balanced_and_asan_clean as assert_fixture_balanced,
    assert_source_balanced_and_asan_clean as assert_source_balanced,
};

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

/// Bug caught (High: leak, found by the slice 6 spike): a suspending `entrypoint` that waits after
/// `background worker(rows)` leaked its array (2 allocations) even when the worker finished: the
/// spawn copied the consumed binding and only a plain spawner released the original. `give` and
/// inferred-give positions, binding read after the spawn or not.
#[test]
fn a_suspending_spawner_releases_the_array_a_spawn_consumed() {
    for (name, parameter, after_spawn) in [
        ("given", "give rows", ""),
        ("inferred", "rows", ""),
        ("used_after", "rows", "  print(rows.count().toString())\n"),
    ] {
        let source = format!(
            "function worker({parameter}: array<int>) -> nothing {{
  wait sleep(5)
  print(rows.count().toString())
}}

function entrypoint() -> nothing {{
  let rows: array<int> = [10, 20, 30]
  background worker(rows)
{after_spawn}  wait sleep(100)
  print(`parent`)
}}
"
        );
        let expected = if after_spawn.is_empty() {
            "3\nparent\n"
        } else {
            "3\n3\nparent\n"
        };
        assert_source_balanced(&format!("sm_spawner_array_{name}"), &source, expected);
    }
}

/// Bug caught (High: leak): an errors-capable call result no binding holds, in a statement that
/// suspends, had no frame home and was never released (S4b leftover): `wait slow(label(1).or(..))`
/// allocated 2 strings and freed 1. String, array, map and shape payloads, and a `nothing` one.
#[test]
fn an_unbound_errors_temporary_of_a_suspending_statement_is_released() {
    let source = "shape Part {
  name: string
  qty: int
}

function label(n: int) -> string errors {
  return `abc${n}`
}

function listing(n: int) -> array<string> errors {
  return [`a${n}`, `b`]
}

function table(n: int) -> map<string, string> errors {
  return { `k`: `v${n}` }
}

function part(n: int) -> Part errors {
  let made: Part = { name: `p${n}`, qty: n }
  return made
}

function slow(first: int) -> int {
  wait sleep(1)
  return first
}

function entrypoint() -> nothing {
  let fallbackText: string = `x`
  let fallbackList: array<string> = []
  let fallbackTable: map<string, string> = {}
  let fallbackPart: Part = { name: `f`, qty: 0 }
  let first = wait slow(label(1).or(fallbackText).count())
  let second = wait slow(listing(2).or(fallbackList).count())
  let third = wait slow(table(3).or(fallbackTable).count())
  let fourth = wait slow(part(4).or(fallbackPart).qty)
  print(first.toString())
  print(second.toString())
  print(third.toString())
  print(fourth.toString())
}
";
    assert_source_balanced("errors_temporary_suspending", source, "4\n2\n1\n4\n");
}

/// Bug caught (High: leak; C7 leftover): a `.copy()` of a type parameter `T` no binding holds, in
/// an instance whose `T` is a `maybe`, mints an envelope cell that `open_temporaries` did not
/// release (the filter listed string, array, map and shape instances only): `burn(found)` of a
/// `maybe<string>` allocated 6 heap cells and freed 4. A union or channel `T` cannot reach it: its
/// `.copy()` is refused before codegen (parked in the report).
#[test]
fn a_type_parameter_copy_instantiated_as_a_maybe_is_released() {
    let source = "shape Row {
  name: string
}

function burn<T>(share value: T) -> nothing {
  let items: array<T> = []
  value.copy()
  items.get(0).or(value.copy())
  print(1)
}

function entrypoint() -> nothing {
  let n = 4
  let names: array<string> = [`a ${n}`]
  let found: maybe<string> = names.get(0)
  burn(found)
  let numbers: array<int> = [n]
  let number: maybe<int> = numbers.get(0)
  burn(number)
  let rows: array<Row> = [{ name: `b ${n}` }]
  let row: maybe<Row> = rows.get(0)
  burn(row)
  print(2)
}
";
    assert_source_balanced("type_parameter_copy_maybe", source, "1\n1\n1\n2\n");
}

/// Bug caught (High: leak; C7 left it for a suspending statement): the statement-end temporary of an
/// errors-capable call result no binding holds, one program per inner type, with the consumer a plain
/// function (the plain path) and a suspending one (a frame region, since the pair and the view of a
/// union die with the resume invocation). The release picks its form from the one errors-capable
/// release the locals use, for a string, a decimal `number`, a bignum, a union and a `maybe` of one.
#[test]
fn an_unbound_errors_temporary_is_released_for_every_inner_type() {
    let preamble = "shape Tiny {
  flag: boolean
}
shape Big {
  label: string
  count: int
}
shape Thing = Tiny | Big
";
    // (name, producer functions, fallback setup, the unbound temporary as an argument, parameter type)
    let cases = [
        (
            "string",
            "function produce(n: int) -> string errors {
  return `abc${n}`
}",
            "let fallback: string = `x`",
            "produce(1).or(fallback)",
            "string",
        ),
        (
            "decimal",
            "function produce(n: int) -> number<20> errors {
  let made: number<20> = 2.5
  return made
}",
            "let fallback: number<20> = 0.5",
            "produce(1).or(fallback)",
            "number<20>",
        ),
        (
            "bignum",
            "function produce(n: int) -> number<40> errors {
  let made: number<40> = 2.5
  return made
}",
            "let fallback: number<40> = 0.5",
            "produce(1).or(fallback)",
            "number<40>",
        ),
        (
            "union",
            "function produce(n: int) -> Thing errors {
  let big: Big = { label: `l${n}`, count: n }
  return big
}",
            "let spare: Big = { label: `spare`, count: 0 }
  let fallback: Thing = spare",
            "produce(1).or(fallback)",
            "Thing",
        ),
        (
            "maybe_union",
            "function produce(n: int) -> maybe<Thing> errors {
  let things: array<Thing> = []
  let big: Big = { label: `l${n}`, count: n }
  things.add(big)
  let found = things.get(0)
  return found
}",
            "let fallback: maybe<Thing> = none",
            "produce(1).or(fallback)",
            "maybe<Thing>",
        ),
    ];
    for (name, producer, fallback, argument, parameter) in cases {
        for suspending in [false, true] {
            let (consumer, call) = if suspending {
                (
                    format!(
                        "function consume(value: {parameter}) -> int {{
  wait sleep(1)
  return 1
}}"
                    ),
                    format!("let result = wait consume({argument})"),
                )
            } else {
                (
                    format!(
                        "function consume(value: {parameter}) -> int {{
  return 1
}}"
                    ),
                    format!("let result = consume({argument})"),
                )
            };
            let source = format!(
                "{preamble}
{producer}

{consumer}

function entrypoint() -> nothing {{
  {fallback}
  {call}
  print(result.toString())
}}
"
            );
            assert_source_balanced(
                &format!(
                    "errors_temporary_{name}_{}",
                    if suspending { "suspending" } else { "plain" }
                ),
                &source,
                "1\n",
            );
        }
    }
}

/// Bug caught (High: use after free; C7 caused it, its release frees the spawner's map at scope exit):
/// a spawned `MapEntry` shared the map's own key and value pointers (a string key, a string or array
/// value) instead of copying them, so a task still reading the entry after its spawner returned read
/// freed memory. The spawn copies the halves through the owned-copy operation and the ladder releases
/// the copies. A suspending task and a plain one, string and array values.
#[test]
fn a_spawned_map_entry_owns_a_copy_of_its_key_and_value() {
    let cases = [
        (
            "string_value",
            "map<string, string>",
            "`value${2}`",
            "print(entry.value)",
            "key1\nvalue2\n",
        ),
        (
            "array_value",
            "map<string, array<int>>",
            "[7, 8, 9]",
            "print(entry.value.count().toString())",
            "key1\n3\n",
        ),
    ];
    for (name, map_type, value, read_value, expected) in cases {
        for suspending_task in [false, true] {
            let pause = if suspending_task {
                "wait sleep(150)"
            } else {
                "sleepBlocking(150)"
            };
            let source = format!(
                "function reader(entry: MapEntry<string, {value_type}>) -> nothing {{
  {pause}
  print(entry.key)
  {read_value}
}}

function launch() -> nothing {{
  let table: {map_type} = {{}}
  table.set(`key${{1}}`, {value})
  for (entry in table) {{
    background reader(entry)
  }}
}}

function entrypoint() -> nothing {{
  launch()
  sleepBlocking(400)
}}
",
                value_type = map_type
                    .trim_start_matches("map<string, ")
                    .trim_end_matches('>'),
            );
            // The array-valued map ends in `>>`, which the trim above leaves one `>` short.
            let source = if name == "array_value" {
                source.replace(
                    "MapEntry<string, array<int>",
                    "MapEntry<string, array<int> >",
                )
            } else {
                source
            };
            assert_source_balanced(
                &format!(
                    "map_entry_copy_{name}_{}",
                    if suspending_task { "sm" } else { "plain" }
                ),
                &source,
                expected,
            );
        }
    }
}

/// A program whose `launch` spawns `eat` with a `maybe<element>` parameter (`give` when `given`,
/// suspending when `suspending`). `spawn` is the spawn statement over `found`, a binding of what
/// `pick()` returned; `after_spawn` reads `found` again when the spawn must copy it.
fn maybe_spawn_program(
    element_type: &str,
    element_value: &str,
    suspending: bool,
    given: bool,
    spawn: &str,
    after_spawn: &str,
) -> String {
    format!(
        "shape Big {{
  label: string
  count: int
}}

function eat({give}held: maybe<{element_type}>) -> nothing {{
  {pause}
  print(held.exists())
}}

function pick() -> maybe<{element_type}> {{
  let source: array<{element_type}> = [{element_value}]
  return source.get(0)
}}

function launch() -> nothing {{
  let found = pick()
  {spawn}
  {after_spawn}
}}

function entrypoint() -> nothing {{
  launch()
  wait sleep(300)
}}
",
        give = if given { "give " } else { "" },
        pause = if suspending { "wait sleep(1)" } else { "" },
    )
}

/// Bug caught: pre-existing, a leak found by the C8 round-2 review: `prepare_bg_arg_for_ctx` released
/// the consumed original of a `background` argument after its copy only for an array, a map and a
/// shape, so a `maybe` handed to a task left its original unreleased (and a `maybe` of a shape
/// given to a `give` parameter left the payload cell its copy minted). Tier Medium. One program per
/// element kind and callee kind, for an argument the spawn consumes, one it gives to a `give`
/// parameter, one the caller reads again (the task takes a copy), and a fresh call result.
macro_rules! maybe_spawn_cases {
    ($($name:ident: $element_type:literal, $element_value:literal, $suspending:literal, $given:literal, $spawn:literal, $after_spawn:literal, $expected:literal;)+) => {
        $(
            #[test]
            fn $name() {
                let source = maybe_spawn_program(
                    $element_type, $element_value, $suspending, $given, $spawn, $after_spawn,
                );
                assert_source_balanced(stringify!($name), &source, $expected);
            }
        )+
    };
}

maybe_spawn_cases! {
    a_consumed_maybe_string_given_to_a_plain_task_is_released_once: "string", "`kept`.toUpperCase()", false, false, "background eat(found)", "", "true\n";
    a_consumed_maybe_string_given_to_a_suspending_task_is_released_once: "string", "`kept`.toUpperCase()", true, false, "background eat(found)", "", "true\n";
    a_given_maybe_string_given_to_a_plain_task_is_released_once: "string", "`kept`.toUpperCase()", false, true, "background eat(found)", "", "true\n";
    a_given_maybe_string_given_to_a_suspending_task_is_released_once: "string", "`kept`.toUpperCase()", true, true, "background eat(found)", "", "true\n";
    a_copied_maybe_string_given_to_a_plain_task_is_released_once: "string", "`kept`.toUpperCase()", false, false, "background eat(found)", "print(found.exists())", "true\ntrue\n";
    a_copied_maybe_string_given_to_a_suspending_task_is_released_once: "string", "`kept`.toUpperCase()", true, false, "background eat(found)", "print(found.exists())", "true\ntrue\n";
    a_fresh_maybe_string_given_to_a_plain_task_is_released_once: "string", "`kept`.toUpperCase()", false, false, "background eat(pick())", "print(found.exists())", "true\ntrue\n";
    a_consumed_maybe_shape_given_to_a_plain_task_is_released_once: "Big", "{ label: `kept`, count: 3 }", false, false, "background eat(found)", "", "true\n";
    a_consumed_maybe_shape_given_to_a_suspending_task_is_released_once: "Big", "{ label: `kept`, count: 3 }", true, false, "background eat(found)", "", "true\n";
    a_copied_maybe_shape_given_to_a_plain_task_is_released_once: "Big", "{ label: `kept`, count: 3 }", false, false, "background eat(found)", "print(found.exists())", "true\ntrue\n";
    a_copied_maybe_shape_given_to_a_suspending_task_is_released_once: "Big", "{ label: `kept`, count: 3 }", true, false, "background eat(found)", "print(found.exists())", "true\ntrue\n";
    a_fresh_maybe_shape_given_to_a_plain_task_is_released_once: "Big", "{ label: `kept`, count: 3 }", false, false, "background eat(pick())", "print(found.exists())", "true\ntrue\n";
}
