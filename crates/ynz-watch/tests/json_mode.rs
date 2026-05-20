/// Integration tests for `--json` structured event mode.
///
/// Tests use JsonEmitter + JsonEvent directly (without spawning the watch binary)
/// to verify schema correctness, ordering invariants, and timestamp format.
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ynz_watch::{
    db::init_db,
    json_emitter::JsonEmitter,
    json_events::{ShutdownReason, WatchEvent, SCHEMA_VERSION, ts},
    project::WatchSourceFile,
    rebuild::rebuild_one_with_emitter,
};

/// A thread-safe shared buffer for capturing JsonEmitter output in tests.
struct CaptureBuf(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for CaptureBuf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}
// Arc<Mutex<Vec<u8>>> is Send automatically; no unsafe needed here.

/// Collect all emitted JSON lines as parsed serde_json::Values.
fn collect_events(bytes: &[u8]) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).expect("invalid JSON in emitter output"))
        .collect()
}

/// Make a minimal compilable WatchDb and write the file to disk (rebuild_one reads from disk).
fn make_valid_db(path: &str) -> (ynz_watch::db::WatchDb, PathBuf) {
    let src = "function entrypoint() -> nothing { print(`hello`) }\n";
    let p = PathBuf::from(path);
    // Write to disk so rebuild_one's fs::read_to_string succeeds.
    std::fs::write(&p, src).expect("could not write test fixture");
    let db = init_db(&[WatchSourceFile {
        path: p.clone(),
        text: src.to_string(),
    }]);
    (db, p)
}

// WHY: every --json event must include `schema_version` matching SCHEMA_VERSION constant.
//      If schema_version is missing or wrong, consumers can't detect schema changes between
//      intermediate milestones and silently parse fields that no longer exist.
#[test]
fn every_emitted_event_has_schema_version() {
    let shared = Arc::new(Mutex::new(Vec::<u8>::new()));
    let mut emitter = JsonEmitter::new_writer(CaptureBuf(Arc::clone(&shared)));

    let (timestamp, schema_version) = ts();
    emitter.emit(&WatchEvent::WatchReady {
        timestamp,
        schema_version,
        watching: vec!["foo.ynz".into()],
    }).unwrap();

    let (timestamp, schema_version) = ts();
    emitter.emit(&WatchEvent::BuildStart {
        timestamp,
        schema_version,
        file: "foo.ynz".into(),
    }).unwrap();

    let bytes = shared.lock().unwrap().clone();
    let events = collect_events(&bytes);
    for ev in &events {
        assert_eq!(
            ev.get("schema_version").and_then(|v| v.as_str()),
            Some(SCHEMA_VERSION),
            "event must have schema_version = SCHEMA_VERSION; got: {ev}"
        );
    }
}

// WHY: the `type` field must use kebab-case JSON names per design/watch.md locked schema.
//      If serde renames to something else (snake_case, PascalCase), downstream consumers
//      that pattern-match on `"build-start"` etc. silently drop all events.
#[test]
fn event_type_field_uses_kebab_case() {
    let shared = Arc::new(Mutex::new(Vec::<u8>::new()));
    let mut emitter = JsonEmitter::new_writer(CaptureBuf(Arc::clone(&shared)));

    let (timestamp, schema_version) = ts();
    emitter.emit(&WatchEvent::BuildStart {
        timestamp: timestamp.clone(),
        schema_version: schema_version.clone(),
        file: "foo.ynz".into(),
    }).unwrap();

    emitter.emit(&WatchEvent::ChildSpawn {
        timestamp,
        schema_version,
        pid: 42,
    }).unwrap();

    let bytes = shared.lock().unwrap().clone();
    let events = collect_events(&bytes);
    assert_eq!(
        events[0].get("type").and_then(|v| v.as_str()),
        Some("build-start"),
        "BuildStart must serialize type as 'build-start'"
    );
    assert_eq!(
        events[1].get("type").and_then(|v| v.as_str()),
        Some("child-spawn"),
        "ChildSpawn must serialize type as 'child-spawn'"
    );
}

