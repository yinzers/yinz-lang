/// Integration tests for incremental rebuild via salsa: shadow-state propagation + DB rebuild + counter reset.
///
/// Verifies that `WatchDb::rebuild_db()` preserves source state across the DB drop+recreate
/// and resets the periodic-rebuild counter.
use std::path::{Path, PathBuf};

use ynz_watch::{
    db::{init_db, WatchDb},
    project::WatchSourceFile,
};

/// Create a minimal in-memory `WatchDb` from a slice of (path, text) pairs.
fn make_db(sources: &[(&str, &str)]) -> WatchDb {
    let entries: Vec<WatchSourceFile> = sources
        .iter()
        .map(|(path, text)| WatchSourceFile {
            path: PathBuf::from(*path),
            text: text.to_string(),
        })
        .collect();
    init_db(&entries)
}

// bug: rebuild_db() drops or reverts source text (a registered file, or an update_source edit made before the rebuild), so watch reports 0 errors on broken code | tier: Medium
#[test]
fn rebuild_db_round_trips_source_text() {
    let mut db = make_db(&[
        (
            "/tmp/a.ynz",
            "function entrypoint() -> Nothing { print(42) }\n",
        ),
        ("/tmp/b.ynz", "// module b\n"),
        ("/tmp/x.ynz", "// v0\n"),
    ]);
    db.update_source(Path::new("/tmp/x.ynz"), "// v1\n".to_string());

    db.rebuild_db();

    let snap = db.source_snapshot();
    assert_eq!(
        snap.get("/tmp/a.ynz").map(String::as_str),
        Some("function entrypoint() -> Nothing { print(42) }\n"),
        "source text must round-trip through rebuild_db()"
    );
    assert_eq!(
        snap.get("/tmp/b.ynz").map(String::as_str),
        Some("// module b\n"),
        "all registered sources must survive rebuild_db()"
    );
    assert_eq!(
        snap.get("/tmp/x.ynz").map(String::as_str),
        Some("// v1\n"),
        "an update_source edit must survive rebuild_db()"
    );
}

// bug: the periodic-rebuild trigger fires below its threshold, or rebuild_count stays non-zero after rebuild_db() so the trigger never fires again at N rebuilds | tier: Medium
#[test]
fn rebuild_count_resets_after_periodic_rebuild() {
    use std::time::Duration;
    let mut db = make_db(&[("/tmp/a.ynz", "// a\n")]);

    // Simulate the state after N rebuilds: drive run_codegen on valid source.
    let entry = std::path::Path::new("/tmp/a.ynz");
    db.update_source(entry, "function entrypoint() -> Nothing { }\n".to_string());
    let _outcome = db.run_codegen(entry);

    // Counter should now be 1; threshold of 1 triggers rebuild, a threshold of 2 does not.
    assert!(
        db.should_periodic_rebuild(1, Duration::from_secs(9999)),
        "should_periodic_rebuild must return true when rebuild_count >= threshold"
    );
    assert!(
        !db.should_periodic_rebuild(2, Duration::from_secs(9999)),
        "should_periodic_rebuild must return false below the threshold"
    );

    db.rebuild_db();
    assert_eq!(
        db.rebuild_count(),
        0,
        "rebuild_count resets to 0 after rebuild_db()"
    );
    assert!(
        !db.should_periodic_rebuild(1, Duration::from_secs(9999)),
        "should_periodic_rebuild must return false immediately after rebuild_db()"
    );
}
