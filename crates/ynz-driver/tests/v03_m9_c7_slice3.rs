//! v0.3-M9 C7 slice 3: inline aggregate locals (nested shapes, `maybe`, unions, `fixed`,
//! errors-capable values, SoA arrays) are released at their scope exits through the one release
//! helper, and nothing is freed early or twice (`IMP-ownership.md` "Scope-exit release on the
//! plain path").
//!
//! Each case runs at both optimization levels. The counters say every allocation was freed (the
//! LEAK side) and AddressSanitizer with `free_fill_byte=0` says nothing was freed early or twice
//! (the SAFETY side). All are High tier: a wrong release is a use-after-free, a double free or a
//! leak, silent at exit 0.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    run_balanced,
};

/// What a case checks the counters of.
#[derive(Clone, Copy)]
enum Balance {
    Heap,
    HeapAndStrings,
    Strings,
}

fn check(label: &str, program: &str, expected: &str, balance: Balance) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        match balance {
            Balance::Heap => assert_heap_balanced(label, &counters, no_optimize),
            Balance::HeapAndStrings => {
                assert_heap_balanced(label, &counters, no_optimize);
                assert_strings_balanced(label, &counters, no_optimize);
            }
            Balance::Strings => assert_strings_balanced(label, &counters, no_optimize),
        }
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

/// `case!(name, PROGRAM, "stdout", Balance::X, "the bug it catches (High)")`.
macro_rules! case {
    ($case:ident, $program:expr, $expected:literal, $balance:expr, $bug:literal) => {
        #[doc = $bug]
        #[test]
        fn $case() {
            check(stringify!($case), $program, $expected, $balance);
        }
    };
}

// ── Item 1: nested shapes ───────────────────────────────────────────────────────────────────

const NESTED_SHAPE_LOCAL: &str = "
shape Tag {
  name: string
  id: int
}

shape Wrap {
  inner: Tag
  label: string
}

function make(index: int) -> Wrap {
  let tag: Tag = { name: `t`, id: index }
  return { inner: tag, label: `l` }
}

function entrypoint() -> nothing {
  let total: int = 0
  for (index in range(0, 100)) {
    let wrap: Wrap = make(index)
    total = total + wrap.inner.id
  }
  print(total)
}
";

case!(
    a_nested_shape_local_is_released_with_its_inner_cell,
    NESTED_SHAPE_LOCAL,
    "4950\n",
    Balance::HeapAndStrings,
    "a shape holding a shape (each shape-typed field is a heap cell) is never released at its scope exit: 100 cells and 200 strings leak (High)"
);

// ── Item 1: `maybe` locals ─────────────────────────────────────────────────────────────────

const MAYBE_LOCALS_OF_EVERY_PAYLOAD_KIND_ARE_RELEASED: &str = r"
shape Tag {
  name: string
  id: int
}
function entrypoint() -> nothing {
  let names: array<string> = [`aa`, `bb`]
  let tags: array<Tag> = [{ name: `n`, id: 1 }]
  let rows: array<array<int>> = [[1, 2]]
  let nums: array<number<34> > = [1.50]
  let total: int = 0
  for (i in range(0, 100)) {
    let first: maybe<string> = names.get(0)
    let tag: maybe<Tag> = tags.get(0)
    let row: maybe<array<int>> = rows.get(0)
    let num: maybe<number<34> > = nums.get(0)
    total = total + first.or(`x`).count() + tag.or({ name: `z`, id: 0 }).id + row.or([]).count()
  }
  print(total)
}
";

case!(
    maybe_locals_of_every_payload_kind_are_released,
    MAYBE_LOCALS_OF_EVERY_PAYLOAD_KIND_ARE_RELEASED,
    "500\n",
    Balance::HeapAndStrings,
    "a `maybe` local (string, shape, array and number payloads) is never released at its scope exit: its payload copy leaks per iteration (High)"
);

const A_MAYBE_REASSIGNED_FROM_A_CALL_RELEASES_THE_VALUE_IT_REPLACES: &str = r"
shape Tag {
  name: string
  id: int
}
function findTag(share tags: array<Tag>, index: int) -> maybe<Tag> {
  return tags.get(index)
}
function findName(share names: array<string>, index: int) -> maybe<string> {
  return names.get(index)
}
function entrypoint() -> nothing {
  let tags: array<Tag> = [{ name: `a${1}`, id: 1 }, { name: `b${2}`, id: 2 }]
  let names: array<string> = [`aa-${1}`, `bb-${2}`]
  let held: maybe<Tag> = findTag(tags, 0)
  let label: maybe<string> = findName(names, 0)
  let total: int = 0
  for (index in range(0, 50)) {
    held = findTag(tags, index % 2)
    label = findName(names, index % 2)
    total = total + held.or({ name: `z`, id: 0 }).id + label.or(`q`).count()
  }
  print(total)
}
";

case!(
    a_maybe_reassigned_from_a_call_releases_the_value_it_replaces,
    A_MAYBE_REASSIGNED_FROM_A_CALL_RELEASES_THE_VALUE_IT_REPLACES,
    "275\n",
    Balance::HeapAndStrings,
    "a `maybe` reassigned in a loop reads its old payload after the new value reused its slot, or orphans the heap cell and strings a returned `maybe` carries (High)"
);

// ── Item 1: union views ─────────────────────────────────────────────────────────────────────

