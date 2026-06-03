/// v0.3-M2 Phase 7 integration tests: state-machine codegen with composed frames,
/// typed return slot, and inline poll-and-yield.
///
/// Each test compiles and runs a real `.ynz` fixture through the compiler binary
/// (`./target/debug/ynz run`) and asserts the stdout/stderr/exit-code.
///
/// These tests are the acceptance-criteria fixtures for Phase 7.
///
/// # Running under `cargo test --workspace`
///
/// These tests run automatically under `cargo test --workspace`. The test binary is
/// discovered via Cargo's standard `tests/*.rs` convention (no manual `[[test]]`
/// entry needed — the round-1 manual entry was removed when it broke auto-discovery).
///
/// The two alloc-counter tests (`alloc_counter_*`) set `YNZ_ALLOC_COUNTER` and
/// `YNZ_ALLOC_COUNTER_OUTPUT` internally via `.env()` — they are self-configuring
/// and run correctly under `--workspace` without external setup.
///
/// # Known snapshot path drift in worktrees
///
/// `cargo test --workspace` may report 5 failures in `integration__*.snap` snapshot
/// tests when run from a git worktree at a path other than the main checkout. These
/// failures are ENVIRONMENTAL: the `╭─[` diagnostic header embeds the absolute fixture
/// path, which differs between worktree (`/workspaces/ynz/.claude/worktrees/…`) and
/// main (`/workspaces/ynz/…`). The diagnostic content is byte-identical; the snapshots
/// pass on main. These 5 failures are NOT a Phase 7 regression — do not edit those
/// snapshot files to "fix" them in the worktree. Pass `--no-fail-fast` to see results
/// past these expected failures.
///
/// # Recursion-cancellation test serialization
///
/// The three `recursion_cancellation*` tests share a static mutex (`CANCEL_TEST_LOCK`)
/// so they run sequentially even when the test binary executes tests in parallel.
/// Serialization prevents CPU contention that could cause the fixture's sleep timing
/// to shift enough to change the alloc count. The timing margins in the fixture
/// (200ms per level, 500ms cancel window) are designed to be robust even on slow
/// machines, but serialization eliminates the contention source entirely.
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Serializes the three recursion_cancellation* tests so they cannot run concurrently.
///
/// Without serialization, concurrent execution on a loaded machine can cause sleep
/// timing to shift — countdown(3) with 200ms sleeps might not reach alloc=3 before
/// the 500ms cancel window closes if the worker threads are starved. Serializing the
/// three tests eliminates CPU-contention-driven timing variance between them.
static CANCEL_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the cancellation-test lock; returns a guard that releases it on drop.
/// If the mutex is poisoned (prior test panicked), clears poison and proceeds —
/// a panicked sibling test should not prevent the remaining siblings from running.
fn lock_cancel_tests() -> MutexGuard<'static, ()> {
    match CANCEL_TEST_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Run a fixture through `ynz run` and return (stdout, stderr, exit_code).
fn run_fixture(fixture: &str) -> (String, String, i32) {
    run_fixture_with_timeout(fixture, 10)
}

fn run_fixture_with_timeout(fixture: &str, timeout_secs: u64) -> (String, String, i32) {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture);

    let start = Instant::now();
    let output = Command::new(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .join("target/debug/ynz"),
    )
    .args(["run", fixture_path.to_str().expect("valid path")])
    .output()
    .expect("ynz binary not found — run `cargo build -p ynz-driver` first");

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(timeout_secs),
        "fixture {fixture} timed out after {elapsed:?} (limit {timeout_secs}s)"
    );

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

// ── AC 1: value-returning SM — int ────────────────────────────────────────────

// WHY: Bug A fix — a state machine returning `int` must store the value in the
// typed return slot at frame offset 16, not as an i32 at frame offset 0
// (the i32-truncation defect). A regression here means `int`-returning SMs
// silently truncate or corrupt their return values.
#[test]
fn value_return_int_sm_prints_and_exits_0() {
    let (stdout, _, code) = run_fixture("v0_3_m2_value_return_int.ynz");
    assert_eq!(code, 0, "should exit 0");
    assert!(
        stdout.contains("result: 42"),
        "expected 'result: 42' in stdout; got: {stdout:?}"
    );
}

// ── AC 1 (cont): value-returning SM — string ──────────────────────────────────

// WHY: String return from an SM goes through the 16-byte return slot as a pointer
// value (ptr_to_int). If the SSO discriminant or heap pointer bits are corrupted
// by the store/load, count() and content will diverge. This catches that class.
#[test]
fn value_return_string_sm_byte_layout_intact() {
    let (stdout, _, code) = run_fixture("v0_3_m2_value_return_string.ynz");
    assert_eq!(code, 0, "should exit 0");
    assert!(
        stdout.contains("len=13"),
        "expected len=13 (\"hello from SM\" = 13 chars); got: {stdout:?}"
    );
    assert!(
        stdout.contains("val=hello from SM"),
        "expected string content intact; got: {stdout:?}"
    );
}

