# Todos: ynz

Global cross-workstream items only. Granular per-chat work lives in:
- `.claude/plans/active/{slug}.md` for planned work

---

## Now (active)

- [ ] **M4 P2 — Parser** — Branch `feat/m4-parser` pushed (84db1d2). PR at https://github.com/patrickrizzardi/ynz/pull/new/feat/m4-parser — Patrick opens + merges. P3a blocked until merged.

## Soon (committed, not started)

- [x] **M4 P2 — Parser** — DONE (84db1d2, branch feat/m4-parser). ShapeDecl AST, FieldAccess, StructLit, PostfixOp, SelfValue, FieldAssign, Dynamic/SelfType types, ownership modifiers parse, 68 parse tests green.
- [ ] **M4 P3a — Typeck shapes** — After P2. Shape table salsa query, struct literals, field access/assign, method resolution, hidden fields.
- [ ] **M4 P3b — Inheritance + follows + dynamic** — After P3a.
- [ ] **M4 P3c — Ownership analysis** — After P3b. Borrow-check salsa query, const deep-immutability, use-after-give.
- [ ] **M4 P4 — Codegen** — After P3c. LLVM `readonly`/`noalias` attrs, heap alloc/drop, vtable for `dynamic`.
- [ ] **M4 P5 — Catch-up** — Wrapping/saturating int methods + `int.max`/`int.min`/`number.epsilon`. Can start after P3a.
- [ ] **M4 P6 — Driver + fixtures** — After P4. Full fixture suite (12 positive + 20 negative).
- [ ] **M4 P7 — Verification + tag `v0.1.0-m4`** — After P6.

## Later (idea bin — not committed)

- [ ] **`<>` generics syntax — compiler** — When M5 (generics) is implemented, the compiler must use `<>` not `[]` for type parameters. Parser, AST, typeck, and codegen must all follow `array<T>`, `map<K, V>`, `fixed<T>` syntax.
- [ ] **Jargon-CI sweep** — add a CI script that greps `design/*.md`, `spec/*.md`, all `.claude/rules/*.md`, and crate source files for banned-jargon words (`monomorphization`, `vtable`, `devirtualization`, `dyn`, `infer`/`inference` outside the dual-audience inference.md, etc.) AND for the same words in user-facing diagnostics in `crates/ynz-diagnostics/`. Allow the words inside explicit "Internal terminology note" blocks but fail elsewhere. Per Patrick: "we can add a todo for chekcing ALL erorrs and coments and hints are non jargon but not do it now." Scope: extend `crates/ynz-diagnostics/src/banned_jargon.rs` AND add a doc-grep CI step.
- [ ] Wire up GitHub Actions CI (ci.yml already written, just needs configuration)
- [ ] macOS CI golden hash for ynz-codegen

## Done (recent)

- [x] **M3 — control flow + user functions (9653dbd, tag v0.1.0-m3)** — `if`, `while`, `for`, multi-case `if`, user functions with params/return, two-pass typeck, return-path analysis, full LLVM lowering. 310 tests. `fib(10) = 55`.
- [x] **Design-lockdown (PRs #5 + #14)** — `shape` keyword locked, 3 rule files (inference.md, plan-invariants.md, vocabulary.md), 5 graveyard entries, Golden Rules updated.
- [x] **M2 Phase 7 — verification sweep + tag (c39fe8a, v0.1.0-m2)** — TODO clean, 148 banners removed, CHANGELOG written.
- [x] **M2 Phases 1–6** — numerics, lexer, parser, typeck, codegen, driver. All on main.
- [x] **M1 compiler end-to-end** — `ynz run hello.ynz` → `hello, yinz`.