const A_UNION_LOCAL_A_GIVEN_UNION_AND_A_REASSIGNED_UNION_ARE_RELEASED: &str = r"
shape Circle {
  label: string
}
shape Square {
  side: int
}
shape Figure = Circle | Square
function describe(give figure: Figure) -> int {
  if (figure) {
    is Circle => {
      return figure.label.count()
    }
    is Square => {
      return figure.side
    }
  }
  return 0
}
function entrypoint() -> nothing {
  let total: int = 0
  let other: Circle = { label: `x` }
  let kept: Figure = other
  for (index in range(0, 50)) {
    let circle: Circle = { label: `round-${index}` }
    let figure: Figure = circle
    total = total + describe(figure)
    let again: Circle = { label: `y-${index}` }
    let next: Figure = again
    kept = next
    if (kept) {
      is Circle => {
        total = total + kept.label.count()
      }
      is Square => {
        total = total + 1
      }
    }
  }
  print(total)
}
";

case!(
    a_union_local_a_given_union_and_a_reassigned_union_are_released,
    A_UNION_LOCAL_A_GIVEN_UNION_AND_A_REASSIGNED_UNION_ARE_RELEASED,
    "580\n",
    Balance::Strings,
    "a union local is a view of a slot, so a release that treated it as the slot reads the tag from the payload; a given union parameter has no slot to release (High)"
);

// ── Item 1 and 5: `fixed` lists ────────────────────────────────────────────────────────────

const FIXED_LISTS_OF_STRINGS_SHAPES_AND_ARRAYS_ARE_RELEASED_WITH_THEIR_ITEMS: &str = r"
shape Tag {
  name: string
  id: int
}
function entrypoint() -> nothing {
  let total: int = 0
  for (index in range(0, 50)) {
    let names: fixed<string> = [`a${index}`, `b${index}`]
    let first: Tag = { name: `t${index}`, id: index }
    let second: Tag = { name: `u${index}`, id: 1 }
    let tags: fixed<Tag> = [first, second]
    let row: array<int> = [index]
    let other: array<int> = [1, 2]
    let rows: fixed<array<int> > = [row, other]
    names.set(0, `c${index}`)
    let fallback: string = `f`
    total = total + names[0].or(fallback).count() + rows.count() + tags.count()
  }
  print(total)
}
";

case!(
    fixed_lists_of_strings_shapes_and_arrays_are_released_with_their_items,
    FIXED_LISTS_OF_STRINGS_SHAPES_AND_ARRAYS_ARE_RELEASED_WITH_THEIR_ITEMS,
    "340\n",
    Balance::HeapAndStrings,
    "a `fixed` local never released at its scope exit leaks every string, shape cell and array it holds (High)"
);

const A_FIXED_LIST_OF_UNIONS_RELEASES_THE_ITEM_AN_OVERWRITE_REPLACES: &str = r"
shape Circle {
  label: string
}
shape Square {
  side: int
}
shape Figure = Circle | Square
function entrypoint() -> nothing {
  let total: int = 0
  for (index in range(0, 50)) {
    let circle: Circle = { label: `c${index}` }
    let square: Square = { side: 2 }
    let other: Circle = { label: `d${index}` }
    let first: Figure = circle
    let second: Figure = square
    let third: Figure = other
    let figs: fixed<Figure> = [first, second]
    figs.set(0, third)
    total = total + figs.count()
  }
  print(total)
}
";

case!(
    a_fixed_list_of_unions_releases_the_item_an_overwrite_replaces,
    A_FIXED_LIST_OF_UNIONS_RELEASES_THE_ITEM_AN_OVERWRITE_REPLACES,
    "100\n",
    Balance::Strings,
    "a union item replaced by `fixed.set` leaks its string variant, or is released through the wrong layout and double-freed (High)"
);

// ── Item 1: errors-capable locals ───────────────────────────────────────────────────────────

const AN_ERRORS_CAPABLE_LOCAL_IS_RELEASED_THROUGH_ITS_ORIGINAL_SLOT: &str = r"
function load(index: int) -> string errors {
  return `value-${index}`
}
function loadRows(index: int) -> array<int> errors {
  return [index, index]
}
function relay(index: int) -> string errors {
  let inner = load(index)
  return inner
}
function entrypoint() -> nothing {
  let total: int = 0
  for (index in range(0, 50)) {
    let first = load(index)
    let rows = loadRows(index)
    let second = relay(index)
    total = total + first.or(`d`).count() + rows.or([]).count() + second.or(`e`).count()
  }
  print(total)
}
";

case!(
    an_errors_capable_local_is_released_through_its_original_slot,
    AN_ERRORS_CAPABLE_LOCAL_IS_RELEASED_THROUGH_ITS_ORIGINAL_SLOT,
    "880\n",
    Balance::HeapAndStrings,
    "an errors-capable local (string and array payloads, one returned through a relay) is never released: the ok payload leaks per call (High)"
);

const A_STRING_TAKEN_FROM_AN_ERRORS_CAPABLE_RESULT_IS_NOT_RELEASED_TWICE: &str = r"
function load(index: int) -> string errors {
  return `value-${index}`
}
function twin(index: int) -> int errors {
  let first = load(index)
  let second = first
  let one: string = first
  let two: string = second
  return one.count() + two.count()
}
function entrypoint() -> nothing {
  let total: int = 0
  for (index in range(0, 50)) {
    total = total + twin(index).or(0)
  }
  print(total)
}
";

case!(
    a_string_taken_from_an_errors_capable_result_is_not_released_twice,
    A_STRING_TAKEN_FROM_AN_ERRORS_CAPABLE_RESULT_IS_NOT_RELEASED_TWICE,
    "780\n",
    Balance::Strings,
    "a string read off a result at its last use is taken by a store, then freed again by the result's own release (double free, High)"
);
