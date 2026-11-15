//! v0.3-M9 C1f (#423): a union that is not recursive is stored by value, so moving one through a
//! store, a return, an `errors` result, a `wait`-staged argument or a hidden default allocates
//! NOTHING. Only a recursive union boxes its shape variant, one heap cell per box a stored value owns (the cost of
//! `Box<T>` in a Rust enum).
//!
//! Tier: High (data integrity: the layout is the contract C6's release helper reads; an
//! allocation hiding in a hot store is a Golden Rule 8 regression). Each fixture runs its
//! operation in a loop, so a per-operation allocation shows as a count that scales with the loop.

mod support;

use support::{build, run_counted};

/// Build `fixture` (optimized), run it with the alloc counter on, assert it ran clean and printed
/// `expected_stdout`, and return the number of `ynz_alloc` calls it made.
fn heap_allocations(fixture: &str, expected_stdout: &str) -> u64 {
    let built = build(fixture, false);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "{fixture}: stderr:\n{}", output.stderr);
    assert_eq!(output.stdout, expected_stdout, "{fixture}: wrong output");
    counters.alloc
}

// Bug caught: an `errors` result whose success word cannot hold a slot re-homes the union in two
// heap cells per call (300 calls = 600 allocations before the by-value result).
#[test]
fn errors_return_allocates_nothing() {
    assert_eq!(
        heap_allocations("v0_3_m9_union_alloc_errors.ynz", "44850\n"),
        0,
        "an `errors` return of a union must not allocate"
    );
}

/// `fixture` and its `baseline` (the same program over `int`) must allocate the same number of
/// times: the union adds nothing. Both must print what they print and run clean.
fn assert_allocates_like_int(
    fixture: &str,
    expected: &str,
    baseline: &str,
    baseline_expected: &str,
) {
    let union_allocations = heap_allocations(fixture, expected);
    let int_allocations = heap_allocations(baseline, baseline_expected);
    assert_eq!(
        union_allocations, int_allocations,
        "{fixture} allocated {union_allocations} times, its int baseline {baseline} {int_allocations}"
    );
}

// Bug caught: a returned union re-homed in heap cells (#410's `Return` home, two cells per call).
#[test]
fn union_return_allocates_nothing() {
    assert_eq!(
        heap_allocations("v0_3_m9_union_alloc_return.ynz", "22950\n"),
        0,
        "a returned union must not allocate"
    );
}

// Bug caught: a union field was a pointer to an envelope plus a payload cell, and a hidden default
// into one (#421) took two counted cells per construction.
#[test]
fn union_shape_field_and_hidden_default_allocate_nothing() {
    assert_eq!(
        heap_allocations("v0_3_m9_union_alloc_field.ynz", "23250\n"),
        0,
        "a shape with a union field, and its hidden default, must not allocate"
    );
}

// Bug caught: a `wait`-staged union argument took two counted cells per call (#410's heap home).
#[test]
fn union_wait_arg_allocates_like_int() {
    assert_allocates_like_int(
        "v0_3_m9_union_alloc_wait_arg.ynz",
        "5150\n",
        "v0_3_m9_union_alloc_wait_arg_int.ynz",
        "5000\n",
    );
}

// Bug caught: a union binding crossing a `wait` was two counted cells (`union_to_heap_cell`).
#[test]
fn union_crossing_binding_allocates_like_int() {
    assert_allocates_like_int(
        "v0_3_m9_union_alloc_crossing.ynz",
        "33\n",
        "v0_3_m9_union_alloc_crossing_int.ynz",
        "33\n",
    );
}

// Bug caught: an array element or map value was an envelope pointer plus a payload cell.
#[test]
fn union_container_store_allocates_like_int() {
    assert_allocates_like_int(
        "v0_3_m9_union_alloc_container.ynz",
        "22950\n",
        "v0_3_m9_union_alloc_container_int.ynz",
        "44850\n",
    );
}

// A recursive union cannot hold its payload inline: one heap cell per box. Each build stores an
// `End`, a `Node` and a `Node` into `next` fields, and each store MOVES its value (the language
// refuses a read of the source afterwards), as `Box::new(prev)` does in Rust: one cell per box
// created, no clone. 100 builds x 3 cells.
// Bug caught: every store deep-cloned the chain it was given (1 + 2 + 3 cells per build, 600),
// above Rust's cost for a move (Golden Rule 8).
#[test]
fn recursive_union_store_allocates_one_cell_per_box() {
    assert_eq!(
        heap_allocations("v0_3_m9_union_alloc_recursive.ynz", "100\n"),
        300,
        "100 builds x 3 moved boxes must make 300 cells and nothing else"
    );
}

