# Standard Library — Overview

Design document for the Yinz standard library. Content here is in planning phase — no implementation yet.

---

## Philosophy

The standard library follows all 12 golden rules. Everything is dot-method driven, autocomplete-first, self-documenting, and designed so a jr dev can use it without reading docs. If Python can do it in one line, Yinz should too — but type-safe and compiled.

---

## Tree Shaking — Non-Negotiable

The compiler only includes stdlib code your program actually uses. If you never touch `request`, `tensor`, or `date`, none of that code exists in your binary. Zero bloat.

```
function entrypoint() -> nothing {
  let x = math.sqrt(16)
  print(x)
}
// Binary contains: math.sqrt, print, and their dependencies only.
// date, request, file, tensor, cli — none of it is included.
```

This means: the stdlib can be as comprehensive as we want. Add everything. The compiler ships only what's used.

---

## Auto-Import — No Import Statement Needed

The standard library is always available. Developers never write `import { sqrt } from "math"`. They just write `math.sqrt()`. The compiler and IDE know the entire standard library at build time. Tree shaking strips anything unused.

This applies to all stdlib modules: `math`, `file`, `request`, `server`, `date`, `duration`, `json`, `csv`, `terminal`, `cli`, `path`, `directory`, `random`, `stats`, `convert`, `physics`, `matrix`, `vector`, `tensor`, `vectors`, `env`.

---

## Design Principles

1. **Everything is dot-methods on the relevant type.** `file.read()`, `date.now()`, `request.get()`. Autocomplete-driven discovery.
2. **All I/O uses the `errors` system.** File reads, HTTP calls, parsing — auto-propagate or handle explicitly.
3. **Immutable by default for data types.** Dates, strings — methods return new values, never mutate.
4. **`number` is decimal, `float` is binary, `int` is whole numbers.** Default prevents floating point surprises. IDE teaches when each is appropriate.
5. **Scripting should be as easy as Python.** One-file scripts, CLI tools, data processing — minimal boilerplate.
6. **Tree shaking is mandatory.** Be as comprehensive as wanted. No bloat.
7. **Speed is always the priority.** Vectorize, use SIMD, inline aggressively. Even precision operations should be as fast as possible.

---

## Module Areas — Ships In

Each module ships in a specific Yinz version per `design/mvp-scope.md`. The granular versioning means each module gets dedicated design + implementation when its turn comes.

| Module | Ships In | Notes |
|--------|----------|-------|
| `file` + `path` + `directory` | **v0.5** | Tight trio; see [filesystem.md](filesystem.md) |
| `math` | **v0.6** | See [math.md](math.md) |
| `cli` + `env` + `process` | **v0.7** | Tight trio; see [cli.md](cli.md) |
| `json` | **v0.8** | Part of [data.md](data.md) |
| `date` + `duration` | **v0.9** | Tight pair; see [dates.md](dates.md) |
| `db` (database) | **v0.10** | DuckDB + Postgres only at launch; other drivers (MySQL, SQLite, etc.) deferred until after v1.0. See [database.md](database.md) |
| `log` (basic) | **v0.11** | Logging framework v0.23 |
| `random` | **v0.12** | |
| Testing framework | **v0.13** | Language feature, not stdlib (`test` keyword) |
| `regex` | **v0.14** | |
| `request` (outbound HTTP) | **v0.15** | Three-tier API (helpers + builder + raw sockets) — see [network.md](network.md) |
| `stats` | **v0.16** | Built on math |
| `crypto` / `hash` | **v0.17** | SHA, AES, HMAC, KDF |
| `compression` | **v0.18** | gzip, zstd, maybe brotli |
| `terminal` | **v0.19** | ANSI colors, cursor |
| `csv` | **v0.20** | Part of [data.md](data.md) |
| `server` (inbound HTTP) | **v0.21** | Builds on `request` (v0.15); shares `Request`/`Response` types — see [network.md](network.md) |
| Logging framework | **v0.23** | Structured logging, sinks, filters; builds on basic `log` |
| Process spawning | **v0.24** | `process.spawn`, pipes, signals; distinct from v0.7 `process` |
| `ml` / `tensor` | **v2+** | DEFERRED — see [ml.md](ml.md) and `design/mvp-scope.md#v2--deferred-features` |
| `markets` | **v2+** | DEFERRED — see [markets.md](markets.md) and `design/mvp-scope.md#v2--deferred-features` |
| TCP/UDP networking beyond `request`/`server` | **v2+** | DEFERRED |
| Additional DB drivers (MySQL, SQLite, MariaDB, MS SQL, etc.) | **post-v1.0** | DuckDB + Postgres ship in stdlib at v0.10; other backends deferred until core is stable |

For language features (not stdlib) — strings, collections, concurrency keywords — see the relevant `spec/*.md` and `design/*.md`. All language features ship in v0.1.

---

## Resolved Stdlib Questions (formerly open)

