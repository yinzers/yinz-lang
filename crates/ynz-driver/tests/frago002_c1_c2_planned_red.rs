// v0.3 Concurrency Hardening — FRAGO 002 planned-RED gate (Phase 3 step 3.0,
// dispatch hardening-p3.0-20260906-a1). Plan
// `.claude/planning/active/2026-09-04-v0-3-concurrency-hardening/plan.md`; findings
// `.claude/planning/active/2026-09-04-v0-3-concurrency-hardening/audit.md` FRAGO 002.
//
// These are the five probe programs FRAGO 002 built and verified
// (`target/p2b-probe/`, gitignored — never committed there) as A, D, G, J, N,
// committed here as fixtures by step 3.0. All five were `#[ignore]`d and failing
// then — a pin that passes before its fix lands is measuring nothing.
//
// STATE AFTER Phase 3 step 3.2 (dispatch hardening-p3.2-20260906-a1): ALL FIVE pins
// are FIXED and LIVE — no `#[ignore]` anywhere in this file, every one run by a plain
// `cargo test -p ynz-driver`, and a red in any of them is a real re-opening of its
// cluster rather than a planned RED. Step 3.1 closed C1 (A, D, G, J); step 3.2 closed
// C2 (N) by replacing the two parallel per-type owned-copy dispatches with one shared
// routine (`ynz_typeck::owned_copy`).
//
// ── C1 (probes A, D, G, J) ──────────────────────────────────────────────────────
// Producer: `collect_crossings_in_stmts` (crates/ynz-typeck/src/check.rs). Once
// `past_wait` is true, a statement that is ITSELF a suspension point (a conduit
// `.send()`, a suspending call, a `let x = <suspending expr>`) records its
// result-binding but never has its OWN operands scanned — the `If`/`While`/`For`/
// `Match` arms call `collect_ident_refs_in_stmt`; the direct suspending forms fall
// through `_ => {}`. A pre-suspension local read ONLY by such a statement never
// enters the crossing set, gets no frame slot, and the resumed continuation reads an
// uninitialised alloca. Closes M8 FR #11(a) (probes A, D) and FR #11(b) (probes G,
// J). One producer, four probes, because FRAGO 002's own Probe H control proved it:
// adding one harmless read of the local before the suspending statement fixes both
// symptoms.
//
// ── C2 (probe N) ────────────────────────────────────────────────────────────────
// Producer: `.copy()`'s per-type lowering had an `AliasNoOp` arm that returned the
// receiver's OWN pointer for `fixed<T>` instead of an independent copy — one of two
// parallel per-type dispatches that both defaulted to aliasing. Both now consume the
// single owned-copy table in `ynz_typeck::owned_copy`, which has exactly two answers
// per type: a real copy, or a compile-time refusal with teaching text. Closes M8
// FR #10.
//
// One file for both clusters, not two: `fr23_uaf_planned_red.rs` and
// `d5_frame_slot_collision_planned_red.rs` are each scoped to ONE defect family
// (spawn-receiver UAF; loop-var frame-slot keying) discovered by their own gate.
// FRAGO 002 is itself the shared filing these five probes came out of — one audit
// entry naming C1 and C2 together as the not-one-not-six answer to Phase 2 question
// (b) — so one file mirrors the audit structure it locks rather than inventing a
// finer split the audit itself doesn't draw. C3/S1 (parked 33/34) are NOT here:
// Phase 3's checklist schedules them after C1/C2 and they have no probe fixture yet.
//
// test-ratchet: mirrors the fr23 / d5 planned-RED precedent exactly — a still-planned
// RED is `#[ignore]`d so a plain `cargo test` / `cargo nextest` run does not see it as
// a failure (nextest fail-fast is a per-target failure; see
// `.claude/plans/parked.md` entry 52), is run explicitly with `-- --ignored` to
// observe the RED, and its fixing FRAGO removes the `#[ignore]` mark rather than
// deleting or weakening the assertion. Step 3.1 did exactly that for A, D, G and J,
// and step 3.2 for N; every assertion is byte-identical to the one that was failing,
// except N's WHY comment, which drops its "or a refusal" branch now that the ruling
// has been applied and `fixed<T>` landed on the copy side of it.
//
// Run the whole file (dev container) — nothing here is ignored any more:
//   docker compose exec dev cargo test -p ynz-driver --test frago002_c1_c2_planned_red

use std::{
    path::PathBuf,
    process::{Command, Output},
    time::Duration,
};

/// Watchdog ceiling for a compiled fixture run. A trip is a real hang-class
/// miscompile, never a slow test — fix the codegen, don't widen this.
const RUN_WATCHDOG: Duration = Duration::from_secs(60);

