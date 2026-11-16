//! v0.3-M9 C7 review round 3: the round-2 fixes under exits (`return`) taken inside a statement
//! whose temporaries are still live, and a `give` of a place read through a temporary.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    run_balanced,
};

fn assert_balanced_and_clean(label: &str, program: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        assert_heap_balanced(label, &counters, no_optimize);
        assert_strings_balanced(label, &counters, no_optimize);
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

const ERRORS_TEMPORARY_ACROSS_EXITS: &str = "
function make(n: int) -> string errors {
  let a: string = `ab`
  return a.toUpperCase()
}
function makeList(n: int) -> array<string> errors {
  let list: array<string> = [`a`, `bb`, `ccc`]
  return list
}
function scan(limit: int) -> int {
  let i: int = 0
  while (make(i).failed() == false) {
    if (make(i).or(`z`).count() > 1) {
      if (i > limit) {
        return i
      }
    }
    i = i + 1
  }
  return 0
}
function walk(limit: int) -> int {
  let fallback: array<string> = []
  let total: int = 0
  for (word in makeList(limit).or(fallback)) {
    if (word.count() > limit) {
      return total + word.count()
    }
    total = total + word.count()
  }
  return total
}
function entrypoint() -> nothing {
  let t: int = 0
  for (k in range(0, 5)) {
    t = t + scan(k) + walk(k)
  }
  print(t)
}
";

#[test]
fn an_errors_temporary_is_released_at_every_exit_of_the_statement_that_holds_it() {
    assert_balanced_and_clean("c7r3_ec_exits", ERRORS_TEMPORARY_ACROSS_EXITS, "37\n");
}

const GIVE_THROUGH_TEMPORARY: &str = "
function eatText(give text: string) -> int {
  return text.count()
}
function eatPair(give first: string, give second: string) -> int {
  return first.count() + second.count()
}
function findText(share rows: array<string>, index: int) -> maybe<string> {
  return rows.get(index)
}
function make(n: int) -> string errors {
  let a: string = `ab`
  return a.toUpperCase()
}
function entrypoint() -> nothing {
  let rows: array<string> = [`abc`]
  let emptyRows: array<string> = []
  let t: int = 0
  for (i in range(0, 5)) {
    t = t + eatText(findText(rows, 0).or(`z`))
    t = t + eatText(findText(emptyRows, 0).or(`zz`))
    t = t + eatPair(findText(rows, 0).or(`z`), make(i).or(`y`))
    t = t + eatPair(make(i).or(`y`), findText(rows, 0).or(`z`))
  }
  print(t)
}
";

#[test]
fn a_give_of_a_place_read_through_a_temporary_hands_the_callee_a_copy() {
    // per round: 3 + 2 + 5 + 5 = 15
    assert_balanced_and_clean("c7r3_give_tmp", GIVE_THROUGH_TEMPORARY, "75\n");
}

const ERRORS_TEMPORARY_IN_EXPRESSIONS: &str = "
shape Tag {
  name: string
}
function make(n: int) -> string errors {
  let a: string = `ab`
  return a.toUpperCase()
}
function makeTag(n: int) -> Tag errors {
  let tag: Tag = { name: `abc` }
  return tag
}
function join(first: string, second: string) -> string {
  return `${first}${second}`
}
function broken(n: int) -> boolean {
  return make(n).failed()
}
function both(n: int) -> boolean {
  return make(n).failed() == false && makeTag(n).failed() == false
}
function pick(n: int) -> int {
  if (make(n).failed() == false && makeTag(n).or({ name: `z` }).name.count() == 3) {
    return make(n).or(`q`).count()
  }
  return 0
}
function entrypoint() -> nothing {
  let t: int = 0
  for (i in range(0, 5)) {
    t = t + join(make(i).or(`y`), join(make(i).or(`x`), make(i).or(`w`))).count()
    if (broken(i)) {
      t = t + 100
    }
    if (both(i)) {
      t = t + 1
    }
    t = t + pick(i)
  }
  print(t)
}
";

#[test]
fn an_errors_temporary_in_a_return_a_short_circuit_and_nested_calls_is_released_once() {
    // per round: 6 + 0 + 1 + 2 = 9
    assert_balanced_and_clean("c7r3_ec_expr", ERRORS_TEMPORARY_IN_EXPRESSIONS, "45\n");
}
