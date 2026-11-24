// v0.3-M9 C5a (plan #17): the persisted transfer record, the loop-carried give rejection and the
// store-hands-it-over rule.
//
// WHY (High tier: ownership / data integrity): spike S3 found six programs typeck accepts that
// print correctly today and become use-after-free under ANY scope-exit release pass, because the
// transfer record missed the transfer. They are one class: a heap value stored without a sink,
// plus multi-root alias. Decided (Patrick, on the Chief Architect's recommendation, "deny and
// teach"): storing a heap value HANDS IT OVER, like a give or a send, so the one transfer
// decision (`check_transfer`) sees every store, the record notes the transfer, and reading the
// source afterwards is a compile error. Each test below asserts typeck's recorded ownership facts
// directly (not a program's output): there is no release pass yet, so the miss is only
// observable here.
//
//   hole  program                             expected fact
//   t3    outer binding given in a loop       rejected: used again on the next pass (Consumed)
//   t6b   bag.items = numbers; return bag     `numbers` stored in `bag.items`; `bag` by return
//   g1    container.add(numbers)              `numbers` stored in `container`; `container` by
//         return container                    return
//   g8    container[0] = numbers              `numbers` stored in `container[0]`; `container`
//         return container                    by return
//   g7    acc = per (per-iteration local)     `per` stored in `acc`; `acc` anchors its own class
//   t7    { first: numbers, second: moreNumbers } `numbers`, `moreNumbers` each stored in the literal
//
// Each fact test failed on the base commit (the record had no `Escape::Stored`: a store joined
// two classes instead); the rejection tests failed because typeck accepted the program.
//
// Tests are tables of NAMED cases, one named test per program, split only where the cases need a
// different harness: `record_cases!` (the record's facts for one function), the `cases!` decision
// tables in `support` (accepted, or rejected with the use-after-give / use-after-store text),
// the spawn inference pair, and `internal_error_cases!` (the recorder driven through its public
// API). The accepted-program guards and `a_given_then_revived_binding_keeps_its_given_escape` were
// green on arrival on purpose: they pin over-rejection and the record's flow-insensitivity
// against the path-aware revive (`revive_paths.rs`), which must not leak into the record (a
// record that forgot the give would let a release pass free the class early).

#[macro_use]
mod support;

use support::{
    assert_accepted, assert_rejected_with, assert_use_after_give, error_diagnostics, errors, run,
    EAT,
};
use ynz_diagnostics::DiagnosticKind;
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::binding_names::BindingNames;
use ynz_typeck::check::BgOwnership;
use ynz_typeck::non_owned_reads::NonOwnedReads;
use ynz_typeck::transfer_record::{
    Escape, FunctionTransfers, ReleaseAnchor, RoleEvidence, TransferRecorder,
};
use ynz_typeck::{check_query, CheckOutput};

/// Check `main_source` (as `main.ynz`) in a project that also holds `library_source` (as
/// `lib.ynz`, imported with ``import { .. } from `lib` ``). The files must exist on disk for the
/// import to resolve; each call gets its own `TempDir`, so parallel tests never share one and the
/// directory is removed even when the check panics.
fn run_with_library(library_source: &str, main_source: &str) -> CheckOutput {
    let project_directory = tempfile::tempdir().expect("create the project directory");
    std::fs::write(
        project_directory.path().join("yinz.toml"),
        "[project]\nname = \"test\"\n",
    )
    .expect("write yinz.toml");
    let library_path = project_directory.path().join("lib.ynz");
    let main_path = project_directory.path().join("main.ynz");
    std::fs::write(&library_path, library_source).expect("write lib.ynz");
    std::fs::write(&main_path, main_source).expect("write main.ynz");

    let mut db = CompilerDb::default();
    let library_file = SourceFile::new(
        &db,
        library_path.display().to_string(),
        library_source.to_string(),
    );
    let main_file = SourceFile::new(
        &db,
        main_path.display().to_string(),
        main_source.to_string(),
    );
    db.register_source(library_file);
    db.register_source(main_file);
    (*check_query(&db, main_file)).clone()
}

/// The record of `function_name` in `output`, which must be free of errors.
fn record_of(output: &CheckOutput, function_name: &str) -> FunctionTransfers {
    let found = errors(output);
    assert!(found.is_empty(), "expected a clean program, got {found:#?}");
    output
        .typed_module
        .transfers
        .function(function_name)
        .unwrap_or_else(|| panic!("no transfer record for `{function_name}`"))
        .clone()
}

fn founder_name(transfers: &FunctionTransfers, member: &str) -> String {
    let class = transfers
        .class_of(member)
        .unwrap_or_else(|| panic!("`{member}` is in no class"));
    match &class.anchor {
        ReleaseAnchor::Founder { codegen_name, .. } => codegen_name.clone(),
        other => panic!("expected a founder for `{member}`, got {other:?}"),
    }
}

fn escapes_of(transfers: &FunctionTransfers, member: &str) -> Vec<Escape> {
    transfers
        .class_of(member)
        .unwrap_or_else(|| panic!("`{member}` is in no class"))
        .escapes
        .clone()
}

const BAG: &str = "shape Bag {\n  items: Array<Int>\n}\n";
const PAIR: &str = "shape Pair {\n  first: Array<Int>\n  second: Array<Int>\n}\n";

/// A callee that takes a whole `Bag` (`give`), and a maker of a fresh one to reassign it with.
const EAT_BAG: &str = "shape Bag {\n  items: Array<Int>\n}
function eatBag(give bag: Bag) -> Nothing {
  print(bag.items.count().toString())
}
function freshBag() -> Bag {
  let made: Bag = { items: [4, 5] }
  return made
}
";

/// The escape of a value stored in `place` (the place as typeck names it, backticks included).
fn stored(place: &str) -> Escape {
    Escape::Stored {
        place: place.to_string(),
    }
}

// ── the same programs, read after the store: rejected (deny and teach) ──────

/// The store hands `numbers` over, so this read of `numbers` is the compile error with the store's
/// text.
fn assert_read_after_store_rejected(source: &str) {
    assert_rejected_with(source, "was stored in");
}

