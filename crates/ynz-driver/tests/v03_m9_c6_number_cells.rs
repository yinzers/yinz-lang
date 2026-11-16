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
        assert_eq!(
            run.stdout, expected,
            "under ASan, no_optimize={no_optimize}"
        );
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
         let first = pick(values, 0)\n  let second = pick(values, 1)\n  \
         let latest: maybe<number> = values.get(0)\n  for (position in range(0, 2)) {\n    \
         let found = values.get(position)\n    latest = found\n  }\n  let scratch = clobber(5)\n  \
         let fallback: number = 0.0\n  \
         print(`${first.or(fallback)} ${second.or(fallback)} ${scratch} ${latest.or(fallback)}`)\n",
    );
    assert_prints(&source, "2.5 5.00 54 5.00\n");
}

#[test]
fn fixed_of_numbers_holds_its_own_decimals() {
    // Bug caught: `fixed<number>` cells holding the address of the number they were built from,
    // so a later write to that binding changed the list, and a `set` of a loop body's slot read
    // garbage at -O0. Tier: High.
    let source = program(
        "",
        "  let seed: number = 1.5\n  let cells: fixed<number> = [seed, seed * 2.0]\n  seed = 9.0\n  \
         let fallback: number = 0.0\n  print(`${cells[0].or(fallback)}`)\n  \
         for (round in range(0, 2)) {\n    let step: number = seed * 2.0\n    cells.set(round, step)\n    \
         seed = seed + 1.0\n  }\n  let total: number = 0.0\n  for (cell in cells) {\n    \
         total = total + cell\n  }\n  \
         print(`${cells[0].or(fallback)} ${cells[1].or(fallback)} ${total}`)\n",
    );
    assert_prints(&source, "1.5\n18.00 20.00 38.00\n");
}

#[test]
fn channel_of_numbers_delivers_each_sent_value() {
    // Bug caught: a sent number still pointing at the producer's loop-body slot, so every
    // receive read the last value. Tier: High.
    let source = program(
        "function produce(line: channel<number>) -> nothing {\n  let seed: number = 1.5\n  \
         for (round in range(0, 3)) {\n    let step: number = seed * 2.0\n    seed = seed + 1.0\n    \
         line.send(step)\n  }\n}\n",
        "  let line: channel<number> = channel<number>(4)\n  produce(line)\n  \
         let fallback: number = 0.0\n  for (round in range(0, 3)) {\n    let got = line.receive()\n    \
         print(got.or(fallback))\n  }\n",
    );
    assert_prints(&source, "3.00\n5.00\n7.00\n");
}

#[test]
fn map_entry_values_copied_out_of_a_loop_survive_the_loop() {
    // Bug caught: a map loop's per-turn value buffer escaping into an array of numbers.
    // Tier: High.
    let source = program(
        "function make(seed: number) -> map<string, number> {\n  \
         let doubled: number = seed * 2.0\n  let table: map<string, number> = {}\n  \
         table.set(`a`, seed)\n  table.set(`b`, doubled)\n  return table\n}\n",
        "  let seed: number = 1.5\n  let table = make(seed)\n  let kept: array<number> = []\n  \
         for (entry in table) {\n    kept.add(entry.value)\n  }\n  let scratch = clobber(5)\n  \
         let fallback: number = 0.0\n  print(`${kept[0].or(fallback)} ${kept[1].or(fallback)} ${scratch}`)\n",
    );
    assert_prints(&source, "1.5 3.00 54\n");
}

