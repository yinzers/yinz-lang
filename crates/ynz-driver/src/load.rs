use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

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

    let mut entry = "entrypoint.ynz".to_string();
    let mut name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());
    let mut version = "0.0.0".to_string();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        // F5 (SCRATCH-audit-2026-07-11-non-concurrency.md): split on `=` and match the
        // trimmed KEY exactly, rather than `strip_prefix`-ing the whole line — the old
        // `line.strip_prefix("entry")` matched `entrypoint_foo = "x"` and
        // `line.strip_prefix("version")` matched `versionabc = ...` as if they were the
        // real key, silently mis-parsing any key that merely starts with one of ours.
        let Some((key_part, value_part)) = line.split_once('=') else {
            continue;
        };
        let key = key_part.trim();
        match key {
            "entry" => {
                if let Some(val) = parse_toml_string(value_part) {
                    entry = val;
                }
            }
            "name" => {
                if let Some(val) = parse_toml_string(value_part) {
                    name = val;
                }
            }
            "version" => {
                if let Some(val) = parse_toml_string(value_part) {
                    version = val;
                }
            }
            _ => {
                if !key.is_empty() {
                    diags.push(Diagnostic::warning(
                        SourceSpan::new(toml_path.display().to_string(), 0, 0),
                        format!("Unknown field `{key}` in yinz.toml — ignored."),
                        "Supported fields: `entry`, `name`, `version`.",
                        "Unknown fields are ignored for forward-compatibility with future Yinz versions.",
                    ));
                }
            }
        }
    }

    ProjectConfig {
        entry,
        name,
        version,
    }
}

/// Parse the value portion of a TOML key = "value" line.
///
/// `rest` is everything AFTER the key name — i.e. it starts with optional
/// whitespace, then `=`, then the value.  The function strips the `=` itself
/// and any surrounding whitespace before unquoting.
///
/// Returns `None` when the value is empty or the key was not present.
fn parse_toml_string(rest: &str) -> Option<String> {
    let rest = rest
        .trim_start_matches(|c: char| c.is_whitespace() || c == '=')
        .trim();

    // F4 (SCRATCH-audit-2026-07-11-non-concurrency.md): a lone quote character is an
    // unterminated/malformed value (`entry = "` trims down to a single `"` byte) —
    // treat it as invalid rather than falling through to the quote-strip below, where
    // that single byte satisfies BOTH `starts_with('"')` and `ends_with('"')` and
    // `&rest[1..rest.len() - 1]` becomes the invalid byte range `1..0` — a panic that
    // presents a user's yinz.toml typo as a compiler-bug ICE banner.
    if rest == "\"" || rest == "'" {
        return None;
    }

    // Strip surrounding quotes (single or double) — `rest.len() >= 2` is guaranteed
    // here (the lone-quote case above already returned), so this slice is always valid.
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
        entry: "entrypoint.ynz".to_string(),
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
    /// Module path relative to project root (no `.ynz` suffix) — used for mangling.
    #[allow(dead_code)]
    pub module_path: String,
    /// Source text.
    pub text: String,
}

