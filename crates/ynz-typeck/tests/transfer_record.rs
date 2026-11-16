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
// WHY this many tests for one High-tier unit (the SOP says stop at about 8 and justify): the
// unit is the whole ownership record, and each test is one way the checker can lose a transfer
// at its own code site (a field store, a container store, an index store, a reassignment, a
// literal, a `let`, a callee's store, a call-result receiver, a loop cell, a builtin result, a
// loop give, a give handed up from an inner loop, a give made in a `while` condition, five kinds
// of event the recorder cannot place). The case tables below are one NAMED test per case, so the
// runner parallelizes them and names the failing program; a case is in a table only when it takes
// a different checker path. The accepted-program guards and
// `a_given_then_revived_binding_keeps_its_given_escape` were green on arrival on purpose: they pin
// over-rejection and the record's flow-insensitivity against the path-aware revive
// (`revive_paths.rs`), which must not leak into the record (a record that forgot the give would
// let a release pass free the class early).

#[macro_use]
mod support;

use support::{
    assert_accepted, assert_rejected_with, assert_use_after_give, error_diagnostics, errors, run,
    EAT,
};
use ynz_diagnostics::DiagnosticKind;
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::binding_names::BindingNames;
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

/// Run `source`, require zero errors, return the record of `function_name`.
fn transfers_of(source: &str, function_name: &str) -> FunctionTransfers {
    record_of(&run(source), function_name)
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

const BAG: &str = "shape Bag {\n  items: array<int>\n}\n";
const PAIR: &str = "shape Pair {\n  first: array<int>\n  second: array<int>\n}\n";

/// A callee that takes a whole `Bag` (`give`), and a maker of a fresh one to reassign it with.
const EAT_BAG: &str = "shape Bag {\n  items: array<int>\n}
function eatBag(give bag: Bag) -> nothing {
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

// ── the S3 holes: a heap value stored without a sink ────────────────────────

/// (High) t6b `bag.items = numbers; return bag`: the field-set stores `numbers` in `bag`, so
/// `numbers` is handed over: the record notes the transfer on `numbers`, and `bag` (which owns the
/// value now) escapes by return. Before, the two shared one class and a release pass could not tell
/// who frees the array.
#[test]
fn t6b_field_set_hands_the_value_to_the_field() {
    let source = format!(
        "{BAG}function make() -> Bag {{
  let numbers: array<int> = [1, 2, 3]
  let bag: Bag = {{ items: [0] }}
  bag.items = numbers
  return bag
}}"
    );
    let transfers = transfers_of(&source, "make");
    assert!(!transfers.same_class("numbers", "bag"), "{transfers:#?}");
    assert_eq!(
        escapes_of(&transfers, "numbers"),
        vec![stored("`bag.items`")]
    );
    assert_eq!(escapes_of(&transfers, "bag"), vec![Escape::Returned]);
}

/// (High) g1 `container.add(numbers); return container`: the container `.add` hands `numbers` over
/// to `container`.
#[test]
fn g1_container_add_hands_the_value_to_the_container() {
    let source = "function make() -> array<array<int>> {
  let numbers: array<int> = [1, 2, 3]
  let container: array<array<int>> = []
  container.add(numbers)
  return container
}";
    let transfers = transfers_of(source, "make");
    assert!(
        !transfers.same_class("numbers", "container"),
        "{transfers:#?}"
    );
    assert_eq!(
        escapes_of(&transfers, "numbers"),
        vec![stored("`container`")]
    );
    assert_eq!(escapes_of(&transfers, "container"), vec![Escape::Returned]);
}

/// (High) g8 `container[0] = numbers; return container`: the index-set hands `numbers` over to the
/// slot.
#[test]
fn g8_index_set_hands_the_value_to_the_slot() {
    let source = "function make() -> array<array<int>> {
  let numbers: array<int> = [1, 2, 3]
  let container: array<array<int>> = [[0]]
  container[0] = numbers
  return container
}";
    let transfers = transfers_of(source, "make");
    assert!(
        !transfers.same_class("numbers", "container"),
        "{transfers:#?}"
    );
    assert_eq!(
        escapes_of(&transfers, "numbers"),
        vec![stored("`container[0]`")]
    );
    assert_eq!(escapes_of(&transfers, "container"), vec![Escape::Returned]);
}

/// (High) g7 `acc = per` inside a loop: `acc` outlives `per`, so a release at the back-edge of
/// `per` would free what `acc` still holds. The reassignment hands `per` over: `per` escapes, and
/// `acc` is the founder of its own class (it owns the value it now holds).
#[test]
fn g7_assign_from_a_loop_local_hands_the_value_to_the_outliving_binding() {
    let source = "function underTest() -> int {
  let acc: array<int> = [0]
  let i: int = 0
  while (i < 2) {
    let per: array<int> = [i, i, i]
    acc = per
    i = i + 1
  }
  return acc.count()
}";
    let transfers = transfers_of(source, "underTest");
    assert!(!transfers.same_class("acc", "per"), "{transfers:#?}");
    assert_eq!(escapes_of(&transfers, "per"), vec![stored("`acc`")]);
    assert_eq!(founder_name(&transfers, "acc"), "acc");
}

/// (High) t7 `{ first: numbers, second: moreNumbers }` returned through `pair`: the literal takes
/// over EVERY named heap part, not only the first. Before, `moreNumbers` was released while the
/// returned `pair` held it.
#[test]
fn t7_struct_literal_hands_every_part_to_the_literal() {
    let source = format!(
        "{PAIR}function make() -> Pair {{
  let numbers: array<int> = [1, 2, 3]
  let moreNumbers: array<int> = [4, 5]
  let pair: Pair = {{ first: numbers, second: moreNumbers }}
  return pair
}}"
    );
    let transfers = transfers_of(&source, "make");
    assert!(
        !transfers.same_class("numbers", "moreNumbers"),
        "{transfers:#?}"
    );
    assert_eq!(
        escapes_of(&transfers, "numbers"),
        vec![stored(
            "the literal `{ first: numbers, second: moreNumbers }`"
        )]
    );
    assert_eq!(
        escapes_of(&transfers, "moreNumbers"),
        vec![stored(
            "the literal `{ first: numbers, second: moreNumbers }`"
        )]
    );
    assert_eq!(escapes_of(&transfers, "pair"), vec![Escape::Returned]);
}

