//! v0.3-M9 C6: the parked entries 69, 70 and 71 that name a spawn argument the C6 release helper
//! does not cover by itself (entries 67 and 68 are the `looped_*` cases of
//! `v03_m9_c6_release_per_type.rs`).
//!
//! Entry 71: a `background` task is handed a union argument. A union has no owned copy, so the
//! spawn path has to give the task its own slot or refuse the spawn. Every case spawns from a
//! function that has returned, and then clobbers the stack, before the task reads: the task must
//! still see the variant and the string the spawner held at the spawn.
//! High tier: data integrity (a task reading the spawner's dead frame).

mod support;

use support::{build_asan, build_source, run_asan_binary, run_counted};

/// What the callee does while the spawner moves on, which decides the spawn arm.
#[derive(Clone, Copy)]
enum Callee {
    /// Never suspends: the task runs on the blocking pool through a closure.
    Cpu,
    /// Suspends first: the task is a state machine on the I/O pool.
    StateMachine,
}

/// How the union reaches the spawn: a field read (`box.choice`), which the spawner still owns
/// afterwards, or a binding the spawn consumes.
#[derive(Clone, Copy)]
enum Argument {
    FieldRead,
    ConsumedBinding,
    /// A `Tag` binding handed to the union parameter, which the spawn consumes.
    ShapeIntoUnion,
}

const DECLARATIONS: &str = "shape Circle {\n  radius: float\n}\nshape Tag {\n  name: string\n}\n\
                            shape Box {\n  choice: Circle | Tag\n  id: int\n}\n";

fn union_spawn_program(callee: Callee, argument: Argument, tasks: usize) -> String {
    let wait_for_the_spawner = match callee {
        Callee::Cpu => {
            // A loop LLVM cannot fold, long enough that the spawner has returned and the
            // entrypoint has clobbered the stack before the task reads its argument.
            "  let spins = 0\n  let mixed = 1\n  while (spins < 60000000) {\n    \
             mixed = (mixed * 31 + spins) % 1000003\n    spins = spins + 1\n  }\n  \
             if (mixed < 0) {\n    print(mixed)\n  }\n"
        }
        Callee::StateMachine => "  wait sleep(150)\n",
    };
    let spawn_and_overwrite = match argument {
        Argument::FieldRead => format!(
            "  let tag: Tag = {{ name: `boxed` }}\n  let box: Box = {{ choice: tag, id: 1 }}\n  \
             for (round in range(0, {tasks})) {{\n    background eat(box.choice)\n  }}\n  \
             let circle: Circle = {{ radius: 2.0 }}\n  let next: Circle | Tag = circle\n  \
             box.choice = next\n  print(box.id)\n"
        ),
        Argument::ConsumedBinding => format!(
            "  let tag: Tag = {{ name: `boxed` }}\n  let choice: Circle | Tag = tag\n  \
             background eat(choice)\n  print({tasks})\n"
        ),
        Argument::ShapeIntoUnion => format!(
            "  let tag: Tag = {{ name: `boxed` }}\n  background eat(tag)\n  print({tasks})\n"
        ),
    };
    format!(
        "{DECLARATIONS}\nfunction eat(held: Circle | Tag) -> nothing {{\n{wait_for_the_spawner}  \
         if (held) {{\n    is Tag => {{\n      print(`task sees ${{held.name}}`)\n    }}\n    \
         is Circle => {{\n      print(`task sees circle`)\n    }}\n  }}\n}}\n\n\
         function clobber(seed: int) -> int {{\n  let plusOne = seed + 1\n  \
         let doubled = plusOne * 2\n  return doubled + plusOne\n}}\n\n\
         function launch() -> nothing {{\n{spawn_and_overwrite}}}\n\n\
         function entrypoint() -> nothing {{\n  launch()\n  let scratch = clobber(5)\n  \
         print(scratch)\n  wait sleep(1500)\n}}\n"
    )
}

