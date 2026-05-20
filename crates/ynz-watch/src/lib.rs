pub mod child;
pub mod db;
mod error;
mod event_loop;
pub mod json_emitter;
pub mod json_events;
pub mod memory;
pub mod project;
pub mod rebuild;
pub mod ui;
pub mod watcher;

pub use error::{Result, WatchError};
pub use watcher::{FileWatcher, WatchEvent};

use std::path::PathBuf;

use child::ChildHandle;
use db::WatchDb;
use json_emitter::JsonEmitter;
use json_events::{ShutdownReason, WatchEvent as JsonWatchEvent, ts};
use memory::{MemoryConfig, RssCheckResult, check_rss, hard_stop_message};


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
/// 3. Emit `watch-ready` (--json) or print status (text mode).
/// 4. Run initial build before waiting for events.
/// 5. Start the file watcher with the debounced event channel.
/// 6. Enter the event loop: on file change → rebuild; on Ctrl+C → clean exit.
///
/// # Failure modes
///
/// - No yinz.toml in project mode → exit 2 with WatchError::NoProjectFile diagnostic.
/// - Source file unreadable at init → exit 2 with WatchError::SourceRead diagnostic.
/// - File watcher init failure → exit 2 with WatchError::WatcherInit diagnostic.
/// - All compile errors are recoverable: watch continues after printing diagnostics.
/// - Child spawn failure: logged as Infra error; watch continues (no binary running).
/// - EPIPE (--json pipe closed): emit WatchShutdown to stderr, exit 0.
///
/// # Side effects
///
/// Spawns and manages child processes (the compiled Yinz program). Child is killed
/// (SIGTERM → SIGKILL) on each rebuild or Ctrl+C. May emit NDJSON to stdout.
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

    // 4. Build optional JSON emitter; emit watch-ready.
    let mut emitter: Option<JsonEmitter> = if config.json {
        let mut e = JsonEmitter::new_stdout();
        let (timestamp, schema_version) = ts();
        let watching: Vec<String> = watch_paths.iter().map(|p| p.display().to_string()).collect();
        if let Err(err) = e.emit(&JsonWatchEvent::WatchReady {
            timestamp,
            schema_version,
            watching,
        }) {
            if is_epipe(&err) {
                rebuild::emit_pipe_closed_to_stderr();
                return 0;
            }
            eprintln!("ynz watch: json emit error: {err}");
        }
        Some(e)
    } else {
        println!("ynz watch: watching {}", config.path.display());
        None
    };

    // 5. Initial build before waiting for events.
    let mut current_child: Option<ChildHandle> = None;
    if run_rebuild_cycle(
        &mut watch_db,
        &entry_path,
        &out_dir,
        &config,
        &mut current_child,
        emitter.as_mut(),
    ) {
        // EPIPE on initial build — downstream consumer already gone.
        rebuild::emit_pipe_closed_to_stderr();
        return 0;
    }

    // 6. Start file watcher.
    let file_watcher = match watcher::FileWatcher::new(&watch_paths, debounce_ms) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };

    // 7. Memory config (Layer 2: periodic rebuild; Layer 3: RSS hard-stop).
    let rebuild_after_n = memory::read_u64_env("YNZ_WATCH_REBUILD_AFTER", 500);
    let rebuild_after_hours = memory::read_u64_env("YNZ_WATCH_REBUILD_AFTER_HOURS", 4);
    let rebuild_after_duration = std::time::Duration::from_secs(rebuild_after_hours * 3600);
    let mem_config = MemoryConfig::default();
    let mut last_warn: Option<std::time::Instant> = None;
    let mut rss_unavailable_logged = false;

    // 8. Event loop.
    // epipe: set to true when an emit() returns BrokenPipe; event loop exits on next iteration.
    // oom_stop: set to true when RSS exceeds hard-stop ceiling; exits 2.
    let epipe = std::cell::Cell::new(false);
    let oom_stop = std::cell::Cell::new(false);
    event_loop::run_event_loop(&file_watcher, &config, |_changed_path| {
        if epipe.get() || oom_stop.get() {
            return;
        }

        let got_epipe = run_rebuild_cycle(
            &mut watch_db,
            &entry_path,
            &out_dir,
            &config,
            &mut current_child,
            emitter.as_mut(),
        );
        if got_epipe {
            epipe.set(true);
            return;
        }

        // Layer 2: periodic DB rebuild.
        if watch_db.should_periodic_rebuild(rebuild_after_n, rebuild_after_duration) {
            watch_db.rebuild_db();
        }

        // Layer 3: RSS polling.
        if !rss_unavailable_logged {
            match check_rss(&mem_config, &mut last_warn) {
                RssCheckResult::Ok => {}
                RssCheckResult::Warn { rss_mb, threshold_mb } => {
                    if !config.json {
                        eprintln!(
                            "ynz watch: memory warning — {rss_mb}MB (threshold: {threshold_mb}MB)"
                        );
                    }
                    if let Some(ref mut e) = emitter {
                        let (timestamp, schema_version) = ts();
                        let _ = e.emit(&JsonWatchEvent::MemoryWarning {
                            timestamp,
                            schema_version,
                            rss_mb,
                            threshold_mb,
                        });
                    }
                }
                RssCheckResult::Stop { rss_mb, threshold_mb } => {
                    let msg = hard_stop_message(rss_mb, threshold_mb, &config.path.display().to_string());
                    eprintln!("{msg}");
                    if let Some(ref mut e) = emitter {
                        let (timestamp, schema_version) = ts();
                        let _ = e.emit(&JsonWatchEvent::MemoryStop {
                            timestamp,
                            schema_version,
                            rss_mb,
                            threshold_mb,
                        });
                    }
                    oom_stop.set(true);
                }
                RssCheckResult::Unavailable => {
                    rss_unavailable_logged = true;
                    eprintln!("ynz watch: memory polling unavailable on this platform; \
                               hard-stop disabled for this session");
                    if let Some(ref mut e) = emitter {
                        let (timestamp, schema_version) = ts();
                        let _ = e.emit(&JsonWatchEvent::MemoryUnavailable {
                            timestamp,
                            schema_version,
                            reason: "polling unavailable on this platform".into(),
                        });
                    }
                }
            }
        }
    });

    // Kill child on exit (Ctrl+C path — Drop on current_child also fires as fallback).
    if let Some(ref mut c) = current_child {
        c.kill_gracefully(2000);
    }

    // Emit watch-shutdown. Exit 2 on OOM hard-stop.
    if oom_stop.get() {
        return 2;
    }
    if let Some(ref mut e) = emitter {
        let (timestamp, schema_version) = ts();
        let reason = if epipe.get() {
            ShutdownReason::PipeClosed
        } else {
            ShutdownReason::CtrlC
        };
        if let Err(err) = e.emit(&JsonWatchEvent::WatchShutdown {
            timestamp,
            schema_version,
            reason,
        }) {
            if is_epipe(&err) {
                rebuild::emit_pipe_closed_to_stderr();
            }
        }
    }

    // Cleanup tempdir on exit.
    let _ = std::fs::remove_dir_all(&out_dir);

    0
}

/// Run one rebuild + (optionally) child-spawn cycle.
///
/// Returns `true` if EPIPE was detected during JSON event emission (caller should exit loop).
fn run_rebuild_cycle(
    db: &mut WatchDb,
    entry_path: &std::path::Path,
    out_dir: &std::path::Path,
    config: &WatchConfig,
    current_child: &mut Option<ChildHandle>,
    emitter: Option<&mut JsonEmitter>,
) -> bool {
    use rebuild::rebuild_one_with_emitter;
    let (_, epipe) = rebuild_one_with_emitter(
        db,
        entry_path,
        entry_path,
        out_dir,
        config.check,
        current_child,
        emitter,
    );
    epipe
}

/// Check whether an I/O error is a broken pipe (EPIPE).
fn is_epipe(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::BrokenPipe
}

