# Compiler Design — Performance, Incremental Builds, IDE

Design document for compiler and IDE language server implementation. Not user-facing.

---

## Primary Design Goal — Fast Incremental, Thorough Full Builds

**Runtime performance is never sacrificed for compile speed.** LLVM optimization, ownership analysis, monomorphization — all of this takes time and stays in the full build.

**Incremental build speed is aggressively optimized.** A developer changing one file should not wait for the whole project to recompile. The goal: sub-second recompile for single-file changes in typical projects.

---

## Lessons From Other Languages

### Why Rust is slow to compile:
1. **Monomorphization** — every generic usage (`array<Player>`, `array<string>`) generates separate compiled code. Multiplies the work significantly.
2. **Borrow checker** — deep per-reference analysis to prove safety across the whole function.
3. **LLVM optimization** — multi-pass, heavy optimization produces fast binaries at the cost of compile time.
4. **Whole-crate compilation** — Rust compiles large units at once rather than file-by-file.

### Why Go is fast to compile:
1. Simple type system — less analysis per file.
2. Purpose-built compiler — optimized for compile speed from the start, not bolted on.
3. File-level compilation — change one file, recompile that file.

### Yinz targets both:
- **Full builds**: accept Rust-like compile times — they happen once, incremental takes over.
- **Incremental builds**: target Go-like speed — only recompile what changed.
- **Release builds**: full LLVM optimization. Accept the time — this is for production.

---

## Build Modes

### Debug (default — `ynz build`)
- Minimal LLVM optimization
- Fast compile
- Decent runtime speed
- Used during development

### Release (`ynz build --release`)
- Full LLVM optimization
- Slow compile
- Maximum runtime speed
- Used for production binaries

Debug and release produce different binaries. Development always uses debug. Deployment always uses release.

---

## Static Linking — `musl` libc as Default

`ynz build --release` produces a **statically-linked binary** by default, linking against `musl libc`. The binary runs on any Linux kernel ≥ 3.2 with no glibc version dependency.

Dynamic glibc linking is available via `ynz build --dynamic-glibc` for users who explicitly need glibc-specific behavior (locale support beyond what musl provides, certain NSS plugins, etc.). The `--kernel` build (per `design/future/no-runtime-mode.md`) uses no libc at all.

### Why musl + static by default

Rust's default (`glibc` dynamic linking) produces binaries built on Ubuntu 22.04 (`glibc 2.35`) that fail to run on RHEL 7 (`glibc 2.17`) with `GLIBC_2.35 not found`. Python's `manylinux` infrastructure exists entirely to work around this — building wheels on ancient CentOS images to target the oldest supported glibc. Go made the opposite choice in 2009: static linking by default, binaries run anywhere with the right architecture and kernel. Result: "Go binaries work everywhere" became a marketing sentence as well as a correctness property.

Yinz inherits Go's choice. "`ynz build` produces a binary that runs anywhere" is high ergonomic value, especially for a language being adopted by jr devs who don't yet understand glibc symbol versioning.

### Tradeoff

`musl libc` has known performance differences from `glibc` in some areas:
- `musl`'s malloc is notably slower than `glibc`'s tcmalloc-derived allocator for multithreaded workloads. **Mitigated** by Yinz's per-request arena allocator (per `design/future/arena.md`) — most allocation paths in Yinz code don't use libc malloc directly.
- DNS resolution has known limitations (musl doesn't support `/etc/nsswitch.conf` plugins, doesn't fall back to TCP for large responses on older versions). **Mitigated** by Yinz's network stdlib using its own resolver where appropriate (decision deferred to v0.16 http milestone).

For workloads where these specific musl limitations matter, `--dynamic-glibc` is the escape valve. The default favors deployability.

### Cross-references

- `lockin-build-and-crossplat.md` Finding #12 for glibc version skew details.
- `design/future/no-runtime-mode.md` for the `--kernel` build mode (no libc at all).

---

## Generics & Monomorphization — Deduplication by Default

Yinz generates a specialized version per concrete generic instantiation (each concrete type combination produces its own specialized machine code) — same model as Rust, same model as C++ templates. To prevent the compile-time scaling problem Rust hits, the compiler tracks generated instantiations in a **shared instantiation cache** keyed by `(type-path, concrete-type-arguments)` and deduplicates identical instantiations across the build graph.

> **Internal terminology note**: this technique is called *monomorphization* in compiler literature ("one shape per concrete type"). Banned from user-facing diagnostics, IDE hints, and spec docs per Golden Rule 12. Use "specialized version per type" in any text a Yinz user might read.

### Why deduplication is the default