cases! { assert_read_after_store_rejected;
    read_after_container_add_is_rejected: "function underTest() -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  let container: Array<Array<Int>> = []
  container.add(numbers)
  return numbers.count() + container.count()
}", "(High) `container.add(numbers)` then reading `numbers` and `container` (the old `store_then_read_both`, `bg_arg_alias_container_add_red`): accepted before, so a release pass freed `numbers`'s array under the container";
    read_after_field_set_is_rejected: format!("{BAG}function underTest() -> Int {{
  let numbers: Array<Int> = [1, 2, 3]
  let bag: Bag = {{ items: [0] }}
  bag.items = numbers
  return numbers.count() + bag.items.count()
}}"), "(High) t6b: `bag.items = numbers` then reading `numbers`";
    read_after_index_set_is_rejected: "function underTest() -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  let container: Array<Array<Int>> = [[0]]
  container[0] = numbers
  return numbers.count() + container.count()
}", "(High) g8: `container[0] = numbers` then reading `numbers`";
    read_after_let_is_rejected: "function underTest() -> Int {
  let first: Array<Int> = [1]
  let second: Array<Int> = first
  return first.count() + second.count()
}", "(High) `let second = first` then reading `first`: two names for one value, the old silent alias";
    read_after_assign_is_rejected: "function underTest() -> Int {
  let acc: Array<Int> = [0]
  let per: Array<Int> = [1]
  acc = per
  return per.count() + acc.count()
}", "(High) g7: `acc = per` then reading `per`";
    read_after_map_set_key_and_value_is_rejected: "function underTest() -> Int {
  let numbers: Array<Int> = [1]
  let table: Map<Int, Array<Int>> = {}
  table.set(1, numbers)
  return numbers.count() + table.count()
}", "(High) `table.set(1, numbers)` then reading `numbers`: the map's own table of values is the other owner";
    read_after_literal_part_is_rejected: format!("{PAIR}function underTest() -> Int {{
  let numbers: Array<Int> = [1, 2, 3]
  let moreNumbers: Array<Int> = [4, 5]
  let pair: Pair = {{ first: numbers, second: moreNumbers }}
  return moreNumbers.count() + pair.first.count()
}}"), "(High) t7: `{ first: numbers, second: moreNumbers }` then reading `moreNumbers`: every part is handed over, not only the first";
}

// ── a read inside the `is` arm that narrowed the handed-over binding ────────

/// A union with one shape variant that holds heap data, and a callee that takes it over.
const FIGURE: &str = "shape Circle {
  label: Array<Int>
}
shape Square {
  side: Int
}
shape Figure = Circle | Square
function eatCircle(give circle: Circle) -> Nothing {
  print(circle.label.count().toString())
}
";

cases! { assert_read_after_store_rejected;
    read_after_store_in_a_narrowed_arm_is_rejected: format!("{FIGURE}function underTest(give figure: Figure) -> Nothing {{
  let circles: Array<Circle> = []
  if (figure) {{
    is Circle => {{
      circles.add(figure)
      print(figure.label.count().toString())
    }}
    is Square => {{ }}
  }}
}}"), "(High) `circles.add(figure)` inside the `is Circle` arm, then reading `figure` in the same arm: the arm's narrowed read skipped the consumed check, so a release pass freed the label under the container";
    read_after_let_in_a_narrowed_arm_is_rejected: format!("{FIGURE}function underTest(give figure: Figure) -> Nothing {{
  if (figure) {{
    is Circle => {{
      let owner: Circle = figure
      print(figure.label.count().toString())
    }}
    is Square => {{ }}
  }}
}}"), "(High) `let owner = figure` inside the arm, then reading `figure`: the same narrowed read, through a `let` store";
}

cases! { assert_use_after_give;
    read_after_give_in_a_narrowed_arm_is_rejected: format!("{FIGURE}function underTest(give figure: Figure) -> Nothing {{
  if (figure) {{
    is Circle => {{
      eatCircle(figure)
      print(figure.label.count().toString())
    }}
    is Square => {{ }}
  }}
}}"), "(High) a give inside the `is Circle` arm, then reading `figure` in the arm: the narrowed read skipped the use-after-give check";
    second_give_in_a_narrowed_arm_is_rejected: format!("{FIGURE}function underTest(give figure: Figure) -> Nothing {{
  if (figure) {{
    is Circle => {{
      eatCircle(figure)
      eatCircle(figure)
    }}
    is Square => {{ }}
  }}
}}"), "(High) two gives of the narrowed binding in one arm: a double free under a release pass";
}

cases! { assert_accepted;
    reads_before_the_store_in_a_narrowed_arm_are_accepted: format!("{FIGURE}function underTest(give figure: Figure) -> Nothing {{
  let circles: Array<Circle> = []
  if (figure) {{
    is Circle => {{
      print(figure.label.count().toString())
      circles.add(figure)
    }}
    is Square => {{
      print(figure.side.toString())
    }}
  }}
}}"), "(High) over-rejection: a narrowed read before the store, and a read in the other arm, stay legal";
}

// ── one value handed to two owners inside ONE statement ─────────────────────
//
// Each operand is read before a LATER operand of the same statement consumes it, so the read site
// saw a live binding; the sink then has to report it. All but the index store compiled: the sink
// skipped any binding already consumed when the sink started, on the belief that its read had been
// reported. The index store was refused already; it guards the statement form with no call.

const EATERS: &str =
    "function eatBoth(give numbers: Array<Int>, give rows: Array<Array<Int>>) -> Nothing {
  print(numbers.count().toString())
}
function readAndEat(share numbers: Array<Int>, give rows: Array<Array<Int>>) -> Nothing {
  print(numbers.count().toString())
}
function eatBack(give numbers: Array<Int>) -> Array<Int> {
  return numbers
}
function countOf(give numbers: Array<Int>) -> Int {
  return numbers.count()
}
";

cases! { assert_read_after_store_rejected;
    give_then_literal_store_in_one_call_is_rejected: format!("{EATERS}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1]
  eatBoth(numbers, [numbers])
}}"), "(High) `eatBoth(numbers, [numbers])`: the literal takes `numbers` over and the call is given `numbers` too; two owners, a double free under a release pass";
    share_then_literal_store_in_one_call_is_rejected: format!("{EATERS}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1]
  readAndEat(numbers, [numbers])
}}"), "(High) `readAndEat(numbers, [numbers])`: the callee reads `numbers` while it owns `rows`, which holds the same array; a read after free inside the callee";
}

cases! { assert_use_after_give;
    literal_store_then_give_in_one_literal_is_rejected: format!("{EATERS}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1]
  let container: Array<Array<Int>> = [numbers, eatBack(numbers)]
  print(container.count().toString())
}}"), "(High) `[numbers, eatBack(numbers)]`: `eatBack` takes `numbers` and hands it back, and the literal stores `numbers` as well; the literal holds one array twice";
    collection_store_then_give_in_one_call_is_rejected: format!("{EATERS}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1]
  let table: Map<Array<Int>, Array<Int>> = {{}}
  table.set(numbers, eatBack(numbers))
}}"), "(High) `table.set(numbers, eatBack(numbers))`: the key and the value are the same array, two owners in one map";
    index_store_after_a_give_in_the_index_is_rejected: format!("{EATERS}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1]
  let container: Array<Array<Int>> = [[0], [0]]
  container[countOf(numbers) - 1] = numbers
}}"), "(High) `container[countOf(numbers) - 1] = numbers`: the index gives `numbers` away and the store hands it to `container` too";
}

// ── `self` handed over, then read ───────────────────────────────────────────
//
// `self` is a binding like any other, but its read never reached the consumed-read check, so a
// `give self` handed over or given away stayed readable (and a second store was skipped silently).

const KEEPER: &str = "shape Box {
  items: Array<Int>
}
function eat(give box: Box) -> Nothing {
  print(box.items.count().toString())
}
";

