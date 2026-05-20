use serde::Serialize;

/// Schema version field embedded in every --json event.
///
/// Pre-v0.2.0: `"v0.2-m4-unstable"` — schema is mutable between intermediate milestones.
/// Consumers must pin to a specific Yinz binary version.
/// Post-v0.2.0 final: drops to `"v0.2"`; semver applies to field additions/removals.
pub const SCHEMA_VERSION: &str = "v0.2-m4-unstable";

/// Span within a source file (byte offsets).
#[derive(Debug, Clone, Serialize)]
pub struct SourceSpanJson {
    pub start: usize,
    pub end: usize,
}

/// A single typed watch event, serialized as NDJSON on stdout.
///
/// Every variant includes `schema_version` via `#[serde(tag = "type")]` tagging.
/// Variant names use `rename_all = "kebab-case"` so the `"type"` tag serializes as
/// kebab-case (e.g., `"watch-ready"`, `"build-start"`). Field names are kept as the
/// Rust identifier — snake_case in JSON (`"schema_version"`, `"duration_ms"`) per
/// the locked schema in `design/watch.md`.
///
/// Timestamp fields are RFC 3339 UTC with milliseconds:
///   `"2026-05-20T14:30:00.123Z"` — locale-independent; downstream parsers handle timezones.
///
/// Event-ordering invariants (see design/watch.md):
///   - `watch-ready` is always first; `watch-shutdown` is always last.
///   - Every `build-start` precedes exactly one `build-end` for the same file.
///   - `child-spawn` ONLY appears after `build-end { outcome: "ok" }`.
///   - `child-exit` follows `child-spawn` (eventually).
///   - `memory-warning` precedes any `memory-stop`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum WatchEvent {
    /// First event emitted when watch is ready; lists paths being watched.
    WatchReady {
        timestamp: String,
        schema_version: String,
        watching: Vec<String>,
    },

    /// Emitted when a file change is detected and compilation starts.
    BuildStart {
        timestamp: String,
        schema_version: String,
        file: String,
    },

    /// Emitted when compilation completes (with or without errors).
    BuildEnd {
        timestamp: String,
        schema_version: String,
        file: String,
        outcome: BuildOutcome,
        duration_ms: u128,
    },

    /// One event per compiler diagnostic emitted during a build.
    Diagnostic {
        timestamp: String,
        schema_version: String,
        file: String,
        severity: DiagnosticSeverity,
        span: SourceSpanJson,
        what: String,
        what_instead: String,
        why: String,
    },

    /// Emitted after a successful build when the compiled binary is spawned.
    ChildSpawn {
        timestamp: String,
        schema_version: String,
        pid: u32,
    },

    /// Emitted when the child process exits (between build cycles or on watch shutdown).
    ChildExit {
        timestamp: String,
        schema_version: String,
        pid: u32,
        exit_code: i32,
    },

    /// Emitted when a watched source file disappears (watch continues; shadow state retained).
    FileRemoved {
        timestamp: String,
        schema_version: String,
        file: String,
    },

    /// Emitted once when RSS exceeds the soft-warn threshold (Layer 3).
    MemoryWarning {
        timestamp: String,
        schema_version: String,
        rss_mb: u64,
        threshold_mb: u64,
    },

    /// Emitted just before watch exits due to RSS exceeding the hard-stop ceiling (Layer 3).
    MemoryStop {
        timestamp: String,
        schema_version: String,
        rss_mb: u64,
        threshold_mb: u64,
    },

    /// Emitted once when RSS polling is unavailable on this platform (degraded-mode notice).
    MemoryUnavailable {
        timestamp: String,
        schema_version: String,
        reason: String,
    },

    /// Last event emitted when the watch daemon shuts down for any reason.
    WatchShutdown {
        timestamp: String,
        schema_version: String,
        reason: ShutdownReason,
    },
}

/// Compile outcome for a `build-end` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildOutcome {
    Ok,
    Errors,
}

/// Diagnostic severity for a `diagnostic` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Suggestion,
}

/// Shutdown reason for a `watch-shutdown` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShutdownReason {
    CtrlC,
    Fatal,
    Oom,
    PipeClosed,
}

