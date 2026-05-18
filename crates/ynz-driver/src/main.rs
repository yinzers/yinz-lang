mod build;
mod load;
mod run;

use std::{path::PathBuf, process};

use clap::{Parser, Subcommand};

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
    Build {
        /// Source file or project root directory to compile.
        file: PathBuf,
    },
    /// Compile and immediately run a Yinz source file or project.
    Run {
        /// Source file or project root directory to compile and run.
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Build { file } => {
            let result = build::build(&file);
            if !result.success {
                eprint!("{}", result.stderr_output);
                process::exit(1);
            }
        }
        Command::Run { file } => {
            let code = run::run(&file);
            process::exit(code);
        }
    }
}
