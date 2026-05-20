use std::collections::HashMap;

use lsp_types::Url;
use salsa::Setter as _;
use ynz_parser::db::{CompilerDb, SourceFile};

use crate::{capabilities::PositionEncoding, position::LineTable};

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
    /// Negotiated position encoding for this session.
    pub encoding: PositionEncoding,
    /// Set by `shutdown` request; `exit` checks this.
    pub shutdown_requested: bool,
}

impl ServerState {
    pub fn new(encoding: PositionEncoding) -> Self {
        Self {
            db: CompilerDb::default(),
            open_documents: HashMap::new(),
            line_tables: HashMap::new(),
            encoding,
            shutdown_requested: false,
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
            self.line_tables.insert(uri.clone(), LineTable::new(&new_text));
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
    uri.to_file_path().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| uri.to_string())
}