/// Format the current time as RFC 3339 UTC with milliseconds.
///
/// Output: `"2026-05-20T14:30:00.123Z"` — always UTC, always with `.nnnZ` suffix.
/// Consumers can rely on this format for regex matching and log indexing.
pub fn now_rfc3339() -> String {
    use chrono::Utc;
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Build a standard event with `timestamp` + `schema_version` already set.
pub fn ts() -> (String, String) {
    (now_rfc3339(), SCHEMA_VERSION.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // WHY: every --json event must include schema_version as a literal string constant,
    //      never inlined. If this drifts (e.g. a typo in one event), consumers get
    //      inconsistent schema_version fields and can't reliably detect schema changes.
    #[test]
    fn schema_version_constant_matches_expected() {
        assert_eq!(SCHEMA_VERSION, "v0.2-m4-unstable");
    }

    // WHY: RFC 3339 UTC with milliseconds is the locked timestamp format per design/watch.md.
    //      If now_rfc3339() drifts (loses the Z suffix, drops ms, uses local time), downstream
    //      log parsers and the json_mode timestamp-format assertion fail silently.
    //      Expected: YYYY-MM-DDTHH:MM:SS.mmmZ (exactly 24 chars, always UTC).
    #[test]
    fn timestamp_format_matches_rfc3339_utc() {
        let ts = now_rfc3339();
        // Expected: YYYY-MM-DDTHH:MM:SS.mmmZ  (exactly 24 chars)
        assert_eq!(ts.len(), 24, "timestamp `{ts}` must be 24 chars");
        assert!(ts.ends_with('Z'), "timestamp must end with 'Z' (UTC)");
        assert_eq!(&ts[4..5], "-", "timestamp[4] must be '-'");
        assert_eq!(&ts[7..8], "-", "timestamp[7] must be '-'");
        assert_eq!(&ts[10..11], "T", "timestamp[10] must be 'T'");
        assert_eq!(&ts[13..14], ":", "timestamp[13] must be ':'");
        assert_eq!(&ts[16..17], ":", "timestamp[16] must be ':'");
        assert_eq!(&ts[19..20], ".", "timestamp[19] must be '.'");
        // All other chars must be ASCII digits.
        let digit_positions: &[usize] = &[0,1,2,3,5,6,8,9,11,12,14,15,17,18,20,21,22];
        for &i in digit_positions {
            assert!(
                ts.as_bytes()[i].is_ascii_digit(),
                "timestamp[{i}] must be a digit in `{ts}`"
            );
        }
    }

    // WHY: every event must serialize to a parseable JSON object. If a variant is accidentally
    //      missing a field or has a type mismatch, serde_json::to_string panics or produces
    //      invalid JSON that downstream jq / log parsers reject silently.
    #[test]
    fn all_event_variants_serialize_to_valid_json() {
        let (timestamp, schema_version) = ts();
        let events = vec![
            WatchEvent::WatchReady {
                timestamp: timestamp.clone(),
                schema_version: schema_version.clone(),
                watching: vec!["foo.ynz".into()],
            },
            WatchEvent::BuildStart {
                timestamp: timestamp.clone(),
                schema_version: schema_version.clone(),
                file: "foo.ynz".into(),
            },
            WatchEvent::BuildEnd {
                timestamp: timestamp.clone(),
                schema_version: schema_version.clone(),
                file: "foo.ynz".into(),
                outcome: BuildOutcome::Ok,
                duration_ms: 42,
            },
            WatchEvent::ChildSpawn {
                timestamp: timestamp.clone(),
                schema_version: schema_version.clone(),
                pid: 12345,
            },
            WatchEvent::ChildExit {
                timestamp: timestamp.clone(),
                schema_version: schema_version.clone(),
                pid: 12345,
                exit_code: 0,
            },
            WatchEvent::WatchShutdown {
                timestamp: timestamp.clone(),
                schema_version: schema_version.clone(),
                reason: ShutdownReason::CtrlC,
            },
        ];

        for ev in events {
            let json = serde_json::to_string(&ev)
                .unwrap_or_else(|e| panic!("Failed to serialize event: {e}"));
            let parsed: serde_json::Value = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("Invalid JSON from serialize: {e}\nJSON: {json}"));
            assert!(
                parsed.is_object(),
                "serialized event must be a JSON object; got: {json}"
            );
        }
    }
}
