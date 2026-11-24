// v0.3-M9 C6: which reads of a place are a view of it or a copy (`non_owned_reads`).
//
// WHY (High tables: memory safety / data integrity; Medium tables: cost only): a view points at the place instead of copying
// it, so a view that outlives a write to the place reads a changed or freed value, and a view that
// is written changes the place under its owner. A binding that could be a view but is copied is a
// clone where Rust borrows (Golden Rule 6: a recursive walk that peels one link per call copies the
// whole tail at every step). Typeck's record (`TypedModule::non_owned_reads`) decides, so these tests
// assert the record directly; the alloc counts and the AddressSanitizer runs are in the driver.
//
// Each case is one NAMED test over a small program. The view is named by its value expression
// (`outer.inner`) and which occurrence of that text it is.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::check_query;

/// Check `program`, and assert for each `(text, occurrence, expected)` that the `let` whose value
/// is the `occurrence`-th (1-based) occurrence of `text` is (or is not) a view.
fn assert_views(prelude: &str, program_body: &str, expectations: &[(&str, usize, bool)]) {
    let program = &format!("{prelude}{program_body}");
    let db = CompilerDb::default();
    let source_file = SourceFile::new(&db, "test.ynz".to_string(), program.to_string());
    let output = (*check_query(&db, source_file)).clone();
    let errors: Vec<String> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, ynz_diagnostics::Severity::Error))
        .map(|diagnostic| format!("{diagnostic:?}"))
        .collect();
    assert!(errors.is_empty(), "program must check clean: {errors:#?}");
    for (text, occurrence, expected) in expectations {
        let start = program
            .match_indices(text)
            .nth(occurrence - 1)
            .unwrap_or_else(|| panic!("no occurrence {occurrence} of `{text}` in:\n{program}"))
            .0;
        let found = output
            .typed_module
            .non_owned_reads
            .views()
            .any(|(from, to)| from == start && to == start + text.len());
        assert_eq!(
            found, *expected,
            "`{text}` (occurrence {occurrence}) view: expected {expected}, got {found} in:\n{program}"
        );
    }
}

const PRELUDE: &str = "shape Inner {\n  id: Int\n}\nshape Outer {\n  inner: Inner\n  tag: Int\n}\nfunction bump(lend outer: Outer) -> Nothing {\n  outer.tag = 3\n}\n";
const CHAIN_PRELUDE: &str = "shape Inner {\n  id: Int\n}\nshape Middle {\n  inner: Inner\n  tag: Int\n}\nshape Wrapper {\n  middle: Middle\n  count: Int\n}\n";
const ELEMENT_PRELUDE: &str = "shape Inner {\n  id: Int\n}\nfunction bump(lend items: Array<Inner>) -> Nothing {\n  let extra: Inner = { id: 5 }\n  items.add(extra)\n}\n";

macro_rules! high_view_cases {
    ($($case:ident: $prelude:expr, $program:expr, [$($expectation:expr),+ $(,)?], $bug:literal;)+) => {
        $(
            // bug: typeck marks a let a view while a write, a lend call, a reassignment, a suspension or a spawn can change or free its place (read of changed or freed memory), or marks a read a view that must copy | tier: High
            #[doc = $bug]
            #[test]
            fn $case() {
                assert_views($prelude, $program, &[$($expectation),+]);
            }
        )+
    };
}

macro_rules! medium_view_cases {
    ($($case:ident: $prelude:expr, $program:expr, [$($expectation:expr),+ $(,)?], $bug:literal;)+) => {
        $(
            // bug: typeck refuses a view of a place nothing writes while the view is live, so the let copies where Rust borrows (extra cells) | tier: Medium
            #[doc = $bug]
            #[test]
            fn $case() {
                assert_views($prelude, $program, &[$($expectation),+]);
            }
        )+
    };
}

