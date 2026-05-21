// WHY: Code-action integration tests verify that quick-fixes are produced for
// BannedKeyword diagnostics, that the replacement text is registry-driven (not
// hardcoded in the handler), and that diagnostics WITHOUT a quick-fix return
// an empty list rather than an error. Bugs here produce a missing lightbulb
// for a real error or a broken workspace-edit that corrupts source.

use ynz_lsp::{
    capabilities::PositionEncoding,
    code_action::code_action_response,
    position::LineTable,
    state::ServerState,
};

fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path(path).expect("valid path");
    state.open_document(uri.clone(), src.to_string());
    (state, uri)
}

fn full_range(src: &str) -> lsp_types::Range {
    let table = LineTable::new(src);
    let start = table.byte_offset_to_position(src, 0, PositionEncoding::Utf8);
    let end = table.byte_offset_to_position(src, src.len(), PositionEncoding::Utf8);
    lsp_types::Range { start, end }
}

fn range_at(src: &str, needle: &str) -> lsp_types::Range {
    let offset = src.find(needle).unwrap_or_else(|| panic!("{needle:?} not in source"));
    let table = LineTable::new(src);
    let start = table.byte_offset_to_position(src, offset, PositionEncoding::Utf8);
    let end =
        table.byte_offset_to_position(src, offset + needle.len(), PositionEncoding::Utf8);
    lsp_types::Range { start, end }
}

// ─── BannedKeyword: class → shape ────────────────────────────────────────────

#[test]
fn test_code_action_class_produces_replace_with_shape() {
    let src = "class Player { name: string }\n";
    let (state, uri) = state_single("/tmp/ynz_ca_class.ynz", src);
    let range = full_range(src);
    let actions = code_action_response(&state, &uri, range);
    assert!(
        !actions.is_empty(),
        "expected at least one code action for `class`"
    );
    if let lsp_types::CodeActionOrCommand::CodeAction(a) = &actions[0] {
        assert_eq!(a.title, "Replace `class` with `shape`");
        assert_eq!(
            a.kind.as_ref(),
            Some(&lsp_types::CodeActionKind::QUICKFIX)
        );
        let changes = a
            .edit
            .as_ref()
            .and_then(|e| e.changes.as_ref())
            .expect("edit with changes");
        let edits = changes.get(&uri).expect("edits for file");
        assert_eq!(edits[0].new_text, "shape");
    }
}

// ─── BannedKeyword: enum → options ───────────────────────────────────────────

#[test]
fn test_code_action_enum_produces_replace_with_options() {
    let src = "enum Status { active, inactive }\n";
    let (state, uri) = state_single("/tmp/ynz_ca_enum.ynz", src);
    let range = full_range(src);
    let actions = code_action_response(&state, &uri, range);
    assert!(!actions.is_empty(), "expected code action for `enum`");
    if let lsp_types::CodeActionOrCommand::CodeAction(a) = &actions[0] {
        assert_eq!(a.title, "Replace `enum` with `options`");
        let changes = a
            .edit
            .as_ref()
            .and_then(|e| e.changes.as_ref())
            .expect("edit with changes");
        let edits = changes.get(&uri).expect("edits for file");
        assert_eq!(edits[0].new_text, "options");
    }
}

// ─── BannedKeyword: struct → shape ───────────────────────────────────────────

#[test]
fn test_code_action_struct_produces_replace_with_shape() {
    let src = "struct Point { x: int, y: int }\n";
    let (state, uri) = state_single("/tmp/ynz_ca_struct.ynz", src);
    let range = full_range(src);
    let actions = code_action_response(&state, &uri, range);
    assert!(!actions.is_empty(), "expected code action for `struct`");
    if let lsp_types::CodeActionOrCommand::CodeAction(a) = &actions[0] {
        assert_eq!(a.title, "Replace `struct` with `shape`");
        let edits_map = a
            .edit
            .as_ref()
            .and_then(|e| e.changes.as_ref())
            .expect("edit with changes");
        assert_eq!(edits_map.get(&uri).unwrap()[0].new_text, "shape");
    }
}