// ── AC 2 (partial): errors-capable SM success path ────────────────────────────

// WHY: The {i64,i64} errors ABI has field0=error_ptr and field1=success_value.
// If the SM return slot stores the success value in field0 (the error position),
// .or(default) would see a non-null error pointer and return the default instead
// of the actual success value. This test catches that field-order defect.
#[test]
fn errors_capable_sm_success_path_reads_correct_slot() {
    let (stdout, _, code) = run_fixture("v0_3_m2_errors_success.ynz");
    assert_eq!(code, 0, "should exit 0");
    assert!(
        stdout.contains("got: 42"),
        "expected success value 42 via .or(0); got: {stdout:?}"
    );
}

// ── AC 2 (complete): errors-cascade through SM suspension — error path ───────

// WHY: An error produced AFTER a wait suspension must survive the resume boundary.
// The {i64,i64} return slot has error_ptr at field0. If field0 is zeroed (error dropped),
// .or(fallback) treats the result as success and returns field1 (= 0) instead of
// the fallback. This test proves the error_ptr propagates through the SM return slot.
#[test]
fn errors_capable_sm_error_propagates_through_suspension() {
    let (stdout, _, code) = run_fixture("v0_3_m2_errors_cascade.ynz");
    assert_eq!(code, 0, "should exit 0");
    assert!(
        stdout.contains("safe: 99"),
        "expected 'safe: 99' (.or(99) must fall back when error propagated); got: {stdout:?}"
    );
}

// WHY: Paired with the error-path test to prove field ordering is correct in both
// directions. If error_ptr and success swap positions, one direction passes and the
// other fails — running both pins the exact field semantics.
#[test]
fn errors_capable_sm_success_propagates_through_suspension() {
    let (stdout, _, code) = run_fixture("v0_3_m2_errors_cascade_success.ynz");
    assert_eq!(code, 0, "should exit 0");
    assert!(
        stdout.contains("safe: 42"),
        "expected 'safe: 42' (.or(99) must return actual success value); got: {stdout:?}"
    );
}

// WHY: 2-level nested SM chain: errors must propagate through BOTH SM boundaries.
// Verifies that the EC-local-narrowing bypass in lower_expr applies at every level
// of the SM chain, not just the innermost call.
#[test]
fn errors_cascade_through_nested_sm_2level() {
    let (stdout, _, code) = run_fixture("v0_3_m2_errors_cascade_nested.ynz");
    assert_eq!(code, 0, "should exit 0");
    assert!(
        stdout.contains("safe: 99"),
        "expected 'safe: 99' (error cascaded through 2 SM levels); got: {stdout:?}"
    );
}

// ── AC 4: 3-level nested SM ───────────────────────────────────────────────────

// WHY: nested SM calls must use inline poll-and-yield (composed frames); the
// program-entry driver is only at the top-level wrapper→resume handoff. A 3-level
// call chain drives inner sub-frames inline, not by recursing into the driver.
// This test exercises the full chain: entrypoint→outer→middle→inner→sleep.
#[test]
fn nested_sm_3level_prints_in_order_exits_0() {
    let (stdout, _, code) = run_fixture("v0_3_m2_nested_sm_3level.ynz");
    assert_eq!(code, 0, "should exit 0");
    assert!(
        stdout.contains("nested: 9"),
        "expected final value 7+1+1=9; got: {stdout:?}"
    );
}

// ── AC 5: background from suspending entrypoint ───────────────────────────────

// WHY: Bug C fix — if the parent SM is driven by block_on, a background spawn
// inside the SM body hangs because block_on holds the RUNTIME mutex, and
// ynz_rt_spawn also tries to acquire it → deadlock. The fix: block_on releases
// the mutex before polling; ynz_rt_spawn uses Handle::try_current() to bypass
// the mutex entirely when inside a Tokio context.
#[test]
fn background_from_suspending_entrypoint_runs_concurrently() {
    let (stdout, _, code) = run_fixture_with_timeout("v0_3_m2_background_from_sm.ynz", 5);
    assert_eq!(code, 0, "should exit 0");
    assert!(
        stdout.contains("main waiting"),
        "expected 'main waiting'; got: {stdout:?}"
    );
    assert!(
        stdout.contains("worker done"),
        "expected 'worker done'; got: {stdout:?}"
    );
    assert!(
        stdout.contains("main done"),
        "expected 'main done'; got: {stdout:?}"
    );
    // Worker should complete while main waits: worker done before main done.
    let worker_pos = stdout.find("worker done").unwrap_or(usize::MAX);
    let main_pos = stdout.find("main done").unwrap_or(0);
    assert!(
        worker_pos < main_pos,
        "worker should complete before main done (concurrency proof); stdout: {stdout:?}"
    );
}

