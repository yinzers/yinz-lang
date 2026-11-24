use std::io::{self, Write};

use crate::json_events::WatchEvent;

/// Emits NDJSON events to stdout, one JSON object per line.
///
/// Each call to `emit` serializes the event and writes it followed by a newline,
/// then flushes. Flushing after every event is critical: downstream automation
/// consumers (`jq`, log aggregators, CI dashboards) need each event to appear
/// immediately, not buffered in a 4KB block.
///
/// # EPIPE handling
///
/// If the downstream consumer closes the pipe (e.g., `ynz watch --json | head -1`
/// and `head` exits), the next `emit` call returns `Err(BrokenPipe)`. The caller
/// should detect this, write a `WatchShutdown { reason: pipe-closed }` event to
/// STDERR as a last-ditch message, and exit 0 (not a crash).
///
/// # Side effects
///
/// Writes one NDJSON line + flushes stdout per call.
///
/// Time: O(n) where n = JSON field count (dominated by serde serialization). Space: O(n).
pub struct JsonEmitter {
    out: Box<dyn Write + Send>,
}

impl JsonEmitter {
    /// Create a new `JsonEmitter` writing to stdout.
    pub fn new_stdout() -> Self {
        JsonEmitter {
            out: Box::new(io::stdout()),
        }
    }

    /// Create a `JsonEmitter` writing to a custom writer (for tests).
    pub fn new_writer(w: impl Write + Send + 'static) -> Self {
        JsonEmitter { out: Box::new(w) }
    }

    /// Serialize `event` as one NDJSON line and flush.
    ///
    /// Returns `Err` on EPIPE or other I/O failures. Caller should handle EPIPE by
    /// logging `"pipe-closed"` to stderr and exiting cleanly.
    pub fn emit(&mut self, event: &WatchEvent) -> io::Result<()> {
        let json = serde_json::to_string(event)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.out.write_all(json.as_bytes())?;
        self.out.write_all(b"\n")?;
        self.out.flush()
    }
}
