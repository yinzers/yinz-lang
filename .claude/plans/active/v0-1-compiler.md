---
slug: v0-1-compiler
owner: patrick
status: active
files:
  - Cargo.toml
  - crates/**
  - tests/**
  - rust-toolchain.toml
  - .github/workflows/**
created: 2026-05-12
last_updated: 2026-05-13
---

# Plan: v0.1 Compiler Implementation (Umbrella)

This is the **roadmap** for v0.1 — the milestone index, locked decisions, and shared quality gates. Each milestone has its own plan file:

- M1 — `.claude/plans/done/m1-hello-world.md` ✓
- M2 — `.claude/plans/done/m2-literals-arithmetic.md` ✓
- M3 — `.claude/plans/active/m3-control-flow-fns.md` (next — to be created)
- M4–M8 — not yet planned (one file per milestone when it becomes active)

When a new milestone starts, run `/plan` to create its plan file and the slug will rebind automatically.

---

## Context & Why

**Goal.** Build the Yinz v0.1 compiler — the first runnable slice of the Yinz language. v0.1's scope is "core language only, no stdlib" per `design/mvp-scope.md`. Output: an `ynz` CLI that can `build` and `run` programs written against the v0.1 language surface (variables, functions, types, ownership, generics, collections, options/unions/maybe, control flow, strings, errors, modules, concurrency keywords parsing sequentially, decimal128 numerics, doc comments, sensitive modifier).

**Background.** Yinz is a compiled systems language (LLVM-targeting, no GC, ownership-based). The compiler is written in Rust with `inkwell` for LLVM, `salsa` for incremental computation (also serves the v0.2 LSP), `ariadne` for diagnostics, and a hand-written recursive-descent parser (so error messages can carry position-specific suggestions per Golden Rule 11 — the compiler is a teacher).

**Constraints.**
- Rust stable toolchain.
- LLVM via `inkwell` — pinned to LLVM 18 (`llvm18-1-prefer-dynamic`).
- Salsa from day 1 (non-negotiable per `design/compiler-language.md` — retrofit cost would be a 6-month side-quest before v0.2 LSP).
- No external runtime dependencies in produced binaries except libc (and `libynz_rt.a` from M2 onward).
- Compiler-error format must follow the WHAT/WHAT-INSTEAD/WHY three-part shape from `design/compiler-errors.md` from day 1.

**Success criteria for the full v0.1 release:**
- `ynz run hello.ynz` works for every program covered by `spec/**/*.md` examples that don't import stdlib.
- All compiler errors follow the three-part format and pass an audit against `design/compiler-errors.md`'s banned-jargon list.
- Incremental rebuilds hit the sub-second target from `design/compiler.md` (single-file change, warm cache, typical project).
- The compiler is structured as queries (salsa) so v0.2 LSP can wrap them without restructuring.

---

## Locked Decisions (shared across milestones)

These were settled during M1/M2 and apply to every downstream milestone. Per-milestone decisions live in each milestone's plan file.

**Toolchain (locked M1):**
- Rust 1.95.0 stable, LLVM 18.1.8, inkwell 0.9.0 (`llvm18-1-prefer-dynamic` — Ubuntu packages don't ship static Polly), salsa 0.26.2, ariadne 0.6.0, clap 4.6.1, insta 1.47.2.
- `LLVM_SYS_181_PREFIX=/usr/lib/llvm-18` in `.cargo/config.toml` for Linux; macOS needs `brew --prefix llvm@18`.

**Crate layout (locked M1, extended M2):**
- `ynz-diagnostics` — three-part Diagnostic, DiagnosticBucket (50-cap), ariadne render, BANNED_JARGON, jargon audit test.
- `ynz-ast` — AST node types (extended each milestone).
- `ynz-parser` — salsa-tracked lex + parse queries, hand-written.
- `ynz-typeck` — salsa-tracked check query, `PrimitiveIntrinsicTable` (replaces M1's `BuiltinTable` in M2), scope.
- `ynz-codegen` — salsa-tracked codegen query, `emit_artifact` (inkwell, isolated), per-triple SHA-256 golden, IR snapshot.
- `ynz-numerics` (M2+) — pure-Rust IEEE 754 decimal128 (internal-use).
- `ynz-runtime` (M2+) — C-ABI shims, `staticlib` → `libynz_rt.a`.
- `ynz-driver` — `ynz build` + `ynz run` CLI, load/build/link/run, integration tests.

**Architectural patterns (locked M1):**
- Every cross-stage call goes through a salsa query (lex → parse → check → codegen).
- `inkwell::Module` / `inkwell::Context` confined to `crates/ynz-codegen/src/emit.rs` — never escape via public API.
- Codegen returns `Arc<CompiledArtifact> { object_bytes: Vec<u8>, ir_text: String, sha256: [u8; 32] }`.
- Object-file SHA-256 is the reproducibility contract (per target triple). IR text is informational.
- 50-error diagnostic cap with "and N more hidden" footer.
- All diagnostics use three-part WHAT/WHAT-INSTEAD/WHY format, enforced by Diagnostic constructor (panics on empty field).
- Banned-jargon audit runs in CI over all diagnostic call sites.

**Project structure (locked M1):**
- `yinz.toml` defines project root. Imports are root-relative. No `mod` declarations.
- Single-segment imports = stdlib, multi-segment = project files.

**Runtime ABI (locked M2):**
- C-ABI shims with pass-by-pointer for decimals: `extern "C" ynz_decimal_add(*const, *const, *mut)`.
- Caller-owned buffers for to-string functions; no heap allocation. Min sizes: int ≥ 24, float ≥ 32, decimal ≥ 48.
- Int overflow via `llvm.s{add,sub,mul}.with.overflow.i64` intrinsics, branch to runtime panic.
- Decimal-by-zero, int-by-zero → runtime panic. Float-by-zero → IEEE infinity.
- `libynz_rt.a` discovery: `crates/ynz-driver/build.rs` emits `cargo:rustc-env=YNZ_RT_LIB_DIR=...`.

---

## Roadmap (milestones)

### Milestone 1 (M1): Hello-world end-to-end — COMPLETE (2026-05-12)
End-to-end walking skeleton. `function main() -> nothing { print("hello, yinz") }` compiles and runs.
**Tag**: `v0.1.0-m1` at commit `820bfdc`.
**Plan**: `.claude/plans/done/m1-hello-world.md`.

### Milestone 2 (M2): Literals + variables + arithmetic — COMPLETE (2026-05-13)
`let` / `const`, integer / float / decimal literals, full operator set, polymorphic `print`, always-succeeds conversion intrinsics. Hand-rolled IEEE 754 decimal128 in `ynz-numerics` + `ynz-runtime`.
**Tag**: `v0.1.0-m2` at commit `c39fe8a`.
**Plan**: `.claude/plans/done/m2-literals-arithmetic.md`.

### Milestone 3 (M3): Control flow + user functions — multi-session
`if` / `else`, multi-case `if`, `for x in ...` (with a temporary `range` builtin until proper iterables), `while`, early `return`, user-defined functions with parameters and return types, block scoping.
**Status**: NEXT — to be planned.
**Plan**: `.claude/plans/active/m3-control-flow-fns.md` (pending).
**Depends on**: M2 ✓.

### Milestone 4 (M4): Types + ownership — multi-session
`type Foo { ... }` declarations with fields and methods. Ownership modifiers (`share`, `lend`, `give`, `copy`, `.freeze`). Ownership analysis as a salsa query. Heap allocation via libc `malloc`/`free`. Drop-on-scope-exit. Hardest milestone in v0.1 — ownership is the core safety property.
**Depends on**: M3.
**Catch-up obligations from M2**: overflow escape methods on `int`; `int.max` / `int.min` / `number.max` / `number.epsilon` constants; general method dispatch (replaces M2's intrinsic-table dispatch).

### Milestone 5 (M5): Generics + collections — multi-session
Function generics `function foo[T](...)` and type generics `array[T]` / `fixed[T]` / `map[K,V]`. Monomorphization. Bracket sugar (`arr[i]`, `m[k]`) desugars to `.get()` / `.set()`. `Iterable[T]` contract reserved for M7.
**Depends on**: M4.
**Note**: docs updated 2026-05-13 to use `<>` generics syntax; compiler implementation must follow `array<T>`, `map<K, V>`, `fixed<T>`, `function foo<T>()`.

### Milestone 6 (M6): Options + unions + maybe + narrowing — multi-session
`options Status { ... }` declarations, union types `A or B`, `maybe T` sugar for `T or none`, `if (x is Type)` pattern narrowing as a flow-sensitive analysis.
**Depends on**: M5.
**Catch-up obligations from M2**: `.toInt()` on number/float (returns `maybe int`); `string.toInt()` / `.toNumber()` / `.toFloat()`; compile-error suggestions for mixed-type arithmetic involving fallible directions.

### Milestone 7 (M7): Strings (full) + errors + iterables — multi-session
Full Unicode strings (`.get` for code points, `.byteAt`, `.graphemeAt`), interpolation, the `errors` keyword with flow-sensitive auto-propagation ("cascades"), `Iterable[T]` and `FallibleIterable[T]` contracts wired to `for x in iter` desugaring.
**Depends on**: M6.

### Milestone 8 (M8): Modules + remaining + v0.1 tag — multi-session
`import` / `export`, root-relative paths, aliases with `as`, duplicate-name compile error. Doc comments (`///`) parsed and preserved on signatures. Sensitive type modifier (auto-redact in print output). Concurrency keywords (`wait`, `background`) parse and type-check, run sequentially. **Bignum `number[N]` for N ∈ (34, 4096]** — multi-u128 coefficient path with mixed-precision promotion + narrowing-warning rounding (per `design/numeric-types.md` lines 65–78). Polish + audit + v0.1.0 tag.
**Depends on**: M7.
**Non-negotiable carry-from-M2**: `number[N]` for N > 34 is the load-bearing v0.1 promise (exact decimal at any reasonable precision). M2 reserves the syntax and emits a three-part error pointing here. M8 must close the loop before v0.1.0 ships.

