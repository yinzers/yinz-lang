//! State-machine codegen with composed frames, a typed return slot and inline poll-and-yield:
//! each case builds and runs a real `.ynz` fixture through the compiler binary and asserts its
//! stdout, exit code or heap counters.
//!
//! The recursion-cancellation case reads the fixture's timing windows (200ms per level, 500ms
//! cancel, 50ms shutdown): see `fixtures/v0_3_m2_recursive_cancel.ynz`.

mod support;

use support::{build, compile, run_counted, run_counted_with_drain_budget, run_plain, Counters};

/// Build `fixture`, run it with the alloc counters on and return them; the program must exit 0.
fn counters_of(fixture: &str) -> Counters {
    let built = build(fixture, false);
    let (run, counters) = run_counted(&built);
    assert_eq!(run.exit_code, 0, "{fixture} must exit 0; stderr:\n{}", run.stderr);
    counters
}

/// Build and run `fixture`: it must exit 0, and its stdout is returned.
fn stdout_of(fixture: &str) -> String {
    let run = run_plain(fixture, false);
    assert_eq!(
        run.exit_code, 0,
        "{fixture} must exit 0; stdout={:?} stderr={:?}",
        run.stdout, run.stderr
    );
    run.stdout
}

/// One named test per input class of the `state-machine value flow` unit; each case is a test
/// the runner names.
macro_rules! state_machine_value_flow_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a state machine corrupts, drops or mis-slots a value across a return, an error propagation or a resume (wrong printed value, crash), or a spawn from a suspending body deadlocks | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

state_machine_value_flow_high_case! {
    // case: an `int`-returning SM stores its result at frame offset 0 as an i32 instead of in the typed return slot (truncation)
    value_return_int_sm_prints_and_exits_0 => {
        let stdout = stdout_of("v0_3_m2_value_return_int.ynz");
        assert!(stdout.contains("result: 42"), "expected 'result: 42' in stdout; got: {stdout:?}");
    }
    // case: a `String` returned through the 16-byte return slot loses its SSO discriminant or heap pointer bits
    value_return_string_sm_byte_layout_intact => {
        let stdout = stdout_of("v0_3_m2_value_return_string.ynz");
        assert!(stdout.contains("len=13"), "expected len=13 (\"hello from SM\"); got: {stdout:?}");
        assert!(stdout.contains("val=hello from SM"), "expected string content intact; got: {stdout:?}");
    }
    // case: an errors-capable SM puts the success value in the error field of its `{i64,i64}` slot, so `.or(default)` returns the default
    errors_capable_sm_success_path_reads_correct_slot => {
        let stdout = stdout_of("v0_3_m2_errors_success.ynz");
        assert!(stdout.contains("got: 42"), "expected success value 42 via .or(0); got: {stdout:?}");
    }
    // case: an error produced after a `wait` is zeroed at the resume boundary, so `.or(99)` reads success field 0
    errors_capable_sm_error_propagates_through_suspension => {
        let stdout = stdout_of("v0_3_m2_errors_cascade.ynz");
        assert!(stdout.contains("safe: 99"), "expected 'safe: 99' (.or(99) falls back when the error propagated); got: {stdout:?}");
    }
    // case: a success value produced after a `wait` lands in the error field when the slot fields swap
    errors_capable_sm_success_propagates_through_suspension => {
        let stdout = stdout_of("v0_3_m2_errors_cascade_success.ynz");
        assert!(stdout.contains("safe: 42"), "expected 'safe: 42' (.or(99) returns the actual success value); got: {stdout:?}");
    }
    // case: an error loses its local narrowing at the second level of a nested SM chain
    errors_cascade_through_nested_sm_2level => {
        let stdout = stdout_of("v0_3_m2_errors_cascade_nested.ynz");
        assert!(stdout.contains("safe: 99"), "expected 'safe: 99' (error cascaded through 2 SM levels); got: {stdout:?}");
    }
    // case: a 3-level nested SM chain drives its sub-frames by recursing into the program-entry driver instead of inline poll-and-yield
    nested_sm_3level_prints_in_order_exits_0 => {
        let stdout = stdout_of("v0_3_m2_nested_sm_3level.ynz");
        assert!(stdout.contains("nested: 9"), "expected final value 7+1+1=9; got: {stdout:?}");
    }
    // case: a fn that suspends only through a callee (no explicit `wait`) is not classified as a state machine and crashes with "no reactor running"
    transitive_suspends_no_explicit_wait_runs_correctly => {
        let stdout = stdout_of("v0_3_m2_transitive_no_wait.ynz");
        assert!(stdout.contains("bar done"), "expected 'bar done'; got: {stdout:?}");
    }
    // case: an error raised before the first `wait` (state 0) does not survive the SM return slot
    state_machine_errors_before_first_wait => {
        let stdout = stdout_of("v0_3_m2_errors_before_first_wait.ynz");
        assert!(stdout.contains("v=99"), "expected 'v=99' (.or(99) falls back when the error propagated before the wait); got: {stdout:?}");
    }
    // case: an errors-capable `wait` result used in arithmetic panics codegen (raw error pointer where an int was expected), on the success and the error path
    errors_arith_across_wait_success_and_error_paths => {
        let stdout = stdout_of("v0_3_m2_errors_arith_across_wait.ynz");
        assert_eq!(stdout.trim(), "success=52\nerror=99", "success path: 42+10=52; error path: .or(99)=99");
    }
    // case: the result of a first suspending call read after a second suspension has no frame slot and the LLVM verifier aborts
    result_binding_crosses_later_suspension_compiles_and_runs => {
        let stdout = stdout_of("v0_3_m2_result_binding_crosses_later_suspension_error.ynz");
        assert_eq!(stdout.trim(), "10", "slot=5 + other=5 must equal 10");
    }
    // case: a `background` spawn inside a suspending entrypoint deadlocks on the runtime mutex (Bug C) or the worker does not run concurrently
    background_from_suspending_entrypoint_runs_concurrently => {
        let stdout = stdout_of("v0_3_m2_background_from_sm.ynz");
        assert_eq!(
            stdout, "main waiting\nworker done\nmain done\n",
            "the worker must complete while main waits"
        );
    }
}