high_view_cases! {
    a_field_read_from_a_share_parameter_is_a_view:
        PRELUDE, "function read(share outer: Outer) -> Int {\n  let inner = outer.inner\n  return inner.id + outer.tag\n}\nfunction entrypoint() -> Nothing {\n  print(`x`)\n}\n",
        [("outer.inner", 1, true)],
        "a binding of a field of a `share` parameter copied where Rust borrows (O(n^2) cells in a recursive walk, Rule 6)";
    a_root_field_written_while_the_view_is_live_is_not_a_view:
        PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let second: Inner = { id: 9 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  outer.inner = second\n  print(inner.id)\n}\n",
        [("outer.inner", 2, false)],
        "a view read after its place was overwritten reads the new value, where the binding must keep the old (value semantics broken, High)";
    a_root_passed_to_a_lend_parameter_while_the_view_is_live_is_not_a_view:
        PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  bump(outer)\n  print(inner.id)\n}\n",
        [("outer.inner", 1, false)],
        "a view live across a call that may write its root, read after the write (High)";
    a_root_reassigned_while_the_view_is_live_is_not_a_view:
        PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let second: Inner = { id: 9 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let other: Outer = { inner: second, tag: 4 }\n  let inner = outer.inner\n  outer = other\n  print(inner.id)\n}\n",
        [("outer.inner", 1, false)],
        "a view of a nested shape read after its root was reassigned (the old root's cells are released) reads freed memory (C7 slice 3b, High)";
    a_recursive_union_field_written_while_the_view_is_live_is_not_a_view:
        PRELUDE, "shape End {\n  flag: Boolean\n}\nshape Node {\n  value: Int\n  next: Node | End\n}\nfunction entrypoint() -> Nothing {\n  let stop: End = { flag: true }\n  let last: Node = { value: 3, next: stop }\n  let head: Node = { value: 1, next: last }\n  let view = head.next\n  let tail: End = { flag: false }\n  let wrapped: Node | End = tail\n  head.next = wrapped\n  print(head.value)\n  if (view) {\n    is Node => {\n      print(view.value)\n    }\n    is End => {\n      print(0)\n    }\n  }\n}\n",
        [("head.next", 1, false)],
        "a view of a recursive-union box read after a field assignment released the box (C7 slice 3b, High)";
    a_written_view_is_not_a_view:
        PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  inner.id = 5\n  print(inner.id)\n}\n",
        [("outer.inner", 1, false)],
        "a write through a view that changes the place under its owner (High)";
    a_returned_view_is_not_a_view:
        PRELUDE, "function pick(share outer: Outer) -> Inner {\n  let inner = outer.inner\n  return inner\n}\nfunction entrypoint() -> Nothing {\n  print(`x`)\n}\n",
        [("outer.inner", 1, false)],
        "a view that leaves the function pointing into a place the caller still holds (High)";
    an_annotated_binding_is_not_a_view:
        PRELUDE, "function read(share outer: Outer) -> Int {\n  let inner: Inner = outer.inner\n  return inner.id\n}\nfunction entrypoint() -> Nothing {\n  print(`x`)\n}\n",
        [("outer.inner", 1, false)],
        "an annotated binding can wrap the value into a union slot, which a view cannot be";
    a_view_live_across_a_written_wait_is_not_a_view:
        PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  wait sleep(1)\n  print(inner.id)\n}\n",
        [("outer.inner", 1, false)],
        "a view held across a suspension is a pointer into a frame the resume function no longer has (High)";
    a_view_live_across_a_background_is_not_a_view:
        PRELUDE, "function tick() -> Nothing {\n  print(`tick`)\n}\nfunction entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  background tick()\n  print(inner.id)\n}\n",
        [("outer.inner", 1, false)],
        "a view live across a `background` spawn read after the task may have run (High)";
    a_bits_field_read_is_not_a_view:
        PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let tag = outer.tag\n  print(tag + outer.inner.id)\n}\n",
        [("outer.tag", 2, false)],
        "a bits read recorded as a view, which codegen would lower as a pointer (boundary of the heap-valued rule)";
    a_write_through_a_target_with_no_root_name_is_not_a_view:
        PRELUDE, "function makeOuter() -> Outer {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  return outer\n}\nfunction entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  makeOuter().tag = 7\n  print(inner.id)\n}\n",
        [("outer.inner", 1, false)],
        "a write whose target names no binding was skipped, so a view live across it kept its place unproven (Fail Fast: unproven must keep the copy)";
    a_loop_variable_is_not_a_view_when_the_body_overwrites_an_item:
        PRELUDE, "function entrypoint() -> Nothing {\n  let rows: Array<Array<Int>> = [[1]]\n  for (row in rows) {\n    rows[0] = [9]\n    print(row.count())\n  }\n}\n",
        [("rows", 2, false)],
        "a loop variable read after the body freed its item (High)";
    a_loop_variable_is_not_a_view_when_a_lend_callee_overwrites_an_item:
        PRELUDE, "function clobber(lend rows: Array<Array<Int>>) -> Nothing {\n  rows[0] = [9]\n}\nfunction entrypoint() -> Nothing {\n  let rows: Array<Array<Int>> = [[1]]\n  for (row in rows) {\n    clobber(rows)\n    print(row.count())\n  }\n}\n",
        [("rows", 3, false)],
        "a loop variable read after a `lend` callee freed its item (High)";
    a_reassigned_binding_does_not_point_through_a_loop_variable:
        PRELUDE, "shape Part {\n  qty: Int\n}\nfunction entrypoint() -> Nothing {\n  let first: Part = { qty: 1 }\n  let stock: Map<String, Part> = { `a`: first }\n  let keep: Part = { qty: 0 }\n  for (entry in stock) {\n    keep = entry.value\n  }\n  print(keep.qty)\n}\n",
        [("entry.value", 1, false)],
        "a binding assigned from the loop variable kept the last iteration's staging (High: `m5_p3_mapshape_iter_escape`)";
    a_view_passed_to_a_lend_parameter_is_not_a_view:
        PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  rename(inner)\n  print(outer.tag)\n}\nfunction rename(lend inner: Inner) -> Nothing {\n  inner.id = 8\n}\n",
        [("outer.inner", 1, false)],
        "a view handed to a callee that writes it changes the place under its owner (High)";
    a_read_through_a_view_is_a_view_of_the_original_root:
        CHAIN_PRELUDE, "function read(share wrapper: Wrapper) -> Int {\n  let middle = wrapper.middle\n  let inner = middle.inner\n  return inner.id + middle.tag\n}\nfunction entrypoint() -> Nothing {\n  print(`x`)\n}\n",
        [("wrapper.middle", 1, true), ("middle.inner", 1, true)],
        "`let b = a.g` with `a` a view of `x` copied the tail again (O(n^2) over a chain of aliases, Rule 6)";
    a_chained_view_is_not_a_view_when_the_original_root_is_written:
        CHAIN_PRELUDE, "function entrypoint() -> Nothing {\n  let leaf: Inner = { id: 1 }\n  let middle: Middle = { inner: leaf, tag: 2 }\n  let wrapper: Wrapper = { middle: middle, count: 3 }\n  let held = wrapper.middle\n  let inner = held.inner\n  wrapper.count = 4\n  print(inner.id + held.tag)\n}\n",
        [("wrapper.middle", 1, false), ("held.inner", 1, false)],
        "a view read through another after the original root was written reads a changed place (High)";
    a_chained_view_is_not_a_view_when_the_view_it_goes_through_is_written:
        CHAIN_PRELUDE, "function read(share wrapper: Wrapper) -> Int {\n  let held = wrapper.middle\n  let inner = held.inner\n  held.tag = 7\n  return inner.id\n}\nfunction entrypoint() -> Nothing {\n  print(`x`)\n}\n",
        [("wrapper.middle", 1, false), ("held.inner", 1, false)],
        "a view whose parent view is written, which changes the place both name (High)";
    an_element_view_is_not_a_view_when_the_collection_grows_while_it_is_read:
        ELEMENT_PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let second: Inner = { id: 2 }\n  let items: Array<Inner> = [first]\n  let item = items[0]\n  items.add(second)\n  print(item.exists())\n}\n",
        [("items[0]", 1, false)],
        "an element view read after `add` reallocated the buffer reads freed memory (High)";
    an_element_view_is_not_a_view_when_the_collection_is_passed_to_a_lend_parameter:
        ELEMENT_PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let items: Array<Inner> = [first]\n  let item = items[0]\n  bump(items)\n  print(item.exists())\n}\n",
        [("items[0]", 1, false)],
        "an element view read after a callee may write the collection (High)";
    an_element_view_is_not_a_view_when_an_element_is_replaced_while_it_is_read:
        ELEMENT_PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let second: Inner = { id: 2 }\n  let items: Array<Inner> = [first]\n  let item = items[0]\n  items.set(0, second)\n  print(item.exists())\n}\n",
        [("items[0]", 1, false)],
        "an element view read after `set` replaced the element reads the new value (High)";
}

