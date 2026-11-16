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
    build_source, run_balanced,
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
function twice<T>(give first: T, give second: T) -> nothing {
  print(1)
}
function entrypoint() -> nothing {
  let n = 4
  let name: string = `a ${n}`
  let other: string = `b ${n}`
  twice(name, other)
  twice(`c ${n}`, `d ${n}`)
  twice(5, 6)
}
";

#[test]
fn a_given_type_parameter_string_is_released_by_the_generic_callee() {
    // Bug (High: leak): `twice<T>` omitted both `give` parameters as `Omitted(TypeParameter)`, so
    // each call leaked two strings.
    assert_balanced_and_clean("c7_r1_twice", TWICE, "1\n1\n1\n");
}

#[test]
fn an_int_instance_of_a_generic_give_compiles_with_no_release() {
    // Bug caught: releasing `T` = `int` would load a pointer from an int slot (or null the int on
    // a move). The instance owns no heap, so its IR has no release at all.
    at_both_optimization_levels(|no_optimize| {
        let built = build_source(
            "
function twice<T>(give first: T, give second: T) -> nothing {
  let held: T = first
  print(1)
}
function entrypoint() -> nothing {
  twice(5, 6)
}
",
            "c7_r1_int_instance",
            no_optimize,
        );
        let ir = std::fs::read_to_string(&built.ir).expect("read the IR");
        let released = ir
            .lines()
            .any(|line| line.contains("call") && line.contains("release"));
        assert!(
            !released && !ir.contains("define internal void @ynz_release"),
            "no_optimize={no_optimize}: an `int` instance releases nothing:\n{ir}"
        );
    });
}

#[test]
fn a_type_parameter_local_is_released() {
    // Bug (High: leak): `let held: T = value` was `Omitted(TypeParameter)`; its store also copied
    // the string and left `value` unreleased.
    assert_balanced_and_clean(
        "c7_r1_local",
        "
function hold<T>(give value: T) -> nothing {
  let held: T = value
  print(1)
}
function entrypoint() -> nothing {
  let n = 4
  hold(`a ${n}`)
  let name: string = `b ${n}`
  hold(name)
  hold(7)
  hold([n, 1])
}
",
        "1\n1\n1\n1\n",
    );
}

#[test]
fn a_type_parameter_returned_or_stored_is_released_once_by_its_new_holder() {
    // Bug (High: double free): a `T` handed on by `return` or by a store must not also be released
    // by the parameter it left.
    assert_balanced_and_clean(
        "c7_r1_moved_on",
        "
function same<T>(give value: T) -> T {
  return value
}
function wrap<T>(give value: T) -> array<T> {
  let items: array<T> = []
  items.add(value)
  return items
}
function entrypoint() -> nothing {
  let n = 4
  let back: string = same(`a ${n}`)
  print(back)
  let name: string = `b ${n}`
  let again: string = same(name)
  print(again)
  let rows: array<string> = wrap(`c ${n}`)
  print(rows.count())
  print(same(5))
  let nums: array<int> = wrap(6)
  print(nums.count())
  let lists: array<array<int>> = wrap([n, 1])
  print(lists.count())
}
",
        "a 4\nb 4\n1\n5\n1\n1\n",
    );
}

#[test]
fn a_type_parameter_moved_on_some_paths_is_released_only_where_it_is_live() {
    // Bug (High: double free or leak): `value` is stored on one path only, so it is
    // `ReleaseIfLive`: released when the flag is false, not when it was stored.
    assert_balanced_and_clean(
        "c7_r1_some_paths",
        "
function maybeKeep<T>(give value: T, flag: boolean) -> array<T> {
  let items: array<T> = []
  if (flag) {
    items.add(value)
  }
  return items
}
function entrypoint() -> nothing {
  let n = 4
  let kept: array<string> = maybeKeep(`a ${n}`, true)
  let dropped: array<string> = maybeKeep(`b ${n}`, false)
  print(kept.count() + dropped.count())
  let numbers: array<int> = maybeKeep(5, true)
  let skipped: array<int> = maybeKeep(6, false)
  print(numbers.count() + skipped.count())
  let lists: array<array<int>> = maybeKeep([n, 1], true)
  let gone: array<array<int>> = maybeKeep([n, 2], false)
  print(lists.count() + gone.count())
}
",
        "1\n1\n1\n",
    );
}

#[test]
fn a_reassigned_type_parameter_local_releases_the_value_it_replaces() {
    // Bug (High: leak): each assignment to a `T` local drops the old value unreleased.
    assert_balanced_and_clean(
        "c7_r1_reassign",
        "
function pick<T>(give first: T, give second: T) -> T {
  let held: T = first
  held = second
  return held
}
function entrypoint() -> nothing {
  let n = 4
  print(pick(`a ${n}`, `b ${n}`))
  print(pick(1, 2))
  print(pick([n], [n, n]).count())
}
",
        "b 4\n2\n2\n",
    );
}