/// One named test per input class of the `state-machine frame balance` unit; each case is a
/// test the runner names.
macro_rules! state_machine_frame_balance_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a composed state-machine frame (or a staging slot) is freed fewer times than it is allocated, so every run leaks | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

/// The program freed every frame it allocated.
fn assert_frames_balanced(fixture: &str) {
    let counters = counters_of(fixture);
    assert_eq!(
        counters.free, counters.alloc,
        "{fixture}: alloc/free must be balanced (no leak); got {counters:?}"
    );
}

state_machine_frame_balance_high_case! {
    // case: a 3-level synchronous SM tree leaks a sub-frame
    alloc_counter_3level_synchronous_tree_balances => {
        assert_frames_balanced("v0_3_m2_alloc_proof_3level.ynz");
    }
    // case: a background spawn's separate task tree is never freed
    alloc_counter_background_spawn_balances => {
        assert_frames_balanced("v0_3_m2_alloc_proof_background.ynz");
    }
    // case: a frame-backed crossing local leaks with its frame
    alloc_counter_crossing_locals_balance => {
        assert_frames_balanced("v0_3_m3a_p1_alloc_count.ynz");
    }
    // case: a shape crossing local heap-promoted separately from its frame (alloc=2, free=1) leaks the shape buffer
    alloc_counter_shape_crossing_local_no_leak => {
        assert_frames_balanced("v0_3_m3a_p1_shape_crossing_alloc_balance.ynz");
    }
    // case: the `-> number errors` frame-staging slot allocated outside the composed frame (the round-19 leak pattern) is never freed
    alloc_counter_number_errors_suspending_no_leak => {
        assert_frames_balanced("v0_3_m3a_p1_number_errors_returning_suspending_fn.ynz");
    }
    // case: a `while` loop over suspending iterations leaks per-iteration frames
    alloc_counter_while_loop_suspension_balances => {
        assert_frames_balanced("v0_3_m3a_p2_while_alloc_count.ynz");
    }
    // case: a `for` loop over suspending iterations leaks its frame-backed loop index or per-iteration frames
    alloc_counter_for_loop_suspension_balances => {
        assert_frames_balanced("v0_3_m3a_p3_for_alloc_count.ynz");
    }
    // case: a cancelled recursive SM leaks the heap-boxed child frames because the Drop chain walk is missing
    recursion_cancellation_no_leak => {
        // 50ms shutdown timeout: long enough for Tokio to join the worker threads (so Drop has
        // run before the counters are read), short enough to fire before countdown(1)'s 200ms
        // sleep ends at ~600ms. 0ms races the counter read against the worker's Drop.
        let built = build("v0_3_m2_recursive_cancel.ynz", false);
        let (_run, counters) = run_counted_with_drain_budget(&built, Some("50"));
        // Non-vacuity: entrypoint + root + countdown(2) + countdown(1) were live at the cancel;
        // fewer means the chain was not reached and a balanced count proves nothing.
        assert!(
            counters.alloc >= 4,
            "must reach alloc=4 (entrypoint+root+countdown(2)+countdown(1)) before the cancel at \
             500ms; got {counters:?} — check fixture timing or machine load"
        );
        assert_eq!(
            counters.alloc, counters.free,
            "alloc/free must balance after cancellation: SpawnStateFnFuture::Drop must walk the \
             recursion chain; got {counters:?}"
        );
    }
}

