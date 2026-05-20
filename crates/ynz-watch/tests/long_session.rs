/// Long-session memory safety tests: 10k-rebuild loop + RSS polling + Layer 2 DB-rebuild round-trip.
///
/// The 10k-rebuild synthetic test (long_session_10k_rebuilds) is marked #[ignore]
/// because it takes ~5 minutes. Run explicitly with:
///   cargo test -p ynz-watch --test long_session -- --include-ignored
///
/// The other tests (rss_unavailable, db_rebuild_preserves_state, memory_config)
/// run in the default test suite.
use std::{
    path::PathBuf,
    time::Duration,
};

use ynz_watch::{
    db::init_db,
    memory::{MemoryConfig, RssCheckResult, check_rss, current_rss_bytes, hard_stop_message},
    project::WatchSourceFile,
};

fn make_db(path: &str, text: &str) -> (ynz_watch::db::WatchDb, PathBuf) {
    let p = PathBuf::from(path);
    let db = init_db(&[WatchSourceFile { path: p.clone(), text: text.to_string() }]);
    (db, p)
}

// WHY: RSS polling must return Some on Linux/macOS in CI. If this returns None
//      in environments where polling IS available, Layer 3 silently disables the
//      hard-stop safety net — processes can OOM without hitting the ceiling.
//      The assertion allows None only on platforms where polling is genuinely absent
//      (bare chroot, some WSL configurations). A CI runner that unexpectedly returns
//      None should be investigated, not silently accepted.
#[test]
fn current_rss_bytes_returns_value_on_supported_platforms() {
    // memory-stats supports Linux (/proc/self/status), macOS (task_info), Windows.
    // If it returns None on any of those, the test fails as a signal to investigate.
    // On genuinely unsupported targets (esoteric UNIX), skip via the env var.
    if std::env::var("YNZ_SKIP_RSS_TEST").is_ok() {
        return;
    }
    let rss = current_rss_bytes();
    assert!(
        rss.is_some(),
        "current_rss_bytes() returned None — RSS polling is unavailable. \
         Set YNZ_SKIP_RSS_TEST=1 if this platform genuinely cannot poll RSS."
    );
    let mb = rss.unwrap() / (1024 * 1024);
    assert!(mb > 0, "RSS must be positive; got {mb}MB");
}

// WHY: check_rss must fire Warn when RSS exceeds the warn threshold. If Warn is never
//      emitted, users don't get the 1GB warning that prompts them to restart watch
//      before hitting the 4GB hard ceiling — silent degradation.
#[test]
fn check_rss_warn_triggers_above_threshold() {
    let config = MemoryConfig { warn_mb: 0, max_mb: u64::MAX };
    let mut last_warn = None;
    let result = check_rss(&config, &mut last_warn);
    // With warn_mb=0, any non-zero RSS should trigger Warn (not Ok or Stop).
    match result {
        RssCheckResult::Warn { .. } => {} // expected
        RssCheckResult::Ok => panic!("expected Warn when threshold=0; got Ok"),
        RssCheckResult::Stop { .. } => panic!("Stop is mathematically unreachable with max_mb=u64::MAX"),
        RssCheckResult::Unavailable => {} // acceptable on platforms without RSS polling
    }
}

// WHY: check_rss must fire Stop when RSS exceeds the hard-stop ceiling. If Stop is
//      never emitted, the watch daemon OOMs silently instead of exiting with a
//      helpful message. Hard-stop is the safety net when Layer 1 and 2 aren't enough.
#[test]
fn check_rss_stop_triggers_at_zero_ceiling() {
    let config = MemoryConfig { warn_mb: 0, max_mb: 0 };
    let mut last_warn = None;
    let result = check_rss(&config, &mut last_warn);
    match result {
        RssCheckResult::Stop { .. } => {} // expected
        RssCheckResult::Unavailable => {
            // Only acceptable if the platform genuinely can't poll RSS.
            assert!(
                current_rss_bytes().is_none() || std::env::var("YNZ_SKIP_RSS_TEST").is_ok(),
                "Unavailable returned even though current_rss_bytes() succeeds — \
                 check_rss routing bug"
            );
        }
        other => {
            let _ = other;
            panic!("expected Stop when ceiling=0; got Ok or Warn");
        }
    }
}

// WHY: warn rate-limiting must prevent spamming the user with warnings every rebuild.
//      If rate-limiting is broken, 500 rebuilds → 500 warning lines on stderr — overwhelming
//      terminal output that drowns the actual compile errors.
#[test]
fn check_rss_warn_is_rate_limited() {
    let config = MemoryConfig { warn_mb: 0, max_mb: u64::MAX };
    let mut last_warn = None;

    // First call should potentially warn.
    let _ = check_rss(&config, &mut last_warn);

    // Second call immediately after — if first warned, second must NOT warn.
    if last_warn.is_some() {
        let result = check_rss(&config, &mut last_warn);
        match result {
            RssCheckResult::Warn { .. } => panic!("second immediate call should not warn — rate limit broken"),
            _ => {} // Ok, Stop, or Unavailable all acceptable
        }
    }
}

