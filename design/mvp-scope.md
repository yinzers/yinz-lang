# MVP Scope — Granular Versioning Roadmap

Every version of Yinz between v0.1 and v1.0 ships ONE focused thing — either a stdlib module, a tight pair of related modules, or a compiler-infrastructure feature. This document is the source of truth for "is feature X in v0.N?"

The granular model exists because (a) each version has predictable scope, (b) each module gets dedicated design + implementation attention, (c) nothing gets forgotten in a "v1.0 mega-release."

**Each version's module spec is designed just-in-time when that version is up to implement.** Locking the LIST of versions now without designing every module's API up front lets us start fast and adapt as we learn.

---

## ⚠️ DO NOT FORGET — Required Teaching Surface for Every Version

**This checklist applies to every version below, no exceptions.** Every version that adds ANY user-facing feature (language feature, stdlib module, compiler optimization, tooling) MUST ship ALL of the following in the same version. Not a future version. Not "we'll add it later." The whole point of the granular versioning model is that each version is complete — compiler + IDE + docs + demos all move together.

### Registry (`registry/features.toml`)
- [ ] New keyword → `[[keyword]]` entry with `token`, `since`, `description`
- [ ] New banned jargon (term we reject from error messages) → `[[banned_jargon]]` entry
- [ ] New primitive method on int/float/string/bool → `[[primitive_intrinsic]]` entry
- [ ] New muted-hint domain (IDE inline annotation) → `[[muted_hint_domain]]` entry with `placement_category`, `example_source`, `example_hint_rendered`
- [ ] New lint/suggestion rule → `[[lint_rule]]` entry (v0.4+ when the linting tier ships)
- [ ] Any feature deliberately deferred to a future version → `[[deferred_language_feature]]` or `[[deferred_tooling_feature]]` entry so it can never be silently forgotten

### Compiler diagnostics — every new error/warning gets all three parts
- [ ] **WHAT**: what went wrong, in plain English, no jargon
- [ ] **WHAT INSTEAD**: the exact fix the user should make
- [ ] **WHY**: the specific, contextual reason — not generic ("avoids allocation") but tied to the actual call site ("scores isn't used again after this line, so...")
- [ ] No bare "invalid syntax" or "type error" without the three-part body
- [ ] If a diagnostic has a single unambiguous WHAT-INSTEAD fix, wire a code-action quick-fix in the LSP

### LSP + IDE (from v0.2 onward)
- [ ] New muted-hint domain wired in `crates/ynz-lsp/src/inlay_hint.rs` (or equivalent handler)
- [ ] New lint rule emitted as an LSP `Diagnostic` with the correct severity
- [ ] If an existing keyword's behavior CHANGES in this version (e.g., `wait`/`background` changing from sequential to concurrent in v0.3), update its hover text in `registry/features.toml` — stale hover docs actively mislead users
- [ ] New code-action quick-fix if any diagnostic has a concrete single-step WHAT-INSTEAD

### VSCode extension (from v0.2 onward)
- [ ] `tooling/vscode-ynz/package.json` version bump to match the Yinz release version
- [ ] At least one new screenshot showing the new IDE surface added to `tooling/vscode-ynz/screenshots/`
- [ ] `tooling/vscode-ynz/README.md` updated if the new capability changes what users do in the editor
- [ ] `.vsix` attached to the GitHub release alongside the Yinz binary

### Demo files (from v0.1 onward — per `.claude/rules/plan-invariants.md`)
- [ ] `examples/pirates-roster/entrypoint.ynz` extended with a realistic section showing the new feature in context — not a toy snippet, something that looks like real code
- [ ] `examples/primantis-orders/v{0.N}_errors.ynz` (or milestone-specific `m{N}_errors.ynz`) — intentional triggers for every new compile error class, each with `// WHY: <DiagnosticClassName>` comment

### Spec + design docs
- [ ] `spec/feature.md` written or updated — audience is an 18-year-old JS dev, examples-heavy, plain English
- [ ] `design/feature.md` written or updated — rationale, alternatives considered, locked decisions
- [ ] `spec/overview.md` table of contents updated if a new spec file was created
- [ ] `design/decisions.md` index updated if a new design file was created
- [ ] Resolved open questions moved from `design/open-questions.md` to the relevant design file

### The test
> If a new contributor checked out this version's git tag and tried to use the feature, would their editor tell them everything they need to know — autocomplete, hover docs, inline hints, clear errors with fixes? If anything is missing, the checklist wasn't completed.

---

## v0.1 — Core language only

The absolute minimum: the language compiles and runs a hello-world program. No stdlib modules.

**Language features (all v0.1):**
- Variables (`let`, `const`)
- Functions (with ownership modifiers: `share`/`lend`/`give`/`copy`/`.freeze`)
- Types (struct-like with fields and methods)
- Options (named value sets, replaces enums)
- Unions (`shape Foo = A | B | C`)
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
- Boolean type (`boolean`, literals `true` / `false`)
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

> **⚠️ DO NOT FORGET** (checklist at top): registry entries for all new keywords + banned jargon + deferred features; WHAT/WHAT-INSTEAD/WHY for every new error; `pirates-roster` demo; error gallery files; spec + design docs. *(v0.1 shipped — checklist applied across M1–M8.)*

---

## v0.2 — Dev-loop tooling

