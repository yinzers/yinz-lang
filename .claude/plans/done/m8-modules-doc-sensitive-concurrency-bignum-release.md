---
slug: m8-modules-doc-sensitive-concurrency-bignum-release
type: execution
owner: patrick
status: done
roadmap: v0-1-compiler
depends_on: [m7-strings-errors-iterables]
files:
  - crates/**
  - examples/pirates-roster/**
  - examples/primantis-orders/m8_errors.ynz
  - examples/multi-file/**
  - design/modules.md
  - design/doc-comments.md
  - design/sensitive.md
  - design/concurrency.md
  - design/numeric-types.md
  - spec/modules.md
  - spec/doc-comments.md
  - spec/sensitive.md
  - spec/concurrency.md
  - spec/numeric-types.md
  - CHANGELOG.md
  - Cargo.toml
  - .claude/plans/active/v0-1-compiler.md
created: 2026-05-18
last_updated: 2026-05-18-r2
---

# Plan: M8 — Modules, Doc Comments, Sensitive, Concurrency Keywords, Bignum, v0.1.0 Release

**Milestone**: 8 of 8 in `v0-1-compiler` roadmap.
**Goal**: Ship the final v0.1 language surface — modules + doc comments + sensitive modifier + `wait`/`background` concurrency keywords (sequential semantics) + bignum `number<N>` for N ∈ (34, 4096] — then audit + tag **v0.1.0**.
**Branches**: one feature branch per phase, prefix `feat/m8-*` (release phase = `release/v0.1.0`).
**Status**: pending_approval

---

## Context & Why

### What's already done (M1–M7)
Yinz now compiles end-to-end with strings, errors, iterables, generics, options, unions, shapes, ownership, control flow, and numerics (decimal128 capped at 34 digits). 782 tests green at `v0.1.0-m7`. Every milestone has shipped a runnable demo and an error gallery.

### What's missing for v0.1
Five feature areas remain before v0.1 can ship:

1. **Modules** — currently the driver compiles ONE `.ynz` file. Real projects need `import`/`export` across files. Single-file is the duct-tape-est thing in the compiler today.
2. **Doc comments** — `///` is currently silently treated as `//` by the lexer. The spec says doc strings attach to exported items and survive through the AST for the future `ynz doc` tool (v1.1).
3. **`sensitive` modifier** — type-system surface so secret values auto-redact in `print()` + interpolation. The env-based source (`env.get()` returns `sensitive string`) ships v0.7; M8 ships the type machinery + the manual `sensitive(literal)` constructor.
4. **Concurrency keywords** — `wait` and `background` must PARSE and TYPE-CHECK so code can be written today; real auto-parallelization arrives v0.3. M8 = parse + typeck + sequential lowering (`wait foo()` = `foo()`; `background foo()` = `foo()` with return discarded; background ownership rules per design/concurrency.md enforced).
5. **Bignum `number<N>` for N > 34** — the M2 load-bearing carry-over. Multi-u128 coefficient + bignum add/sub/mul/div + mixed-precision promotion + narrowing-warning rounding. This is the v0.1 "exact decimal at any reasonable precision" promise; if M8 drops anything, it isn't this.

Plus the syntax migration `number[N]` → `number<N>` (parser is out of sync with design/spec; M5 locked `<>` for generics in 2026-05-17), banned-jargon additions for concurrency + visibility vocabulary, the demo extension (`examples/pirates-roster/` becomes a multi-file project), the error gallery (`examples/primantis-orders/m8_errors.ynz`), an audit sweep, and the **v0.1.0** tag.

### Why now
M7 shipped 2026-05-18; this is the next milestone on the v0-1-compiler roadmap. Patrick's instruction: "if M8 has to drop something, it isn't bignum." Master plan line 211: bignum is non-negotiable because v0.1 promises exact-decimal-at-any-reasonable-precision.

### Constraints
- Each phase = one PR. Patrick reviews each phase's demo extension + error-gallery extension before merge.
- Bouncer enforces the 6-subsection `## Invariants This Milestone Must Preserve` block (Safety / Performance / Teaching / Runtime Dependencies / Kernel-Mode Behavior / Demo & Error Gallery).
- No mocking the test database (per `rules/testing.md`) — Yinz tests use `cargo test` on real codegen output via the LLVM toolchain.
- Pre-v1.0 policy: breaking changes are fine. We can migrate `number[N]` → `number<N>` without a deprecation alias.
- v0.1.0 is INTERNAL — no public launch, no docs site. v1.0 is the public launch.

### Success criteria
- All five M8 feature areas implemented per spec, with passing fixtures.
- Multi-file `examples/pirates-roster/` project compiles + runs end-to-end demonstrating M1–M8.
- `examples/primantis-orders/m8_errors.ynz` triggers every M8 compile-error class.
- All 782 M7 tests still pass; M8 adds an additional ~120-200 tests.
- `m2_bignum_deferral` fixture either deleted or updated (the catch-up marker M2 left).
- `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo fmt --check` all green.
- `Cargo.toml` at `0.1.0`. `v0.1.0` tag pushed. CHANGELOG entries for M1–M8.
- `/audit` sweep run on the integrated v0.1 surface; all CRITICAL findings fixed.

---

## Research Findings

### Current compiler state (surveyed 2026-05-18)

| Area | State | M8 work needed |
|---|---|---|
| Lexer (`crates/ynz-parser/src/lexer.rs`) | 64 tokens; `//` swallows everything to newline (no `///` distinction); no keyword reservation for wait/background/sensitive/import/export | Add 6 keyword tokens + `DocComment(String)` trivia + add concurrency/visibility banned-keyword diagnostics. |
| Driver (`crates/ynz-driver/src/main.rs`) | Single-file: `Cli::Run { file: PathBuf }` reads one file, `SourceFile::new(&db, name, text)` creates one input | Detect project root (find nearest `yinz.toml`); walk `src/**/*.ynz`; build N salsa inputs; gate Build/Run on entry symbol from yinz.toml. |
| Parser (`crates/ynz-parser/src/parser.rs`) | 2798 lines; `parse_number_type` still uses `[N]` syntax (line 454); no import/export/sensitive/wait/background grammar | Migrate to `<N>` (P0); add module-level `import`/`export` items; doc-comment attachment; `sensitive` modifier in type positions; `wait`/`background` expression prefixes. |
| AST (`crates/ynz-ast/src/nodes.rs`) | 731 lines; 3 `Item` variants (FunctionDecl, ShapeDecl, OptionsDecl); no Module/Import/Export nodes | Add `ImportDecl`, `ExportDecl` items; doc-string fields on FunctionDecl/ShapeDecl/OptionsDecl/Field; `Type::Sensitive(Box<Type>)`; `Expr::Wait(Box<Expr>)` + `Expr::Background(Box<Expr>)`; `Type::Number` precision becomes 1..=4096 (parser already validates the bound). |
| Typeck (`crates/ynz-typeck/src/check.rs`) | 3126 lines; `Type::Number { precision }` rejects `precision != 34` as `Type::Error` (types.rs:24) | Resolve cross-file symbols (module table); enforce import/export visibility; sensitive type propagation through string ops + `.reveal()` method; background `.share` rejection; mixed-precision arithmetic. |
| Codegen (`crates/ynz-codegen/src/emit.rs`) | 3723 lines; `Type::Number` lowers to decimal128 (`{i64,i64}` ABI for {bits high, bits low}) | Multi-u128 coefficient ABI for N > 34; runtime helpers `ynz_bignum_*`; sensitive auto-redact via runtime hook (print + interpolation); concurrency keywords lower to direct calls (sequential semantics). |
| Numerics (`crates/ynz-numerics/`) | decimal128 (bits/format/ops/parse/wide) — 34-digit hardware-fast path | Add `decimalN/` submodule with chunked-u128 storage, bignum add/sub/mul/div, mixed-precision promotion, narrowing rounding. Validated against IEEE 754-2008 test vectors + Python `decimal` differential. |
| Runtime (`crates/ynz-runtime/`) | Print/format helpers, allocator, error-frame stack | Add `ynz_print_sensitive` redaction; bignum-format/parse C ABI; concurrency hooks may need no-op stubs (sequential = direct call). |
| Banned-jargon (`crates/ynz-diagnostics/src/banned_jargon.rs`) | M7-locked list (`monad`, `lift`, `Result`, `Option`, ...) | Add concurrency words (`async`, `await`, `promise`, `future`, `goroutine`) + visibility words (`pub`, `private`, `protected`, `public`). |

### Locked decisions in scope (from Step 0–5 questions with patrick)

| Decision | Locked value |
|---|---|
| Plan shape | Single execution plan, sequenced phases (P0–P8) |
| Module depth | Full v0.1 spec: project-root-relative paths, named + namespace imports, `as` aliases, duplicate-name compile error, re-export, unused-import warning, circular references via multi-pass |
| Concurrency scope | Parse + typeck + sequential lowering (`wait` = direct call; `background` = direct call + return discard + ownership-`.share` rejection) |
| Sensitive scope | Full type-system surface; manual `sensitive(literal)` source; env-based source defers to v0.7 |
| Bignum scope | Full IEEE 754-2008 conformance — multi-u128 chunked coefficient, all 4 ops, mixed-precision promotion, narrowing rounding warning. This is the load-bearing `number` promise. |
| Doc-comment scope | Lexer recognizes `///`; parser attaches to AST; doc strings preserved through pipeline for v1.1 `ynz doc` |
| Number syntax migration | `number[N]` → `number<N>` lands in P0 doc-lockdown (parser change + integration test snapshot update) |
| File discovery | Walk `src/**/*.ynz`, parse all in parallel, codegen tree-shakes unused functions from binary |
| yinz.toml fields | Three: `entry`, `name`, `version`. Unknown fields warn (forward-compat). v0.22 adds `[dependencies]`; v1.x adds `[lint]`. |
| Banned-jargon additions | Concurrency (`async`, `await`, `promise`, `future`, `goroutine`) + visibility (`pub`, `private`, `protected`, `public`). |
| Master plan tracking | P0 flips M8 status `planned`→`active` in v0-1-compiler.md; P8 release flips `active`→`done`. |
| Release scope | Tag + audit, no public launch. v1.0 is public launch per mvp-scope.md. |

### Cross-references

- `design/modules.md` (109 lines) — module rules locked
- `design/doc-comments.md` (29 lines) — `///` only, exported items only, fields supported
- `design/sensitive.md` (61 lines) — type modifier, auto-redact, `.reveal()`
- `design/concurrency.md` (239 lines) — auto-parallelization via dep graph; M8 parses keywords, runs sequentially
- `design/numeric-types.md` lines 41–80 — `number<N>` parameterized precision; 34 hardware-fast; N > 34 bignum
- `design/main-entry.md` — `entrypoint()` is the entry function; file name flexible via yinz.toml
- `.claude/rules/non-oop.md` — module functions are still standalone functions; `import { foo }` brings function-call form; no methods-on-instances
- `.claude/rules/auto-promotion.md` — does NOT apply to most M8 features (modules, doc comments, sensitive, concurrency keywords are all source-level features without a stricter codegen form); bignum analysis required per "Auto-Promotion Analysis" subsection (see `### Performance` invariant below).
- `.claude/plans/done/m7-strings-errors-iterables.md` — most recent shipped milestone; 11-phase template inspires M8's shape

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Multi-file driver work expands beyond P2 scope | Medium | P2 PR blows the soft size limit; M8 stalls | Pre-write the salsa-inputs-as-Vec architecture in P0 doc-lockdown commit notes. If P2 hits 600+ lines, split into P2a (driver + project loader) and P2b (typeck cross-file resolution). |
| Bignum arithmetic correctness | High | Silent rounding errors in user code | Validate every op against IEEE 754-2008 test vectors AND differential test against Python `decimal` (10k random tuples per op). Property tests: commutativity, associativity (where applicable), round-trip identity. Treat any test failure as a hard ship-blocker. |
| Bignum performance regressions | Medium | `number<34>` users slow down even though hardware path should still apply | Branch on `precision <= 34` at op entry: hardware path unchanged. Add a `cargo bench` check that 34-digit ops stay within 5% of M7 baseline. |
| Sensitive propagation gaps | Medium | A string op produces a non-sensitive string from sensitive input → secret leak | Audit every string method (`.contains`, `.indexOf`, `.startsWith`, etc.) for propagation rules per `design/sensitive.md`. The propagation table is normative; tests assert every method preserves sensitivity. `.length` returns plain `int` per design (length isn't secret). |
| Background-task ownership rule (`.share` rejection) | Low | Compiler accepts dangling-borrow patterns | M4 already has ownership analysis (`is_consumed`, etc.). Reuse the infrastructure: when `background foo(arg)` is type-checked, if `foo`'s signature is `share`, reject. The compiler already knows ownership modifiers. |
| Doc-comment lexer regression | Low | `//` comments accidentally tokenize as `///` | Lexer test: explicit fixtures for `///`, `////` (treated as `///` content), `// regular`, `// ///` (regular comment containing slashes), `///\n///\n///` (multi-line attach). |
| Module circular-import detection | Medium | Compiler hangs or stack-overflows on a graph with no cycle, OR misses a real cycle and produces incoherent typeck output | Multi-pass design eliminates the hang risk (no recursive resolution). Add an explicit cycle test: `a.ynz import b; b.ynz import a` — compiles cleanly. Also test: file imports itself (`a.ynz import a`) — should be a compile error (self-import is nonsense, even though circular through B is fine). |
| yinz.toml schema bikeshed creep | Low | P2 derails on "what fields ship now" debate | Locked above: `entry`, `name`, `version` ONLY. Unknown fields warn. Defer everything else to v0.22/v1.x. |
| Number syntax migration breaks user code | Low | M2-era fixtures use `number[N]` syntax | Grep fixture corpus: only `m2_bignum_deferral.ynz` uses `number[100]`. Update one fixture + the snapshot. No user code exists outside the test corpus. |
| Release audit surfaces critical bugs | Medium | P8 release tag delayed by emergency fix-up cycle | This is the POINT of the audit. Treat P8 as variable-length: 1-2 days if clean, 3-5 if findings need fixing. Per CLAUDE.md rule 11 (no priority deflection), every confirmed audit finding gets fixed before tag. |
| Banned-jargon false positives | Low | Diagnostic tests fail on legitimate prose using `async` in a non-Yinz-jargon way | The list governs USER-FACING diagnostic strings only (`crates/ynz-diagnostics/src/banned_jargon.rs`). Spec/design prose is exempt (per dual-audience rule in `.claude/rules/inference.md`). |
| `cc -no-pie` linker hack accumulates over M8 surface | Low | Multi-file linking may surface new relocation issues | Existing build.rs comment says PIC codegen deferred to v0.2. Multi-file produces multiple .o files but linked the same way. Monitor; if it breaks, add to v0.2 deferred list, don't fix in M8. |

---

## Questions

(All resolved during Step 5a — see Locked decisions table in Research Findings.)

---

## Risk Assessment & Rollout Strategy

**Risk level: HIGH**

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | Compiler project |
| Touches auth/permissions | No | Compiler project |
| Raw SQL / literals | No | No SQL surface |
| Modifies existing data | Yes | Migrates `number[N]` → `number<N>` syntax in parser; refactors single-file driver to multi-file |
| Third-party integration | No | LLVM/inkwell already integrated |
| Changes existing endpoints | Yes (compiler) | `ynz build`/`ynz run` now take a project root; bare-file fallback preserved |

**Why HIGH**: bignum arithmetic correctness is load-bearing (silent rounding errors in financial/scientific code are catastrophic and undetectable until production). Multi-file driver is the biggest structural change since M1. Sensitive-propagation gaps would cause secret leaks. Three load-bearing concerns simultaneously = HIGH.

**Mitigations applied**:
- Comprehensive test coverage (IEEE 754 vectors + Python `decimal` differential + property tests for bignum; full propagation table tested for sensitive; cycle + duplicate + unused tests for modules) → lowers from HIGH to MEDIUM
- Pre-v1.0 policy means breaking changes are acceptable; no migration shims required → lowers from MEDIUM to LOW
- Audit phase (P8) sweeps the integrated surface before tag → lowers from LOW to acceptable for tag

**Rollout plan**:
1. **Internal-only release**: this is v0.1.0, no public users. Each phase ships its own PR with insta snapshots + fixture runs. P8 audit gate before tag.
2. **No staged rollout**: there's no production. Compiler users (= patrick + future contributors) get v0.1.0 in one drop.
3. **Post-release** (v0.2 work): if a user reports a bug, fix in a `v0.1.1` patch tag or wait for v0.2.

---

## Invariants This Milestone Must Preserve

### Safety
- **Const deep-immutability paths (M4 invariant — preserved)**: `const` bindings cannot be reassigned; cannot be `.lend`'d; cannot be `.give`'d; cannot have fields mutated; mutable-inference cannot accidentally widen const. All five paths must remain enforced through M8's new surface (e.g., a `const sensitive` value still cannot be `.lend`'d).
- **`sensitive T` propagation**: every string method that returns a `sensitive string` must do so deterministically (compile-time table). A sensitive value reaching `print()` MUST emit `[REDACTED]` not the underlying bytes. `.reveal()` is the ONLY way to extract the raw value.
- **Background-task ownership**: `background foo(x)` where `foo` takes `share` is a compile error. Only `give` (move) and `.copy()` are valid. Reuses M4's `is_consumed` scope-tracking infrastructure.
- **Module visibility**: a non-`export`'d item is invisible to importers. Type-check rejects `import { internalFn }` when `internalFn` lacks `export`.
- **Duplicate-import detection**: two imports bringing the same name (or namespace) into the same file is a compile error with the three-part teaching diagnostic.
- **Bignum precision boundary**: `number<N>` for `N > 4096` is a compile error (parser already enforces 1..=4096; M8 lifts the M2-era `N != 34 → Type::Error` clamp at typeck).
- **Doc comments on private items are silent** (per `design/doc-comments.md` "Exported Items Only" — has no effect, not a warning in M8; lint-tier warning is v0.4).

