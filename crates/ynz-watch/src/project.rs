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
/// Project mode: directory path — walks UP to find nearest `yinz.toml`, then watches all
/// `.ynz` files under the project root.
///
/// # `yinz.toml` entry formats supported
///
/// Single-entry:   `entry = "entrypoint.ynz"`
/// Multi-entry:    `[entries]\nbackfill = "ships/backfill.ynz"\n...`
///
/// For multi-entry projects, the directory hint is used to pick the matching entry.
/// If the hint matches exactly one entry (by path prefix) it is selected automatically.
///
/// # Failure modes
///
/// - Source file unreadable → `WatchError::SourceRead`
/// - No `yinz.toml` found anywhere up the directory tree → `WatchError::NoProjectFile`
/// - Directory read failure → `WatchError::Io`
///
/// Time: O(n) where n = number of .ynz files in project. Space: O(n).
pub fn resolve_target(path: &Path) -> Result<WatchTarget> {
    if path.is_file() {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        // If there's a yinz.toml above this file, use project mode — registering only
        // the entry file causes cross-module imports to fail (shared files not in DB).
        if let Some(root) = find_project_root(canonical.parent().unwrap_or(&canonical)) {
            let root = std::fs::canonicalize(&root).unwrap_or(root);
            return resolve_project_with_entry(&root, &canonical);
        }

        // True single-file mode: no yinz.toml anywhere above — imports unsupported.
        let text = std::fs::read_to_string(&canonical).map_err(|e| WatchError::SourceRead {
            path: canonical.clone(),
            reason: e.to_string(),
        })?;
        return Ok(WatchTarget {
            sources: vec![WatchSourceFile {
                path: canonical.clone(),
                text,
            }],
            entry: canonical,
            project_root: None,
        });
    }

    // For directory paths: walk up to find the nearest yinz.toml.
    // Make the hint absolute before walking up — canonicalize("") fails on Linux,
    // so we must never let an empty/relative path reach find_project_root.
    let hint_dir = {
        let p = path.to_path_buf();
        if p.is_absolute() {
            p
        } else {
            std::env::current_dir().unwrap_or_default().join(&p)
        }
    };

    let root = find_project_root(&hint_dir).ok_or_else(|| WatchError::NoProjectFile {
        root: hint_dir.clone(),
    })?;

    // root is now guaranteed absolute (walk started from absolute hint_dir).
    // Canonicalize to resolve any remaining symlinks so stored paths match what
    // ynz-typeck's resolve_module_path returns after its own canonicalize call.
    let root = std::fs::canonicalize(&root).unwrap_or(root);

    resolve_project(&root, &hint_dir)
}

/// Walk up from `start` to find the nearest directory containing `yinz.toml`.
fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };
    loop {
        if current.join("yinz.toml").exists() {
            return Some(current);
        }
        match current.parent() {
            Some(p) if p != current => current = p.to_path_buf(),
            _ => return None,
        }
    }
}

/// Resolve all `.ynz` files under a project root with an explicit entry file.
///
/// Used when the user passed a `.ynz` file path directly but a `yinz.toml` exists
/// above it — we load all project files so cross-module imports resolve correctly,
/// but honour the user's explicit entry instead of reading it from `yinz.toml`.
fn resolve_project_with_entry(root: &Path, entry: &Path) -> Result<WatchTarget> {
    let mut sources = Vec::new();
    let mut visited = HashSet::new();
    collect_ynz_files(root, &mut sources, &mut visited)?;
    sources.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(WatchTarget {
        entry: entry.to_path_buf(),
        sources,
        project_root: Some(root.to_path_buf()),
    })
}

/// Resolve all `.ynz` files under a project root using `yinz.toml`.
///
/// `hint` is the path the user originally passed — used to select the right entry
/// in multi-entry projects.
fn resolve_project(root: &Path, hint: &Path) -> Result<WatchTarget> {
    let toml_path = root.join("yinz.toml");

    // Try [entries] table first, then fall back to `entry = "..."`.
    let entry_name = if let Some(entries) = parse_entries_table_from_toml(&toml_path) {
        pick_entry_from_hint(&entries, root, hint).unwrap_or_else(|| {
            entries
                .into_values()
                .next()
                .unwrap_or_else(|| "entrypoint.ynz".to_string())
        })
    } else {
        parse_entry_from_toml(&toml_path).unwrap_or_else(|| "entrypoint.ynz".to_string())
    };

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

/// Parse `[entries]` table from yinz.toml: returns map of name → relative path.
///
/// Returns `None` if no `[entries]` section is present.
fn parse_entries_table_from_toml(path: &Path) -> Option<std::collections::HashMap<String, String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut in_entries = false;
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[entries]" {
            in_entries = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_entries = false;
            continue;
        }
        if in_entries {
            if let Some((key, val)) = trimmed.split_once('=') {
                let key = key.trim().to_string();
                let val = val.trim().trim_matches('"').trim_matches('\'').to_string();
                if !key.is_empty() && !val.is_empty() {
                    map.insert(key, val);
                }
            }
        }
    }
    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

