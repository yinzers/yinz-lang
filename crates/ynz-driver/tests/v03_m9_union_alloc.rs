//! v0.3-M9 C1f (#423): a union that is not recursive is stored by value, so moving one through a
//! store, a return, an `errors` result, a `wait`-staged argument or a hidden default allocates
//! NOTHING. Only a recursive union boxes its shape variant, one heap cell per store (the cost of
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
fn assert_allocates_like_int(fixture: &str, expected: &str, baseline: &str, baseline_expected: &str) {
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

// A recursive union cannot hold its payload inline: one heap cell per stored shape variant.
#[test]
fn recursive_union_store_allocates_one_cell_per_store() {
    assert_eq!(
        heap_allocations("v0_3_m9_union_alloc_recursive.ynz", "100\n"),
        300,
        "100 builds x 3 stores must make 300 cells and nothing else"
    );
}