/// (High) `let second = first`: the second name takes the value over, so `first` is handed to
/// `second` and `second` is the founder of the value it now owns.
#[test]
fn let_of_a_whole_binding_hands_the_value_to_the_new_binding() {
    let source = "function underTest() -> int {
  let first: array<int> = [1]
  let second: array<int> = first
  return second.count()
}";
    let transfers = transfers_of(source, "underTest");
    assert!(!transfers.same_class("first", "second"), "{transfers:#?}");
    assert_eq!(escapes_of(&transfers, "first"), vec![stored("`second`")]);
    assert_eq!(founder_name(&transfers, "second"), "second");
}

// ── the same programs, read after the store: rejected (deny and teach) ──────

/// The store hands `numbers` over, so this read of `numbers` is the compile error with the store's
/// text.
fn assert_read_after_store_rejected(source: &str) {
    assert_rejected_with(source, "was stored in");
}

cases! { assert_read_after_store_rejected;
    read_after_container_add_is_rejected: "function underTest() -> int {
  let numbers: array<int> = [1, 2, 3]
  let container: array<array<int>> = []
  container.add(numbers)
  return numbers.count() + container.count()
}", "(High) `container.add(numbers)` then reading `numbers` and `container` (the old `store_then_read_both`, `bg_arg_alias_container_add_red`): accepted before, so a release pass freed `numbers`'s array under the container";
    read_after_field_set_is_rejected: format!("{BAG}function underTest() -> int {{
  let numbers: array<int> = [1, 2, 3]
  let bag: Bag = {{ items: [0] }}
  bag.items = numbers
  return numbers.count() + bag.items.count()
}}"), "(High) t6b: `bag.items = numbers` then reading `numbers`";
    read_after_index_set_is_rejected: "function underTest() -> int {
  let numbers: array<int> = [1, 2, 3]
  let container: array<array<int>> = [[0]]
  container[0] = numbers
  return numbers.count() + container.count()
}", "(High) g8: `container[0] = numbers` then reading `numbers`";
    read_after_let_is_rejected: "function underTest() -> int {
  let first: array<int> = [1]
  let second: array<int> = first
  return first.count() + second.count()
}", "(High) `let second = first` then reading `first`: two names for one value, the old silent alias";
    read_after_assign_is_rejected: "function underTest() -> int {
  let acc: array<int> = [0]
  let per: array<int> = [1]
  acc = per
  return per.count() + acc.count()
}", "(High) g7: `acc = per` then reading `per`";
    read_after_map_set_key_and_value_is_rejected: "function underTest() -> int {
  let numbers: array<int> = [1]
  let table: map<int, array<int>> = {}
  table.set(1, numbers)
  return numbers.count() + table.count()
}", "(High) `table.set(1, numbers)` then reading `numbers`: the map's own table of values is the other owner";
    read_after_literal_part_is_rejected: format!("{PAIR}function underTest() -> int {{
  let numbers: array<int> = [1, 2, 3]
  let moreNumbers: array<int> = [4, 5]
  let pair: Pair = {{ first: numbers, second: moreNumbers }}
  return moreNumbers.count() + pair.first.count()
}}"), "(High) t7: `{ first: numbers, second: moreNumbers }` then reading `moreNumbers`: every part is handed over, not only the first";
}

// ── a read inside the `is` arm that narrowed the handed-over binding ────────

/// A union with one shape variant that holds heap data, and a callee that takes it over.
const FIGURE: &str = "shape Circle {
  label: array<int>
}
shape Square {
  side: int
}
shape Figure = Circle | Square
function eatCircle(give circle: Circle) -> nothing {
  print(circle.label.count().toString())
}
";

cases! { assert_read_after_store_rejected;
    read_after_store_in_a_narrowed_arm_is_rejected: format!("{FIGURE}function underTest(give figure: Figure) -> nothing {{
  let circles: array<Circle> = []
  if (figure) {{
    is Circle => {{
      circles.add(figure)
      print(figure.label.count().toString())
    }}
    is Square => {{ }}
  }}
}}"), "(High) `circles.add(figure)` inside the `is Circle` arm, then reading `figure` in the same arm: the arm's narrowed read skipped the consumed check, so a release pass freed the label under the container";
    read_after_let_in_a_narrowed_arm_is_rejected: format!("{FIGURE}function underTest(give figure: Figure) -> nothing {{
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
    read_after_give_in_a_narrowed_arm_is_rejected: format!("{FIGURE}function underTest(give figure: Figure) -> nothing {{
  if (figure) {{
    is Circle => {{
      eatCircle(figure)
      print(figure.label.count().toString())
    }}
    is Square => {{ }}
  }}
}}"), "(High) a give inside the `is Circle` arm, then reading `figure` in the arm: the narrowed read skipped the use-after-give check";
    second_give_in_a_narrowed_arm_is_rejected: format!("{FIGURE}function underTest(give figure: Figure) -> nothing {{
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
    reads_before_the_store_in_a_narrowed_arm_are_accepted: format!("{FIGURE}function underTest(give figure: Figure) -> nothing {{
  let circles: array<Circle> = []
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
    "function eatBoth(give numbers: array<int>, give rows: array<array<int>>) -> nothing {
  print(numbers.count().toString())
}
function readAndEat(share numbers: array<int>, give rows: array<array<int>>) -> nothing {
  print(numbers.count().toString())
}
function eatBack(give numbers: array<int>) -> array<int> {
  return numbers
}
function countOf(give numbers: array<int>) -> int {
  return numbers.count()
}
";

cases! { assert_read_after_store_rejected;
    give_then_literal_store_in_one_call_is_rejected: format!("{EATERS}function underTest() -> nothing {{
  let numbers: array<int> = [1]
  eatBoth(numbers, [numbers])
}}"), "(High) `eatBoth(numbers, [numbers])`: the literal takes `numbers` over and the call is given `numbers` too; two owners, a double free under a release pass";
    share_then_literal_store_in_one_call_is_rejected: format!("{EATERS}function underTest() -> nothing {{
  let numbers: array<int> = [1]
  readAndEat(numbers, [numbers])
}}"), "(High) `readAndEat(numbers, [numbers])`: the callee reads `numbers` while it owns `rows`, which holds the same array; a read after free inside the callee";
}

