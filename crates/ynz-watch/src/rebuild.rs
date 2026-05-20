use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use ynz_diagnostics::render;

use crate::{
    db::{RebuildOutcome, WatchDb},
    error::{Result, WatchError},
    ui,
};

/// Outcome of a single rebuild-and-maybe-spawn cycle.
pub enum CycleOutcome {
    // CARVE-OUT: `binary` is part of the public CycleOutcome contract; consumed by callers that spawn the compiled program.
    #[allow(dead_code)]
    /// Build succeeded; binary written to the given path.
    Success { binary: PathBuf },
    /// Build produced compile errors (already rendered to stdout).
    Errors,
    // CARVE-OUT: String payload exposed for callers that surface infra failures (--json emitters, log sinks).
    #[allow(dead_code)]
    /// Infrastructure failure (read error, codegen write failure, etc.).
    Infra(String),
}

/// Orchestrate one rebuild cycle for the given entry path.
///
/// # Flow
///
/// 1. Read the changed file from disk and update the DB.
/// 2. Run `codegen_query` via `db.run_codegen(entry_path)`.
/// 3. On errors: render diagnostics and return `CycleOutcome::Errors`.
/// 4. On success: write the object bytes to `out_dir/binary`, return the path.
///
/// # Failure modes
///
/// - File unreadable: returns `CycleOutcome::Infra` with WHAT/WHAT-INSTEAD/WHY.
/// - Object-write failure (disk full, permissions): returns `CycleOutcome::Infra`.
/// - Compile errors: returns `CycleOutcome::Errors` after rendering diagnostics.
///
/// # Side effects
///
/// Writes status lines to stdout via `ui::`. Writes the compiled binary to `out_dir`.
///
/// Time: O(1) amortized (salsa caches unchanged queries). Space: O(object_bytes) per cycle.
pub fn rebuild_one(
    db: &mut WatchDb,
    changed_path: &Path,
    entry_path: &Path,
    out_dir: &Path,
    check_only: bool,
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
    db.update_source(changed_path, text);

    let path_str = entry_path.display().to_string();
    ui::print_building(&path_str);

    // 3. Run compiler pipeline.
    let mut outcome = db.run_codegen(entry_path);
    let elapsed = start.elapsed().as_millis();

    // Patch elapsed_ms into the outcome (db.run_codegen returns 0 — measured here).
    match &mut outcome {
        RebuildOutcome::Success { elapsed_ms, .. } => *elapsed_ms = elapsed,
        RebuildOutcome::Errors { elapsed_ms, .. } => *elapsed_ms = elapsed,
        _ => {}
    }

    match &outcome {
        RebuildOutcome::Errors { diags, sources, elapsed_ms } => {
            // Print rendered diagnostics then the error summary line.
            let rendered = render(diags, sources, false);
            if !rendered.is_empty() {
                print!("{rendered}");
            }
            ui::print_errors(outcome.error_count(), *elapsed_ms);
            CycleOutcome::Errors
        }

        RebuildOutcome::Success { object_bytes, elapsed_ms } => {
            if check_only {
                ui::print_success(*elapsed_ms);
                // --check: no binary written, return a dummy path.
                return CycleOutcome::Success {
                    binary: PathBuf::new(),
                };
            }

            // 4. Write binary to out_dir.
            let binary_name = entry_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "program".to_string());
            let binary_path = out_dir.join(&binary_name);

            if let Err(e) = write_binary(object_bytes, &binary_path) {
                eprintln!("{e}");
                return CycleOutcome::Infra(e.to_string());
            }

            ui::print_success(*elapsed_ms);
            CycleOutcome::Success { binary: binary_path }
        }

        RebuildOutcome::Infra(msg) => {
            eprintln!("{msg}");
            CycleOutcome::Infra(msg.clone())
        }
    }
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

    // Link.
    let status = Command::new("cc")
        .arg("-o")
        .arg(binary_path)
        .arg(&obj_path)
        .status()
        .map_err(|e| WatchError::CodegenWrite {
            path: binary_path.to_path_buf(),
            reason: format!("could not invoke linker `cc`: {e}"),
        })?;

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

