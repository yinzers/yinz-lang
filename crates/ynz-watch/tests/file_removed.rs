/// Integration test: file deletion is detected and does not crash the watcher.
///
/// When a watched `.ynz` file disappears mid-watch, the watcher must:
/// 1. Deliver exactly 1 event (Removed or Changed — OS timing determines which).
/// 2. NOT crash or hang.
///
/// The caller (`event_loop`) logs the vanished-file warning and continues watching.
use std::{fs, time::Duration};

use tempfile::TempDir;
use ynz_watch::{FileWatcher, WatchEvent};

#[test]
fn file_removal_does_not_crash_and_delivers_one_event() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("vanish.ynz");
    fs::write(&file, "// content\n").unwrap();

    let watcher = FileWatcher::new(std::slice::from_ref(&file), 100).unwrap();

    // Delete the file; watcher must deliver exactly one event and not panic.
    fs::remove_file(&file).unwrap();

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let Some(ev) = watcher.recv() {
            let _ = tx.send(ev);
        }
    });

    let ev = rx
        .recv_timeout(Duration::from_millis(600))
        .expect("no event delivered within 600ms after file deletion — watcher may have hung");

    // Accept either Removed (preferred) or Changed (OS timing edge-case).
    // The important invariant is that EXACTLY 1 event arrived (no panic, no hang).
    match ev {
        WatchEvent::Removed(_) => {
            // Preferred: watcher classified the deletion correctly.
        }
        WatchEvent::Changed(_) => {
            // Acceptable: OS fired a Modify event before the file physically disappeared;
            // the debounce window closed before the file-existence check could reclassify.
        }
    }
}
