//! `textDocument/documentLink` LSP handler — clickable import path ranges.
//!
//! Walks the parsed module's `Item::ImportDecl` and `Item::ReExport` nodes and
//! produces a `DocumentLink` for each import source string.  Clicking the link
//! in the editor opens the target `.ynz` file.
//!
//! In single-file mode (`state.project_root` is `None`) the handler returns an
//! empty list — there is no project root to resolve relative import paths against.

use lsp_types::{DocumentLink, Range};
use ynz_ast::nodes::Item;

use crate::{capabilities::PositionEncoding, position::LineTable, state::ServerState};

/// Build the `textDocument/documentLink` response for `uri`.
///
/// Returns one `DocumentLink` per import source path found in the module.
/// The link's `target` is `Some(url)` when the file exists on disk, `None` when
/// the import path cannot be resolved (unresolved imports still get a range so
/// the user can see which string is the import path).
///
/// Time: O(imports) — one pass over top-level items.
pub fn document_link_response(state: &ServerState, uri: &lsp_types::Url) -> Vec<DocumentLink> {
    let project_root = match &state.project_root {
        Some(r) => r.clone(),
        None => return Vec::new(),
    };

    let text = match state.text_for(uri) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let sf = match state.source_file_for(uri) {
        Some(sf) => sf,
        None => return Vec::new(),
    };

    let parse_output = ynz_parser::queries::parse_query(&state.db, sf);
    let module = &parse_output.module;
    let table = LineTable::new(text);

    let mut links = Vec::new();

    for item in &module.items {
        match item {
            Item::ImportDecl(decl) => {
                let link = make_link(
                    text,
                    &decl.source,
                    &decl.source_span,
                    &table,
                    &project_root,
                    state.encoding,
                );
                links.push(link);
            }
            Item::ReExport(re) => {
                let link = make_link(
                    text,
                    &re.source,
                    &re.source_span,
                    &table,
                    &project_root,
                    state.encoding,
                );
                links.push(link);
            }
            _ => {}
        }
    }

    links
}

fn make_link(
    text: &str,
    source: &str,
    source_span: &ynz_diagnostics::SourceSpan,
    table: &LineTable,
    project_root: &std::path::Path,
    encoding: PositionEncoding,
) -> DocumentLink {
    let start = table.byte_offset_to_position(text, source_span.start, encoding);
    let end = table.byte_offset_to_position(text, source_span.end.min(text.len()), encoding);
    let range = Range { start, end };

    let abs_path = project_root.join(format!("{source}.ynz"));
    let target = if abs_path.exists() {
        lsp_types::Url::from_file_path(&abs_path).ok()
    } else {
        None
    };

    DocumentLink {
        range,
        target,
        tooltip: Some(format!("Open {source}.ynz")),
        data: None,
    }
}