LSP + `ynz watch` + `ynz fmt`. Shipped as five milestones in the `v0-2-dev-loop-tooling` roadmap.

### v0.2-M1: Feature Inventory & SSOT Registry (shipped — tag `v0.2.0-m1`)

Single source of truth for all feature inventories. `registry/features.toml` + `crates/ynz-registry/` replace the scattered per-crate tables.

### v0.2-M2: LSP Thin Slice + VSCode Plugin (this milestone)

**In scope:**
- `ynz-lsp` binary — JSON-RPC stdio server backed by existing salsa queries
- VSCode extension (`tooling/vscode-ynz/`) — spawns `ynz-lsp`, ships syntax highlighting
- Autocomplete (registry-driven: keywords, intrinsics, type constants, deferred-features marked deprecated)
- Inline diagnostics with WHAT/WHAT-INSTEAD/WHY content preserved end-to-end
- Hover docs for every registered keyword, intrinsic, type constant, and deferred feature
- TextMate grammar generated from registry (`crates/ynz-tmgrammar/`) — committed + consistency-tested
- VSCode marketplace publish as preview (`.vsix` fallback if verification stalls)

**Locked decisions:**
- LSP framework: `tower-lsp` vs `lsp-server` — decided by Phase 1 research spike; locks for v0.2-M5
- Extension home: in-repo subdir `tooling/vscode-ynz/` (atomic version bumps, single source of truth)
- Marketplace: preview publish in M2; `.vsix` fallback if objective friction triggers fire
- TextMate grammar: registry-derived — `crates/ynz-tmgrammar` generates, committed artifact, consistency-tested

**Explicitly out of scope (deferred to v0.2-M5):**
- `textDocument/definition` (go-to-def), `textDocument/references` (find-refs), `textDocument/rename`
- `textDocument/formatting` / `textDocument/rangeFormatting` (waits on v0.2-M3 `ynz-fmt`)
- `textDocument/inlayHint` (muted-hint surfaces per `design/inference.md`)
- `textDocument/codeAction`, `textDocument/semanticTokens`
- Doc-comment integration in hover, pull-diagnostics model, structured `Diagnostic.data`
- `Diagnostic.code` / `Diagnostic.codeDescription` fields

**Design doc:** `design/lsp.md`

### v0.2-M3: `ynz fmt` (active — tag `v0.2.0-m3` pending)

**Locked decisions (2026-05-20):**
- CLI flag set: `ynz fmt <path>`, `--all` (walk `yinz.toml` project), `--check` (CI gate), `--stdin` (editor/LSP pipe)
- Comment handling: additive `lex_with_trivia()` re-lex pass in `ynz-parser` — captures every `//` + `///` span without altering the existing `lex()` path
- Registry consumer only: formatter reads keyword spellings, banned-jargon, and reserved-name protection from `ynz-registry`; adds zero new registry entries
- Algorithm: decided by Phase 1 empirical spike (prettier-style vs rustfmt-style — locked measurement gates in `design/fmt.md`)
- Zero config: one canonical output per AST; no `.ynzfmt.toml`
- Safety invariant: `parse(fmt(x)).ast == parse(x).ast` modulo trivia — verified by Phase 4 property test

**What ships:**
- `ynz-fmt` library crate (`format(source) -> Result<String, FmtError>`) — consumed by v0.2-M5 LSP `textDocument/formatting`
- Four CLI modes in `ynz-driver`: single-file, `--all`, `--check`, `--stdin`
- Idempotency + semantic round-trip property tests
- Mass-rewrite of all existing `.ynz` examples to canonical form

**Explicitly out of scope (deferred):**
- `textDocument/formatting` LSP wiring — v0.2-M5
- `format_range(source, range)` API — v0.2-M5 if proven necessary
- Embedded SQL formatting — v0.5+ database stdlib milestone
- Import sorting — v0.4 Tier 3 lint suggestions

**Design doc:** `design/fmt.md`

### v0.2-M4: `ynz watch` (shipping)

**Design doc:** `design/watch.md`

**Locked decisions**:
- **Architecture: daemon** — one long-running process holds one `CompilerDb`; file events mutate `SourceFile.text` salsa inputs; downstream queries invalidate automatically. Sub-second target depends on this.
- **Default behavior: build + run** — `ynz watch foo.ynz` rebuilds AND re-executes on every save. `--check` skips the run step (CI gates, build-only use case).
- **Output: clear-screen by default** — `--no-clear` preserves scrollback for CI logs.
- **`--json` ships in M4 (not deferred)** — emits NDJSON event stream on stdout; schema includes `schema_version: "v0.2-m4-unstable"` field; stable + semver-bound at v0.2.0 final.
- **Memory defense: three layers** — (1) salsa LRU caps per query, (2) periodic DB drop + recreate every N=500 rebuilds or 4h, (3) `memory-stats` RSS polling with soft-warn at 1GB + hard-stop at 4GB.
- **File watching: `notify = "8"` + `notify-debouncer-mini = "0.7"`** — cross-platform; debouncer-only coalescing (100ms window).
- **Process group kill** — child spawned in own process group via `nix::unistd::setsid()` (Unix); SIGTERM → 2s grace → SIGKILL. Catches double-forked children.
- **Shadow source state** — `WatchDb` holds `HashMap<PathBuf, String>` outside salsa; DB rebuild repopulates salsa from shadow. Shadow is source of truth; salsa is derived cache.

