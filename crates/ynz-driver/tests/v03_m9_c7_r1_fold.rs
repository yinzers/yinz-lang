//! v0.3-M9 C7 fold: the last plain-path producer (a `maybe<Union>` a function returns) and the
//! probes the fold checks (a `.copy()` of a `maybe` at a UFCS `give`).
//!
//! Counters assert what the emitter RELEASES; AddressSanitizer asserts it releases nothing a
//! second holder still owns. Both optimization levels run at once per test.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    run_balanced,
};

const UNION_MAYBE_RETURNED: &str = "
shape Tiny {
  flag: Boolean
}
shape Big {
  label: String
  count: Int
}
shape Thing = Tiny | Big

function describe(share thing: Thing) -> String {
  if (thing) {
    is Tiny => {
      return `tiny`
    }
    is Big => {
      return `big ${thing.label}`
    }
  }
  return `none`
}

function fetch(share things: Array<Thing>, index: Int) -> Maybe<Thing> {
  let found = things.get(index)
  return found
}

function entrypoint() -> Nothing {
  let things: Array<Thing> = []
  let tiny: Tiny = { flag: true }
  things.add(tiny)
  let big: Big = { label: `kept`, count: 1 }
  things.add(big)
  let index = 0
  while (index < 3) {
    let picked = fetch(things, index)
    if (picked.exists()) {
      print(describe(picked.value))
    }
    if (picked.exists() == false) {
      print(`none`)
    }
    index = index + 1
  }
  print(`end`)
}
";

const UNION_MAYBE_RETURNED_EXPECTED: &str = "tiny\nbig kept\nnone\nend\n";

/// A `maybe<Union>` a function returns is released by its caller: heap blocks and strings freed
/// equal allocated, and nothing is freed twice (High).
#[test]
fn a_maybe_union_a_function_returns_is_released() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(
            "c7_fold_maybe_union",
            UNION_MAYBE_RETURNED,
            UNION_MAYBE_RETURNED_EXPECTED,
            no_optimize,
        );
        assert_heap_balanced("c7_fold_maybe_union", &counters, no_optimize);
        assert_strings_balanced("c7_fold_maybe_union", &counters, no_optimize);
        assert_asan_clean(
            "c7_fold_maybe_union",
            UNION_MAYBE_RETURNED,
            UNION_MAYBE_RETURNED_EXPECTED,
            no_optimize,
        );
    });
}

const UNION_MAYBE_FORMS: &str = "
shape Tiny {
  flag: Boolean
}
shape Big {
  label: String
  count: Int
}
shape Thing = Tiny | Big

function direct(share things: Array<Thing>, index: Int) -> Maybe<Thing> {
  return things.get(index)
}

function passed(share things: Array<Thing>, index: Int) -> Maybe<Thing> {
  let found = direct(things, index)
  return found
}

function entrypoint() -> Nothing {
  let things: Array<Thing> = []
  let tiny: Tiny = { flag: true }
  things.add(tiny)
  let big: Big = { label: `kept`, count: 1 }
  things.add(big)
  let index = 0
  while (index < 3) {
    let first = direct(things, index)
    let second = passed(things, index)
    direct(things, index)
    if (passed(things, index).exists()) {
      print(`has`)
    }
    if (first.exists() == false) {
      print(`none`)
    }
    if (second.exists()) {
      print(`second`)
    }
    index = index + 1
  }
  print(`end`)
}
";

const UNION_MAYBE_FORMS_EXPECTED: &str = "has\nsecond\nhas\nsecond\nnone\nend\n";

/// A `maybe<Union>` returned straight from a `get`, through a local, discarded, and read in
/// place is released once each (High).
#[test]
fn every_form_of_a_returned_maybe_union_is_released() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(
            "c7_fold_maybe_union_forms",
            UNION_MAYBE_FORMS,
            UNION_MAYBE_FORMS_EXPECTED,
            no_optimize,
        );
        assert_heap_balanced("c7_fold_maybe_union_forms", &counters, no_optimize);
        assert_strings_balanced("c7_fold_maybe_union_forms", &counters, no_optimize);
        assert_asan_clean(
            "c7_fold_maybe_union_forms",
            UNION_MAYBE_FORMS,
            UNION_MAYBE_FORMS_EXPECTED,
            no_optimize,
        );
    });
}

// ── Probe: a `maybe` a UFCS call gives ───────────────────────────────────────────────────────

const MAYBE_GIVEN_BY_UFCS: &str = "
shape Basket {
  size: Int
}
shape Tag {
  name: String
}
function firstTag(share list: Array<Tag>) -> Maybe<Tag> {
  let found = list.get(0)
  return found
}
function eatTag(share basket: Basket, give found: Maybe<Tag>) -> Int {
  return found.or({name: `z`}).name.count()
}
function entrypoint() -> Nothing {
  let tags: Array<Tag> = [{name: `nn`}]
  let current: Maybe<Tag> = tags.get(0)
  let basket: Basket = { size: 1 }
  let copied: Int = basket.eatTag(current.copy())
  let returned: Int = basket.eatTag(firstTag(tags))
  print(copied + returned)
}
";

/// A `.copy()` of a `maybe`, or a function's `maybe` result, at the `give` position of a UFCS call
/// (`list.eatTag(x)`) is handed over once, like the plain call `eatTag(x)` (High).
#[test]
fn a_fresh_maybe_given_through_a_ufcs_call_is_handed_over_once() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_fold_ufcs_give", MAYBE_GIVEN_BY_UFCS, "4\n", no_optimize);
        assert_heap_balanced("c7_fold_ufcs_give", &counters, no_optimize);
        assert_strings_balanced("c7_fold_ufcs_give", &counters, no_optimize);
        assert_asan_clean("c7_fold_ufcs_give", MAYBE_GIVEN_BY_UFCS, "4\n", no_optimize);
    });
}
