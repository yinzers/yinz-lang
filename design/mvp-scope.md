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
- Generics — both **type generics** (`array<T>`, `map<K,V>`) AND **function generics** (`function foo<T>(...)`)
- Collections (`fixed<T>`, `array<T>`, `map<K,V>`) with bracket sugar for `.get()` / `.set()`
- Control flow (`if`, multi-case `if`, `for`, `while`, early returns)
- Strings (with interpolation, indexing via code-point / byte / grapheme)
- Scope (block scoping, file-level constants)
- Destructuring (object destructuring, no array destructuring)
- Type conversion (dot methods, no `as` keyword)
- Errors (`errors` keyword, flow-sensitive auto-propagation)
- Ownership (`share`/`lend`/`give`/`copy`/`.freeze`)
- Numeric types (`number` = decimal128 default, `number<N>` up to 4096, `float` = f64, `int` = i64)
- Concurrency keywords parse + type-check (`wait`, `background`) — runs SEQUENTIALLY (auto-parallelization comes in v0.3)
- Iterables — built-in iteration over collections (`for (x in arr)`). Custom `follows Iterable<T>` is v1.0.
- Modules (`import`, `export`, root-relative paths, aliases with `as`, duplicate-name compile error)
- Main entry (`function entrypoint()`)
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

## v0.3 — Auto-parallelization optimization + Auto-SoA

The compiler's dependency-graph analysis engages. Existing v0.1+ code that uses `wait`/`background` keywords — or that has no concurrency keywords at all but has parallelizable independent operations — runs faster automatically. No syntax change.

This is a compiler-internal milestone. From the user's perspective, code just gets faster.

### Auto-parallelization (locked design, deferred from v0.1)

The compiler's automatic dependency-graph analysis that schedules independent operations to run in parallel without `wait`/`background` keywords. This is the "default fast" Yinz promise — most code is automatically parallel.

- **Why this version**: The dependency analysis is a significant sub-project. Shipping v0.1 + v0.2 with the optimization deferred means users get the SYNTAX (`wait`, `background` keywords parse, type-check, and have correct sequential semantics) but not the SPEED. Their code still WORKS — it just runs sequentially.
- **Substitute used pre-this-version**: Sequential execution. All `wait` calls happen in order. All `background` calls run on a single thread. Correct behavior, no parallelism gain.
- **Locked design**: See `design/concurrency.md` and `spec/concurrency.md`

### Auto-SoA layout transformation (locked design, deferred from v0.1)

The compiler auto-transforms `array<Shape>` storage from Array-of-Structs to Struct-of-Arrays when a hot loop accesses only 1-2 fields, enabling SIMD vectorization and improving cache locality. Same external API (`arr[i].field` works identically); the memory layout is the only thing that changes.