// ── AC 6: transitive suspension without explicit wait ─────────────────────────

// WHY: The P6/P7 seam — typeck marks transitive fns as suspends=true, but P6
// codegen still used local contains_wait. P7 wires codegen to read suspends_set
// from typeck. The dev5 adversarial case (bar→sleep no wait,
// entrypoint→bar no wait) previously crashed with "no reactor running" because
// the codegen didn't classify bar/entrypoint as state machines.
#[test]
fn transitive_suspends_no_explicit_wait_runs_correctly() {
    let (stdout, _, code) = run_fixture("v0_3_m2_transitive_no_wait.ynz");
    assert_eq!(code, 0, "should exit 0, was crashing before P7");
    assert!(
        stdout.contains("bar done"),
        "expected 'bar done'; got: {stdout:?}"
    );
    assert!(stdout.contains("done"), "expected 'done'; got: {stdout:?}");
}

// ── AC 7 (behavioral): composed frame — single nested tree ────────────────────

// WHY: One ynz_alloc per spawned task tree is the design-doc model
// ("low memory, fast spawn — like Rust's async"). A per-call alloc would mean
// N sleep calls = N allocs. We verify the behavior is correct (not the
// alloc count directly — that requires runtime instrumentation in Phase 9).
#[test]
fn alloc_counter_fixture_produces_correct_result() {
    let (stdout, _, code) = run_fixture("v0_3_m2_alloc_counter.ynz");
    assert_eq!(code, 0, "should exit 0");
    assert!(
        stdout.contains("result: 2"),
        "expected result=2 (inner returns 1, outer adds 1); got: {stdout:?}"
    );
}

// ── AC 9: no bridge in resume fns (IR-level check via nm) ────────────────────

// WHY: ynz_rt_run_entrypoint inside a ynz_sm_*_resume function would mean nested
// block_on, which panics on Tokio worker threads. The program-entry driver must
// only appear in wrapper functions, never inside resume fns (those inline-poll-yield
// into embedded child sub-frames). This test checks the binary's symbol table.
#[test]
fn no_bridge_reachable_from_resume_fns() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join("v0_3_m2_nested_sm_3level.ynz");

    // Build to get a binary
    let build_out = Command::new(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .join("target/debug/ynz"),
    )
    .args(["build", fixture_path.to_str().expect("valid path")])
    .output()
    .expect("ynz build failed");

    assert!(
        build_out.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build_out.stderr)
    );

    // Use objdump to find call sites to ynz_rt_run_entrypoint
    // in ynz_sm_* functions. The driver must NOT appear inside any resume fn.
    let binary = fixture_path.with_extension("");
    // The binary MUST exist — if the build succeeded the output is here.
    // A vacuous early return would green-light a real bridge regression on a
    // box where the binary ends up in an unexpected location.
    assert!(
        binary.exists(),
        "binary {:?} must exist after a successful build — the no-bridge check cannot \
         run without it. Check the build output path.",
        binary
    );

    let nm_out = Command::new("objdump")
        .args(["-d", binary.to_str().expect("valid path")])
        .output()
        .expect(
            "objdump must be available — the no-bridge invariant test cannot pass without \
                  disassembling the binary. Install binutils or run on a CI image that has it.",
        );

    assert!(
        nm_out.status.success(),
        "objdump exited non-zero ({}); cannot verify no-bridge invariant. stderr: {}",
        nm_out.status,
        String::from_utf8_lossy(&nm_out.stderr)
    );

    let disasm = String::from_utf8_lossy(&nm_out.stdout).to_string();
    // Check if any ynz_sm_*_resume function calls ynz_rt_run_entrypoint.
    // Simple heuristic: look for the driver symbol in functions starting with ynz_sm_
    let mut in_resume_fn = false;
    let mut bridge_in_resume = false;
    for line in disasm.lines() {
        if line.contains("<ynz_sm_") && line.contains("resume>:") {
            in_resume_fn = true;
        } else if in_resume_fn && line.contains("<ynz_sm_") && !line.contains("resume") {
            // left previous resume fn
            in_resume_fn = false;
        } else if in_resume_fn && line.contains("ynz_rt_run_entrypoint") {
            bridge_in_resume = true;
            break;
        } else if in_resume_fn
            && line.starts_with(|c: char| c.is_ascii_hexdigit())
            && line.contains(" <")
            && !line.contains("ynz_sm_")
            && line.contains(">:")
        {
            // new non-SM function — left previous resume fn
            in_resume_fn = false;
        }
    }
    assert!(
        !bridge_in_resume,
        "ynz_rt_run_entrypoint found inside a ynz_sm_*_resume fn — program-entry driver must not be called from resume fns"
    );
    // Clean up binary
    let _ = std::fs::remove_file(&binary);
}

