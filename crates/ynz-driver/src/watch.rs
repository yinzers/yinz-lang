use std::path::{Path, PathBuf};

use ynz_watch::WatchConfig;

/// Driver shim for `ynz watch`. Converts CLI args into a `WatchConfig` and
/// delegates to `ynz_watch::run`.
pub fn watch(file: Option<&Path>, check: bool, json: bool, no_clear: bool) -> i32 {
    let path = file
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let config = WatchConfig {
        path,
        check,
        json,
        no_clear,
    };

    ynz_watch::run(config)
}
