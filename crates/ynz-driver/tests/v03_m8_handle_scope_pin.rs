// v0.3-M9 C8 (card #385): a task handle leaving its scope STOPS its task.
//
// These two tests began as the v0.3-M8 Phase 7 pins of the re-deferral (a handle leaving its
// block did not cancel the child; no `ynz_handle_free` call was emitted). C8 lands the
// scope-exit release for suspension frames, so the pins are rewritten into the behaviour they
// guarded: the child's completion line never prints, and the handle's release is what stops it.
//
// The fixture binds `h = background worker()` inside an `if` block of a suspending
// `entrypoint`; the block exits while the child is still in its first sleep. The release at the
// block end calls `ynz_handle_free`, which aborts the child at its next suspension. The child's
// two output lines (`child: after first sleep`, `child: done`) are therefore lost; the parent's
// three lines are not.
//
// Both tests run the fixture compiled (`ynz build`, not `ynz run`) at one optimization level
// each, so a scope-exit release that only one level emits is caught. Plan: card #385, plan #17.
// C8 slice 2 built the release (the tests were planned RED before it). C8 slice 7 retired the
// old "task keeps running" registry deferral and its Tier 3 lint.
//
// One named test per level, no fixture loop (`.claude/rules/test-parallelism.md`).

mod support;

const FIXTURE: &str = "v0_3_m8_p7_handle_scope_exit_pin.ynz";

/// The line the child prints last. A task that was stopped never reaches it.
const CHILD_COMPLETION_LINE: &str = "child: done";

/// Lines the parent prints, in the order it prints them (the parent is a single task).
const PARENT_LINES: [&str; 3] = [
    "parent: leaving block",
    "parent: block exited",
    "parent: done",
];

/// Run the fixture at one optimization level and check the handle stopped its child.
fn assert_handle_leaving_scope_stops_the_child(no_optimize: bool) {
    let run = support::run_plain(FIXTURE, no_optimize);
    assert_eq!(
        run.exit_code, 0,
        "{FIXTURE} (no_optimize={no_optimize}): must exit 0; stderr:\n{}",
        run.stderr
    );
    assert!(
        !run.stdout.lines().any(|line| line == CHILD_COMPLETION_LINE),
        "{FIXTURE} (no_optimize={no_optimize}): the child printed `{CHILD_COMPLETION_LINE}` after \
         its handle left scope, so the task was not stopped; stdout:\n{}",
        run.stdout
    );
    let parent_lines: Vec<&str> = run
        .stdout
        .lines()
        .filter(|line| line.starts_with("parent: "))
        .collect();
    assert_eq!(
        parent_lines, PARENT_LINES,
        "{FIXTURE} (no_optimize={no_optimize}): stopping the child must not disturb the parent; \
         stderr:\n{}",
        run.stderr
    );
    // The child may have printed its first line before the abort reached it, never its last; any
    // other line is a program that is not this fixture.
    let unexpected: Vec<&str> = run
        .stdout
        .lines()
        .filter(|line| !line.starts_with("parent: ") && *line != "child: after first sleep")
        .collect();
    assert!(
        unexpected.is_empty(),
        "{FIXTURE} (no_optimize={no_optimize}): unexpected lines {unexpected:?}"
    );
}

/// A handle binding leaving its block stops the child: its completion line never prints
/// (optimized build).
#[test]
fn handle_leaving_its_block_stops_the_child_optimized() {
    assert_handle_leaving_scope_stops_the_child(false);
}

/// The same at `--no-optimize`, where the optimizer cannot hide a release that was never emitted,
/// and the IR carries the call that does the stopping: one `ynz_handle_free` for the one handle
/// binding. The IR check is the second half of the proof, not the proof: the behaviour above is.
#[test]
fn handle_leaving_its_block_stops_the_child_unoptimized() {
    assert_handle_leaving_scope_stops_the_child(true);
    let built = support::build(FIXTURE, true);
    let ir = std::fs::read_to_string(&built.ir).expect("the emitted .ll is readable");
    let handle_free_calls = ir
        .lines()
        .filter(|line| line.contains("call ") && line.contains("@ynz_handle_free("))
        .count();
    assert_eq!(
        handle_free_calls, 1,
        "{FIXTURE}: one `ynz_handle_free` call for the one handle binding's scope exit"
    );
    let spawn_calls = ir
        .lines()
        .filter(|line| line.contains("call ") && line.contains("@ynz_rt_spawn_handle("))
        .count();
    assert_eq!(
        spawn_calls, 1,
        "{FIXTURE}: the handle-form spawn must compile to exactly one ynz_rt_spawn_handle call"
    );
}
