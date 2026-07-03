mod build;
mod fmt;
mod json_diagnostic;
mod load;
mod run;
mod watch;

use std::{path::PathBuf, process};

use clap::{Parser, Subcommand};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const EXIT_OK: i32 = 0;
const EXIT_COMPILE_ERROR: i32 = 1;
const EXIT_INFRA_ERROR: i32 = 2;

const ICE_BANNER: &str = "\
\u{256d}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}\n\
\u{2502}  Yinz compiler bug \u{2014} not your code.                            \u{2502}\n\
\u{2502}                                                                 \u{2502}\n\
\u{2502}  ynz hit an internal panic and cannot continue. Your program    \u{2502}\n\
\u{2502}  may be fine. The details below help us fix the bug.            \u{2502}\n\
\u{2502}                                                                 \u{2502}\n\
\u{2502}  Please file an issue at:                                       \u{2502}\n\
\u{2502}  https://github.com/yinzers/yinz-lang/issues                  \u{2502}\n\
\u{2502}                                                                 \u{2502}\n\
\u{2502}  For a full backtrace, re-run with:                             \u{2502}\n\
\u{2502}  RUST_BACKTRACE=1 ynz build <your-file>                        \u{2502}\n\
\u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}";

/// The Yinz compiler.
#[derive(Parser)]
#[command(name = "ynz", version = VERSION, about = "The Yinz compiler")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile a Yinz source file or project to a native binary.
    ///
    /// Two modes:
    ///
    ///   Single file: `ynz build foo.ynz` — compiles one .ynz file standalone.
    ///
    ///   Project: `ynz build .` or `ynz build path/to/project/` — compiles
    ///   all .ynz files in the directory. A `yinz.toml` is required.
    ///
    /// The compiled binary lands next to the source (single-file mode) or in
    /// the project root (project mode).
    ///
    /// Pass `--emit-ir` to write LLVM IR alongside the binary (as `<binary>.ll`).
    /// Useful for inspecting what the backend produced.
    ///
    /// Exit codes:
    ///   0 — success
    ///   1 — source errors (your code has a problem)
    ///   2 — infrastructure error (missing linker, can't read/write files)
    Build {
        /// Source file or project root directory to compile.
        file: PathBuf,
        /// Dump LLVM IR alongside the compiled binary (writes to <binary>.ll).
        #[arg(long)]
        emit_ir: bool,
        /// Emit structured NDJSON diagnostic output on stdout instead of ariadne text.
        ///
        /// Each diagnostic is one JSON object per line. The final line is a summary
        /// object with error/warning/suggestion counts and an exit_code field.
        /// See `design/lsp.md` for the full schema.
        #[arg(long)]
        json: bool,
        /// Disables the auto-parallelization independence-analysis pass and everything
        /// that gates on its predicate (e.g. the v0.3-M4 false-sharing padding
        /// transform), lowering statements in plain source order as the cross-impl
        /// consistency oracle.
        ///
        /// This does NOT suppress explicit, user-written `background` spawns — those
        /// still run on real threads regardless of the flag (the runtime never reads
        /// it). Output determinism between the two modes comes from the language's
        /// channel-synchronization semantics, not from thread suppression.
        #[arg(long, hide = true)]
        no_auto_parallel: bool,
    },
    /// Compile and immediately run a Yinz source file or project.
    ///
    /// Same input modes as `build`. The binary is removed after the program
    /// exits unless `--keep` is passed.
    ///
    /// Pass `--emit-ir` to write LLVM IR alongside the binary (as `<binary>.ll`).
    ///
    /// Pass `--reveal-sensitive` to print sensitive values in plain text instead of
    /// `[REDACTED]`. Dev only — this flag will be stripped from release builds
    /// (`--release`, when it ships) to prevent accidental production leaks.
    ///
    /// The compiled program's exit code is returned to your shell.
    /// Exit code 2 from ynz itself means an infrastructure problem
    /// (missing linker, can't read source, etc.) — your code may be fine.
    Run {
        /// Source file or project root directory to compile and run.
        file: PathBuf,
        /// Keep the compiled binary after the program exits (default: delete).
        #[arg(long)]
        keep: bool,
        /// Dump LLVM IR alongside the compiled binary (writes to <binary>.ll).
        #[arg(long)]
        emit_ir: bool,
        /// Dev only: print sensitive values in plain text instead of [REDACTED].
        /// Will be stripped from release builds (`--release`) when it ships.
        #[arg(long)]
        reveal_sensitive: bool,
    },
    /// Format a Yinz source file or project in canonical style.
    ///
    /// Modes:
    ///
    ///   Single file: `ynz fmt foo.ynz` — rewrites in-place (atomic write).
    ///
    ///   Project: `ynz fmt --all [dir]` — walks yinz.toml project and formats every .ynz file.
    ///
    ///   Check: `ynz fmt --check foo.ynz` — read-only; exit 0 if canonical, exit 1 if file
    ///   would change. Prints which files would change. No filesystem writes.
    ///
    ///   Stdin: `ynz fmt --stdin` — read source on stdin, write formatted result to stdout.
    ///
    /// Exit codes:
    ///   0 — success / already canonical
    ///   1 — source has parse errors OR (for --check) file(s) would change
    ///   2 — infrastructure error (can't read/write file, missing yinz.toml for --all)
    Fmt {
        /// Source file to format (omit when using --all or --stdin).
        file: Option<PathBuf>,
        /// Walk the yinz.toml project and format every .ynz file.
        #[arg(long, conflicts_with = "stdin")]
        all: bool,
        /// Read-only check: exit 0 if canonical, exit 1 if file(s) would change.
        #[arg(long, conflicts_with = "stdin")]
        check: bool,
        /// Read source on stdin; write formatted output to stdout.
        #[arg(long, conflicts_with = "all", conflicts_with = "file")]
        stdin: bool,
    },
    /// Watch a Yinz source file or project for changes and rebuild on every save.
    ///
    /// Modes:
    ///
    ///   Single file: `ynz watch foo.ynz` — watches `foo.ynz`; rebuilds + re-runs on save.
    ///
    ///   Project: `ynz watch .` or `ynz watch ./path/` — reads `yinz.toml` at the root;
    ///   watches all `.ynz` files; rebuilds + re-runs on any save.
    ///
    /// Flags:
    ///
    ///   `--check`    — build only; do not execute the program after a successful build.
    ///                  Exit code 1 on first-build compile failure (useful for CI).
    ///
    ///   `--json`     — emit a structured NDJSON event stream on stdout instead of
    ///                  normal status lines. One JSON object per line. See `design/watch.md`
    ///                  for the full event schema.
    ///
    ///   `--no-clear` — do not clear the terminal between rebuild cycles. Useful when
    ///                  watching CI logs or debugging the watcher itself.
    ///
    /// Tuning (via environment variables, documented in `design/watch.md`):
    ///   YNZ_WATCH_DEBOUNCE_MS, YNZ_WATCH_REBUILD_AFTER, YNZ_WATCH_MAX_RSS_MB, etc.
    ///
    /// Exit codes:
    ///   0 — clean exit (Ctrl+C or pipe closed)
    ///   1 — first-build compile failure with --check
    ///   2 — infrastructure error (can't start watcher, no yinz.toml, memory hard-stop)
    Watch {
        /// Source file or project root directory to watch (defaults to current directory).
        file: Option<PathBuf>,
        /// Build only; do not execute the compiled program.
        #[arg(long)]
        check: bool,
        /// Emit structured NDJSON event stream on stdout instead of normal status output.
        #[arg(long)]
        json: bool,
        /// Do not clear the terminal between rebuild cycles.
        #[arg(long)]
        no_clear: bool,
    },
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => (*s).to_string(),
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => s.clone(),
                None => "(no message)".to_string(),
            },
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "(unknown location)".to_string());
        eprintln!("{ICE_BANNER}");
        eprintln!("\npanic message: {msg}");
        eprintln!("location:      {location}");
        process::exit(EXIT_INFRA_ERROR);
    }));

    let cli = Cli::parse();

    match cli.command {
        Command::Build {
            file,
            emit_ir,
            json,
            no_auto_parallel,
        } => {
            if json {
                // --json mode: run check_query only (no codegen/link); emit NDJSON to stdout.
                // Default ariadne output is suppressed. Exit code mirrors the process exit.
                let exit_code = build::build_json(&file);
                process::exit(exit_code);
            }

            // Thread --no-auto-parallel through the salsa barrier via env var.
            // codegen_query is salsa-tracked and cannot take extra parameters; emit_artifact
            // (a normal Rust function called inside codegen_query) reads this env var to
            // decide whether to skip the independence-analysis pass entirely.
            // The env var is set BEFORE the build call so it is visible during salsa dispatch.
            if no_auto_parallel {
                std::env::set_var("YNZ_NO_AUTO_PARALLEL", "1");
            }

            let result = build::build(&file);

            if result.success {
                if let Some(binary) = &result.binary {
                    println!("Build succeeded: {}", binary.display());
                    if emit_ir {
                        if let Some(ir) = &result.ir_text {
                            let ir_path = binary.with_extension("ll");
                            if let Err(e) = std::fs::write(&ir_path, ir) {
                                eprintln!(
                                    "ynz: could not write IR to `{}`: {e}",
                                    ir_path.display()
                                );
                                process::exit(EXIT_INFRA_ERROR);
                            }
                            println!("LLVM IR written to: {}", ir_path.display());
                        }
                    }
                }
                if !result.stderr_output.is_empty() {
                    eprint!("{}", result.stderr_output);
                }
                process::exit(EXIT_OK);
            } else {
                eprint!("{}", result.stderr_output);
                let code = match &result.failure_kind {
                    Some(build::FailureKind::InfraError) => EXIT_INFRA_ERROR,
                    _ => EXIT_COMPILE_ERROR,
                };
                process::exit(code);
            }
        }
        Command::Run {
            file,
            keep,
            emit_ir,
            reveal_sensitive,
        } => {
            let code = run::run(&file, keep, emit_ir, reveal_sensitive);
            process::exit(code);
        }
        Command::Fmt {
            file,
            all,
            check,
            stdin,
        } => {
            let code = fmt::fmt(file.as_deref(), all, check, stdin);
            process::exit(code);
        }
        Command::Watch {
            file,
            check,
            json,
            no_clear,
        } => {
            let code = watch::watch(file.as_deref(), check, json, no_clear);
            process::exit(code);
        }
    }
}
