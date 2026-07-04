// v0.3-M5 Phase 4 step 5 — the SoA analysis exit-criterion suite.
//
// Asserts the EXACT verdict/reason payload for every `m5_p4_soa_*` fixture at the
// analysis level (`soa_candidate_query`), the authority resolution
// (`layout_decisions_query` — D11 padding-wins precedence, E3's one-source
// mitigation), and the entry gates (`YNZ_NO_AUTO_PARALLEL` env; kernel mode via the
// pure `soa::analyze` core — kernel is not reachable through the production query
// today, same posture as the cpu_promotion tests). The fixtures live in
// crates/ynz-driver/tests/fixtures/ — the SAME files the byte-layout integration
// test (crates/ynz-driver/tests/integration.rs) and Phase 5's dual-mode oracle run,
// so analysis verdicts and runtime behavior are asserted against one artifact.

use std::sync::Mutex;

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::soa;
use ynz_typeck::{
    check_query, layout_decisions_query, module_signatures_query, soa_candidate_query,
    FieldSegment, LayoutDecisions, LayoutKind, SoaCandidate, SoaDeclineReason, SoaVerdict,
};

/// Serializes every test in this binary: the salsa queries read the process-global
/// `YNZ_NO_AUTO_PARALLEL` env var at their entry, and cargo runs the tests in this
/// file on parallel threads — without the lock one test's `set_var` races another
/// test's query and both flake (the queries.rs ENV_LOCK precedent). Every test
/// holds the lock and clears the var before querying.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn fixture_src(name: &str) -> String {
    let path = format!(
        "{}/../ynz-driver/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {path} must exist for the analysis suite: {e}"))
}

/// Fresh db → `soa_candidate_query` rows for one fixture. Caller holds ENV_LOCK
/// with the env var in the state it wants the query to see.
fn candidates_for(name: &str) -> Vec<SoaCandidate> {
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, name.to_string(), fixture_src(name));
    db.register_source(sf);
    (*soa_candidate_query(&db, sf)).clone()
}

/// Fresh db → the layout AUTHORITY for one fixture.
fn layout_for(name: &str) -> LayoutDecisions {
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, name.to_string(), fixture_src(name));
    db.register_source(sf);
    (*layout_decisions_query(&db, sf)).clone()
}

fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
    guard
}

#[test]
fn qualifying_admits_with_exact_len_and_hot_fields() {
    // WHY: the one Admitted cell — a 72-element (> 64 strict) literal whose only
    // use is a 2-field hot loop must admit with the exact provable length and the
    // hot-field union in DECLARED order.
    let _guard = env_guard();
    let rows = candidates_for("m5_p4_soa_qualifying.ynz");
    assert_eq!(
        rows.len(),
        1,
        "exactly one array<Shape> binding; got: {rows:?}"
    );
    assert_eq!(rows[0].array_name, "pts");
    assert_eq!(rows[0].shape_name, "Point");
    assert_eq!(
        rows[0].verdict,
        SoaVerdict::Admitted {
            provable_len: 72,
            hot_fields: vec!["x".to_string(), "y".to_string()],
        }
    );
}

#[test]
fn qualifying_authority_resolves_soa_with_declared_order_segments() {
    // WHY: the E10 forward-compat surface — an admitted, un-padded candidate must
    // resolve to `Soa` whose segments are ALL declared fields in declared order
    // (segment_index = declared position); v0.8's serializer codegen reconstructs
    // unified values from exactly this metadata, so it must be real and tested.
    let _guard = env_guard();
    let layout = layout_for("m5_p4_soa_qualifying.ynz");
    assert!(
        layout.padded_shapes.is_empty(),
        "Point never crosses a spawn; padded set must be empty, got: {:?}",
        layout.padded_shapes
    );
    assert_eq!(
        layout.arrays.len(),
        1,
        "one decision row; got: {:?}",
        layout.arrays
    );
    let d = &layout.arrays[0];
    assert_eq!(d.array_name, "pts");
    assert_eq!(d.shape_name, "Point");
    assert_eq!(
        d.kind,
        LayoutKind::Soa {
            segments: vec![
                FieldSegment {
                    field: "x".to_string(),
                    segment_index: 0
                },
                FieldSegment {
                    field: "y".to_string(),
                    segment_index: 1
                },
            ],
        }
    );
}