cases! { assert_read_after_store_rejected;
    read_of_self_after_a_store_is_rejected: format!("{KEEPER}function keep(give self: Box, lend boxes: Array<Box>) -> Nothing {{
  boxes.add(self)
  print(self.items.count().toString())
}}"), "(High) `boxes.add(self)` then reading `self`: the container frees the box under the read";
    second_store_of_self_is_rejected: format!("{KEEPER}function keep(give self: Box, lend boxes: Array<Box>) -> Nothing {{
  boxes.add(self)
  boxes.add(self)
}}"), "(High) `self` stored twice: one box, two slots, a double free";
    read_of_self_after_a_let_is_rejected: format!("{KEEPER}function keep(give self: Box) -> Nothing {{
  let mine = self
  print(self.items.count().toString())
  print(mine.items.count().toString())
}}"), "(High) `let mine = self` then reading `self`: two names for one box";
}

cases! { assert_use_after_give;
    read_of_self_after_a_give_is_rejected: format!("{KEEPER}function keep(give self: Box) -> Nothing {{
  eat(self)
  print(self.items.count().toString())
}}"), "(High) `eat(self)` then reading `self`: `eat` frees the box under the read";
}

cases! { assert_loop_store_rejected;
    store_of_self_in_a_loop_is_rejected: format!("{KEEPER}function keep(give self: Box, lend boxes: Array<Box>) -> Nothing {{
  for (i in [1, 2]) {{
    boxes.add(self)
  }}
}}"), "(High) `boxes.add(self)` in a loop: the second pass stores the box again";
}

// ── the legal forms next to the same-statement and `self` rejections ────────
//
// Green on arrival: they pin that the one consumed-read reporter (a reported-span set) does not
// reject a read that comes before the hand-over, a read of a copy, or a read of a `share self`.

cases! { assert_accepted;
    copy_then_store_in_one_literal_is_accepted: format!("{EATERS}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1]
  let boxes: Array<Array<Int>> = [numbers.copy(), numbers]
  print(boxes.count().toString())
}}"), "(High) over-rejection: `[numbers.copy(), numbers]` stores a copy and then `numbers`; the copy is a fresh value, `numbers` is handed over once";
    read_in_the_key_then_store_in_one_call_is_accepted: format!("{EATERS}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1]
  let table: Map<Int, Array<Int>> = {{}}
  table.set(numbers.count(), numbers)
}}"), "(High) over-rejection: `table.set(numbers.count(), numbers)` reads `numbers` for the key and then hands it over; a read that ends before the hand-over is legal";
    copy_of_the_value_beside_a_give_in_one_call_is_accepted: format!("{EATERS}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1]
  eatBoth(numbers, [numbers.copy()])
}}"), "(High) over-rejection: `eatBoth(numbers, [numbers.copy()])` gives `numbers` once; the literal holds a copy";
    share_self_read_twice_is_accepted: format!("{KEEPER}function twice(share self: Box) -> Int {{
  return self.items.count() + self.items.count()
}}"), "(High) over-rejection: a `share self` is never handed over, so reading it again is legal";
    give_self_stored_once_is_accepted: format!("{KEEPER}function keep(give self: Box, lend boxes: Array<Box>) -> Nothing {{
  print(self.items.count().toString())
  boxes.add(self)
}}"), "(High) over-rejection: a `give self` read and then stored once is legal";
}

// ── t3: a move inside a loop, rejected ──────────────────────────────────────

/// The program must be rejected with the canonical use-after-give text and the loop wording.
fn assert_loop_give_rejected(source: &str) {
    let found = error_diagnostics(&run(source))
        .into_iter()
        .map(|diagnostic| (diagnostic.kind.clone(), diagnostic.what.clone()))
        .collect::<Vec<_>>();
    assert!(
        found.iter().any(|(kind, what)| {
            *kind == Some(DiagnosticKind::Consumed)
                && what.contains("was already given away")
                && what.contains("previous pass")
        }),
        "expected the loop-carried give to be rejected for:\n{source}\ngot {found:#?}"
    );
}

/// The store twin: same rejection with the use-after-store template.
fn assert_loop_store_rejected(source: &str) {
    let found = error_diagnostics(&run(source))
        .into_iter()
        .map(|diagnostic| (diagnostic.kind.clone(), diagnostic.what.clone()))
        .collect::<Vec<_>>();
    assert!(
        found.iter().any(|(kind, what)| {
            *kind == Some(DiagnosticKind::ConsumedByStore)
                && what.contains("was stored in")
                && what.contains("previous pass")
        }),
        "expected the loop-carried store to be rejected for:\n{source}\ngot {found:#?}"
    );
}

/// The send twin: same rejection with the use-after-send template.
fn assert_loop_send_rejected(source: &str) {
    let found = error_diagnostics(&run(source))
        .into_iter()
        .map(|diagnostic| (diagnostic.kind.clone(), diagnostic.what.clone()))
        .collect::<Vec<_>>();
    assert!(
        found.iter().any(|(kind, what)| {
            *kind == Some(DiagnosticKind::ConsumedBySend)
                && what.contains("was sent into")
                && what.contains("previous pass")
        }),
        "expected the loop-carried send to be rejected for:\n{source}\ngot {found:#?}"
    );
}

cases! { assert_loop_give_rejected;
    give_in_a_while_loop_is_rejected: format!(
        "{EAT}function underTest(condition: Boolean) -> Nothing {{
  let numbers: Array<Int> = [1, 2, 3]
  let i: Int = 0
  while (i < 3) {{
    if (condition) {{ return }}
    eat(numbers)
    i = i + 1
  }}
}}"
    ), "(High) t3, the exact spike probe: an outer binding given on one pass of a `while` is gone on the next; typeck accepted it, so a release pass would double-free";
    give_in_a_for_loop_is_rejected: format!(
        "{EAT}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    eat(numbers)
  }}
}}"
    ), "(High) t3 in numbers `for` loop: the second loop shape is checked by its own code path";
    give_of_a_binding_that_took_a_value_over_in_a_loop_is_rejected: format!(
        "{EAT}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1, 2, 3]
  let newOwner: Array<Int> = numbers
  for (i in range(0, 3)) {{
    eat(newOwner)
  }}
}}"
    ), "(High) t3 through a hand-over: `let newOwner = numbers` handed `numbers`'s value to `newOwner`, which is declared outside the loop and given on every pass";
    give_in_a_while_condition_is_rejected: format!(
        "function holds(give rows: Array<Int>) -> Boolean {{
  return rows.count() > 0
}}
{EAT}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1, 2, 3]
  while (holds(numbers)) {{
    print(`pass`)
  }}
}}"
    ), "(High) t3 through the condition: a `while` condition runs at the top of every pass, so a give made in it reaches the next pass; the loop was opened only after the condition was checked, so typeck accepted a double give";
    give_in_an_inner_loop_reaches_the_outer_pass: format!(
        "{EAT_BAG}function underTest() -> Nothing {{
  let bag: Bag = {{ items: [1, 2, 3] }}
  let view: Array<Int> = bag.items
  for (j in range(0, 2)) {{
    print(view.count().toString())
    for (i in range(0, 1)) {{
      eatBag(bag)
      bag = freshBag()
    }}
  }}
}}"
    ), "(High) t3 across nested loops: the inner loop revives `bag` so it carries nothing, but the outer pass reads `view`, a piece of the value given; without the hand-up of the inner loop's give, that read of freed memory is accepted";
}

