/// Integration tests for Phase 2 incremental rebuild via salsa.
///
/// Verifies that the salsa cache is hot on the second rebuild (edit with no AST change)
/// and that `WatchDb::rebuild_db()` preserves source state across the DB drop+recreate.
use std::path::{Path, PathBuf};

use ynz_watch::{
    db::{WatchDb, init_db},
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


// WHY: shadow source state must survive a salsa DB drop+recreate (Layer 2 periodic rebuild).
//      If this breaks, rebuild_db() silently empties the DB and watch reports 0 errors on
//      broken code — silent-wrong-output. Do NOT loosen: fix the shadow/repopulate logic.
#[test]
fn rebuild_db_round_trips_source_text() {
    let mut db = make_db(&[
        ("/tmp/a.ynz", "function entrypoint() -> nothing { print(42) }\n"),
        ("/tmp/b.ynz", "// module b\n"),
    ]);

    db.rebuild_db();

    let snap = db.source_snapshot();
    assert_eq!(
        snap.get("/tmp/a.ynz").map(String::as_str),
        Some("function entrypoint() -> nothing { print(42) }\n"),
        "source text must round-trip through rebuild_db()"
    );
    assert_eq!(
        snap.get("/tmp/b.ynz").map(String::as_str),
        Some("// module b\n"),
        "all registered sources must survive rebuild_db()"
    );
}

// WHY: update_source must propagate the new text to the snapshot (shadow map) so that
//      (a) the next rebuild uses the updated text, and (b) rebuild_db() sees the new content.
//      Catches bugs where salsa gets the update but the shadow doesn't, causing incorrect
//      content after a periodic DB rebuild.
#[test]
fn update_source_reflects_in_snapshot() {
    let mut db = make_db(&[("/tmp/x.ynz", "// v0\n")]);

    db.update_source(Path::new("/tmp/x.ynz"), "// v1\n".to_string());

    let snap = db.source_snapshot();
    assert_eq!(
        snap.get("/tmp/x.ynz").map(String::as_str),
        Some("// v1\n"),
        "update_source must propagate to the snapshot"
    );
}

// WHY: rebuild_count must reset to 0 after rebuild_db() so the Layer 2 periodic-rebuild
//      trigger fires again after N rebuilds, not N×2 or never again.
#[test]
fn rebuild_count_resets_after_periodic_rebuild() {
    use std::time::Duration;
    let mut db = make_db(&[("/tmp/a.ynz", "// a\n")]);

    // Simulate the state after N rebuilds: drive run_codegen on valid source.
    let entry = std::path::Path::new("/tmp/a.ynz");
    db.update_source(entry, "function entrypoint() -> nothing { }\n".to_string());
    let _outcome = db.run_codegen(entry);

    // Counter should now be 1; threshold of 1 triggers rebuild.
    assert!(
        db.should_periodic_rebuild(1, Duration::from_secs(9999)),
        "should_periodic_rebuild must return true when rebuild_count >= threshold"
    );

    db.rebuild_db();
    assert_eq!(db.rebuild_count(), 0, "rebuild_count resets to 0 after rebuild_db()");
    assert!(
        !db.should_periodic_rebuild(1, Duration::from_secs(9999)),
        "should_periodic_rebuild must return false immediately after rebuild_db()"
    );
}