cases! { assert_use_after_give;
    literal_store_then_give_in_one_literal_is_rejected: format!("{EATERS}function underTest() -> nothing {{
  let numbers: array<int> = [1]
  let container: array<array<int>> = [numbers, eatBack(numbers)]
  print(container.count().toString())
}}"), "(High) `[numbers, eatBack(numbers)]`: `eatBack` takes `numbers` and hands it back, and the literal stores `numbers` as well; the literal holds one array twice";
    collection_store_then_give_in_one_call_is_rejected: format!("{EATERS}function underTest() -> nothing {{
  let numbers: array<int> = [1]
  let table: map<array<int>, array<int>> = {{}}
  table.set(numbers, eatBack(numbers))
}}"), "(High) `table.set(numbers, eatBack(numbers))`: the key and the value are the same array, two owners in one map";
    index_store_after_a_give_in_the_index_is_rejected: format!("{EATERS}function underTest() -> nothing {{
  let numbers: array<int> = [1]
  let container: array<array<int>> = [[0], [0]]
  container[countOf(numbers) - 1] = numbers
}}"), "(High) `container[countOf(numbers) - 1] = numbers`: the index gives `numbers` away and the store hands it to `container` too";
}

// ── `self` handed over, then read ───────────────────────────────────────────
//
// `self` is a binding like any other, but its read never reached the consumed-read check, so a
// `give self` handed over or given away stayed readable (and a second store was skipped silently).

const KEEPER: &str = "shape Box {
  items: array<int>
}
function eat(give box: Box) -> nothing {
  print(box.items.count().toString())
}
";

cases! { assert_read_after_store_rejected;
    read_of_self_after_a_store_is_rejected: format!("{KEEPER}function keep(give self: Box, lend boxes: array<Box>) -> nothing {{
  boxes.add(self)
  print(self.items.count().toString())
}}"), "(High) `boxes.add(self)` then reading `self`: the container frees the box under the read";
    second_store_of_self_is_rejected: format!("{KEEPER}function keep(give self: Box, lend boxes: array<Box>) -> nothing {{
  boxes.add(self)
  boxes.add(self)
}}"), "(High) `self` stored twice: one box, two slots, a double free";
    read_of_self_after_a_let_is_rejected: format!("{KEEPER}function keep(give self: Box) -> nothing {{
  let mine = self
  print(self.items.count().toString())
  print(mine.items.count().toString())
}}"), "(High) `let mine = self` then reading `self`: two names for one box";
}

cases! { assert_use_after_give;
    read_of_self_after_a_give_is_rejected: format!("{KEEPER}function keep(give self: Box) -> nothing {{
  eat(self)
  print(self.items.count().toString())
}}"), "(High) `eat(self)` then reading `self`: `eat` frees the box under the read";
}

cases! { assert_loop_store_rejected;
    store_of_self_in_a_loop_is_rejected: format!("{KEEPER}function keep(give self: Box, lend boxes: array<Box>) -> nothing {{
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
    copy_then_store_in_one_literal_is_accepted: format!("{EATERS}function underTest() -> nothing {{
  let numbers: array<int> = [1]
  let boxes: array<array<int>> = [numbers.copy(), numbers]
  print(boxes.count().toString())
}}"), "(High) over-rejection: `[numbers.copy(), numbers]` stores a copy and then `numbers`; the copy is a fresh value, `numbers` is handed over once";
    read_in_the_key_then_store_in_one_call_is_accepted: format!("{EATERS}function underTest() -> nothing {{
  let numbers: array<int> = [1]
  let table: map<int, array<int>> = {{}}
  table.set(numbers.count(), numbers)
}}"), "(High) over-rejection: `table.set(numbers.count(), numbers)` reads `numbers` for the key and then hands it over; a read that ends before the hand-over is legal";
    copy_of_the_value_beside_a_give_in_one_call_is_accepted: format!("{EATERS}function underTest() -> nothing {{
  let numbers: array<int> = [1]
  eatBoth(numbers, [numbers.copy()])
}}"), "(High) over-rejection: `eatBoth(numbers, [numbers.copy()])` gives `numbers` once; the literal holds a copy";
    share_self_read_twice_is_accepted: format!("{KEEPER}function twice(share self: Box) -> int {{
  return self.items.count() + self.items.count()
}}"), "(High) over-rejection: a `share self` is never handed over, so reading it again is legal";
    give_self_stored_once_is_accepted: format!("{KEEPER}function keep(give self: Box, lend boxes: array<Box>) -> nothing {{
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
        "{EAT}function underTest(condition: boolean) -> nothing {{
  let numbers: array<int> = [1, 2, 3]
  let i: int = 0
  while (i < 3) {{
    if (condition) {{ return }}
    eat(numbers)
    i = i + 1
  }}
}}"
    ), "(High) t3, the exact spike probe: an outer binding given on one pass of a `while` is gone on the next; typeck accepted it, so a release pass would double-free";
    give_in_a_for_loop_is_rejected: format!(
        "{EAT}function underTest() -> nothing {{
  let numbers: array<int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    eat(numbers)
  }}
}}"
    ), "(High) t3 in numbers `for` loop: the second loop shape is checked by its own code path";
    give_of_a_binding_that_took_a_value_over_in_a_loop_is_rejected: format!(
        "{EAT}function underTest() -> nothing {{
  let numbers: array<int> = [1, 2, 3]
  let newOwner: array<int> = numbers
  for (i in range(0, 3)) {{
    eat(newOwner)
  }}
}}"
    ), "(High) t3 through a hand-over: `let newOwner = numbers` handed `numbers`'s value to `newOwner`, which is declared outside the loop and given on every pass";
    give_in_a_while_condition_is_rejected: format!(
        "function holds(give rows: array<int>) -> boolean {{
  return rows.count() > 0
}}
{EAT}function underTest() -> nothing {{
  let numbers: array<int> = [1, 2, 3]
  while (holds(numbers)) {{
    print(`pass`)
  }}
}}"
    ), "(High) t3 through the condition: a `while` condition runs at the top of every pass, so a give made in it reaches the next pass; the loop was opened only after the condition was checked, so typeck accepted a double give";
    give_in_an_inner_loop_reaches_the_outer_pass: format!(
        "{EAT_BAG}function underTest() -> nothing {{
  let bag: Bag = {{ items: [1, 2, 3] }}
  let view: array<int> = bag.items
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
    store_of_an_outer_binding_in_a_loop_is_rejected: "function underTest() -> nothing {
  let numbers: array<int> = [1, 2, 3]
  for (j in range(0, 2)) {
    let newOwner: array<int> = numbers
    print(newOwner.count().toString())
  }
}", "(High) t3 for a store: `let newOwner = numbers` hands `numbers` over on every pass, and the next pass reads `numbers` again (a release pass would free it once per pass)";
    container_add_of_an_outer_binding_in_a_loop_is_rejected: "function underTest() -> nothing {
  let numbers: array<int> = [1, 2, 3]
  let container: array<array<int>> = []
  for (j in range(0, 2)) {
    container.add(numbers)
  }
}", "(High) t3 for a container store: `container.add(numbers)` hands `numbers` over on every pass";
}