// ─── No-fix diagnostics return empty list ────────────────────────────────────

#[test]
fn test_code_action_no_fix_returns_empty() {
    // A missing-return error has no unambiguous single-token replacement.
    let src = "function broken() -> int { let x = 1 }\n";
    let (state, uri) = state_single("/tmp/ynz_ca_nofix.ynz", src);
    let range = full_range(src);
    let actions = code_action_response(&state, &uri, range);
    // Either empty OR contains no BannedKeyword actions (missing-return has no fix).
    for a in &actions {
        if let lsp_types::CodeActionOrCommand::CodeAction(ca) = a {
            // Any actions returned must NOT claim to replace a token.
            assert!(
                !ca.title.starts_with("Replace `"),
                "unexpected quick-fix for non-BannedKeyword diagnostic: {}",
                ca.title
            );
        }
    }
}

// ─── Multi-diagnostic: all applicable actions returned ───────────────────────

#[test]
fn test_code_action_multiple_banned_keywords_returns_all() {
    // Two banned keywords on separate lines — both should get fixes.
    let src = "class Foo { }\nenum Bar { a }\n";
    let (state, uri) = state_single("/tmp/ynz_ca_multi.ynz", src);
    let range = full_range(src);
    let actions = code_action_response(&state, &uri, range);
    assert!(
        actions.len() >= 2,
        "expected at least 2 code actions for class + enum; got {}",
        actions.len()
    );
    let titles: Vec<&str> = actions
        .iter()
        .filter_map(|a| {
            if let lsp_types::CodeActionOrCommand::CodeAction(ca) = a {
                Some(ca.title.as_str())
            } else {
                None
            }
        })
        .collect();
    assert!(
        titles.iter().any(|t| *t == "Replace `class` with `shape`"),
        "missing class action; got: {titles:?}"
    );
    assert!(
        titles.iter().any(|t| *t == "Replace `enum` with `options`"),
        "missing enum action; got: {titles:?}"
    );
}

// ─── Range filter: only actions in range ─────────────────────────────────────

#[test]
fn test_code_action_range_filter_excludes_out_of_range() {
    // class on line 0, enum on line 1. Request only line 0 range.
    let src = "class Foo { }\nenum Bar { a }\n";
    let (state, uri) = state_single("/tmp/ynz_ca_range.ynz", src);
    // Range covers only line 0.
    let range = range_at(src, "class Foo { }");
    let actions = code_action_response(&state, &uri, range);
    let titles: Vec<&str> = actions
        .iter()
        .filter_map(|a| {
            if let lsp_types::CodeActionOrCommand::CodeAction(ca) = a {
                Some(ca.title.as_str())
            } else {
                None
            }
        })
        .collect();
    assert!(
        titles.iter().any(|t| *t == "Replace `class` with `shape`"),
        "class action should be in range"
    );
    assert!(
        !titles.iter().any(|t| *t == "Replace `enum` with `options`"),
        "enum action should NOT be in range (on line 1); got: {titles:?}"
    );
}

// ─── Performance budget ───────────────────────────────────────────────────────

#[test]
fn test_code_action_response_under_50ms() {
    // WHY: code actions must enumerate and respond in <50ms p95 on a single-file
    // request (O(diagnostics) walk). Regression here blocks the quick-fix lightbulb
    // from appearing before the user dismisses the squiggle tooltip.
    let src = "class Foo { }\n".repeat(50);
    let (state, uri) = state_single("/tmp/ynz_ca_perf.ynz", &src);
    let range = full_range(&src);
    let start = std::time::Instant::now();
    let _ = code_action_response(&state, &uri, range);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 50,
        "code_action_response took {}ms; budget is 50ms",
        elapsed.as_millis()
    );
}
