use std::collections::HashMap;

/// Supertrait for databases that support project-file lookup by path.
/// Implemented by `CompilerDb` so salsa tracked functions can call
/// `source_by_path` without unsafe downcasting.
pub trait SourceFileRegistry: salsa::Database {
    fn source_by_path(&self, path: &str) -> Option<SourceFile>;
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

impl SourceFileRegistry for CompilerDb {
    fn source_by_path(&self, path: &str) -> Option<SourceFile> {
        self.source_registry.get(path).copied()
    }
}
