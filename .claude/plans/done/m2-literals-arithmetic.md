---
slug: m2-literals-arithmetic
owner: patrick
status: done
files:
  - crates/ynz-numerics/**
  - crates/ynz-runtime/**
  - crates/ynz-parser/**
  - crates/ynz-typeck/**
  - crates/ynz-codegen/**
  - crates/ynz-driver/**
  - spec/operators.md
  - spec/variables.md
  - spec/numeric-types.md
created: 2026-05-12
last_updated: 2026-05-13
completed: 2026-05-13
tag: v0.1.0-m2
parent: v0-1-compiler
---

# Plan: M2 — Literals + Variables + Arithmetic (ARCHIVED)

**Parent milestone**: see `.claude/plans/active/v0-1-compiler.md` for the v0.1 roadmap.

**Scope**: `let` / `const` declarations with type annotations and local inference, integer / float / decimal literals, full operator set (arithmetic, comparison, boolean, bitwise) for `int` / `float` / `number[34]` / `bool`, polymorphic `print` over primitives, always-succeeds conversion intrinsics (`.toNumber`, `.toFloat`, `.toString`). Hand-rolled IEEE 754 decimal128 lives in a new `ynz-numerics` crate (Rust-internal) wrapped by a new `ynz-runtime` crate (C-ABI, builds to `libynz_rt.a`, linked into every generated binary).

**Outcome**: COMPLETE (2026-05-13) — commit c39fe8a on main, tagged `v0.1.0-m2`.

**Headline integration test (M2 contract)**:
```yinz
function main() -> nothing {
  let price = 0.1 + 0.2          // number, exact 0.3
  let count: int = 42
  let active = true
  print(price)                    // 0.3
  print(count * count - 1)        // 1763
  print(active && (count > 0))    // true
}
```
This file compiles, runs, and prints exactly `0.3\n1763\ntrue\n` on both Linux and macOS. Captured as `crates/ynz-driver/tests/fixtures/m2_smoke.ynz`.

---

## What M2 explicitly was NOT (deferred to later milestones, with explicit owners)

- **Bignum `number[N]` for N > 34** — syntactically reserved in M2; compile error at typeck pointing to M8. M8 carries the implementation.
- **Overflow escape valves: `.wrappingAdd()`, `.wrappingSub()`, `.wrappingMul()`, `.saturatingAdd()`, `.saturatingSub()`, `.saturatingMul()`** — int overflow panics; the escape methods don't exist yet. **M4 carries this.**
- **Type-attached constants: `int.max`, `int.min`, `number.max`, `number.epsilon`, etc.** — **M4 carries this.**
- **Fallible conversions: `.toInt()` on number/float, `string.toInt()` / `string.toNumber()` / `string.toFloat()`** — return `maybe T` which doesn't exist until M6. **M6 carries this.**
- **User-defined types, methods, ownership** — M4.
- **Generics, collections, monomorphization** — M5.
- **Control flow** (`if`, `for`, `while`, early return) — M3.
- **User-defined functions beyond `main`** — M3.
- **`maybe`, `options`, unions, narrowing** — M6.
- **Strings beyond ASCII byte arrays** — full Unicode + interpolation lands in M7.
- **Compound assignment / increment** — banned by spec, parser emits teaching diagnostic.
- **Ternary `?:`** — banned by spec, parser emits teaching diagnostic.

**Catch-Up list for downstream milestones**:
- **M4 must catch up**: overflow escape methods on `int`; `int.max` / `int.min` constants; `number.max` / `number.epsilon`; rewire M2's intrinsic-table dispatch to general method dispatch.
- **M6 must catch up**: `.toInt()` on number/float (returns `maybe int`); `string.toInt()` / `string.toNumber()` / `string.toFloat()` (return `maybe T`); compile-error suggestions for mixed-type arithmetic involving these fallible directions.
- **M8 must catch up**: bignum `number[N]` for N ∈ (34, 4096]; remove the M2 deferral compile error; finalise the IEEE-754-conformance test sweep on the bignum path.

---

## Spec corrections shipped in M2

| Inconsistency | Canonical decision | Fixed in phase |
|---|---|---|
| `spec/variables.md:48` said `let x = 42 // compiler knows: number`; `spec/numeric-types.md:206` said `let x = 42 // inferred as int`. | **`let x = 42` infers as `int`** per Golden Rule 10 ("efficiency first, dynamic after — default = most performant"). | P4 (typeck) |
| `spec/numeric-types.md:211` said "Mixed-type expressions promote to the most capable type in the expression." | **Mixed-type expressions are a compile error** with a three-part diagnostic pointing at the relevant `.toX()` method. No implicit numeric coercion. | P4 (typeck) |
| `spec/operators.md` did not list `%` in the arithmetic section. | Added `%` at precedence level 3. | P2 (lexer) |

---

## Pre-Phase-1 decisions locked (Sonnet review 2026-05-12)

- **`FloatLit(f64)` removed from Token + AST**: All decimal literals (including `1.0`) produce `NumberLit(String)` at lex time. The `float` type is set by typeck when it sees a `: float` annotation.
- **`PrimitiveIntrinsicTable` REPLACES `BuiltinTable`**: M2 does not add alongside M1's `BuiltinTable`. Phase 4 removes `BuiltinTable` and replaces it with `PrimitiveIntrinsicTable`. `print` becomes polymorphic over all primitive types.
- **`int.toString()` memory model = thread-local static buffer**: `@ynz_int_to_string` in the runtime uses a thread-local `[u8; 22]` buffer. Safe for M2's single-threaded programs. Same pattern: `float.toString()` uses `[u8; 32]`, `number.toString()` uses `[u8; 48]`.
- **`libynz_rt.a` path mechanism = `build.rs` + `cargo:rustc-env`**: `crates/ynz-driver/build.rs` emits `cargo:rustc-env=YNZ_RT_LIB_DIR=<target/{profile}/>` and `cargo:rustc-env=YNZ_RT_LIB_NAME=ynz_rt`.

## Architectural decisions locked at M2 planning

- **Two runtime crates: `ynz-numerics` (pure Rust, internal-use decimal128) + `ynz-runtime` (umbrella, C-ABI, `staticlib` target).** Single-crate dual-output was rejected — the shim layer is real work, not a cargo flag.
- **Decimal128 in M2 = IEEE 754 decimal128 core operations only.** Add, subtract, multiply, divide, negate, abs, compare. No sqrt, ln, exp, sin, fma, rem. No `%` on `number` (compile error pointing at v0.7 `math` module). Integer `%` and float `%` work (LLVM `srem` / `frem`).
- **Codegen calls into runtime via plain C-ABI extern fns.** `extern "C" ynz_decimal_add(*const Decimal128Bits, *const Decimal128Bits, *mut Decimal128Bits)`. `Decimal128Bits` is `[u8; 16]`.
- **Integer overflow check codegen = LLVM checked-arithmetic intrinsics.** `llvm.sadd.with.overflow.i64`, `llvm.ssub.with.overflow.i64`, `llvm.smul.with.overflow.i64`. Each op produces `{i64, i1}`; the `i1` flag branches to a runtime panic stub on true.
- **Decimal-by-zero, int-by-zero**: runtime panic. **Float-by-zero**: returns IEEE infinity per binary64 spec.
- **Number literal forms**: M2 lexer accepts decimal, hex `0x2A`, binary `0b1010`, and underscores `1_000_000`. No octal.
- **Integer division**: truncates toward zero (LLVM `sdiv`, Rust, C, Go).
- **Runtime to-string ABI = caller-owned buffer, no heap.** All formatting runtime functions take a caller-allocated buffer pointer; nothing returns an owned pointer. Sizes locked: int ≥ 24, float ≥ 32, decimal ≥ 48.

---

## Phase 1: `ynz-numerics` + `ynz-runtime` crates + decimal128 implementation
**PR scope**: Two new crates. `ynz-numerics` implements IEEE 754 decimal128 (BID encoding, u128-backed coefficient) with `add`/`sub`/`mul`/`div`/`neg`/`abs`/`compare`. `ynz-runtime` re-exports them as `#[no_mangle] extern "C"` shims and builds to `libynz_rt.a`.
**Branch**: `feat/numerics-runtime`
**Est. lines**: ~2500
**Status**: COMPLETE (2026-05-12) — commit 59fcee2, 118 tests green. Key correctness fixes: `round_half_even` using `2*r vs divisor`, alignment threshold `aligned_digits > 68`, single-signal division rounding. Big-O docs added: `U256::div_rem` O(256) binary long division, Knuth Algorithm D replacement target at v0.4.

**IEEE 754 conformance**: Hursley `.decTest` corpus, pinned SHA-256, M2 subset (`dqAdd`, `dqSubtract`, `dqMultiply`, `dqDivide`, `dqCompare`, `dqAbs`, `dqMinus`, `dqPlus`), round-half-even only. Differential test against Python `decimal` (10k random tuples per CI). Property tests for commutativity, identity, sign, round-trip.

**Acceptance criteria** (all met):
- [x] `cargo test -p ynz-numerics` passes 100% of the M2-subset IEEE 754 corpus.
- [x] Differential test runs 10k tuples against Python `decimal`.
- [x] Property tests pass.
- [x] `cargo build -p ynz-runtime --release` produces `libynz_rt.a`.
- [x] M1's `ynz run hello.ynz` still passes.
- [x] `build.rs` resolves `YNZ_RT_LIB_DIR` correctly.
- [x] Format shims accept caller-owned buffers per the locked ABI.
- [x] Corpus SHA-256 pinned.

---

## Phase 2: Lexer extension (M2 token set)
**PR scope**: Extend `ynz-parser::lex` to recognize the M2 token set: `let`, `const`, `true`, `false` keywords; integer / float / decimal literals; arithmetic / comparison / boolean / bitwise operators; `=` and `:` punctuation.
**Branch**: `feat/m2-lexer`
**Est. lines**: ~400
**Status**: COMPLETE (2026-05-12) — commit a8c3efe. 39 lex tests green. Token count locked at 42 (10 M1 + 32 M2). Key implementation: `//` comments stripped before `lex_one`; dot-method-call disambiguation (`.` only consumed as decimal point when followed by a digit so `42.toString()` works); `validate_underscores` checks each digit segment; 4 plumbing tokens (Dot, LBracket, RBracket, Comma) added ahead of schedule for P3; `spec/operators.md` updated to include `%`.

**New `Token` variants (M2)**:
- Keywords: `Let`, `Const`, `True`, `False`
- Literals: `IntLit(i64)`, `NumberLit(String)` — string preserves source bytes for lossless decimal128 decoding in parser.
- Operators: `Plus`, `Minus`, `Star`, `Slash`, `Percent`, `EqEq`, `NotEq`, `Lt`, `LtEq`, `Gt`, `GtEq`, `AmpAmp`, `PipePipe`, `Bang`, `Amp`, `Pipe`, `Caret`, `Tilde`, `LtLt`, `GtGt`
- Punctuation: `Eq` (assignment), `Colon`

**Decimal-vs-int literal classification rule**:
- A numeric literal containing `.` or `e`/`E` is a **number** literal → `NumberLit(String)`.
- A numeric literal with no `.` and no exponent is an **int** literal → `IntLit(i64)`.
- A hex / binary literal is **always int** → `IntLit(i64)`.
- `42.0` is a **number** literal, not an int. There is no `float` literal form.

**Banned operator diagnostics**: `+=`, `++`, `-=`, `--`, `*=`, `/=`, `%=` each emit a three-part teaching diagnostic.

**Acceptance criteria** (all met):
- [x] M2 source token-stream snapshot matches.
- [x] All seven malformed-literal cases produce three-part diagnostics.
- [x] All seven banned-operator cases produce teaching diagnostics.
- [x] Hex / binary / scientific / underscore-separator forms lex correctly.
- [x] `m2_token_variant_count_locked` test pins the new count.

---

## Phase 3: AST + parser extension (M2 surface)
**PR scope**: Extend `ynz-ast::nodes` with `Stmt::Let`, `Stmt::Assign`, `Expr::IntLit`/`NumberLit`/`BoolLit`/`BinOp`/`UnaryOp`/`MethodCall`. Add `Type::Int`/`Float`/`Number`/`Bool`. Implement Pratt-style precedence climbing per `spec/operators.md`.
**Branch**: `feat/m2-parser`
**Est. lines**: ~700
**Status**: COMPLETE (2026-05-12) — commit 6cee795 on main. 30 parse tests green. Key decisions: Pratt BP table encoded as `infix_bp()` (pub for spec-parity test); `is_stmt_boundary()` recovery avoids consuming `}` / keywords as atoms; `parse_method_call` handles `receiver.method(args)`; `number[N]` deferral diagnostic for N != 34 points at v0.8; `parser_precedence_table_matches_spec` test reads spec at runtime and asserts BP/level alignment.

**Precedence-climbing strategy**: Pratt-style. Each token has a left binding power; parser recursively gathers operands while next operator's left BP exceeds the current minimum. Unary operators right-associative prefix at BP 12; method-call `.` and call-parens `(...)` left-associative postfix at BP 13.

**Acceptance criteria** (all met):
- [x] M2 representative source parses to snapshot AST.
- [x] All five negative cases produce three-part diagnostics.
- [x] Precedence-climber tests cover every operator pair at spec's precedence boundary.
- [x] `parser_precedence_table_matches_spec` test passes.
- [x] Variant-count tests pin M2 counts.
- [x] M1 parser tests still pass.

---

## Phase 4: Typeck extension + primitive intrinsic table + spec corrections
**PR scope**: Extend type system with `Type::{Int, Float, Number, Bool}`. Type-check M2 expression set. Add `PrimitiveIntrinsicTable` (polymorphic `print` + 8 conversion methods).
**Branch**: `feat/m2-typeck`
**Est. lines**: ~900
**Status**: COMPLETE (2026-05-12) — 38 typeck tests green. Key decisions: `builtins.rs` git-mv'd to `intrinsics.rs` (clean blame); `BuiltinTable` → `PrimitiveIntrinsicTable`; `scope.rs` (new) with Levenshtein-distance suggestion for undefined vars; literal inference with annotation hint (`IntLit` → number/float when annotated); mixed-type-arithmetic suggestion picks `.toNumber()` / `.toFloat()` per direction, lists both for number+float tradeoff; `spec/variables.md:48` corrected (int not number); `spec/numeric-types.md:211` corrected (compile-error behavior).

**Type rules (locked)**:

| Op | Operand types | Result type | Notes |
|---|---|---|---|
| `+ - * /` | `int, int` | `int` | overflow → runtime panic |
| `+ - * /` | `float, float` | `float` | IEEE binary64 |
| `+ - * /` | `number, number` | `number` | IEEE decimal128 via runtime |
| `%` | `int, int` | `int` | truncating; LLVM `srem` |
| `%` | `float, float` | `float` | LLVM `frem` |
| `%` | `number, number` | **compile error** | pointing at v0.7 `math` module |
| `< <= > >= == !=` | `T, T` (same numeric T) | `bool` | |
| `&& ||` | `bool, bool` | `bool` | short-circuit |
| `& | ^` | `int, int` | `int` | |
| `<< >>` | `int, int` | `int` | `>>` is arithmetic |
| Any binary | mismatched types | **compile error** | three-part with `.toX()` suggestion |

**Primitive intrinsic table**:
- `print` polymorphic over `int | float | number | bool | string`.
- `int.toNumber() -> number`, `int.toFloat() -> float`, `int.toString() -> string`
- `number.toFloat() -> float`, `number.toString() -> string`
- `float.toNumber() -> number`, `float.toString() -> string`
- `bool.toString() -> string`

**Acceptance criteria** (all met):
- [x] All mismatch-matrix cells covered by negative tests.
- [x] Each diagnostic suggests the correct `.toX()` method.
- [x] Const-reassignment, undefined-variable, arity, deferral errors all produce three-part diagnostics.
- [x] `spec/variables.md` and `spec/numeric-types.md` corrections committed.
- [x] Variant-count test for `Type` pins M2 count.

---

## Phase 5: Codegen extension (LLVM ops + runtime calls + overflow + short-circuit)
**PR scope**: Extend `ynz-codegen::emit_artifact` to lower M2 typed AST. Stack-allocated locals; int overflow checks via LLVM intrinsics; decimal ops via runtime calls; short-circuit `&&`/`||` via basic-block + phi; comparison ops; bitwise ops; conversion intrinsics; div-by-zero checks.
**Branch**: `feat/m2-codegen`
**Est. lines**: ~1100
**Status**: COMPLETE (2026-05-13) — commit ed6120a on main. Key decisions and bugs found:
1. `runtime_decls.rs` — single struct holding all extern C declarations.
2. `emit.rs` fully rewritten — `let`/assign lowering with alloca+store, int overflow via `llvm.sadd/ssub/smul.with.overflow.i64`, float native ops, decimal128 via runtime, short-circuit with phi nodes, polymorphic `print` lowering per type.
3. **`expr_types` key bug fix** — changed from `usize` (span.start) to `(usize, usize)` (span.start, span.end): a BinOp's span.start equals its leftmost child's span.start, so parent's type was overwriting child's in the HashMap — caused "unsupported binop Gt Bool" for `count > 0` inside `active && (count > 0)`.
4. `ValueKind::basic()` not `.left()` in inkwell 0.9.
5. `ynz_numerics::parse(s) -> Option<u128>` not `Decimal128::parse()`.

**Lowering rules (locked)**:

| Yinz construct | LLVM IR |
|---|---|
| `let x = expr` | `%x = alloca <ty>; store <ty> <expr-value>, ptr %x` |
| Read `x` | `%tmp = load <ty>, ptr %x` |
| `int + int` | `llvm.sadd.with.overflow.i64` + branch to panic on overflow |
| `int / int` | div-by-zero check + `sdiv i64` |
| `number + number` | `call void @ynz_decimal_add(ptr %a, ptr %b, ptr %out)` |
| `a && b` | basic-block branching + phi at merge |
| `int.toFloat()` | `sitofp i64 %x to double` |
| `int.toString()` | `%buf = alloca [24 x i8]; %n = call i64 @ynz_int_to_string(i64 %x, ptr %buf, i64 24)` |
| `print(<primitive>)` | the right `.toString` first, then `call i32 @puts(ptr %s)` |

**Memory model for M2 decimal locals**: `alloca` of `[16 x i8]` for each `number` local. No heap allocation in M2.

**Acceptance criteria** (all met):
- [x] `m2_smoke.ynz` compiles, links, runs, prints `0.3\n1763\ntrue\n`.
- [x] SHA-256 golden for `m2_smoke` committed per target triple.
- [x] Reproducibility test passes.
- [x] Int-overflow runtime test exits non-zero with three-part panic.
- [x] Int- and number-div-by-zero runtime tests exit non-zero.
- [x] Float-div-by-zero produces `inf`.
- [x] `let x = 0.1 + 0.2; print(x)` outputs exactly `0.3\n` (load-bearing decimal exactness test).

---

## Phase 6: Driver integration + polymorphic-print integration + M2 fixture suite
**PR scope**: Wire P1–P5 through the driver. Add M2 integration tests covering full surface: smoke fixture, mixed-type errors, overflow panics, const-reassignment, deferral errors, banned-syntax fixtures.
**Branch**: `feat/m2-driver`
**Est. lines**: ~500
**Status**: COMPLETE (2026-05-13) — commit f089c2e on main. 8 M2 integration tests. ABI fix: format shims are `(value)->ptr`.

**Fixtures shipped**:
- `m2_smoke.ynz` — headline integration test
- `m2_mixed_int_number.ynz`, `m2_mixed_int_float.ynz`, `m2_mixed_number_float.ynz` — mixed-type errors
- `m2_const_reassign.ynz`
- `m2_int_overflow.ynz`, `m2_int_div_by_zero.ynz`, `m2_number_div_by_zero.ynz` — runtime panics
- `m2_float_div_by_zero.ynz` — produces `inf`
- `m2_bignum_deferral.ynz` — deferral error
- `m2_compound_assign.ynz`, `m2_ternary_attempt.ynz` — teaching diagnostics
- `m2_decimal_exactness.ynz` — the `0.1 + 0.2 == 0.3` headline

**Acceptance criteria** (all met):
- [x] Every M2 fixture produces expected stdout / stderr.
- [x] M2_smoke and m2_decimal_exactness exit 0 with expected stdout.
- [x] Every negative fixture exits non-zero with expected stderr.
- [x] M1's `hello.ynz` integration test still passes.

---

## Phase 7: M2 verification sweep + tag `v0.1.0-m2`
**PR scope**: No new features. TODO sweep. Comment rules sweep. M2 explicit-non-goals audit. Catch-up list audit. CHANGELOG entry. Tag.
**Branch**: `chore/m2-verification`
**Est. lines**: ~80
**Status**: COMPLETE (2026-05-13) — commit c39fe8a on main, tagged `v0.1.0-m2`. TODO sweep clean. Comment rules sweep: 148 section banners removed from 19 files (Hard Rule 6), changelog-style enum doc history stripped, grouping labels removed from lex test. Spec corrections verified. CHANGELOG.md written. All variant count gates confirmed. `ynz run m2_smoke.ynz` → `0.3\n1763\ntrue\n`.

**Acceptance criteria** (all met):
- [x] TODO sweep returns zero matches.
- [x] Comment rules sweep: zero section banners, no "what" comments, all `// SAFETY:` blocks intact.
- [x] Catch-up list audit passes.
- [x] M2 "explicitly NOT" list audited; no slips.
- [x] Spec corrections verified.
- [x] CHANGELOG entry committed.
- [x] Git tag `v0.1.0-m2` created.

---

## M2 Quality Checklist (all met)

M2 inherited every M1 quality-checklist item. Additional M2-specific items, all met:

- [x] `ynz-numerics` passes the M2-subset IEEE 754 decimal128 conformance corpus (Hursley `.decTest`, pinned SHA-256 in `CORPUS.sha256`)
- [x] `ynz-numerics` differential test against Python `decimal` passes 10k random tuples on CI
- [x] `ynz-numerics` property tests (commutativity, identity, sign, round-trip) all pass
- [x] `libynz_rt.a` builds clean on Linux and macOS; symbol export verified via `nm`
- [x] Generated binaries link `libynz_rt.a` and resolve all `ynz_*` extern symbols
- [x] M2 smoke fixture runs end-to-end and produces the expected stdout
- [x] M2 decimal-exactness fixture (`0.1 + 0.2 → 0.3` exact) passes
- [x] M2 integer-overflow runtime test produces a three-part panic on stderr and exits non-zero
- [x] M2 div-by-zero runtime tests (int + number) panic; float div-by-zero produces `inf` and exits 0
- [x] Mixed-type arithmetic compile errors point at the correct `.toX()` method per the type-rule matrix
- [x] `number[N]` for N != 34 produces the M8-deferral compile error
- [x] Spec corrections committed (`spec/variables.md`, `spec/numeric-types.md`, `spec/operators.md`)
- [x] M2-extended variant-count tests pin their new counts with `// test-ratchet:` markers
- [x] Object-file SHA-256 reproducibility contract still holds
