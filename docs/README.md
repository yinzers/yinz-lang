---
name: "README"
description: "Every design topic has its own file. This is the index. One line per topic, link to the file."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-04"
status: "active"
author: "patrick"
metadata:
  type: "index"
---

# Design Decisions — Index

Every design topic has its own file. This is the index. One line per topic, link to the file.

---

## Cross-Cutting Architectural Principles (apply to every design decision)

| Principle | Canonical file | What's in it |
|-----------|----------------|--------------|
| **Non-OOP model** | [`.claude/rules/non-oop.md`](../.claude/rules/non-oop.md) | Locked r10–r13 (2026-05-16). Data shapes + standalone functions + UFCS dot-call sugar. NO methods inside shapes. NO `override` keyword. `extends` is data-only inheritance. `follows` checked by structural function-signature matching. `dynamic Foo` for runtime polymorphism. Drift back into OOP is the most common modeling mistake. |
| **Dot-postfix rule** | [`.claude/rules/dot-postfix.md`](../.claude/rules/dot-postfix.md) | Locked r4 (2026-05-15). `value.field` (no parens) = data access; `value.method()` (parens) = action. Applies to UFCS method calls, `.copy()`, `.freeze()`, intrinsics, type-attached constants. Does NOT apply to ownership modifiers (compiler-inferred at call sites only). |
| **Examples use real operations** | [`.claude/rules/dot-postfix.md`](../.claude/rules/dot-postfix.md) | Every example in spec/design/plan/rule files MUST use real Yinz operations from the current scope — no invented APIs for illustration. |

## Language Design