### v0.2-M5: LSP Full + v0.2.0 Release (in progress)

**8 new LSP capabilities**:
- `textDocument/definition` — go-to-def, same-file and cross-file
- `textDocument/references` — find all references across open project files; `$/progress` for slow scans
- `textDocument/rename` + `textDocument/prepareRename` — atomic `WorkspaceEdit`; rejects invalid names via registry
- `textDocument/formatting` + `textDocument/rangeFormatting` — delegates to `ynz-fmt::format` / `format_range`
- `textDocument/inlayHint` — 9 domains; 5 firing (variable_type, ownership_call_site, copy_points, array_to_fixed_promotion, let_to_const_promotion); 4 protocol-only awaiting v0.3+ data
- `textDocument/codeAction` — quick-fixes from registry WHAT-INSTEAD; `Replace \`X\` with \`Y\`` format
- `textDocument/semanticTokens` — keyword/type/function/variable/parameter/field/option-variant/number/string/comment classification; delta-encoded
- Structured `Diagnostic.code` + `Diagnostic.data` fields; doc-comment hover enrichment

**3 compiler correctness bug-fixes**:
- Hidden-field default eval: `hidden bar: string = "default"` now evaluates the default correctly (was null-init)
- Dynamic-dispatch call-site coercion: passing `ConcreteFoo` to `dynamic Foo` param now accepted + vtable-dispatches correctly (was typeck error)
- UFCS const-lend check: `const p; p.heal(20)` where `heal: lend self` now errors (parity with function-call form which already errored)

**Tooling**:
- `ynz build --json` — structured NDJSON diagnostic output; schema stabilized at v0.2.0
- `format_range(source, range)` API added to `ynz-fmt` library
- VSCode extension v0.2.0: screenshots, `\n\n` separator polish, all 8 new capabilities wired

**Cuts the `v0.2.0` release tag** (first plain-version tag; no `-mN` suffix) in Phase 12. Plan: `.claude/planning/done/2026-05-20-v0-2-m5-lsp-full-and-release/plan.md`.

> **⚠️ DO NOT FORGET** (checklist at top): registry entries (SSOT keyword/intrinsic/jargon/hint domains); LSP capabilities wired; WHAT/WHAT-INSTEAD/WHY for every new error; extension version bump + screenshots; `pirates-roster` demo; error gallery files. *(v0.2 shipped — checklist applied across M1–M5.)*

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

> **⚠️ DO NOT FORGET** (checklist at top): new `[[muted_hint_domain]]` registry entries for `background_routing` and `channel_capacity` and `auto_arc`; `wait_points` domain activated (was protocol-only); `wait`/`background` hover docs updated (behavior changes from sequential); new lint rules `array-using-soa-layout` + `cross-thread-fields-not-padded` in registry; WHAT/WHAT-INSTEAD/WHY for all new errors (lend-across-thread, wait-on-non-may-block, channel-closed, large-copy warning); SoA debugger DAP integration; extension bump + screenshots for each new IDE surface; `pirates-roster` demo extended per milestone; per-milestone error gallery files (`v0_3_m{1..4}_errors.ynz`). Roadmap: `.claude/planning/active/2026-05-21-v0-3-concurrency-perf/roadmap.md`.

---

## v0.4 — Linting tier (compile-time suggestions)

The compiler starts emitting the third severity tier — suggestions — during normal `ynz build`. Errors and warnings have existed since v0.1; v0.4 introduces the proactive teaching tier.

**Initial rule set ships with v0.4** (see `design/linting.md` for the full curated list). Module-specific suggestion rules ship attached to their respective modules in subsequent versions.

No separate `ynz lint` command — the compiler IS the linter. Customization (config file) comes in v1.x.

**Co-shipping: `set<T>` core collection** (committed 2026-06-13) — the unique-value collection sibling of `map<K,V>`, plus the `array.unique()` method and the `array<T> → set<T>` auto-promotion. Designed in `design/collections.md`. **Why v0.4 specifically:** it's the last language-feature version before the stdlib sequence (v0.5+) begins — so it lands before any stdlib leans on it — AND it's the *earliest* slot where set can ship COMPLETE rather than as a half-feature: its `prefer-set-for-membership` lint + muted hint need this version's linting tier, and its `array→set` auto-promotion codegen needs v0.3's whole-program analysis framework. Shipping the type earlier (the v0.1 collection infra exists) would mean a build-twice: type now, teaching surfaces bolted on later. v0.4 is where all four surfaces (type + auto-promotion + lint + hint) can exist at once.

**Co-shipping candidate: `--release` flag** — LLVM `-O3`, strip debug info, disable dev-only flags. Locked direction; see `design/future/release-mode.md`. May ship in v0.4 alongside the linting work, or slip to a later perf-focused slot if scope demands.

> **⚠️ DO NOT FORGET** (checklist at top): `[[lint_rule]]` registry entries for every new Tier 3 suggestion rule (incl. `prefer-set-for-membership`); `[[primitive_intrinsic]]` entries for `set<T>` methods + `array.unique()`; `[[muted_hint_domain]]` for the array→set hint; LSP `Diagnostic.severity = hint` wired per rule; hover text for each rule follows WHAT/WHAT-INSTEAD/WHY; `--release` flag (if co-shipping) must suppress or adjust hint output; extension version bump + screenshot showing the suggestion squiggle; `pirates-roster` demo with at least two lint rules + a `set<T>` in context; error gallery showing triggered suggestions + set misuse (e.g. indexing a set).

