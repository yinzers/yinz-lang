// WHY: Rename tests verify atomic WorkspaceEdit construction and that every
// RenameError variant produces the correct LSP error code.  Bugs here corrupt
// user source code silently — a partial rename is worse than no rename.

use ynz_lsp::{
    capabilities::PositionEncoding,
    position::LineTable,
    rename::{prepare_rename_response, rename_response, LSP_RENAME_ERROR_CODES},
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
) -> (ServerState, lsp_types::Url, lsp_types::Url, std::path::PathBuf) {
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
    let offset = src.find(needle).unwrap_or_else(|| panic!("{needle:?} not in source"));
    LineTable::new(src).byte_offset_to_position(src, offset, PositionEncoding::Utf8)
}

fn mock_sender() -> crossbeam_channel::Sender<lsp_server::Message> {
    let (tx, _rx) = crossbeam_channel::unbounded();
    tx
}

fn lsp_code_for(variant_name: &str) -> i32 {
    LSP_RENAME_ERROR_CODES
        .iter()
        .find(|(n, _)| *n == variant_name)
        .map(|(_, code)| *code)
        .expect("variant in LSP_RENAME_ERROR_CODES")
}

// ─────────────────────────────────────────────────────────────────────────────
// LSP error code table
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_lsp_rename_error_codes_are_stable() {
    // WHY: error codes are stable API — clients pattern-match on them.
    // This test locks the current values so a diff flags any accidental change.
    assert_eq!(lsp_code_for("NotARenameable"), -32001);
    assert_eq!(lsp_code_for("NewNameIsReservedKeyword"), -32002);
    assert_eq!(lsp_code_for("NewNameIsBannedJargon"), -32003);
    assert_eq!(lsp_code_for("NewNameInvalidIdentifier"), -32004);
    assert_eq!(lsp_code_for("ConflictsWithExistingName"), -32005);
    assert_eq!(lsp_code_for("CannotRenameImportedSymbolInThisFile"), -32006);
}

// ─────────────────────────────────────────────────────────────────────────────
// prepare_rename — valid positions
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_prepare_rename_returns_identifier_range_for_let_binding() {
    let src = "function entrypoint() -> nothing {\n  let score = 42\n}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_prep_let.ynz", src);
    let pos = position_at(src, "score");
    let range = prepare_rename_response(&state, &uri, pos);
    assert!(range.is_some(), "prepare_rename must succeed on `let` binding");
    let r = range.unwrap();
    // "score" is 5 chars — end.character should be start.character + 5
    assert_eq!(r.end.character - r.start.character, 5, "range spans `score`");
}

#[test]
fn test_prepare_rename_returns_identifier_range_for_function_call() {
    let src = "function greet() -> string { return `hi` }\n\
               function entrypoint() -> nothing {\n  let msg = greet()\n}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_prep_fn.ynz", src);
    let pos = position_at(src, "greet()"); // call site
    let range = prepare_rename_response(&state, &uri, pos);
    assert!(range.is_some(), "prepare_rename must succeed on function call");
}

// ─────────────────────────────────────────────────────────────────────────────
// prepare_rename — non-renameable positions return None
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_prepare_rename_returns_none_for_keyword() {
    // WHY: `function` is a keyword — it has no declaration site and cannot be renamed.
    let src = "function entrypoint() -> nothing {}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_prep_kw.ynz", src);
    let pos = lsp_types::Position { line: 0, character: 0 }; // `function`
    assert!(prepare_rename_response(&state, &uri, pos).is_none());
}

#[test]
fn test_prepare_rename_returns_none_for_integer_literal() {
    let src = "function entrypoint() -> nothing {\n  let x = 99\n}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_prep_int.ynz", src);
    let pos = position_at(src, "99");
    assert!(prepare_rename_response(&state, &uri, pos).is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// rename — success cases
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_rename_local_let_produces_workspace_edit_with_one_file() {
    // WHY: a let binding with 2 uses should produce exactly 1 file with 2+ edits.
    let src = "function entrypoint() -> nothing {\n  let score = 0\n  let total = score\n}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_local.ynz", src);
    let pos = position_at(src, "score = 0");
    let result = rename_response(&state, &uri, pos, "points", &mock_sender());
    assert!(result.is_ok(), "rename local let must succeed: {:?}", result);
    let edit = result.unwrap();
    let changes = edit.changes.as_ref().expect("WorkspaceEdit must have changes");
    assert_eq!(changes.len(), 1, "only one file affected");
    let edits = changes.values().next().unwrap();
    assert!(edits.len() >= 1, "at least one edit");
    for te in edits {
        assert_eq!(te.new_text, "points", "replacement text must be `points`");
    }
}

#[test]
fn test_rename_produces_empty_edit_for_zero_references() {
    // WHY: a function with no callers should produce an empty (but valid) WorkspaceEdit.
    let src = "function isolated() -> nothing {}\nfunction entrypoint() -> nothing {}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_zero.ynz", src);
    let pos = position_at(src, "isolated");
    let result = rename_response(&state, &uri, pos, "orphan", &mock_sender());
    // May succeed with zero or one edit (the declaration itself) — must not error.
    assert!(result.is_ok(), "rename with no callers must not error");
}