cases! { assert_loop_store_rejected;
    store_of_an_outer_binding_in_a_loop_is_rejected: "function underTest() -> Nothing {
  let numbers: Array<Int> = [1, 2, 3]
  for (j in range(0, 2)) {
    let newOwner: Array<Int> = numbers
    print(newOwner.count().toString())
  }
}", "(High) t3 for a store: `let newOwner = numbers` hands `numbers` over on every pass, and the next pass reads `numbers` again (a release pass would free it once per pass)";
    container_add_of_an_outer_binding_in_a_loop_is_rejected: "function underTest() -> Nothing {
  let numbers: Array<Int> = [1, 2, 3]
  let container: Array<Array<Int>> = []
  for (j in range(0, 2)) {
    container.add(numbers)
  }
}", "(High) t3 for a container store: `container.add(numbers)` hands `numbers` over on every pass";
}

cases! { assert_loop_send_rejected;
    send_in_a_for_loop_is_rejected: "function producer(lend wire: Channel<Array<Int>>) -> Nothing {
  let sent: Array<Int> = [10, 20, 30]
  for (i in range(0, 3)) {
    wire.send(sent)
  }
  wire.close()
}", "(High) t3 for `send`: the loop-carried check also covers a send, whose wording is its own arm; an accepted program sends one array twice and two receivers free it";
}

cases! { assert_accepted;
    give_of_a_loop_local_is_accepted: format!(
        "{EAT}function underTest() -> Nothing {{
  for (i in range(0, 3)) {{
    let numbers: Array<Int> = [1, 2, 3]
    eat(numbers)
  }}
}}"
    ), "(High) over-rejection: a give of a binding the loop itself declares carries nothing into the next pass";
    give_then_reassign_is_accepted: format!(
        "{EAT}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    eat(numbers)
    numbers = [4, 5]
  }}
}}"
    ), "(High) over-rejection: a reassignment before the back-edge revives the binding";
    give_on_a_path_that_returns_is_accepted: format!(
        "{EAT}function underTest(condition: Boolean) -> Nothing {{
  let numbers: Array<Int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    if (condition) {{
      eat(numbers)
      return
    }}
  }}
}}"
    ), "(High) over-rejection: a give on a path that leaves the loop never reaches the next pass";
    unread_class_mate_of_a_revived_binding_is_accepted: format!(
        "{EAT_BAG}function underTest() -> Nothing {{
  let bag: Bag = {{ items: [1, 2, 3] }}
  let view: Array<Int> = bag.items
  for (i in range(0, 3)) {{
    eatBag(bag)
    bag = freshBag()
  }}
}}"
    ), "(High) over-rejection: a class-mate the loop never mentions stays consumed after the given binding is revived, but nothing in the next pass reads it";
    give_after_the_loop_is_accepted: format!(
        "{EAT}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    print(numbers.count().toString())
  }}
  eat(numbers)
}}"
    ), "(High) over-rejection: a give after the loop is the ordinary single give";
    give_of_an_outer_loop_local_in_an_inner_loop_is_accepted: format!(
        "{EAT}function underTest() -> Nothing {{
  for (j in range(0, 2)) {{
    let numbers: Array<Int> = [1, 2, 3]
    for (i in range(0, 1)) {{
      eat(numbers)
      numbers = [0]
    }}
  }}
}}"
    ), "(High) over-rejection: a give the inner loop hands up is of a binding the outer loop declares afresh each pass, so the outer loop carries nothing";
}

// ── accepted and rejected programs next to the record ───────────────────────

/// A store of a parameter the caller keeps is refused with the parameter's own text.
fn assert_stored_parameter_rejected(source: &str) {
    assert_rejected_with(source, "is a parameter of");
}

cases! { assert_stored_parameter_rejected;
    storing_a_borrowed_parameter_is_rejected: "function stash(lend bucket: Array<Array<Int>>, rows: Array<Int>) -> Nothing {
  bucket.add(rows)
}", "(High) a store of a parameter the caller keeps (`lend`, `share` or bare) is refused: the caller owns the value, so the container could not own it too";
    a_callee_that_stores_a_bare_parameter_is_rejected: "function put(lend container: Array<Array<Int>>, numbers: Array<Int>) -> Nothing {
  container.add(numbers)
}", "(High) a callee that stores a parameter it does not own would hand the caller's value to its container behind the caller's back: the store is refused inside the callee, which must say `give`";
}

cases! { assert_accepted;
    storing_a_string_hands_nothing_over: "function underTest() -> Int {
  let name: String = `ada`
  let names: Array<String> = []
  names.add(name)
  let same: String = name
  return names.count() + same.count()
}", "(High) over-rejection: a string is a counted heap allocation, but a store copies it or takes it by move, so storing one hands nothing over and the name stays readable";
    storing_a_sensitive_string_hands_nothing_over: "function underTest() -> Int {
  let key: sensitive String = sensitive(`abc`)
  let keys: Array<sensitive String> = []
  keys.add(key)
  let same: sensitive String = key
  print(key)
  return keys.count()
}", "(High) over-rejection: a `sensitive string` is the string pointer with a label and its copy is the receiver's bits (`owned_copy_plan`), so storing one hands nothing over";
    storing_a_range_hands_nothing_over: "shape Window { span: Range }
function underTest() -> Int {
  let wholeRange = range(0, 3)
  let sameRange = wholeRange
  let window: Window = { span: wholeRange }
  let total = 0
  for (i in wholeRange) {
    total = total + i
  }
  for (j in sameRange) {
    total = total + j
  }
  for (k in window.span) {
    total = total + k
  }
  return total
}", "(High) over-rejection: a `Range` is its start and end and nothing can change them, so a `let` and a struct literal holding `wholeRange` leave it readable (an `Array<Range>` does not build end to end yet, so it is not pinned)";
    a_callee_that_only_reads_an_argument_leaves_it_readable: "function tally(lend into: Array<Int>, from: Array<Int>) -> Nothing {
  into.add(from.count())
}
function underTest() -> Int {
  let from: Array<Int> = [1, 2, 3]
  let into: Array<Int> = []
  tally(into, from)
  return from.count() + into.count()
}", "(High) over-rejection: a callee that only READS `from` into `into` takes nothing over, so both stay usable";
}

fn assert_read_after_call_give_rejected(source: &str) {
    assert_use_after_give(source);
}

cases! { assert_read_after_call_give_rejected;
    read_after_a_direct_call_store_is_rejected: format!("{PUT}function underTest() -> Int {{
  let numbers: Array<Int> = [1, 2, 3]
  let container: Array<Array<Int>> = []
  put(container, numbers)
  return numbers.count()
}}"), "(High) `put(container, numbers)` stores `numbers` INSIDE the callee: the caller's `numbers` is given away at the call, so the read is refused (before, the call joined the two classes and `numbers` stayed readable)";
    read_after_a_relayed_call_store_is_rejected: format!("{PUT}function underTest() -> Int {{
  let numbers: Array<Int> = [1, 2, 3]
  let container: Array<Array<Int>> = []
  relay(container, numbers)
  return numbers.count()
}}"), "(High) the store happens in the callee's own call (`relay` -> `put`): `relay`'s parameter is `give` too";
}

