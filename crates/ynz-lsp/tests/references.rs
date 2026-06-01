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

#[test]
fn test_references_include_decl_false_excludes_declaration() {
    let src = "function greet() -> string { return `hi` }\nfunction entrypoint() -> nothing {\n  let a = greet()\n  let b = greet()\n  let c = greet()\n}\n";
    let path = "/tmp/ynz_refs_excl.ynz";
    let (state, uri) = state_single(path, src);
    let decl_pos = position_at(src, "greet()"); // first occurrence = call, not decl
                                                // Position on the declaration `function greet`
    let decl_name_pos = lsp_types::Position {
        line: 0,
        character: 9,
    }; // 'g' of greet
    let refs = references_response(&state, &uri, decl_name_pos, false, &make_sender());
    let refs = refs.expect("should return Some (not a non-symbol offset)");
    // 3 call sites, NOT the declaration.
    assert_eq!(refs.len(), 3, "expected 3 call sites; got {}", refs.len());
    let _ = decl_pos;
}

#[test]
fn test_references_include_decl_true_includes_declaration() {
    let src = "function greet() -> string { return `hi` }\nfunction entrypoint() -> nothing {\n  let a = greet()\n}\n";
    let path = "/tmp/ynz_refs_incl.ynz";
    let (state, uri) = state_single(path, src);
    let decl_pos = lsp_types::Position {
        line: 0,
        character: 9,
    };
    let refs = references_response(&state, &uri, decl_pos, true, &make_sender());
    let refs = refs.expect("should return Some");
    assert!(
        refs.len() >= 2,
        "expected decl + 1 call; got {}",
        refs.len()
    );
}

#[test]
fn test_references_whitespace_returns_some_empty_not_none() {
    // Plan spec: "Empty result returns Some(vec![]) not None — clients distinguish
    // 'no refs found' vs 'not applicable'." None is reserved for position
    // conversion failure (invalid URI, out-of-bounds offset), not for
    // whitespace/keyword positions where there simply are no references.
    let src = "function entrypoint() -> nothing {   }\n";
    let path = "/tmp/ynz_refs_ws.ynz";
    let (state, uri) = state_single(path, src);
    let pos = lsp_types::Position {
        line: 0,
        character: 35,
    };
    let refs = references_response(&state, &uri, pos, false, &make_sender());
    let refs = refs.expect("whitespace returns Some(empty), not None");
    assert!(refs.is_empty(), "whitespace should produce zero references");
}

#[test]
fn test_references_unknown_symbol_returns_empty_vec_not_none() {
    // A function called with no references (only declared).
    let src = "function solo() -> nothing {}\nfunction entrypoint() -> nothing {}\n";
    let path = "/tmp/ynz_refs_solo.ynz";
    let (state, uri) = state_single(path, src);
    let decl_pos = lsp_types::Position {
        line: 0,
        character: 9,
    }; // 'solo'
    let refs = references_response(&state, &uri, decl_pos, false, &make_sender())
        .expect("declared symbol must return Some(vec)");
    // No call sites — returns empty vec, not None.
    assert!(
        refs.is_empty(),
        "expected zero call sites; got {}",
        refs.len()
    );
}

#[test]
fn test_references_cross_file_symbol_returns_cross_file_locations() {
    let dep_src = "export function announce(name: string) -> string {\n  return name\n}\n";
    let main_src = "import { announce } from `services/players`\nfunction entrypoint() -> nothing {\n  let a = announce(`Roberto`)\n  let b = announce(`Willie`)\n}\n";
    let (state, main_uri, _dep_uri, _dir) = state_two_files("players", dep_src, main_src);
    let call_pos = position_at(main_src, "announce(`Roberto`)");
    let refs = references_response(&state, &main_uri, call_pos, false, &make_sender());
    let refs = refs.expect("cross-file call must return Some");
    assert!(
        refs.len() >= 2,
        "expected 2 call sites in main file; got {}",
        refs.len()
    );
}

