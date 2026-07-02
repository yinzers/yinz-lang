---
name: "R5-composed-suspension-spike-report"
plan-id: "2026-07-02-v0-3-m4-channels-arc-release"
phase: "Phase 0"
session-id: "executor-2026-07-02-m4-p0"
created_at: "2026-07-02"
metadata:
  type: "spike-report"
---

# R5 Composed-Suspension Spike Report (v0.3-M4 Phase 0)

**Verdict: S1 GREEN · S2 GREEN · S3 GREEN — no STOP condition triggered; dormant override #2 NOT armed; the plan proceeds.**

## What the spike proves

R5 is the composed-suspension deadlock risk: a `background`-spawned CHILD suspends on
`channel.send()`-on-full WHILE the PARENT polls `channel.receive()` on its handle. The feared
failure is a circular wait — the child (blocked on send) waits for the parent to receive, and the
parent (blocked on receive) is never woken. Three confirmed trap doors could produce it: modelling
the child as a run-once `CpuJoinHandle` closure (1a), re-polling the one-shot `ynz_rt_join_poll`
(1b), or storing the channel endpoint future in the type-punned `sleep_handle` frame slot (1c).

The spike executed the composed scenario **through the real compiler** (`ynz run` on real `.ynz` —
never a hand-written Rust model, per the M2-spike false-ACCEPT lesson) on the design Phase 1/2 will
ship:

- an **independent joinable Tokio task** for the child (`tokio::spawn`) — not a `CpuJoinHandle`;
- **real `tokio::sync::mpsc` endpoint futures** (`Sender::send`, `Receiver::poll_recv`) polled with
  the **forwarded waker** from the enclosing `SpawnStateFnFuture::poll` (the
  `ynz_rt_async_sleep_poll` no-fabricated-waker discipline);
- endpoint futures owned by the **handle/runtime object** — never the frame's `sleep_handle` slot,
  which stayed NULL throughout (so the cancellation drop path is a safe no-op).

