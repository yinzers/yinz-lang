# Todos: ynz

Global cross-workstream items only. Granular per-chat work lives in:
- `.claude/plans/active/{slug}.md` for planned work

---

## Now (active)

- [ ] **M3 plan (Opus)** — Run `/plan` for M3 (control flow + user functions). User-defined function registry is new infrastructure at every layer; LLVM basic-block branching for `if`/`else`/`while`; `range` temporary builtin design decision; early `return` with unreachable tracking. Multi-session, plan with Opus first.

## Soon (committed, not started)

- [ ] **M3 implementation** — After plan approval: `if`/`else`, `while`, `for x in range(...)`, early `return`, user-defined functions with parameters and return types, block scoping. Depends on M3 plan.

## Later (idea bin — not committed)

- [ ] **`<>` generics syntax — compiler** — When M5 (generics) is implemented, the compiler must use `<>` not `[]` for type parameters. Docs were updated (2026-05-13). Parser, AST, typeck, and codegen must all follow `array<T>`, `map<K, V>`, `fixed<T>`, `function foo<T>()` syntax.
- [ ] Wire up GitHub Actions CI once repo is pushed to GitHub (ci.yml already written, just needs a remote)
- [ ] macOS CI golden hash for ynz-codegen

## Done (recent)

- [x] **M2 Phase 7 — verification sweep + tag (c39fe8a, v0.1.0-m2)** — TODO clean, 148 banners removed, CHANGELOG written.
- [x] **M2 Phase 6 — driver integration (f089c2e)** — 8 integration tests, ABI fix.
- [x] **M2 Phase 5 — LLVM codegen (ed6120a)** — Full M2 lowering, expr_types span-key bug fixed.
- [x] **M2 Phases 1–4 — numerics, lexer, parser, typeck** — All on main.
- [x] **M1 compiler end-to-end** — `ynz run hello.ynz` → `hello, yinz`.