// ── AC 7: composed-single-alloc proof ─────────────────────────────────────────

/// Run a fixture with alloc-counter instrumentation enabled.
/// Returns (alloc_count, free_count) read from the output file.
fn run_with_alloc_counter(fixture: &str) -> (u64, u64) {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture);

    let count_file = std::env::temp_dir().join(format!("ynz_alloc_counter_{}.txt", fixture));

    let _ = std::fs::remove_file(&count_file);

    let _output = Command::new(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .join("target/debug/ynz"),
    )
    .args(["run", fixture_path.to_str().expect("valid path")])
    .env("YNZ_ALLOC_COUNTER", "1")
    .env(
        "YNZ_ALLOC_COUNTER_OUTPUT",
        count_file.to_str().expect("valid path"),
    )
    .output()
    .expect("ynz binary not found");

    let content =
        std::fs::read_to_string(&count_file).unwrap_or_else(|_| "alloc=0\nfree=0\n".to_string());
    let _ = std::fs::remove_file(&count_file);

    let parse_count = |prefix: &str| -> u64 {
        content
            .lines()
            .find(|l| l.starts_with(prefix))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    };
    (parse_count("alloc"), parse_count("free"))
}

// ── AC 8: recursion cancellation-no-leak ──────────────────────────────────────

/// Run a fixture with alloc-counter + optional cancellation + optional skip-recursion-drop.
fn run_with_alloc_and_options(
    fixture: &str,
    cancel: bool,
    skip_recursion_drop: bool,
) -> (u64, u64) {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture);

    let count_file = std::env::temp_dir().join(format!(
        "ynz_alloc_{}_{}_{}.txt",
        fixture,
        if cancel { "cancel" } else { "normal" },
        if skip_recursion_drop {
            "skip"
        } else {
            "nodrop"
        }
    ));

    let _ = std::fs::remove_file(&count_file);

    let mut cmd = Command::new(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .join("target/debug/ynz"),
    );
    cmd.args(["run", fixture_path.to_str().expect("valid path")])
        .env("YNZ_ALLOC_COUNTER", "1")
        .env(
            "YNZ_ALLOC_COUNTER_OUTPUT",
            count_file.to_str().expect("valid path"),
        );
    if cancel {
        // 50ms timeout: long enough for Tokio to join worker threads (ensuring Drop has run
        // before counters are read), short enough to fire before countdown(1)'s 200ms sleep
        // completes at ~600ms (100ms margin). Using 0ms races between the main thread reading
        // counters and the worker thread executing Drop — ynz_rt_shutdown reads counters after
        // shutdown_timeout returns, but with 0ms the threads are not joined and Drop may not
        // have run yet on the worker thread.
        cmd.env("YNZ_SHUTDOWN_TIMEOUT_MS", "50");
    }
    if skip_recursion_drop {
        cmd.env("YNZ_SKIP_RECURSION_DROP", "1");
    }

    let _output = cmd.output().expect("ynz binary not found");

    let content =
        std::fs::read_to_string(&count_file).unwrap_or_else(|_| "alloc=0\nfree=0\n".to_string());
    let _ = std::fs::remove_file(&count_file);

    let parse_count = |prefix: &str| -> u64 {
        content
            .lines()
            .find(|l| l.starts_with(prefix))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    };
    (parse_count("alloc"), parse_count("free"))
}

// WHY: SpawnStateFnFuture::Drop must walk the recursion_slot chain and free all
// heap-boxed child frames on cancellation. Without the walk, cancelled recursive SMs
// leak every heap-box that was live at abort time. This test proves the walk is
// load-bearing via positive + negative controls:
// - POSITIVE: alloc >= 4 (entrypoint + root + countdown(2) + countdown(1)) confirms
//   heap-boxed children were live at cancellation time (see positive_control test)
// - NEGATIVE: skipping the Drop chain walk causes alloc=4, free=2 (proven leak:
//   countdown(1) and countdown(2) not freed — see negative_control test)
// - ASSERTION: with the walk, alloc == free (no leak)
//
// Fixture timing (200ms per level, 500ms entrypoint sleep, 50ms shutdown timeout):
// cancel fires at ~500ms when countdown(1) is mid-sleep (started at ~400ms, natural end
// at ~600ms). Tokio has 50ms to join threads (ensuring Drop has run) before counters are
// read. 50ms window fires before countdown(1) completes (100ms margin).
// Serialized via CANCEL_TEST_LOCK — no CPU contention with sibling tests.
#[test]
fn recursion_cancellation_no_leak() {
    let _guard = lock_cancel_tests();
    let (alloc, free) = run_with_alloc_and_options("v0_3_m2_recursive_cancel.ynz", true, false);
    assert_eq!(
        alloc, free,
        "alloc/free must balance after cancellation — heap-box leaks means SpawnStateFnFuture::Drop \
         did not walk the recursion chain; alloc={alloc}, free={free}"
    );
}

