/// Integration test: introduce and clear a compile error mid-watch.
///
/// Uses `WatchDb` directly (not the full watch binary) to verify:
/// 1. A broken source produces a `RebuildOutcome::Errors` with diagnostics.
/// 2. A repaired source produces `RebuildOutcome::Success` on the next rebuild.
///
/// This is the core incremental-rebuild correctness test — it confirms that salsa
/// invalidates correctly when the user fixes a bug and saves again.
use std::path::{Path, PathBuf};

use ynz_watch::{
    db::{init_db, RebuildOutcome, WatchDb},
    project::WatchSourceFile,
};

fn make_db_with_program(path: &str, text: &str) -> WatchDb {
    init_db(&[WatchSourceFile {
        path: PathBuf::from(path),
        text: text.to_string(),
    }])
}

// The same WatchDb session must go valid→broken→valid across saves without a stale cache
// producing the wrong outcome. Do NOT relax the fix=success assertion.
// bug: after a user fixes a compile error, watch still reports the old error (or misses a new one) because the incremental cache is stale | tier: Medium
#[test]
fn error_then_fix_recovers_cleanly() {
    let path = "/tmp/test_watch_errors.ynz";
    let valid_src = "function entrypoint() -> Nothing {\n    print(`hello`)\n}\n";
    let broken_src = "function entrypoint() -> Nothing {\n    unknownIdent\n}\n";

    let mut db = make_db_with_program(path, valid_src);
    let entry = Path::new(path);

    // Initial build on valid source — should succeed.
    let first = db.run_codegen(entry);
    assert!(
        first.is_success(),
        "initial build on valid source should succeed; diagnostics:\n{}",
        first.render_diagnostics()
    );

    // Introduce an error.
    db.update_source(entry, broken_src.to_string());
    let broken = db.run_codegen(entry);
    assert!(
        matches!(broken, RebuildOutcome::Errors { .. }),
        "broken source should produce RebuildOutcome::Errors; got: {}",
        broken.render_diagnostics()
    );
    assert!(
        broken.error_count() > 0,
        "broken source should have at least 1 error"
    );

    // Fix the error.
    db.update_source(entry, valid_src.to_string());
    let fixed = db.run_codegen(entry);
    assert!(
        fixed.is_success(),
        "fixed source should succeed; diagnostics:\n{}",
        fixed.render_diagnostics()
    );
}