#[test]
fn threefield_union_declines_field_union_too_wide() {
    // WHY: D5 — the ≤2-field criterion is the UNION across all loops; a 3-field
    // hot loop declines with the fields in declared order.
    let _guard = env_guard();
    let rows = candidates_for("m5_p4_soa_threefield.ynz");
    assert_eq!(rows.len(), 1, "exactly one binding; got: {rows:?}");
    assert_eq!(rows[0].array_name, "rs");
    assert_eq!(
        rows[0].verdict,
        SoaVerdict::Declined(SoaDeclineReason::FieldUnionTooWide {
            fields: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        })
    );
}

#[test]
fn escaping_declines_binding_and_records_param_row() {
    // WHY: D4 — an array passed as a call argument declines (the analysis cannot
    // see all accesses), and the callee's array<Shape> PARAMETER is its own
    // declined row (S2 finding 5: params are recorded, never silently absent).
    let _guard = env_guard();
    let rows = candidates_for("m5_p4_soa_escaping.ynz");
    assert_eq!(rows.len(), 2, "param row + binding row; got: {rows:?}");
    // Item order: sumX is declared first → its param row comes first.
    assert_eq!(rows[0].array_name, "pts");
    assert_eq!(
        rows[0].verdict,
        SoaVerdict::Declined(SoaDeclineReason::Escapes {
            how: "function parameter — cross-function by definition (D4)".to_string(),
        })
    );
    assert_eq!(rows[1].array_name, "pts");
    assert_eq!(
        rows[1].verdict,
        SoaVerdict::Declined(SoaDeclineReason::Escapes {
            how: "passed to `sumX()` (D4)".to_string(),
        })
    );
}

#[test]
fn growth_op_declines_grown() {
    // WHY: D3 — any `.add()` on the binding declines it (growth under segmented
    // layout forces per-segment re-layout); growth precedes the length/union
    // checks in the banked decline precedence.
    let _guard = env_guard();
    let rows = candidates_for("m5_p4_soa_growth.ynz");
    assert_eq!(rows.len(), 1, "exactly one binding; got: {rows:?}");
    assert_eq!(rows[0].array_name, "pts");
    assert_eq!(
        rows[0].verdict,
        SoaVerdict::Declined(SoaDeclineReason::Grown {
            op: ".add()".to_string()
        })
    );
}

#[test]
fn lend_self_shape_declines_at_shape_level() {
    // WHY: E6 (roadmap mandate) — a standalone `lend self: Pirate` function means
    // SoA would split storage that function expects contiguous; the filter is
    // shape-level and fires regardless of call sites.
    let _guard = env_guard();
    let rows = candidates_for("m5_p4_soa_lendself.ynz");
    assert_eq!(rows.len(), 1, "exactly one array binding; got: {rows:?}");
    assert_eq!(rows[0].array_name, "crew");
    assert_eq!(rows[0].shape_name, "Pirate");
    assert_eq!(
        rows[0].verdict,
        SoaVerdict::Declined(SoaDeclineReason::LendSelfMethod {
            function: "recordHit".to_string(),
        })
    );
}

#[test]
fn runtime_length_declines_and_returned_array_escapes() {
    // WHY: D3 — a call-initialized binding has no compile-time-provable length;
    // and the factory's own `out` binding declines as a return escape (D4).
    let _guard = env_guard();
    let rows = candidates_for("m5_p4_soa_runtime_length.ynz");
    assert_eq!(
        rows.len(),
        2,
        "factory `out` + entrypoint `pts`; got: {rows:?}"
    );
    // Item order: makePoints is declared first → its `out` row comes first.
    assert_eq!(rows[0].array_name, "out");
    assert_eq!(
        rows[0].verdict,
        SoaVerdict::Declined(SoaDeclineReason::Escapes {
            how: "returned from the function".to_string(),
        })
    );
    assert_eq!(rows[1].array_name, "pts");
    assert_eq!(
        rows[1].verdict,
        SoaVerdict::Declined(SoaDeclineReason::LengthNotProvable)
    );
}