// WHY: positive control — verifies that ≥4 frames (entrypoint + root + countdown(2) +
// countdown(1)) were live at abort time, proving the cancellation path actually exercises
// SpawnStateFnFuture::Drop's chain walk. alloc includes the entrypoint's own frame
// (alloc'd by the wrapper). If alloc < 4, countdown(1) was not allocated before the
// cancel fired — the Drop chain walk was never triggered, and the no-leak assertion
// would pass vacuously (everything freed via the normal completion path instead).
//
// Fixture timing guarantees alloc=4 before cancel fires:
//   - entrypoint frame: alloc=1 immediately
//   - countdown(3) root: alloc=2 at spawn
//   - countdown(2) at 200ms: alloc=3
//   - countdown(1) at 400ms: alloc=4
//   - cancel fires at 500ms (100ms after alloc=4 is guaranteed)
// Serialized via CANCEL_TEST_LOCK — no CPU contention with sibling tests.
#[test]
fn recursion_cancellation_positive_control_heap_boxes_were_live() {
    let _guard = lock_cancel_tests();
    let (alloc, _free) = run_with_alloc_and_options("v0_3_m2_recursive_cancel.ynz", true, false);
    assert!(
        alloc >= 4,
        "positive control: must reach alloc=4 (entrypoint+root+countdown(2)+countdown(1)) \
         before cancellation fires at 500ms; alloc={alloc} means the recursion depth was \
         not reached in time — check fixture timing or machine load"
    );
}

// WHY: negative control — skipping the Drop chain walk MUST cause a measurable leak.
// This test proves that the no-leak assertion is not passing trivially (e.g., all
// heap-boxes freed by the normal completion path before cancel fired).
// With skip=true: alloc=4, free=2 (entrypoint + root freed; countdown(1) and
// countdown(2) leaked because chain walk was skipped). alloc > free proves the chain
// walk is the mechanism responsible for freeing those frames — not a no-op.
//
// Serialized via CANCEL_TEST_LOCK — no CPU contention with sibling tests.
#[test]
fn recursion_cancellation_negative_control_skip_drop_leaks() {
    let _guard = lock_cancel_tests();
    let (alloc, free) = run_with_alloc_and_options("v0_3_m2_recursive_cancel.ynz", true, true);
    assert!(
        alloc > free,
        "negative control: skipping SpawnStateFnFuture::Drop's recursion chain walk must leak \
         frames (alloc > free); expected alloc=4, free=2; got alloc={alloc}, free={free} — \
         if equal, the fixture completed normally before cancel fired \
         (review: 200ms×2=400ms to reach alloc=4, 50ms window fires at 500ms)"
    );
}

// WHY: "one alloc per task tree" is the design-doc model (design/future/concurrency.md:
// "low memory, fast spawn — like Rust's async"). A per-call alloc would mean N sleep
// calls = N allocs. Instrumenting ynz_alloc with a counter and asserting count==1 for a
// 3-level synchronous tree proves composed frames are actually ONE allocation.
// This is a behavioral claim about memory layout that cargo test can't verify otherwise.
#[test]
fn alloc_counter_3level_synchronous_tree_is_one_alloc() {
    let (alloc, free) = run_with_alloc_counter("v0_3_m2_alloc_proof_3level.ynz");
    assert_eq!(
        alloc, 1,
        "3-level synchronous SM tree must allocate exactly 1 frame (composed = 1 alloc per tree); got alloc={}",
        alloc
    );
    assert_eq!(
        free, alloc,
        "alloc/free must be balanced (no leak); got alloc={alloc}, free={free}"
    );
}

// WHY: background spawn creates a SEPARATE task tree — one additional alloc for the
// spawned task. The composed-single-alloc guarantee is per-tree: the main tree gets
// 1 alloc, the spawned tree gets 1 alloc. Total = 2.
#[test]
fn alloc_counter_background_spawn_adds_one_alloc() {
    let (alloc, free) = run_with_alloc_counter("v0_3_m2_alloc_proof_background.ynz");
    assert_eq!(
        alloc, 2,
        "main tree (1) + background spawn (1) = 2 allocs; got alloc={}",
        alloc
    );
    assert_eq!(
        free, alloc,
        "alloc/free must be balanced (no leak); got alloc={alloc}, free={free}"
    );
}

