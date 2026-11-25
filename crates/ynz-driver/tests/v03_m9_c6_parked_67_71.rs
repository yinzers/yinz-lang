//! v0.3-M9 C6: the parked entries 69, 70 and 71 that name a spawn argument the C6 release helper
//! does not cover by itself (entries 67 and 68 are the `looped_*` cases of
//! `v03_m9_c6_release_per_type.rs`).
//!
//! Entry 71: a `background` task is handed a union argument. A union has no owned copy, so the
//! spawn gives the task a slot of its own, filled by value. Every case spawns from a
//! function that has returned, and then clobbers the stack, before the task reads: the task must
//! still see the variant and the string the spawner held at the spawn.
//! High tier: data integrity (a task reading the spawner's dead frame).

mod support;

use support::{
    at_both_optimization_levels, build_asan, build_source, concurrently, run_asan_binary,
    run_asan_binary_until_tasks_finish, run_counted, run_counted_until_tasks_finish,
};

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

const DECLARATIONS: &str = "shape Circle {\n  radius: Float\n}\nshape Tag {\n  name: String\n}\n\
                            shape Box {\n  choice: Circle | Tag\n  id: Int\n}\n";

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
        "{DECLARATIONS}\nfunction eat(held: Circle | Tag) -> Nothing {{\n{wait_for_the_spawner}  \
         if (held) {{\n    is Tag => {{\n      print(`task sees ${{held.name}}`)\n    }}\n    \
         is Circle => {{\n      print(`task sees circle`)\n    }}\n  }}\n}}\n\n\
         function clobber(seed: Int) -> Int {{\n  let plusOne = seed + 1\n  \
         let doubled = plusOne * 2\n  return doubled + plusOne\n}}\n\n\
         function launch() -> Nothing {{\n{spawn_and_overwrite}}}\n\n\
         function entrypoint() -> Nothing {{\n  launch()\n  let scratch = clobber(5)\n  \
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
    let (output, _) = run_counted_until_tasks_finish(&built);
    assert_eq!(output.exit_code, 0, "{source}\nstderr:\n{}", output.stderr);
    assert_eq!(
        output.stdout.matches("task sees boxed\n").count(),
        tasks,
        "every task must read the Tag the spawner held at the spawn:\n{}\n{source}",
        output.stdout
    );
    let run = run_asan_binary_until_tasks_finish(&build_asan(&built));
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

/// `(cell alloc, cell free, string alloc, string free)` of one run that spawns `tasks` tasks from
/// a `for` loop, each handed `box.choice` (a `Tag` holding a string).
fn counted_union_loop_run(suspending_callee: bool, tasks: usize) -> (u64, u64, u64, u64) {
    let callee_wait = if suspending_callee {
        "  wait sleep(1)\n"
    } else {
        ""
    };
    let source = format!(
        "{DECLARATIONS}\nfunction eat(held: Circle | Tag) -> Nothing {{\n{callee_wait}  \
         print(777)\n}}\n\nfunction launch() -> Nothing {{\n  \
         let tag: Tag = {{ name: `boxed` }}\n  let box: Box = {{ choice: tag, id: 1 }}\n  \
         for (round in range(0, {tasks})) {{\n    background eat(box.choice)\n  }}\n  \
         print(box.id)\n}}\n\nfunction entrypoint() -> Nothing {{\n  launch()\n  \
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
        "shape End {{\n  flag: Boolean\n}}\nshape Node {{\n  value: Int\n  next: Node | End\n}}\n\n\
         function build(length: Int) -> Node {{\n  let stop: End = {{ flag: true }}\n  \
         let head: Node = {{ value: 0, next: stop }}\n  let round = 1\n  \
         while (round < length) {{\n    let node: Node = {{ value: round, next: head }}\n    \
         head = node\n    round = round + 1\n  }}\n  return head\n}}\n\n\
         function show(chain: Node) -> Nothing {{\n{callee_wait}  print(777)\n}}\n\n\
         function launch() -> Nothing {{\n  let head = build({CHAIN_LENGTH})\n  \
         for (round in range(0, {tasks})) {{\n    background show(head)\n  }}\n  \
         print(head.value)\n}}\n\nfunction entrypoint() -> Nothing {{\n  launch()\n  \
         wait sleep(300)\n}}\n"
    )
}

