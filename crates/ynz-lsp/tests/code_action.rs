// WHY: Code-action integration cases verify that quick-fixes are produced for
// BannedKeyword, banned-jargon, old-builtin-type and import diagnostics, that
// the replacement text is registry-driven (not hardcoded in the handler), and
// that diagnostics WITHOUT a fix return nothing instead of a spurious edit.
// Bugs here produce a missing lightbulb for a real error or a broken
// workspace-edit that corrupts source.
use lsp_types::{CodeAction, CodeActionOrCommand, TextEdit, Url};
use ynz_lsp::{
    capabilities::PositionEncoding, code_action::code_action_response, position::LineTable,
    state::ServerState,
};

/// One codeAction request: the open files, the request range and the file it targets.
struct Scenario {
    state: ServerState,
    uri: Url,
    range: lsp_types::Range,
}

fn open_in_project(
    root: &std::path::Path,
    files: &[(&str, &str)],
    target: &str,
    range_of: impl Fn(&str) -> lsp_types::Range,
) -> Scenario {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    state.project_root = Some(root.to_path_buf());
    let mut target_source = "";
    for (relative_path, source) in files {
        let uri = Url::from_file_path(root.join(relative_path)).expect("valid path");
        state.open_document(uri, source.to_string());
        if *relative_path == target {
            target_source = source;
        }
    }
    Scenario {
        state,
        uri: Url::from_file_path(root.join(target)).expect("valid path"),
        range: range_of(target_source),
    }
}

fn single_file(path: &str, src: &str, range_of: impl Fn(&str) -> lsp_types::Range) -> Scenario {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = Url::from_file_path(path).expect("valid path");
    state.open_document(uri.clone(), src.to_string());
    Scenario {
        state,
        uri,
        range: range_of(src),
    }
}

fn full_range(src: &str) -> lsp_types::Range {
    range_between(src, 0, src.len())
}

fn range_at(src: &str, needle: &str) -> lsp_types::Range {
    let offset = src
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in source"));
    range_between(src, offset, offset + needle.len())
}

fn range_between(src: &str, start: usize, end: usize) -> lsp_types::Range {
    let table = LineTable::new(src);
    lsp_types::Range {
        start: table.byte_offset_to_position(src, start, PositionEncoding::Utf8),
        end: table.byte_offset_to_position(src, end, PositionEncoding::Utf8),
    }
}

fn code_actions(actions: &[CodeActionOrCommand]) -> Vec<&CodeAction> {
    actions
        .iter()
        .filter_map(|action| match action {
            CodeActionOrCommand::CodeAction(code_action) => Some(code_action),
            CodeActionOrCommand::Command(_) => None,
        })
        .collect()
}

fn edits_for<'a>(actions: &[&'a CodeAction], uri: &Url) -> Vec<&'a TextEdit> {
    actions
        .iter()
        .filter_map(|action| action.edit.as_ref()?.changes.as_ref()?.get(uri))
        .flatten()
        .collect()
}

fn banned_keywords_on_two_lines(range_of: fn(&str) -> lsp_types::Range) -> Scenario {
    single_file(
        "/tmp/ynz_ca_multi.ynz",
        "class Foo { }\nenum Bar { a }\n",
        range_of,
    )
}

macro_rules! code_action_case {
    ($name:ident, $scenario:expr, $check:expr) => {
        // bug: codeAction offers no fix, a wrong replacement, a fix outside the requested range or a spurious fix for a banned keyword, banned jargon, old builtin type or import diagnostic | tier: Medium
        #[test]
        fn $name() {
            let scenario: Scenario = $scenario;
            let response = code_action_response(&scenario.state, &scenario.uri, scenario.range);
            let actions = code_actions(&response);
            let check: fn(&[&CodeAction], &Url) = $check;
            check(&actions, &scenario.uri);
        }
    };
}