#[test]
fn small_n_declines_below_size_threshold() {
    // WHY: the strict `> 64` threshold — a fully-qualifying 4-element loop
    // declines with the exact provable length in the reason payload.
    let _guard = env_guard();
    let rows = candidates_for("m5_p4_soa_small_n.ynz");
    assert_eq!(rows.len(), 1, "exactly one binding; got: {rows:?}");
    assert_eq!(rows[0].array_name, "pts");
    assert_eq!(
        rows[0].verdict,
        SoaVerdict::Declined(SoaDeclineReason::BelowSizeThreshold { len: 4 })
    );
}

#[test]
fn reassigned_binding_declines_as_escape() {
    // WHY: reassign-staleness (closing-round finding 1) — the walk's recorded
    // initializer signals (provable_len = 72) describe the ORIGINAL array only; a
    // mid-function `pts = [...]` rebinds the name, so an Admitted verdict would
    // carry a stale provable_len into Phase 5's buffer sizing (the E3/E7
    // silent-miscompile class). The otherwise-fully-qualifying binding must
    // conservatively decline — and the SAME statement's RHS declines the source
    // binding as an alias (both halves of the Assign arm, exact reasons).
    let _guard = env_guard();
    let rows = candidates_for("m5_p4_soa_reassigned.ynz");
    assert_eq!(
        rows.len(),
        2,
        "`pts` + `replacement`, declaration order; got: {rows:?}"
    );
    assert_eq!(rows[0].array_name, "pts");
    assert_eq!(
        rows[0].verdict,
        SoaVerdict::Declined(SoaDeclineReason::Escapes {
            how: "reassigned after initial binding".to_string(),
        })
    );
    assert_eq!(rows[1].array_name, "replacement");
    assert_eq!(
        rows[1].verdict,
        SoaVerdict::Declined(SoaDeclineReason::Escapes {
            how: "stored into another binding (aliases the array)".to_string(),
        })
    );
}

#[test]
fn let_shadowed_binding_declines_as_rebound() {
    // WHY: re-`let`-shadow staleness (closing round 2) — same-scope re-`let`
    // shadowing is LEGAL Yinz in a non-suspending function (check_let just
    // overwrites the scope entry), and Pass 1 previously kept the FIRST record on
    // a shadow, so a 72-element qualifying binding shadowed by a 4-element re-`let`
    // would stay Admitted { provable_len: 72 } — a 68-element over-size fed into
    // Phase 5's buffer sizing (the E3/E7 silent-miscompile class; the re-`let`
    // sibling of the Assign-arm reassign decline above). The larger-then-smaller
    // direction is the actual miscompile-risk direction; the record must decline,
    // never carry the stale first length.
    let _guard = env_guard();
    let rows = candidates_for("m5_p4_soa_let_shadow.ynz");
    assert_eq!(rows.len(), 1, "one name → one record; got: {rows:?}");
    assert_eq!(rows[0].array_name, "pts");
    assert_eq!(
        rows[0].verdict,
        SoaVerdict::Declined(SoaDeclineReason::Escapes {
            how: "rebound by a later let".to_string(),
        })
    );
}

#[test]
fn no_field_access_loop_declines_no_per_field_loop_access() {
    // WHY: closing-round finding 2 — an above-threshold array iterated by a
    // count-only loop has an EMPTY per-field union: nothing to segment for, AoS
    // stays optimal. This is the one live SoaDeclineReason branch the original
    // step-4 fixture table never covered; the exact-reason assertion closes it.
    let _guard = env_guard();
    let rows = candidates_for("m5_p4_soa_no_field_access.ynz");
    assert_eq!(rows.len(), 1, "exactly one binding; got: {rows:?}");
    assert_eq!(rows[0].array_name, "pts");
    assert_eq!(
        rows[0].verdict,
        SoaVerdict::Declined(SoaDeclineReason::NoPerFieldLoopAccess)
    );
}