// Medium cases: each a different place or read form (field of a share parameter, of an unwritten
// local, element reads by index/get/first/last, chained view, loop variable, view passed to a
// reader) whose view the record must keep, because refusing it only costs a copy.
medium_view_cases! {
    a_field_read_from_a_local_that_is_not_written_is_a_view:
        PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  print(inner.id + outer.tag)\n}\n",
        [("outer.inner", 1, true)],
        "a local root read but never written treated as written, so every binding of it copies (cost)";
    a_root_written_after_the_last_read_of_the_view_is_still_a_view:
        PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  print(inner.id)\n  outer.tag = 5\n}\n",
        [("outer.inner", 1, true)],
        "a write after the view's last read ends nothing, so refusing the view costs a copy (cost)";
    a_loop_variable_is_a_view_while_the_body_never_writes_the_iterated_array:
        PRELUDE, "function entrypoint() -> Nothing {\n  let rows: Array<Array<Int>> = [[1]]\n  for (row in rows) {\n    print(row.count())\n  }\n}\n",
        [("rows", 2, true)],
        "a loop variable copied per iteration although nothing writes the array (cost, Rule 6)";
    a_view_passed_to_a_share_parameter_is_still_a_view:
        PRELUDE, "function look(share inner: Inner) -> Int {\n  return inner.id\n}\nfunction entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let outer: Outer = { inner: first, tag: 2 }\n  let inner = outer.inner\n  print(look(inner) + outer.tag)\n}\n",
        [("outer.inner", 1, true)],
        "a view refused because it was read by a callee that only reads it (cost, Rule 6)";
    an_index_read_of_a_share_parameter_is_a_view:
        ELEMENT_PRELUDE, "function read(share items: Array<Inner>) -> Int {\n  let item = items[0]\n  if (item.exists()) {\n    return item.value.id\n  }\n  return 0\n}\nfunction entrypoint() -> Nothing {\n  print(`x`)\n}\n",
        [("items[0]", 1, true)],
        "an element read copied where Rust borrows (cost, Rule 6)";
    a_get_read_of_a_share_parameter_is_a_view:
        ELEMENT_PRELUDE, "function read(share items: Array<Inner>) -> Int {\n  let item = items.get(1)\n  if (item.exists()) {\n    return item.value.id\n  }\n  return 0\n}\nfunction entrypoint() -> Nothing {\n  print(`x`)\n}\n",
        [("items.get(1)", 1, true)],
        "an element read copied where Rust borrows (cost, Rule 6)";
    a_first_read_of_a_share_parameter_is_a_view:
        ELEMENT_PRELUDE, "function read(share items: Array<Inner>) -> Int {\n  let item = items.first()\n  if (item.exists()) {\n    return item.value.id\n  }\n  return 0\n}\nfunction entrypoint() -> Nothing {\n  print(`x`)\n}\n",
        [("items.first()", 1, true)],
        "an element read copied where Rust borrows (cost, Rule 6)";
    a_last_read_of_a_share_parameter_is_a_view:
        ELEMENT_PRELUDE, "function read(share items: Array<Inner>) -> Int {\n  let item = items.last()\n  if (item.exists()) {\n    return item.value.id\n  }\n  return 0\n}\nfunction entrypoint() -> Nothing {\n  print(`x`)\n}\n",
        [("items.last()", 1, true)],
        "an element read copied where Rust borrows (cost, Rule 6)";
    an_element_read_of_a_local_collection_is_a_view_until_the_collection_is_written:
        ELEMENT_PRELUDE, "function entrypoint() -> Nothing {\n  let first: Inner = { id: 1 }\n  let items: Array<Inner> = [first]\n  let item = items[0]\n  print(item.exists())\n  let second: Inner = { id: 2 }\n  items.add(second)\n}\n",
        [("items[0]", 1, true)],
        "a write after the view's last read ends nothing (cost)";
}