cases! { assert_loop_send_rejected;
    send_in_a_for_loop_is_rejected: "function producer(lend wire: channel<array<int>>) -> nothing {
  let sent: array<int> = [10, 20, 30]
  for (i in range(0, 3)) {
    wire.send(sent)
  }
  wire.close()
}", "(High) t3 for `send`: the loop-carried check also covers a send, whose wording is its own arm; an accepted program sends one array twice and two receivers free it";
}

cases! { assert_accepted;
    give_of_a_loop_local_is_accepted: format!(
        "{EAT}function underTest() -> nothing {{
  for (i in range(0, 3)) {{
    let numbers: array<int> = [1, 2, 3]
    eat(numbers)
  }}
}}"
    ), "(High) over-rejection: a give of a binding the loop itself declares carries nothing into the next pass";
    give_then_reassign_is_accepted: format!(
        "{EAT}function underTest() -> nothing {{
  let numbers: array<int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    eat(numbers)
    numbers = [4, 5]
  }}
}}"
    ), "(High) over-rejection: a reassignment before the back-edge revives the binding";
    give_on_a_path_that_returns_is_accepted: format!(
        "{EAT}function underTest(condition: boolean) -> nothing {{
  let numbers: array<int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    if (condition) {{
      eat(numbers)
      return
    }}
  }}
}}"
    ), "(High) over-rejection: a give on a path that leaves the loop never reaches the next pass";
    unread_class_mate_of_a_revived_binding_is_accepted: format!(
        "{EAT_BAG}function underTest() -> nothing {{
  let bag: Bag = {{ items: [1, 2, 3] }}
  let view: array<int> = bag.items
  for (i in range(0, 3)) {{
    eatBag(bag)
    bag = freshBag()
  }}
}}"
    ), "(High) over-rejection: a class-mate the loop never mentions stays consumed after the given binding is revived, but nothing in the next pass reads it";
    give_after_the_loop_is_accepted: format!(
        "{EAT}function underTest() -> nothing {{
  let numbers: array<int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    print(numbers.count().toString())
  }}
  eat(numbers)
}}"
    ), "(High) over-rejection: a give after the loop is the ordinary single give";
    give_of_an_outer_loop_local_in_an_inner_loop_is_accepted: format!(
        "{EAT}function underTest() -> nothing {{
  for (j in range(0, 2)) {{
    let numbers: array<int> = [1, 2, 3]
    for (i in range(0, 1)) {{
      eat(numbers)
      numbers = [0]
    }}
  }}
}}"
    ), "(High) over-rejection: a give the inner loop hands up is of a binding the outer loop declares afresh each pass, so the outer loop carries nothing";
}

// ── the record's boundaries (no over-joining, anchors, escapes) ─────────────

/// (High) Bits are copied, not handed over: `container.add(numbers[0].or(0))` stores an int and `let
/// piece = numbers.count()` is a number, so `numbers` stays readable and nothing is recorded for
/// it.
#[test]
fn storing_bits_hands_nothing_over() {
    let source = "function underTest() -> int {
  let numbers: array<int> = [1, 2, 3]
  let container: array<int> = []
  container.add(numbers[0].or(0))
  container.add(numbers.count())
  return container.count() + numbers.count()
}";
    let transfers = transfers_of(source, "underTest");
    assert_eq!(escapes_of(&transfers, "numbers"), Vec::<Escape>::new());
}

/// (High) A string is immortal bytes under C3, so storing one hands nothing over and the name
/// stays readable.
#[test]
fn storing_a_string_hands_nothing_over() {
    assert_accepted(
        "function underTest() -> int {
  let name: string = `ada`
  let names: array<string> = []
  names.add(name)
  let same: string = name
  return names.count() + same.count()
}",
    );
}

/// (High) A `sensitive string` is the string pointer with a label; its copy is the receiver's bits
/// (`owned_copy_plan`), so storing one hands nothing over and the name stays readable.
#[test]
fn storing_a_sensitive_string_hands_nothing_over() {
    assert_accepted(
        "function underTest() -> int {
  let key: sensitive string = sensitive(`abc`)
  let keys: array<sensitive string> = []
  keys.add(key)
  let same: sensitive string = key
  print(key)
  return keys.count()
}",
    );
}

/// (High) A `range` is its start and end and nothing can change them: `owned_copy_plan` calls its
/// copy the receiver's bits, so storing one hands nothing over. `let sameRange = wholeRange` and a
/// struct literal holding `wholeRange` leave `wholeRange` readable, so `for (i in wholeRange)` is a
/// valid program. Every form here builds end to end; an `array<range>` does not (codegen cannot
/// lower a range element yet), so it is not pinned as accepted.
#[test]
fn storing_a_range_hands_nothing_over() {
    assert_accepted(
        "shape Window { span: range }
function underTest() -> int {
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
}",
    );
}

