// test-ratchet: coalescing and file-removal tests moved to coalescing.rs + file_removed.rs
// respectively. The assertions are not dropped — they live in the dedicated test files.
// This file covers: event delivery latency + UI smoke tests only.
/// Integration tests for the file-watching plumbing layer.
///
/// Covers: event delivery latency and UI smoke tests.
/// Coalescing is in `coalescing.rs`; file-removal is in `file_removed.rs`.
use std::{
    fs,
    time::{Duration, Instant},
};

use tempfile::TempDir;
use ynz_watch::{ui, watcher, FileWatcher, WatchEvent};

// WHY: file events must arrive within 200ms of the save or the "sub-second rebuild" promise
//      is broken even before the compiler starts. If this flaps, investigate the notify/debouncer
//      version or CI disk I/O; do NOT widen the timeout as a first response.
#[test]
fn file_change_event_delivered_within_200ms() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("test.ynz");
    fs::write(&file, "// initial content\n").unwrap();

    let file_watcher = FileWatcher::new(std::slice::from_ref(&file), watcher::read_debounce_ms()).unwrap();

    let start = Instant::now();
    // Modify the file; expect a Changed event within 100ms debounce + delivery overhead.
    fs::write(&file, "// updated content\n").unwrap();

    // Blocking recv on a thread; collect via channel so the test runner stays responsive.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let Some(ev) = file_watcher.recv() {
            let _ = tx.send(ev);
        }
    });

    let ev = rx.recv_timeout(Duration::from_millis(600)).expect(
        "no file-change event within 600ms (debounce 100ms + 500ms headroom for CI)",
    );
    let elapsed = start.elapsed();

    match ev {
        WatchEvent::Changed(path) => {
            assert_eq!(
                path.canonicalize().unwrap(),
                file.canonicalize().unwrap(),
                "event path should match the modified file"
            );
        }
        WatchEvent::Removed(_) => panic!("expected Changed, got Removed"),
    }

    assert!(
        elapsed < Duration::from_millis(600),
        "event delivered in {}ms, expected ≤600ms",
        elapsed.as_millis()
    );
}

// WHY: ui functions are the only user-visible output from ynz watch; a panic in any of
//      them aborts the entire daemon. Non-TTY context is the most common test environment
//      and also the most likely to trigger edge cases (ANSI sequences ignored, no flush).
#[test]
fn ui_functions_smoke() {
    // Verify all pub ui functions are callable without panicking in a non-TTY context.
    ui::clear(true); // no_clear=true → always a no-op regardless of TTY status
    ui::print_watching();
    ui::print_building("foo.ynz");
    ui::print_success(42);
    ui::print_file_removed("foo.ynz");
}