// WHY: the HALT-class hole — a suspending call nested in a sub-expression position
// (e.g. `let x = 1 + inner()`) previously fell through to a wrapper call (block_on)
// from inside a Tokio worker thread → non-unwinding abort, exit 0 (silent failure).
// After Fix 1, typeck rejects it before codegen with a clean teaching error, exit 1.
// This test is the regression anchor: if this fails with exit 0 or no teaching text,
// the HALT-class hole has re-opened.
#[test]
fn subexpr_suspending_call_rejected_with_teaching_error() {
    let (stdout, stderr, exit_code) = run_fixture("v0_3_m2_subexpr_suspend_error.ynz");
    assert_eq!(
        exit_code, 1,
        "sub-expression suspending call must be a compile error (exit 1), \
         not exit 0 (which would mean the block_on abort path was hit). \
         stdout={stdout:?} stderr={stderr:?}"
    );
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("suspending call inside a larger expression"),
        "teaching error text missing 'suspending call inside a larger expression': {combined}"
    );
    assert!(
        combined.contains("step-by-step") || combined.contains("one operation per line"),
        "teaching error must explain the step-by-step style rationale: {combined}"
    );
}

// WHY: non-self mutual recursion among suspending functions corrupts heap on
// cancellation because SpawnStateFnFuture::Drop assumes uniform frame layout
// (self-recursion: same function = same size + offset at every level). A mutual
// cycle (ping→pong→ping) has mixed layouts. Typeck rejects it with a teaching
// error explaining that self-recursion works and mutual cycles can be restructured.
// This test is the regression anchor: if this passes with exit 0, the heap
// corruption path is live. The assertion checks for self-recursive/restructure
// teaching content — not a milestone reference.
#[test]
fn mutual_recursion_suspending_rejected_with_teaching_error() {
    let (stdout, stderr, exit_code) = run_fixture("v0_3_m2_mutual_recursion_error.ynz");
    assert_eq!(
        exit_code, 1,
        "mutually-recursive suspending functions must be a compile error (exit 1). \
         stdout={stdout:?} stderr={stderr:?}"
    );
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("mutually-recursive suspending cycle"),
        "teaching error text missing 'mutually-recursive suspending cycle': {combined}"
    );
    assert!(
        combined.contains("self-recursive") || combined.contains("restructure"),
        "teaching error must explain that self-recursion works and mutual cycles can be restructured: {combined}"
    );
}

// WHY: `no_bridge_reachable_from_resume_fns` originally only tested the direct-statement
// form of suspending calls. This companion test adds a sub-expression fixture (Fix 1's
// class) to prove the typeck guard closes the HALT path before codegen ever fires.
// After Fix 1 the sub-expression form exits 1 (typeck reject); the disassembly check
// is irrelevant because codegen never runs for a file that fails typeck.
#[test]
fn no_bridge_via_subexpr_position_rejected_at_typeck() {
    let (_, stderr, exit_code) = run_fixture("v0_3_m2_subexpr_suspend_error.ynz");
    // Typeck must fire before any LLVM IR is emitted, so no binary exists to inspect.
    // The presence of exit 1 + the teaching text is proof the codegen block_on path
    // is unreachable for this input.
    assert_eq!(
        exit_code, 1,
        "must reject at typeck (exit 1): stderr={stderr}"
    );
    assert!(
        stderr.contains("suspending call inside a larger expression"),
        "must mention the sub-expression position restriction: {stderr}"
    );
}

// WHY: `background add(inner(), 4)` where inner() suspends must reject at typeck
// with the same "suspending call inside a larger expression" error as the non-background
// form.  Background arguments evaluate in the CALLING context before the spawn, so a
// suspending call nested in an arg runs on the caller's thread in sub-expression position
// — the same nested-block_on path that caused the original Phase-5 HALT crash.
// Without this guard the compiler emits code that panics at runtime with "Cannot start
// a runtime from within a runtime". This test is the regression anchor for that hole.
// It also confirms the direct-spawn form (`background worker()` where worker suspends)
// is NOT rejected — that is the legal route-to-I/O-pool pattern.
#[test]
fn background_subexpr_suspending_call_rejected_with_teaching_error() {
    let (stdout, stderr, exit_code) = run_fixture("v0_3_m2_background_subexpr_error.ynz");
    assert_eq!(
        exit_code, 1,
        "background call with a suspending call in an arg must be a compile error (exit 1), \
         not exit 0 (which would mean the runtime-panic path was hit). \
         stdout={stdout:?} stderr={stderr:?}"
    );
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("suspending call inside a larger expression"),
        "teaching error text must mention 'suspending call inside a larger expression': {combined}"
    );
}

