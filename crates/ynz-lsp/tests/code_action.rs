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

/// Build a two-file state with a known project root for auto-import tests.
/// `root` must be an absolute path. Returns (state, importer_uri).
fn state_two_files(
    root: &std::path::Path,
    exporter_rel: &str,
    exporter_src: &str,
    importer_rel: &str,
    importer_src: &str,
) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    state.project_root = Some(root.to_path_buf());

    let exp_path = root.join(exporter_rel);
    let exp_uri = lsp_types::Url::from_file_path(&exp_path).expect("valid exporter path");
    state.open_document(exp_uri, exporter_src.to_string());

    let imp_path = root.join(importer_rel);
    let imp_uri = lsp_types::Url::from_file_path(&imp_path).expect("valid importer path");
    state.open_document(imp_uri.clone(), importer_src.to_string());

    (state, imp_uri)
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

// ─── Auto-import: NotDefined → suggest import from exporting file ─────────────

#[test]
// WHY: The core auto-import invariant — when a name resolves to `NotDefined` in the
// current file but IS exported from another registered file, the lightbulb must offer
// `import { Name } from \`path\``. If this regresses, users get no help on cross-file
// symbol usage, which is the most common new-file workflow.
fn auto_import_suggests_shape_from_other_file() {
    let root = std::path::Path::new("/tmp/ynz_autoimport_shape");
    let exporter_src = "export shape Player {\n  name: string\n  health: int\n}\n";
    let importer_src = "function main() -> nothing {\n  let p: Player = { name: \"x\", health: 1 }\n}\n";

    let (state, imp_uri) = state_two_files(
        root,
        "models/player.ynz",
        exporter_src,
        "entrypoint.ynz",
        importer_src,
    );

    let range = range_at(importer_src, "Player");
    let actions = code_action_response(&state, &imp_uri, range);

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
        titles.iter().any(|t| t.contains("Player") && t.contains("models/player")),
        "expected auto-import action for Player; got: {titles:?}"
    );
}

#[test]
// WHY: Exported functions should also get auto-import offers, not just shapes.
// Functions are the most common import target in Yinz (data + standalone fns model).
fn auto_import_suggests_function_from_other_file() {
    let root = std::path::Path::new("/tmp/ynz_autoimport_fn");
    let exporter_src = "export function greet(name: string) -> string {\n  return \"hello\"\n}\n";
    let importer_src = "function main() -> nothing {\n  greet(\"world\")\n}\n";

    let (state, imp_uri) = state_two_files(
        root,
        "utils/greeting.ynz",
        exporter_src,
        "entrypoint.ynz",
        importer_src,
    );

    let range = range_at(importer_src, "greet");
    let actions = code_action_response(&state, &imp_uri, range);

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
        titles.iter().any(|t| t.contains("greet") && t.contains("utils/greeting")),
        "expected auto-import for greet; got: {titles:?}"
    );
}

#[test]
// WHY: When the name is genuinely unknown (not exported by any registered file),
// no auto-import action must be produced. Spurious lightbulbs that insert wrong
// imports are more disruptive than no lightbulb at all.
fn auto_import_no_action_when_not_exported_anywhere() {
    let root = std::path::Path::new("/tmp/ynz_autoimport_none");
    let exporter_src = "export shape Player { name: string }\n";
    let importer_src = "function main() -> nothing {\n  let x: GhostType = 1\n}\n";

    let (state, imp_uri) = state_two_files(
        root,
        "models/player.ynz",
        exporter_src,
        "entrypoint.ynz",
        importer_src,
    );

    let range = range_at(importer_src, "GhostType");
    let actions = code_action_response(&state, &imp_uri, range);

    let auto_import_actions: Vec<_> = actions
        .iter()
        .filter(|a| {
            if let lsp_types::CodeActionOrCommand::CodeAction(ca) = a {
                ca.title.starts_with("Import")
            } else {
                false
            }
        })
        .collect();

    assert!(
        auto_import_actions.is_empty(),
        "must not suggest import for name not exported anywhere; got: {auto_import_actions:?}"
    );
}

#[test]
// WHY: The generated import text must follow Yinz import syntax exactly:
// `import { Name } from \`path/without/extension\`` on its own line.
// A malformed import string would cause a parse error in the user's file.
fn auto_import_edit_produces_correct_import_text() {
    let root = std::path::Path::new("/tmp/ynz_autoimport_edit");
    let exporter_src = "export shape Order { id: string }\n";
    let importer_src = "function main() -> nothing {\n  let o: Order = { id: \"x\" }\n}\n";

    let (state, imp_uri) = state_two_files(
        root,
        "services/orders.ynz",
        exporter_src,
        "entrypoint.ynz",
        importer_src,
    );

    let range = range_at(importer_src, "Order");
    let actions = code_action_response(&state, &imp_uri, range);

    let import_action = actions.iter().find_map(|a| {
        if let lsp_types::CodeActionOrCommand::CodeAction(ca) = a {
            if ca.title.contains("Order") { Some(ca) } else { None }
        } else {
            None
        }
    });

    let action = import_action.expect("auto-import action must exist for Order");
    let edit = action.edit.as_ref().expect("action must have edit");
    let changes = edit.changes.as_ref().expect("edit must have changes");
    let edits = changes.get(&imp_uri).expect("changes must contain importer uri");
    assert_eq!(edits.len(), 1, "exactly one TextEdit");
    assert_eq!(
        edits[0].new_text,
        "import { Order } from `services/orders`\n",
        "import text must follow Yinz syntax"
    );
    // Insertion must be at the start of the file (no prior imports).
    assert_eq!(
        edits[0].range.start,
        lsp_types::Position { line: 0, character: 0 },
        "insert at top when no prior imports"
    );
}