fn assert_chain_spawns_balance(suspending_callee: bool, no_optimize: bool) {
    let counts = |tasks: usize| {
        let source = chain_loop_program(suspending_callee, tasks);
        let built = build_source(&source, "c6 recursive chain spawn", no_optimize);
        let (output, counters) = run_counted(&built);
        assert_eq!(output.exit_code, 0, "{source}\nstderr:\n{}", output.stderr);
        assert_eq!(
            output.stdout.matches("777\n").count(),
            tasks,
            "no_optimize={no_optimize}\n{source}"
        );
        let run = run_asan_binary(&build_asan(&built));
        assert!(
            !run.stderr.contains("AddressSanitizer"),
            "AddressSanitizer reported an error (no_optimize={no_optimize}):\n{}",
            run.stderr
        );
        (counters.alloc, counters.free)
    };
    let ((alloc_one, free_one), (alloc_four, free_four)) = concurrently(|| counts(1), || counts(4));
    assert!(
        alloc_four - alloc_one >= 3 * CHAIN_LENGTH as u64,
        "no_optimize={no_optimize}: three more spawns copied less than three chains: {alloc_one} -> {alloc_four} cells"
    );
    assert_eq!(
        alloc_four - alloc_one,
        free_four - free_one,
        "no_optimize={no_optimize}: three more spawns of a {CHAIN_LENGTH}-node chain must free every cell they allocate"
    );
}

/// One named test per spawn path of the `spawn a union argument` run unit; each case runs at both optimization levels (the spawn-argument cases also under AddressSanitizer).
macro_rules! union_spawn_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a spawned task reads a union argument the spawner's frame, envelope or release helper already freed, or the spawn leaks or double-frees the copy it made | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

union_spawn_high_case! {
    // case: a CPU task is handed a pointer to the spawner's dead union envelope
    cpu_callee_field_read => {
        at_both_optimization_levels(|no_optimize| {
            assert_task_reads_its_own_union(Callee::Cpu, Argument::FieldRead, no_optimize);
        });
    }
    // case: a CPU task is handed a pointer to the spawner's dead union envelope
    cpu_callee_consumed_binding => {
        at_both_optimization_levels(|no_optimize| {
            assert_task_reads_its_own_union(Callee::Cpu, Argument::ConsumedBinding, no_optimize);
        });
    }
    // case: a state-machine task reads the string the spawner overwrote and freed
    state_machine_callee_field_read => {
        at_both_optimization_levels(|no_optimize| {
            assert_task_reads_its_own_union(Callee::StateMachine, Argument::FieldRead, no_optimize);
        });
    }
    // case: a state-machine task reads a union binding the spawn consumed
    state_machine_callee_consumed_binding => {
        at_both_optimization_levels(|no_optimize| {
            assert_task_reads_its_own_union(Callee::StateMachine, Argument::ConsumedBinding, no_optimize);
        });
    }
    // case: a CPU task handed a plain shape for its union parameter reads a dead envelope
    cpu_callee_shape_into_union => {
        at_both_optimization_levels(|no_optimize| {
            assert_task_reads_its_own_union(Callee::Cpu, Argument::ShapeIntoUnion, no_optimize);
        });
    }
    // case: a state-machine task handed a plain shape for its union parameter
    state_machine_callee_shape_into_union => {
        at_both_optimization_levels(|no_optimize| {
            assert_task_reads_its_own_union(Callee::StateMachine, Argument::ShapeIntoUnion, no_optimize);
        });
    }
    // case: three more loop iterations add exactly as many frees as allocations for a CPU callee: the task's slot and the string it owns are released through the one release helper; bug: the task's union slot (a cell, and a string the slot owns) freed by nobody, so a spawn in a loop grows the heap.
    cpu_callee_releases_its_union_slot_in_a_spawn_loop => {
        let (
            (alloc_one, free_one, string_alloc_one, string_free_one),
            (alloc_four, free_four, string_alloc_four, string_free_four),
        ) = concurrently(
            || counted_union_loop_run(false, 1),
            || counted_union_loop_run(false, 4),
        );
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
    // case: the recursive copy or release of a self-referential union misses a box per node, or frees one twice..
    recursive_chain_spawn_in_a_loop_balances_cpu_callee => {
        at_both_optimization_levels(|no_optimize| assert_chain_spawns_balance(false, no_optimize));
    }
    // case: as above, through the state-machine retire ladder.
    recursive_chain_spawn_in_a_loop_balances_state_machine_callee => {
        at_both_optimization_levels(|no_optimize| assert_chain_spawns_balance(true, no_optimize));
    }
}
