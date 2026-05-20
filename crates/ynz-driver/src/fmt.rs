use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
};

use ynz_diagnostics::render;

use crate::load::find_project_root;

const EXIT_OK: i32 = 0;
const EXIT_SRC_ERROR: i32 = 1;
const EXIT_INFRA_ERROR: i32 = 2;

/// Dispatch to the appropriate formatter mode based on the CLI flags.
///
/// Called from `main.rs` with the parsed Clap arguments.
pub fn fmt(file: Option<&Path>, all: bool, check: bool, stdin: bool) -> i32 {
    if stdin {
        return fmt_stdin();
    }
    if all {
        let dir = file.unwrap_or(Path::new("."));
        return fmt_all(dir, check);
    }
    match file {
        Some(p) => fmt_single(p, check),
        None => {
            eprintln!("ynz fmt: provide a file path or use --all or --stdin");
            EXIT_INFRA_ERROR
        }
    }
}

// ── Single-file mode ──────────────────────────────────────────────────────────

/// Format (or check) a single `.ynz` file.
///
/// - `check_only = false` (default): rewrite in-place with an atomic same-dir tempfile rename.
/// - `check_only = true` (`--check`): read-only; print `Would reformat: {path}` to stderr if the
///   file would change; return exit 1 if so.
fn fmt_single(path: &Path, check_only: bool) -> i32 {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ynz fmt: cannot read `{}`: {e}", path.display());
            return EXIT_INFRA_ERROR;
        }
    };

    let file_name = path.display().to_string();
    match ynz_fmt::format_named(&source, &file_name) {
        Err(ynz_fmt::FmtError::ParseError(diags)) => {
            let mut sources = HashMap::new();
            sources.insert(file_name, source.clone());
            eprint!("{}", render(&diags, &sources, false));
            EXIT_SRC_ERROR
        }
        Err(ynz_fmt::FmtError::InvalidInput(msg)) => {
            eprintln!("ynz fmt: invalid input ({}): {msg}", path.display());
            EXIT_INFRA_ERROR
        }
        Ok(formatted) => {
            if formatted == source {
                return EXIT_OK;
            }
            if check_only {
                eprintln!("Would reformat: {}", path.display());
                return EXIT_SRC_ERROR;
            }
            write_atomic(path, &formatted)
        }
    }
}

/// Write `content` to `path` via an atomic tempfile-rename in the same directory.
///
/// Same-dir rename is atomic on every Unix filesystem; cross-filesystem rename would
/// fail with EXDEV and degrade to non-atomic copy+delete.
fn write_atomic(path: &Path, content: &str) -> i32 {
    let dir = path.parent().unwrap_or(Path::new("."));
    let tmp_path = dir.join(format!(".ynz_fmt_{}.tmp", std::process::id()));

    if let Err(e) = std::fs::write(&tmp_path, content) {
        eprintln!(
            "ynz fmt: cannot write temp file `{}`: {e}",
            tmp_path.display()
        );
        return EXIT_INFRA_ERROR;
    }

    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        eprintln!(
            "ynz fmt: cannot rename `{}` → `{}`: {e}",
            tmp_path.display(),
            path.display()
        );
        return EXIT_INFRA_ERROR;
    }

    eprintln!("ynz fmt: rewrote {}", path.display());
    EXIT_OK
}

// ── Project mode (--all) ──────────────────────────────────────────────────────

/// Format (or check) every `.ynz` file in the project rooted at `dir`.
///
/// Requires a `yinz.toml` to locate the project root.  Continues on individual
/// file errors (parse errors or infra errors); collects and returns the worst exit
/// code seen: 2 (infra) > 1 (parse) > 0 (clean).
fn fmt_all(dir: &Path, check_only: bool) -> i32 {
    let project_root = match find_project_root(dir) {
        Some(r) => r,
        None => {
            eprintln!(
                "ynz fmt: no `yinz.toml` found above `{}` — pass a path directly or create a yinz.toml project",
                dir.display()
            );
            return EXIT_INFRA_ERROR;
        }
    };

    let files = collect_ynz_files_for_fmt(&project_root);
    if files.is_empty() {
        return EXIT_OK;
    }

    let mut worst = EXIT_OK;
    for file in &files {
        let code = fmt_single(file, check_only);
        if code > worst {
            worst = code;
        }
    }
    worst
}

/// Walk `root` for `.ynz` files, returning sorted paths.
///
/// Unlike the project-build walker in `load.rs` this version is formatter-only:
/// no symlink cycle detection (formatter is non-destructive) and no diagnostic
/// emission (infra errors are silently skipped).
fn collect_ynz_files_for_fmt(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    walk_dir_for_fmt(root, &mut files);
    files.sort();
    files
}

fn walk_dir_for_fmt(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_dir_for_fmt(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("ynz") {
            out.push(path);
        }
    }
}

// ── Stdin mode ────────────────────────────────────────────────────────────────

/// Read source from stdin, format it, and write the result to stdout.
///
/// Used by editor/LSP pipelines until `textDocument/formatting` is wired
/// in v0.2-M5 (at which point the library `format()` function is called
/// directly; this mode remains available for shell pipelines).
fn fmt_stdin() -> i32 {
    let mut source = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut source) {
        eprintln!("ynz fmt: cannot read stdin: {e}");
        return EXIT_INFRA_ERROR;
    }

    match ynz_fmt::format_named(&source, "<stdin>") {
        Err(ynz_fmt::FmtError::ParseError(diags)) => {
            let mut sources = HashMap::new();
            sources.insert("<stdin>".to_string(), source.clone());
            eprint!("{}", render(&diags, &sources, false));
            EXIT_SRC_ERROR
        }
        Err(ynz_fmt::FmtError::InvalidInput(msg)) => {
            eprintln!("ynz fmt: invalid input: {msg}");
            EXIT_INFRA_ERROR
        }
        Ok(formatted) => {
            print!("{formatted}");
            EXIT_OK
        }
    }
}
