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

/// The debounce window used when `YNZ_WATCH_DEBOUNCE_MS` is unset or unusable.
const DEFAULT_DEBOUNCE_MS: u64 = 100;

/// Read the debounce window from `YNZ_WATCH_DEBOUNCE_MS`, falling back to
/// [`DEFAULT_DEBOUNCE_MS`].
///
/// Invalid values (non-numeric, zero) fall back to the default, and a warning is printed so
/// the user knows their env var didn't take effect. This is the only place the process
/// environment is touched; the decision itself lives in the pure [`debounce_ms_from`], which
/// is what the unit tests exercise — so no test ever mutates the process-global environment
/// (that shared state made the old tests race each other under the parallel runner).
pub fn read_debounce_ms() -> u64 {
    let raw = std::env::var("YNZ_WATCH_DEBOUNCE_MS").ok();
    match debounce_ms_from(raw.as_deref()) {
        Ok(ms) => ms,
        Err(warning) => {
            eprintln!("ynz watch: {warning}; using {DEFAULT_DEBOUNCE_MS}ms");
            DEFAULT_DEBOUNCE_MS
        }
    }
}

/// Decide the debounce window from the raw `YNZ_WATCH_DEBOUNCE_MS` value (`None` = unset).
///
/// Pure: no I/O, no environment access. `Ok(ms)` is the window to use (the default when the
/// variable is unset); `Err(reason)` means the value was present but unusable — the caller
/// warns with `reason` and falls back to [`DEFAULT_DEBOUNCE_MS`].
fn debounce_ms_from(raw: Option<&str>) -> std::result::Result<u64, String> {
    let Some(s) = raw else {
        return Ok(DEFAULT_DEBOUNCE_MS);
    };
    match s.parse::<u64>() {
        Ok(0) => Err("YNZ_WATCH_DEBOUNCE_MS=0 is not valid (minimum 1)".to_string()),
        Ok(v) => Ok(v),
        Err(_) => Err(format!("YNZ_WATCH_DEBOUNCE_MS={s:?} is not a valid number")),
    }
}

/// Produce a one-element path vec for single-file watch mode.
pub fn single_file_paths(path: &Path) -> Vec<PathBuf> {
    vec![path.to_path_buf()]
}

#[cfg(test)]
mod tests {
    use super::*;

    // WHY (all debounce_ms_from tests): the debounce window controls event coalescing —
    //      too small = spurious rebuilds; an invalid env var must fall back (not panic).
    //      If the fallback breaks, a misconfigured env var crashes the watcher before the
    //      first file event. Tests cover the default, custom, invalid, and zero cases.
    //
    //      They test the pure decision function with the raw value passed in, NEVER
    //      `read_debounce_ms` itself: that would require `set_var`/`remove_var` on the
    //      process-global environment, and cargo runs these tests concurrently in one
    //      process — the old env-mutating versions failed nondeterministically (0 or 1
    //      failures across identical runs). Removing the shared state is the fix; a
    //      `#[serial]` guard would only have hidden it.

    #[test]
    fn debounce_ms_from_unset_uses_default() {
        assert_eq!(debounce_ms_from(None), Ok(DEFAULT_DEBOUNCE_MS));
        assert_eq!(DEFAULT_DEBOUNCE_MS, 100, "the documented default is 100ms");
    }

    #[test]
    fn debounce_ms_from_custom_value() {
        assert_eq!(debounce_ms_from(Some("250")), Ok(250));
    }

    #[test]
    fn debounce_ms_from_invalid_falls_back_with_a_reason() {
        let err = debounce_ms_from(Some("notanumber")).expect_err("non-numeric must fall back");
        assert!(
            err.contains("notanumber") && err.contains("not a valid number"),
            "the warning must name the bad value and why it was rejected; got {err:?}"
        );
    }

    #[test]
    fn debounce_ms_from_zero_falls_back_with_a_reason() {
        let err = debounce_ms_from(Some("0")).expect_err("zero must fall back");
        assert!(
            err.contains("minimum 1"),
            "the warning must state the minimum; got {err:?}"
        );
    }
}
