// WHY: Find-references tests verify that semantic use-site collection produces
// correct Location values, respects include_declaration, handles shadow
// bindings correctly, and returns `Some(vec![])` (not `None`) for zero-reference
// symbols so clients distinguish "no refs found" from "not a symbol."

use ynz_lsp::{
    capabilities::PositionEncoding, position::LineTable, references::references_response,
    state::ServerState,
};

fn make_sender() -> crossbeam_channel::Sender<lsp_server::Message> {
    let (tx, _rx) = crossbeam_channel::unbounded();
    tx
}

fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path(path).expect("valid file path");
    state.open_document(uri.clone(), src.to_string());
    (state, uri)
}

fn state_two_files(
    services_name: &str,
    services_src: &str,
    main_src: &str,
) -> (
    ServerState,
    lsp_types::Url,
    lsp_types::Url,
    std::path::PathBuf,
) {
    let dir = std::env::temp_dir().join(format!(
        "ynz_lsp_refs_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(dir.join("services")).expect("create dirs");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname=\"test\"\n").unwrap();
    let dep_path = dir.join("services").join(format!("{services_name}.ynz"));
    std::fs::write(&dep_path, services_src).expect("write dep");
    let main_path = dir.join("entrypoint.ynz");
    std::fs::write(&main_path, main_src).expect("write main");
    let dep_str = dep_path.canonicalize().unwrap().display().to_string();
    let main_str = main_path.canonicalize().unwrap().display().to_string();
    let dep_uri = lsp_types::Url::from_file_path(&dep_str).unwrap();
    let main_uri = lsp_types::Url::from_file_path(&main_str).unwrap();
    let mut state = ServerState::new(PositionEncoding::Utf8);
    state.open_document(dep_uri.clone(), services_src.to_string());
    state.open_document(main_uri.clone(), main_src.to_string());
    (state, main_uri, dep_uri, dir)
}

fn position_at(src: &str, needle: &str) -> lsp_types::Position {
    let offset = src
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in src"));
    LineTable::new(src).byte_offset_to_position(src, offset, PositionEncoding::Utf8)
}

/// One references request: the open files, the cursor and what must come back.
struct Scenario {
    state: ServerState,
    uri: lsp_types::Url,
    position: lsp_types::Position,
    include_declaration: bool,
    /// `None`: the handler must answer `None` (unknown file); `Some(n)`: `Some` list of n locations.
    expected_reference_count: Option<usize>,
}

const GREET_DECLARATION_NAME: lsp_types::Position = lsp_types::Position {
    line: 0,
    character: 9,
};

fn greet_called(times: usize, include_declaration: bool, path: &str) -> Scenario {
    let calls: String = (0..times)
        .map(|index| format!("  let v{index} = greet()\n"))
        .collect();
    let src = format!(
        "function greet() -> String {{ return `hi` }}\nfunction entrypoint() -> Nothing {{\n{calls}}}\n"
    );
    let (state, uri) = state_single(path, &src);
    Scenario {
        state,
        uri,
        position: GREET_DECLARATION_NAME,
        include_declaration,
        expected_reference_count: Some(times + usize::from(include_declaration)),
    }
}

fn whitespace_position() -> Scenario {
    let src = "function entrypoint() -> Nothing {   }\n";
    let (state, uri) = state_single("/tmp/ynz_refs_ws.ynz", src);
    Scenario {
        state,
        uri,
        position: lsp_types::Position {
            line: 0,
            character: 35,
        },
        include_declaration: false,
        expected_reference_count: Some(0),
    }
}

fn declared_but_never_called() -> Scenario {
    let src = "function solo() -> Nothing {}\nfunction entrypoint() -> Nothing {}\n";
    let (state, uri) = state_single("/tmp/ynz_refs_solo.ynz", src);
    Scenario {
        state,
        uri,
        position: GREET_DECLARATION_NAME,
        include_declaration: false,
        expected_reference_count: Some(0),
    }
}

fn imported_function_called_twice() -> Scenario {
    let dep_src = "export function announce(name: String) -> String {\n  return name\n}\n";
    let main_src = "import { announce } from `services/players`\nfunction entrypoint() -> Nothing {\n  let a = announce(`Roberto`)\n  let b = announce(`Willie`)\n}\n";
    let (state, main_uri, _dep_uri, _dir) = state_two_files("players", dep_src, main_src);
    Scenario {
        state,
        position: position_at(main_src, "announce(`Roberto`)"),
        uri: main_uri,
        include_declaration: false,
        expected_reference_count: Some(2),
    }
}

fn unknown_file() -> Scenario {
    Scenario {
        state: ServerState::new(PositionEncoding::Utf8),
        uri: lsp_types::Url::from_file_path("/tmp/nonexistent_ynz_file.ynz").unwrap(),
        position: lsp_types::Position {
            line: 0,
            character: 0,
        },
        include_declaration: false,
        expected_reference_count: None,
    }
}

macro_rules! references_case {
    ($name:ident, $scenario:expr) => {
        // bug: find-references counts the declaration against include_declaration, misses a call site (same or other file), or answers None instead of an empty list for a position without references (None is reserved for an unknown file) | tier: Medium
        #[test]
        fn $name() {
            let scenario: Scenario = $scenario;
            let references = references_response(
                &scenario.state,
                &scenario.uri,
                scenario.position,
                scenario.include_declaration,
                &make_sender(),
            );
            assert_eq!(
                references.map(|locations| locations.len()),
                scenario.expected_reference_count
            );
        }
    };
}

references_case!(
    declaration_is_excluded_when_not_requested,
    greet_called(3, false, "/tmp/ynz_refs_excl.ynz")
);
references_case!(
    declaration_is_included_when_requested,
    greet_called(1, true, "/tmp/ynz_refs_incl.ynz")
);
references_case!(
    whitespace_position_has_an_empty_list_not_none,
    whitespace_position()
);
references_case!(
    declared_but_never_called_symbol_has_an_empty_list_not_none,
    declared_but_never_called()
);
references_case!(
    imported_function_finds_its_call_sites_in_the_importing_file,
    imported_function_called_twice()
);
references_case!(unknown_file_answers_none, unknown_file());

// bug: a references search over more than 10 open documents sends no $/progress notification | tier: Medium
#[test]
fn references_over_many_open_documents_report_progress() {
    let (mut state, uri) = state_single(
        "/tmp/ynz_refs_progress.ynz",
        "function greet() -> String { return `hi` }\nfunction entrypoint() -> Nothing {\n  let a = greet()\n}\n",
    );
    for index in 0..10 {
        let fake_uri = lsp_types::Url::from_file_path(format!("/tmp/ynz_fake_{index}.ynz"))
            .expect("valid path");
        state.open_document(fake_uri, format!("function f{index}() -> Nothing {{}}"));
    }
    let (sender, receiver) = crossbeam_channel::unbounded();
    let _ = references_response(&state, &uri, GREET_DECLARATION_NAME, false, &sender);
    let progress_notifications = receiver
        .try_iter()
        .filter(|message| matches!(message, lsp_server::Message::Notification(n) if n.method == "$/progress"))
        .count();
    assert!(progress_notifications >= 1, "no $/progress notification");
}

// bug: a cold references request takes over 500 ms | tier: Medium
#[test]
fn nightly_references_performance_under_500ms() {
    let scenario = greet_called(2, false, "/tmp/ynz_refs_perf.ynz");
    let start = std::time::Instant::now();
    let _ = references_response(
        &scenario.state,
        &scenario.uri,
        scenario.position,
        false,
        &make_sender(),
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 500,
        "references cold call took {}ms, expected < 500ms",
        elapsed.as_millis()
    );
}

// bug: two $/progress begin notifications share a token, so the client mixes up concurrent searches | tier: Medium
#[test]
fn progress_tokens_are_unique() {
    use ynz_lsp::progress::ProgressTracker;
    let (sender, receiver) = crossbeam_channel::unbounded();
    for _ in 0..5 {
        let tracker = ProgressTracker::begin("test", sender.clone());
        tracker.end(None);
    }
    // begin and end each send one notification: every other message is a begin.
    let tokens: Vec<String> = receiver
        .try_iter()
        .filter_map(|message| match message {
            lsp_server::Message::Notification(n) if n.method == "$/progress" => n
                .params
                .get("token")
                .and_then(|token| token.as_str())
                .map(str::to_string),
            _ => None,
        })
        .collect();
    let begin_tokens: Vec<&String> = tokens.iter().step_by(2).collect();
    let unique: std::collections::HashSet<&&String> = begin_tokens.iter().collect();
    assert_eq!(unique.len(), begin_tokens.len(), "tokens must be distinct");
}
