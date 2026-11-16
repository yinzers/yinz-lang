//! v0.3-M9 C6 (parked entry 70): a `number` stored in an `array` or a `map` lives IN the
//! container's cell, 16 bytes of decimal128, as it does in a Rust `Vec<Decimal>`.
//!
//! Before, the cell held 8 bytes of POINTER bits into the frame that produced the number: an
//! `array<number>` returned from its producer, read in a spawned task after the spawner moved on,
//! or filled in a loop body read garbage (the loop variable's storage is reused every turn).
//! Each program below builds the container in one frame, clobbers the stack, and reads it in
//! another; every one is compared byte for byte at -O0 and -O2 and run under AddressSanitizer
//! with freed memory filled with zero.
//! High tier: data integrity (a container reading a dead frame).

mod support;

use support::{build_asan, build_source, run_asan_binary, run_counted};

/// A function that overwrites the stack the producing functions just left.
const CLOBBER: &str = "function clobber(seed: int) -> int {\n  let plusOne = seed + 1\n  \
                       let doubled = plusOne * 2\n  let tripled = doubled * 3\n  \
                       return tripled + doubled + plusOne\n}\n\n";

/// The same program must print `expected` at -O0 and -O2, plainly and under ASan.
fn assert_prints(source: &str, expected: &str) {
    for no_optimize in [false, true] {
        let built = build_source(source, "c6 number cells", no_optimize);
        let (output, _) = run_counted(&built);
        assert_eq!(output.exit_code, 0, "{source}\nstderr:\n{}", output.stderr);
        assert_eq!(
            output.stdout, expected,
            "no_optimize={no_optimize}\n{source}"
        );
        let run = run_asan_binary(&build_asan(&built));
        assert!(
            !run.stderr.contains("AddressSanitizer"),
            "AddressSanitizer reported an error (no_optimize={no_optimize}):\n{}\n{source}",
            run.stderr
        );
        assert_eq!(run.stdout, expected, "under ASan, no_optimize={no_optimize}");
    }
}

fn program(functions: &str, entrypoint_body: &str) -> String {
    format!("{functions}\n{CLOBBER}function entrypoint() -> nothing {{\n{entrypoint_body}}}\n")
}

#[test]
fn array_of_numbers_returned_from_its_producer() {
    // Bug caught: cells pointing into the dead producer frame (garbage at -O0). Tier: High.
    let source = program(
        "function make(amount: number) -> array<number> {\n  \
         let doubled: number = amount * 2.0\n  let values: array<number> = [amount, doubled]\n  \
         return values\n}\n",
        "  let seed: number = 1.5\n  let amount: number = seed + 1.0\n  let values = make(amount)\n  \
         let scratch = clobber(5)\n  let fallback: number = 0.0\n  \
         print(`${values[0].or(fallback)} ${values[1].or(fallback)} ${scratch}`)\n",
    );
    assert_prints(&source, "2.5 5.00 54\n");
}

#[test]
fn array_of_numbers_filled_in_a_loop_body() {
    // Bug caught: every cell pointing at the loop body's one reused number slot, so all cells
    // read the last value and `contains` found nothing. Tier: High.
    let source = program(
        "",
        "  let seed: number = 1.5\n  let values: array<number> = []\n  \
         for (round in range(0, 5)) {\n    let step: number = seed * 2.0\n    seed = seed + 1.0\n    \
         values.add(step)\n  }\n  let total: number = 0.0\n  for (item in values) {\n    \
         total = total + item\n  }\n  let target: number = 5.0\n  \
         print(`${total} ${values.count()} ${values.contains(target)}`)\n",
    );
    assert_prints(&source, "35.00 5 true\n");
}

#[test]
fn copy_of_an_array_of_numbers_is_independent_of_a_later_set() {
    // Bug caught: a copy and its source reading one cell, or `set` rewriting a cell a read of
    // it still views. Tier: High.
    let source = program(
        "",
        "  let seed: number = 1.5\n  let doubled: number = seed * 2.0\n  \
         let values: array<number> = [seed, doubled]\n  let copy: array<number> = values.copy()\n  \
         let fallback: number = 0.0\n  let before: number = values[0].or(fallback)\n  \
         let replacement: number = 100.0\n  values.set(0, replacement)\n  \
         print(`${before} ${copy[0].or(fallback)} ${values[0].or(fallback)}`)\n",
    );
    assert_prints(&source, "1.5 1.5 100.0\n");
}

#[test]
fn map_of_numbers_returned_from_its_producer() {
    // Bug caught: map value cells pointing into the dead producer frame. Tier: High.
    let source = program(
        "function make(amount: number) -> map<int, number> {\n  \
         let doubled: number = amount * 2.0\n  let table: map<int, number> = {}\n  \
         table.set(1, amount)\n  table.set(2, doubled)\n  return table\n}\n",
        "  let seed: number = 1.5\n  let amount: number = seed + 1.0\n  let table = make(amount)\n  \
         let scratch = clobber(5)\n  let fallback: number = 0.0\n  \
         print(`${table.get(1).or(fallback)} ${table.get(2).or(fallback)} ${scratch}`)\n",
    );
    assert_prints(&source, "2.5 5.00 54\n");
}

#[test]
fn array_of_arrays_of_numbers_returned_from_its_producer() {
    // Bug caught: the inner array's cells dangling although the outer array is by value.
    // Tier: High.
    let source = program(
        "function make(amount: number) -> array<array<number>> {\n  \
         let doubled: number = amount * 2.0\n  let inner: array<number> = [amount, doubled]\n  \
         let outer: array<array<number>> = [inner]\n  return outer\n}\n",
        "  let seed: number = 1.5\n  let amount: number = seed + 1.0\n  let outer = make(amount)\n  \
         let scratch = clobber(5)\n  let fallback: number = 0.0\n  let row = outer[0].or([])\n  \
         print(`${row[1].or(fallback)} ${scratch}`)\n",
    );
    assert_prints(&source, "5.00 54\n");
}

#[test]
fn maybe_number_read_from_an_element_survives_its_array_and_the_next_read() {
    // Bug caught: a `maybe<number>` bound from an element read keeping a pointer into the
    // per-site read buffer, which the next read at the same site overwrites. Tier: High.
    let source = program(
        "function pick(share values: array<number>, position: int) -> maybe<number> {\n  \
         return values.get(position)\n}\n\
         function make(amount: number) -> array<number> {\n  \
         let doubled: number = amount * 2.0\n  let values: array<number> = [amount, doubled]\n  \
         return values\n}\n",
        "  let seed: number = 1.5\n  let amount: number = seed + 1.0\n  let values = make(amount)\n  \
         let held: array<maybe<number>> = []\n  \
         let first = pick(values, 0)\n  let second = pick(values, 1)\n  let scratch = clobber(5)\n  \
         let fallback: number = 0.0\n  \
         print(`${first.or(fallback)} ${second.or(fallback)} ${scratch} ${held.count()}`)\n",
    );
    assert_prints(&source, "2.5 5.00 54 0\n");
}
