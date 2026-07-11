//! v0.3-M6 Phase 3b (FRAGO 001 / P2-5): recursion-chain × spike CPU-handle cleanup leak,
//! RED→GREEN class.
//!
//! Authored BEFORE the fix (RED-repro-before-fix per verification.md): each test asserts
//! the CORRECT behavior and fails RED on the pre-fix tree for the documented reason —
//! `SpawnStateFnFuture::drop` runs `cleanup_spike_cpu_handles` on the ROOT frame only,
//! while the recursion-chain drop walk frees each chain child's sleep handle + frame but
//! never its spike CPU handles. A chain child cancelled while parked at its CPU join
//! therefore leaks its boxed `CpuJoinHandle`s. Pre-fix signature: `handle_alloc=4,
//! handle_free=2` (the chain child's 2 boxed handles leak).
//!
//! Observability: the `Box<CpuJoinHandle>` lives on the RUST allocator, which the
//! `ynz_alloc`/`ynz_free` counter pair never sees — so these tests read the env-gated
//! `handle_alloc=`/`handle_free=` lines the runtime appends to the counter file (same
//! `YNZ_ALLOC_COUNTER` gate as the frame counters).
//!
//! The cancel-timing tests are serialized via a file-local mutex (mirroring
//! `m2_state_machine_integration.rs`'s `CANCEL_TEST_LOCK`) so CPU contention between
//! sibling tests cannot skew the fixture's timing windows.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

/// All four counters the runtime dumps at shutdown: frame alloc/free (ynz allocator)
/// and CpuJoinHandle alloc/free (Rust allocator, handle parity).
struct Counters {
    alloc: u64,
    free: u64,
    handle_alloc: u64,
    handle_free: u64,
}

static CANCEL_TEST_LOCK: Mutex<()> = Mutex::new(());

fn lock_cancel_tests() -> MutexGuard<'static, ()> {
    match CANCEL_TEST_LOCK.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Run the recursive-spike-cancel fixture with the alloc counter enabled and return all
/// four counters. `skip_recursion_drop` arms the negative control (`YNZ_SKIP_RECURSION_DROP`).
///
/// `YNZ_SHUTDOWN_TIMEOUT_MS=200`: generous margin for the worker threads to run
/// `SpawnStateFnFuture::drop` before the counters are read. Generous is SAFE here:
/// nothing polls the dropped future after cancellation, so the chain child's handles
/// stay live-in-frame (leaked or cleaned, per the code under test) regardless of the
/// timeout length — a longer window cannot turn a leak into a false pass.
fn run_recursive_spike_cancel(skip_recursion_drop: bool) -> Counters {
    let count_file = std::env::temp_dir().join(format!(
        "ynz_handle_count_recursive_spike_{}_{}.txt",
        if skip_recursion_drop { "skip" } else { "walk" },
        std::process::id(),
    ));
    let _ = std::fs::remove_file(&count_file);

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ynz"));
    cmd.args([
        "run",
        fixture("v0_3_m6_recursive_spike_cancel.ynz")
            .to_str()
            .expect("valid path"),
    ])
    .env("YNZ_ALLOC_COUNTER", "1")
    .env(
        "YNZ_ALLOC_COUNTER_OUTPUT",
        count_file.to_str().expect("valid path"),
    )
    .env("YNZ_SHUTDOWN_TIMEOUT_MS", "200");
    if skip_recursion_drop {
        cmd.env("YNZ_SKIP_RECURSION_DROP", "1");
    }

    let _output = cmd.output().expect("ynz binary not found");

    let content = std::fs::read_to_string(&count_file)
        .unwrap_or_else(|_| "alloc=0\nfree=0\nhandle_alloc=0\nhandle_free=0\n".to_string());
    let _ = std::fs::remove_file(&count_file);

    // Full "prefix=" match: "handle_free=" never collides with "free=" and vice versa.
    let parse_count = |prefix: &str| -> u64 {
        content
            .lines()
            .find(|l| l.starts_with(prefix))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    };
    Counters {
        alloc: parse_count("alloc="),
        free: parse_count("free="),
        handle_alloc: parse_count("handle_alloc="),
        handle_free: parse_count("handle_free="),
    }
}

