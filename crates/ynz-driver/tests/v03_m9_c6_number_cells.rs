//! v0.3-M9 C6 (parked entry 70): a `number` stored in an `array` or a `map` lives IN the
//! container's cell, 16 bytes of decimal128, as it does in a Rust `Vec<Decimal>`.
//!
//! A cell that held a pointer into the frame that produced the number would read garbage once that
//! frame returned or its loop body reused the slot. Each program below builds the container in one
//! frame, clobbers the stack, and reads it in another; every one is compared byte for byte at -O0 and -O2 and run under AddressSanitizer
//! with freed memory filled with zero.
//! High tier: data integrity (a container reading a dead frame).

mod support;

use support::{
    at_both_optimization_levels, build_asan, build_source, concurrently, run_asan_binary,
    run_asan_binary_until_tasks_finish, run_counted, run_counted_until_tasks_finish,
};

/// A function that overwrites the stack the producing functions just left.
const CLOBBER: &str = "function clobber(seed: Int) -> Int {\n  let plusOne = seed + 1\n  \
                       let doubled = plusOne * 2\n  let tripled = doubled * 3\n  \
                       return tripled + doubled + plusOne\n}\n\n";

/// The same program must print `expected` at -O0 and -O2, plainly and under ASan.
fn assert_prints(source: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
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
    });
}

fn program(functions: &str, entrypoint_body: &str) -> String {
    format!("{functions}\n{CLOBBER}function entrypoint() -> nothing {{\n{entrypoint_body}}}\n")
}

#[test]
fn array_of_numbers_returned_from_its_producer() {
    // Bug caught: cells pointing into the dead producer frame (garbage at -O0). Tier: High.
    let source = program(
        "function make(amount: Number) -> Array<Number> {\n  \
         let doubled: Number = amount * 2.0\n  let values: Array<Number> = [amount, doubled]\n  \
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
        "function make(amount: Number) -> Map<Int, Number> {\n  \
         let doubled: Number = amount * 2.0\n  let table: Map<Int, Number> = {}\n  \
         table.set(1, amount)\n  table.set(2, doubled)\n  return table\n}\n",
        "  let seed: number = 1.5\n  let amount: number = seed + 1.0\n  let table = make(amount)\n  \
         let scratch = clobber(5)\n  let fallback: number = 0.0\n  \
         print(`${table[1].or(fallback)} ${table[2].or(fallback)} ${scratch}`)\n",
    );
    assert_prints(&source, "2.5 5.00 54\n");
}