### Performance
- **LLVM `readonly` + `noalias` on share/lend params (M4 invariant — preserved)**: function parameters declared `share` continue to emit `readonly`; `lend` emits `noalias` + writable. M8 surfaces (sensitive, bignum, modules) must preserve this contract end-to-end.
- **Bignum hot-path branch**: at every bignum arithmetic op, branch on `precision <= 34`; if true, use the existing decimal128 hardware path unchanged. `cargo bench` target: 34-digit ops stay within 5% of M7 baseline. **Baseline reference**: tag `v0.1.0-m7` (commit `b24a1b0`); the P0 phase captures `cargo bench --bench decimal128_hot_path -- --save-baseline m7_baseline` so P6's comparison is against an explicit named baseline, not a moving target.
- **`number<N>` storage**: u128 for N ≤ 34 (hardware-fast); for N > 34, a fixed-size array of u128 chunks (size = ⌈N/34⌉). Memory bounded at ~1.7 KB per value at N=4096 (per design/numeric-types.md "Why 4096").
- **Multi-file salsa caching**: each `SourceFile` is its own salsa input. Editing one file invalidates downstream queries for that file but not others — incremental rebuild stays per-file granular.
- **Codegen tree-shaking**: unused functions (not transitively called from `entrypoint`) are stripped from the final binary. Existing M1 behavior preserved; M8 adds cross-module reachability.
- **Sensitive auto-redaction = single runtime branch**: `ynz_print_sensitive` checks a one-byte tag and emits `[REDACTED]` if set; non-sensitive print path unchanged. Zero overhead for non-sensitive values.

**Auto-Promotion Analysis** (per `.claude/rules/auto-promotion.md`):

| Feature | Stricter form? | Codegen auto-promote | Muted hint | Tier 3 lint | Rationale |
|---|---|---|---|---|---|
| `number<N>` for N ≤ 34 | n/a | already on hardware path | n/a | n/a | Compiler proves at parse time |
| `number<N>` for N > 34 in arithmetic with smaller `<M>` | Promote result to wider precision (N) automatically | YES | YES (locked: `number<N>` muted hint at the assignment site) | YES (`prefer-explicit-precision-when-mixing`) | Per design/numeric-types.md "Mixed-precision arithmetic" — binary ops promote; assignment to narrower precision warns. Hint shows the inferred precision; lint suggests explicit annotation. Both surfaces apply (typeable form exists: explicit `number<N>` annotation). |
| `import` with all-unused names | n/a | n/a | n/a | YES — `unused-import` warning (per design/modules.md "Unused Imports Are Warnings") | The "stricter form" is removing the import. Pure source-level concern; no codegen change. |
| `wait foo()` where `foo` has no side effects | Drop the `wait` (already sequential) | NO | NO | YES (`unnecessary-wait`) — v0.4 lint | M8 ships keyword + sequential semantics; the lint surface is v0.4 territory. Note the deferral in milestone-plan checklist. |
| `background foo()` for trivially-cheap fns | Drop the `background` (already runs immediately) | NO | NO | YES (`unnecessary-background`) — v0.4 lint | Same rationale — defer Tier 3 lints to v0.4. |
| Doc comments on private items | Drop the doc (no effect per spec) | NO | NO | YES (`doc-on-private-item`) — v0.4 lint | Same — defer to v0.4. |

For Tier 3 lints deferred to v0.4, the muted-hint protocol still defers to v0.4 LSP work (`design/ide-hints.md`). M8 codegen + typeck simply implement the locked semantics; the teaching surfaces ride v0.2/v0.4.

### Teaching
- All new diagnostics follow the WHAT / WHAT-INSTEAD / WHY three-part format per `design/teaching-mission.md`.
- **New banned-jargon entries**: `async`, `await`, `promise`, `future`, `goroutine` (concurrency) + `pub`, `private`, `protected`, `public` (visibility). Diagnostic test (`tests/jargon_audit.rs` style) asserts no diagnostic string contains any banned word.
- **Diagnostic teaching pairs added**:
  - User writes `pub function foo()` → `pub` is banned, suggest `export function foo()`. Why: "Yinz has two visibility states — exported (`export`) or private (default). No `pub`, no `protected`, no modifiers."
  - User writes `async function foo()` → `async` is banned, suggest `wait` at call site instead. Why: "Yinz auto-parallelizes reads via the dependency graph. Functions don't need `async`; callers use `wait` only when explicit ordering matters."
  - User writes `background processData(data)` where `processData` is `share data: Data` → "Cannot share with a background task. Background tasks may outlive the current function; a shared borrow would dangle. Use `give` or `.copy()`." (Exact wording from `spec/concurrency.md`.)
  - User writes `import { foo } from "./services"` → relative path rejected, suggest `"services/users"` (project-root). Why: "Yinz uses project-root paths so moving a file never breaks imports."
  - User writes `import * as users from "..."` → wildcard rejected, suggest `import users from "..."` (namespace import). Why: "Module namespace imports are explicit and tree-shake the same way."
  - User writes `number[100]` (M2 syntax) → migrate diagnostic: `Use number<100> — angle brackets for type parameters. M5 unified all generic syntax (array<T>, map<K,V>, number<N>) on <>.`
  - User writes `let huge: number<5000> = ...` → compile error with the v2+ deferral pointer (existing message stays).
  - User assigns `number<100>` value to `number<34>` binding → narrowing warning per design/numeric-types.md.
- **Sensitive-leak warning**: when `.reveal()` is called in an output context (print/log/interpolation), IDE Tier 3 warning per spec; M8 emits a Tier 3 lint suggestion (visible at compile, hint surface defers to v0.2 LSP).
- **Doc comment on private item**: M8 ships silent per spec (no warning until v0.4 lint tier).