const EAT2: &str = "function eat_all(give rows: array<array<int>>) -> nothing {
  print(rows.count().toString())
}\n";

/// (High) A store of a value the program does not read again is accepted, and the container
/// can be given on: the stored value travels with it, so `numbers`'s transfer is the store and the
/// container's is the give.
#[test]
fn a_store_then_a_give_of_the_container_is_accepted_and_recorded_twice() {
    let source = format!(
        "{EAT2}function underTest() -> nothing {{
  let numbers: array<int> = [1, 2, 3]
  let container: array<array<int>> = []
  container.add(numbers)
  eat_all(container)
}}"
    );
    let transfers = transfers_of(&source, "underTest");
    assert_eq!(
        escapes_of(&transfers, "numbers"),
        vec![stored("`container`")]
    );
    assert_eq!(
        escapes_of(&transfers, "container"),
        vec![Escape::Given {
            callee: "eat_all".to_string()
        }]
    );
}

// Escapes are recorded per class: a store, a give, a send and a store into a temporary are
// facts the release pass reads (a `return` is asserted by the S3 tests above).

/// (High) A `send` is recorded as `Sent` on the channel: a release pass that read no escape
/// would free the sent array at the producer's scope exit while the receiver reads it.
#[test]
fn a_send_is_recorded_as_an_escape_on_its_channel() {
    let source = "function producer(lend wire: channel<array<int>>) -> nothing {
  let sent: array<int> = [10, 20, 30]
  wire.send(sent)
  wire.close()
}";
    let transfers = transfers_of(source, "producer");
    assert_eq!(
        escapes_of(&transfers, "sent"),
        vec![Escape::Sent {
            channel: "wire".to_string()
        }]
    );
}

/// (High) A store into a place no binding names (`makeBag().items = numbers`) is recorded, not
/// dropped: the temporary holds `numbers`, so nothing here may release it as a plain local.
#[test]
fn a_store_into_a_temporary_is_recorded_as_an_escape() {
    let source = format!(
        "{BAG}function makeBag() -> Bag {{
  return {{ items: [0] }}
}}
function underTest() -> nothing {{
  let numbers: array<int> = [1, 2, 3]
  makeBag().items = numbers
}}"
    );
    let transfers = transfers_of(&source, "underTest");
    assert_eq!(
        escapes_of(&transfers, "numbers"),
        vec![stored("`makeBag().items`")]
    );
}

/// (High) A store of a parameter the caller keeps (`lend`, `share` or bare) is refused: the
/// caller owns the value, so the container could not own it too.
#[test]
fn storing_a_borrowed_parameter_is_rejected() {
    assert_rejected_with(
        "function stash(lend bucket: array<array<int>>, rows: array<int>) -> nothing {
  bucket.add(rows)
}",
        "is a parameter of `stash`",
    );
}

/// (High) A piece of a parameter the caller keeps stays recorded as the caller's: a class with a
/// borrowed parameter in it is never released here.
#[test]
fn a_borrowed_parameter_in_the_class_means_the_caller_owns_it() {
    let source = "function underTest(share bag: Bag) -> int {
  let view: array<int> = bag.items
  return view.count()
}";
    let transfers = transfers_of(&format!("{BAG}{source}"), "underTest");
    assert!(transfers.same_class("bag", "view"), "{transfers:#?}");
    let class = transfers.class_of("view").expect("class");
    assert!(
        matches!(&class.anchor, ReleaseAnchor::CallerOwns { .. }),
        "{class:#?}"
    );
}

/// (High) The founder is the owner whose scope exits last: a piece declared deeper never anchors
/// a class an outer owner is in.
#[test]
fn the_outer_owner_is_the_founder_of_a_class_with_a_deeper_piece() {
    let source = "function underTest() -> int {
  let outer: Bag = { items: [1] }
  let total: int = 0
  if (true) {
    let inner: array<int> = outer.items
    total = inner.count()
  }
  return outer.items.count() + total
}";
    let transfers = transfers_of(&format!("{BAG}{source}"), "underTest");
    assert_eq!(founder_name(&transfers, "inner"), "outer");
}

/// (High) An imported function that returns a `maybe` holding a `channel` hands back a counted
/// reference of its own (`copied_at_return`: its `return` shares what it does not move, C7 slice 3b;
/// ruled by the Software Architect within the Chief Architect's counted-reference rule). The holder
/// binds the result by move and is its own founder, so `held` and `local` are each released once and
/// no class is left without a static founder (this was the reproducer of `NoStaticFounder`).
#[test]
fn a_maybe_channel_an_imported_function_returns_is_an_owner_in_a_class_of_its_own() {
    let library =
        "export function firstChan(share chans: array<channel<int>>) -> maybe<channel<int>> {
  return chans.first()
}\n";
    let main = "import { firstChan } from `lib`
function underTest(condition: boolean) -> nothing {
  let held: maybe<channel<int>> = none
  if (condition) {
    let local: array<channel<int>> = [channel<int>()]
    held = firstChan(local)
  }
}
function entrypoint() -> nothing {
}
";
    let transfers = record_of(&run_with_library(library, main), "underTest");
    assert!(!transfers.same_class("held", "local"), "{transfers:#?}");
    assert_eq!(founder_name(&transfers, "held"), "held");
    assert_eq!(founder_name(&transfers, "local"), "local");
}

/// (High) An imported function (or a local one) that returns its `channel` argument hands back a
/// counted reference of its own (`ynz_channel_share` at the callee's `return`, `copied_at_return`),
/// so the binding that holds the result owns it. It joins nothing, so `held` and `local` are each the founder of a class of
/// their own and the channel is released once per reference.
#[test]
fn a_handed_back_channel_is_a_counted_reference_in_a_class_of_its_own() {
    let library = "export function same(share ch: channel<int>) -> channel<int> {
  return ch
}\n";
    let main = "import { same } from `lib`
