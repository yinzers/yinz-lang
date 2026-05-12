# Todos: ynz

Global cross-workstream items only. Granular per-chat work lives in:
- `.claude/plans/active/{slug}.md` for planned work

---

## Now (active)

- [ ] **Comment cleanup sweep** — Strip section banners (`// ── X ──` style, Hard Rule 6) from Phase 2–4 files that are already on main: `lexer.rs`, `parser.rs`, `nodes.rs`, `token.rs`. Also `check.rs` on current branch. Remove "what" comments (e.g. `// ── M1 expressions ──` inside match arms). Pending user approval to start.

- [ ] **Phase 4 review + commit** — User reviewing Phase 4 diff on `feat/m2-typeck`. Once approved: commit, merge to main, branch `feat/m2-codegen` for Phase 5.

## Soon (committed, not started)

- [ ] **M2 Phases 5–7** — After Phase 4 merges: codegen (LLVM ops + runtime calls), driver integration + fixtures, verification sweep + tag `v0.1.0-m2`.

## Later (idea bin — not committed)

- [ ] Wire up GitHub Actions CI once repo is pushed to GitHub (ci.yml already written, just needs a remote)
- [ ] macOS CI golden hash for ynz-codegen (currently only `hello.x86_64-linux.sha256` committed)

## Done (recent)

- [x] **M2 Phase 4 — Typeck extension** (feat/m2-typeck, awaiting commit) — PrimitiveIntrinsicTable, scope.rs, full M2 type rules, spec corrections. Bouncer caught + fixed test weakening in `m1_source_type_checks_clean`.
- [x] **M2 Phase 3 — AST + parser (6cee795)** — Pratt climber, Stmt::Let/Assign, all M2 Expr/Type variants, spec-parity test. 30 parse tests.
- [x] **M2 Phase 2 — Lexer extension (a8c3efe)** — 42 tokens, banned-op diagnostics, malformed-literal recovery. 39 lex tests.
- [x] **M2 Phase 1 — ynz-numerics + ynz-runtime (59fcee2)** — IEEE 754 decimal128 from scratch, libynz_rt.a, 118 tests.
- [x] **M1 compiler end-to-end (2026-05-12)** — `ynz run hello.ynz` → `hello, yinz`. 51 tests. Committed to main (820bfdc).
- [x] **Compiler error-message audit (2026-05-12)** — wrote `design/compiler-errors.md`, swept all spec files for jargon, banned-jargon CI gate.
