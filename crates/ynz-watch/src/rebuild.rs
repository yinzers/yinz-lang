use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

// Runtime library bytes — embedded at compile time, extracted to a temp file at link time.
// Same mechanism as ynz-driver/src/build.rs. The ynz-watch build.rs emits YNZ_RT_LIB_PATH.
static RUNTIME_LIB_BYTES: &[u8] = include_bytes!(env!("YNZ_RT_LIB_PATH"));

use ynz_diagnostics::render;

use crate::{
    child::ChildHandle,
    db::{RebuildOutcome, WatchDb},
    error::{Result, WatchError},
    json_emitter::JsonEmitter,
    json_events::{ts, BuildOutcome, WatchEvent as JsonEvent},
    ui,
};

/// Outcome of a single rebuild-and-maybe-spawn cycle.
pub enum CycleOutcome {
    /// Build succeeded; binary written to the given path.
    // CARVE-OUT: `binary` consumed by --json ChildSpawn event emitters.
    #[allow(dead_code)]
    Success { binary: PathBuf },
    /// Build produced compile errors (already rendered to stdout).
    Errors,
    /// Infrastructure failure (read error, codegen write failure, etc.).
    // CARVE-OUT: String payload exposed for callers that surface infra failures (--json emitters, log sinks).
    #[allow(dead_code)]
    Infra(String),
}

/// Orchestrate one rebuild cycle for the given entry path.
///
/// # Flow
///
/// 1. Read the changed file from disk and update the DB.
/// 2. Run `codegen_query` via `db.run_codegen(entry_path)`.
/// 3. On errors: render diagnostics and return `CycleOutcome::Errors`.
/// 4. On success (non-check): kill prior child (SIGTERM → 2s → SIGKILL), spawn new.
///
/// # Failure modes
///
/// - File unreadable: returns `CycleOutcome::Infra` with WHAT/WHAT-INSTEAD/WHY.
/// - Object-write failure (disk full, permissions): returns `CycleOutcome::Infra`.
/// - Compile errors: returns `CycleOutcome::Errors` after rendering diagnostics.
/// - Child spawn failure: returns `CycleOutcome::Infra`; no child is left running.
///
/// # Side effects
///
/// Writes status lines to stdout via `ui::`. Writes the compiled binary to `out_dir`.
/// Kills the prior child process (via SIGTERM → SIGKILL grace period).
///
/// Time: O(1) amortized (salsa caches unchanged queries) + up to 2s SIGTERM grace.
/// Space: O(object_bytes) per cycle.
pub fn rebuild_one(
    db: &mut WatchDb,
    changed_path: &Path,
    entry_path: &Path,
    out_dir: &Path,
    check_only: bool,
    current_child: &mut Option<ChildHandle>,
    json_mode: bool,
) -> CycleOutcome {
    let start = Instant::now();

    // 1. Read updated source from disk.
    let text = match fs::read_to_string(changed_path) {
        Ok(t) => t,
        Err(e) => {
            let msg = format!(
                "WHAT: Could not read `{}`.\n\
                 WHAT INSTEAD: Check that the file exists and is readable.\n\
                 WHY: {e}",
                changed_path.display()
            );
            eprintln!("{msg}");
            return CycleOutcome::Infra(msg);
        }
    };

    // 2. Update DB (shadow FIRST, then salsa input).
    // Skip if unchanged — prevents IN_OPEN feedback loop from notify 8.x.
    if db.source_unchanged(changed_path, &text) {
        return CycleOutcome::Infra("no-change".into());
    }
    db.update_source(changed_path, text);

    let path_str = entry_path.display().to_string();
    if !json_mode {
        ui::print_building(&path_str);
    }

    // 3. Run compiler pipeline.
    let mut outcome = db.run_codegen(entry_path);
    let elapsed = start.elapsed().as_millis();

    // Patch elapsed_ms into the outcome (db.run_codegen returns 0 — measured here).
    match &mut outcome {
        RebuildOutcome::Success { elapsed_ms, .. } => *elapsed_ms = elapsed,
        RebuildOutcome::Errors { elapsed_ms, .. } => *elapsed_ms = elapsed,
        _ => {}
    }

    // Text-mode diagnostic rendering.
    if !json_mode {
        if let RebuildOutcome::Errors {
            ref diags,
            ref sources,
            elapsed_ms,
        } = outcome
        {
            let rendered = render(diags, sources, false);
            if !rendered.is_empty() {
                print!("{rendered}");
            }
            ui::print_errors(outcome.error_count(), elapsed_ms);
        }
    }

    // Shared binary-write + child-spawn step.
    let cycle = finish_rebuild(outcome, entry_path, out_dir, check_only, current_child);

    // Text-mode success line.
    if !json_mode && matches!(cycle, CycleOutcome::Success { .. }) {
        let elapsed = start.elapsed().as_millis();
        ui::print_success(elapsed);
    }

    cycle
}

