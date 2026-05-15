# Open Questions

Design decisions that haven't been made yet. When one is resolved, move it to its dedicated design file and remove the entry here.

Resolved questions are NOT listed here — see `design/decisions.md` for the index of resolved topics.

Deferred features (decided not to ship in v0.1) are documented per-version in `design/mvp-scope.md` under the version where they're planned to land.

---

## HTTP Module Design — Three-Tier API

When v0.15 (`http` client) comes up for implementation, the module needs a focused design session. The general shape is locked but specifics need work:

**Tier 1 — High-level helpers:** `http.get(url)`, `http.post(url, body)`, `http.put`, `http.delete`, `http.websocket(url)`. The common-case API, dot-method-first.

**Tier 2 — Mid-level request builder:** `http.request().method("GET").header(...).timeout(5).send()` for cases that don't fit the simple helpers. Step-by-step style — every operation gets its own line.

**Tier 3 — Low-level socket access:** `net.tcp.connect(host, port)` returning a raw socket. The floor of the user-accessible network stack. Framework authors can build their own routing layer, their own protocol implementations, anything on top of this. Going lower means FFI (deferred to v2+).

**Open sub-questions for the v0.15 design session:**

- Exact dot-method names for the high-level tier (`http.get` vs `http.fetch` vs `http.request`?)
- WebSocket lifecycle — connect, message events, close handling. Builds on FallibleIterable contract per `design/iterables.md`?
- TLS configuration — defaults to validating certs; how to opt out for self-signed in dev?
- Streaming bodies — for large uploads/downloads, can the body be an iterable?
- Cookie handling — first-class or always manual headers?
- Proxy support — env var detection (`HTTP_PROXY`) automatic, or always explicit?

These get answered when v0.15 is up to design. Not blockers for v0.1-v0.14.

---