function passOn(share ch: channel<int>) -> channel<int> {
  return ch
}
function viaImport(condition: boolean) -> nothing {
  let held: channel<int> = channel<int>()
  if (condition) {
    let local: channel<int> = channel<int>()
    held = same(local)
  }
}
function viaLocal(condition: boolean) -> nothing {
  let held: channel<int> = channel<int>()
  if (condition) {
    let local: channel<int> = channel<int>()
    held = passOn(local)
  }
}
function entrypoint() -> nothing {
}
";
    let output = run_with_library(library, main);
    for function in ["viaImport", "viaLocal"] {
        let transfers = record_of(&output, function);
        assert!(
            !transfers.same_class("held", "local"),
            "{function}: {transfers:#?}"
        );
        assert_eq!(founder_name(&transfers, "held"), "held");
        assert_eq!(founder_name(&transfers, "local"), "local");
    }
}

/// (High) An imported function that returns a `maybe` (or a union) may hand back a piece of its
/// argument. The binding that holds the result outlives `local`, so it owns a copy
/// (`MustCopy` in `non_owned_reads`): it joins nothing and `local` is its own founder.
#[test]
fn an_untraceable_maybe_result_is_a_copy_owner_in_a_class_of_its_own() {
    let library = "export function firstRow(share rows: array<array<int>>) -> maybe<array<int>> {
  return rows.first()
}\n";
    let main = "import { firstRow } from `lib`
function underTest(condition: boolean) -> int {
  let held: maybe<array<int>> = none
  if (condition) {
    let local: array<array<int>> = [[1]]
    held = firstRow(local)
  }
  return 1
}
function entrypoint() -> nothing {
}
";
    let transfers = record_of(&run_with_library(library, main), "underTest");
    assert!(!transfers.same_class("held", "local"), "{transfers:#?}");
    assert_eq!(founder_name(&transfers, "held"), "held");
    assert_eq!(founder_name(&transfers, "local"), "local");
}

/// (High) The field read that used to leave no static founder (`cell = owner.rows`, `cell`
/// outlives `owner`) is a copy typeck judged `MustCopy`: `cell` owns a value of its own and joins
/// nothing, so each is the founder of a class of its own.
#[test]
fn a_field_read_that_outlives_its_root_is_a_copy_owner_in_a_class_of_its_own() {
    let source = format!(
        "{BUCKET}function make() -> Bucket {{
  let bucket: Bucket = {{ rows: [9] }}
  return bucket
}}
function underTest(condition: boolean) -> int {{
  let cell: array<int> = make().rows
  if (condition) {{
    let owner: Bucket = make()
    cell = owner.rows
  }}
  return cell.count()
}}"
    );
    let transfers = transfers_of(&source, "underTest");
    assert!(!transfers.same_class("cell", "owner"), "{transfers:#?}");
    assert_eq!(founder_name(&transfers, "cell"), "cell");
    assert_eq!(founder_name(&transfers, "owner"), "owner");
}

const BUCKET: &str = "shape Bucket {\n  rows: array<int>\n}\n";

// ── a call that stores one argument into another ────────────────────────────

const PUT: &str =
    "function put(lend container: array<array<int>>, give numbers: array<int>) -> nothing {
  container.add(numbers)
}
function relay(lend container: array<array<int>>, give numbers: array<int>) -> nothing {
  put(container, numbers)
}\n";

/// (High) A callee that stores a parameter it does not own would hand the caller's value to its
/// container behind the caller's back: the store is refused inside the callee, which must say
/// `give` (so every caller hands the value over for good).
#[test]
fn a_callee_that_stores_a_bare_parameter_is_rejected() {
    assert_rejected_with(
        "function put(lend container: array<array<int>>, numbers: array<int>) -> nothing {
  container.add(numbers)
}",
        "is a parameter of `put` that has no ownership word",
    );
}

fn assert_read_after_call_give_rejected(source: &str) {
    assert_use_after_give(source);
}

cases! { assert_read_after_call_give_rejected;
    read_after_a_direct_call_store_is_rejected: format!("{PUT}function underTest() -> int {{
  let numbers: array<int> = [1, 2, 3]
  let container: array<array<int>> = []
  put(container, numbers)
  return numbers.count()
}}"), "(High) `put(container, numbers)` stores `numbers` INSIDE the callee: the caller's `numbers` is given away at the call, so the read is refused (before, the call joined the two classes and `numbers` stayed readable)";
    read_after_a_relayed_call_store_is_rejected: format!("{PUT}function underTest() -> int {{
  let numbers: array<int> = [1, 2, 3]
  let container: array<array<int>> = []
  relay(container, numbers)
  return numbers.count()
}}"), "(High) the store happens in the callee's own call (`relay` -> `put`): `relay`'s parameter is `give` too";
}

/// (High) `put(container, numbers)` records the give on the caller's `numbers`: a release pass sees
/// `numbers` leave.
#[test]
fn a_call_that_stores_a_given_argument_is_recorded_as_a_give() {
    let source = format!(
        "{PUT}function make() -> array<array<int>> {{
  let numbers: array<int> = [1, 2, 3]
  let container: array<array<int>> = []
  put(container, numbers)
  return container
}}"
    );
    let transfers = transfers_of(&source, "make");
    assert!(
        !transfers.same_class("numbers", "container"),
        "{transfers:#?}"
    );
    assert_eq!(
        escapes_of(&transfers, "numbers"),
        vec![Escape::Given {
            callee: "put".to_string()
        }]
    );
}

/// (High) The converse: a callee that only READS `from` into `into` takes nothing over, so both
/// stay usable (no false rejection of a program that is correct).
#[test]
fn a_callee_that_only_reads_an_argument_leaves_it_readable() {
    assert_accepted(
        "function tally(lend into: array<int>, from: array<int>) -> nothing {
  into.add(from.count())
}
function underTest() -> int {
  let from: array<int> = [1, 2, 3]
  let into: array<int> = []
  tally(into, from)
  return from.count() + into.count()
}",
    );
}

// ── the remaining no-owner cases ────────────────────────────────────────────

fn assert_no_owner(transfers: &FunctionTransfers, member: &str) {
    let class = transfers
        .class_of(member)
        .unwrap_or_else(|| panic!("`{member}` is in no class"));
    assert_eq!(class.anchor, ReleaseAnchor::NoOwner, "{class:#?}");
}

