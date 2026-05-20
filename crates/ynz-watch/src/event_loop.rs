use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
};

use crate::{
    ui::{self, print_file_removed},
    watcher::{FileWatcher, WatchEvent},
    WatchConfig,
};

/// The watch event loop: receives debounced file events and dispatches them.
///
/// # Flow
///
/// 1. Install Ctrl+C handler that sets the shutdown flag.
/// 2. Print "Watching…" idle prompt.
/// 3. Block on the next debounced file event from the watcher.
/// 4. On `Changed(path)`:  log "[file change] <path>" + call the rebuild callback.
/// 5. On `Removed(path)`:  log "[file removed] <path>" via `ui::print_file_removed`;
///    do NOT crash; do NOT trigger a rebuild (shadow state retains last-known text —
///    handled by rebuild callback when it receives None from disk read).
/// 6. After each event, check shutdown flag; exit cleanly if set.
/// 7. On watcher channel close (watcher dropped): exit cleanly.
///
/// # Failure modes
///
/// - Ctrl+C: sets shutdown flag; loop exits after current event; child killed by caller.
/// - Watcher channel closed unexpectedly: loop exits with a stderr warning.
/// - Rebuild callback panics: propagates (panic hook in ynz-driver prints ICE banner).
///
/// # Side effects
///
/// Writes status lines to stdout. Calls `callback` for each `Changed` event.
///
/// Time: O(1) per event  Space: O(1)
pub fn run_event_loop<F>(
    watcher: &FileWatcher,
    config: &WatchConfig,
    mut on_change: F,
) where
    F: FnMut(&PathBuf),
{
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);

    ctrlc::set_handler(move || {
        shutdown_clone.store(true, Ordering::SeqCst);
    })
    .expect("ynz watch: could not install Ctrl+C handler");

    ui::print_watching();

    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        match watcher.recv() {
            None => {
                // Channel closed — watcher dropped or OS error.
                eprintln!("ynz watch: file watcher channel closed unexpectedly; exiting");
                break;
            }
            Some(WatchEvent::Changed(path)) => {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                // Clear before rebuild status so we don't interleave with child stdout.
                // Flushing the status line BEFORE the callback starts prevents garbled output
                // when the child prints continuously (plan-review Round 2 adversarial concern).
                ui::clear(config.no_clear);
                on_change(&path);
                // print_watching() omitted here: on_change callback includes the idle prompt
                // via print_success / print_errors; adding it again would double the line.
            }
            Some(WatchEvent::Removed(path)) => {
                print_file_removed(&path.display().to_string());
                // No rebuild: shadow state retains last-known source text; rebuild callback
                // will re-read from disk, handle the missing file gracefully.
            }
        }

        if shutdown.load(Ordering::SeqCst) {
            break;
        }
    }
}

