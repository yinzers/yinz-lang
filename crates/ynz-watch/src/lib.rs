mod error;
mod event_loop;
pub mod ui;
pub mod watcher;

pub use error::{Result, WatchError};
pub use watcher::{FileWatcher, WatchEvent};

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
///   1 — compile failure
///   2 — infrastructure error (watcher init failed, no yinz.toml, OOM hard-stop, etc.)
pub fn run(config: WatchConfig) -> i32 {
    let debounce_ms = watcher::read_debounce_ms();

    let paths = watcher::single_file_paths(&config.path);

    let file_watcher = match watcher::FileWatcher::new(&paths, debounce_ms) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };

    if !config.json {
        println!("ynz watch: watching {}", config.path.display());
    }

    event_loop::run_event_loop(&file_watcher, &config, |path| {
        if config.json {
            // --json emitter wired in a later milestone; log to stderr for now so stdout stays NDJSON-clean.
            eprintln!("[file change] {}", path.display());
        } else {
            // Placeholder rebuild callback: logs the change + reports instant success.
            // Replaced by real compilation (check_query + codegen_query) when the DB layer lands.
            ui::print_building(&path.display().to_string());
            ui::print_success(0);
        }
    });

    0
}
