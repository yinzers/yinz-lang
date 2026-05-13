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

**Performance**: `.get(index)` compiles to a bounds check + conditional. In debug mode, this is always present. In release mode, the compiler eliminates bounds checks it can prove are safe statically (fixed-size arrays with known indices). The performance impact is negligible in practice.

**Consistency**: Maps already use `.get(key)` returning `maybe V`. Same pattern everywhere. No special case for arrays.
