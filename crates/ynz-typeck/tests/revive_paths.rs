// v0.3-M9 C5a round 2: a reassignment revives a given binding only for the code it dominates.
//
// WHY (High tier: ownership / data integrity): `Scope` used to clear the consumed state on ANY
// reassignment, even one inside an `if`, a match arm, or a loop that may run zero times. A later
// give or read of `x` then touched a value already given away on some path: a double free or a
// use-after-free under any scope-exit release pass. Rust rejects the same programs. The join
// after a branch is "maybe given" unless EVERY path that reaches it revived the binding; reading
// a maybe-given binding is the ordinary use-after-give error (the canonical `Consumed` text).
//
// Each rejected case failed on the base commit (typeck accepted the program); each accepted case
// is a guard that the fix does not reject a program every path of which revives the binding.

use ynz_diagnostics::Severity;
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput};

const EAT: &str = "function eat(give rows: array<int>) -> nothing {
  print(rows.count().toString())
}\n";

fn run(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    // Every program needs an `entrypoint`; the cases under test are the other functions.
    let program = format!("{EAT}{source}\nfunction entrypoint() -> nothing {{\n}}\n");
    let source_file = SourceFile::new(&db, "test.ynz".to_string(), program);
    (*check_query(&db, source_file)).clone()
}

fn errors(source: &str) -> Vec<String> {
    run(source)
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, Severity::Error))
        .map(|diagnostic| format!("{diagnostic:?}"))
        .collect()
}

macro_rules! rejected {
    ($case:ident, $bug:literal, $source:expr) => {
        #[doc = $bug]
        #[test]
        fn $case() {
            let found = errors($source);
            assert!(
                found.iter().any(|e| e.contains("was already given away")),
                "expected a use-after-give rejection, got {found:#?}"
            );
        }
    };
}

macro_rules! accepted {
    ($case:ident, $bug:literal, $source:expr) => {
        #[doc = $bug]
        #[test]
        fn $case() {
            let found = errors($source);
            assert!(found.is_empty(), "expected accepted, got {found:#?}");
        }
    };
}

rejected!(
    loop_revive_under_an_if_is_carried,
    "give, then revive only under `if`, inside a loop: the next pass gives `x` again (High)",
    "function l(c: boolean) -> nothing {
  let x: array<int> = [1, 2, 3]
  for (j in range(0, 2)) {
    eat(x)
    if (c) { x = [0] }
  }
}"
);

rejected!(
    loop_revive_in_a_zero_trip_inner_loop_is_carried,
    "give, then revive inside an inner loop that may run zero times: the next pass gives `x` again (High)",
    "function l() -> nothing {
  let x: array<int> = [1, 2, 3]
  for (j in range(0, 2)) {
    eat(x)
    for (k in range(0, 0)) { x = [0] }
  }
}"
);

rejected!(
    read_after_an_if_that_revives_on_one_path,
    "give, revive only under `if`, then read: on the path where the condition was false `x` is gone (High)",
    "function l(c: boolean) -> int {
  let x: array<int> = [1, 2, 3]
  eat(x)
  if (c) { x = [0] }
  return x.count()
}"
);

rejected!(
    read_after_a_match_whose_else_arm_does_not_revive,
    "give, revive in one match arm only, then read: the other arm leaves `x` given away (High)",
    "function l(n: int) -> int {
  let x: array<int> = [1, 2, 3]
  eat(x)
  if (n) {
    1 => { x = [0] }
    else => { print(`other`) }
  }
  return x.count()
}"
);

rejected!(
    read_after_a_match_with_no_else_arm,
    "give, revive in every listed arm of a non-exhaustive match, then read: no arm matching is a path that never revived (High)",
    "function l(n: int) -> int {
  let x: array<int> = [1, 2, 3]
  eat(x)
  if (n) {
    1 => { x = [0] }
    2 => { x = [1] }
  }
  return x.count()
}"
);

rejected!(
    read_after_a_while_that_revives_in_its_body,
    "give, revive inside a `while` body, then read: the body may run zero times (High)",
    "function l(c: boolean) -> int {
  let x: array<int> = [1, 2, 3]
  eat(x)
  while (c) { x = [0] }
  return x.count()
}"
);

accepted!(
    revive_in_every_arm_of_a_match_with_else,
    "give, revive in every arm including `else`, then read: every path revived (High)",
    "function l(n: int) -> int {
  let x: array<int> = [1, 2, 3]
  eat(x)
  if (n) {
    1 => { x = [0] }
    else => { x = [1] }
  }
  return x.count()
}"
);

accepted!(
    revive_beside_an_arm_that_always_returns,
    "give, one arm returns, the other revives, then read: the returning arm never reaches the join (High)",
    "function l(n: int) -> int {
  let x: array<int> = [1, 2, 3]
  eat(x)
  if (n) {
    1 => { return 0 }
    else => { x = [0] }
  }
  return x.count()
}"
);

accepted!(
    unconditional_revive_after_a_loop,
    "give, run a loop that does not mention `x`, revive after it, then read (High)",
    "function l() -> int {
  let x: array<int> = [1, 2, 3]
  eat(x)
  for (j in range(0, 2)) { print(`tick`) }
  x = [0]
  return x.count()
}"
);

accepted!(
    revive_in_a_loop_body_then_revive_again_after_it,
    "give and revive on every pass, then revive after the loop and read (High)",
    "function l() -> int {
  let x: array<int> = [1, 2, 3]
  for (j in range(0, 2)) {
    eat(x)
    x = [0]
  }
  x = [1]
  return x.count()
}"
);

accepted!(
    give_and_revive_both_inside_the_branch,
    "give then revive inside one `if` body: both paths leave `x` live (High)",
    "function l(c: boolean) -> int {
  let x: array<int> = [1, 2, 3]
  if (c) {
    eat(x)
    x = [0]
  }
  return x.count()
}"
);

accepted!(
    unconditional_revive_straight_line,
    "give then revive on the same path, then read: the existing revive-on-reassign rule (High)",
    "function l() -> int {
  let x: array<int> = [1, 2, 3]
  eat(x)
  x = [4, 5]
  return x.count()
}"
);
