use std::{path::Path, process};

use crate::build::{build_into, FailureKind};

/// Build and execute `source_path`. Propagates the exit code of the produced binary.
///
/// The binary is placed in a per-invocation temp directory (mode 0o700, random name)
/// so it is isolated from the project tree and never races with other concurrent builds.
///
/// When `keep` is false (default), the binary is removed after execution.
/// When `keep` is true, the binary is left in place.
///
/// When `emit_ir` is true, the LLVM IR is written alongside the binary as `<binary>.ll`.
pub fn run(source_path: &Path, keep: bool, emit_ir: bool) -> i32 {
    let bin_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ynz: could not create temp directory for binary: {e}");
            return 2;
        }
    };

    let result = build_into(source_path, bin_dir.path());

    if !result.success {
        eprint!("{}", result.stderr_output);
        return match &result.failure_kind {
            Some(FailureKind::InfraError) => 2,
            _ => 1,
        };
    }

    // Emit warnings even on success so users see them.
    if !result.stderr_output.is_empty() {
        eprint!("{}", result.stderr_output);
    }

    let binary = result.binary.expect("success implies binary is set");

    if emit_ir {
        if let Some(ir) = &result.ir_text {
            let ir_path = binary.with_extension("ll");
            if let Err(e) = std::fs::write(&ir_path, ir) {
                eprintln!("ynz: could not write IR to `{}`: {e}", ir_path.display());
                return 2;
            }
            println!("LLVM IR written to: {}", ir_path.display());
        }
    }

    let status = process::Command::new(&binary).status().unwrap_or_else(|e| {
        eprintln!("ynz: failed to run `{}`: {e}", binary.display());
        process::exit(2);
    });

    if keep {
        // Consume the TempDir without running its cleanup — both the directory
        // and the binary inside it persist after this function returns. The
        // user is told where to find it.
        let kept_dir = bin_dir.into_path();
        println!("Binary kept at: {}", binary.display());
        let _ = kept_dir;
    }
    // When !keep, bin_dir drops at the end of scope and removes the directory
    // (and the binary inside it).

    status.code().unwrap_or(1)
}
