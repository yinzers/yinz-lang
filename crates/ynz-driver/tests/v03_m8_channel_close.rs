// v0.3-M8 Phase 4 — channel close semantics, the transfer rule, fr12 marshalling, and the
// runtime-side P2-3 fix, locked end-to-end by `.ynz` fixtures.
//
// Plan: `.claude/planning/active/2026-07-04-v0-3-m8-concurrency-completion/plan.md` Phase 4
// (steps 3, 3a, 3b, 3d, 4, 5). Designs: `IMP-concurrency.md` "Channel Close — End-of-Stream
// Semantics" and `IMP-ownership.md` "Transfer — Who Else Holds This Value" (both signed
// 2026-09-03, `audit.md`'s two SIGN-OFF records).
//
// RED-commit protocol (plan step 3a / step 5): every test here was authored and run RED
// BEFORE the implementation landed — each fixture's header records the observed RED reason —
// and flips GREEN in the commits that land the mechanism. Do NOT weaken an assertion, widen
// the watchdog, or drop a parity gate: a red here is a hang, a leak, a double free, or a
// use-after-free in the flagship concurrency surface.
//
// One named test per fixture (no fixture loop — `.claude/rules/test-parallelism.md`), so the
// runner parallelizes them and names the failing case.

use std::{
    path::PathBuf,
    process::{Command, Output},
    time::Duration,
};

