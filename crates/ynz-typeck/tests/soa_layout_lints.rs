// v0.3-M5 Phase 7 step 2 — the `array-using-soa-layout` lint against the layout authority.
//
// WHY: the lint must fire on exactly the arrays the layout AUTHORITY resolved to `Soa`
// (never a re-derived twin set — authoritative-derivation.md / the E3 corpse). This
// agreement test is a protected keep (rule 3 of that doc: two derivations tied together).
//
// The fixture is the SAME m5_p4_soa_* file the analysis suite (soa_analysis.rs) and
// the byte-layout integration test assert on.

use ynz_diagnostics::Diagnostic;
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::soa_layout_lints;

fn fixture_src(name: &str) -> String {
    let path = format!(
        "{}/../ynz-driver/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {path} must exist for the lint suite: {e}"))
}

/// Fresh db → `soa_layout_lints` rows for one fixture.
fn lints_for(name: &str) -> Vec<Diagnostic> {
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, name.to_string(), fixture_src(name));
    db.register_source(sf);
    soa_layout_lints(&db, sf)
}

// bug: the SoA layout lint fires on an array the layout authority resolved to AoS (padded shape, D11) because it re-derives instead of reading the authority | tier: Low
#[test]
fn cross_thread_padded_shape_array_fires_no_lint() {
    // D11 precedence — padding wins; the both-candidate shape's array resolves
    // to AoS in the authority, so the lint (which reads ONLY the authority) must
    // stay silent even though the analysis admitted the candidate.
    //
    // The salsa queries read the process-global `YNZ_NO_AUTO_PARALLEL` / `YNZ_SOA_FORCE` env vars;
    // this is the only test in its binary, so clearing them here cannot race another test.
    std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
    std::env::remove_var("YNZ_SOA_FORCE");
    let lints = lints_for("m5_p4_soa_both_candidate.ynz");
    assert!(
        lints.is_empty(),
        "padded-shape arrays are never SoA'd (D11) and must not fire; got: {lints:#?}"
    );
}