// ── how a spawn in a loop takes its binding ─────────────────────────────────

/// The ownership typeck inferred for the one `background work(rows)` spawn in `source`; the
/// program must be free of errors.
fn spawn_ownership_of(source: &str) -> Vec<BgOwnership> {
    let output = run(source);
    let found = errors(&output);
    assert!(found.is_empty(), "{found:#?}");
    output
        .typed_module
        .background_arg_inferred_ownership
        .values()
        .cloned()
        .collect()
}

fn assert_spawn_copies(source: &str) {
    assert_eq!(spawn_ownership_of(source), vec![BgOwnership::Copy]);
}

fn assert_spawn_gives(source: &str) {
    assert_eq!(spawn_ownership_of(source), vec![BgOwnership::Give]);
}

const WORK: &str = "function work(rows: Array<Int>) -> Nothing {
  print(rows.count().toString())
}
";

cases! { assert_spawn_copies;
    a_spawn_in_a_loop_of_an_outer_binding_copies_it: format!("{WORK}function underTest() -> Nothing {{
  let rows: Array<Int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    background work(rows)
  }}
}}"), "(High) a `background` spawn inside a loop of a binding declared OUTSIDE it: the next pass spawns with the same binding, so the first spawn must copy, never give (a second give is a second owner of one array)";
}

cases! { assert_spawn_gives;
    a_spawn_in_a_loop_of_a_loop_local_gives_it: format!("{WORK}function underTest() -> Nothing {{
  for (i in range(0, 3)) {{
    let rows: Array<Int> = [1, 2, 3]
    background work(rows)
  }}
}}"), "(High) a spawn of a binding the loop declares afresh each pass gives it: the copy that an outer binding needs would be a leak per pass";
}

// ── what the record says about each binding ─────────────────────────────────

const BUCKET: &str = "shape Bucket {\n  rows: Array<Int>\n}\n";

const EAT2: &str = "function eat_all(give rows: Array<Array<Int>>) -> Nothing {
  print(rows.count().toString())
}\n";

const PUT: &str =
    "function put(lend container: Array<Array<Int>>, give numbers: Array<Int>) -> Nothing {
  container.add(numbers)
}
function relay(lend container: Array<Array<Int>>, give numbers: Array<Int>) -> Nothing {
  put(container, numbers)
}\n";

const FIRST_BAG: &str = "shape Bag {
  items: Array<Int>
}
function firstBag<T>(share bags: Array<T>, share fallback: T) -> T {
  return bags[0].or(fallback)
}
";

const MAYBE_CHANNEL_LIBRARY: &str =
    "export function firstChan(share chans: Array<Channel<Int>>) -> Maybe<Channel<Int>> {
  return chans.first()
}\n";

const MAYBE_CHANNEL_MAIN: &str = "import { firstChan } from `lib`
function underTest(condition: Boolean) -> Nothing {
  let held: Maybe<Channel<Int>> = none
  if (condition) {
    let local: Array<Channel<Int>> = [Channel<Int>()]
    held = firstChan(local)
  }
}
function entrypoint() -> Nothing {
}
";

const HANDED_BACK_LIBRARY: &str = "export function same(share ch: Channel<Int>) -> Channel<Int> {
  return ch
}\n";

const HANDED_BACK_MAIN: &str = "import { same } from `lib`
function passOn(share ch: Channel<Int>) -> Channel<Int> {
  return ch
}
function viaImport(condition: Boolean) -> Nothing {
  let held: Channel<Int> = Channel<Int>()
  if (condition) {
    let local: Channel<Int> = Channel<Int>()
    held = same(local)
  }
}
function viaLocal(condition: Boolean) -> Nothing {
  let held: Channel<Int> = Channel<Int>()
  if (condition) {
    let local: Channel<Int> = Channel<Int>()
    held = passOn(local)
  }
}
function entrypoint() -> Nothing {
}
";

const UNTRACEABLE_MAYBE_LIBRARY: &str =
    "export function firstRow(share rows: Array<Array<Int>>) -> Maybe<Array<Int>> {
  return rows.first()
}\n";

const UNTRACEABLE_MAYBE_MAIN: &str = "import { firstRow } from `lib`
function underTest(condition: Boolean) -> Int {
  let held: Maybe<Array<Int>> = none
  if (condition) {
    let local: Array<Array<Int>> = [[1]]
    held = firstRow(local)
  }
  return 1
}
function entrypoint() -> Nothing {
}
";

const IMPORTED_RESULT_LIBRARY: &str = "export function fresh() -> Array<Int> {
  return [1]
}\n";

const IMPORTED_RESULT_MAIN: &str = "import { fresh } from `lib`
function underTest() -> Int {
  let result: Array<Int> = fresh()
  return result.count()
}
function entrypoint() -> Nothing {
}
";

fn assert_no_owner(transfers: &FunctionTransfers, member: &str) {
    let class = transfers
        .class_of(member)
        .unwrap_or_else(|| panic!("`{member}` is in no class"));
    assert_eq!(class.anchor, ReleaseAnchor::NoOwner, "{class:#?}");
}

/// One named test per program: `record_cases!(name: output, "function", |transfers| { checks },
/// "bug it catches (High)";)` runs the program, requires zero errors, and hands the record of
/// `function` to the checks.
macro_rules! record_cases {
    ($($case:ident: $output:expr, $function:literal, |$transfers:ident| $checks:block, $bug:literal;)+) => {
        $(
            // bug: typeck's transfer record loses or misplaces a transfer (a store, give, send, join or owner), so a release pass frees a value twice, too early or never | tier: High
            #[doc = $bug]
            #[test]
            fn $case() {
                let $transfers = record_of(&$output, $function);
                $checks
            }
        )+
    };
}