use ynz_typeck::non_owned_reads::ReadVerdict;

/// Check `program`, and assert for each `(text, occurrence, expected)` that the read at the
/// `occurrence`-th (1-based) occurrence of `text` has exactly the verdict `expected` (`None`: not
/// judged).
fn assert_verdicts(program: &str, expectations: &[(&str, usize, Option<ReadVerdict>)]) {
    let db = CompilerDb::default();
    let source_file = SourceFile::new(&db, "test.ynz".to_string(), program.to_string());
    let output = (*check_query(&db, source_file)).clone();
    let errors: Vec<String> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, ynz_diagnostics::Severity::Error))
        .map(|diagnostic| format!("{diagnostic:?}"))
        .collect();
    assert!(errors.is_empty(), "program must check clean: {errors:#?}");
    for (text, occurrence, expected) in expectations {
        let start = program
            .match_indices(text)
            .nth(occurrence - 1)
            .unwrap_or_else(|| panic!("no occurrence {occurrence} of `{text}` in:\n{program}"))
            .0;
        let found = output
            .typed_module
            .non_owned_reads
            .verdict(start, start + text.len());
        assert_eq!(
            found, *expected,
            "`{text}` (occurrence {occurrence}) verdict: expected {expected:?}, got {found:?} in:\n{program}"
        );
    }
}

