use std::{path::Path, process};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

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
///
/// When `reveal_sensitive` is true, `YNZ_REVEAL_SENSITIVE=1` is set in the child
/// process environment. The runtime reads this to print sensitive values in plain text
/// instead of `[REDACTED]`. Dev-only flag; stripped from release builds when `--release`
/// ships.
pub fn run(source_path: &Path, keep: bool, emit_ir: bool, reveal_sensitive: bool) -> i32 {
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

    let mut cmd = process::Command::new(&binary);
    if reveal_sensitive {
        cmd.env("YNZ_REVEAL_SENSITIVE", "1");
    }
    let status = cmd.status().unwrap_or_else(|e| {
        eprintln!("ynz: failed to run `{}`: {e}", binary.display());
        process::exit(2);
    });

    if keep {
        // Consume the TempDir without running its cleanup — both the directory
        // and the binary inside it persist after this function returns. The
        // user is told where to find it.
        let kept_dir = bin_dir.keep();
        println!("Binary kept at: {}", binary.display());
        let _ = kept_dir;
    }
    // When !keep, bin_dir drops at the end of scope and removes the directory
    // (and the binary inside it).

    exit_code_for(&status)
}

/// Translate the produced binary's `ExitStatus` into the exit code `ynz run` itself
/// returns to its own caller (the shell, or a CI runner consuming `ynz run`'s exit code).
///
/// Normal exit (`status.code()` is `Some`) is propagated unchanged — this is the
/// existing, unmodified contract documented on [`run`] ("Propagates the exit code of
/// the produced binary.").
///
/// Signal termination (Unix only — `status.code()` returns `None` when a process is
/// killed by a signal rather than exiting normally) reports the signal by name on
/// stderr and returns `128 + signal number`, the same convention POSIX shells use for
/// `$?` on a signal-terminated child. A crash is distinguishable from the compiled
/// Yinz program calling `exit(1)` on purpose: the two report different exit codes and
/// only the crash path prints a signal name.
#[cfg(unix)]
fn exit_code_for(status: &process::ExitStatus) -> i32 {
    match status.code() {
        Some(code) => code,
        None => {
            // `ExitStatus` on Unix is documented to carry exactly one of code()/signal()
            // — code() being None here means signal() must be Some. Fail safe (exit 1)
            // rather than panic if that invariant is ever violated on some future target.
            let Some(sig) = status.signal() else {
                return 1;
            };
            let name = nix::sys::signal::Signal::try_from(sig)
                .map(|s| s.as_str().to_string())
                .unwrap_or_else(|_| format!("signal {sig}"));
            eprintln!(
                "RUNTIME ERROR: the program was killed by signal {sig} ({name}).\n\n  \
                 Check for unbounded recursion or unsafe array indexing — the two most \
                 common causes. If the source looks correct, file a bug with a minimal \
                 repro.\n\n  \
                 Why: {name} means the program tried to access memory it doesn't own. \
                 This can also happen if the compiler produced incorrect code (a compiler \
                 miscompile) — Yinz reports the signal by name instead of masking it as a \
                 bare exit code so a real crash is never indistinguishable from the \
                 program exiting on purpose."
            );
            128 + sig
        }
    }
}

#[cfg(not(unix))]
fn exit_code_for(status: &process::ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}