/// Pick the best entry from a multi-entry map given the user's hint path.
///
/// Match strategy: find any entry whose value (relative to root) shares a path component
/// with the hint. E.g. hint `ships/scripts/backfill` matches entry path
/// `scripts/backfill/entrypoint.ynz` because `scripts/backfill` appears in both.
fn pick_entry_from_hint(
    entries: &std::collections::HashMap<String, String>,
    root: &Path,
    hint: &Path,
) -> Option<String> {
    // Normalise hint relative to root if possible.
    let hint_rel = hint.strip_prefix(root).unwrap_or(hint);
    let hint_str = hint_rel.to_string_lossy().replace('\\', "/");

    // Exact name match first (e.g. user passed the entry name literally).
    if let Some(v) = entries.get(hint_str.as_str()) {
        return Some(v.clone());
    }

    // Path-component match: pick the entry whose path contains all components of the hint.
    let hint_parts: Vec<&str> = hint_str.split('/').filter(|s| !s.is_empty()).collect();
    let mut best: Option<&str> = None;
    for val in entries.values() {
        let val_norm = val.replace('\\', "/");
        let matches = hint_parts.iter().all(|part| val_norm.contains(part));
        if matches {
            best = Some(val);
            break;
        }
    }
    best.map(|s| s.to_string())
}

/// Parse `entry = "..."` from a minimal yinz.toml (single-entry format).
fn parse_entry_from_toml(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim();
        // Skip lines inside a [section] — only want top-level entry = "..."
        if line.starts_with('[') {
            continue;
        }
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

    // WHY (all project tests): resolve_target is the entry point for every watch session —
    //      wrong path resolution silently watches the wrong files (missed rebuilds) or crashes
    //      with a confusing error. Single-file mode and project mode are distinct code paths;
    //      both must be verified.

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
        // WHY: if no yinz.toml is found anywhere up the tree from an isolated temp dir,
        //      we must return NoProjectFile — not silently watch nothing.
        let dir = TempDir::new().unwrap();
        // Use a sub-subdir that is fully isolated (no yinz.toml anywhere above in /tmp).
        let sub = dir.path().join("a").join("b");
        std::fs::create_dir_all(&sub).unwrap();
        let result = resolve_target(&sub);
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

    #[test]
    fn project_mode_walks_up_to_find_yinz_toml() {
        // WHY: yinz.toml is always at project root, never in subdirectories. Passing a
        //      subdir (e.g. `ynz watch ships/scripts/backfill`) must find root yinz.toml.
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("ships").join("scripts").join("backfill");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.path().join("yinz.toml"), "entry = \"main.ynz\"\n").unwrap();
        std::fs::write(dir.path().join("main.ynz"), "// main\n").unwrap();

        let target = resolve_target(&sub).unwrap();
        assert_eq!(target.project_root, Some(dir.path().to_path_buf()));
        assert_eq!(target.entry, dir.path().join("main.ynz"));
    }

    #[test]
    fn project_mode_multi_entry_picks_matching_entry() {
        // WHY: [entries] table multi-entry projects must use the hint path to select
        //      the right entry — otherwise `ynz watch ships/backfill` always runs the
        //      wrong entry or fails.
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("ships").join("backfill");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            dir.path().join("yinz.toml"),
            "[project]\nname = \"trading\"\n\n[entries]\nbackfill = \"ships/backfill/entrypoint.ynz\"\nother = \"ships/other/entrypoint.ynz\"\n",
        ).unwrap();
        std::fs::write(sub.join("entrypoint.ynz"), "// backfill\n").unwrap();

        let target = resolve_target(&sub).unwrap();
        assert_eq!(target.project_root, Some(dir.path().to_path_buf()));
        assert_eq!(
            target.entry,
            dir.path().join("ships/backfill/entrypoint.ynz")
        );
    }
}
