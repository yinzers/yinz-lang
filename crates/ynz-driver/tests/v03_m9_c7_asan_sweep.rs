//! v0.3-M9 C7 AddressSanitizer sweep: build EVERY fixture at -O0 and at -O2, run each under
//! ASan, and list each one ASan reports on (a use after free, a double free, an invalid free, an
//! out-of-bounds read or write).
//!
//! The release plan's safety measure is that no fixture frees early, twice, or frees a value it
//! does not own. The per-feature ASan tests pin the cases a change targets; this sweep checks a
//! release-path change against the whole corpus. It is `#[ignore]` (it builds ~950 programs twice);
//! it is a report, not a gate.
//!
//! ```text
//! cargo test -p ynz-driver --test v03_m9_c7_asan_sweep -- --ignored --nocapture
//! ```
//!
//! The report goes to `$YNZ_ASAN_SWEEP_OUT` (default `<workspace>/target/asan_sweep.tsv`), one
//! line per fixture that is not clean: `class <TAB> level <TAB> fixture <TAB> first ASan line`.
//! Classes: `asan` (ASan reported), `no-build` (did not compile: an expected-error fixture),
//! `no-run` (the run panicked: a watchdog trip). A non-zero exit with no ASan report is a
//! program's own outcome (an expected runtime failure) and is counted, not listed.
//!
//! `$YNZ_ASAN_SWEEP_FILTER`, when set, keeps only the fixtures whose name contains it.

mod support;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

enum Verdict {
    Clean,
    ExitedNonZero,
    Asan(String),
    Unmeasured(&'static str),
}

/// Build `fixture` at one level, instrument it, run it under ASan.
fn check(fixture: &str, no_optimize: bool) -> Verdict {
    let Ok(built) = std::panic::catch_unwind(|| support::build(fixture, no_optimize)) else {
        return Verdict::Unmeasured("no-build");
    };
    let Ok(run) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        support::run_asan_binary(&support::build_asan(&built))
    })) else {
        return Verdict::Unmeasured("no-run");
    };
    if let Some(line) = run
        .stderr
        .lines()
        .find(|line| line.contains("ERROR: AddressSanitizer"))
    {
        return Verdict::Asan(line.to_string());
    }
    if run.exit_code == 0 {
        Verdict::Clean
    } else {
        Verdict::ExitedNonZero
    }
}

#[test]
#[ignore = "a report over every fixture at two levels (~1,900 builds); run with --ignored"]
fn sweep_every_fixture_under_asan_at_both_levels() {
    let filter = std::env::var("YNZ_ASAN_SWEEP_FILTER").unwrap_or_default();
    let fixtures_dir = support::fixture_path("");
    let mut names: Vec<String> = std::fs::read_dir(&fixtures_dir)
        .expect("read the fixtures directory")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(filter.as_str()))
        .collect();
    names.sort();
    let jobs: Vec<(&str, bool)> = names
        .iter()
        .flat_map(|name| [(name.as_str(), true), (name.as_str(), false)])
        .collect();

    let cursor = AtomicUsize::new(0);
    let findings: Mutex<Vec<(&'static str, &'static str, String, String)>> = Mutex::new(Vec::new());
    let clean = [AtomicUsize::new(0), AtomicUsize::new(0)];
    let nonzero = [AtomicUsize::new(0), AtomicUsize::new(0)];
    let level_name = |no_optimize: bool| if no_optimize { "O0" } else { "O2" };
    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .min(4);
    // A worker claims the next (fixture, level) as it frees up: per-fixture cost varies widely.
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let position = cursor.fetch_add(1, Ordering::Relaxed);
                let Some(&(fixture, no_optimize)) = jobs.get(position) else {
                    break;
                };
                let level_index = usize::from(!no_optimize);
                let record = |class: &'static str, detail: String| {
                    findings.lock().expect("findings mutex").push((
                        class,
                        level_name(no_optimize),
                        fixture.to_string(),
                        detail,
                    ));
                };
                match check(fixture, no_optimize) {
                    Verdict::Clean => {
                        clean[level_index].fetch_add(1, Ordering::Relaxed);
                    }
                    Verdict::ExitedNonZero => {
                        nonzero[level_index].fetch_add(1, Ordering::Relaxed);
                    }
                    Verdict::Asan(line) => record("asan", line),
                    Verdict::Unmeasured(class) => record(class, String::new()),
                }
            });
        }
    });

    let mut findings = findings.into_inner().expect("findings mutex");
    findings.sort();
    let mut report = String::new();
    for (class, level, fixture, detail) in &findings {
        report.push_str(&format!("{class}\t{level}\t{fixture}\t{detail}\n"));
    }
    let out_path = std::env::var("YNZ_ASAN_SWEEP_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/asan_sweep.tsv")
        });
    std::fs::write(&out_path, &report).expect("write the sweep report");

    for (level_index, level) in ["O0", "O2"].iter().enumerate() {
        let count_of = |class: &str| {
            findings
                .iter()
                .filter(|(found_class, found_level, _, _)| {
                    *found_class == class && found_level == level
                })
                .count()
        };
        println!(
            "{level}: {} fixtures, {} clean, {} exited non-zero without a report, {} asan, \
             {} no-build, {} no-run",
            names.len(),
            clean[level_index].load(Ordering::Relaxed),
            nonzero[level_index].load(Ordering::Relaxed),
            count_of("asan"),
            count_of("no-build"),
            count_of("no-run"),
        );
    }
    println!("report: {}", out_path.display());
}
