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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_events::{ts, WatchEvent};

    // WHY: JsonEmitter must write exactly one NDJSON line per emit() call. If it buffers
    //      multiple events or omits the newline, downstream `jq` / log parsers get either
    //      nothing (waiting for flush) or malformed multi-event lines. Each call must be
    //      atomic: one serialized JSON object + one newline + one flush.
    // A thread-safe shared buffer for testing JsonEmitter output.
    use std::sync::{Arc, Mutex};

    struct SharedBuf(Arc<Mutex<Vec<u8>>>);
    impl Write for SharedBuf {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    // Arc<Mutex<Vec<u8>>> implements Send automatically; no unsafe needed here.

    #[test]
    fn emit_writes_single_ndjson_line_per_call() {
        let shared = Arc::new(Mutex::new(Vec::<u8>::new()));
        let mut emitter = JsonEmitter::new_writer(SharedBuf(Arc::clone(&shared)));

        let (timestamp, schema_version) = ts();
        let ev = WatchEvent::BuildStart {
            timestamp,
            schema_version,
            file: "foo.ynz".into(),
        };
        emitter.emit(&ev).unwrap();

        let bytes = shared.lock().unwrap().clone();
        let output = String::from_utf8(bytes).unwrap();
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(
            lines.len(),
            1,
            "emit must produce exactly one line per call"
        );
        let parsed: serde_json::Value =
            serde_json::from_str(lines[0]).expect("emitted line must be valid JSON");
        assert_eq!(
            parsed.get("type").and_then(|v| v.as_str()),
            Some("build-start"),
            "type field must be 'build-start'"
        );
        assert!(
            parsed.get("schema_version").is_some(),
            "event must have schema_version field"
        );
    }
}
