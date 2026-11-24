// WHY: Document link tests verify that the LSP handler emits clickable ranges
// for import paths and nothing otherwise. Bugs here silently break
// Ctrl-click navigation on imports — the most common way developers jump
// between files.

use ynz_lsp::{
    capabilities::PositionEncoding, document_link::document_link_response, state::ServerState,
};

const IMPORTING_SOURCE: &str =
    "import { foo } from `services/users`\nfunction entrypoint() -> Nothing { }\n";
const IMPORT_FREE_SOURCE: &str = "function entrypoint() -> Nothing { }\n";

macro_rules! document_link_case {
    ($name:ident, $source:expr, $project_root:expr, $expected_link_count:expr) => {
        // bug: documentLink returns a link for a file without imports or in single-file mode, none for an import in a project, or a target URL for a file that does not exist | tier: Medium
        #[test]
        fn $name() {
            let mut state = ServerState::new(PositionEncoding::Utf8);
            let uri =
                lsp_types::Url::from_file_path(concat!("/tmp/dl_", stringify!($name), ".ynz"))
                    .expect("valid path");
            state.project_root = $project_root.map(std::path::PathBuf::from);
            state.open_document(uri.clone(), $source.to_string());
            let links = document_link_response(&state, &uri);
            assert_eq!(links.len(), $expected_link_count, "got: {links:?}");
            // No target file exists under the root: a link must not point at a missing file.
            assert!(
                links.iter().all(|link| link.target.is_none()),
                "a link to a missing file must have no target; got: {links:?}"
            );
        }
    };
}

document_link_case!(
    import_in_a_project_yields_one_link,
    IMPORTING_SOURCE,
    Some("/tmp/nonexistent_root"),
    1
);
document_link_case!(
    file_without_imports_yields_no_links,
    IMPORT_FREE_SOURCE,
    Some("/tmp/nonexistent_root"),
    0
);
document_link_case!(
    single_file_mode_yields_no_links,
    IMPORTING_SOURCE,
    None::<&str>,
    0
);
