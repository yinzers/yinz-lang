/// Layer 3 memory defense: cross-platform RSS polling via `memory-stats` crate.
///
/// After every rebuild cycle, the event loop calls `check_rss` to sample process
/// RSS and decide whether to warn the user or hard-stop the watch daemon.
///
/// # Thresholds (default, configurable via env vars)
///
/// | Threshold | Default | Env var               | Action                                  |
/// |---|---|---|---|
/// | Soft warn | 1024 MB | `YNZ_WATCH_RSS_WARN_MB` | Print warning + JSON `memory-warning`; rate-limited 1/60s |
/// | Hard stop | 4096 MB | `YNZ_WATCH_MAX_RSS_MB`  | Print WHAT/WHAT-INSTEAD/WHY + JSON `memory-stop` + exit 2 |
///
/// # Platform support (via `memory-stats` crate)
///
/// | OS      | Backend                                |
/// |---------|----------------------------------------|
/// | Linux   | `/proc/self/status` (`VmRSS`)          |
/// | macOS   | mach `task_info(MACH_TASK_BASIC_INFO)` |
/// | Windows | `GetProcessMemoryInfo`                 |
///
/// Returns `None` if polling is unavailable on this platform (rare; emits
/// `MemoryUnavailable` event once and continues without the hard-stop safety net).
use std::time::{Duration, Instant};

use memory_stats::memory_stats;

/// Current RSS (Resident Set Size) in bytes, or `None` if polling is unavailable.
///
/// Calls `memory_stats()` from the `memory-stats` crate (cross-platform).
/// Returns `None` only on platforms where the crate cannot read process memory
/// (rare — typically chroot environments without /proc).
///
/// Time: O(1) — one syscall (stat or task_info). Space: O(1).
pub fn current_rss_bytes() -> Option<u64> {
    memory_stats().map(|s| s.physical_mem as u64)
}

/// Memory thresholds read from env vars at startup.
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    /// Soft-warn threshold in MB (default 1024).
    pub warn_mb: u64,
    /// Hard-stop threshold in MB (default 4096).
    pub max_mb: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            warn_mb: read_mb_env("YNZ_WATCH_RSS_WARN_MB", 1024),
            max_mb: read_mb_env("YNZ_WATCH_MAX_RSS_MB", 4096),
        }
    }
}

/// Outcome of an RSS check.
pub enum RssCheckResult {
    /// RSS within bounds.
    Ok,
    /// RSS exceeded soft-warn threshold (rate-limited; may not fire every call).
    Warn { rss_mb: u64, threshold_mb: u64 },
    /// RSS exceeded hard-stop ceiling. Watch must exit code 2.
    Stop { rss_mb: u64, threshold_mb: u64 },
    /// RSS polling unavailable on this platform.
    Unavailable,
}

/// Sample process RSS and classify the result against the configured thresholds.
///
/// `last_warn`: mutable reference to the `Instant` of the last warning emission,
/// used for rate-limiting (emit at most 1 warning per 60s).
///
/// Returns `RssCheckResult::Unavailable` if polling returns `None`. The caller
/// must emit a `MemoryUnavailable` event on the FIRST `Unavailable` result and
/// then stop calling (or at least stop emitting the warning).
///
/// Time: O(1). Space: O(1).
pub fn check_rss(config: &MemoryConfig, last_warn: &mut Option<Instant>) -> RssCheckResult {
    match current_rss_bytes() {
        None => RssCheckResult::Unavailable,
        Some(rss_bytes) => {
            let rss_mb = rss_bytes / (1024 * 1024);
            if rss_mb >= config.max_mb {
                return RssCheckResult::Stop {
                    rss_mb,
                    threshold_mb: config.max_mb,
                };
            }
            if rss_mb >= config.warn_mb {
                let now = Instant::now();
                let should_warn = match *last_warn {
                    None => true,
                    Some(t) => now.duration_since(t) >= Duration::from_secs(60),
                };
                if should_warn {
                    *last_warn = Some(now);
                    return RssCheckResult::Warn {
                        rss_mb,
                        threshold_mb: config.warn_mb,
                    };
                }
            }
            RssCheckResult::Ok
        }
    }
}

/// Hard-stop message following WHAT/WHAT-INSTEAD/WHY format (Golden Rule 11).
pub fn hard_stop_message(rss_mb: u64, threshold_mb: u64, args: &str) -> String {
    format!(
        "WHAT: ynz watch hit {rss_mb}MB memory; this is the safety stop (ceiling: {threshold_mb}MB).\n\
         WHAT INSTEAD: Run `ynz watch {args}` to restart.\n\
         WHY: If this happens frequently, set `YNZ_WATCH_REBUILD_AFTER=200` (default 500) to \
         rebuild the compiler state more often. This releases accumulated compiler cache \
         more frequently."
    )
}

/// Read a u64 MB value from an env var, falling back to `default_mb`.
pub fn read_mb_env(var: &str, default_mb: u64) -> u64 {
    match std::env::var(var) {
        Ok(s) => match s.parse::<u64>() {
            Ok(v) if v > 0 => v,
            _ => {
                eprintln!("ynz watch: {var}={s:?} is not a valid number; using {default_mb}");
                default_mb
            }
        },
        Err(_) => default_mb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // WHY: MemoryConfig must read YNZ_WATCH_MAX_RSS_MB from the environment. If it always
    //      returns the hard-coded default, the env var knob in the user-facing docs is broken
    //      silently — users who set YNZ_WATCH_MAX_RSS_MB=1 to test the limit would never see
    //      it trigger. Do NOT change to assert the default; that's not what this test is for.
    #[test]
    fn memory_config_reads_env_vars() {
        std::env::set_var("YNZ_WATCH_MAX_RSS_MB", "8192");
        std::env::set_var("YNZ_WATCH_RSS_WARN_MB", "2048");
        let cfg = MemoryConfig::default();
        assert_eq!(cfg.max_mb, 8192);
        assert_eq!(cfg.warn_mb, 2048);
        std::env::remove_var("YNZ_WATCH_MAX_RSS_MB");
        std::env::remove_var("YNZ_WATCH_RSS_WARN_MB");
    }

    // WHY: read_mb_env must fall back to the default when the value is not a valid number.
    //      If it panics or returns 0, the watch daemon crashes on misconfigured env vars
    //      instead of printing a warning and continuing.
    #[test]
    fn read_mb_env_falls_back_on_invalid() {
        std::env::set_var("YNZ_WATCH_TEST_VAR", "notanumber");
        assert_eq!(read_mb_env("YNZ_WATCH_TEST_VAR", 999), 999);
        std::env::remove_var("YNZ_WATCH_TEST_VAR");
    }

    // WHY: the hard_stop_message must contain all three parts of the WHAT/WHAT-INSTEAD/WHY
    //      format. If any part is missing, the user's terminal shows a cryptic stop without
    //      actionable guidance — defeating the teaching mission.
    #[test]
    fn hard_stop_message_contains_three_parts() {
        let msg = hard_stop_message(4096, 4096, "foo.ynz");
        assert!(msg.contains("WHAT:"), "message must have WHAT");
        assert!(msg.contains("WHAT INSTEAD:"), "message must have WHAT INSTEAD");
        assert!(msg.contains("WHY:"), "message must have WHY");
    }
}