/// A function spawns three tasks with an `array<number>` it still reads, then returns; the
/// caller clobbers the stack before the tasks (which first wait or spin) read their copies.
fn spawned_numbers_program(suspending_callee: bool) -> String {
    let wait_for_the_spawner = if suspending_callee {
        "  wait sleep(100)\n"
    } else {
        "  let spins = 0\n  let mixed = 1\n  while (spins < 60000000) {\n    \
         mixed = (mixed * 31 + spins) % 1000003\n    spins = spins + 1\n  }\n  \
         if (mixed < 0) {\n    print(mixed)\n  }\n"
    };
    format!(
        "function show(values: array<number>) -> nothing {{\n{wait_for_the_spawner}  \
         let fallback: number = 0.0\n  let first: number = values[0].or(fallback)\n  \
         let second: number = values[1].or(fallback)\n  print(`${{first}} ${{second}}`)\n}}\n\n\
         function launch(amount: number) -> nothing {{\n  let doubled: number = amount * 2.0\n  \
         let values: array<number> = [amount, doubled]\n  \
         for (round in range(0, 3)) {{\n    background show(values)\n  }}\n  \
         print(values.count())\n}}\n\n{CLOBBER}\
         function entrypoint() -> nothing {{\n  let seed: number = 1.5\n  \
         let amount: number = seed + 1.0\n  launch(amount)\n  let scratch = clobber(5)\n  \
         print(scratch)\n  wait sleep(1500)\n}}\n"
    )
}

fn assert_spawned_tasks_read_their_numbers(suspending_callee: bool) {
    let source = spawned_numbers_program(suspending_callee);
    for no_optimize in [false, true] {
        let built = build_source(&source, "c6 number spawn", no_optimize);
        let (output, _) = run_counted(&built);
        assert_eq!(output.exit_code, 0, "{source}\nstderr:\n{}", output.stderr);
        assert_eq!(
            output.stdout.matches("2.5 5.00\n").count(),
            3,
            "{}",
            output.stdout
        );
        assert!(
            output.stdout.contains("2\n") && output.stdout.contains("54\n"),
            "{}",
            output.stdout
        );
        let run = run_asan_binary(&build_asan(&built));
        assert!(
            !run.stderr.contains("AddressSanitizer"),
            "AddressSanitizer reported an error:\n{}",
            run.stderr
        );
        assert_eq!(
            run.stdout.matches("2.5 5.00\n").count(),
            3,
            "{}",
            run.stdout
        );
    }
}

#[test]
fn spawned_cpu_tasks_read_the_numbers_of_a_finished_spawner() {
    // Bug caught: the task's copy of an `array<number>` still pointing into the spawner's
    // dead frame (parked entry 70's original shape). Tier: High.
    assert_spawned_tasks_read_their_numbers(false);
}

#[test]
fn spawned_state_machine_tasks_read_the_numbers_of_a_finished_spawner() {
    // Bug caught: as above through the state-machine arm. Tier: High.
    assert_spawned_tasks_read_their_numbers(true);
}

#[test]
fn spawns_of_an_array_of_numbers_in_a_loop_balance() {
    // Bug caught: a spawn copy of an `array<number>` that leaks, or frees cells it shares with
    // the spawner. Three more spawns must add as many frees as allocations. Tier: High.
    let counts = |tasks: usize| {
        let source = format!(
            "function show(values: array<number>) -> nothing {{\n  print(777)\n}}\n\n\
             function launch(amount: number) -> nothing {{\n  let doubled: number = amount * 2.0\n  \
             let values: array<number> = [amount, doubled]\n  \
             for (round in range(0, {tasks})) {{\n    background show(values)\n  }}\n  \
             print(values.count())\n}}\n\nfunction entrypoint() -> nothing {{\n  \
             let seed: number = 1.5\n  let amount: number = seed + 1.0\n  launch(amount)\n  \
             wait sleep(300)\n}}\n"
        );
        let built = build_source(&source, "c6 number spawn counts", false);
        let (output, counters) = run_counted(&built);
        assert_eq!(output.exit_code, 0, "{}", output.stderr);
        assert_eq!(
            output.stdout.matches("777\n").count(),
            tasks,
            "{}",
            output.stdout
        );
        (counters.alloc, counters.free)
    };
    let (alloc_one, free_one) = counts(1);
    let (alloc_four, free_four) = counts(4);
    assert!(alloc_four > alloc_one, "three more spawns copied nothing");
    assert_eq!(alloc_four - alloc_one, free_four - free_one);
}