The questions that previously lived in `design/open-questions.md` have been resolved:

- **Package manager design** — RESOLVED. See `design/packages.md`. Ships v0.22.
- **Testing framework** — RESOLVED. Built into the language as the `test` keyword. See `design/testing.md`. Ships v0.13.
- **Logging framework** — RESOLVED. Basic `log` ships v0.11; full framework v0.23.
- **Regex** — Ships v0.14. Engine choice + detailed API designed at that version.
- **Crypto / hashing** — Ships v0.17.
- **Networking beyond HTTP** — Raw sockets at the bottom of the `request` module (v0.15) provide the floor (`net.tcp.connect(host, port)`). TCP/UDP as standalone modules deferred to v2+.
- **Process spawning** — Ships v0.24.
- **Environment variables** — RESOLVED. `env` module ships in the v0.7 CLI trio.
- **Compression** — Ships v0.18.
- **Database (`db` module)** — RESOLVED. Ships v0.10 with **DuckDB + Postgres only**. All other drivers (MySQL, SQLite, MariaDB, MS SQL, etc.) **deferred until after v1.0 launch**. See `design/stdlib/database.md`.

Open questions remaining: the per-module API designs themselves (designed at each module's version turn). The **committed** module list (the version table above) is locked; **candidate** additions not yet slotted into the sequence are tracked in the next section.

---

## Candidate Modules (brainstorm — not yet locked into the version sequence)

Proposed 2026-06-13. These are NOT promises and NOT designed yet — they're a holding pen so good ideas don't get lost. When one is committed, it moves into the version table above (or the v2+ deferred rows) and gets a design doc at its version turn. This is the single home for "stdlib ideas we like but haven't sequenced" — don't scatter them into other docs.

### Strong candidates (near-embarrassing gaps)

| Candidate | What | Note |
|---|---|---|
| **`encoding`** | base64 (std + URL-safe), hex, URL/percent codec | **Already has a design doc** → [encoding.md](encoding.md). `stdlib-design.md` Rule 8 names base64; `request` v0.15 needs URL-encoding. Slot before/with v0.15. |
| **`uuid`** | UUID v4/v7, ULID generation | Ubiquitous, tiny. Pure-compute, no I/O. |
| **`tls`** | TLS handshake + cert validation | `request` (v0.15) and `server` (v0.21) imply HTTPS — TLS is the module-sized dependency neither currently names. Sequence *before* v0.15. |
| **auth: password hashing + JWT** | argon2/bcrypt KDF, JWT sign/verify | Distinct from `crypto` v0.17 primitives (SHA/AES/HMAC). Auth is universal. |
| **`toml`** (+ maybe `yaml`) | config-format parse/emit | Dogfoods `yinz.toml` (Yinz's own config format); sibling to `json`/`csv` in `data.md`. |

### Folded into existing modules (NOT new modules)

| Idea | Folds into | Why not standalone |
|---|---|---|
| **WebSocket** | `network.md` / `server` (v0.21) | WS bootstraps via an HTTP `Upgrade` handshake on the same accept loop, then switches to RFC 6455 frame protocol. Belongs as a `server` section, not a separate module — but the frame protocol is real added code, not free. |
| **`set<T>`** ✅ COMMITTED 2026-06-13 → **v0.4** | core collections (`design/collections.md`) — a **language feature**, like `map` (co-ships with the v0.4 linting tier) | A set is `map<K,V>` with keys only: O(1) membership + auto-uniqueness + set algebra. Inherently growable (no "fixed set"). Designed in `collections.md`. Includes `array<T> → set<T>` auto-promotion (membership-only). |
| **`array.unique()`** ✅ COMMITTED 2026-06-13 | core collections (`design/collections.md`) | The "dedupe a list once" case — a method on `array`, not a type. Ships with `set<T>`. |

### Worth designing (secondary)

| Candidate | What |
|---|---|
| **i18n / `locale`** | locale-aware formatting + message catalogs. Fits the "no platform-default locale" rule (Rule 3) — explicit locale, never silent. |
| **validation / schema** | runtime validation at the external-data boundary (parsed JSON → typed `shape`). OPEN: may be a language feature (`shape` + `errors`) rather than a stdlib module — decide before designing. |
| **test-framework extensions** | property-based testing, fuzzing, benchmarking. Next tier above the `test` keyword (v0.13). |
| **`dns`** | name resolution; networking floor below `request`. |
| **caching / LRU** | in-memory bounded cache (Rule 4: bounded by default). |
| **templating** | HTML/text templating; ties to the GUI/web work (`design/future/gui/`). |
| **observability** | metrics + tracing, above basic `log` (v0.11) / logging framework (v0.23). |

### v2+ / niche (parked so they're not forgotten)

Media codecs (image/audio/PDF), email/SMTP, gRPC/protobuf, complex numbers + units-of-measure, embedded I/O (serial/USB/Bluetooth). None committed; each needs a real use case to graduate.
