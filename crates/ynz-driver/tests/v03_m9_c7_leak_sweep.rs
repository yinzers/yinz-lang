//! v0.3-M9 C7 leak sweep: build EVERY fixture, run it with the alloc and string counters, and
//! classify each leaking one as plain-path or state-machine-only.
//!
//! The release plan's measure is `frees == allocations`. This sweep is the list of where that
//! still fails, so a producer fix can be checked against the whole corpus and the next agent
//! can see what is left. It is `#[ignore]` because it builds ~950 programs (about 3 to 5
//! minutes on 4 cores): a dedicated-lane test, not a pre-push one.
//!
//! It asserts the one thing C7 owns: a `plain`-class fixture leaks only if it is listed in
//! `KNOWN_PLAIN_LEAKS` with its reason, and every listed one still leaks (a fixed leak must leave
//! the list). State-machine leaks are C8's input and are reported, not asserted.
//!
//! Re-run (writes the report, prints a summary):
//!
//! ```text
//! cargo test -p ynz-driver --test v03_m9_c7_leak_sweep -- --ignored --nocapture
//! ```
//!
//! The report goes to `$YNZ_LEAK_SWEEP_OUT` (default `<workspace>/target/leak_sweep.tsv`), one
//! line per leaking fixture:
//! `class <TAB> fixture <TAB> concurrency-keywords <TAB> alloc/free <TAB> strings alloc/free`
//!
//! Classes:
//! - `plain`: the emitted IR has no `ynz_sm_*_resume` function, so only the plain-path release
//!   (C7) can have produced the leak.
//! - `state-machine`: the IR has at least one `ynz_sm_*_resume` function. State-machine
//!   functions release nothing yet (C8), so these are C8's inputs. A state-machine fixture can
//!   ALSO hold a plain-path leak; the report cannot tell, so `plain` is a lower bound and the
//!   `state-machine` list is where to look for a plain leak that needs reproducing alone.
//! - `no-build` / `no-run`: the fixture did not compile (an expected-error fixture) or the
//!   program exited without writing its counters; these are listed apart, never counted as
//!   balanced.
//!
//! `$YNZ_LEAK_SWEEP_FILTER`, when set, keeps only the fixtures whose name contains it.

mod support;

use std::path::PathBuf;

/// The words that make a fixture "concurrency" for the sweep's grouping: a fixture without any is
/// a plain program, whatever else it does.
const CONCURRENCY_KEYWORDS: [&str; 5] = ["background", "wait", "channel", "errors", "fails"];

/// The `plain`-class fixtures that leak today, each with the producer that is not fixed. Found by
/// this sweep in C7 test review r2; the entry leaves the list in the change that fixes it.
const KNOWN_PLAIN_LEAKS: [(&str, &str); 1] = [(
    "m5_p3_sweep_mapentry_bg_escape.ynz",
    "a `MapEntry` loop variable passed as a `background` argument: 8 allocations, 7 frees",
)];

struct Outcome {
    class: &'static str,
    fixture: String,
    keywords: String,
    heap: (u64, u64),
    strings: (u64, u64),
}

/// All the text of a fixture (a project directory's files joined), for the keyword scan.
fn fixture_source(fixture: &str) -> String {
    let path = support::fixture_path(fixture);
    if !path.is_dir() {
        return std::fs::read_to_string(&path).unwrap_or_default();
    }
    let mut text = String::new();
    let mut pending = vec![path];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                pending.push(entry_path);
            } else if let Ok(contents) = std::fs::read_to_string(&entry_path) {
                text.push_str(&contents);
            }
        }
    }
    text
}

fn keywords_in(source: &str) -> String {
    let words: std::collections::HashSet<&str> = source
        .split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .collect();
    let found: Vec<&str> = CONCURRENCY_KEYWORDS
        .iter()
        .copied()
        .filter(|keyword| words.contains(keyword))
        .collect();
    if found.is_empty() {
        "-".to_string()
    } else {
        found.join(",")
    }
}

