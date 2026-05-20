use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::error::{Result, WatchError};

/// Resolved watch configuration: a list of paths to watch + the entry file.
pub struct WatchTarget {
    /// All `.ynz` files in the project (or the single file for single-file mode).
    pub sources: Vec<WatchSourceFile>,
    /// Entry file: the first file passed to `codegen_query`.
    pub entry: PathBuf,
    /// Project root, if operating in project mode (yinz.toml found).
    pub project_root: Option<PathBuf>,
}

/// A single source file entry with its canonical path and initial text.
pub struct WatchSourceFile {
    pub path: PathBuf,
    pub text: String,
}

/// Resolve the watch target from the CLI path arg.
///
/// Single-file mode: `path.ynz` — watches only that file.
/// Project mode: `./dir/` or implicit `.` — reads `yinz.toml`, walks `.ynz` files.
///
/// # Failure modes
///
/// - Source file unreadable → `WatchError::SourceRead`
/// - Directory without `yinz.toml` → `WatchError::NoProjectFile`
/// - Directory read failure → `WatchError::Io`
///
/// Time: O(n) where n = number of .ynz files in project. Space: O(n).
pub fn resolve_target(path: &Path) -> Result<WatchTarget> {
    if path.is_file() {
        let text = std::fs::read_to_string(path).map_err(|e| WatchError::SourceRead {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;
        Ok(WatchTarget {
            sources: vec![WatchSourceFile {
                path: path.to_path_buf(),
                text,
            }],
            entry: path.to_path_buf(),
            project_root: None,
        })
    } else {
        resolve_project(path)
    }
}

/// Resolve all `.ynz` files under a project root (requires `yinz.toml`).
fn resolve_project(root: &Path) -> Result<WatchTarget> {
    let toml_path = root.join("yinz.toml");
    if !toml_path.exists() {
        return Err(WatchError::NoProjectFile {
            root: root.to_path_buf(),
        });
    }

    // Read entry point from yinz.toml (minimal parse: just "entry = ...").
    let entry_name = parse_entry_from_toml(&toml_path).unwrap_or_else(|| "entrypoint.ynz".to_string());
    let entry = root.join(&entry_name);

    let mut sources = Vec::new();
    let mut visited = HashSet::new();
    collect_ynz_files(root, &mut sources, &mut visited)?;
    sources.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(WatchTarget {
        entry,
        sources,
        project_root: Some(root.to_path_buf()),
    })
}

/// Parse `entry = "..."` from a minimal yinz.toml.
fn parse_entry_from_toml(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("entry") {
            let rest = rest.trim();
            if let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim().trim_matches('"').trim_matches('\'');
                if !rest.is_empty() {
                    return Some(rest.to_string());
                }
            }
        }
    }
    None
}

/// Recursively collect all `.ynz` files under `dir`.
///
/// Time: O(n) where n = total files in project tree. Space: O(d) where d = max depth.
fn collect_ynz_files(
    dir: &Path,
    out: &mut Vec<WatchSourceFile>,
    visited: &mut HashSet<PathBuf>,
) -> Result<()> {
    let read_dir = std::fs::read_dir(dir).map_err(WatchError::Io)?;

    for entry in read_dir.flatten() {
        let path = entry.path();
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if meta.file_type().is_symlink() {
            if let Ok(canon) = std::fs::canonicalize(&path) {
                if canon.is_dir() {
                    if visited.insert(canon.clone()) {
                        collect_ynz_files(&path, out, visited)?;
                    }
                    continue;
                }
            }
        }

        if meta.is_dir() {
            let canon = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if visited.insert(canon) {
                collect_ynz_files(&path, out, visited)?;
            }
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) == Some("ynz") {
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    return Err(WatchError::SourceRead {
                        path: path.clone(),
                        reason: e.to_string(),
                    });
                }
            };
            out.push(WatchSourceFile { path, text });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn single_file_mode() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("foo.ynz");
        std::fs::write(&file, "// source\n").unwrap();

        let target = resolve_target(&file).unwrap();
        assert_eq!(target.sources.len(), 1);
        assert_eq!(target.entry, file);
        assert!(target.project_root.is_none());
    }

    #[test]
    fn project_mode_requires_yinz_toml() {
        let dir = TempDir::new().unwrap();
        let result = resolve_target(dir.path());
        assert!(matches!(result, Err(WatchError::NoProjectFile { .. })));
    }

    #[test]
    fn project_mode_discovers_ynz_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("yinz.toml"), "entry = \"main.ynz\"\n").unwrap();
        std::fs::write(dir.path().join("main.ynz"), "// main\n").unwrap();
        std::fs::write(dir.path().join("lib.ynz"), "// lib\n").unwrap();

        let target = resolve_target(dir.path()).unwrap();
        assert_eq!(target.sources.len(), 2);
        assert_eq!(target.project_root, Some(dir.path().to_path_buf()));
        assert_eq!(target.entry, dir.path().join("main.ynz"));
    }
}
