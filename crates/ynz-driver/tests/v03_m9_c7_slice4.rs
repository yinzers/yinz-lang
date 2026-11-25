//! v0.3-M9 C7 slice 4: the release of a nested-shape cell checks for null only where a `Take` can
//! null it, and a union of heap variants carries a tag so a scope exit can release the variant it
//! holds (`IMP-ownership.md` "Scope-exit release on the plain path", `IMP-unions.md`).
//!
//! Counter cases run at both optimization levels; the counters say every allocation was freed and
//! AddressSanitizer with `free_fill_byte=0` says nothing was freed early or twice. All are High
//! tier: a wrong release is a leak, a double free or a use-after-free, silent at exit 0.

mod support;

use support::{
    assert_asan_and_leaks_clean, assert_asan_clean, assert_heap_balanced, assert_strings_balanced,
    at_both_optimization_levels, run_balanced,
};

// ── Item 0: the nested-shape null check ─────────────────────────────────────────────────────

const NESTED_SHAPE_TAKEN: &str = "
shape Tag {
  name: String
  id: Int
}
shape Wrap {
  inner: Tag
  count: Int
}
function make(index: Int) -> Wrap {
  let tag: Tag = { name: `t${index}`, id: index }
  return { inner: tag, count: 3 }
}
function entrypoint() -> Nothing {
  let taken: Tag = make(5).inner
  print(taken.id)
}
";

// ── Slice 4: a union of heap variants carries a tag ─────────────────────────────────────────

const THE_REPRODUCER: &str = "
function entrypoint() -> Nothing {
  let a: String | Array<Int> = `x`
  let b: String | Array<Int> = [1, 2]
  print(1)
}
";

const A_UNION_HOLDING_A_CHANNEL: &str = "
function keep(share value: String | Channel<Int>) -> Int {
  return 1
}
function relay(share value: String | Channel<Int>) -> String | Channel<Int> {
  return value
}
function make(flag: Boolean) -> String | Channel<Int> {
  if (flag) {
    return `hello`
  }
  let made: Channel<Int> = Channel<Int>()
  return made
}
function entrypoint() -> Nothing {
  let total: Int = 0
  for (round in range(0, 5)) {
    let a: String | Channel<Int> = Channel<Int>()
    let b: String | Channel<Int> = `x${round}`
    let ch: Channel<Int> = Channel<Int>()
    let c: String | Channel<Int> = ch
    let d: String | Channel<Int> = make(false)
    let e: String | Channel<Int> = make(true)
    let f: String | Channel<Int> = relay(a)
    total = total + keep(a) + keep(b) + keep(c) + keep(d) + keep(e) + keep(f)
  }
  print(total)
}
";

const PARAMETER_RETURN_STORE_AND_REASSIGN: &str = "
shape Holder {
  held: String | Array<Int>
  label: String
}
function keep(share value: String | Array<Int>) -> Int {
  return 1
}
function consume(give value: String | Array<Int>) -> Int {
  return 2
}
function make(flag: Boolean) -> String | Array<Int> {
  if (flag) {
    return `hello`
  }
  return [1, 2, 3]
}
function relay(share value: String | Array<Int>) -> String | Array<Int> {
  return value
}
function entrypoint() -> Nothing {
  let total: Int = 0
  for (round in range(0, 20)) {
    let a: String | Array<Int> = make(true)
    let b: String | Array<Int> = make(false)
    total = total + keep(a) + keep(b)
    let c: String | Array<Int> = relay(a)
    let d: String | Array<Int> = `d${round}`
    total = total + consume(d)
    let holder: Holder = { held: b, label: `l${round}` }
    a = make(false)
    total = total + keep(a)
  }
  print(total)
}
";

const A_SHARED_UNION_RETURNED: &str = "
shape Circle {
  radius: Int
}
function relay(share value: Circle | String) -> Circle | String {
  return value
}
function entrypoint() -> Nothing {
  let a: Circle | String = `x${1}`
  let c: Circle | String = relay(a)
  print(1)
}
";

const A_UNION_OF_A_STRING_AND_AN_INT: &str = "
function keep(share value: String | Int) -> Int {
  return 1
}
function entrypoint() -> Nothing {
  let a: String | Int = 5
  let b: String | Int = `x`
  print(keep(a) + keep(b))
}
";