const VIEW: Option<ReadVerdict> = Some(ReadVerdict::View);
const COPY: Option<ReadVerdict> = Some(ReadVerdict::MustCopy);

macro_rules! high_verdict_cases {
    ($($case:ident: $program:expr, [$($expectation:expr),+ $(,)?], $bug:literal;)+) => {
        $(
            // bug: typeck records a non-let read (argument, UFCS receiver, reassignment, loop variable, Maybe binding, return) as a view when a write can change or free its place, or judges a site that must stay unjudged | tier: High
            #[doc = $bug]
            #[test]
            fn $case() {
                assert_verdicts($program, &[$($expectation),+]);
            }
        )+
    };
}

macro_rules! medium_verdict_cases {
    ($($case:ident: $program:expr, [$($expectation:expr),+ $(,)?], $bug:literal;)+) => {
        $(
            // bug: typeck records a copy for a non-let read whose place nothing writes, so it copies where Rust borrows (extra cells) | tier: Medium
            #[doc = $bug]
            #[test]
            fn $case() {
                assert_verdicts($program, &[$($expectation),+]);
            }
        )+
    };
}

high_verdict_cases! {
    an_argument_read_from_a_collection_another_argument_lends_is_a_copy:
        "function replace(share first: Array<Int>, lend rows: Array<Array<Int>>) -> Int {\n  rows[0] = [9]\n  return first.count()\n}\nfunction entrypoint() -> Nothing {\n  let rows: Array<Array<Int>> = [[1, 2, 3]]\n  let fallback: Array<Int> = []\n  print(replace(rows[0].or(fallback), rows))\n}\n",
        [("rows[0].or(fallback)", 1, COPY)],
        "a `share` argument that points at an item the call's `lend` argument frees (High)";
    an_argument_for_a_parameter_the_callee_may_write_is_not_judged:
        "function change(lend rows: Array<Int>) -> Nothing {\n  rows.add(1)\n}\nfunction entrypoint() -> Nothing {\n  let rows: Array<Array<Int>> = [[1]]\n  let fallback: Array<Int> = []\n  change(rows[0].or(fallback))\n}\n",
        [("rows[0].or(fallback)", 1, None)],
        "an argument at a `lend` position recorded as a view, which would hand the callee a pointer into the caller's element";
    a_ufcs_receiver_read_from_a_list_another_argument_lends_is_a_copy:
        "shape Bucket {\n  rows: Array<Int>\n  id: Int\n}\nfunction total(share bucket: Bucket, lend others: Array<Bucket>) -> Int {\n  others.set(0, { rows: [7], id: 2 })\n  return bucket.rows.count()\n}\nfunction entrypoint() -> Nothing {\n  let first: Bucket = { rows: [1], id: 1 }\n  let buckets: Array<Bucket> = [first]\n  let fallback: Bucket = { rows: [], id: 0 }\n  print(buckets[0].or(fallback).total(buckets))\n}\n",
        [("buckets[0].or(fallback)", 1, COPY)],
        "a UFCS receiver that points at an element the call's `lend` argument frees (High)";
    a_reassignment_from_a_field_nobody_writes_is_a_copy:
        "shape Bucket {\n  rows: Array<Int>\n  id: Int\n}\nfunction entrypoint() -> Nothing {\n  let bucket: Bucket = { rows: [1, 2], id: 1 }\n  let rows: Array<Int> = [1]\n  rows = bucket.rows\n  print(rows.count())\n}\n",
        [("bucket.rows", 1, COPY)],
        "a reassigned binding owns every value it holds: a view would lose the release of its first value (C7 slice 3b, High)";
    a_reassignment_from_a_field_that_is_then_overwritten_is_a_copy:
        "shape Bucket {\n  rows: Array<Int>\n  id: Int\n}\nfunction entrypoint() -> Nothing {\n  let bucket: Bucket = { rows: [1, 2], id: 1 }\n  let rows: Array<Int> = [1]\n  rows = bucket.rows\n  bucket.rows = [9]\n  print(rows.count())\n}\n",
        [("bucket.rows", 1, COPY)],
        "a reassigned binding that points at the field the next statement overwrites (High)";
    a_map_loop_variable_is_a_copy_when_the_body_writes_the_map:
        "function entrypoint() -> Nothing {\n  let table: Map<String, Array<Int>> = { `a`: [1] }\n  for (entry in table) {\n    table.set(`a`, [9])\n    print(entry.value.count())\n  }\n}\n",
        [("table", 2, COPY)],
        "a map-entry variable that points at a value the body replaced (High)";
    a_loop_variable_of_a_union_list_is_a_copy_when_the_body_overwrites_an_item:
        "shape Tiny {\n  flag: Boolean\n}\nshape Big {\n  name: String\n  weight: Int\n}\nshape Thing = Tiny | Big\nfunction describe(share thing: Thing) -> String {\n  if (thing) {\n    is Big => {\n      return thing.name\n    }\n    is Tiny => {\n      return `tiny`\n    }\n  }\n  return `none`\n}\nfunction entrypoint() -> Nothing {\n  let big: Big = { name: `b`, weight: 5 }\n  let first: Thing = big\n  let things: Array<Thing> = [first]\n  for (thing in things) {\n    let other: Big = { name: `o`, weight: 6 }\n    let wrapped: Thing = other\n    things[0] = wrapped\n    print(describe(thing))\n  }\n}\n",
        [("things", 2, COPY)],
        "a union loop variable that points at an item the body overwrote (High)";
    a_loop_over_a_list_of_ints_is_not_judged:
        "function entrypoint() -> Nothing {\n  let numbers: Array<Int> = [1]\n  for (number in numbers) {\n    print(number)\n  }\n}\n",
        [("numbers", 2, None)],
        "a bits loop variable recorded as a view, which codegen would lower as a pointer";
    a_maybe_array_element_let_is_a_copy_when_the_element_is_overwritten:
        "function entrypoint() -> Nothing {\n  let rows: Array<Array<Int>> = [[1]]\n  let picked = rows.get(0)\n  rows[0] = [9]\n  let fallback: Array<Int> = []\n  let found = picked.or(fallback)\n  print(found.count())\n}\n",
        [("rows.get(0)", 1, COPY)],
        "a `Maybe<Array>` binding that points at the element the list overwrote (High)";
    a_returned_place_read_has_no_entry_because_it_always_copies:
        "shape Bucket {\n  rows: Array<Int>\n  id: Int\n}\nfunction pick(share bucket: Bucket) -> Array<Int> {\n  return bucket.rows\n}\nfunction entrypoint() -> Nothing {\n  let bucket: Bucket = { rows: [1], id: 1 }\n  print(pick(bucket).count())\n}\n",
        [("bucket.rows", 1, None)],
        "a returned field recorded as a view, which would outlive the frame it points into (High)";
}