#[test]
fn array_of_arrays_of_numbers_returned_from_its_producer() {
    // Bug caught: the inner array's cells dangling although the outer array is by value.
    // Tier: High.
    let source = program(
        "function make(amount: Number) -> Array<Array<Number>> {\n  \
         let doubled: Number = amount * 2.0\n  let inner: Array<Number> = [amount, doubled]\n  \
         let outer: Array<Array<Number>> = [inner]\n  return outer\n}\n",
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
        "function pick(share values: Array<Number>, position: Int) -> Maybe<Number> {\n  \
         return values.get(position)\n}\n\
         function make(amount: Number) -> Array<Number> {\n  \
         let doubled: Number = amount * 2.0\n  let values: Array<Number> = [amount, doubled]\n  \
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
        "function produce(line: Channel<Number>) -> Nothing {\n  let seed: Number = 1.5\n  \
         for (round in range(0, 3)) {\n    let step: Number = seed * 2.0\n    seed = seed + 1.0\n    \
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
        "function make(seed: Number) -> Map<String, Number> {\n  \
         let doubled: Number = seed * 2.0\n  let table: Map<String, Number> = {}\n  \
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
    at_both_optimization_levels(|no_optimize| {
        let built = build_source(&source, "c6 number spawn", no_optimize);
        let (output, _) = run_counted_until_tasks_finish(&built);
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
        let run = run_asan_binary_until_tasks_finish(&build_asan(&built));
        assert!(
            !run.stderr.contains("AddressSanitizer"),
            "AddressSanitizer reported an error (no_optimize={no_optimize}):\n{}",
            run.stderr
        );
        assert_eq!(
            run.stdout.matches("2.5 5.00\n").count(),
            3,
            "{}",
            run.stdout
        );
    });
}

#[test]
fn spawned_cpu_tasks_read_the_numbers_of_a_finished_spawner() {
    // Bug caught: the task's copy of an `array<number>` still pointing into the spawner's
    // dead frame (the spawn case of a cell pointing into a dead frame). Tier: High.
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
    let ((alloc_one, free_one), (alloc_four, free_four)) = concurrently(|| counts(1), || counts(4));
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
    let (few, many) = concurrently(|| allocations(10), || allocations(1000));
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
        "shape Circle {\n  radius: Float\n}\nshape Boxed {\n  value: Number\n}\n\
         shape Thing = Circle | Boxed\n\n\
         function describe(share held: Thing) -> Number {\n  let fallback: Number = 0.0\n  \
         if (held) {\n    is Boxed => {\n      return held.value\n    }\n    is Circle => {\n      \
         return fallback\n    }\n  }\n  return fallback\n}\n\n\
         function make(seed: Number) -> Array<Thing> {\n  let doubled: Number = seed * 2.0\n  \
         let first: Boxed = { value: seed }\n  let second: Boxed = { value: doubled }\n  \
         let items: Array<Thing> = []\n  let one: Thing = first\n  let two: Thing = second\n  \
         items.add(one)\n  items.add(two)\n  return items\n}\n\n\
         function produce(seed: Number) -> Thing {\n  let doubled: Number = seed * 2.0\n  \
         let boxed: Boxed = { value: doubled }\n  let wrapped: Thing = boxed\n  return wrapped\n}\n\n\
         function slow(held: Thing) -> Nothing {\n  wait sleep(80)\n  print(`task ${describe(held)}`)\n}\n\n\
         function launch(seed: Number) -> Nothing {\n  let doubled: Number = seed * 2.0\n  \
         let boxed: Boxed = { value: doubled }\n  let wrapped: Thing = boxed\n  \
         background slow(wrapped)\n}\n",
        "  let seed: number = 1.5\n  let items = make(seed)\n  let single = produce(seed)\n  \
         launch(seed)\n  let scratch = clobber(5)\n  let spare: Boxed = { value: 0.0 }\n  \
         let fallback: Thing = spare\n  let one = items[1].or(fallback)\n  \
         print(`${describe(one)} ${describe(single)} ${scratch}`)\n  wait sleep(400)\n",
    );
    at_both_optimization_levels(|no_optimize| {
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
            "AddressSanitizer reported an error (no_optimize={no_optimize}):\n{}",
            run.stderr
        );
        assert!(run.stdout.contains("3.00 3.00 54\n") && run.stdout.contains("task 3.00\n"));
    });
}

#[test]
fn map_keyed_by_numbers_returned_from_its_producer() {
    // Bug caught: a `number` key held as 8 bytes of pointer bits into the producer's frame, so
    // after `keyed` returns `entry.key` reads the dead frame (garbage at -O0) and `get(2.5)`
    // compares addresses and always misses.
    // Tier: High.
    let source = program(
        "function keyed() -> Map<Number, String> {\n  let table: Map<Number, String> = {}\n  \
         let price: Number = 2.5\n  table.set(price, `two and a half`)\n  return table\n}\n",
        "  let table = keyed()\n  let scratch = clobber(3)\n  for (entry in table) {\n    \
         print(entry.key)\n  }\n  let probe: number = 2.5\n  \
         print(table[probe].or(`missing`))\n  print(scratch)\n",
    );
    assert_prints(&source, "2.5\ntwo and a half\n36\n");
}

// ── `number` map keys (parked entry 70, the keys half) ────────────────────────────────────────
//
// A key is held inline, 16 bytes, hashed and compared as those 16 bytes and nothing more. The same
// value in the same scale is one key; `1.0` and `1.00` are two (the number-key hashing semantics
// beyond storage are an open item, B4's "map<number, V> key hashing").

#[test]
fn number_key_is_found_by_an_identical_computed_key() {
    // Bug caught: lookup comparing the key's address: `1.0` computed from other numbers misses
    // the entry stored under the literal `1.0`. Covers `get`, `has` and the bracket read.
    // Tier: High.
    let source = program(
        "",
        "  let table: map<number, string> = {}\n  let stored: number = 1.0\n  \
         table.set(stored, `one`)\n  let half: number = 0.5\n  let computed: number = half + half\n  \
         print(table[computed].or(`missing`))\n  print(table.has(computed))\n  \
         print(table.get(computed).or(`missing`))\n  let other: number = 1.5\n  \
         print(table.has(other))\n  print(table[other].or(`missing`))\n",
    );
    assert_prints(&source, "one\ntrue\none\nfalse\nmissing\n");
}

