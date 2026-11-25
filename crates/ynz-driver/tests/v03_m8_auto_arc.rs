//! Auto-Arc codegen emission (topology B), locked end-to-end by `.ynz` fixtures: the
//! beneficial-emission condition admits exactly the groups the design names, every decline
//! leaves the shipped copy path, and the refcount protocol balances under real concurrent load.
//! Design: `IMP-ownership.md` "Auto-Arc — Sharing Topology Across `background` Boundaries".
//!
//! Two tiers over the same fixtures: the run of each fixture (exit 0, stdout multiset, counted
//! alloc == free) is High; the extra-work claim (how many `ynz_arc_*` calls the IR carries, how
//! many allocations a group saves) is Medium.

mod support;

use support::{build, run_counted, Built};

/// Stdout lines in sorted order: tasks print concurrently, so only the multiset is the contract.
fn sorted_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.lines().collect();
    lines.sort_unstable();
    lines
}

/// Run `fixture` and check the Arc contract: exit 0 (a signal is a use-after-free or double free
/// on the shared block), the expected stdout multiset (a wrong total is a torn read or a task
/// reading a value it should not see) and `alloc == free` with a non-vacuous counter.
fn assert_fixture_runs_balanced(fixture: &str, expected_stdout: &str) {
    let built = build(fixture, false);
    let (run, counters) = run_counted(&built);
    assert_eq!(
        run.exit_code, 0,
        "{fixture}: must exit 0; stderr:\n{}",
        run.stderr
    );
    assert_eq!(
        sorted_lines(&run.stdout),
        sorted_lines(expected_stdout),
        "{fixture}: stdout multiset mismatch; stderr:\n{}",
        run.stderr
    );
    assert!(
        counters.alloc > 0,
        "{fixture}: vacuous parity run, the alloc counter saw nothing"
    );
    assert_eq!(
        counters.alloc, counters.free,
        "{fixture}: alloc/free parity broken: a leaked count or a skipped release"
    );
}

/// One named test per admission input class of the `Auto-Arc group run` unit; each case is a
/// test the runner names.
macro_rules! auto_arc_run_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal;)+) => { $(
        // bug: an Auto-Arc group (or a declined group's copy path) tears a read, leaks a refcount, double frees the shared block or shows a task a stale value | tier: High
        #[test]
        fn $case_name() {
            assert_fixture_runs_balanced($fixture, $expected_stdout);
        }
    )+ };
}

auto_arc_run_high_case! {
    // case: two spawns of one read-only shape share one block; each task's reference is released exactly once
    two_spawn_group_emits_one_block_two_clones_and_balances, "m8_arc_two_spawn_group.ynz", "tasks saw 84\ncaller keeps Three Rivers 6x7\n";
    // case: a single reader is not a sharing case; the shipped one-copy path balances
    one_spawn_emits_no_arc_symbol_at_all, "m8_arc_one_spawn_noop.ynz", "task saw 42\ncaller keeps Three Rivers 6x7\n";
    // case: a caller write between the spawns forces a decline; the second task copies the CURRENT value (42 + 70)
    caller_side_write_between_spawns_declines_and_second_task_sees_the_update, "m8_arc_write_between_declines.ynz", "tasks total 112\ncaller keeps Stargell 10x7\n";
    // case: a write inside a member spawn's own argument list (`bump(scene)`) forces a decline; admitting it made task 2 read the block minted before the bump
    caller_side_write_inside_a_member_spawns_own_argument_declines, "m8_arc_write_in_member_arg_declines.ynz", "task saw 112\ntask saw 106\ncaller keeps Wagner 106x7\n";
    // case: a `wait` between the spawns would force the transient across a frame boundary; the group declines
    suspension_between_spawns_declines, "m8_arc_suspend_between_declines.ynz", "tasks total 84\ncaller keeps Mazeroski 6x7\n";
    // case: an early return between the spawns would skip the transient's straight-line release; the group declines
    early_return_between_spawns_declines, "m8_arc_return_between_declines.ynz", "tasks total 84\ncaller keeps Bonds 6x7\n";
    // case: a top-level rebinding between spawns ends one group and starts another; each pair reads its own value (2*42 + 2*100)
    top_level_rebinding_between_spawns_is_a_group_boundary_not_a_decline, "m8_arc_rebind_boundary.ynz", "tasks total 284\ncaller keeps Kiner 10x10\n";
    // case: an explicit `.copy()` spawn opts out of the group and takes its own copy (3 * 42)
    explicit_copy_at_one_spawn_opts_that_spawn_out_of_the_group, "m8_arc_explicit_copy_opts_out.ynz", "tasks total 126\ncaller keeps Beaumont 6x7\n";
    // case: handle-form spawns (`let h = background f(v)`) join a group through the same recording path
    handle_form_spawns_form_a_group_too, "m8_arc_handle_form_group.ynz", "handles total 84\ncaller keeps Clemente 6x7\n";
    // case: non-suspending callees run on the blocking pool; the closure-body free releases the task reference
    cpu_arm_group_releases_through_the_closure_body_free, "m8_arc_cpu_arm_group.ynz", "cpu task saw 42\ncpu task saw 42\ncpu task saw 42\ncaller keeps Smithfield 6x7\n";
    // case: four tasks read one shared block under concurrent load (4 * 25 * 30*35 = 105000); a torn read or an early free changes it
    hammer_four_tasks_read_one_shared_block_under_concurrent_load, "m8_arc_hammer_shared_shape.ynz", "tasks total 105000\ncaller keeps Fort Pitt 30x35 depth 2\n";
    // case: a shape with a `number` field is excluded from sharing (its i128 alignment differs from the block's copy layout) and runs on the copy path
    number_field_excludes_the_shape_from_arc_sharing_and_declines_correctly, "m8_arc_number_field_declines.ynz", "tasks saw 84\ncaller keeps Three Rivers 6x7 1.5\n";
}

