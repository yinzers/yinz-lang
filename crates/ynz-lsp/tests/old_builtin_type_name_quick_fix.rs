// WHY: the teaching error for an old lowercase built-in type (`int`) carries a quick-fix that
// rewrites it to the PascalCase name. A missing or wrong fix leaves the user retyping every
// type by hand after the rename. This file is separate from code_action.rs because its source
// holds the old spelling `int` on purpose, so the PascalCase codemod never rewrites it
// (FILES_NEVER_REWRITTEN in ynz-pascalcase-codemod).

use ynz_lsp::{
    capabilities::PositionEncoding, code_action::code_action_response, position::LineTable,
    state::ServerState,
};

// bug: the old lowercase type `int` gets no quick-fix, or a fix that writes anything but `Int` over exactly the `int` token | tier: Medium
#[test]
fn old_lowercase_type_spelling_gets_a_quick_fix_to_pascal_case() {
    let src = "function entrypoint() -> Nothing {\n  let count: int = 1\n  print(count)\n}\n";
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path("/tmp/ynz_old_builtin_type.ynz").expect("valid path");
    state.open_document(uri.clone(), src.to_string());

    let offset = src.find(": int").expect("`: int` in source") + 2;
    let table = LineTable::new(src);
    let range = lsp_types::Range {
        start: table.byte_offset_to_position(src, offset, PositionEncoding::Utf8),
        end: table.byte_offset_to_position(src, offset + 3, PositionEncoding::Utf8),
    };
    let actions = code_action_response(&state, &uri, range);
    let action = actions
        .iter()
        .find_map(|entry| match entry {
            lsp_types::CodeActionOrCommand::CodeAction(a) if a.title.contains("`Int`") => Some(a),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no quick-fix for `int`: {actions:#?}"));
    assert_eq!(action.title, "Write `Int` instead of `int`");
    assert_eq!(
        action.kind.as_ref(),
        Some(&lsp_types::CodeActionKind::QUICKFIX)
    );
    let edits = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .expect("edit for the file");
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "Int");
    assert_eq!(edits[0].range, range);
}
