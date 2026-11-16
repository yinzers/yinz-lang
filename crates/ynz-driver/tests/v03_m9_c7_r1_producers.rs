//! v0.3-M9 C7 review round 1, builder 2: three leak producers on the plain-local path, each fixed
//! at the one place that decides who owns the value (`IMP-ownership.md` "The persisted release
//! plan" through "Non-owned reads").
//!
//! Counter cases run at both optimization levels; the counters say every allocation was freed and
//! AddressSanitizer with `free_fill_byte=0` says nothing was freed early or twice. All are High
//! tier: a wrong release is a leak, a double free or a use-after-free, silent at exit 0.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    run_balanced,
};

/// Run `program` at both optimization levels: it prints `expected`, balances heap and strings and
/// is ASan-clean.
fn assert_balanced_and_clean(label: &str, program: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        assert_heap_balanced(label, &counters, no_optimize);
        assert_strings_balanced(label, &counters, no_optimize);
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

// ── Group 3: a built-in that builds a fresh heap value is an owner's value ──────────────────

const SPLIT_BOUND_BY_LET: &str = "
function entrypoint() -> nothing {
  let s: string = `a, b, c`
  let parts: array<string> = s.split(`, `)
  print(parts.count())
}
";

/// Bug (High: leak): `provenance` classed a built-in's heap result as `Reaches([receiver])`
/// unless the method was in `builtin_method_returns_fresh`; `split` was not, so `parts` was a
/// piece of `s`, never an owner, and its array and three strings were never released.
#[test]
fn a_split_result_bound_by_let_is_an_owner_and_is_released() {
    assert_balanced_and_clean("c7r1_split_let", SPLIT_BOUND_BY_LET, "3\n");
}

const SPLIT_TEMPORARIES: &str = "
function countOf(share cells: array<string>) -> int {
  return cells.count()
}
function entrypoint() -> nothing {
  let s: string = `a,b,c`
  let total: int = s.split(`,`).count()
  total = total + countOf(s.split(`,`))
  for (part in s.split(`,`)) {
    total = total + part.count()
  }
  let upper: array<string> = s.toUpperCase().split(`,`)
  total = total + upper.count()
  print(total)
}
";

/// Bug (High: leak): a `split` result nothing binds (a receiver, a borrowed argument, a `for`
/// iterable) was not a temporary, so it was never released at its statement's end.
#[test]
fn a_split_result_nothing_binds_is_released_at_the_end_of_its_statement() {
    assert_balanced_and_clean("c7r1_split_temporaries", SPLIT_TEMPORARIES, "12\n");
}

const SPLIT_HANDED_OVER: &str = "
shape Row {
  cells: array<string>
}
function pieces(share s: string) -> array<string> {
  return s.split(`,`)
}
function entrypoint() -> nothing {
  let s: string = `a,b,c`
  let parts: array<string> = s.split(`,`)
  let row: Row = { cells: parts }
  let nested: array<array<string>> = [s.split(`,`).copy()]
  let again: array<string> = pieces(s)
  again = s.split(`,`)
  print(row.cells.count() + nested.count() + again.count())
}
";

/// Bug (High: double free): a `split` result that is an owner is handed over by a store and
/// copied by `.copy()`; each must be released once.
#[test]
fn a_split_result_stored_copied_returned_or_reassigned_is_released_once() {
    assert_balanced_and_clean("c7r1_split_handed_over", SPLIT_HANDED_OVER, "7\n");
}