/// (High) A string, array or map field of a temporary that a `let` keeps is TAKEN, not copied
/// (`ReadVerdict::Take`): the binding is the owner of the field, a founder that releases it.
/// This binding was `NoOwner` before the take existed (C7 slice 2, item 6: the pin
/// `a_piece_of_a_temporary_is_recorded_as_no_owner` moved here).
#[test]
fn a_field_taken_out_of_a_temporary_is_owned_by_its_binding() {
    let source = format!(
        "{BUCKET}function make() -> Bucket {{
  let bucket: Bucket = {{ rows: [9] }}
  return bucket
}}
function underTest() -> int {{
  let cell: array<int> = make().rows
  return cell.count()
}}"
    );
    let transfers = transfers_of(&source, "underTest");
    let class = transfers.class_of("cell").expect("`cell` is in a class");
    assert!(
        matches!(&class.anchor, ReleaseAnchor::Founder { codegen_name, .. } if codegen_name == "cell"),
        "{class:#?}"
    );
}

/// (High) `NoOwner` for a loop cell of a temporary (`for (row in make())`).
#[test]
fn a_loop_cell_of_a_temporary_is_recorded_as_no_owner() {
    let source = "function make() -> array<array<int>> {
  let rows: array<array<int>> = [[1]]
  return rows
}
function underTest() -> int {
  let total: int = 0
  for (row in make()) {
    total = total + row.count()
  }
  return total
}";
    assert_no_owner(&transfers_of(source, "underTest"), "row");
}

/// (High) An IMPORTED function's result of a type copied at `return` is an owned value, as a local
/// function's is: the declared return type says what the unseen `return` does, so the binding that
/// holds it founds its own class and releases it (C7 slice 3b; it was `NoOwner`, a leak).
#[test]
fn an_imported_functions_result_is_owned_by_the_binding_that_holds_it() {
    let library = "export function fresh() -> array<int> {
  return [1]
}\n";
    let main = "import { fresh } from `lib`
function underTest() -> int {
  let result: array<int> = fresh()
  return result.count()
}
function entrypoint() -> nothing {
}
";
    let transfers = record_of(&run_with_library(library, main), "underTest");
    assert_eq!(founder_name(&transfers, "result"), "result");
}

// ── inference that feeds the record ─────────────────────────────────────────

/// (High) A `background` spawn inside a loop of a binding declared OUTSIDE it: the next pass
/// spawns with the same binding, so the first spawn must copy, never give (a second give is a
/// second owner of one array).
#[test]
fn a_spawn_in_a_loop_of_an_outer_binding_copies_it() {
    use ynz_typeck::check::BgOwnership;
    let spawn_in = |declared_outside: bool| -> Vec<BgOwnership> {
        let (outside, inside) = if declared_outside {
            ("  let rows: array<int> = [1, 2, 3]\n", "")
        } else {
            ("", "    let rows: array<int> = [1, 2, 3]\n")
        };
        let source = format!(
            "function work(rows: array<int>) -> nothing {{
  print(rows.count().toString())
}}
function underTest() -> nothing {{
{outside}  for (i in range(0, 3)) {{
{inside}    background work(rows)
  }}
}}"
        );
        let output = run(&source);
        let found = errors(&output);
        assert!(found.is_empty(), "{found:#?}");
        output
            .typed_module
            .background_arg_inferred_ownership
            .values()
            .cloned()
            .collect()
    };
    assert_eq!(spawn_in(true), vec![BgOwnership::Copy]);
    assert_eq!(spawn_in(false), vec![BgOwnership::Give]);
}

/// (High) A builtin method's result can be an ARGUMENT, not only a piece of the receiver:
/// `cache.first().or(fallback)` returns `fallback` when the cache is empty (a view while nothing
/// copies it, a copy that owns its value once it is returned), `left.concat(right)`
/// holds the elements of both. Returned, `fallback` / `right` must escape with it.
#[test]
fn a_builtin_methods_result_reaches_its_heap_arguments() {
    let or_source = "function pick(cache: array<array<int>>) -> array<int> {
  let fallback: array<int> = [1]
  let found: array<int> = cache.first().or(fallback)
  return found
}";
    let transfers = transfers_of(or_source, "pick");
    // `found` is returned, so typeck judges `cache.first().or(fallback)` a copy: `found` owns a value
    // of its own and joins nothing.
    assert!(!transfers.same_class("found", "fallback"), "{transfers:#?}");
    assert!(transfers
        .class_of("found")
        .is_some_and(|class| class.members[0].copy_owner));

    let concat_source = "function both() -> array<array<int>> {
  let left: array<array<int>> = [[1]]
  let right: array<array<int>> = [[2]]
  let joined: array<array<int>> = left.concat(right)
  return joined
}";
    let transfers = transfers_of(concat_source, "both");
    assert!(transfers.same_class("left", "right"), "{transfers:#?}");
    assert_eq!(escapes_of(&transfers, "right"), vec![Escape::Returned]);
}

// ── receivers that are not a name chain (fixer round 1) ─────────────────────

const FIRST_BAG: &str = "shape Bag {
  items: array<int>
}
function firstBag<T>(share bags: array<T>, share fallback: T) -> T {
  return bags[0].or(fallback)
}
";

/// (High) Bug caught: `firstBag(bags, empty).items` is a piece of `bags` (the callee returns one of its
/// elements), but the field access rooted only in NAME chains and recorded no root, so `piece` was
/// free to be released while `bags` still held it. (A generic callee: a function returning a shape, an
/// array, a map or a string hands back an owned copy, so only a type parameter still returns a piece.)
#[test]
fn a_field_of_a_call_result_is_a_piece_of_the_calls_arguments() {
    let source = format!(
        "{FIRST_BAG}function underTest() -> int {{
  let bags: array<Bag> = [{{ items: [1] }}]
  let empty: Bag = {{ items: [] }}
  let piece: array<int> = firstBag(bags, empty).items
  return piece.count()
}}"
    );
    let transfers = transfers_of(&source, "underTest");
    // The read copies the field (a call result that may be a piece of its arguments is not a
    // temporary to take from), so `piece` owns a value of its own: it joins nothing and is
    // released at its own scope exit, and `bags` is never released while `piece` points into it.
    assert!(!transfers.same_class("bags", "piece"), "{transfers:#?}");
    assert_eq!(founder_name(&transfers, "piece"), "piece");
}