// ─── BannedKeyword ───────────────────────────────────────────────────────────
code_action_case!(
    class_keyword_gets_a_quick_fix_replacing_it_with_shape,
    single_file(
        "/tmp/ynz_ca_class.ynz",
        "class Player { name: String }\n",
        full_range
    ),
    |actions, uri| {
        let edits = edits_for(actions, uri);
        assert!(
            edits.iter().any(|edit| edit.new_text == "shape"),
            "got {edits:?}"
        );
        assert!(
            actions
                .iter()
                .all(|action| action.kind == Some(lsp_types::CodeActionKind::QUICKFIX)),
            "every action is a quick-fix"
        );
    }
);
code_action_case!(
    enum_keyword_gets_a_quick_fix_replacing_it_with_options,
    single_file(
        "/tmp/ynz_ca_enum.ynz",
        "enum Status { active, inactive }\n",
        full_range
    ),
    |actions, uri| {
        let edits = edits_for(actions, uri);
        assert!(
            edits.iter().any(|edit| edit.new_text == "options"),
            "got {edits:?}"
        );
    }
);
code_action_case!(
    range_over_two_banned_keywords_offers_both_fixes,
    banned_keywords_on_two_lines(full_range),
    |actions, uri| {
        let new_texts: Vec<&str> = edits_for(actions, uri)
            .iter()
            .map(|edit| edit.new_text.as_str())
            .collect();
        assert!(
            new_texts.contains(&"shape") && new_texts.contains(&"options"),
            "got {new_texts:?}"
        );
    }
);
code_action_case!(
    range_over_one_banned_keyword_excludes_the_fix_on_another_line,
    banned_keywords_on_two_lines(|src| range_at(src, "class Foo { }")),
    |actions, uri| {
        let new_texts: Vec<&str> = edits_for(actions, uri)
            .iter()
            .map(|edit| edit.new_text.as_str())
            .collect();
        assert!(
            new_texts.contains(&"shape"),
            "class is in range; got {new_texts:?}"
        );
        assert!(
            !new_texts.contains(&"options"),
            "enum is on line 1, out of range; got {new_texts:?}"
        );
    }
);
// A missing-return error has no unambiguous single-token replacement.
code_action_case!(
    error_without_a_fix_offers_no_replacement,
    single_file(
        "/tmp/ynz_ca_nofix.ynz",
        "function broken() -> Int { let x = 1 }\n",
        full_range
    ),
    |actions, _uri| {
        let titles: Vec<&str> = actions.iter().map(|action| action.title.as_str()).collect();
        assert!(
            titles.iter().all(|title| !title.starts_with("Replace `")),
            "unexpected quick-fix for a diagnostic without one: {titles:?}"
        );
    }
);

// ─── Banned jargon ───────────────────────────────────────────────────────────
// `infer` is banned jargon; its replacement text comes from the registry.
code_action_case!(
    banned_jargon_identifier_gets_the_registry_replacement,
    single_file(
        "/tmp/ynz_ca_jargon_infer.ynz",
        "let infer = 5\n",
        full_range
    ),
    |actions, uri| {
        let action = actions
            .iter()
            .find(|action| action.title.contains("infer"))
            .unwrap_or_else(|| panic!("no action mentions `infer`: {actions:#?}"));
        let expected_replacement = ynz_registry::banned_jargon_lookup("infer")
            .expect("registry has a banned_jargon entry for `infer`")
            .replacement;
        let edits = edits_for(&[*action], uri);
        assert_eq!(edits[0].new_text, expected_replacement);
    }
);

// ─── Old lowercase builtin type ──────────────────────────────────────────────
// The edit text comes from the one shared old->new table; the edit must cover
// exactly the `int` token.
code_action_case!(
    old_lowercase_type_spelling_gets_a_quick_fix_to_pascal_case,
    {
        let src = "function entrypoint() -> Nothing {\n  let count: int = 1\n  print(count)\n}\n";
        let offset = src.find(": int").expect("`: int` in source") + 2;
        single_file("/tmp/ynz_old_builtin_type.ynz", src, |_| {
            range_between(src, offset, offset + 3)
        })
    },
    |actions, uri| {
        let int_fixes: Vec<&&CodeAction> = actions
            .iter()
            .filter(|action| {
                edits_for(&[**action], uri)
                    .iter()
                    .any(|edit| edit.new_text == "Int")
            })
            .collect();
        assert_eq!(int_fixes.len(), 1, "one `Int` fix; got {actions:#?}");
        assert_eq!(int_fixes[0].kind, Some(lsp_types::CodeActionKind::QUICKFIX));
        let edits = edits_for(&[*int_fixes[0]], uri);
        assert_eq!(edits.len(), 1);
        assert_eq!(
            edits[0].range.end.character - edits[0].range.start.character,
            3
        );
    }
);

// ─── Auto-import and unused import ───────────────────────────────────────────
const PLAYER_EXPORTER: &str = "export shape Player {\n  name: String\n  health: Int\n}\n";
const GREET_EXPORTER: &str = "export function greet(name: String) -> String {\n  return `hi`\n}\n";

