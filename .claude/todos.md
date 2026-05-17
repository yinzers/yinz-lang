# Todos: ynz

Global cross-workstream items only. Granular per-chat work lives in:
- `.claude/plans/active/{slug}.md` for planned work

---

## Now (active)

- [ ] **M4 P3c — Ownership** — Branch `feat/m4-ownership` pushed (7c86f6a). PR at https://github.com/patrickrizzardi/ynz/pull/new/feat/m4-ownership — Patrick merges. P4 (codegen) next.

## Soon (committed, not started)

- [ ] **M4 P4 — Codegen** — After P3c merges. LLVM `readonly`/`noalias` attrs on params, heap alloc (`ynz_alloc`/`ynz_free`), drop-on-scope-exit, vtable for `dynamic`. Needs LLVM 18 dev in container.
- [ ] **M4 P5 — Catch-up** — Wrapping/saturating int methods + `int.max`/`int.min`/`number.epsilon`. Independent of P4, can start after P3c.
- [ ] **M4 P6 — Driver + fixtures** — After P4. Full fixture suite (12 positive + 20 negative).
- [ ] **M4 P7 — Verification + tag `v0.1.0-m4`** — After P6.

## Later (idea bin — not committed)

- [ ] **`<>` generics syntax — compiler** — When M5 (generics) is implemented, the compiler must use `<>` not `[]` for type parameters. Parser, AST, typeck, and codegen must all follow `array<T>`, `map<K, V>`, `fixed<T>` syntax.
- [ ] **Jargon-CI sweep** — add a CI script that greps `design/*.md`, `spec/*.md`, all `.claude/rules/*.md`, and crate source files for banned-jargon words. Scope: extend `crates/ynz-diagnostics/src/banned_jargon.rs` AND add a doc-grep CI step.
- [ ] Wire up GitHub Actions CI (ci.yml already written, just needs configuration)
- [ ] macOS CI golden hash for ynz-codegen

## Done (recent)

- [x] **M4 P3c — Ownership (7c86f6a)** — `is_consumed` scope tracking, use-after-give error, const-cannot-be-lent, const-cannot-be-given. 102 typeck tests green.
- [x] **M4 P3b — Inheritance (3508e7b)** — `extends` field inheritance + cycle detection, `follows` contract verification, `Type::Dynamic`. 96 tests green.
- [x] **M4 P3a — Typeck shapes (244ac6d)** — ShapeTable, struct-lit typeck, field access/assign, UFCS, hidden-field guard, base-shape guard. 90 tests green.
- [x] **M4 P2 — Parser (84db1d2)** — ShapeDecl AST, FieldAccess, StructLit, PostfixOp, SelfValue, FieldAssign, Dynamic/SelfType, ownership modifiers parse. 68 parse tests green.
- [x] **M4 P1 — Lexer (05c5296)** — 8 new tokens (49→57), 6 banned-keyword handlers. All tests green.
- [x] **M3 — control flow + user functions (9653dbd, tag v0.1.0-m3)** — `if`, `while`, `for`, multi-case `if`, user functions, return-path analysis. 310 tests. `fib(10) = 55`.
- [x] **Design-lockdown (PRs #5 + #14)** — `shape` keyword locked, 3 rule files, 5 graveyard entries, Golden Rules updated.
- [x] **M2 complete (c39fe8a, tag v0.1.0-m2)** — Numerics, variables, arithmetic. 118+ tests.
- [x] **M1 compiler end-to-end** — `ynz run hello.ynz` → `hello, yinz`.
