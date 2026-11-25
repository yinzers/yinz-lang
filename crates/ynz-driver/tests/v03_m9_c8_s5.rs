//! v0.3-M9 C8 slice 5: the spawn ladder and the leaks it left. Each program frees every
//! allocation, clean under AddressSanitizer, at both optimization levels.

mod support;

use support::{
    assert_fixture_balanced_and_asan_clean as assert_fixture_balanced,
    assert_source_balanced_and_asan_clean as assert_source_balanced,
};

/// A program whose `launch` spawns `eat` with the union parameter `held: Circle | Tag`
/// (`give` when `given`), a suspending callee when `suspending`.
fn union_spawn_program(suspending: bool, given: bool, launch_body: &str) -> String {
    format!(
        "shape Circle {{
  radius: Float
}}
shape Tag {{
  name: String
}}
shape Box {{
  choice: Circle | Tag
  id: Int
}}

function makeTag() -> Tag {{
  return {{ name: `made` }}
}}

function eat({give}held: Circle | Tag) -> Nothing {{
  {pause}
  print(777)
}}

function launch() -> Nothing {{
{launch_body}
}}

function entrypoint() -> Nothing {{
  launch()
  wait sleep(300)
}}
",
        give = if given { "give " } else { "" },
        pause = if suspending {
            "wait sleep(1)"
        } else {
            "let ready: Int = 1"
        },
    )
}

const SPAWN_CONSUMED_SHAPE: &str = "  let tag: Tag = { name: `boxed` }
  background eat(tag)";
const SPAWN_FRESH_CALL: &str = "  background eat(makeTag())";
const SPAWN_SHAPE_USED_AFTER: &str = "  let tag: Tag = { name: `boxed` }
  background eat(tag)
  let size: Int = tag.name.count()
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
fn assert_union_spawn_balanced(name: &str, launch_body: &str, suspending: bool, given: bool) {
    let source = union_spawn_program(suspending, given, launch_body);
    let label = format!(
        "union_spawn_{name}_{}_{}",
        if suspending { "sm" } else { "plain" },
        if given { "give" } else { "borrow" }
    );
    assert_source_balanced(&label, &source, "777\n");
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
        ("function entrypoint() -> Nothing", "  wait sleep(150)", "")
    } else {
        (
            "function launch() -> Nothing",
            "  sleepBlocking(150)",
            "function entrypoint() -> Nothing {\n  launch()\n}\n",
        )
    };
    format!(
        "{shape_preamble}
function work(give value: {value_type}) -> Nothing {{
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

/// Bug caught: pre-existing; handle-form spawn leaked the consumed original; The
/// task got a copy and nothing released the binding the spawn consumed (a shape with a string:
/// 2 strings allocated, 1 freed; an array of strings likewise), on the plain path and in a
/// suspending spawner. The original is freed exactly once: counters balance and ASan is clean.
fn assert_handle_form_spawn_balanced(suspending_spawner: bool, consumed: HandleFormConsumed) {
    let path = if suspending_spawner { "sm" } else { "plain" };
    match consumed {
        HandleFormConsumed::Shape => {
            let shape = "shape Barge {
  name: String
  tonnage: Int
}
";
            let program = handle_form_program(
                suspending_spawner,
                shape,
                "Barge",
                "{ name: `Mon${1}`, tonnage: 7 }",
                "  print(value.name)",
            );
            assert_source_balanced(
                &format!("handle_form_shape_{path}"),
                &program,
                "Mon1\ndone\n",
            );
        }
        HandleFormConsumed::Array => {
            let program = handle_form_program(
                suspending_spawner,
                "",
                "Array<String>",
                "[`a${1}`, `b${2}`]",
                "  print(value.count().toString())",
            );
            assert_source_balanced(&format!("handle_form_array_{path}"), &program, "2\ndone\n");
        }
    }
}

enum HandleFormConsumed {
    Shape,
    Array,
}

/// Bug caught (High: leak): a fresh shape (a call result or a literal) a `background` call takes is
/// re-homed by a copy, and `Expr::Background` suppresses its statement-end temporary, so the original's
/// string had no releaser (`c3c_field_assign`: 3 strings allocated, 2 freed). Plain and suspending
/// callee, plain and suspending spawner.
fn assert_fresh_shape_spawn_balanced(
    suspending_callee: bool,
    suspending_spawner: bool,
    fresh: &str,
    expected: &str,
    fresh_name: &str,
) {
    let source = format!(
        "shape Tag {{
  name: String
  id: Int
}}

function makeTag(count: Int) -> Tag {{
  let tag: Tag = {{ name: `tag-${{count}}`, id: 1 }}
  tag.name = `renamed`
  return tag
}}

function show(tag: Tag) -> Nothing {{
  {pause}
  print(tag.id.toString())
}}

function launch() -> Nothing {{
  background show({fresh})
  {hold}
}}

function entrypoint() -> Nothing {{
  launch()
  wait sleep(150)
}}
",
        pause = if suspending_callee {
            "wait sleep(1)"
        } else {
            "let ready: Int = 1"
        },
        hold = if suspending_spawner {
            "wait sleep(1)"
        } else {
            "let ready: Int = 1"
        },
    );
    assert_source_balanced(
        &format!(
            "fresh_shape_spawn_{}_{}_{fresh_name}",
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
        ),
        &source,
        expected,
    );
}

const FRESH_CALL: &str = "makeTag(1)";
const FRESH_LITERAL: &str = "{ name: `lit${2}`, id: 2 }";