/// Count CALLS of a runtime symbol in IR text (`call ... @sym(`), never its `declare` line.
fn ir_call_count(ir: &str, symbol: &str) -> usize {
    let needle = format!("@{symbol}(");
    ir.lines()
        .filter(|line| line.contains("call ") && line.contains(&needle))
        .count()
}

/// The `--no-optimize` IR of `fixture`: pins what codegen emits, before the optimizer can inline
/// or fold the call shape counted here.
fn unoptimized_ir(fixture: &str) -> String {
    let built: Built = build(fixture, true);
    std::fs::read_to_string(&built.ir).expect("the emitted .ll is readable")
}

/// One named test per group shape of the `Auto-Arc sharing cost` unit; each case is a test the
/// runner names. `$arc_calls` is the `(ynz_arc_new, ynz_arc_clone, ynz_arc_free)` CALL counts
/// (`free` is the caller's transient releases plus, for the CPU arm, the closure-body releases;
/// the state-machine ladder's release is in the runtime, not the IR).
macro_rules! auto_arc_cost_medium_case {
    ($($case_name:ident, $fixture:literal, $arc_calls:expr;)+) => { $(
        // bug: a group of spawns of one read-only shape does not share one block, or a single reader gets an Arc, so the program does extra copies | tier: Medium
        #[test]
        fn $case_name() {
            let ir = unoptimized_ir($fixture);
            assert_eq!(
                (
                    ir_call_count(&ir, "ynz_arc_new"),
                    ir_call_count(&ir, "ynz_arc_clone"),
                    ir_call_count(&ir, "ynz_arc_free"),
                ),
                $arc_calls,
                "{}: (ynz_arc_new, ynz_arc_clone, ynz_arc_free) call counts must match the group shape",
                $fixture
            );
        }
    )+ };
}

auto_arc_cost_medium_case! {
    // case: two spawns share ONE `new`, two `clone`s, one transient `free`
    two_spawn_group_emits_one_block_and_two_clones, "m8_arc_two_spawn_group.ynz", (1, 2, 1);
    // case: one reader emits no `ynz_arc_*` call at all
    single_reader_emits_no_arc_call, "m8_arc_one_spawn_noop.ynz", (0, 0, 0);
    // case: spawns 1-2 and 3-4 are two groups (two `new`, four `clone`s, two `free`s)
    rebinding_boundary_forms_two_groups, "m8_arc_rebind_boundary.ynz", (2, 4, 2);
    // case: the CPU arm carries three task releases and one transient release in the IR
    cpu_arm_group_carries_closure_body_releases, "m8_arc_cpu_arm_group.ynz", (1, 3, 4);
}