---

## v0.5 — File system (tight trio)

Three modules bundled together because they're tightly coupled:

**Co-shipping candidate: `{ptr, len}` string overhaul** — migrating strings from NUL-terminated C strings to `{ptr, len}` slices. v0.5 is the most likely landing slot because file I/O is the first stdlib that needs embedded-NUL-safe strings (reading binary as a string-typed buffer). See `design/future/string-ptr-len-overhaul.md`. May slip later if file-I/O doesn't actually need embedded NULs in v0.5.

- **`file`** — read, write, append, exists, delete
- **`path`** — join, dirname, basename, extname, normalize, isAbsolute
- **`directory`** — list, create, delete, exists, isDir

Module-specific lint suggestions ship with this version (e.g., "prefer `path.join()` over string concatenation").

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for all `file`, `path`, `directory` methods; module-specific lint rules as `[[lint_rule]]` entries; WHAT/WHAT-INSTEAD/WHY for all new I/O errors; `Iterable<T>` / `FallibleIterable<T>` hover docs if they surface here; new stdlib example project under `examples/<pittsburgh-themed>/` (single-entry layout); extension version bump + screenshot; spec + design docs written.

---

## v0.6 — `math`

Self-contained module. sqrt, abs, min/max, floor/ceil/round, trig (sin/cos/tan and inverses), log/exp, pow, constants (`math.pi`, `math.e`, etc.).

