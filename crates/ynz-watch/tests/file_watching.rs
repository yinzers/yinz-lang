/// Integration test for the file-watching plumbing layer: a modified file reaches the caller.
///
/// Coalescing is in `coalescing.rs`; file-removal is in `file_removed.rs`.
use std::{fs, time::Duration};

use tempfile::TempDir;
use ynz_watch::{watcher, FileWatcher, WatchEvent};

// bug: a write to a watched file is never delivered as a Changed event for that path | tier: Medium
#[test]
fn file_change_delivers_changed_event_for_watched_path() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("test.ynz");
    fs::write(&file, "// initial content\n").unwrap();

    let file_watcher =
        FileWatcher::new(std::slice::from_ref(&file), watcher::read_debounce_ms()).unwrap();

    // Modify the file; expect a Changed event after the debounce window.
    fs::write(&file, "// updated content\n").unwrap();

    // Blocking recv on a thread; collect via channel so the test runner stays responsive.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let Some(ev) = file_watcher.recv() {
            let _ = tx.send(ev);
        }
    });

    let ev = rx
        .recv_timeout(Duration::from_secs(3))
        .expect("no file-change event within 3s (liveness bound, about 10x the debounce window)");

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
}
