//! v0.3-M9 C7 AddressSanitizer sweep: build EVERY fixture at -O0 and at -O2, run each under
//! ASan, and assert that the fixtures ASan reports on (a use after free, a double free, an
//! invalid free, an out-of-bounds read or write) are exactly `KNOWN_ASAN_REPORTS`.
//!
//! The release plan's safety measure is that no fixture frees early, twice, or frees a value it
//! does not own. The per-feature ASan tests pin the cases a change targets; this sweep checks a
//! release-path change against the whole corpus. It runs in the nightly lane (`nightly_`):
//! it builds ~950 programs twice (about 30 minutes on 4 cores), too long for a pre-push run.
//!
//! ```text
//! cargo nextest run -p ynz-driver --test v03_m9_c7_asan_sweep --ignore-default-filter --no-capture
//! ```
//!
//! The report goes to `$YNZ_ASAN_SWEEP_OUT` (default `<workspace>/target/asan_sweep.tsv`), one
//! line per fixture that is not clean: `class <TAB> level <TAB> fixture <TAB> first ASan line`.
//! Classes: `asan` (ASan reported), `no-build` (did not compile: an expected-error fixture),
//! `no-run` (the run panicked: a watchdog trip). A non-zero exit with no ASan report is a
//! program's own outcome (an expected runtime failure) and is counted, not listed.
//!
//! `$YNZ_ASAN_SWEEP_FILTER`, when set, keeps only the fixtures whose name contains it, and the
//! assertion is skipped (a filtered run cannot see the whole list).

mod support;

use std::path::PathBuf;

/// The fixtures ASan reports on today, each with why that is intended. An entry leaves the list
/// only when the fixture stops being reported.
const KNOWN_ASAN_REPORTS: [(&str, &str); 1] = [(
    "v0_3_m6_signal_terminated_stack_overflow.ynz",
    "overflows the stack on purpose, to test the signal-terminated exit",
)];

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

// bug: a release-path change frees early, twice or a value it does not own in a fixture no per-feature test targets (ASan use after free, double free, invalid free) | tier: High
#[test]
fn nightly_sweep_every_fixture_under_asan_at_both_levels() {
    let filter = std::env::var("YNZ_ASAN_SWEEP_FILTER").unwrap_or_default();
    let names = support::fixture_names(&filter);
    // One job per (fixture, level), so a slow fixture's two builds run on different workers.
    let jobs: Vec<String> = names
        .iter()
        .flat_map(|name| [format!("O0 {name}"), format!("O2 {name}")])
        .collect();
    let verdicts = support::sweep_in_parallel(&jobs, |job| {
        let (level, fixture) = job.split_once(' ').expect("a job is `level fixture`");
        check(fixture, level == "O0")
    });

    // (class, level, fixture, detail), in job order, which is fixture order.
    let mut findings: Vec<(&str, &str, &str, String)> = Vec::new();
    let mut clean = [0usize; 2];
    let mut nonzero = [0usize; 2];
    for (job, verdict) in verdicts {
        let (level, fixture) = job.split_once(' ').expect("a job is `level fixture`");
        let level_index = usize::from(level == "O2");
        match verdict {
            Verdict::Clean => clean[level_index] += 1,
            Verdict::ExitedNonZero => nonzero[level_index] += 1,
            Verdict::Asan(line) => findings.push(("asan", level, fixture, line)),
            Verdict::Unmeasured(class) => findings.push((class, level, fixture, String::new())),
        }
    }
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
            clean[level_index],
            nonzero[level_index],
            count_of("asan"),
            count_of("no-build"),
            count_of("no-run"),
        );
    }
    println!("report: {}", out_path.display());

    // A listed fixture whose report is gone while its entry stays also fails here.
    let mut reported: Vec<&str> = findings
        .iter()
        .filter(|(class, _, _, _)| *class == "asan")
        .map(|(_, _, fixture, _)| *fixture)
        .collect();
    reported.sort_unstable();
    reported.dedup();
    let mut listed: Vec<&str> = KNOWN_ASAN_REPORTS.iter().map(|(name, _)| *name).collect();
    listed.sort_unstable();
    if filter.is_empty() {
        assert_eq!(
            reported, listed,
            "the fixtures ASan reports on differ from KNOWN_ASAN_REPORTS: a fixture on the left \
             only is a new report to fix (see the report file); one on the right only is no \
             longer reported, so remove it from the list"
        );
    }
}