Module-specific lint suggestions: "prefer `math.pi` over hardcoded 3.14159."

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` + `[[type_attached_constant]]` entries for all `math.*` methods and constants; lint rule for hardcoded approximations; WHAT/WHAT-INSTEAD/WHY for domain errors (sqrt of negative, etc.); stdlib example project; extension bump + screenshot; spec + design docs.

---

## v0.7 — CLI essentials (tight trio)

Three modules bundled — all about "running as a program":

- **`cli`** — argument parsing (positional + flags + options)
- **`env`** — environment variables (`env.get(name)`, `env.getOr(name, default)`, `env.set`)
- **`process`** — `process.exit(code)`, `process.pid`, `process.parentPid`, `process.startedAt`, `process.uptime`, `process.args` (raw argv), `process.workingDirectory`, `process.onShutdown(handler)`, `process.isRunning()` (graceful loop pattern)

See `design/stdlib/cli.md` for the full design (TBD when this version is up).

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for all `cli`, `env`, `process` methods; WHAT/WHAT-INSTEAD/WHY for all new errors (missing required arg, invalid env var type, etc.); stdlib example project; extension bump + screenshot; spec + design docs.

---

## v0.8 — `json`

Parse, stringify, prettify. Universal data interchange.

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for `json.*` methods; WHAT/WHAT-INSTEAD/WHY for parse errors (malformed JSON, type mismatch on deserialize); compiler-generated typed serializers per `design/stdlib-design.md` Rule 6 (no runtime reflection); lint suggestions if any; stdlib example project; extension bump + screenshot; spec + design docs.

---

## v0.9 — `date` + `duration` (tight pair)

Always paired. `date.now()`, `date.from()`, comparisons, formatting, parsing. `duration` construction, arithmetic, conversion (seconds/minutes/hours/days).

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for all `date`/`duration` methods; Rule 3 compliance (no platform-dependent locale/timezone defaults — explicit always); WHAT/WHAT-INSTEAD/WHY for parse errors; stdlib example project; extension bump + screenshot; spec + design docs.

---

## v0.10 — `db` (database)

**DuckDB and Postgres to start, in that priority order**: DuckDB first (embedded, in-process — easiest "hello world with a database"), then Postgres (network database, client/server). **All other drivers (MySQL, SQLite, MariaDB, MS SQL, etc.) deferred until after v1.0 launch.**

The `db` module is one of the most substantial stdlib entries — see `design/stdlib/database.md` for the full early design, including the embedded SQL syntax open question. Headline features:

- **Structured query layer** — type-safe, compiler-validated. Covers filters, joins, group by, aggregates, subqueries, window functions, `NOT EXISTS`/`EXISTS`, `HAVING`, `DISTINCT`, `LIMIT`/`OFFSET`. Types derived from migration history automatically — no separate model file.
- **Raw SQL escape hatch** — for what the structured layer can't express. Embedded SQL syntax (exact form TBD at design time — see `design/stdlib/database.md` "Embedded SQL Syntax + IDE Support"). Typed results always enforced even for raw queries. Injection structurally impossible: parameters always passed as separate typed values, never interpolated.
- **Iterator model** — `db.query()` returns a streaming iterator over a small ring buffer. `.collect()` is explicit opt-in, not default.
- **Direct wire-to-struct deserialization** — compiler generates a typed deserializer per shape at compile time. No intermediate allocation layer.
- **Migrations** — atomic (transactional DDL), pre-flight schema validation, content-hash migration tracking (not filename-based).
- **Compiler as query advisor** — N+1 detection, missing-index suggestions, query rewrite hints. Rule 11 format: WHAT/WHAT-INSTEAD/WHY at every warning.
- **Typed runtime errors** — `DatabaseError` with structured fields + Postgres error-code mapping to human-readable summaries and suggestions.

**IDE support for embedded SQL (ships with this version)**:
- Syntax coloring for SQL inside the raw escape hatch construct
- Format-on-save: SQL content formatted with standard SQL indentation, first keyword (`INSERT INTO`, `SELECT`, `FROM`, etc.) indented at surrounding `const`-indentation + project-standard indent width

**Hard dependencies**: v0.5 (file — schema snapshot files), v0.9 (date/duration — timestamp wire deserialization).

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for all `db.*` query methods; embedded SQL syntax highlighting wired in LSP (ships with this version per spec); WHAT/WHAT-INSTEAD/WHY for all DB errors (connection failed, type mismatch, N+1 detection hint); compiler-generated typed deserializers per `stdlib-design.md` Rule 6; IDE support for embedded SQL; stdlib example project; extension bump + screenshot; spec + design docs.

---

## v0.11 — `log` (basic)

`log.info()`, `log.warn()`, `log.error()`, `log.debug()`. Starter logging — the full framework (structured logging, sinks, filters, log levels per module) ships in v0.23.

Module-specific lint suggestion: "prefer `log.info()` over `print()` in non-test code when `log` module is available."

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for `log.*` methods; lint rule `prefer-log-over-print` in registry; `print()` → `log.*` code-action quick-fix wired in LSP; WHAT/WHAT-INSTEAD/WHY for misconfigured log output; stdlib example project; extension bump; spec + design docs.

---

## v0.12 — `random`

Tiny module. `random.int(min, max)`, `random.float()`, `random.choice(array)`, `random.shuffle(array)`, `random.seed(n)` for deterministic testing.

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for all `random.*` methods; stdlib example project; extension bump; spec + design docs. (Tiny module — checklist is short here.)

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

> **⚠️ DO NOT FORGET** (checklist at top): `test` keyword activated in registry (was `[[deferred_language_feature]]` — promote it); `[[primitive_intrinsic]]` entries for all assertion functions; `ynz test` CLI output follows WHAT/WHAT-INSTEAD/WHY format for assertion failures; assertion failures must show actual vs expected values clearly; extension integration (test results in Problems panel); stdlib example with a real test suite; spec + design docs.

---

## v0.14 — `regex`

Substantial design surface (engine choice, flags, captures, replace). Gets its own milestone.

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for `regex.*` methods; RE2/NFA-only enforcement (per `stdlib-design.md` Rule 7 — no backtracking engine, no backreferences); WHAT/WHAT-INSTEAD/WHY for invalid pattern errors and ReDoS-risk patterns; stdlib example project; extension bump; spec + design docs.

---

## v0.15 — `request` (outbound HTTP client)

Three-tier API design (see HTTP open question in `design/open-questions.md` to be designed when this version is up):

1. **High-level helpers** — `request.get(url)`, `request.post(url, body)`, `request.put(url, body)`, `request.delete(url)`, `request.websocket(url)`
2. **Mid-level builder** — `request.build()` returns a configurable Request value; configure step-by-step (`req.method("PATCH")`, `req.header(name, value)`, `req.timeout(5)`, `req.send()`) — no chaining per Golden Rule 7
3. **Low-level socket access** — `net.tcp.connect(host, port)` returning a raw socket. The floor of the user-accessible network stack (anything lower is FFI territory).

With TLS support from day 1 in this version.

**Naming note**: the module is called `request` (not `http`) so the direction is unambiguous on read — `request.get(url)` is clearly outbound. The inbound counterpart is `server` (v0.21). Shared `Request`/`Response` types (capital — they're types per Rule 13) live in both modules' surface; both directions touch them since HTTP semantics are direction-agnostic at the message level.

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for all `request.*` methods; TLS support wired from day one (no plain HTTP default); WHAT/WHAT-INSTEAD/WHY for network errors (timeout, DNS failure, non-2xx status); stdlib example project; extension bump; spec + design docs.

---

## v0.16 — `stats`

mean, median, mode, stddev, variance, percentile, histogram. Built on `math`.

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for all `stats.*` methods; WHAT/WHAT-INSTEAD/WHY for edge cases (empty array, single element); stdlib example project; extension bump; spec + design docs.

---

## v0.17 — `crypto` / `hash`

SHA-256, SHA-512, AES-GCM, HMAC, key derivation (PBKDF2/Argon2). Careful design needed.

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries; sensitive-type modifier applied to key/secret values so they auto-redact in logs; WHAT/WHAT-INSTEAD/WHY for misuse errors (wrong key size, wrong mode, etc.); stdlib example project; extension bump; spec + design docs. This module has the highest security surface — design review before execution plan.

---

## v0.18 — `compression`

gzip, zstd, optionally brotli. Wraps system libs via the compiler-internal FFI (since user-facing FFI is v2+).

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for `compression.*` methods; WHAT/WHAT-INSTEAD/WHY for decompression errors (corrupt data, wrong format); stdlib example project; extension bump; spec + design docs.

---

## v0.19 — `terminal`

ANSI colors, cursor positioning, terminal-size detection. For richer CLI output.

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for `terminal.*` methods; no-op behavior when stdout is not a TTY (piped output must not contain escape codes — this is a common gotcha); WHAT/WHAT-INSTEAD/WHY for terminal errors; stdlib example project; extension bump; spec + design docs.

---

## v0.20 — `csv`

Read, write, optionally streaming for huge files. Less common than JSON but useful.

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for `csv.*` methods; streaming mode must follow `FallibleIterable<T>` protocol (per `design/iterables.md`); WHAT/WHAT-INSTEAD/WHY for parse errors; stdlib example project; extension bump; spec + design docs.

---

## v0.21 — `server` (inbound HTTP server)

Builds on the `request` module (v0.15) — shares the `Request`/`Response` types and the underlying HTTP wire-protocol implementation. Adds the inbound side: routing, middleware, request/response handler abstractions. Substantial module.

**API shape (locked at v0.21 design time, not now):** module-level functions on the singleton `server` namespace — `server.route(method, path, handler)`, `server.middleware(fn)`, `server.listen(port)`. The single-server-per-process case is overwhelmingly the norm; multi-server is rare enough to handle as a v1+ extension if real demand surfaces.

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for `server.*` methods; `Request`/`Response` types updated in registry if any fields change from v0.15; WHAT/WHAT-INSTEAD/WHY for server errors (port in use, handler panic, malformed response); LSP autocomplete for route handler signatures; stdlib example project (a real HTTP server); extension bump; spec + design docs.

---

## v0.22 — Package manager

`ynz add`, `ynz remove`, `ynz update`, `ynz install` + lock file (`yinz.lock`, TOML format). See `design/packages.md` for the full design.

**Source types supported:** git URLs, local paths. No public registry yet — the registry is v1.2 (after the language stabilizes at v1.0).

Install mechanism targets bun-class speed (content-addressed cache, hard-links, parallel resolver).

**Multi-entry projects ship in v0.22 too** — the `[entries]` table in `yinz.toml` (one project, multiple named binaries) lands here alongside `[dependencies]`. Each named entry is a **ship**; the canonical folder convention is `ships/<name>/entrypoint.ynz` plus a top-level `shared/` for cross-ship code. See `examples/stadium-fleet/` for the layout and `design/open-questions.md` "Workspace / Multi-Package Projects" for the locked rationale.

**Why this version (late in the v0 train)**: There is no public release until v1.0. Shipping the package manager early would mean every pre-v1.0 breaking language change cracks every package — packages would live in an unstable language for ~17 releases. Landing it at v0.22 puts packages into a stable-ish language with the stdlib mostly built (so packages have real APIs to depend on), and gives the package manager one polish cycle before v1.0's backwards-compat promise kicks in. Per `design/versioning.md`, pre-v1.0 has no compatibility guarantee, so the early-shipped value (ecosystem bootstrap, "fill gaps with packages") doesn't accrue until there's a public community — which is v1.0.

> **⚠️ DO NOT FORGET** (checklist at top): `ynz add/remove/update/install` CLI help text follows WHAT/WHAT-INSTEAD/WHY for all errors (package not found, version conflict, network failure); lock file format documented in spec; multi-entry project layout documented in spec (`examples/stadium-fleet/` demo updated); extension integration (show installed packages, dependency warnings); spec + design docs.

---

## v0.23 — Logging framework

Structured logging on top of v0.11's basic `log` module. Sinks (file, stdout, syslog), filters, log levels per module, structured fields, contextual loggers.

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for new `log.*` methods beyond v0.11's basics; `prefer-log-over-print` lint rule from v0.11 updated if sink API changes how logging works; WHAT/WHAT-INSTEAD/WHY for sink configuration errors; stdlib example project; extension bump; spec + design docs.

---

## v0.24 — Process spawning

`process.spawn(cmd, args)`, pipes (stdin/stdout/stderr), signal handling beyond `onShutdown`. Distinct from v0.7's `process.exit/.pid/.isRunning` (which are about the current process).

> **⚠️ DO NOT FORGET** (checklist at top): `[[primitive_intrinsic]]` entries for `process.spawn` and pipe methods; WHAT/WHAT-INSTEAD/WHY for spawn errors (command not found, permission denied, pipe broken); `errors` keyword integration (spawned process error propagates as first-class error); stdlib example project; extension bump; spec + design docs.

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

- **Why this version**: Built-in `for` over collections (`array`, `fixed`, `map`, ranges) works without this. Built-in `for` over fallible iterables like `file.lines()` works in v0.5. Custom user types implementing the contracts is the extension that ships at v1.0.
- **Substitute used pre-v1.0**: Users with iterable-like data expose a `.items()` method returning `array<T>` and `for (item in foo.items())`. Lossy compared to true iteration (materializes the whole collection) but works.
- **Locked design**: See `design/iterables.md` and `spec/iterables.md`

### Deprecation marking (locked design, deferred from v0.1)

A way to mark stdlib functions / language features as deprecated, with compiler warnings on use.

- **Why this version**: Only relevant post-v1.0 when backwards-compatibility kicks in. v0.1 follows the no-backwards-compatibility-pre-release policy — breaking changes are fine.
- **Substitute used pre-v1.0**: None needed.
- **Locked design**: See `design/versioning.md` and `design/linting.md`

> **⚠️ DO NOT FORGET** (checklist at top): **full WHAT/WHAT-INSTEAD/WHY audit of every existing compile error** (v1.0 ships this as a named requirement — "All compile errors reviewed"); operator overloading registry entries (`follows Add`, `follows Subtract`, etc.); custom `Iterable<T>` contract entries; formal grammar doc written; extension v1.0 release with all screenshots refreshed; spec + design docs for operator overloading and custom iterables; v1.0 release blog post / announcement (public launch).

---

## v1.1 — Post-launch polish tooling

### `ynz doc` and `ynz repl` (locked design, deferred from v0.1)

- **`ynz doc`** — generate static API documentation from `///` doc comments
- **`ynz repl`** — interactive REPL for learning and exploration

