//! v0.3-M9 C7 slice 3b: the pieces that finish C7 (`IMP-ownership.md` "Scope-exit release on the
//! plain path"): a nested shape moves out of a temporary, holders of an untraceable `maybe` /
//! union of a channel own a counted reference, a reassigned binding reads consistently, an
//! errors-capable local releases its old result at a reassignment, and writes through a place a
//! `let` views are guarded under ASan.
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

// ── Item 1: a nested shape moves out of a temporary ─────────────────────────────────────────

const TAKE_NESTED_SHAPE: &str = "
shape Tag {
  name: string
  id: int
}
shape Wrap {
  inner: Tag
  label: string
}
function make(index: int) -> Wrap {
  let tag: Tag = { name: `t${index}`, id: index }
  return { inner: tag, label: `l${index}` }
}
function entrypoint() -> nothing {
  let total: int = 0
  let kept: Tag = { name: `k`, id: 7 }
  for (index in range(0, 100)) {
    let taken: Tag = make(index).inner
    kept = make(index + 1).inner
    total = total + taken.id + kept.id
  }
  print(total)
}
";

#[test]
fn a_nested_shape_taken_from_a_temporary_is_not_copied_and_balances() {
    // Bug (cost, Golden Rule 8; High: a wrong take is a double free): `let taken = make(i).inner`
    // deep-copies the nested shape's strings where Rust moves it. 200 calls build 2 strings and one
    // cell each; the `let` and the reassignment must add no string and no cell.
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("take_nested", TAKE_NESTED_SHAPE, "10000\n", no_optimize);
        assert_eq!(
            (counters.alloc, counters.free),
            (200, 200),
            "no_optimize={no_optimize}: one cell per `make` call, none per take: {counters:?}"
        );
        assert_eq!(
            (counters.string_alloc, counters.string_free),
            (401, 401),
            "no_optimize={no_optimize}: two strings per `make` call and the literal `k`: {counters:?}"
        );
        assert_asan_clean("take_nested", TAKE_NESTED_SHAPE, "10000\n", no_optimize);
    });
}

const TAKE_A_NESTED_SHAPE_OF_A_NESTED_SHAPE: &str = "
shape Leaf {
  name: string
  numbers: array<int>
}
shape Branch {
  leaf: Leaf
  label: string
}
shape Trunk {
  branch: Branch
  note: string
}
function grow(index: int) -> Trunk {
  let leaf: Leaf = { name: `leaf${index}`, numbers: [index, 1] }
  let branch: Branch = { leaf: leaf, label: `branch${index}` }
  return { branch: branch, note: `note${index}` }
}
function entrypoint() -> nothing {
  let total: int = 0
  for (index in range(0, 50)) {
    let branch: Branch = grow(index).branch
    let leaf: Leaf = grow(index).branch.leaf
    total = total + branch.leaf.numbers.count() + leaf.name.count() + branch.label.count()
  }
  print(total)
}
";

case!(
    a_nested_shape_two_levels_deep_taken_from_a_temporary_balances,
    TAKE_A_NESTED_SHAPE_OF_A_NESTED_SHAPE,
    "780\n",
    Balance::HeapAndStrings,
    "taking a nested shape (and one of its own nested shapes) out of a temporary frees the emptied cell twice, or leaves the temporary's release reading the taken cell (High)"
);

// ── Item 6: a reassigned binding owns every value it holds ──────────────────────────────────

const A_REASSIGNED_MAYBE_COPIES_AT_EVERY_EVENT: &str = r"
shape Tag {
  name: string
  id: int
}
function entrypoint() -> nothing {
  let tags: array<Tag> = [{ name: `a${1}`, id: 1 }, { name: `b${2}`, id: 2 }]
  let names: array<string> = [`aa-${1}`, `bb-${2}`]
  let total: int = 0
  for (round in range(0, 50)) {
    let held: maybe<Tag> = tags.get(0)
    let label: maybe<string> = names.get(0)
    held = tags.get(1)
    label = names.get(1)
    total = total + held.or({ name: `z`, id: 0 }).id + label.or(`q`).count()
  }
  print(total)
}
";

case!(
    an_annotated_maybe_reassigned_from_an_element_releases_its_first_value,
    A_REASSIGNED_MAYBE_COPIES_AT_EVERY_EVENT,
    "300\n",
    Balance::HeapAndStrings,
    "an annotated `maybe` whose `let` copies and whose reassignment viewed an element lost the release of its first payload: one string per iteration (High)"
);

const A_REASSIGNED_ARRAY_COPIES_AT_EVERY_EVENT: &str = r"
shape Bucket {
  rows: array<int>
  id: int
}
function entrypoint() -> nothing {
  let bucket: Bucket = { rows: [10, 20, 30], id: 1 }
  let total: int = 0
  for (round in range(0, 50)) {
    let rows: array<int> = [round]
    rows = bucket.rows
    total = total + rows.count()
  }
  print(total)
}
";

case!(
    an_array_reassigned_from_a_field_releases_its_first_value_and_its_copy,
    A_REASSIGNED_ARRAY_COPIES_AT_EVERY_EVENT,
    "150\n",
    Balance::Heap,
    "an array binding that is reassigned from a field it viewed lost its first value (leak) or freed the field's own array (High)"
);

// ── Item 6: an errors-capable local that is reassigned releases its old result ──────────────

const AN_ERRORS_CAPABLE_LOCAL_REASSIGNED_RELEASES_THE_OLD_RESULT: &str = r"
function load(index: int) -> string errors {
  return `value-${index}`
}
function loadRows(index: int) -> array<int> errors {
  return [index, index]
}
function entrypoint() -> nothing {
  let total: int = 0
  let result = load(0)
  let rows = loadRows(0)
  for (index in range(0, 50)) {
    result = load(index)
    rows = loadRows(index)
    if (result.failed()) {
      print(`bad`)
    }
    total = total + result.count()
    let taken: string = result
    total = total + taken.count()
    result = load(index + 1)
    total = total + result.or(`x`).count() + rows.or([]).count()
  }
  print(total)
}
";

case!(
    an_errors_capable_local_reassigned_releases_the_old_result,
    AN_ERRORS_CAPABLE_LOCAL_REASSIGNED_RELEASES_THE_OLD_RESULT,
    "1271\n",
    Balance::HeapAndStrings,
    "an errors-capable local that is reassigned (outside its loop, after a narrowing read, after its string was taken) leaks the old ok payload or frees a taken one twice (High)"
);