record_cases! {
    a_field_set_hands_the_value_to_the_field: run(&format!(
        "{BAG}function make() -> Bag {{
  let numbers: Array<Int> = [1, 2, 3]
  let bag: Bag = {{ items: [0] }}
  bag.items = numbers
  return bag
}}"
    )), "make", |transfers| {
        assert!(!transfers.same_class("numbers", "bag"), "{transfers:#?}");
        assert_eq!(escapes_of(&transfers, "numbers"), vec![stored("`bag.items`")]);
        assert_eq!(escapes_of(&transfers, "bag"), vec![Escape::Returned]);
    }, "(High) `bag.items = numbers; return bag`: the field-set stores `numbers` in `bag`, so `numbers` is handed over and `bag` (which owns the value now) escapes by return; before, the two shared one class and a release pass could not tell who frees the array";
    a_container_add_hands_the_value_to_the_container: run("function make() -> Array<Array<Int>> {
  let numbers: Array<Int> = [1, 2, 3]
  let container: Array<Array<Int>> = []
  container.add(numbers)
  return container
}"), "make", |transfers| {
        assert!(!transfers.same_class("numbers", "container"), "{transfers:#?}");
        assert_eq!(escapes_of(&transfers, "numbers"), vec![stored("`container`")]);
        assert_eq!(escapes_of(&transfers, "container"), vec![Escape::Returned]);
    }, "(High) `container.add(numbers); return container`: the container `.add` hands `numbers` over to `container`";
    an_index_set_hands_the_value_to_the_slot: run("function make() -> Array<Array<Int>> {
  let numbers: Array<Int> = [1, 2, 3]
  let container: Array<Array<Int>> = [[0]]
  container[0] = numbers
  return container
}"), "make", |transfers| {
        assert!(!transfers.same_class("numbers", "container"), "{transfers:#?}");
        assert_eq!(escapes_of(&transfers, "numbers"), vec![stored("`container[0]`")]);
        assert_eq!(escapes_of(&transfers, "container"), vec![Escape::Returned]);
    }, "(High) `container[0] = numbers; return container`: the index-set hands `numbers` over to the slot";
    an_assignment_from_a_loop_local_hands_the_value_to_the_outliving_binding: run("function underTest() -> Int {
  let acc: Array<Int> = [0]
  let i: Int = 0
  while (i < 2) {
    let per: Array<Int> = [i, i, i]
    acc = per
    i = i + 1
  }
  return acc.count()
}"), "underTest", |transfers| {
        assert!(!transfers.same_class("acc", "per"), "{transfers:#?}");
        assert_eq!(escapes_of(&transfers, "per"), vec![stored("`acc`")]);
        assert_eq!(founder_name(&transfers, "acc"), "acc");
    }, "(High) `acc = per` inside a loop: `acc` outlives `per`, so a release at the back-edge of `per` would free what `acc` still holds; `per` escapes and `acc` is the founder of its own class";
    a_struct_literal_hands_every_part_to_the_literal: run(&format!(
        "{PAIR}function make() -> Pair {{
  let numbers: Array<Int> = [1, 2, 3]
  let moreNumbers: Array<Int> = [4, 5]
  let pair: Pair = {{ first: numbers, second: moreNumbers }}
  return pair
}}"
    )), "make", |transfers| {
        assert!(!transfers.same_class("numbers", "moreNumbers"), "{transfers:#?}");
        let literal = "the literal `{ first: numbers, second: moreNumbers }`";
        assert_eq!(escapes_of(&transfers, "numbers"), vec![stored(literal)]);
        assert_eq!(escapes_of(&transfers, "moreNumbers"), vec![stored(literal)]);
        assert_eq!(escapes_of(&transfers, "pair"), vec![Escape::Returned]);
    }, "(High) `{ first: numbers, second: moreNumbers }` returned through `pair`: the literal takes over EVERY named heap part, not only the first; before, `moreNumbers` was released while the returned `pair` held it";
    a_let_of_a_whole_binding_hands_the_value_to_the_new_binding: run("function underTest() -> Int {
  let first: Array<Int> = [1]
  let second: Array<Int> = first
  return second.count()
}"), "underTest", |transfers| {
        assert!(!transfers.same_class("first", "second"), "{transfers:#?}");
        assert_eq!(escapes_of(&transfers, "first"), vec![stored("`second`")]);
        assert_eq!(founder_name(&transfers, "second"), "second");
    }, "(High) `let second = first`: the second name takes the value over, so `first` is handed to `second` and `second` is the founder of the value it now owns";
    storing_bits_hands_nothing_over: run("function underTest() -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  let container: Array<Int> = []
  container.add(numbers[0].or(0))
  container.add(numbers.count())
  return container.count() + numbers.count()
}"), "underTest", |transfers| {
        assert_eq!(escapes_of(&transfers, "numbers"), Vec::<Escape>::new());
    }, "(High) over-joining: bits are copied, not handed over, so storing an int taken from `numbers` records nothing for it";
    a_store_then_a_give_of_the_container_is_recorded_twice: run(&format!(
        "{EAT2}function underTest() -> Nothing {{
  let numbers: Array<Int> = [1, 2, 3]
  let container: Array<Array<Int>> = []
  container.add(numbers)
  eat_all(container)
}}"
    )), "underTest", |transfers| {
        assert_eq!(escapes_of(&transfers, "numbers"), vec![stored("`container`")]);
        assert_eq!(
            escapes_of(&transfers, "container"),
            vec![Escape::Given { callee: "eat_all".to_string() }]
        );
    }, "(High) a store of a value the program does not read again is accepted and the container can be given on: the stored value travels with it, so `numbers`'s transfer is the store and the container's is the give";
    a_send_is_recorded_as_an_escape_on_its_channel: run("function producer(lend wire: Channel<Array<Int>>) -> Nothing {
  let sent: Array<Int> = [10, 20, 30]
  wire.send(sent)
  wire.close()
}"), "producer", |transfers| {
        assert_eq!(
            escapes_of(&transfers, "sent"),
            vec![Escape::Sent { channel: "wire".to_string() }]
        );
    }, "(High) a `send` is recorded as `Sent` on the channel: a release pass that read no escape would free the sent array at the producer's scope exit while the receiver reads it";
    a_store_into_a_temporary_is_recorded_as_an_escape: run(&format!(
        "{BAG}function makeBag() -> Bag {{
  return {{ items: [0] }}
}}
function underTest() -> Nothing {{
  let numbers: Array<Int> = [1, 2, 3]
  makeBag().items = numbers
}}"
    )), "underTest", |transfers| {
        assert_eq!(escapes_of(&transfers, "numbers"), vec![stored("`makeBag().items`")]);
    }, "(High) a store into a place no binding names (`makeBag().items = numbers`) is recorded, not dropped: the temporary holds `numbers`, so nothing may release it as a plain local";
    a_borrowed_parameter_in_the_class_means_the_caller_owns_it: run(&format!(
        "{BAG}function underTest(share bag: Bag) -> Int {{
  let view: Array<Int> = bag.items
  return view.count()
}}"
    )), "underTest", |transfers| {
        assert!(transfers.same_class("bag", "view"), "{transfers:#?}");
        let class = transfers.class_of("view").expect("class");
        assert!(matches!(&class.anchor, ReleaseAnchor::CallerOwns { .. }), "{class:#?}");
    }, "(High) a piece of a parameter the caller keeps stays recorded as the caller's: a class with a borrowed parameter in it is never released here";
    the_outer_owner_is_the_founder_of_a_class_with_a_deeper_piece: run(&format!(
        "{BAG}function underTest() -> Int {{
  let outer: Bag = {{ items: [1] }}
  let total: Int = 0
  if (true) {{
    let inner: Array<Int> = outer.items
    total = inner.count()
  }}
  return outer.items.count() + total
}}"
    )), "underTest", |transfers| {
        assert_eq!(founder_name(&transfers, "inner"), "outer");
    }, "(High) the founder is the owner whose scope exits last: a piece declared deeper never anchors a class an outer owner is in";
    a_maybe_channel_an_imported_function_returns_is_an_owner_in_a_class_of_its_own: run_with_library(MAYBE_CHANNEL_LIBRARY, MAYBE_CHANNEL_MAIN), "underTest", |transfers| {
        assert!(!transfers.same_class("held", "local"), "{transfers:#?}");
        assert_eq!(founder_name(&transfers, "held"), "held");
        assert_eq!(founder_name(&transfers, "local"), "local");
    }, "(High) an imported function that returns a `Maybe` holding a `Channel` hands back a counted reference of its own (`copied_at_return`), so the holder is its own founder and each is released once; no class is left without a static founder (the reproducer of `NoStaticFounder`)";
    a_channel_handed_back_by_an_imported_function_is_a_counted_reference_in_a_class_of_its_own: run_with_library(HANDED_BACK_LIBRARY, HANDED_BACK_MAIN), "viaImport", |transfers| {
        assert!(!transfers.same_class("held", "local"), "{transfers:#?}");
        assert_eq!(founder_name(&transfers, "held"), "held");
        assert_eq!(founder_name(&transfers, "local"), "local");
    }, "(High) an imported function that returns its `Channel` argument hands back a counted reference (`ynz_channel_share` at the callee's `return`), so the binding that holds it owns it and the channel is released once per reference";
    a_channel_handed_back_by_a_local_function_is_a_counted_reference_in_a_class_of_its_own: run_with_library(HANDED_BACK_LIBRARY, HANDED_BACK_MAIN), "viaLocal", |transfers| {
        assert!(!transfers.same_class("held", "local"), "{transfers:#?}");
        assert_eq!(founder_name(&transfers, "held"), "held");
        assert_eq!(founder_name(&transfers, "local"), "local");
    }, "(High) the same counted reference when the function that returns its `Channel` argument is a local one";
    an_untraceable_maybe_result_is_a_copy_owner_in_a_class_of_its_own: run_with_library(UNTRACEABLE_MAYBE_LIBRARY, UNTRACEABLE_MAYBE_MAIN), "underTest", |transfers| {
        assert!(!transfers.same_class("held", "local"), "{transfers:#?}");
        assert_eq!(founder_name(&transfers, "held"), "held");
        assert_eq!(founder_name(&transfers, "local"), "local");
    }, "(High) an imported function that returns a `Maybe` may hand back a piece of its argument; the binding that holds the result outlives `local`, so it owns a copy (`MustCopy`): it joins nothing and `local` is its own founder";
    a_field_read_that_outlives_its_root_is_a_copy_owner_in_a_class_of_its_own: run(&format!(
        "{BUCKET}function make() -> Bucket {{
  let bucket: Bucket = {{ rows: [9] }}
  return bucket
}}
function underTest(condition: Boolean) -> Int {{
  let cell: Array<Int> = make().rows
  if (condition) {{
    let owner: Bucket = make()
    cell = owner.rows
  }}
  return cell.count()
}}"
    )), "underTest", |transfers| {
        assert!(!transfers.same_class("cell", "owner"), "{transfers:#?}");
        assert_eq!(founder_name(&transfers, "cell"), "cell");
        assert_eq!(founder_name(&transfers, "owner"), "owner");
    }, "(High) the field read that used to leave no static founder (`cell = owner.rows`, `cell` outlives `owner`) is a copy typeck judged `MustCopy`: `cell` owns a value of its own and joins nothing";
    a_call_that_stores_a_given_argument_is_recorded_as_a_give: run(&format!(
        "{PUT}function make() -> Array<Array<Int>> {{
  let numbers: Array<Int> = [1, 2, 3]
  let container: Array<Array<Int>> = []
  put(container, numbers)
  return container
}}"
    )), "make", |transfers| {
        assert!(!transfers.same_class("numbers", "container"), "{transfers:#?}");
        assert_eq!(
            escapes_of(&transfers, "numbers"),
            vec![Escape::Given { callee: "put".to_string() }]
        );
    }, "(High) `put(container, numbers)` records the give on the caller's `numbers`: a release pass sees `numbers` leave";
    a_field_taken_out_of_a_temporary_is_owned_by_its_binding: run(&format!(
        "{BUCKET}function make() -> Bucket {{
  let bucket: Bucket = {{ rows: [9] }}
  return bucket
}}
function underTest() -> Int {{
  let cell: Array<Int> = make().rows
  return cell.count()
}}"
    )), "underTest", |transfers| {
        let class = transfers.class_of("cell").expect("`cell` is in a class");
        assert!(
            matches!(&class.anchor, ReleaseAnchor::Founder { codegen_name, .. } if codegen_name == "cell"),
            "{class:#?}"
        );
    }, "(High) a string, array or map field of a temporary that a `let` keeps is TAKEN, not copied (`ReadVerdict::Take`): the binding is the owner of the field, a founder that releases it (it was `NoOwner` before the take existed, a leak)";
    a_loop_cell_of_a_temporary_is_recorded_as_no_owner: run("function make() -> Array<Array<Int>> {
  let rows: Array<Array<Int>> = [[1]]
  return rows
}
function underTest() -> Int {
  let total: Int = 0
  for (row in make()) {
    total = total + row.count()
  }
  return total
}"), "underTest", |transfers| {
        assert_no_owner(&transfers, "row");
    }, "(High) `NoOwner` for a loop cell of a temporary (`for (row in make())`): nothing may release a cell of a value the loop does not own";
    an_imported_functions_result_is_owned_by_the_binding_that_holds_it: run_with_library(IMPORTED_RESULT_LIBRARY, IMPORTED_RESULT_MAIN), "underTest", |transfers| {
        assert_eq!(founder_name(&transfers, "result"), "result");
    }, "(High) an imported function's result of a type copied at `return` is an owned value, as a local function's is: the binding that holds it founds its own class and releases it (it was `NoOwner`, a leak)";
    a_builtin_or_result_is_a_copy_that_joins_nothing: run("function pick(cache: Array<Array<Int>>) -> Array<Int> {
  let fallback: Array<Int> = [1]
  let found: Array<Int> = cache.first().or(fallback)
  return found
}"), "pick", |transfers| {
        assert!(!transfers.same_class("found", "fallback"), "{transfers:#?}");
        assert!(transfers
            .class_of("found")
            .is_some_and(|class| class.members[0].copy_owner));
    }, "(High) a builtin method's result can be an ARGUMENT: `cache.first().or(fallback)` returns `fallback` when the cache is empty; returned, typeck judges it a copy, so `found` owns a value of its own and joins nothing";
    a_builtin_concat_result_holds_the_elements_of_both_arguments: run("function both() -> Array<Array<Int>> {
  let left: Array<Array<Int>> = [[1]]
  let right: Array<Array<Int>> = [[2]]
  let joined: Array<Array<Int>> = left.concat(right)
  return joined
}"), "both", |transfers| {
        assert!(transfers.same_class("left", "right"), "{transfers:#?}");
        assert_eq!(escapes_of(&transfers, "right"), vec![Escape::Returned]);
    }, "(High) `left.concat(right)` holds the elements of both arguments: returned, `right` must escape with it";
    a_field_of_a_call_result_is_a_piece_of_the_calls_arguments: run(&format!(
        "{FIRST_BAG}function underTest() -> Int {{
  let bags: Array<Bag> = [{{ items: [1] }}]
  let empty: Bag = {{ items: [] }}
  let piece: Array<Int> = firstBag(bags, empty).items
  return piece.count()
}}"
    )), "underTest", |transfers| {
        assert!(!transfers.same_class("bags", "piece"), "{transfers:#?}");
        assert_eq!(founder_name(&transfers, "piece"), "piece");
    }, "(High) `firstBag(bags, empty).items` is a piece of `bags` (the callee returns one of its elements), but the field access rooted only in NAME chains and recorded no root, so `piece` was released while `bags` still held it; the read copies the field, so `piece` owns a value of its own";
    storing_a_field_of_a_generic_call_result_keeps_it_apart_from_the_arguments: run(&format!(
        "{FIRST_BAG}function underTest() -> Array<Array<Int>> {{
  let bags: Array<Bag> = [{{ items: [1] }}]
  let empty: Bag = {{ items: [] }}
  let container: Array<Array<Int>> = []
  let piece: Array<Int> = firstBag(bags, empty).items
  container.add(piece)
  return container
}}"
    )), "underTest", |transfers| {
        assert!(!transfers.same_class("bags", "piece"), "{transfers:#?}");
    }, "(High) a generic callee's `return` copies a heap-typed result, so storing `firstBag(bags, empty).items` is accepted and `bags` and `container` hold different arrays (it was refused as a piece of `bags`, which also left the result unreleased: a leak per call)";
    a_store_into_a_piece_of_a_call_result_hands_the_value_over: run(&format!(
        "{FIRST_BAG}function underTest() -> Nothing {{
  let bags: Array<Bag> = [{{ items: [1] }}]
  let empty: Bag = {{ items: [] }}
  let numbers: Array<Int> = [5]
  firstBag(bags, empty).items = numbers
}}"
    )), "underTest", |transfers| {
        assert_eq!(
            escapes_of(&transfers, "numbers"),
            vec![stored("`firstBag(bags, empty).items`")]
        );
    }, "(High) a store whose PLACE is a call result (`firstBag(bags, empty).items = numbers`) hands `numbers` over to the place, whatever the place is; it was recorded as a store into a temporary";
    a_loop_cell_joins_everything_the_iterated_value_reaches: run("function underTest() -> Int {
  let rows: Array<Array<Int>> = [[1]]
  let extra: Array<Array<Int>> = [[2]]
  let total: Int = 0
  for (row in rows.concat(extra)) {
    total = total + row.count()
  }
  return total
}"), "underTest", |transfers| {
        assert!(transfers.same_class("rows", "extra"), "{transfers:#?}");
        assert!(transfers.same_class("rows", "row"), "{transfers:#?}");
    }, "(High) `for (row in rows.concat(extra))` joined the loop cell to nothing (the iterated expression is a method call, not a name), so `row` was free to outlive `rows` and `extra`";
    a_given_then_revived_binding_keeps_its_given_escape: run(&format!(
        "{EAT}function underTest(condition: Boolean) -> Int {{
  let numbers: Array<Int> = [1, 2, 3]
  if (condition) {{
    eat(numbers)
    numbers = [0]
  }}
  return numbers.count()
}}"
    )), "underTest", |transfers| {
        assert_eq!(
            escapes_of(&transfers, "numbers"),
            vec![Escape::Given { callee: "eat".to_string() }]
        );
    }, "(High) a binding given away on one path and revived by a reassignment keeps its `Given` escape (the record is flow-insensitive on purpose): dropping it at the revive would let a release pass free the class on the path where it WAS given away, a double free";
}