/// Bug caught (High: leak): `h.receive()` of a `maybe<T>` or `maybe<T> errors` task result left the
/// received binding's payload (a string, a shape's cell) and, for an errors result, the 16-byte
/// envelope cell with no owner: the binding was treated as borrowing a handle buffer. A plain
/// `maybe<T>` now arrives in a counted cell copied from the return slot, so the binding owns
/// every maybe it receives; a `number` still sits in the handle's buffer.
fn assert_received_maybe_balanced(
    index: usize,
    return_type: &str,
    returned: &str,
    read: &str,
    printed: &str,
) {
    let shape = "shape Part {
  name: String
  qty: Int
}
";
    let source = format!(
        "{shape}
function pick() -> {return_type} {{
  wait sleep(5)
  let items: Array<String> = [`s${{1}}`]
  let parts: Array<Part> = [{{ name: `p${{1}}`, qty: 3 }}]
  return {returned}
}}

function consume() -> Nothing errors {{
  let h = background pick()
  let m = h.receive()
  if (m.exists()) {{
    {read}
  }}
  print(`done`)
}}

function entrypoint() -> Nothing {{
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

/// Bug caught (High: leak, found by the slice 6 spike): a suspending `entrypoint` that waits after
/// `background worker(rows)` leaked its array (2 allocations) even when the worker finished: the
/// spawn copied the consumed binding and only a plain spawner released the original. `give` and
/// inferred-give positions, binding read after the spawn or not.
fn assert_suspending_spawner_array_balanced(
    name: &str,
    parameter: &str,
    after_spawn: &str,
    expected: &str,
) {
    let source = format!(
        "function worker({parameter}: Array<Int>) -> Nothing {{
  wait sleep(5)
  print(rows.count().toString())
}}

function entrypoint() -> Nothing {{
  let rows: Array<Int> = [10, 20, 30]
  background worker(rows)
{after_spawn}  wait sleep(100)
  print(`parent`)
}}
"
    );
    assert_source_balanced(&format!("sm_spawner_array_{name}"), &source, expected);
}

/// Bug caught (High: leak; C7 left it for a suspending statement): the statement-end temporary of an
/// errors-capable call result no binding holds, one program per inner type, with the consumer a plain
/// function (the plain path) and a suspending one (a frame region, since the pair and the view of a
/// union die with the resume invocation). The release picks its form from the one errors-capable
/// release the locals use, for a string, a decimal `number`, a bignum, a union and a `maybe` of one.
/// One inner type of an errors-capable call result: the producer function, the fallback binding
/// and the parameter type the unbound temporary `produce(1).or(fallback)` is passed as.
struct ErrorsInner {
    name: &'static str,
    producer: &'static str,
    fallback: &'static str,
    parameter: &'static str,
}

const ERRORS_STRING: ErrorsInner = ErrorsInner {
    name: "string",
    producer: "function produce(n: Int) -> String errors {
  return `abc${n}`
}",
    fallback: "let fallback: String = `x`",
    parameter: "String",
};
const ERRORS_DECIMAL: ErrorsInner = ErrorsInner {
    name: "decimal",
    producer: "function produce(n: Int) -> Number<20> errors {
  let made: Number<20> = 2.5
  return made
}",
    fallback: "let fallback: Number<20> = 0.5",
    parameter: "Number<20>",
};
const ERRORS_BIGNUM: ErrorsInner = ErrorsInner {
    name: "bignum",
    producer: "function produce(n: Int) -> Number<40> errors {
  let made: Number<40> = 2.5
  return made
}",
    fallback: "let fallback: Number<40> = 0.5",
    parameter: "Number<40>",
};
const ERRORS_UNION: ErrorsInner = ErrorsInner {
    name: "union",
    producer: "function produce(n: Int) -> Thing errors {
  let big: Big = { label: `l${n}`, count: n }
  return big
}",
    fallback: "let spare: Big = { label: `spare`, count: 0 }
  let fallback: Thing = spare",
    parameter: "Thing",
};
const ERRORS_MAYBE_UNION: ErrorsInner = ErrorsInner {
    name: "maybe_union",
    producer: "function produce(n: Int) -> Maybe<Thing> errors {
  let things: Array<Thing> = []
  let big: Big = { label: `l${n}`, count: n }
  things.add(big)
  let found = things.get(0)
  return found
}",
    fallback: "let fallback: Maybe<Thing> = none",
    parameter: "Maybe<Thing>",
};

