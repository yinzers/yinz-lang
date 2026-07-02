---
name: "SCRATCH-stdlib-network"
description: "Two paired modules, both speaking HTTP, paired by protocol but split by direction so neither name is ambiguous about which side you're on."
tags:
  - "yinz-compiler"
created_at: "2026-05-20"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Standard Library — Network (request + server)

Two paired modules, both speaking HTTP, paired by protocol but split by direction so neither name is ambiguous about which side you're on.

| Module | Direction | Ships in |
|---|---|---|
| `request` | Outbound — your program reaches out to other servers | v0.15 |
| `server`  | Inbound — your program serves endpoints for other clients | v0.21 |

Both modules share the `Request` / `Response` types (capital — types per Rule 13), since HTTP messages are direction-agnostic at the wire level. The `request` module BUILDS Requests and CONSUMES Responses; the `server` module CONSUMES Requests and BUILDS Responses.

Early design — finalized API shape locks at v0.15 (request) and v0.21 (server). What follows is the direction-of-travel, not the committed surface.

---

## `request` — outbound HTTP (v0.15)

All operations use the `errors` system — handle or propagate.

### High-level helpers (tier 1)

```
let response = request.get(`https://api.example.com/users`)
if (response.failed()) {
  log(response.message)
  return
}
let users = response.json()

let response = request.post(`https://api.example.com/users`, {
  body: { name: `Alice`, email: `alice@example.com` },
  headers: { authorization: `Bearer token123` }
})

request.put(url, options)
request.delete(url, options)
request.patch(url, options)
request.websocket(url)
```

### Mid-level builder (tier 2)

For cases the helpers don't cover (custom methods, complex headers, retries, timeouts). Step-by-step on a named variable — no chaining (Golden Rule 7):

```
let req = request.build()
req.method(`PATCH`)
req.url(`https://api.example.com/users/42`)
req.header(`authorization`, `Bearer token`)
req.body({ status: `active` })
req.timeout(5)
let response = req.send()
```

### Low-level socket access (tier 3)

```
let socket = net.tcp.connect(host, port)
```

Raw socket access. The floor of the user-accessible network stack — anything lower is FFI territory (v2+).

---

## `server` — inbound HTTP (v0.21)

Module-level functions on the singleton `server` namespace. Routes register at module scope, then `server.listen()` starts accepting connections.

```
server.route(`GET`, `/users`, listUsers)
server.route(`POST`, `/users`, createUser)
server.route(`GET`, `/users/:id`, getUser)
server.middleware(authCheck)
server.listen(3000)
```

Handler signature:

```
function listUsers(req: Request) -> Response errors {
  let users = loadUsers()
  return Response.json(users)
}
```

---

## Expansion candidates (post-v0.21)

- WebSocket support beyond the initial `request.websocket(url)` (full bidirectional protocol, framing)
- Middleware composition patterns (ordering, short-circuiting)
- Request/response streaming (chunked transfer, server-sent events)
- File upload handling
- Cookie management (parse/set helpers on both directions)
- CORS configuration on `server`
- HTTPS/TLS certificate configuration
- Rate limiting helpers
- Retry logic with backoff (likely on the `request` builder)
- Multi-server-per-process (if the single-server assumption breaks)

---

## v0.15+ Async I/O Surface

The canonical deferral spec for async network operations lives in the feature registry:

```
registry/features.toml → [[deferred_tooling_feature]] name = "async-io-stdlib-intrinsics-v0-5"
```

That registry entry is the SSOT — this section is a cross-reference, not the authority.

**Planned async surface (ships with v0.15 http module)**:

- `request.getAsync(url) -> Response errors` — non-blocking HTTP GET; `wait request.getAsync(url)` suspends the caller while the network round-trip completes.
- `request.postAsync(url, options) -> Response errors` — non-blocking HTTP POST.
- Equivalent `Async` variants for the other HTTP methods.

Async server handlers (inbound) will use the same `wait` pattern — a handler declared `-> Response errors` that calls `wait db.queryAsync(...)` will be compiled as a state machine and suspend per connection without blocking an OS thread per request.

**Why deferred**: connection pooling design, TLS configuration surface, error variant taxonomy (network timeout vs DNS failure vs TLS error), and request/response streaming all belong to the v0.15 milestone where the full HTTP surface is designed. The state-machine ABI that makes `wait` work was validated in v0.3-M2.
