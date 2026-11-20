---
name: "IMP-compiler"
description: "Design document for compiler and IDE language server implementation. Not user-facing."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Compiler Design — Performance, Incremental Builds, IDE

Design document for compiler and IDE language server implementation. Not user-facing.

---

## Primary Design Goal — Fast Incremental, Thorough Full Builds

**Runtime performance is never sacrificed for compile speed.** LLVM optimization, ownership analysis, monomorphization — all of this takes time and stays in the full build.

**Incremental build speed is aggressively optimized.** A developer changing one file should not wait for the whole project to recompile. The goal: sub-second recompile for single-file changes in typical projects.

---

## Compile-Speed Targets

Yinz targets two speeds:

- **Full builds** keep every optimization, including ownership analysis and per-type specialization, and may take Rust-like time: they happen once, then incremental takes over.
- **Incremental builds** recompile only what changed (see "Incremental Compilation Strategy").

---

## Build Modes

**What ships today:** `ynz build` always runs the LLVM optimization pipeline. `ynz build --no-optimize` compiles at `-O0` for faster compiles; program behavior is identical in both modes. There is no `--release` flag yet (`crates/ynz-driver/src/main.rs`; `--reveal-sensitive` is documented as stripped from release builds "when it ships").

**Designed, not built:**

- `ynz build --release`: full LLVM optimization, the mode for production binaries.
- A statically linked binary against `musl libc` by default for `--release`, so the binary runs on any Linux kernel from 3.2 with no glibc version dependency. `ynz build --dynamic-glibc` is the escape hatch for programs that need glibc-specific behavior (locale support beyond musl, NSS plugins). musl's slower allocator is covered by a per-request arena allocator ([`SCRATCH-future-arena.md`](../scratchpad/SCRATCH-future-arena.md)); its DNS limits by the network stdlib using its own resolver (decided at the `request` module).
- The `--kernel` build, which uses no libc at all ([`IMP-no-runtime-mode.md`](IMP-no-runtime-mode.md)).

---

## Generics & Monomorphization — Deduplication by Default

Yinz generates a specialized version per concrete generic instantiation (each concrete type combination produces its own specialized machine code), so generic code has no runtime dispatch cost. To keep compile time from growing with the number of files that use the same instantiation, the compiler tracks generated instantiations in a **shared instantiation cache** keyed by `(type-path, concrete-type-arguments)` and deduplicates identical instantiations across the build graph.

> **Internal terminology note**: this technique is called *monomorphization* in compiler literature ("one shape per concrete type"). Banned from user-facing diagnostics, IDE hints, and spec docs per Golden Rule 5. Use "specialized version per type" in any text a Yinz user might read.

### Why deduplication is the default

Without deduplication every file that uses `Array<String>` would compile its own copy. With it, identical instantiations from different files share LLVM IR: compiled once, linked everywhere. Specialization keeps its speed and compile time stops scaling with the number of users of a type.

### Auto-promotion (codegen-only)

Per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md):
- **Codegen**: shared instantiation cache active by default in all build modes. Always applies when the compiler proves two instantiations are identical-modulo-call-site (same type, same generic arguments, same `follows` constraints).
- **Muted IDE hint**: not applicable — there's no source-level "rewrite to share" form. The user wrote idiomatic generic code; the compiler handled the dedup.
- **Tier 3 lint suggestion**: not applicable — same reason.

### When dedup doesn't apply (designed, not built)

Shared instantiations cannot be optimized differently per call site (no per-caller inline decisions, no per-caller constant-propagation across the generic boundary). A release-performance flag (name not final) is designed to disable deduplication and generate a fresh specialized version per call site: slower compile, potentially faster runtime for hot generic code. The standard release build keeps deduplication on.

### Salsa integration

The deduplication cache is keyed by Salsa queries ([`docs/internal/implementation/IMP-compiler-language.md`](IMP-compiler-language.md)) so it persists across builds. An identical instantiation from unchanged source need not be recompiled. Combined with Yinz's incremental compilation, this means: edit one file, recompile that file's changes only, and any unchanged-but-shared generic instantiations stay cached.

### Cross-references

- [`docs/internal/implementation/IMP-compiler-language.md`](IMP-compiler-language.md) for the Salsa caching model.

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

`ynz watch` is a long-running daemon holding one `CompilerDb`. File saves mutate `SourceFile.text` salsa inputs; salsa invalidates and recomputes only the changed query results. Target: sub-second warm rebuild for single-file edits.

Quick-start: `ynz watch foo.ynz` — builds + re-runs on every save. `--check` skips the run step. `--json` emits a structured NDJSON event stream on stdout.

See [`docs/internal/implementation/IMP-watch.md`](IMP-watch.md) for full architecture: daemon model, file-watcher integration, child process lifecycle, `--json` schema, memory-defense layers (LRU caps + periodic DB rebuild + RSS polling), debounce strategy, and cross-platform notes.

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

`items[5]` reads through `.get(5)` and returns `Maybe<T>`; there is no bare read that can run past the end (see [`IMP-collections.md`](IMP-collections.md), "Bracket Sugar"). A write past the end is a runtime error, or a compile error when the index is a literal on a `Fixed<T>` of known size.

**Why**: out-of-bounds access is a common cause of crashes and buffer overflows, so the compiler makes the safe form the only form (Golden Rule 2).

**Performance**: `.get(index)` compiles to a bounds check and a branch. The designed optimization (not built) is an index-range proof pass over the Yinz IR before LLVM sees it, removing checks it can prove safe (a `Fixed<T>` with a known index), including patterns LLVM's own analysis misses. Maps follow the same pattern: `.get(key)` returns `Maybe<V>`.