fn assert_errors_temporary_balanced(inner: &ErrorsInner, suspending: bool) {
    let preamble = "shape Tiny {
  flag: Boolean
}
shape Big {
  label: String
  count: Int
}
shape Thing = Tiny | Big
";
    let ErrorsInner {
        name,
        producer,
        fallback,
        parameter,
    } = inner;
    let argument = "produce(1).or(fallback)";
    let (consumer, call) = if suspending {
        (
            format!(
                "function consume(value: {parameter}) -> Int {{
  wait sleep(1)
  return 1
}}"
            ),
            format!("let result = wait consume({argument})"),
        )
    } else {
        (
            format!(
                "function consume(value: {parameter}) -> Int {{
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

function entrypoint() -> Nothing {{
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

/// Bug caught (High: use after free; C7 caused it, its release frees the spawner's map at scope exit):
/// a spawned `MapEntry` shared the map's own key and value pointers (a string key, a string or array
/// value) instead of copying them, so a task still reading the entry after its spawner returned read
/// freed memory. The spawn copies the halves through the owned-copy operation and the ladder releases
/// the copies. A suspending task and a plain one, string and array values.
fn assert_map_entry_spawn_balanced(
    name: &str,
    map_type: &str,
    entry_value_type: &str,
    value: &str,
    read_value: &str,
    suspending_task: bool,
    expected: &str,
) {
    let pause = if suspending_task {
        "wait sleep(150)"
    } else {
        "sleepBlocking(150)"
    };
    let source = format!(
        "function reader(entry: MapEntry<String, {entry_value_type}>) -> Nothing {{
  {pause}
  print(entry.key)
  {read_value}
}}

function launch() -> Nothing {{
  let table: {map_type} = {{}}
  table.set(`key${{1}}`, {value})
  for (entry in table) {{
    background reader(entry)
  }}
}}

function entrypoint() -> Nothing {{
  launch()
  sleepBlocking(400)
}}
"
    );
    assert_source_balanced(
        &format!(
            "map_entry_copy_{name}_{}",
            if suspending_task { "sm" } else { "plain" }
        ),
        &source,
        expected,
    );
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
  label: String
  count: Int
}}

function eat({give}held: Maybe<{element_type}>) -> Nothing {{
  {pause}
  print(held.exists())
}}

function pick() -> Maybe<{element_type}> {{
  let source: Array<{element_type}> = [{element_value}]
  return source.get(0)
}}

function launch() -> Nothing {{
  let found = pick()
  {spawn}
  {after_spawn}
}}

function entrypoint() -> Nothing {{
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
/// given to a `give` parameter left the payload cell its copy minted). One program per
/// element kind and callee kind, for an argument the spawn consumes, one it gives to a `give`
/// parameter, one the caller reads again (the task takes a copy), and a fresh call result.
macro_rules! maybe_spawn_cases {
    ($($name:ident: $element_type:literal, $element_value:literal, $suspending:literal, $given:literal, $spawn:literal, $after_spawn:literal, $expected:literal;)+) => {
        $(
            // bug: a maybe handed to a spawned task leaks the consumed original or the payload cell its copy minted, silently at exit 0 | tier: High
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
    a_consumed_maybe_string_given_to_a_plain_task_is_released_once: "String", "`kept`.toUpperCase()", false, false, "background eat(found)", "", "true\n";
    a_consumed_maybe_string_given_to_a_suspending_task_is_released_once: "String", "`kept`.toUpperCase()", true, false, "background eat(found)", "", "true\n";
    a_given_maybe_string_given_to_a_plain_task_is_released_once: "String", "`kept`.toUpperCase()", false, true, "background eat(found)", "", "true\n";
    a_given_maybe_string_given_to_a_suspending_task_is_released_once: "String", "`kept`.toUpperCase()", true, true, "background eat(found)", "", "true\n";
    a_copied_maybe_string_given_to_a_plain_task_is_released_once: "String", "`kept`.toUpperCase()", false, false, "background eat(found)", "print(found.exists())", "true\ntrue\n";
    a_copied_maybe_string_given_to_a_suspending_task_is_released_once: "String", "`kept`.toUpperCase()", true, false, "background eat(found)", "print(found.exists())", "true\ntrue\n";
    a_fresh_maybe_string_given_to_a_plain_task_is_released_once: "String", "`kept`.toUpperCase()", false, false, "background eat(pick())", "print(found.exists())", "true\ntrue\n";
    a_consumed_maybe_shape_given_to_a_plain_task_is_released_once: "Big", "{ label: `kept`, count: 3 }", false, false, "background eat(found)", "", "true\n";
    a_consumed_maybe_shape_given_to_a_suspending_task_is_released_once: "Big", "{ label: `kept`, count: 3 }", true, false, "background eat(found)", "", "true\n";
    a_copied_maybe_shape_given_to_a_plain_task_is_released_once: "Big", "{ label: `kept`, count: 3 }", false, false, "background eat(found)", "print(found.exists())", "true\ntrue\n";
    a_copied_maybe_shape_given_to_a_suspending_task_is_released_once: "Big", "{ label: `kept`, count: 3 }", true, false, "background eat(found)", "print(found.exists())", "true\ntrue\n";
    a_fresh_maybe_shape_given_to_a_plain_task_is_released_once: "Big", "{ label: `kept`, count: 3 }", false, false, "background eat(pick())", "print(found.exists())", "true\ntrue\n";
    a_given_maybe_shape_given_to_a_plain_task_is_released_once: "Big", "{ label: `kept`, count: 3 }", false, true, "background eat(found)", "", "true\n";
    a_given_maybe_shape_given_to_a_suspending_task_is_released_once: "Big", "{ label: `kept`, count: 3 }", true, true, "background eat(found)", "", "true\n";
    a_fresh_maybe_shape_given_to_a_given_parameter_is_released_once: "Big", "{ label: `kept`, count: 3 }", false, true, "background eat(pick())", "print(found.exists())", "true\ntrue\n";
    a_given_maybe_decimal_given_to_a_plain_task_is_released_once: "Number<20> ", "2.5", false, true, "background eat(found)", "", "true\n";
    a_given_maybe_decimal_given_to_a_suspending_task_is_released_once: "Number<20> ", "2.5", true, true, "background eat(found)", "", "true\n";
    a_consumed_maybe_decimal_given_to_a_suspending_task_is_released_once: "Number<20> ", "2.5", true, false, "background eat(found)", "", "true\n";
}

/// Bug caught: pre-existing, a leak found by the C8 round-2 review: a string passed at a BORROWED
/// position of a callee that has a `give` parameter elsewhere (`worker(give rows, label)`) was copied
/// for the task, and the ladder's flat free (kept for every position of such a callee) freed nothing
/// of a string, so the task's copy leaked (10 bytes). One program per ownership: the
/// spawner never reads the string again, reads it after the spawn (still its own, copied once), and
/// the callee suspends. Only the two suspending programs fail without the fix; the plain-callee ones
/// pin the path that already released the copy.
macro_rules! borrowed_string_cases {
    ($($name:ident: $suspending:literal, $after_spawn:literal, $expected:literal;)+) => {
        $(
            // bug: a string at a borrowed position of a spawned callee that has a give parameter leaks the task copy, silently at exit 0 | tier: High
            #[test]
            fn $name() {
                let source = format!(
                    "function worker(give rows: Array<Int>, label: String) -> Nothing {{
  {pause}
  print(`${{rows.count()}}${{label}}`)
}}

function launch() -> Nothing {{
  let rows: Array<Int> = [1, 2]
  let label = `abcdefghij`.toUpperCase()
  background worker(rows, label)
  {after_spawn}
}}

function entrypoint() -> Nothing {{
  launch()
  wait sleep(300)
}}
",
                    pause = if $suspending { "wait sleep(1)" } else { "" },
                    after_spawn = $after_spawn,
                );
                assert_source_balanced(stringify!($name), &source, $expected);
            }
        )+
    };
}

borrowed_string_cases! {
    a_borrowed_string_the_spawner_never_reads_again_is_released_once: false, "", "2ABCDEFGHIJ\n";
    a_borrowed_string_the_spawner_reads_again_is_copied_and_released_once: false, "print(label)", "ABCDEFGHIJ\n2ABCDEFGHIJ\n";
    a_borrowed_string_given_to_a_suspending_callee_is_released_once: true, "", "2ABCDEFGHIJ\n";
    a_borrowed_string_read_again_after_a_suspending_spawn_is_released_once: true, "print(label)", "ABCDEFGHIJ\n2ABCDEFGHIJ\n";
}

/// A program whose `entrypoint` is a resume function (it waits) that binds `pick()`, a `maybe` of
/// `element_type` a plain function returned, and runs `body` over it.
fn resume_function_maybe_program(element_type: &str, element_value: &str, body: &str) -> String {
    format!(
        "shape Big {{
  label: String
  count: Int
}}

function pick() -> Maybe<{element_type}> {{
  let source: Array<{element_type}> = [{element_value}]
  return source.get(0)
}}

function eat(give held: Maybe<{element_type}>) -> Nothing {{
  wait sleep(1)
  print(held.exists())
}}

function forward(give held: Maybe<{element_type}>) -> Nothing {{
  background eat(held)
}}

function viaWait() -> Maybe<{element_type}> {{
  wait sleep(1)
  return pick()
}}

function entrypoint() -> Nothing {{
{body}
}}
"
    )
}

/// Bug caught: pre-existing (a C7 guard no C8 slice lifted), found by the C8 round-3 review: a
/// resume function that bound a `maybe` of a string, flat shape or decimal a plain function returned
/// copied the payload and released nothing of the call result (`store_binding_of_read_into` and
/// `remember_fresh_payload_maybe` took it only outside a resume function), so each such binding
/// leaked one payload; a `return` of it from a resume function did the same. One program
/// per element kind and use of the binding: read, given to a plain function, forwarded to a task by a
/// plain function and spawned by a suspending one, returned through a wait.
macro_rules! resume_function_maybe_cases {
    ($($name:ident: $element_type:literal, $element_value:literal, $body:expr, $expected:literal;)+) => {
        $(
            // bug: a resume function binding, giving, spawning or returning a maybe a plain function returned leaks the call result payload, silently at exit 0 | tier: High
            #[test]
            fn $name() {
                let source = resume_function_maybe_program($element_type, $element_value, $body);
                assert_source_balanced(stringify!($name), &source, $expected);
            }
        )+
    };
}

const READ_THEN_WAIT: &str = "  let found = pick()
  print(found.exists())
  wait sleep(1)";
const GIVE_TO_PLAIN: &str = "  let found = pick()
  forward(found)
  wait sleep(300)";
const SPAWN_FROM_SUSPENDING: &str = "  let found = pick()
  background eat(found)
  wait sleep(300)";
const RETURN_THROUGH_WAIT: &str = "  let found = viaWait()
  print(found.exists())";

resume_function_maybe_cases! {
    a_resume_function_binding_a_returned_maybe_string_frees_the_call_result: "String", "`kept`.toUpperCase()", READ_THEN_WAIT, "true\n";
    a_resume_function_binding_a_returned_maybe_shape_frees_the_call_result: "Big", "{ label: `kept`.toUpperCase(), count: 3 }", READ_THEN_WAIT, "true\n";
    a_resume_function_binding_a_returned_maybe_decimal_frees_the_call_result: "Number<20> ", "2.5", READ_THEN_WAIT, "true\n";
    a_resume_function_giving_a_returned_maybe_string_to_a_plain_function_frees_it_once: "String", "`kept`.toUpperCase()", GIVE_TO_PLAIN, "true\n";
    a_resume_function_giving_a_returned_maybe_shape_to_a_plain_function_frees_it_once: "Big", "{ label: `kept`.toUpperCase(), count: 3 }", GIVE_TO_PLAIN, "true\n";
    a_suspending_spawner_of_a_returned_maybe_string_frees_the_call_result: "String", "`kept`.toUpperCase()", SPAWN_FROM_SUSPENDING, "true\n";
    a_suspending_spawner_of_a_returned_maybe_shape_frees_the_call_result: "Big", "{ label: `kept`.toUpperCase(), count: 3 }", SPAWN_FROM_SUSPENDING, "true\n";
    a_resume_function_returning_a_returned_maybe_string_passes_its_payload_on: "String", "`kept`.toUpperCase()", RETURN_THROUGH_WAIT, "true\n";
    a_resume_function_returning_a_returned_maybe_shape_passes_its_payload_on: "Big", "{ label: `kept`.toUpperCase(), count: 3 }", RETURN_THROUGH_WAIT, "true\n";
}

/// A launcher that is itself a resume function (it waits first) spawns `eat` with the `maybe` of
/// `element_type` a plain function returned: the spawned copy and the consumed call result are each
/// freed once, for a `give` or a borrowed parameter and a plain or a suspending callee.
fn suspending_launcher_program(
    element_type: &str,
    element_value: &str,
    given: bool,
    suspending: bool,
) -> String {
    format!(
        "shape Big {{
  label: String
  count: Int
}}

function pick() -> Maybe<{element_type}> {{
  let source: Array<{element_type}> = [{element_value}]
  return source.get(0)
}}

function eat({give}held: Maybe<{element_type}>) -> Nothing {{
  {pause}
  print(held.exists())
}}

function launch() -> Nothing {{
  wait sleep(1)
  let found = pick()
  background eat(found)
}}

function entrypoint() -> Nothing {{
  launch()
  wait sleep(300)
}}
",
        give = if given { "give " } else { "" },
        pause = if suspending { "wait sleep(1)" } else { "" },
    )
}

