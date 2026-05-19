mod build;
mod load;
mod run;

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
\u{2502}  https://github.com/patrickrizzardi/ynz/issues                  \u{2502}\n\
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
    },
    /// Compile and immediately run a Yinz source file or project.
    ///
    /// Same input modes as `build`. The binary is removed after the program
    /// exits unless `--keep` is passed.
    ///
    /// Pass `--emit-ir` to write LLVM IR alongside the binary (as `<binary>.ll`).
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
        Command::Build { file, emit_ir } => {
            let result = build::build(&file);

            if result.success {
                if let Some(binary) = &result.binary {
                    println!("Build succeeded: {}", binary.display());
                    if emit_ir {
                        if let Some(ir) = &result.ir_text {
                            let ir_path = binary.with_extension("ll");
                            if let Err(e) = std::fs::write(&ir_path, ir) {
                                eprintln!("ynz: could not write IR to `{}`: {e}", ir_path.display());
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
        Command::Run { file, keep, emit_ir } => {
            let code = run::run(&file, keep, emit_ir);
            process::exit(code);
        }
    }
}
