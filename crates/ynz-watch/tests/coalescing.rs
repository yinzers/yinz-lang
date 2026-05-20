/// Integration test: atomic-write sequences produce exactly 1 debounced event.
///
/// Many editors (VS Code, Vim, Emacs) save via a write-tempfile + rename sequence.
/// Without debouncing, that produces 3-4 raw notify events. This test verifies the
/// debouncer coalesces them into exactly 1 `Changed` event.
use std::{
    fs,
    time::{Duration, Instant},
};

use tempfile::TempDir;
use ynz_watch::{FileWatcher, WatchEvent};

#[test]
fn atomic_write_produces_exactly_one_event() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target.ynz");
    let tmp = dir.path().join("target.ynz.tmp");
    fs::write(&target, "// v0\n").unwrap();

    // Use 150ms debounce to give the coalescer room on CI (above the 100ms default).
    let watcher = FileWatcher::new(std::slice::from_ref(&target), 150).unwrap();

    // Simulate a VS Code / Vim-style atomic editor save: write tempfile → rename onto target.
    fs::write(&tmp, "// v1\n").unwrap();
    fs::rename(&tmp, &target).unwrap();

    let (tx, rx) = std::sync::mpsc::channel::<WatchEvent>();
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_millis(700);
        while Instant::now() < deadline {
            match watcher.recv() {
                Some(ev) => {
                    let _ = tx.send(ev);
                }
                None => break,
            }
        }
    });

    // Wait for the debounce window + margin to fully expire before counting.
    std::thread::sleep(Duration::from_millis(500));

    let mut events: Vec<WatchEvent> = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        events.push(ev);
    }

    assert_eq!(
        events.len(),
        1,
        "atomic write should produce exactly 1 debounced event, got {} \
         (debouncer window = 150ms; if this fails on CI it may need wider window)",
        events.len()
    );
    match &events[0] {
        WatchEvent::Changed(_) => {}
        WatchEvent::Removed(_) => panic!("expected Changed, got Removed for an atomic write"),
    }
}