/// Bug caught: found by the C8 round-3 security review; the same producer as the resume-function
/// cases above: a launcher that waits before it binds and spawns a returned `maybe` leaked the call
/// result (13 bytes for a string, 141 for a shape). Element kind, `give` or borrowed
/// parameter, plain or suspending callee.
macro_rules! suspending_launcher_cases {
    ($($name:ident: $element_type:literal, $element_value:literal, $given:literal, $suspending:literal;)+) => {
        $(
            // bug: a launcher that waits before it binds and spawns a returned maybe leaks the call result, silently at exit 0 | tier: High
            #[test]
            fn $name() {
                let source = suspending_launcher_program(
                    $element_type, $element_value, $given, $suspending,
                );
                assert_source_balanced(stringify!($name), &source, "true\n");
            }
        )+
    };
}

suspending_launcher_cases! {
    a_suspending_launcher_spawning_a_returned_maybe_string_to_a_given_plain_task_frees_every_copy: "String", "`kept`.toUpperCase()", true, false;
    a_suspending_launcher_spawning_a_returned_maybe_string_to_a_borrowed_suspending_task_frees_every_copy: "String", "`kept`.toUpperCase()", false, true;
    a_suspending_launcher_spawning_a_returned_maybe_shape_to_a_given_suspending_task_frees_every_copy: "Big", "{ label: `kept`.toUpperCase(), count: 3 }", true, true;
    a_suspending_launcher_spawning_a_returned_maybe_shape_to_a_borrowed_plain_task_frees_every_copy: "Big", "{ label: `kept`.toUpperCase(), count: 3 }", false, false;
    a_suspending_launcher_spawning_a_returned_maybe_decimal_to_a_given_suspending_task_frees_every_copy: "Number<20> ", "2.5", true, true;
}

