// v0.3-M5 Phase 4 step 5: the SoA analysis exit-criterion suite. Asserts the exact verdict and
// reason payload for every `m5_p4_soa_*` fixture at the analysis level (`soa_candidate_query`),
// the layout authority (`layout_decisions_query`: padding wins over SoA) and the entry gates
// (`YNZ_NO_AUTO_PARALLEL`, `YNZ_SOA_FORCE`, kernel mode through the pure `soa::analyze` core).
// The fixtures live in crates/ynz-driver/tests/fixtures/, the same files the driver's byte-layout
// integration test runs, so analysis verdicts and runtime behaviour are asserted against one
// artifact.
//
// Tests are split by tier and by entry point: safety verdicts and the layout authority are High
// (a wrong admit lets a segmented layout split storage the program reads whole, or sizes its
// buffer from a stale length); cost-only declines and the gates are Medium (extra work, output
// unchanged). Each table is a named case per fixture; a case lists the exact candidate rows.

use std::sync::Mutex;

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::soa;
use ynz_typeck::{
    check_query, layout_decisions_query, module_signatures_query, soa_candidate_query,
    FieldSegment, LayoutDecisions, LayoutKind, SoaCandidate, SoaDeclineReason, SoaVerdict,
};

/// Serializes every test in this binary on the process-global environment variables
/// `YNZ_NO_AUTO_PARALLEL` and `YNZ_SOA_FORCE`: the salsa queries read them at their entry, and
/// cargo runs the tests of this file on parallel threads, so without the lock one test's
/// `set_var` races another test's query. Every test holds the lock and clears both variables
/// before querying.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    let guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
    std::env::remove_var("YNZ_SOA_FORCE");
    guard
}

