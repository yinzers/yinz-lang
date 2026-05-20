# Session State: ynz

**Last Updated**: 2026-05-20 (v0.2.0-m3 SHIPPED: ynz fmt formatter + CLI, 1143 tests, tag v0.2.0-m3)

---

## Active Workstreams

*(auto-rebuilt by SessionStart hook from `.claude/plans/active/*.md` front-matter — do not edit by hand)*

<!-- RADAR-START -->
### Active Roadmaps
- v0-2-dev-loop-tooling (patrick) — 1 active plans — 2026-05-20

### Active Workstreams
- v0-2-m4-watch (Patrick Rizzardi) — 9 files touched — 0/120 done — roadmap: v0-2-dev-loop-tooling — 2026-05-20
<!-- RADAR-END -->

---

## Environment & Commands (CRITICAL — survives compaction)

**Project**: ynz
**Language**: Rust (compiler implementation)
**Toolchain**: Rust 1.95 stable, LLVM 18.1.8, cargo workspace
**LLVM prefix**: `/usr/lib/llvm-18` (set in `.cargo/config.toml` via `LLVM_SYS_PREFIX`)

```bash
source $HOME/.cargo/env    # activate Rust in this shell session

cargo build --workspace    # build all crates
cargo test --workspace     # run all tests (310 as of M3)
cargo clippy --workspace -- -D warnings
cargo fmt --all

# Run the compiler
./target/debug/ynz run crates/ynz-driver/tests/fixtures/hello.ynz
# → hello, yinz

./target/debug/ynz run crates/ynz-driver/tests/fixtures/m3_fib.ynz
# → 55

./target/debug/ynz run crates/ynz-driver/tests/fixtures/m4_player.ynz
# → Patrick / 120 / Patrick  (M4 P4 success-criteria fixture)

# Current branch: main (v0.1.0 shipped; v0.2-M1 planning in progress)
```

---

## Active Decisions (append with WHY)

