# Panic Safety — No Try/Catch, Task-Isolated Panics

**Status**: Locked, v0.2 implementation.

User spec target: `spec/errors.md` (already documents the `errors` keyword; this design adds the panic side).

---

## The Decision

Yinz has two failure mechanisms, and **NEITHER uses try/catch**:

1. **Errors** — KNOWN failure modes the caller MUST handle. Declared in function signatures with the `errors` keyword. Auto-propagate up the call stack until handled.
2. **Panics** — UNKNOWN/unexpected failures (bugs, invariant violations). NOT declared in signatures. Auto-isolate to the `background` task they happen in; bubble up to the task supervisor.

No `try { } catch { }` syntax. No mutex poisoning. Cleanup happens via drop-on-scope-exit during stack unwind. Supervisor pattern at the task boundary handles recovery.

---

## The Distinction — Errors vs Panics

The distinction is **intent and recoverability**, not "known vs unknown":

### Errors — expected, recoverable, caller has a choice

```yinz
function readFile(path: string) -> string errors {
  // file might not exist; caller can use a default, retry, or fail the request
}

function parseAge(input: string) -> int errors {
  // input might be invalid; caller can prompt the user
}
```

The function signature DECLARES the failure modes via `errors`. The caller MUST handle (or propagate). The `errors` keyword carries the error type-class through the type system.

Errors flow through normal control flow. No `try/catch` — errors auto-propagate:

```yinz
function loadUser(id: int) -> User errors {
  let data = readFile("user-${id}.json")   // if readFile fails, this returns the error
  let user = parseUser(data)               // same — auto-propagation
  return user
}
```

If the caller wants to handle a specific error instead of propagating, they use `is` pattern narrowing on the error variant (the v0.1 spec covers this in `spec/errors.md`).

### Panics — unexpected, unrecoverable in scope, indicates a BUG

```yinz
panic("market data desync detected — refusing to continue")  // explicit
let item = array.get(5)  // returns maybe T — the array might be too short
                         // (no panic; the user explicitly handles maybe)

// What CAN panic implicitly:
let result = 10 / divisor   // panics if divisor == 0 (programmer should have checked)
let x = invariant_was_supposed_to_hold   // hit "unreachable" code → panic
```

Panics are NOT in function signatures. They represent "the program reached a state where its assumptions are broken." The caller has no good recovery — recovery requires fresh state.

---

## NO try/catch — How recovery works without it

For ERRORS: `errors` keyword + auto-propagation. The function that wants to handle uses `is` to narrow, otherwise the error flows up. This already works in v0.1 spec.

For PANICS: **task isolation via `background`** + **supervisor pattern** + **automatic resource cleanup**.

### Task isolation

```yinz
function main() -> nothing {
  while (true) {
    let order = getNextOrder()
    background processOrder(order)  // each order runs in its own task
  }
}

function processOrder(order: Order) -> nothing {
  // If THIS panics:
  //   - just this task dies
  //   - all resources it held drop cleanly via stack unwind
  //   - the supervisor (whoever spawned this background task) is notified
  //   - main loop keeps running, continues processing other orders
}
```

A panic in a `background` task kills ONLY that task. Its resources (file handles, locks, allocated memory) get released via the standard drop-on-scope-exit mechanism during unwind. The main thread keeps running.

Main-thread panics are different: they terminate the process (exit code != 0). External orchestration (Docker, systemd, k8s) handles process restart per industry standard. The language doesn't build process-level restart in — that's the wrong layer.

### Resource cleanup via drop-on-scope-exit

When a panic unwinds the stack:

1. Each scope's owned values run their drop logic (file handles closed, locks released, memory freed, decrement-on-Arc, etc.)
2. Resources held by the panicking task are released BEFORE the supervisor is notified
3. No partial state lingers in the runtime; the next task sees a clean slate

This is automatic — same mechanism as scope-exit cleanup, just triggered by panic-unwind instead of normal return.

### Supervisor pattern (no try/catch needed)

Patterns from Erlang/BEAM and BullMQ. The parent of a `background` task gets notified on child panic:

```yinz
let task = background processOrder(order)
task.onPanic = (e: Panic) => {
  log.error("order processing crashed: ${e.message}")
  metrics.bump("order.panic")
  // task is already dead; we just observed and logged
}

// OR: use stdlib supervisor helpers
supervise.alwaysRestart(processOrders, onPanic: (e) => log.error(e))
// see design/future/supervisor.md for the full API
```

The supervisor doesn't "catch" the panic — the panic has already killed the task. The supervisor is notified AFTER cleanup and decides what to do next (log, restart, escalate, ignore).

---

## NO mutex poisoning

