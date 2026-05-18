# Standard Library — Overview

Design document for the Yinz standard library. Content here is in planning phase — no implementation yet.

---

## Philosophy

The standard library follows all 12 golden rules. Everything is dot-method driven, autocomplete-first, self-documenting, and designed so a jr dev can use it without reading docs. If Python can do it in one line, Yinz should too — but type-safe and compiled.

---

## Tree Shaking — Non-Negotiable

The compiler only includes stdlib code your program actually uses. If you never touch `Http`, `Tensor`, or `Date`, none of that code exists in your binary. Zero bloat.

```
function entrypoint() -> nothing {
  let x = math.sqrt(16)
  print(x)
}
// Binary contains: math.sqrt, print, and their dependencies only.
// Date, Http, File, Tensor, CLI — none of it is included.
```

This means: the stdlib can be as comprehensive as we want. Add everything. The compiler ships only what's used.

---

## Auto-Import — No Import Statement Needed

The standard library is always available. Developers never write `import { sqrt } from "math"`. They just write `math.sqrt()`. The compiler and IDE know the entire standard library at build time. Tree shaking strips anything unused.

This applies to all stdlib modules: `math`, `file`, `http`, `date`, `duration`, `json`, `csv`, `terminal`, `cli`, `path`, `directory`, `random`, `stats`, `convert`, `physics`, `matrix`, `vector`, `tensor`, `vectors`, `env`.

---

## Design Principles

1. **Everything is dot-methods on the relevant type.** `file.read()`, `date.now()`, `http.get()`. Autocomplete-driven discovery.
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
| `file` + `path` + `directory` | **v0.6** | Tight trio; see [filesystem.md](filesystem.md) |
| `math` | **v0.7** | See [math.md](math.md) |
| `cli` + `env` + `process` | **v0.8** | Tight trio; see [cli.md](cli.md) |
| `json` | **v0.9** | Part of [data.md](data.md) |
| `date` + `duration` | **v0.10** | Tight pair; see [dates.md](dates.md) |
| `log` (basic) | **v0.11** | Logging framework v0.22 |
| `random` | **v0.12** | |
| Testing framework | **v0.13** | Language feature, not stdlib (`test` keyword) |
| `regex` | **v0.14** | |
| `http` client | **v0.15** | Three-tier API (helpers + builder + raw sockets) — see [http.md](http.md) |
| `stats` | **v0.16** | Built on math |
| `crypto` / `hash` | **v0.17** | SHA, AES, HMAC, KDF |
| `compression` | **v0.18** | gzip, zstd, maybe brotli |
| `terminal` | **v0.19** | ANSI colors, cursor |
| `csv` | **v0.20** | Part of [data.md](data.md) |
| `http.server` | **v0.21** | Builds on http client |
| Logging framework | **v0.22** | Structured logging, sinks, filters; builds on basic `log` |
| Process spawning | **v0.23** | `process.spawn`, pipes, signals; distinct from v0.8 `process` |
| `ml` / `tensor` | **v2+** | DEFERRED — see [ml.md](ml.md) and `design/mvp-scope.md#v2--deferred-features` |
| `markets` | **v2+** | DEFERRED — see [markets.md](markets.md) and `design/mvp-scope.md#v2--deferred-features` |
| TCP/UDP networking beyond `http` | **v2+** | DEFERRED |
| Database drivers | **packages only** | Not in stdlib; community ships via package registry (v1.2+) |

For language features (not stdlib) — strings, collections, concurrency keywords — see the relevant `spec/*.md` and `design/*.md`. All language features ship in v0.1.

---

## Resolved Stdlib Questions (formerly open)

The questions that previously lived in `design/open-questions.md` have been resolved:

- **Package manager design** — RESOLVED. See `design/packages.md`. Ships v0.5.
- **Testing framework** — RESOLVED. Built into the language as the `test` keyword. See `design/testing.md`. Ships v0.13.
- **Logging framework** — RESOLVED. Basic `log` ships v0.11; full framework v0.22.
- **Regex** — Ships v0.14. Engine choice + detailed API designed at that version.
- **Crypto / hashing** — Ships v0.17.
- **Networking beyond HTTP** — Raw sockets at the bottom of the http module (v0.15) provide the floor. TCP/UDP as standalone modules deferred to v2+.
- **Process spawning** — Ships v0.23.
- **Environment variables** — RESOLVED. `env` module ships in the v0.8 CLI trio.
- **Compression** — Ships v0.18.
- **Database drivers** — Not stdlib. Community packages from v1.2 onwards.

Open questions remaining: the per-module API designs themselves (designed at each module's version turn). The list of modules is locked.