code_action_case!(
    exported_shape_is_offered_as_an_import_at_the_top_of_the_file,
    open_in_project(
        std::path::Path::new("/tmp/ynz_autoimport_shape"),
        &[
            ("models/player.ynz", PLAYER_EXPORTER),
            (
                "entrypoint.ynz",
                "function main() -> Nothing {\n  let p: Player = { name: `x`, health: 1 }\n}\n"
            ),
        ],
        "entrypoint.ynz",
        |src| range_at(src, "Player")
    ),
    |actions, uri| {
        let edits = edits_for(actions, uri);
        let import = edits
            .iter()
            .find(|edit| edit.new_text == "import { Player } from `models/player`\n")
            .unwrap_or_else(|| panic!("no import edit for Player: {edits:?}"));
        assert_eq!(
            import.range.start,
            lsp_types::Position {
                line: 0,
                character: 0
            },
            "insert at the top when there is no prior import"
        );
    }
);
code_action_case!(
    exported_function_is_offered_as_an_import,
    open_in_project(
        std::path::Path::new("/tmp/ynz_autoimport_fn"),
        &[
            ("utils/greeting.ynz", GREET_EXPORTER),
            (
                "entrypoint.ynz",
                "function main() -> Nothing {\n  greet(`world`)\n}\n"
            ),
        ],
        "entrypoint.ynz",
        |src| range_at(src, "greet")
    ),
    |actions, uri| {
        let edits = edits_for(actions, uri);
        assert!(
            edits
                .iter()
                .any(|edit| edit.new_text == "import { greet } from `utils/greeting`\n"),
            "got {edits:?}"
        );
    }
);
code_action_case!(
    new_import_lands_after_the_existing_imports,
    open_in_project(
        std::path::Path::new("/tmp/ynz_autoimport_after"),
        &[
            ("services/orders.ynz", "export shape Order { id: String }\n"),
            ("services/users.ynz", "export shape User { name: String }\n"),
            (
                "entrypoint.ynz",
                "import { User } from `services/users`\n\nfunction main() -> Nothing {\n  let o: Order = { id: `x` }\n}\n"
            ),
        ],
        "entrypoint.ynz",
        |src| range_at(src, "Order")
    ),
    |actions, uri| {
        let edits = edits_for(actions, uri);
        let import = edits
            .iter()
            .find(|edit| edit.new_text.contains("Order"))
            .unwrap_or_else(|| panic!("no import edit for Order: {edits:?}"));
        assert!(
            import.range.start.line > 0,
            "the new import must land after the existing one on line 0"
        );
    }
);
code_action_case!(
    name_exported_nowhere_gets_no_import,
    open_in_project(
        std::path::Path::new("/tmp/ynz_autoimport_none"),
        &[
            (
                "models/player.ynz",
                "export shape Player { name: String }\n"
            ),
            (
                "entrypoint.ynz",
                "function main() -> Nothing {\n  let x: GhostType = 1\n}\n"
            ),
        ],
        "entrypoint.ynz",
        |src| range_at(src, "GhostType")
    ),
    |actions, _uri| {
        let imports: Vec<&&CodeAction> = actions
            .iter()
            .filter(|action| action.title.starts_with("Import"))
            .collect();
        assert!(imports.is_empty(), "got {imports:?}");
    }
);
code_action_case!(
    unused_import_gets_a_fix_deleting_its_line,
    open_in_project(
        std::path::Path::new("/tmp/ynz_unused_import_action"),
        &[
            ("utils/greeting.ynz", GREET_EXPORTER),
            (
                "entrypoint.ynz",
                "import { greet } from `utils/greeting`\n\nfunction main() -> Nothing {\n  let x = 1\n}\n"
            ),
        ],
        "entrypoint.ynz",
        |src| range_at(src, "greet")
    ),
    |actions, uri| {
        let removal = actions
            .iter()
            .find(|action| action.title.contains("Remove unused import"))
            .unwrap_or_else(|| panic!("no remove-import action: {actions:#?}"));
        let edits = edits_for(&[*removal], uri);
        assert_eq!(edits.len(), 1, "exactly one TextEdit");
        assert_eq!(edits[0].new_text, "");
        assert_eq!(edits[0].range.start.line, 0);
    }
);
// Real files on disk so import resolution finds the use of `greet`.
code_action_case!(
    used_import_gets_no_remove_fix,
    {
        let root = std::path::Path::new("/tmp/ynz_used_import_no_action");
        let importer_source = "import { greet } from `utils/greeting`\n\nfunction main() -> Nothing {\n  greet(`world`)\n}\n";
        std::fs::create_dir_all(root.join("utils")).unwrap();
        std::fs::write(root.join("utils/greeting.ynz"), GREET_EXPORTER).unwrap();
        std::fs::write(root.join("entrypoint.ynz"), importer_source).unwrap();
        std::fs::write(
            root.join("yinz.toml"),
            "[project]\nname = \"test\"\nentry = \"entrypoint.ynz\"\n",
        )
        .unwrap();
        open_in_project(
            root,
            &[
                ("utils/greeting.ynz", GREET_EXPORTER),
                ("entrypoint.ynz", importer_source),
            ],
            "entrypoint.ynz",
            |src| range_at(src, "greet"),
        )
    },
    |actions, _uri| {
        let removals: Vec<&&CodeAction> = actions
            .iter()
            .filter(|action| action.title.contains("Remove unused import"))
            .collect();
        assert!(removals.is_empty(), "got {removals:?}");
    }
);

// bug: code actions on 50 banned keywords take over 50 ms | tier: Medium
#[test]
fn nightly_code_action_response_under_50ms() {
    let src = "class Foo { }\n".repeat(50);
    let scenario = single_file("/tmp/ynz_ca_perf.ynz", &src, full_range);
    let start = std::time::Instant::now();
    let _ = code_action_response(&scenario.state, &scenario.uri, scenario.range);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 50,
        "code_action_response took {}ms; budget is 50ms",
        elapsed.as_millis()
    );
}