- **Why this version**: Polish tooling. Not blocking development or language usability. Post-launch additions.
- **Substitute used pre-v1.1**: No static doc generation (read the source). No REPL (write a small script and `ynz run` it).
- **Trigger to land**: v1.1 (post-launch polish milestone).
- **Locked design**: See `spec/doc-comments.md`

> **⚠️ DO NOT FORGET** (checklist at top): `ynz doc` output format documented; `///` doc-comment registry entries if any new metadata fields are introduced; `ynz repl` tutorial mode follows WHAT/WHAT-INSTEAD/WHY for REPL-specific errors; extension integration for REPL (inline output, REPL panel); spec + design docs.

---

## v1.2 — Public package registry

Built in Yinz itself. The dogfood milestone — by v1.2 we have everything needed (server v0.21, file system v0.5, env v0.7, JSON v0.8, logging framework v0.23, crypto v0.17, compression v0.18, package manager v0.22), and building the registry in Yinz proves the language can build real services. If we can't, that's a signal the language has gaps to fix.

Registry isn't deployed publicly until shortly before this version ships. The language launch (v1.0) uses git URLs + local paths for packages until v1.2.

### Public Package Registry (locked design, deferred from v0.22)

Server-side infrastructure for hosting and serving Yinz packages — the `ynz add some-package` discovery + download flow against a public registry.