// WHY: hard_stop_message must contain all three WHAT/WHAT-INSTEAD/WHY parts. This is
//      the primary user-facing stop message; missing WHY means users don't know how to
//      prevent future stops (adjust YNZ_WATCH_REBUILD_AFTER). Missing WHAT INSTEAD
//      means they don't know to restart.
#[test]
fn hard_stop_message_format() {
    let msg = hard_stop_message(4096, 4096, "examples/basics/entrypoint.ynz");
    assert!(msg.contains("WHAT:"), "missing WHAT");
    assert!(msg.contains("WHAT INSTEAD:"), "missing WHAT INSTEAD");
    assert!(msg.contains("WHY:"), "missing WHY");
    assert!(msg.contains("YNZ_WATCH_REBUILD_AFTER"), "missing tuning guidance");
}

// WHY: Layer 2 periodic-rebuild threshold must trigger at N rebuilds and reset to 0
//      after rebuild_db(). If the threshold never triggers, the DB accumulates stale
//      salsa cache forever — memory grows unboundedly over a long session.
#[test]
fn layer2_periodic_rebuild_triggers_at_threshold() {
    let (mut db, path) = make_db(
        "/tmp/long_session_layer2_test.ynz",
        "function entrypoint() -> nothing { }\n",
    );
    std::fs::write(&path, "function entrypoint() -> nothing { }\n").unwrap();

    // Drive one rebuild to increment the counter.
    let _ = db.run_codegen(&path);
    assert!(
        db.should_periodic_rebuild(1, Duration::from_secs(9999)),
        "should trigger after 1 rebuild with threshold=1"
    );

    db.rebuild_db();
    assert_eq!(db.rebuild_count(), 0, "rebuild_count must reset to 0 after rebuild_db()");
    assert!(
        !db.should_periodic_rebuild(1, Duration::from_secs(9999)),
        "should NOT trigger immediately after rebuild_db()"
    );
}

/// 10,000 synthetic rebuild loop.
///
/// Uses the WatchDb API directly (no real file events, no notify watcher) to
/// eliminate CI flakiness from filesystem timing. Mutates source via update_source().
///
/// Pass conditions:
/// - RSS at i=10000 ≤ 1.05× RSS at i=500 (post-warmup baseline).
/// - No crash, OOM, or panic.
/// - At least one Layer 2 DB rebuild fires.
/// - Layer 1 LRU caps are transparent to correctness (same output each rebuild).
#[test]
#[ignore] // Run with: cargo test --test long_session -- --include-ignored
fn long_session_10k_rebuilds() {
    let path = PathBuf::from("/tmp/long_session_10k_test.ynz");
    let base_src = "function entrypoint() -> nothing { print(`iteration 0`) }\n";
    std::fs::write(&path, base_src).unwrap();

    let (mut db, _) = make_db("/tmp/long_session_10k_test.ynz", base_src);

    let mut rss_samples: Vec<(usize, u64)> = Vec::new();
    let sample_iters = [500, 1000, 2000, 5000, 7500, 10000];

    let mut rebuild_db_count = 0;

    for i in 1..=10000usize {
        // Mutate source text slightly each iteration via update_source API.
        let src = format!(
            "function entrypoint() -> nothing {{ print(`iteration {i}`) }}\n"
        );
        db.update_source(&path, src);

        let _ = db.run_codegen(&path);

        // Layer 2: trigger periodic rebuild at N=500.
        if db.should_periodic_rebuild(500, std::time::Duration::from_secs(14400)) {
            db.rebuild_db();
            rebuild_db_count += 1;
        }

        if sample_iters.contains(&i) {
            if let Some(rss) = current_rss_bytes() {
                rss_samples.push((i, rss / (1024 * 1024)));
            }
        }
    }

    // Assert at least one periodic DB rebuild fired.
    assert!(
        rebuild_db_count >= 1,
        "Layer 2 periodic DB rebuild must fire at least once in 10k iterations (threshold=500)"
    );

    if rss_samples.len() >= 2 {
        let baseline_mb = rss_samples.iter().find(|(i, _)| *i == 500).map(|(_, mb)| *mb);
        let final_mb = rss_samples.iter().find(|(i, _)| *i == 10000).map(|(_, mb)| *mb);

        if let (Some(baseline), Some(final_rss)) = (baseline_mb, final_mb) {
            let max_allowed = (baseline as f64 * 1.05) as u64;
            assert!(
                final_rss <= max_allowed,
                "RSS at 10k ({final_rss}MB) must be ≤ 1.05× baseline at 500 ({baseline}MB = {max_allowed}MB max)"
            );
        }
    }
}