/// A program whose `eat` takes the `maybe<Big>` parameter `held` (declared with `modifier`: `give `,
/// `share ` or nothing) and runs `body`; `entry` is `entrypoint`'s body.
fn parameter_maybe_program(modifier: &str, body: &str, entry: &str) -> String {
    format!(
        "shape Big {{
  label: String
  count: Int
}}

function pick() -> Maybe<Big> {{
  let source: Array<Big> = [{{ label: `kept`.toUpperCase(), count: 3 }}]
  return source.get(0)
}}

function eat({modifier}held: Maybe<Big>) -> Nothing {{
{body}
}}

function entrypoint() -> Nothing {{
{entry}
}}
"
    )
}

const MOVE_INTO_LOCAL: &str = "  let mine = held
  print(mine.exists())";
const WAIT_THEN_MOVE_INTO_LOCAL: &str = "  wait sleep(1)
  let mine = held
  print(mine.exists())";
const MOVE_AND_READ_VALUE: &str = "  let mine = held
  let fallback: Big = { label: `x`, count: 0 }
  print(mine.or(fallback).label)";
const WAIT_THEN_READ_VALUE: &str = "  wait sleep(1)
  let fallback: Big = { label: `x`, count: 0 }
  print(held.or(fallback).label)";
const COPY_AND_KEEP: &str = "  let mine = held.copy()
  print(mine.exists())
  print(held.exists())";

/// Bug caught: a C7 defect found after close, on the C8 round-3 review: moving a `maybe<Shape>` into
/// a local copied the payload's string fields although the source is consumed and nothing releases
/// it (`maybe_payload_stable_bits`' moved branch cloned them, a C6 shape from before C7's release of
/// a consumed source), so each such move leaked the strings of the payload. It reached a `give`
/// parameter (`let mine = held`), a local moved to a local and the same on the state-machine path.
/// A suspending callee given a call result also leaked the call result's payload cell (the staged
/// envelope pointed at a cell nothing freed). A `share` or bare parameter keeps copying
/// (`.copy()`): the caller keeps its value, and each side frees its own.
macro_rules! parameter_maybe_cases {
    ($($name:ident: $modifier:literal, $body:expr, $entry:literal, $expected:literal;)+) => {
        $(
            // bug: moving a maybe shape parameter into a local leaks the payload strings or the call result cell, or frees a value the caller keeps, silently at exit 0 | tier: High
            #[test]
            fn $name() {
                let source = parameter_maybe_program($modifier, $body, $entry);
                assert_source_balanced(stringify!($name), &source, $expected);
            }
        )+
    };
}

parameter_maybe_cases! {
    a_given_maybe_shape_moved_into_a_local_is_released_once: "give ", MOVE_INTO_LOCAL, "  eat(pick())", "true\n";
    a_given_maybe_shape_moved_into_a_local_after_a_wait_is_released_once: "give ", WAIT_THEN_MOVE_INTO_LOCAL, "  eat(pick())", "true\n";
    a_given_maybe_shape_from_a_local_moved_into_a_local_is_released_once: "give ", MOVE_INTO_LOCAL, "  let found = pick()\n  eat(found)", "true\n";
    a_given_maybe_shape_from_a_local_moved_after_a_wait_is_released_once: "give ", WAIT_THEN_MOVE_INTO_LOCAL, "  let found = pick()\n  eat(found)", "true\n";
    a_moved_given_maybe_shape_keeps_its_payload_readable: "give ", MOVE_AND_READ_VALUE, "  eat(pick())", "KEPT\n";
    a_suspending_callee_given_a_call_result_maybe_shape_frees_the_payload_cell: "give ", WAIT_THEN_READ_VALUE, "  eat(pick())", "KEPT\n";
    a_moved_local_maybe_shape_is_released_once: "give ", "  print(held.exists())", "  let found = pick()\n  let moved = found\n  print(moved.exists())\n  eat(pick())", "true\ntrue\n";
    a_bare_maybe_shape_parameter_copy_leaves_the_caller_its_own: "", COPY_AND_KEEP, "  let found = pick()\n  eat(found)\n  print(found.exists())", "true\ntrue\ntrue\n";
    a_share_maybe_shape_parameter_copy_leaves_the_caller_its_own: "share ", COPY_AND_KEEP, "  let found = pick()\n  eat(found)\n  print(found.exists())", "true\ntrue\ntrue\n";
}