fn fixture_src(name: &str) -> String {
    let path = format!(
        "{}/../ynz-driver/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("fixture {path} must exist for the analysis suite: {err}"))
}

/// One expected candidate row: array binding, element shape, verdict.
type ExpectedRow = (&'static str, &'static str, SoaVerdict);

fn row(array_name: &'static str, shape_name: &'static str, verdict: SoaVerdict) -> ExpectedRow {
    (array_name, shape_name, verdict)
}

fn admitted(provable_len: i64, hot_fields: &[&str]) -> SoaVerdict {
    SoaVerdict::Admitted {
        provable_len,
        hot_fields: hot_fields.iter().map(|field| field.to_string()).collect(),
    }
}

fn declined_escaping(how: &str) -> SoaVerdict {
    SoaVerdict::Declined(SoaDeclineReason::Escapes {
        how: how.to_string(),
    })
}

fn assert_rows(found: Vec<SoaCandidate>, expected: Vec<ExpectedRow>) {
    let found: Vec<(String, String, SoaVerdict)> = found
        .into_iter()
        .map(|candidate| {
            (
                candidate.array_name,
                candidate.shape_name,
                candidate.verdict,
            )
        })
        .collect();
    let expected: Vec<(String, String, SoaVerdict)> = expected
        .into_iter()
        .map(|(array_name, shape_name, verdict)| {
            (array_name.to_string(), shape_name.to_string(), verdict)
        })
        .collect();
    assert_eq!(found, expected);
}

/// `soa_candidate_query` rows for one fixture in a fresh database, under the given environment.
fn candidates_under_env(name: &str, environment: &[(&str, &str)]) -> Vec<SoaCandidate> {
    let _guard = env_guard();
    for (variable, value) in environment {
        std::env::set_var(variable, value);
    }
    let mut db = CompilerDb::default();
    let source_file = SourceFile::new(&db, name.to_string(), fixture_src(name));
    db.register_source(source_file);
    let rows = (*soa_candidate_query(&db, source_file)).clone();
    for (variable, _) in environment {
        std::env::remove_var(variable);
    }
    rows
}

fn assert_candidates(name: &str, expected: Vec<ExpectedRow>) {
    assert_rows(candidates_under_env(name, &[]), expected);
}

/// Pure-core `soa::analyze` rows for one fixture with explicit flags (the checker itself never
/// reads the SoA environment variables).
fn pure_core_for(
    name: &str,
    kernel_mode: bool,
    no_auto_parallel: bool,
    force: Option<soa::SoaForce>,
) -> Vec<SoaCandidate> {
    let _guard = env_guard();
    let mut db = CompilerDb::default();
    let source_file = SourceFile::new(&db, name.to_string(), fixture_src(name));
    db.register_source(source_file);
    let check_output = check_query(&db, source_file);
    let signatures = module_signatures_query(&db, source_file);
    soa::analyze(
        &check_output.typed_module,
        &signatures.shape_table,
        kernel_mode,
        no_auto_parallel,
        force,
    )
}

fn layout_for(name: &str) -> LayoutDecisions {
    let _guard = env_guard();
    let mut db = CompilerDb::default();
    let source_file = SourceFile::new(&db, name.to_string(), fixture_src(name));
    db.register_source(source_file);
    (*layout_decisions_query(&db, source_file)).clone()
}

fn assert_layout(
    name: &str,
    padded_shapes: &[&str],
    array_name: &str,
    shape_name: &str,
    expected_kind: LayoutKind,
) {
    let layout = layout_for(name);
    let mut found_padded: Vec<&String> = layout.padded_shapes.iter().collect();
    found_padded.sort();
    assert_eq!(found_padded, padded_shapes, "padded shapes differ");
    assert_eq!(
        layout.arrays.len(),
        1,
        "one decision row; got: {:?}",
        layout.arrays
    );
    let decision = &layout.arrays[0];
    assert_eq!(decision.array_name, array_name);
    assert_eq!(decision.shape_name, shape_name);
    assert_eq!(decision.kind, expected_kind);
}

macro_rules! safety_candidate_cases {
    ($($case:ident: $fixture:expr => $rows:expr;)+) => {
        $(
            // bug: the SoA analysis admits an array it must decline (a param or argument escape, growth, a lend-self shape, an unprovable or stale length after a reassign or re-let, a handed-over loop row) or reports a wrong provable length, so the segmented layout reads or sizes memory wrongly | tier: High
            #[test]
            fn $case() {
                assert_candidates($fixture, $rows);
            }
        )+
    };
}

macro_rules! cost_candidate_cases {
    ($($case:ident: $fixture:expr => $rows:expr;)+) => {
        $(
            // bug: the SoA analysis keeps AoS for a qualifying array or segments one that costs more than it saves (below the size threshold, more than two hot fields, no per-field loop access) | tier: Medium
            #[test]
            fn $case() {
                assert_candidates($fixture, $rows);
            }
        )+
    };
}

macro_rules! gate_cases {
    ($($case:ident: $fixture:expr, [$kernel:expr, $no_auto_parallel:expr, $force:expr] => $rows:expr;)+) => {
        $(
            // bug: the SoA analysis ignores kernel mode, the no-auto-parallel gate or `SoaForce::Aos`, or lets a force-soa override outrank a hard gate (a layout the gate forbids, output unchanged) | tier: Medium
            #[test]
            fn $case() {
                let found = pure_core_for($fixture, $kernel, $no_auto_parallel, $force);
                assert_rows(found, $rows);
            }
        )+
    };
}

macro_rules! environment_cases {
    ($($case:ident: $fixture:expr, $environment:expr => $rows:expr;)+) => {
        $(
            // bug: `soa_candidate_query` misreads `YNZ_NO_AUTO_PARALLEL` or `YNZ_SOA_FORCE` (an unrecognised value not ignored, force outranking the no-auto-parallel gate), so the layout differs from the one requested (output unchanged) | tier: Medium
            #[test]
            fn $case() {
                assert_rows(candidates_under_env($fixture, $environment), $rows);
            }
        )+
    };
}

safety_candidate_cases! {
    a_qualifying_array_is_admitted_with_its_exact_length_and_hot_fields:
        "m5_p4_soa_qualifying.ynz"
        => vec![row("pts", "Point", admitted(72, &["x", "y"]))];
    a_parameter_and_a_call_argument_decline_as_escapes:
        "m5_p4_soa_escaping.ynz"
        => vec![
            row("pts", "Point", declined_escaping("function parameter — cross-function by definition (D4)")),
            row("pts", "Point", declined_escaping("passed to `sumX()` (D4)")),
        ];
    a_grown_array_declines_as_grown:
        "m5_p4_soa_growth.ynz"
        => vec![row("pts", "Point", SoaVerdict::Declined(SoaDeclineReason::Grown { op: ".add()".to_string() }))];
    a_lend_self_shape_declines_at_shape_level:
        "m5_p4_soa_lendself.ynz"
        => vec![row("crew", "Pirate", SoaVerdict::Declined(SoaDeclineReason::LendSelfMethod { function: "recordHit".to_string() }))];
    a_call_initialised_array_has_no_provable_length_and_a_returned_array_escapes:
        "m5_p4_soa_runtime_length.ynz"
        => vec![
            row("out", "Point", declined_escaping("returned from the function")),
            row("pts", "Point", SoaVerdict::Declined(SoaDeclineReason::LengthNotProvable)),
        ];
    a_reassigned_array_declines_and_the_source_of_the_assignment_is_an_alias:
        "m5_p4_soa_reassigned.ynz"
        => vec![
            row("pts", "Point", declined_escaping("reassigned after initial binding")),
            row("replacement", "Point", declined_escaping("stored into another binding (aliases the array)")),
        ];
    a_re_let_shadowed_array_declines_as_rebound:
        "m5_p4_soa_let_shadow.ynz"
        => vec![row("pts", "Point", declined_escaping("rebound by a later let"))];
    a_list_whose_loop_hands_a_row_over_declines_as_an_escape:
        "v0_3_m9_c1q_handover_soa_list.ynz"
        => vec![
            row("pts", "P", declined_escaping("a loop row is handed over (C1q)")),
            row("kept", "P", SoaVerdict::Declined(SoaDeclineReason::Grown { op: ".add()".to_string() })),
        ];
    a_candidate_that_will_be_padded_is_still_admitted_by_the_walk:
        "m5_p4_soa_both_candidate.ynz"
        => vec![row("season", "Tally", admitted(66, &["hits", "outs"]))];
}

cost_candidate_cases! {
    an_array_below_the_size_threshold_declines_with_its_length:
        "m5_p4_soa_small_n.ynz"
        => vec![row("pts", "Point", SoaVerdict::Declined(SoaDeclineReason::BelowSizeThreshold { len: 4 }))];
    a_three_field_hot_union_declines_with_the_fields_in_declared_order:
        "m5_p4_soa_threefield.ynz"
        => vec![row("rs", "Reading", SoaVerdict::Declined(SoaDeclineReason::FieldUnionTooWide {
            fields: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        }))];
    a_count_only_loop_declines_for_lack_of_per_field_access:
        "m5_p4_soa_no_field_access.ynz"
        => vec![row("pts", "Point", SoaVerdict::Declined(SoaDeclineReason::NoPerFieldLoopAccess))];
}

macro_rules! layout_authority_cases {
    ($($case:ident: $fixture:expr, $padded:expr, ($array:expr, $shape:expr) => $kind:expr;)+) => {
        $(
            // bug: the layout authority resolves an admitted array to the wrong layout (a padded cross-thread shape split into segments so false sharing returns, an un-padded one left whole, or segments missing or out of declared order) | tier: High
            #[test]
            fn $case() {
                assert_layout($fixture, $padded, $array, $shape, $kind);
            }
        )+
    };
}

layout_authority_cases! {
    an_unpadded_admitted_array_resolves_to_soa_with_segments_in_declared_order:
        "m5_p4_soa_qualifying.ynz", &[], ("pts", "Point")
        => LayoutKind::Soa {
            segments: vec![
                FieldSegment { field: "x".to_string(), segment_index: 0 },
                FieldSegment { field: "y".to_string(), segment_index: 1 },
            ],
        };
    padding_wins_over_soa_for_a_cross_thread_shape:
        "m5_p4_soa_both_candidate.ynz", &["Tally"], ("season", "Tally")
        => LayoutKind::Aos { declined: SoaDeclineReason::CrossThreadPadded };
}

// bug: a force-soa override admits an array the analysis declined for safety (a grown array), so a layout codegen cannot prove is compiled | tier: High
#[test]
fn force_soa_leaves_the_growth_decline_live() {
    let rows = pure_core_for(
        "m5_p4_soa_growth.ynz",
        false,
        false,
        Some(soa::SoaForce::Soa),
    );
    assert_rows(
        rows,
        vec![row(
            "pts",
            "Point",
            SoaVerdict::Declined(SoaDeclineReason::Grown {
                op: ".add()".to_string(),
            }),
        )],
    );
}

gate_cases! {
    with_no_gate_the_qualifying_fixture_is_admitted:
        "m5_p4_soa_qualifying.ynz", [false, false, None]
        => vec![row("pts", "Point", admitted(72, &["x", "y"]))];
    kernel_mode_empties_the_candidate_list:
        "m5_p4_soa_qualifying.ynz", [true, false, None]
        => vec![];
    force_aos_empties_the_candidate_list:
        "m5_p4_soa_qualifying.ynz", [false, false, Some(soa::SoaForce::Aos)]
        => vec![];
    force_soa_admits_the_below_threshold_array_with_its_hot_fields:
        "m5_p4_soa_small_n.ynz", [false, false, Some(soa::SoaForce::Soa)]
        => vec![row("pts", "Point", admitted(4, &["x", "y"]))];
    no_auto_parallel_outranks_force_soa:
        "m5_p4_soa_qualifying.ynz", [false, true, Some(soa::SoaForce::Soa)]
        => vec![];
    kernel_mode_outranks_force_soa:
        "m5_p4_soa_qualifying.ynz", [true, false, Some(soa::SoaForce::Soa)]
        => vec![];
}

environment_cases! {
    the_no_auto_parallel_variable_empties_the_candidate_list:
        "m5_p4_soa_qualifying.ynz", &[("YNZ_NO_AUTO_PARALLEL", "1")]
        => vec![];
    the_force_soa_variable_admits_the_below_threshold_array:
        "m5_p4_soa_small_n.ynz", &[("YNZ_SOA_FORCE", "soa")]
        => vec![row("pts", "Point", admitted(4, &["x", "y"]))];
    the_force_aos_variable_empties_the_candidate_list:
        "m5_p4_soa_qualifying.ynz", &[("YNZ_SOA_FORCE", "aos")]
        => vec![];
    an_unrecognised_force_value_is_ignored:
        "m5_p4_soa_small_n.ynz", &[("YNZ_SOA_FORCE", "SOA")]
        => vec![row("pts", "Point", SoaVerdict::Declined(SoaDeclineReason::BelowSizeThreshold { len: 4 }))];
    the_no_auto_parallel_variable_outranks_the_force_soa_variable:
        "m5_p4_soa_qualifying.ynz", &[("YNZ_NO_AUTO_PARALLEL", "1"), ("YNZ_SOA_FORCE", "soa")]
        => vec![];
}