---

## Shared Quality Checklist (applies to every milestone)

Every milestone PR must pass these before merging to main:

- [ ] All compiler-emitted diagnostics use the three-part WHAT/WHAT-INSTEAD/WHY format (Diagnostic struct enforces at construction).
- [ ] No banned-jargon strings in emitted diagnostics (enforced by `tests/jargon_audit.rs`).
- [ ] No `unwrap()` outside test code anywhere in `crates/`.
- [ ] No `panic!()` outside test code anywhere in `crates/`.
- [ ] All public APIs documented with at least one-line `///` doc comments.
- [ ] `cargo clippy --workspace -- -D warnings` passes.
- [ ] `cargo fmt --check` passes.
- [ ] All snapshot tests have inline `// WHY:` comments stating the invariant they protect.
- [ ] CI passes on both Linux and macOS.
- [ ] Salsa queries: every cross-stage call goes through a query.
- [ ] LLVM context lifetimes documented at the codegen module level (no `inkwell::Module` / `inkwell::Context` leak outside `emit.rs`).
- [ ] No dependency uses a `*` version constraint; `Cargo.lock` committed.
- [ ] Object-file SHA-256 reproducibility contract still holds (per target triple).
- [ ] Existing prior-milestone integration tests still pass.

Rust-conventions note: `~/.claude/rules/testing.md` is TypeScript-specific (`bun:test`, `*.spec.ts`); not applicable to this Rust codebase. The PRINCIPLES still apply (WHY-comments on tests, test the contract not the implementation, never weaken a test to make it pass).

---

## Anti-Pattern Callouts (shared)

- **Splitting into commits instead of PRs**: each phase is one PR with one branch. No phase mashes itself into "I'll split it later" — the branch name and est-lines are in the milestone plan, and each PR's scope is bounded by its phase block.
- **Shadow main branches**: every phase merges to `main` before the next starts. No long-lived umbrella branches.
- **Building the engine before shipping value**: every milestone produces a usable artifact, not a layer. After M1, we have a real binary. After M2, we have variables. Honest disclosure when infrastructure ships ahead of value (e.g., M1 P2 diagnostics) — it gets called out, not dressed up.
- **Hotfix that isn't**: N/A while there are no production users. Will revisit when v0.1 actually ships externally.
- **Abandoned branches**: each phase is single-session-scoped. Branches that go stale get merged or deleted at session end.
- **Flag graveyards**: N/A — the compiler doesn't use feature flags. `--release` is a user-facing build mode, not a feature flag.
- **Snapshot rot**: snapshot file updates require an inline `// test-ratchet: <reason>` marker AND a WHY-comment note explaining the invariant the snapshot protects. Mechanical hook enforcement on Edit/Write.

---

## Reviewer Disputes

(none currently outstanding)
