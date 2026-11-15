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
