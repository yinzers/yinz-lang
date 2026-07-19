// v0.3-M7 FRAGO 011 planned-RED gate — fr23: non-plain-ident background-spawn
// receivers ride as raw pointers (roadmap Capability Ledger row
// `2026-07-04-v0-3-m6-concurrency-hotfix#8-fr23`; plan
// `2026-07-04-v0-3-m7-optimizer-pipeline` Future Requirements #9 / risk row R11).
//
// The fr23 disposition-(b) gate (audit.md entry `executor-2026-07-17-fr23-uaf-gate`,
// 2026-07-17, HEAD 3e3bf6c) CONFIRMED the UAF LIVE for two receiver shapes:
//
//   B' — maybe-payload receiver (`first.value.haul()` where `first: maybe<Cargo>` was
//        materialized from an index): the payload's stack alloca (`%first_pay_own`)
//        rides un-upgraded into the bg ctx. WRONG at BOTH tiers (optimized: `0/0` 6/6;
//        O0: nondeterministic stomp sentinels).
//   C2 — call-materialized receiver (`background makeCargo().haul()`): the callee's
//        `%call_shape_ret` alloca rides ptrtoint into the ctx; spawner returns
//        immediately. WRONG 6/6 at O0; the optimized tier's correct output is
//        stack-layout LUCK over IR-proven identical dangling — not safety.
//
// Root cause (both shapes): `is_heap_arg` (crates/ynz-codegen/src/emit.rs, the
// background-spawn heap-upgrade gate) admits only `Expr::Ident` (with inferred
// ownership) or an explicit `.copy()` postfix; every other receiver expression gets
// `BgArgFreeKind::None` — a raw-pointer ride into a task that outlives the frame.
// check.rs's spawn-receiver ownership path likewise returns silent None for
// non-ident receivers.
//
// FIXED — v0.3-M7 Phase 9 (FRAGO 016, disposition (a)): typeck's
// `bg_arg_is_materialized_shape_temp` records both shapes as `Give` in
// `background_arg_inferred_ownership`, and codegen's `is_heap_arg` gate consults
// that ONE authoritative record by span for any expression shape — the receiver's
// pointed-to bytes now heap-upgrade (`ynz_alloc` + memcpy, freed by the task's
// existing `BgArgFreeKind::HeapShape` ladder) before the ctx is built.
//
// test-ratchet: authored as FRAGO 011 planned-RED locks (2026-07-17); converted to
// permanent green regression locks by the Phase 9 fix (`#[ignore]` removed per the
// Phase-1/Phase-3 RED-set precedent). They assert the CORRECT contract (both tiers
// print real values). Do NOT weaken an assertion, widen the watchdog, or delete
// these tests: a red here is a live use-after-free in the flagship concurrency
// surface — the test-weakening corpse applies.
//
// v0.3-M7 FRAGO 024 (round 8, 2026-07-18): the FRAGO 022/023 default-deny
// redesign's OWN security re-check found two NEW, qualitatively different bugs
// beyond the six-round allowlist saga above — see the `fr24_*` tests near the end
// of this file. Same "do not weaken" discipline applies.

use std::{
    path::PathBuf,
    process::{Command, Output},
    time::Duration,
};

/// Watchdog ceiling for a compiled fixture run. A trip is a real deadlock/miscompile,
/// never a slow test — fix the codegen, don't widen this.
const RUN_WATCHDOG: Duration = Duration::from_secs(60);