#[test]
fn storing_numbers_allocates_nothing_per_element() {
    // Bug caught: a heap cell per stored number (the design the cost check rejected: more than
    // Rust's inline `Vec<Decimal>`). Only the array's header and buffer growth allocate, so
    // 1000 numbers cost a few more allocations than 10, not 990. Tier: Medium.
    let allocations = |count: usize| {
        let source = program(
            "",
            &format!(
                "  let values: array<number> = []\n  let step: number = 1.5\n  \
                 for (round in range(0, {count})) {{\n    values.add(step)\n  }}\n  \
                 print(values.count())\n"
            ),
        );
        let built = build_source(&source, "c6 number allocation count", false);
        let (output, counters) = run_counted(&built);
        assert_eq!(output.stdout, format!("{count}\n"));
        counters.alloc
    };
    let few = allocations(10);
    let many = allocations(1000);
    assert!(
        many - few <= 8,
        "1000 numbers allocated {many} cells against {few} for 10: a cell per element"
    );
}

#[test]
fn union_variant_holding_a_number_survives_container_return_and_spawn() {
    // Bug caught: a number reaching a union through a shape variant (a bare `number` variant is
    // refused: "can't be stored in a union yet") pointing into the building frame, in an array
    // of unions, as a returned union, or as a spawned task's argument. Tier: High.
    let source = program(
        "shape Circle {\n  radius: float\n}\nshape Boxed {\n  value: number\n}\n\
         shape Thing = Circle | Boxed\n\n\
         function describe(share held: Thing) -> number {\n  let fallback: number = 0.0\n  \
         if (held) {\n    is Boxed => {\n      return held.value\n    }\n    is Circle => {\n      \
         return fallback\n    }\n  }\n  return fallback\n}\n\n\
         function make(seed: number) -> array<Thing> {\n  let doubled: number = seed * 2.0\n  \
         let first: Boxed = { value: seed }\n  let second: Boxed = { value: doubled }\n  \
         let items: array<Thing> = []\n  let one: Thing = first\n  let two: Thing = second\n  \
         items.add(one)\n  items.add(two)\n  return items\n}\n\n\
         function produce(seed: number) -> Thing {\n  let doubled: number = seed * 2.0\n  \
         let boxed: Boxed = { value: doubled }\n  let wrapped: Thing = boxed\n  return wrapped\n}\n\n\
         function slow(held: Thing) -> nothing {\n  wait sleep(80)\n  print(`task ${describe(held)}`)\n}\n\n\
         function launch(seed: number) -> nothing {\n  let doubled: number = seed * 2.0\n  \
         let boxed: Boxed = { value: doubled }\n  let wrapped: Thing = boxed\n  \
         background slow(wrapped)\n}\n",
        "  let seed: number = 1.5\n  let items = make(seed)\n  let single = produce(seed)\n  \
         launch(seed)\n  let scratch = clobber(5)\n  let spare: Boxed = { value: 0.0 }\n  \
         let fallback: Thing = spare\n  let one = items[1].or(fallback)\n  \
         print(`${describe(one)} ${describe(single)} ${scratch}`)\n  wait sleep(400)\n",
    );
    for no_optimize in [false, true] {
        let built = build_source(&source, "c6 union number", no_optimize);
        let (output, _) = run_counted(&built);
        assert_eq!(output.exit_code, 0, "{source}\nstderr:\n{}", output.stderr);
        assert!(
            output.stdout.contains("3.00 3.00 54\n"),
            "{}",
            output.stdout
        );
        assert!(output.stdout.contains("task 3.00\n"), "{}", output.stdout);
        let run = run_asan_binary(&build_asan(&built));
        assert!(
            !run.stderr.contains("AddressSanitizer"),
            "AddressSanitizer reported an error:\n{}",
            run.stderr
        );
        assert!(run.stdout.contains("3.00 3.00 54\n") && run.stdout.contains("task 3.00\n"));
    }
}