// Bug caught: storing a recursive union copied the variant's bytes into ONE new cell, so the nested
// cells of the original and the copy were the same cells and a recursive release (C6) would free
// them twice. An explicit `.copy()` of the four-cell chain makes four cells per copy (the three
// boxes the chain owns, then the binding's box takes the copy by move): 100 copies, 400 more than
// the chain built alone.
#[test]
fn recursive_union_copy_clones_every_cell() {
    let chain_alone = heap_allocations("v0_3_m9_union_alloc_recursive_chain.ynz", "100\n");
    let with_copies = heap_allocations("v0_3_m9_union_alloc_recursive_copy.ynz", "300\n");
    assert_eq!(
        with_copies - chain_alone,
        400,
        "100 copies of a four-cell chain must make 400 cells; chain alone {chain_alone}, with copies {with_copies}"
    );
}

// Bug caught: a store of a chain nobody reads again deep-cloned it (four cells) where Rust moves
// the `Box`: the move needs only the binding's own cell, one per store.
#[test]
fn recursive_union_move_clones_nothing() {
    let chain_alone = heap_allocations("v0_3_m9_union_alloc_recursive_chain.ynz", "100\n");
    let with_moves = heap_allocations("v0_3_m9_union_alloc_recursive_move.ynz", "300\n");
    assert_eq!(
        with_moves - chain_alone,
        100,
        "100 moves of a chain must make one cell each; chain alone {chain_alone}, with moves {with_moves}"
    );
}

/// How many more cells `fixture` allocates than `baseline`, both run clean and printing what they
/// print.
fn extra_cells(baseline: (&str, &str), fixture: (&str, &str)) -> u64 {
    let baseline_cells = heap_allocations(baseline.0, baseline.1);
    let fixture_cells = heap_allocations(fixture.0, fixture.1);
    fixture_cells - baseline_cells
}

// Shape byte copies of a shape that owns a recursive union (`shape_bytes_to_owned`,
// `shape_bytes_to_heap_cell`, `maybe_payload_stable_bits`, `.copy()`, an array store). A holder
// built with makeHolder owns four cells. Bug caught by each: a copy that shared its source's
// boxes (a release of both frees them twice), and, after the deep-copy fix, a MOVE that still
// cloned them (above Rust's cost).

#[test]
fn shape_move_into_a_binding_clones_nothing() {
    assert_eq!(
        heap_allocations("v0_3_m9_union_alloc_shape_moved_binding.ynz", "100\n"),
        400,
        "100 holders x 4 cells, moved into a binding with no clone"
    );
}

#[test]
fn shape_explicit_copy_is_deep_even_when_the_source_is_not_read_again() {
    let extra = extra_cells(
        ("v0_3_m9_union_alloc_shape_moved_binding.ynz", "100\n"),
        ("v0_3_m9_union_alloc_shape_explicit_copy.ynz", "200\n"),
    );
    assert_eq!(
        extra, 400,
        "`.copy()` of a holder clones its four cells, once"
    );
}

#[test]
fn shape_field_store_moves_a_binding_and_copies_a_copy_once() {
    let moved = extra_cells(
        ("v0_3_m9_union_alloc_shape_moved_binding.ynz", "100\n"),
        ("v0_3_m9_union_alloc_shape_moved_field.ynz", "200\n"),
    );
    assert_eq!(
        moved, 100,
        "a moved holder costs the field's own cell, nothing else"
    );
    let copied = extra_cells(
        ("v0_3_m9_union_alloc_shape_moved_field.ynz", "200\n"),
        ("v0_3_m9_union_alloc_shape_copied_field.ynz", "300\n"),
    );
    assert_eq!(
        copied, 400,
        "a copy stored in a field is cloned once (four cells), not again by the store"
    );
}

#[test]
fn shape_alias_binding_owns_a_deep_copy() {
    let extra = extra_cells(
        ("v0_3_m9_union_alloc_shape_moved_field.ynz", "200\n"),
        ("v0_3_m9_union_alloc_shape_alias_binding.ynz", "300\n"),
    );
    assert_eq!(
        extra, 400,
        "`let copied = outer.inner` leaves the field in place, so the binding clones four cells"
    );
}

#[test]
fn shape_array_store_moves_a_binding_and_copies_a_copy_once() {
    let extra = extra_cells(
        ("v0_3_m9_union_alloc_shape_moved_array.ynz", "100\n"),
        ("v0_3_m9_union_alloc_shape_copied_array.ynz", "200\n"),
    );
    assert_eq!(
        extra, 400,
        "an array store of a copy clones four cells, once"
    );
}

#[test]
fn shape_returned_parameter_is_copied_once_by_the_binding() {
    let extra = extra_cells(
        ("v0_3_m9_union_alloc_shape_moved_binding.ynz", "100\n"),
        ("v0_3_m9_union_alloc_shape_returned_parameter.ynz", "200\n"),
    );
    assert_eq!(
        extra, 400,
        "the alias binding of a returned parameter clones four cells; the return clones none"
    );
}

#[test]
fn maybe_payload_binding_clones_the_boxes_it_reads() {
    let extra = extra_cells(
        ("v0_3_m9_union_alloc_shape_maybe_read_once.ynz", "1\n"),
        ("v0_3_m9_union_alloc_shape_maybe_read.ynz", "100\n"),
    );
    assert_eq!(
        extra,
        99 * 4,
        "99 more reads of a `maybe<Holder>` element, four cells each"
    );
}
