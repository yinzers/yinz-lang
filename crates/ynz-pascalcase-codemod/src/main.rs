//! `pascalcase-codemod`: renames Yinz built-in type names to PascalCase, only in type positions.
//!
//! Modes (by file extension, or `--mode` when a directory is given):
//! - ynz:      `.ynz` and `.formatted` files
//! - markdown: `.md`, fenced blocks tagged `ynz` / `yinz`
//! - rust:     `.rs`, string literals whose content is a complete Yinz source file

mod markdown;
mod rust_strings;
mod yinz_source;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;

const USAGE: &str = "\
usage: pascalcase-codemod [--dry-run] [--leftovers] [--lenient] [--mode ynz|markdown|rust] <file-or-directory>...

  A file is processed by its extension (.ynz/.formatted, .md, .rs).
  A directory is searched recursively for files of --mode (default: ynz); .git, .claude,
  target, node_modules and marketing are never entered.
  --dry-run    count renames, write nothing
  --leftovers  (ynz mode) also list every old built-in spelling that was NOT renamed, with
               the neighbouring tokens, for spot-checking
  --lenient    (ynz mode) also rename files that have parse diagnostics (error fixtures): the type
               positions the parser recorded are renamed anyway and the file is marked LENIENT";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Mode {
    Ynz,
    Markdown,
    Rust,
}

fn mode_of_extension(path: &Path) -> Option<Mode> {
    match path.extension()?.to_str()? {
        "ynz" | "formatted" => Some(Mode::Ynz),
        "md" => Some(Mode::Markdown),
        "rs" => Some(Mode::Rust),
        _ => None,
    }
}

const DIRECTORIES_NEVER_ENTERED: &[&str] = &[".git", ".claude", "target", "node_modules", "marketing"];

fn collect_files(root: &Path, wanted_mode: Mode, files: &mut Vec<(PathBuf, Mode)>) {
    if root.is_file() {
        if let Some(mode) = mode_of_extension(root) {
            files.push((root.to_path_buf(), mode));
        } else {
            eprintln!("skipping {}: no known extension", root.display());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        eprintln!("cannot read directory {}", root.display());
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if !DIRECTORIES_NEVER_ENTERED.contains(&name.to_string_lossy().as_ref()) {
                collect_files(&path, wanted_mode, files);
            }
        } else if mode_of_extension(&path) == Some(wanted_mode) {
            files.push((path, wanted_mode));
        }
    }
}

struct Options {
    dry_run: bool,
    show_leftovers: bool,
    lenient: bool,
}

#[derive(Default)]
struct FileReport {
    path: PathBuf,
    mode: Option<Mode>,
    renames: usize,
    detail: String,
    unparseable_reason: Option<String>,
    ambiguous: Vec<(usize, String)>,
    notes: Vec<String>,
    error: Option<String>,
}

fn process_file(path: &Path, mode: Mode, options: &Options) -> FileReport {
    let mut report = FileReport {
        path: path.to_path_buf(),
        mode: Some(mode),
        ..FileReport::default()
    };
    let original = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            report.error = Some(format!("cannot read as UTF-8: {error}"));
            return report;
        }
    };
    let label = path.to_string_lossy().to_string();

    let outcome = std::panic::catch_unwind(|| match mode {
        Mode::Ynz => process_ynz(&label, &original, options),
        Mode::Markdown => process_markdown(&label, &original),
        Mode::Rust => process_rust(&label, &original),
    });
    let Ok(outcome) = outcome else {
        report.unparseable_reason = Some("the parser panicked on this input".to_string());
        return report;
    };

    report.renames = outcome.renames;
    report.detail = outcome.detail;
    report.unparseable_reason = outcome.unparseable_reason;
    report.ambiguous = outcome.ambiguous;
    report.notes = outcome.notes;

    if let Some(new_text) = outcome.new_text {
        if new_text != original && !options.dry_run {
            if let Err(error) = std::fs::write(path, new_text) {
                report.error = Some(format!("cannot write: {error}"));
            }
        }
    }
    report
}

#[derive(Default)]
struct Outcome {
    new_text: Option<String>,
    renames: usize,
    detail: String,
    unparseable_reason: Option<String>,
    ambiguous: Vec<(usize, String)>,
    notes: Vec<String>,
}

fn process_ynz(label: &str, original: &str, options: &Options) -> Outcome {
    let on_diagnostics = if options.lenient {
        yinz_source::ParseDiagnostics::Tolerate
    } else {
        yinz_source::ParseDiagnostics::Reject
    };
    match yinz_source::rewrite_yinz_source(label, original, on_diagnostics) {
        Err(unparseable) => Outcome {
            unparseable_reason: Some(unparseable.reason),
            ..Outcome::default()
        },
        Ok(rewrite) => {
            let mut notes = Vec::new();
            if let Some(problem) = &rewrite.tolerated_parse_problem {
                notes.push(format!("LENIENT: parse problem went past, review this diff: {problem}"));
            }
            if options.show_leftovers {
                for leftover in yinz_source::leftovers_after(original, &rewrite.renames) {
                    notes.push(format!(
                        "leftover `{}` line {} (after `{}`, before `{}`)",
                        leftover.name, leftover.line, leftover.previous, leftover.next
                    ));
                }
            }
            Outcome {
                new_text: Some(yinz_source::apply_renames(original, &rewrite.renames)),
                renames: rewrite.renames.len(),
                ambiguous: rewrite
                    .ambiguous_constant_receivers
                    .into_iter()
                    .collect(),
                notes,
                ..Outcome::default()
            }
        }
    }
}