| Topic | File | What's in it |
|-------|------|--------------|
| Type system | [`docs/internal/implementation/IMP-type-system.md`](internal/implementation/IMP-type-system.md) | `shape`, `base`, `extends` (data-only inheritance), `follows` (structural function-signature matching), structural typing, `\|` (unions), `maybe`, `options`, `hidden` (per-field visibility within exported shapes). Removed by r10-r15 (landed): `override` keyword (function overloading by argument type), scalar type aliases (`shape UserId = string` — pure documentation sugar, banned: parameter names + comments do the job). **Supported**: union aliases (`shape Result = Success \| Failure`) — these are real type unions shipped in M6, not scalar sugar. |
| Generics | [`docs/internal/implementation/IMP-generics.md`](internal/implementation/IMP-generics.md) | Type generics `<T>`, function generics, `follows` constraints inline, type inference at call sites |
| Ownership | [`docs/internal/implementation/IMP-ownership.md`](internal/implementation/IMP-ownership.md) | `.share`/`.lend`/`.give`/`.copy`/`.freeze`, no direct array indexing |
| Collections | [`docs/internal/implementation/IMP-collections.md`](internal/implementation/IMP-collections.md) | `fixed`/`array`/`map`, no chaining, method naming, bracket sugar for `.get()`/`.set()`, string indexing methods |
| Array element storage (by-value, v0.3-M5) | [`docs/internal/implementation/IMP-collections.md`](internal/implementation/IMP-collections.md#array-element-storage--by-value-inline-v03-m5) | `array<Shape>` elements stored by value inline in the heap buffer (elem_size-aware ABI) — fixes the stack-dangling class the M3a guard masked; one-allocation buffer, copy-on-persist snapshot semantics (with TS-aliasing teaching note), field-wise value `contains`, element-blind drop parity, serialization forward-compat |
| Auto-SoA layout (v0.3-M5) | [`docs/internal/implementation/IMP-collections.md`](internal/implementation/IMP-collections.md#auto-soa-layout-v03-m5) | Qualifying `array<Shape>` stored as per-field segments in one allocation; admission criteria (provable length > 64, no growth, no escape, ≤2-field union), padding-wins layout authority, kernel-mode gate, Tier 3 lint `array-using-soa-layout`, honest O0/-O2 performance provenance |
| Maybe / optional values | [`docs/internal/implementation/IMP-maybe.md`](internal/implementation/IMP-maybe.md) | `maybe T` sugar for `T \| none`, LLVM lowering decision table, flow-sensitive `.value` narrowing rules, `none`-inference rules, v0.1 cycle-leak limitation (M5) |
| Error handling | [`docs/internal/implementation/IMP-errors.md`](internal/implementation/IMP-errors.md) | `errors` keyword, no try/catch, flow-sensitive auto-propagation narrowing |
| Functions | [`docs/internal/implementation/IMP-functions.md`](internal/implementation/IMP-functions.md) | `function` keyword, `-> nothing`, no tuples, closure syntax |
| Numeric types | [`docs/internal/implementation/IMP-numeric-types.md`](internal/implementation/IMP-numeric-types.md) | `number`/`float`/`int`, `number<N>` parameterized precision (cap 4096), handwritten impls, overflow methods |
| Naming | [`docs/reference/REF-naming.md`](reference/REF-naming.md) | Human-readable keywords, capital = type rule, comments syntax |
| Options types | [`docs/internal/implementation/IMP-options.md`](internal/implementation/IMP-options.md) | `options` keyword (replaces `enum`), LLVM i8 lowering, exhaustiveness enforcement, ambiguous-shorthand resolution, function-vs-shorthand priority, single/empty-variant rejection, `.toString()` via global variant-name table |
| Union types | [`docs/internal/implementation/IMP-unions.md`](internal/implementation/IMP-unions.md) | `\|` union syntax, LLVM lowering decision table (pointer-niche vs tagged-struct), `is`-exact-type rule (no subtype), exhaustiveness enforcement, single-variant rejection, no user-layout-override rationale |
| Flow-sensitive narrowing | [`docs/internal/implementation/IMP-narrowing.md`](internal/implementation/IMP-narrowing.md) | Full rules table for `.value` and `is` narrowing: positive/negative forms, `&&` propagation, `\|\|` non-propagation (locked diagnostic text), early-return narrowing (recognized-exit set), reassignment invalidation, lend-call invalidation, closure non-propagation, v0.2 LSP hint obligations |
| Control flow | [`docs/internal/implementation/IMP-control-flow.md`](internal/implementation/IMP-control-flow.md) | No standalone `else`, multi-case `if`, exhaustiveness, jump table optimization |
| Scope | [`docs/internal/implementation/IMP-scope.md`](internal/implementation/IMP-scope.md) | Block scoping, no mutable globals, const expressions, export for sharing |
| Main entry | [`docs/internal/implementation/IMP-main-entry.md`](internal/implementation/IMP-main-entry.md) | `function entrypoint()`, file from yinz.toml, args from stdlib, errors to default handler |
| Doc comments | [`docs/internal/implementation/IMP-doc-comments.md`](internal/implementation/IMP-doc-comments.md) | Go-model `//` leading comments (no `///`, no block docs), exported items only, field documentation. `ynz doc` generator design in [`docs/internal/scratchpad/SCRATCH-future-doc-generator.md`](internal/scratchpad/SCRATCH-future-doc-generator.md). |
| Testing | [`docs/internal/implementation/IMP-testing.md`](internal/implementation/IMP-testing.md) | Built-in `test` keyword, setup/teardown (file+per-test), single-level groups, `assertFails`/`assertPanics`, file-level parallelism |
| Packages | [`docs/internal/implementation/IMP-packages.md`](internal/implementation/IMP-packages.md) | `ynz add/remove/update`, lock file, yinz_modules, tree shaking |
| Operators | [`docs/internal/implementation/IMP-operators.md`](internal/implementation/IMP-operators.md) | `follows` contracts, `Self` keyword, `print()` default, `&&`/`\|\|`/`!` symbols, bitwise symbols, no `===` |
| Sensitive values | [`docs/internal/implementation/IMP-sensitive.md`](internal/implementation/IMP-sensitive.md) | `sensitive` modifier, auto-redact in all output, `.reveal()` explicit opt-in, stripped from release |
| FFI | [`docs/internal/implementation/IMP-ffi.md`](internal/implementation/IMP-ffi.md) | `foreign` keyword, wrap in safe functions, compiler requires `wait` (DEFERRED to v2+) |
| Iterables | [`docs/internal/implementation/IMP-iterables.md`](internal/implementation/IMP-iterables.md) | `follows Iterable<T>`, `next()` with `maybe T`, hidden state fields |
| GPU dispatch | [`docs/internal/implementation/IMP-gpu.md`](internal/implementation/IMP-gpu.md) | MVP2+ vision: `gpu` call-site keyword, compiler manages CPU/GPU dispatch (DEFERRED to v2+) |
| Destructuring | [`docs/internal/implementation/IMP-destructuring.md`](internal/implementation/IMP-destructuring.md) | Object only, no array, `as` rename, parameter destructuring |
| Inline / anonymous shape types | [`docs/internal/implementation/IMP-inline-shape-types.md`](internal/implementation/IMP-inline-shape-types.md) | `{ field: T }` in type-annotation position; structural typing (two identical inline shapes are the same type); canonical-name hoisting implementation; no `hidden` in inline shapes; named shapes remain nominal |
| Type conversion | [`docs/internal/implementation/IMP-type-conversion.md`](internal/implementation/IMP-type-conversion.md) | Dot methods, no `as` keyword, safe vs unsafe split, no ternary |
| Concurrency | [`docs/internal/implementation/IMP-concurrency.md`](internal/implementation/IMP-concurrency.md) | Auto-parallelization, `wait`, `background`, ownership with tasks. Auto-parallelization shipped in v0.3 (I/O-overlap + pure-CPU statement parallelization). |
| Strings (internal) | [`docs/internal/implementation/IMP-strings.md`](internal/implementation/IMP-strings.md) | UTF-8 internal encoding locked, UTF-8 file I/O default, SIMD-accelerated validation/traversal target |
| Modules | [`docs/internal/implementation/IMP-modules.md`](internal/implementation/IMP-modules.md) | `import`/`export`, no defaults, no wildcards, root-relative paths, stdlib auto-import |
| Configuration | [`docs/internal/implementation/IMP-config.md`](internal/implementation/IMP-config.md) | Three layers, TOML choice, no env splitting, `set` functions |
| Linting & build | [`docs/internal/implementation/IMP-linting.md`](internal/implementation/IMP-linting.md) | "Compiler IS the linter" — three-tier diagnostics (errors/warnings/suggestions), curated v0.4 rule list, three-part WHAT/INSTEAD/WHY format, customization v1.x |
| Versioning | [`docs/internal/decisions/ADR-versioning.md`](internal/decisions/ADR-versioning.md) | Pre-release delete policy, post-release major bumps, no backwards compat |

## Compiler & Tooling

| Topic | File | What's in it |
|-------|------|--------------|
| Compiler design | [`docs/internal/implementation/IMP-compiler.md`](internal/implementation/IMP-compiler.md) | Incremental builds, IDE language server, no-indexing rationale |
| Compiler implementation language | [`docs/internal/decisions/ADR-compiler-language.md`](internal/decisions/ADR-compiler-language.md) | Rust + Salsa + inkwell + ariadne + hand-written recursive descent — decision and rationale |
| MVP scope | [`docs/reference/REF-mvp-scope.md`](reference/REF-mvp-scope.md) | Granular 24-version sequence to v1.0 + 3 post-launch versions |
| Compiler error style | [`docs/reference/REF-compiler-errors.md`](reference/REF-compiler-errors.md) | Required three-part WHAT/WHAT-INSTEAD/WHY format, jargon ban-list, tone guide, multi-error strategy |
| Formatter (`ynz fmt`) | [`docs/internal/implementation/IMP-fmt.md`](internal/implementation/IMP-fmt.md) | Zero-config canonical formatting, `ynz-fmt` crate, comment attachment rules, LSP format-on-save consumer |
| Language server (`ynz-lsp`) | [`docs/internal/implementation/IMP-lsp.md`](internal/implementation/IMP-lsp.md) | Salsa-backed JSON-RPC server, capability negotiation, go-to-def/find-refs/rename/format/inlay-hints/code-actions, self-hosting migration plan |
| Watch daemon (`ynz watch`) | [`docs/internal/implementation/IMP-watch.md`](internal/implementation/IMP-watch.md) | Long-running rebuild-on-save + re-run daemon, shared `CompilerDb` with the LSP, cross-platform process management |
| Feature registry | [`docs/internal/implementation/IMP-feature-registry.md`](internal/implementation/IMP-feature-registry.md) | [`registry/features.toml`](../registry/features.toml) SSOT for keywords/jargon/intrinsics/deferred-features/hint-domains, `ynz-registry` crate codegen, carve-out policy |
| Cross-module frame layout (M3e) | [`docs/internal/scratchpad/SCRATCH-future-cross-module-frame-serialization.md`](internal/scratchpad/SCRATCH-future-cross-module-frame-serialization.md) | Codegen-side `frame_layouts_query` (salsa, `ynz-codegen`) — NOT export-table serialization in typeck. Forced by: (1) separate compilation → no shared LLVM module; (2) shape ABI sizes need LLVM `TargetData` → cannot compute accurately in `ynz-typeck`. One LLVM-accurate computation, used by both emitter and importer (no-duct-tape #7 — kills the lossy typeck reimplementation). Approved 2026-06-05 (Patrick). |

## Reference

| Topic | File | What's in it |
|-------|------|--------------|
| Golden rules | [`docs/reference/REF-golden-rules.md`](reference/REF-golden-rules.md) | All 13 rules with full reasoning. Rule 11 expanded — teaching mission. Rule 8 clarification block (zero-cost meaning). Rule 12 union-syntax exception (`\|` not `or`). |
| Teaching mission | [`docs/reference/REF-teaching-mission.md`](reference/REF-teaching-mission.md) | First-class language goal — compiler as mentor, three-part diagnostic format, IDE as a teaching surface (muted-hint protocol), university-adoption aspiration |
| IDE hints protocol | [`docs/reference/REF-ide-hints.md`](reference/REF-ide-hints.md) | Muted-text protocol for the v0.2 LSP — what gets hinted, styling rules, tooltip format, the click-to-make-explicit guarantee |
| Open questions | [`docs/internal/scratchpad/SCRATCH-open-questions.md`](internal/scratchpad/SCRATCH-open-questions.md) | Unresolved design decisions |
| MVP Scope | [`docs/reference/REF-mvp-scope.md`](reference/REF-mvp-scope.md) | Per-version feature breakdown v0.1–v1.2 + v2+, including deferred features with substitutes and triggers |

## Future Designs (locked, awaiting implementation milestone)

| Topic | File | What's in it |
|-------|------|--------------|
| Future index | [`docs/internal/scratchpad/SCRATCH-future-designs-index.md`](internal/scratchpad/SCRATCH-future-designs-index.md) | TOC for all future-locked designs + parking lot for ideas not yet committed |
| Concurrency (no function coloring) | [`docs/internal/implementation/IMP-no-function-coloring.md`](internal/implementation/IMP-no-function-coloring.md) | v0.2 — whole-program may-block analysis, auto-inserted `wait`, FFI annotation, stackless state machines |
| Panic safety | [`docs/internal/scratchpad/SCRATCH-future-panic-safety.md`](internal/scratchpad/SCRATCH-future-panic-safety.md) | v0.2 — task-isolated panics, no try/catch, no mutex poisoning, drop-on-scope-exit cleanup, supervisor pattern |
| Supervisor helpers | [`docs/internal/scratchpad/SCRATCH-future-supervisor.md`](internal/scratchpad/SCRATCH-future-supervisor.md) | v0.2 — stdlib `supervise.alwaysRestart` / `.withBackoff` / `.maxRestarts`, default-supervision meta-rule for stdlib long-running loops |
| Self-referential shapes | [`docs/internal/scratchpad/SCRATCH-future-self-references.md`](internal/scratchpad/SCRATCH-future-self-references.md) | v0.3+ — Approach A (relative/offset pointers). Compiler auto-detects, `self-referential` modifier as muted IDE hint. Rejection rationale for B (fix-up on move) and C (pin-in-place). |
| No-runtime mode | [`docs/internal/implementation/IMP-no-runtime-mode.md`](internal/implementation/IMP-no-runtime-mode.md) | v0.3 — `--kernel` flag, plug-in runtime architecture (user provides allocator/scheduler/panic handler/output). Chipset, kernel, NASA-grade targets. |
| Arena allocators | [`docs/internal/scratchpad/SCRATCH-future-arena.md`](internal/scratchpad/SCRATCH-future-arena.md) | v0.2 (A1/A2) + v0.3+ (B). `arena scratch {}` scope blocks ship v0.2 — 10-100x faster than malloc for scope-bounded workloads. Compiler internals should adopt arenas in M8 polish. |
| HTTP framework | [`docs/internal/scratchpad/SCRATCH-future-http-framework.md`](internal/scratchpad/SCRATCH-future-http-framework.md) | v0.3+ — supervised-by-default HTTP server. Per-request task isolation, accept-loop supervision, custom `supervise:` config option, default 500 handler. |
| Compiled-package binary format | [`docs/internal/scratchpad/SCRATCH-future-packages.md`](internal/scratchpad/SCRATCH-future-packages.md) | v0.1 binary-format reservation + v0.2 implementation. May-block metadata, ownership signatures, kernel-mode compatibility flags, allocator requirements, LLVM attribute hints, self-referential markers, doc comments per exported item. |
| Release-mode builds | [`docs/internal/scratchpad/SCRATCH-future-release-mode.md`](internal/scratchpad/SCRATCH-future-release-mode.md) | v0.4+ — `--release` flag: LLVM `-O3`, strip debug info, disable dev-only flags (`--reveal-sensitive`, `--emit-ir`). Strips dev-only env-var checks via `cfg(release_build)`. |
| String `{ptr, len}` overhaul | [`docs/internal/scratchpad/SCRATCH-future-string-ptr-len-overhaul.md`](internal/scratchpad/SCRATCH-future-string-ptr-len-overhaul.md) | TBD (likely v0.5 alongside file I/O) — migrate strings from NUL-terminated C strings to `{ptr, len}` slices. Removes embedded-NUL footgun, makes `length` O(1). Multi-day rewrite. |
| macOS platform support | [`docs/internal/scratchpad/SCRATCH-future-macos-platform-support.md`](internal/scratchpad/SCRATCH-future-macos-platform-support.md) | Deferred — macOS removed from CI 2026-06-01 (codegen golden tests are x86_64-linux-pinned; some macOS failures hint at real codegen differences unverifiable from Linux). Linux x86_64 is the only verified target. Re-add `macos-latest` once macOS codegen is validated + per-triple goldens recorded on a Mac. |
| GUI & cross-platform apps | `design/future/gui/` (folder) | Post-v0.5 — webview-hosted native shell (Tauri/Capacitor model): one HTML/CSS/JS frontend (user's framework), compiled to native binaries for web/desktop/iOS/Android. Yinz owns the shell + IPC bridge + device-capability layer + per-platform compile (incl. WASM web target), NOT the frontend framework. Decision rationale (covers most use cases + feasible to build) + rejected alternatives (pixel-perfect renderer, "use Flutter") + the "can't snapshot React into native" locked reasoning all in `architecture.md`. Logic is Rust-class native; UI is webview-class (far faster than Electron). |

## Standard Library

All stdlib design lives in `design/stdlib/`. See [`docs/internal/scratchpad/SCRATCH-stdlib-overview.md`](internal/scratchpad/SCRATCH-stdlib-overview.md) for the index.