### Runtime Dependencies
- **Modules**: filesystem access at compile time to walk `src/**/*.ynz` (already needed in M1 for `load_source`); no runtime dependency added.
- **Doc comments**: no runtime dependency; pure compile-time data preserved on AST.
- **Sensitive**: runtime `ynz_print_sensitive` C function that branches on the redaction tag; depends on the existing `ynz-runtime` crate's print path.
- **Concurrency keywords**: M8 = sequential lowering, so `wait foo()` and `background foo()` both compile to a direct call. NO runtime dependency on a scheduler (that's v0.3 work).
- **Bignum**: depends on `malloc` for the chunked-coefficient storage when N > 34. For N ≤ 34, hardware path = no allocation. Runtime helpers `ynz_bignum_alloc`, `ynz_bignum_free`, `ynz_bignum_add`, `ynz_bignum_sub`, `ynz_bignum_mul`, `ynz_bignum_div`, `ynz_bignum_format`, `ynz_bignum_parse`.

### Kernel-Mode Behavior
- **Modules**: always work in `--kernel` mode (compile-time only; no runtime dependency).
- **Doc comments**: always work.
- **Sensitive**: `sensitive T` and `.reveal()` work without an allocator; the `ynz_print_sensitive` runtime hook requires `print` to exist, which is itself a runtime-dependent feature. Kernel-mode (v0.3+) will need a user-provided print sink — covered by `design/future/no-runtime-mode.md`'s plug-in scheme.
- **Concurrency keywords** (`wait`, `background`): work in `--kernel` mode in M8 because sequential lowering = direct call (no scheduler needed). v0.3 auto-parallelization will require a user-provided scheduler in `--kernel` mode.
- **Bignum**: COMPILE ERROR in `--kernel` mode for `number<N>` with N > 34 unless user provides an allocator via the v0.3+ plug-in API (per `design/future/no-runtime-mode.md`). N ≤ 34 works (hardware path, no allocation). Error format: WHAT/WHAT-INSTEAD/WHY pointing to the plug-in allocator spec.

### Demo & Error Gallery
- **`examples/pirates-roster/`** becomes a multi-file project after P2:
  - `examples/pirates-roster/yinz.toml` (entry = `src/entrypoint.ynz`, name = `basics`, version = `0.1.0`)
  - `examples/pirates-roster/entrypoint.ynz` (top-level — imports + glue + runs all sections)
  - `examples/pirates-roster/src/services/players.ynz` (exported shapes, exported functions — demonstrates `import { ... }` form)
  - `examples/pirates-roster/src/services/inventory.ynz` (exported namespace — demonstrates `import inventory from "..."` form)
  - `examples/pirates-roster/src/utils/math_extra.ynz` (exported standalone functions, demonstrates UFCS across files)
  - Each M8 phase adds its section to `entrypoint.ynz`: doc comments on player/inventory items (P3), `sensitive` API-key demo (P4), `wait`/`background` in a checkout flow (P5), high-precision astro calc with `number<200>` (P6).
- **`examples/primantis-orders/m8_errors.ynz`**: per-phase additions to the gallery — one fixture file containing intentional triggers for every M8 compile-error class:
  - P0: number-syntax migration error (`number[34]` rejected, suggest `<>`)
  - P2 (modules): relative path (`"./foo"`), wildcard import (`import *`), default export (`export default`), duplicate-name collision, missing-export, side-effect import (`import "x"` with no binding), circular self-import (`a.ynz import a`)
  - P4 (sensitive): `.reveal()` in print context, mixed sensitive + non-sensitive in interpolation that loses sensitivity (this should NOT happen — but the test enforces propagation)
  - P5 (concurrency): `background foo(data)` where `foo` is `share data` (the dangling-borrow error)
  - P6 (bignum): `number<5000>` (above the 4096 cap), narrowing assignment warning

---

## Phases

> **Project shipping conventions** (Step 4a detection):
> - Per-phase ships via `/pr` (`<project>/.claude/skills/pr/SKILL.md` exists).
> - Per-milestone ships via `/release` (`<project>/.claude/skills/release/SKILL.md` exists).
> - All phases and the release follow this pattern.

### Phase 0: Doc lockdown + `number[N]` → `number<N>` syntax migration + master-plan radar
**PR scope**: Update design/spec docs to reflect M8 locked decisions; migrate `number[N]` → `number<N>` parser surface; flip v0-1-compiler.md M8 from `planned` to `active`; bump CHANGELOG `[Unreleased]` section to seed M8.
**Branch**: `chore/m8-doc-lockdown`
**Flag**: N/A
**Est. lines**: ~250 (mostly doc edits + ~50 lines of parser change + integration test snapshot update)
**Ships via**: `/pr`
**Objective**: Every doc reflects the locked M8 decisions; parser accepts `number<N>` for N ∈ 1..=4096; integration test `m2_bignum_deferral_produces_diagnostic` updated; master plan radar reflects M8 active.
**Why this phase exists**: M5–M7 followed this pattern (doc lockdown FIRST, then code) — prevents the "code shipped but spec/design didn't keep up" failure mode. Catches scope drift before any compiler change.
**Current-state anchors**:
- `crates/ynz-parser/src/parser.rs:454` — `parse_number_type` consumes `[N]`; migrate to `<N>` (verify it doesn't conflict with generic syntax for OTHER types).
- `crates/ynz-driver/tests/integration.rs:222–234` — `m2_bignum_deferral_produces_diagnostic`; the diagnostic message changes from "v0.7" / "M8" to "available starting at `number<35>`" once bignum lands (Phase 6). In P0 the syntax migration is the only change.
- `crates/ynz-driver/tests/fixtures/m2_bignum_deferral.ynz` — update from `number[100]` to `number<100>`.
- `.claude/plans/active/v0-1-compiler.md:206-211` — M8 milestone block; flip status `planned` → `active`.
- `CHANGELOG.md` — seed `[Unreleased]` section with M8 header.
**Files (expected scope)**:
- `crates/ynz-parser/src/parser.rs` (migrate `parse_number_type` from `[` to `<`)
- `crates/ynz-parser/tests/parse.rs` (add `number<N>` accept + `number[N]` redirect tests)
- `crates/ynz-driver/tests/fixtures/m2_bignum_deferral.ynz`
- `crates/ynz-driver/tests/snapshots/integration__m2_bignum_deferral_stderr.snap`
- `.claude/plans/active/v0-1-compiler.md` (status update + last_updated bump)
- `CHANGELOG.md` (M8 `[Unreleased]` section)
- Spec/design verification grep across `/spec/` and `/design/` — fix any `number[N]` mentions (should be zero per pre-survey).
**Deviation rule**: P0 must NOT touch the typeck precision clamp (that's P6). The parser accepts `number<N>` for N ∈ 1..=4096 BUT typeck still rejects N != 34 with the existing diagnostic. The integration test's snapshot is updated to reflect the new syntax, not new semantics.
**Steps**:
1. Grep `/workspaces/ynz/{spec,design}/**/*.md` for `number[` — confirm zero hits (already done in research; spec/design already use `<N>`).
2. Migrate `parse_number_type`: consume `Token::Lt` instead of `Token::LBracket`; expect `Token::Gt` instead of `Token::RBracket`. Diagnostic strings updated from `number[N]` → `number<N>` in the error suggestions.
3. Add PARSER-level redirect for `number[`: in `parse_number_type`, when the next token after `number` is `Token::LBracket` (not `Token::Lt`), emit a three-part diagnostic `Use number<N> — angle brackets for type parameters. M5 unified all generic syntax (array<T>, map<K,V>, number<N>) on <>.`. Then consume the bracketed body for error recovery (skip until matching `]`) and return `Type::Error`. The redirect happens at the PARSER (parse_number_type sees the token after `number`), not the lexer — the lexer emits `Token::Number` then `Token::LBracket` independently; only the parser has the multi-token context required to know "the user wrote `number[N]` meaning the type form, not `someArray[i]` meaning index access." Diagnostic is graceful migration aid; pre-v1.0 the bracket form is gone, but the error teaches the new syntax.
4. Update the M2 fixture and snapshot. Old snap content `number[100]` → `number<100>`; M8 message stays "available starting at `number<35>`" (still N != 34 message, since bignum hasn't shipped yet in P0).
5. Update `examples/pirates-roster/entrypoint.ynz` if it shows `number[34]` anywhere (it doesn't per grep — number type usage is plain `number`).
6. Update v0-1-compiler.md M8 status to `active`, bump `last_updated`.
7. Seed `CHANGELOG.md` `[Unreleased]` section with `## M8 — Modules, Doc Comments, Sensitive, Concurrency Keywords, Bignum`.
8. **Capture M7 bench baseline** (the named reference P6 will compare against): `cargo bench --bench decimal128_hot_path -- --save-baseline m7_baseline`. Commit the baseline file path (or proof of its capture) into the PR description so P6 can reproduce. If no decimal128 bench exists yet, ADD one in P0 measuring the four ops at N=34 with a fixed test corpus (10 randomly-but-seeded operand pairs); the m7_baseline value is what those benches produce on commit `b24a1b0`.
9. Run plan-radar rebuild (handled automatically by Stop hook).
**Acceptance criteria**:
- [ ] `number<100>` parses successfully and produces the v0.7/M8 deferral diagnostic at typeck (existing behavior, new syntax).
- [ ] `number[100]` produces a three-part migration diagnostic pointing at `<N>` syntax. Test: new fixture `crates/ynz-parser/tests/fixtures/number_bracket_migration.ynz`.
- [ ] `cargo test --workspace` green; the only snapshot changes are the M2 fixture + the new bracket-migration test.
- [ ] `cargo clippy --workspace -- -D warnings` green.
- [ ] `cargo fmt --check` green.
- [ ] No banned-jargon hits in M8 plan file (this file) per Bouncer Stop-hook audit.
- [ ] v0-1-compiler.md radar shows M8 as `active`.
**Quality gate**:
- [ ] No design/spec drift left after P0 — every `number[N]` source citation either migrated or proven to be a code comment about the OLD syntax (rare; check carefully).
- [ ] Migration diagnostic teaches the WHY, not just the WHAT. Reviewer reads diagnostic out loud — if it doesn't explain "M5 unified ...", it fails.
**Verification**: `cargo test -p ynz-parser parse_number_type` + `cargo test -p ynz-driver m2_bignum_deferral` + `./target/debug/ynz run /tmp/test.ynz` where `/tmp/test.ynz` contains `function entrypoint() -> nothing { let x: number<100> = 1.0 }` (expect M8 deferral diagnostic).

---

### Phase 1: Lexer — 6 keyword tokens + `///` doc-comment trivia + concurrency/visibility banned-keyword diagnostics
**PR scope**: Add the new tokens M8 needs. Pure lexer work. No grammar consumption yet.
**Branch**: `feat/m8-lexer`
**Flag**: N/A
**Est. lines**: ~350 (token enum entries + lexer match arms + banned-keyword handlers + ~150 lines of tests)
**Ships via**: `/pr`
**Objective**: Lexer recognizes the 6 new M8 keywords and the `///` doc-comment trivia; banned-keyword diagnostics for `async`/`await`/`promise`/`future`/`goroutine`/`pub`/`private`/`protected`/`public`.
**Why this phase exists**: M4/M5/M6/M7 all followed the lexer-first pattern. Tokens are the foundation; parser changes downstream depend on them. Splitting lexer into its own PR keeps the diff reviewable.
**Current-state anchors**:
- `crates/ynz-parser/src/token.rs:25-225` — Token enum, current count 64. M8 adds 6: `Import`, `Export`, `Sensitive`, `Wait`, `Background`, `DocComment(String)`. Final count = 70.
- `crates/ynz-parser/src/lexer.rs:66-83` — `skip_whitespace_and_comments` swallows `//` to newline. Modify to distinguish `///` (collect as content) from `//` (still trivia).
- `crates/ynz-parser/src/lexer.rs:343-365` — M3/M4/M5/M6/M7 keyword arms; M8 adds keyword arms for the 6 new ones.
- `crates/ynz-diagnostics/src/banned_jargon.rs:21-67` — Banned-jargon list. M8 adds concurrency + visibility entries.
**Files (expected scope)**:
- `crates/ynz-parser/src/token.rs` (6 new variants; update count comment 64→70)
- `crates/ynz-parser/src/lexer.rs` (6 new keyword arms; `///` handling; banned-keyword arms for the 9 jargon entries with three-part redirect diagnostics)
- `crates/ynz-diagnostics/src/banned_jargon.rs` (9 new entries: 5 concurrency + 4 visibility)
- `crates/ynz-parser/tests/lex.rs` (new tests: ~30 new test functions covering each keyword, doc-comment trivia variants, banned-keyword diagnostics)
- `crates/ynz-typeck/tests/check.rs:1248` (update token-variant-count test from 64 to 70 with `// test-ratchet: M8 P1 adds 6 keywords ...`)
**Deviation rule**: P1 produces TOKENS only. No AST changes, no parser grammar changes, no typeck. If you find yourself editing parser.rs or nodes.rs, STOP — that's P2/P3/P4/P5/P6 work.
**Steps**:
1. Add `Import`, `Export`, `Sensitive`, `Wait`, `Background` to `Token` enum (5 simple variants).
2. Add `DocComment { content: String, break_after: bool }` to `Token` enum. `content` is the line text stripped of leading `///` + one optional space. `break_after: true` iff the next non-whitespace, non-`///` token in the source is preceded by a blank line (a line containing only whitespace) between this `///` line and that next token. Regular `//` comments do NOT count as a break — they're trivia and ignored when scanning for the next non-trivia content. This flag is the mechanism by which P3's parser detects "doc-comment chain broken from the following item" without needing the parser to re-scan whitespace.
3. Update lexer keyword-match arms — each new keyword is a single-string match per the existing M3/M4/M5/M6/M7 pattern.
4. Update `skip_whitespace_and_comments`:
   - When `//` is followed by `/`, this is `///` — switch to doc-comment mode: collect bytes until `\n`, strip leading `/// ` (or `///`); then peek forward through trivia (whitespace, `//` regular comments) to determine `break_after`:
     - Scan past any whitespace and `//` lines.
     - During the scan, count the number of `\n` characters traversed before reaching either: (a) another `///` line, (b) a non-trivia token, or (c) EOF.
     - If the scan crossed a "blank line" (i.e., the source between this `///`'s terminating `\n` and the next non-trivia content has TWO or more `\n`s with only whitespace between them), set `break_after: true`.
     - Otherwise `break_after: false`.
     - Emit `Token::DocComment { content, break_after }`. Reset lexer position to the start of the next non-trivia content (do NOT consume it).
   - When `//` is NOT followed by `/`, behavior unchanged (skip to newline).
   - `////` (four slashes) treated as `///` followed by `/` content — i.e., `Token::DocComment { content: "/...", break_after: ... }`. The fourth slash is content. (Deliberate — anything past the third slash is part of the doc.)
   - Worked example for the ambiguous case: source `///foo\n// internal\n///bar\nexport function ...`. Lexer emits: `DocComment { content: "foo", break_after: false }` (next non-trivia is another `///` — no blank line between), skips `// internal` as trivia, emits `DocComment { content: "bar", break_after: false }` (next non-trivia is `export` — no blank line), then `Token::Export`. Parser attaches "foo\nbar" to the function. Worked example for the broken case: source `///foo\n\n///bar\nexport function ...`. Lexer emits: `DocComment { content: "foo", break_after: true }` (TWO `\n`s between this and next `///`), `DocComment { content: "bar", break_after: false }`, then `Token::Export`. Parser DISCARDS "foo" group (break_after: true ended it), attaches "bar" to the function. Worked example for the orphan case: source `///foo\nexport function ...\n\nexport shape S { ... }`. The function gets "foo" attached. Shape S has no DocComment preceding it — `doc = None`.
5. Add banned-keyword arms for the 9 jargon entries. Each emits a three-part diagnostic via `emit_banned_keyword` (existing helper):
   - `async` → "Yinz auto-parallelizes reads via the dependency graph. Functions don't need `async`; callers use `wait` only when explicit ordering matters."
   - `await` → "Use `wait` instead. Yinz's `wait` keyword is the call-site form (`wait foo()`) — no function-level annotation needed."
   - `promise`, `future`, `goroutine` → all redirect to "Yinz has `wait` (force completion) and `background` (run outside this function's lifetime). Two keywords cover what other languages use a dozen for."
   - `pub`, `private`, `protected`, `public` → "Yinz has two visibility states — exported (`export`) or private (default). No `pub`, no `protected`, no modifiers."
6. Update `crates/ynz-diagnostics/src/banned_jargon.rs` BANNED_JARGON list with the 9 entries.
7. Update lexer test snapshot count.
8. Update the existing token-variant-count test in `crates/ynz-typeck/tests/check.rs:1248` with `// test-ratchet: M8 P1 adds 6 keyword tokens — Import, Export, Sensitive, Wait, Background, DocComment. Count 64→70.`
9. Extend `examples/primantis-orders/m8_errors.ynz` with intentional triggers for each banned-jargon and the `number[` migration diagnostic (already added in P0; P1 adds 9 more error triggers).
**Acceptance criteria**:
- [ ] All 6 new keyword tokens tokenize correctly in isolation (one test per keyword).
- [ ] `/// foo` tokenizes as `DocComment { content: "foo", break_after: false }` when followed by another doc or an item (one leading space stripped).
- [ ] `///foo` (no space) tokenizes as `DocComment { content: "foo", ... }` (no space to strip; content is `foo`).
- [ ] `///` (empty) tokenizes as `DocComment { content: "", ... }`.
- [ ] `//// foo` tokenizes as `DocComment { content: "/ foo", ... }` (fourth slash is content).
- [ ] `// foo` (regular comment) is consumed as trivia, NO `DocComment` token emitted.
- [ ] `\n/// line1\n/// line2\nexport ...` produces TWO `DocComment` tokens both with `break_after: false`.
- [ ] `\n/// line1\n\n/// line2\nexport ...` (blank line between) produces TWO `DocComment` tokens: first with `break_after: true`, second with `break_after: false`.
- [ ] `\n/// line1\n// internal\n/// line2\nexport ...` (regular comment between) produces TWO `DocComment` tokens both with `break_after: false` (regular `//` does NOT break the chain — see P1 Step 4 worked examples).
- [ ] `\n/// line1\nexport function ...\n\nexport shape ...` produces `DocComment { ..., break_after: false }` followed by Export → Function tokens, then later Export → Shape with NO intervening DocComment (shape gets no doc).
- [ ] Each of the 9 banned-keyword diagnostics matches its expected three-part text.
- [ ] `cargo test --workspace` green.
- [ ] `tests/jargon_audit.rs`-style sweep passes: no diagnostic string in the codebase mentions any banned word.
- [ ] m8_errors.ynz gallery file compiles in "report-only" mode (Yinz multi-errors up to 50) and produces the expected diagnostic stream.
**Quality gate**:
- [ ] No grammar consumption added — `parse_module` ignores `DocComment` and `Import`/`Export` tokens for now (will be P2/P3).
- [ ] Token-variant-count test ratcheted up with explicit `// test-ratchet:` comment per Bouncer rule.
- [ ] Banned-keyword diagnostic uses words from Yinz vocabulary only (no recursive jargon violations).
**Verification**: `cargo test -p ynz-parser lex` + `cargo test -p ynz-diagnostics jargon`.

---

### Phase 2: Modules — multi-file driver + `import`/`export` grammar + cross-file typeck
**PR scope**: The single biggest phase of M8 by line count. Driver becomes project-aware (finds `yinz.toml`, walks `src/**/*.ynz`); parser consumes `import`/`export` items; typeck resolves symbols across files; circular references + duplicate-name + unused-import all work per spec.
**Branch**: `feat/m8-modules`
**Flag**: N/A
**Est. lines**: ~800 (driver: ~200; parser: ~150; typeck: ~250; tests: ~200). Soft size limit — split into 2a/2b if needed (see Deviation rule).
**Ships via**: `/pr`
**Objective**: A real multi-file Yinz project compiles end-to-end. `examples/pirates-roster/` is restructured into the multi-file layout described in `## Demo & Error Gallery`.
**Why this phase exists**: Modules unlock everything downstream — doc comments are most interesting on cross-file exports; sensitive types are passed via cross-file imports; concurrency keywords need cross-file dependency analysis (long term). And single-file is the M1 duct-tape that has to go before v0.1.0 ships.
**Current-state anchors**:
- `crates/ynz-driver/src/main.rs:14-31` — CLI: `Build`/`Run` take a single `file: PathBuf`. M8 changes this: take a project root or an entrypoint file; if file, find the nearest `yinz.toml` ancestor.
- `crates/ynz-driver/src/load.rs:8` — `load_source(path, diags)` reads ONE file. M8 adds `load_project(root, diags) -> Vec<(path, text)>`.
- `crates/ynz-driver/src/build.rs:41-44` — Creates ONE `SourceFile` and runs codegen on it. M8 creates N `SourceFile` inputs and runs a project-level codegen query.
- `crates/ynz-parser/src/parser.rs:parse_module` — Currently parses items at module top level. M8 adds `Item::Import { ... }`, `Item::ExportFunctionDecl`, `Item::ExportShapeDecl`, `Item::ExportOptionsDecl`, `Item::ExportConst`, `Item::ReExport { ... }`.
- `crates/ynz-typeck/src/check.rs` — Two-pass typeck. M8 adds Pass-0: collect all exported symbols across all files; Pass-1: build per-file SymbolTable including imports; Pass-2 unchanged.
- `crates/ynz-ast/src/nodes.rs:11-23` — `Item` enum, current count 3. M8 **LOCKED AST shape** (no code-time TBD per `no-duct-tape.md`): add 1 new variant `Item::ImportDecl { kind, source, span }` + add a new variant `Item::ConstDecl { is_exported, ... }` (top-level const declarations didn't exist pre-M8 — confirm in P2 research step; if they DO exist, add `is_exported: bool` to the existing variant) + add a new variant `Item::ReExport { items, source, span }`. The existing `FunctionDecl`, `ShapeDecl`, `OptionsDecl` variants each grow a single `is_exported: bool` field — chosen over per-form discriminated variants because (a) the EXPORTABILITY is a boolean attribute on an item, not a different shape, and (b) discriminated variants would duplicate every typeck/codegen pattern match. Final Item variant count: 6 (Function, Shape, Options, ImportDecl, ConstDecl, ReExport).
**Files (expected scope)**:
- `crates/ynz-driver/src/main.rs` (CLI shape; project-vs-file detection)
- `crates/ynz-driver/src/load.rs` (add `load_project` walking `src/**/*.ynz` + parsing yinz.toml)
- `crates/ynz-driver/src/build.rs` (multi-input salsa pipeline)
- `crates/ynz-driver/src/run.rs` (project-aware run)
- `crates/ynz-driver/src/toml.rs` (NEW — minimal TOML parser for yinz.toml; use the existing `toml` crate if not already a dep)
- `crates/ynz-parser/src/parser.rs` (import/export grammar)
- `crates/ynz-ast/src/nodes.rs` (ImportDecl, is_exported flags, ReExport)
- `crates/ynz-typeck/src/check.rs` (cross-file symbol resolution, duplicate-name detection, unused-import warning)
- `crates/ynz-typeck/src/scope.rs` (module-level symbol table)
- `crates/ynz-codegen/src/emit.rs` (mangled symbol names by module path; tree-shaking remains in place)
- `examples/pirates-roster/yinz.toml` (NEW)
- `examples/pirates-roster/entrypoint.ynz` (top-level imports + glue)
- `examples/pirates-roster/src/services/players.ynz` (NEW)
- `examples/pirates-roster/src/services/inventory.ynz` (NEW)
- `examples/pirates-roster/src/utils/math_extra.ynz` (NEW)
- `examples/pirates-roster/src/services/index.ynz` (NEW — re-export demo)
- `examples/primantis-orders/m8_errors.ynz` (extend with module errors)
- `crates/ynz-driver/tests/integration.rs` (~10 new test functions: single-file fallback, multi-file project, circular imports, duplicate-name error, missing-export error, unused-import warning, wildcard rejection, default-export rejection, relative-path rejection, side-effect-import rejection)
- `crates/ynz-driver/tests/fixtures/m8_modules_*` (NEW — ~10 fixture projects, each a directory with yinz.toml + src/*.ynz)
**Deviation rule**: P2 must not introduce doc comments / sensitive / concurrency / bignum grammar. If you find yourself reaching into one of those areas while wiring modules, STOP — file it as a separate phase. If P2 grows past ~1000 lines, SPLIT into P2a (driver + load_project + yinz.toml) and P2b (parser grammar + typeck cross-file + fixture suite + demo restructure).
**Steps**:
1. **CLI shape**: `ynz build [path]` / `ynz run [path]`. If `[path]` is a directory, treat it as project root; look for `yinz.toml`. If `[path]` is a `.ynz` file, walk parents until `yinz.toml` is found, use that as project root; if no yinz.toml anywhere, treat the file as a bare single-file project (M1-compat path — internal tests still use this).
2. **yinz.toml parsing**: minimal TOML schema with `entry`, `name`, `version` fields. Unknown keys emit a warning (`Unknown field 'foo' in yinz.toml — Yinz will ignore it. Was this for a future version's feature?`). Missing fields default: `entry = "src/entrypoint.ynz"`, `name = <dirname>`, `version = "0.0.0"`.
3. **load_project**: glob `src/**/*.ynz` relative to project root; sort by path for deterministic order; read all files in parallel via rayon (already an inkwell transitive dep).
4. **Multi-input salsa**: Build a `Vec<SourceFile>` (one per file). Each parses independently via existing `parse_query`. Cross-file analysis lives in a new project-level salsa query (`project_typeck_query`) that depends on each file's parse.
5. **Parser grammar** (AST shape locked in research-anchors above — no code-time TBD):
   - `import { name1, name2 as alias } from "services/users"` → `Item::ImportDecl { kind: ImportKind::Named { items: [...] }, source: "services/users", span }`
   - `import users from "services/users"` → `Item::ImportDecl { kind: ImportKind::Namespace { local_name: "users" }, source: "services/users", span }`
   - `import users as serviceUsers from "..."` → namespace with alias (handled inside `ImportKind::Namespace { local_name: "serviceUsers" }`)
   - `export function foo() { ... }` → existing `FunctionDecl` with `is_exported: true`
   - `export shape Player { ... }` → existing `ShapeDecl` with `is_exported: true`
   - `export options Status { ... }` → existing `OptionsDecl` with `is_exported: true`
   - `[export] const MAX_HEALTH = 100` → `Item::ConstDecl { name, value, type_annotation, is_exported, span }` (top-level const). **P2 research step (Step 0)**: verify `Item::ConstDecl` doesn't already exist (M2 only ships *let/const inside function bodies* per the parser survey; top-level const has not been needed). If it doesn't, add it. If it does (unlikely), extend with `is_exported`.
   - `export { fetchUser, createUser } from "services/users"` → `Item::ReExport { items: [...], source: "services/users", span }`
   - Rejected at parse time:
     - `import * as X from "..."` — three-part diagnostic
     - `export default ...` — three-part diagnostic
     - `import "./foo"` (relative path) — three-part diagnostic (path string content checked at parse)
     - `import "foo"` (side-effect) — must bind; three-part diagnostic
6. **Typeck cross-file resolution**:
   - Pass-0: enumerate every file's parsed AST, collect `(file, exported_name) -> ItemRef` into a global ExportTable.
   - Pass-1 per file: build local SymbolTable; resolve `import` statements against ExportTable; reject missing exports with three-part diagnostic naming the file + closest match suggestion.
   - Duplicate-name detection per spec: if file has two imports producing the same local name, three-part error per `design/modules.md`.
   - Unused-import warning: after typeck, walk the AST; any imported name with zero usages → warning (not error).
   - Circular imports: zero special handling needed — Pass-0 sees all files, Pass-1 sees all signatures, resolution is whole-graph.
   - Self-import (`a.ynz` imports `a`): compile error — "A file cannot import from itself."
7. **Codegen mangling**: function `foo` in `services/players.ynz` mangles to `services__players__foo` (or similar) to avoid collisions. Tree-shaking proceeds from the entrypoint's reachability graph; unused exports get stripped.
8. **Demo restructure**: split `examples/pirates-roster/entrypoint.ynz` into the layout above. Each file gets its M1–M7 features split logically: `services/players.ynz` (shapes, methods), `services/inventory.ynz` (collections + maps + options), `utils/math_extra.ynz` (UFCS helpers for ints/floats), `services/index.ynz` (re-export demo for v0.22 prep).
9. **Error gallery extension**: add module-error triggers to `examples/primantis-orders/m8_errors.ynz` (one trigger per error class enumerated above).
10. **Test corpus**: ~10 new fixture projects, each a self-contained directory under `crates/ynz-driver/tests/fixtures/m8_modules_*/`. Each fixture has its own `yinz.toml` + `src/*.ynz`.
**Acceptance criteria**:
- [ ] `./target/debug/ynz run examples/pirates-roster/entrypoint.ynz` runs successfully and prints the M1–M7 demo output (no regression).
- [ ] `./target/debug/ynz run examples/pirates-roster/` (project root) also runs successfully.
- [ ] Circular import test (`a` imports `b`, `b` imports `a`, both define a shape used by the other) compiles cleanly.
- [ ] Duplicate-name test produces the spec's exact three-part diagnostic.
- [ ] Missing-export test produces a three-part diagnostic naming the missing item + suggesting the closest available export.
- [ ] Unused-import test produces a warning (NOT an error) and the program still compiles.
- [ ] All four banned forms (wildcard, default, relative, side-effect) produce three-part diagnostics at parse time.
- [ ] Self-import test (`a.ynz import a`) produces a three-part diagnostic.
- [ ] Single-file fallback path: `./target/debug/ynz run /tmp/hello.ynz` (no yinz.toml found anywhere) still works (M1 path preserved).
- [ ] All 782 M7 tests pass; M8 P2 adds ~30 tests on top.
- [ ] `cargo clippy --workspace -- -D warnings` green.
**Quality gate**:
- [ ] No silent picking of duplicate names (last-import-wins is banned per spec).
- [ ] **Tree-shaking still works** — `cargo test multi_file_unreachable_stripped` confirms a function declared but never called is stripped from the binary. **Test mechanism**: build a 2-file project where `services/unused.ynz` exports `unusedFunction` (with a unique string literal in its body, e.g. `print("UNUSED_MARKER_xyz")`) but no other file imports it; build to a binary; use `nm -g <binary>` (POSIX) OR `objdump -t <binary>` to dump symbols; assert `unusedFunction`'s mangled symbol is absent; ALSO grep the binary for the string literal `UNUSED_MARKER_xyz` and assert it's absent (string literal removal confirms full strip, not just symbol stripping).
- [ ] **Salsa cache invalidation works per-file** — editing `players.ynz` does NOT re-parse `inventory.ynz`. **Test mechanism**: use salsa's `durability` + `revision` accessors (via `db.report_untracked_read`) OR a manual parse-counter wrapping the `parse_query` function (an `Arc<AtomicU64>` incremented inside the query body, only readable in test mode behind a `#[cfg(test)]` gate). After building the project initially, edit `players.ynz` text (via `source_file.set_text(&mut db, new_text)`), call `parse_query(&db, players_source)` — counter increments by 1. Call `parse_query(&db, inventory_source)` — counter does NOT increment (salsa returns the memoized result).
**Verification**:
```bash
./target/debug/ynz run examples/pirates-roster/
cargo test -p ynz-driver m8_modules
cargo test --workspace
```

---

### Phase 3: Doc comments — AST attachment + preservation
**PR scope**: `///` tokens (from P1) get attached to the following item as a doc string. AST nodes grow a `doc: Option<String>` field. Doc strings on private items are silently allowed (per spec; lint deferred to v0.4).
**Branch**: `feat/m8-doc-comments`
**Flag**: N/A
**Est. lines**: ~250
**Ships via**: `/pr`
**Objective**: A `///` doc comment immediately above an exported function/shape/options/const survives through parse → typeck → codegen, attached to the AST node. AST round-trip preserves doc text exactly.
**Why this phase exists**: Doc comments are the foundation for `ynz doc` (v1.1). Even though M8 doesn't generate docs, the AST must carry them. Without M8 attachment, v1.1 would need to redo the lexer/parser surface.
**Current-state anchors**:
- AST node families that get doc fields: `FunctionDecl`, `ShapeDecl`, `OptionsDecl`, `Item::ExportConst` (from P2), shape fields (struct `Field` in nodes.rs)
- Parser attachment logic at module top level: consume `DocComment` tokens before each item; concatenate multi-line; attach to next item; clear buffer on item save
- Spec rule: "A blank line between the comment and the item breaks the association." → implement via lexer position tracking (blank line between last DocComment and next item → discard the doc buffer)
**Files (expected scope)**:
- `crates/ynz-ast/src/nodes.rs` (`doc: Option<String>` on FunctionDecl, ShapeDecl, OptionsDecl, ExportConst, Field)
- `crates/ynz-parser/src/parser.rs` (doc-comment collection at module top level + at field positions; blank-line break detection)
- `crates/ynz-parser/tests/parse.rs` (doc-comment attachment tests; ~15 new tests)
- `crates/ynz-typeck/src/check.rs` (preserve through typeck — likely no logic, just passthrough)
- `crates/ynz-codegen/src/emit.rs` (emit `!llvm.dbg` metadata for source location; doc strings preserved on the AST for v1.1)
- `examples/pirates-roster/src/services/players.ynz` (add `///` comments on exported items — demo)
- `examples/primantis-orders/m8_errors.ynz` (add: doc comment broken by blank line not attached)
**Deviation rule**: P3 must NOT add doc-on-private-item warning (that's v0.4 lint). M8 spec says "no effect on generated docs" — silent preservation.
**Steps**:
1. Add `doc: Option<String>` to FunctionDecl, ShapeDecl, OptionsDecl, ExportConst, Field. `None` = no doc.
2. Parser top-level loop: maintain a `doc_buffer: Vec<String>`. On `Token::DocComment { content, break_after }`, push `content` to buffer; if `break_after: true`, **discard the buffer immediately** (the chain is broken from whatever follows). On the next non-DocComment token (e.g., `Token::Export`, `Token::Function`, etc.) reached with a non-empty buffer, attach the buffer (joined with `\n`) to the parsed item and clear the buffer.
3. Blank-line break detection is **delegated to the lexer's `break_after` flag** (set in P1; see P1 Step 4 worked examples). The parser does NOT re-scan whitespace — it reads only the boolean flag. This is the mechanism that makes the spec's "blank line breaks association" rule concrete and uneliminable.
4. Field doc-comment attachment: same logic, applied inside `parse_shape_body`.
5. Multi-line doc test: `\n/// line1\n/// line2\n` → attached string is `"line1\nline2"`.
6. Add `///` comments to exported items in `examples/pirates-roster/src/services/players.ynz` to demonstrate the feature.
7. Add intentional broken-attachment trigger to `examples/primantis-orders/m8_errors.ynz`.
**Acceptance criteria**:
- [ ] `/// fetches a user.\nexport function fetchUser() ...` parses with `doc = Some("fetches a user.")` on the FunctionDecl.
- [ ] Multi-line: `/// line1\n/// line2\nexport function ...` → `doc = Some("line1\nline2")`.
- [ ] Blank-line break: `/// orphan\n\nexport function ...` → `doc = None` (orphan discarded).
- [ ] Doc on field: `/// player health\nhealth: int` inside a shape body → field's `doc = Some("player health")`.
- [ ] Doc on private (non-exported) item: silently allowed; preserved on AST; emits no warning in M8 (lint surface defers to v0.4).
- [ ] `cargo test --workspace` green; ~15 new parser tests.
**Quality gate**:
- [ ] Doc strings round-trip through parse → typeck → codegen unchanged (no LLVM mangling, no stripping).
- [ ] Field doc + shape doc are independently attached (a field doc doesn't blow away its shape's doc).
- [ ] Existing `//` regular comments unchanged.
**Verification**: `cargo test -p ynz-parser doc_comment`.

---

### Phase 4: Sensitive type modifier + auto-redaction
**PR scope**: `sensitive T` type, `sensitive(value)` constructor, `.reveal()` method, propagation through string ops, runtime auto-redaction in `print` + interpolation.
**Branch**: `feat/m8-sensitive`
**Flag**: N/A
**Est. lines**: ~600 (AST/parser ~100, typeck ~200, codegen + runtime ~200, tests ~100)
**Ships via**: `/pr`
**Objective**: Type-system surface for sensitive values is complete. Manual sources work today; env-based sources defer to v0.7.
**Why this phase exists**: Security primitive. The accidental-secret-leak pattern is the #1 secret exposure category (per `design/sensitive.md` rationale). Auto-redaction by default closes the gap.
**Current-state anchors**:
- `crates/ynz-typeck/src/types.rs` — Type enum (20 variants); M8 adds `Sensitive(Box<Type>)`. New count 21.
- Existing string-op typeck table (`.toUpperCase`, `.toLowerCase`, `.trim`, `.contains`, `.split`, `.replace`, etc.) — each annotated for sensitivity propagation.
- `crates/ynz-runtime/src/lib.rs` — print path (`ynz_print_string` for plain strings). M8 adds `ynz_print_sensitive` that emits `[REDACTED]`.
- `--reveal-sensitive` CLI flag on `ynz run`.
**Files (expected scope)**:
- `crates/ynz-ast/src/nodes.rs` (Type::Sensitive)
- `crates/ynz-parser/src/parser.rs` (parse `sensitive T` in type positions; parse `sensitive(expr)` constructor; parse `.reveal()` method call)
- `crates/ynz-typeck/src/types.rs` (Type::Sensitive variant; propagation table for string ops)
- `crates/ynz-typeck/src/check.rs` (sensitive propagation logic; .reveal() returns underlying type; .length on sensitive string returns plain int per spec)
- `crates/ynz-typeck/src/intrinsics.rs` (per-method sensitivity propagation rules)
- `crates/ynz-codegen/src/emit.rs` (sensitive struct layout: same as plain string + 1-byte tag; .reveal() = strip the tag; print of sensitive routes to ynz_print_sensitive)
- `crates/ynz-runtime/src/lib.rs` (ynz_print_sensitive function)
- `crates/ynz-driver/src/main.rs` (--reveal-sensitive flag handling for `ynz run`)
- `crates/ynz-driver/tests/fixtures/m8_sensitive_*.ynz` (~6 fixtures: basic redaction, .reveal(), propagation through ops, .length returns plain int, multiple sensitive in interpolation, --reveal-sensitive flag override)
- `examples/pirates-roster/entrypoint.ynz` (sensitive section demonstrating apiKey-like pattern)
- `examples/primantis-orders/m8_errors.ynz` (sensitive in print without .reveal — no error, but `.reveal()` in print context produces a Tier 3 lint suggestion)
**Deviation rule**: P4 must NOT introduce env stdlib (`env.get()`). v0.7 owns that.
**Steps**:
1. Add `Type::Sensitive(Box<Type>)` to Type enum. Update variant-count test ratchet.
2. Parse `sensitive T` in type-position contexts (variable annotation, field type, parameter type, return type).
3. Parse `sensitive(expr)` constructor as a function-like call form. Typeck: `sensitive(plain_string)` → `sensitive string`. Reject `sensitive(int)` etc. — only string types allowed in v0.1 (per spec; future may extend).
4. Typeck: locked propagation table covering every M7 string method. The rule per spec: string operations on sensitive values stay sensitive; non-string extractions (length, byte/code-point reads, boolean tests) return plain types because the spec says "length isn't secret" and the same rationale extends to other scalars derived from but not containing the secret bytes.

   **Locked propagation table** (assert this in `crates/ynz-typeck/tests/sensitive_propagation.rs` — one test per row):

   | Method (on `sensitive string`) | Returns sensitive? | Reasoning |
   |---|---|---|
   | `.contains(needle: string) -> bool` | NO (returns `bool`) | Boolean result; no string bytes returned |
   | `.indexOf(needle: string) -> maybe int` | NO (returns `maybe int`) | Index position is not the secret |
   | `.startsWith(prefix: string) -> bool` | NO (returns `bool`) | Boolean result |
   | `.endsWith(suffix: string) -> bool` | NO (returns `bool`) | Boolean result |
   | `.toUpperCase() -> string` | YES (returns `sensitive string`) | Whole string is the secret in a different case |
   | `.toLowerCase() -> string` | YES (returns `sensitive string`) | Same — case folding doesn't reduce sensitivity |
   | `.substring(start, end) -> string` | YES (returns `sensitive string`) | A substring of a secret may BE the secret or part of it |
   | `.trim() -> string` | YES (returns `sensitive string`) | Trimmed secret is still the secret |
   | `.split(delim: string) -> array<string>` | YES (returns `array<sensitive string>`) | Each piece may contain part of the secret |
   | `.replace(needle, replacement: string) -> string` | YES (returns `sensitive string`) | Modified secret retains its sensitive nature; replacement may also contain secret material |
   | `.byteAt(i: int) -> int` | NO (returns `int`) | Per `.length isn't secret` rationale — a single byte value doesn't usefully leak; bulk-byte extraction would, but loop+`byteAt` is detectable via Tier 3 lint (v0.4) |
   | `.get(i: int) -> int` | NO (returns `int` — code point) | Same rationale as `byteAt` |
   | `.graphemeAt(i: int) -> string` | YES (returns `sensitive string`) | A grapheme IS a string fragment that may contain secret material |
   | `.count() -> int` | NO (returns `int`) | Code-point count is not the secret per spec |
   | `.byteCount() -> int` | NO (returns `int`) | Same |
   | `.graphemeCount() -> int` | NO (returns `int`) | Same |
   | Interpolation `` `pre${x}post` `` where `x: sensitive string` | YES (interpolated result is `sensitive string`) | Per spec; result auto-redacts as `pre[REDACTED]post` at print time |
   | Multiple sensitive interpolands `` `${a}:${b}` `` (both sensitive) | YES — single redaction per interpoland | Each `${...}` site redacts independently; result printed as `[REDACTED]:[REDACTED]` (NOT `[REDACTED]` for the whole) |
   | Mixed sensitive + non-sensitive `` `${plain}=${secret}` `` | YES — whole result sensitive | Once any interpoland is sensitive, the result string is sensitive (containing the redaction marker in a sensitive container); print shows `plain=[REDACTED]`; the plain prefix bytes are not redacted but the whole RESULT cannot be passed to a non-sensitive sink |

   **Future M7 methods not yet shipped** (e.g., `.padStart`, `.padEnd` if added): default rule is **preserve sensitivity** unless a future M-plan explicitly overrides. Add a test (`crates/ynz-typeck/tests/sensitive_propagation_completeness.rs`) that iterates every method registered in the M7 intrinsic table for `string` and asserts each one has a propagation rule entry — fails compile if a new method is added without a propagation decision.
5. `.reveal()` method on `sensitive T` returns `T` (strips the modifier). Typeck implements as a special-case body operation per `.claude/rules/dot-postfix.md` (parens for actions).
6. Codegen: sensitive string = `{ bytes_ptr, len, capacity, is_sensitive_tag: u8 }` (extend existing string struct OR wrap in a new struct — TBD at implementation; prefer extending if SSO byte budget allows).
7. Print path: `print(sensitive_string)` lowers to `ynz_print_sensitive`. `ynz_print_sensitive` checks the tag and emits `[REDACTED]` or the underlying bytes per `--reveal-sensitive` runtime flag.
8. `--reveal-sensitive` CLI flag on `ynz run`. NOT on `ynz build --release` (per spec — flag stripped from release binaries; but in v0.1 we don't HAVE release builds yet, so the flag works on debug runs only — defer the "stripped from release" check to v0.4 release-mode work, document in `### Forward-Compatibility Constraints` of this plan).
9. Interpolation: `` `key: ${apiKey}` `` where `apiKey` is sensitive → interpolated result is `sensitive string` AND the runtime substitutes `[REDACTED]` at the interpolation site for the embedded value.
10. Demo + error gallery extensions.
**Acceptance criteria**:
- [ ] `let k: sensitive string = sensitive("abc")` typechecks.
- [ ] `print(k)` prints `[REDACTED]`.
- [ ] `print(k.reveal())` prints `abc` (no flag needed; .reveal() is the explicit opt-in).
- [ ] `k.toUpperCase()` is type `sensitive string`; `print(k.toUpperCase())` prints `[REDACTED]`.
- [ ] `k.length` is type `int`; `print(k.length)` prints `3` (length isn't secret per spec).
- [ ] `` `key=${k}` `` is type `sensitive string`; print prints `key=[REDACTED]`.
- [ ] `ynz run --reveal-sensitive` prints the raw value (debug mode override).
- [ ] M4 ownership rules preserved: `const sensitive` cannot be `.lend`'d; `let sensitive` can be reassigned.
**Quality gate**:
- [ ] Every string method audited for propagation rule. No method silently returns plain string from sensitive input.
- [ ] `.reveal()` in output context (print call argument, interpolation expression) emits a Tier 3 lint suggestion (visible at compile; LSP teaching surface defers to v0.2).
- [ ] No runtime overhead on non-sensitive prints (branch on tag once; non-sensitive path unchanged).
**Verification**: `cargo test -p ynz-driver m8_sensitive`.

---

### Phase 5: Concurrency keywords — `wait` + `background` parse + typeck + sequential lowering
**PR scope**: Both keywords parse, type-check, and lower to sequential semantics. Ownership rule (background-rejects-`.share`) enforced.
**Branch**: `feat/m8-concurrency`
**Flag**: N/A
**Est. lines**: ~400
**Ships via**: `/pr`
**Objective**: Programs can be WRITTEN with `wait foo()` and `background foo()` today. Real auto-parallelization comes v0.3; M8 ships sequential lowering. The point is to lock the syntax + semantics surface so v0.3 work doesn't disturb user code.
**Why this phase exists**: Master plan locks `wait`/`background` for v0.1. Locking the syntax + ownership rule NOW means v0.3 auto-parallelization is purely a codegen change, no spec or grammar change.
**Current-state anchors**:
- `wait` + `background` tokens already exist (from P1).
- M4 ownership analysis (`is_consumed`, share/lend/give signatures) — reused for the background-share rejection.
- AST `Expr` enum — add `Wait(Box<Expr>)` and `Background(Box<Expr>)` variants.
- Statement form: `background foo()` may appear as a statement (return-value-discarded form) or as an expression `let handle = background foo()` (handle-form; .send/.receive are v0.3, so M8 typeck rejects the handle form OR types it as `Background<T>` placeholder).
**Files (expected scope)**:
- `crates/ynz-ast/src/nodes.rs` (Expr::Wait, Expr::Background; update variant-count tests)
- `crates/ynz-parser/src/parser.rs` (parse `wait expr` and `background expr` as prefix-keyword expressions)
- `crates/ynz-typeck/src/check.rs` (typeck Wait/Background; background-share rejection per design/concurrency.md ownership rules; background ownership inference: if return value used after, infer `.copy()`; if not, infer `.give()`)
- `crates/ynz-codegen/src/emit.rs` (Wait = direct call; Background = direct call + return discard for statement form)
- `crates/ynz-driver/tests/fixtures/m8_concurrency_*.ynz` (~6 fixtures: wait on stdlib-like call, background fire-and-forget, background with give-inferred, background with copy-inferred, background-share rejected, background on share function rejected)
- `examples/pirates-roster/entrypoint.ynz` (concurrency section)
- `examples/primantis-orders/m8_errors.ynz` (background-share rejection trigger)
**Deviation rule**: P5 must NOT implement auto-parallelization (v0.3) or background handles (`.send`/`.receive` — v0.3). The handle form `let h = background foo()` is a compile error in M8 with a three-part diagnostic pointing at v0.3.
**Steps**:
1. Add `Expr::Wait(Box<Expr>, SourceSpan)` and `Expr::Background(Box<Expr>, SourceSpan)` to AST.
2. Parser: prefix keywords. `wait` and `background` may appear before any expression. `wait foo(args)` and `background foo(args)` are the common forms; `wait x.method()` and `background x.method()` also valid.
3. Typeck `Expr::Wait`: typecheck inner expression normally; Wait's type = inner type. (Sequential = wait does nothing observable.)
4. Typeck `Expr::Background`: typecheck inner expression. The inner MUST be a function call (Call or MethodCall). Reject `background x + 1` etc. with three-part diagnostic.
5. Background ownership rule (LOCKED via the type-contract, NOT runtime lifetime analysis): the `background` keyword places the called function under the v0.3 contract "may outlive the caller's frame," even though M8's sequential lowering runs it in-place. The compiler enforces the CONTRACT, not the M8-specific lowering. This is the same principle as M5's first-class `range` — the SOURCE semantics is locked even when v0.1 codegen happens to be simpler.

   For each parameter:
   - **If sig is `share`**: REJECT regardless of argument shape. Compile error with three-part diagnostic byte-identical to `spec/concurrency.md` line 167-175. WHY: "Background tasks may outlive the current function. A shared borrow would dangle when the owner's scope ends. Use a function with `give` (move) or pass `value.copy()` to a `give`-signature function." This applies even if the user passes `x.copy()` to a share-sig function — the SIGNATURE forbids share-with-background, regardless of the call-site expression.
   - **If sig is `lend`**: REJECT. Same dangling-borrow concern; lend is the mutable form of share. Three-part diagnostic mirrors share's. (Spec/design don't explicitly enumerate this case — locking it here per "the type-contract owns the rule" rationale; lend is conceptually share-with-write.)
   - **If sig is `give`**: ACCEPT. The compiler INFERS the call-site form (see Step 6 below for the inference rule, aligned with `.claude/rules/inference.md`).

   **Paper-Trace examples** locked in `crates/ynz-typeck/tests/background_ownership.rs`:

   - **Accepted**: `function processEvent(give event: WebhookEvent) -> nothing { ... }; function handler(event: WebhookEvent) -> nothing { background processEvent(event); /* event NOT used after */ }`. Compiler infers `.give()` (move). No diagnostic. M4 `is_consumed` tracking marks `event` as consumed; any subsequent use is a use-after-give error.
   - **Rejected missing-copy (use-after-without-explicit-copy)**: `function processEvent(give event: WebhookEvent) -> nothing { ... }; function handler(event: WebhookEvent) -> Response errors { background processEvent(event); log(\`Queued: ${event.id}\`); /* event used after */ return Response.ok }`. REJECT with the missing-`.copy()` teaching diagnostic from Step 6 below. Reason: `event` is `WebhookEvent` (heap-owning shape, not trivially-copyable); the post-use forces a copy, but the cost must be visible at the call site.
   - **Accepted with explicit copy**: same handler rewritten with `background processEvent(event.copy())` — typechecks. The `.copy()` materializes an independent owned value passed to `processEvent`; `event` remains owned by `handler` and usable on the next line. This is the canonical spec form per `spec/concurrency.md` line 194-199.
   - **Accepted (trivially-copyable, no explicit copy needed)**: `function processCount(give count: int) -> nothing { ... }; function handler() -> nothing { let n: int = 5; background processCount(n); print(n); /* n used after */ }`. `int` is trivially-copyable per M4; compiler auto-copies; no diagnostic. Muted hint: `.copy (trivially-copyable, 8 bytes, zero cost)`.
   - **Rejected**: `function readData(share data: Data) -> nothing { ... }; function handler(data: Data) { background readData(data) }`. REJECT with the three-part diagnostic. Reason: signature is `share`; the v0.3 contract forbids share-with-background.
   - **Still rejected (sig is what matters, not call-site form)**: `function readData(share data: Data) -> nothing { ... }; function handler(data: Data) { background readData(data.copy()) }`. REJECT — even though `.copy()` would create an independent owned value, the SIGNATURE `share data` is the rule's gate, not the argument expression.
6. **Background call-site ownership inference** (aligned with `.claude/rules/inference.md` to AVOID re-introducing Graveyard Entry 2 — "Requiring Explicit Ownership Annotation at Call Sites"):

   The compiler INFERS the call-site form per the existing rule (signature drives + body usage refines). It does NOT REQUIRE the user to type `.copy()` or `.give()` explicitly. The IDE muted hint surfaces the inferred form:

   - When the signature is `give` and the argument is **NOT used after** the `background` call site → compiler infers `.give()` (move). IDE muted hint at the call site renders as informational text: `.give (moved — not used after)`. **Click jumps to function signature** (per inference.md, ownership-at-call-sites is the "informational category" — no typeable body syntax, so click-to-make-explicit isn't applicable; click jumps to where the modifier IS visible, the signature).
   - When the signature is `give` and the argument **IS used after** the `background` call → **REQUIRE explicit `.copy()` at the call site, with a three-part teaching diagnostic** if it's missing. This preserves the M4 contract (auto-copy ONLY for trivially-copyable scalars; heap values require explicit `.copy()` to make the cost visible) AND matches `spec/concurrency.md` line 194-199 which shows `background processEvent(event.copy())` as the canonical usage pattern. **Diagnostic format** if user wrote `background processEvent(event)` and `event` is read after the call: WHAT "`event` is used after this `background` call, but `processEvent`'s `give` signature would move it away." WHAT-INSTEAD "Call `.copy()` explicitly: `background processEvent(event.copy())`. This creates an independent copy for the background task while keeping the original usable." WHY "Background tasks may outlive the current function. Auto-copying heap values silently would hide the cost from readers — Yinz makes the copy visible at the call site so reviewers can see the allocation. Trivially-copyable scalars (int, float, bool) auto-copy because there's no observable cost; owned heap values do not." NOTE: this is NOT Graveyard Entry 2 ("Requiring Explicit Ownership Annotation at Call Sites") because the rule is type-driven (post-use of a `give`-sig arg requires the cost-visible form), not a blanket "annotate every call site." Graveyard Entry 2 forbids requiring `.share`/`.lend`/`.give` modifiers on EVERY call site; this rule requires `.copy()` only when the type-flow proves a copy is needed AND the cost is non-trivial (heap allocation). The compiler still INFERS ownership for trivially-copyable types without diagnostic.
   - **Auto-copy for trivially-copyable types preserved**: when the argument's type is M4-classifiable as trivially-copyable (int, float, bool, fixed-size value types with no heap-owning fields), the compiler auto-copies and the muted hint renders `.copy (trivially-copyable, 8 bytes, zero cost)`. No diagnostic, no explicit `.copy()` required. This matches the spec's "small value (string, int)" auto-copy case in the ownership table.
   - **Tier 3 lint `unnecessary-background-copy`** (DEFERRED to v0.4 lint surface): when the user writes `value.copy()` in a position where the compiler can prove `value` isn't used after, suggest dropping the `.copy()` to enable the move. M8 ships the typeck-level rule (require `.copy()` when used after); the lint suggestion that goes the OTHER direction (drop `.copy()` when not needed) defers to v0.4.

7. Background return value: M8 spec is the fire-and-forget form (`background foo()` as a statement). The handle form `let h = background foo()` is a compile error with the v0.3 pointer. Diagnostic: WHAT "Storing the result of `background` is not yet supported." WHAT-INSTEAD "Use `background foo()` as a statement to fire-and-forget; or wait for v0.3 which adds `.send()`/`.receive()` on background handles." WHY "M8 ships the keyword surface; the handle type and its communication primitives ship with the auto-parallelization scheduler in v0.3."
8. Codegen Wait: emit the inner call directly. No scheduler call, no synchronization primitive.
9. Codegen Background: emit the inner call directly; ignore the return value. (Sequential = runs to completion before the next statement.)
10. Demo + error gallery extensions.
**Acceptance criteria**:
- [ ] `wait foo()` parses + typechecks + runs identically to `foo()` (no observable difference at runtime).
- [ ] `background foo()` parses + typechecks + runs identically to `foo()` (return value discarded; runs sequentially in M8).
- [ ] `background process(data)` where `process: function(share data: Data)` is a compile error with the spec-exact diagnostic.
- [ ] `background process(data)` where `process: function(give data: Data)` and `data` is unused after → typechecks with give-inferred (no `.copy()`).
- [ ] `background process(data)` where `process: function(give data: Data)` and `data` IS used after → typeck demands explicit `.copy()` (per spec).
- [ ] `let h = background foo()` (handle form) → compile error pointing at v0.3.
- [ ] M4 ownership invariants preserved: `const` arg passed to `background give-fn` is rejected (can't give a const).
**Quality gate**:
- [ ] No scheduler dependency added — `wait`/`background` lower to direct calls.
- [ ] Background-share rejection diagnostic text byte-identical to `spec/concurrency.md` line 167-175.
- [ ] M8 errors gallery triggers the background-share rejection.
**Verification**: `cargo test -p ynz-driver m8_concurrency`.

---

### Phase 6: Bignum — `number<N>` for N ∈ (34, 4096]
**PR scope**: The M2 carry-over. Full IEEE 754-2008 conformance for parameterized precision. Hardware path (N ≤ 34) preserved; bignum path (N > 34) added. Mixed-precision promotion + narrowing-warning rounding.
**Branch**: `feat/m8-bignum`
**Flag**: N/A
**Est. lines**: ~1000 (numerics ~600, typeck ~150, codegen ~150, tests ~100). Largest phase by code volume.
**Ships via**: `/pr`
**Objective**: `let chaotic: number<200> = 0.123456789...` works end-to-end. All four arithmetic ops produce IEEE 754-2008 compliant results. Differential against Python `decimal` passes 10k random tuples per op.
**Why this phase exists**: M2 carry-over. The load-bearing v0.1 promise.
**Current-state anchors**:
- `crates/ynz-numerics/src/decimal128/` — 34-digit hardware path. Files: bits.rs, format.rs, ops.rs, parse.rs, wide.rs.
- `crates/ynz-typeck/src/types.rs:24` — Type::Number rejects N != 34 as Type::Error. Lift in P6.
- `crates/ynz-codegen/src/emit.rs` — Type::Number lowers to {i64, i64} ABI (decimal128 high/low). Extend to bignum ABI for N > 34.
- `design/numeric-types.md` lines 41-80 — precision rules + mixed-precision semantics.
**Files (expected scope)**:
- `crates/ynz-numerics/src/decimal_n/` (NEW submodule)
  - `mod.rs`
  - `bignum.rs` (chunked-u128 coefficient storage; size = ⌈N/34⌉ chunks)
  - `ops.rs` (bignum add, sub, mul, div with proper rounding)
  - `format.rs` (bignum → string)
  - `parse.rs` (string → bignum)
- `crates/ynz-numerics/src/lib.rs` (re-export decimal_n)
- `crates/ynz-typeck/src/types.rs` (lift the N != 34 → Error clamp; add precision validity check 1..=4096)
- `crates/ynz-typeck/src/check.rs` (mixed-precision arithmetic typeck: binary op of `number<A>` + `number<B>` → `number<max(A,B)>`; assignment of `number<A>` to `number<B>` where B<A produces narrowing warning)
- `crates/ynz-codegen/src/emit.rs` (bignum ABI: pointer to heap-allocated chunks for N > 34; hardware {i64,i64} path preserved for N ≤ 34)
- `crates/ynz-runtime/src/lib.rs` (ynz_bignum_alloc, _free, _add, _sub, _mul, _div, _format, _parse)
- `crates/ynz-numerics/tests/conformance.rs` (IEEE 754-2008 test vectors)
- `crates/ynz-numerics/tests/differential.rs` (Python decimal differential — 10k random tuples per op)
- `crates/ynz-numerics/tests/properties.rs` (proptest: commutativity, associativity, round-trip)
- `crates/ynz-driver/tests/fixtures/m8_bignum_*.ynz` (~8 fixtures: basic number<100>, mixed-precision promotion, narrowing warning, exact decimal math at N=200, N=4096 boundary, N=5000 deferral, addition/subtraction/multiplication/division correctness)
- `crates/ynz-driver/tests/integration.rs` (UPDATE `m2_bignum_deferral_produces_diagnostic` to assert success now — bignum landed; the catch-up marker fires)
- `examples/pirates-roster/entrypoint.ynz` (bignum section with `number<200>` astro-physics-style calc)
- `examples/primantis-orders/m8_errors.ynz` (number<5000> too-large trigger; narrowing assignment warning)
**Deviation rule**: P6 must NOT change the 34-digit hot path. The decimal128 implementation in `crates/ynz-numerics/src/decimal128/` stays byte-identical (`cargo bench` confirms within 5%).
**Steps**:
1. **Storage model (LOCKED)**: **heap-allocated, single-owner, value-semantics**. `decimal_n::BigNum { precision: u16, chunks: *mut u128 /* heap-allocated array of ⌈N/34⌉ u128s */, sign: bool, exponent: i32 }`. The codegen passes bignum values as a 16-byte struct `{ precision: u16, sign: bool, exponent: i32, pad, chunks: *mut u128 }` with the chunks array on the heap. Total in-place footprint = 16 bytes (the struct). Heap footprint per binding = ⌈N/34⌉ * 16 bytes. At N=4096, 120 chunks * 16 = 1920 bytes; that's slightly above the design doc's "~1.7 KB" rough estimate but within the predictable-performance bound (the design doc estimates the COEFFICIENT bytes only; the bookkeeping overhead is fine).

   **Why heap (not stack-inline or hybrid)**: stack-inline would require const-generic storage (`[u128; N_CHUNKS]` where `N_CHUNKS` is type-level), which Yinz does not support in v0.1 (const generics are a v2+ feature per design/mvp-scope.md). Hybrid (small-bignum-on-stack, large-bignum-on-heap) doubles codegen complexity for marginal benefit since most bignum users are at N=70..200 (the cited use cases — physics, finance) where heap allocation is dwarfed by op cost. Heap-only is the locked choice.

   **Kernel-mode compatibility**: per `### Kernel-Mode Behavior` invariant above, `number<N>` for N > 34 is a compile error in `--kernel` mode unless the user provides a custom allocator via the v0.3+ plug-in API (`design/future/no-runtime-mode.md`). The hardware path (N ≤ 34) works in `--kernel` because it's stack-only.

   **Ownership tracking**: bignum bindings follow M4's ownership rules — `let x: number<100> = 1.0` owns its heap buffer; `let y = x` moves the buffer (single-owner invariant preserved); `let y = x.copy()` allocates a new buffer; binding drop frees the buffer via `ynz_bignum_free`. No reference counting, no GC.
2. **Parse**: extend `decimal128::parse` for arbitrary precision. Decimal-string → bignum chunks.
3. **Add/Sub**: schoolbook addition on chunks with carry propagation. O(chunks).
4. **Mul**: Karatsuba multiplication on chunks. O(chunks^log2(3)). Cap at the precision boundary — extra digits round half-even.
5. **Div**: long division with half-even rounding at the precision boundary. O(chunks^2) worst case — acceptable per design's "predictable performance" character.
6. **Format**: bignum → string (reuse the decimal128 format logic, generalized for chunk count).
7. **Mixed-precision typeck**: when typechecking `BinOp { lhs: number<A>, rhs: number<B> }`, result type = `number<max(A,B)>`. Both operands implicitly promote to the wider precision.
8. **Narrowing warning typeck**: when typechecking `let x: number<B> = expr` where expr has type `number<A>` and A > B, emit a Tier 2 warning per spec.
9. **Codegen**:
   - For `number<N>` with N ≤ 34: existing {i64, i64} ABI unchanged.
   - For N > 34: pass values as `i8*` (pointer to heap-allocated bignum struct). Arithmetic dispatches to `ynz_bignum_*` runtime functions.
   - Promotion at binary op: if one operand is hardware-path and the other is bignum, allocate a bignum copy of the hardware-path value and operate as bignum. Result is bignum.
   - Narrowing at assignment: emit a narrowing call that takes the bignum and the target precision; produces a hardware-path or smaller-bignum value with proper rounding.
10. **Conformance testing**:
   - IEEE 754-2008 decimal128 vectors: extend the existing test suite to cover N > 34.
   - Python `decimal` differential: 10k random (a, op, b, precision) tuples per op. Generate via proptest seeds; compare bit-pattern of result. Fail on any mismatch.
   - Property tests: commutativity (`a + b == b + a`), associativity (where applicable), round-trip identity (`parse(format(x)) == x` — note: this is parse/format identity, NOT arithmetic identity; see "Round-trip equality NOT guaranteed" note below).
   - **Deterministic vectors** (locked in `crates/ynz-numerics/tests/deterministic_vectors.rs`) — written into the plan NOW so an executor cannot tune-the-test-to-fit-the-impl. Each row's expected output was computed from Python `decimal` with `getcontext().prec` set to the listed precision and `rounding=ROUND_HALF_EVEN`:

     | # | Inputs | Op | Precision | Expected output | Test concern |
     |---|---|---|---|---|---|
     | 1 | `Decimal('0.1')`, `Decimal('0.2')` | + | 100 | `Decimal('0.3')` | Decimal-arithmetic exactness; classic JS-trap regression test |
     | 2 | `Decimal('0.5')` | narrow 34 → 33 | (halfway tie) | `Decimal('0')` (half-even → even) | Half-even rounding mode at the tie |
     | 3 | `Decimal('1.5')` | narrow 34 → 33 | (halfway tie) | `Decimal('2')` (half-even → even) | Half-even rounding ties consistently to even |
     | 4 | `Decimal('2.5')` | narrow 34 → 33 | (halfway tie) | `Decimal('2')` (half-even — `2` is even, NOT `3`) | Half-even ≠ half-up; this is the bit-for-bit Python match |
     | 5 | `Decimal('1')`, `Decimal('3')` | / | 100 | `Decimal('0.333...333')` with 100 threes, last digit rounded half-even | Division of repeating fraction at precision boundary |
     | 6 | `Decimal('9.999...9')` (34 nines), `Decimal('0.000...01')` (35-digit place) | + | 35 | `Decimal('1.000...0E+0')` (35 zeros — carry propagates from hardware path into bignum chunk boundary) | Hardware-to-bignum carry-propagation; tests the cross-storage-model boundary |
     | 7 | `Decimal('1')`, `Decimal('1') * Decimal('1E+200')` | + | 200 | `Decimal('1.000...01E+200')` (199 zeros) | Extreme operand-magnitude asymmetry; tests the Karatsuba mul + add alignment |
     | 8 | `Decimal('0')`, `Decimal('-0')` | + | 34 | `Decimal('0')` (per IEEE 754 — +0 + -0 = +0) | Sign-zero edge case (IEEE 754 specific) |
     | 9 | `Decimal('1E+50')`, `Decimal('-1E+50')` | + | 100 | `Decimal('0')` exactly | Large-magnitude cancellation |
     | 10 | `Decimal('-0')`, `Decimal('1')` | * | 34 | `Decimal('-0')` (per IEEE 754 sign rules) | Negative-zero sign propagation through multiply |
     | 11 | At N=68 (exact 2-chunk boundary): `Decimal('1E+34')` (= 10^34, the first value that overflows the 34-digit hardware coefficient), `Decimal('1')` | + | 68 | `Decimal('10000000000000000000000000000000001')` (a 35-digit integer — coefficient stored in the 2nd u128 chunk + 1st chunk; exponent = 0). Python: `getcontext().prec=68; Decimal('1E+34') + Decimal('1')` returns this exactly. | Hardware-coefficient overflow: result requires 35 digits, exceeds the hardware-fast `MAX_COEFFICIENT` (= 10^34 − 1 per `crates/ynz-numerics/src/decimal128/bits.rs:32`), forces bignum path |
     | 12 | At N=2049 (mid-range odd chunk count, ⌈2049/34⌉=61): `Decimal('1') / Decimal('7')` | / | 2049 | Python `decimal`-computed result (canonical) | Odd-chunk-count division — verifies bignum loop handles non-aligned chunk counts |
     | 13 | At N=4096 (max precision): `Decimal('1.234...') * Decimal('5.678...')` (random but seeded) | * | 4096 | Python-computed canonical | Largest-precision multiplication — must not exceed predictable-performance bound (< 10ms per op) |
     | 14 | Mixed-precision: `Decimal('9.999...9')` at N=35 (35 nines) | narrow 35 → 34 | (narrowing causes carry from bignum into a hardware-path representation) | `Decimal('1.000...0E+1')` (34 zeros, exponent shifts up) | Narrowing AT the boundary value where carry crosses the storage-model boundary in REVERSE |

     **Worked Paper-Trace for row 1 (canonical example)**: Inputs `a = Decimal('0.1')` stored as coefficient `1`, exponent `-1`; `b = Decimal('0.2')` stored as coefficient `2`, exponent `-1`. Expected: align exponents (both at `-1`), sum coefficients (`1 + 2 = 3`), result `coefficient=3, exponent=-1` = `0.3`. Compare bit-patterns of stored decimal128 representation. Python: `Decimal('0.1') + Decimal('0.2')` returns `Decimal('0.3')` (exact, not `Decimal('0.300...4')` as binary float would).

   **Round-trip equality NOT guaranteed for arithmetic**: per Python `decimal` behavior, `(x / y) * y` does NOT generally equal `x` due to rounding at the precision boundary. Yinz's `number<N>` follows this same semantic — intermediate operations round to N digits half-even, and round-trip identity holds only for `parse(format(x)) == x` (lossless string round-trip), NOT for arithmetic round-trips. Document this in `spec/numeric-types.md` as part of the M8 spec update.
11. **Integration test flip**: `m2_bignum_deferral_produces_diagnostic` now ASSERTS the program runs and prints the correct high-precision value, not the deferral diagnostic. Snapshot updated. The `CATCH-UP M8` comment in integration.rs is removed (the catch-up has happened).
12. **Demo + error gallery** extensions per spec.
**Acceptance criteria**:
- [ ] `let x: number<100> = 1.0 / 3.0` produces a 100-digit result matching Python `decimal` getcontext().prec=100.
- [ ] `let a: number<34> = 1.0; let b: number<100> = 2.0; let c = a + b` → `c` is `number<100>` (promoted) per design.
- [ ] `let a: number<100> = 1.0; let b: number<34> = a` → narrowing warning emitted; `b` ends up rounded to 34 digits half-even.
- [ ] `let huge: number<5000> = 0.1` → existing compile error preserved (cap is 4096).
- [ ] 10k differential tests against Python `decimal` pass for each of add/sub/mul/div at random precisions in {35, 70, 200, 1000, 4096}.
- [ ] `cargo bench` shows N ≤ 34 ops within 5% of M7 baseline (no hot-path regression).
- [ ] `m2_bignum_deferral` integration test FLIPS from "expect diagnostic" to "expect success".
- [ ] `cargo test --workspace` green.
**Quality gate**:
- [ ] Bignum ops PANIC at runtime if they detect bit corruption (defensive — should never happen, but the cost is one tag check).
- [ ] Hot-path (34-digit) bench delta < 5%.
- [ ] No mutable static state in bignum (all values are owned per Yinz's no-shared-mutable rule).
- [ ] Conformance tests run as part of `cargo test --workspace` — no opt-in flag.
**Verification**: `cargo test -p ynz-numerics conformance` + `cargo test -p ynz-numerics differential` + `cargo test -p ynz-driver m8_bignum` + `cargo bench --bench decimal128_hot_path`.

---

### Phase 7: Final fixtures + demo polish + M8 errors gallery completion
**PR scope**: All M8 fixtures consolidated; `examples/pirates-roster/` end-to-end demo runs all M1–M8 features; `examples/primantis-orders/m8_errors.ynz` triggers every M8 compile-error class; final pass on the integration test suite for v0.1 surface.
**Branch**: `feat/m8-fixtures-demo`
**Flag**: N/A
**Est. lines**: ~300
**Ships via**: `/pr`
**Objective**: All loose ends from P1–P6 closed. Demo + error gallery complete. patrick's hands-on UX review surface ready.
**Why this phase exists**: M5/M6/M7 each had a dedicated fixtures + demo phase (M7 P5). Concentrating this work in one PR makes the demo review focused: patrick runs `examples/pirates-roster/` once, runs `examples/primantis-orders/m8_errors.ynz` once, and gives a thumbs-up or itemizes UX gaps.
**Current-state anchors**: Each prior M8 phase added partial fixtures + demo content. P7 consolidates, fills gaps, and ensures every M8 feature has at least one fixture demonstrating the success path AND one error trigger.
**Files (expected scope)**:
- `crates/ynz-driver/tests/fixtures/m8_*.ynz` (any missing fixtures filled in)
- `examples/pirates-roster/entrypoint.ynz` (final pass — every section flows, M8 features integrated with M1–M7)
- `examples/pirates-roster/src/services/*.ynz` (polish — comments cleaned, doc comments on every export, sensitive used in a realistic API-key pattern, concurrency in a realistic checkout flow, bignum in a realistic high-precision calc)
- `examples/pirates-roster/README.md` (UPDATE the milestone table to mark M8 as shipped)
- `examples/primantis-orders/m8_errors.ynz` (every M8 compile-error class triggered; one comment per trigger explaining what it tests)
- `examples/primantis-orders/README.md` (UPDATE)
- `crates/ynz-driver/tests/integration.rs` (final integration tests for full-pipeline M8)
**Deviation rule**: P7 must NOT introduce new language features. Anything found in P7 that requires new feature work goes BACK to P1–P6 as a fix — P7 is consolidation only.
**Steps**:
1. Run `examples/pirates-roster/entrypoint.ynz` end-to-end. Capture stdout. Compare to expected golden output for each M-section.
2. Run `examples/primantis-orders/m8_errors.ynz` in report-only mode (Yinz multi-errors). Capture stderr. Compare to expected golden diagnostic stream.
3. Identify gaps — any M8 feature without a basics-demo section? Any compile-error class without a gallery trigger? Fill them.
4. Polish: rewrite any awkward demo prose; ensure every section uses real Yinz operations from the current scope (per `.claude/rules/dot-postfix.md` examples rule).
5. README updates: milestone table marked, deferred-features list refreshed (anything M8 newly closed gets removed; v0.2/v0.3 entries verified accurate).
6. Final integration test: a single `examples_basics_runs_end_to_end` test that builds + runs + asserts stdout matches the golden file. **Golden file location**: `examples/pirates-roster/expected_stdout.txt` (committed to the repo; regenerated only when patrick approves a demo behavior change). P7 also commits `examples/pirates-roster/expected_stdout.txt.regenerate.sh` — a script that runs the demo and writes the captured stdout to the golden file. Reviewer protocol: if `examples_basics_runs_end_to_end` fails, EITHER the demo regressed (fix the regression) OR the demo intentionally changed (run the regenerate script + commit the new golden + explain the change in PR description). Never blind-update the golden.
7. **Combined-feature integration fixtures** (NEW per Required Fix #10 from plan-review — catches combinatorial bugs that per-feature fixtures miss). Add at minimum these three fixture projects:
   - `crates/ynz-driver/tests/fixtures/m8_combo_modules_sensitive_concurrency/` — multi-file project where one module exports a `sensitive` API-key type, another module imports it, and the third module calls a `background` request function that takes the sensitive value as a `give` argument. Asserts: import resolution preserves sensitivity, redaction happens at print across module boundaries, `background` ownership inference picks `.give()` correctly.
   - `crates/ynz-driver/tests/fixtures/m8_combo_modules_bignum_interpolation/` — multi-file project where module A defines a high-precision constant `let CHAOTIC_INITIAL: number<200> = ...`, module B imports it and computes `CHAOTIC_INITIAL * x` returning `number<200>`, module C imports both and interpolates the result into a string `` `Result: ${value}` ``. Asserts: bignum survives across module boundaries, interpolation handles `number<N>` for N > 34, the formatted string matches Python `decimal`'s output bit-for-bit.
   - `crates/ynz-driver/tests/fixtures/m8_combo_doc_sensitive_bignum/` — single-module project demonstrating all three on one shape: `/// API key (sensitive). \nexport shape ApiClient { /// the secret key — never log. \n key: sensitive string, /// rate-limit budget at high precision \n budget: number<100> }`. Asserts: doc comments attach to the shape AND each field; sensitive + bignum coexist on the same shape; `print(client)` redacts only the sensitive field.

   Each fixture project gets its own integration test that builds, runs, and asserts golden output. Failure of any combo test = combinatorial bug; routed to the responsible phase (P2/P4/P5/P6) for fix, NOT a P7 issue.
**Acceptance criteria**:
- [ ] `./target/debug/ynz run examples/pirates-roster/` runs successfully and produces the golden output (`examples/pirates-roster/expected_stdout.txt`).
- [ ] `examples/primantis-orders/m8_errors.ynz` triggers EVERY M8 compile-error class (count: ~20 distinct triggers).
- [ ] `examples/pirates-roster/README.md` accurately reflects M8 status.
- [ ] No feature added in P1-P6 lacks a basics-demo section.
- [ ] No compile-error class added in P1-P6 lacks an m8_errors gallery trigger.
- [ ] All three combined-feature integration tests pass (`m8_combo_modules_sensitive_concurrency`, `m8_combo_modules_bignum_interpolation`, `m8_combo_doc_sensitive_bignum`).
- [ ] `cargo test --workspace` green; all integration tests pass.
**Quality gate**:
- [ ] patrick runs the demo manually and confirms UX feels right (golden output review).
- [ ] patrick runs the error gallery manually and confirms diagnostics teach properly.
- [ ] No "TODO" / "PLACEHOLDER" / `// will add later` comments in any M8 fixture/demo.
**Verification**: `./target/debug/ynz run examples/pirates-roster/` + `./target/debug/ynz run --max-errors 100 examples/primantis-orders/m8_errors.ynz` + `cargo test --workspace`.

---

### Phase 8: v0.1.0 audit sweep + version bump + tag + release
**PR scope**: `/audit` across the integrated v0.1 surface; fix CRITICAL findings; bump `Cargo.toml` to `0.1.0`; CHANGELOG entries for M1–M8 generated from PR merge history; tag `v0.1.0` with patrick's approval; flip v0-1-compiler.md M8 status `active`→`done`.
**Branch**: `release/v0.1.0`
**Flag**: N/A
**Est. lines**: variable (audit findings drive scope; could be small if clean, larger if multiple issues surface)
**Ships via**: `/release`
**Objective**: v0.1.0 tag pushed. No public launch (per locked scope) — internal milestone only.
**Why this phase exists**: M7 was a per-milestone release tag. v0.1.0 is the cap on v0.1 development. /release skill is the canonical path; /audit gates the tag.
**Current-state anchors**:
- `Cargo.toml` workspace.package.version (currently `0.1.0-m7`; bumps to `0.1.0`).
- `CHANGELOG.md` per existing format.
- `.claude/plans/active/v0-1-compiler.md` M8 status block.
**Files (expected scope)**:
- `Cargo.toml` (version `0.1.0-m7` → `0.1.0`)
- `Cargo.lock` (regenerated)
- `CHANGELOG.md` (full M1-M8 cumulative entries; if existing per-milestone tags are recorded, link to them)
- `.claude/plans/active/v0-1-compiler.md` (status `active` → `done`; bump last_updated)
- Any files needing fix-up from audit findings (TBD by audit results)
- `.analysis/*.md` (audit output; not committed to main — staged under `.analysis/` per CLAUDE.md rule)
**Deviation rule**: P8 must NOT introduce new features. Audit-driven fixes are bug fixes only. If the audit surfaces a missing feature (e.g., "sensitive doesn't propagate through interpolation"), that's a P4 BUG and is fixed as part of P4 follow-up, NOT as a P8 deviation.
**Steps**:
1. Run `/audit` skill against the working tree. Produces `.analysis/*.md` reports across security, reliability, bugs, performance, etc.
2. Triage findings per CLAUDE.md rule 11 (no priority deflection — every confirmed real finding gets fixed).
3. Fix CRITICAL findings inline. If a finding requires a new feature, route it back to the appropriate P1-P6 (separate PR).
4. Generate CHANGELOG section from git log of merged PRs since v0.1.0-m7 tag. Per /release skill conventions.
5. Bump Cargo.toml workspace version to `0.1.0`. Run `cargo build --workspace` to refresh lockfile.
6. Final test run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo fmt --check`. All green.
7. Tag `v0.1.0` with patrick's explicit approval (per /release skill flow). Push tag.
8. Flip v0-1-compiler.md M8 status `active` → `done`. Bump last_updated.
9. (Optional, per Step 5e: archive v0-1-compiler.md to `.claude/plans/done/`. Decision deferred — flip status first, archive at end of session if no v0.1.1 work expected.)
10. Move this plan file to `.claude/plans/done/m8-modules-doc-sensitive-concurrency-bignum-release.md`.
**Acceptance criteria**:
- [ ] `/audit` ran and produced .analysis/*.md reports.
- [ ] Every CRITICAL finding has a fix landed (either in P8 inline or routed to a P1-P6 follow-up PR).
- [ ] `cargo test --workspace` green at workspace version `0.1.0`.
- [ ] `cargo clippy --workspace -- -D warnings` green.
- [ ] `Cargo.toml` at `0.1.0`. `Cargo.lock` regenerated.
- [ ] CHANGELOG has a `## [0.1.0] - 2026-XX-XX` section with all M1-M8 highlights.
- [ ] `git tag v0.1.0` pushed (with patrick approval — explicit YES).
- [ ] v0-1-compiler.md radar shows M8 as `done`.
- [ ] Plan file moves to `done/`.
**Quality gate**:
- [ ] No "DEFERRED" or "TODO" left in v0.1 surface. Anything not done is documented as v0.2+ scope in mvp-scope.md.
- [ ] Tag commit has v0.1.0 in its commit message subject + body lists M1-M8 milestones.
- [ ] No public-facing announcement (per locked scope — v0.1.0 is internal).
**Verification**: `git tag --list v0.1.0` + `cargo --version` (verify Cargo.toml) + `cat .claude/plans/done/m8-*.md` (confirm move).

---

## Quality Checklist (verify at completion)

- [ ] All inputs validated (parser handles malformed yinz.toml; lexer rejects malformed `///` boundaries; etc.)
- [ ] Auth/authz: N/A (compiler project, no auth surface)
- [ ] Error handling: every diagnostic three-part WHAT/WHAT-INSTEAD/WHY; banned-jargon test sweep passes
- [ ] No SQL/XSS/path-traversal/secret-exposure: N/A (compiler project)
- [ ] Performance: bignum hot path (N ≤ 34) within 5% of M7 baseline (bench)
- [ ] Tests: ~150-200 new tests across M8; M7's 782 still pass
- [ ] Existing tests still pass (regression sweep)
- [ ] Types are complete (no `any` — Rust workspace; no `unwrap()` in pipeline-critical code without test-ratchet rationale)
- [ ] Follows existing codebase conventions (M3-M7 patterns mirrored in each phase)
- [ ] Bouncer 6-subsection Invariants block present (see above)
- [ ] Banned-jargon sync test green (`crates/ynz-diagnostics/src/banned_jargon.rs` matches design/compiler-errors.md)
- [ ] Examples basics demo runs end-to-end and prints golden output
- [ ] Examples errors gallery triggers every M8 compile-error class
- [ ] `cargo bench --bench decimal128_hot_path` within 5% of M7 baseline
- [ ] `cargo clippy --workspace -- -D warnings` green
- [ ] `cargo fmt --check` green

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each phase ships as its own PR via `/pr`. Stop between phases per Step 9 of /plan. P2 may split into P2a/P2b if it grows past ~1000 lines, but each remains its own PR.
- **Shadow main branches**: each phase branches off `main` after the previous phase merges. No multi-week feature branches; each phase is one session of work.
- **Building the engine before shipping value**: every phase delivers user-observable Yinz-language progress. P0 ships a syntax migration users can write; P1 ships keyword reservations + redirect diagnostics; P2 ships full multi-file projects; P3 ships doc preservation; P4 ships sensitive surface; P5 ships concurrency keywords; P6 ships bignum; P7 polishes; P8 tags. No phase is "infrastructure for a future phase" — every phase ships features.
- **Hotfix that isn't**: P8 audit-driven fixes are routed to the appropriate phase (P1-P6) as separate PRs if they're feature-shaped. The release PR contains only audit-bug fixes + version bump + CHANGELOG.
- **Abandoned branches**: each phase branch deletes after merge. No long-lived integration branches.
- **Flag graveyards**: M8 has no feature flags. All features ship behind the v0.1.0 tag boundary. The /audit skill's pre-ship review is the gate, not feature flags.
- **P6 hot-path "simplification"**: a tired implementer who notices that `if precision <= 34 { hardware_path } else { bignum_path }` could be "simplified" by unifying both paths into the bignum path will tank the 34-digit benchmark by 10-50× (chunked u128 arithmetic vs single u128 register ops). The deviation rule in P6 explicitly says "must NOT change the 34-digit hot path." Any PR that touches `crates/ynz-numerics/src/decimal128/` outside of mechanical refactoring requires a `cargo bench --bench decimal128_hot_path -- --baseline m7_baseline` run posted in the PR description showing < 5% drift. The m7_baseline was captured in P0 specifically to make this gate enforceable.

---

## Reviewer Disputes

This section tracks plan-reviewer feedback rounds and any push-backs from the planner.

### Round 1 (2026-05-18) — Plan reviewer verdict: BLOCK (10 required fixes)

All 10 required fixes accepted and addressed:

1. **Bignum boundary cases** → Added `### Deterministic Vectors` table inside P6 Step 10 with 14 worked rows (including the canonical `0.1 + 0.2 = 0.3` Paper-Trace example, half-even tie cases at 0.5/1.5/2.5, division-of-repeating-fraction at N=100, hardware-to-bignum carry boundary, sign-zero edges, mid-range odd-chunk-count division, N=4096 perf-bound multiplication, mixed-precision narrowing at the boundary). Each row's expected output is Python `decimal`-derived; locked in `crates/ynz-numerics/tests/deterministic_vectors.rs`.

2. **`///` ambiguous case `///foo\n// bar\n/// baz\n`** → Locked the rule in P1 Step 4: regular `//` comments are trivia and do NOT break the chain; result = `"foo\nbaz"` attached. Worked example added in P1.

3. **Blank-line break detection mechanism** → Locked via lexer-emitted `Token::DocComment { content, break_after }`. The `break_after: bool` flag is set by the lexer when scanning forward through trivia, eliminating the handwaving "parser detects blank lines" problem.

4. **Sensitive propagation table** → Enumerated full 16-method M7 string-method table in P4 Step 4 with locked propagation decisions + reasoning per method. Added a completeness-check test that fails compile if a new M7 string method is added without a propagation rule.

5. **Background `.share` rejection rule precision** → Locked at SIGNATURE level (not call-site form). Even `background fn(x.copy())` is rejected if `fn`'s sig is `share`. Rationale: the v0.3 contract is what's locked; M8's sequential lowering doesn't relax the contract. Paper-Trace examples (accepted/inferred-copy/rejected/still-rejected) added to P5 Step 5.

6. **Bignum storage model** → Locked to heap-allocated single-owner value-semantics. Hybrid stack/heap rejected (requires const generics, v2+). Kernel-mode behavior clearly stated: N > 34 is a compile error in `--kernel` until v0.3 plug-in allocator API ships.

7. **Mixed-precision round-trip equality** → Added explicit "Round-trip equality NOT guaranteed for arithmetic" note in P6 Step 10. `parse(format(x)) == x` holds (lossless string round-trip); `(x / y) * y == x` does NOT generally hold (precision-boundary rounding accumulates). Spec/numeric-types.md to be updated in P6 spec sweep.

8. **`number[N]` migration layer** → Corrected: redirect happens at PARSER level (`parse_number_type`), not lexer. The lexer emits `Token::Number` then `Token::LBracket` independently; the parser has the multi-token context. P0 Step 3 reworded.

9. **Background ownership inference vs Graveyard Entry 2** → Reframed entirely. Compiler INFERS `.copy()`/`.give()` at call site per usage; IDE muted hint surfaces the choice (informational only — call-site modifiers aren't typeable Yinz syntax per inference.md; click jumps to function signature). Explicit `.copy()` is LEGAL (one of several legal forms per Graveyard Entry 2), never required. Tier 3 lint `expensive-background-copy` is the suggestion path (deferred to v0.4 per the locked auto-promotion analysis).

10. **Combined-feature integration tests** → Added three combo fixtures to P7 Step 7: modules+sensitive+concurrency, modules+bignum+interpolation, doc+sensitive+bignum. Each gets its own integration test asserting golden output. Failure routes to the responsible phase for fix, not P7.

Non-blocking concerns also addressed:
- M7 baseline commit named: `v0.1.0-m7` / `b24a1b0`. P0 Step 8 captures the named baseline via `cargo bench --save-baseline m7_baseline`.
- Salsa cache invalidation test mechanism specified: parse-counter `Arc<AtomicU64>` (`#[cfg(test)]`-gated) + assert edit-one-file does-not-bump-other-file's counter.
- Tree-shaking test mechanism specified: `nm -g <binary>` symbol dump + string-literal grep for `UNUSED_MARKER_xyz` (full strip, not just symbol).
- Golden output file location: `examples/pirates-roster/expected_stdout.txt`, committed; regenerate via committed script with explicit reviewer protocol.
- P6 hot-path "simplification" anti-pattern added to Anti-Pattern Callouts.

No push-backs filed against any required fix — all ten represented real silent-bug or anti-pattern risks the original draft did not adequately defend against.

### Round 2 (2026-05-18-r2) — Plan reviewer verdict: PASS (5 non-blocking concerns)

All 5 non-blocking concerns addressed pre-implementation:

1. **Row 11 in determinism table** → Replaced the self-flagged "revise at impl time" row with a concrete locked case: `Decimal('1E+34') + Decimal('1')` at prec=68 = `10000000000000000000000000000000001` (35-digit integer). This tests the hardware-coefficient overflow boundary AT the exact MAX_COEFFICIENT cited in `crates/ynz-numerics/src/decimal128/bits.rs:32`. Locks Python-derivable expected.

2. **P5 Step 6 auto-copy-for-heap sneak-in** → Reframed to MATCH the M4 contract: auto-copy preserved ONLY for trivially-copyable scalars (int, float, bool); heap-owning values (shapes, arrays, maps, strings) REQUIRE explicit `.copy()` at the call site when used after `background`, enforced via three-part teaching diagnostic. The post-use-without-copy case is now a REJECT, not a silent auto-copy. Matches `spec/concurrency.md` line 194-199 exactly. Explicitly documented why this is NOT Graveyard Entry 2 (the rule is type-flow-driven for non-trivial cost cases, not a blanket call-site annotation requirement).

3. **P2 Item AST shape duct-tape** → Locked: 6 final Item variants — Function, Shape, Options, ImportDecl, ConstDecl, ReExport. `is_exported: bool` on Function/Shape/Options/ConstDecl. No code-time TBD. P2 research step verifies whether `Item::ConstDecl` already exists; if not, P2 adds it.

4. **P5 Step 7 duplicate numbering** → Fixed: steps now run 1–10 (was 1, 2, ..., 7, 7, 8, 9 — duplicate 7 from a Step 6 insertion).

5. **Demo restructure byte-identical assertion** → Implicit in P2 acceptance criterion 1 ("prints the M1-M7 demo output (no regression)") but worth surfacing: a byte-equality assertion is the failure mode the integration test catches. The `expected_stdout.txt` golden (P7) is the canonical comparison artifact; P2's "no regression" claim is verified by running the existing M7 demo command and capturing baseline stdout BEFORE the split, then asserting the restructured demo produces byte-identical output AFTER the split. P2 executor: run `./target/debug/ynz run examples/pirates-roster/entrypoint.ynz > /tmp/m7_baseline_stdout.txt` before any restructure, then diff post-restructure output against the baseline. Lock the protocol.

Additional non-binding suggested adversarial cases from round 2 (not required, but tracked for potential P6/P4/P2 fixture-corpus expansion):
- **Bignum negative-precision deep-narrow** (5000 → 10 with complex trailing carry chains) — augments row 14
- **Sensitive array indexing** (`parts[0]: sensitive string` when `parts: array<sensitive string>`) — augments sensitive propagation table; P4 fixture
- **Module re-export shadowing** (`export { foo } from "a"` collides with local `export function foo`) — P2 fixture
- **Background ownership on dynamic dispatch** (`background dyn.run()` where `dyn: dynamic Runnable`) — P5 fixture; rule: dynamic-dispatch background follows the contract method's signature

These four suggestions are filed as "P-phase fixture corpus expansion" — they extend test coverage without changing semantics. Executors of P2/P4/P5/P6 should consider adding fixtures for each if test budget allows; not required for the milestone to ship.

---

## Out-of-Scope For This Plan (per-milestone guardrails)

### Out-of-scope for M8 (CURRENT — do NOT slip these in)

- **Auto-parallelization** (`wait`/`background` runtime dependency graph + thread pool) — v0.3 per mvp-scope.md
- **Background handle communication** (`.send()`/`.receive()`) — v0.3 per design/concurrency.md
- **Database operations under concurrency** (`db.insert()` sequencing) — MVP2 per design/concurrency.md
- **`env.get()` returning `sensitive string` by default** — v0.7 per mvp-scope.md
- **`ynz doc` static API documentation generator** — v1.1 per mvp-scope.md
- **LSP / muted-hint IDE surfaces** — v0.2 per mvp-scope.md
- **Tier 3 lint suggestions** (`unused-import` warning is in scope as a parse/typeck warning, but the proactive lint tier `unnecessary-wait`, `doc-on-private-item`, `prefer-explicit-precision-when-mixing` etc. defer to v0.4)
- **Public package registry** — v1.2 per mvp-scope.md
- **Public language launch** (announcements, docs site, etc.) — v1.0 per mvp-scope.md
- **Operator overloading** for `+`/`-` on user types — v1.0 per mvp-scope.md
- **Custom iterables** (user types implementing `follows Iterable<T>`) — v1.0 per mvp-scope.md
- **Sized integer variants** (`int<8>`, `int<32>` etc.) — v2+ per mvp-scope.md
- **Sized float variants** (`f32`) — v2+ per mvp-scope.md
- **Arbitrary-precision decimal beyond `number<4096>`** — v2+ per mvp-scope.md
- **FFI** (`foreign` keyword) — v2+ per mvp-scope.md
- **GPU dispatch** — v2+ per mvp-scope.md
- **Arena allocators in user code** (`arena scratch { ... }` blocks) — v0.2 per design/future/arena.md
- **Arena allocators in compiler internals** (compile-speed optimization) — M8 polish per design/decisions.md; deferred as a v0.2 follow-up since it's a perf optimization, not a user-facing v0.1 feature. Documented in `.claude/todos.md` after this plan lands.
- **Sensitive `--reveal-sensitive` flag stripped from `ynz build --release`** — v0.4 release-mode work (no release-mode in v0.1)
- **Verified `{ }` blocks (unsafe escape hatch)** — v0.3+ per vocabulary.md
- **Self-references** — v0.3+ per design/future/self-references.md
- **Kernel-mode** (`--kernel` flag + plug-in allocator API) — v0.3+ per design/future/no-runtime-mode.md
- **Public registry, package install, lock file** — v0.22 per design/packages.md
- **Lint customization config** (`[lint]` in yinz.toml) — v1.x per design/linting.md

---

## Forward-Compatibility Constraints

Constraints M8 must preserve so v0.2+ can land cleanly:

- **`--reveal-sensitive` runtime flag stripped from release builds (v0.4)**: M8 ships this flag on `ynz run` for debug-mode override. v0.4 release-build work must add a compile-time check that strips the flag from release-mode binaries. M8 design assumes this future work — no flag-aware codegen in M8.
- **Background-task handle (v0.3)**: M8 rejects `let h = background foo()` (the handle form). v0.3 lifts this rejection and adds `.send()`/`.receive()` on a `Background<T>` type. M8's grammar (`Expr::Background(Box<Expr>)`) already supports the parse tree — v0.3 adds typeck + codegen for the handle.
- **Module re-export resolution depth (v0.22)**: M8 implements single-hop re-export (`export { X } from "..."`). Multi-hop chains work transitively via the same Pass-0 ExportTable. v0.22 may add cycle-detection in re-export chains (rare). M8's typeck already detects circular RE-EXPORTS (vs circular imports, which are fine).
- **Bignum precision cap (v2+)**: M8 caps at `number<4096>`. v2+ may lift this to truly arbitrary precision — when that happens, the existing `number<N>` storage layout (chunked-u128) generalizes cleanly. The 4096 limit is a precision-validity check at parse/typeck time, not a fundamental storage limitation.
- **Stdlib auto-import (v0.5+)**: M8 has no stdlib modules. The auto-import infrastructure (compiler knows the stdlib without `import` statements) ships with the first stdlib module in v0.5 (`file` + `path` + `directory`). M8's module resolver assumes "stdlib is empty for now" — when v0.5 lands, the resolver adds a special-case lookup for `<stdlib-module-name>` BEFORE consulting the project's user files.
- **LSP per-file salsa caching (v0.2)**: M8 multi-file driver creates one `SourceFile` salsa input per file. v0.2 LSP work depends on this granularity for edit-one-file-don't-re-parse-the-world incremental rebuilds. M8 codegen must keep cross-file dependencies expressed at the salsa-query level, not collapsed into one module-wide blob.
- **yinz.toml schema evolution (v0.22+)**: M8 ships 3 fields (`entry`, `name`, `version`). Unknown fields emit a warning (per Step 5a locked decision). v0.22 will add `[dependencies]` table; v1.x will add `[lint]` table. M8's parser must be lenient — unknown TOML keys/tables don't error.
- **`--kernel` mode compile errors for malloc-dependent features (v0.3+)**: When v0.3 lands kernel-mode, the compiler must emit a compile error for `number<N>` with N > 34 (heap allocation required) AND `sensitive` runtime print (depends on print which depends on libc) AND background handles (depend on scheduler). M8 must annotate these features as "requires runtime" in the codegen pass — the annotation is a future-Bouncer check, not user-visible in M8.

---

## Cross-references

- Master plan: `.claude/plans/active/v0-1-compiler.md` (M8 milestone block at line 206)
- Most recent shipped milestone: `.claude/plans/done/m7-strings-errors-iterables.md` (shape inspiration for M8)
- M4 ownership infrastructure (reused by P5 background analysis): `.claude/plans/done/m4-shapes-functions-ownership.md`
- M5 generics syntax migration (informs P0 `number[N]` → `number<N>` migration): `.claude/plans/done/m5-generics.md`
- Design docs: `design/modules.md`, `design/doc-comments.md`, `design/sensitive.md`, `design/concurrency.md`, `design/numeric-types.md`, `design/main-entry.md`
- Spec docs: `spec/modules.md`, `spec/doc-comments.md`, `spec/sensitive.md`, `spec/concurrency.md`, `spec/numeric-types.md`, `spec/main.md`
- Rules: `.claude/rules/non-oop.md`, `.claude/rules/plan-invariants.md`, `.claude/rules/auto-promotion.md`, `.claude/rules/stdlib-design.md`, `.claude/rules/vocabulary.md`, `.claude/rules/inference.md`
- Skills: `<project>/.claude/skills/pr/SKILL.md`, `<project>/.claude/skills/release/SKILL.md`
- Graveyard: `.claude/graveyard.md` (Const Deep-Immutability Invariant, Requiring Explicit Ownership Annotation at Call Sites)
- Mvp-scope: `design/mvp-scope.md` (v0.1 / v0.2 / v0.3 / v1.0 / v2+ split — every "out of scope" claim above cites a specific section)