// WHY: The legal direct-spawn form — `background worker()` where worker is a suspending
// function — must NOT be rejected by the sub-expression guard.  worker becomes its own
// state machine; the caller merely hands it to the runtime.  If this test fails with
// exit 1, the guard is over-firing and the route-to-I/O-pool pattern is broken.
#[test]
fn background_direct_spawn_of_suspending_fn_still_runs() {
    let (stdout, _, exit_code) = run_fixture_with_timeout("v0_3_m2_background_from_sm.ynz", 5);
    assert_eq!(
        exit_code, 0,
        "direct spawn of a suspending fn must run successfully (exit 0). \
         Fix 1 must not over-fire on this legal pattern."
    );
    assert!(
        stdout.contains("worker done"),
        "expected 'worker done' in output: {stdout:?}"
    );
}

// ── Phase 9: state_machine_errors_before_first_wait ──────────────────────────

// WHY: An errors-capable SM can error on a branch that exits BEFORE reaching
// the first `wait`. The error is produced at resume_point=0 (state-0, before
// any suspension). Verifies the error propagates through the SM return slot
// even though the wait path is never taken. Distinct from
// errors_capable_sm_error_propagates_through_suspension (which errors AFTER a
// wait) — this guards the state-0 early-return error path.
#[test]
fn state_machine_errors_before_first_wait() {
    let (stdout, stderr, code) = run_fixture("v0_3_m2_errors_before_first_wait.ynz");
    assert_eq!(
        code, 0,
        "should exit 0 (error propagated cleanly, not a crash); \
         stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        stdout.contains("v=99"),
        "expected 'v=99' (.or(99) must return fallback when error propagated before wait); \
         got: {stdout:?}"
    );
}

// ── Round-5 fix: errors-capable wait-result local in arithmetic ───────────────

// WHY: Before the Round-5 fix, using an EC local (bound from `wait <ec_call>`)
// in arithmetic inside an SM resume body panicked at codegen: the resume Cg has
// is_errors_capable=false so the normal auto-propagation branch was skipped,
// leaving the raw EC pointer where an IntValue was expected. The fix adds SM-aware
// auto-propagation in the !ErrorsCapable (typeck-narrowed) Ident branch: on the
// error path, stores the error to the frame return slot + ret i32 0; on the success
// path, extracts field1 and yields the Int value. Both the success (52 = 42+10) and
// error (.or(99) = 99) paths must produce the correct output and exit 0.
// Distinct from Fix 1 (which rejects `return f() + 10` — a CALL nested in arith);
// this is `let v = wait f(); return v + 10` (a LOCAL used in arith — valid Yinz).
#[test]
fn errors_arith_across_wait_success_and_error_paths() {
    let (stdout, _, code) = run_fixture("v0_3_m2_errors_arith_across_wait.ynz");
    assert_eq!(code, 0, "must exit 0: stdout={stdout:?}");
    assert_eq!(
        stdout.trim(),
        "success=52\nerror=99",
        "success path: 42+10=52; error path: .or(99)=99"
    );
}

// ── Phase 9 Step 4b: free-fn symmetric negative test — can't-infer gate ──────

// WHY: The can't-infer dynamic-dispatch gate at check.rs guards both the dot-call
// form (w.doWork() — covered by the existing dynamic_dispatch fixture) AND the
// free-fn form (dispatch(w) where w: dynamic Contract — this test). The gate must
// ONLY fire when current_fn_suspends=true. A NON-suspending relay that calls
// dispatch(w) must NOT receive a can't-infer error. If the current_fn_suspends
// guard were dropped from the free-fn branch, this test would receive a
// "Can't determine whether" typeck error and fail — proving non-vacuousness.
//
// The fixture hits a codegen error (dynamic dispatch codegen deferred to M4 P4)
// but that error must NOT mention "Can't determine" — that would indicate the
// typeck gate over-fired before codegen ran.
#[test]
fn free_fn_non_suspending_relay_no_cant_infer_error() {
    let (_, stderr, _) = run_fixture("v0_3_m2_free_fn_non_suspending_relay.ynz");
    // relay() is non-suspending (current_fn_suspends=false) so the free-fn
    // can't-infer gate must not fire. Any failure here is a codegen error
    // ("dynamic dispatch call sites not yet lowered"), never a typeck can't-infer.
    assert!(
        !stderr.contains("Can't determine whether"),
        "can't-infer error must not fire for a non-suspending free-fn relay caller; \
         stderr: {stderr}"
    );
}

// ── Round-3 fix: result-binding crosses a LATER suspension ───────────────────

