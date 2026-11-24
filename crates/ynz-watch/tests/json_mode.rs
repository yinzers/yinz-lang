/// Integration tests for `--json` structured event mode.
///
/// Tests use JsonEmitter + JsonEvent directly (without spawning the watch binary)
/// to verify schema correctness, ordering invariants, and timestamp format.
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ynz_watch::{
    db::init_db,
    json_emitter::JsonEmitter,
    json_events::{ts, ShutdownReason, WatchEvent, SCHEMA_VERSION},
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
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
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
    let src = "function entrypoint() -> Nothing { print(`hello`) }\n";
    let p = PathBuf::from(path);
    // Write to disk so rebuild_one's fs::read_to_string succeeds.
    std::fs::write(&p, src).expect("could not write test fixture");
    let db = init_db(&[WatchSourceFile {
        path: p.clone(),
        text: src.to_string(),
    }]);
    (db, p)
}

// One named test per event type the emitter serialises. Each emits one event through a real
// JsonEmitter and checks the single NDJSON line it wrote.
macro_rules! emitted_event_case {
    ($name:ident, $build_event:expr, type: $expected_type:literal $(, $extra_key:literal: $extra_value:literal)?) => {
        // bug: a --json event is missing schema_version, uses a type name other than its kebab-case schema name, or does not occupy exactly one line, so consumers drop or misparse it | tier: Medium
        #[test]
        fn $name() {
            let shared = Arc::new(Mutex::new(Vec::<u8>::new()));
            let mut emitter = JsonEmitter::new_writer(CaptureBuf(Arc::clone(&shared)));

            let (timestamp, schema_version) = ts();
            let build_event: fn(String, String) -> WatchEvent = $build_event;
            emitter
                .emit(&build_event(timestamp, schema_version))
                .unwrap();

            let bytes = shared.lock().unwrap().clone();
            let events = collect_events(&bytes);
            assert_eq!(events.len(), 1, "one emit must write exactly one line");
            let event = &events[0];
            assert_eq!(
                event.get("schema_version").and_then(|v| v.as_str()),
                Some(SCHEMA_VERSION),
                "event must have schema_version = SCHEMA_VERSION; got: {event}"
            );
            assert_eq!(
                event.get("type").and_then(|v| v.as_str()),
                Some($expected_type),
                "event type must serialize as the kebab-case schema name"
            );
            $(
                assert_eq!(
                    event.get($extra_key).and_then(|v| v.as_str()),
                    Some($extra_value),
                    "{} must serialize kebab-case",
                    $extra_key
                );
            )?
        }
    };
}

emitted_event_case!(
    watch_ready_event_has_schema_version_and_kebab_type,
    |timestamp, schema_version| WatchEvent::WatchReady {
        timestamp,
        schema_version,
        watching: vec!["foo.ynz".into()],
    },
    type: "watch-ready"
);

emitted_event_case!(
    build_start_event_has_schema_version_and_kebab_type,
    |timestamp, schema_version| WatchEvent::BuildStart {
        timestamp,
        schema_version,
        file: "foo.ynz".into(),
    },
    type: "build-start"
);

emitted_event_case!(
    child_spawn_event_has_schema_version_and_kebab_type,
    |timestamp, schema_version| WatchEvent::ChildSpawn {
        timestamp,
        schema_version,
        pid: 42,
    },
    type: "child-spawn"
);

emitted_event_case!(
    watch_shutdown_event_serializes_ctrl_c_reason,
    |timestamp, schema_version| WatchEvent::WatchShutdown {
        timestamp,
        schema_version,
        reason: ShutdownReason::CtrlC,
    },
    type: "watch-shutdown",
    "reason": "ctrl-c"
);

/// Run one headless check-only rebuild of `source_on_disk` through `rebuild_one_with_emitter`
/// and return every JSON event it emitted.
fn rebuild_events(path: &str, source_on_disk: &str, out_dir_name: &str) -> Vec<serde_json::Value> {
    let shared = Arc::new(Mutex::new(Vec::<u8>::new()));
    let mut emitter_box = JsonEmitter::new_writer(CaptureBuf(Arc::clone(&shared)));

    // The db starts valid; rebuild_one_with_emitter reads the (possibly broken) text from disk.
    let (mut db, path) = make_valid_db(path);
    std::fs::write(&path, source_on_disk).expect("could not write test fixture");

    let out_dir = std::env::temp_dir().join(out_dir_name);
    let _ = std::fs::create_dir_all(&out_dir);

    let mut current_child = None;
    let _ = rebuild_one_with_emitter(
        &mut db,
        &path,
        &path,
        &out_dir,
        true, // check_only — no binary spawn
        &mut current_child,
        Some(&mut emitter_box),
        true, // force: skip no-change guard, test always runs one build
        true, // no_clear: tests run headless
    );

    let _ = std::fs::remove_dir_all(&out_dir);
    let bytes = shared.lock().unwrap().clone();
    collect_events(&bytes)
}

// bug: a successful --json rebuild emits build-end before build-start, or a build-end without outcome "ok" and duration_ms, so CI tooling mis-times or never runs the program | tier: Medium
#[test]
fn successful_rebuild_emits_build_start_then_build_end_ok() {
    let events = rebuild_events(
        "/tmp/json_mode_ok_test.ynz",
        "function entrypoint() -> Nothing { print(`hello`) }\n",
        "ynz-watch-json-test",
    );

    let types: Vec<&str> = events
        .iter()
        .filter_map(|event| event.get("type").and_then(|v| v.as_str()))
        .collect();
    let start_index = types.iter().position(|&kind| kind == "build-start");
    let end_index = types.iter().position(|&kind| kind == "build-end");
    assert!(
        start_index.is_some() && end_index.is_some() && start_index < end_index,
        "build-start must be emitted before build-end; got order: {types:?}"
    );

    let build_end = &events[end_index.unwrap()];
    assert_eq!(
        build_end.get("outcome").and_then(|v| v.as_str()),
        Some("ok"),
        "build-end outcome must be 'ok' for successful compilation"
    );
    assert!(
        build_end.get("duration_ms").is_some(),
        "build-end must have duration_ms field"
    );
}

// bug: a failing --json rebuild swallows its diagnostics or emits them without the what/what_instead/why/severity/span fields, so a CI dashboard sees an error with no context | tier: Medium
#[test]
fn failing_rebuild_emits_structured_diagnostic_events() {
    let events = rebuild_events(
        "/tmp/json_mode_diagnostic_test.ynz",
        "function entrypoint() -> Nothing {\n    unknownIdent\n}\n",
        "ynz-watch-json-diag-test",
    );

    let first = events
        .iter()
        .find(|event| event.get("type").and_then(|v| v.as_str()) == Some("diagnostic"))
        .expect("at least one diagnostic event must be emitted for broken code");

    assert!(
        first
            .get("what")
            .and_then(|v| v.as_str())
            .map(|text| !text.is_empty())
            .unwrap_or(false),
        "diagnostic must have non-empty 'what' field"
    );
    let missing_fields: Vec<&str> = ["what_instead", "why", "severity", "span"]
        .into_iter()
        .filter(|field| first.get(*field).is_none())
        .collect();
    assert!(
        missing_fields.is_empty(),
        "diagnostic is missing fields {missing_fields:?}"
    );
}