fn assert_task_reads_its_own_union(callee: Callee, argument: Argument, no_optimize: bool) {
    let tasks = match argument {
        Argument::FieldRead => 3,
        Argument::ConsumedBinding | Argument::ShapeIntoUnion => 1,
    };
    let source = union_spawn_program(callee, argument, tasks);
    let built = build_source(&source, "c6 union spawn", no_optimize);
    let (output, _) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "{source}\nstderr:\n{}", output.stderr);
    assert_eq!(
        output.stdout.matches("task sees boxed\n").count(),
        tasks,
        "every task must read the Tag the spawner held at the spawn:\n{}\n{source}",
        output.stdout
    );
    let run = run_asan_binary(&build_asan(&built));
    assert!(
        !run.stderr.contains("AddressSanitizer"),
        "AddressSanitizer reported an error:\n{}\n{source}",
        run.stderr
    );
    assert_eq!(
        run.stdout.matches("task sees boxed\n").count(),
        tasks,
        "under AddressSanitizer:\n{}\n{source}",
        run.stdout
    );
}

macro_rules! union_spawn_cases {
    ($name:ident, $bug:literal, $callee:expr, $argument:expr) => {
        #[doc = $bug]
        mod $name {
            use super::*;

            #[test]
            fn optimized() {
                assert_task_reads_its_own_union($callee, $argument, false);
            }

            #[test]
            fn no_optimize() {
                assert_task_reads_its_own_union($callee, $argument, true);
            }
        }
    };
}

union_spawn_cases!(
    cpu_callee_field_read,
    "parked 71: a CPU task is handed a pointer to the spawner's dead union envelope (High)",
    Callee::Cpu,
    Argument::FieldRead
);
union_spawn_cases!(
    cpu_callee_consumed_binding,
    "parked 71: a CPU task is handed a pointer to the spawner's dead union envelope (High)",
    Callee::Cpu,
    Argument::ConsumedBinding
);
union_spawn_cases!(
    state_machine_callee_field_read,
    "parked 71: a state-machine task reads the string the spawner overwrote and freed (High)",
    Callee::StateMachine,
    Argument::FieldRead
);
union_spawn_cases!(
    state_machine_callee_consumed_binding,
    "parked 71: a state-machine task reads a union binding the spawn consumed (High)",
    Callee::StateMachine,
    Argument::ConsumedBinding
);

/// `(cell alloc, cell free, string alloc, string free)` of one run that spawns `tasks` tasks from
/// a `for` loop, each handed `box.choice` (a `Tag` holding a string).
fn counted_union_loop_run(suspending_callee: bool, tasks: usize) -> (u64, u64, u64, u64) {
    let callee_wait = if suspending_callee {
        "  wait sleep(1)\n"
    } else {
        ""
    };
    let source = format!(
        "{DECLARATIONS}\nfunction eat(held: Circle | Tag) -> nothing {{\n{callee_wait}  \
         print(777)\n}}\n\nfunction launch() -> nothing {{\n  \
         let tag: Tag = {{ name: `boxed` }}\n  let box: Box = {{ choice: tag, id: 1 }}\n  \
         for (round in range(0, {tasks})) {{\n    background eat(box.choice)\n  }}\n  \
         print(box.id)\n}}\n\nfunction entrypoint() -> nothing {{\n  launch()\n  \
         wait sleep(300)\n}}\n"
    );
    let built = build_source(&source, "c6 union loop counts", false);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "{source}\nstderr:\n{}", output.stderr);
    assert_eq!(
        output.stdout.matches("777\n").count(),
        tasks,
        "every task must have run before the counters are read:\n{source}"
    );
    (
        counters.alloc,
        counters.free,
        counters.string_alloc,
        counters.string_free,
    )
}

