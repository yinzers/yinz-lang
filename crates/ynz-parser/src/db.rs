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
#[salsa::db]
#[derive(Default)]
pub struct CompilerDb {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for CompilerDb {}
