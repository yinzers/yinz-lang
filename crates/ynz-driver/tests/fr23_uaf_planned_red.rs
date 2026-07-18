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
