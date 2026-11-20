// v0.3-M9 C5a round 2: a reassignment revives a given binding only for the code it dominates.
//
// WHY (High tier: ownership / data integrity): `Scope` used to clear the consumed state on ANY
// reassignment, even one inside an `if`, a match arm, or a loop that may run zero times. A later
// give or read of `numbers` then touched a value already given away on some path: a double free or
// a use-after-free under any scope-exit release pass. Rust rejects the same programs. The join
// after a branch is "maybe given" unless EVERY path that reaches it revived the binding; reading
// a maybe-given binding is the ordinary use-after-give error (the canonical `Consumed` text).
//
// Each rejected case failed on the commit it was written against (red commit `e2d4344` for the
// first six, the other two fail on that commit's source too; the security-round cases failed on
// `855b79e`, and all of them but the `{via}` clause cases still failed on `ff122f3`). The
// accepted cases and `read_after_sibling_arms_that_each_give` were green on arrival on purpose:
// they guard that the fix does not reject a program every path of which revives the binding, nor
// accept one every path of which gave it.
//
// WHY this many tests for one High-tier unit (the SOP says stop at about 8 and justify): the unit
// is the join of the consumed state, and its contract is a table over path shapes, one named test
// per shape. A rejected case is a shape the join must keep "maybe given" (an `if` with no else,
// a match arm that does not revive, a match with no else, a `while` body, a zero-trip inner
// loop, a loop-carried back-edge, an arm that gives while another revives, a revive nested under
// two `if`s). An accepted case is a shape the join must still treat as revived (every arm
// including `else`, an arm that always returns, a revive after a loop, a revive both in and after
// a loop, a give and revive inside one branch, the straight-line rule, nested matches). The
// security-round cases add the shapes the join still got wrong: a re-give after a reassignment
// in one arm, a `while` condition, a union with a non-shape variant, the alias class and origin a
// one-path reassignment changes, and sibling arms that each give (one accepted, one read after
// them rejected). Three more tests pin the `{via}` clause of the maybe-given text, one per path
// shape that names a different clause (`if`, `match`, loop): the text contract of the same join,
// not another join shape. Two shapes that take the same checker path are not both here.

#[macro_use]
mod support;

use support::{assert_accepted, assert_use_after_give, EAT};

/// Programs here are checked with `eat` in scope, so the sources below only hold the function
/// under test.
fn assert_revive_rejected(source: &str) {
    assert_use_after_give(&format!("{EAT}{source}"));
}

fn assert_revive_accepted(source: &str) {
    assert_accepted(&format!("{EAT}{source}"));
}

cases! { assert_revive_rejected;
    loop_revive_under_an_if_is_carried: "function underTest(condition: Boolean) -> Nothing {
  let numbers: Array<Int> = [1, 2, 3]
  for (j in range(0, 2)) {
    eat(numbers)
    if (condition) { numbers = [0] }
  }
}", "give, then revive only under `if`, inside a loop: the next pass gives `numbers` again (High)";
}

cases! { assert_revive_rejected;
    loop_revive_in_a_zero_trip_inner_loop_is_carried: "function underTest() -> Nothing {
  let numbers: Array<Int> = [1, 2, 3]
  for (j in range(0, 2)) {
    eat(numbers)
    for (k in range(0, 0)) { numbers = [0] }
  }
}", "give, then revive inside an inner loop that may run zero times: the next pass gives `numbers` again (High)";
}

cases! { assert_revive_rejected;
    read_after_an_if_that_revives_on_one_path: "function underTest(condition: Boolean) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  if (condition) { numbers = [0] }
  return numbers.count()
}", "give, revive only under `if`, then read: on the path where the condition was false `numbers` is gone (High)";
}

cases! { assert_revive_rejected;
    read_after_a_match_whose_else_arm_does_not_revive: "function underTest(choice: Int) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  if (choice) {
    1 => { numbers = [0] }
    else => { print(`other`) }
  }
  return numbers.count()
}", "give, revive in one match arm only, then read: the other arm leaves `numbers` given away (High)";
}

cases! { assert_revive_rejected;
    read_after_a_match_with_no_else_arm: "function underTest(choice: Int) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  if (choice) {
    1 => { numbers = [0] }
    2 => { numbers = [1] }
  }
  return numbers.count()
}", "give, revive in every listed arm of a non-exhaustive match, then read: no arm matching is a path that never revived (High)";
}