The two mpsc endpoints wake each other independently (Tokio registers the receiver's waker on empty
and the sender's waker on full), so there is no circular wait.

## How it was driven through the real compiler (throwaway mechanism — torn down)

Channel + handle syntax does not exist until Phase 1/2, so the scenario was reached WITHOUT any
durable codegen/typeck change:

- A real `.ynz` program wrote `wait sleep(SPIKE_SENTINEL_MS)` — lowered exactly like any
  `wait sleep(ms)`: a genuine state-machine suspension point whose poll forwards the enclosing
  task's real waker.
- Throwaway runtime shims (`crates/ynz-runtime/src/spike_composed.rs`, plus two sentinel branches in
  `ynz_rt_async_sleep_create` / `ynz_rt_async_sleep_poll`) detected the sentinel and drove the
  composed mpsc scenario instead of arming a timer. `spike_create` returned a NULL handle, so the
  parent stored NULL in `sleep_handle` (trap door 1c avoided by construction); the endpoints lived
  in a runtime-owned `SpikeHandle`.

All of this scaffolding was **torn down** after the run (spike discipline). The two persisted
artifacts are this report and [`composed-scenario.ynz`](composed-scenario.ynz) (the composed
scenario in intended channel syntax — the seed Phase 2 grows into a real build-blocking fixture).

## Verdicts (measured, through `ynz run`, 30s deadlock-guard timeout)

| Verdict | Criterion | Result |
|---|---|---|
| **S1** | Composed scenario completes — no deadlock/hang, timing-verified suspension | **GREEN** |
| **S2** | No synchronous blocking call in the executed send/recv/handle path (grep-audit) | **GREEN** |
| **S3** | No trap door engaged (no `CpuJoinHandle` model, no `join_poll` re-poll, no `sleep_handle` tenancy — endpoints in the handle object) | **GREEN** |

### S1 — measured run output

```
SPIKE-R5: composed scenario completed (no deadlock/hang)
SPIKE-R5:   received_sum            = 30 (expected 30)
SPIKE-R5:   parent_pending_count    = 2 (>=1 ⇒ parent genuinely suspended)
SPIKE-R5:   child_send_suspended    = true (true ⇒ child suspended on send-on-full)
SPIKE-R5:   send2_after_first_recv  = true (true ⇒ composed-suspension ordering held)
SPIKE-R5:   elapsed                 = 26.470629ms
SPIKE-R5:   S1 verdict              = GREEN
spike-parent-resumed
----
exit_code=0  wall_ms=407
```

- `received_sum = 30` — the parent observed BOTH values (10 + 20); the full composed handshake
  completed.
- `parent_pending_count = 2` — the parent genuinely SUSPENDED twice and was re-woken via the
  forwarded waker (the poll-yield path was exercised, not short-circuited).
- `child_send_suspended = true` — the child genuinely suspended on the full (capacity-1) channel
  (`try_send` failed before the awaiting `send`).
- `send2_after_first_recv = true` — the child's blocked second send completed ONLY AFTER the parent
  received the first value and freed capacity. **This ordering is the composed-suspension proof:**
  the two endpoints made progress by waking each other, not by a blocking wait.
- Clean exit 0 in ~407ms wall (≈26ms scenario + process startup) — no hang; the 30s deadlock guard
  never fired.

### S2 — grep-audit (executed composed path clean)

The channel operations in the executed path use only `poll_recv` (non-blocking poll with the
forwarded waker), `.await` (async suspension inside the spawned child task), and `try_send` /
`send().await`. No `block_on`, `blocking_recv`, `blocking_send`, `spawn_blocking`, `thread::sleep`,
or `.park()` in the send/recv/handle path. (The program's single top-level `block_on` is the
`ynz_rt_run_entrypoint` program-entry driver present for ALL Yinz programs — not a blocking call in
the channel path.)

### S3 — trap doors structurally absent

- **1a avoided** — the child is a `tokio::spawn` independent joinable task
  (`tokio::task::JoinHandle`), never a run-once `CpuJoinHandle` closure.
- **1b avoided** — the parent drives via `Receiver::poll_recv`, never `ynz_rt_join_poll` (no
  one-shot re-poll).
- **1c avoided** — `spike_create` returned NULL; the parent's `sleep_handle` slot held NULL
  throughout; the endpoint future (`mpsc::Receiver`) lived in the runtime-owned `SpikeHandle`. The
  cancellation drop path (`runtime.rs` `FrameDropGuard`, guarded by `if !handle_ptr.is_null()`)
  is therefore a safe no-op.

## Design lock resolved by the spike

**Lock 11 (Assumption 11) — frame-header slot verdict: NO new frame-header slot is forced by the
runtime architecture.** The spike proved the composed scenario runs deadlock-free with the endpoint
futures living OUTSIDE the frame, in the handle/runtime object (`sleep_handle` stayed NULL). Phase 2
must persist a single opaque handle pointer across the parent's suspension; that pointer must live
in a DEDICATED slot (never the type-punned `sleep_handle` — trap door 1c), but whether that
dedicated slot is a NEW frame-header slot or a crossing-local slot is a Phase 2 codegen decision,
not a runtime-architecture requirement. The spike deliberately sidestepped frame-slot persistence
(it used a runtime-owned registry) to isolate the runtime-architecture question; the
`FRAME_HEADER_SIZE` / `SPIKE_HANDLE_BASE_OFFSET` / `FrameLayout` ripple is named-but-not-forced
Phase 2 work — it becomes real only if Phase 2's codegen picks a new header slot as the cleanest
persistence.

## Reviewer note (no self-graded ACCEPT)

Per the plan's Phase 0 reviewer fan-out, the adversarial-tester must independently audit these
verdicts against the executed run — this report is the producer's honest record, not a self-graded
pass. The composed `.ynz` seed and the exact throwaway mechanism are documented above so the audit
can reproduce the run before the scaffolding was torn down (re-create `spike_composed.rs` + the two
sentinel branches from git history / this report if re-execution is required).