/// A program whose suspending `eat` takes the `maybe` of `element_type` it is given as an explicit
/// `.copy()` of `pick()`'s result, and reads it after its own `wait`.
fn copied_maybe_program(element_type: &str, element_value: &str, read: &str) -> String {
    format!(
        "shape Big {{
  label: String
  count: Int
}}

function pick() -> Maybe<{element_type}> {{
  let source: Array<{element_type}> = [{element_value}]
  return source.get(0)
}}

function eat(give held: Maybe<{element_type}>) -> Nothing {{
  wait sleep(1)
{read}
}}

function entrypoint() -> Nothing {{
  eat(pick().copy())
}}
"
    )
}

/// Bug caught: found by the C8 round-3 review's probe of what a suspending callee can be given at a
/// `give` position that no call built: the only `maybe` that is not a call result is `none` or an
/// explicit `.copy()`, and the `.copy()`'s two heap cells (the envelope, the shape's or decimal's
/// payload) were never freed once the staged envelope pointed at them. (No UAF: the
/// envelope is staged into a frame region, and a `maybe` literal does not exist.)
macro_rules! copied_maybe_cases {
    ($($name:ident: $element_type:literal, $element_value:literal, $read:literal, $expected:literal;)+) => {
        $(
            // bug: an explicit copy of a maybe given to a suspending callee leaks its envelope and payload cells, silently at exit 0 | tier: High
            #[test]
            fn $name() {
                let source = copied_maybe_program($element_type, $element_value, $read);
                assert_source_balanced(stringify!($name), &source, $expected);
            }
        )+
    };
}

copied_maybe_cases! {
    a_copied_maybe_shape_given_to_a_suspending_callee_frees_both_cells: "Big", "{ label: `kept`.toUpperCase(), count: 3 }", "  let fallback: Big = { label: `x`, count: 0 }\n  print(held.or(fallback).label)", "KEPT\n";
    a_copied_maybe_decimal_given_to_a_suspending_callee_frees_both_cells: "Number<20> ", "2.5", "  print(held.or(1.5))", "2.5\n";
    a_copied_maybe_string_given_to_a_suspending_callee_frees_the_envelope_cell: "String", "`kept`.toUpperCase()", "  print(held.or(`x`))", "KEPT\n";
}

/// One named test per spawn-ladder or release form of the `slice 5 spawn ladder and the leaks it left` run unit; each case is balanced (heap and strings) and ASan clean at both optimization levels.
macro_rules! s5_spawn_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a spawned task's argument, a task result received by a handle or an unbound errors temporary leaks, double frees or is read after free, silently at exit 0 | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

