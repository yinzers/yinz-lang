# Todos: ynz

Global cross-workstream items only. Granular per-chat work lives in:
- `.claude/plans/active/{slug}.md` for planned work

---

## Now (active)

*(v0.2-M1 execution in progress — Phase 0 branch `chore/v0-2-m1-doc-lockdown`. See `.claude/plans/active/v0-2-m1-feature-inventory-sync.md` for the plan.)*

- [ ] **m8-typeck-cross-file-resolution still in active/** — audit as of v0.2-M1 Phase 0 (2026-05-19): plan is `status: active` / `pending_approval` / `0/32 done`. Roadmap: v0-1-compiler. This is real unfinished work (cross-file import/export typeck — `Item::ImportDecl(_) => {}` currently silently ignored). Action needed: either (a) approve the plan and start execution in a dedicated chat, OR (b) move plan to `paused/` if v0.2-M1 is higher priority. Does NOT appear to be a ghost resurrected by git-mv — it's genuinely incomplete M8 work. Investigate before v0.2-M2 LSP work begins (cross-file resolution is a precondition for LSP "go-to-definition" working across files).

## Soon (committed, not started)

- [ ] **Hidden-field default evaluation in struct literals** — `emit.rs:lower_struct_lit` zero-inits hidden fields; string/shape hidden fields with non-zero defaults are silently wrong. Revisit when v0.2 LSP work begins.
- [ ] **Dynamic dispatch call-site coercion** — `coerce_to_dynamic` infrastructure is in place (vtable globals emitted) but passing a concrete shape to a `dynamic Foo` parameter is not yet wired. Defer to post-M5.
- [ ] **UFCS const-lend check** — `check.rs` comment (line ~936): receiver ownership not checked for dot-call UFCS; only free-function-call form is checked. Low priority — the function-call form produces the correct error.

## Later (idea bin — not committed)

- [ ] **Jargon-CI sweep** — add a CI script that greps `design/*.md`, `spec/*.md`, all `.claude/rules/*.md`, and crate source files for banned-jargon words. Scope: extend `crates/ynz-diagnostics/src/banned_jargon.rs` AND add a doc-grep CI step.
- [ ] Wire up GitHub Actions CI (ci.yml already written, just needs configuration)
- [ ] macOS CI golden hash for ynz-codegen

## Done (recent)

- [x] **M6 complete (tag pending v0.1.0-m6, 631 tests)** — options+unions+narrowing: options types (i8 tags, multi-case, toString), union types (tagged-struct, Is-arm narrowing), fallible conversions (.toInt/.toFloat/.toNumber), early-return narrowing, shape aliases (shape S = A|B), string parsing runtime. Plan moved to done/. M2+M3 catch-up items closed.
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