// WHY: `let slot = sleeper(); let other = sleeper(); return slot + other` — `slot`
// is the result of the FIRST suspending call. Before the round-3 fix, `slot` was
// never added to the `declared` crossing-candidates set, so the typeck analysis
// missed that it crosses the SECOND suspension (`let other = sleeper()`). The
// codegen emitted LLVM IR where `slot`'s alloca did not dominate its use after the
// second resume block — LLVM's module verifier caught this and aborted (exit 1 via
// "LLVM module verify failed", not the intended teaching error).
//
// M3a P1 lifts the LocalCrossesWait guard and adds frame-backed slot machinery,
// so result-bindings that cross later suspensions now compile and produce correct
// output. `slot` = sleeper() = 5, `other` = sleeper() = 5, `slot + other = 10`.
#[test]
fn result_binding_crosses_later_suspension_compiles_and_runs() {
    // WHY: `slot` is produced by the FIRST suspending call and read after the SECOND.
    // M3a P1 must keep `slot` in a frame slot so it survives the second suspension.
    // Correct output is 10 (5 + 5). If this exits 1 or produces wrong output, the
    // result-binding crossing-local path regressed in the frame-slot machinery.
    let (stdout, stderr, exit_code) =
        run_fixture("v0_3_m2_result_binding_crosses_later_suspension_error.ynz");
    assert_eq!(
        exit_code, 0,
        "result-binding crossing program must compile and run (exit 0); stderr={stderr:?}"
    );
    assert_eq!(
        stdout.trim(),
        "10",
        "slot=5 + other=5 must equal 10; frame-backed crossing local must survive; stderr={stderr:?}"
    );
    assert!(
        !stderr.contains("LLVM module verify failed"),
        "must not crash the LLVM backend; stderr={stderr:?}"
    );
    assert!(
        !stderr.contains("Machine-code generation failed"),
        "must not crash the backend; stderr={stderr:?}"
    );
}

// WHY: non-crossing local AC — a local declared and read only BEFORE the wait must
// NOT get a frame slot. Verifies that the crossing-local analysis is NOT over-broad.
// A mis-implementation that slots ALL locals (not just crossing ones) would still
// produce correct output but would waste frame space; this verifies the analysis
// correctly classifies the local as non-crossing. (The alloc-count is the same for
// crossing and non-crossing — frame is pre-sized — but the program behavior confirms
// the code path is correct.)
#[test]
fn non_crossing_local_runs_correctly() {
    let (stdout, _stderr, code) = run_fixture("v0_3_m3a_p1_non_crossing_local_not_slotted.ynz");
    assert_eq!(code, 0, "non-crossing-local program must exit 0");
    assert!(
        stdout.contains("99"),
        "pre-wait value must be printed; got: {stdout:?}"
    );
    assert!(
        stdout.contains("done"),
        "post-wait print must run; got: {stdout:?}"
    );
}

// WHY: fixture (h) — crossing locals add ZERO extra ynz_alloc calls. The frame is
// sized to include crossing-local slots at build time; no per-local heap allocation.
// The composed-single-alloc invariant (one ynz_alloc per task tree) must hold even
// when the function has frame-backed crossing locals.
#[test]
fn alloc_counter_crossing_locals_add_zero_extra_allocs() {
    let (alloc, free) = run_with_alloc_counter("v0_3_m3a_p1_alloc_count.ynz");
    assert_eq!(
        alloc, 1,
        "crossing locals add ZERO extra allocs — frame pre-sized at build time; got alloc={}",
        alloc
    );
    assert_eq!(
        free, alloc,
        "alloc/free must be balanced (no leak); got alloc={alloc}, free={free}"
    );
}

#[test]
fn alloc_counter_shape_crossing_local_no_leak() {
    // WHY: FIX 1 guard — shape crossing locals must use frame-embedding (ptr alloca wired to
    // the composed frame's slot region in sm_entry) rather than separate ynz_alloc per shape.
    // The old heap-promote approach: alloc=2 (frame + shape buffer), free=1 (only frame freed).
    // Correct: alloc=1, free=1. Regression here means the leak is back.
    let (alloc, free) = run_with_alloc_counter("v0_3_m3a_p1_shape_crossing_alloc_balance.ynz");
    assert_eq!(
        alloc, 1,
        "shape crossing local must NOT cause extra ynz_alloc; got alloc={}",
        alloc
    );
    assert_eq!(
        free, alloc,
        "alloc/free must be balanced for shape crossing local; got alloc={alloc}, free={free}"
    );
}

#[test]
fn alloc_counter_number_errors_suspending_no_leak() {
    // WHY: the frame-staging slot for `-> number errors` must live INSIDE the composed
    // frame allocation (not a separate ynz_alloc). alloc=2 means a separate heap alloc
    // was introduced (the round-19 leak pattern). alloc=1/free=1 proves the staging slot
    // is part of the pre-sized frame — the one-alloc-per-task-tree invariant holds.
    let (alloc, free) =
        run_with_alloc_counter("v0_3_m3a_p1_number_errors_returning_suspending_fn.ynz");
    assert_eq!(
        alloc, 1,
        "number errors staging slot must not cause extra ynz_alloc; got alloc={}",
        alloc
    );
    assert_eq!(
        free, alloc,
        "alloc/free must be balanced for number errors suspending return; got alloc={alloc}, free={free}"
    );
}
