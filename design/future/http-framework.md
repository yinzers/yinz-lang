# HTTP Framework — Supervision by Default

**Status**: Locked, v0.3+ implementation. Ships after v0.2 stdlib basics (concurrency, supervisor, allocators) are in.

User spec target: `spec/stdlib/http.md` (when implemented).

---

## The Decision

The Yinz stdlib HTTP server is **supervised by default**. Request handlers run in isolated `background` tasks. The accept loop is itself supervised — if it crashes, the framework restarts it. The user can override the supervision policy with explicit config, but the default is production-grade resilience.

This applies the [supervisor meta-rule](supervisor.md#default-supervision-rule-meta): any stdlib API owning a long-running loop is supervised by default. HTTP is the canonical example.

---

## Why this matters

Most production HTTP servers want this exact behavior:

- One request handler panics → 500 to that client, server keeps serving others
- Accept loop crashes (rare) → framework restarts it
- All panics logged with stack trace + WHY (per Golden Rule 11)
- Custom panic handlers possible (alerting, metrics, structured logging)

Every HTTP framework in every language eventually grows this. In Yinz, it's the default — the user gets resilience without remembering to wire it up. Per the BullMQ pattern Patrick referenced during the design-lockdown conversation: BullMQ workers rarely crash their host process because the framework handles isolation; Yinz HTTP should match that.

---

## API surface

### Basic (supervised by default)

```ynz
let server = http.listen(8080)
server.handle("/users", getUsers)
server.handle("/orders", placeOrder)
```

Behind the scenes:
- Each request handler runs in a `background` task (isolation per request)
- A request handler panic kills only that task; 500 sent to client; server keeps accepting
- The accept loop itself is supervised — restart on crash with backoff

The IDE shows muted hints indicating the supervision policy:

```ynz
let server = http.listen(8080)
//          ^ muted: " (supervised: per-request isolation, restart accept loop on panic)"
//            hover-tooltip explains the default and shows how to customize
```

### Custom supervision policy

```ynz
let server = http.listen(8080, supervise: {
  onPanic: (e: Panic, request: Request) => {
    alerting.page("HTTP handler crashed", e, request.method, request.path)
  },
  maxAcceptRestarts: 100,
  acceptBackoff: 1.second,
})
```

The `supervise` config option overrides specific behaviors. Unspecified fields fall back to defaults. The IDE hint changes when custom config is provided (cautionary red-tint if the user disables supervision entirely).

### Disabling supervision (advanced — strongly discouraged)

```ynz
let server = http.listen(8080, supervise: false)
//                                        ^ panics now bubble up to main
```

The IDE flags this as cautionary (red-tinted) muted hint with a tooltip explaining the implications. We don't BLOCK it (some advanced use cases want raw control), but we make it obvious.

---

## What gets supervised

| Component | Supervision | Failure mode |
|-----------|-------------|--------------|
| Accept loop | Yes (restart on panic, with backoff) | If accept itself panics (rare), framework restarts the listener |
| Per-request handler | Yes (isolated `background` task) | Panic → 500 response, server keeps accepting |
| Background middleware tasks | Yes (when registered as `background middleware`) | Panic in middleware doesn't kill the request |
| WebSocket connections | Yes (per-connection task) | Connection panic doesn't kill other connections |
| Server-Sent Events streams | Yes (per-stream task) | Stream panic doesn't kill other streams |

---

## Default 500 handler

When a request handler panics, the default response:

```
HTTP/1.1 500 Internal Server Error
Content-Type: text/plain

Internal Server Error
Request ID: <correlation-id>
```

In dev mode (`ynz run` not `ynz build --release`), the response includes the panic message and stack trace for debugging. In release mode, only "Internal Server Error" + request ID is returned — full panic info goes to the server log.

Users can override the default 500 handler:

```ynz
let server = http.listen(8080, on500: (e: Panic, request: Request) -> Response {
  // custom error response — log to monitoring, return a custom error page, etc.
})
```

---

## Cross-references to other stdlib patterns

The HTTP framework is the canonical EXAMPLE of the [supervisor meta-rule](supervisor.md). Other stdlib APIs that follow the same pattern:

- `queue.consume(handler)` — message processing tasks supervised
- `file.watch(path, handler)` — fs event handlers supervised
- `websocket.serve(handler)` — connection handlers supervised
- `cron.schedule(spec, handler)` — scheduled handlers supervised
- `stream.subscribe(handler)` — stream processors supervised

Each of these gets a similar API: default supervision, optional `supervise:` config, IDE muted hints showing the policy.

---

## v0.3+ Implementation notes

Depends on v0.2 stdlib basics (concurrency, supervisor helpers, allocators).

The v0.3+ milestone plan must address:

- Protocol support: HTTP/1.1 baseline, HTTP/2 for v1.0?, HTTP/3 (QUIC) as a separate milestone?
- TLS: built-in via system libraries, or stdlib? `https.listen()` separate from `http.listen()`?
- Routing: simple `server.handle("/path", fn)` for v0.3, more advanced patterns (path params, middleware chains) later?
- Performance: zero-copy from network buffer to handler? Sendfile for static assets?
- Static file serving: bundled in `http`, or separate `static.serve()` module?
- Middleware ordering: how does user compose multiple middleware? Chain syntax? Decorator-style?

These are stdlib design questions — `design/stdlib/http.md` is the right place for the detailed design. This doc is the supervision-by-default contract; everything else can be designed when the milestone is scheduled.

---

## Why this is v0.3+, not v0.2

v0.2 ships: stdlib basics, concurrency, supervisor helpers, allocators. An HTTP framework needs all of those plus protocol parsing, routing, TLS, etc. Adding it to v0.2 would balloon scope.

Locking the SUPERVISION-BY-DEFAULT design now (v0.1) means when v0.3 starts on HTTP, the supervision question is already answered. The v0.3 milestone plan can focus on protocol, routing, TLS — not re-relitigate supervision.

---

## Cross-references

- [`design/future/supervisor.md`](supervisor.md) (the stdlib supervisor helpers HTTP uses)
- [`design/future/concurrency.md`](concurrency.md) (the `background` keyword + scheduler HTTP requires)
- [`design/future/panic-safety.md`](panic-safety.md) (task isolation, drop-on-unwind that HTTP relies on)
- [`design/ide-hints.md`](../ide-hints.md) (muted supervision-policy rendering)
