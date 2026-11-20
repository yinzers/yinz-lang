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

/// [`assert_balanced_and_clean`] for a program that builds no string.
fn assert_heap_balanced_and_clean(label: &str, program: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        assert_heap_balanced(label, &counters, no_optimize);
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

// ── Group 3: a built-in that builds a fresh heap value is an owner's value ──────────────────

const SPLIT_BOUND_BY_LET: &str = "
function entrypoint() -> Nothing {
  let s: String = `a, b, c`
  let parts: Array<String> = s.split(`, `)
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
function countOf(share cells: Array<String>) -> Int {
  return cells.count()
}
function entrypoint() -> Nothing {
  let s: String = `a,b,c`
  let total: Int = s.split(`,`).count()
  total = total + countOf(s.split(`,`))
  for (part in s.split(`,`)) {
    total = total + part.count()
  }
  let upper: Array<String> = s.toUpperCase().split(`,`)
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
  cells: Array<String>
}
function pieces(share s: String) -> Array<String> {
  return s.split(`,`)
}
function entrypoint() -> Nothing {
  let s: String = `a,b,c`
  let parts: Array<String> = s.split(`,`)
  let row: Row = { cells: parts }
  let nested: Array<Array<String>> = [s.split(`,`).copy()]
  let again: Array<String> = pieces(s)
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

// ── Group 4: a `.copy()` of a `maybe` is one owner, taken once ──────────────────────────────

const MAYBE_COPY_BOUND: &str = "
shape Part {
  qty: Int
}
function entrypoint() -> Nothing {
  let parts: Array<Part> = [{qty: 1}, {qty: 2}]
  let first: Maybe<Part> = parts.get(0)
  let saved: Maybe<Part> = first.copy()
  let again: Maybe<Part> = parts.get(1)
  again = first.copy()
  let numbers: Array<Int> = [4, 5]
  let held: Maybe<Int> = numbers.get(1)
  let heldCopy: Maybe<Int> = held.copy()
  let missing: Maybe<Part> = parts.get(9)
  let missingCopy: Maybe<Part> = missing.copy()
  print(saved.or({qty: 9}).qty + again.or({qty: 9}).qty + heldCopy.or(0) + missingCopy.or({qty: 7}).qty)
}
";

/// Bug (High: leak): `emit_owned_copy` mints a heap envelope cell and a payload for a `maybe`
/// copy, then the `let` copied the envelope again into the binding's slot and orphaned the first
/// (4 allocs / 2 frees for `saved = m.copy()`; 3/2 for a `maybe<int>`).
#[test]
fn a_maybe_copy_bound_by_let_or_assignment_is_taken_not_copied_again() {
    assert_heap_balanced_and_clean("c7r1_maybe_copy_bound", MAYBE_COPY_BOUND, "14\n");
}

const MAYBE_COPY_PERSISTED: &str = "
shape Tag {
  name: String
}
function entrypoint() -> Nothing {
  let tags: Array<Tag> = [{name: `x`}, {name: `yy`}]
  let first: Maybe<Tag> = tags.get(1)
  let boxed: Array<Maybe<Tag> > = [first.copy()]
  boxed.set(0, first.copy())
  boxed.add(first.copy())
  let numbers: Array<Int> = [1, 2]
  let held: Maybe<Int> = numbers.get(1)
  let heldBoxed: Array<Maybe<Int> > = [held.copy()]
  print(boxed.count() + heldBoxed.count())
}
";

/// Bug (High: leak): a `.copy()` of a `maybe` stored into an array literal, `set` or `add` was
/// cloned a second time by the persist (`value_to_stable_bits`), orphaning the copy's cell, payload
/// cell and string.
#[test]
fn a_maybe_copy_stored_into_an_array_is_kept_by_the_array() {
    assert_balanced_and_clean("c7r1_maybe_copy_persisted", MAYBE_COPY_PERSISTED, "3\n");
}

const MAYBE_COPY_TEMPORARY_AND_RETURNED: &str = "
shape Tag {
  name: String
}
function nameLength(share found: Maybe<Tag>) -> Int {
  return found.or({name: `none`}).name.count()
}
function copyOf(share found: Maybe<Tag>) -> Maybe<Tag> {
  return found.copy()
}
function entrypoint() -> Nothing {
  let tags: Array<Tag> = [{name: `x`}, {name: `yy`}]
  let first: Maybe<Tag> = tags.get(1)
  let total: Int = first.copy().or({name: `z`}).name.count()
  total = total + nameLength(first.copy())
  let back: Maybe<Tag> = copyOf(first)
  total = total + nameLength(back)
  print(total)
}
";

/// Bug (High: leak): a `.copy()` of a `maybe` nothing binds (a receiver, a borrowed argument) was
/// not a temporary, and a `return` of one copied it again.
#[test]
fn a_maybe_copy_nothing_binds_or_a_function_returns_is_released_once() {
    assert_balanced_and_clean(
        "c7r1_maybe_copy_temp",
        MAYBE_COPY_TEMPORARY_AND_RETURNED,
        "6\n",
    );
}

// ── Group 5: a `fixed` list of shapes owns the cell of each item ────────────────────────────

const FIXED_OF_FLAT_SHAPES: &str = "
shape Part {
  qty: Int
}
function entrypoint() -> Nothing {
  let pair: Fixed<Part> = [{qty: 1}, {qty: 2}]
  let copied: Fixed<Part> = pair.copy()
  pair.set(0, {qty: 5})
  pair[1] = {qty: 7}
  let boxes: Fixed<Maybe<Int> > = [none, none]
  print(pair.count() + copied.count() + boxes.count())
}
";

/// Bug (High: leak): `heap_class_of` called a `fixed` of a shape that owns no heap `NoHeap`, so the
/// binding was never released, yet every item sits in a heap cell of its own
/// (`value_to_stable_bits`): `let pair: fixed<Part> = [{qty: 1}, {qty: 2}]` was 2 allocs / 0 frees.
#[test]
fn a_fixed_list_of_heap_free_shapes_releases_its_item_cells() {
    assert_heap_balanced_and_clean("c7r1_fixed_flat_shapes", FIXED_OF_FLAT_SHAPES, "6\n");
}

/// [`assert_balanced_and_clean`] for a program that allocates strings only.
fn assert_strings_balanced_and_clean(label: &str, program: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        assert_strings_balanced(label, &counters, no_optimize);
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

// ── Group 3 (string character reads): `get` and `graphemeAt` build a fresh string ───────────

const STRING_CHARACTER_READS: &str = "
function entrypoint() -> Nothing {
  let s: String = `hello`
  let first = s.get(0)
  let grapheme = s.graphemeAt(1)
  let outside = s.get(100)
  let total: Int = first.or(`z`).count() + grapheme.or(`z`).count() + outside.or(`z`).count()
  total = total + s.get(2).or(`z`).count() + s.graphemeAt(3).or(`z`).count()
  let i: Int = 0
  while (i < 3) {
    let each = s.get(i)
    total = total + each.or(`z`).count()
    i = i + 1
  }
  print(total)
}
";

/// Bug (High: leak): `ynz_string_codepoint_at` / `ynz_string_grapheme_at` build a new string, which
/// `let c = s.get(0)` then cloned again (the binding copies a `maybe<string>` it does not take) and
/// a `maybe` nothing binds never freed (`let s = ...; let c = s.get(0)`: strings 3/2).
#[test]
fn a_string_character_read_frees_the_string_it_built() {
    assert_strings_balanced_and_clean("c7r1_string_get", STRING_CHARACTER_READS, "8\n");
}

// ── Group 4: a shape with a `maybe` field owns the field's envelope cell ────────────────────

const SHAPE_WITH_A_MAYBE_FIELD: &str = "
shape Part {
  qty: Int
}
shape Holder {
  found: Maybe<Part>
}
shape Slot {
  count: Maybe<Int>
}
function entrypoint() -> Nothing {
  let parts: Array<Part> = [{qty: 1}, {qty: 2}]
  let current: Maybe<Part> = parts.get(0)
  let missing: Maybe<Part> = parts.get(9)
  let holder: Holder = { found: missing.copy() }
  holder.found = current.copy()
  let rows: Array<Holder> = [{ found: current.copy() }, { found: current.copy() }]
  let slot: Slot = { count: none }
  print(rows.count() + holder.found.or({qty: 0}).qty)
}
";

/// Bug (High: leak): `shape_class` classed a shape whose only heap is a `maybe` field of a
/// heap-free payload `NoHeap`, but a `maybe` field is a pointer to a heap envelope cell
/// (`store_field`), so no release was planned for the shape: `let slot: Slot = { count: none }`
/// was 2 allocs / 0 frees.
#[test]
fn a_shape_with_a_maybe_field_releases_the_fields_envelope_cell() {
    assert_heap_balanced_and_clean("c7r1_maybe_field", SHAPE_WITH_A_MAYBE_FIELD, "3\n");
}

// ── Group 4: a `maybe` returned, persisted or given has one owner of its payload ────────────

const MAYBE_RETURNED: &str = "
shape Tag {
  name: String
}
function firstTag(share list: Array<Tag>) -> Maybe<Tag> {
  let found = list.get(0)
  return found
}
function ownedTag(share list: Array<Tag>) -> Maybe<Tag> {
  let found: Maybe<Tag> = list.get(0)
  let mine: Maybe<Tag> = found.copy()
  return mine
}
function passThrough(share found: Maybe<Tag>) -> Maybe<Tag> {
  return found
}
function firstWord(share list: Array<String>) -> Maybe<String> {
  let found = list.get(0)
  return found
}
function entrypoint() -> Nothing {
  let tags: Array<Tag> = [{name: `n`}]
  let words: Array<String> = [`aa`, `b`]
  let first = firstTag(tags)
  let owned = ownedTag(tags)
  let through = passThrough(first)
  let word = firstWord(words)
  let again: Maybe<Tag> = firstTag(tags)
  again = ownedTag(tags)
  print(first.or({name: `z`}).name.count() + owned.or({name: `z`}).name.count()
    + through.or({name: `z`}).name.count() + word.or(`z`).count() + again.or({name: `z`}).name.count())
}
";

/// Bug (High: leak): a function that returned a `maybe` copied its payload at the `return`, yet
/// typeck called the returned local "returned" and planned no release for it (`copied_at_return`
/// said a `maybe` is not copied), so the local's own copy of the shape's string leaked (6/5), and a
/// returned owned local was copied twice.
#[test]
fn a_maybe_a_function_returns_is_copied_once_and_its_local_released() {
    assert_balanced_and_clean("c7r1_maybe_returned", MAYBE_RETURNED, "6\n");
}

const MAYBE_PERSISTED_AND_GIVEN: &str = "
shape Tag {
  name: String
}
function firstTag(share list: Array<Tag>) -> Maybe<Tag> {
  let found = list.get(0)
  return found
}
function findFirst(share text: String) -> Maybe<String> {
  return text.get(0)
}
function eatTag(give found: Maybe<Tag>) -> Int {
  return found.or({name: `z`}).name.count()
}
function entrypoint() -> Nothing {
  let tags: Array<Tag> = [{name: `n`}]
  let text: String = `hello`
  let boxed: Array<Maybe<Tag> > = [firstTag(tags)]
  boxed.add(firstTag(tags))
  let words: Array<Maybe<String> > = [findFirst(text)]
  words.add(text.get(1))
  let current: Maybe<Tag> = tags.get(0)
  let total: Int = eatTag(current.copy()) + eatTag(firstTag(tags))
  print(boxed.count() + words.count() + total)
}
";

/// Bug (High: leak): a `maybe` a function returned (or a string `get` built, or a `.copy()` of a
/// `maybe`) owns its payload, but a persist into an array literal or `add` cloned it again, and a
/// `give` parameter was handed the payload's heap cell its release does not free.
#[test]
fn a_fresh_maybe_stored_or_given_hands_its_payload_over_once() {
    assert_balanced_and_clean(
        "c7r1_maybe_persisted_given",
        MAYBE_PERSISTED_AND_GIVEN,
        "6\n",
    );
}
