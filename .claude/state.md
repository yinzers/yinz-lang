# Session State: ynz

**Last Updated**: 2026-05-12

---

## Active Workstreams

*(auto-rebuilt by SessionStart hook from `.claude/plans/active/*.md` front-matter — do not edit by hand)*

<!-- RADAR-START -->
*(no active workstreams)*
<!-- RADAR-END -->

---

## Environment & Commands (CRITICAL — survives compaction)

**Project**: ynz
**Package Manager**: {bun/npm/yarn/pnpm}
**Container**: {yes/no — exec pattern if yes}
**Database**: {connection / db name}

```bash
# Common commands
```

---

## Active Decisions (append with WHY)

- [2026-05-12] **Compiler implementation language = Rust**: Mature LLVM bindings (inkwell), strong ADT/pattern-matching for AST, salsa framework gives incremental builds + LSP "for free." See `design/compiler-language.md`.
- [2026-05-12] **MVP scope split into v0.1 / v0.2 / v0.3 / v1.0 / v2+**: Concurrency keywords parse from day 1 but run sequentially until v0.3 (when auto-parallelization optimization engages). See `design/mvp-scope.md`.
- [2026-05-12] **Error auto-propagation = flow-sensitive narrowing (Option B under, Option A in feel)**: If user calls `.failed()` before using the success value, auto-propagation suppressed; otherwise compiler auto-propagates at first use. Same `.failed()`/`.or()` API works inside AND outside `errors` functions. See `design/errors.md`.
- [2026-05-12] **Generic functions = v0.1, `[T]` syntax with `follows` constraints inline**: Type inference at call sites. `where` clauses rejected — inline keeps constraint visible next to the parameter. See `design/generics.md`.
- [2026-05-12] **Numeric types = handwritten, validated against IEEE 754 test vectors**: `number` = decimal128 (default), `number[N]` up to N=4096, `float` = f64, `int` = i64. Sized variants (`int[N]`, `f32`) deferred. Overflow panics by default with `.wrappingAdd()`/`.saturatingAdd()` escape valves. See `design/numeric-types.md` + `design/deferrals.md`.
- [2026-05-12] **Strings use `.get()` (code point) + `.byteAt()` + `.graphemeAt()`**: No `char` type. Default indexing is by Unicode code point. Bytes and graphemes are explicit alternates. See `spec/strings.md`.
- [2026-05-12] **Bracket sugar for `.get()` and `.set()` on all collections AND maps**: `arr[i]`, `m["key"]`, `s[i]` all desugar to `.get()`. Writes via `arr[i] = v` desugar to `.set()`. Strings immutable (no write sugar). Types reject bracket access entirely — forces dot for fields. Reverses earlier no-`map[key]` decision. See `design/collections.md`.
- [2026-05-12] **Iterable contract = two types (`Iterable[T]`, `FallibleIterable[T]`)**: In-memory collections follow `Iterable[T]`; I/O sources follow `FallibleIterable[T]`. Same `for` syntax; compiler infers fallibility from the source's contract and auto-propagates errors when needed. Stdlib adapters `.orSkipFailures()` and `.withErrors()` for ergonomic fallible-to-infallible conversion. See `design/iterables.md` + `spec/iterables.md`.
- [2026-05-12] **Import aliases + duplicate-name compile error**: TS-style `{ name as renamed }` and `namespace as renamed`. Duplicate names (including stdlib-vs-local collisions) refuse to silently pick — compile error forces aliasing. See `design/modules.md` + `spec/modules.md`.
- [2026-05-12] **Lock file = TOML, flat array of `[[package]]` tables**: Same format as `yinz.toml`. Diff-friendly, manually editable in emergencies. Install mechanism (content-addressed global cache, hard-links, parallel resolver, lazy integrity) aims for bun-class speed — v0.5 work. See `design/packages.md` + `spec/packages.md`.
- [2026-05-12] **Granular versioning sequence (23 versions to v1.0 + 3 post-launch versions)**: Each version ships ONE focused thing (single module, tight pair, or compiler-infrastructure feature). v0.1 = core language only. v0.2 = LSP+watch+fmt. v0.3 = auto-parallelization. v0.4 = linting tier. v0.5 = package manager. v0.6-v0.23 = stdlib modules one-at-a-time. v1.0 = stability + operator overloading + custom iterables + grammar lock (public launch). v1.1 = ynz doc + ynz repl. v1.2 = public package registry (dogfooded in Yinz). v1.x = lint customization config. v2+ = FFI, GPU, ML, etc. See `design/mvp-scope.md` for the full sequence.
- [2026-05-12] **"Compiler IS the linter" — no separate `ynz lint` command**: Linting is a third tier of compiler diagnostics (suggestions, alongside errors and warnings). No plugin API. Customization via `[lint]` in `yinz.toml` ships v1.x. Extreme orgs can `[lint] enabled = false` and run their own lint package. Initial rule set (errors/warnings/suggestions) curated in `design/linting.md`. Module-specific rules ship attached to each module's version.
- [2026-05-12] **Testing — setup/teardown, single-level groups, assertFails + assertPanics, file-level parallelism**: `setup` (per-test) and `setup file` (per-file). Optional `group "name" { ... }` blocks, no nesting allowed. `assertFails` catches ONLY errors-system failures; `assertPanics` catches ONLY panics — separate so test bugs that panic always propagate visibly. Files run in parallel by default, tests within a file run sequentially. `--serial` flag to force all-serial. Ships v0.13. See `design/testing.md` + `spec/testing.md`.
- [2026-05-12] **`process` module (v0.8) — minimal API**: `process.exit(code)`, `process.pid`, `process.parentPid`, `process.startedAt`, `process.uptime`, `process.args` (raw argv), `process.workingDirectory`, `process.onShutdown(handler)`, `process.isRunning()`. No `process.env` (use `env` module). No per-signal handlers in v0.8 (just `onShutdown` for SIGTERM/SIGINT/SIGHUP). Process spawning deferred to v0.23. See open-questions.md (now resolved).
- [2026-05-12] **`running()` = `process.isRunning()` (module method, not builtin)**: Top-level globals reserved for things every program needs (`print`, `panic`). `process.isRunning()` returns false on shutdown signals — pairs with `process.onShutdown()` for callback-style cleanup. Works in any context. 30s hard timeout before forced termination if loops don't yield. Updates `spec/linting.md` to use the new form.
- [2026-05-12] **Teaching mission codified as a first-class language goal**: Created `design/teaching-mission.md`. Expanded Rule 11 (compiler is a teacher) to require all diagnostics follow WHAT/WHAT-INSTEAD/WHY three-part format. Added 4th decision criterion in `.claude/rules/language-design.md`: "Does this teach the user something, or just hide complexity?" Long-term aspiration: Yinz becomes a CS-101 teaching language (production-grade AND approachable). Updated project `CLAUDE.md` Rule 11.
- [2026-05-12] **Compiler error style spec + jargon audit**: Created `design/compiler-errors.md` — the canonical style spec for all compiler diagnostics. Includes required three-part WHAT/WHAT-INSTEAD/WHY format, banned-jargon list ("propagate", "narrow", "infer", "polymorphic", etc.), tone guide, multi-error strategy. Audited existing `spec/**/*.md` and `design/**/*.md` for jargon in user-facing error messages and prose — rewrote `spec/errors.md`, `spec/control-flow.md`, `spec/unions.md`, `spec/type-conversion.md`, `spec/main.md`, `spec/testing.md`, `spec/types.md`, `spec/functions.md` to remove banned terms. "Auto-propagation" kept as Yinz's official feature name but must be explained in plain English on first use. Final compiler-errors audit deferred — `spec/linting.md` notes its catalog examples are abbreviated.

---

## Superseded / Archived

- (none)

---

## Project-Wide Notes

*(cross-workstream context, gotchas, user preferences not tied to one plan)*