// Medium cases: each a different site kind (call argument, UFCS receiver, map/union loop
// variable, loop variable across a wait, `Maybe` element) whose view must be kept.
medium_verdict_cases! {
    arguments_of_one_collection_with_no_lend_of_it_are_views:
        "function show(share first: Array<Int>, share second: Array<Int>) -> Int {\n  return first.count() + second.count()\n}\nfunction entrypoint() -> Nothing {\n  let rows: Array<Array<Int>> = [[1], [2, 3]]\n  let fallback: Array<Int> = []\n  print(show(rows[0].or(fallback), rows[1].or(fallback)))\n}\n",
        [("rows[0].or(fallback)", 1, VIEW), ("rows[1].or(fallback)", 1, VIEW)],
        "an argument copied although no argument of the call writes its collection (cost, Rule 6)";
    an_argument_is_a_view_when_the_other_argument_is_a_different_root:
        "function replace(share first: Array<Int>, lend others: Array<Array<Int>>) -> Int {\n  others[0] = [9]\n  return first.count()\n}\nfunction entrypoint() -> Nothing {\n  let rows: Array<Array<Int>> = [[1, 2, 3]]\n  let others: Array<Array<Int>> = [[4]]\n  let fallback: Array<Int> = []\n  print(replace(rows[0].or(fallback), others))\n}\n",
        [("rows[0].or(fallback)", 1, VIEW)],
        "an argument copied because a lend of an unrelated root was taken for a lend of its own (cost, Rule 6)";
    a_ufcs_receiver_with_no_lend_of_its_list_is_a_view:
        "shape Bucket {\n  rows: Array<Int>\n  id: Int\n}\nfunction total(share bucket: Bucket) -> Int {\n  return bucket.rows.count()\n}\nfunction entrypoint() -> Nothing {\n  let first: Bucket = { rows: [1], id: 1 }\n  let buckets: Array<Bucket> = [first]\n  let fallback: Bucket = { rows: [], id: 0 }\n  print(buckets[0].or(fallback).total())\n}\n",
        [("buckets[0].or(fallback)", 1, VIEW)],
        "a UFCS receiver copied although the call writes nothing (cost, Rule 6)";
    a_map_loop_variable_is_a_view_while_the_body_never_writes_the_map:
        "function entrypoint() -> Nothing {\n  let table: Map<String, Array<Int>> = { `a`: [1] }\n  for (entry in table) {\n    print(entry.value.count())\n  }\n}\n",
        [("table", 2, VIEW)],
        "a map-entry loop variable copied per iteration although nothing writes the map (cost, Rule 6)";
    a_loop_variable_of_a_union_list_is_a_view_while_the_body_never_writes_the_list:
        "shape Tiny {\n  flag: Boolean\n}\nshape Big {\n  name: String\n  weight: Int\n}\nshape Thing = Tiny | Big\nfunction describe(share thing: Thing) -> String {\n  if (thing) {\n    is Big => {\n      return thing.name\n    }\n    is Tiny => {\n      return `tiny`\n    }\n  }\n  return `none`\n}\nfunction entrypoint() -> Nothing {\n  let big: Big = { name: `b`, weight: 5 }\n  let first: Thing = big\n  let things: Array<Thing> = [first]\n  for (thing in things) {\n    print(describe(thing))\n  }\n}\n",
        [("things", 2, VIEW)],
        "a union loop variable copied per iteration although nothing writes the list (cost, Rule 6)";
    a_loop_variable_stays_a_view_across_a_wait_while_nothing_writes_the_array:
        "function entrypoint() -> Nothing {\n  let rows: Array<Array<Int>> = [[1]]\n  for (row in rows) {\n    wait sleep(1)\n    print(row.count())\n  }\n}\n",
        [("rows", 2, VIEW)],
        "a loop variable copied only because the body suspends (cost, Rule 6)";
    a_maybe_array_element_let_is_a_view_while_the_list_is_not_written:
        "function entrypoint() -> Nothing {\n  let rows: Array<Array<Int>> = [[1]]\n  let picked = rows.get(0)\n  let fallback: Array<Int> = []\n  let found = picked.or(fallback)\n  print(found.count())\n}\n",
        [("rows.get(0)", 1, VIEW)],
        "a `Maybe<Array>` binding copying its payload although the list is never written (cost, Rule 6)";
}