// Timing triage: the cancel-timing tests below ride the fixture's wall-clock margins
// (±300ms around the 1000ms cancel — see fixtures/v0_3_m6_recursive_spike_cancel.ynz's
// timeline) and carry a low-probability flake risk: under extreme load skew a chain
// child's CPU group can complete naturally before the cancel fires, collapsing the
// parked-at-join window the assertions depend on. Repeated CI flakes on these tests
// should be triaged as timing-margin drift (widen the fixture's burn/cancel windows),
// not dismissed — the deterministic unit test
// `recursion_chain_child_spike_handles_freed_on_drop` (ynz-runtime lib tests) is the
// primary timing-independent proof of the chain-child handle cleanup.

// WHY: THE RED→GREEN gate for P2-5. A recursion-chain child cancelled while parked at its
// CPU join must have its boxed CpuJoinHandles freed by SpawnStateFnFuture::drop's chain
// walk through the SAME cleanup_spike_cpu_handles choke point the root frame uses.
// Pre-fix: handle_alloc=4, handle_free=2 — the chain child's 2 handles leak because the
// walk frees only the child's sleep handle + frame.
#[test]
fn recursive_spike_cancellation_frees_chain_child_cpu_handles() {
    let _guard = lock_cancel_tests();
    let c = run_recursive_spike_cancel(false);
    assert_eq!(
        c.handle_alloc, c.handle_free,
        "CpuJoinHandle alloc/free must balance after cancellation — a chain child parked \
         at its CPU join leaked its boxed handles (the recursion-chain drop walk did not \
         run cleanup_spike_cpu_handles on the child frame); handle_alloc={}, handle_free={}",
        c.handle_alloc, c.handle_free
    );
}

// WHY: positive control — proves the nested branch-arm CPU group in a zero-param
// self-recursive suspending host is genuinely ADMITTED by the compiler (the Phase-0
// reachability claim) AND that a chain child spawned its own group before the cancel
// fired. handle_alloc >= 4 = root group (2) + chain child group (2). If this fails,
// either the admission shape regressed (group declined → handles never allocated) or
// the cancel fired before the chain child spawned — both falsify the repro's premise,
// and the parity test above would pass vacuously.
#[test]
fn recursive_spike_cancellation_positive_control_chain_child_group_spawned() {
    let _guard = lock_cancel_tests();
    let c = run_recursive_spike_cancel(false);
    assert!(
        c.handle_alloc >= 4,
        "positive control: must reach handle_alloc>=4 (root group 2 + chain child group 2) \
         before cancellation fires at ~1000ms; handle_alloc={} means the nested group was \
         not admitted or the chain child had not spawned its group in time — check the \
         admission shape or fixture timing/machine load",
        c.handle_alloc
    );
}

// WHY: negative control — durable non-vacuity witness. Skipping the recursion-chain drop
// walk (YNZ_SKIP_RECURSION_DROP=1) must produce a measurable handle leak EVEN AFTER the
// fix, because the chain-child cleanup lives inside the walk. handle_alloc > handle_free
// proves the walk is the mechanism responsible for freeing the child's handles — the
// parity test above is not passing trivially (e.g. all handles freed by Ready polls
// before the cancel fired).
#[test]
fn recursive_spike_cancellation_negative_control_skip_walk_leaks_handles() {
    let _guard = lock_cancel_tests();
    let c = run_recursive_spike_cancel(true);
    assert!(
        c.handle_alloc > c.handle_free,
        "negative control: skipping the recursion-chain drop walk must leak the chain \
         child's CpuJoinHandles (handle_alloc > handle_free); expected handle_alloc=4, \
         handle_free=2; got handle_alloc={}, handle_free={} — the leak witness vanished, \
         so the parity test may be passing vacuously",
        c.handle_alloc,
        c.handle_free
    );
}

// WHY: frame-parity regression guard (already GREEN pre-fix). The ynz-allocator frame
// counters must stay balanced on this cancellation path: the chain walk frees each
// child's frame, and the handle-cleanup fix must not disturb that. A frame imbalance
// here is a NEW regression, distinct from the handle leak (which the ynz counter
// cannot see).
#[test]
fn recursive_spike_cancellation_frame_alloc_free_parity() {
    let _guard = lock_cancel_tests();
    let c = run_recursive_spike_cancel(false);
    assert_eq!(
        c.alloc, c.free,
        "frame alloc/free must balance after cancellation (regression guard — this \
         parity was green before the handle fix); alloc={}, free={}",
        c.alloc, c.free
    );
}