When a Rust thread panics holding a Mutex, Rust marks the mutex "poisoned" — subsequent lockers get an error indicating data MIGHT be corrupt. The annoyance/benefit ratio is poor in practice; most code unwraps the poison and continues anyway.

Yinz doesn't do this. Reasoning:

- Drop-on-scope-exit ALREADY releases the lock cleanly when the task panics.
- The locked data MIGHT be in a half-updated state, but that's a logic bug the programmer should fix, not a runtime safety mechanism. We trust the programmer to make state changes atomic (one full update under one lock acquire).
- Lints and IDE hints encourage atomic update patterns. Compile-time analysis can warn on multi-step update patterns inside locks.

Programs that need "did the previous holder die mid-update?" semantics can build it explicitly with a status flag — they're rare enough not to be the default.

---

## What about main()? — External orchestration

A panic in `main()` (or in any code path NOT inside a `background` task) terminates the process. The Yinz runtime prints the stack trace + WHAT/WHAT-INSTEAD/WHY (where applicable), then exits with non-zero exit code.

**Restart is NOT a language responsibility.** External orchestration (Docker, systemd, k8s, PM2, your hand-rolled supervisor script) handles process restart. This is the same model every modern language uses for production resilience — Go, Rust, Node, Python, Ruby. Language-level main-restart has problems:

1. Persistent corrupt state: if `main` crashed because of bad in-memory state, restarting with the same memory just crashes again. External restart gets a FRESH process.
2. Production needs more than restart: zero-downtime deploys, scaling, health checks, log aggregation, alerting — all handled by orchestrators, not languages.
3. Different deployments need different policies: restart-immediately vs restart-with-backoff vs alert-on-call-and-don't-restart. The language can't know which is right.

For the supervisor pattern WITHIN a process, see [`design/future/supervisor.md`](supervisor.md).

---

## Explicit non-design: `try { } recover { }` REJECTED

During the design-lockdown conversation, I (Claude) proposed a `try { } recover (e: Panic) { }` syntax for explicit panic recovery in scope. Patrick caught this as a re-introduction of try/catch under a different name and rejected it.

The supervisor pattern at the task boundary handles every legitimate recovery use case:

- Long-running server: each request in a `background` task; per-request panic doesn't kill the server.
- Daemon process: each worker in a `background` task; per-worker panic kills the worker only, supervisor restarts it.
- Trading bot: per-order processing in a `background` task; one bad order doesn't kill the bot.

`try { } recover` would add a SECOND recovery mechanism — task boundary AND in-scope catch. Two mechanisms doing the same thing violates Yinz's design philosophy ("one concept = one keyword"). Rejected.

**Graveyard Entry 5 catches re-introduction** of try/catch/recover tokens in the parser, AST, or spec docs. The mechanical enforcement exists specifically because this kind of decision gets re-relitigated in 6-month windows.

---

## What about JavaScript/PHP comparisons

- **JavaScript**: uncaught error anywhere can crash the whole process. Production-resilience comes from Docker/PM2/whatever restarting the process. Yinz is BETTER: panics auto-isolate to background tasks, only main-panic crashes the process.
- **PHP**: per-request process isolation. A fatal error in one request doesn't kill the server; other requests keep being served. Yinz achieves the same with the `background` + supervisor pattern but with cleaner semantics (no shared-nothing per-request process model required).

Yinz with the supervisor pattern + auto-isolated background tasks gives you PHP-like per-request resilience inside a single process, plus orchestrator-level process restart for the catastrophic case. The two layers handle different failure scales.

---

## Implementation notes (v0.2)

- Panic unwind: standard libunwind-style stack unwinding. Drop runs for every owned value as the stack pops.
- Supervisor notification: each `background` task has a handle the parent can attach `onPanic` to. Panic notifications go through the channel/queue primitives the stdlib provides.
- Performance: panic itself is slow (stack unwind isn't free). Normal control flow has zero overhead — no `try/catch` infrastructure that costs even for non-panicking code.
- Codegen: panic call sites get a special unwind table entry. The runtime walks the table during unwind.

The v0.2 milestone plan must include these in `### Performance` and `### Runtime Dependencies` invariants.

---

## Cross-references

- [`spec/errors.md`](../../spec/errors.md) (user spec for the `errors` keyword)
- [`design/errors.md`](../errors.md) (design rationale for `errors`-keyword auto-propagation, why no try/catch)
- [`design/future/concurrency.md`](concurrency.md) (the `background` keyword + scheduler)
- [`design/future/supervisor.md`](supervisor.md) (stdlib supervisor helpers)
- [`design/ownership.md`](../ownership.md) (drop-on-scope-exit cleanup during unwind)
- `.claude/graveyard.md` Entry 5 (mechanical enforcement of try/catch rejection)
