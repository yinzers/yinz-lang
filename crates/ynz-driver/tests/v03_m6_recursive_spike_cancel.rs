//! Recursion chain x spike CPU-handle cleanup on cancellation.
//!
//! `SpawnStateFnFuture::drop` ran `cleanup_spike_cpu_handles` on the ROOT frame only, while the
//! recursion-chain drop walk freed each chain child's sleep handle and frame but never its spike
//! CPU handles. A chain child cancelled while parked at its CPU join leaked its boxed
//! `CpuJoinHandle`s (before the fix `handle_alloc=4, handle_free=2`).
//!
//! Observability: the `Box<CpuJoinHandle>` lives on the RUST allocator, which the
//! `ynz_alloc`/`ynz_free` counter pair never sees, so the test reads the `handle_alloc=` and
//! `handle_free=` lines the runtime appends to the counter file (same `YNZ_ALLOC_COUNTER` gate as
//! the frame counters).
//!
//! The cancel timing is set by the fixture (`fixtures/v0_3_m6_recursive_spike_cancel.ynz`
//! timeline, +-300ms margins around the 1000ms cancel). A flake here is timing-margin drift to
//! fix by widening the fixture's burn/cancel windows, never by loosening an assertion; the
//! timing-independent proof of the chain-child handle cleanup is the ynz-runtime unit test
//! `recursion_chain_child_spike_handles_freed_on_drop`.

use std::path::PathBuf;
use std::process::Command;

/// The four counters the runtime dumps at shutdown: frame alloc/free (ynz allocator) and
/// CpuJoinHandle alloc/free (Rust allocator).
struct Counters {
    alloc: u64,
    free: u64,
    handle_alloc: u64,
    handle_free: u64,
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A counter the runtime wrote, matched on the whole `key=` prefix so `free=` never matches
/// `handle_free=`. Panics when absent: a silent `0` would turn "the counter did not report" into
/// "the program balanced".
fn counter_value(file_text: &str, prefix: &str) -> u64 {
    file_text
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("counter `{prefix}` missing from:\n{file_text}"))
        .trim()
        .parse()
        .unwrap_or_else(|err| panic!("counter `{prefix}` is not a number ({err}) in:\n{file_text}"))
}

/// Run the recursive-spike-cancel fixture with the alloc counter enabled.
///
/// `YNZ_SHUTDOWN_TIMEOUT_MS=900` gives the worker threads room to run
/// `SpawnStateFnFuture::drop` before the counters are read, and the child's two in-flight
/// `burn()` blocking-pool tasks (~600ms each, finishing ~1300ms) time to finish and free their
/// ctx copies. Generous is safe: nothing polls the dropped future after cancellation, so a longer
/// window cannot turn a leak into a false pass.
fn run_recursive_spike_cancel() -> Counters {
    let count_file = std::env::temp_dir().join(format!(
        "ynz_handle_count_recursive_spike_{}.txt",
        std::process::id(),
    ));
    let _ = std::fs::remove_file(&count_file);

    let output = Command::new(env!("CARGO_BIN_EXE_ynz"))
        .args([
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
        .env("YNZ_SHUTDOWN_TIMEOUT_MS", "900")
        .output()
        .expect("spawn `ynz run`");

    let content = std::fs::read_to_string(&count_file).unwrap_or_else(|err| {
        panic!(
            "counter file {} not written ({err}); exit {:?}, stderr:\n{}",
            count_file.display(),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    let _ = std::fs::remove_file(&count_file);
    Counters {
        alloc: counter_value(&content, "alloc="),
        free: counter_value(&content, "free="),
        handle_alloc: counter_value(&content, "handle_alloc="),
        handle_free: counter_value(&content, "handle_free="),
    }
}

// bug: a recursion-chain child cancelled while parked at its CPU join leaks its boxed CpuJoinHandles (or its frame) because the chain drop walk skips the spike-handle cleanup | tier: High
#[test]
fn recursive_spike_cancellation_frees_chain_child_cpu_handles() {
    let counters = run_recursive_spike_cancel();
    // Non-vacuity: root group (2) + chain-child group (2) were spawned before the cancel; fewer
    // means the nested group was not admitted or the cancel fired first, and the parity checks
    // below would pass without exercising the chain walk.
    assert!(
        counters.handle_alloc >= 4,
        "must reach handle_alloc>=4 (root group 2 + chain child group 2) before cancellation \
         fires at ~1000ms; handle_alloc={} means the nested group was not admitted or the chain \
         child had not spawned its group in time — check the admission shape or machine load",
        counters.handle_alloc
    );
    assert_eq!(
        counters.handle_alloc, counters.handle_free,
        "CpuJoinHandle alloc/free must balance after cancellation: a chain child parked at its \
         CPU join leaked its boxed handles; handle_alloc={}, handle_free={}",
        counters.handle_alloc, counters.handle_free
    );
    assert_eq!(
        counters.alloc, counters.free,
        "frame alloc/free must balance after cancellation (the chain walk frees each child's \
         frame, which the handle fix must not disturb); alloc={}, free={}",
        counters.alloc, counters.free
    );
}