- [2026-05-12] **Compiler implementation language = Rust**: Mature LLVM bindings (inkwell), strong ADT/pattern-matching for AST, salsa framework gives incremental builds + LSP "for free." See `design/compiler-language.md`.
- [2026-05-12] **MVP scope split into v0.1 / v0.2 / v0.3 / v1.0 / v2+**: Concurrency keywords parse from day 1 but run sequentially until v0.3 (when auto-parallelization optimization engages). See `design/mvp-scope.md`.
- [2026-05-12] **Error auto-propagation = flow-sensitive narrowing (Option B under, Option A in feel)**: If user calls `.failed()` before using the success value, auto-propagation suppressed; otherwise compiler auto-propagates at first use. Same `.failed()`/`.or()` API works inside AND outside `errors` functions. See `design/errors.md`.
- [2026-05-17] **M4 shipped (tag v0.1.0-m4, 316 tests)**: shapes, UFCS methods, ownership modifiers (share/lend/give), const deep-immutability, extends/follows/base/hidden, LLVM readonly+noalias, ynz_alloc/ynz_free runtime shims, vtable globals, wrapping/saturating int arithmetic, type-attached constants. Plan: `.claude/plans/done/m4-shapes-functions-ownership.md`.
- [2026-05-12] **Generic functions = v0.1, `<T>` syntax with `follows` constraints inline**: Type inference at call sites. `where` clauses rejected — inline keeps constraint visible next to the parameter. See `design/generics.md`. (Syntax locked at `<>` not `[]` since 2026-05-17 M5 P0; `[]` is index access only.)
- [2026-05-12] **Numeric types = handwritten, validated against IEEE 754 test vectors**: `number` = decimal128 (default), `number[N]` up to N=4096, `float` = f64, `int` = i64. Sized variants (`int[N]`, `f32`) deferred to v2+. Overflow panics by default with `.wrappingAdd()`/`.saturatingAdd()` escape valves. See `design/numeric-types.md` + `design/mvp-scope.md#v2--deferred-features`.
- [2026-05-12] **Strings use `.get()` (code point) + `.byteAt()` + `.graphemeAt()`**: No `char` type. Default indexing is by Unicode code point. Bytes and graphemes are explicit alternates. See `spec/strings.md`.
- [2026-05-12] **Bracket sugar for `.get()` and `.set()` on all collections AND maps**: `arr[i]`, `m["key"]`, `s[i]` all desugar to `.get()`. Writes via `arr[i] = v` desugar to `.set()`. Strings immutable (no write sugar). Types reject bracket access entirely — forces dot for fields. Reverses earlier no-`map[key]` decision. See `design/collections.md`.
- [2026-05-12] **Iterable contract = two types (`Iterable[T]`, `FallibleIterable[T]`)**: In-memory collections follow `Iterable[T]`; I/O sources follow `FallibleIterable[T]`. Same `for` syntax; compiler infers fallibility from the source's contract and auto-propagates errors when needed. Stdlib adapters `.orSkipFailures()` and `.withErrors()` for ergonomic fallible-to-infallible conversion. See `design/iterables.md` + `spec/iterables.md`.
- [2026-05-12] **Import aliases + duplicate-name compile error**: TS-style `{ name as renamed }` and `namespace as renamed`. Duplicate names (including stdlib-vs-local collisions) refuse to silently pick — compile error forces aliasing. See `design/modules.md` + `spec/modules.md`.
- [2026-05-12] **Lock file = TOML, flat array of `[[package]]` tables**: Same format as `yinz.toml`. Diff-friendly, manually editable in emergencies. Install mechanism (content-addressed global cache, hard-links, parallel resolver, lazy integrity) aims for bun-class speed — v0.22 work. See `design/packages.md` + `spec/packages.md`.
- [2026-05-12; package mgr moved to v0.22 on 2026-05-20] **Granular versioning sequence (24 versions to v1.0 + 3 post-launch versions)**: Each version ships ONE focused thing. v0.1 = core language only. v0.2 = LSP+watch+fmt. v0.3 = auto-parallelization. v0.4 = linting tier. v0.5–v0.21 + v0.23–v0.24 = stdlib modules. v0.22 = package manager (moved late so it lands into a stable-ish language — no public release until v1.0 means early-shipping the package manager would just churn packages every pre-v1.0 breaking change). v1.0 = stability + grammar lock. See `design/mvp-scope.md`.
- [2026-05-12] **"Compiler IS the linter" — no separate `ynz lint` command**: Third tier of compiler diagnostics (suggestions). Customization via `[lint]` in `yinz.toml` ships v1.x. See `design/linting.md`.
- [2026-05-12] **Teaching mission codified**: All diagnostics follow WHAT/WHAT-INSTEAD/WHY three-part format. Enforced by Diagnostic constructor. IDE muted hints extend this to all teaching surfaces. See `design/teaching-mission.md`, `design/ide-hints.md`.
- [2026-05-12] **Compiler error style spec + jargon audit**: `design/compiler-errors.md` is the canonical style spec. Banned-jargon list enforced by `crates/ynz-diagnostics/src/banned_jargon.rs`. See `design/compiler-errors.md`.
- [2026-05-12] **Error-flow metaphor = "cascades", not "bubbles up"**: Per patrick's preference. Updated across all spec/design docs.
- [2026-05-12] **M1 compiler complete (820bfdc, tag v0.1.0-m1)**: `ynz run hello.ynz` → `hello, yinz`. 51 tests.
- [2026-05-12] **M2 decimal128 strategy = hand-rolled, no crates**: `int` = i64, `float` = f64, `number` = hand-rolled decimal128 in `ynz-numerics`, IEEE 754 test vectors.
- [2026-05-13] **M2 complete (tag v0.1.0-m2)**: Variables, arithmetic, all M2 types, 118+ tests. Full LLVM codegen for M2 AST.
- [2026-05-14] **Design-lockdown (PRs #5 + #14)**: `shape` keyword locked for M4 type declarations (not `type`). 3 new rule files (inference.md, plan-invariants.md, vocabulary.md). 5 graveyard entries. Golden Rules 8/11/12 clarified. M4+ plans must include 5-subsection Invariants block.
- [2026-05-14] **M3 complete (9653dbd, tag v0.1.0-m3)**: Control flow (`if`, `while`, `for`), multi-case `if`, user-defined functions with params + return types, two-pass typeck, return-path analysis, full LLVM lowering. 310 tests. `fib(10) = 55`. Plan: `.claude/plans/done/m3-control-flow-fns.md`.
- [2026-05-16] **M4 Doc-PRs 1+2 complete (54521dd)**: Non-OOP model locked. `shape`/`follows`/`extends`/UFCS locked. Body-level `.share/.lend/.give` removed. Annotation-only struct literals. `override` removed. All design+spec docs rewritten. Commit message: "M4 P1 (lexer) cleared to start."
- [2026-05-16] **M4 P1 complete (05c5296, merged to main)**: 8 new tokens (49→57). 6 banned-keyword handlers. All tests green.
- [2026-05-16] **M4 P2 complete (84db1d2, merged)**: AST + parser for shapes. 4 new Expr variants, 1 Stmt, 2 Type variants. 68 parse tests green.
- [2026-05-16] **M4 P3a complete (244ac6d, merged)**: ShapeTable, Type::Shape, struct-lit typeck, field access/assign, UFCS, hidden-field guard, base-shape guard. 90 typeck tests green.
- [2026-05-16] **M4 P3b complete (3508e7b, merged)**: `extends` field inheritance + cycle detection, `follows` contract verification, `Type::Dynamic`. 96 typeck tests green.
- [2026-05-17] **M4 P3c complete (7c86f6a, branch feat/m4-ownership, PR open)**: `is_consumed` scope tracking, use-after-give, const-cannot-be-lent/given. All 5 const deep-immutability paths covered. 102 typeck tests green. PR: https://github.com/patrickrizzardi/ynz/pull/new/feat/m4-ownership
- [2026-05-17] **M4 merged to main (direct merge by patrick)**: feat/m4-verification (P3c–P7 bundled — ownership, codegen, fixtures, v0.1.0-m4 prep) landed on main. Cargo.toml at `0.1.0-m4`. Tag `v0.1.0-m4` local.
- [2026-05-20] **v0.2-M2 SHIPPED (tag v0.2.0-m2, 1028 tests)**: ynz-lsp crate (JSON-RPC stdio, salsa-backed diagnostics/completion/hover), VSCode extension (.vsix + stable URL yinz-latest.vsix), registry-derived TM grammar (ynz-tmgrammar). Every registry entry auto-appears in IDE. Plan: `.claude/plans/done/v0-2-m2-lsp-thin-slice.md`.
- [2026-05-17] **M5 plan approved + Phase 0 shipped (524ca2e, branch chore/m5-doc-lockdown)**: Generics + Collections + Maybe<T> milestone planned and plan-reviewer Round 2 PASS. Locked decisions: `maybe<T>` moves from M6 to M5 (cleanest .get() API); map = Swiss Tables + SipHash-2-4 + perfect-hash for static-key literals; for-loop over built-in collections is typeck+codegen special-case with REPLACE-AT M7 markers; auto-promotion `array<T>` → `fixed<T>` ships codegen-only in M5 (Tier 3 lint defers to v0.4, muted hint defers to v0.2). 8-phase plan (P0-P6). P0 (doc lockdown — master plan M5/M6/M7 paragraphs updated to `<>`, design/maybe.md created with 9-row LLVM lowering decision table + 10-row flow-sensitive .value rules + 9-row none-inference rules + documented v0.1 cycle-leak limitation, spec/maybe.md syntax-updated) SHIPPED on branch `chore/m5-doc-lockdown`, awaiting merge to main. Plan: `.claude/plans/active/m5-generics.md`.
- [2026-05-17] **M5 P1 shipped (49940c9)**: Lexer + AST scaffolding. Tok::None, 3 Type variants, 2 Expr variants, 1 Stmt variant. 401 tests.
- [2026-05-18] **M7 SHIPPED (tag v0.1.0-m7, 782 tests)**: Full Unicode strings (backtick-only, NFC equality, 16 methods, SIMD search, interpolation builder), `errors` keyword (flow-sensitive auto-propagation, Frame/SourceLoc/trace, {i64,i64} ABI), Iterable<T> protocol (synthesized wrappers, user shapes, string iteration, range first-class, REPLACE-AT M7 markers all gone). 9 PRs across 11 phases (P0–P5 + P6). Plan: `.claude/plans/done/m7-strings-errors-iterables.md`.
- [2026-05-18] **M7 STARTED — P0 doc lockdown active (branch chore/m7-doc-lockdown)**: 4 locked decisions from pre-draft confirmed: (1) SSO ships in M7 (23-byte inline, 24-byte struct, tag byte at offset 23); (2) SIMD UTF-8 ships in M7 (`simdutf8` crate); (3) Synthesized iterator wrappers (`ArrayIter<T>`, `FixedIter<T,N>`, `MapIter<K,V>`, `StringCodePointIter`) + muted-hint surface deferred to v0.2 LSP; (4) Full base error shape ships: `.message`, `.suggestions`, `.trace` (`array<Frame>`), `.source` (`SourceLoc`), compile-time-emitted frame stack (NOT libunwind). P0 additional locks: single backtick-only string form (removed double-quote form), `.orSkipFailures()` is PURE (no I/O — separate `.logSkippedFailuresTo(sink)` for logging), `.withErrors()` returns `Iterable<maybe T errors>` (NOT `Iterable<Result<T>>`), `Frame.line` is `maybe int` one-based, `unicase` crate for case-folding, `unicode-normalization` + NFC cache bit, MapEntry dual destructuring forms. Plan: `.claude/plans/active/m7-strings-errors-iterables.md`.
- [2026-05-18] **M6 SHIPPED (tag v0.1.0-m6, 631 tests)**: All 8 phases P0–P6 done via PRs #25+#26. options types (i8 tags, exhaustiveness, toString), union types (tagged-struct, is-narrowing, shape aliases), fallible conversions (.toInt/.toFloat/.toNumber on float/number/string), early-return narrowing, 3 new design docs, M2+M3 catch-up closed. Plan archived: `.claude/plans/done/m6-options-unions.md`.
- [2026-05-17] **M5 FULLY SHIPPED (tag v0.1.0-m5, 574 tests)**: All 8 phases P0–P6 done via PRs #17–#24. Generics engine (MonomorphizationTable, GenericFnTable, GenericShapeTable, follows constraints), BuiltinArray/Fixed/Maybe/Map typeck, LLVM codegen (ynz_array_* runtime, {i64,i64} maybe, SipHash-2-4, Swiss Tables), 5 runnable fixtures, examples/basics M4+M5 showcase, m5_errors gallery. Plan archived: `.claude/plans/done/m5-generics.md`.

---

## Superseded / Archived

- (none)

---

## Project-Wide Notes

*(cross-workstream context, gotchas, user preferences not tied to one plan)*

- **M4 catch-up obligations from M3**: replace read-only params with `share`/`lend`/`give` ownership annotations; update `m3_share_param_deferral.ynz` stderr snapshot when `share` works; add overflow escape methods on `int`; add type-attached constants (`int.max`); generalize `PrimitiveIntrinsicTable` to general method dispatch.
- **M6 catch-up obligations from M3**: `is Type` narrowing in multi-case `if`; options-variant matching; exhaustiveness checking for options/unions.
- **M7 catch-up obligations from M3**: replace `range` builtin with `Iterable[T]` protocol; allow Range as first-class value; remove M3 special-cases in typeck + codegen; Unicode canonical equivalence for string multi-case (`ynz_string_eq`).
- **M4 plan must include**: 5-subsection Invariants block per `.claude/rules/plan-invariants.md` (Safety, Performance, Teaching, Runtime Dependencies, Kernel-Mode Behavior). const deep-immutability invariants required in Safety + Performance. `shape` keyword reservation in P1 lexer.