- **Why this version**: The language isn't publicly launched until v1.0. Before launch, breaking changes are fine; there's no community of authors to support. After v1.0 stabilizes, building the registry — in Yinz itself, as the project's first major dogfooding test — proves the language can build real services.
- **Substitute used pre-v1.2**: Package manager (v0.22) supports git URLs and local paths. `ynz add github:user/repo` works fine. Public registry isn't required for the package manager to be useful.
- **Trigger to land**: v1.2 milestone, after v1.0 launch stabilizes.
- **Locked design**: See `design/packages.md`

> **⚠️ DO NOT FORGET** (checklist at top): registry API surface documented in spec; `ynz add some-package` errors follow WHAT/WHAT-INSTEAD/WHY (package not found, version conflict, auth error); extension integration (browse packages, see README in hover); spec + design docs. This is dogfood — if the registry server itself produces bad errors or lacks LSP support, it undermines the whole teaching mission.

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

> **⚠️ DO NOT FORGET** (checklist at top): `[lint]` config schema documented in spec; config validation errors follow WHAT/WHAT-INSTEAD/WHY; LSP reads `yinz.toml` lint config and adjusts displayed diagnostics live; spec + design docs.

---

## v2+ — Deferred features

Headline entries with full rationale:

### Sized integer variants (locked design, deferred from v0.1)

Angle-bracket-parameterized sized integers (`int<8>`, `int<16>`, `int<32>`, `uint<8>`, `uint<16>`, `uint<32>`, `uint<64>`, etc.) and signed equivalents.

- **Why v2+**: Requires const generics (numeric values as type parameters) — a meaningful compiler sub-project that took Rust years to land well. v0.1 users have no legitimate need: FFI is deferred, binary protocols can use byte arrays + stdlib helpers.
- **Substitute used pre-v2**: Use plain `int` (= i64) for all whole numbers. Covers ±9.2 × 10^18 — bigger than any count a human writes by hand. Precision loss vs sized variants only matters at FFI boundaries, which aren't in v0.1.
- **Trigger to land**: Either (a) FFI work begins, OR (b) a real user workload needs to interop with a binary protocol that v0.1's byte-array helpers can't ergonomically handle.
- **Locked design**: See `design/numeric-types.md` and `spec/numeric-types.md`
- **Registry entries**: `[[deferred_language_feature]]` names `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` — DONE M1 P5a

### Sized float variants (locked design, deferred from v0.1)

Single-precision (32-bit) IEEE 754 binary float.

- **Why v2+**: Only essential for GPU compute and ML workloads (both v2+). For graphics/physics in v0.1, `float` (= f64) is overkill but works fine.
- **Substitute used pre-v2**: Use `float` (f64) for all binary-float math. Slower than f32 on SIMD-heavy workloads but correct.
- **Trigger to land**: GPU dispatch begins OR ML stdlib begins.
- **Locked design**: See `design/numeric-types.md` and `spec/numeric-types.md`
- **Registry entries**: `[[deferred_language_feature]]` names `f32`, `f64` — DONE M1 P5a

### Arbitrary-precision decimal (locked design, deferred from v0.1)

`number` precision larger than 4096 significant digits — true unbounded decimal arithmetic.

- **Why v2+**: 4096 digits handles every realistic scientific calculation (gravitational wave numerics top out at ~200 digits; even number-theory research rarely exceeds 2000). Unbounded precision means unbounded memory per value and unbounded per-operation time — breaks the language's "predictable performance" character. Real arbitrary-precision libraries (GMP, MPFR) are massive projects.
- **Substitute used pre-v2**: Use `number<N>` with N up to 4096. Compile error if a user tries `number<5000>` — error message explicitly points to this deferral.
- **Trigger to land**: A real user workload genuinely exceeds 4096 digits AND can't be restructured to fit. This is a deliberately high bar.
- **Locked design**: See `design/numeric-types.md` and `spec/numeric-types.md`
- **Registry entries**: No token reserved yet — no `[[deferred_language_feature]]` entry. Tracked in this doc only until a token/syntax is reserved.