const A_UNION_LOCAL_STORED_INTO_A_RECURSIVE_FIELD: &str = "
shape End {
  flag: Boolean
}
shape Node {
  value: Int
  label: String
  next: Node | End
}
function entrypoint() -> Nothing {
  let total: Int = 0
  for (round in range(0, 20)) {
    let stop: End = { flag: true }
    let tail: End = { flag: false }
    let head: Node = { value: 1, label: `head${round}`, next: stop }
    let second: Node = { value: 9, label: `second${round}`, next: tail }
    let wrapped: Node | End = second
    head.next = wrapped
    total = total + head.value
  }
  print(total)
}
";

const SUSPENDING_FUNCTIONS_PASS_AND_RETURN_A_HEAP_UNION: &str = "
shape Holder {
  held: String | Array<Int>
  label: String
}
function keep(share value: String | Array<Int>) -> Int {
  wait sleep(1)
  return 1
}
function make(flag: Boolean) -> String | Array<Int> {
  wait sleep(1)
  if (flag) {
    return `hello`
  }
  return [1, 2, 3]
}
function take(give value: String | Array<Int>) -> Int {
  wait sleep(1)
  return 2
}
function relay(share value: String | Array<Int>) -> String | Array<Int> {
  wait sleep(1)
  return value
}
function entrypoint() -> Nothing {
  wait sleep(1)
  wait make(true)
  let a: String | Array<Int> = `x`
  let first: Int = wait keep(a)
  let b: String | Array<Int> = [1]
  let second: Int = wait take(b)
  let h: Holder = { held: `s`, label: `l` }
  print(first + second)
}
";

const A_UNION_CALL_RESULT_STORED_INTO_A_RECURSIVE_FIELD: &str = "
shape End {
  flag: Boolean
}
shape Node {
  value: Int
  label: String
  next: Node | End
}
function build(round: Int) -> Node | End {
  let tail: End = { flag: false }
  let second: Node = { value: 9, label: `second${round}`, next: tail }
  let wrapped: Node | End = second
  return wrapped
}
function entrypoint() -> Nothing {
  let total: Int = 0
  for (round in range(0, 20)) {
    let stop: End = { flag: true }
    let head: Node = { value: 1, label: `head${round}`, next: stop }
    head.next = build(round)
    let again: Node | End = build(round)
    total = total + head.value
  }
  print(total)
}
";

/// One named test per program of the `slice 4 nested take and tagged union release` run unit; each case runs at both optimization levels (counters and AddressSanitizer).
macro_rules! slice4_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a nested-shape take or a tagged union of heap variants is leaked, freed twice or read stale at a scope exit, silently at exit 0 | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

