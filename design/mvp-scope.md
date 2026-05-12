# MVP Scope — Granular Versioning Roadmap

Every version of Yinz between v0.1 and v1.0 ships ONE focused thing — either a stdlib module, a tight pair of related modules, or a compiler-infrastructure feature. This document is the source of truth for "is feature X in v0.N?"

The granular model exists because (a) each version has predictable scope, (b) each module gets dedicated design + implementation attention, (c) nothing gets forgotten in a "v1.0 mega-release."

**Each version's module spec is designed just-in-time when that version is up to implement.** Locking the LIST of versions now without designing every module's API up front lets us start fast and adapt as we learn.

---

## v0.1 — Core language only

The absolute minimum: the language compiles and runs a hello-world program. No stdlib modules.

**Language features (all v0.1):**
- Variables (`let`, `const`)
- Functions (with ownership modifiers: `share`/`lend`/`give`/`copy`/`.freeze`)
- Types (struct-like with fields and methods)
- Options (named value sets, replaces enums)
- Unions (`type Foo = A or B or C`)
- Maybe types (`maybe T`, `none`, `.exists()`, `.value`, `.or(default)`)
- Generics — both **type generics** (`array[T]`, `map[K,V]`) AND **function generics** (`function foo[T](...)`)
- Collections (`fixed[T]`, `array[T]`, `map[K,V]`) with bracket sugar for `.get()` / `.set()`
- Control flow (`if`, multi-case `if`, `for`, `while`, early returns)
- Strings (with interpolation, indexing via code-point / byte / grapheme)
- Scope (block scoping, file-level constants)
- Destructuring (object destructuring, no array destructuring)
- Type conversion (dot methods, no `as` keyword)
- Errors (`errors` keyword, flow-sensitive auto-propagation)
- Ownership (`share`/`lend`/`give`/`copy`/`.freeze`)
- Numeric types (`number` = decimal128 default, `number[N]` up to 4096, `float` = f64, `int` = i64)
- Concurrency keywords parse + type-check (`wait`, `background`) — runs SEQUENTIALLY (auto-parallelization comes in v0.3)
- Iterables — built-in iteration over collections (`for (x in arr)`). Custom `follows Iterable[T]` is v1.0.
- Modules (`import`, `export`, root-relative paths, aliases with `as`, duplicate-name compile error)
- Main entry (`function main()`)
- Doc comments (`///`)
- Sensitive type modifier (auto-redact in output)
- Operators (built-in `+`, `-`, `*`, `/`, `%`, `&&`, `||`, `!`, comparison, bitwise). Operator overloading is v1.0.
- Test keyword reserved in parser (rejected at compile until v0.13)

**Built-in globals:**
- `print(value)` — output to stdout
- `panic(message)` — terminate with error message

**Tooling:**
- `ynz build` — compile
- `ynz run` — compile + execute

**Programs you can write in v0.1:** hello world, math demos, pure-computation programs. No file I/O, no networking, no testing.

---

## v0.2 — Dev-loop tooling

LSP + `ynz watch` + `ynz fmt`. All shipped together as one milestone because they're the same kind of feature: making the development loop fast and pleasant.

- **LSP** — autocomplete, hover, go-to-definition, rename, inline errors. Built on the salsa queries the v0.1 compiler already uses (see `design/compiler-language.md`).
- **`ynz watch`** — recompile-on-save with sub-second turnaround for typical changes
- **`ynz fmt`** — canonical code formatter (no config — Yinz has one style)

---

## v0.3 — Auto-parallelization optimization

The compiler's dependency-graph analysis engages. Existing v0.1+ code that uses `wait`/`background` keywords — or that has no concurrency keywords at all but has parallelizable independent operations — runs faster automatically. No syntax change.

This is a compiler-internal milestone. From the user's perspective, code just gets faster.

---

## v0.4 — Linting tier (compile-time suggestions)

The compiler starts emitting the third severity tier — suggestions — during normal `ynz build`. Errors and warnings have existed since v0.1; v0.4 introduces the proactive teaching tier.

**Initial rule set ships with v0.4** (see `design/linting.md` for the full curated list). Module-specific suggestion rules ship attached to their respective modules in subsequent versions.

No separate `ynz lint` command — the compiler IS the linter. Customization (config file) comes in v1.x.

---

## v0.5 — Package manager

`ynz add`, `ynz remove`, `ynz update`, `ynz install` + lock file (`yinz.lock`, TOML format). See `design/packages.md` for the full design.

