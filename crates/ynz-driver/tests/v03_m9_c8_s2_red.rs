//! v0.3-M9 C8 slice 2: the producers of the slice that were red before it was built (card #385,
//! plan #17). Each program is the smallest that leaks for that producer alone; counters and
//! AddressSanitizer both run at -O0 and -O2, each level in its own process.
//!
//! - `wait_statement_temporary_*`: a temporary built in a statement that suspends. It is an
//!   argument staged into a suspending callee's frame, so it must survive the `wait` and be
//!   released at the end of its statement (a frame slot, not an entry-block slot).
//! - `reassign_*`: a frame-backed local (string, array) reassigned in a resume function releases
//!   the value it replaces.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    run_balanced,
};

/// Array and string temporaries passed to a suspending callee, which keeps them across its own
/// `wait`. Both die at the end of their statement, after the callee returned.
const TEMPORARY_ACROSS_WAIT: &str = "
function make() -> array<int> {
  return [1, 2, 3]
}

function measure(rows: array<int>) -> int {
  wait sleep(1)
  return rows.count()
}

function describe(text: string) -> int {
  wait sleep(1)
  return text.count()
}

function entrypoint() -> nothing {
  let rowCount: int = wait measure(make())
  let textCount: int = wait describe(`crew`.toUpperCase())
  print(rowCount)
  print(textCount)
}
";

/// The same statements repeated by a loop: the frame slot of each temporary is nulled after its
/// release, so an iteration never frees the previous iteration's value a second time.
const TEMPORARY_ACROSS_WAIT_IN_LOOP: &str = "
function make() -> array<int> {
  return [1, 2, 3]
}

function measure(rows: array<int>) -> int {
  wait sleep(1)
  return rows.count()
}

function entrypoint() -> nothing {
  let total: int = 0
  let round: int = 0
  while (round < 3) {
    let rowCount: int = wait measure(make())
    total = total + rowCount
    round = round + 1
  }
  print(total)
}
";

/// A frame-backed local reassigned after a `wait`: the old value is released at the assignment.
const REASSIGN_ARRAY_AND_STRING: &str = "
function entrypoint() -> nothing {
  let rows: array<int> = [1, 2]
  let label: string = `old`.toUpperCase()
  wait sleep(1)
  rows = [3, 4, 5]
  label = `fresh`.toUpperCase()
  wait sleep(1)
  print(rows.count())
  print(label.count())
}
";

fn assert_balanced_program(label: &str, program: &str, expected: &str, strings: bool) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        assert_heap_balanced(label, &counters, no_optimize);
        if strings {
            assert_strings_balanced(label, &counters, no_optimize);
        }
    });
}

fn assert_asan_clean_program(label: &str, program: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

#[test]
fn wait_statement_temporary_array_and_string_are_released() {
    assert_balanced_program(
        "wait_statement_temporary",
        TEMPORARY_ACROSS_WAIT,
        "3\n4\n",
        true,
    );
}

#[test]
fn wait_statement_temporary_stays_asan_clean() {
    assert_asan_clean_program(
        "wait_statement_temporary_asan",
        TEMPORARY_ACROSS_WAIT,
        "3\n4\n",
    );
}

#[test]
fn wait_statement_temporary_in_a_loop_is_released_once_per_iteration() {
    assert_balanced_program(
        "wait_statement_temporary_loop",
        TEMPORARY_ACROSS_WAIT_IN_LOOP,
        "9\n",
        false,
    );
    assert_asan_clean_program(
        "wait_statement_temporary_loop_asan",
        TEMPORARY_ACROSS_WAIT_IN_LOOP,
        "9\n",
    );
}

#[test]
#[ignore = "planned RED (v0.3-M9 C8 S2): a resume function does not release the value a reassignment replaces"]
fn reassign_releases_the_old_array_and_string_in_a_resume_function() {
    assert_balanced_program(
        "reassign_in_resume_function",
        REASSIGN_ARRAY_AND_STRING,
        "3\n5\n",
        true,
    );
    assert_asan_clean_program(
        "reassign_in_resume_function_asan",
        REASSIGN_ARRAY_AND_STRING,
        "3\n5\n",
    );
}