/// Write raw object bytes to the binary path using the linker.
///
/// The object bytes from codegen_query are a relocatable `.o`; we link them via the
/// system linker (cc/ld) to produce an executable — mirrors the existing build pipeline.
///
/// Failure modes:
/// - linker not found → `WatchError::CodegenWrite`
/// - linker exits non-zero → `WatchError::CodegenWrite` with stderr
/// - write to temp .o fails → `WatchError::CodegenWrite`
///
/// Time: O(1) compile + O(n) link where n = object size. Space: O(n).
fn write_binary(object_bytes: &[u8], binary_path: &Path) -> Result<()> {
    use std::process::Command;

    // Write object file next to binary path.
    let obj_path = binary_path.with_extension("o");
    fs::write(&obj_path, object_bytes).map_err(|e| WatchError::CodegenWrite {
        path: obj_path.clone(),
        reason: e.to_string(),
    })?;

    // Extract embedded runtime library to a temp file for the linker.
    let rt_tmp = tempfile::NamedTempFile::new().map_err(|e| WatchError::CodegenWrite {
        path: binary_path.to_path_buf(),
        reason: format!("could not create temp file for runtime library: {e}"),
    })?;
    fs::write(rt_tmp.path(), RUNTIME_LIB_BYTES).map_err(|e| WatchError::CodegenWrite {
        path: binary_path.to_path_buf(),
        reason: format!("could not write runtime library to temp file: {e}"),
    })?;

    // Link — probe the same candidates as ynz build (clang-18 first).
    let linker = ["clang-18", "clang", "cc", "gcc", "g++"]
        .iter()
        .find(|&&c| {
            std::process::Command::new(c)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        })
        .copied()
        .unwrap_or("cc");

    let status = Command::new(linker)
        .arg("-no-pie") // LLVM emits non-PIC relocations; -no-pie matches (mirrors ynz build)
        .arg("-o")
        .arg(binary_path)
        .arg(&obj_path)
        .arg(rt_tmp.path()) // runtime library: ynz_string_*, ynz_siphash_*, etc.
        .status()
        .map_err(|e| WatchError::CodegenWrite {
            path: binary_path.to_path_buf(),
            reason: format!("could not invoke linker `{linker}`: {e}"),
        })?;
    // rt_tmp dropped here — linker already finished reading it.

    // Remove object file regardless of link outcome.
    let _ = fs::remove_file(&obj_path);

    if !status.success() {
        return Err(WatchError::CodegenWrite {
            path: binary_path.to_path_buf(),
            reason: format!("linker exited with status {status}"),
        });
    }

    // Make binary executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(binary_path)
            .map_err(|e| WatchError::CodegenWrite {
                path: binary_path.to_path_buf(),
                reason: e.to_string(),
            })?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(binary_path, perms).map_err(|e| WatchError::CodegenWrite {
            path: binary_path.to_path_buf(),
            reason: e.to_string(),
        })?;
    }

    Ok(())
}

/// Binary-write + child-spawn step, shared between text-mode and JSON-mode paths.
///
/// Takes the raw `RebuildOutcome` (already timed) and produces the final `CycleOutcome`.
/// Calling this from both paths keeps the write + spawn logic in one place.
///
/// Time: O(object_bytes) for link + O(1) for spawn. Space: O(object_bytes).
fn finish_rebuild(
    raw_outcome: RebuildOutcome,
    entry_path: &Path,
    out_dir: &Path,
    check_only: bool,
    current_child: &mut Option<ChildHandle>,
) -> CycleOutcome {
    match raw_outcome {
        RebuildOutcome::Errors { .. } => CycleOutcome::Errors,

        RebuildOutcome::Success {
            ref object_bytes, ..
        } => {
            if check_only {
                return CycleOutcome::Success {
                    binary: PathBuf::new(),
                };
            }
            let binary_name = entry_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "program".to_string());
            let binary_path = out_dir.join(&binary_name);

            if let Err(e) = write_binary(object_bytes, &binary_path) {
                eprintln!("{e}");
                return CycleOutcome::Infra(e.to_string());
            }

            if let Some(ref mut old) = current_child {
                old.kill_gracefully(2000);
            }
            *current_child = None;

            match ChildHandle::spawn(&binary_path) {
                Ok(c) => {
                    *current_child = Some(c);
                    CycleOutcome::Success {
                        binary: binary_path,
                    }
                }
                Err(e) => {
                    eprintln!("{e}");
                    CycleOutcome::Infra(e.to_string())
                }
            }
        }

        RebuildOutcome::Infra(msg) => {
            eprintln!("{msg}");
            CycleOutcome::Infra(msg)
        }
    }
}

