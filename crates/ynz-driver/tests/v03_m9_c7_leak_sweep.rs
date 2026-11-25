//! v0.3-M9 C7 leak sweep: build EVERY fixture, run it with the alloc and string counters, and
//! classify each leaking one as plain-path or state-machine-only.
//!
//! The release plan's measure is `frees == allocations`. This sweep is the list of where that
//! still fails, so a producer fix can be checked against the whole corpus and the next agent
//! can see what is left. It runs in the nightly lane (`nightly_`): it
//! builds ~950 programs (about 3 to 5 minutes on 4 cores), too long for a pre-push run.
//!
//! It is a ratchet, one list per class. C7: a `plain`-class fixture leaks only if it is listed in
//! `KNOWN_PLAIN_LEAKS` with its producer card. C8 (card #385): a `state-machine`-class fixture
//! leaks only if it is listed in `KNOWN_STATE_MACHINE_LEAKS` with its producer card. For both
//! lists, every listed fixture must still leak (a fixed leak must leave its list) and no unlisted
//! fixture of that class may leak (a new leak fails).
//!
//! Re-run (writes the report, prints a summary):
//!
//! ```text
//! cargo nextest run -p ynz-driver --test v03_m9_c7_leak_sweep --ignore-default-filter --no-capture
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
//!   functions release their frames (C8), so none may leak. A state-machine fixture can
//!   ALSO hold a plain-path leak; the report cannot tell, so `plain` is a lower bound; a
//!   `state-machine` leak is reproduced alone to find which kind it is.
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

/// Producer cards of the known leaks (see the ratchet in the module doc).
const CARD_702: &str = "card #702: a number-to-string result is never released";
const CARD_703: &str = "card #703: the key and value of a map for-loop are never released";
const CARD_497: &str = "card #497: a maybe-array returned from a function is never released";

/// The `plain`-class fixtures that leak today, each with the producer card that is not fixed. The
/// entry leaves the list in the change that fixes it.
const KNOWN_PLAIN_LEAKS: [(&str, &str); 7] = [
    ("bignum_number_conversions.ynz", CARD_702),
    ("card399_map_for_key_value.ynz", CARD_703),
    ("number_to_string_owned.ynz", CARD_702),
    ("v0_3_m7_p3_dangling_stack_return_siblings.ynz", CARD_702),
    ("v0_3_m7_p3_maybe_number_payload.ynz", CARD_702),
    ("v0_3_m9_c1m_maybe_array_returned.ynz", CARD_497),
    ("v0_3_m9_c3a_to_string_distinct_results.ynz", CARD_702),
];

/// The `state-machine`-class fixtures that leak today, each with the producer card that is not
/// fixed. The entry leaves the list in the change that fixes it.
const KNOWN_STATE_MACHINE_LEAKS: [(&str, &str); 31] = [
    (
        "v0_3_hardening_c1_number_local_blocking_channel_send.ynz",
        CARD_702,
    ),
    (
        "v0_3_m3a_p1_ec_crossing_local_propagated_number.ynz",
        CARD_702,
    ),
    (
        "v0_3_m3a_p1_number_errors_returning_suspending_fn.ynz",
        CARD_702,
    ),
    (
        "v0_3_m3a_p1_number_errors_suspending_error_path.ynz",
        CARD_702,
    ),
    ("v0_3_m3a_p3_audit_array_number_no_leak.ynz", CARD_702),
    ("v0_3_m3a_p3_audit_number_crossing.ynz", CARD_702),
    ("v0_3_m3a_p3_for_number_loop_var.ynz", CARD_702),
    ("v0_3_m3b_p5_parallel_number_crosses_wait.ynz", CARD_702),
    (
        "v0_3_m3b_p5_parallel_number_ec_inline_collect.ynz",
        CARD_702,
    ),
    ("v0_3_m3e_number_crossing_local_direct", CARD_702),
    ("v0_3_m3e_reexport_ec_number", CARD_702),
    ("v0_3_m3f_ec_failed_then_ok.ynz", CARD_702),
    ("v0_3_m3f_ec_same_callee_aliasing.ynz", CARD_702),
    ("v0_3_m3f_ec_three_bindings.ynz", CARD_702),
    ("v0_3_m3f_parallel_group_decimal128_sibling.ynz", CARD_702),
    ("v0_3_m3f_parallel_group_ec_number_and_bool.ynz", CARD_702),
    ("v0_3_m4_handle_collect_number.ynz", CARD_702),
    ("v0_3_m4_r5xr8_suspend_then_collect.ynz", CARD_702),
    (
        "v0_3_m6_non_escaping_args_false_positive_sweep.ynz",
        CARD_702,
    ),
    ("v0_3_m6_number_arg_background_cpu.ynz", CARD_702),
    ("v0_3_m6_number_arg_background_sm.ynz", CARD_702),
    ("v0_3_m6_number_arg_cpu_member.ynz", CARD_702),
    ("v0_3_m6_number_arg_fused_cpu_member.ynz", CARD_702),
    ("v0_3_m6_number_arg_fused_group.ynz", CARD_702),
    ("v0_3_m6_number_arg_parallel_group.ynz", CARD_702),
    ("v0_3_m6_number_arg_pure_call.ynz", CARD_702),
    ("v0_3_m7_p3_dangling_stack_return_sm_tier.ynz", CARD_702),
    ("v0_3_m7_p6_backedge_value_integrity.ynz", CARD_702),
    (
        "v0_3_m9_c1h_number_literal_arg_suspending_call.ynz",
        CARD_702,
    ),
    ("v0_3_m9_slot_p26_sibling_int_then_number.ynz", CARD_702),
    ("v0_3_m9_slot_q05_errors_value_shadow.ynz", CARD_702),
];

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