/// Repeat count for the two probes (G, J) whose corruption is non-deterministic at
/// `-O0` (~50% and ~65%-ish observed here, see each test's WHY). The RED assertion
/// requires ALL N runs to match the correct value — today that is false with
/// overwhelming probability (missing all N wrong draws by chance is `(1-p)^N`; at
/// N=30 and the LOWER observed rate p≈0.4, that is ~2e-6). After the real fix lands
/// the defect is gone, not merely less likely, so all N runs pass with certainty —
/// this is not a coin-flip in either direction, unlike a bare single-run assertion
/// would be.
const NONDETERMINISM_SAMPLE_COUNT: usize = 30;

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

/// Build one fixture at one optimization tier into an isolated tmpdir and return the
/// built binary's path plus a ready-to-run `Command` for it.
fn build_fixture(fixture_name: &str, tier_args: &[&str]) -> (tempfile::TempDir, PathBuf) {
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
        "ynz build failed for {fixture_name}:\n{}",
        String::from_utf8_lossy(&build_out.stderr)
    );

    let binary = isolated_src.with_extension("");
    (tmp, binary)
}

/// C1 contract (probes A and D): exit 0, exact correct stdout, at ONE tier. Today
/// this is false at BOTH tiers for A and D, but the FAILURE SHAPE differs by tier
/// (SIGABRT at one, silent wrong value at the other) — asserting the single correct
/// contract independently per tier is honest about that rather than picking one
/// symptom to key on.
fn assert_tier_prints_correct(
    fixture_name: &str,
    tier_args: &[&str],
    tier_label: &str,
    expected: &str,
    cluster_label: &str,
) {
    let (_tmp, binary) = build_fixture(fixture_name, tier_args);
    let mut run_cmd = Command::new(&binary);
    run_cmd.env("CLICOLOR", "0");
    let run = run_with_watchdog(run_cmd);
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();

    assert!(
        run.status.success(),
        "{fixture_name} ({tier_label}): did not exit 0 (status: {:?}); stderr:\n{}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        stdout, expected,
        "{fixture_name} ({tier_label}): wrong stdout ({cluster_label})"
    );
}

fn assert_both_tiers_print_correct(fixture_name: &str, expected: &str, cluster_label: &str) {
    assert_tier_prints_correct(
        fixture_name,
        &["--no-optimize"],
        "O0 escape hatch",
        expected,
        cluster_label,
    );
    assert_tier_prints_correct(
        fixture_name,
        &[],
        "default optimized",
        expected,
        cluster_label,
    );
}

/// C1 contract (probes G and J): run the SAME `-O0` binary N times and require every
/// run to print the correct total. Non-flaky in the direction that matters: missing
/// the defect across N runs is a vanishing probability today, and a real fix makes
/// every run correct with certainty (see `NONDETERMINISM_SAMPLE_COUNT`'s doc comment).
fn assert_o0_always_correct(fixture_name: &str, expected: &str) {
    let (_tmp, binary) = build_fixture(fixture_name, &["--no-optimize"]);
    let mut wrong_runs = Vec::new();
    for i in 0..NONDETERMINISM_SAMPLE_COUNT {
        let mut run_cmd = Command::new(&binary);
        run_cmd.env("CLICOLOR", "0");
        let run = run_with_watchdog(run_cmd);
        let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
        if !run.status.success() || stdout != expected {
            wrong_runs.push((i, run.status, stdout));
        }
    }
    assert!(
        wrong_runs.is_empty(),
        "{fixture_name} (O0, {} runs): {} run(s) produced the wrong result (FRAGO 002 \
         cluster C1, non-deterministic at -O0 — a blocked channel send's resume read a \
         stale/uninitialised slot instead of re-reading the local). First offender: \
         {:?}",
        NONDETERMINISM_SAMPLE_COUNT,
        wrong_runs.len(),
        wrong_runs.first().unwrap()
    );
}

// LIVE REGRESSION LOCK since Phase 3 step 3.1 (dispatch hardening-p3.1-20260906-a1) —
// `collect_crossings_in_stmts` now scans EVERY suspending statement's own operands, so
// `rows` gets its frame slot. A red here is a real re-opening of FRAGO 002 cluster C1.
#[test]
fn frago002_c1_red_array_before_wait_channel_send() {
    // WHY: `rows` (array<int>) is declared before `wait sleep(20)` and read only by
    // `wire.send(rows)`, itself a suspension point. Observed 2026-09-06: SIGABRT
    // (misaligned pointer dereference) at the default optimized tier; "0" (wrong,
    // correct is "3") at -O0 — the poison read manifests differently per tier but is
    // wrong at both. Closes M8 FR #11(a).
    assert_both_tiers_print_correct(
        "v0_3_hardening_c1_array_before_wait_channel_send.ynz",
        "3\nend\n",
        "FRAGO 002 cluster C1 — a pre-suspension local was read through an \
         uninitialised/stale frame slot",
    );
}