/// Rebuild cycle with optional JSON event emission.
///
/// Wraps `rebuild_one` with JSON event emission for `--json` mode.
/// Emits: `build-start`, `diagnostic`* (one per compile error), `build-end`, and
/// optionally `child-spawn` after a successful non-check rebuild.
///
/// Caller owns `emitter`; passing `None` for text-mode output.
///
/// Time: same as `rebuild_one` + O(d) JSON serialization where d = diagnostic count.
/// Space: O(d) for diagnostic JSON.
#[allow(clippy::too_many_arguments)]
/// Returns `(CycleOutcome, epipe_detected: bool)`.
/// If `epipe_detected` is true, the downstream JSON pipe closed — caller should exit the loop.
pub fn rebuild_one_with_emitter(
    db: &mut WatchDb,
    changed_path: &Path,
    entry_path: &Path,
    out_dir: &Path,
    check_only: bool,
    current_child: &mut Option<ChildHandle>,
    mut emitter: Option<&mut JsonEmitter>,
    // force=true: skip the no-change guard. Set for the initial build where the shadow is
    // pre-populated by WatchDb::from_target; content will appear unchanged even though
    // no prior compile has run. Event-triggered rebuilds pass false.
    force: bool,
    // no_clear=true: skip terminal clear before printing build status (--no-clear flag or
    // initial build where we don't want to wipe any prior output).
    no_clear: bool,
) -> (CycleOutcome, bool) {
    let start = Instant::now();
    let mut epipe = false;

    let json_mode = emitter.is_some();

    if let Some(ref mut e) = emitter {
        let (timestamp, schema_version) = ts();
        if let Err(err) = e.emit(&JsonEvent::BuildStart {
            timestamp,
            schema_version,
            file: entry_path.display().to_string(),
        }) {
            if err.kind() == std::io::ErrorKind::BrokenPipe {
                emit_pipe_closed_to_stderr();
                return (CycleOutcome::Errors, true);
            }
        }
    }

    // Run the compile step directly to preserve the raw RebuildOutcome (with diagnostics).
    let text = match fs::read_to_string(changed_path) {
        Ok(t) => t,
        Err(e) => {
            let msg = format!(
                "WHAT: Could not read `{}`.\n\
                 WHAT INSTEAD: Check that the file exists and is readable.\n\
                 WHY: {e}",
                changed_path.display()
            );
            if let Some(ref mut e_emitter) = emitter {
                let (timestamp, schema_version) = ts();
                let _ = e_emitter.emit(&JsonEvent::BuildEnd {
                    timestamp,
                    schema_version,
                    file: entry_path.display().to_string(),
                    outcome: BuildOutcome::Errors,
                    duration_ms: start.elapsed().as_millis(),
                });
            }
            return (CycleOutcome::Infra(msg), false);
        }
    };

    // Break the IN_OPEN feedback loop: notify 8.x watches IN_OPEN, so every
    // fs::read_to_string fires a new inotify event. If content is identical to
    // the shadow state, skip the rebuild — no user-visible change occurred.
    // `force` bypasses this for the initial build (shadow pre-populated, no prior compile).
    if !force && db.source_unchanged(changed_path, &text) {
        return (CycleOutcome::Infra("no-change".into()), false);
    }

    db.update_source(changed_path, text);

    if !json_mode {
        ui::clear(no_clear);
        ui::print_building(&entry_path.display().to_string());
    }

    let mut raw_outcome = db.run_codegen(entry_path);
    let elapsed_ms = start.elapsed().as_millis();
    match &mut raw_outcome {
        RebuildOutcome::Success { elapsed_ms: em, .. } => *em = elapsed_ms,
        RebuildOutcome::Errors { elapsed_ms: em, .. } => *em = elapsed_ms,
        _ => {}
    }

    // Emit diagnostic events (one per compile error) before build-end.
    if let Some(ref mut e_emitter) = emitter {
        if let RebuildOutcome::Errors {
            ref diags,
            ref sources,
            ..
        } = raw_outcome
        {
            for diag in diags.iter() {
                use ynz_diagnostics::Severity;
                let severity = match diag.severity {
                    Severity::Error => crate::json_events::DiagnosticSeverity::Error,
                    Severity::Warning => crate::json_events::DiagnosticSeverity::Warning,
                    Severity::Suggestion => crate::json_events::DiagnosticSeverity::Suggestion,
                };
                let (timestamp, schema_version) = ts();
                let _ = e_emitter.emit(&JsonEvent::Diagnostic {
                    timestamp,
                    schema_version,
                    file: diag.span.file.clone(),
                    severity,
                    span: crate::json_events::SourceSpanJson {
                        start: diag.span.start,
                        end: diag.span.end,
                    },
                    what: diag.what.clone(),
                    what_instead: diag.what_instead.clone(),
                    why: diag.why.clone(),
                });
            }
            let _ = sources; // sources used by text-mode render; not needed for JSON
        }
    }

    // Text-mode: render diagnostics before finish_rebuild moves raw_outcome.
    if !json_mode {
        if let RebuildOutcome::Errors {
            ref diags,
            ref sources,
            elapsed_ms,
        } = raw_outcome
        {
            let rendered = ynz_diagnostics::render(diags, sources, false);
            if !rendered.is_empty() {
                print!("{rendered}");
            }
            ui::print_errors(raw_outcome.error_count(), elapsed_ms);
        }
    }

    // Convert raw outcome to CycleOutcome, handling binary-write + child-spawn.
    let outcome = finish_rebuild(raw_outcome, entry_path, out_dir, check_only, current_child);

    if let Some(ref mut e) = emitter {
        match &outcome {
            CycleOutcome::Success { .. } => {
                let (timestamp, schema_version) = ts();
                if let Err(err) = e.emit(&JsonEvent::BuildEnd {
                    timestamp,
                    schema_version,
                    file: entry_path.display().to_string(),
                    outcome: BuildOutcome::Ok,
                    duration_ms: elapsed_ms,
                }) {
                    if err.kind() == std::io::ErrorKind::BrokenPipe {
                        epipe = true;
                    }
                }
                if !epipe && !check_only {
                    if let Some(ref ch) = current_child {
                        let (timestamp, schema_version) = ts();
                        if let Err(err) = e.emit(&JsonEvent::ChildSpawn {
                            timestamp,
                            schema_version,
                            pid: ch.pid(),
                        }) {
                            if err.kind() == std::io::ErrorKind::BrokenPipe {
                                epipe = true;
                            }
                        }
                    }
                }
            }
            CycleOutcome::Errors => {
                let (timestamp, schema_version) = ts();
                if let Err(err) = e.emit(&JsonEvent::BuildEnd {
                    timestamp,
                    schema_version,
                    file: entry_path.display().to_string(),
                    outcome: BuildOutcome::Errors,
                    duration_ms: elapsed_ms,
                }) {
                    if err.kind() == std::io::ErrorKind::BrokenPipe {
                        epipe = true;
                    }
                }
            }
            CycleOutcome::Infra(_) => {
                let (timestamp, schema_version) = ts();
                if let Err(err) = e.emit(&JsonEvent::BuildEnd {
                    timestamp,
                    schema_version,
                    file: entry_path.display().to_string(),
                    outcome: BuildOutcome::Errors,
                    duration_ms: elapsed_ms,
                }) {
                    if err.kind() == std::io::ErrorKind::BrokenPipe {
                        epipe = true;
                    }
                }
            }
        }
    }

    // Text-mode success line.
    if !json_mode && matches!(outcome, CycleOutcome::Success { .. }) {
        ui::print_success(start.elapsed().as_millis());
    }

    (outcome, epipe)
}

/// Emit "pipe-closed" last-ditch message to stderr when EPIPE fires on stdout.
pub fn emit_pipe_closed_to_stderr() {
    eprintln!("ynz watch: downstream consumer closed the pipe (watch-shutdown: pipe-closed)");
}