### FFI (Foreign Function Interface) (locked design, deferred from v0.1)

The `foreign` keyword and machinery to call C / C++ / Rust libraries from Yinz.

- **Why v2+**: Significant design surface (ownership across the boundary, type mapping, safety guarantees). Most v0.1 code doesn't need it — stdlib is in scope. Stdlib internals can use compiler-private FFI without exposing a user-facing `foreign` keyword.
- **Substitute used pre-v2**: Stdlib modules that need C interop (file I/O, networking, math) call C internally via compiler-private mechanisms. Users don't see this. If a user genuinely needs to call a third-party C library, they have to wait for v2+ or contribute to the stdlib.
- **Trigger to land**: v2+ work begins OR a stdlib gap creates a real need.
- **Locked design**: See `design/ffi.md` and `spec/ffi.md`
- **Registry entries**: `[[deferred_language_feature]]` name `foreign` — DONE M1 P5b

### GPU dispatch (locked design, deferred from v0.1)

The `gpu` call-site keyword and kernel compilation to GPU shader / compute languages.

- **Why v2+**: Massive scope (kernel compilation, ABI design, runtime fallback). No v0.1 user has shown demand. Was tagged MVP2+ in original design.
- **Substitute used pre-v2**: None — `gpu` keyword is reserved but not parseable. Compile error if used.
- **Trigger to land**: v2+ AND a real ML/compute workload requires it.
- **Locked design**: See `design/gpu.md` and `spec/concurrency.md`
- **Registry entries**: `[[deferred_language_feature]]` name `gpu` — DONE M1 P5b

### ML stdlib (locked design, deferred from v0.1)

Tensors, neural network primitives, autodiff, optimizers.

- **Why v2+**: v0.1 stdlib focus is general-purpose. ML requires its own deep design (compatible with `float`/`f32`, GPU dispatch, etc.) and is a v2+ concern.
- **Substitute used pre-v2**: None. ML workloads run in Python until then.
- **Trigger to land**: v2+ AND GPU dispatch lands.
- **Locked design**: See `design/stdlib/ml.md`
- **Registry entries**: No `[[deferred_*]]` entry — covered by `[[deferred_stdlib_api]]` (RESERVED kind, zero M1 entries per schema). Populated in the ML stdlib milestone, not M1.

### Markets stdlib (locked design, deferred from v0.1)

Financial data ingestion, brokerage integrations, market data feeds.

- **Why v2+**: Niche stdlib module. Not load-bearing for v0.1.
- **Substitute used pre-v2**: None. Users write HTTP-based integrations directly.
- **Trigger to land**: v2+.
- **Locked design**: See `design/stdlib/markets.md`
- **Registry entries**: No `[[deferred_*]]` entry — covered by `[[deferred_stdlib_api]]` (RESERVED kind, zero M1 entries per schema). Populated in the Markets stdlib milestone, not M1.

### Self-hosted compiler (locked design, deferred from v0.1)

The Yinz compiler rewritten in Yinz (current bootstrap is in Rust).

- **Why v2+**: Need feature parity in v1.0+ stable Yinz first. Bootstrap-in-Rust serves us for years.
- **Substitute used pre-v2**: Rust bootstrap compiler — see `design/compiler-language.md`.
- **Trigger to land**: v2+ AND the language is stable enough to self-host.
- **Locked design**: See `design/compiler-language.md`
- **Registry entries**: No entry — compiler-internal milestone with no user-facing token reserved. No `[[deferred_language_feature]]` or `[[deferred_tooling_feature]]` entry applies.

---

## How to decide if a feature is v0.N or vN+1

Three questions:

1. **Does it stand alone as a focused thing?** If yes, candidate for its own version. If it's part of a tightly-coupled pair (file+path+directory; date+duration; cli+env+process), bundle them.
2. **Does the v0.1 syntax surface depend on it?** If yes, it's v0.1. Things that change every function signature (ownership) can't defer.
3. **Does it unlock the most value for the next version's work?** (Order versions for value compounding — LSP early so everyone benefits, stdlib ramp before the package manager so packages land into stable APIs rather than churning every release, etc.)

When in doubt, defer to a later version. We can always pull features forward when they prove easy; pushing them back is harder once users depend on them.

---

## Tight Pair Rationale

Some modules are bundled into one version because designing/shipping them separately would create awkward intermediate states:

| Bundle | Why bundled |
|--------|------------|
| `file` + `path` + `directory` (v0.5) | `file.read()` needs `path` for argument types. `directory` is the iteration form of `file`. All three together. |
| `cli` + `env` + `process` (v0.7) | Every CLI tool needs all three. Designing them together keeps conventions consistent. |
| `date` + `duration` (v0.9) | Durations only make sense in relation to dates. Arithmetic between them is the same module's concern. |
| LSP + `ynz watch` + `ynz fmt` (v0.2) | All dev-loop tooling — landing them together gives the dev experience a single noticeable jump rather than three small ones. |