Rust generates a separate compiled copy of every generic instantiation in every crate that uses it. Ten crates all using `Vec<String>` produce ten compiled copies. Feldera documented Rust compile times of 25-45 minutes on a 64-core machine for their project before restructuring (https://www.feldera.com/blog/cutting-down-rust-compile-times-from-30-to-2-minutes-with-one-thousand-crates). Restructuring into 1,000 crates dropped compile time from 30 to 2 minutes — 15× improvement by changing crate boundaries, not the language.

Yinz inherits the perf benefits of specialized-per-type generation (no runtime dispatch overhead) but avoids the quadratic scaling by deduplicating. Identical instantiations from different files share LLVM IR — compiled once, linked everywhere.

### Auto-promotion (codegen-only)

Per `.claude/rules/auto-promotion.md`:
- **Codegen**: shared instantiation cache active by default in all build modes. Always applies when the compiler proves two instantiations are identical-modulo-call-site (same type, same generic arguments, same `follows` constraints).
- **Muted IDE hint**: not applicable — there's no source-level "rewrite to share" form. The user wrote idiomatic generic code; the compiler handled the dedup.
- **Tier 3 lint suggestion**: not applicable — same reason.

### When dedup doesn't apply (release-perf escape hatch)

Shared instantiations cannot be optimized differently per call site (no per-caller inline decisions, no per-caller constant-propagation across the generic boundary). For performance-critical instantiations where per-site optimization matters, `ynz build --release-perf` (final flag name TBD) disables deduplication and generates a fresh specialized version per call site — slower compile, potentially faster runtime for heavily-templated hot code. Standard `ynz build --release` keeps deduplication active because the typical perf delta is small and the compile-time savings are large.

### Salsa integration

The deduplication cache is keyed by Salsa queries (`design/compiler-language.md`) so it persists across builds. An identical instantiation from unchanged source need not be recompiled. Combined with Yinz's incremental compilation, this means: edit one file, recompile that file's changes only, and any unchanged-but-shared generic instantiations stay cached.

### Cross-references

- `lockin-build-and-crossplat.md` Finding #13 for Rust's specialized-per-type compile-scaling pain (called "monomorphization" in the source — same concept).
- `design/compiler-language.md` for the Salsa caching model.

---

## Incremental Compilation Strategy

### What gets cached between builds:
- **Type signatures** (Pass 1 output) — cached per file. Only invalidated when exports change.
- **Ownership proofs** — cached per function. Only reruns when function body changes.
- **Compiled output** — cached per file. Only reruns when source changes.
- **Import graph** — tracks which files depend on which. Determines what to invalidate on change.

### What triggers recompilation:
- Changed file → recompile that file
- Changed file's exports changed → recompile all importers (transitively)
- Changed file's exports unchanged (internal refactor) → only recompile that file

The multi-pass type system supports this: Pass 1 (type signatures) is almost always cached. Only Pass 3 (function body compilation) reruns for changed files.

### Parallelism:
- Independent files compile in parallel across CPU cores.
- The import graph determines which files are independent.
- Files with no shared dependencies compile simultaneously.

---

## Watch Mode Implementation

`ynz watch` runs a file system watcher. On change:

1. Detect changed files via OS file watcher (inotify/kqueue/FSEvents)
2. Walk the import graph to find affected files
3. Invalidate caches for affected files
4. Recompile incrementally (in parallel where possible)
5. If `--run`: restart the running process with the new binary

Target: sub-second full cycle for single-file changes with warm cache.

---

## IDE Language Server — Incremental Analysis

The language server (LSP implementation) does incremental analysis, not full-project reanalysis on every keystroke.

### Rust-analyzer approach (what to emulate):
- Demand-driven analysis — only analyze what the IDE needs right now
- Salsa-style incremental computation — cache everything, invalidate only what changed
- File-level granularity — change one file, re-analyze that file and its immediate dependents

### What the IDE computes incrementally:
- **Type errors** — per function, re-runs when function body changes
- **Ownership violations** — per function, re-runs when function body or its callees change
- **Unused imports** — per file, lightweight
- **Missing return paths** — per function
- **Autocomplete** — served from the export index (always cached, updated on save)

### What's NOT incremental (runs on save, not on keystroke):
- Duplicate code detection — requires cross-file analysis, heavier
- Execution plan visualization — requires full dependency graph for the function
- Project-wide unused export analysis — requires full import graph traversal

### Performance targets:
- Autocomplete response: < 50ms
- Error highlighting after keystroke: < 100ms
- Full file re-analysis on save: < 500ms
- Project-wide analysis (duplicate detection, unused exports): < 2s for typical projects

These are targets, not guarantees. The architecture should be designed to hit them in typical project sizes (< 100k lines of Yinz code).

---

## IDE Export Index

The export index is the data structure that powers autocomplete and auto-import. Built at project load time, updated incrementally on file save.

Contents:
- Every exported symbol (functions, types, options) across all project files
- Every standard library symbol (always present — stdlib is built into the compiler)
- Every symbol from installed packages

The index maps: symbol name → file path → type signature.

IDE uses this for:
- Autocomplete: type a name, index returns all matching exports and their types
- Auto-import: type an unknown name, index finds which file to import from
- Hover documentation: index returns the type signature and any doc comments

---

## No Direct Array Indexing — Safety Rationale

`items[5]` is a compile error. All collection access goes through `.get(index)` which returns `maybe T`.

**Why**: Out-of-bounds array access is one of the most common causes of runtime crashes and security vulnerabilities (buffer overflows) in systems languages. If the compiler can enforce safe access universally, there's no reason not to. The cost (slightly more verbose access) is far outweighed by the elimination of an entire crash category.

**Performance**: `.get(index)` compiles to a bounds check + conditional. In debug mode, this is always present. In release mode, the compiler eliminates bounds checks it can prove are safe statically (fixed-size arrays with known indices). Critically, Yinz runs its own index-range proof pass before emitting LLVM IR — LLVM's alias analysis fails to eliminate bounds checks for certain index patterns (notably widening-multiply computations like `(a as int) * b`), but Yinz's type-level integer-range tracking can prove these safe at the Yinz IR level before LLVM ever sees them. The performance impact is negligible in practice.

**Consistency**: Maps already use `.get(key)` returning `maybe V`. Same pattern everywhere. No special case for arrays.
