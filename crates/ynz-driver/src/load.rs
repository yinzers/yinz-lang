use std::path::{Path, PathBuf};

use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

/// Read a source file from disk, verify it is valid UTF-8, and return the text.
///
/// On invalid UTF-8, pushes a diagnostic and returns `None`.
pub fn load_source(path: &Path, diags: &mut DiagnosticBucket) -> Option<String> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            diags.push(Diagnostic::error(
                SourceSpan::new(path.display().to_string(), 0, 0),
                format!("Could not read `{}`: {e}", path.display()),
                "Check that the file exists and you have permission to read it.",
                "The compiler needs to read your source file before it can compile it.",
            ));
            return None;
        }
    };

    match String::from_utf8(bytes.clone()) {
        Ok(s) => Some(s),
        Err(e) => {
            // Point to the first invalid byte.
            let offset = e.utf8_error().valid_up_to();
            diags.push(Diagnostic::error(
                SourceSpan::new(path.display().to_string(), offset, offset + 1),
                format!("`{}` contains bytes that are not valid UTF-8.", path.display()),
                "Save the file with UTF-8 encoding (most editors use this by default).",
                "Yinz source files must be UTF-8. \
                 Other encodings (Latin-1, Windows-1252, etc.) may look similar but will cause this error.",
            ));
            None
        }
    }
}

/// Minimal `yinz.toml` config — three fields, everything else warns.
#[allow(dead_code)]
pub struct ProjectConfig {
    pub entry: String,
    pub name: String,
    pub version: String,
}

/// Try to find the nearest `yinz.toml` by walking up the directory tree.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        if dir.join("yinz.toml").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Parse `yinz.toml` (minimal subset: `entry`, `name`, `version`).
pub fn load_project_config(root: &Path, diags: &mut DiagnosticBucket) -> ProjectConfig {
    let toml_path = root.join("yinz.toml");
    let text = match std::fs::read_to_string(&toml_path) {
        Ok(t) => t,
        Err(_) => return default_config(root),
    };

    let mut entry = "src/entrypoint.ynz".to_string();
    let mut name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());
    let mut version = "0.0.0".to_string();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("entry") {
            if let Some(val) = parse_toml_string(rest) {
                entry = val;
            }
        } else if let Some(rest) = line.strip_prefix("name") {
            if let Some(val) = parse_toml_string(rest) {
                name = val;
            }
        } else if let Some(rest) = line.strip_prefix("version") {
            if let Some(val) = parse_toml_string(rest) {
                version = val;
            }
        } else if !line.starts_with('[') {
            // Unknown key — warn
            let key = line.split('=').next().unwrap_or("").trim();
            if !key.is_empty() {
                diags.push(Diagnostic::error(
                    SourceSpan::new(toml_path.display().to_string(), 0, 0),
                    format!("Unknown field `{key}` in yinz.toml — ignored."),
                    "Supported fields: `entry`, `name`, `version`.",
                    "Unknown fields are ignored for forward-compatibility with future Yinz versions.",
                ));
            }
        }
    }

    ProjectConfig {
        entry,
        name,
        version,
    }
}

fn parse_toml_string(rest: &str) -> Option<String> {
    let rest = rest
        .trim_start_matches(|c: char| c.is_whitespace() || c == '=')
        .trim();
    // Strip surrounding quotes (single or double)
    let inner = if (rest.starts_with('"') && rest.ends_with('"'))
        || (rest.starts_with('\'') && rest.ends_with('\''))
    {
        &rest[1..rest.len() - 1]
    } else {
        rest
    };
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

fn default_config(root: &Path) -> ProjectConfig {
    ProjectConfig {
        entry: "src/entrypoint.ynz".to_string(),
        name: root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".to_string()),
        version: "0.0.0".to_string(),
    }
}

/// Loaded source file: path + text.
pub struct SourceEntry {
    /// Canonical file path (for diagnostics).
    pub path: PathBuf,
    /// Module path relative to `src/` (no `.ynz` suffix) — used for mangling.
    #[allow(dead_code)]
    pub module_path: String,
    /// Source text.
    pub text: String,
}

/// Load all `.ynz` files under the project root for a project build.
///
/// Prefers `root/src/` if it exists (conventional layout), but falls back
/// to walking the whole project root so projects with arbitrary layouts work.
/// Returns entries sorted by path for deterministic ordering.
pub fn load_project(root: &Path, diags: &mut DiagnosticBucket) -> Vec<SourceEntry> {
    let src_dir = root.join("src");
    let walk_root = if src_dir.exists() { src_dir } else { root.to_path_buf() };

    let mut entries: Vec<SourceEntry> = Vec::new();
    collect_ynz_files(&walk_root, &walk_root, &mut entries, diags);
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

fn collect_ynz_files(
    src_root: &Path,
    dir: &Path,
    entries: &mut Vec<SourceEntry>,
    diags: &mut DiagnosticBucket,
) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) => {
            diags.push(Diagnostic::error(
                SourceSpan::new(dir.display().to_string(), 0, 0),
                format!("Cannot read directory `{}`: {e}", dir.display()),
                "Check that the directory exists and you have read permission.",
                "`ynz build` walks `src/**/*.ynz` to find all source files.",
            ));
            return;
        }
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ynz_files(src_root, &path, entries, diags);
        } else if path.extension().is_some_and(|e| e == "ynz") {
            let module_path = path
                .strip_prefix(src_root)
                .unwrap_or(&path)
                .with_extension("")
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");

            match std::fs::read_to_string(&path) {
                Ok(text) => entries.push(SourceEntry {
                    path,
                    module_path,
                    text,
                }),
                Err(e) => {
                    diags.push(Diagnostic::error(
                        SourceSpan::new(path.display().to_string(), 0, 0),
                        format!("Could not read `{}`: {e}", path.display()),
                        "Check that the file exists and you have read permission.",
                        "All `.ynz` files under `src/` are compiled as part of the project.",
                    ));
                }
            }
        }
    }
}
