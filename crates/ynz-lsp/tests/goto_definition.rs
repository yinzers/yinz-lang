// WHY: Go-to-definition integration tests verify that `def_site_for_offset` results
// are correctly converted to LSP Location values and that the URI↔SourceFile
// mapping is symmetric for same-file and cross-file jumps. Bugs here produce
// "No definition found" silently or jump to the wrong file/line.

use ynz_lsp::{
    capabilities::PositionEncoding, goto_definition::definition_response, position::LineTable,
    state::ServerState,
};

/// Build a `ServerState` with one registered file.
fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path(path).expect("valid file path");
    state.open_document(uri.clone(), src.to_string());
    (state, uri)
}

/// Build a `ServerState` with two real on-disk files and return (state, main_uri, dep_uri, tmpdir).
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
        "ynz_lsp_gotodef_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(dir.join("services")).expect("create dirs");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").unwrap();
    let dep_path = dir.join("services").join(format!("{services_name}.ynz"));
    std::fs::write(&dep_path, services_src).expect("write dep");
    let main_path = dir.join("entrypoint.ynz");
    std::fs::write(&main_path, main_src).expect("write main");

    let dep_path_str = dep_path.canonicalize().unwrap().display().to_string();
    let main_path_str = main_path.canonicalize().unwrap().display().to_string();

    let dep_uri = lsp_types::Url::from_file_path(&dep_path_str).expect("valid dep path");
    let main_uri = lsp_types::Url::from_file_path(&main_path_str).expect("valid main path");

    let mut state = ServerState::new(PositionEncoding::Utf8);
    state.open_document(dep_uri.clone(), services_src.to_string());
    state.open_document(main_uri.clone(), main_src.to_string());

    (state, main_uri, dep_uri, dir)
}

fn offset_of(src: &str, needle: &str) -> usize {
    src.find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in source"))
}

fn position_at(src: &str, needle: &str) -> lsp_types::Position {
    let offset = offset_of(src, needle);
    LineTable::new(src).byte_offset_to_position(src, offset, PositionEncoding::Utf8)
}

/// One go-to-definition request: where the cursor is and where the jump must land.
struct Scenario {
    state: ServerState,
    cursor_uri: lsp_types::Url,
    cursor: lsp_types::Position,
    /// File and line of the declaration, or `None` when nothing is definable under the cursor.
    expected_definition: Option<(lsp_types::Url, u32)>,
}

fn same_file_function_call() -> Scenario {
    let src = "function greet() -> String { return `hi` }\nfunction entrypoint() -> Nothing {\n  let msg = greet()\n}\n";
    let (state, uri) = state_single("/tmp/ynz_gotodef_fn.ynz", src);
    let cursor = position_at(src, "greet()");
    Scenario {
        state,
        expected_definition: Some((uri.clone(), 0)),
        cursor_uri: uri,
        cursor,
    }
}

fn same_file_shape_annotation() -> Scenario {
    let src = "shape Player {\n  name: String\n  health: Int\n}\nfunction entrypoint() -> Nothing {\n  let p: Player = { name: `Pat`, health: 100 }\n}\n";
    let (state, uri) = state_single("/tmp/ynz_gotodef_shape.ynz", src);
    // `: ` is two characters; land on the `P` of `Player`.
    let annotation = position_at(src, ": Player");
    let cursor = lsp_types::Position {
        line: annotation.line,
        character: annotation.character + 2,
    };
    Scenario {
        state,
        expected_definition: Some((uri.clone(), 0)),
        cursor_uri: uri,
        cursor,
    }
}

fn cross_file_imported_function() -> Scenario {
    let dep_src = "export function announce(name: String) -> String {\n  return name\n}\n";
    let main_src = "import { announce } from `services/players`\nfunction entrypoint() -> Nothing {\n  let msg = announce(`Roberto`)\n}\n";
    let (state, main_uri, dep_uri, _dir) = state_two_files("players", dep_src, main_src);
    Scenario {
        state,
        expected_definition: Some((dep_uri, 0)),
        cursor_uri: main_uri,
        cursor: position_at(main_src, "announce(`Roberto`)"),
    }
}

fn keyword_has_no_definition() -> Scenario {
    let src = "function entrypoint() -> Nothing {}\n";
    let (state, uri) = state_single("/tmp/ynz_gotodef_kw.ynz", src);
    Scenario {
        state,
        cursor_uri: uri,
        cursor: lsp_types::Position {
            line: 0,
            character: 0,
        },
        expected_definition: None,
    }
}

macro_rules! definition_case {
    ($name:ident, $scenario:ident) => {
        // bug: go-to-definition returns no location, the wrong file or the wrong line for a function call, a shape annotation or an imported function, or invents one for a keyword | tier: Medium
        #[test]
        fn $name() {
            let scenario = $scenario();
            let result =
                definition_response(&scenario.state, &scenario.cursor_uri, scenario.cursor);
            match scenario.expected_definition {
                Some((expected_uri, expected_line)) => {
                    let Some(lsp_types::GotoDefinitionResponse::Scalar(location)) = result else {
                        panic!("expected one location; got {result:?}");
                    };
                    assert_eq!(location.uri, expected_uri);
                    assert_eq!(location.range.start.line, expected_line);
                }
                None => assert!(result.is_none(), "got {result:?}"),
            }
        }
    };
}

definition_case!(
    same_file_function_call_jumps_to_its_declaration,
    same_file_function_call
);
definition_case!(
    same_file_shape_annotation_jumps_to_its_declaration,
    same_file_shape_annotation
);
definition_case!(
    cross_file_imported_function_jumps_to_the_dependency_file,
    cross_file_imported_function
);
definition_case!(
    keyword_under_cursor_has_no_definition,
    keyword_has_no_definition
);

// bug: a cold go-to-definition takes over 100 ms | tier: Medium
#[test]
fn nightly_goto_def_response_under_100ms() {
    let src = "function greet() -> String { return `hi` }\nfunction entrypoint() -> Nothing {\n  let msg = greet()\n}\n";
    let (state, uri) = state_single("/tmp/ynz_gotodef_perf.ynz", src);
    let pos = position_at(src, "greet()");
    // Cold call (cache miss): parse + typeck + symbol lookup.
    let start = std::time::Instant::now();
    let _ = definition_response(&state, &uri, pos);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 100,
        "go-to-def cold call took {}ms, expected < 100ms",
        elapsed.as_millis()
    );
}
