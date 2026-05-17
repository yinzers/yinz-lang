# Todos: ynz

Global cross-workstream items only. Granular per-chat work lives in:
- `.claude/plans/active/{slug}.md` for planned work

---

## Now (active)

- [ ] **M4 P6 — Driver + fixtures** — Full fixture suite (12 positive + 20 negative M4 fixtures), valgrind clean, IR snapshots. Branch `feat/m4-driver-fixtures`.
- [ ] **M4 P7 — Verification + tag `v0.1.0-m4`** — TODO sweep, jargon audit, Bouncer clean, CHANGELOG, tag. After P6.

## Soon (committed, not started)

_(nothing — P6 and P7 are the remaining M4 work)_

## Later (idea bin — not committed)

- [ ] **`<>` generics syntax — compiler** — When M5 (generics) is implemented, the compiler must use `<>` not `[]` for type parameters. Parser, AST, typeck, and codegen must all follow `array<T>`, `map<K, V>`, `fixed<T>` syntax.
- [ ] **Jargon-CI sweep** — add a CI script that greps `design/*.md`, `spec/*.md`, all `.claude/rules/*.md`, and crate source files for banned-jargon words. Scope: extend `crates/ynz-diagnostics/src/banned_jargon.rs` AND add a doc-grep CI step.
- [ ] Wire up GitHub Actions CI (ci.yml already written, just needs configuration)
- [ ] macOS CI golden hash for ynz-codegen

## Done (recent)

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
