// v0.3-M5 Phase 7 step 2 — the `array-using-soa-layout` lint firing-site suite.
//
// WHY: the lint is the ONLY teaching surface for the auto-SoA layout transform (no
// muted-hint domain exists by design — no typeable source form, plan Divergence 5),
// so it must fire on exactly the arrays the layout AUTHORITY resolved to `Soa`
// (never a re-derived twin set — authoritative-derivation.md / the E3 corpse),
// carry the registry rule name as its LSP Diagnostic.code, substitute the per-site
// teaching vars, and stay silent for every declined/gated case.
//
// Fixtures are the SAME m5_p4_soa_* files the analysis suite (soa_analysis.rs) and
// the byte-layout integration test assert on, so lint firing and analysis verdicts
// are checked against one artifact.

use std::sync::Mutex;

use ynz_diagnostics::{Diagnostic, DiagnosticKind, Severity};
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{soa_layout_lints, ARRAY_USING_SOA_LAYOUT};

/// Serializes every test in this binary: the salsa queries read the process-global
/// `YNZ_NO_AUTO_PARALLEL` env var at their entry, and cargo runs tests on parallel
/// threads — without the lock one test's `set_var` races another test's query and
/// both flake (the soa_analysis.rs ENV_LOCK precedent).
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn fixture_src(name: &str) -> String {
    let path = format!(
        "{}/../ynz-driver/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {path} must exist for the lint suite: {e}"))
}

/// Fresh db → `soa_layout_lints` rows for one fixture. Caller holds ENV_LOCK with
/// the env vars in the state it wants the queries to see.
fn lints_for(name: &str) -> Vec<Diagnostic> {
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, name.to_string(), fixture_src(name));
    db.register_source(sf);
    soa_layout_lints(&db, sf)
}

fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
    std::env::remove_var("YNZ_SOA_FORCE");
    guard
}

#[test]
fn admitted_soa_array_fires_one_lint_with_substituted_teaching_vars() {
    // WHY: the positive cell — the one admitted array must fire exactly one
    // suggestion anchored on its declaration, carrying the rule name as its
    // diagnostic code and the per-site vars (array/shape/fields/len) substituted
    // into the registry templates (Golden Rule 11: contextual, never generic).
    let _guard = env_guard();
    let lints = lints_for("m5_p4_soa_qualifying.ynz");
    assert_eq!(
        lints.len(),
        1,
        "one admitted array → one lint; got: {lints:#?}"
    );
    let lint = &lints[0];
    assert_eq!(
        lint.severity,
        Severity::Suggestion,
        "the SoA layout lint is a Tier 3 suggestion, never an error"
    );
    assert!(
        matches!(&lint.kind, Some(DiagnosticKind::LintRule { rule }) if *rule == ARRAY_USING_SOA_LAYOUT),
        "diagnostic code must be the registry rule name; got: {:?}",
        lint.kind
    );
    assert!(
        lint.what.contains("`pts`"),
        "WHAT names the array: {}",
        lint.what
    );
    assert!(
        lint.what.contains("`Point`"),
        "WHAT names the shape: {}",
        lint.what
    );
    assert!(
        lint.why.contains("`x`, `y`"),
        "WHY carries the hot-field union in declared order: {}",
        lint.why
    );
    assert!(
        lint.why.contains("72"),
        "WHY carries the provable element count: {}",
        lint.why
    );
    // The honest Phase 6 measurement pair (risk E14 / FRAGO 017): the optimized
    // figure and the shipped-today figure must BOTH be present, never conflated.
    assert!(
        lint.why.contains("3.3x") && lint.why.contains("about the same speed"),
        "WHY cites both honest measurements: {}",
        lint.why
    );
}

#[test]
fn declined_wide_field_union_array_fires_no_lint() {
    // WHY: a declined array (3-field loop union, D5) keeps the by-value AoS layout —
    // a lint claiming per-field storage there would teach a falsehood.
    let _guard = env_guard();
    let lints = lints_for("m5_p4_soa_threefield.ynz");
    assert!(
        lints.is_empty(),
        "declined arrays must not fire the layout lint; got: {lints:#?}"
    );
}

#[test]
fn cross_thread_padded_shape_array_fires_no_lint() {
    // WHY: D11 precedence — padding wins; the both-candidate shape's array resolves
    // to AoS in the authority, so the lint (which reads ONLY the authority) must
    // stay silent even though the analysis admitted the candidate.
    let _guard = env_guard();
    let lints = lints_for("m5_p4_soa_both_candidate.ynz");
    assert!(
        lints.is_empty(),
        "padded-shape arrays are never SoA'd (D11) and must not fire; got: {lints:#?}"
    );
}

#[test]
fn no_auto_parallel_env_silences_the_lint() {
    // WHY: `--no-auto-parallel` disables the SoA analysis at the query gate (D1);
    // with no admitted candidates there is no layout to teach — the lint must be
    // silent, matching codegen's sequential-oracle posture.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("YNZ_SOA_FORCE");
    std::env::set_var("YNZ_NO_AUTO_PARALLEL", "1");
    let lints = lints_for("m5_p4_soa_qualifying.ynz");
    std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
    assert!(
        lints.is_empty(),
        "--no-auto-parallel must silence the layout lint; got: {lints:#?}"
    );
}
