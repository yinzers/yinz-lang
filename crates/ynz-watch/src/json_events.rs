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

    // Locked --json timestamp format (design/watch.md): RFC 3339 UTC with milliseconds,
    // YYYY-MM-DDTHH:MM:SS.mmmZ. Every digit is masked to `9` so one comparison checks the
    // length, the separators, the `Z` suffix and that all other characters are digits.
    // bug: now_rfc3339 loses the Z suffix, drops milliseconds or uses local time, so downstream log parsers reject every --json timestamp | tier: Medium
    #[test]
    fn timestamp_format_matches_rfc3339_utc() {
        let timestamp = now_rfc3339();
        let digits_masked: String = timestamp
            .chars()
            .map(|character| {
                if character.is_ascii_digit() {
                    '9'
                } else {
                    character
                }
            })
            .collect();
        assert_eq!(
            digits_masked, "9999-99-99T99:99:99.999Z",
            "timestamp `{timestamp}` must be YYYY-MM-DDTHH:MM:SS.mmmZ"
        );
    }
}
