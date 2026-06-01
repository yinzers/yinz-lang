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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{capabilities::PositionEncoding, state::ServerState};

    fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
        let mut state = ServerState::new(PositionEncoding::Utf8);
        let uri = lsp_types::Url::from_file_path(path).expect("valid path");
        state.open_document(uri.clone(), src.to_string());
        (state, uri)
    }

    #[test]
    fn no_project_root_returns_empty() {
        // WHY: In single-file mode there is no project root, so relative import
        // paths cannot be resolved. The handler must return [] rather than panic
        // or guess a path — callers would see garbage URLs.
        let src = "import { foo } from \"services/users\"\nfunction entrypoint() -> nothing { }\n";
        let (state, uri) = state_single("/tmp/ynz_dl_no_root.ynz", src);
        // state.project_root is None — single-file mode
        let links = document_link_response(&state, &uri);
        assert!(links.is_empty(), "no root → empty links, got: {links:?}");
    }

    #[test]
    fn import_with_nonexistent_target_has_no_url_but_has_range() {
        // WHY: An import whose .ynz file doesn't exist on disk must still get a
        // DocumentLink with a valid range (so the editor can underline the path)
        // but target must be None (we can't construct a valid file:// URL for a
        // missing file). target=Some(nonexistent) would cause the editor to show
        // a broken link, which is worse than no target.
        //
        // Uses backtick syntax — the canonical Yinz import path delimiter.
        let src = "import { foo } from `services/users`\nfunction entrypoint() -> nothing { }\n";
        let mut state = ServerState::new(PositionEncoding::Utf8);
        let uri = lsp_types::Url::from_file_path("/tmp/ynz_dl_nofile.ynz").expect("valid");
        state.project_root = Some(std::path::PathBuf::from("/tmp/nonexistent_project_root"));
        state.open_document(uri.clone(), src.to_string());

        let links = document_link_response(&state, &uri);
        assert_eq!(links.len(), 1, "one import → one link");
        assert!(links[0].target.is_none(), "missing file → target None");
        // source field holds the path without backtick delimiters; tooltip wraps it.
        assert!(
            links[0]
                .tooltip
                .as_deref()
                .unwrap_or("")
                .contains("services/users"),
            "tooltip should name the import path; got: {:?}",
            links[0].tooltip
        );
    }
}