#[test]
fn number_key_identical_overwrites_and_another_scale_is_another_key() {
    // Bug caught: an identical key inserting a second entry (count 2), an overwrite replacing the
    // stored key, or a key of another scale (`1.00` against `1.0`) found as the first.
    // Tier: High.
    let source = program(
        "",
        "  let table: map<number, string> = {}\n  let first: number = 1.0\n  \
         let second: number = 0.5 + 0.5\n  let third: number = 1.00\n  \
         table.set(first, `first`)\n  table.set(second, `second`)\n  print(table.count())\n  \
         table[third] = `third`\n  print(table.count())\n  for (entry in table) {\n    \
         print(`${entry.key} ${entry.value}`)\n  }\n",
    );
    assert_prints(&source, "1\n2\n1.0 second\n1.00 third\n");
}

#[test]
fn number_keys_survive_growth_past_several_rehashes() {
    // Bug caught: a rehash that hashes the pointer or 8 of the 16 key bytes, or moves value
    // cells but not keys, losing entries once the table grows (16 slots at first, 200 keys
    // here, and the insertion-order buffer holds 64). Tier: High.
    let source = program(
        "",
        "  let table: map<number, int> = {}\n  let step: number = 0.5\n  let key: number = 0.0\n  \
         for (round in range(0, 200)) {\n    table.set(key, round)\n    key = key + step\n  }\n  \
         let probe: number = 0.0\n  let total = 0\n  let missing = 0\n  \
         for (round in range(0, 200)) {\n    total = total + table[probe].or(0 - 1)\n    \
         probe = probe + 0.5\n  }\n  let beyond: number = 100.0\n  \
         print(`${table.count()} ${total} ${table[beyond].or(0 - 1)}`)\n  \
         let key_total: number = 0.0\n  let value_total = 0\n  for (entry in table) {\n    \
         key_total = key_total + entry.key\n    value_total = value_total + entry.value\n  }\n  \
         print(`${key_total} ${value_total}`)\n  print(missing)\n",
    );
    assert_prints(&source, "200 19900 -1\n9950.0 19900\n0\n");
}

#[test]
fn copy_of_a_number_keyed_map_is_independent_of_a_later_set() {
    // Bug caught: a copy sharing the source's key buffer (or reading it at 8 bytes a key), so
    // a later overwrite or insert shows in the copy. The values are arrays, so the copy also
    // runs the nested-value deep copy, which reads each key back and sets it again. Tier: High.
    let source = program(
        "",
        "  let table: map<number, array<int>> = {}\n  let low: number = 1.5\n  let high: number = 2.5\n  \
         let one: array<int> = [1]\n  let two: array<int> = [2, 2]\n  table.set(low, one)\n  \
         table.set(high, two)\n  let copy = table.copy()\n  let probe: number = 1.5\n  \
         let replacement: array<int> = [7, 7, 7]\n  table.set(probe, replacement)\n  \
         let extra: number = 3.5\n  table.set(extra, [4, 4, 4, 4])\n  let absent: array<int> = []\n  \
         print(`${copy.count()} ${table.count()}`)\n  \
         let copied_row = copy[probe].or(absent)\n  let table_row = table[probe].or(absent)\n  \
         print(`${copied_row.count()} ${table_row.count()}`)\n  \
         print(copy.has(extra))\n  for (entry in copy) {\n    print(`${entry.key} ${entry.value.count()}`)\n  }\n",
    );
    assert_prints(&source, "2 3\n1 3\nfalse\n1.5 1\n2.5 2\n");
}

#[test]
fn number_keyed_map_literal_and_channel_round_trip() {
    // Bug caught: a literal or a channel hop (a copy of the map) keeping 8-byte keys. Tier: High.
    let source = program(
        "",
        "  let literal: map<number, string> = {1.5: `a`, 2.5: `b`}\n  let probe: number = 2.5\n  \
         let line: channel<map<number, string>> = channel<map<number, string>>(2)\n  \
         line.send(literal.copy())\n  let empty: map<number, string> = {}\n  \
         let got = line.receive()\n  let received = got.or(empty)\n  print(received[probe].or(`missing`))\n  \
         for (entry in received) {\n    print(entry.key)\n  }\n",
    );
    assert_prints(&source, "b\n1.5\n2.5\n");
}

