# Session State: ynz

**Last Updated**: 2026-05-17

---

## Active Workstreams

*(auto-rebuilt by SessionStart hook from `.claude/plans/active/*.md` front-matter — do not edit by hand)*

<!-- RADAR-START -->
- m4-shapes-functions-ownership (patrick) — 14 files touched — 0/117 done — 2026-05-16-r17
- v0-1-compiler (patrick) — 5 files touched — 0/184 done — 2026-05-12-r4
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

# Current branch: feat/m4-catchup-numerics (M4 P5 committed: 5a21258)
# M4 status: P1-P5 done; P6 (driver + fixtures) is next
```

---

## Active Decisions (append with WHY)

- [2026-05-12] **Compiler implementation language = Rust**: Mature LLVM bindings (inkwell), strong ADT/pattern-matching for AST, salsa framework gives incremental builds + LSP "for free." See `design/compiler-language.md`.
- [2026-05-12] **MVP scope split into v0.1 / v0.2 / v0.3 / v1.0 / v2+**: Concurrency keywords parse from day 1 but run sequentially until v0.3 (when auto-parallelization optimization engages). See `design/mvp-scope.md`.
- [2026-05-12] **Error auto-propagation = flow-sensitive narrowing (Option B under, Option A in feel)**: If user calls `.failed()` before using the success value, auto-propagation suppressed; otherwise compiler auto-propagates at first use. Same `.failed()`/`.or()` API works inside AND outside `errors` functions. See `design/errors.md`.
- [2026-05-12] **Generic functions = v0.1, `[T]` syntax with `follows` constraints inline**: Type inference at call sites. `where` clauses rejected — inline keeps constraint visible next to the parameter. See `design/generics.md`.
- [2026-05-12] **Numeric types = handwritten, validated against IEEE 754 test vectors**: `number` = decimal128 (default), `number[N]` up to N=4096, `float` = f64, `int` = i64. Sized variants (`int[N]`, `f32`) deferred to v2+. Overflow panics by default with `.wrappingAdd()`/`.saturatingAdd()` escape valves. See `design/numeric-types.md` + `design/mvp-scope.md#v2--deferred-features`.
- [2026-05-12] **Strings use `.get()` (code point) + `.byteAt()` + `.graphemeAt()`**: No `char` type. Default indexing is by Unicode code point. Bytes and graphemes are explicit alternates. See `spec/strings.md`.
- [2026-05-12] **Bracket sugar for `.get()` and `.set()` on all collections AND maps**: `arr[i]`, `m["key"]`, `s[i]` all desugar to `.get()`. Writes via `arr[i] = v` desugar to `.set()`. Strings immutable (no write sugar). Types reject bracket access entirely — forces dot for fields. Reverses earlier no-`map[key]` decision. See `design/collections.md`.
- [2026-05-12] **Iterable contract = two types (`Iterable[T]`, `FallibleIterable[T]`)**: In-memory collections follow `Iterable[T]`; I/O sources follow `FallibleIterable[T]`. Same `for` syntax; compiler infers fallibility from the source's contract and auto-propagates errors when needed. Stdlib adapters `.orSkipFailures()` and `.withErrors()` for ergonomic fallible-to-infallible conversion. See `design/iterables.md` + `spec/iterables.md`.
- [2026-05-12] **Import aliases + duplicate-name compile error**: TS-style `{ name as renamed }` and `namespace as renamed`. Duplicate names (including stdlib-vs-local collisions) refuse to silently pick — compile error forces aliasing. See `design/modules.md` + `spec/modules.md`.
- [2026-05-12] **Lock file = TOML, flat array of `[[package]]` tables**: Same format as `yinz.toml`. Diff-friendly, manually editable in emergencies. Install mechanism (content-addressed global cache, hard-links, parallel resolver, lazy integrity) aims for bun-class speed — v0.5 work. See `design/packages.md` + `spec/packages.md`.
- [2026-05-12] **Granular versioning sequence (23 versions to v1.0 + 3 post-launch versions)**: Each version ships ONE focused thing. v0.1 = core language only. v0.2 = LSP+watch+fmt. v0.3 = auto-parallelization. v0.4 = linting tier. v0.5 = package manager. v0.6-v0.23 = stdlib modules. v1.0 = stability + grammar lock. See `design/mvp-scope.md`.
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