/// The one correctness line every fixture in this suite must produce: the spawned task
/// reading the receiver's REAL field values, not a dead frame's reuse garbage.
const CORRECT_HAUL_LINE: &str = "haul: 111/222";

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn run_with_watchdog(mut cmd: Command) -> Output {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("watchdog: failed to spawn child process");
    let start = std::time::Instant::now();
    loop {
        match child
            .try_wait()
            .expect("watchdog: failed to poll child status")
        {
            Some(_status) => {
                return child
                    .wait_with_output()
                    .expect("watchdog: failed to collect child output after exit");
            }
            None => {
                if start.elapsed() >= RUN_WATCHDOG {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "WATCHDOG TRIP: fixture did not exit within {RUN_WATCHDOG:?} — treat \
                         as a hang-class miscompile; fix the codegen, never widen this timeout"
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Build the fixture at ONE tier and assert the spawned task printed the receiver's
/// real values. Unlike the optimizer_red_gate differential harness, the O0 run is NOT
/// a trustworthy anchor here — shape B' is corrupt at BOTH tiers — so each tier is
/// held to the absolute correct contract independently.
fn assert_tier_prints_correct_haul(fixture_name: &str, tier_args: &[&str], tier_label: &str) {
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    let src = fixture(fixture_name);
    let isolated_src = tmp.path().join(src.file_name().expect("fixture filename"));
    std::fs::copy(&src, &isolated_src).expect("failed to copy fixture into tmpdir");

    let mut args: Vec<&str> = vec!["build"];
    args.extend_from_slice(tier_args);
    let src_str = isolated_src.to_str().unwrap();
    args.push(src_str);
    let build_out = Command::new(ynz_binary())
        .args(&args)
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz build");
    assert!(
        build_out.status.success(),
        "ynz build ({tier_label}) failed for {fixture_name}:\n{}",
        String::from_utf8_lossy(&build_out.stderr)
    );

    let binary = isolated_src.with_extension("");
    let mut run_cmd = Command::new(&binary);
    run_cmd.env("CLICOLOR", "0");
    let run = run_with_watchdog(run_cmd);
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();

    assert_eq!(
        run.status.code().unwrap_or(-1),
        0,
        "{fixture_name} ({tier_label}): non-zero exit; stderr:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        stdout.contains(CORRECT_HAUL_LINE),
        "{fixture_name} ({tier_label}): spawned task did not print the receiver's real \
         values ({CORRECT_HAUL_LINE:?}) — it read a dead stack frame (fr23 UAF, FRAGO 011). \
         stdout:\n{stdout}"
    );
}

/// Both tiers must independently satisfy the correct contract: the shipped default
/// (optimized) AND the `--no-optimize` O0 escape hatch.
fn assert_both_tiers_print_correct_haul(fixture_name: &str) {
    assert_tier_prints_correct_haul(fixture_name, &["--no-optimize"], "O0 escape hatch");
    assert_tier_prints_correct_haul(fixture_name, &[], "default optimized");
}

#[test]
fn fr23_red_maybe_payload_spawn_receiver_reads_live_values() {
    // WHY: locks fr23 shape B' — a maybe-payload spawn receiver's backing storage
    // must outlive the spawner's frame. Today the payload alloca rides raw into the
    // task and the task reads dead-frame garbage at BOTH tiers (optimized 0/0,
    // O0 stomp sentinels) — confirmed-live 2026-07-17, FRAGO 011.
    assert_both_tiers_print_correct_haul("v0_3_m7_fr23_maybe_payload_spawn_receiver.ynz");
}

#[test]
fn fr23_red_call_materialized_spawn_receiver_reads_live_values() {
    // WHY: locks fr23 shape C2 — a call-materialized spawn receiver
    // (`background makeCargo().haul()`) must not hand the callee-return temp's
    // stack address to the task. Today O0 prints partially-stomped garbage 6/6;
    // the optimized tier's correct output is layout luck over IR-proven identical
    // dangling — confirmed-live 2026-07-17, FRAGO 011.
    assert_both_tiers_print_correct_haul("v0_3_m7_fr23_call_materialized_spawn_receiver.ynz");
}

#[test]
fn fr23_generic_call_materialized_spawn_receiver_reads_live_values() {
    // WHY: locks the GENERIC-callee C2 variant (`background identity(c).haul()` with
    // `identity<T>(give T) -> T`). Generic functions live in `generic_fn_table.fns`,
    // not `sig_table.fns`, so a sig_table-only admission read in
    // `bg_arg_is_materialized_shape_temp` silently missed the callee and reproduced
    // the fr23 UAF at both tiers — live-reproduced 2026-07-18 (Phase 9 security
    // fix-round). A red here means the generic-table fallback in the C2 arm regressed.
    assert_both_tiers_print_correct_haul(
        "v0_3_m7_fr23_generic_call_materialized_spawn_receiver.ynz",
    );
}

#[test]
fn fr23_generic_call_nested_arg_spawn_receiver_reads_live_values() {
    // WHY: locks the GENERIC-callee C2 variant with a NON-IDENT argument
    // (`background identity(makeCargo()).haul()` — `identity<T>`'s `T` is resolvable
    // ONLY from the nested call `makeCargo()`, never from a plain ident). The C2
    // admission arm's substitution-seeding loop originally consulted only
    // `Expr::Ident` args, so this shape silently fell through un-admitted and
    // reproduced the fr23 UAF at both tiers — live-reproduced 2026-07-18 (M7
    // cumulative completion-gate round 2, item 3). A red here means
    // `bg_arg_type_readonly`'s nested-call resolution regressed.
    assert_both_tiers_print_correct_haul("v0_3_m7_fr23_generic_call_nested_arg_spawn_receiver.ynz");
}

#[test]
fn fr23_generic_call_nested_generic_arg_spawn_receiver_reads_live_values() {
    // WHY: locks the GENERIC-callee C2 variant nested TWO levels deep
    // (`background identity(identity(makeCargo())).haul()` — the outer `identity`'s
    // argument is ITSELF a call to a GENERIC function, not a concrete one). Round 2's
    // fix (FRAGO 018) only resolved a nested CONCRETE call argument
    // (`bg_arg_type_readonly` read `sig_table` only); a nested GENERIC call left `T`
    // unresolved and the receiver fell through un-admitted — live-reproduced
    // 2026-07-18 (M7 completion-gate round 3, FRAGO 019). Closed by collapsing the
    // whole resolution into ONE recursive helper (`bg_call_return_type_readonly`) so
    // this depth and every depth beyond it resolve from the same definition. A red
    // here means the recursive resolver regressed to a depth-limited special case.
    assert_both_tiers_print_correct_haul(
        "v0_3_m7_fr23_generic_call_nested_generic_arg_spawn_receiver.ynz",
    );
}

#[test]
fn fr23_generic_call_triple_nested_spawn_receiver_reads_live_values() {
    // WHY: proves the resolver is genuinely RECURSIVE, not a fixed number of
    // hand-unrolled nesting levels — three levels of generic nesting
    // (`identity(identity(identity(makeCargo())))`) must resolve from the exact same
    // fix as the 2-deep sibling above. A red here while the 2-deep sibling stays
    // green would mean the fix is depth-bounded rather than truly recursive (M7
    // completion-gate round 3, FRAGO 019).
    assert_both_tiers_print_correct_haul(
        "v0_3_m7_fr23_generic_call_triple_nested_spawn_receiver.ynz",
    );
}

#[test]
fn fr23_generic_call_ufcs_nested_arg_spawn_receiver_reads_live_values() {
    // WHY: locks the GENERIC-callee C2 variant whose argument is a UFCS METHOD-CALL
    // CHAIN (`background identity(makeCargo().reroute()).haul()` — `reroute` is a
    // concrete `Cargo -> Cargo` UFCS function). Rounds 2/3 (FRAGO 018/019) made the
    // nested-argument resolver RECURSIVE for `Expr::Call`, but its match had exactly
    // two arms (`Ident`, `Call`) — a `MethodCall` (UFCS chain) nested inside a generic
    // callee's argument was never classified, so `identity`'s `T` stayed unresolved
    // and the receiver fell through un-admitted — live-reproduced independently by two
    // reviewers, 2026-07-18 (M7 completion-gate round 4, FRAGO 020). Closed by
    // collapsing the top-level admission check and the nested-argument resolver into
    // ONE exhaustively-matched classifier (`bg_expr_resolved_type`, no `_ =>`
    // catch-all) so a `MethodCall` is classified exactly once, consulted everywhere. A
    // red here means the unified classifier's `MethodCall` arm regressed.
    assert_both_tiers_print_correct_haul(
        "v0_3_m7_fr23_generic_call_ufcs_nested_arg_spawn_receiver.ynz",
    );
}

#[test]
fn fr23_generic_call_fieldaccess_nested_arg_spawn_receiver_reads_live_values() {
    // WHY: locks the GENERIC-callee C2 variant whose argument is a MAYBE-PAYLOAD
    // FIELD ACCESS (`background identity(first.value).haul()` where
    // `first: maybe<Cargo>`). Same class of gap as the UFCS sibling test above: the
    // nested-argument resolver never classified `Expr::FieldAccess`, even though the
    // top-level admission predicate already recognized `.value` (the B' class, FRAGO
    // 016) — nested inside a generic callee's argument, it fell through un-admitted —
    // live-reproduced independently by two reviewers, 2026-07-18 (M7 completion-gate
    // round 4, FRAGO 020). Closed by the SAME unified classifier as the UFCS sibling.
    // A red here means the unified classifier's `FieldAccess` arm regressed.
    assert_both_tiers_print_correct_haul(
        "v0_3_m7_fr23_generic_call_fieldaccess_nested_arg_spawn_receiver.ynz",
    );
}

#[test]
fn fr23_generic_maybe_payload_spawn_receiver_reads_live_values() {
    // WHY: locks the GENERIC-container B' variant — a `maybe<Cargo>` binding whose
    // type arrives through a generic instantiation (`let first = identity(m)`,
    // un-annotated). The B' admission arm reads `binding_ty_narrowed` (the concrete
    // instantiated scope type) and never touches the fn tables, so it is generic-safe
    // by construction — this test locks that construction against a future rewrite
    // that re-keys the arm on a table lookup (Phase 9 security fix-round, 2026-07-18).
    assert_both_tiers_print_correct_haul("v0_3_m7_fr23_generic_maybe_payload_spawn_receiver.ynz");
}

#[test]
fn fr23_sm_arm_call_materialized_spawn_receiver_reads_live_values() {
    // WHY: locks the C2 shape through the STATE-MACHINE spawn arm
    // (`lower_sm_background_spawn` — the callee suspends via `wait sleep`). The CPU
    // and SM arms share `prepare_bg_arg_for_ctx`, but until this test that sharing
    // was an inspection claim for the fr23 receiver shapes, not a verified contract
    // (Phase 9 code-reviewer fix-round, 2026-07-18).
    assert_both_tiers_print_correct_haul("v0_3_m7_fr23_sm_call_materialized_spawn_receiver.ynz");
}

#[test]
fn fr23_sm_arm_maybe_payload_spawn_receiver_reads_live_values() {
    // WHY: locks the B' shape through the STATE-MACHINE spawn arm — same
    // shared-`prepare_bg_arg_for_ctx` contract as the SM C2 lock above, for the
    // maybe-payload receiver (Phase 9 code-reviewer fix-round, 2026-07-18).
    assert_both_tiers_print_correct_haul("v0_3_m7_fr23_sm_maybe_payload_spawn_receiver.ynz");
}

// ── FRAGO 021 round-6 findings, closed by the FRAGO 022 default-deny redesign ──
//
// The four tests below lock FRAGO 021's confirmed-live findings (`background
// haul({ weight: 111, tag: 222 })`, `entry.value.haul()` on a `MapEntry<K,Cargo>`,
// `identity(c.copy())`, `identity(wait makeCargo())`). Each was reproduced live as
// a real UAF against the pre-FRAGO-022 allowlist (`bg_arg_is_materialized_shape_temp`)
// and checked in as documented RED per no-duct-tape.md's legitimate-inverse
// pattern (see each fixture's own header). None of the four needed a per-shape
// special-case arm to pass here: `bg_arg_is_provably_safe`'s wildcard `_ => false`
// catches `StructLit`/`FieldAccess`/an unresolved generic substitution seed by
// construction — the fix is the ARCHITECTURE, not four more allowlist entries. A
// red on any of these means the default-deny wildcard regressed back toward an
// allowlist.

#[test]
fn fr23_structlit_spawn_receiver_reads_live_values() {
    // WHY: locks FRAGO 021 finding 1 — a bare anonymous struct literal used
    // directly as a `background` spawn ARGUMENT (`background haul({ weight: 111,
    // tag: 222 })`). Confirmed-live UAF 2026-07-18 against the old allowlist (O0
    // deterministic `haul: 777777/<leaked-address>` 6/6). `bg_arg_is_provably_safe`
    // never special-cases `StructLit` — it falls through the trailing wildcard to
    // `Give` because a `StructLit` is not `Ident`/`SelfValue`/a literal primitive/a
    // provably-non-`Shape` `Call`/`MethodCall`.
    assert_both_tiers_print_correct_haul("v0_3_m7_fr23_structlit_spawn_receiver.ynz");
}

#[test]
fn fr23_mapentry_value_spawn_receiver_reads_live_values() {
    // WHY: locks FRAGO 021 finding 2 — `entry.value.haul()` inside `for (entry in
    // fleet)`, where `entry.value`'s `Cargo` resolves via the MapEntry `.value`
    // producer, NOT the `maybe<Shape>.value` narrowing the old classifier's
    // `.value` arm recognized. Confirmed-live UAF 2026-07-18 (O0 deterministic
    // `haul: 777888/222`; optimized tier leaked stack addresses).
    // `bg_arg_is_provably_safe` never inspects WHICH `.value` producer a
    // `FieldAccess` came from — every `FieldAccess` falls through the wildcard to
    // `Give` — and `background_spawn_call_form`'s receiver gate was widened to the
    // SAME predicate (FRAGO 022), so the UFCS-chain receiver is normalized for
    // admission instead of silently bailing.
    assert_both_tiers_print_correct_haul("v0_3_m7_fr23_mapentry_value_spawn_receiver.ynz");
}

#[test]
fn fr23_generic_call_copy_nested_arg_spawn_receiver_reads_live_values() {
    // WHY: locks FRAGO 021 finding 3 — `identity(c.copy()).haul()` (`identity<T>(give
    // value: T) -> T`), where `T` can only be resolved from the `PostfixOp{Copy}`
    // nested inside the generic call's argument. Confirmed-live UAF 2026-07-18 (O0
    // mixed garbage `haul: 111/0`/`haul: 0/0`). `bg_expr_resolved_type`'s
    // `PostfixOp` arm is UNCHANGED — it still returns `None` for a nested `.copy()`
    // — but `bg_arg_is_provably_safe`'s `Call` arm now reads that as FAIL-CLOSED:
    // an unresolved substitution seed leaves `T` an unbound `TypeParam`, which
    // `type_provably_not_shape` does not recognize as safe, so the OUTER
    // `identity(...)` call defaults to `Give` without any new arm added to the
    // seeding resolver.
    assert_both_tiers_print_correct_haul(
        "v0_3_m7_fr23_generic_call_copy_nested_arg_spawn_receiver.ynz",
    );
}

#[test]
fn fr23_generic_call_wait_nested_arg_spawn_receiver_reads_live_values() {
    // WHY: locks FRAGO 021 finding 4 — `identity(wait makeCargo()).haul()`, the
    // same substitution-seeding defect class as the `.copy()` sibling above, on a
    // `Wait`-wrapped leaf (M8 sequential semantics make `wait expr` type-identical
    // to `expr`, but the seeding resolver's `Wait` arm was never taught to unwrap
    // it). Confirmed-live UAF 2026-07-18 (O0 `haul: 111/0` 5/6, `haul: 0/0` 1/6).
    // Closed by the SAME fail-closed `Call`-arm reasoning as the `.copy()` sibling:
    // an unresolved seed fails closed regardless of WHICH nested shape caused the
    // seeding gap.
    assert_both_tiers_print_correct_haul(
        "v0_3_m7_fr23_generic_call_wait_nested_arg_spawn_receiver.ynz",
    );
}

// ── FRAGO 024 round-8 findings — the default-deny redesign's OWN security
// re-check surfaced two NEW, qualitatively different bugs beyond the six-round
// allowlist saga above: a structural wiring gap (the admission machinery ran on
// only two of the syntactic positions a `background` spawn can occupy) and a
// `SelfValue` false-safe classification (a nested-spawn free-ladder race, not a
// materialization gap). Neither is "an 11th allowlist shape" — see each test's
// WHY for the distinct root cause.

#[test]
fn fr24_fieldassign_spawn_receiver_reads_live_values() {
    // WHY: locks FRAGO 024 Bug 1 — the ownership-recording machinery was wired
    // into only `check_stmts`'s Stmt::Expr match and `check_let`'s handle-form;
    // every OTHER statement form (`Assign`, `FieldAssign`, `IndexAssign`) routed
    // through the generic `infer_expr` `Expr::Background` arm with NO recording
    // at all. `hd.slot = background makeCargo().haul()` (a `FieldAssign` target)
    // confirmed-live UAF 2026-07-18 (O0 `haul: 0/777777`, `haul: 0/0`). Closed by
    // moving the recording loop into the generic arm itself — the ONE place every
    // spawn form, in every statement position, provably passes through. A red
    // here means the structural admission backstop regressed.
    assert_both_tiers_print_correct_haul("v0_3_m7_fr24_fieldassign_spawn_receiver.ynz");
}

#[test]
fn fr24_nested_self_spawn_receiver_reads_live_values() {
    // WHY: locks FRAGO 024 Bug 2 — `bg_arg_is_provably_safe`'s `SelfValue => true`
    // arm assumed `self`'s backing storage always outlives any spawn using it,
    // true for the single-level case FRAGO 021 arm #15 tested but false for a
    // NESTED spawn: a `give self` parameter whose OWNING function is itself
    // reached via `background` (`background relay(makeCargo())` where
    // `relay(give self: Cargo) { background self.haul() }`). The OUTER task's
    // free ladder frees `self`'s heap cell immediately after the inner
    // fire-and-forget spawn returns, racing the inner task's delayed read.
    // Confirmed-live 14/14 (both tiers) 2026-07-18 — `weight` corrupted to
    // garbage every run while `tag` (the second field) survived. Closed by
    // removing `SelfValue` from the safe set; it now defaults to `Give` via the
    // trailing wildcard like every other non-enumerated shape. A red here means
    // `SelfValue` regressed back into the safe set.
    assert_both_tiers_print_correct_haul("v0_3_m7_fr24_nested_self_spawn_receiver.ynz");
}

#[test]
fn fr24_nested_ident_spawn_receiver_control_reads_live_values() {
    // WHY: CONTROL for the Bug 2 lock above — identical shape, but the receiver
    // parameter is a plain-named `Ident` (`cargo`) instead of `SelfValue`. This
    // construction was ALREADY correct before the FRAGO 024 fix (the `Ident`
    // liveness path in `check_stmts` always records SOME ownership entry,
    // forcing heap-upgrade) — locked so a future change cannot silently regress
    // the `Ident` path while touching the `SelfValue` one.
    assert_both_tiers_print_correct_haul("v0_3_m7_fr24_nested_ident_spawn_receiver_control.ynz");
}