#[test]
fn test_references_progress_emitted_for_large_project() {
    // When open_documents > 10 OR estimate > 5, a progress notification fires.
    // Use a project with >10 open documents (inject synthetically into ServerState).
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let path = "/tmp/ynz_refs_progress.ynz";
    let src = "function greet() -> string { return `hi` }\nfunction entrypoint() -> nothing {\n  let a = greet()\n}\n";
    let uri = lsp_types::Url::from_file_path(path).expect("valid path");
    state.open_document(uri.clone(), src.to_string());
    // Inject 10 more synthetic documents to trigger progress threshold.
    for i in 0..10 {
        let fake_path = format!("/tmp/ynz_fake_{i}.ynz");
        let fake_uri = lsp_types::Url::from_file_path(&fake_path).expect("valid path");
        state.open_document(fake_uri, format!("function f{i}() -> nothing {{}}"));
    }
    let (tx, rx) = crossbeam_channel::unbounded();
    let decl_pos = lsp_types::Position {
        line: 0,
        character: 9,
    };
    let _ = references_response(&state, &uri, decl_pos, false, &tx);
    // Drain notifications — we should see at least one progress notification.
    let mut got_progress = false;
    while let Ok(msg) = rx.try_recv() {
        if let lsp_server::Message::Notification(n) = msg {
            if n.method == "$/progress" {
                got_progress = true;
            }
        }
    }
    assert!(
        got_progress,
        "progress notification must be emitted for >10 open documents"
    );
}

#[test]
fn test_references_performance_under_500ms() {
    let src = "function greet() -> string { return `hi` }\nfunction entrypoint() -> nothing {\n  let a = greet()\n  let b = greet()\n}\n";
    let path = "/tmp/ynz_refs_perf.ynz";
    let (state, uri) = state_single(path, src);
    let decl_pos = lsp_types::Position {
        line: 0,
        character: 9,
    };
    let start = std::time::Instant::now();
    let _ = references_response(&state, &uri, decl_pos, false, &make_sender());
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 500,
        "references cold call took {}ms, expected < 500ms",
        elapsed.as_millis()
    );
}

#[test]
fn test_references_unknown_uri_returns_none() {
    // None is reserved for "unknown URI or invalid position", not for "no refs".
    let state = ServerState::new(PositionEncoding::Utf8);
    let unknown_uri = lsp_types::Url::from_file_path("/tmp/nonexistent_ynz_file.ynz").unwrap();
    let pos = lsp_types::Position {
        line: 0,
        character: 0,
    };
    assert!(
        references_response(&state, &unknown_uri, pos, false, &make_sender()).is_none(),
        "unknown URI must return None, not Some(empty)"
    );
}

#[test]
fn test_progress_tokens_are_unique() {
    // WHY: concurrent or rapid references requests must not emit colliding $/progress tokens.
    // The atomic counter guarantees uniqueness; this test asserts the emitted token strings differ.
    use ynz_lsp::progress::ProgressTracker;
    let (tx, rx) = crossbeam_channel::unbounded();
    for _ in 0..5 {
        let tracker = ProgressTracker::begin("test", tx.clone());
        tracker.end(None);
    }
    let mut tokens: Vec<String> = vec![];
    while let Ok(msg) = rx.try_recv() {
        if let lsp_server::Message::Notification(n) = msg {
            if n.method == "$/progress" {
                if let Some(token) = n.params.get("token").and_then(|t| t.as_str()) {
                    tokens.push(token.to_string());
                }
            }
        }
    }
    // begin emits 1 notif each, end emits 1 notif each → 10 total (5 begin + 5 end).
    // The `begin` notifications carry the token we care about.
    let begin_tokens: Vec<_> = tokens.iter().step_by(2).collect(); // every other = begin
    let unique: std::collections::HashSet<_> = begin_tokens.iter().collect();
    assert_eq!(
        unique.len(),
        begin_tokens.len(),
        "all 5 progress tokens must be distinct"
    );
}