cases! { assert_revive_rejected;
    read_after_a_while_that_revives_in_its_body: "function underTest(condition: Boolean) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  while (condition) { numbers = [0] }
  return numbers.count()
}", "give, revive inside a `while` body, then read: the body may run zero times (High)";
}

cases! { assert_revive_accepted;
    revive_in_every_arm_of_a_match_with_else: "function underTest(choice: Int) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  if (choice) {
    1 => { numbers = [0] }
    else => { numbers = [1] }
  }
  return numbers.count()
}", "give, revive in every arm including `else`, then read: every path revived (High)";
}

cases! { assert_revive_accepted;
    revive_beside_an_arm_that_always_returns: "function underTest(choice: Int) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  if (choice) {
    1 => { return 0 }
    else => { numbers = [0] }
  }
  return numbers.count()
}", "give, one arm returns, the other revives, then read: the returning arm never reaches the join (High)";
}

cases! { assert_revive_accepted;
    unconditional_revive_after_a_loop: "function underTest() -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  for (j in range(0, 2)) { print(`tick`) }
  numbers = [0]
  return numbers.count()
}", "give, run a loop that does not mention `numbers`, revive after it, then read (High)";
}

cases! { assert_revive_accepted;
    revive_in_a_loop_body_then_revive_again_after_it: "function underTest() -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  for (j in range(0, 2)) {
    eat(numbers)
    numbers = [0]
  }
  numbers = [1]
  return numbers.count()
}", "give and revive on every pass, then revive after the loop and read (High)";
}

cases! { assert_revive_accepted;
    give_and_revive_both_inside_the_branch: "function underTest(condition: Boolean) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  if (condition) {
    eat(numbers)
    numbers = [0]
  }
  return numbers.count()
}", "give then revive inside one `if` body: both paths leave `numbers` live (High)";
}

cases! { assert_revive_accepted;
    unconditional_revive_straight_line: "function underTest() -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  numbers = [4, 5]
  return numbers.count()
}", "give then revive on the same path, then read: the existing revive-on-reassign rule (High)";
}

cases! { assert_revive_rejected;
    read_after_a_match_where_one_arm_gives_and_the_other_revives: "function underTest(choice: Int) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  if (choice) {
    1 => { eat(numbers) }
    else => { numbers = [0] }
  }
  return numbers.count()
}", "arm 1 gives `numbers`, the else arm revives it, then read: the join must keep arm 1's give, so one arm's revive cannot launder another arm's give (High)";
}

cases! { assert_revive_rejected;
    revive_under_a_nested_if_only: "function underTest(condition: Boolean, otherCondition: Boolean) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  if (condition) {
    if (otherCondition) { numbers = [0] }
  }
  return numbers.count()
}", "give, revive only under two nested `if`s, then read: neither join covers the outer skip path (High)";
}

cases! { assert_revive_accepted;
    revive_in_every_leaf_of_nested_matches: "function underTest(choice: Int, otherChoice: Int) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  if (choice) {
    1 => {
      if (otherChoice) {
        1 => { numbers = [0] }
        else => { numbers = [1] }
      }
    }
    else => { numbers = [2] }
  }
  return numbers.count()
}", "give, revive in every leaf of a match nested in a match, then read: every path revived (High)";
}

/// The maybe-given read keeps the canonical `Consumed` text and adds the `{via}` clause naming
/// the path that did not reassign the binding. `case` is `(the clause, a program)`.
fn assert_maybe_given_names_the_path(case: &(&str, &str)) {
    let (clause, source) = case;
    let program = format!("{EAT}{source}");
    let found = support::errors(&support::run(&program));
    assert!(
        found.iter().any(|message| message.contains(&format!(
            "was already given away and cannot be used here, because {clause} never reassigned it."
        ))),
        "expected the clause `{clause}` for:\n{program}\ngot {found:#?}"
    );
}

cases! { assert_maybe_given_names_the_path;
    maybe_given_after_an_if_names_the_if_path: ("the path where the `if` above did not run", "function underTest(condition: Boolean) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  if (condition) { numbers = [0] }
  return numbers.count()
}"), "a read after an `if` that reassigned on one path said only `already given away`, which over-claims on the reassigned path (High: the wrong text sends the user to the wrong fix)";
}