/// Load all `.ynz` files under the project root for a project build.
///
/// Walks from the project root (where `yinz.toml` lives). All `.ynz` files
/// are included regardless of directory structure — the spec says paths are
/// project-root-relative, no `src/` convention required.
/// Returns entries sorted by path for deterministic ordering.
pub fn load_project(root: &Path, diags: &mut DiagnosticBucket) -> Vec<SourceEntry> {
    let mut entries: Vec<SourceEntry> = Vec::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    collect_ynz_files(root, root, &mut entries, diags, &mut visited);
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

/// Time: O(n) where n = total files in project tree. Space: O(d) where d = max directory depth.
fn collect_ynz_files(
    src_root: &Path,
    dir: &Path,
    entries: &mut Vec<SourceEntry>,
    diags: &mut DiagnosticBucket,
    visited: &mut HashSet<PathBuf>,
) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) => {
            diags.push(Diagnostic::error(
                SourceSpan::new(dir.display().to_string(), 0, 0),
                format!("Cannot read directory `{}`: {e}", dir.display()),
                "Check that the directory exists and you have read permission.",
                "`ynz build` walks all `.ynz` files from the project root.",
            ));
            return;
        }
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        // Use symlink_metadata so we inspect the symlink itself, not its target.
        // This prevents following symlinks into directories (which can form cycles).
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if meta.file_type().is_symlink() {
            // Resolve to check for cycles; skip symlinks that point to directories.
            if let Ok(canon) = std::fs::canonicalize(&path) {
                if canon.is_dir() && !visited.insert(canon.clone()) {
                    diags.push(Diagnostic::error(
                        SourceSpan::new(path.display().to_string(), 0, 0),
                        format!(
                            "Symbolic-link cycle detected in project tree at `{}`.",
                            path.display()
                        ),
                        "Remove the cycle, or replace the symlink with a real directory.",
                        "Yinz walks the project tree to find `.ynz` source files. \
                         A cycle of symlinks would loop forever. \
                         The walk skips symlinks that lead back into the project.",
                    ));
                    // Skip symlinked directories entirely (cycle or not).
                }
                // Symlinks to .ynz files are handled below through the is_dir() == false path.
            }
            continue;
        }

        if meta.is_dir() {
            // Canonical path for the directory — track to detect hard-link cycles (rare but possible).
            let canon = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if !visited.insert(canon) {
                // Already visited this canonical path — skip to avoid loops.
                continue;
            }
            collect_ynz_files(src_root, &path, entries, diags, visited);
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
                        "All `.ynz` files under the project root are compiled.",
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_toml(dir: &Path, contents: &str) {
        std::fs::write(dir.join("yinz.toml"), contents).expect("write yinz.toml");
    }

    // F4 (SCRATCH-audit-2026-07-11-non-concurrency.md): an unterminated quoted value
    // (`entry = "`) must never panic — it must be treated as an invalid/empty value,
    // falling back to the default entry.
    #[test]
    fn f4_unterminated_double_quote_does_not_panic() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_toml(dir.path(), "entry = \"\n");
        let mut diags = DiagnosticBucket::new();
        let cfg = load_project_config(dir.path(), &mut diags);
        // Malformed value falls back to the default — no panic, which is the
        // load-bearing assertion (this call itself is the test).
        assert_eq!(cfg.entry, "entrypoint.ynz");
    }

    #[test]
    fn f4_unterminated_single_quote_does_not_panic() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_toml(dir.path(), "entry = '\n");
        let mut diags = DiagnosticBucket::new();
        let cfg = load_project_config(dir.path(), &mut diags);
        assert_eq!(cfg.entry, "entrypoint.ynz");
    }

    #[test]
    fn f4_lone_quote_char_value_does_not_panic() {
        // The narrowest possible repro: `rest` trims down to exactly one quote byte.
        assert_eq!(parse_toml_string("= \""), None);
        assert_eq!(parse_toml_string("= '"), None);
    }

    // F5 (SCRATCH-audit-2026-07-11-non-concurrency.md): a key that merely starts with
    // a real key name (`entrypoint_foo`, `versionabc`) must NOT be treated as that key
    // — `strip_prefix`-style prefix matching silently mis-parsed these.
    #[test]
    fn f5_prefix_key_is_not_matched_as_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_toml(
            dir.path(),
            "entrypoint_foo = \"should-not-become-entry.ynz\"\n",
        );
        let mut diags = DiagnosticBucket::new();
        let cfg = load_project_config(dir.path(), &mut diags);
        assert_eq!(
            cfg.entry, "entrypoint.ynz",
            "`entrypoint_foo` must not be parsed as the `entry` key"
        );
    }

    #[test]
    fn f5_prefix_key_is_not_matched_as_version() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_toml(dir.path(), "versionabc = \"9.9.9\"\n");
        let mut diags = DiagnosticBucket::new();
        let cfg = load_project_config(dir.path(), &mut diags);
        assert_eq!(
            cfg.version, "0.0.0",
            "`versionabc` must not be parsed as the `version` key"
        );
    }

    #[test]
    fn f5_exact_key_still_parses_normally() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_toml(
            dir.path(),
            "entry = \"main.ynz\"\nname = \"demo\"\nversion = \"1.2.3\"\n",
        );
        let mut diags = DiagnosticBucket::new();
        let cfg = load_project_config(dir.path(), &mut diags);
        assert_eq!(cfg.entry, "main.ynz");
        assert_eq!(cfg.name, "demo");
        assert_eq!(cfg.version, "1.2.3");
    }
}