s5_spawn_high_case! {
    // case: Bug caught (High: leak): the value half of a `MapEntry` spawn argument (a shape cell and its string) was never freed, for a plain task and for a suspending one.
    a_mapentry_spawn_argument_frees_its_shape_value_and_string => {
        assert_fixture_balanced("v0_3_m9_c8_s5_mapentry_shape_string_spawn.ynz", "a1\na1\n");
    }
    // case: Bug caught (High: leak): the decimal key and value cells of a `MapEntry` spawn argument.
    a_mapentry_spawn_argument_frees_its_decimal_key_and_value_cells => {
        assert_fixture_balanced("v0_3_m9_c8_s5_mapentry_decimal_spawn.ynz", "2.5\n");
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_consumed_shape_plain_borrow => {
        assert_union_spawn_balanced("consumed_shape", SPAWN_CONSUMED_SHAPE, false, false);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_consumed_shape_plain_give => {
        assert_union_spawn_balanced("consumed_shape", SPAWN_CONSUMED_SHAPE, false, true);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_consumed_shape_sm_borrow => {
        assert_union_spawn_balanced("consumed_shape", SPAWN_CONSUMED_SHAPE, true, false);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_consumed_shape_sm_give => {
        assert_union_spawn_balanced("consumed_shape", SPAWN_CONSUMED_SHAPE, true, true);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_fresh_call_plain_borrow => {
        assert_union_spawn_balanced("fresh_call", SPAWN_FRESH_CALL, false, false);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_fresh_call_plain_give => {
        assert_union_spawn_balanced("fresh_call", SPAWN_FRESH_CALL, false, true);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_fresh_call_sm_borrow => {
        assert_union_spawn_balanced("fresh_call", SPAWN_FRESH_CALL, true, false);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_fresh_call_sm_give => {
        assert_union_spawn_balanced("fresh_call", SPAWN_FRESH_CALL, true, true);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_shape_used_after_plain_borrow => {
        assert_union_spawn_balanced("shape_used_after", SPAWN_SHAPE_USED_AFTER, false, false);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_shape_used_after_sm_borrow => {
        assert_union_spawn_balanced("shape_used_after", SPAWN_SHAPE_USED_AFTER, true, false);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_consumed_union_local_plain_borrow => {
        assert_union_spawn_balanced("consumed_union_local", SPAWN_CONSUMED_UNION_LOCAL, false, false);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_consumed_union_local_plain_give => {
        assert_union_spawn_balanced("consumed_union_local", SPAWN_CONSUMED_UNION_LOCAL, false, true);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_consumed_union_local_sm_borrow => {
        assert_union_spawn_balanced("consumed_union_local", SPAWN_CONSUMED_UNION_LOCAL, true, false);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_consumed_union_local_sm_give => {
        assert_union_spawn_balanced("consumed_union_local", SPAWN_CONSUMED_UNION_LOCAL, true, true);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_union_field_plain_borrow => {
        assert_union_spawn_balanced("union_field", SPAWN_UNION_FIELD, false, false);
    }
    a_union_parameter_of_a_spawned_task_owns_one_copy_and_frees_it_union_field_sm_borrow => {
        assert_union_spawn_balanced("union_field", SPAWN_UNION_FIELD, true, false);
    }
    // case: Bug caught (High: leak): the callee of a spawned task releases the fields of a flat shape it takes at a `give` position and nulls its frame slot, so the ladder, which read the cell's pointer from that slot, never freed the 128-byte cell (4 allocations, 3 frees). The descriptor now carries the cell's own pointer (`BG_ARG_KIND_OWNED_CELL`).
    a_shape_given_to_a_spawned_suspending_task_has_its_cell_freed_by_the_ladder => {
        assert_fixture_balanced("v0_3_m6_ufcs_background.ynz", "Mon\ndone\n");
        assert_fixture_balanced("v0_3_m6_ufcs_background_multifield.ynz", "Mon\n42\ndone\n");
    }
    a_handle_form_spawn_releases_the_consumed_original_once_shape_plain_spawner => {
        assert_handle_form_spawn_balanced(false, HandleFormConsumed::Shape);
    }
    a_handle_form_spawn_releases_the_consumed_original_once_shape_suspending_spawner => {
        assert_handle_form_spawn_balanced(true, HandleFormConsumed::Shape);
    }
    a_handle_form_spawn_releases_the_consumed_original_once_array_plain_spawner => {
        assert_handle_form_spawn_balanced(false, HandleFormConsumed::Array);
    }
    a_handle_form_spawn_releases_the_consumed_original_once_array_suspending_spawner => {
        assert_handle_form_spawn_balanced(true, HandleFormConsumed::Array);
    }
    a_fresh_shape_a_spawn_copies_has_its_original_released_plain_callee_plain_spawner_call => {
        assert_fresh_shape_spawn_balanced(false, false, FRESH_CALL, "1\n", "call");
    }
    a_fresh_shape_a_spawn_copies_has_its_original_released_plain_callee_plain_spawner_literal => {
        assert_fresh_shape_spawn_balanced(false, false, FRESH_LITERAL, "2\n", "literal");
    }
    a_fresh_shape_a_spawn_copies_has_its_original_released_plain_callee_suspending_spawner_call => {
        assert_fresh_shape_spawn_balanced(false, true, FRESH_CALL, "1\n", "call");
    }
    a_fresh_shape_a_spawn_copies_has_its_original_released_plain_callee_suspending_spawner_literal => {
        assert_fresh_shape_spawn_balanced(false, true, FRESH_LITERAL, "2\n", "literal");
    }
    a_fresh_shape_a_spawn_copies_has_its_original_released_suspending_callee_plain_spawner_call => {
        assert_fresh_shape_spawn_balanced(true, false, FRESH_CALL, "1\n", "call");
    }
    a_fresh_shape_a_spawn_copies_has_its_original_released_suspending_callee_plain_spawner_literal => {
        assert_fresh_shape_spawn_balanced(true, false, FRESH_LITERAL, "2\n", "literal");
    }
    a_fresh_shape_a_spawn_copies_has_its_original_released_suspending_callee_suspending_spawner_call => {
        assert_fresh_shape_spawn_balanced(true, true, FRESH_CALL, "1\n", "call");
    }
    a_fresh_shape_a_spawn_copies_has_its_original_released_suspending_callee_suspending_spawner_literal => {
        assert_fresh_shape_spawn_balanced(true, true, FRESH_LITERAL, "2\n", "literal");
    }
    a_maybe_received_from_a_handle_is_owned_by_its_binding_string => {
        assert_received_maybe_balanced(0, "Maybe<String>", "items.get(0)", "print(m.value)", "s1\n");
    }
    a_maybe_received_from_a_handle_is_owned_by_its_binding_string_errors => {
        assert_received_maybe_balanced(1, "Maybe<String> errors", "items.get(0)", "print(m.value)", "s1\n");
    }
    a_maybe_received_from_a_handle_is_owned_by_its_binding_shape => {
        assert_received_maybe_balanced(2, "Maybe<Part>", "parts.get(0)", "print(m.value.name)", "p1\n");
    }
    a_maybe_received_from_a_handle_is_owned_by_its_binding_shape_errors => {
        assert_received_maybe_balanced(3, "Maybe<Part> errors", "parts.get(0)", "print(m.value.name)", "p1\n");
    }
    a_maybe_received_from_a_handle_is_owned_by_its_binding_int => {
        assert_received_maybe_balanced(4, "Maybe<Int>", "[1].get(0)", "print(m.value.toString())", "1\n");
    }
    a_maybe_received_from_a_handle_is_owned_by_its_binding_int_errors => {
        assert_received_maybe_balanced(5, "Maybe<Int> errors", "[1].get(0)", "print(m.value.toString())", "1\n");
    }
    // case: A `number` received from a handle sits in the handle's own buffer: the binding frees nothing of it, and the program still balances (no double free of the buffer).
    a_number_received_from_a_handle_is_not_freed_by_its_binding => {
    let source = "function pick() -> Number<20> {
  wait sleep(5)
  return 2.5
}

function pickFail() -> Number<20> errors {
  wait sleep(5)
  return 3.5
}

function consume() -> Nothing errors {
  let h = background pick()
  let m = h.receive()
  print(m)
  let g = background pickFail()
  let n = g.receive()
  print(n)
  print(`done`)
}

function entrypoint() -> Nothing {
  let r = consume()
  if (r.failed()) {
    print(`failed`)
  }
}
";
    assert_source_balanced("handle_receive_number", source, "2.5\n3.5\ndone\n");
    }
    a_suspending_spawner_releases_the_array_a_spawn_consumed_given => {
        assert_suspending_spawner_array_balanced("given", "give rows", "", "3\nparent\n");
    }
    a_suspending_spawner_releases_the_array_a_spawn_consumed_inferred => {
        assert_suspending_spawner_array_balanced("inferred", "rows", "", "3\nparent\n");
    }
    a_suspending_spawner_releases_the_array_a_spawn_consumed_used_after => {
        assert_suspending_spawner_array_balanced(
            "used_after",
            "rows",
            "  print(rows.count().toString())\n",
            "3\n3\nparent\n",
        );
    }
    // case: Bug caught (High: leak): an errors-capable call result no binding holds, in a statement that suspends, had no frame home and was never released (S4b leftover): `wait slow(label(1).or(..))` allocated 2 strings and freed 1. String, array, map and shape payloads, and a `nothing` one.
    an_unbound_errors_temporary_of_a_suspending_statement_is_released => {
    let source = "shape Part {
  name: String
  qty: Int
}

function label(n: Int) -> String errors {
  return `abc${n}`
}

function listing(n: Int) -> Array<String> errors {
  return [`a${n}`, `b`]
}

function table(n: Int) -> Map<String, String> errors {
  return { `k`: `v${n}` }
}

function part(n: Int) -> Part errors {
  let made: Part = { name: `p${n}`, qty: n }
  return made
}

function slow(first: Int) -> Int {
  wait sleep(1)
  return first
}

function entrypoint() -> Nothing {
  let fallbackText: String = `x`
  let fallbackList: Array<String> = []
  let fallbackTable: Map<String, String> = {}
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
    // case: Bug caught (High: leak; C7 leftover): a `.copy()` of a type parameter `T` no binding holds, in an instance whose `T` is a `maybe`, mints an envelope cell that `open_temporaries` did not release (the filter listed string, array, map and shape instances only): `burn(found)` of a `maybe<string>` allocated 6 heap cells and freed 4. A union or channel `T` cannot reach it: its `.copy()` is refused before codegen (parked in the report).
    a_type_parameter_copy_instantiated_as_a_maybe_is_released => {
    let source = "shape Row {
  name: String
}

function burn<T>(share value: T) -> Nothing {
  let items: Array<T> = []
  value.copy()
  items.get(0).or(value.copy())
  print(1)
}

function entrypoint() -> Nothing {
  let n = 4
  let names: Array<String> = [`a ${n}`]
  let found: Maybe<String> = names.get(0)
  burn(found)
  let numbers: Array<Int> = [n]
  let number: Maybe<Int> = numbers.get(0)
  burn(number)
  let rows: Array<Row> = [{ name: `b ${n}` }]
  let row: Maybe<Row> = rows.get(0)
  burn(row)
  print(2)
}
";
    assert_source_balanced("type_parameter_copy_maybe", source, "1\n1\n1\n2\n");
    }
    an_unbound_errors_temporary_is_released_for_every_inner_type_string_plain => {
        assert_errors_temporary_balanced(&ERRORS_STRING, false);
    }
    an_unbound_errors_temporary_is_released_for_every_inner_type_string_suspending => {
        assert_errors_temporary_balanced(&ERRORS_STRING, true);
    }
    an_unbound_errors_temporary_is_released_for_every_inner_type_decimal_plain => {
        assert_errors_temporary_balanced(&ERRORS_DECIMAL, false);
    }
    an_unbound_errors_temporary_is_released_for_every_inner_type_decimal_suspending => {
        assert_errors_temporary_balanced(&ERRORS_DECIMAL, true);
    }
    an_unbound_errors_temporary_is_released_for_every_inner_type_bignum_plain => {
        assert_errors_temporary_balanced(&ERRORS_BIGNUM, false);
    }
    an_unbound_errors_temporary_is_released_for_every_inner_type_bignum_suspending => {
        assert_errors_temporary_balanced(&ERRORS_BIGNUM, true);
    }
    an_unbound_errors_temporary_is_released_for_every_inner_type_union_plain => {
        assert_errors_temporary_balanced(&ERRORS_UNION, false);
    }
    an_unbound_errors_temporary_is_released_for_every_inner_type_union_suspending => {
        assert_errors_temporary_balanced(&ERRORS_UNION, true);
    }
    an_unbound_errors_temporary_is_released_for_every_inner_type_maybe_union_plain => {
        assert_errors_temporary_balanced(&ERRORS_MAYBE_UNION, false);
    }
    an_unbound_errors_temporary_is_released_for_every_inner_type_maybe_union_suspending => {
        assert_errors_temporary_balanced(&ERRORS_MAYBE_UNION, true);
    }
    a_spawned_map_entry_owns_a_copy_of_its_key_and_value_string_value_plain => {
        assert_map_entry_spawn_balanced(
            "string_value",
            "Map<String, String>",
            "String",
            "`value${2}`",
            "print(entry.value)",
            false,
            "key1\nvalue2\n",
        );
    }
    a_spawned_map_entry_owns_a_copy_of_its_key_and_value_string_value_sm => {
        assert_map_entry_spawn_balanced(
            "string_value",
            "Map<String, String>",
            "String",
            "`value${2}`",
            "print(entry.value)",
            true,
            "key1\nvalue2\n",
        );
    }
    a_spawned_map_entry_owns_a_copy_of_its_key_and_value_array_value_plain => {
        assert_map_entry_spawn_balanced(
            "array_value",
            "Map<String, Array<Int>>",
            "Array<Int> ",
            "[7, 8, 9]",
            "print(entry.value.count().toString())",
            false,
            "key1\n3\n",
        );
    }
    a_spawned_map_entry_owns_a_copy_of_its_key_and_value_array_value_sm => {
        assert_map_entry_spawn_balanced(
            "array_value",
            "Map<String, Array<Int>>",
            "Array<Int> ",
            "[7, 8, 9]",
            "print(entry.value.count().toString())",
            true,
            "key1\n3\n",
        );
    }
}
