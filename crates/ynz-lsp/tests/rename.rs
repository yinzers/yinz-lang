// WHY: Rename tests verify atomic WorkspaceEdit construction and that every
// rejected rename answers with its stable LSP error code (clients pattern-match
// on them). Bugs here corrupt user source code silently: a partial rename is
// worse than no rename.

use ynz_lsp::{
    capabilities::PositionEncoding,
    position::LineTable,
    rename::{prepare_rename_response, rename_response},
    state::ServerState,
};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers (mirror goto_definition.rs test helpers)
// ─────────────────────────────────────────────────────────────────────────────

fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path(path).expect("valid path");
    state.open_document(uri.clone(), src.to_string());
    (state, uri)
}

fn state_two_files(
    dep_name: &str,
    dep_src: &str,
    main_src: &str,
) -> (
    ServerState,
    lsp_types::Url,
    lsp_types::Url,
    std::path::PathBuf,
) {
    let dir = std::env::temp_dir().join(format!(
        "ynz_lsp_rename_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(dir.join("services")).expect("create dirs");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").unwrap();
    let dep_path = dir.join("services").join(format!("{dep_name}.ynz"));
    std::fs::write(&dep_path, dep_src).expect("write dep");
    let main_path = dir.join("entrypoint.ynz");
    std::fs::write(&main_path, main_src).expect("write main");

    let dep_path_str = dep_path.canonicalize().unwrap().display().to_string();
    let main_path_str = main_path.canonicalize().unwrap().display().to_string();
    let dep_uri = lsp_types::Url::from_file_path(&dep_path_str).unwrap();
    let main_uri = lsp_types::Url::from_file_path(&main_path_str).unwrap();

    let mut state = ServerState::new(PositionEncoding::Utf8);
    state.open_document(dep_uri.clone(), dep_src.to_string());
    state.open_document(main_uri.clone(), main_src.to_string());

    (state, main_uri, dep_uri, dir)
}

fn position_at(src: &str, needle: &str) -> lsp_types::Position {
    let offset = src
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in source"));
    LineTable::new(src).byte_offset_to_position(src, offset, PositionEncoding::Utf8)
}

fn mock_sender() -> crossbeam_channel::Sender<lsp_server::Message> {
    let (tx, _rx) = crossbeam_channel::unbounded();
    tx
}

fn error_code(result: Result<lsp_types::WorkspaceEdit, (i32, String)>) -> i32 {
    match result {
        Ok(edit) => panic!("rename must be rejected; got edit {edit:?}"),
        Err((code, _message)) => code,
    }
}

macro_rules! prepare_rename_case {
    ($name:ident, $src:expr, $needle:expr, $expected_width:expr) => {
        // bug: prepare_rename accepts a keyword or literal, or rejects a renameable identifier, or returns a range that is not the identifier | tier: Medium
        #[test]
        fn $name() {
            let src: &str = $src;
            let (state, uri) =
                state_single(concat!("/tmp/ynz_rename_", stringify!($name), ".ynz"), src);
            let range = prepare_rename_response(&state, &uri, position_at(src, $needle));
            let expected_width: Option<u32> = $expected_width;
            assert_eq!(
                range.map(|range| range.end.character - range.start.character),
                expected_width
            );
        }
    };
}

prepare_rename_case!(
    let_binding_name_is_renameable_over_exactly_its_identifier,
    "function entrypoint() -> Nothing {\n  let score = 42\n}\n",
    "score",
    Some(5)
);
prepare_rename_case!(
    function_call_site_is_renameable,
    "function greet() -> String { return `hi` }\nfunction entrypoint() -> Nothing {\n  let msg = greet()\n}\n",
    "greet()",
    Some(5)
);
prepare_rename_case!(
    keyword_is_not_renameable,
    "function entrypoint() -> Nothing {}\n",
    "function",
    None
);
prepare_rename_case!(
    integer_literal_is_not_renameable,
    "function entrypoint() -> Nothing {\n  let x = 99\n}\n",
    "99",
    None
);

macro_rules! rename_case {
    ($name:ident, $src:expr, $needle:expr, $new_name:expr, $check:expr) => {
        // bug: rename returns a wrong or partial WorkspaceEdit, errors on a symbol nobody calls, or accepts a bad new name, a conflicting name or a non-symbol position instead of the stable error code | tier: Medium
        #[test]
        fn $name() {
            let src: &str = $src;
            let (state, uri) =
                state_single(concat!("/tmp/ynz_rename_", stringify!($name), ".ynz"), src);
            let result = rename_response(
                &state,
                &uri,
                position_at(src, $needle),
                $new_name,
                &mock_sender(),
            );
            let check: fn(Result<lsp_types::WorkspaceEdit, (i32, String)>) = $check;
            check(result);
        }
    };
}

rename_case!(
    local_let_rename_edits_one_file_and_every_edit_writes_the_new_name,
    "function entrypoint() -> Nothing {\n  let score = 0\n  let total = score\n}\n",
    "score = 0",
    "points",
    |result| {
        let edit = result.expect("rename of a local let must succeed");
        let changes = edit.changes.expect("WorkspaceEdit must have changes");
        assert_eq!(changes.len(), 1, "only one file affected");
        let edits = changes.values().next().unwrap();
        assert_eq!(edits.len(), 2, "declaration and the one use");
        assert!(edits.iter().all(|edit| edit.new_text == "points"));
    }
);
rename_case!(
    function_without_callers_renames_without_error,
    "function isolated() -> Nothing {}\nfunction entrypoint() -> Nothing {}\n",
    "isolated",
    "orphan",
    |result| assert!(result.is_ok(), "got {result:?}")
);
rename_case!(
    new_name_that_is_not_an_identifier_is_rejected_with_32004,
    "function entrypoint() -> Nothing {\n  let x = 1\n}\n",
    "x",
    "123invalid",
    |result| assert_eq!(error_code(result), -32004)
);
rename_case!(
    new_name_that_is_a_keyword_is_rejected_with_32002,
    "function entrypoint() -> Nothing {\n  let count = 0\n}\n",
    "count",
    "let",
    |result| assert_eq!(error_code(result), -32002)
);
rename_case!(
    new_name_that_is_banned_jargon_is_rejected_with_32003,
    "function entrypoint() -> Nothing {\n  let x = 1\n}\n",
    "x",
    "class",
    |result| assert_eq!(error_code(result), -32003)
);
rename_case!(
    position_on_whitespace_is_rejected_with_32001,
    "function entrypoint() -> Nothing {  }\n",
    "  }",
    "newname",
    |result| assert_eq!(error_code(result), -32001)
);
// Two top-level functions with one name is a compile error right after the rename.
rename_case!(
    new_name_of_an_existing_function_is_rejected_with_32005,
    "function greet() -> String { return `hi` }\nfunction entrypoint() -> Nothing {}\n",
    "greet",
    "entrypoint",
    |result| assert_eq!(error_code(result), -32005)
);

// Own test, not a case of the table: it needs a two-file project on disk.
// bug: renaming an imported symbol from its import site edits only the importing file and leaves the origin broken, instead of the stable -32006 | tier: Medium
#[test]
fn renaming_an_imported_symbol_from_its_import_site_is_rejected_with_32006() {
    let dep_src = "export function greet() -> String { return `hi` }\n";
    let main_src = "import { greet } from `services/helpers`\nfunction entrypoint() -> Nothing {\n  let msg = greet()\n}\n";
    let (state, main_uri, _dep_uri, _dir) = state_two_files("helpers", dep_src, main_src);

    let call_site = position_at(main_src, "greet()");
    let result = rename_response(&state, &main_uri, call_site, "hello", &mock_sender());
    assert_eq!(error_code(result), -32006);
}

// bug: rename takes over 1 s and F2 feels frozen | tier: Medium
#[test]
fn nightly_rename_response_under_1000ms() {
    let src = "function greet() -> String { return `hi` }\n\
               function entrypoint() -> Nothing {\n  let a = greet()\n  let b = greet()\n}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_perf.ynz", src);
    let pos = position_at(src, "greet() -> String");
    let start = std::time::Instant::now();
    let _ = rename_response(&state, &uri, pos, "sayHi", &mock_sender());
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 1000,
        "rename took {}ms, expected < 1000ms",
        elapsed.as_millis()
    );
}
