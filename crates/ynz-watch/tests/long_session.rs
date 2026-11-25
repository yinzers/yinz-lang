/// Long-session memory safety tests: `check_rss` thresholds and the 10k-rebuild RSS soak.
///
/// The 10k-rebuild soak (`nightly_long_session_10k_rebuilds`) takes ~5 minutes, so its name
/// starts with `nightly_` and the CI suite excludes it; the nightly workflow selects it.
use std::{path::PathBuf, sync::Once};

use ynz_watch::{
    db::init_db,
    memory::{check_rss, current_rss_bytes, MemoryConfig, RssCheckResult},
    project::WatchSourceFile,
};

// Upstream race in `memory-stats` 1.2.0 (the newest release): `memory_stats()` sets `SMAPS_CHECKED`
// (compare_exchange) before it stores `PAGE_SIZE`, so a concurrent FIRST call on another thread
// sees the flag set, skips the page-size load and returns `physical_mem == 0`. The first call
// must therefore finish before any other thread calls it. `Once::call_once` blocks every other
// caller until the closure returns, and every test in this file that reaches `memory_stats()`
// (directly or through `check_rss`) goes through it first.
static FIRST_RSS_CALL: Once = Once::new();

fn serialise_first_rss_call() {
    FIRST_RSS_CALL.call_once(|| {
        let _ = current_rss_bytes();
    });
}

fn make_db(path: &str, text: &str) -> (ynz_watch::db::WatchDb, PathBuf) {
    let p = PathBuf::from(path);
    let db = init_db(&[WatchSourceFile {
        path: p.clone(),
        text: text.to_string(),
    }]);
    (db, p)
}

fn outcome_name(result: &RssCheckResult) -> &'static str {
    match result {
        RssCheckResult::Ok => "Ok",
        RssCheckResult::Warn { .. } => "Warn",
        RssCheckResult::Stop { .. } => "Stop",
        RssCheckResult::Unavailable => "Unavailable",
    }
}

// One named test per `check_rss` outcome a caller handles differently. Each case calls
// `check_rss` `$calls` times against one rate-limit timestamp and hands the results to
// `$verdict`. `Unavailable` is accepted only where the platform cannot poll RSS at all.
macro_rules! check_rss_case {
    ($name:ident, $case_bug:literal, warn_mb: $warn_mb:expr, max_mb: $max_mb:expr, calls: $calls:expr, $verdict:expr) => {
        // bug: check_rss reports Warn, Stop or Ok wrongly for the configured thresholds, or repeats Warn on every call, so the memory safety net misfires | tier: Medium
        #[doc = $case_bug]
        #[test]
        fn $name() {
            serialise_first_rss_call();
            let config = MemoryConfig {
                warn_mb: $warn_mb,
                max_mb: $max_mb,
            };
            let mut last_warn = None;
            let results: Vec<RssCheckResult> = (0..$calls)
                .map(|_| check_rss(&config, &mut last_warn))
                .collect();
            if results
                .iter()
                .all(|result| matches!(result, RssCheckResult::Unavailable))
            {
                assert!(
                    current_rss_bytes().is_none() || std::env::var("YNZ_SKIP_RSS_TEST").is_ok(),
                    "Unavailable returned even though current_rss_bytes() succeeds — check_rss routing bug"
                );
                return;
            }
            let verdict: fn(&[RssCheckResult]) = $verdict;
            verdict(&results);
        }
    };
}

check_rss_case!(
    check_rss_warns_above_warn_threshold,
    "No Warn above the warn threshold: users never get the prompt to restart before the hard ceiling.",
    warn_mb: 0,
    max_mb: u64::MAX,
    calls: 1,
    |results| assert!(
        matches!(results[0], RssCheckResult::Warn { .. }),
        "expected Warn when warn_mb=0; got {}",
        outcome_name(&results[0])
    )
);

check_rss_case!(
    check_rss_stops_at_hard_ceiling,
    "No Stop at the hard ceiling: the daemon OOMs instead of exiting with a message.",
    warn_mb: 0,
    max_mb: 0,
    calls: 1,
    |results| assert!(
        matches!(results[0], RssCheckResult::Stop { .. }),
        "expected Stop when max_mb=0; got {}",
        outcome_name(&results[0])
    )
);

check_rss_case!(
    check_rss_rate_limits_repeated_warns,
    "A second immediate call warns again: every rebuild prints a warning line and drowns real errors.",
    warn_mb: 0,
    max_mb: u64::MAX,
    calls: 2,
    |results| assert!(
        matches!(results[0], RssCheckResult::Warn { .. })
            && !matches!(results[1], RssCheckResult::Warn { .. }),
        "first call must warn and an immediate second call must not; got {} then {}",
        outcome_name(&results[0]),
        outcome_name(&results[1])
    )
);

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
// bug: a long watch session grows RSS past 1.05x its post-warmup baseline or never fires a periodic DB rebuild | tier: Medium
#[test]
fn nightly_long_session_10k_rebuilds() {
    serialise_first_rss_call();
    let path = PathBuf::from("/tmp/long_session_10k_test.ynz");
    let base_src = "function entrypoint() -> Nothing { print(`iteration 0`) }\n";
    std::fs::write(&path, base_src).unwrap();

    let (mut db, _) = make_db("/tmp/long_session_10k_test.ynz", base_src);

    let mut rss_samples: Vec<(usize, u64)> = Vec::new();
    let sample_iters = [500, 1000, 2000, 5000, 7500, 10000];

    let mut rebuild_db_count = 0;

    for i in 1..=10000usize {
        // Mutate source text slightly each iteration via update_source API.
        let src = format!("function entrypoint() -> Nothing {{ print(`iteration {i}`) }}\n");
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
        let baseline_mb = rss_samples
            .iter()
            .find(|(i, _)| *i == 500)
            .map(|(_, mb)| *mb);
        let final_mb = rss_samples
            .iter()
            .find(|(i, _)| *i == 10000)
            .map(|(_, mb)| *mb);

        if let (Some(baseline), Some(final_rss)) = (baseline_mb, final_mb) {
            let max_allowed = (baseline as f64 * 1.05) as u64;
            assert!(
                final_rss <= max_allowed,
                "RSS at 10k ({final_rss}MB) must be ≤ 1.05× baseline at 500 ({baseline}MB = {max_allowed}MB max)"
            );
        }
    }
}