// bug: a fixture leaks (frees != allocations) that is not in its class's known list, or a listed leak is fixed but stays listed | tier: High
#[test]
fn nightly_sweep_every_fixture_for_leaks_and_classify_them() {
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

    if filter.is_empty() {
        // Bug caught (High: leak): a producer that no fixture of the corpus exercises the counters
        // of, or one that is fixed while its entry stays listed.
        assert_leaks_match_known_list("plain", &leaks, &KNOWN_PLAIN_LEAKS);
        // Bug caught (High: leak, card #385 exit test): a state-machine function or a dropped task
        // whose frame release misses a heap value or a string, or a fixed one still listed.
        assert_leaks_match_known_list("state-machine", &leaks, &KNOWN_STATE_MACHINE_LEAKS);
    }
}

/// The ratchet for one class: the fixtures of `class` that leak must be exactly the ones in
/// `known`. A fixture leaking but unlisted is a new leak to fix; one listed but balanced is fixed,
/// so its entry must leave the list.
/// O(n log n) time, O(n) memory: sorts the names of the leaking fixtures and of the list.
fn assert_leaks_match_known_list(class: &str, leaks: &[Outcome], known: &[(&str, &str)]) {
    let mut leaking: Vec<&str> = leaks
        .iter()
        .filter(|outcome| outcome.class == class)
        .map(|outcome| outcome.fixture.as_str())
        .collect();
    leaking.sort_unstable();
    let known_names: std::collections::HashSet<&str> =
        known.iter().map(|(name, _)| *name).collect();
    let leaking_names: std::collections::HashSet<&str> = leaking.iter().copied().collect();

    let new_leaks: Vec<String> = leaks
        .iter()
        .filter(|outcome| outcome.class == class && !known_names.contains(outcome.fixture.as_str()))
        .map(|outcome| {
            format!(
                "{} (heap {}/{}, strings {}/{})",
                outcome.fixture,
                outcome.heap.0,
                outcome.heap.1,
                outcome.strings.0,
                outcome.strings.1
            )
        })
        .collect();
    assert!(
        new_leaks.is_empty(),
        "{class} fixtures leak that are not in the known list; fix the leak (do not add it): \
         {new_leaks:#?}"
    );

    let mut fixed: Vec<String> = known
        .iter()
        .filter(|(name, _)| !leaking_names.contains(name))
        .map(|(name, producer)| format!("{name} ({producer})"))
        .collect();
    fixed.sort_unstable();
    assert!(
        fixed.is_empty(),
        "these {class} fixtures no longer leak; remove them from the known list: {fixed:#?}"
    );
}