// LIVE REGRESSION LOCK since Phase 3 step 3.1 (dispatch hardening-p3.1-20260906-a1) —
// the severity case of cluster C1 (no channel, no `background`, ordinary code). A red
// here is silent wrong output at exit 0 in the default mode; treat it as a stop-ship.
#[test]
fn frago002_c1_red_array_before_wait_suspending_call() {
    // WHY: `rows` is passed as a plain argument to `useRows`, whose OWN body
    // suspends (`wait sleep(1)`) — no channel, no `background`, no `.copy()`, no
    // `errors`. Observed 2026-09-06: "6\nend\n" (wrong, correct is "3\nend\n") at
    // the default optimized tier, exit 0; SIGABRT at -O0. This is the severity
    // finding FRAGO 002 ranks ahead of the channel-transfer shape — ordinary code,
    // silent wrong output, default mode. Closes M8 FR #11(a); also disproves
    // `fuzz_grammar/mod.rs::take_or_make_array`'s doc-comment claim that the defect
    // is "specific to the channel-transfer path."
    assert_both_tiers_print_correct(
        "v0_3_hardening_c1_array_before_wait_suspending_call.ynz",
        "3\nend\n",
        "FRAGO 002 cluster C1 — a pre-suspension local was read through an \
         uninitialised/stale frame slot",
    );
}

// LIVE REGRESSION LOCK since Phase 3 step 3.1 (dispatch hardening-p3.1-20260906-a1) —
// 30/30 correct at -O0 after the fix (was ~half wrong). A red here means a blocked
// send's resume is reading a stale slot again; the run count makes flakiness in the
// pass direction vanishingly unlikely, so one red run is a real defect.
#[test]
fn frago002_c1_red_number_local_blocking_channel_send() {
    // WHY: `price` (number) is read by three `wire.send(price)` statements into a
    // capacity-1 channel, so sends 2 and 3 block (a suspension). Observed
    // 2026-09-06: 30/30 correct ("10.2") at the default optimized tier (LLVM
    // apparently folds the poison load to something benign — labelled inference,
    // FRAGO 002; masked, not fixed), but non-deterministic at -O0 ("6.8" instead of
    // "10.2" in roughly half of runs observed here). This pin targets -O0 only,
    // where the defect is directly observable. Closes M8 FR #11(b).
    assert_o0_always_correct(
        "v0_3_hardening_c1_number_local_blocking_channel_send.ynz",
        "10.2\n",
    );
}

// LIVE REGRESSION LOCK since Phase 3 step 3.1 (dispatch hardening-p3.1-20260906-a1) —
// 30/30 correct at -O0 after the fix (was printing raw heap addresses in a majority of
// runs). Confirms in the tree that the discriminator was never the element type.
#[test]
fn frago002_c1_red_int_local_blocking_channel_send() {
    // WHY: int-local twin of the number probe (same producer, same shape) — added
    // because `fuzz_grammar/mod.rs` disagreed with itself about whether `int` was
    // ever confirmed safe (`FeedFn::send_count`'s comment said untested;
    // `stmt_background_drain_loop`'s said general to both int and number; parked
    // 49(b) relayed the wrong one). Observed 2026-09-06: 30/30 correct ("102") at
    // the default optimized tier; raw heap-address-looking integers printed instead
    // of "102" in a majority of -O0 runs observed here — confirms the
    // discriminator is whether a local is read by a statement that suspends, not
    // the element type. Closes M8 FR #11(b), int variant.
    assert_o0_always_correct(
        "v0_3_hardening_c1_int_local_blocking_channel_send.ynz",
        "102\n",
    );
}

// LIVE REGRESSION LOCK since Phase 3 step 3.2 (dispatch hardening-p3.2-20260906-a1) — the
// `AliasNoOp` arm is gone; `fixed<T>` copies its cells into fresh storage.
#[test]
fn frago002_c2_red_fixed_copy_aliases_source() {
    // WHY: `b = a.copy()` on a `fixed<int>` aliases `a`'s own storage; `b.set(0,
    // 99)` then mutates `a`. Observed 2026-09-06: "99" (wrong, correct is "1") at
    // BOTH tiers, deterministically — no transfer, no diagnostic, silent wrong
    // answer in the default mode. Shares a design ancestor (not a code line) with
    // M8 FR #9's bg-arg escape door. Patrick's 2026-09-06 ruling (plan.md Phase 3
    // step 3.2) required every type in `AliasNoOp` to become a real deep copy OR a
    // compile-time refusal. `fixed<T>` became a real copy — its cells are inline
    // values, so copying them into fresh storage IS an independent list — which is
    // why this stays a VALUE contract (`1`) rather than a refusal assertion. Closes
    // M8 FR #10.
    assert_both_tiers_print_correct(
        "v0_3_hardening_c2_fixed_copy_alias.ynz",
        "1\n",
        "FRAGO 002 cluster C2 — .copy()'s AliasNoOp arm returned the receiver's own \
         pointer instead of an independent copy",
    );
}