// WHY: build-end must include `outcome: "ok"` when compilation succeeds. Consumers that
//      check `"build-end".outcome == "ok"` before deciding to run the program would
//      silently never run it if the outcome field is wrong or missing.
#[test]
fn build_end_outcome_ok_on_success() {
    let shared = Arc::new(Mutex::new(Vec::<u8>::new()));
    let buf_clone = Arc::clone(&shared);
    let mut emitter_box = JsonEmitter::new_writer(CaptureBuf(buf_clone));

    let (mut db, path) = make_valid_db("/tmp/json_mode_ok_test.ynz");
    let out_dir = std::env::temp_dir().join("ynz-watch-json-test");
    let _ = std::fs::create_dir_all(&out_dir);

    let mut current_child = None;
    let _ = rebuild_one_with_emitter(
        &mut db, &path, &path, &out_dir, true, // check_only — no binary spawn
        &mut current_child,
        Some(&mut emitter_box),
        true, // force: skip no-change guard, test always runs one build
    );

    let bytes = shared.lock().unwrap().clone();
    let events = collect_events(&bytes);

    // Expect build-start + build-end.
    let build_end = events.iter().find(|ev| {
        ev.get("type").and_then(|v| v.as_str()) == Some("build-end")
    }).expect("no build-end event emitted");

    assert_eq!(
        build_end.get("outcome").and_then(|v| v.as_str()),
        Some("ok"),
        "build-end outcome must be 'ok' for successful compilation"
    );
    assert!(
        build_end.get("duration_ms").is_some(),
        "build-end must have duration_ms field"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

// WHY: event ordering invariant — build-start must always precede build-end for the same
//      file. If build-end arrives before build-start, consumers building a timeline or
//      measuring build duration get incorrect data. This invariant is load-bearing for
//      CI tooling that reads --json output.
#[test]
fn build_start_precedes_build_end() {
    let shared = Arc::new(Mutex::new(Vec::<u8>::new()));
    let buf_clone = Arc::clone(&shared);
    let mut emitter_box = JsonEmitter::new_writer(CaptureBuf(buf_clone));

    let (mut db, path) = make_valid_db("/tmp/json_mode_order_test.ynz");
    let out_dir = std::env::temp_dir().join("ynz-watch-json-order-test");
    let _ = std::fs::create_dir_all(&out_dir);

    let mut current_child = None;
    let _ = rebuild_one_with_emitter(
        &mut db, &path, &path, &out_dir, true,
        &mut current_child,
        Some(&mut emitter_box),
        true, // force: skip no-change guard
    );

    let bytes = shared.lock().unwrap().clone();
    let events = collect_events(&bytes);
    let types: Vec<&str> = events
        .iter()
        .filter_map(|ev| ev.get("type").and_then(|v| v.as_str()))
        .collect();

    let start_idx = types.iter().position(|&t| t == "build-start");
    let end_idx = types.iter().position(|&t| t == "build-end");

    assert!(start_idx.is_some(), "build-start must be emitted");
    assert!(end_idx.is_some(), "build-end must be emitted");
    assert!(
        start_idx.unwrap() < end_idx.unwrap(),
        "build-start must precede build-end; got order: {:?}", types
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

// WHY: diagnostic events must be emitted when compilation fails in --json mode.
//      If diagnostics are swallowed, a CI dashboard sees `build-end { outcome: "errors" }`
//      with zero context — operators can't distinguish "unknown identifier" from "syntax error"
//      and have to re-run in text mode to see what went wrong. Each diagnostic must carry
//      the three-part WHAT/WHAT-INSTEAD/WHY shape per Golden Rule 11.
#[test]
fn diagnostic_events_emitted_on_compile_error() {
    let shared = Arc::new(Mutex::new(Vec::<u8>::new()));
    let buf_clone = Arc::clone(&shared);
    let mut emitter_box = JsonEmitter::new_writer(CaptureBuf(buf_clone));

    // Start with valid source, then switch to broken — rebuild_one_with_emitter reads from disk.
    let path = PathBuf::from("/tmp/json_mode_diagnostic_test.ynz");
    let broken_src = "function entrypoint() -> nothing {\n    unknownIdent\n}\n";
    let (mut db, _) = make_valid_db("/tmp/json_mode_diagnostic_test.ynz");
    // Write broken source to disk so rebuild_one_with_emitter's fs::read_to_string sees it.
    std::fs::write(&path, broken_src).expect("could not write broken test fixture");

    let out_dir = std::env::temp_dir().join("ynz-watch-json-diag-test");
    let _ = std::fs::create_dir_all(&out_dir);

    let mut current_child = None;
    let _ = rebuild_one_with_emitter(
        &mut db, &path, &path, &out_dir, true,
        &mut current_child,
        Some(&mut emitter_box),
        true, // force: skip no-change guard
    );

    let bytes = shared.lock().unwrap().clone();
    let events = collect_events(&bytes);

    let diag_events: Vec<&serde_json::Value> = events.iter().filter(|ev| {
        ev.get("type").and_then(|v| v.as_str()) == Some("diagnostic")
    }).collect();

    assert!(!diag_events.is_empty(), "at least one diagnostic event must be emitted for broken code");

    let first = diag_events[0];
    assert!(first.get("what").and_then(|v| v.as_str()).map(|s| !s.is_empty()).unwrap_or(false),
        "diagnostic must have non-empty 'what' field");
    assert!(first.get("what_instead").is_some(), "diagnostic must have 'what_instead' field");
    assert!(first.get("why").is_some(), "diagnostic must have 'why' field");
    assert!(first.get("severity").is_some(), "diagnostic must have 'severity' field");
    assert!(first.get("span").is_some(), "diagnostic must have 'span' field");

    let _ = std::fs::remove_dir_all(&out_dir);
}

// WHY: WatchShutdown with reason ctrl-c must be the last event consumers receive.
//      If watch-shutdown is missing or not last, consumers can't reliably detect clean
//      watch exit vs crash. The "watch-shutdown is always last" invariant is in the schema.
#[test]
fn watch_shutdown_serializes_correctly() {
    let shared = Arc::new(Mutex::new(Vec::<u8>::new()));
    let mut emitter = JsonEmitter::new_writer(CaptureBuf(Arc::clone(&shared)));

    let (timestamp, schema_version) = ts();
    emitter.emit(&WatchEvent::WatchShutdown {
        timestamp,
        schema_version,
        reason: ShutdownReason::CtrlC,
    }).unwrap();

    let bytes = shared.lock().unwrap().clone();
    let events = collect_events(&bytes);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].get("type").and_then(|v| v.as_str()),
        Some("watch-shutdown")
    );
    assert_eq!(
        events[0].get("reason").and_then(|v| v.as_str()),
        Some("ctrl-c"),
        "ShutdownReason::CtrlC must serialize as 'ctrl-c' (kebab-case)"
    );
}
