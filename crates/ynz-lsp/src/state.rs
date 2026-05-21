use std::{collections::HashMap, path::{Path, PathBuf}};

use lsp_types::Url;
use salsa::Setter as _;
use ynz_parser::db::{CompilerDb, SourceFile};

use crate::{capabilities::PositionEncoding, position::LineTable};

/// Build a `file://` URI from a registered `SourceFile`'s canonical path.
///
/// Inverse of `uri_to_path`: used by go-to-def and find-references to convert
/// origin-file paths returned by `def_site_for_offset` / `references_for_offset`
/// back into URIs the LSP client can follow.
pub fn uri_for_source_file(db: &CompilerDb, sf: SourceFile) -> Url {
    let path = sf.path(db);
    Url::from_file_path(&*path).unwrap_or_else(|_| {
        // Non-filesystem paths (e.g. synthetic test files without a real path):
        // construct a fallback URI so the client sees something rather than panicking.
        Url::parse(&format!("file://{path}")).unwrap_or_else(|_| {
            Url::parse("file:///unknown").expect("static fallback URI is valid")
        })
    })
}

/// All mutable state owned by the LSP server for the lifetime of the process.
///
/// A single worker thread owns `ServerState` and processes all requests
/// sequentially — no Arc<Mutex<...>> needed (see design/lsp.md §Dispatch Model).
pub struct ServerState {
    pub db: CompilerDb,
    /// Mirrors the text the client has open. Applied before writing salsa inputs
    /// so we have a UTF-8 string to build line tables from.
    pub open_documents: HashMap<Url, String>,
    /// Pre-computed line-offset tables per open document.
    pub line_tables: HashMap<Url, LineTable>,
    /// Last set of diagnostics published per URI. Used to clear stale squiggles
    /// when a file is fixed (push empty list) and to avoid redundant publishes.
    pub last_published: HashMap<Url, Vec<lsp_types::Diagnostic>>,
    /// Negotiated position encoding for this session.
    pub encoding: PositionEncoding,
    /// Set by `shutdown` request; `exit` checks this.
    pub shutdown_requested: bool,
    /// Project root directory (contains `yinz.toml`). Set from the LSP Initialize
    /// `rootUri`/`rootPath`, or inferred lazily on first file open. `None` in
    /// single-file mode (no `yinz.toml` found anywhere).
    pub project_root: Option<PathBuf>,
}

impl ServerState {
    pub fn new(encoding: PositionEncoding) -> Self {
        Self {
            db: CompilerDb::default(),
            open_documents: HashMap::new(),
            line_tables: HashMap::new(),
            last_published: HashMap::new(),
            encoding,
            shutdown_requested: false,
            project_root: None,
        }
    }

    /// Set the project root from a file URI (e.g. `rootUri` from Initialize).
    /// Walks up from the URI's directory to find the nearest `yinz.toml`.
    /// If a root is found, immediately indexes all `.ynz` files in the project
    /// so cross-file completion and auto-import work without the user opening each file.
    pub fn set_project_root_from_uri(&mut self, uri: &Url) {
        if self.project_root.is_some() {
            return;
        }
        if let Ok(path) = uri.to_file_path() {
            if let Some(root) = find_project_root(&path) {
                self.project_root = Some(root.clone());
                self.index_project_files(&root);
            }
        }
    }

    /// Infer project root lazily from a file path when no rootUri was provided.
    pub fn infer_project_root_from_file(&mut self, file_path: &str) {
        if self.project_root.is_some() {
            return;
        }
        if let Some(root) = find_project_root(Path::new(file_path)) {
            self.project_root = Some(root.clone());
            self.index_project_files(&root);
        }
    }

    /// Walk `root` and register every `.ynz` file in the salsa db (read from disk).
    /// Already-registered files (open documents) are skipped. This runs once when the
    /// project root is discovered so cross-file queries see the whole project, not just
    /// the files the user has already opened.
    fn index_project_files(&mut self, root: &Path) {
        self.index_dir(root);
    }

    /// Public entry point for `serve()` which calls this before `main_loop`.
    pub fn index_project_files_pub(&mut self, root: &Path) {
        self.index_dir(root);
    }

    fn index_dir(&mut self, dir: &Path) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.index_dir(&path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("ynz") {
                let path_str = path.to_string_lossy().into_owned();
                // Skip files already registered (open documents take priority).
                if self.db.source_by_path(&path_str).is_some() {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&path) {
                    let sf = SourceFile::new(&self.db, path_str, text);
                    self.db.register_source(sf);
                }
            }
        }
    }

    /// Register a new document. Creates the salsa SourceFile input and warms
    /// the salsa cache by triggering a parse on the next query.
    pub fn open_document(&mut self, uri: Url, text: String) {
        let path = uri_to_path(&uri);
        let sf = SourceFile::new(&self.db, path, text.clone());
        self.db.register_source(sf);
        self.line_tables.insert(uri.clone(), LineTable::new(&text));
        self.open_documents.insert(uri, text);
    }

    /// Update an existing document with a full-text replacement (FULL sync mode).
    /// Writes the new text to the salsa input, triggering incremental invalidation.
    pub fn update_document(&mut self, uri: &Url, new_text: String) {
        if let Some(sf) = self.source_file_for(uri) {
            sf.set_text(&mut self.db).to(new_text.clone());
            self.line_tables
                .insert(uri.clone(), LineTable::new(&new_text));
            self.open_documents.insert(uri.clone(), new_text);
        }
    }

    pub fn close_document(&mut self, uri: &Url) {
        self.open_documents.remove(uri);
        self.line_tables.remove(uri);
    }

    /// Look up the registered SourceFile for a URI.
    pub fn source_file_for(&self, uri: &Url) -> Option<SourceFile> {
        let path = uri_to_path(uri);
        self.db.source_by_path(&path)
    }

    pub fn text_for(&self, uri: &Url) -> Option<&str> {
        self.open_documents.get(uri).map(String::as_str)
    }

    pub fn line_table_for(&self, uri: &Url) -> Option<&LineTable> {
        self.line_tables.get(uri)
    }
}

/// Convert a file URI to the path string used in salsa inputs and diagnostic spans.
/// Uses the URI's path component; falls back to the full URI string for non-file URIs.
pub fn uri_to_path(uri: &Url) -> String {
    uri.to_file_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| uri.to_string())
}

/// Walk up from `start` to find the nearest ancestor directory containing `yinz.toml`.
/// Returns `None` in single-file mode (no project manifest found).
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
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

/// Compute the project-root-relative import path for a source file.
///
/// Given `/project/root/services/users.ynz` and root `/project/root`,
/// returns `services/users` (forward-slash separated, no `.ynz` extension).
pub fn import_path_for_file(file_path: &str, project_root: &Path) -> Option<String> {
    let path = Path::new(file_path);
    let rel = path.strip_prefix(project_root).ok()?;
    let without_ext = rel.with_extension("");
    Some(without_ext.to_string_lossy().replace('\\', "/"))
}