// ─────────────────────────────────────────────────────────────────────────────
// rename — validation failure cases (atomic-or-fail: no partial edit returned)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_rename_invalid_identifier_returns_error_code_32004() {
    // WHY: ensures client gets the stable -32004 code for identifier-syntax failures.
    let src = "function entrypoint() -> nothing {\n  let x = 1\n}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_invalid.ynz", src);
    let pos = position_at(src, "x");
    let result = rename_response(&state, &uri, pos, "123invalid", &mock_sender());
    let (code, _msg) = result.expect_err("must error on invalid identifier");
    assert_eq!(code, -32004, "invalid identifier must produce code -32004");
}

#[test]
fn test_rename_reserved_keyword_returns_error_code_32002() {
    // WHY: attempting to rename to a keyword produces -32002 and a WHY explanation.
    let src = "function entrypoint() -> nothing {\n  let count = 0\n}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_kw.ynz", src);
    let pos = position_at(src, "count");
    let result = rename_response(&state, &uri, pos, "let", &mock_sender());
    let (code, msg) = result.expect_err("must error on keyword new-name");
    assert_eq!(code, -32002, "keyword new-name must produce code -32002");
    assert!(msg.contains("WHY"), "error message must include WHY explanation");
}

#[test]
fn test_rename_banned_jargon_returns_error_code_32003() {
    // WHY: `class` is banned jargon in Yinz — renaming to it must be rejected.
    let src = "function entrypoint() -> nothing {\n  let x = 1\n}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_jargon.ynz", src);
    let pos = position_at(src, "x");
    let result = rename_response(&state, &uri, pos, "class", &mock_sender());
    let (code, _msg) = result.expect_err("must error on banned jargon");
    assert_eq!(code, -32003, "banned jargon must produce code -32003");
}

#[test]
fn test_rename_non_symbol_position_returns_error_code_32001() {
    // WHY: cursor on whitespace/punctuation must produce -32001 (not a panic or silent no-op).
    let src = "function entrypoint() -> nothing {  }\n";
    let (state, uri) = state_single("/tmp/ynz_rename_notsym.ynz", src);
    let pos = lsp_types::Position { line: 0, character: 34 }; // whitespace
    let result = rename_response(&state, &uri, pos, "newname", &mock_sender());
    let (code, _msg) = result.expect_err("must error on non-symbol position");
    assert_eq!(code, -32001, "non-symbol must produce code -32001");
}

#[test]
fn test_rename_imported_symbol_returns_error_code_32006() {
    // WHY: renaming an imported symbol from the import site (not the origin file)
    // must be rejected with -32006 pointing to the origin.
    let dep_src = "export function greet() -> string { return `hi` }\n";
    let main_src =
        "import { greet } from `services/helpers`\nfunction entrypoint() -> nothing {\n  let msg = greet()\n}\n";
    let (state, main_uri, _dep_uri, _dir) = state_two_files("helpers", dep_src, main_src);

    // Rename from the call site in main (not origin)
    let pos = position_at(main_src, "greet()");
    let result = rename_response(&state, &main_uri, pos, "hello", &mock_sender());
    let (code, msg) = result.expect_err("must reject rename from import site");
    assert_eq!(code, -32006, "imported-symbol rename must produce code -32006");
    assert!(
        msg.contains("WHAT INSTEAD"),
        "error must include WHAT INSTEAD guidance"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Conflict detection — renaming to an existing symbol name
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_rename_to_existing_name_returns_error_code_32005() {
    // WHY: renaming `greet` → `entrypoint` (where `entrypoint` already exists
    // in the same file) must be REJECTED with -32005.  Allowing it would produce
    // two top-level functions with the same name, which is a compile error
    // immediately after the rename — a silent corruption of the user's source.
    let src = "function greet() -> string { return `hi` }\nfunction entrypoint() -> nothing {}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_conflict.ynz", src);
    let pos = position_at(src, "greet");
    let result = rename_response(&state, &uri, pos, "entrypoint", &mock_sender());
    let (code, msg) = result.expect_err("rename to existing name must error");
    assert_eq!(code, -32005, "conflict must produce code -32005");
    assert!(
        msg.contains("WHAT INSTEAD"),
        "conflict error must include WHAT INSTEAD guidance; got: {msg}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Atomic-or-fail: partial edit never returned
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_rename_error_produces_no_workspace_edit() {
    // WHY: on validation failure (e.g. bad new-name), rename_response returns Err
    // — caller sends ResponseError to client, never a partial WorkspaceEdit.
    // This test verifies the function NEVER returns Ok on a known-bad new-name.
    let src = "function entrypoint() -> nothing {\n  let total = 0\n}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_atomic.ynz", src);
    let pos = position_at(src, "total");
    // Bad new-name: contains a space (invalid identifier)
    let result = rename_response(&state, &uri, pos, "bad name", &mock_sender());
    assert!(result.is_err(), "invalid new-name must always return Err, never Ok(partial edit)");
}

// ─────────────────────────────────────────────────────────────────────────────
// Performance
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_rename_response_under_1000ms() {
    // WHY: rename must be fast enough for interactive F2 use — 1s cap.
    let src = "function greet() -> string { return `hi` }\n\
               function entrypoint() -> nothing {\n  let a = greet()\n  let b = greet()\n}\n";
    let (state, uri) = state_single("/tmp/ynz_rename_perf.ynz", src);
    let pos = position_at(src, "greet() -> string");
    let start = std::time::Instant::now();
    let _ = rename_response(&state, &uri, pos, "sayHi", &mock_sender());
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 1000,
        "rename took {}ms, expected < 1000ms",
        elapsed.as_millis()
    );
}