- **Why this version**: SoA analysis is a substantial codegen optimization pass — requires the ownership system to be working (M4, v0.1), the `array<T>` implementation to be stable (v0.1), and an optimization-pass framework that can decide per-array (v0.2 LSP work surfaces some of this). v0.1 ships the basic `shape`/`array` infrastructure; v0.2 ships LSP/watch/fmt; v0.3 is the right slot for ambitious cross-cutting compiler analyses (it's also when auto-parallelization lands).
- **Substitute used pre-this-version**: Default Array-of-Structs layout. Manual SoA via parallel `array<T>` of each field is possible if a user really needs it pre-v0.3, but no compiler help.
- **Locked design**: See `design/future/auto-soa.md`

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

### Within-File Test Parallelization and Cross-File Resource Locks (locked design, deferred from v0.1)

`parallel file` declaration to enable within-file test parallelism. `sequential "resource-name"` declarations to serialize files that share a resource (e.g., two test files both writing to the `users` DB table).

- **Why v0.14+**: v0.13 ships with file-level parallelism only. The 95% case is "files in parallel, tests within a file sequential" and it works fine. Adding refinements upfront creates complexity for everyone to solve a problem only large test suites hit.
- **Substitute used pre-v0.14+**: Users are responsible for test isolation. `setup file { db.connect(...) }` opens a per-file connection. Files that genuinely share state should be designed differently or use `--serial` mode.
- **Trigger to land**: v0.14+ if real demand surfaces (e.g., a project with massive test files that need within-file parallelism, or users reporting they can't isolate cross-file state cleanly).
- **Locked design**: See `design/testing.md` and `spec/testing.md`

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

- **Formal grammar lock** — the EBNF / parser becomes the contract for backward compatibility
- **All compile errors reviewed** for the WHAT/WHAT-INSTEAD/WHY format per `design/teaching-mission.md`
- **Backward-compatibility policy** kicks in (see `design/versioning.md`)

v1.0 is when the language becomes "stable" — breaking changes after this require a major version bump.

### Operator overloading (locked design, deferred from v0.1)

User-defined types can `follows Add`, `follows Subtract`, etc. and use `+`, `-`, `*`, `/` operators.

- **Why this version**: Not load-bearing for v0.1. Built-in numeric types use the built-in operators just fine. Custom-type overloading is polish, not core. Ready to ship at public launch.
- **Substitute used pre-v1.0**: Users with custom math types write `.add()`, `.subtract()` methods explicitly. Verbose but works.
- **Locked design**: See `design/operators.md` and `spec/operators.md`

### Custom iterables (locked design, deferred from v0.1)

User types can implement `Iterable<T>` or `FallibleIterable<T>` and be iterated with `for`.

- **Why this version**: Built-in `for` over collections (`array`, `fixed`, `map`, ranges) works without this. Built-in `for` over fallible iterables like `file.lines()` works in v0.6. Custom user types implementing the contracts is the extension that ships at v1.0.
- **Substitute used pre-v1.0**: Users with iterable-like data expose a `.items()` method returning `array<T>` and `for (item in foo.items())`. Lossy compared to true iteration (materializes the whole collection) but works.
- **Locked design**: See `design/iterables.md` and `spec/iterables.md`

### Deprecation marking (locked design, deferred from v0.1)

A way to mark stdlib functions / language features as deprecated, with compiler warnings on use.

- **Why this version**: Only relevant post-v1.0 when backwards-compatibility kicks in. v0.1 follows the no-backwards-compatibility-pre-release policy — breaking changes are fine.
- **Substitute used pre-v1.0**: None needed.
- **Locked design**: See `design/versioning.md` and `design/linting.md`

---

## v1.1 — Post-launch polish tooling

### `ynz doc` and `ynz repl` (locked design, deferred from v0.1)

- **`ynz doc`** — generate static API documentation from `///` doc comments
- **`ynz repl`** — interactive REPL for learning and exploration

- **Why this version**: Polish tooling. Not blocking development or language usability. Post-launch additions.
- **Substitute used pre-v1.1**: No static doc generation (read the source). No REPL (write a small script and `ynz run` it).
- **Trigger to land**: v1.1 (post-launch polish milestone).
- **Locked design**: See `spec/doc-comments.md`

---

## v1.2 — Public package registry

Built in Yinz itself. The dogfood milestone — by v1.2 we have everything needed (http.server v0.21, file system v0.6, env v0.8, JSON v0.9, logging framework v0.22, crypto v0.17, compression v0.18), and building the registry in Yinz proves the language can build real services. If we can't, that's a signal the language has gaps to fix.

Registry isn't deployed publicly until shortly before this version ships. The language launch (v1.0) uses git URLs + local paths for packages until v1.2.

### Public Package Registry (locked design, deferred from v0.5)

Server-side infrastructure for hosting and serving Yinz packages — the `ynz add some-package` discovery + download flow against a public registry.

- **Why this version**: The language isn't publicly launched until v1.0. Before launch, breaking changes are fine; there's no community of authors to support. After v1.0 stabilizes, building the registry — in Yinz itself, as the project's first major dogfooding test — proves the language can build real services.
- **Substitute used pre-v1.2**: Package manager (v0.5) supports git URLs and local paths. `ynz add github:user/repo` works fine. Public registry isn't required for the package manager to be useful.
- **Trigger to land**: v1.2 milestone, after v1.0 launch stabilizes.
- **Locked design**: See `design/packages.md`

---

## v1.x — Lint customization config

The `[lint]` section in `yinz.toml` becomes configurable per `design/linting.md`:
- Disable rules by ID
- Adjust severity per rule
- Tune rule parameters
- Define pattern-based custom rules (covers ~95% of org-specific needs)

This is the escape valve that makes the "compiler IS the linter" decision durable. Most orgs get what they need via this config. Extreme outliers can disable built-in lint entirely (`[lint] enabled = false`) and run their own lint package.

Exact version (v1.3? v1.5?) decided based on demand.

### Lint Customization Config (locked design, deferred from v0.4)

The `[lint]` section in `yinz.toml` becomes fully configurable — disable rules, adjust severity per rule, tune rule parameters (e.g., `max-function-length = 75`), define pattern-based custom rules, or disable built-in linting entirely (`enabled = false`).

- **Why this version**: v0.4 ships the linting tier with curated defaults. Customization adds a configuration surface that should be designed against real usage patterns — too early creates a config syntax we're stuck with.
- **Substitute used pre-this-version**: Curated default rule set per `design/linting.md`. Cannot be customized; take-it-or-leave-it.
- **Locked design**: See `design/linting.md`

---

## v2+ — Deferred features

Headline entries with full rationale:

### Sized integer variants (locked design, deferred from v0.1)

Angle-bracket-parameterized sized integers (`int<8>`, `int<16>`, `int<32>`, `uint<8>`, `uint<16>`, `uint<32>`, `uint<64>`, etc.) and signed equivalents.

- **Why v2+**: Requires const generics (numeric values as type parameters) — a meaningful compiler sub-project that took Rust years to land well. v0.1 users have no legitimate need: FFI is deferred, binary protocols can use byte arrays + stdlib helpers.
- **Substitute used pre-v2**: Use plain `int` (= i64) for all whole numbers. Covers ±9.2 × 10^18 — bigger than any count a human writes by hand. Precision loss vs sized variants only matters at FFI boundaries, which aren't in v0.1.
- **Trigger to land**: Either (a) FFI work begins, OR (b) a real user workload needs to interop with a binary protocol that v0.1's byte-array helpers can't ergonomically handle.
- **Locked design**: See `design/numeric-types.md` and `spec/numeric-types.md`

### Sized float variants (locked design, deferred from v0.1)

Single-precision (32-bit) IEEE 754 binary float.

- **Why v2+**: Only essential for GPU compute and ML workloads (both v2+). For graphics/physics in v0.1, `float` (= f64) is overkill but works fine.
- **Substitute used pre-v2**: Use `float` (f64) for all binary-float math. Slower than f32 on SIMD-heavy workloads but correct.
- **Trigger to land**: GPU dispatch begins OR ML stdlib begins.
- **Locked design**: See `design/numeric-types.md` and `spec/numeric-types.md`

### Arbitrary-precision decimal (locked design, deferred from v0.1)

`number` precision larger than 4096 significant digits — true unbounded decimal arithmetic.

- **Why v2+**: 4096 digits handles every realistic scientific calculation (gravitational wave numerics top out at ~200 digits; even number-theory research rarely exceeds 2000). Unbounded precision means unbounded memory per value and unbounded per-operation time — breaks the language's "predictable performance" character. Real arbitrary-precision libraries (GMP, MPFR) are massive projects.
- **Substitute used pre-v2**: Use `number<N>` with N up to 4096. Compile error if a user tries `number<5000>` — error message explicitly points to this deferral.
- **Trigger to land**: A real user workload genuinely exceeds 4096 digits AND can't be restructured to fit. This is a deliberately high bar.
- **Locked design**: See `design/numeric-types.md` and `spec/numeric-types.md`

### FFI (Foreign Function Interface) (locked design, deferred from v0.1)

The `foreign` keyword and machinery to call C / C++ / Rust libraries from Yinz.

- **Why v2+**: Significant design surface (ownership across the boundary, type mapping, safety guarantees). Most v0.1 code doesn't need it — stdlib is in scope. Stdlib internals can use compiler-private FFI without exposing a user-facing `foreign` keyword.
- **Substitute used pre-v2**: Stdlib modules that need C interop (file I/O, networking, math) call C internally via compiler-private mechanisms. Users don't see this. If a user genuinely needs to call a third-party C library, they have to wait for v2+ or contribute to the stdlib.
- **Trigger to land**: v2+ work begins OR a stdlib gap creates a real need.
- **Locked design**: See `design/ffi.md` and `spec/ffi.md`

### GPU dispatch (locked design, deferred from v0.1)

The `gpu` call-site keyword and kernel compilation to GPU shader / compute languages.

- **Why v2+**: Massive scope (kernel compilation, ABI design, runtime fallback). No v0.1 user has shown demand. Was tagged MVP2+ in original design.
- **Substitute used pre-v2**: None — `gpu` keyword is reserved but not parseable. Compile error if used.
- **Trigger to land**: v2+ AND a real ML/compute workload requires it.
- **Locked design**: See `design/gpu.md` and `spec/concurrency.md`

### ML stdlib (locked design, deferred from v0.1)

Tensors, neural network primitives, autodiff, optimizers.

- **Why v2+**: v0.1 stdlib focus is general-purpose. ML requires its own deep design (compatible with `float`/`f32`, GPU dispatch, etc.) and is a v2+ concern.
- **Substitute used pre-v2**: None. ML workloads run in Python until then.
- **Trigger to land**: v2+ AND GPU dispatch lands.
- **Locked design**: See `design/stdlib/ml.md`

### Markets stdlib (locked design, deferred from v0.1)

Financial data ingestion, brokerage integrations, market data feeds.

- **Why v2+**: Niche stdlib module. Not load-bearing for v0.1.
- **Substitute used pre-v2**: None. Users write HTTP-based integrations directly.
- **Trigger to land**: v2+.
- **Locked design**: See `design/stdlib/markets.md`

### Self-hosted compiler (locked design, deferred from v0.1)

The Yinz compiler rewritten in Yinz (current bootstrap is in Rust).

- **Why v2+**: Need feature parity in v1.0+ stable Yinz first. Bootstrap-in-Rust serves us for years.
- **Substitute used pre-v2**: Rust bootstrap compiler — see `design/compiler-language.md`.
- **Trigger to land**: v2+ AND the language is stable enough to self-host.
- **Locked design**: See `design/compiler-language.md`

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
