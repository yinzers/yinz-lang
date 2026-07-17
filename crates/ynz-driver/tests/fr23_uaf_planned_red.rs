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
// These tests are the no-duct-tape planned-RED inverse: they assert the CORRECT
// contract (both tiers print real values), so they FAIL today and go green only when
// the real fix — give/copy machinery for field/index/call-materialized spawn
// receivers — lands. Do NOT weaken an assertion, widen the watchdog, or assert the
// currently-observed wrong output to force green: a red here is a live
// use-after-free in the flagship concurrency surface.
//
// test-ratchet: planned-RED lock authored per FRAGO 011 (2026-07-17) — `#[ignore]`d
// pending the fr23 fix (morning disposition: FRAGO-inserted phase in this plan vs a
// scoped M8-adjacent follow-up). The fixing phase removes the `#[ignore]` marks per
// the Phase-1/Phase-3 RED-set precedent; deleting or weakening these tests instead
// is the test-weakening corpse.
//
// Run explicitly (dev container): docker compose run --rm dev \
//   cargo test -p ynz-driver --test fr23_uaf_planned_red -- --ignored

use std::{
    path::PathBuf,
    process::{Command, Output},
    time::Duration,
};

/// Watchdog ceiling for a compiled fixture run. A trip is a real deadlock/miscompile,
/// never a slow test — fix the codegen, don't widen this.
const RUN_WATCHDOG: Duration = Duration::from_secs(60);

/// The one correctness line both fixtures must produce: the spawned task reading the
/// receiver's REAL field values, not a dead frame's reuse garbage.
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
#[ignore = "planned-RED: fr23 confirmed-live UAF, FRAGO 011 — fix is give/copy machinery for non-ident spawn receivers"]
fn fr23_red_maybe_payload_spawn_receiver_reads_live_values() {
    // WHY: locks fr23 shape B' — a maybe-payload spawn receiver's backing storage
    // must outlive the spawner's frame. Today the payload alloca rides raw into the
    // task and the task reads dead-frame garbage at BOTH tiers (optimized 0/0,
    // O0 stomp sentinels) — confirmed-live 2026-07-17, FRAGO 011.
    assert_both_tiers_print_correct_haul("v0_3_m7_fr23_maybe_payload_spawn_receiver.ynz");
}

#[test]
#[ignore = "planned-RED: fr23 confirmed-live UAF, FRAGO 011 — fix is give/copy machinery for non-ident spawn receivers"]
fn fr23_red_call_materialized_spawn_receiver_reads_live_values() {
    // WHY: locks fr23 shape C2 — a call-materialized spawn receiver
    // (`background makeCargo().haul()`) must not hand the callee-return temp's
    // stack address to the task. Today O0 prints partially-stomped garbage 6/6;
    // the optimized tier's correct output is layout luck over IR-proven identical
    // dangling — confirmed-live 2026-07-17, FRAGO 011.
    assert_both_tiers_print_correct_haul("v0_3_m7_fr23_call_materialized_spawn_receiver.ynz");
}