slice4_high_case! {
    // case: Bug (High: double free): the take nulls the cell in the temporary; the temporary's release of that shape must skip it.
    a_nested_shape_a_take_moves_out_of_a_temporary_is_released_once => {
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced("taken", NESTED_SHAPE_TAKEN, "5\n", no_optimize);
            assert_heap_balanced("taken", &counters, no_optimize);
            assert_strings_balanced("taken", &counters, no_optimize);
            assert_asan_clean("taken", NESTED_SHAPE_TAKEN, "5\n", no_optimize);
        });
    }
    // case: Bug (High: leak): `UntaggedUnion` was held as one raw pointer with no tag, so a scope exit could not say which variant to free and leaked the string and the array's two allocations.
    a_union_of_a_string_and_an_array_releases_the_variant_it_holds => {
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced("reproducer", THE_REPRODUCER, "1\n", no_optimize);
            assert_eq!(
                (counters.alloc, counters.free),
                (2, 2),
                "no_optimize={no_optimize}: the array header and buffer: {counters:?}"
            );
            assert_eq!(
                (counters.string_alloc, counters.string_free),
                (1, 1),
                "no_optimize={no_optimize}: the one string: {counters:?}"
            );
            assert_asan_and_leaks_clean("reproducer", THE_REPRODUCER, "1\n", no_optimize);
        });
    }
    // case: Bug (High: leak, or a double free of the counted channel): a union local, a `return` of one and a copy of one that holds a channel must each hold one counted reference and give it back once. LeakSanitizer reports an `Arc` nobody gave back; ASan a reference given back twice.
    a_union_holding_a_channel_gives_its_reference_back_once => {
        at_both_optimization_levels(|no_optimize| {
            assert_asan_and_leaks_clean(
                "union_channel",
                A_UNION_HOLDING_A_CHANNEL,
                "30\n",
                no_optimize,
            );
            let counters = run_balanced(
                "union_channel",
                A_UNION_HOLDING_A_CHANNEL,
                "30\n",
                no_optimize,
            );
            assert_strings_balanced("union_channel", &counters, no_optimize);
        });
    }
    // case: Bug (High: leak, double free): each path that moves or copies a heap-variant union (a `share` parameter, a `give` parameter, a `return` of a parameter and of a fresh value, a shape field, a reassignment) must free the one variant each owner holds exactly once.
    a_union_of_heap_variants_balances_through_parameter_return_store_and_reassignment => {
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced(
                "union_flow",
                PARAMETER_RETURN_STORE_AND_REASSIGN,
                "100\n",
                no_optimize,
            );
            assert_heap_balanced("union_flow", &counters, no_optimize);
            assert_strings_balanced("union_flow", &counters, no_optimize);
            assert_asan_and_leaks_clean(
                "union_flow",
                PARAMETER_RETURN_STORE_AND_REASSIGN,
                "100\n",
                no_optimize,
            );
        });
    }
    // case: Bug (High: double free, found while building the tag): a `return` of a `share` union parameter persisted the slot by bytes, so the caller's binding and the returned value held one string and both freed it. A copy of a slot that holds a string, array, map or channel variant now copies the value (`union_persist_into_as`, `union_deepen_fn`).
    a_string_variant_a_return_copies_is_not_freed_by_both_owners => {
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced("shared_return", A_SHARED_UNION_RETURNED, "1\n", no_optimize);
            assert_strings_balanced("shared_return", &counters, no_optimize);
            assert_asan_and_leaks_clean("shared_return", A_SHARED_UNION_RETURNED, "1\n", no_optimize);
        });
    }
    // case: Bug (High: crash): `string | int` was classified as its one heap variant, so the scope exit freed the raw bits of `5` as a string pointer (segfault). It is a tagged union.
    a_union_of_a_string_and_a_scalar_does_not_free_the_scalar_as_a_string => {
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced(
                "string_or_int",
                A_UNION_OF_A_STRING_AND_AN_INT,
                "2\n",
                no_optimize,
            );
            assert_strings_balanced("string_or_int", &counters, no_optimize);
            assert_asan_and_leaks_clean(
                "string_or_int",
                A_UNION_OF_A_STRING_AND_AN_INT,
                "2\n",
                no_optimize,
            );
        });
    }
    // case: Bug (High: leak): `head.next = wrapped` allocated a fresh box for the moved source and left the source's emptied box unfreed, one 32-byte cell per store (20 rounds: 80 allocations, 60 frees). A moved union binding hands its box cell to the field.
    a_union_local_stored_into_a_recursive_union_field_leaves_no_box_shell => {
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced(
                "recursive_store",
                A_UNION_LOCAL_STORED_INTO_A_RECURSIVE_FIELD,
                "20\n",
                no_optimize,
            );
            assert_eq!(
                (counters.alloc, counters.free),
                (60, 60),
                "no_optimize={no_optimize}: three cells per round (two shapes' boxes, one `End` box): {counters:?}"
            );
            assert_strings_balanced("recursive_store", &counters, no_optimize);
            assert_asan_and_leaks_clean(
                "recursive_store",
                A_UNION_LOCAL_STORED_INTO_A_RECURSIVE_FIELD,
                "20\n",
                no_optimize,
            );
        });
    }
    // case: Bug (High: use-after-free or a stale frame read): in a state-machine function a heap-variant union is a parameter region, a returned region and a field of a local shape. A state-machine function releases nothing yet (C8), so this checks ASan with leak detection off: nothing is read from a dead frame or freed early.
    a_suspending_function_passes_and_returns_a_heap_union_without_a_stale_read => {
        at_both_optimization_levels(|no_optimize| {
            assert_asan_clean(
                "sm_heap_union",
                SUSPENDING_FUNCTIONS_PASS_AND_RETURN_A_HEAP_UNION,
                "3\n",
                no_optimize,
            );
        });
    }
    // case: Bug (High: leak): a moved recursive-union value a function returned was copied into a new box by the store and the box the call returned was dropped, 40 shells over 20 rounds (140 allocations, 100 frees).
    a_union_call_result_stored_into_a_field_or_a_let_leaves_no_box_shell => {
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced(
                "recursive_call_result",
                A_UNION_CALL_RESULT_STORED_INTO_A_RECURSIVE_FIELD,
                "20\n",
                no_optimize,
            );
            assert_eq!(
                (counters.alloc, counters.free),
                (100, 100),
                "no_optimize={no_optimize}: {counters:?}"
            );
            assert_asan_and_leaks_clean(
                "recursive_call_result",
                A_UNION_CALL_RESULT_STORED_INTO_A_RECURSIVE_FIELD,
                "20\n",
                no_optimize,
            );
        });
    }
}