fn process_markdown(label: &str, original: &str) -> Outcome {
    let rewrite = markdown::rewrite_markdown(label, original);
    let notes = rewrite
        .blocks_unparseable
        .iter()
        .map(|(line, reason)| format!("unparseable ynz block at line {line}: {reason}"))
        .collect();
    Outcome {
        detail: format!(
            "blocks rewritten {}, unchanged {}, unparseable {}",
            rewrite.blocks_rewritten,
            rewrite.blocks_unchanged,
            rewrite.blocks_unparseable.len()
        ),
        renames: rewrite.renames,
        ambiguous: rewrite.ambiguous,
        notes,
        new_text: Some(rewrite.new_text),
        unparseable_reason: None,
    }
}

fn process_rust(label: &str, original: &str) -> Outcome {
    let rewrite = rust_strings::rewrite_rust_file(label, original);
    let notes = rewrite
        .suspect_skipped
        .iter()
        .map(|(line, reason)| format!("skipped Yinz-looking string at line {line}: {reason}"))
        .collect();
    Outcome {
        detail: format!(
            "strings rewritten {}, parsed-but-unchanged {}, skipped {}",
            rewrite.strings_rewritten, rewrite.strings_unchanged, rewrite.strings_skipped
        ),
        renames: rewrite.renames,
        ambiguous: rewrite.ambiguous,
        notes,
        new_text: Some(rewrite.new_text),
        unparseable_reason: None,
    }
}

fn main() -> ExitCode {
    let mut dry_run = false;
    let mut show_leftovers = false;
    let mut lenient = false;
    let mut directory_mode = Mode::Ynz;
    let mut roots: Vec<PathBuf> = Vec::new();

    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--dry-run" => dry_run = true,
            "--leftovers" => show_leftovers = true,
            "--lenient" => lenient = true,
            "--mode" => {
                directory_mode = match arguments.next().as_deref() {
                    Some("ynz") => Mode::Ynz,
                    Some("markdown" | "md") => Mode::Markdown,
                    Some("rust" | "rs") => Mode::Rust,
                    other => {
                        eprintln!("--mode needs ynz, markdown or rust, got {other:?}\n{USAGE}");
                        return ExitCode::from(2);
                    }
                };
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            flag if flag.starts_with("--") => {
                eprintln!("unknown flag {flag}\n{USAGE}");
                return ExitCode::from(2);
            }
            path => roots.push(PathBuf::from(path)),
        }
    }
    if roots.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }

    let options = Options {
        dry_run,
        show_leftovers,
        lenient,
    };
    let mut files = Vec::new();
    for root in &roots {
        collect_files(root, directory_mode, &mut files);
    }
    files.sort();
    files.dedup();

    // The parser panics (caught per file) on some pathological inputs; keep stderr readable.
    std::panic::set_hook(Box::new(|_| {}));

    let worker_count = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
    let next_file = Mutex::new(0usize);
    let reports: Mutex<Vec<FileReport>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            std::thread::Builder::new()
                .stack_size(64 * 1024 * 1024)
                .spawn_scoped(scope, || loop {
                    let index = {
                        let mut guard = next_file.lock().unwrap();
                        let index = *guard;
                        *guard += 1;
                        index
                    };
                    let Some((path, mode)) = files.get(index) else {
                        break;
                    };
                    let report = process_file(path, *mode, &options);
                    reports.lock().unwrap().push(report);
                })
                .expect("spawn worker thread");
        }
    });

    let mut reports = reports.into_inner().unwrap();
    reports.sort_by(|a, b| a.path.cmp(&b.path));
    print_reports(&reports, dry_run)
}

fn print_reports(reports: &[FileReport], dry_run: bool) -> ExitCode {
    let mut files_changed = 0;
    let mut total_renames = 0;
    let mut unparseable_files = Vec::new();
    let mut had_error = false;

    for report in reports {
        if let Some(error) = &report.error {
            had_error = true;
            println!("ERROR {}: {error}", report.path.display());
        }
        if let Some(reason) = &report.unparseable_reason {
            unparseable_files.push((&report.path, reason));
            continue;
        }
        if report.renames > 0 {
            files_changed += 1;
            total_renames += report.renames;
        }
        let interesting = report.renames > 0
            || !report.notes.is_empty()
            || !report.ambiguous.is_empty()
            || report.mode != Some(Mode::Ynz);
        if interesting {
            let detail = if report.detail.is_empty() {
                String::new()
            } else {
                format!(" ({})", report.detail)
            };
            println!("{}: {} renames{detail}", report.path.display(), report.renames);
        }
        for note in &report.notes {
            println!("    {note}");
        }
        for (line, name) in &report.ambiguous {
            println!(
                "    AMBIGUOUS line {line}: `{name}.<constant>` left unchanged because this file also declares a variable or field named `{name}`"
            );
        }
    }

    println!();
    if !unparseable_files.is_empty() {
        println!("UNPARSEABLE (left untouched):");
        for (path, reason) in &unparseable_files {
            println!("  {}: {reason}", path.display());
        }
        println!();
    }
    println!(
        "{} files scanned, {} changed{}, {} renames, {} unparseable",
        reports.len(),
        files_changed,
        if dry_run { " (dry run, nothing written)" } else { "" },
        total_renames,
        unparseable_files.len()
    );
    if had_error {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