**Source types supported:** git URLs, local paths. No public registry yet — the registry is v1.2 (after the language stabilizes at v1.0).

Install mechanism targets bun-class speed (content-addressed cache, hard-links, parallel resolver).

---

## v0.6 — File system (tight trio)

Three modules bundled together because they're tightly coupled:

- **`file`** — read, write, append, exists, delete
- **`path`** — join, dirname, basename, extname, normalize, isAbsolute
- **`directory`** — list, create, delete, exists, isDir

Module-specific lint suggestions ship with this version (e.g., "prefer `path.join()` over string concatenation").

---

## v0.7 — `math`

Self-contained module. sqrt, abs, min/max, floor/ceil/round, trig (sin/cos/tan and inverses), log/exp, pow, constants (`math.pi`, `math.e`, etc.).

Module-specific lint suggestions: "prefer `math.pi` over hardcoded 3.14159."

---

## v0.8 — CLI essentials (tight trio)

Three modules bundled — all about "running as a program":

- **`cli`** — argument parsing (positional + flags + options)
- **`env`** — environment variables (`env.get(name)`, `env.getOr(name, default)`, `env.set`)
- **`process`** — `process.exit(code)`, `process.pid`, `process.parentPid`, `process.startedAt`, `process.uptime`, `process.args` (raw argv), `process.workingDirectory`, `process.onShutdown(handler)`, `process.isRunning()` (graceful loop pattern)

See `design/stdlib/cli.md` for the full design (TBD when this version is up).

---

## v0.9 — `json`

Parse, stringify, prettify. Universal data interchange.

---

## v0.10 — `date` + `duration` (tight pair)

Always paired. `date.now()`, `date.from()`, comparisons, formatting, parsing. `duration` construction, arithmetic, conversion (seconds/minutes/hours/days).

---

## v0.11 — `log` (basic)

`log.info()`, `log.warn()`, `log.error()`, `log.debug()`. Starter logging — the full framework (structured logging, sinks, filters, log levels per module) ships in v0.22.

Module-specific lint suggestion: "prefer `log.info()` over `print()` in non-test code when `log` module is available."

---

## v0.12 — `random`

Tiny module. `random.int(min, max)`, `random.float()`, `random.choice(array)`, `random.shuffle(array)`, `random.seed(n)` for deterministic testing.

---

## v0.13 — Testing framework (`test` keyword + `ynz test`)

The `test` keyword has been reserved in the parser since v0.1. v0.13 ships the runner.

Includes:
- `test "description" { ... }` blocks
- `setup` / `teardown` (per-test, parameterless) and `setup file` / `teardown file` (per-file)
- Optional single-level `group "name" { ... }` blocks (no nesting allowed)
- Assertions: `assert`, `assertEqual`, `assertNotEqual`, `assertGreaterThan`, `assertLessThan`, `assertContains`, `assertFails`, `assertPanics`
- `ynz test` runner with substring filtering against file paths, group names, test descriptions
- Files run in parallel by default; tests within a file run sequentially
- `--serial` flag to force all-serial execution; `--parallel N` to cap parallelism count
- Test output mirrors source structure (file → group → tests)

See `design/testing.md` for the full design.

---

## v0.14 — `regex`

Substantial design surface (engine choice, flags, captures, replace). Gets its own milestone.

---

## v0.15 — `http` client

Three-tier API design (see HTTP open question in `design/open-questions.md` to be designed when this version is up):

1. **High-level helpers** — `http.get(url)`, `http.post(url, body)`, `http.put`, `http.delete`, `http.websocket(url)`
2. **Mid-level request builder** — `http.request().method("GET").header(...).timeout(5).send()` for cases not covered by helpers
3. **Low-level socket access** — `net.tcp.connect(host, port)` returning a raw socket. The floor of the user-accessible network stack (anything lower is FFI territory).

With TLS support from day 1 in this version.

---

## v0.16 — `stats`

mean, median, mode, stddev, variance, percentile, histogram. Built on `math`.

---

## v0.17 — `crypto` / `hash`

SHA-256, SHA-512, AES-GCM, HMAC, key derivation (PBKDF2/Argon2). Careful design needed.

---

## v0.18 — `compression`

gzip, zstd, optionally brotli. Wraps system libs via the compiler-internal FFI (since user-facing FFI is v2+).

---

## v0.19 — `terminal`

ANSI colors, cursor positioning, terminal-size detection. For richer CLI output.

---

## v0.20 — `csv`

Read, write, optionally streaming for huge files. Less common than JSON but useful.