/// (High) A generic callee's `return` copies a heap-typed result at its instance, so
/// `firstBag(bags, empty).items` is a field of an owned value, not a piece of `bags`: storing it
/// is accepted and `bags` and `container` hold different arrays. (It was refused as a piece of
/// `bags`, which also left the result unreleased: a leak per call.)
#[test]
fn storing_a_field_of_a_generic_call_result_is_accepted_it_owns_its_value() {
    let source = format!(
        "{FIRST_BAG}function underTest() -> array<array<int>> {{
  let bags: array<Bag> = [{{ items: [1] }}]
  let empty: Bag = {{ items: [] }}
  let container: array<array<int>> = []
  let piece: array<int> = firstBag(bags, empty).items
  container.add(piece)
  return container
}}"
    );
    let transfers = transfers_of(&source, "underTest");
    assert!(!transfers.same_class("bags", "piece"), "{transfers:#?}");
}

/// (High) Bug caught: a store whose PLACE is a call result (`firstBag(bags, empty).items = numbers`) was
/// recorded as a store into a temporary. It hands `numbers` over to the place: `numbers` escapes,
/// whatever the place is.
#[test]
fn a_store_into_a_piece_of_a_call_result_hands_the_value_over() {
    let source = format!(
        "{FIRST_BAG}function underTest() -> nothing {{
  let bags: array<Bag> = [{{ items: [1] }}]
  let empty: Bag = {{ items: [] }}
  let numbers: array<int> = [5]
  firstBag(bags, empty).items = numbers
}}"
    );
    let transfers = transfers_of(&source, "underTest");
    assert_eq!(
        escapes_of(&transfers, "numbers"),
        vec![stored("`firstBag(bags, empty).items`")]
    );
}

/// (High) Bug caught: `for (row in rows.concat(extra))` joined the loop cell to nothing (the
/// iterated expression is a method call, not a name), so `row` was free to outlive `rows`/`extra`.
#[test]
fn a_loop_cell_joins_everything_the_iterated_value_reaches() {
    let source = "function underTest() -> int {
  let rows: array<array<int>> = [[1]]
  let extra: array<array<int>> = [[2]]
  let total: int = 0
  for (row in rows.concat(extra)) {
    total = total + row.count()
  }
  return total
}";
    let transfers = transfers_of(source, "underTest");
    for member in ["rows", "extra", "row"] {
        assert!(
            transfers.same_class("rows", member),
            "{member}: {transfers:#?}"
        );
    }
}

/// (High) Bug caught: a binding given away on one path and revived by a reassignment must keep
/// its `Given` escape (the record is flow-insensitive on purpose). Dropping the escape at the
/// revive would let a release pass free the class on the path where it WAS given away, a double
/// free; keeping it costs only a leak of the revived value.
#[test]
fn a_given_then_revived_binding_keeps_its_given_escape() {
    let source = format!(
        "{EAT}function underTest(condition: boolean) -> int {{
  let numbers: array<int> = [1, 2, 3]
  if (condition) {{
    eat(numbers)
    numbers = [0]
  }}
  return numbers.count()
}}"
    );
    let transfers = transfers_of(&source, "underTest");
    assert_eq!(
        escapes_of(&transfers, "numbers"),
        vec![Escape::Given {
            callee: "eat".to_string()
        }]
    );
}

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

/// (High) Bug caught: an event with no function being walked used to return silently, so the
/// record claimed a partition it never built and a release pass would have trusted it.
#[test]
fn an_event_before_any_function_is_an_internal_error() {
    assert_one_internal_error(
        |recorder| recorder.join(&key(0, "first"), &key(2, "second"), None),
        "while no function was being walked",
    );
}

/// (High) Bug caught: a join naming a binding the function never declared used to be dropped,
/// leaving two values in separate classes that one value reaches.
#[test]
fn a_join_naming_an_undeclared_binding_is_an_internal_error() {
    assert_one_internal_error(
        |recorder| {
            recorder.begin_function("underTest");
            recorder.declare(key(0, "declared"), vec![0], RoleEvidence::Owner, None);
            recorder.join(&key(0, "declared"), &key(2, "undeclared"), None);
        },
        "in `underTest`, which it never saw declared",
    );
}

/// (High) Bug caught: an escape of an undeclared binding used to be dropped, so a value that
/// leaves the function looked as if it stayed and a release pass would free it.
#[test]
fn an_escape_of_an_undeclared_binding_is_an_internal_error() {
    assert_one_internal_error(
        |recorder| {
            recorder.begin_function("underTest");
            recorder.escape(&key(2, "undeclared"), Escape::Returned);
        },
        "in `underTest`, which it never saw declared",
    );
}

/// (High) Bug caught: a reassignment's role evidence for an undeclared binding used to be
/// dropped, so a piece could be recorded as an owner.
#[test]
fn a_binding_event_for_an_undeclared_binding_is_an_internal_error() {
    assert_one_internal_error(
        |recorder| {
            recorder.begin_function("underTest");
            recorder.note_event(&key(2, "undeclared"), RoleEvidence::Piece, None);
        },
        "in `underTest`, which it never saw declared",
    );
}

/// (High) Bug caught: a declared binding the name table lacks used to be skipped when the record
/// was built, so its function vanished from the record without an error.
#[test]
fn a_declared_binding_with_no_codegen_name_is_an_internal_error() {
    let mut recorder = TransferRecorder::default();
    recorder.begin_function("underTest");
    recorder.declare(key(0, "declared"), vec![0], RoleEvidence::Owner, None);
    let record = recorder.finish(&BindingNames::default(), &NonOwnedReads::default());
    let messages = record.internal_errors();
    assert!(
        messages.len() == 1 && messages[0].contains("has no codegen name from type checking"),
        "expected one missing-name internal error, got {messages:#?}"
    );
    assert!(record.function("underTest").is_none());
}
