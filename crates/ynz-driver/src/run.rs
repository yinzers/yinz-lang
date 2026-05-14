use std::{path::Path, process};

use crate::build::build;

/// Build and execute `source_path`. Propagates the exit code of the produced binary.
pub fn run(source_path: &Path) -> i32 {
    let result = build(source_path);

    if !result.success {
        eprint!("{}", result.stderr_output);
        return 1;
    }

    // Emit warnings even on success so users see them.
    if !result.stderr_output.is_empty() {
        eprint!("{}", result.stderr_output);
    }

    let binary = result.binary.expect("success implies binary is set");

    let status = process::Command::new(&binary).status().unwrap_or_else(|e| {
        eprintln!("ynz: failed to run `{}`: {e}", binary.display());
        process::exit(1);
    });

    // Clean up the binary after running.
    let _ = std::fs::remove_file(&binary);

    status.code().unwrap_or(1)
}
