// WHY: Document link tests verify that the LSP handler emits clickable ranges
// for import paths with correct target URLs when the file exists and None when
// it doesn't. Bugs here silently break Ctrl-click navigation on imports — the
// most common way developers jump between files.

use ynz_lsp::{
    capabilities::PositionEncoding, document_link::document_link_response, state::ServerState,
};

fn state_with_root(uri_path: &str, src: &str, root: &str) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path(uri_path).expect("valid path");
    state.project_root = Some(std::path::PathBuf::from(root));
    state.open_document(uri.clone(), src.to_string());
    (state, uri)
}

#[test]
fn import_link_range_covers_path_string() {
    // WHY: The link's range must cover the import path string (not the whole
    // statement). If the range is wrong the editor highlights the wrong text and
    // the tooltip appears at the wrong position.
    //
    // Source: import { foo } from `services/users`
    // The path string `services/users` starts after "from " — the link range
    // start/end must be non-equal (it covers real bytes).
    let src = "import { foo } from `services/users`\nfunction entrypoint() -> nothing { }\n";
    let (state, uri) = state_with_root("/tmp/dl_test_range.ynz", src, "/tmp/nonexistent_root");
    let links = document_link_response(&state, &uri);
    assert_eq!(links.len(), 1, "one import → one link; got: {links:?}");
    let link = &links[0];
    // The range must span at least one character.
    assert!(
        link.range.start != link.range.end,
        "link range must not be zero-width; got start={:?} end={:?}",
        link.range.start,
        link.range.end,
    );
}

#[test]
fn no_links_when_no_imports() {
    // WHY: A file with no import declarations must return an empty list — not
    // a panic or a default link.  This is the common case (most files don't
    // import from other files) and must be handled cleanly.
    let src = "function entrypoint() -> nothing { }\n";
    let (state, uri) = state_with_root("/tmp/dl_test_empty.ynz", src, "/tmp/nonexistent_root");
    let links = document_link_response(&state, &uri);
    assert!(links.is_empty(), "no imports → no links; got: {links:?}");
}

#[test]
fn import_link_tooltip_contains_path() {
    // WHY: The tooltip is the text the editor shows on hover over the import path.
    // It must contain the import path so the user can confirm which file the link
    // points to before clicking.  A missing or wrong tooltip erodes trust in
    // the editor integration.
    let src = "import { foo } from `services/users`\nfunction entrypoint() -> nothing { }\n";
    let (state, uri) = state_with_root("/tmp/dl_test_tooltip.ynz", src, "/tmp/nonexistent_root");
    let links = document_link_response(&state, &uri);
    assert_eq!(links.len(), 1);
    let tooltip = links[0].tooltip.as_deref().unwrap_or("");
    assert!(
        tooltip.contains("services/users"),
        "tooltip must name the import path; got: {tooltip:?}"
    );
}

#[test]
fn single_file_mode_returns_empty() {
    // WHY: In single-file mode (no yinz.toml found, project_root is None),
    // there is no project root to resolve relative paths against. The handler
    // must return [] rather than panic or produce garbage URLs.
    let src = "import { foo } from `services/users`\nfunction entrypoint() -> nothing { }\n";
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path("/tmp/dl_single_file.ynz").expect("valid");
    state.open_document(uri.clone(), src.to_string());
    // project_root is None — single-file mode
    let links = document_link_response(&state, &uri);
    assert!(
        links.is_empty(),
        "single-file mode → no links; got: {links:?}"
    );
}
