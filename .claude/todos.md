# Todos: ynz

Global cross-workstream items only. Granular per-chat work lives in:
- `.claude/plans/active/{slug}.md` for planned work
- `.claude/work/{slug}/todos.md` for unplanned chat-scoped work

---

## Now (active cross-workstream items)

*(none — M1 shipped, M2 not started)*

## Now (active)

- [ ] **M2 Phase 1 PR** — Branch `feat/numerics-runtime` pushed (59fcee2). `gh auth login` needed to create draft PR via CLI. Manual PR URL: `https://github.com/patrickrizzardi/ynz/pull/new/feat/numerics-runtime`. Merge before starting Phase 2.
- [ ] **M2 Phase 2 — Lexer extension (`feat/m2-lexer`)** — Start in new chat (current chat near context limit). Extend lexer with M2 token set: `let`/`const`/`true`/`false` keywords; int/float/number literals (decimal, hex, binary, scientific, underscore separators); arithmetic/comparison/boolean operators; `=` and `:` punctuation. No `FloatLit` token. Add `%` to spec/operators.md. Variant-count test bumped with `// test-ratchet:` marker.

## Soon (committed, not started)

- [ ] **M2 Phases 3–7** — After Phase 2 merges: parser extension (Pratt precedence), typeck (PrimitiveIntrinsicTable replaces BuiltinTable), codegen (LLVM ops + runtime calls), driver integration + fixtures, verification sweep + tag `v0.1.0-m2`.

## Later (idea bin — not committed)

- [ ] Wire up GitHub Actions CI once repo is pushed to GitHub (ci.yml already written, just needs a remote)
- [ ] macOS CI golden hash for ynz-codegen (currently only `hello.x86_64-linux.sha256` committed)

## Done (recent)

- [x] **M1 compiler end-to-end (2026-05-12)** — `ynz run hello.ynz` → `hello, yinz`. 51 tests green. Committed to main (820bfdc). Full pipeline: lex → parse → typecheck → LLVM codegen → link → execute, all wired as salsa queries.
- [x] **Compiler error-message audit (2026-05-12)** — wrote `design/compiler-errors.md`, swept all spec files for jargon, banned-jargon CI gate wired into `ynz-diagnostics` test suite.
