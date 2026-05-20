# Todos: ynz

Global cross-workstream items only. Granular per-chat work lives in:
- `.claude/plans/active/{slug}.md` for planned work

---

## Now (active)

*(v0.2-M1 SHIPPED — all 8 phases merged, Cargo.toml bumped to 0.2.0-m1, tag cut pending /release. v0.2-M2 (LSP) is next. Plan moved to done/.)*

- [ ] **m8-typeck-cross-file-resolution still in active/** — audit as of v0.2-M1 Phase 0 (2026-05-19): plan is `status: active` / `pending_approval` / `0/32 done`. Roadmap: v0-1-compiler. This is real unfinished work (cross-file import/export typeck — `Item::ImportDecl(_) => {}` currently silently ignored). Action needed: either (a) approve the plan and start execution in a dedicated chat, OR (b) move plan to `paused/` if v0.2-M1 is higher priority. Does NOT appear to be a ghost resurrected by git-mv — it's genuinely incomplete M8 work. Investigate before v0.2-M2 LSP work begins (cross-file resolution is a precondition for LSP "go-to-definition" working across files).

## Soon (committed, not started)

- [ ] **Hidden-field default evaluation in struct literals** — `emit.rs:lower_struct_lit` zero-inits hidden fields; string/shape hidden fields with non-zero defaults are silently wrong. Revisit when v0.2 LSP work begins.
- [ ] **Dynamic dispatch call-site coercion** — `coerce_to_dynamic` infrastructure is in place (vtable globals emitted) but passing a concrete shape to a `dynamic Foo` parameter is not yet wired. Defer to post-M5.
- [ ] **UFCS const-lend check** — `check.rs` comment (line ~936): receiver ownership not checked for dot-call UFCS; only free-function-call form is checked. Low priority — the function-call form produces the correct error.

- [ ] **watch-lru-runtime-tuning** — `YNZ_WATCH_LRU_SCALE`, `YNZ_WATCH_LRU_PARSE`, `YNZ_WATCH_LRU_SIG`, `YNZ_WATCH_LRU_CHECK`, `YNZ_WATCH_LRU_CODEGEN` env vars are documented in `design/watch.md` and locked in the M4 plan but NOT yet wired to `set_lru_capacity` at watch boot. **Why deferred**: salsa 0.26 `set_lru_capacity` requires `&mut db` access; threading that through the `WatchDb` API at runtime adds 15-20 lines but shifts the wire-up to Phase 6+. **Cost to fix**: ~20 lines in `crates/ynz-watch/src/db.rs::WatchDb::apply_lru_env_overrides()` + call at boot; env-var parsing already exists in `memory.rs::read_mb_env`. **Trigger**: a user reports salsa cache OOM that can't be solved by `YNZ_WATCH_REBUILD_AFTER` tuning alone.

## Later (idea bin — not committed)

- [ ] **fmt-inter-arm-comments** — comments placed BETWEEN two match arms (`pattern1 => stmt\n// comment\npattern2 => stmt`) are silently dropped by the formatter. The `trailing-comment-after-last-arm` fix landed in Phase 4 captures AFTER the last arm; BETWEEN arms is a pre-existing bug (confirmed pre-fix on `main`). `When triggered:` fix when implementing Phase 3.5 or Phase 4 follow-up on comment attachment; add a `comment_between_arms.ynz` fixture to scope the bug explicitly. See `crates/ynz-fmt/src/walker.rs` `Stmt::Match` emit code for the fix location.

- [ ] **parser-infinite-loop-on-error-gallery-fixtures** — `v0_2_m1_errors.ynz` and `m1_errors.ynz` cause the parser to hang (infinite loop on error recovery). Pre-existing bug, predates the formatter. The `idempotency.rs` and `semantic_roundtrip.rs` and `mass_rewrite.rs` tests explicitly skip these two files. `When triggered:` fix when the parser's error-recovery loop is audited for termination guarantees (likely as part of the v0.3 parser hardening pass).

- [ ] **lsp-vs-cli-exact-divergence** — `regression_lsp_vs_cli_divergence` currently asserts boolean error-presence agreement (CLI exits non-zero ↔ LSP publishes ≥1 diagnostic). Exact diagnostic count matching requires `ynz build --json` structured output (ariadne pretty-print is unreliable to count via regex). Add a `--json` output mode to `ynz-driver` then tighten this test to count-level assertions. Deferred from v0.2-M2 Phase 8.

- [ ] **lsp-completion-typeck-receiver-narrowing** — wire `module_signatures_query` into the LSP completion handler to narrow after-dot completions by the receiver's actual type (e.g. `score.` shows only `int` methods when `score: int`). Requires `type_of_expression_at_offset(db, source, byte_offset) -> Option<Type>` helper in ynz-typeck (not yet exposed). Deferred from v0.2-M2 Phase 4; the registry adapter correctly filters by receiver type when given `Some("int")` — the LSP server always passes `None` until this is wired. Trigger: when ynz-typeck exposes a per-offset type query.

- [ ] **vscode-extension-ci-workflow** — GitHub Actions to build + publish `tooling/vscode-ynz/` on release tags (currently manual). Deferred from v0.2-M2 Phase 7; M2 ships extension via local cargo+npm or marketplace publish, no CI yet. Pick up whenever marketplace publishing automation is wanted OR when a non-Patrick contributor needs to repro the build.

- [ ] **marketplace-publish-followup** — register VSCode publisher `yinz-lang` and run `vsce publish --pre-release`. Objectively-triggered fallback fired during v0.2-M2 Phase 7 per trigger #3: "Marketplace requires account setup Patrick can't single-handedly resolve in one session" — Azure DevOps org provisioning page non-functional; PAT could not be generated; publisher account could not be created. Extension shipped as .vsix at https://github.com/yinzers/yinz-lang/releases/tag/ynz-vscode-v0.2.0-m2 instead.

- [ ] **vscode-extension-screenshots** — take 3 screenshots of the installed Yinz extension: hover.png (hover over a keyword), autocomplete.png (completion popup after `int.`), diagnostic.png (red squiggle on an error). Commit to `tooling/vscode-ynz/screenshots/` and update the README screenshots section. Deferred from v0.2-M2 Phase 7 because publisher registration was blocked (no working extension to screenshot against a marketplace listing).

- [ ] **vscode-extension-visual-polish** — a potential post-Phase 9 pass on extension UX. Known items: (1) diagnostic message `\n\n` shows as raw text in the Problems panel — need a better separator strategy for non-markdown surfaces; (2) shape field completion after `.` requires `type_of_expression_at_offset` in typeck (already in lsp-completion-typeck-receiver-narrowing entry above); (3) screenshots once marketplace or .vsix is verified working. Other candidates TBD — leave scope open for discussion when we get there.

- [ ] **Jargon-CI sweep** — add a CI script that greps `design/*.md`, `spec/*.md`, all `.claude/rules/*.md`, and crate source files for banned-jargon words. Scope: extend `crates/ynz-diagnostics/src/banned_jargon.rs` AND add a doc-grep CI step.
- [ ] Wire up GitHub Actions CI (ci.yml already written, just needs configuration)
- [ ] macOS CI golden hash for ynz-codegen

- [ ] **lsp-range-formatting** — add `format_range(source, range)` to ynz-fmt library + textDocument/rangeFormatting LSP handler. Deferred from v0.2-M3 (whole-file formatting was enough for editor format-on-save). Pick up IF v0.2-M5 LSP proves a need.

- [ ] **fmt-inter-element-comments** — implement element-level comment attachment in `emit_expr` for `ArrayLit`/`MapLit`/`StructLit` when long-line split is triggered. Locked spec: `[1, // note\n 2, 3]` → comment moves to own line ABOVE element 2 at element indent. Deferred from v0.2-M3 Phase 3 because it requires making `emit_expr` comment-aware (significant scope); Phase 3's `comment_in_array.ynz` tests the leading-comment-before-stmt case instead. Implement when taking up Phase 3.5 or Phase 4.

- [ ] **fmt-diff-mode** — add `ynz fmt --diff` flag emitting unified diff of what would change. Deferred from v0.2-M3 (not blocking ship; useful for code review tooling). No specific trigger; nice-to-have.

- [ ] **update-plan-invariants-entrypoint-path** — update `.claude/rules/plan-invariants.md` to point at `examples/pirates-roster/entrypoint.ynz` (NOT `src/entrypoint.ynz` which is stale; actual path verified 2026-05-20). Trivial doc edit; do whenever passing through the rule file.

- [ ] **per-phase-rule-reminder-block-in-code-reviewer-prompts** — extend each phase's Exit Sequence code-reviewer prompt to explicitly remind the agent about `~/.claude/rules/comments.md` + Golden Rule 11 WHY-quality + Yinz vocabulary (per agent-dispatch-rule-reminders memory). Deferred from v0.2-M3 round 1 review; non-blocking but tracked.

- [ ] **watch-interactive-commands** — press 'r' to rebuild, 'q' to quit, etc. in `ynz watch`. Deferred from v0.2-M4; not blocking ship. Pick up IF terminal-only users surface real demand. Locked design pointer in `design/watch.md` future-proofing section.

- [ ] **watch-lsp-shared-daemon** — investigate sharing the long-lived `CompilerDb` between `ynz-watch` and `ynz-lsp`. Deferred from v0.2-M4 (independent daemons OK for M4). Pick up IF v0.3 needs both running concurrently against the same project.

- [ ] **watch-windows-validation** — full Windows validation pass: RSS via `memory-stats`, child kill via TerminateProcess, process group via `CREATE_NEW_PROCESS_GROUP`. Implementation present from M4 but tested manually only. Pick up when Yinz formally supports Windows.

- [ ] **watch-json-schema-stabilize** — at v0.2.0 final tag, drop `-unstable` suffix from `--json` `schema_version` field; commit to semver-bound schema changes. Locked trigger: v0.2.0 release.

- [ ] **lsp-salsa-cancellation** — salsa 0.26 queries are not cancellable mid-execution; a long `references_for_offset` scan blocks subsequent `didChange` notifications. Acceptable in v0.2-M5 thin-slice dispatch model (single-threaded; in-flight completes before mutations). Trigger: a user reports typing-freeze during a cross-file references scan, OR v0.3+ multi-window editing surfaces measurable contention. Fix: investigate salsa snapshot API (`Snapshot<CompilerDb>`) for concurrent reads; main loop retains `&mut` for mutations, references/refs/rename run on snapshots. Cross-reference: `design/lsp.md` "Concurrency model" section, `crates/ynz-lsp/src/server.rs` main dispatch.

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

## webpage-foundation deferred items (2026-05-20)

**[webpage] Font vendoring** — @nuxt/fonts 0.14.0 downloads fonts at build time (not committed to repo). CI needs internet access for Google Fonts. Fix when: DO App Platform build fails network-restricted OR Google Fonts rate-limits CI.
- What: configure @nuxt/fonts to write to `website/public/_fonts/` so font files can be committed
- Why deferred: @nuxt/fonts 0.14.0 doesn't support source-dir output; needs module upgrade or custom Nitro plugin
- Cost to fix: 1 session (research module config, write custom copy script or upgrade module version)
- Trigger: DO build fails, Google Fonts rate-limits, or security policy prohibits outbound build-time requests

**[webpage] CSP for Shiki inline styles** — Any future CSP at DO edge or via `nuxt-security` MUST allow `style-src 'unsafe-inline'` OR migrate Shiki to class-based theming (`{ colorReplacements: ... }` or CSS classes mode). Fix before M7 launch when CSP is configured.
- Trigger: M7 launch — when deploying to production and adding security headers
