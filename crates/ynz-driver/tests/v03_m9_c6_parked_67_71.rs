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
        Argument::ConsumedBinding => 1,
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