/// Build and run one fixture (optimized). `Err(class)` when it cannot be measured.
fn measure(fixture: &str) -> Result<Outcome, &'static str> {
    let built =
        std::panic::catch_unwind(|| support::build(fixture, false)).map_err(|_| "no-build")?;
    let ir = std::fs::read_to_string(&built.ir).unwrap_or_default();
    let has_state_machine = ir.lines().any(|line| {
        line.starts_with("define ") && line.contains("@ynz_sm_") && line.contains("_resume(")
    });
    let (_, counters) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        support::run_counted(&built)
    }))
    .map_err(|_| "no-run")?;
    Ok(Outcome {
        class: if has_state_machine {
            "state-machine"
        } else {
            "plain"
        },
        fixture: fixture.to_string(),
        keywords: keywords_in(&fixture_source(fixture)),
        heap: (counters.alloc, counters.free),
        strings: (counters.string_alloc, counters.string_free),
    })
}

#[test]
#[ignore = "builds and runs ~950 programs (about 5 minutes on 4 cores): a dedicated-lane test; run with --ignored"]
fn sweep_every_fixture_for_leaks_and_classify_them() {
    let filter = std::env::var("YNZ_LEAK_SWEEP_FILTER").unwrap_or_default();
    let names = support::fixture_names(&filter);

    let mut leaks: Vec<Outcome> = Vec::new();
    let mut unmeasured: Vec<(String, &'static str)> = Vec::new();
    let mut balanced = 0usize;
    for (fixture, measured) in support::sweep_in_parallel(&names, measure) {
        match measured {
            Ok(outcome)
                if outcome.heap.0 != outcome.heap.1 || outcome.strings.0 != outcome.strings.1 =>
            {
                leaks.push(outcome);
            }
            Ok(_) => balanced += 1,
            Err(class) => unmeasured.push((fixture.to_string(), class)),
        }
    }
    leaks.sort_by(|left, right| (left.class, &left.fixture).cmp(&(right.class, &right.fixture)));

    let mut report = String::new();
    for outcome in &leaks {
        report.push_str(&format!(
            "{}\t{}\t{}\t{}/{}\t{}/{}\n",
            outcome.class,
            outcome.fixture,
            outcome.keywords,
            outcome.heap.0,
            outcome.heap.1,
            outcome.strings.0,
            outcome.strings.1
        ));
    }
    for (fixture, class) in &unmeasured {
        report.push_str(&format!("{class}\t{fixture}\n"));
    }
    let out_path = std::env::var("YNZ_LEAK_SWEEP_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/leak_sweep.tsv")
        });
    std::fs::write(&out_path, &report).expect("write the sweep report");

    let count_of = |class: &str, plain_source: bool| {
        leaks
            .iter()
            .filter(|outcome| outcome.class == class && (outcome.keywords == "-") == plain_source)
            .count()
    };
    println!(
        "swept {} fixtures: {} balanced, {} leaking, {} unmeasured (report: {})",
        names.len(),
        balanced,
        leaks.len(),
        unmeasured.len(),
        out_path.display()
    );
    println!(
        "leaking, no concurrency keyword: plain {}, state-machine {}",
        count_of("plain", true),
        count_of("state-machine", true)
    );
    println!(
        "leaking, with a concurrency keyword: plain {}, state-machine {}",
        count_of("plain", false),
        count_of("state-machine", false)
    );

    // Bug caught (High: leak): a producer on the plain path that no fixture of the corpus
    // exercises the counters of, or one that is fixed while its entry stays listed.
    let mut plain_leaks: Vec<&str> = leaks
        .iter()
        .filter(|outcome| outcome.class == "plain")
        .map(|outcome| outcome.fixture.as_str())
        .collect();
    plain_leaks.sort_unstable();
    let mut listed: Vec<&str> = KNOWN_PLAIN_LEAKS.iter().map(|(name, _)| *name).collect();
    listed.sort_unstable();
    if filter.is_empty() {
        assert_eq!(
            plain_leaks, listed,
            "the plain-path leaks of the corpus differ from KNOWN_PLAIN_LEAKS: a fixture on the \
             left only is a new plain-path leak to fix; one on the right only is fixed, so remove \
             it from the list"
        );
    }
}