/// One named test per input class of the `state-machine frame allocation count` unit; each case
/// is a test the runner names.
macro_rules! state_machine_frame_count_medium_case {
    ($($case_name:ident, $fixture:literal, $expected_allocs:literal;)+) => { $(
        // bug: a state-machine task tree allocates more than its one composed frame (a per-call, per-iteration or per-local heap allocation) while still freeing it | tier: Medium
        #[test]
        fn $case_name() {
            let counters = counters_of($fixture);
            assert_eq!(
                counters.alloc, $expected_allocs,
                "{}: one ynz_alloc per task tree; got {counters:?}", $fixture
            );
        }
    )+ };
}

state_machine_frame_count_medium_case! {
    // case: N sleep calls in a 3-level synchronous tree cost N allocs instead of 1 (the composed-frame design)
    alloc_counter_3level_synchronous_tree_is_one_alloc, "v0_3_m2_alloc_proof_3level.ynz", 1;
    // case: a background spawn costs more than one extra alloc for its own tree (1 + 1 = 2)
    alloc_counter_background_spawn_adds_one_alloc, "v0_3_m2_alloc_proof_background.ynz", 2;
    // case: crossing locals get heap-allocated per local instead of living in the pre-sized frame
    alloc_counter_crossing_locals_add_zero_extra_allocs, "v0_3_m3a_p1_alloc_count.ynz", 1;
    // case: a shape crossing local gets a separate buffer instead of embedding in the frame
    alloc_counter_shape_crossing_local_no_extra_alloc, "v0_3_m3a_p1_shape_crossing_alloc_balance.ynz", 1;
    // case: the `-> number errors` staging slot is a separate allocation instead of part of the frame
    alloc_counter_number_errors_suspending_no_extra_alloc, "v0_3_m3a_p1_number_errors_returning_suspending_fn.ynz", 1;
    // case: a `while` loop over suspending iterations allocates per iteration (would show alloc=6)
    alloc_counter_while_loop_suspension_one_alloc, "v0_3_m3a_p2_while_alloc_count.ynz", 1;
    // case: a `for` loop's frame-backed index allocates separately per iteration (would show alloc=6)
    alloc_counter_for_loop_suspension_one_alloc, "v0_3_m3a_p3_for_alloc_count.ynz", 1;
}

/// One named test per input class of the `suspending call in a larger expression refused`
/// typeck unit; each case is a test the runner names.
macro_rules! suspending_call_refusal_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: typeck accepts a suspending call nested in a larger expression, or a mutually-recursive suspending cycle, and codegen reaches the nested `block_on` abort or corrupts the heap on cancellation | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

/// `fixture` is refused at compile time and its diagnostic contains `key_phrase`.
fn assert_refused_with(fixture: &str, key_phrase: &str) {
    let compiled = compile(fixture);
    assert!(
        !compiled.success,
        "{fixture} must be a compile error, not a program; diagnostics:\n{}",
        compiled.diagnostics
    );
    assert!(
        compiled.diagnostics.contains(key_phrase),
        "{fixture}: the teaching error must contain {key_phrase:?}; diagnostics:\n{}",
        compiled.diagnostics
    );
}

suspending_call_refusal_high_case! {
    // case: `let x = 1 + inner()` falls through to a `block_on` wrapper call inside a Tokio worker thread (non-unwinding abort, exit 0)
    subexpr_suspending_call_rejected_with_teaching_error => {
        assert_refused_with("v0_3_m2_subexpr_suspend_error.ynz", "suspending call inside a larger expression");
    }
    // case: `background add(inner(), 4)` evaluates a suspending argument on the caller's thread in sub-expression position (runtime panic)
    background_subexpr_suspending_call_rejected_with_teaching_error => {
        assert_refused_with("v0_3_m2_background_subexpr_error.ynz", "suspending call inside a larger expression");
    }
    // case: a mutually-recursive suspending cycle (ping -> pong -> ping) has mixed frame layouts and corrupts the heap on cancellation
    mutual_recursion_suspending_rejected_with_teaching_error => {
        assert_refused_with("v0_3_m2_mutual_recursion_error.ynz", "mutually-recursive suspending cycle");
    }
}

// bug: the can't-infer dynamic-dispatch gate fires for a non-suspending free-fn relay, so typeck rejects a valid program | tier: High
#[test]
fn free_fn_non_suspending_relay_no_cant_infer_error() {
    // The fixture still stops in codegen (dynamic dispatch call sites are not lowered yet), so
    // the only observable accept/reject signal is that the failure is never the typeck gate.
    let compiled = compile("v0_3_m2_free_fn_non_suspending_relay.ynz");
    assert!(
        !compiled.diagnostics.contains("Can't determine whether"),
        "the can't-infer gate must not fire for a non-suspending free-fn relay; diagnostics:\n{}",
        compiled.diagnostics
    );
}