cases! { assert_maybe_given_names_the_path;
    maybe_given_after_a_match_names_the_match_path: ("a path through the `match` above", "function underTest(choice: Int) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  if (choice) {
    1 => { numbers = [0] }
    else => { print(`other`) }
  }
  return numbers.count()
}"), "the same over-claim after a `match` where one arm does not reassign (High)";
}

cases! { assert_maybe_given_names_the_path;
    maybe_given_after_a_loop_names_the_loop_path: ("the path where the loop above ran zero times", "function underTest(condition: Boolean) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  while (condition) { numbers = [0] }
  return numbers.count()
}"), "the same over-claim after a loop whose body reassigns but may run zero times (High)";
}

// Security round 2: each case below was accepted on `855b79e` (a give or read of a value
// already given away on some path, or a give of a piece another value still holds).

cases! { assert_revive_rejected;
    an_arm_that_reassigns_then_gives_again_stays_given: "function underTest(condition: Boolean) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  if (condition) {
    numbers = [0]
    eat(numbers)
    numbers = [1]
  }
  return numbers.count()
}", "an arm that reassigned a given binding and then gave it again was put back as live, not as given, for the paths after it (High: double give)";
}

cases! { assert_revive_rejected;
    a_while_condition_give_holds_after_the_loop: "function ok(give rows: Array<Int>) -> Boolean {
  return true
}
function underTest() -> Nothing {
  let numbers: Array<Int> = [1, 2, 3]
  while (ok(numbers)) { numbers = [0] }
  eat(numbers)
}", "the last, false check of a `while` condition gives `numbers` on every path out of the loop; a reassignment in the body hid it (High: double give)";
}

cases! { assert_revive_rejected;
    a_union_with_a_non_shape_variant_is_not_exhaustive: "shape Circle { radius: Number }
shape Pair = Circle | Int
function underTest(pairValue: Pair) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  eat(numbers)
  if (pairValue) {
    is Circle => { numbers = [0] }
  }
  return numbers.count()
}", "a union match counted as covering every value though its `int` variant enters no arm (High: read after give)";
}

cases! { assert_revive_rejected;
    a_class_mate_reassigned_on_one_path_still_reaches_the_value_on_the_other: "shape Bag { items: Array<Int> }
function eatBag(give bag: Bag) -> Nothing {
  print(bag.items.count().toString())
}
function underTest(condition: Boolean) -> Int {
  let bag: Bag = { items: [1, 2, 3] }
  let view: Array<Int> = bag.items
  if (condition) { view = [0] }
  eatBag(bag)
  return view.count()
}", "a reassignment under `if` moved `view` out of `bag`'s class on every path, so giving `bag` left `view` readable where it still reaches `bag`'s value (High: read after give). Was `an_alias_reassigned_on_one_path_still_aliases_on_the_other` with a whole-value `let` alias; a whole-value alias cannot exist any more (storing hands the value over), so the class-mate is a piece";
}

/// The give itself must be refused: the value may be a piece another value still holds.
fn assert_give_refused(source: &str) {
    let program = format!("{EAT}{source}");
    let found = support::errors(&support::run(&program));
    assert!(
        found
            .iter()
            .any(|message| message.contains("this line gives it away")),
        "expected the give to be refused for:\n{program}\ngot {found:#?}"
    );
}

cases! { assert_give_refused;
    a_piece_reassigned_on_one_path_is_still_a_piece_on_the_other: "shape Box { items: Array<Int> }
function underTest(condition: Boolean, box: Box) -> Nothing {
  let piece: Array<Int> = box.items
  if (condition) { piece = [0] }
  eat(piece)
}", "a reassignment under `if` made `piece` owned on every path, so a field of a borrowed parameter could be given away (High: double free)";
}

cases! { assert_revive_accepted;
    sibling_arms_may_each_give_the_same_binding: "function underTest(choice: Int) -> Nothing {
  let numbers: Array<Int> = [1, 2, 3]
  if (choice) {
    1 => { eat(numbers) }
    else => { eat(numbers) }
  }
}", "arm 1's give leaked into arm 2, so a binding given once on each of two sibling paths was rejected as already given away (High: false rejection of a valid give)";
}

cases! { assert_revive_rejected;
    read_after_sibling_arms_that_each_give: "function underTest(choice: Int) -> Int {
  let numbers: Array<Int> = [1, 2, 3]
  if (choice) {
    1 => { eat(numbers) }
    else => { eat(numbers) }
  }
  return numbers.count()
}", "every arm gives `numbers`, then a read: given on every path, so the join must still reject it (High: read after give)";
}
