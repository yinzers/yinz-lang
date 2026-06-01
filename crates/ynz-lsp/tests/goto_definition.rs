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

// Same-file: function definition

#[test]
fn test_goto_def_same_file_function_returns_location() {
    let src = "function greet() -> string { return `hi` }\nfunction entrypoint() -> nothing {\n  let msg = greet()\n}\n";
    let path = "/tmp/ynz_gotodef_fn.ynz";
    let (state, uri) = state_single(path, src);
    let pos = position_at(src, "greet()"); // offset at call site
    let result = definition_response(&state, &uri, pos);
    assert!(
        result.is_some(),
        "definition must resolve for same-file function call"
    );
    if let Some(lsp_types::GotoDefinitionResponse::Scalar(loc)) = result {
        // Declaration is at start of file — line 0.
        assert_eq!(loc.range.start.line, 0, "greet declared on line 0");
        assert_eq!(&loc.uri, &uri, "same-file jump stays in same URI");
    }
}

#[test]
fn test_goto_def_same_file_shape_returns_location() {
    let src = "shape Player {\n  name: string\n  health: int\n}\nfunction entrypoint() -> nothing {\n  let p: Player = { name: `Pat`, health: 100 }\n}\n";
    let path = "/tmp/ynz_gotodef_shape.ynz";
    let (state, uri) = state_single(path, src);
    // Position at `Player` in the type annotation.
    let annotation_pos = position_at(src, ": Player");
    // + 2 bytes to skip `: ` and land on `P`
    let pos = lsp_types::Position {
        line: annotation_pos.line,
        character: annotation_pos.character + 2,
    };
    let result = definition_response(&state, &uri, pos);
    assert!(
        result.is_some(),
        "definition must resolve for shape annotation"
    );
    if let Some(lsp_types::GotoDefinitionResponse::Scalar(loc)) = result {
        assert_eq!(loc.range.start.line, 0, "Player declared on line 0");
    }
}

// Edge cases: no definition found

#[test]
fn test_goto_def_whitespace_returns_none() {
    let src = "function entrypoint() -> nothing {   }\n";
    let path = "/tmp/ynz_gotodef_ws.ynz";
    let (state, uri) = state_single(path, src);
    // Position in the whitespace inside the braces.
    let pos = lsp_types::Position {
        line: 0,
        character: 35,
    };
    assert!(definition_response(&state, &uri, pos).is_none());
}

#[test]
fn test_goto_def_keyword_returns_none() {
    let src = "function entrypoint() -> nothing {}\n";
    let path = "/tmp/ynz_gotodef_kw.ynz";
    let (state, uri) = state_single(path, src);
    // Position on `function` keyword (offset 0).
    let pos = lsp_types::Position {
        line: 0,
        character: 0,
    };
    assert!(definition_response(&state, &uri, pos).is_none());
}

#[test]
fn test_goto_def_integer_literal_returns_none() {
    let src = "function entrypoint() -> nothing {\n  let x = 42\n}\n";
    let path = "/tmp/ynz_gotodef_int.ynz";
    let (state, uri) = state_single(path, src);
    let pos = position_at(src, "42");
    assert!(definition_response(&state, &uri, pos).is_none());
}

// Cross-file: imported function jumps to dep file

#[test]
fn test_goto_def_cross_file_imported_function_returns_dep_uri() {
    let dep_src = "export function announce(name: string) -> string {\n  return name\n}\n";
    let main_src = "import { announce } from `services/players`\nfunction entrypoint() -> nothing {\n  let msg = announce(`Roberto`)\n}\n";
    let (state, main_uri, dep_uri, _dir) = state_two_files("players", dep_src, main_src);

    let pos = position_at(main_src, "announce(`Roberto`)");
    let result = definition_response(&state, &main_uri, pos);
    assert!(result.is_some(), "cross-file import must resolve");
    if let Some(lsp_types::GotoDefinitionResponse::Scalar(loc)) = result {
        // Jump must point to the dep file, NOT the main file.
        assert_ne!(loc.uri, main_uri, "cross-file jump must go to dep file");
        assert_eq!(loc.uri, dep_uri, "jump must point to players.ynz");
        // `announce` is declared on line 0 of dep file.
        assert_eq!(
            loc.range.start.line, 0,
            "announce declared on line 0 of dep"
        );
    }
}

// Performance: < 100ms p95 for definition request

#[test]
fn test_goto_def_response_under_100ms() {
    let src = "function greet() -> string { return `hi` }\nfunction entrypoint() -> nothing {\n  let msg = greet()\n}\n";
    let path = "/tmp/ynz_gotodef_perf.ynz";
    let (state, uri) = state_single(path, src);
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