// ── the recorder's own holes ────────────────────────────────────────────────

/// Every event the recorder cannot place must come back as one internal-error value (which
/// `TypedModule::with_codegen_names` turns into a hard error), never as a silent return. The
/// checker cannot produce these events today, so the recorder is driven through its public API:
/// `drive` makes the calls, and the record must hold exactly one message containing `needle`.
/// (`BindingNames::default()` names nothing, so a binding `drive` declares adds a missing-name
/// message of its own; only `needle` is counted.)
fn assert_one_internal_error(drive: impl FnOnce(&mut TransferRecorder), needle: &str) {
    let mut recorder = TransferRecorder::default();
    drive(&mut recorder);
    let record = recorder.finish(&BindingNames::default(), &NonOwnedReads::default());
    let messages = record.internal_errors();
    let mentioning = messages
        .iter()
        .filter(|message| message.contains(needle))
        .count();
    assert_eq!(
        mentioning, 1,
        "expected one internal error containing `{needle}`, got {messages:#?}"
    );
}

fn key(start: usize, name: &str) -> (usize, usize, String) {
    (start, start + 1, name.to_string())
}

/// One named test per event the recorder cannot place:
/// `internal_error_cases!(name: |recorder| drive, "needle", "bug it catches (High)";)`.
macro_rules! internal_error_cases {
    ($($case:ident: |$recorder:ident| $drive:block, $needle:literal, $bug:literal;)+) => {
        $(
            // bug: the transfer recorder drops an event it cannot place instead of reporting an internal error, so a release pass trusts a partition that was never built | tier: High
            #[doc = $bug]
            #[test]
            fn $case() {
                assert_one_internal_error(|$recorder| $drive, $needle);
            }
        )+
    };
}