#[test]
fn both_candidate_admits_at_the_candidate_walk() {
    // WHY: the candidate walk must NOT know about padding (one derivation per
    // question) — `season` fully qualifies, so the walk admits it; the decline
    // belongs to the AUTHORITY (next test), never re-derived in the walk.
    let _guard = env_guard();
    let rows = candidates_for("m5_p4_soa_both_candidate.ynz");
    assert_eq!(rows.len(), 1, "exactly one array binding; got: {rows:?}");
    assert_eq!(rows[0].array_name, "season");
    assert_eq!(rows[0].shape_name, "Tally");
    assert_eq!(
        rows[0].verdict,
        SoaVerdict::Admitted {
            provable_len: 66,
            hot_fields: vec!["hits".to_string(), "outs".to_string()],
        }
    );
}

#[test]
fn both_candidate_authority_resolves_padding_wins() {
    // WHY: D11 / E3 — the ONE authority resolves a simultaneously-padded-and-
    // admitted shape to `Aos { CrossThreadPadded }`, NEVER `Soa`; correctness
    // under false sharing outranks cache locality. The byte-layout half (padded
    // IR intact + dual-mode-identical stdout) lives in the driver integration
    // test on this same fixture.
    let _guard = env_guard();
    let layout = layout_for("m5_p4_soa_both_candidate.ynz");
    assert!(
        layout.padded_shapes.contains("Tally"),
        "Tally crosses the background spawn and must be padded; got: {:?}",
        layout.padded_shapes
    );
    assert_eq!(
        layout.arrays.len(),
        1,
        "one decision row; got: {:?}",
        layout.arrays
    );
    let d = &layout.arrays[0];
    assert_eq!(d.array_name, "season");
    assert_eq!(d.shape_name, "Tally");
    assert_eq!(
        d.kind,
        LayoutKind::Aos {
            declined: SoaDeclineReason::CrossThreadPadded,
        }
    );
}

#[test]
fn no_auto_parallel_env_empties_the_candidate_list() {
    // WHY: D1/A3 — the query entry-gates on the ONE `no_auto_parallel_env()`
    // predicate; under the flag the qualifying fixture must yield ZERO candidates
    // (a fresh db per half so the salsa cache re-evaluates with the env visible).
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    // Sanity half: without the flag the qualifying fixture admits (a regression
    // here would make the gated half vacuously pass).
    std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
    let on = candidates_for("m5_p4_soa_qualifying.ynz");
    assert_eq!(
        on.len(),
        1,
        "sanity: default mode must yield the candidate; got: {on:?}"
    );

    std::env::set_var("YNZ_NO_AUTO_PARALLEL", "1");
    let off = candidates_for("m5_p4_soa_qualifying.ynz");
    std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
    assert!(
        off.is_empty(),
        "YNZ_NO_AUTO_PARALLEL=1 must empty the candidate list; got: {off:?}"
    );
}

#[test]
fn kernel_mode_pure_core_empties_the_candidate_list() {
    // WHY: D1 — kernel mode disables SoA analysis entirely. The production query
    // is never kernel today (`--kernel` wires only through the test-only check
    // path), so the gate is proven on the pure `analyze` core with the flag
    // driven explicitly — the same posture as the cpu_promotion kernel test.
    let _guard = env_guard();
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(
        &db,
        "m5_p4_soa_qualifying.ynz".to_string(),
        fixture_src("m5_p4_soa_qualifying.ynz"),
    );
    db.register_source(sf);
    let check_out = check_query(&db, sf);
    let sig_out = module_signatures_query(&db, sf);

    // Sanity half: kernel off admits.
    let on = soa::analyze(&check_out.typed_module, &sig_out.shape_table, false, false);
    assert_eq!(
        on.len(),
        1,
        "sanity: non-kernel must yield the candidate; got: {on:?}"
    );

    let off = soa::analyze(&check_out.typed_module, &sig_out.shape_table, true, false);
    assert!(
        off.is_empty(),
        "kernel mode must empty the candidate list; got: {off:?}"
    );
}
