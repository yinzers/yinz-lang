//! v0.3-M9 C7 review r1 (ruled by the Chief Architect): a `give` parameter or a local of a type
//! parameter `T` in a generic function is released like any owner. Typeck records its fate once,
//! as if `T` owned heap (`ReleaseKind::TypeParameter`); codegen lowers the body once per instance
//! and resolves the release by the instance's type, so `T` = `string` frees a string and `T` =
//! `int` releases nothing.
//!
//! Counters say every allocation was freed (the bug leaked every given `T`); AddressSanitizer
//! says nothing was freed twice or early. Both run at both optimization levels. High tier:
//! ownership and data integrity.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    run_balanced,
};

fn assert_balanced_and_clean(label: &str, program: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        // A program that allocates only strings has no array or cell to balance.
        if counters.alloc > 0 {
            assert_heap_balanced(label, &counters, no_optimize);
        }
        assert_strings_balanced(label, &counters, no_optimize);
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

const TWICE: &str = "
function twice<T>(give first: T, give second: T) -> Nothing {
  print(1)
}
function entrypoint() -> Nothing {
  let n = 4
  let name: String = `a ${n}`
  let other: String = `b ${n}`
  twice(name, other)
  twice(`c ${n}`, `d ${n}`)
  twice(5, 6)
}
";

/// One named test per ownership path of the `generic give / T local release` run unit; each case balances counters and runs under AddressSanitizer at both optimization levels.
macro_rules! r1_generic_give_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a give parameter or local of a generic type parameter T is leaked, freed twice or released at the wrong instance type, silently at exit 0 | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

r1_generic_give_high_case! {
    // case: Bug (High: leak): `twice<T>` omitted both `give` parameters as `Omitted(TypeParameter)`, so each call leaked two strings.
    a_given_type_parameter_string_is_released_by_the_generic_callee => {
        assert_balanced_and_clean("c7_r1_twice", TWICE, "1\n1\n1\n");
    }
    // case: Bug (High: leak): `let held: T = value` was `Omitted(TypeParameter)`; its store also copied the string and left `value` unreleased.
    a_type_parameter_local_is_released => {
    assert_balanced_and_clean(
        "c7_r1_local",
        "
function hold<T>(give value: T) -> Nothing {
  let held: T = value
  print(1)
}
function entrypoint() -> Nothing {
  let n = 4
  hold(`a ${n}`)
  let name: String = `b ${n}`
  hold(name)
  hold(7)
  hold([n, 1])
}
",
        "1\n1\n1\n1\n",
    );
    }
    // case: Bug (High: double free): a `T` handed on by `return` or by a store must not also be released by the parameter it left.
    a_type_parameter_returned_or_stored_is_released_once_by_its_new_holder => {
    assert_balanced_and_clean(
        "c7_r1_moved_on",
        "
function same<T>(give value: T) -> T {
  return value
}
function wrap<T>(give value: T) -> Array<T> {
  let items: Array<T> = []
  items.add(value)
  return items
}
function entrypoint() -> Nothing {
  let n = 4
  let back: String = same(`a ${n}`)
  print(back)
  let name: String = `b ${n}`
  let again: String = same(name)
  print(again)
  let rows: Array<String> = wrap(`c ${n}`)
  print(rows.count())
  print(same(5))
  let nums: Array<Int> = wrap(6)
  print(nums.count())
  let lists: Array<Array<Int>> = wrap([n, 1])
  print(lists.count())
}
",
        "a 4\nb 4\n1\n5\n1\n1\n",
    );
    }
    // case: Bug (High: double free or leak): `value` is stored on one path only, so it is `ReleaseIfLive`: released when the flag is false, not when it was stored.
    a_type_parameter_moved_on_some_paths_is_released_only_where_it_is_live => {
    assert_balanced_and_clean(
        "c7_r1_some_paths",
        "
function maybeKeep<T>(give value: T, flag: Boolean) -> Array<T> {
  let items: Array<T> = []
  if (flag) {
    items.add(value)
  }
  return items
}
function entrypoint() -> Nothing {
  let n = 4
  let kept: Array<String> = maybeKeep(`a ${n}`, true)
  let dropped: Array<String> = maybeKeep(`b ${n}`, false)
  print(kept.count() + dropped.count())
  let numbers: Array<Int> = maybeKeep(5, true)
  let skipped: Array<Int> = maybeKeep(6, false)
  print(numbers.count() + skipped.count())
  let lists: Array<Array<Int>> = maybeKeep([n, 1], true)
  let gone: Array<Array<Int>> = maybeKeep([n, 2], false)
  print(lists.count() + gone.count())
}
",
        "1\n1\n1\n",
    );
    }
    // case: Bug (High: leak): each assignment to a `T` local drops the old value unreleased.
    a_reassigned_type_parameter_local_releases_the_value_it_replaces => {
    assert_balanced_and_clean(
        "c7_r1_reassign",
        "
function pick<T>(give first: T, give second: T) -> T {
  let held: T = first
  held = second
  return held
}
function entrypoint() -> Nothing {
  let n = 4
  print(pick(`a ${n}`, `b ${n}`))
  print(pick(1, 2))
  print(pick([n], [n, n]).count())
}
",
        "b 4\n2\n2\n",
    );
    }
    // case: Bug (High: leak): `value.copy()` and an `.or` fallback built from it have type `T`, which `temporaries` did not list, so the copy leaked in every instance whose `T` owns heap (string, array, shape); the `int` instance owns nothing and must stay untouched.
    a_fresh_type_parameter_value_no_binding_holds_is_released_per_instance => {
    assert_balanced_and_clean(
        "c7_r1_temporary",
        "
shape Row {
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
  let name: String = `a ${n}`
  burn(name)
  burn([n, 1])
  let row: Row = { name: `b ${n}` }
  burn(row)
  burn(5)
}
",
        "1\n1\n1\n1\n",
    );
    }
}
