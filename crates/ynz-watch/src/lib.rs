pub mod child;
pub mod db;
mod error;
mod event_loop;
pub mod project;
pub mod rebuild;
pub mod ui;
pub mod watcher;

pub use error::{Result, WatchError};
pub use watcher::{FileWatcher, WatchEvent};

use std::path::PathBuf;

use child::ChildHandle;
use db::WatchDb;

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
///
/// # Flow
///
/// 1. Resolve watch target (single-file or project mode).
/// 2. Populate `WatchDb` with initial source state.
/// 3. Run initial build before waiting for events.
/// 4. Start the file watcher with the debounced event channel.
/// 5. Enter the event loop: on file change → rebuild; on Ctrl+C → clean exit.
///
/// # Failure modes
///
/// - No yinz.toml in project mode → exit 2 with WatchError::NoProjectFile diagnostic.
/// - Source file unreadable at init → exit 2 with WatchError::SourceRead diagnostic.
/// - File watcher init failure → exit 2 with WatchError::WatcherInit diagnostic.
/// - All compile errors are recoverable: watch continues after printing diagnostics.
/// - Child spawn failure: logged as Infra error; watch continues (no binary running).
///
/// # Side effects
///
/// Spawns and manages child processes (the compiled Yinz program). Child is killed
/// (SIGTERM → SIGKILL) on each rebuild or Ctrl+C.
///
/// Time: O(1) per event (salsa amortizes). Space: O(n) where n = source files.
pub fn run(config: WatchConfig) -> i32 {
    let debounce_ms = watcher::read_debounce_ms();

    // 1. Resolve watch target.
    let target = match project::resolve_target(&config.path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };

    let entry_path = target.entry.clone();
    let watch_paths: Vec<PathBuf> = if let Some(ref root) = target.project_root {
        vec![root.clone()]
    } else {
        watcher::single_file_paths(&config.path)
    };

    // 2. Populate WatchDb.
    let mut watch_db = WatchDb::from_target(&target);

    // 3. Allocate per-pid tempdir for compiled binaries.
    let out_dir_path = std::env::temp_dir().join(format!("ynz-watch-{}", std::process::id()));
    let out_dir = match std::fs::create_dir_all(&out_dir_path).map(|_| out_dir_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ynz watch: could not create temp dir: {e}");
            return 2;
        }
    };

    // 4. Initial build before waiting for events.
    if !config.json {
        println!("ynz watch: watching {}", config.path.display());
    }
    let mut current_child: Option<ChildHandle> = None;
    run_rebuild_cycle(&mut watch_db, &entry_path, &out_dir, &config, &mut current_child);

    // 5. Start file watcher.
    let file_watcher = match watcher::FileWatcher::new(&watch_paths, debounce_ms) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };

    // 6. Event loop.
    event_loop::run_event_loop(&file_watcher, &config, |_changed_path| {
        run_rebuild_cycle(&mut watch_db, &entry_path, &out_dir, &config, &mut current_child);
    });

    // Kill child on exit (Ctrl+C path — Drop on current_child also fires as fallback).
    if let Some(ref mut c) = current_child {
        c.kill_gracefully(2000);
    }

    // Cleanup tempdir on exit.
    let _ = std::fs::remove_dir_all(&out_dir);

    0
}

/// Run one rebuild + (optionally) child-spawn cycle.
fn run_rebuild_cycle(
    db: &mut WatchDb,
    entry_path: &std::path::Path,
    out_dir: &std::path::Path,
    config: &WatchConfig,
    current_child: &mut Option<ChildHandle>,
) {
    use rebuild::rebuild_one;
    let _ = rebuild_one(db, entry_path, entry_path, out_dir, config.check, current_child);
}
