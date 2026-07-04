// v0.3-M5 Phase 7 step 4 — consumer-seam proof for the `array-using-soa-layout` lint.
//
// WHY: the typeck suite (ynz-typeck/tests/soa_layout_lints.rs) proves the pure builder
// in isolation; THIS test proves the lint actually reaches a real consumer's output —
// `codegen_query`'s diagnostic bucket, the one both driver stderr paths (`ynz build` /
// `ynz run`) render. Without it, the three merge seams compile but are never
// assertion-tested end-to-end, and a dropped merge line would ship silently.

use std::path::PathBuf;

use ynz_codegen::codegen_query;
use ynz_diagnostics::{DiagnosticKind, Severity};
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::ARRAY_USING_SOA_LAYOUT;

/// Same fixture the analysis + lint suites assert on, so verdict, lint text, and
/// consumer delivery are all checked against ONE artifact.
const QUALIFYING_FIXTURE: &str = "m5_p4_soa_qualifying.ynz";

fn fixture_src() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ynz-driver/tests/fixtures")
        .join(QUALIFYING_FIXTURE);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

#[test]
fn soa_layout_lint_reaches_codegen_query_diagnostic_bucket() {
    // This binary holds exactly one test, so there is no intra-binary env race
    // (the ENV_LOCK discipline in soa_layout_lints.rs exists for multi-test
    // binaries); still scrub the gate vars so an ambient shell setting cannot
    // flip the verdict.
    std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
    std::env::remove_var("YNZ_SOA_FORCE");

    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, QUALIFYING_FIXTURE.to_string(), fixture_src());
    db.register_source(sf);

    let output = codegen_query(&db, sf);

    assert!(
        !output.diagnostics.has_errors(),
        "the qualifying fixture must compile clean; got: {:#?}",
        output.diagnostics
    );

    let soa_lints: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(&d.kind, Some(DiagnosticKind::LintRule { rule }) if *rule == ARRAY_USING_SOA_LAYOUT)
        })
        .collect();
    assert_eq!(
        soa_lints.len(),
        1,
        "exactly one SoA layout lint must ride codegen_query's bucket; got: {soa_lints:#?}"
    );
    assert_eq!(
        soa_lints[0].severity,
        Severity::Suggestion,
        "the lint stays a Tier 3 suggestion through the consumer seam"
    );
    // Template substitution is the typeck suite's job; here assert only the one
    // consumer-visible anchor (the array name) survived the trip.
    assert!(
        soa_lints[0].what.contains("`pts`"),
        "consumer-delivered WHAT names the array: {}",
        soa_lints[0].what
    );
}
