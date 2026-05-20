use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};

use crate::error::{Result, WatchError};

/// A debounced file-system change event, after coalescing.
///
/// `notify-debouncer-mini` collapses rapid bursts (write-tempfile + rename) into a
/// single delivery, so one editor save produces exactly one event here — not 3-4.
pub enum WatchEvent {
    /// A watched `.ynz` file was modified (or the watched path was created/renamed-onto).
    Changed(PathBuf),
    /// A watched `.ynz` file was deleted.
    Removed(PathBuf),
}

type NotifyDebouncer = Debouncer<RecommendedWatcher>;

/// Wraps `notify-debouncer-mini`; exposes a blocking iterator of debounced events.
///
/// The debouncer coalesces editor-save sequences (write-tempfile + rename + chmod)
/// into a single `Changed` event within the window. This is the ONLY coalescing
/// layer — there is no second dedup pass in `event_loop.rs`.
///
/// # Debounce window
///
/// Default 100ms (configurable via `YNZ_WATCH_DEBOUNCE_MS`). Editor saves settle
/// within ~50ms; 100ms is conservative without feeling laggy.
pub struct FileWatcher {
    // Kept alive: if _debouncer is dropped, the background thread stops and no more events arrive.
    _debouncer: NotifyDebouncer,
    rx: mpsc::Receiver<WatchEvent>,
}

impl FileWatcher {
    /// Subscribe to all `.ynz` file changes under `paths`.
    ///
    /// `debounce_ms` — window in ms; use `read_debounce_ms()` to pull from env.
    ///
    /// # Failure modes
    ///
    /// Returns `WatchError::WatcherInit` if `notify` cannot subscribe to a path
    /// (path doesn't exist, permissions denied, unsupported filesystem).
    pub fn new(paths: &[PathBuf], debounce_ms: u64) -> Result<Self> {
        let (tx, rx) = mpsc::channel();

        let tx_clone = tx;
        let mut debouncer = new_debouncer(
            Duration::from_millis(debounce_ms),
            move |res: DebounceEventResult| {
                match res {
                    Ok(events) => {
                        for event in events {
                            let path = event.path.clone();
                            // Map notify event kind → our WatchEvent:
                            //   AnyContinuous = ongoing write (in-progress save) → Changed
                            //   Any           = final event after debounce window closes → Changed
                            // We can't distinguish Remove from Modify in the debounced result;
                            // treating both as Changed is safe (spurious rebuild > missed rebuild).
                            // All debouncer events map to Changed; classify() reclassifies to
                            // Removed if the path is gone by the time recv() is called.
                            // Spurious rebuild (Changed on a remove) is always safe; missed
                            // rebuild (dropping a real change) is not — so Changed is the
                            // correct safe default.
                            let ev = WatchEvent::Changed(path);
                            let _ = tx_clone.send(ev);
                        }
                    }
                    Err(e) => {
                        eprintln!("ynz watch: file watcher error: {e}");
                    }
                }
            },
        )
        .map_err(|e| WatchError::WatcherInit {
            path: paths.first().cloned().unwrap_or_default(),
            reason: e.to_string(),
        })?;

        // Subscribe to each path.
        for path in paths {
            debouncer
                .watcher()
                .watch(path, RecursiveMode::Recursive)
                .map_err(|e| WatchError::WatcherInit {
                    path: path.clone(),
                    reason: e.to_string(),
                })?;
        }

        Ok(Self {
            _debouncer: debouncer,
            rx,
        })
    }

    /// Block until the next debounced event arrives.
    ///
    /// Returns `None` if the watcher has been dropped (channel closed).
    ///
    /// Post-debounce, checks if the path still exists: if the file has disappeared,
    /// the event is emitted as `Removed` rather than `Changed`. This lets the caller
    /// log the vanished-file warning without attempting a rebuild against a missing file.
    pub fn recv(&self) -> Option<WatchEvent> {
        let ev = self.rx.recv().ok()?;
        Some(self.classify(ev))
    }

    /// Reclassify a Changed event as Removed if the path no longer exists.
    ///
    /// The debouncer cannot distinguish Remove from Modify (both arrive as `Any`).
    /// Checking file existence post-debounce catches the remove case at the cost of
    /// a single stat(2) call per event — negligible overhead.
    fn classify(&self, ev: WatchEvent) -> WatchEvent {
        match ev {
            WatchEvent::Changed(ref path) => {
                if !path.exists() {
                    WatchEvent::Removed(path.clone())
                } else {
                    ev
                }
            }
            other => other,
        }
    }
}

/// Read the debounce window from `YNZ_WATCH_DEBOUNCE_MS`, falling back to 100ms.
///
/// Invalid values (non-numeric, zero) are silently ignored and the default is used.
/// A warning is printed so the user knows their env var didn't take effect.
pub fn read_debounce_ms() -> u64 {
    const DEFAULT: u64 = 100;
    match std::env::var("YNZ_WATCH_DEBOUNCE_MS") {
        Ok(s) => match s.parse::<u64>() {
            Ok(0) => {
                eprintln!(
                    "ynz watch: YNZ_WATCH_DEBOUNCE_MS=0 is not valid (minimum 1); using {DEFAULT}ms"
                );
                DEFAULT
            }
            Ok(v) => v,
            Err(_) => {
                eprintln!(
                    "ynz watch: YNZ_WATCH_DEBOUNCE_MS={s:?} is not a valid number; using {DEFAULT}ms"
                );
                DEFAULT
            }
        },
        Err(_) => DEFAULT,
    }
}

/// Produce a one-element path vec for single-file watch mode.
pub fn single_file_paths(path: &Path) -> Vec<PathBuf> {
    vec![path.to_path_buf()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_debounce_ms_default() {
        std::env::remove_var("YNZ_WATCH_DEBOUNCE_MS");
        assert_eq!(read_debounce_ms(), 100);
    }

    #[test]
    fn read_debounce_ms_custom() {
        std::env::set_var("YNZ_WATCH_DEBOUNCE_MS", "250");
        assert_eq!(read_debounce_ms(), 250);
        std::env::remove_var("YNZ_WATCH_DEBOUNCE_MS");
    }

    #[test]
    fn read_debounce_ms_invalid_falls_back() {
        std::env::set_var("YNZ_WATCH_DEBOUNCE_MS", "notanumber");
        assert_eq!(read_debounce_ms(), 100);
        std::env::remove_var("YNZ_WATCH_DEBOUNCE_MS");
    }

    #[test]
    fn read_debounce_ms_zero_falls_back() {
        std::env::set_var("YNZ_WATCH_DEBOUNCE_MS", "0");
        assert_eq!(read_debounce_ms(), 100);
        std::env::remove_var("YNZ_WATCH_DEBOUNCE_MS");
    }
}
