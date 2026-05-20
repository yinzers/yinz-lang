mod error;

pub use error::{Result, WatchError};

use std::path::PathBuf;

/// Configuration for a `ynz watch` session.
///
/// Built by the driver shim in `crates/ynz-driver/src/watch.rs` from CLI args.
pub struct WatchConfig {
    /// Path to watch: a single `.ynz` file OR a project root directory (with `yinz.toml`).
    pub path: PathBuf,
    /// Build only; do not spawn the compiled binary.
    pub check: bool,
    /// Emit NDJSON event stream on stdout; suppress normal text output.
    pub json: bool,
    /// Do not clear the terminal between rebuild cycles.
    pub no_clear: bool,
}

/// Entry point called by the driver's `Watch` subcommand handler.
///
/// Returns an exit code suitable for `process::exit`:
///   0 — clean exit (Ctrl+C, pipe-closed)
///   1 — compile failure (first-build error; behavior extended in later phases)
///   2 — infrastructure error (watcher init failed, no yinz.toml, OOM hard-stop, etc.)
pub fn run(config: WatchConfig) -> i32 {
    eprintln!(
        "ynz watch: not yet implemented (watching: {})",
        config.path.display()
    );
    1
}
