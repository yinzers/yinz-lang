// The `--no-auto-parallel` gate on the false-sharing analysis (v0.3-M4 Phase 4).
//
// WHY: this flag has never gated a LAYOUT transform before — the plan requires proof,
// not assumption, that under the sequential-oracle posture the authoritative analysis
// yields NO cross-thread shapes (so codegen emits the genuinely unpadded layout and
// the decline lint stays silent). This file holds exactly ONE test and nothing else:
// it mutates the process-global `YNZ_NO_AUTO_PARALLEL` env var, and being the sole
// test in its own integration-test binary is what makes that mutation race-free
// (cargo runs each tests/*.rs file as a separate process).
//
// The end-to-end half of the proof (byte-identical program output + padded-vs-unpadded
// IR in the two modes, through the real compiler binary) lives in
// crates/ynz-driver/tests/false_sharing_gating.rs.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::check_query;

const SRC: &str = "shape Tally { hits: int\n outs: int }\n\
     function record(give t: Tally) -> nothing { wait sleep(1)\n print(t.hits.toString()) }\n\
     function entrypoint() -> nothing { let t: Tally = { hits: 3, outs: 1 }\n background record(t) }";

#[test]
fn no_auto_parallel_empties_the_padded_set_and_silences_the_lint() {
    // Sanity half: without the flag the shape pads (same program as the positive
    // integration test — a regression here means the gate test would vacuously pass).
    std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, "gate_on.ynz".to_string(), SRC.to_string());
    let on = (*check_query(&db, sf)).clone();
    assert!(
        on.typed_module.cross_thread_padded_shapes.contains("Tally"),
        "sanity: default mode must pad the crossing shape; got: {:?}",
        on.typed_module.cross_thread_padded_shapes
    );

    // Gated half: the SAME program under the flag yields the empty padded set (the
    // sequential lowering reads this set, so its layout is genuinely unpadded) and
    // fires no cross-thread lint (padding is deliberately off — nagging would be a
    // false positive against the user's own flag).
    std::env::set_var("YNZ_NO_AUTO_PARALLEL", "1");
    let db2 = CompilerDb::default();
    let sf2 = SourceFile::new(&db2, "gate_off.ynz".to_string(), SRC.to_string());
    let off = (*check_query(&db2, sf2)).clone();
    std::env::remove_var("YNZ_NO_AUTO_PARALLEL");

    assert!(
        off.typed_module.cross_thread_padded_shapes.is_empty(),
        "--no-auto-parallel must yield an EMPTY padded set; got: {:?}",
        off.typed_module.cross_thread_padded_shapes
    );
    let lint_fired = off.diagnostics.iter().any(|d| {
        matches!(&d.kind, Some(ynz_diagnostics::DiagnosticKind::LintRule { rule })
            if *rule == "cross-thread-fields-not-padded")
    });
    assert!(
        !lint_fired,
        "--no-auto-parallel must silence the cross-thread-fields-not-padded lint"
    );
}
