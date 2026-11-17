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
    at_both_optimization_levels, build_source, run_balanced,
};

// ── Item 0: the nested-shape null check ─────────────────────────────────────────────────────

const NESTED_SHAPE_NEVER_TAKEN: &str = "
shape Tag {
  name: string
  id: int
}
shape Wrap {
  inner: Tag
  count: int
}
function entrypoint() -> nothing {
  let tag: Tag = { name: `t`, id: 4 }
  let wrap: Wrap = { inner: tag, count: 3 }
  print(wrap.inner.id + wrap.count)
}
";

const NESTED_SHAPE_TAKEN: &str = "
shape Tag {
  name: string
  id: int
}
shape Wrap {
  inner: Tag
  count: int
}
function make(index: int) -> Wrap {
  let tag: Tag = { name: `t${index}`, id: index }
  return { inner: tag, count: 3 }
}
function entrypoint() -> nothing {
  let taken: Tag = make(5).inner
  print(taken.id)
}
";

fn ir_of(program: &str, label: &str, no_optimize: bool) -> String {
    let built = build_source(program, label, no_optimize);
    std::fs::read_to_string(&built.ir).expect("read emitted IR")
}

#[test]
fn a_nested_shape_no_take_reads_is_released_without_a_null_check() {
    // Bug (cost, Golden Rule 8; High: the check is what keeps a taken cell from a double free, so
    // it must stay where a take exists): a cell release that checks for null where nothing can
    // null the cell costs 0.1-0.25 ns per release over Rust's `Box` drop.
    let ir = ir_of(NESTED_SHAPE_NEVER_TAKEN, "never_taken", true);
    assert!(
        ir.contains("ynz_release_shape:Tag_cell") && !ir.contains("release_cell_taken"),
        "no `Take` reads a `Tag`: its cell release must not test for null:\n{ir}"
    );
}

#[test]
fn a_nested_shape_a_take_reads_is_released_behind_a_null_check_and_balances() {
    // Bug (High: double free): the take nulls the cell in the temporary; the temporary's release
    // of that shape must skip it.
    let ir = ir_of(NESTED_SHAPE_TAKEN, "taken", true);
    assert!(
        ir.contains("release_cell_taken"),
        "a `Take` moves a `Tag` out of a temporary: its cell release must test for null:\n{ir}"
    );
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("taken", NESTED_SHAPE_TAKEN, "5\n", no_optimize);
        assert_heap_balanced("taken", &counters, no_optimize);
        assert_strings_balanced("taken", &counters, no_optimize);
        assert_asan_clean("taken", NESTED_SHAPE_TAKEN, "5\n", no_optimize);
    });
}

// ── Slice 4: a union of heap variants carries a tag ─────────────────────────────────────────

const THE_REPRODUCER: &str = "
function entrypoint() -> nothing {
  let a: string | array<int> = `x`
  let b: string | array<int> = [1, 2]
  print(1)
}
";