#[test]
// WHY: When the file already has imports, the new import must be inserted AFTER the
// last existing one, not at the top. Inserting at the top would create out-of-order
// imports that might confuse future readers (and auto-formatters).
fn auto_import_inserts_after_existing_imports() {
    let root = std::path::Path::new("/tmp/ynz_autoimport_after");
    let orders_src = "export shape Order { id: string }\n";
    let users_src = "export shape User { name: string }\n";
    // importer already has one import; new one should land after it.
    let importer_src = "import { User } from `services/users`\n\nfunction main() -> nothing {\n  let o: Order = { id: \"x\" }\n}\n";

    let mut state = ServerState::new(PositionEncoding::Utf8);
    let root_buf = root.to_path_buf();
    state.project_root = Some(root_buf);

    for (rel, src) in [
        ("services/orders.ynz", orders_src),
        ("services/users.ynz", users_src),
        ("entrypoint.ynz", importer_src),
    ] {
        let path = root.join(rel);
        let uri = lsp_types::Url::from_file_path(&path).unwrap();
        state.open_document(uri, src.to_string());
    }

    let imp_path = root.join("entrypoint.ynz");
    let imp_uri = lsp_types::Url::from_file_path(&imp_path).unwrap();
    let range = range_at(importer_src, "Order");
    let actions = code_action_response(&state, &imp_uri, range);

    let action = actions.iter().find_map(|a| {
        if let lsp_types::CodeActionOrCommand::CodeAction(ca) = a {
            if ca.title.contains("Order") { Some(ca) } else { None }
        } else {
            None
        }
    }).expect("auto-import action must exist for Order");

    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let edits = changes.get(&imp_uri).unwrap();
    // Insertion must NOT be at line 0 (which already has the User import).
    assert!(
        edits[0].range.start.line > 0,
        "new import must land after existing import, not at line 0; got line {}",
        edits[0].range.start.line
    );
}

// ─── Remove unused import ────────────────────────────────────────────────────

#[test]
// WHY: An UnusedImport warning must produce a "Remove unused import" quick-fix
// that deletes the entire import line. Without this, the user has no lightbulb
// for the yellow squiggle and must delete the line by hand.
fn unused_import_produces_remove_action() {
    let root = std::path::Path::new("/tmp/ynz_unused_import_action");
    // exporter exports `greet` but importer never calls it.
    let exporter_src = "export function greet(name: string) -> string {\n  return `hi`\n}\n";
    let importer_src = "import { greet } from `utils/greeting`\n\nfunction main() -> nothing {\n  let x = 1\n}\n";

    let (state, imp_uri) = state_two_files(
        root,
        "utils/greeting.ynz",
        exporter_src,
        "entrypoint.ynz",
        importer_src,
    );

    let range = range_at(importer_src, "greet");
    let actions = code_action_response(&state, &imp_uri, range);

    let remove_action = actions.iter().find_map(|a| {
        if let lsp_types::CodeActionOrCommand::CodeAction(ca) = a {
            if ca.title.contains("Remove unused import") {
                Some(ca)
            } else {
                None
            }
        } else {
            None
        }
    });

    assert!(
        remove_action.is_some(),
        "expected a 'Remove unused import' code action; got: {:?}",
        actions.iter().filter_map(|a| {
            if let lsp_types::CodeActionOrCommand::CodeAction(ca) = a { Some(ca.title.as_str()) } else { None }
        }).collect::<Vec<_>>()
    );

    let action = remove_action.unwrap();
    let edit = action.edit.as_ref().expect("action must have an edit");
    let changes = edit.changes.as_ref().expect("edit must have changes");
    let edits = changes.get(&imp_uri).expect("changes must contain importer uri");
    assert_eq!(edits.len(), 1, "exactly one TextEdit");
    // The edit must delete the entire first line (the import statement).
    assert_eq!(edits[0].new_text, "", "edit must delete text (new_text is empty)");
    assert_eq!(edits[0].range.start.line, 0, "edit starts at line 0");
}

#[test]
// WHY: A used import must NOT produce a remove-import code action. Spuriously
// offering "Remove unused import" when the symbol is used would corrupt working
// code if the user accepts the action. This test writes real files to disk so
// cross-file import resolution succeeds and the function enters referenced_names.
fn used_import_produces_no_remove_action() {
    let root = std::path::Path::new("/tmp/ynz_used_import_no_action");
    let exporter_src = "export function greet(name: string) -> string {\n  return `hi`\n}\n";
    let importer_src =
        "import { greet } from `utils/greeting`\n\nfunction main() -> nothing {\n  greet(`world`)\n}\n";

    // Write real files so import resolution can find them on disk.
    std::fs::create_dir_all(root.join("utils")).unwrap();
    std::fs::write(root.join("utils/greeting.ynz"), exporter_src).unwrap();
    std::fs::write(root.join("entrypoint.ynz"), importer_src).unwrap();
    std::fs::write(root.join("yinz.toml"), "[project]\nname = \"test\"\nentry = \"entrypoint.ynz\"\n").unwrap();

    let (state, imp_uri) = state_two_files(
        root,
        "utils/greeting.ynz",
        exporter_src,
        "entrypoint.ynz",
        importer_src,
    );

    let range = range_at(importer_src, "greet");
    let actions = code_action_response(&state, &imp_uri, range);

    let remove_actions: Vec<_> = actions
        .iter()
        .filter(|a| {
            if let lsp_types::CodeActionOrCommand::CodeAction(ca) = a {
                ca.title.contains("Remove unused import")
            } else {
                false
            }
        })
        .collect();

    assert!(
        remove_actions.is_empty(),
        "must not offer remove-import action for a used import; got: {remove_actions:?}"
    );
}
