use std::collections::HashMap;

/// Supertrait for databases that support project-file lookup by path.
///
/// Applying `#[salsa::db]` here (and on its `impl` for `CompilerDb`) registers
/// `dyn SourceFileRegistry` as a salsa view — enabling salsa tracked functions
/// to declare `db: &dyn SourceFileRegistry` and use `source_by_path` directly
/// without unsafe downcasting.
#[salsa::db]
pub trait SourceFileRegistry: salsa::Database {
    fn source_by_path(&self, path: &str) -> Option<SourceFile>;
    /// Returns all canonical paths registered in this project.
    ///
    /// Perf: O(N) allocation per call (clones the full keyset). Not a hot-path concern at
    /// v0.2 project scales (<100 files), but callers should avoid calling this in tight loops.
    /// Trigger to revisit: project size exceeds 100 files and LSP references/rename latency
    /// becomes noticeable — at that point, switch to an iterator API or a cached snapshot.
    fn all_source_paths(&self) -> Vec<String>;
}

/// A single source file as a salsa input.
///
/// The driver creates one `SourceFile` per file before running any queries.
/// Changing `text` or `path` will invalidate all downstream queries automatically.
#[salsa::input]
pub struct SourceFile {
    /// The file path used in diagnostic spans (e.g. `"hello.ynz"`).
    pub path: String,
    /// The full UTF-8 source text. Must be valid UTF-8 — the driver validates
    /// this before constructing a SourceFile.
    pub text: String,
}

/// The concrete salsa Database for the ynz compiler.
///
/// Every compilation session creates one `CompilerDb` that lives for the
/// duration of the session. Individual queries are cached inside the db and
/// re-used across incremental rebuilds.
///
/// The `source_registry` maps canonical file paths to their SourceFile inputs so
/// cross-file queries can look up imported files by path without creating duplicate
/// salsa inputs. The driver must call `register_source` for every file before
/// running any queries.
#[salsa::db]
#[derive(Default)]
pub struct CompilerDb {
    storage: salsa::Storage<Self>,
    source_registry: HashMap<String, SourceFile>,
}

impl CompilerDb {
    /// Register a SourceFile in the path → SourceFile registry.
    /// Must be called for all project files before any query runs.
    pub fn register_source(&mut self, sf: SourceFile) {
        let path = sf.path(self).clone();
        self.source_registry.insert(path, sf);
    }

    /// Look up a SourceFile by its canonical path.
    /// Returns `None` if the file was not registered (not part of the project).
    pub fn source_by_path(&self, path: &str) -> Option<SourceFile> {
        self.source_registry.get(path).copied()
    }
}

#[salsa::db]
impl salsa::Database for CompilerDb {}

#[salsa::db]
impl SourceFileRegistry for CompilerDb {
    fn source_by_path(&self, path: &str) -> Option<SourceFile> {
        self.source_registry.get(path).copied()
    }

    fn all_source_paths(&self) -> Vec<String> {
        self.source_registry.keys().cloned().collect()
    }
}