#[test]
fn a_union_of_a_string_and_an_array_releases_the_variant_it_holds() {
    // Bug (High: leak): `UntaggedUnion` was held as one raw pointer with no tag, so a scope exit
    // could not say which variant to free and leaked the string and the array's two allocations.
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

const A_UNION_HOLDING_A_CHANNEL: &str = "
function keep(share value: string | channel<int>) -> int {
  return 1
}
function relay(share value: string | channel<int>) -> string | channel<int> {
  return value
}
function make(flag: boolean) -> string | channel<int> {
  if (flag) {
    return `hello`
  }
  let made: channel<int> = channel<int>()
  return made
}
function entrypoint() -> nothing {
  let total: int = 0
  for (round in range(0, 5)) {
    let a: string | channel<int> = channel<int>()
    let b: string | channel<int> = `x${round}`
    let ch: channel<int> = channel<int>()
    let c: string | channel<int> = ch
    let d: string | channel<int> = make(false)
    let e: string | channel<int> = make(true)
    let f: string | channel<int> = relay(a)
    total = total + keep(a) + keep(b) + keep(c) + keep(d) + keep(e) + keep(f)
  }
  print(total)
}
";

#[test]
fn a_union_holding_a_channel_gives_its_reference_back_once() {
    // Bug (High: leak, or a double free of the counted channel): a union local, a `return` of one
    // and a copy of one that holds a channel must each hold one counted reference and give it back
    // once. LeakSanitizer reports an `Arc` nobody gave back; ASan a reference given back twice.
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

const PARAMETER_RETURN_STORE_AND_REASSIGN: &str = "
shape Holder {
  held: string | array<int>
  label: string
}
function keep(share value: string | array<int>) -> int {
  return 1
}
function consume(give value: string | array<int>) -> int {
  return 2
}
function make(flag: boolean) -> string | array<int> {
  if (flag) {
    return `hello`
  }
  return [1, 2, 3]
}
function relay(share value: string | array<int>) -> string | array<int> {
  return value
}
function entrypoint() -> nothing {
  let total: int = 0
  for (round in range(0, 20)) {
    let a: string | array<int> = make(true)
    let b: string | array<int> = make(false)
    total = total + keep(a) + keep(b)
    let c: string | array<int> = relay(a)
    let d: string | array<int> = `d${round}`
    total = total + consume(d)
    let holder: Holder = { held: b, label: `l${round}` }
    a = make(false)
    total = total + keep(a)
  }
  print(total)
}
";

#[test]
fn a_union_of_heap_variants_balances_through_parameter_return_store_and_reassignment() {
    // Bug (High: leak, double free): each path that moves or copies a heap-variant union
    // (a `share` parameter, a `give` parameter, a `return` of a parameter and of a fresh value, a
    // shape field, a reassignment) must free the one variant each owner holds exactly once.
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

const A_SHARED_UNION_RETURNED: &str = "
shape Circle {
  radius: int
}
function relay(share value: Circle | string) -> Circle | string {
  return value
}
function entrypoint() -> nothing {
  let a: Circle | string = `x${1}`
  let c: Circle | string = relay(a)
  print(1)
}
";

#[test]
fn a_string_variant_a_return_copies_is_not_freed_by_both_owners() {
    // Bug (High: double free, found while building the tag): a `return` of a `share` union
    // parameter persisted the slot by bytes, so the caller's binding and the returned value held
    // one string and both freed it. A copy of a slot that holds a string, array, map or channel
    // variant now copies the value (`union_persist_into_as`, `union_deepen_fn`).
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("shared_return", A_SHARED_UNION_RETURNED, "1\n", no_optimize);
        assert_strings_balanced("shared_return", &counters, no_optimize);
        assert_asan_and_leaks_clean("shared_return", A_SHARED_UNION_RETURNED, "1\n", no_optimize);
    });
}

const A_UNION_OF_A_STRING_AND_AN_INT: &str = "
function keep(share value: string | int) -> int {
  return 1
}
function entrypoint() -> nothing {
  let a: string | int = 5
  let b: string | int = `x`
  print(keep(a) + keep(b))
}
";

#[test]
fn a_union_of_a_string_and_a_scalar_does_not_free_the_scalar_as_a_string() {
    // Bug (High: crash): `string | int` was classified as its one heap variant, so the scope exit
    // freed the raw bits of `5` as a string pointer (segfault). It is a tagged union.
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

const A_UNION_LOCAL_STORED_INTO_A_RECURSIVE_FIELD: &str = "
shape End {
  flag: boolean
}
shape Node {
  value: int
  label: string
  next: Node | End
}
function entrypoint() -> nothing {
  let total: int = 0
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

#[test]
fn a_union_local_stored_into_a_recursive_union_field_leaves_no_box_shell() {
    // Bug (High: leak): `head.next = wrapped` allocated a fresh box for the moved source and left
    // the source's emptied box unfreed, one 32-byte cell per store (20 rounds: 80 allocations, 60
    // frees). A moved union binding hands its box cell to the field.
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

const SUSPENDING_FUNCTIONS_PASS_AND_RETURN_A_HEAP_UNION: &str = "
shape Holder {
  held: string | array<int>
  label: string
}
function keep(share value: string | array<int>) -> int {
  wait sleep(1)
  return 1
}
function make(flag: boolean) -> string | array<int> {
  wait sleep(1)
  if (flag) {
    return `hello`
  }
  return [1, 2, 3]
}
function take(give value: string | array<int>) -> int {
  wait sleep(1)
  return 2
}
function relay(share value: string | array<int>) -> string | array<int> {
  wait sleep(1)
  return value
}
function entrypoint() -> nothing {
  wait sleep(1)
  wait make(true)
  let a: string | array<int> = `x`
  let first: int = wait keep(a)
  let b: string | array<int> = [1]
  let second: int = wait take(b)
  let h: Holder = { held: `s`, label: `l` }
  print(first + second)
}
";

#[test]
fn a_suspending_function_passes_and_returns_a_heap_union_without_a_stale_read() {
    // Bug (High: use-after-free or a stale frame read): in a state-machine function a heap-variant
    // union is a parameter region, a returned region and a field of a local shape. A state-machine
    // function releases nothing yet (C8), so this checks ASan with leak detection off: nothing is
    // read from a dead frame or freed early.
    at_both_optimization_levels(|no_optimize| {
        assert_asan_clean(
            "sm_heap_union",
            SUSPENDING_FUNCTIONS_PASS_AND_RETURN_A_HEAP_UNION,
            "3\n",
            no_optimize,
        );
    });
}

const A_UNION_CALL_RESULT_STORED_INTO_A_RECURSIVE_FIELD: &str = "
shape End {
  flag: boolean
}
shape Node {
  value: int
  label: string
  next: Node | End
}
function build(round: int) -> Node | End {
  let tail: End = { flag: false }
  let second: Node = { value: 9, label: `second${round}`, next: tail }
  let wrapped: Node | End = second
  return wrapped
}
function entrypoint() -> nothing {
  let total: int = 0
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

#[test]
fn a_union_call_result_stored_into_a_field_or_a_let_leaves_no_box_shell() {
    // Bug (High: leak): a moved recursive-union value a function returned was copied into a new
    // box by the store and the box the call returned was dropped, 40 shells over 20 rounds
    // (140 allocations, 100 frees).
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

// ── PM ruling B: the tag follows the heap ───────────────────────────────────────────────────

/// A function that takes a union, and a `let` that holds one: the two places a union gets a slot.
fn union_program(union_type: &str, value: &str) -> String {
    format!(
        "
function show(value: {union_type}) -> nothing {{
  print(1)
}}
function entrypoint() -> nothing {{
  let held: {union_type} = {value}
  show(held)
  print(2)
}}
"
    )
}

#[test]
fn a_union_of_value_types_has_no_tag_and_a_union_with_a_heap_variant_has() {
    // Bug caught (High: layout and release): the union tag `{ i64 tag, payload }` applies only to a
    // union with at least one heap variant (`string | int` included), as the Product Manager ruled
    // (ruling B). A value-only union (`int | float`) is the variant's bare bits: no slot struct,
    // no release function. A tag on it would cost a word per value and a release with nothing to
    // release.
    at_both_optimization_levels(|no_optimize| {
        let value_only = build_source(
            &union_program("int | float", "3"),
            "value_only_union",
            no_optimize,
        );
        let value_only_ir = std::fs::read_to_string(&value_only.ir).expect("read the IR");
        assert!(
            !value_only_ir.contains("{ i64, [1 x i64] }")
                && !value_only_ir.contains("{ i64, i64 }"),
            "no_optimize={no_optimize}: `int | float` must have no tagged slot:\n{value_only_ir}"
        );
        assert!(
            !value_only_ir.contains("ynz_release_union"),
            "no_optimize={no_optimize}: `int | float` has nothing to release:\n{value_only_ir}"
        );
    });
    // The optimizer may fold a tag away, so the positive half reads the unoptimized IR.
    let with_heap = build_source(&union_program("string | int", "3"), "heap_union", true);
    let with_heap_ir = std::fs::read_to_string(&with_heap.ir).expect("read the IR");
    assert!(
        with_heap_ir.contains("{ i64, [1 x i64] }") && with_heap_ir.contains("ynz_release_union("),
        "`string | int` must carry a tag word and release by it:\n{with_heap_ir}"
    );
}