internal_error_cases! {
    an_event_before_any_function_is_an_internal_error: |recorder| {
        recorder.join(&key(0, "first"), &key(2, "second"), None)
    }, "while no function was being walked", "(High) an event with no function being walked used to return silently, so the record claimed a partition it never built";
    a_join_naming_an_undeclared_binding_is_an_internal_error: |recorder| {
        recorder.begin_function("underTest");
        recorder.declare(key(0, "declared"), vec![0], RoleEvidence::Owner, None);
        recorder.join(&key(0, "declared"), &key(2, "undeclared"), None);
    }, "in `underTest`, which it never saw declared", "(High) a join naming a binding the function never declared used to be dropped, leaving two values in separate classes that one value reaches";
    an_escape_of_an_undeclared_binding_is_an_internal_error: |recorder| {
        recorder.begin_function("underTest");
        recorder.escape(&key(2, "undeclared"), Escape::Returned);
    }, "in `underTest`, which it never saw declared", "(High) an escape of an undeclared binding used to be dropped, so a value that leaves the function looked as if it stayed and a release pass would free it";
    a_binding_event_for_an_undeclared_binding_is_an_internal_error: |recorder| {
        recorder.begin_function("underTest");
        recorder.note_event(&key(2, "undeclared"), RoleEvidence::Piece, None);
    }, "in `underTest`, which it never saw declared", "(High) a reassignment's role evidence for an undeclared binding used to be dropped, so a piece could be recorded as an owner";
    a_declared_binding_with_no_codegen_name_is_an_internal_error: |recorder| {
        recorder.begin_function("underTest");
        recorder.declare(key(0, "declared"), vec![0], RoleEvidence::Owner, None);
    }, "has no codegen name from type checking", "(High) a declared binding the name table lacks used to be skipped when the record was built, so its function vanished from the record without an error";
}
