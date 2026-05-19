mod build;
mod load;
mod run;

use std::{path::PathBuf, process};

use clap::{Parser, Subcommand};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const EXIT_OK: i32 = 0;
const EXIT_COMPILE_ERROR: i32 = 1;
const EXIT_INFRA_ERROR: i32 = 2;

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
    /// Exit codes:
    ///   0 — success
    ///   1 — source errors (your code has a problem)
    ///   2 — infrastructure error (missing linker, can't read/write files)
    Build {
        /// Source file or project root directory to compile.
        file: PathBuf,
    },
    /// Compile and immediately run a Yinz source file or project.
    ///
    /// Same input modes as `build`. The binary is removed after the program
    /// exits unless `--keep` is passed.
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
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Build { file } => {
            let result = build::build(&file);

            if result.success {
                if let Some(binary) = &result.binary {
                    println!("Build succeeded: {}", binary.display());
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
        Command::Run { file, keep } => {
            let code = run::run(&file, keep);
            process::exit(code);
        }
    }
}