/// A function spawns three tasks with a `map<number, string>` it still reads, then returns; the
/// caller clobbers the stack before the tasks (which first wait or spin) read their copies.
fn spawned_number_keys_program(suspending_callee: bool) -> String {
    let wait_for_the_spawner = if suspending_callee {
        "  wait sleep(100)\n"
    } else {
        "  let spins = 0\n  let mixed = 1\n  while (spins < 60000000) {\n    \
         mixed = (mixed * 31 + spins) % 1000003\n    spins = spins + 1\n  }\n  \
         if (mixed < 0) {\n    print(mixed)\n  }\n"
    };
    format!(
        "function show(table: map<number, string>) -> nothing {{\n{wait_for_the_spawner}  \
         let probe: number = 5.00\n  let key_total: number = 0.0\n  let names = ``\n  \
         for (entry in table) {{\n    key_total = key_total + entry.key\n    names = `${{names}}${{entry.value}} `\n  }}\n  \
         print(`${{table[probe].or(`missing`)}} ${{names}}${{key_total}}`)\n}}\n\n\
         function launch(amount: number) -> nothing {{\n  let doubled: number = amount * 2.0\n  \
         let table: map<number, string> = {{}}\n  table.set(amount, `two`)\n  \
         table.set(doubled, `four`)\n  \
         for (round in range(0, 3)) {{\n    background show(table)\n  }}\n  \
         print(table.count())\n}}\n\n{CLOBBER}\
         function entrypoint() -> nothing {{\n  let seed: number = 1.5\n  \
         let amount: number = seed + 1.0\n  launch(amount)\n  let scratch = clobber(5)\n  \
         print(scratch)\n  wait sleep(1500)\n}}\n"
    )
}

fn assert_spawned_tasks_read_their_number_keys(suspending_callee: bool) {
    let source = spawned_number_keys_program(suspending_callee);
    let expected_lines = |stdout: &str| {
        assert_eq!(
            stdout.matches("four two four 7.50\n").count(),
            3,
            "{stdout}"
        );
        assert!(
            stdout.contains("2\n") && stdout.contains("54\n"),
            "{stdout}"
        );
    };
    at_both_optimization_levels(|no_optimize| {
        let built = build_source(&source, "c6 number key spawn", no_optimize);
        let (output, _) = run_counted_until_tasks_finish(&built);
        assert_eq!(output.exit_code, 0, "{source}\nstderr:\n{}", output.stderr);
        expected_lines(&output.stdout);
        let run = run_asan_binary_until_tasks_finish(&build_asan(&built));
        assert!(
            !run.stderr.contains("AddressSanitizer"),
            "AddressSanitizer reported an error (no_optimize={no_optimize}):\n{}",
            run.stderr
        );
        expected_lines(&run.stdout);
    });
}

#[test]
fn spawned_cpu_tasks_read_the_number_keys_of_a_finished_spawner() {
    // Bug caught: a task's copy of the map holding keys that point into the spawner's dead frame.
    // Tier: High.
    assert_spawned_tasks_read_their_number_keys(false);
}

#[test]
fn spawned_state_machine_tasks_read_the_number_keys_of_a_finished_spawner() {
    // Bug caught: as above through the state-machine arm. Tier: High.
    assert_spawned_tasks_read_their_number_keys(true);
}

#[test]
fn spawns_of_a_number_keyed_map_in_a_loop_balance() {
    // Bug caught: a spawn copy of a number-keyed map that leaks or frees buffers it shares with
    // the spawner (the key buffer is sized by the key, not by 8). Three more spawns must add as
    // many frees as allocations. Tier: High.
    let counts = |tasks: usize| {
        let source = format!(
            "function show(table: map<number, array<int>>) -> nothing {{\n  print(777)\n}}\n\n\
             function launch(amount: number) -> nothing {{\n  let doubled: number = amount * 2.0\n  \
             let items: array<int> = [1, 2]\n  let table: map<number, array<int>> = {{}}\n  \
             table.set(amount, items.copy())\n  table.set(doubled, items)\n  \
             for (round in range(0, {tasks})) {{\n    background show(table)\n  }}\n  \
             print(table.count())\n}}\n\nfunction entrypoint() -> nothing {{\n  \
             let seed: number = 1.5\n  let amount: number = seed + 1.0\n  launch(amount)\n  \
             wait sleep(300)\n}}\n"
        );
        let built = build_source(&source, "c6 number key spawn counts", false);
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
    let ((alloc_one, free_one), (alloc_four, free_four)) = concurrently(|| counts(1), || counts(4));
    assert!(alloc_four > alloc_one, "three more spawns copied nothing");
    assert_eq!(alloc_four - alloc_one, free_four - free_one);
}
