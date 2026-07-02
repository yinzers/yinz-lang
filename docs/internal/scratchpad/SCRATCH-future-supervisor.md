---
name: "SCRATCH-future-supervisor"
description: "Design notes for Yinz stdlib supervisor helpers (e.g. supervise.alwaysRestart) that auto-restart panicking background tasks, locked for the v0.2 implementation."
tags:
  - "yinz-compiler"
created_at: "2026-05-14"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Supervisor — Stdlib Helpers + Default-Supervision Rule

**Status**: Locked, v0.2 implementation (ships alongside `background` and the scheduler).

User spec target: `spec/stdlib/supervisor.md` (to be created when stdlib lands).

---

## The Decision

Yinz stdlib provides supervisor helpers for the common patterns around `background` tasks. The meta-rule: **any stdlib API owning a long-running loop is supervised by default**, with explicit override available.

Supervisor helpers turn the [supervisor pattern](panic-safety.md#supervisor-pattern-no-trycatch-needed) into one-line stdlib calls. Without them, every server/worker/daemon would re-implement the same `while (true) { background ... }` loop with the same error handling.

---

## API surface

```ynz
// Most basic: spawn a task and restart it forever on panic.
supervise.alwaysRestart(processOrders)

// With observability hook for each panic.
supervise.alwaysRestart(processOrders, onPanic: (e: Panic) => log.error(e))

// Exponential backoff between restarts (prevents tight crash loops).
supervise.alwaysRestart(
  processOrders,
  backoff: duration.exponential(initial: 100.ms, max: 30.seconds, multiplier: 2),
)

// Limited restart count — give up after N panics in a window.
supervise.alwaysRestart(
  processOrders,
  maxRestarts: 10,
  maxRestartsWindow: 1.minute,
  onGiveUp: (e) => alerting.page("processOrders died permanently", e),
)

// One-shot: spawn the task, observe panics but do NOT restart.
let handle = supervise.watch(processOrders, onPanic: (e) => log.error(e))
// handle.cancel() to stop supervising; handle.wait() to block until task exits

// Pool of supervised tasks (for parallel workers).
supervise.pool(
  processOrders,
  count: 8,
  onPanic: (worker, e) => log.error("worker ${worker.id} crashed", e),
)
```

All configuration is via named parameters on a single call — no chaining. Names are dot-discoverable per Golden Rule 1.

---

## Default-supervision rule (meta)

Any stdlib API that owns a long-running loop is supervised by default. The user can override with explicit config, but the default behavior is safety-by-default. Examples:

- `server.listen(8080)` — request handlers run in supervised tasks (covered in [`docs/internal/scratchpad/SCRATCH-future-http-framework.md`](SCRATCH-future-http-framework.md))
- `queue.consume(handler)` — message processing runs in supervised tasks
- `file.watch(path, handler)` — fs event handlers run in supervised tasks
- `websocket.serve(handler)` — connection handlers run in supervised tasks

This is the meta-rule: **if a stdlib API spawns or holds tasks the user didn't write, those tasks are supervised**. The user doesn't have to remember to wrap them.

For comparison: in Rust/Tokio, you spawn a task and if it panics, you find out via the JoinHandle's Result. Most code never checks. In Node.js with Bull/BullMQ, you opt INTO error handlers and forget. In Yinz, the safe behavior is the default.

---

## API details

### `supervise.alwaysRestart(task)`

Spawns `task` as a `background` task. If the task panics, the supervisor:
1. Receives the panic via `task.onPanic` (already documented in `panic-safety.md`)
2. Logs the panic to the supervisor's `onPanic` callback if provided
3. Re-spawns the task (with backoff if configured)
4. Continues forever (or until `maxRestarts` exceeded)

If the task returns normally (no panic), the supervisor does NOT restart it. `alwaysRestart` is specifically for panic-recovery.

### `.withBackoff(initial, max, multiplier)`

Adds exponential backoff between restarts. Without it, a task that panics immediately on startup creates a tight crash loop hammering the runtime. Backoff applies after the FIRST panic; the first restart is immediate.

### `.maxRestarts(count, within: duration)`

Sliding-window restart limit. After `count` panics within `duration`, supervision gives up and calls `onGiveUp` if provided. Default: unlimited restarts (alwaysRestart lives up to its name).

### `onPanic:` / `onGiveUp:` named parameters

Observability hooks. `onPanic:` fires every time the supervised task panics (and gets restarted). `onGiveUp:` fires when `maxRestarts` is exceeded and supervision stops.

### `supervise.pool(task, count, onPanic:)`

Spawns `count` instances of `task` as supervised workers. Each worker is independently supervised — if one panics, only that one restarts, not the others. Common pattern for queue workers, request handlers, parallel processors.

### `supervise.watch(task, onPanic:)` — one-shot

Spawns a `background` task with a panic handler attached. Does NOT auto-restart — the task is allowed to die. Use when you want observability without restart (e.g., a fire-and-forget computation that you log on failure).

---

## What this is NOT

- **Not exception handling** — try/catch is rejected (see `panic-safety.md`). The supervisor pattern is about CONTAINMENT of bugs, not normal control flow.
- **Not a replacement for `errors`** — known failure modes use the `errors` keyword and auto-propagate. The supervisor handles UNKNOWN panics, not known errors.
- **Not process-level orchestration** — Docker/systemd/k8s handle process restart. Supervisors live INSIDE a process.
- **Not a generic message-passing framework** — Erlang's supervision trees are more elaborate (Actors, mailboxes, OTP). Yinz's stdlib supervisors are the 80% pattern that covers the 95% of use cases.

---

## v0.2 Implementation notes

- The supervisor builds on `background` task primitives (see [`docs/internal/implementation/IMP-no-function-coloring.md`](../implementation/IMP-no-function-coloring.md)).
- Restart logic lives in the supervisor itself, not the runtime — implemented as a Yinz stdlib type, not a special-cased compiler feature.
- Backoff uses a monotonic clock (won't drift on system-clock changes).
- The `onPanic` callback runs in the supervisor's task, not the panicked task — the panicked task is already dead.
- Pool semantics: if all workers panic simultaneously, backoff applies per-worker, not globally. Otherwise a single bad input could drain all 8 workers' restart budgets simultaneously.

The v0.2 milestone plan must address: cancellation (how does `supervise.alwaysRestart` stop?), task identity (how does `onPanic:` know which worker?), graceful shutdown (drain in-flight tasks before exiting?).

---

## Cross-references

- [`docs/internal/implementation/IMP-no-function-coloring.md`](../implementation/IMP-no-function-coloring.md) (`background` keyword, scheduler)
- [`docs/internal/scratchpad/SCRATCH-future-panic-safety.md`](SCRATCH-future-panic-safety.md) (task isolation, drop-on-unwind)
- [`docs/internal/scratchpad/SCRATCH-future-http-framework.md`](SCRATCH-future-http-framework.md) (HTTP server is supervised by default — canonical example of the meta-rule)
- `design/stdlib/` (where stdlib design lives; this doc is the supervisor contract)
