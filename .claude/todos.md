# Todos: ynz

Global cross-workstream items only. Granular per-chat work lives in:
- `.claude/plans/active/{slug}.md` for planned work

---

## Now (active)

- [ ] **M6 — Options + Unions + Narrowing** — Approved 2026-05-18 (r2). Plan in `.claude/plans/active/m6-options-unions.md`. P0 doc lockdown in progress. Phases: P0 (docs) → P1 (lexer) → P2 (parser) → P3a (options typeck) → P3b (union+narrowing typeck) → P4 (codegen) → P5 (catch-up fixtures) → P6 (demo+tag).

**M6 catch-up obligations (active — must close by M6 P5):**
- [ ] `.toInt()` on `int` → identity (M2 plan §62)
- [ ] `.toInt()` on `float` → `maybe<int>` with NaN+OOR rules (M2 plan §62; locked codegen in P4)
- [ ] `.toInt()` on `number` (decimal128) → `maybe<int>` (M2 plan §62)
- [ ] `string.toInt()` / `.toNumber()` / `.toFloat()` → `maybe<T>` (M2 plan §62; locked parsing rule in P4)
- [ ] `is Type =>` in multi-case `if` — close M3 deferral fixture `m3_is_type_deferral.ynz` (M3 plan, REPLACE-AT M6 marker)
- [ ] Early-return narrowing for `.value` on `maybe<T>` (M5 `design/maybe.md` deferral)
- [ ] `||` propagation rule for narrowing (M5 deferred)

## Soon (committed, not started)

- [ ] **Hidden-field default evaluation in struct literals** — `emit.rs:lower_struct_lit` zero-inits hidden fields; string/shape hidden fields with non-zero defaults are silently wrong. Revisit when v0.2 LSP work begins.
- [ ] **Dynamic dispatch call-site coercion** — `coerce_to_dynamic` infrastructure is in place (vtable globals emitted) but passing a concrete shape to a `dynamic Foo` parameter is not yet wired. Defer to post-M5.
- [ ] **UFCS const-lend check** — `check.rs` comment (line ~936): receiver ownership not checked for dot-call UFCS; only free-function-call form is checked. Low priority — the function-call form produces the correct error.

## Later (idea bin — not committed)

- [ ] **Jargon-CI sweep** — add a CI script that greps `design/*.md`, `spec/*.md`, all `.claude/rules/*.md`, and crate source files for banned-jargon words. Scope: extend `crates/ynz-diagnostics/src/banned_jargon.rs` AND add a doc-grep CI step.
- [ ] Wire up GitHub Actions CI (ci.yml already written, just needs configuration)
- [ ] macOS CI golden hash for ynz-codegen

## Done (recent)

- [x] **M5 complete (tag v0.1.0-m5, 574 tests)** — Generics `<T>`, `fixed<T>`, `array<T>`, `map<K,V>`, `maybe<T>`, `.exists()`/`.value`/`.or()`, bracket sugar, SipHash-2-4, Swiss Tables, monomorphization, M4 catch-up (wrapping/saturating, type-attached constants). Plan moved to `done/m5-generics.md`.
- [x] **M4 complete (tag v0.1.0-m4, 316 tests)** — P1 lexer, P2 parser, P3a/b/c typeck, P4 codegen, P5 catch-up, P6 fixtures, P7 verification. Plan moved to `done/m4-shapes-functions-ownership.md`.
- [x] **M4 P5 — Catch-up (5a21258)** — 6 wrapping/saturating int methods, type-attached constants (`int.max`/`int.min`/`number.*`/`float.*`). M2 fixtures closed.
- [x] **M4 P4 — Codegen (05bb47d)** — Shape LLVM struct types, UFCS dispatch, `readonly`/`noalias` attrs, `ynz_alloc`/`ynz_free`, vtable globals. `m4_player.ynz` → `Patrick / 120 / Patrick`.
- [x] **M4 P3c — Ownership (7c86f6a)** — `is_consumed` scope tracking, use-after-give error, const-cannot-be-lent, const-cannot-be-given. 102 typeck tests green.
- [x] **M4 P3b — Inheritance (3508e7b)** — `extends` field inheritance + cycle detection, `follows` contract verification, `Type::Dynamic`. 96 tests green.
- [x] **M4 P3a — Typeck shapes (244ac6d)** — ShapeTable, struct-lit typeck, field access/assign, UFCS, hidden-field guard, base-shape guard. 90 tests green.
- [x] **M4 P2 — Parser (84db1d2)** — ShapeDecl AST, FieldAccess, StructLit, PostfixOp, SelfValue, FieldAssign, Dynamic/SelfType, ownership modifiers parse. 68 parse tests green.
- [x] **M4 P1 — Lexer (05c5296)** — 8 new tokens (49→57), 6 banned-keyword handlers. All tests green.
- [x] **M3 — control flow + user functions (9653dbd, tag v0.1.0-m3)** — `if`, `while`, `for`, multi-case `if`, user functions, return-path analysis. 310 tests. `fib(10) = 55`.
- [x] **Design-lockdown (PRs #5 + #14)** — `shape` keyword locked, 3 rule files, 5 graveyard entries, Golden Rules updated.
- [x] **M2 complete (c39fe8a, tag v0.1.0-m2)** — Numerics, variables, arithmetic. 118+ tests.
- [x] **M1 compiler end-to-end** — `ynz run hello.ynz` → `hello, yinz`.