/// Liveness ceiling for one compiled fixture run. A trip is a hang (a receiver that was never
/// woken by `close()`, a parked send that never landed) — never a slow test. Generous by an
/// order of magnitude over the observed run so a loaded parallel lane cannot false-trip it.
const RUN_WATCHDOG: Duration = Duration::from_secs(120);

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
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
            Some(_) => {
                return child
                    .wait_with_output()
                    .expect("watchdog: failed to collect child output");
            }
            None => {
                if start.elapsed() >= RUN_WATCHDOG {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "WATCHDOG TRIP: fixture did not exit within {RUN_WATCHDOG:?} — a hang \
                         (a receiver `close()` never woke, or a parked send that never landed), \
                         never a slow test; fix the runtime, never widen this timeout."
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// `ynz run <fixture>` → (stdout, stderr, exit code).
fn ynz_run(name: &str) -> (String, String, i32) {
    let mut cmd = Command::new(ynz_binary());
    cmd.args(["run", fixture(name).to_str().expect("utf-8 fixture path")])
        .env("CLICOLOR", "0");
    let out = run_with_watchdog(cmd);
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

/// Run a fixture with the runtime's alloc counter on and return (alloc, free). The counter
/// env vars are read by the RUNTIME at process start (`ynz_rt_init`), so they reach the
/// compiled program through `ynz run` unchanged.
fn ynz_run_with_alloc_counter(name: &str) -> (u64, u64) {
    let count_file = std::env::temp_dir().join(format!(
        "ynz_m8_p4_alloc_{name}_{}.txt",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&count_file);
    let mut cmd = Command::new(ynz_binary());
    cmd.args(["run", fixture(name).to_str().expect("utf-8 fixture path")])
        .env("CLICOLOR", "0")
        .env("YNZ_ALLOC_COUNTER", "1")
        .env(
            "YNZ_ALLOC_COUNTER_OUTPUT",
            count_file.to_str().expect("utf-8 count path"),
        );
    let _ = run_with_watchdog(cmd);
    let content =
        std::fs::read_to_string(&count_file).unwrap_or_else(|_| "alloc=0\nfree=0\n".to_string());
    let _ = std::fs::remove_file(&count_file);
    let parse = |prefix: &str| -> u64 {
        content
            .lines()
            .find(|l| l.starts_with(prefix))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    };
    (parse("alloc"), parse("free"))
}

/// Exit 0 + byte-exact stdout.
fn assert_runs(name: &str, expected_stdout: &str) {
    let (stdout, stderr, code) = ynz_run(name);
    assert_eq!(
        code, 0,
        "{name}: must exit 0 (a compile error here = the surface did not ship; a signal = a \
         use-after-free or double free); stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, expected_stdout,
        "{name}: exact expected stdout; stderr:\n{stderr}"
    );
}

/// Exit 0 + byte-exact stdout + exact alloc/free parity: `alloc == free + expected_gap`, where
/// the gap is the set of heap values still HELD at exit (no scope-exit drop pass exists, so a
/// received or buffered value is held to exit by its binding or by the channel). A smaller gap
/// is a double free (the ladder, the glue, or `refuse_closed` freed a value someone still
/// holds); a larger gap is a leak (P2-3's class).
fn assert_runs_with_parity(name: &str, expected_stdout: &str, expected_gap: u64, gap_explained: &str) {
    assert_runs(name, expected_stdout);
    let (alloc, free) = ynz_run_with_alloc_counter(name);
    assert!(
        alloc > 0,
        "{name}: alloc=0 — the counter saw nothing, parity is vacuous"
    );
    assert_eq!(
        alloc,
        free + expected_gap,
        "{name}: alloc must equal free + {expected_gap} ({gap_explained}); smaller = a double \
         free, larger = a leak; alloc={alloc} free={free}"
    );
}

// ── step 3a: `.copy()` on `map<K, V>` ────────────────────────────────────────

#[test]
fn m8_p4_map_copy_is_independent_of_the_original() {
    // WHY: the independence lock committed RED on the alias no-op (copy observed `b: 99`).
    assert_runs(
        "v0_3_m8_p4_map_copy_independent.ynz",
        "original b: 99\ncopy b: 20\noriginal count: 3\ncopy count: 2\n",
    );
}

// ── steps 2/3: the close mechanism + `receive()` → `maybe<T>` ────────────────

#[test]
fn m8_p4_close_then_receive_drains_then_none() {
    assert_runs("v0_3_m8_p4_close_drain_then_none.ynz", "total: 10\nend\n");
}

#[test]
fn m8_p4_double_close_is_a_safe_no_op() {
    assert_runs(
        "v0_3_m8_p4_close_double_idempotent.ynz",
        "1\nclosed twice ok\n",
    );
}

#[test]
fn m8_p4_send_after_close_is_the_typed_runtime_error() {
    assert_runs("v0_3_m8_p4_send_after_close_refused.ynz", "refused\nend\n");
}

#[test]
fn m8_p4_concurrent_send_during_close_is_linearized_at_the_sender_lock() {
    // WHY: delivered == received under EVERY interleaving; the fixture never asserts the
    // ordering the implementation does not provide (a `try_send` after `close()` returned
    // may still land if its clone was taken first — obligation (5)).
    assert_runs(
        "v0_3_m8_p4_close_concurrent_send_linearized.ynz",
        "parity ok\nend\n",
    );
}

#[test]
fn m8_p4_close_wakes_a_parked_receiver() {
    // WHY: liveness — the watchdog is the gate for "stayed parked".
    assert_runs(
        "v0_3_m8_p4_close_wakes_parked_receiver.ynz",
        "woken by close\n",
    );
}

#[test]
fn m8_p4_in_flight_pre_close_send_lands() {
    assert_runs("v0_3_m8_p4_close_inflight_send_lands.ynz", "1\n2\nend\n");
}

#[test]
fn m8_p4_drop_without_close_is_unchanged() {
    assert_runs("v0_3_m8_p4_drop_without_close_unchanged.ynz", "10\n");
}

// ── the `h.receive()` `-> maybe<T>` precondition (audit.md, known-live bug) ──

#[test]
fn m8_p4_handle_receive_of_a_plain_maybe_return_does_not_segfault() {
    // WHY: RED today with exit 139 — `HANDLE_RET_KIND_VALUE_WORD` hands the parent a
    // dead-frame address for a `{tag, payload}` aggregate.
    assert_runs("v0_3_m8_p4_handle_maybe_return.ynz", "7\ndone\n");
}

#[test]
fn m8_p4_handle_receive_of_a_maybe_errors_return_stays_correct() {
    // WHY: the errors-capable twin — GREEN today; locked so the fix is judged on both shapes.
    assert_runs("v0_3_m8_p4_handle_maybe_errors_return.ynz", "7\ndone\n");
}

// ── the owned-heap element class: `channel<array<int>>` ───────────────────────

#[test]
fn m8_p4_chan_array_roundtrip_receiver_is_sole_holder() {
    assert_runs_with_parity(
        "v0_3_m8_p4_chan_array_roundtrip.ynz",
        "3\n10\n30\nend\n",
        2,
        "2 = the task-built array now held by the receiver's `got`",
    );
}

#[test]
fn m8_p4_chan_array_send_after_close_frees_the_refused_payload_once() {
    // WHY: P2-3 fixed in the runtime's `refuse_closed` — release + glue, exactly once.
    assert_runs_with_parity(
        "v0_3_m8_p4_chan_array_send_after_close.ynz",
        "refused\nend\n",
        0,
        "the refused array was consumed at typeck and freed by refuse_closed's glue",
    );
}

#[test]
fn m8_p4_chan_array_buffered_at_exit_is_held_by_the_channel() {
    assert_runs_with_parity(
        "v0_3_m8_p4_chan_array_drop_with_buffered.ynz",
        "buffered\n",
        2,
        "2 = the buffered array held by the never-torn-down channel",
    );
}

// ── the owned-heap element class: `channel<map<string, int>>` ─────────────────

#[test]
fn m8_p4_chan_map_roundtrip_both_task_built_and_give_bg_arg() {
    assert_runs_with_parity(
        "v0_3_m8_p4_chan_map_roundtrip.ynz",
        "2\n2\n3\n9\nend\n",
        10,
        "5 = the task-built map held by `first`'s value, 5 = the given spawner map held by \
         `second`'s value (header + ctrl + keys + vals + insert_order each)",
    );
}

#[test]
fn m8_p4_chan_map_send_after_close_frees_the_refused_payload_once() {
    assert_runs_with_parity(
        "v0_3_m8_p4_chan_map_send_after_close.ynz",
        "refused\nend\n",
        0,
        "the refused map was consumed at typeck and freed by refuse_closed's glue (ynz_map_drop)",
    );
}

#[test]
fn m8_p4_chan_map_buffered_at_exit_is_held_by_the_channel() {
    assert_runs_with_parity(
        "v0_3_m8_p4_chan_map_drop_with_buffered.ynz",
        "buffered\n",
        5,
        "5 = the buffered map held by the never-torn-down channel",
    );
}

// ── the ownership-flow class ─────────────────────────────────────────────────

#[test]
fn m8_p4_flow_give_parameter_carries_ownership_through_a_call() {
    assert_runs("v0_3_m8_p4_flow_give_through_call.ynz", "3\nend\n");
}

#[test]
fn m8_p4_flow_two_hop_relay_with_give_on_both_frames() {
    assert_runs("v0_3_m8_p4_flow_two_hop_give.ynz", "4\nend\n");
}

#[test]
fn m8_p4_flow_every_admitted_payload_form_sends() {
    assert_runs_with_parity(
        "v0_3_m8_p4_flow_admitted_forms.ynz",
        "3\n4\n4\n2\n0\n5\nend\n",
        12,
        "6 received arrays × 2 counted allocs each, held by the consumer at exit",
    );
}

#[test]
fn m8_p4_reassigning_a_consumed_binding_revives_it() {
    // WHY: RED today — refused "already given away" (a correct program rejected).
    assert_runs("v0_3_m8_p4_revive_on_reassign.ynz", "3\n2\n4\n5\n");
}

// ── step 3d: fr12 — `channel<number>` decimal128 marshalling ──────────────────

#[test]
fn m8_p4_number_channel_round_trips_through_a_receive_side_suspension() {
    assert_runs_with_parity(
        "v0_3_m8_p4_number_chan_roundtrip.ynz",
        "2.5\n2.5\nend\n",
        0,
        "two 16-byte cells minted at the sends, two freed at the receives",
    );
}

#[test]
fn m8_p4_number_channel_send_after_close_frees_the_minted_cell() {
    assert_runs_with_parity(
        "v0_3_m8_p4_number_chan_send_after_close.ynz",
        "refused\nend\n",
        0,
        "the refused cell freed by refuse_closed through ynz_number_cell_free",
    );
}

#[test]
fn m8_p4_number_channel_buffered_cell_is_held_by_the_channel() {
    assert_runs_with_parity(
        "v0_3_m8_p4_number_chan_drop_with_buffered.ynz",
        "buffered\n",
        1,
        "1 = the buffered 16-byte cell held by the never-torn-down channel",
    );
}

#[test]
fn m8_p4_number_channel_parked_send_is_drained_after_close() {
    assert_runs_with_parity(
        "v0_3_m8_p4_number_chan_parked_send_drained_after_close.ynz",
        "1.5\n2.5\nend\n",
        0,
        "two cells minted (one parked), two freed at the receives",
    );
}