---

## v0.21 — `http.server`

Builds on `http` client. Routing, middleware, request/response abstractions. Substantial module.

---

## v0.22 — Logging framework

Structured logging on top of v0.11's basic `log` module. Sinks (file, stdout, syslog), filters, log levels per module, structured fields, contextual loggers.

---

## v0.23 — Process spawning

`process.spawn(cmd, args)`, pipes (stdin/stdout/stderr), signal handling beyond `onShutdown`. Distinct from v0.8's `process.exit/.pid/.isRunning` (which are about the current process).

---

## v1.0 — Public launch and stability

Public launch milestone. Ships:

- **Operator overloading** — user types can `follows Add`, `follows Subtract`, etc., and use `+`, `-`, `*`, `/` operators
- **Custom iterables** — user types can `follows Iterable[T]` and `follows FallibleIterable[T]`
- **Formal grammar lock** — the EBNF / parser becomes the contract for backward compatibility
- **All compile errors reviewed** for the WHAT/WHAT-INSTEAD/WHY format per `design/teaching-mission.md`
- **Backward-compatibility policy** kicks in (see `design/versioning.md`)

v1.0 is when the language becomes "stable" — breaking changes after this require a major version bump.

---

## v1.1 — Post-launch polish tooling

- **`ynz doc`** — generate static API docs from `///` comments
- **`ynz repl`** — interactive REPL for learning and exploration

---

## v1.2 — Public package registry

Built in Yinz itself. The dogfood milestone — by v1.2 we have everything needed (http.server v0.21, file system v0.6, env v0.8, JSON v0.9, logging framework v0.22, crypto v0.17, compression v0.18), and building the registry in Yinz proves the language can build real services. If we can't, that's a signal the language has gaps to fix.

Registry isn't deployed publicly until shortly before this version ships. The language launch (v1.0) uses git URLs + local paths for packages until v1.2.

---

## v1.x — Lint customization config

The `[lint]` section in `yinz.toml` becomes configurable per `design/linting.md`:
- Disable rules by ID
- Adjust severity per rule
- Tune rule parameters
- Define pattern-based custom rules (covers ~95% of org-specific needs)

This is the escape valve that makes the "compiler IS the linter" decision durable. Most orgs get what they need via this config. Extreme outliers can disable built-in lint entirely (`[lint] enabled = false`) and run their own lint package.

Exact version (v1.3? v1.5?) decided based on demand.

---

## v2+ — Deferred features

See `design/deferrals.md` for the authoritative ledger. Headline entries:

- **FFI** (call C/C++/Rust libraries from Yinz)
- **GPU dispatch** (the `gpu` call-site keyword, kernel compilation)
- **Sized integer variants** (`int[N]`, `uint[N]` for N != 64)
- **Sized float variants** (`f32`)
- **Arbitrary-precision decimal** beyond `number[4096]`
- **ML stdlib** (tensors, neural net primitives)
- **Markets stdlib** (financial data, brokerage integrations)
- **Self-hosted compiler** (Yinz compiler written in Yinz)
- **Deprecation marking** (only relevant post-v1.0)

---

## How to decide if a feature is v0.N or vN+1

Three questions:

1. **Does it stand alone as a focused thing?** If yes, candidate for its own version. If it's part of a tightly-coupled pair (file+path+directory; date+duration; cli+env+process), bundle them.
2. **Does the v0.1 syntax surface depend on it?** If yes, it's v0.1. Things that change every function signature (ownership) can't defer.
3. **Does it unlock the most value for the next version's work?** (Order versions for value compounding — LSP early so everyone benefits, package manager before stdlib ramp so people can fill gaps with third-party packages, etc.)

When in doubt, defer to a later version. We can always pull features forward when they prove easy; pushing them back is harder once users depend on them.

---

## Tight Pair Rationale

Some modules are bundled into one version because designing/shipping them separately would create awkward intermediate states:

| Bundle | Why bundled |
|--------|------------|
| `file` + `path` + `directory` (v0.6) | `file.read()` needs `path` for argument types. `directory` is the iteration form of `file`. All three together. |
| `cli` + `env` + `process` (v0.8) | Every CLI tool needs all three. Designing them together keeps conventions consistent. |
| `date` + `duration` (v0.10) | Durations only make sense in relation to dates. Arithmetic between them is the same module's concern. |
| LSP + `ynz watch` + `ynz fmt` (v0.2) | All dev-loop tooling — landing them together gives the dev experience a single noticeable jump rather than three small ones. |
