# Design Decisions — Index

Every design topic has its own file. This is the index. One line per topic, link to the file.

---

## Cross-Cutting Architectural Principles (apply to every design decision)

| Principle | Canonical file | What's in it |
|-----------|----------------|--------------|
| **Non-OOP model** | `.claude/rules/non-oop.md` | Locked r10–r13 (2026-05-16). Data shapes + standalone functions + UFCS dot-call sugar. NO methods inside shapes. NO `override` keyword. `extends` is data-only inheritance. `follows` checked by structural function-signature matching. `dynamic Foo` for runtime polymorphism. Drift back into OOP is the most common modeling mistake. |
| **Dot-postfix rule** | `.claude/rules/dot-postfix.md` | Locked r4 (2026-05-15). `value.field` (no parens) = data access; `value.method()` (parens) = action. Applies to UFCS method calls, `.copy()`, `.freeze()`, intrinsics, type-attached constants. Does NOT apply to ownership modifiers (compiler-inferred at call sites only). |
| **Examples use real operations** | `.claude/rules/dot-postfix.md` | Every example in spec/design/plan/rule files MUST use real Yinz operations from the current scope — no invented APIs for illustration. |

## Language Design

> **NOTE**: rows below predate the r10–r13 non-OOP lockdown. Doc-PR 2 (Task #8) rewrites `design/type-system.md`, `spec/types.md`, `design/ownership.md`, `spec/ownership.md`, `spec/operators.md`, `design/iterables.md`, `spec/iterables.md`, and `spec/overview.md` to remove methods-inside-shapes, remove `override`, redocument `extends` as data-only, remove body-level `.share()/.lend()/.give()` syntax, and update all examples. Index rows here will be updated when those rewrites land.

| Topic | File | What's in it |
|-------|------|--------------|
| Type system | `design/type-system.md` | `shape`, `base`, `extends` (data-only inheritance), `follows` (structural function-signature matching), structural typing, `\|` (unions), `maybe`, `options`, `hidden` (per-field visibility within exported shapes). ⚠️ Removed by r10-r15: `override` keyword (function overloading by argument type), scalar type aliases (`shape UserId = string` — pure documentation sugar, banned: parameter names + comments do the job). **Supported**: union aliases (`shape Result = Success \| Failure`) — these are real type unions shipped in M6, not scalar sugar. Doc-PR 2 (Task #8) updates this file. |
| Generics | `design/generics.md` | Type generics `<T>`, function generics, `follows` constraints inline, type inference at call sites |
| Ownership | `design/ownership.md` | `.share`/`.lend`/`.give`/`.copy`/`.freeze`, no direct array indexing |
| Collections | `design/collections.md` | `fixed`/`array`/`map`, no chaining, method naming, bracket sugar for `.get()`/`.set()`, string indexing methods |
| Error handling | `design/errors.md` | `errors` keyword, no try/catch, flow-sensitive auto-propagation narrowing |
| Functions | `design/functions.md` | `function` keyword, `-> nothing`, no tuples, closure syntax |
| Numeric types | `design/numeric-types.md` | `number`/`float`/`int`, `number<N>` parameterized precision (cap 4096), handwritten impls, overflow methods |
| Naming | `design/naming.md` | Human-readable keywords, capital = type rule, comments syntax |
| Options types | `design/options.md` | `options` keyword (replaces `enum`), LLVM i8 lowering, exhaustiveness enforcement, ambiguous-shorthand resolution, function-vs-shorthand priority, single/empty-variant rejection, `.toString()` via global variant-name table |
| Union types | `design/unions.md` | `\|` union syntax, LLVM lowering decision table (pointer-niche vs tagged-struct), `is`-exact-type rule (no subtype), exhaustiveness enforcement, single-variant rejection, no user-layout-override rationale |
| Flow-sensitive narrowing | `design/narrowing.md` | Full rules table for `.value` and `is` narrowing: positive/negative forms, `&&` propagation, `\|\|` non-propagation (locked diagnostic text), early-return narrowing (recognized-exit set), reassignment invalidation, lend-call invalidation, closure non-propagation, v0.2 LSP hint obligations |
| Control flow | `design/control-flow.md` | No standalone `else`, multi-case `if`, exhaustiveness, jump table optimization |
| Scope | `design/scope.md` | Block scoping, no mutable globals, const expressions, export for sharing |
| Main entry | `design/main-entry.md` | `function entrypoint()`, file from yinz.toml, args from stdlib, errors to default handler |
| Doc comments | `design/doc-comments.md` | Go-model `//` leading comments (no `///`, no block docs), exported items only, field documentation. `ynz doc` generator design in `design/future/doc-generator.md`. |
| Testing | `design/testing.md` | Built-in `test` keyword, setup/teardown (file+per-test), single-level groups, `assertFails`/`assertPanics`, file-level parallelism |
| Packages | `design/packages.md` | `ynz add/remove/update`, lock file, yinz_modules, tree shaking |
| Operators | `design/operators.md` | `follows` contracts, `Self` keyword, `print()` default, `&&`/`\|\|`/`!` symbols, bitwise symbols, no `===` |
| Sensitive values | `design/sensitive.md` | `sensitive` modifier, auto-redact in all output, `.reveal()` explicit opt-in, stripped from release |
| FFI | `design/ffi.md` | `foreign` keyword, wrap in safe functions, compiler requires `wait` (DEFERRED to v2+) |
| Iterables | `design/iterables.md` | `follows Iterable<T>`, `next()` with `maybe T`, hidden state fields |
| GPU dispatch | `design/gpu.md` | MVP2+ vision: `gpu` call-site keyword, compiler manages CPU/GPU dispatch (DEFERRED to v2+) |
| Destructuring | `design/destructuring.md` | Object only, no array, `as` rename, parameter destructuring |
| Inline / anonymous shape types | `design/inline-shape-types.md` | `{ field: T }` in type-annotation position; structural typing (two identical inline shapes are the same type); canonical-name hoisting implementation; no `hidden` in inline shapes; named shapes remain nominal |
| Type conversion | `design/type-conversion.md` | Dot methods, no `as` keyword, safe vs unsafe split, no ternary |
| Concurrency | `design/concurrency.md` | Auto-parallelization, `wait`, `background`, ownership with tasks (optimization DEFERRED to v0.3) |
| Strings (internal) | `design/strings.md` | UTF-8 internal encoding locked, UTF-8 file I/O default, SIMD-accelerated validation/traversal target |
| Modules | `design/modules.md` | `import`/`export`, no defaults, no wildcards, root-relative paths, stdlib auto-import |
| Configuration | `design/config.md` | Three layers, TOML choice, no env splitting, `set` functions |
| Linting & build | `design/linting.md` | "Compiler IS the linter" — three-tier diagnostics (errors/warnings/suggestions), curated v0.4 rule list, three-part WHAT/INSTEAD/WHY format, customization v1.x |
| Versioning | `design/versioning.md` | Pre-release delete policy, post-release major bumps, no backwards compat |

## Compiler & Tooling

| Topic | File | What's in it |
|-------|------|--------------|
| Compiler design | `design/compiler.md` | Incremental builds, IDE language server, no-indexing rationale |
| Compiler implementation language | `design/compiler-language.md` | Rust + Salsa + inkwell + ariadne + hand-written recursive descent — decision and rationale |
| MVP scope | `design/mvp-scope.md` | Granular 24-version sequence to v1.0 + 3 post-launch versions |
| Compiler error style | `design/compiler-errors.md` | Required three-part WHAT/WHAT-INSTEAD/WHY format, jargon ban-list, tone guide, multi-error strategy |
| Cross-module frame layout (M3e) | `design/future/cross-module-frame-serialization.md` | Codegen-side `frame_layouts_query` (salsa, `ynz-codegen`) — NOT export-table serialization in typeck. Forced by: (1) separate compilation → no shared LLVM module; (2) shape ABI sizes need LLVM `TargetData` → cannot compute accurately in `ynz-typeck`. One LLVM-accurate computation, used by both emitter and importer (no-duct-tape #7 — kills the lossy typeck reimplementation). Approved 2026-06-05 (Patrick). |

## Reference

| Topic | File | What's in it |
|-------|------|--------------|
| Golden rules | `design/golden-rules.md` | All 13 rules with full reasoning. Rule 11 expanded — teaching mission. Rule 8 clarification block (zero-cost meaning). Rule 12 union-syntax exception (`\|` not `or`). |
| Teaching mission | `design/teaching-mission.md` | First-class language goal — compiler as mentor, three-part diagnostic format, IDE as a teaching surface (muted-hint protocol), university-adoption aspiration |
| IDE hints protocol | `design/ide-hints.md` | Muted-text protocol for the v0.2 LSP — what gets hinted, styling rules, tooltip format, the click-to-make-explicit guarantee |
| Open questions | `design/open-questions.md` | Unresolved design decisions |
| MVP Scope | `design/mvp-scope.md` | Per-version feature breakdown v0.1–v1.2 + v2+, including deferred features with substitutes and triggers |

## Future Designs (locked, awaiting implementation milestone)

| Topic | File | What's in it |
|-------|------|--------------|
| Future index | `design/future/index.md` | TOC for all future-locked designs + parking lot for ideas not yet committed |
| Concurrency (no function coloring) | `design/no-function-coloring.md` | v0.2 — whole-program may-block analysis, auto-inserted `wait`, FFI annotation, stackless state machines |
| Panic safety | `design/future/panic-safety.md` | v0.2 — task-isolated panics, no try/catch, no mutex poisoning, drop-on-scope-exit cleanup, supervisor pattern |
| Supervisor helpers | `design/future/supervisor.md` | v0.2 — stdlib `supervise.alwaysRestart` / `.withBackoff` / `.maxRestarts`, default-supervision meta-rule for stdlib long-running loops |
| Self-referential shapes | `design/future/self-references.md` | v0.3+ — Approach A (relative/offset pointers). Compiler auto-detects, `self-referential` modifier as muted IDE hint. Rejection rationale for B (fix-up on move) and C (pin-in-place). |
| No-runtime mode | `design/no-runtime-mode.md` | v0.3 — `--kernel` flag, plug-in runtime architecture (user provides allocator/scheduler/panic handler/output). Chipset, kernel, NASA-grade targets. |
| Arena allocators | `design/future/arena.md` | v0.2 (A1/A2) + v0.3+ (B). `arena scratch {}` scope blocks ship v0.2 — 10-100x faster than malloc for scope-bounded workloads. Compiler internals should adopt arenas in M8 polish. |
| HTTP framework | `design/future/http-framework.md` | v0.3+ — supervised-by-default HTTP server. Per-request task isolation, accept-loop supervision, custom `supervise:` config option, default 500 handler. |
| Compiled-package binary format | `design/future/packages.md` | v0.1 binary-format reservation + v0.2 implementation. May-block metadata, ownership signatures, kernel-mode compatibility flags, allocator requirements, LLVM attribute hints, self-referential markers, doc comments per exported item. |
| Release-mode builds | `design/future/release-mode.md` | v0.4+ — `--release` flag: LLVM `-O3`, strip debug info, disable dev-only flags (`--reveal-sensitive`, `--emit-ir`). Strips dev-only env-var checks via `cfg(release_build)`. |
| String `{ptr, len}` overhaul | `design/future/string-ptr-len-overhaul.md` | TBD (likely v0.5 alongside file I/O) — migrate strings from NUL-terminated C strings to `{ptr, len}` slices. Removes embedded-NUL footgun, makes `length` O(1). Multi-day rewrite. |
| macOS platform support | `design/future/macos-platform-support.md` | Deferred — macOS removed from CI 2026-06-01 (codegen golden tests are x86_64-linux-pinned; some macOS failures hint at real codegen differences unverifiable from Linux). Linux x86_64 is the only verified target. Re-add `macos-latest` once macOS codegen is validated + per-triple goldens recorded on a Mac. |

## Standard Library

All stdlib design lives in `design/stdlib/`. See `design/stdlib/overview.md` for the index.