/// Three more loop iterations add exactly as many frees as allocations for a CPU callee: the
/// task's slot and the string it owns are released through the one release helper.
#[test]
fn cpu_callee_releases_its_union_slot_in_a_spawn_loop() {
    // Bug caught: the task's union slot (a cell, and a string the slot owns) freed by nobody, so
    // a spawn in a loop grows the heap. Tier: High.
    let (alloc_one, free_one, string_alloc_one, string_free_one) = counted_union_loop_run(false, 1);
    let (alloc_four, free_four, string_alloc_four, string_free_four) =
        counted_union_loop_run(false, 4);
    assert!(
        string_alloc_four > string_alloc_one,
        "three more spawns copied no string: the slot was not persisted by value"
    );
    assert_eq!(
        (alloc_four - alloc_one, string_alloc_four - string_alloc_one),
        (free_four - free_one, string_free_four - string_free_one),
        "three more spawns must free what they allocate (cells, strings)"
    );
}
union_spawn_cases!(
    cpu_callee_shape_into_union,
    "parked 71: a CPU task handed a plain shape for its union parameter reads a dead envelope (High)",
    Callee::Cpu,
    Argument::ShapeIntoUnion
);
union_spawn_cases!(
    state_machine_callee_shape_into_union,
    "parked 71: a state-machine task handed a plain shape for its union parameter (High)",
    Callee::StateMachine,
    Argument::ShapeIntoUnion
);

/// Parked entry 69: a deep copy (`union_deepen_fn`) and a release (`release_fn`) that walk a
/// self-referential union run over the data. A chain of `CHAIN_LENGTH` nodes is handed to a task
/// in a `for` loop and read by the spawner afterwards, so each spawn copies the whole chain and
/// the task releases it.
const CHAIN_LENGTH: usize = 500;

fn chain_loop_program(suspending_callee: bool, tasks: usize) -> String {
    let callee_wait = if suspending_callee {
        "  wait sleep(1)\n"
    } else {
        ""
    };
    format!(
        "shape End {{\n  flag: boolean\n}}\nshape Node {{\n  value: int\n  next: Node | End\n}}\n\n\
         function build(length: int) -> Node {{\n  let stop: End = {{ flag: true }}\n  \
         let head: Node = {{ value: 0, next: stop }}\n  let round = 1\n  \
         while (round < length) {{\n    let node: Node = {{ value: round, next: head }}\n    \
         head = node\n    round = round + 1\n  }}\n  return head\n}}\n\n\
         function show(chain: Node) -> nothing {{\n{callee_wait}  print(777)\n}}\n\n\
         function launch() -> nothing {{\n  let head = build({CHAIN_LENGTH})\n  \
         for (round in range(0, {tasks})) {{\n    background show(head)\n  }}\n  \
         print(head.value)\n}}\n\nfunction entrypoint() -> nothing {{\n  launch()\n  \
         wait sleep(300)\n}}\n"
    )
}

fn assert_chain_spawns_balance(suspending_callee: bool, no_optimize: bool) {
    let counts = |tasks: usize| {
        let source = chain_loop_program(suspending_callee, tasks);
        let built = build_source(&source, "c6 recursive chain spawn", no_optimize);
        let (output, counters) = run_counted(&built);
        assert_eq!(output.exit_code, 0, "{source}\nstderr:\n{}", output.stderr);
        assert_eq!(output.stdout.matches("777\n").count(), tasks, "{source}");
        let run = run_asan_binary(&build_asan(&built));
        assert!(
            !run.stderr.contains("AddressSanitizer"),
            "AddressSanitizer reported an error:\n{}",
            run.stderr
        );
        (counters.alloc, counters.free)
    };
    let (alloc_one, free_one) = counts(1);
    let (alloc_four, free_four) = counts(4);
    assert!(
        alloc_four - alloc_one >= 3 * CHAIN_LENGTH as u64,
        "three more spawns copied less than three chains: {alloc_one} -> {alloc_four} cells"
    );
    assert_eq!(
        alloc_four - alloc_one,
        free_four - free_one,
        "three more spawns of a {CHAIN_LENGTH}-node chain must free every cell they allocate"
    );
}

#[test]
fn recursive_chain_spawn_in_a_loop_balances_cpu_callee() {
    // Bug caught: the recursive copy or release of a self-referential union misses a box per node,
    // or frees one twice. Tier: High (data integrity).
    assert_chain_spawns_balance(false, false);
    assert_chain_spawns_balance(false, true);
}

#[test]
fn recursive_chain_spawn_in_a_loop_balances_state_machine_callee() {
    // Bug caught: as above, through the state-machine retire ladder. Tier: High.
    assert_chain_spawns_balance(true, false);
    assert_chain_spawns_balance(true, true);
}
